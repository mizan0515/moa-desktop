use std::fs;
use std::path::Path;

use crate::lifecycle::ResumePacket;

pub fn read_resume_packet(path: &Path) -> Result<ResumePacket, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("resume read: {e}"))?;
    let packet: ResumePacket =
        serde_json::from_str(&text).map_err(|e| format!("resume parse: {e}"))?;
    packet.validate()?;
    Ok(packet)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use tempfile::tempdir;

    use crate::orchestrator::state::Lane;
    use crate::policy::review::{
        CommandSourceAdapter, ReviewGate, ReviewKind, ReviewRunRecord, ReviewVerdict,
    };
    use crate::policy::PrimaryRole;

    #[test]
    fn read_resume_packet_roundtrips() {
        let td = tempdir().unwrap();
        let packet = ResumePacket::minimal("s1", "p1");
        let text = serde_json::to_string(&packet).unwrap();
        let path = td.path().join("resume.json");
        std::fs::write(&path, text).unwrap();
        let out = read_resume_packet(&path).unwrap();
        assert_eq!(out.session_id, "s1");
    }

    #[test]
    fn review_run_records_survive_resume_roundtrip() {
        let td = tempdir().unwrap();
        let mut packet = ResumePacket::minimal("s1", "p1");
        packet.review_run_records.push(ReviewRunRecord {
            verdict: ReviewVerdict::Clean,
            reviewer: Lane::Codex,
            review_kind: ReviewKind::CodexAdversarialXHigh,
            review_profile_id: "CodexAdversarialXHigh".into(),
            reasoning_effort: "xhigh".into(),
            model_or_profile_id: "gpt".into(),
            prompt_template_version: "t13-v1".into(),
            prompt_hash: "hash".into(),
            command_source_adapter: CommandSourceAdapter::MoaOrchestrator,
            primary_role: PrimaryRole::Codex,
            session_id: Some("s1".into()),
            command_name: Some("/메인동기화".into()),
            scope: "local diff".into(),
            gate: ReviewGate::PrCreate,
            patch_hash: "patch".into(),
            files_reviewed: vec!["src-tauri/src/commands/mod.rs".into()],
            omitted_files: vec![],
            limitations: vec![],
            evidence: "Verdict: Clean\nok".into(),
            required_actions: vec![],
            created_at: Utc::now(),
            source_output_path: td.path().join("review.md"),
            failed_readonly_attempt_path: None,
            failed_readonly_attempt_evidence: None,
            status_before: None,
            status_after: None,
            review_caused_mutation: false,
        });
        let path = td.path().join("resume.json");
        std::fs::write(&path, serde_json::to_string(&packet).unwrap()).unwrap();

        let out = read_resume_packet(&path).unwrap();
        assert_eq!(out.review_run_records.len(), 1);
        assert_eq!(
            out.review_run_records[0].review_profile_id,
            "CodexAdversarialXHigh"
        );
    }
}
