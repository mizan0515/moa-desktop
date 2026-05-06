//! `MockRunner` — file-backed `ProcessRunner` for `settings.mockMode = true`.
//!
//! Reads a canned JSONL file and emits each non-empty line as a `ProcessLine`
//! on stdout with a fixed inter-line delay (default 100 ms) to imitate real
//! streaming. No child process is spawned. Exit is published as code 0 once
//! all lines have been emitted, matching the `ProcessRunner` contract used by
//! T2 / T5 adapters.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::{mpsc, watch, Mutex};

use crate::process::traits::ProcessControlInner;
use crate::process::{
    ProcessControl, ProcessError, ProcessErrorKind, ProcessExit, ProcessHandle, ProcessLine,
    ProcessRunner, ProcessSpec, Stream,
};

/// Default inter-line delay used when streaming canned output.
pub const DEFAULT_LINE_DELAY: Duration = Duration::from_millis(100);

/// File-backed mock runner. Each `spawn` reads `path` fresh and streams its
/// non-empty lines as stdout `ProcessLine`s spaced by `delay`.
#[derive(Clone)]
pub struct MockRunner {
    path: PathBuf,
    delay: Duration,
}

impl MockRunner {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            delay: DEFAULT_LINE_DELAY,
        }
    }

    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[async_trait]
impl ProcessRunner for MockRunner {
    async fn spawn(&self, spec: ProcessSpec) -> Result<ProcessHandle, ProcessError> {
        // Match TokioProcessRunner contract (process/runner.rs:34) so mock-mode
        // smoke tests catch the same caller bugs production would.
        if spec.argv.is_empty() {
            return Err(ProcessError::empty_argv());
        }

        let raw = tokio::fs::read_to_string(&self.path)
            .await
            .map_err(|e| ProcessError::io(&format!("read mock file {:?}: {e}", self.path)))?;

        let lines: Vec<String> = raw
            .lines()
            .map(|l| l.trim_end_matches('\r').to_string())
            .filter(|l| !l.is_empty())
            .collect();

        let line_buf = spec.line_buf.max(1);
        let (line_tx, line_rx) = mpsc::channel::<ProcessLine>(line_buf);
        let (abort_tx, mut abort_rx) = mpsc::channel::<()>(1);
        let (exit_tx, exit_rx) = watch::channel::<Option<ProcessExit>>(None);

        // Mock has no real PID; expose a deterministic stand-in. Adapters that
        // care about pid uniqueness should track by `ProcessSpec` identity.
        let inner = Arc::new(ProcessControlInner {
            pid: 0,
            aborted: AtomicBool::new(false),
            abort_tx,
            stdin_tx: Mutex::new(None),
            exit_watch: exit_rx,
        });
        let control = ProcessControl { inner };

        let delay = self.delay;

        tokio::spawn(async move {
            let mut seq: u64 = 0;
            let mut was_aborted = false;
            for line in lines {
                tokio::select! {
                    _ = tokio::time::sleep(delay) => {}
                    _ = abort_rx.recv() => {
                        was_aborted = true;
                        break;
                    }
                }
                let pl = ProcessLine {
                    seq,
                    stream: Stream::Stdout,
                    line,
                    partial: false,
                };
                seq += 1;
                if line_tx.send(pl).await.is_err() {
                    was_aborted = true;
                    break;
                }
            }
            drop(line_tx);

            let exit = ProcessExit {
                code: if was_aborted { None } else { Some(0) },
                aborted: was_aborted,
                timed_out: false,
                stderr_tail: String::new(),
                kind: if was_aborted {
                    Some(ProcessErrorKind::Killed)
                } else {
                    None
                },
            };
            let _ = exit_tx.send(Some(exit));
        });

        Ok(ProcessHandle {
            control,
            lines: line_rx,
        })
    }
}
