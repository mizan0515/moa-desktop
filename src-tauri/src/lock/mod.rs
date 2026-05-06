//! T4 — 2-layer lock manager.
//!
//! Layer 1 (`manager`): in-process, tab-aware, ordered (Repo → Project → Lane),
//! with worker→codex transfer state machine and audit log.
//!
//! Layer 2 (`instance`): OS-level fs2 advisory lock keyed by canonical repo,
//! survives Tauri single-instance plugin failure and `--user-data-dir`-based
//! N-instance setups.

pub mod instance;
pub mod manager;

pub use instance::{InstanceLock, InstanceLockError};
pub use manager::{
    AuditEntry, AuditKind, LaneGuard, LaneRequest, LaneState, LockError, LockManager, LockSource,
    ProjectGuard, RepoGuard, Tier, Worker,
};
