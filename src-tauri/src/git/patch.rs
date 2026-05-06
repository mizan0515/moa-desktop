//! Patch extract / verify / apply (T4 §F4).
//!
//! Flow: Worker runs inside a `Worktree`, mutates files, then we
//! 1. `git add -A` inside the worktree
//! 2. `git diff --cached` → unified patch text (also written to disk)
//! 3. `git apply --check <patch>` against the *main repo* — verify cleanly
//!    appliable
//! 4. on confirm: `git apply --index <patch>` against the main repo
//! 5. on reject: caller drops the patch and removes the worktree
//!
//! Step 3 catches "user edited the same file in the main repo while the
//! Worker was running" — apply gets rejected, orchestrator surfaces conflict.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::worktree::{run as run_git, Worktree, WorktreeError};

/// In-memory + on-disk patch artifact.
#[derive(Debug, Clone)]
pub struct Patch {
    pub text: String,
    pub path: PathBuf,
}

impl Patch {
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }
}

/// Stage all changes in the worktree and extract a patch.
///
/// `out_dir` is where the `.patch` file lands (per-session journal dir).
pub fn extract(
    wt: &Worktree,
    out_dir: impl AsRef<Path>,
    name: &str,
) -> Result<Patch, WorktreeError> {
    // Stage including untracked + deletions so the diff is complete.
    let mut add = wt.git();
    add.args(["add", "-A"]);
    run_git(add, "add")?;

    // `--no-color` for stable output, `--binary` so binary edits survive.
    let mut diff = wt.git();
    diff.args(["diff", "--cached", "--no-color", "--binary"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let out = diff.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            WorktreeError::GitMissing
        } else {
            WorktreeError::Io(e)
        }
    })?;
    if !out.status.success() {
        return Err(WorktreeError::Git {
            op: "diff",
            code: out.status.code(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        });
    }
    let text = String::from_utf8_lossy(&out.stdout).into_owned();

    let out_dir = out_dir.as_ref();
    std::fs::create_dir_all(out_dir)?;
    let path = out_dir.join(format!("{name}.patch"));
    std::fs::write(&path, text.as_bytes())?;

    Ok(Patch { text, path })
}

/// `git apply --check` against `repo`. Returns Ok(()) iff the patch applies
/// cleanly. Empty patch is treated as Ok (no-op).
pub fn check(repo: impl AsRef<Path>, patch: &Patch) -> Result<(), WorktreeError> {
    if patch.is_empty() {
        return Ok(());
    }
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(repo.as_ref())
        .args(["apply", "--check"])
        .arg(&patch.path);
    run_git(cmd, "apply --check")
}

/// `git apply --index`. Caller has already run `check`. Empty patch = no-op.
pub fn apply(repo: impl AsRef<Path>, patch: &Patch) -> Result<(), WorktreeError> {
    if patch.is_empty() {
        return Ok(());
    }
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(repo.as_ref())
        .args(["apply", "--index"])
        .arg(&patch.path);
    run_git(cmd, "apply")
}
