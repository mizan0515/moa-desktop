//! T13 L2 — role-aware output scanner.

use serde::{Deserialize, Serialize};

use crate::policy::PrimaryRole;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScanSource {
    Worker,
    Orchestrator,
    SlashCommand,
    Integrator,
    UiFinal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ViolationKind {
    PeerAiCommand,
    NestedAgent,
    WorkerPrivilegeEscalation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleContext {
    pub primary_role: PrimaryRole,
    pub source: ScanSource,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ScanResult {
    Clean,
    Violation {
        violation_kind: ViolationKind,
        evidence: String,
        role_context: RoleContext,
    },
}

impl ScanResult {
    pub fn is_clean(&self) -> bool {
        matches!(self, Self::Clean)
    }
}

const PEER_PATTERNS: &[&str] = &[
    "/codex:",
    "claude -p",
    "claude.exe -p",
    "claude.cmd -p",
    "codex exec",
    "codex.exe exec",
    "codex.cmd exec",
    "codex.ps1 exec",
    "claude mcp",
    "codex mcp",
    "claude_code_peer",
    "call codex",
    "call claude",
    "ask another ai",
    "run another agent",
];

const NESTED_AGENT_PATTERNS: &[&str] = &["teamcreate"];
const WORKER_PRIVILEGE_PATTERNS: &[&str] = &["git push", "gh pr create", "gh pr merge"];

pub fn scan_text(text: &str, context: RoleContext) -> ScanResult {
    let lower = text.to_lowercase();

    for pattern in PEER_PATTERNS {
        if lower.contains(pattern) {
            return violation(ViolationKind::PeerAiCommand, pattern, context);
        }
    }
    for pattern in NESTED_AGENT_PATTERNS {
        if lower.contains(pattern) {
            return violation(ViolationKind::NestedAgent, pattern, context);
        }
    }
    if matches!(context.source, ScanSource::Worker | ScanSource::Integrator) {
        for pattern in WORKER_PRIVILEGE_PATTERNS {
            if lower.contains(pattern) {
                return violation(ViolationKind::WorkerPrivilegeEscalation, pattern, context);
            }
        }
    }

    ScanResult::Clean
}

fn violation(kind: ViolationKind, evidence: &str, context: RoleContext) -> ScanResult {
    if context.source == ScanSource::Orchestrator && matches!(evidence, "claude -p" | "codex exec")
    {
        return ScanResult::Clean;
    }
    ScanResult::Violation {
        violation_kind: kind,
        evidence: evidence.into(),
        role_context: context,
    }
}

pub fn scanner_text_from_json(value: &serde_json::Value) -> Option<String> {
    value
        .get("text")
        .and_then(|v| v.as_str())
        .or_else(|| value.get("message").and_then(|v| v.as_str()))
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(source: ScanSource, primary_role: PrimaryRole) -> RoleContext {
        RoleContext {
            primary_role,
            source,
        }
    }

    #[test]
    fn scanner_blocks_peer_patterns_for_worker_source_both_roles() {
        let patterns = [
            "/codex:rescue",
            "claude -p do it",
            "claude.exe -p do it",
            "claude.cmd -p do it",
            "codex exec review",
            "codex.exe exec review",
            "codex.cmd exec review",
            "codex.ps1 exec review",
            "Claude MCP",
            "Codex MCP",
            "claude_code_peer",
            "TeamCreate",
            "call Codex",
            "call Claude",
            "ask another AI",
            "run another agent",
        ];
        for role in [PrimaryRole::Claude, PrimaryRole::Codex] {
            for pattern in patterns {
                let result = scan_text(pattern, ctx(ScanSource::Worker, role));
                assert!(!result.is_clean(), "pattern should block: {pattern}");
            }
        }
    }

    #[test]
    fn orchestrator_spawn_language_is_allowed() {
        let result = scan_text(
            "Spawning Claude worker for first-pass with claude -p",
            ctx(ScanSource::Orchestrator, PrimaryRole::Claude),
        );
        assert!(result.is_clean());
    }

    #[test]
    fn worker_git_push_is_privilege_violation() {
        let result = scan_text(
            "I will run git push now",
            ctx(ScanSource::Worker, PrimaryRole::Codex),
        );
        assert!(matches!(
            result,
            ScanResult::Violation {
                violation_kind: ViolationKind::WorkerPrivilegeEscalation,
                ..
            }
        ));
    }

    #[test]
    fn benign_agent_words_are_not_nested_agent_violations() {
        for text in ["AGENTS.md", "user-agent header", "AI agent summary"] {
            let result = scan_text(text, ctx(ScanSource::Worker, PrimaryRole::Claude));
            assert!(result.is_clean(), "{text}");
        }
    }
}
