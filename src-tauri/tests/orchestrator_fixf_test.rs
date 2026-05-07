//! FIX-F regression tests — the orchestrator-level fixes that pure unit
//! tests in `src-tauri/src/orchestrator/adversarial.rs` cannot cover:
//!
//! 1. **Cancel multi-lane**: pre-FIX-F the session held a single
//!    `Mutex<Option<ProcessControl>>` slot. Two concurrent first-pass
//!    lanes overwrote each other's handle and `Cancel` only aborted the
//!    last writer. Now `SessionHandle.active` is a `CancelRegistry` and
//!    `abort_all` walks every registered run-id.
//!
//! 2. **Verify outcome propagation**: pre-FIX-F `run_verify_phase` emitted
//!    the outcome but the driver always wrote `Final { ok: true }`. The
//!    pure check below — `VerifyOutcome::Failed { .. }.is_ok() == false`
//!    — is the load-bearing predicate the driver now consults.
//!
//! Pure verdict-gate decisions are tested in
//! `src-tauri/src/orchestrator/adversarial.rs::tests::decide_*`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use moa_desktop_lib::cancel::CancelRegistry;
use moa_desktop_lib::orchestrator::verify::VerifyOutcome;
use moa_desktop_lib::orchestrator::OrchestrationCoordinator;
use moa_desktop_lib::process::traits::{ProcessControl, ProcessControlInner};
use tokio::sync::{mpsc, watch, Mutex as TokioMutex};

/// Build a fake `ProcessControl` whose internal `abort_tx` channel is
/// drained by a tiny task that bumps a shared counter on each abort.
/// Mirrors `cancel::tree_kill::tests::fake_control` (private) but counts
/// fires across multiple lanes so the multi-lane assertion is sharp.
fn fake_control(pid: u32, abort_counter: Arc<AtomicU32>) -> ProcessControl {
    let (abort_tx, mut abort_rx) = mpsc::channel::<()>(4);
    let (_exit_tx, exit_rx) = watch::channel(None);

    let counter = abort_counter.clone();
    tokio::spawn(async move {
        if abort_rx.recv().await.is_some() {
            counter.fetch_add(1, Ordering::SeqCst);
        }
    });

    ProcessControl {
        inner: Arc::new(ProcessControlInner {
            pid,
            aborted: AtomicBool::new(false),
            abort_tx,
            timed_out_pending: Arc::new(AtomicBool::new(false)),
            stdin_tx: TokioMutex::new(None),
            exit_watch: exit_rx,
        }),
    }
}

#[tokio::test]
async fn cancel_aborts_every_registered_lane_not_just_the_last() {
    // Pre-FIX-F: only ONE lane was aborted (whichever lane wrote `active`
    // last). With Vec<AbortHandle>-equivalent (CancelRegistry), every
    // lane's abort fires.
    let coord = OrchestrationCoordinator::new();
    let registry = Arc::new(CancelRegistry::new());

    let counter = Arc::new(AtomicU32::new(0));
    let claude = fake_control(101, counter.clone());
    let codex = fake_control(202, counter.clone());
    let mutation = fake_control(303, counter.clone());

    registry.register("claude-firstpass", claude);
    registry.register("codex-firstpass", codex);
    registry.register("mutation", mutation);
    assert_eq!(registry.count(), 3, "registry holds three concurrent lanes");

    let sid = OrchestrationCoordinator::new_session_id_for_test();
    let _cmd_rx = coord
        .register_with_active(&sid, registry.clone())
        .await;

    // Fire the cancel — must abort all three.
    assert!(coord.cancel_for_test(&sid).await, "cancel must dispatch");

    // Give the supervisor task a chance to record the aborts.
    tokio::time::timeout(std::time::Duration::from_millis(500), async {
        loop {
            if counter.load(Ordering::SeqCst) >= 3 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("all three abort_tx receivers should fire within 500ms");

    assert_eq!(
        counter.load(Ordering::SeqCst),
        3,
        "expected every registered lane to receive abort, not just the last writer"
    );
}

#[tokio::test]
async fn cancel_with_no_lanes_is_a_noop_not_a_panic() {
    let coord = OrchestrationCoordinator::new();
    let registry = Arc::new(CancelRegistry::new());
    let sid = OrchestrationCoordinator::new_session_id_for_test();
    let _cmd_rx = coord.register_with_active(&sid, registry.clone()).await;
    assert!(coord.cancel_for_test(&sid).await);
    assert_eq!(registry.count(), 0);
}

#[test]
fn verify_failed_is_not_ok_and_drives_session_ok_false() {
    // FIX-F — driver reads `outcome.is_ok()` to set `Final { ok }`.
    let failed = VerifyOutcome::Failed {
        exit_code: Some(1),
        duration_ms: 12,
        stdout_tail: String::new(),
        stderr_tail: "compile error".into(),
    };
    assert!(!failed.is_ok(), "failed verify must propagate ok=false");

    let timed_out = VerifyOutcome::TimedOut { after_secs: 300 };
    assert!(!timed_out.is_ok(), "timed-out verify must propagate ok=false");

    let passed = VerifyOutcome::Passed {
        duration_ms: 5,
        stdout_tail: "ok".into(),
    };
    assert!(passed.is_ok());

    // Skipped (no verify command) keeps ok=true so users without a
    // `verify_cmd` aren't punished with a permanent red badge.
    assert!(VerifyOutcome::Skipped.is_ok());
}
