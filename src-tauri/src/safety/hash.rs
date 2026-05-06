//! SHA-256 file snapshot + diff (T4 §F4).
//!
//! Used at lock transfer / Worker turn boundaries: snapshot the worktree's
//! tracked files, run the next Worker, snapshot again, compare. A
//! transfer-time mismatch on a file the *outgoing* Worker did not touch is
//! a sign of orchestrator bug or external interference and aborts the lane.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HashError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// path-relative-to-root → SHA-256 hex.
pub type Snapshot = BTreeMap<String, String>;

/// Snapshot every file under `root`, recursively. Skips `.git/` and any path
/// with a component starting with `.git`. Returns POSIX-style relative paths
/// for stable cross-platform comparison.
pub fn snapshot_dir(root: &Path) -> Result<Snapshot, HashError> {
    let mut out = Snapshot::new();
    walk(root, root, &mut out)?;
    Ok(out)
}

fn walk(root: &Path, cur: &Path, out: &mut Snapshot) -> Result<(), HashError> {
    for entry in std::fs::read_dir(cur)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name_s = name.to_string_lossy();
        if name_s == ".git" || name_s.starts_with(".git") {
            continue;
        }
        let ft = entry.file_type()?;
        if ft.is_dir() {
            walk(root, &path, out)?;
        } else if ft.is_file() {
            let rel = relative_posix(root, &path);
            let hex = hash_file(&path)?;
            out.insert(rel, hex);
        }
    }
    Ok(())
}

fn relative_posix(root: &Path, path: &Path) -> String {
    let rel: PathBuf = path.strip_prefix(root).unwrap_or(path).to_path_buf();
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

pub fn hash_file(path: &Path) -> Result<String, HashError> {
    let f = File::open(path)?;
    let mut r = BufReader::new(f);
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = r.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub modified: Vec<String>,
}

impl Diff {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.modified.is_empty()
    }
}

pub fn diff(before: &Snapshot, after: &Snapshot) -> Diff {
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut modified = Vec::new();

    for (k, v) in after {
        match before.get(k) {
            None => added.push(k.clone()),
            Some(prev) if prev != v => modified.push(k.clone()),
            _ => {}
        }
    }
    for k in before.keys() {
        if !after.contains_key(k) {
            removed.push(k.clone());
        }
    }
    added.sort();
    removed.sort();
    modified.sort();
    Diff {
        added,
        removed,
        modified,
    }
}
