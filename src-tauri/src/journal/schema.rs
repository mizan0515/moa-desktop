//! Journal entry schema (T4 §F6).
//!
//! Append-only JSONL. Schema is intentionally narrow — phase + owner + pid
//! + base hashes + patch ref. Anything richer (full prompts, raw output)
//! lives elsewhere (telemetry, per-attempt logs) so journal stays small
//! enough that startup reconcile can scan it in O(ms).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::lock::manager::Worker;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    SessionStart,
    LockAcquired,
    WorktreeCreated,
    WorkerStarted,
    WorkerFinished,
    PatchExtracted,
    PatchVerified,
    PatchRejected,
    PatchApplied,
    LockTransferStarted,
    LockTransferCompleted,
    WorktreeRemoved,
    SessionEnd,
}

impl Phase {
    /// Phases where we want a `sync_all` after the append. Crash before any
    /// of these = silent data loss is acceptable; crash after = the next
    /// boot's reconcile can rely on this entry.
    pub fn is_critical(self) -> bool {
        matches!(
            self,
            Phase::PatchVerified
                | Phase::PatchApplied
                | Phase::PatchRejected
                | Phase::LockTransferCompleted
                | Phase::SessionEnd
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub seq: u64,
    pub ts_ms: i64,
    pub phase: Phase,
    pub owner: Option<Worker>,
    pub pid: u32,
    /// path → sha256 hex. May be empty before first snapshot.
    #[serde(default)]
    pub base_hashes: BTreeMap<String, String>,
    /// Filesystem path of the extracted .patch (if any).
    #[serde(default)]
    pub patch_path: Option<String>,
    /// Optional human note.
    #[serde(default)]
    pub note: Option<String>,
}
