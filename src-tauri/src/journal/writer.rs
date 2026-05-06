//! Append-only JSONL writer. One writer per (project, session).
//!
//! Durability policy (T4 §F6):
//! - File handle opened once at session start, kept open for session lifetime.
//! - Each `append` is `write_all` + `flush` (BufWriter→OS buffer→disk
//!   pagecache); kernel flushes on schedule.
//! - `Phase::is_critical` entries trigger an extra `sync_all` (fsync) so the
//!   entry survives a hard power loss. Non-critical entries do not — that's
//!   the bounded-batched policy.
//! - Reconcile must treat "last entry missing" as normal; truth = journal ∪
//!   worktree dir scan ∪ patch dir scan.
//! - Per-session single writer = a `parking_lot::Mutex<File>`. Cheap, fair,
//!   no lock starvation since lane mutation lock is *not* held during
//!   journal append (callers must `drop(LaneGuard)` first if they want to
//!   keep contention bounded — but not required for correctness).

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;
use thiserror::Error;

use super::schema::{Entry, Phase};

#[derive(Debug, Error)]
pub enum JournalError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),
}

#[derive(Debug, Clone)]
pub struct JournalWriter {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    file: Mutex<BufWriter<File>>,
    seq: AtomicU64,
    path: PathBuf,
    /// Best-effort warning surfaced via `paths_under_onedrive_or_defender`.
    pub warn_synced_dir: bool,
}

impl JournalWriter {
    /// Create or open `~/.moa-desktop/journals/<project>/<session>.jsonl`.
    /// `base_dir` is `~/.moa-desktop` (parameterized for tests).
    pub fn open(
        base_dir: &Path,
        project_id: &str,
        session_id: &str,
    ) -> Result<Self, JournalError> {
        let dir = base_dir.join("journals").join(project_id);
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{session_id}.jsonl"));

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&path)?;

        // Resume seq from existing entries if any.
        let seq = read_max_seq(&path).unwrap_or(0);

        Ok(Self {
            inner: Arc::new(Inner {
                file: Mutex::new(BufWriter::new(file)),
                seq: AtomicU64::new(seq),
                warn_synced_dir: is_synced_dir(&path),
                path,
            }),
        })
    }

    pub fn path(&self) -> &Path {
        &self.inner.path
    }

    pub fn warn_synced_dir(&self) -> bool {
        self.inner.warn_synced_dir
    }

    /// Build an entry from `(phase, owner, base_hashes, patch_path, note)`,
    /// stamping `seq` + `ts_ms` + `pid`, and append. Returns the assigned
    /// seq.
    pub fn append(&self, mut entry: Entry) -> Result<u64, JournalError> {
        let seq = self.inner.seq.fetch_add(1, Ordering::SeqCst) + 1;
        entry.seq = seq;
        if entry.ts_ms == 0 {
            entry.ts_ms = now_ms();
        }
        if entry.pid == 0 {
            entry.pid = std::process::id();
        }

        let line = serde_json::to_string(&entry)?;
        let critical = entry.phase.is_critical();

        let mut buf = self.inner.file.lock();
        buf.write_all(line.as_bytes())?;
        buf.write_all(b"\n")?;
        buf.flush()?;

        if critical {
            // Reach the inner File for fsync. BufWriter::get_ref → &File.
            buf.get_ref().sync_all()?;
        }

        Ok(seq)
    }

    /// Convenience for the common phases.
    pub fn note(&self, phase: Phase, note: impl Into<String>) -> Result<u64, JournalError> {
        self.append(Entry {
            seq: 0,
            ts_ms: 0,
            phase,
            owner: None,
            pid: 0,
            base_hashes: Default::default(),
            patch_path: None,
            note: Some(note.into()),
        })
    }
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn read_max_seq(path: &Path) -> Option<u64> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut max = 0u64;
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(e) = serde_json::from_str::<Entry>(line) {
            if e.seq > max {
                max = e.seq;
            }
        }
    }
    if max == 0 {
        None
    } else {
        Some(max)
    }
}

/// Heuristic: warn if journal lives under OneDrive / Dropbox / Defender quarantine
/// path. fsync semantics on synced directories are unreliable on Windows.
fn is_synced_dir(path: &Path) -> bool {
    let s = path.to_string_lossy().to_lowercase();
    s.contains("onedrive")
        || s.contains("dropbox")
        || s.contains("google drive")
        || s.contains("\\windows\\softwaredistribution\\")
}
