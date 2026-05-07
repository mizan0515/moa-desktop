//! Lane supervisor + panic boundary.
//!
//! Wraps a `tokio::spawn`'d future so that:
//! - panics are caught and surfaced as `LaneError::Panic` (not propagated to
//!   the runtime, never crash the app).
//! - on `Drop`, the abort handle is fired — the task is cancelled even if
//!   the supervisor is dropped without `await`ing.
//! - cleanup hooks (child process abort, lock release) run when the task
//!   finishes — owned data inside the future is dropped, releasing T2
//!   `ProcessControl` (kill_on_drop=true) and T4 `LaneGuard` (RAII Drop).
//!
//! Unit-tested by `lane_panic_does_not_kill_app` — see tests below.

use std::fmt;

use tokio::task::{AbortHandle, JoinError, JoinHandle};

#[derive(Debug)]
pub enum LaneError {
    /// Lane future panicked. Inner string is the best-effort panic message.
    Panic(String),
    /// Lane was aborted via `LaneSupervisor::abort()` or supervisor drop.
    Aborted,
    /// Task ran to completion but the inner result was an error.
    Inner(String),
}

impl fmt::Display for LaneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LaneError::Panic(m) => write!(f, "lane panic: {m}"),
            LaneError::Aborted => write!(f, "lane aborted"),
            LaneError::Inner(m) => write!(f, "lane error: {m}"),
        }
    }
}

impl std::error::Error for LaneError {}

/// Owns a `JoinHandle` plus the abort handle so dropping the supervisor
/// fires `.abort()`. Callers are expected to `await` `into_outcome()` for
/// the structured result.
///
/// **Backlog #20 (pending)** — AppHandle mock 기반 end-to-end 통합 테스트 부재.
/// 본 구조체 또는 AppHandle 의존부 변경 전에 #20 결정 트리 확인.
pub struct LaneSupervisor<T> {
    join: Option<JoinHandle<T>>,
    abort: AbortHandle,
}

impl<T> LaneSupervisor<T>
where
    T: Send + 'static,
{
    /// Spawn `fut` under a panic boundary. Returns immediately.
    pub fn spawn<F>(fut: F) -> Self
    where
        F: std::future::Future<Output = T> + Send + 'static,
    {
        let join = tokio::spawn(fut);
        let abort = join.abort_handle();
        Self {
            join: Some(join),
            abort,
        }
    }

    /// Force-abort the lane. Idempotent. Returns immediately; the
    /// JoinHandle's await side will see `JoinError::is_cancelled() = true`.
    pub fn abort(&self) {
        self.abort.abort();
    }

    /// Await the lane to completion. Maps panics to `LaneError::Panic` and
    /// aborts to `LaneError::Aborted`. Inner success bubbles up as `Ok(T)`.
    pub async fn into_outcome(mut self) -> Result<T, LaneError> {
        let join = match self.join.take() {
            Some(h) => h,
            None => return Err(LaneError::Aborted),
        };
        match join.await {
            Ok(v) => Ok(v),
            Err(e) => Err(map_join_error(e)),
        }
    }

    /// Test/diag helper — true if the underlying task is still running.
    pub fn is_running(&self) -> bool {
        self.join
            .as_ref()
            .map(|h| !h.is_finished())
            .unwrap_or(false)
    }
}

fn map_join_error(e: JoinError) -> LaneError {
    if e.is_cancelled() {
        return LaneError::Aborted;
    }
    if e.is_panic() {
        // try_into_panic gives us the boxed Any. Best-effort downcast to
        // `&str` / `String` — that's how `panic!("text")` lays it out.
        let any = e.into_panic();
        let msg = panic_message(&any).unwrap_or_else(|| "unknown panic payload".into());
        return LaneError::Panic(msg);
    }
    LaneError::Inner(format!("join error: {e}"))
}

fn panic_message(any: &Box<dyn std::any::Any + Send>) -> Option<String> {
    if let Some(s) = any.downcast_ref::<&'static str>() {
        return Some((*s).to_string());
    }
    if let Some(s) = any.downcast_ref::<String>() {
        return Some(s.clone());
    }
    None
}

impl<T> Drop for LaneSupervisor<T> {
    fn drop(&mut self) {
        // Always fire abort. If task already finished this is a no-op.
        self.abort.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    /// Core requirement from T7-full ticket: a lane panic must not kill the
    /// app. After one lane panics, the supervisor pattern continues to
    /// produce healthy results from other lanes.
    #[tokio::test]
    async fn lane_panic_does_not_kill_app() {
        let sup_panic: LaneSupervisor<i32> = LaneSupervisor::spawn(async {
            panic!("simulated lane panic");
        });
        let result = sup_panic.into_outcome().await;
        match result {
            Err(LaneError::Panic(msg)) => {
                assert!(msg.contains("simulated lane panic"), "got: {msg}")
            }
            other => panic!("expected Panic, got {other:?}"),
        }

        // App is fine — a fresh lane works.
        let sup_ok: LaneSupervisor<i32> = LaneSupervisor::spawn(async { 42 });
        let v = sup_ok.into_outcome().await.unwrap();
        assert_eq!(v, 42);

        // And many concurrent lanes — including more panics interleaved —
        // do not destabilize the runtime.
        let counter = Arc::new(AtomicU32::new(0));
        let mut sups = Vec::new();
        for i in 0..20 {
            let c = counter.clone();
            let s: LaneSupervisor<()> = LaneSupervisor::spawn(async move {
                if i % 5 == 0 {
                    panic!("lane {i} panic");
                }
                c.fetch_add(1, Ordering::SeqCst);
            });
            sups.push(s);
        }
        let mut panics = 0;
        let mut oks = 0;
        for s in sups {
            match s.into_outcome().await {
                Ok(()) => oks += 1,
                Err(LaneError::Panic(_)) => panics += 1,
                Err(e) => panic!("unexpected err {e}"),
            }
        }
        assert_eq!(panics, 4, "expected 4 panicking lanes (0,5,10,15)");
        assert_eq!(oks, 16);
        assert_eq!(counter.load(Ordering::SeqCst), 16);
    }

    #[tokio::test]
    async fn drop_aborts_running_task() {
        let counter = Arc::new(AtomicU32::new(0));
        let c = counter.clone();
        let sup: LaneSupervisor<()> = LaneSupervisor::spawn(async move {
            for _ in 0..50 {
                tokio::time::sleep(Duration::from_millis(20)).await;
                c.fetch_add(1, Ordering::SeqCst);
            }
        });
        tokio::time::sleep(Duration::from_millis(30)).await;
        drop(sup);
        // Give runtime a moment to honor abort.
        tokio::time::sleep(Duration::from_millis(60)).await;
        let mid = counter.load(Ordering::SeqCst);
        // Task should have been aborted before completing 50 ticks (~1 s).
        assert!(mid < 10, "task should have been aborted, got {mid} ticks");
    }

    #[tokio::test]
    async fn explicit_abort_yields_aborted_err() {
        let sup: LaneSupervisor<i32> = LaneSupervisor::spawn(async {
            tokio::time::sleep(Duration::from_secs(10)).await;
            999
        });
        sup.abort();
        match sup.into_outcome().await {
            Err(LaneError::Aborted) => {}
            other => panic!("expected Aborted, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn ok_result_passes_through() {
        let sup: LaneSupervisor<Result<u32, String>> = LaneSupervisor::spawn(async { Ok(7) });
        assert_eq!(sup.into_outcome().await.unwrap(), Ok(7));
    }
}
