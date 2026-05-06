//! OS-level cross-process lock — Layer 2 of T4 §F6.
//!
//! Tauri's single-instance plugin is unreliable on Win11 24H2 and we
//! intentionally allow `--user-data-dir <path>` for crash isolation, so a
//! second moa-desktop process *can* be running. The in-memory `LockManager`
//! protects intra-process safety; this module guarantees that even with N
//! processes, only one holds the mutation lock for a given canonical repo.
//!
//! Implementation: per-repo lock file at
//! `~/.moa-desktop/locks/<repo-key-prefix>.lock`, held with `fs2` advisory
//! exclusive lock. fs2 maps to `LockFileEx(LOCKFILE_EXCLUSIVE_LOCK)` on
//! Windows and `flock(LOCK_EX | LOCK_NB)` on Unix — both auto-release on
//! process death (kernel cleans up handles), so stale-PID detection is
//! handled by the OS for free.
//!
//! We additionally write `{pid, ts}` into the file body so debug tooling
//! (and humans) can see who claims to hold it without resorting to lsof.
//! That metadata is best-effort; the *truth* is the kernel-held lock.

use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use fs2::FileExt;
use thiserror::Error;

use crate::git::canonical::RepoKey;

#[derive(Debug, Error)]
pub enum InstanceLockError {
    #[error("another process holds the lock for this repo")]
    Busy,
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Held lock — drop releases.
#[must_use = "InstanceLock releases the OS-level lock on drop"]
pub struct InstanceLock {
    file: File,
    #[allow(dead_code)]
    path: PathBuf,
}

impl InstanceLock {
    /// `try_acquire` — non-blocking. Returns Busy immediately if held.
    pub fn try_acquire(repo: &RepoKey, base_dir: &Path) -> Result<Self, InstanceLockError> {
        let dir = base_dir.join("locks");
        std::fs::create_dir_all(&dir)?;
        // Use first 16 hex chars (64 bits) — collision-free in practice and
        // keeps filenames short on Windows.
        let path = dir.join(format!("{}.lock", &repo.key[..16]));
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;

        match file.try_lock_exclusive() {
            Ok(()) => {}
            Err(e) => {
                // fs2 returns a generic io::Error with kind WouldBlock or
                // similar; treat any failure as Busy.
                return Err(if e.kind() == std::io::ErrorKind::WouldBlock {
                    InstanceLockError::Busy
                } else {
                    InstanceLockError::Io(e)
                });
            }
        }

        // Best-effort metadata write. If this fails, the lock is still valid.
        let mut f = file;
        let _ = f.set_len(0);
        let _ = f.seek(SeekFrom::Start(0));
        let _ = writeln!(
            f,
            "{{\"pid\":{},\"ts_ms\":{},\"canonical\":{:?}}}",
            std::process::id(),
            now_ms(),
            repo.canonical
        );
        let _ = f.flush();

        Ok(Self { file: f, path })
    }
}

impl Drop for InstanceLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
