//! T13 L3 — policy source hashing and drift detection.

use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::policy::pack::{SourceEntry, SourceKind, SourceRole};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Blocker,
    Warning,
}

pub fn missing_source_severity(kind: SourceKind) -> Severity {
    match kind {
        SourceKind::HotRule => Severity::Blocker,
        SourceKind::TicketCloseRule => Severity::Blocker,
        SourceKind::OnDemandSkill => Severity::Warning,
        SourceKind::RuntimeHealthCheck => Severity::Warning,
        SourceKind::RuntimeSettings => Severity::Warning,
        SourceKind::CodexDesktopOverlay => Severity::Warning,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Drift {
    pub path: PathBuf,
    pub expected_sha256: Option<String>,
    pub actual_sha256: Option<String>,
}

pub fn source_entry(path: PathBuf, kind: SourceKind, role: SourceRole) -> SourceEntry {
    match hash_file(&path) {
        Ok((sha256, size_bytes)) => SourceEntry {
            path,
            kind,
            sha256: Some(sha256),
            size_bytes: Some(size_bytes),
            role,
            present: true,
        },
        Err(_) => SourceEntry {
            path,
            kind,
            sha256: None,
            size_bytes: None,
            role,
            present: false,
        },
    }
}

pub fn detect_drift(baseline: &[SourceEntry]) -> Vec<Drift> {
    baseline
        .iter()
        .filter_map(|entry| {
            let actual = hash_file(&entry.path).ok().map(|(h, _)| h);
            if actual != entry.sha256 {
                Some(Drift {
                    path: entry.path.clone(),
                    expected_sha256: entry.sha256.clone(),
                    actual_sha256: actual,
                })
            } else {
                None
            }
        })
        .collect()
}

pub fn hash_file(path: &Path) -> Result<(String, u64), std::io::Error> {
    let bytes = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok((hex::encode(hasher.finalize()), bytes.len() as u64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn drift_detects_changed_hash() {
        let td = tempdir().unwrap();
        let path = td.path().join("rule.md");
        fs::write(&path, "one").unwrap();
        let entry = source_entry(path.clone(), SourceKind::HotRule, SourceRole::GuardShared);
        fs::write(&path, "two").unwrap();
        let drift = detect_drift(&[entry]);
        assert_eq!(drift.len(), 1);
        assert_eq!(drift[0].path, path);
    }

    #[test]
    fn severity_blockers() {
        assert_eq!(missing_source_severity(SourceKind::HotRule), Severity::Blocker);
        assert_eq!(missing_source_severity(SourceKind::TicketCloseRule), Severity::Blocker);
    }

    #[test]
    fn severity_warnings() {
        assert_eq!(missing_source_severity(SourceKind::OnDemandSkill), Severity::Warning);
        assert_eq!(missing_source_severity(SourceKind::RuntimeHealthCheck), Severity::Warning);
        assert_eq!(missing_source_severity(SourceKind::RuntimeSettings), Severity::Warning);
        assert_eq!(missing_source_severity(SourceKind::CodexDesktopOverlay), Severity::Warning);
    }

    #[test]
    fn missing_file_is_recorded_as_absent_source() {
        let entry = source_entry(
            PathBuf::from("does-not-exist"),
            SourceKind::OnDemandSkill,
            SourceRole::RuntimePatch,
        );
        assert!(!entry.present);
        assert!(entry.sha256.is_none());
    }
}
