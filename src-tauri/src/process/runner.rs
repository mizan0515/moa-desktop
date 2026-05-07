//! `TokioProcessRunner` — the production `ProcessRunner` implementation.
//!
//! See `traits.rs` for design rationale.

use std::io::ErrorKind;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

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
        // Re-inherit OS-essential vars from the parent (Windows: USERPROFILE,
        // APPDATA, PATH, PATHEXT, SystemRoot, ...) BEFORE applying spec.env.
        // Without this the worker CLIs spawn-fail or lose auth/config; see
        // adapters/codex.rs CodexConfig.env contract and the Windows CLI
        // smoke notes.
        for key in &spec.env_inherit {
            if let Some(val) = std::env::var_os(key) {
                cmd.env(key, val);
            }
        }
        // Plugin-specific env wins over inherited values.
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
            ErrorKind::PermissionDenied => ProcessError::permission_denied(&program, e),
            _ => ProcessError::spawn_failed(&program, e),
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
        let timed_out_pending = Arc::new(AtomicBool::new(false));
        let stderr_tail_bytes = spec.stderr_tail_bytes;
        let max_line = spec.max_line_bytes;

        // Supervisor.
        let aborted_sup = aborted.clone();
        let timed_out_sup = timed_out_pending.clone();
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
            timed_out_pending: timed_out_sup,
            stderr_tail_cap: stderr_tail_bytes,
            max_line_bytes: max_line,
        }));

        let inner = ProcessControlInner {
            pid,
            aborted: AtomicBool::new(false), // local view; supervisor owns truth
            abort_tx,
            timed_out_pending,
            stdin_tx: Mutex::new(stdin_tx_opt),
            exit_watch: exit_rx,
        };
        let _ = aborted; // released — supervisor owns it
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
    /// Set by `ProcessControl::wait()` when its own timer fires (BEFORE the
    /// abort signal is sent). The supervisor reads this on the abort branch
    /// to mark the resulting `ProcessExit.timed_out` so EVERY waiter sees a
    /// consistent classification (defect B-2 fix).
    timed_out_pending: Arc<AtomicBool>,
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
        timed_out_pending,
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
            // Backstop: kill_tree may fail (taskkill missing/blocked, EACCES,
            // 5s timeout). Send an immediate SIGKILL/TerminateProcess to the
            // direct child via Tokio so we never hang on an unkilled descendant
            // tree (defect B-1 fix). start_kill is non-blocking; we then bound
            // the reap with our own timeout.
            let _ = child.start_kill();
            let status = match tokio::time::timeout(
                Duration::from_secs(5),
                child.wait(),
            ).await {
                Ok(Ok(s)) => Some(s),
                _ => None, // wait timed out or errored — fall through; kill_on_drop=true cleans up
            };
            ExitState {
                code: status.and_then(|s| s.code()),
                aborted: true,
                timed_out: timed_out_pending.load(Ordering::SeqCst),
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
    // timed_out wins over killed: a timeout-induced abort is still semantically
    // a timeout, not an external cancel (defect B-2 fix — every waiter sees
    // the same classification because supervisor publishes once).
    if state.timed_out {
        return Some(ProcessErrorKind::Timeout);
    }
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

/// Read a stream line-by-line. Emits `partial=true` when:
///   * EOF arrives before a newline, OR
///   * A line exceeds `max_line_bytes` and is force-split.
///
/// Force-splits are taken on a UTF-8 character boundary so multibyte chars
/// (e.g. Korean, Japanese, emoji) are never mangled across two emitted
/// `ProcessLine`s (defect B-3 fix). Any incomplete trailing bytes are carried
/// into the next chunk's buffer.
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

    // UTF-8 grace allowance: a single codepoint is at most 4 bytes, so we
    // permit `buf` to exceed `max_line_bytes` by up to 3 bytes when the
    // trailing bytes form an incomplete multibyte sequence. This lets us
    // emit at least one full codepoint per overflow chunk (avoids the
    // empty-partial-on-cut=0 case Codex flagged).
    const UTF8_GRACE: usize = 3;

    loop {
        // `buf` may already hold carry-over bytes from a previous overflow
        // emit (incomplete trailing UTF-8 sequence + at most one pending
        // post-limit byte). Don't clear it.
        let mut got_newline = false;
        let mut overflowed = false;
        // Set when overflow fires on byte `b` that we did NOT push to `buf`
        // (so `buf` stays at <= max_line_bytes + UTF8_GRACE). `b` rides into
        // the next iteration's buf as the first byte after carry.
        let mut pending_byte: Option<u8> = None;

        // Read until newline OR overflow threshold.
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
                    // Overflow check is BEFORE the push so a clean line of
                    // exactly `max_line_bytes` followed by `\n` does not get
                    // misclassified as overflow (Codex c-3 fix).
                    if buf.len() >= max_line_bytes {
                        let cut = utf8_safe_split(&buf);
                        if cut > 0 || buf.len() >= max_line_bytes + UTF8_GRACE {
                            // Either we have at least one complete UTF-8 char
                            // to emit, or we've hit the grace cap and must
                            // force-flush even if the bytes are invalid.
                            overflowed = true;
                            pending_byte = Some(b);
                            break;
                        }
                        // else: still mid-multibyte; let buf grow within
                        // grace so the next pass produces a non-empty emit
                        // (Codex c-1 fix).
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

        // Build the bytes to emit and the carry-over for next iteration.
        let (emit_bytes, mut carry): (Vec<u8>, Vec<u8>) = if got_newline {
            // Strip trailing CR.
            let mut content = std::mem::take(&mut buf);
            if let Some(&b'\r') = content.last() {
                content.pop();
            }
            (content, Vec::new())
        } else if overflowed {
            // Cut at the largest UTF-8-valid prefix to avoid splitting a
            // multibyte char. The trailing incomplete bytes (≤3) ride along
            // into the next chunk so the *next* emit reconstructs the char.
            let cut = utf8_safe_split(&buf);
            let carry = buf.split_off(cut);
            (std::mem::take(&mut buf), carry)
        } else {
            // EOF mid-line.
            (std::mem::take(&mut buf), Vec::new())
        };

        // Append the post-limit byte (if any) so the next iteration starts
        // with `[utf8_carry..., pending_byte]`. This is what enables the
        // CRLF-at-overflow-boundary case to strip CR cleanly: the \r ends
        // up alone in next iter's buf, the \n triggers got_newline, and the
        // existing trailing-CR strip emits "" (Codex c-2 fix — concat of
        // partial+empty yields the line without CR).
        if let Some(p) = pending_byte.take() {
            carry.push(p);
        }

        let line = String::from_utf8_lossy(&emit_bytes).into_owned();
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

        // Re-seed buf with any carry-over so the next iteration starts with
        // the incomplete UTF-8 trailer + pending byte already present.
        buf = carry;
    }
}

/// Largest prefix length of `buf` that is valid UTF-8 (i.e. ends on a
/// character boundary). Used to avoid splitting multibyte sequences when a
/// line overflows `max_line_bytes`.
fn utf8_safe_split(buf: &[u8]) -> usize {
    match std::str::from_utf8(buf) {
        Ok(_) => buf.len(),
        Err(e) => e.valid_up_to(),
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

    #[test]
    fn utf8_safe_split_keeps_only_valid_prefix() {
        // "안" = E5 95 88 — splitting between byte 1 and 2 must roll back to 0.
        let bytes = [0xE5u8, 0x95];
        assert_eq!(utf8_safe_split(&bytes), 0);

        // Complete ASCII prefix + incomplete tail.
        let mut v = b"hello".to_vec();
        v.extend_from_slice(&[0xE5, 0x95]); // half of "안"
        assert_eq!(utf8_safe_split(&v), 5);

        // All valid.
        let s = "안녕".as_bytes();
        assert_eq!(utf8_safe_split(s), s.len());
    }

    #[test]
    fn classify_exit_timeout_wins_over_killed() {
        let st = ExitState {
            code: None,
            aborted: true,
            timed_out: true,
        };
        assert_eq!(classify_exit(&st, ""), Some(ProcessErrorKind::Timeout));
    }

    #[test]
    fn classify_exit_killed_when_aborted_only() {
        let st = ExitState {
            code: None,
            aborted: true,
            timed_out: false,
        };
        assert_eq!(classify_exit(&st, ""), Some(ProcessErrorKind::Killed));
    }

    // ─── read_stream byte-level unit tests (Codex c-1/c-2/c-3 fixes) ────
    //
    // Subprocess-driven integration tests can't pin Windows CRLF semantics
    // tightly enough to assert single-byte boundary behavior. These drive
    // `read_stream` directly with `Cursor<Vec<u8>>` for deterministic input.

    use std::io::Cursor;
    use tokio::sync::mpsc;

    async fn drive_read_stream(
        bytes: Vec<u8>,
        max_line_bytes: usize,
    ) -> Vec<ProcessLine> {
        let (tx, mut rx) = mpsc::channel::<ProcessLine>(64);
        let seq = Arc::new(AtomicU64::new(0));
        let reader = Cursor::new(bytes);
        let join = tokio::spawn(read_stream(
            reader,
            Stream::Stdout,
            tx,
            seq,
            None,
            max_line_bytes,
        ));
        let mut out = Vec::new();
        while let Some(l) = rx.recv().await {
            out.push(l);
        }
        join.await.expect("read_stream join");
        out
    }

    #[tokio::test]
    async fn c1_no_empty_partial_on_zero_utf8_cut() {
        // "안녕" = E5 95 88 EB 85 95 (6 bytes). max=2 forces overflow checks
        // mid-multibyte. The first overflow attempt at len=2 has cut=0; the
        // grace allowance lets buf grow until cut becomes positive (len=3
        // after pushing the 3rd byte completes "안"). No empty partials.
        let bytes = "안녕\n".as_bytes().to_vec();
        let lines = drive_read_stream(bytes, 2).await;
        for l in &lines {
            assert!(
                !(l.line.is_empty() && l.partial),
                "no empty partial chunk allowed: {lines:?}"
            );
        }
        let joined: String = lines.iter().map(|l| l.line.as_str()).collect();
        assert_eq!(joined, "안녕");
        assert!(!joined.contains('\u{FFFD}'));
    }

    #[tokio::test]
    async fn c2_crlf_at_overflow_boundary_strips_cr_in_concat() {
        // "abc\r\n" with max=3: after pushing a,b,c (buf at limit), \r is
        // the post-limit byte → overflow + carry=[\r]. Next iter reads \n,
        // hits got_newline, strips trailing CR from carry, emits "" clean.
        // Concat = "abc" (CR not leaked).
        let bytes = b"abc\r\n".to_vec();
        let lines = drive_read_stream(bytes, 3).await;
        let joined: String = lines.iter().map(|l| l.line.as_str()).collect();
        assert!(
            !joined.contains('\r'),
            "CR must be stripped across the overflow boundary; got {joined:?}, lines={lines:?}"
        );
        assert!(joined.contains("abc"));
    }

    #[tokio::test]
    async fn c3_exact_max_line_bytes_with_lf_is_clean_emit() {
        // "abcd\n" with max=4: 4 content bytes exactly, then \n. Pre-fix
        // (post-push >= check) tagged this as overflow. Post-fix (pre-push
        // check, with newline check first) sees \n on the 5th read while
        // buf.len()=4 and emits "abcd" non-partial.
        let bytes = b"abcd\n".to_vec();
        let lines = drive_read_stream(bytes, 4).await;
        assert_eq!(lines.len(), 1, "single emit expected: {lines:?}");
        assert_eq!(lines[0].line, "abcd");
        assert!(!lines[0].partial, "must be non-partial: {lines:?}");
    }

    #[tokio::test]
    async fn c3_exact_max_line_bytes_with_crlf_is_clean_emit() {
        // "abcd\r\n" with max=4: same as above but with CRLF. The \r is at
        // position 4 (post-limit byte) → overflow path with pending=Some(\r).
        // The next iter sees \n with carry=[\r] → got_newline strips CR.
        // Concat = "abcd". The line IS split into partial+empty (a tradeoff
        // of the boundary case), but the contract guarantees concat
        // correctness, which is what consumers rely on.
        let bytes = b"abcd\r\n".to_vec();
        let lines = drive_read_stream(bytes, 4).await;
        let joined: String = lines.iter().map(|l| l.line.as_str()).collect();
        assert!(
            !joined.contains('\r'),
            "CR must not leak across overflow boundary: {lines:?}"
        );
        assert_eq!(joined, "abcd");
    }

    #[tokio::test]
    async fn long_korean_line_overflow_no_replacement_chars() {
        // "안" repeated 10 times = 30 bytes. max=8 forces ~4 overflow splits
        // mid-multibyte. The fix ensures no U+FFFD replacement chars in the
        // concatenated output and every char is recovered.
        let payload = "안".repeat(10);
        let mut bytes = payload.as_bytes().to_vec();
        bytes.push(b'\n');
        let lines = drive_read_stream(bytes, 8).await;
        let joined: String = lines.iter().map(|l| l.line.as_str()).collect();
        assert_eq!(joined, payload);
        assert!(!joined.contains('\u{FFFD}'));
        for l in &lines {
            assert!(!(l.line.is_empty() && l.partial));
        }
    }

    #[tokio::test]
    async fn eof_mid_line_emits_partial_with_carry() {
        // Carry from a prior overflow plus EOF mid-line: must emit the
        // remaining bytes (decoded lossily if incomplete) as partial.
        let bytes = "안녕".as_bytes().to_vec(); // 6 bytes, no newline
        let lines = drive_read_stream(bytes, 4).await;
        let joined: String = lines.iter().map(|l| l.line.as_str()).collect();
        assert_eq!(joined, "안녕");
        assert!(lines.last().is_some());
    }

    #[tokio::test]
    async fn empty_stream_yields_nothing() {
        let lines = drive_read_stream(Vec::new(), 1024).await;
        assert!(lines.is_empty());
    }
}
