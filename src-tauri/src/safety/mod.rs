//! T4 — safety primitives shared across git/lock/journal subsystems.

pub mod hash;

pub use hash::{diff, hash_file, snapshot_dir, Diff, HashError, Snapshot};
