//! FIX-C — session_id collision + emit-ordering regression tests.
//!
//! Two races handled together:
//!   1. `session_id` was `orch-{ms}` only — two `orch_start` calls within
//!      the same millisecond produced identical sids and the second silently
//!      overwrote the first in `OrchestrationCoordinator.sessions`.
//!   2. The driver task emits `session_start` immediately on spawn. Without
//!      a handshake, this can race ahead of the frontend store's session
//!      insert. Backend now waits on a oneshot ack before the first emit.
//!
//! These tests stay below the AppHandle layer so they do not need a Tauri
//! mock — they exercise `OrchestrationCoordinator` and the new id generator
//! directly. End-to-end emit ordering is covered on the frontend side
//! (`src/lib/orchestrator/__tests__/sessions.test.ts`).

use std::collections::HashSet;
use std::sync::Arc;

use moa_desktop_lib::orchestrator::OrchestrationCoordinator;
use tokio::task::JoinSet;

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn new_session_id_is_unique_under_high_contention() {
    // 500 concurrent ids — `orch-{ms}` alone collides ~always inside a ms.
    const N: usize = 500;
    let mut tasks = JoinSet::new();
    for _ in 0..N {
        tasks.spawn(async { OrchestrationCoordinator::new_session_id_for_test() });
    }
    let mut seen: HashSet<String> = HashSet::with_capacity(N);
    while let Some(res) = tasks.join_next().await {
        let sid = res.unwrap();
        assert!(seen.insert(sid.clone()), "collision: {sid}");
    }
    assert_eq!(seen.len(), N, "expected {N} unique sids");
}

#[tokio::test]
async fn ack_releases_pending_driver_exactly_once() {
    let coord = Arc::new(OrchestrationCoordinator::new());
    let sid = OrchestrationCoordinator::new_session_id_for_test();
    let rx = coord.register_with_ack(&sid).await;

    // First ack delivers.
    assert!(coord.ack(&sid).await, "first ack should succeed");
    assert!(rx.await.is_ok(), "driver should observe ack");

    // Second ack is a no-op (already taken).
    assert!(!coord.ack(&sid).await, "second ack must be idempotent no-op");
}

#[tokio::test]
async fn ack_unknown_session_is_false() {
    let coord = OrchestrationCoordinator::new();
    assert!(!coord.ack("orch-doesnotexist-0").await);
}
