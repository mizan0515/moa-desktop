//! Canonical repo identity (T4 §F4/§F6).
//!
//! Same on-disk repo can be expressed as `D:\repo`, `d:\repo`, `\\?\D:\repo`,
//! through a junction, or via symlink. Lock manager + journal directory layout
//! key off `RepoKey::from_path` so all of these collapse to the same identity.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Stable identity for a repo on disk.
///
/// `canonical` is human-readable (used in journal paths). `key` is the lock
/// map / lock-file name; deterministic and filesystem-safe.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RepoKey {
    pub canonical: PathBuf,
    pub key: String,
}

impl RepoKey {
    /// Resolve a path to its canonical identity. Resolves symlinks and
    /// junctions; strips Windows `\\?\` prefix; lowercases for case-fold;
    /// then SHA-256 hashes the canonical form for the map/lock-file key.
    ///
    /// Falls back to absolute path on canonicalize failure (e.g. path does
    /// not yet exist). The returned key is still stable for the same input.
    pub fn from_path(p: impl AsRef<Path>) -> Self {
        let raw = p.as_ref();
        let canonical = dunce::canonicalize(raw)
            .or_else(|_| std::fs::canonicalize(raw))
            .unwrap_or_else(|_| absolute_or_self(raw));

        let folded = case_fold(&canonical);
        let mut hasher = Sha256::new();
        hasher.update(folded.as_bytes());
        let key = hex::encode(hasher.finalize());

        RepoKey { canonical, key }
    }
}

fn absolute_or_self(p: &Path) -> PathBuf {
    std::env::current_dir()
        .map(|cwd| cwd.join(p))
        .unwrap_or_else(|_| p.to_path_buf())
}

fn case_fold(p: &Path) -> String {
    // Windows + macOS default FS are case-insensitive; Linux is case-sensitive.
    // For lock identity we collapse case unconditionally — two tabs opening
    // `repo` and `Repo` from the same shell on a case-insensitive volume must
    // see one lock. The cost on Linux is rare false positive (different
    // case-sensitive paths colliding); acceptable vs. mutation race risk.
    p.to_string_lossy().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn case_variants_collapse() {
        let td = tempdir().unwrap();
        let p = td.path().join("Sub");
        std::fs::create_dir(&p).unwrap();

        let upper = RepoKey::from_path(&p);
        let lower_str = p.to_string_lossy().to_lowercase();
        let lower = RepoKey::from_path(PathBuf::from(lower_str));

        assert_eq!(upper.key, lower.key, "case-folded keys must match");
    }

    #[test]
    fn nonexistent_path_still_yields_stable_key() {
        let a = RepoKey::from_path("Z:/nope/maybe");
        let b = RepoKey::from_path("Z:/nope/maybe");
        assert_eq!(a.key, b.key);
        assert_eq!(a.key.len(), 64);
    }

    #[test]
    fn different_paths_differ() {
        let td = tempdir().unwrap();
        let a = td.path().join("a");
        let b = td.path().join("b");
        std::fs::create_dir(&a).unwrap();
        std::fs::create_dir(&b).unwrap();
        assert_ne!(RepoKey::from_path(a).key, RepoKey::from_path(b).key);
    }
}
