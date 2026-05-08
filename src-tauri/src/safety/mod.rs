//! T4 — safety primitives shared across git/lock/journal subsystems.

pub mod command_guard;
pub mod hash;
pub mod scanner;

pub use command_guard::{
    CommandGuardError, CommandSource, GuardDecision, GuardedCommand, WorkerCommandGuard,
};
pub use hash::{diff, hash_file, snapshot_dir, Diff, HashError, Snapshot};
pub use scanner::{scan_text, RoleContext, ScanResult, ScanSource, ViolationKind};
