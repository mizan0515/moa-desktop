//! T13 L3 — structured policy pack schema.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::policy::runtime_profile::RuntimeProfile;
use crate::settings::PolicySyncMode;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyPack {
    pub source_manifest: Vec<SourceEntry>,
    pub runtime_profile: RuntimeProfile,
    pub output_blocklist: Vec<String>,
    pub role_bindings: Vec<RoleBinding>,
    pub token_thresholds: TokenThresholds,
    pub handoff_behavior: HandoffBehavior,
    pub command_permission_classes: Vec<CommandPermissionClass>,
    pub ticket_close_gate: TicketCloseGate,
    pub version: String,
    pub sync_mode: PolicySyncMode,
}

impl Default for PolicyPack {
    fn default() -> Self {
        Self {
            source_manifest: Vec::new(),
            runtime_profile: RuntimeProfile::default(),
            output_blocklist: vec![
                "/codex:".into(),
                "claude -p".into(),
                "codex exec".into(),
                "Claude MCP".into(),
                "Codex MCP".into(),
                "claude_code_peer".into(),
                "TeamCreate".into(),
                "Agent".into(),
            ],
            role_bindings: Vec::new(),
            token_thresholds: TokenThresholds::default(),
            handoff_behavior: HandoffBehavior::default(),
            command_permission_classes: Vec::new(),
            ticket_close_gate: TicketCloseGate::default(),
            version: "safe-default".into(),
            sync_mode: PolicySyncMode::Manual,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceEntry {
    pub path: PathBuf,
    pub kind: SourceKind,
    pub sha256: Option<String>,
    pub size_bytes: Option<u64>,
    pub role: SourceRole,
    pub present: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceKind {
    HotRule,
    OnDemandSkill,
    TicketCloseRule,
    RuntimeHealthCheck,
    RuntimeSettings,
    CodexDesktopOverlay,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceRole {
    GuardClaude,
    GuardCodex,
    GuardShared,
    OutputScannerSource,
    TokenThreshold,
    HandoffPolicy,
    CloseGate,
    RuntimePatch,
    SlashCommandReference,
    RuntimeProfile,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleBinding {
    pub role: SourceRole,
    pub source_paths: Vec<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenThresholds {
    pub compact_recommend_percent: u8,
    pub clear_recommend_after_repeated_compaction: bool,
}

impl Default for TokenThresholds {
    fn default() -> Self {
        Self {
            compact_recommend_percent: 40,
            clear_recommend_after_repeated_compaction: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandoffBehavior {
    pub include_resume_packet: bool,
    pub include_open_questions: bool,
}

impl Default for HandoffBehavior {
    fn default() -> Self {
        Self {
            include_resume_packet: true,
            include_open_questions: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandPermissionClass {
    pub name: String,
    pub permission: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TicketCloseGate {
    pub require_codex_adversarial_xhigh: bool,
    pub require_review_run_record: bool,
}

impl Default for TicketCloseGate {
    fn default() -> Self {
        Self {
            require_codex_adversarial_xhigh: true,
            require_review_run_record: true,
        }
    }
}

pub fn validate_policy_pack(pack: &PolicyPack) -> Result<(), String> {
    if pack.version.trim().is_empty() {
        return Err("policy pack version is required".into());
    }
    for entry in &pack.source_manifest {
        if entry.present && entry.sha256.as_deref().unwrap_or_default().len() != 64 {
            return Err(format!(
                "source entry {} has invalid sha256",
                entry.path.display()
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_default_is_manual_and_blocks_peer_patterns() {
        let pack = PolicyPack::default();
        assert_eq!(pack.sync_mode, PolicySyncMode::Manual);
        assert!(pack.output_blocklist.iter().any(|p| p == "codex exec"));
    }

    #[test]
    fn schema_rejects_bad_hash_for_present_source() {
        let mut pack = PolicyPack::default();
        pack.source_manifest.push(SourceEntry {
            path: PathBuf::from("x"),
            kind: SourceKind::HotRule,
            sha256: Some("bad".into()),
            size_bytes: Some(1),
            role: SourceRole::GuardShared,
            present: true,
        });
        assert!(validate_policy_pack(&pack).is_err());
    }
}
