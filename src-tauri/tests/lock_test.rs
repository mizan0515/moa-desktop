//! T4 — lock manager state machine, ordering, transfer, instance lock.

use moa_desktop_lib::git::RepoKey;
use moa_desktop_lib::lock::manager::{LockError, LockManager, LockSource, Worker};
use moa_desktop_lib::lock::InstanceLock;
use tempfile::tempdir;

fn fixture() -> (LockManager, RepoKey) {
    let td = tempdir().unwrap();
    let mgr = LockManager::new();
    let repo = RepoKey::from_path(td.path());
    // Leak td: tests just need a stable path; td drop ok at end.
    Box::leak(Box::new(td));
    (mgr, repo)
}

#[test]
fn ordered_acquire_repo_project_lane() {
    let (mgr, repo) = fixture();
    let r = mgr.acquire_repo(&repo).unwrap();
    let p = mgr.acquire_project(&r, "proj-1").unwrap();
    let lane = mgr
        .acquire_lane(&p, "main", Worker::Claude, LockSource::Scheduler)
        .unwrap();
    assert_eq!(lane.lock_key(), "main");
    assert_eq!(lane.project_id(), "proj-1");
}

#[test]
fn second_repo_acquire_blocks() {
    let (mgr, repo) = fixture();
    let _r = mgr.acquire_repo(&repo).unwrap();
    match mgr.acquire_repo(&repo) {
        Err(LockError::WouldBlock { .. }) => {}
        other => panic!("expected WouldBlock, got {:?}", other),
    }
}

#[test]
fn worker_source_is_forbidden() {
    let (mgr, repo) = fixture();
    let r = mgr.acquire_repo(&repo).unwrap();
    let p = mgr.acquire_project(&r, "proj-1").unwrap();
    let res = mgr.acquire_lane(&p, "main", Worker::Claude, LockSource::Worker);
    assert_eq!(res.err().unwrap(), LockError::ForbiddenSource);
}

#[test]
fn transfer_state_machine() {
    let (mgr, repo) = fixture();
    let r = mgr.acquire_repo(&repo).unwrap();
    let p = mgr.acquire_project(&r, "proj-1").unwrap();
    let lane = mgr
        .acquire_lane(&p, "main", Worker::Claude, LockSource::Scheduler)
        .unwrap();

    mgr.begin_transfer(&lane, Worker::Codex).unwrap();
    let (state, owner) = mgr.lane_state("proj-1", "main").unwrap();
    use moa_desktop_lib::lock::manager::LaneState;
    assert_eq!(state, LaneState::Transferring);
    assert_eq!(owner, Some(Worker::Claude));

    let new_owner = mgr.complete_transfer(&lane).unwrap();
    assert_eq!(new_owner, Worker::Codex);
    let (state, owner) = mgr.lane_state("proj-1", "main").unwrap();
    assert_eq!(state, LaneState::Acquired);
    assert_eq!(owner, Some(Worker::Codex));

    // Audit log captured.
    let audit = mgr.audit_log();
    use moa_desktop_lib::lock::manager::AuditKind;
    let kinds: Vec<_> = audit.iter().map(|e| e.kind).collect();
    assert!(kinds.contains(&AuditKind::Acquired));
    assert!(kinds.contains(&AuditKind::TransferStarted));
    assert!(kinds.contains(&AuditKind::TransferCompleted));
}

#[test]
fn transfer_to_self_rejected() {
    let (mgr, repo) = fixture();
    let r = mgr.acquire_repo(&repo).unwrap();
    let p = mgr.acquire_project(&r, "proj-1").unwrap();
    let lane = mgr
        .acquire_lane(&p, "main", Worker::Claude, LockSource::Scheduler)
        .unwrap();
    let res = mgr.begin_transfer(&lane, Worker::Claude);
    assert_eq!(res.err().unwrap(), LockError::TransferToSelf);
}

#[test]
fn lane_released_on_drop() {
    let (mgr, repo) = fixture();
    let r = mgr.acquire_repo(&repo).unwrap();
    let p = mgr.acquire_project(&r, "proj-1").unwrap();
    {
        let _lane = mgr
            .acquire_lane(&p, "main", Worker::Claude, LockSource::Scheduler)
            .unwrap();
    }
    // Re-acquire after drop succeeds.
    let _re = mgr
        .acquire_lane(&p, "main", Worker::Codex, LockSource::Scheduler)
        .unwrap();
}

#[test]
fn instance_lock_second_process_refuses_in_same_process() {
    // Same-process: fs2 LockFileEx allows the same process to re-lock the
    // same handle on Windows but a *different* file handle on the same
    // path is rejected — which simulates a second process. So we open via
    // try_acquire twice with two RepoKey-derived handles.
    let td = tempdir().unwrap();
    let repo = RepoKey::from_path(td.path());
    let base = td.path().join("data");

    let l1 = InstanceLock::try_acquire(&repo, &base).expect("first");
    let l2 = InstanceLock::try_acquire(&repo, &base);
    assert!(l2.is_err(), "second acquire must fail");
    drop(l1);

    let _l3 = InstanceLock::try_acquire(&repo, &base).expect("after release");
}
