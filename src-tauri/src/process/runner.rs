//! `TokioProcessRunner` — the production `ProcessRunner` implementation.
//!
//! See `traits.rs` for design rationale.

use std::io::ErrorKind;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};
use tokio::sync::{mpsc, watch, Mutex};

use super::errors::{is_windows_oom_exit, ProcessError, ProcessErrorKind};
use super::kill;
use super::traits::{
    ProcessControl, ProcessControlInner, ProcessExit, ProcessHandle, ProcessLine, ProcessRunner,
    ProcessSpec, StdinCommand, StdinPolicy, Stream,
};

#[derive(Default, Clone, Copy)]
pub struct TokioProcessRunner;

impl TokioProcessRunner {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ProcessRunner for TokioProcessRunner {
    async fn spawn(&self, spec: ProcessSpec) -> Result<ProcessHandle, ProcessError> {
        if spec.argv.is_empty() {
            return Err(ProcessError::empty_argv());
        }

        let program = spec.argv[0].clone();
        let mut cmd = Command::new(&program);
        cmd.args(&spec.argv[1..]);
        cmd.current_dir(&spec.cwd);
        cmd.env_clear();
        for (k, v) in &spec.env {
            cmd.env(k, v);
        }
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        match spec.stdin {
            StdinPolicy::Null => {
                cmd.stdin(Stdio::null());
            }
            StdinPolicy::CloseImmediately | StdinPolicy::Pipe => {
                cmd.stdin(Stdio::piped());
            }
        }
        cmd.kill_on_drop(true);
        // Unix: place child in its own process group so a future SIGKILL to
        // -pid reaches the whole group. Windows: no equivalent in std/tokio
        // — taskkill /T handles descendant traversal at cancel time.
        #[cfg(unix)]
        {
            cmd.process_group(0);
        }

        let mut child = cmd.spawn().map_err(|e| match e.kind() {
            ErrorKind::NotFound => ProcessError::cli_missing(&program, e),
            _ => ProcessError {
                kind: ProcessErrorKind::CliMissing,
                message: format!("spawn {program:?}: {e}"),
                exit_code: None,
                stderr_tail: String::new(),
            },
        })?;

        let pid = child
            .id()
            .ok_or_else(|| ProcessError::io("child PID unavailable immediately after spawn"))?;

        // Take handles BEFORE the supervisor does anything else. Tokio's
        // `child.wait()` may close stdin to avoid deadlocks; taking it first
        // means our explicit stdin lifecycle wins.
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let stdin = child.stdin.take();

        // Channels.
        let (lines_tx, lines_rx) = mpsc::channel::<ProcessLine>(spec.line_buf.max(1));
        let (abort_tx, abort_rx) = mpsc::channel::<()>(1);
        let (exit_tx, exit_rx) = watch::channel::<Option<ProcessExit>>(None);

        let (stdin_tx_opt, stdin_rx_opt) = match spec.stdin {
            StdinPolicy::Pipe => {
                let (tx, rx) = mpsc::channel::<StdinCommand>(8);
                (Some(tx), Some(rx))
            }
            _ => (None, None),
        };

        let aborted = Arc::new(AtomicBool::new(false));
        let stderr_tail_bytes = spec.stderr_tail_bytes;
        let max_line = spec.max_line_bytes;

        // Supervisor.
        let aborted_sup = aborted.clone();
        tokio::spawn(supervisor_task(SupervisorArgs {
            pid,
            child,
            stdout,
            stderr,
            stdin,
            stdin_policy: spec.stdin,
            stdin_rx: stdin_rx_opt,
            lines_tx,
            abort_rx,
            exit_tx,
            aborted: aborted_sup,
            stderr_tail_cap: stderr_tail_bytes,
            max_line_bytes: max_line,
        }));

        let inner = ProcessControlInner {
            pid,
            aborted: AtomicBool::new(false), // local view; supervisor owns truth
            abort_tx,
            stdin_tx: Mutex::new(stdin_tx_opt),
            exit_watch: exit_rx,
        };
        // Mirror aborted state by polling supervisor's atomic. We choose to
        // give `ProcessControl` its own AtomicBool gating idempotency at the
        // caller side (cheaper than an Arc share); the supervisor uses its
        // own atomic for internal logic. This is correct because both paths
        // converge to the same `kill_tree` action and the supervisor's wait
        // is the single source of truth for `ProcessExit.aborted`.
        let _ = aborted; // released
        let control = ProcessControl {
            inner: Arc::new(inner),
        };

        Ok(ProcessHandle {
            control,
            lines: lines_rx,
        })
    }
}

struct SupervisorArgs {
    pid: u32,
    child: Child,
    stdout: Option<ChildStdout>,
    stderr: Option<ChildStderr>,
    stdin: Option<ChildStdin>,
    stdin_policy: StdinPolicy,
    stdin_rx: Option<mpsc::Receiver<StdinCommand>>,
    lines_tx: mpsc::Sender<ProcessLine>,
    abort_rx: mpsc::Receiver<()>,
    exit_tx: watch::Sender<Option<ProcessExit>>,
    aborted: Arc<AtomicBool>,
    stderr_tail_cap: usize,
    max_line_bytes: usize,
}

async fn supervisor_task(args: SupervisorArgs) {
    let SupervisorArgs {
        pid,
        mut child,
        stdout,
        stderr,
        stdin,
        stdin_policy,
        stdin_rx,
        lines_tx,
        mut abort_rx,
        exit_tx,
        aborted,
        stderr_tail_cap,
        max_line_bytes,
    } = args;

    let seq = Arc::new(AtomicU64::new(0));
    let stderr_tail: Arc<Mutex<RingTail>> = Arc::new(Mutex::new(RingTail::new(stderr_tail_cap)));

    // stdout reader.
    let stdout_handle = if let Some(s) = stdout {
        let tx = lines_tx.clone();
        let seq2 = seq.clone();
        Some(tokio::spawn(read_stream(
            s,
            Stream::Stdout,
            tx,
            seq2,
            None,
            max_line_bytes,
        )))
    } else {
        None
    };

    // stderr reader.
    let stderr_handle = if let Some(s) = stderr {
        let tx = lines_tx.clone();
        let seq2 = seq.clone();
        let tail = stderr_tail.clone();
        Some(tokio::spawn(read_stream(
            s,
            Stream::Stderr,
            tx,
            seq2,
            Some(tail),
            max_line_bytes,
        )))
    } else {
        None
    };

    drop(lines_tx); // close after readers finish

    // stdin handling.
    let stdin_handle = match (stdin_policy, stdin, stdin_rx) {
        (StdinPolicy::CloseImmediately, Some(sin), _) => {
            // Just drop it.
            drop(sin);
            None
        }
        (StdinPolicy::Pipe, Some(mut sin), Some(mut rx)) => Some(tokio::spawn(async move {
            while let Some(cmd) = rx.recv().await {
                match cmd {
                    StdinCommand::Write(bytes) => {
                        if sin.write_all(&bytes).await.is_err() {
                            break;
                        }
                        let _ = sin.flush().await;
                    }
                    StdinCommand::Close => break,
                }
            }
            // Dropping sin closes the pipe.
        })),
        _ => None,
    };

    // Race child.wait() vs abort_rx.
    let exit_state = tokio::select! {
        biased;
        _ = abort_rx.recv() => {
            if !aborted.swap(true, Ordering::SeqCst) {
                let _ = kill::kill_tree(pid).await;
            }
            // Reap. kill_on_drop=true is a backstop, but we want the status.
            let status = child.wait().await.ok();
            ExitState {
                code: status.and_then(|s| s.code()),
                aborted: true,
                timed_out: false,
            }
        }
        status = child.wait() => {
            ExitState {
                code: status.ok().and_then(|s| s.code()),
                aborted: false,
                timed_out: false,
            }
        }
    };

    // Drain readers — pipes have closed at this point.
    if let Some(h) = stdout_handle {
        let _ = h.await;
    }
    if let Some(h) = stderr_handle {
        let _ = h.await;
    }
    if let Some(h) = stdin_handle {
        h.abort();
    }

    let stderr_tail_str = stderr_tail.lock().await.snapshot_lossy();
    let kind = classify_exit(&exit_state, stderr_tail_str.as_str());

    let exit = ProcessExit {
        code: exit_state.code,
        aborted: exit_state.aborted,
        timed_out: exit_state.timed_out,
        stderr_tail: stderr_tail_str,
        kind,
    };

    let _ = exit_tx.send(Some(exit));
}

#[derive(Clone, Copy)]
struct ExitState {
    code: Option<i32>,
    aborted: bool,
    timed_out: bool,
}

fn classify_exit(state: &ExitState, _stderr_tail: &str) -> Option<ProcessErrorKind> {
    if state.aborted {
        return Some(ProcessErrorKind::Killed);
    }
    if let Some(code) = state.code {
        if is_windows_oom_exit(code) {
            return Some(ProcessErrorKind::Oom);
        }
    }
    // Adapter is responsible for auth-expired / quota / network /
    // sandbox-denied / malformed-json / test-fail classification. The runner
    // intentionally does not pattern-match stderr substrings.
    None
}

/// Read a stream line-by-line via `read_until('\n')`. Emits `partial=true`
/// when:
///   * EOF arrives before a newline, OR
///   * A line exceeds `max_line_bytes` and is force-split.
async fn read_stream<R>(
    reader: R,
    stream: Stream,
    tx: mpsc::Sender<ProcessLine>,
    seq: Arc<AtomicU64>,
    tail: Option<Arc<Mutex<RingTail>>>,
    max_line_bytes: usize,
) where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    let mut reader = BufReader::new(reader);
    let mut buf: Vec<u8> = Vec::with_capacity(8192);

    loop {
        buf.clear();
        let mut got_newline = false;
        let mut overflowed = false;

        // Read until newline OR max_line_bytes.
        loop {
            let mut byte = [0u8; 1];
            match reader.read(&mut byte).await {
                Ok(0) => break, // EOF
                Ok(_) => {
                    let b = byte[0];
                    if let Some(t) = &tail {
                        t.lock().await.push(b);
                    }
                    if b == b'\n' {
                        got_newline = true;
                        break;
                    }
                    if buf.len() >= max_line_bytes {
                        overflowed = true;
                        // emit and continue with a fresh buffer for the rest of
                        // the long line (will keep overflowing until newline).
                        buf.push(b);
                        break;
                    }
                    buf.push(b);
                }
                Err(_) => return,
            }
        }

        if buf.is_empty() && !got_newline && !overflowed {
            // Genuine EOF, no trailing fragment.
            return;
        }

        // Strip trailing CR if present and we got a newline.
        let mut content = buf.clone();
        if got_newline {
            if let Some(&b'\r') = content.last() {
                content.pop();
            }
        }

        let line = String::from_utf8_lossy(&content).into_owned();
        let partial = !got_newline; // either EOF mid-line or overflow split

        let next = seq.fetch_add(1, Ordering::SeqCst);
        if tx
            .send(ProcessLine {
                seq: next,
                stream,
                line,
                partial,
            })
            .await
            .is_err()
        {
            return;
        }

        if !got_newline && !overflowed {
            // EOF reached on a partial line — done.
            return;
        }
    }
}

/// Bounded byte ring buffer used to keep the last N bytes of stderr.
struct RingTail {
    cap: usize,
    buf: std::collections::VecDeque<u8>,
}

impl RingTail {
    fn new(cap: usize) -> Self {
        Self {
            cap,
            buf: std::collections::VecDeque::with_capacity(cap.min(64 * 1024)),
        }
    }

    fn push(&mut self, b: u8) {
        if self.cap == 0 {
            return;
        }
        if self.buf.len() == self.cap {
            self.buf.pop_front();
        }
        self.buf.push_back(b);
    }

    fn snapshot_lossy(&self) -> String {
        let bytes: Vec<u8> = self.buf.iter().copied().collect();
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_tail_caps() {
        let mut r = RingTail::new(4);
        for b in b"abcdefg" {
            r.push(*b);
        }
        assert_eq!(r.snapshot_lossy(), "defg");
    }

    #[test]
    fn ring_tail_zero_cap_is_noop() {
        let mut r = RingTail::new(0);
        for b in b"abc" {
            r.push(*b);
        }
        assert_eq!(r.snapshot_lossy(), "");
    }
}
