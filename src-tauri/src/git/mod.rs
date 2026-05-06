//! T4 — git worktree mutation flow + repo path canonicalization.

pub mod canonical;
pub mod patch;
pub mod worktree;

pub use canonical::RepoKey;
pub use patch::{apply, check, extract, Patch};
pub use worktree::{Worktree, WorktreeError};

/// Probe `git --version`. Returns Ok(version_line) or `WorktreeError::GitMissing`.
/// Used at startup so the UI can surface a clear "install git" error instead of
/// failing later at first mutation.
pub fn probe() -> Result<String, WorktreeError> {
    use std::process::{Command, Stdio};
    let out = Command::new("git")
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                WorktreeError::GitMissing
            } else {
                WorktreeError::Io(e)
            }
        })?;
    if !out.status.success() {
        return Err(WorktreeError::Git {
            op: "version",
            code: out.status.code(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
