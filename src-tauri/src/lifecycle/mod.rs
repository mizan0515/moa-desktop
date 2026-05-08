//! T13 L5 — resume packet and session lifecycle artifact.

pub mod export;
pub mod import;

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::policy::review::ReviewRunRecord;
use crate::policy::PrimaryRole;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResumePacket {
    pub session_id: String,
    pub project_id: String,
    pub branch: String,
    pub worktree_path: PathBuf,
    pub current_step: String,
    pub last_phase: String,
    pub primary_role: PrimaryRole,
    pub journal_tail: Vec<String>,
    pub synthesis_snapshot: Option<serde_json::Value>,
    pub claim_ledger: Vec<serde_json::Value>,
    pub open_questions: Vec<String>,
    pub lane_states: Vec<LaneState>,
    pub review_run_records: Vec<ReviewRunRecord>,
    pub pending_approvals: Vec<PendingApproval>,
    pub command_history: Vec<String>,
    pub timestamp: DateTime<Utc>,
    pub version_pin: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaneState {
    pub lane: String,
    pub status: String,
    pub resume_path: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingApproval {
    pub permission: String,
    pub step: String,
    pub reason: String,
}

impl ResumePacket {
    pub fn minimal(session_id: impl Into<String>, project_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            project_id: project_id.into(),
            branch: "unknown".into(),
            worktree_path: PathBuf::from("."),
            current_step: "unknown".into(),
            last_phase: "unknown".into(),
            primary_role: PrimaryRole::Claude,
            journal_tail: Vec::new(),
            synthesis_snapshot: None,
            claim_ledger: Vec::new(),
            open_questions: Vec::new(),
            lane_states: Vec::new(),
            review_run_records: Vec::new(),
            pending_approvals: Vec::new(),
            command_history: Vec::new(),
            timestamp: Utc::now(),
            version_pin: "t13-v1".into(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.session_id.trim().is_empty() {
            return Err("session_id is required".into());
        }
        if self.project_id.trim().is_empty() {
            return Err("project_id is required".into());
        }
        if self.branch.trim().is_empty() {
            return Err("branch is required".into());
        }
        if self.worktree_path.as_os_str().is_empty() {
            return Err("worktree_path is required".into());
        }
        if self.current_step.trim().is_empty() {
            return Err("current_step is required".into());
        }
        if self.version_pin.trim().is_empty() {
            return Err("version_pin is required".into());
        }
        Ok(())
    }
}

#[tauri::command]
pub fn export_resume_packet(packet: ResumePacket, output_dir: PathBuf) -> Result<PathBuf, String> {
    export::write_resume_packet(&packet, &output_dir)
}

#[tauri::command]
pub fn import_resume_packet(path: PathBuf) -> Result<ResumePacket, String> {
    import::read_resume_packet(&path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_resume_packet_has_version_pin() {
        let packet = ResumePacket::minimal("s", "p");
        assert_eq!(packet.version_pin, "t13-v1");
        assert_eq!(packet.primary_role, PrimaryRole::Claude);
        assert_eq!(packet.current_step, "unknown");
        assert!(packet.validate().is_ok());
    }

    #[test]
    fn resume_packet_requires_resume_identity() {
        let mut packet = ResumePacket::minimal("s", "p");
        packet.branch.clear();
        assert_eq!(packet.validate(), Err("branch is required".into()));

        let mut packet = ResumePacket::minimal("s", "p");
        packet.current_step.clear();
        assert_eq!(packet.validate(), Err("current_step is required".into()));
    }
}
