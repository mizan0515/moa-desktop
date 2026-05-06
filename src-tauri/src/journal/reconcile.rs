//! Startup reconcile (T4 §F6).
//!
//! On boot we scan `~/.moa-desktop/journals/*` for sessions that ended
//! abnormally — the last entry is not `SessionEnd`, `PatchApplied`, or
//! `PatchRejected`. The orchestrator surfaces these to the user with two
//! choices:
//!
//! - **Cleanup**: discard worktree + patch dir, append `SessionEnd { note:
//!   "abandoned" }`.
//! - **Resume**: re-open the worktree (if it still exists) and continue from
//!   the last completed phase.
//!
//! We deliberately do NOT auto-decide — even with the journal, "what did the
//! user actually want" is ambiguous. Reconcile produces a list; UI handles
//! it.
//!
//! Per durability policy, the journal's last entry may be *missing* even if
//! the operation completed. So we additionally ground-truth against the
//! worktree path and patch path: if the worktree no longer exists and a
//! patch file matches the expected name, we infer at least PatchExtracted.

use std::path::{Path, PathBuf};

use serde::Serialize;

use super::reader;
use super::schema::{Entry, Phase};
use super::writer::JournalError;

#[derive(Debug, Clone, Serialize)]
pub struct UnfinishedSession {
    pub project_id: String,
    pub session_id: String,
    pub journal_path: PathBuf,
    pub last_phase: Option<Phase>,
    pub last_entry: Option<Entry>,
    pub patch_path_exists: bool,
}

/// Scan `<base>/journals/*/<session>.jsonl` for sessions whose last entry is
/// not terminal. Returns one entry per such session.
pub fn scan(base_dir: &Path) -> Result<Vec<UnfinishedSession>, JournalError> {
    let root = base_dir.join("journals");
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for proj in std::fs::read_dir(&root)? {
        let proj = proj?;
        if !proj.file_type()?.is_dir() {
            continue;
        }
        let project_id = proj.file_name().to_string_lossy().into_owned();
        for sess in std::fs::read_dir(proj.path())? {
            let sess = sess?;
            let path = sess.path();
            if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                continue;
            }
            let session_id = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let entries = reader::read_all(&path)?;
            let last = entries.last().cloned();
            let last_phase = last.as_ref().map(|e| e.phase);

            let terminal = matches!(
                last_phase,
                Some(Phase::SessionEnd) | Some(Phase::PatchApplied) | Some(Phase::PatchRejected)
            );
            if terminal {
                continue;
            }

            let patch_path_exists = last
                .as_ref()
                .and_then(|e| e.patch_path.as_ref())
                .map(|p| Path::new(p).exists())
                .unwrap_or(false);

            out.push(UnfinishedSession {
                project_id: project_id.clone(),
                session_id,
                journal_path: path,
                last_phase,
                last_entry: last,
                patch_path_exists,
            });
        }
    }
    Ok(out)
}
