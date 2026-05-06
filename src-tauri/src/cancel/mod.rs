//! T9 — cancellation registry.
//!
//! Process tree-kill itself lives in `process::kill` (T2). This module is the
//! orchestrator-facing registry: it tracks which runs are live so a UI Stop
//! button can abort one or all of them in a single call.

pub mod tree_kill;

pub use tree_kill::{CancelRegistry, RunId};
