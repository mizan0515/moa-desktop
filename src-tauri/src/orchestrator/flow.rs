//! Flow classification heuristic (CODEX-MCP.md § 2.5 결정 트리).
//!
//! Lightweight Rust-side classifier — takes the user's task input + optional
//! file scope hint, returns a default Flow. Frontend may override before the
//! state machine starts (`OrchestrationStart { override_flow: Some(_) }`).
//!
//! Heuristic intentionally simple — anchoring "default to MoA Flow C" per
//! § 핵심 규칙 (false-negative trivial-detection is preferred over
//! false-positive that skips cross-verify).

use crate::orchestrator::state::Flow;

/// Inputs the classifier sees. All optional except `task`.
#[derive(Debug, Clone, Default)]
pub struct ClassifyInput {
    pub task: String,
    /// Files in scope (orchestrator/UI hint). Empty = unknown.
    pub files: Vec<String>,
    /// User-supplied override. If `Some`, classifier returns it verbatim.
    pub user_override: Option<Flow>,
    /// Caller already determined this is research / investigation only.
    pub research_only: bool,
}

/// Default classifier. Bias is **toward MoA Flow C** — only collapse to
/// Flow A when input is unambiguously trivial. Research keywords (ko/en) →
/// Flow D.
pub fn classify(input: &ClassifyInput) -> Flow {
    if let Some(f) = input.user_override {
        return f;
    }
    if input.research_only {
        return Flow::D;
    }

    let task_lc = input.task.to_lowercase();

    // Research / read-only intent — Korean and English markers from
    // CODEX-MCP.md § 2.5 흐름 D.
    let research_markers: &[&str] = &[
        "조사",
        "리서치",
        "research",
        "investigate",
        "디버깅 가설",
        "분석",
        "analyze",
        "explain",
        "review only",
        "read-only",
        "compare",
    ];
    if research_markers.iter().any(|m| task_lc.contains(m)) && !is_explicit_mutation(&task_lc) {
        return Flow::D;
    }

    // Trivial heuristic: single file, short task description, behavior-
    // preserving keywords. Per § 2.5, trivial = "10 줄 미만 + 단일 파일 +
    // behavior-preserving". We can only proxy this from the task string —
    // file count gives us "단일 파일", short task length and keywords give
    // weak signal on the rest.
    let single_file = input.files.len() == 1;
    let short_task = input.task.chars().count() <= 80;
    let trivial_markers: &[&str] = &[
        "typo",
        "오타",
        "rename",
        "리네임",
        "comment",
        "주석",
        "format",
        "포맷",
        "whitespace",
        "공백",
        "docstring",
    ];
    let trivial_word = trivial_markers.iter().any(|m| task_lc.contains(m));

    if single_file && short_task && trivial_word {
        // Mechanical edits or Windows shell tasks bias toward Flow B
        // (Codex-author strength).
        let mechanical_markers: &[&str] =
            &["powershell", "windows", "batch", "regex", "sed-like"];
        if mechanical_markers.iter().any(|m| task_lc.contains(m)) {
            return Flow::B;
        }
        return Flow::A;
    }

    // Default — MoA Flow C.
    Flow::C
}

fn is_explicit_mutation(task_lc: &str) -> bool {
    let markers: &[&str] = &[
        "fix",
        "수정",
        "구현",
        "implement",
        "refactor",
        "리팩터",
        "리팩토링",
        "add",
        "추가",
        "remove",
        "삭제",
        "rename",
        "리네임",
    ];
    markers.iter().any(|m| task_lc.contains(m))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_task_defaults_to_c() {
        let f = classify(&ClassifyInput::default());
        assert_eq!(f, Flow::C);
    }

    #[test]
    fn user_override_wins() {
        let f = classify(&ClassifyInput {
            task: "fix bug".into(),
            user_override: Some(Flow::A),
            ..Default::default()
        });
        assert_eq!(f, Flow::A);
    }

    #[test]
    fn research_keyword_maps_to_d() {
        let f = classify(&ClassifyInput {
            task: "Investigate why the build is slow".into(),
            ..Default::default()
        });
        assert_eq!(f, Flow::D);
    }

    #[test]
    fn research_with_explicit_mutation_intent_demotes_to_c() {
        let f = classify(&ClassifyInput {
            task: "analyze and fix the slow build".into(),
            ..Default::default()
        });
        // Research marker + mutation marker → Flow C wins (real mutation).
        assert_eq!(f, Flow::C);
    }

    #[test]
    fn trivial_single_file_short_task_keyword_maps_to_a() {
        let f = classify(&ClassifyInput {
            task: "fix typo in README".into(),
            files: vec!["README.md".into()],
            ..Default::default()
        });
        assert_eq!(f, Flow::A);
    }

    #[test]
    fn trivial_with_mechanical_marker_maps_to_b() {
        let f = classify(&ClassifyInput {
            task: "rename in PowerShell script".into(),
            files: vec!["run.ps1".into()],
            ..Default::default()
        });
        assert_eq!(f, Flow::B);
    }

    #[test]
    fn multi_file_task_defaults_to_c_even_if_short() {
        let f = classify(&ClassifyInput {
            task: "fix typo".into(),
            files: vec!["a.rs".into(), "b.rs".into()],
            ..Default::default()
        });
        assert_eq!(f, Flow::C);
    }
}
