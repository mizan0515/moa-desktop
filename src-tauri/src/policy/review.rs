//! T13 L2.5 — review verdict, input strategy, and auditable run records.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::orchestrator::state::Lane;
use crate::policy::PrimaryRole;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReviewVerdict {
    Clean,
    Concern,
    Block,
    ReviewRunError,
}

impl ReviewVerdict {
    pub fn aggregate(a: Self, b: Self) -> Self {
        use ReviewVerdict::*;
        match (a, b) {
            (Block, _) | (_, Block) | (ReviewRunError, _) | (_, ReviewRunError) => Block,
            (Concern, _) | (_, Concern) => Concern,
            (Clean, Clean) => Clean,
        }
    }

    pub fn gate_allows_progress(self) -> bool {
        matches!(self, Self::Clean)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReviewKind {
    CodexAdversarialXHigh,
    ClaudeSymmetry,
    Manual,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReviewGate {
    PrCreate,
    PrMerge,
    IntegrateMerge,
    MainApply,
    TicketClose,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CommandSourceAdapter {
    MoaOrchestrator,
    CodexDesktopLeadPowershell,
    CodexDesktopLeadPowershellControlledBypass,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewProfile {
    pub id: ReviewKind,
    pub reviewer: Lane,
    pub reasoning_effort: String,
    pub model_or_profile_id: String,
    pub prompt_template_version: String,
    pub prompt_hash: String,
    pub command_source_adapter: CommandSourceAdapter,
    pub output_capture_required: bool,
}

impl ReviewProfile {
    pub fn codex_adversarial_xhigh(source: CommandSourceAdapter) -> Self {
        Self {
            id: ReviewKind::CodexAdversarialXHigh,
            reviewer: Lane::Codex,
            reasoning_effort: "xhigh".into(),
            model_or_profile_id: "CodexAdversarialXHigh".into(),
            prompt_template_version: "t13-v1".into(),
            prompt_hash: String::new(),
            command_source_adapter: source,
            output_capture_required: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewRunRecord {
    pub verdict: ReviewVerdict,
    pub reviewer: Lane,
    pub review_kind: ReviewKind,
    pub review_profile_id: String,
    pub reasoning_effort: String,
    pub model_or_profile_id: String,
    pub prompt_template_version: String,
    pub prompt_hash: String,
    pub command_source_adapter: CommandSourceAdapter,
    pub primary_role: PrimaryRole,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_name: Option<String>,
    pub scope: String,
    pub gate: ReviewGate,
    pub patch_hash: String,
    pub files_reviewed: Vec<PathBuf>,
    pub omitted_files: Vec<PathBuf>,
    pub limitations: Vec<String>,
    pub evidence: String,
    pub required_actions: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub source_output_path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed_readonly_attempt_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed_readonly_attempt_evidence: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_after: Option<String>,
    #[serde(default)]
    pub review_caused_mutation: bool,
}

impl ReviewRunRecord {
    pub fn is_gate_complete(&self) -> bool {
        let base_complete = self.verdict.gate_allows_progress()
            && self.review_kind == ReviewKind::CodexAdversarialXHigh
            && self.reviewer == Lane::Codex
            && self.review_profile_id == "CodexAdversarialXHigh"
            && self.reasoning_effort == "xhigh"
            && !self.model_or_profile_id.is_empty()
            && !self.prompt_template_version.is_empty()
            && !self.prompt_hash.is_empty()
            && !self.scope.is_empty()
            && !self.patch_hash.is_empty()
            && !self.files_reviewed.is_empty()
            && !self.evidence.is_empty()
            && exact_clean_evidence(&self.evidence)
            && self.source_output_path.is_file()
            && source_output_has_exact_clean(&self.source_output_path);
        if !base_complete {
            return false;
        }
        if self.command_source_adapter
            == CommandSourceAdapter::CodexDesktopLeadPowershellControlledBypass
        {
            let failed = self
                .failed_readonly_attempt_evidence
                .as_deref()
                .unwrap_or_default();
            let failed_file_is_valid = self
                .failed_readonly_attempt_path
                .as_ref()
                .and_then(|path| std::fs::read_to_string(path).ok())
                .is_some_and(|text| controlled_bypass_allowed(&text));
            return self.failed_readonly_attempt_path.is_some()
                && self
                    .failed_readonly_attempt_path
                    .as_ref()
                    .is_some_and(|path| path.is_file())
                && failed_file_is_valid
                && controlled_bypass_allowed(failed)
                && verdict_line_count(&self.evidence) == 1
                && clean_verdict_line_count(&self.evidence) == 1
                && self.status_before.is_some()
                && self.status_before == self.status_after
                && !self.review_caused_mutation;
        }
        true
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewInput {
    pub patch_hash: String,
    pub diff_stat: String,
    pub changed_files: Vec<PathBuf>,
    pub ticket_scope: Vec<PathBuf>,
    pub critical_files: Vec<PathBuf>,
    pub chunks: Vec<ReviewChunk>,
    pub max_context_bytes: usize,
    pub reviewed_subset_reason: Option<String>,
    pub omitted_files: Vec<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewChunk {
    pub files: Vec<PathBuf>,
    pub approx_bytes: usize,
    pub critical: bool,
}

pub fn build_review_input(
    patch_hash: impl Into<String>,
    diff_stat: impl Into<String>,
    changed_files: Vec<PathBuf>,
    ticket_scope: Vec<PathBuf>,
    critical_files: Vec<PathBuf>,
    diff_bytes: usize,
    max_context_bytes: usize,
) -> ReviewInput {
    let mut chunks = Vec::new();
    let mut omitted_files = Vec::new();
    let reviewed_subset_reason;

    if diff_bytes < 50 * 1024 {
        chunks.push(ReviewChunk {
            files: changed_files.clone(),
            approx_bytes: diff_bytes,
            critical: false,
        });
        reviewed_subset_reason = None;
    } else if diff_bytes <= 500 * 1024 {
        let critical = prioritized_files(&changed_files, &critical_files);
        let rest = changed_files
            .iter()
            .filter(|p| !critical.contains(p))
            .cloned()
            .collect::<Vec<_>>();
        chunks.push(ReviewChunk {
            files: critical,
            approx_bytes: diff_bytes.min(max_context_bytes),
            critical: true,
        });
        if !rest.is_empty() {
            chunks.push(ReviewChunk {
                files: rest,
                approx_bytes: diff_bytes
                    .saturating_sub(max_context_bytes)
                    .min(max_context_bytes),
                critical: false,
            });
        }
        reviewed_subset_reason = Some("critical-files-first".into());
    } else {
        let scoped = prioritized_files(&changed_files, &ticket_scope);
        let selected = if scoped.is_empty() {
            changed_files.iter().take(10).cloned().collect::<Vec<_>>()
        } else {
            scoped
        };
        omitted_files = changed_files
            .iter()
            .filter(|p| !selected.contains(p))
            .cloned()
            .collect();
        chunks.push(ReviewChunk {
            files: selected,
            approx_bytes: max_context_bytes,
            critical: true,
        });
        reviewed_subset_reason = Some("large-diff-ticket-scope-chunking".into());
    }

    ReviewInput {
        patch_hash: patch_hash.into(),
        diff_stat: diff_stat.into(),
        changed_files,
        ticket_scope,
        critical_files,
        chunks,
        max_context_bytes,
        reviewed_subset_reason,
        omitted_files,
    }
}

fn prioritized_files(all: &[PathBuf], priority: &[PathBuf]) -> Vec<PathBuf> {
    priority
        .iter()
        .filter(|p| all.contains(p))
        .cloned()
        .collect()
}

pub fn controlled_bypass_allowed(failed_attempt_text: &str) -> bool {
    let verdict_count = failed_attempt_text
        .lines()
        .filter(|line| line.trim() == "Verdict: ReviewRunError")
        .count();
    verdict_count == 1
        && failed_attempt_text.contains("ENV_BLOCKED")
        && failed_attempt_text.contains("WindowsApps")
        && failed_attempt_text.contains("pwsh.exe")
        && failed_attempt_text.contains("blocked by policy")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailedReadonlyReviewAction {
    RetryFormalReadonlyWithNativeCodex,
    ControlledBypassAllowed,
    FailClosed,
}

pub fn classify_failed_readonly_review(text: &str) -> FailedReadonlyReviewAction {
    if controlled_bypass_allowed(text) {
        return FailedReadonlyReviewAction::ControlledBypassAllowed;
    }
    if retry_formal_readonly_with_native_codex(text) {
        return FailedReadonlyReviewAction::RetryFormalReadonlyWithNativeCodex;
    }
    FailedReadonlyReviewAction::FailClosed
}

pub fn retry_formal_readonly_with_native_codex(text: &str) -> bool {
    if text
        .lines()
        .filter(|line| line.trim() == "Verdict: ReviewRunError")
        .count()
        != 1
    {
        return false;
    }

    let lower = text.to_ascii_lowercase();
    let mentions_powershell_wrapper = lower.contains("powershell")
        || lower.contains("nativecommanderror")
        || lower.contains("codex.ps1");
    let mentions_startup_plugin_sync = lower.contains("remote plugin")
        || lower.contains("plugin sync")
        || lower.contains("plugin");
    let mentions_403 = lower.contains(" 403")
        || lower.contains("403 ")
        || lower.contains("statuscode: 403")
        || lower.contains("status code 403")
        || lower.contains("http 403");

    mentions_powershell_wrapper
        && mentions_startup_plugin_sync
        && mentions_403
        && !controlled_bypass_allowed(text)
}

fn exact_clean_evidence(text: &str) -> bool {
    verdict_line_count(text) == 1 && clean_verdict_line_count(text) == 1
}

fn source_output_has_exact_clean(path: &std::path::Path) -> bool {
    std::fs::read_to_string(path).is_ok_and(|text| exact_clean_evidence(&text))
}

pub fn clean_verdict_line_count(text: &str) -> usize {
    text.lines()
        .filter(|line| line.trim() == "Verdict: Clean")
        .count()
}

pub fn verdict_line_count(text: &str) -> usize {
    text.lines()
        .filter(|line| line.trim_start().starts_with("Verdict:"))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn aggregate_verdicts_fail_closed() {
        assert_eq!(
            ReviewVerdict::aggregate(ReviewVerdict::Clean, ReviewVerdict::Concern),
            ReviewVerdict::Concern
        );
        assert_eq!(
            ReviewVerdict::aggregate(ReviewVerdict::Concern, ReviewVerdict::Block),
            ReviewVerdict::Block
        );
        assert_eq!(
            ReviewVerdict::aggregate(ReviewVerdict::Clean, ReviewVerdict::ReviewRunError),
            ReviewVerdict::Block
        );
    }

    #[test]
    fn review_input_small_diff_is_single_chunk() {
        let input = build_review_input(
            "h",
            "1 file",
            vec![p("a.rs")],
            vec![],
            vec![],
            12_000,
            50_000,
        );
        assert_eq!(input.chunks.len(), 1);
        assert!(input.omitted_files.is_empty());
        assert!(input.reviewed_subset_reason.is_none());
    }

    #[test]
    fn review_input_mid_diff_prioritizes_critical_files() {
        let input = build_review_input(
            "h",
            "2 files",
            vec![p("a.rs"), p("b.rs")],
            vec![],
            vec![p("b.rs")],
            100_000,
            50_000,
        );
        assert_eq!(input.chunks[0].files, vec![p("b.rs")]);
        assert_eq!(
            input.reviewed_subset_reason.as_deref(),
            Some("critical-files-first")
        );
    }

    #[test]
    fn review_input_large_diff_marks_omitted_files() {
        let input = build_review_input(
            "h",
            "many",
            vec![p("a.rs"), p("b.rs"), p("c.rs")],
            vec![p("b.rs")],
            vec![],
            600_000,
            50_000,
        );
        assert_eq!(input.chunks[0].files, vec![p("b.rs")]);
        assert_eq!(input.omitted_files, vec![p("a.rs"), p("c.rs")]);
    }

    #[test]
    fn controlled_bypass_requires_specific_readonly_failure_shape() {
        let ok = "Verdict: ReviewRunError\nENV_BLOCKED\nWindowsApps\npwsh.exe\nblocked by policy";
        assert!(controlled_bypass_allowed(ok));
        assert!(!controlled_bypass_allowed(
            "Verdict: ReviewRunError\nblocked by policy"
        ));
        assert!(!controlled_bypass_allowed("Verdict: ReviewRunError\nVerdict: ReviewRunError\nENV_BLOCKED\nWindowsApps\npwsh.exe\nblocked by policy"));
    }

    #[test]
    fn plugin_sync_403_powershell_wrapper_failure_retries_formal_readonly_not_bypass() {
        let text = "Verdict: ReviewRunError\nPowerShell NativeCommandError\ncodex.ps1 startup stderr: remote plugin sync failed with HTTP 403";
        assert_eq!(
            classify_failed_readonly_review(text),
            FailedReadonlyReviewAction::RetryFormalReadonlyWithNativeCodex
        );
        assert!(!controlled_bypass_allowed(text));
    }

    #[test]
    fn plugin_sync_retry_classifier_requires_exact_failure_shape() {
        assert_eq!(
            classify_failed_readonly_review("Verdict: ReviewRunError\nplugin sync 403"),
            FailedReadonlyReviewAction::FailClosed
        );
        assert_eq!(
            classify_failed_readonly_review(
                "Verdict: ReviewRunError\nVerdict: ReviewRunError\nPowerShell codex.ps1 plugin sync 403"
            ),
            FailedReadonlyReviewAction::FailClosed
        );
    }

    #[test]
    fn windowsapps_policy_block_still_uses_controlled_bypass_path() {
        let text = "Verdict: ReviewRunError\nENV_BLOCKED\nWindowsApps\npwsh.exe\nblocked by policy";
        assert_eq!(
            classify_failed_readonly_review(text),
            FailedReadonlyReviewAction::ControlledBypassAllowed
        );
    }

    #[test]
    fn controlled_bypass_record_requires_failed_attempt_and_clean_status() {
        let td = tempfile::tempdir().unwrap();
        let review_path = td.path().join("review.md");
        let failed_path = td.path().join("failed.md");
        std::fs::write(&review_path, "Verdict: Clean\nok").unwrap();
        std::fs::write(
            &failed_path,
            "Verdict: ReviewRunError\nENV_BLOCKED\nWindowsApps\npwsh.exe\nblocked by policy",
        )
        .unwrap();
        let mut record = ReviewRunRecord {
            verdict: ReviewVerdict::Clean,
            reviewer: Lane::Codex,
            review_kind: ReviewKind::CodexAdversarialXHigh,
            review_profile_id: "CodexAdversarialXHigh".into(),
            reasoning_effort: "xhigh".into(),
            model_or_profile_id: "gpt".into(),
            prompt_template_version: "v1".into(),
            prompt_hash: "hash".into(),
            command_source_adapter:
                CommandSourceAdapter::CodexDesktopLeadPowershellControlledBypass,
            primary_role: PrimaryRole::Codex,
            session_id: None,
            command_name: None,
            scope: "local diff".into(),
            gate: ReviewGate::PrCreate,
            patch_hash: "patch".into(),
            files_reviewed: vec![PathBuf::from("a.rs")],
            omitted_files: vec![],
            limitations: vec![],
            evidence: "Verdict: Clean\nok".into(),
            required_actions: vec![],
            created_at: Utc::now(),
            source_output_path: review_path,
            failed_readonly_attempt_path: Some(failed_path),
            failed_readonly_attempt_evidence: Some(
                "Verdict: ReviewRunError\nENV_BLOCKED\nWindowsApps\npwsh.exe\nblocked by policy"
                    .into(),
            ),
            status_before: Some("clean".into()),
            status_after: Some("clean".into()),
            review_caused_mutation: false,
        };
        assert!(record.is_gate_complete());
        record.failed_readonly_attempt_evidence = Some("blocked by policy".into());
        assert!(!record.is_gate_complete());
    }

    #[test]
    fn controlled_bypass_record_rejects_extra_verdict_lines() {
        let td = tempfile::tempdir().unwrap();
        let review_path = td.path().join("review.md");
        let failed_path = td.path().join("failed.md");
        std::fs::write(&review_path, "Verdict: Clean\nVerdict: Concern").unwrap();
        std::fs::write(
            &failed_path,
            "Verdict: ReviewRunError\nENV_BLOCKED\nWindowsApps\npwsh.exe\nblocked by policy",
        )
        .unwrap();
        let record = ReviewRunRecord {
            verdict: ReviewVerdict::Clean,
            reviewer: Lane::Codex,
            review_kind: ReviewKind::CodexAdversarialXHigh,
            review_profile_id: "CodexAdversarialXHigh".into(),
            reasoning_effort: "xhigh".into(),
            model_or_profile_id: "gpt".into(),
            prompt_template_version: "v1".into(),
            prompt_hash: "hash".into(),
            command_source_adapter:
                CommandSourceAdapter::CodexDesktopLeadPowershellControlledBypass,
            primary_role: PrimaryRole::Codex,
            session_id: None,
            command_name: None,
            scope: "local diff".into(),
            gate: ReviewGate::PrCreate,
            patch_hash: "patch".into(),
            files_reviewed: vec![PathBuf::from("a.rs")],
            omitted_files: vec![],
            limitations: vec![],
            evidence: "Verdict: Clean\nVerdict: Concern".into(),
            required_actions: vec![],
            created_at: Utc::now(),
            source_output_path: review_path,
            failed_readonly_attempt_path: Some(failed_path),
            failed_readonly_attempt_evidence: Some(
                "Verdict: ReviewRunError\nENV_BLOCKED\nWindowsApps\npwsh.exe\nblocked by policy"
                    .into(),
            ),
            status_before: Some("clean".into()),
            status_after: Some("clean".into()),
            review_caused_mutation: false,
        };
        assert!(!record.is_gate_complete());
    }

    #[test]
    fn controlled_bypass_record_requires_failed_attempt_file_content() {
        let td = tempfile::tempdir().unwrap();
        let review_path = td.path().join("review.md");
        let failed_path = td.path().join("failed.md");
        std::fs::write(&review_path, "Verdict: Clean\nok").unwrap();
        std::fs::write(&failed_path, "arbitrary existing file").unwrap();
        let record = ReviewRunRecord {
            verdict: ReviewVerdict::Clean,
            reviewer: Lane::Codex,
            review_kind: ReviewKind::CodexAdversarialXHigh,
            review_profile_id: "CodexAdversarialXHigh".into(),
            reasoning_effort: "xhigh".into(),
            model_or_profile_id: "gpt".into(),
            prompt_template_version: "v1".into(),
            prompt_hash: "hash".into(),
            command_source_adapter:
                CommandSourceAdapter::CodexDesktopLeadPowershellControlledBypass,
            primary_role: PrimaryRole::Codex,
            session_id: None,
            command_name: None,
            scope: "local diff".into(),
            gate: ReviewGate::PrCreate,
            patch_hash: "patch".into(),
            files_reviewed: vec![PathBuf::from("a.rs")],
            omitted_files: vec![],
            limitations: vec![],
            evidence: "Verdict: Clean\nok".into(),
            required_actions: vec![],
            created_at: Utc::now(),
            source_output_path: review_path,
            failed_readonly_attempt_path: Some(failed_path),
            failed_readonly_attempt_evidence: Some(
                "Verdict: ReviewRunError\nENV_BLOCKED\nWindowsApps\npwsh.exe\nblocked by policy"
                    .into(),
            ),
            status_before: Some("clean".into()),
            status_after: Some("clean".into()),
            review_caused_mutation: false,
        };
        assert!(!record.is_gate_complete());
    }

    #[test]
    fn controlled_bypass_requires_exact_clean_verdict_line() {
        assert_eq!(clean_verdict_line_count("Verdict: Clean\nok"), 1);
        assert_eq!(
            clean_verdict_line_count("Verdict: Clean but conditional"),
            0
        );
        assert_eq!(clean_verdict_line_count("Verdict: Cleanliness"), 0);
    }

    #[test]
    fn normal_gate_record_requires_exact_clean_output_and_codex_reviewer() {
        let td = tempfile::tempdir().unwrap();
        let review_path = td.path().join("review.md");
        std::fs::write(&review_path, "Verdict: Clean\nok").unwrap();
        let mut record = ReviewRunRecord {
            verdict: ReviewVerdict::Clean,
            reviewer: Lane::Codex,
            review_kind: ReviewKind::CodexAdversarialXHigh,
            review_profile_id: "CodexAdversarialXHigh".into(),
            reasoning_effort: "xhigh".into(),
            model_or_profile_id: "gpt".into(),
            prompt_template_version: "v1".into(),
            prompt_hash: "hash".into(),
            command_source_adapter: CommandSourceAdapter::MoaOrchestrator,
            primary_role: PrimaryRole::Codex,
            session_id: None,
            command_name: None,
            scope: "local diff".into(),
            gate: ReviewGate::PrCreate,
            patch_hash: "patch".into(),
            files_reviewed: vec![PathBuf::from("a.rs")],
            omitted_files: vec![],
            limitations: vec![],
            evidence: "Verdict: Clean\nok".into(),
            required_actions: vec![],
            created_at: Utc::now(),
            source_output_path: review_path.clone(),
            failed_readonly_attempt_path: None,
            failed_readonly_attempt_evidence: None,
            status_before: None,
            status_after: None,
            review_caused_mutation: false,
        };
        assert!(record.is_gate_complete());

        record.reviewer = Lane::Claude;
        assert!(!record.is_gate_complete());

        record.reviewer = Lane::Codex;
        record.evidence = "Verdict: Clean but conditional".into();
        assert!(!record.is_gate_complete());

        record.evidence = "Verdict: Clean\nok".into();
        std::fs::write(&review_path, "Verdict: Cleanliness").unwrap();
        assert!(!record.is_gate_complete());
    }
}
