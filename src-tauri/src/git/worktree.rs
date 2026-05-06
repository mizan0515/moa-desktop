//! `git worktree add/remove` shell-out (T4 §F4).
//!
//! Workers never touch the user's source tree directly. Orchestrator spins
//! up a temp worktree at a sibling path, the Worker mutates inside it, the
//! app extracts a patch, validates, then either applies to the main repo or
//! rejects + cleans up.
//!
//! CLI shell-out vs libgit2/gitoxide: CLI is the single source of truth for
//! the user's git config (longpath, autocrlf, hooks), behaves identically to
//! what the user runs by hand, and avoids a 5MB+ static dep. Tradeoff: we
//! depend on git being on PATH (Phase 1 prereq, surfaced as CliMissing).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorktreeError {
    #[error("git CLI not found on PATH")]
    GitMissing,
    #[error("git worktree {op} failed (exit={code:?}): {stderr}")]
    Git {
        op: &'static str,
        code: Option<i32>,
        stderr: String,
    },
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// A short-lived worktree handle. `remove()` is explicit (not Drop) so the
/// orchestrator decides whether to keep it for post-mortem on patch reject.
#[derive(Debug)]
pub struct Worktree {
    pub repo: PathBuf,
    pub path: PathBuf,
    pub branch: Option<String>,
}

impl Worktree {
    /// Create a worktree at `path` from `repo`. If `branch` is `Some`, creates
    /// a new branch with `-b`. If `None`, detached HEAD at `repo`'s HEAD.
    pub fn add(
        repo: impl AsRef<Path>,
        path: impl AsRef<Path>,
        branch: Option<&str>,
    ) -> Result<Self, WorktreeError> {
        let repo = repo.as_ref().to_path_buf();
        let path = path.as_ref().to_path_buf();

        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(&repo).arg("worktree").arg("add");
        if let Some(b) = branch {
            cmd.arg("-b").arg(b);
        } else {
            cmd.arg("--detach");
        }
        cmd.arg(&path);

        run(cmd, "add")?;

        Ok(Self {
            repo,
            path,
            branch: branch.map(String::from),
        })
    }

    /// `git worktree remove --force <path>`. Idempotent: missing worktree is
    /// not an error (e.g. crash recovery cleanup).
    pub fn remove(&self) -> Result<(), WorktreeError> {
        if !self.path.exists() {
            // `git worktree prune` later reaps the metadata; nothing to do.
            return Ok(());
        }
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.repo)
            .arg("worktree")
            .arg("remove")
            .arg("--force")
            .arg(&self.path);
        run(cmd, "remove")
    }

    /// `git -C <path>` builder, used by `patch` module.
    pub fn git(&self) -> Command {
        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(&self.path);
        cmd
    }
}

pub(crate) fn run(mut cmd: Command, op: &'static str) -> Result<(), WorktreeError> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let out = cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            WorktreeError::GitMissing
        } else {
            WorktreeError::Io(e)
        }
    })?;
    if !out.status.success() {
        return Err(WorktreeError::Git {
            op,
            code: out.status.code(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        });
    }
    Ok(())
}
