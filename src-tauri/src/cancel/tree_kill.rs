//! Cancel registry — tracks live `ProcessControl` handles by run-id and
//! aborts them on user request.
//!
//! Builds on T2: `ProcessControl::abort()` is idempotent and triggers the
//! supervisor's `kill_tree(pid)` (Windows `taskkill /T /F`, Unix `kill -KILL
//! -<pgid>`). Spike S7 verified the Codex 1–2 helper descendants are killed.
//!
//! The registry holds clones (Arc-backed) so a UI Stop button can fire
//! without coordinating with the orchestrator's owning task.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::process::{ProcessControl, ProcessError};

pub type RunId = String;

#[derive(Default)]
pub struct CancelRegistry {
    inner: Mutex<HashMap<RunId, ProcessControl>>,
}

impl CancelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, run_id: impl Into<RunId>, control: ProcessControl) {
        self.inner.lock().unwrap().insert(run_id.into(), control);
    }

    pub fn unregister(&self, run_id: &str) {
        self.inner.lock().unwrap().remove(run_id);
    }

    pub fn pids(&self) -> Vec<u32> {
        self.inner
            .lock()
            .unwrap()
            .values()
            .map(|c| c.pid())
            .collect()
    }

    pub fn count(&self) -> usize {
        self.inner.lock().unwrap().len()
    }

    /// Abort a single run by id. No-op if the run is not registered.
    /// Returns the underlying `abort()` result so callers can log spawn
    /// surface errors; idempotent.
    pub async fn abort_one(&self, run_id: &str) -> Result<(), ProcessError> {
        let control = self.inner.lock().unwrap().get(run_id).cloned();
        match control {
            Some(c) => c.abort().await,
            None => Ok(()),
        }
    }

    /// Abort every registered run. Best-effort: errors from individual
    /// `abort()` calls are collected but each future is awaited.
    /// Returns the number of runs that were issued an abort.
    pub async fn abort_all(&self) -> usize {
        let snapshot: Vec<ProcessControl> =
            self.inner.lock().unwrap().values().cloned().collect();
        let n = snapshot.len();
        for c in snapshot {
            let _ = c.abort().await;
        }
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::traits::{ProcessControl, ProcessControlInner};
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use tokio::sync::{mpsc, watch, Mutex as TokioMutex};

    fn fake_control(pid: u32) -> (ProcessControl, mpsc::Receiver<()>) {
        let (abort_tx, abort_rx) = mpsc::channel::<()>(1);
        let (_exit_tx, exit_rx) = watch::channel(None);
        let inner = ProcessControlInner {
            pid,
            aborted: AtomicBool::new(false),
            abort_tx,
            stdin_tx: TokioMutex::new(None),
            exit_watch: exit_rx,
        };
        (
            ProcessControl {
                inner: Arc::new(inner),
            },
            abort_rx,
        )
    }

    #[tokio::test]
    async fn abort_one_signals_supervisor() {
        let reg = CancelRegistry::new();
        let (ctl, mut rx) = fake_control(1234);
        reg.register("run-a", ctl);
        assert_eq!(reg.count(), 1);
        reg.abort_one("run-a").await.unwrap();
        // The supervisor channel should have received the abort signal.
        assert!(rx.recv().await.is_some());
    }

    #[tokio::test]
    async fn abort_all_fires_every_handle() {
        let reg = CancelRegistry::new();
        let (c1, mut r1) = fake_control(1);
        let (c2, mut r2) = fake_control(2);
        reg.register("a", c1);
        reg.register("b", c2);
        let n = reg.abort_all().await;
        assert_eq!(n, 2);
        assert!(r1.recv().await.is_some());
        assert!(r2.recv().await.is_some());
    }

    #[tokio::test]
    async fn abort_one_unknown_id_is_noop() {
        let reg = CancelRegistry::new();
        // Should not panic / error.
        reg.abort_one("missing").await.unwrap();
    }

    #[tokio::test]
    async fn unregister_removes_entry() {
        let reg = CancelRegistry::new();
        let (c, _) = fake_control(7);
        reg.register("x", c);
        assert_eq!(reg.count(), 1);
        reg.unregister("x");
        assert_eq!(reg.count(), 0);
    }

    #[tokio::test]
    async fn idempotent_abort_does_not_double_signal() {
        let reg = CancelRegistry::new();
        let (c, mut r) = fake_control(9);
        reg.register("y", c);
        reg.abort_one("y").await.unwrap();
        reg.abort_one("y").await.unwrap(); // second is a no-op
        // Only one signal should have been sent.
        assert!(r.recv().await.is_some());
        // Channel buffer is 1 — second recv should not see another value
        // immediately. We use try_recv to assert non-pending state.
        assert!(matches!(
            r.try_recv(),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
                | Err(tokio::sync::mpsc::error::TryRecvError::Disconnected)
        ));
    }
}
