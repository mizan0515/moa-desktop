//! T13 L2 — pre-execution command guard for worker sourced process/tool calls.

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::policy::PrimaryRole;
use crate::safety::scanner::{scan_text, RoleContext, ScanResult, ScanSource};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CommandSource {
    Worker,
    Orchestrator,
    LeadPowershell,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuardedCommand {
    pub executable: String,
    pub argv: Vec<String>,
    pub shell_text: Option<String>,
    pub cwd: PathBuf,
    pub source: CommandSource,
    pub primary_role: PrimaryRole,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GuardDecision {
    Allow,
    Deny { reason: String },
}

#[derive(Debug, Error)]
pub enum CommandGuardError {
    #[error("worker command denied: {0}")]
    PermissionDenied(String),
}

pub struct WorkerCommandGuard;

impl WorkerCommandGuard {
    pub fn check(command: &GuardedCommand) -> Result<GuardDecision, CommandGuardError> {
        let source = match command.source {
            CommandSource::Worker => ScanSource::Worker,
            CommandSource::Orchestrator | CommandSource::LeadPowershell => ScanSource::Orchestrator,
        };
        let context = RoleContext {
            primary_role: command.primary_role,
            source,
        };
        let mut joined = command.executable.clone();
        joined.push(' ');
        joined.push_str(&command.argv.join(" "));
        if let Some(shell) = &command.shell_text {
            joined.push(' ');
            joined.push_str(shell);
        }

        if matches!(command.source, CommandSource::Worker)
            && executable_is_peer_ai(&command.executable)
        {
            return Ok(GuardDecision::Deny {
                reason: format!("worker source cannot execute {}", command.executable),
            });
        }
        let deny_bare_peer_token = command.executable != "worker-output";
        if matches!(command.source, CommandSource::Worker)
            && shell_invokes_peer_ai(&joined, deny_bare_peer_token)
        {
            return Ok(GuardDecision::Deny {
                reason: "worker source cannot invoke peer AI from shell text".into(),
            });
        }

        match scan_text(&joined, context) {
            ScanResult::Clean => Ok(GuardDecision::Allow),
            ScanResult::Violation { evidence, .. } => Ok(GuardDecision::Deny {
                reason: format!("blocked pattern: {evidence}"),
            }),
        }
    }

    pub fn require_allowed(command: &GuardedCommand) -> Result<(), CommandGuardError> {
        match Self::check(command)? {
            GuardDecision::Allow => Ok(()),
            GuardDecision::Deny { reason } => Err(CommandGuardError::PermissionDenied(reason)),
        }
    }
}

#[tauri::command]
pub fn worker_command_guard_check(
    executable: String,
    argv: Vec<String>,
    shell_text: Option<String>,
    cwd: PathBuf,
    primary_role: PrimaryRole,
) -> Result<GuardDecision, String> {
    WorkerCommandGuard::check(&GuardedCommand {
        executable,
        argv,
        shell_text,
        cwd,
        source: CommandSource::Worker,
        primary_role,
    })
    .map_err(|e| e.to_string())
}

fn shell_invokes_peer_ai(text: &str, deny_bare_peer_token: bool) -> bool {
    let normalized = text.replace('\\', "/").to_lowercase();
    let tokens = normalized
        .split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '/')))
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    if deny_bare_peer_token
        && tokens.iter().any(|token| {
            let token = token.trim_matches('/');
            let exe = token.rsplit('/').next().unwrap_or(token);
            matches!(
                exe,
                "codex"
                    | "codex.exe"
                    | "codex.cmd"
                    | "codex.ps1"
                    | "claude"
                    | "claude.exe"
                    | "claude.cmd"
                    | "agent"
                    | "agent.exe"
                    | "agent.cmd"
                    | "teamcreate"
                    | "teamcreate.exe"
                    | "teamcreate.cmd"
            )
        })
    {
        return true;
    }
    tokens.windows(2).any(|pair| {
        let token = pair[0].trim_matches('/');
        let exe = token.rsplit('/').next().unwrap_or(token);
        let arg = pair[1];
        (matches!(exe, "codex" | "codex.exe" | "codex.cmd" | "codex.ps1") && arg == "exec")
            || (matches!(exe, "claude" | "claude.exe" | "claude.cmd") && arg == "-p")
    })
}

fn executable_is_peer_ai(executable: &str) -> bool {
    let lower = executable.replace('\\', "/").to_lowercase();
    let file = lower.rsplit('/').next().unwrap_or(&lower);
    matches!(
        file,
        "claude"
            | "claude.exe"
            | "claude.cmd"
            | "codex"
            | "codex.exe"
            | "codex.cmd"
            | "codex.ps1"
            | "agent"
            | "agent.exe"
            | "agent.cmd"
            | "teamcreate"
            | "teamcreate.exe"
            | "teamcreate.cmd"
    )
}

pub fn validate_repo_local_moa_worktree(repo_root: &Path, worktree: &Path) -> Result<(), String> {
    if has_parent_component(repo_root) || has_parent_component(worktree) {
        return Err("path traversal component is not allowed".into());
    }
    let repo = repo_root
        .canonicalize()
        .map_err(|e| format!("repo root canonicalize: {e}"))?;
    let wt = worktree
        .canonicalize()
        .map_err(|e| format!("worktree canonicalize: {e}"))?;
    let expected_parent = repo.join(".moa-desktop").join("worktrees");
    let parent = wt
        .parent()
        .ok_or_else(|| "worktree path has no parent".to_string())?;
    if parent != expected_parent {
        return Err(format!(
            "mutation worktree must be under {}",
            expected_parent.display()
        ));
    }
    if wt == expected_parent || wt == repo {
        return Err("worktree path cannot be repo root or worktree root".into());
    }
    Ok(())
}

fn has_parent_component(path: &Path) -> bool {
    path.components().any(|c| matches!(c, Component::ParentDir))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn command(executable: &str, argv: &[&str], source: CommandSource) -> GuardedCommand {
        GuardedCommand {
            executable: executable.into(),
            argv: argv.iter().map(|s| s.to_string()).collect(),
            shell_text: None,
            cwd: PathBuf::from("."),
            source,
            primary_role: PrimaryRole::Codex,
        }
    }

    #[test]
    fn worker_source_cannot_execute_peer_ai_commands() {
        for exe in [
            "claude",
            "claude.exe",
            "codex",
            "codex.cmd",
            "C:/Users/x/codex.ps1",
            "Agent",
            "C:/Tools/TeamCreate.exe",
        ] {
            let decision =
                WorkerCommandGuard::check(&command(exe, &["exec"], CommandSource::Worker)).unwrap();
            assert!(matches!(decision, GuardDecision::Deny { .. }), "{exe}");
        }
    }

    #[test]
    fn orchestrator_can_spawn_normal_worker_command() {
        let decision =
            WorkerCommandGuard::check(&command("codex", &["exec"], CommandSource::Orchestrator))
                .unwrap();
        assert_eq!(decision, GuardDecision::Allow);
    }

    #[test]
    fn worker_shell_text_blocks_nested_peer_call() {
        let mut cmd = command("powershell", &[], CommandSource::Worker);
        cmd.shell_text = Some("codex exec review".into());
        let decision = WorkerCommandGuard::check(&cmd).unwrap();
        assert!(matches!(decision, GuardDecision::Deny { .. }));
    }

    #[test]
    fn worker_shell_text_blocks_quoted_peer_ai_paths() {
        for shell in [
            "powershell -Command \"& 'C:\\Tools\\codex.exe' exec review\"",
            "cmd /c \"\\\"codex.cmd\\\" exec review\"",
            "powershell -Command \"& 'C:\\Tools\\claude.exe' -p review\"",
            "powershell -Command \"& 'C:\\Tools\\codex.exe'\"",
            "cmd /c claude",
            "cmd /c Agent",
            "powershell -Command \"& 'C:\\Tools\\TeamCreate.exe'\"",
        ] {
            let mut cmd = command("powershell", &[], CommandSource::Worker);
            cmd.shell_text = Some(shell.into());
            let decision = WorkerCommandGuard::check(&cmd).unwrap();
            assert!(matches!(decision, GuardDecision::Deny { .. }), "{shell}");
        }
    }

    #[test]
    fn worker_output_guard_does_not_block_bare_model_names_as_text() {
        let cmd = GuardedCommand {
            executable: "worker-output".into(),
            argv: vec![],
            shell_text: Some("Codex and Claude gave different advice.".into()),
            cwd: PathBuf::from("."),
            source: CommandSource::Worker,
            primary_role: PrimaryRole::Claude,
        };
        assert_eq!(
            WorkerCommandGuard::check(&cmd).unwrap(),
            GuardDecision::Allow
        );
    }

    #[test]
    fn tauri_worker_guard_uses_worker_source() {
        let decision = worker_command_guard_check(
            "codex".into(),
            vec!["exec".into()],
            None,
            PathBuf::from("."),
            PrimaryRole::Claude,
        )
        .unwrap();
        assert!(matches!(decision, GuardDecision::Deny { .. }));
    }

    #[test]
    fn repo_local_worktree_guard_accepts_literal_child() {
        let td = tempdir().unwrap();
        let repo = td.path();
        let wt_parent = repo.join(".moa-desktop").join("worktrees");
        std::fs::create_dir_all(&wt_parent).unwrap();
        let wt = wt_parent.join("session-1");
        std::fs::create_dir(&wt).unwrap();
        assert!(validate_repo_local_moa_worktree(repo, &wt).is_ok());
    }

    #[test]
    fn repo_local_worktree_guard_rejects_outside_path() {
        let repo_td = tempdir().unwrap();
        let out_td = tempdir().unwrap();
        let outside = out_td.path().join("session-1");
        std::fs::create_dir(&outside).unwrap();
        let err = validate_repo_local_moa_worktree(repo_td.path(), &outside).unwrap_err();
        assert!(err.contains(".moa-desktop"));
    }
}
