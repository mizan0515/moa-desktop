//! `ProcessRunner` trait + handle types.
//!
//! Design (post-adversarial, see TICKETS/T2-process-runner.md):
//!
//! * A single supervisor task owns the `tokio::process::Child`. The handle
//!   exposes only control channels so concurrent abort / wait / read are
//!   structurally allowed (adversarial blocker fix).
//! * `ProcessControl` is `Clone` (Arc-backed) so the orchestrator can hand a
//!   cancel-only handle to a timer task without giving up the line stream.
//! * `ProcessHandle.lines` is moved out by the adapter into its parser task.
//! * Exit is published via `tokio::sync::watch` so multiple consumers (an
//!   adapter parser + a timeout timer + a UI status pump) can observe it.
//! * The runner classifies a small subset of `ProcessErrorKind` itself:
//!   `cli-missing` (spawn ENOENT), `timeout` (own timer), `killed` (own
//!   abort), `oom` (Windows NTSTATUS only — substring matching is rejected
//!   to avoid false positives). Adapters classify everything else.

use async_trait::async_trait;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch, Mutex};

use super::errors::{ProcessError, ProcessErrorKind};

/// What to do with the child's stdin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StdinPolicy {
    /// Don't pipe stdin — child inherits the parent's null device.
    #[default]
    Null,
    /// Pipe stdin and immediately close it (Codex requirement, S2).
    CloseImmediately,
    /// Pipe stdin and let the caller `write_stdin()` / `close_stdin()` (Claude
    /// requirement, S1).
    Pipe,
}

#[derive(Debug, Clone)]
pub struct ProcessSpec {
    pub argv: Vec<String>,
    pub cwd: PathBuf,
    /// Plugin-specific env. Applied AFTER `env_inherit`, so adapter-defined
    /// keys win over inherited ones.
    pub env: HashMap<String, String>,
    /// Whitelist of parent-process env vars to inherit into the child after
    /// the runner calls `env_clear()`. On Windows the CLI workers (Claude /
    /// Codex) need USERPROFILE / APPDATA / PATH / PATHEXT / SystemRoot etc.
    /// to find auth, npm shims, and PowerShell — `env_clear` without a
    /// re-inherit step makes them either fail to spawn or silently lose
    /// their config. Caller may extend or replace via `with_env_inherit`.
    pub env_inherit: Vec<String>,
    pub stdin: StdinPolicy,
    /// Bounded line buffer between supervisor and consumer. Default 1024.
    /// When the consumer falls behind, the readers backpressure naturally
    /// (this matches Unix pipe semantics; adapter is contractually required
    /// to drain).
    pub line_buf: usize,
    /// Maximum bytes per emitted line before forced split with `partial=true`.
    /// Default 1 MiB.
    pub max_line_bytes: usize,
    /// Stderr ring buffer cap, used as `ProcessExit.stderr_tail`. Default
    /// 64 KiB.
    pub stderr_tail_bytes: usize,
}

impl ProcessSpec {
    pub fn new(argv: Vec<String>, cwd: PathBuf) -> Self {
        Self {
            argv,
            cwd,
            env: HashMap::new(),
            env_inherit: default_inherit_keys(),
            stdin: StdinPolicy::Null,
            line_buf: 1024,
            max_line_bytes: 1024 * 1024,
            stderr_tail_bytes: 64 * 1024,
        }
    }

    pub fn with_env(mut self, env: HashMap<String, String>) -> Self {
        self.env = env;
        self
    }

    /// Replace the inherit whitelist. Pass an empty Vec for the previous
    /// `env_clear`-only behavior (typically only useful in tests that pin
    /// the child env exactly).
    pub fn with_env_inherit(mut self, keys: Vec<String>) -> Self {
        self.env_inherit = keys;
        self
    }

    pub fn with_stdin(mut self, stdin: StdinPolicy) -> Self {
        self.stdin = stdin;
        self
    }
}

/// OS-specific default whitelist. On Windows the CLI workers refuse to
/// run without these (auth lookup, npm shim resolution, PowerShell exec).
/// On Unix we keep it minimal — most tools only need PATH / HOME / locale.
///
/// Trust note: `ComSpec` is forwarded as-is. The launcher chain is part
/// of the trusted environment — if the parent shell pointed `ComSpec`
/// at a wrapper, the child sees that wrapper. Callers that need a
/// hardened shell can override via `with_env` or by stripping `ComSpec`
/// from the inherit list with `with_env_inherit`.
pub fn default_inherit_keys() -> Vec<String> {
    let raw: &[&str] = if cfg!(windows) {
        &[
            "PATH",
            "PATHEXT",
            "USERPROFILE",
            "APPDATA",
            "LOCALAPPDATA",
            "SystemRoot",
            "SYSTEMDRIVE",
            "TEMP",
            "TMP",
            "ComSpec",
            "HOMEDRIVE",
            "HOMEPATH",
            "USERNAME",
            "PROGRAMDATA",
            "PROGRAMFILES",
            "PROGRAMFILES(X86)",
        ]
    } else {
        &["PATH", "HOME", "USER", "LANG", "LC_ALL", "TMPDIR", "SHELL"]
    };
    raw.iter().map(|s| s.to_string()).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Stream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessLine {
    pub seq: u64,
    pub stream: Stream,
    pub line: String,
    /// `true` when emitted by force-split (line exceeded `max_line_bytes`)
    /// or by EOF before a newline arrived.
    pub partial: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessExit {
    /// Real exit code if the process exited naturally; `None` if killed
    /// before reporting one.
    pub code: Option<i32>,
    pub aborted: bool,
    pub timed_out: bool,
    /// Last N bytes of stderr (cap = `ProcessSpec.stderr_tail_bytes`). Raw,
    /// unredacted — adapters need fidelity for protocol parsing.
    pub stderr_tail: String,
    pub kind: Option<ProcessErrorKind>,
}

impl ProcessExit {
    /// Did the run succeed by adapter-neutral standards?
    /// (exit code 0, not aborted, not timed out)
    pub fn is_clean(&self) -> bool {
        self.code == Some(0) && !self.aborted && !self.timed_out
    }
}

/// Stdin command channel payload.
///
/// Public so out-of-crate fake runners (integration tests, future
/// benchmark harnesses) can construct a `ProcessControlInner` directly
/// without re-implementing the full supervisor.
#[derive(Debug)]
pub enum StdinCommand {
    Write(Vec<u8>),
    Close,
}

/// Clonable control surface — abort, write_stdin, query pid, wait for exit.
///
/// Multiple holders may call `abort()` concurrently; only the first wins
/// (idempotent via `AtomicBool`).
#[derive(Clone)]
pub struct ProcessControl {
    pub inner: Arc<ProcessControlInner>,
}

pub struct ProcessControlInner {
    pub pid: u32,
    pub aborted: AtomicBool,
    pub abort_tx: mpsc::Sender<()>,
    /// Set to `true` by `wait(Some(d))` immediately before the timer-induced
    /// abort fires, so the supervisor classifies the resulting exit as
    /// `Timeout` (not `Killed`). Shared with the supervisor task.
    pub timed_out_pending: Arc<AtomicBool>,
    pub stdin_tx: Mutex<Option<mpsc::Sender<StdinCommand>>>,
    pub exit_watch: watch::Receiver<Option<ProcessExit>>,
}

impl ProcessControl {
    pub fn pid(&self) -> u32 {
        self.inner.pid
    }

    pub fn aborted(&self) -> bool {
        self.inner.aborted.load(Ordering::SeqCst)
    }

    /// Idempotent. Returns `Ok(())` even if already aborted or already exited.
    pub async fn abort(&self) -> Result<(), ProcessError> {
        if self.inner.aborted.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        // Best-effort signal to the supervisor. If the channel is closed, the
        // supervisor has already finished — that's fine.
        let _ = self.inner.abort_tx.send(()).await;
        Ok(())
    }

    pub async fn write_stdin(&self, bytes: Vec<u8>) -> Result<(), ProcessError> {
        let guard = self.inner.stdin_tx.lock().await;
        let tx = guard
            .as_ref()
            .ok_or_else(|| ProcessError::io("stdin not piped or already closed"))?;
        tx.send(StdinCommand::Write(bytes))
            .await
            .map_err(|_| ProcessError::io("supervisor stdin channel closed"))
    }

    pub async fn close_stdin(&self) -> Result<(), ProcessError> {
        let mut guard = self.inner.stdin_tx.lock().await;
        if let Some(tx) = guard.take() {
            let _ = tx.send(StdinCommand::Close).await;
        }
        Ok(())
    }

    /// Wait for natural exit, optionally bounded by `timeout`. On timeout,
    /// the runner aborts the child and the returned `ProcessExit` carries
    /// `timed_out = true`.
    pub async fn wait(&self, timeout: Option<Duration>) -> Result<ProcessExit, ProcessError> {
        let mut rx = self.inner.exit_watch.clone();
        let wait_fut = async move {
            // Already published?
            if let Some(e) = rx.borrow().clone() {
                return Ok::<ProcessExit, ProcessError>(e);
            }
            loop {
                if rx.changed().await.is_err() {
                    return Err(ProcessError::supervisor_dropped());
                }
                if let Some(e) = rx.borrow().clone() {
                    return Ok(e);
                }
            }
        };
        match timeout {
            None => wait_fut.await,
            Some(d) => match tokio::time::timeout(d, wait_fut).await {
                Ok(r) => r,
                Err(_) => {
                    // Timer fired first — set the shared `timed_out_pending`
                    // flag BEFORE sending abort, so the supervisor records
                    // `timed_out=true` in the published `ProcessExit` once.
                    // This means EVERY waiter (this caller, sibling
                    // `wait()` calls, the watch receiver in T7's
                    // orchestrator) sees a consistent classification —
                    // we no longer mutate a single caller's local copy
                    // (defect B-2 fix).
                    self.inner
                        .timed_out_pending
                        .store(true, Ordering::SeqCst);
                    let _ = self.abort().await;
                    let mut rx2 = self.inner.exit_watch.clone();
                    let final_fut = async move {
                        if let Some(e) = rx2.borrow().clone() {
                            return Ok::<ProcessExit, ProcessError>(e);
                        }
                        loop {
                            if rx2.changed().await.is_err() {
                                return Err(ProcessError::supervisor_dropped());
                            }
                            if let Some(e) = rx2.borrow().clone() {
                                return Ok(e);
                            }
                        }
                    };
                    final_fut.await
                }
            },
        }
    }
}

/// What `spawn()` returns. The adapter typically destructures:
///
/// ```ignore
/// let ProcessHandle { control, mut lines } = runner.spawn(spec).await?;
/// ```
pub struct ProcessHandle {
    pub control: ProcessControl,
    pub lines: mpsc::Receiver<ProcessLine>,
}

#[async_trait]
pub trait ProcessRunner: Send + Sync {
    async fn spawn(&self, spec: ProcessSpec) -> Result<ProcessHandle, ProcessError>;
}
