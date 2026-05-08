//! T13 L4 — privileged slash command registry.

use serde::{Deserialize, Serialize};

use crate::policy::review::{ReviewGate, ReviewRunRecord};
use crate::policy::PrimaryRole;
use crate::safety::{scan_text, RoleContext, ScanResult, ScanSource};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Permission {
    ReadOnly,
    MetaDispatch,
    SessionMgmt,
    DestructiveNetwork,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandSpec {
    pub name: &'static str,
    pub permission: Permission,
    pub steps: &'static [&'static str],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchPreview {
    pub name: String,
    pub permission: Permission,
    pub steps: Vec<String>,
    pub requires_step_confirm: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlashReviewContext {
    pub session_id: String,
    pub patch_hash: String,
}

pub fn registry() -> Vec<CommandSpec> {
    vec![
        CommandSpec {
            name: "/다음세션",
            permission: Permission::SessionMgmt,
            steps: &["emit-resume-packet"],
        },
        CommandSpec {
            name: "/쉽게",
            permission: Permission::ReadOnly,
            steps: &["brief-current-session"],
        },
        CommandSpec {
            name: "/진행",
            permission: Permission::MetaDispatch,
            steps: &["brief", "recommend-next-command"],
        },
        CommandSpec {
            name: "/백로그",
            permission: Permission::DestructiveNetwork,
            steps: &[
                "preview-app-backlog-write",
                "confirm-github-issue-api",
                "mirror-project-memory",
            ],
        },
        CommandSpec {
            name: "/메인동기화",
            permission: Permission::DestructiveNetwork,
            steps: &[
                "guard-clean-branch",
                "pre-pr-review-gate",
                "push-and-pr-create",
                "pre-merge-review-gate",
                "merge-main-and-pull",
            ],
        },
        CommandSpec {
            name: "/병행티켓",
            permission: Permission::SessionMgmt,
            steps: &["call-ticket-decomposer"],
        },
        CommandSpec {
            name: "/병행통합",
            permission: Permission::DestructiveNetwork,
            steps: &[
                "load-lane-results",
                "integrate-merge-review-gate",
                "main-apply-review-gate",
            ],
        },
    ]
}

pub fn dispatch_preview(name: &str) -> Result<DispatchPreview, String> {
    let spec = registry()
        .into_iter()
        .find(|spec| spec.name == name)
        .ok_or_else(|| format!("unknown slash command: {name}"))?;
    Ok(DispatchPreview {
        name: spec.name.into(),
        permission: spec.permission,
        steps: spec.steps.iter().map(|s| (*s).into()).collect(),
        requires_step_confirm: matches!(spec.permission, Permission::DestructiveNetwork),
    })
}

pub fn confirm_step_allowed(
    preview: &DispatchPreview,
    step_index: usize,
    user_confirmed: bool,
    review_records: &[ReviewRunRecord],
    review_context: Option<&SlashReviewContext>,
) -> Result<(), String> {
    let canonical = dispatch_preview(&preview.name)?;
    if step_index >= canonical.steps.len() {
        return Err("step index out of bounds".into());
    }
    if canonical.requires_step_confirm && !user_confirmed {
        return Err("destructive step requires explicit user confirm".into());
    }
    let required_review_gates = canonical
        .steps
        .iter()
        .take(step_index + 1)
        .filter_map(|step| review_gate_for_step(step))
        .collect::<Vec<_>>();
    if !required_review_gates.is_empty() && review_context.is_none() {
        return Err("review gate context is required before continuing".into());
    }
    for gate in required_review_gates {
        let context = review_context.expect("review context checked above");
        let has_gate_record = review_records
            .iter()
            .filter(|record| record.gate == gate)
            .filter(|record| record.patch_hash == context.patch_hash)
            .filter(|record| record.command_name.as_deref() == Some(preview.name.as_str()))
            .filter(|record| record.session_id.as_deref() == Some(context.session_id.as_str()))
            .filter(|record| {
                review_records
                    .iter()
                    .filter(|other| other.gate == record.gate)
                    .filter(|other| other.patch_hash == record.patch_hash)
                    .count()
                    == 1
            })
            .any(ReviewRunRecord::is_gate_complete);
        if !has_gate_record {
            return Err(format!(
                "review gate {gate:?} must be Clean before continuing"
            ));
        }
    }
    Ok(())
}

fn review_gate_for_step(step: &str) -> Option<ReviewGate> {
    match step {
        "pre-pr-review-gate" => Some(ReviewGate::PrCreate),
        "pre-merge-review-gate" => Some(ReviewGate::PrMerge),
        "integrate-merge-review-gate" => Some(ReviewGate::IntegrateMerge),
        "main-apply-review-gate" => Some(ReviewGate::MainApply),
        _ => None,
    }
}

pub fn scan_slash_output(text: &str) -> ScanResult {
    scan_text(
        text,
        RoleContext {
            primary_role: PrimaryRole::Claude,
            source: ScanSource::SlashCommand,
        },
    )
}

#[tauri::command]
pub fn slash_dispatch_preview(name: String) -> Result<DispatchPreview, String> {
    dispatch_preview(&name)
}

#[tauri::command]
pub fn slash_confirm_step(
    preview: DispatchPreview,
    step_index: usize,
    user_confirmed: bool,
) -> Result<bool, String> {
    confirm_step_allowed(&preview, step_index, user_confirmed, &[], None)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use chrono::Utc;

    use crate::orchestrator::state::Lane;
    use crate::policy::review::{CommandSourceAdapter, ReviewKind, ReviewVerdict};

    fn review_context() -> SlashReviewContext {
        SlashReviewContext {
            session_id: "session-1".into(),
            patch_hash: "patch".into(),
        }
    }

    fn complete_review_record(source_output_path: PathBuf, gate: ReviewGate) -> ReviewRunRecord {
        ReviewRunRecord {
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
            session_id: Some("session-1".into()),
            command_name: Some("/메인동기화".into()),
            scope: "local diff".into(),
            gate,
            patch_hash: "patch".into(),
            files_reviewed: vec![PathBuf::from("src-tauri/src/commands/mod.rs")],
            omitted_files: vec![],
            limitations: vec![],
            evidence: "Verdict: Clean\nok".into(),
            required_actions: vec![],
            created_at: Utc::now(),
            source_output_path,
            failed_readonly_attempt_path: None,
            failed_readonly_attempt_evidence: None,
            status_before: None,
            status_after: None,
            review_caused_mutation: false,
        }
    }

    #[test]
    fn registry_has_all_seven_privileged_slash_commands() {
        let names = registry().into_iter().map(|s| s.name).collect::<Vec<_>>();
        assert_eq!(names.len(), 7);
        assert!(names.contains(&"/메인동기화"));
        assert!(names.contains(&"/백로그"));
        assert!(names.contains(&"/병행통합"));
    }

    #[test]
    fn destructive_network_requires_step_confirm() {
        let preview = dispatch_preview("/메인동기화").unwrap();
        let err = confirm_step_allowed(&preview, 0, false, &[], None).unwrap_err();
        assert!(err.contains("confirm"));
    }

    #[test]
    fn review_gate_blocks_non_clean_verdicts() {
        let preview = dispatch_preview("/메인동기화").unwrap();
        assert!(confirm_step_allowed(&preview, 1, true, &[], Some(&review_context())).is_err());
    }

    #[test]
    fn review_gate_requires_complete_server_record() {
        let td = tempfile::tempdir().unwrap();
        let output_path = td.path().join("review.md");
        std::fs::write(&output_path, "Verdict: Clean\nok").unwrap();
        let record = complete_review_record(output_path, ReviewGate::PrCreate);
        let preview = dispatch_preview("/메인동기화").unwrap();
        assert!(
            confirm_step_allowed(&preview, 1, true, &[record], Some(&review_context())).is_ok()
        );
    }

    #[test]
    fn later_destructive_steps_require_prior_review_gates() {
        let preview = dispatch_preview("/메인동기화").unwrap();
        assert!(confirm_step_allowed(&preview, 2, true, &[], Some(&review_context())).is_err());

        let td = tempfile::tempdir().unwrap();
        let output_path = td.path().join("review.md");
        std::fs::write(&output_path, "Verdict: Clean\nok").unwrap();
        let record = complete_review_record(output_path, ReviewGate::PrCreate);
        assert!(
            confirm_step_allowed(&preview, 2, true, &[record], Some(&review_context())).is_ok()
        );
    }

    #[test]
    fn later_review_gates_require_matching_gate_identity() {
        let preview = dispatch_preview("/메인동기화").unwrap();
        let td = tempfile::tempdir().unwrap();
        let pr_output = td.path().join("pr.md");
        let duplicate_output = td.path().join("duplicate-pr.md");
        let merge_output = td.path().join("merge.md");
        std::fs::write(&pr_output, "Verdict: Clean\nok").unwrap();
        std::fs::write(&duplicate_output, "Verdict: Clean\nok").unwrap();
        std::fs::write(&merge_output, "Verdict: Clean\nok").unwrap();

        let pr = complete_review_record(pr_output, ReviewGate::PrCreate);
        let duplicate_pr = complete_review_record(duplicate_output, ReviewGate::PrCreate);
        assert!(confirm_step_allowed(
            &preview,
            4,
            true,
            &[pr.clone(), duplicate_pr],
            Some(&review_context())
        )
        .is_err());

        let merge = complete_review_record(merge_output, ReviewGate::PrMerge);
        assert!(
            confirm_step_allowed(&preview, 4, true, &[pr, merge], Some(&review_context())).is_ok()
        );
    }

    #[test]
    fn review_gate_rejects_stale_patch_or_session_context() {
        let preview = dispatch_preview("/메인동기화").unwrap();
        let td = tempfile::tempdir().unwrap();
        let output_path = td.path().join("review.md");
        std::fs::write(&output_path, "Verdict: Clean\nok").unwrap();
        let mut record = complete_review_record(output_path, ReviewGate::PrCreate);
        record.session_id = Some("session-10".into());
        let stale = SlashReviewContext {
            session_id: "session-1".into(),
            patch_hash: "patch".into(),
        };
        assert!(confirm_step_allowed(&preview, 1, true, &[record], Some(&stale)).is_err());
    }

    #[test]
    fn tauri_confirm_command_cannot_forge_review_gate() {
        let preview = dispatch_preview("/메인동기화").unwrap();
        assert!(slash_confirm_step(preview, 1, true).is_err());
    }

    #[test]
    fn confirm_step_uses_server_registry_not_forged_preview_fields() {
        let forged = DispatchPreview {
            name: "/메인동기화".into(),
            permission: Permission::ReadOnly,
            steps: vec!["safe".into()],
            requires_step_confirm: false,
        };
        assert!(confirm_step_allowed(&forged, 0, false, &[], None).is_err());
    }

    #[test]
    fn slash_output_scanner_rejects_nested_peer_call() {
        assert!(!scan_slash_output("worker should run codex exec").is_clean());
    }
}
