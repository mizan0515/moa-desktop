//! Adversarial round prompt builder (CODEX-MCP.md § 2.6 템플릿 D).
//!
//! Pure helper — given synthesis JSON + original task, produces the prompt
//! string fed to `adapter.firstpass()` for the adversarial reviewer Worker.
//! The reviewer is always whichever Lane did NOT synthesize (Codex default,
//! since Claude is user-facing session holder per § 2.5).

use crate::orchestrator::state::Lane;

const TEMPLATE: &str = r#"[ADVERSARIAL REVIEW — read-only, mutation prohibited]

You are reviewing a synthesis produced by the orchestrator after both Claude
and Codex completed independent first-pass analyses on the task below. Your
job is **not** to redo the first-pass. It is to find what's wrong with the
synthesis: missing possibilities, uncovered angles, silent averaging, and
operational-intent vs ticket-letter drift.

## Original task
{{task}}

## Synthesis (5-column schema)
BEGIN_SYNTHESIS_JSON
{{synthesis_json}}
END_SYNTHESIS_JSON

## Required review categories (address each, even if "no findings")
1. **Missing possibilities / counter-examples** — claims the synthesis treats
   as settled but have plausible counter-cases.
2. **Uncovered angle** — questions the original task implies but the synthesis
   never asked.
3. **Silent averaging** — places where the two first-passes disagreed but the
   synthesis blurred the disagreement instead of preserving it.
4. **Operational intent vs ticket-letter** — the synthesis verifies the
   letter of the ask but missed product/operational intent.

## Output contract (structured)
- ## Verdict: PASS | BLOCKER | NEED_CLARIFICATION (one of these three)
- ## Blockers: bulleted list (empty if PASS)
- ## Counter-examples: bulleted list with concrete repro / file paths
- ## Required follow-up: ordered list of actions before mutation can proceed
- ## Claim Ledger (max 5): each row = claim · evidence · level (L1/L2/L3) ·
  conf (high/med/low) · residual risk

## Constraints
- Read-only. No Edit/Write. No `git` mutations.
- If you cannot verify a claim with evidence, mark it UNVERIFIED — do not
  silently downgrade to "looks fine".
- Round counter is currently {{round}} of max {{max_rounds}}; if you return
  BLOCKER, the orchestrator may run another round (with you as reviewer
  again) up to that ceiling.
"#;

/// Build the adversarial-review prompt body. Caller passes synthesis result
/// already serialized to JSON (the T3 deterministic merge output).
pub fn render_prompt(
    task: &str,
    synthesis_json: &str,
    round: u32,
    max_rounds: u32,
) -> String {
    TEMPLATE
        .replace("{{task}}", task)
        .replace("{{synthesis_json}}", synthesis_json)
        .replace("{{round}}", &round.to_string())
        .replace("{{max_rounds}}", &max_rounds.to_string())
}

/// Default reviewer assignment per CODEX-MCP.md § 2.5: "Codex default, since
/// Claude is user-facing session holder". Returns the Lane that should run
/// the adversarial round.
pub fn default_reviewer(synthesizer: Lane) -> Lane {
    match synthesizer {
        Lane::Claude | Lane::System => Lane::Codex,
        Lane::Codex => Lane::Claude,
    }
}

/// Verdict extracted from the reviewer's terminal output. Conservative —
/// any text containing `BLOCKER` (case-sensitive, on a `## Verdict:` line or
/// bullet) is treated as BLOCKER. The orchestrator does not parse the full
/// claim ledger Rust-side; that's the frontend's job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Blocker,
    NeedClarification,
    Unknown,
}

/// FIX-F — outcome of a single adversarial round, decoupled from the I/O
/// loop so the gate can be unit-tested without spawning an adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdversarialDecision {
    /// PASS — synthesis approved, mutation may proceed.
    Approved,
    /// BLOCKER — re-run adversarial round (still under `max_rounds`).
    Retry { next_round: u32 },
    /// BLOCKER persists at `max_rounds` — escalate to user, fail session.
    EscalateBlocker { round: u32 },
    /// NEED_CLARIFICATION or UNKNOWN — escalate to user, **mutation must
    /// not run**. Pre-FIX-F the loop returned `Ok(())` for both branches,
    /// which silently let an unverified synthesis through to the mutation
    /// owner. The frontend got the `escalation` event but the Rust driver
    /// kept walking.
    EscalateNoMutation { reason: &'static str, round: u32 },
}

/// Pure decision: given the parsed `Verdict` plus the current round /
/// ceiling, what does the orchestrator do next?
pub fn decide(verdict: Verdict, round: u32, max_rounds: u32) -> AdversarialDecision {
    match verdict {
        Verdict::Pass => AdversarialDecision::Approved,
        Verdict::Unknown => AdversarialDecision::EscalateNoMutation {
            reason: "verdict-unknown",
            round,
        },
        Verdict::NeedClarification => AdversarialDecision::EscalateNoMutation {
            reason: "need-clarification",
            round,
        },
        Verdict::Blocker => {
            if round >= max_rounds {
                AdversarialDecision::EscalateBlocker { round }
            } else {
                AdversarialDecision::Retry { next_round: round + 1 }
            }
        }
    }
}

pub fn parse_verdict(reviewer_output: &str) -> Verdict {
    // Look for a line beginning with `## Verdict:` (case-insensitive ws).
    for line in reviewer_output.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed
            .strip_prefix("## Verdict:")
            .or_else(|| trimmed.strip_prefix("## verdict:"))
            .or_else(|| trimmed.strip_prefix("## VERDICT:"))
        {
            let upper = rest.trim().to_uppercase();
            if upper.contains("BLOCKER") {
                return Verdict::Blocker;
            }
            if upper.contains("NEED_CLARIFICATION") || upper.contains("NEED CLARIFICATION") {
                return Verdict::NeedClarification;
            }
            if upper.contains("PASS") {
                return Verdict::Pass;
            }
            return Verdict::Unknown;
        }
    }
    Verdict::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_substitutes_all_placeholders() {
        let p = render_prompt("do X", r#"{"verified":[]}"#, 2, 3);
        assert!(p.contains("do X"));
        assert!(p.contains(r#"{"verified":[]}"#));
        assert!(p.contains("currently 2 of max 3"));
        assert!(!p.contains("{{"));
    }

    #[test]
    fn default_reviewer_pairs_with_synthesizer() {
        assert_eq!(default_reviewer(Lane::Claude), Lane::Codex);
        assert_eq!(default_reviewer(Lane::Codex), Lane::Claude);
        assert_eq!(default_reviewer(Lane::System), Lane::Codex);
    }

    #[test]
    fn verdict_pass() {
        let s = "## Verdict: PASS\n\n## Blockers: none";
        assert_eq!(parse_verdict(s), Verdict::Pass);
    }

    #[test]
    fn verdict_blocker() {
        let s = "## Verdict: BLOCKER\n\n- claim X is wrong";
        assert_eq!(parse_verdict(s), Verdict::Blocker);
    }

    #[test]
    fn verdict_need_clarification() {
        let s = "## Verdict: NEED_CLARIFICATION";
        assert_eq!(parse_verdict(s), Verdict::NeedClarification);
    }

    #[test]
    fn verdict_unknown_when_missing() {
        assert_eq!(parse_verdict("no verdict here"), Verdict::Unknown);
    }

    // ─── FIX-F: gate decision ─────────────────────────────────────────────

    #[test]
    fn decide_pass_approves() {
        assert_eq!(decide(Verdict::Pass, 1, 3), AdversarialDecision::Approved);
        assert_eq!(decide(Verdict::Pass, 3, 3), AdversarialDecision::Approved);
    }

    #[test]
    fn decide_unknown_blocks_mutation() {
        let d = decide(Verdict::Unknown, 1, 3);
        assert_eq!(
            d,
            AdversarialDecision::EscalateNoMutation {
                reason: "verdict-unknown",
                round: 1
            }
        );
    }

    #[test]
    fn decide_need_clarification_blocks_mutation() {
        let d = decide(Verdict::NeedClarification, 2, 3);
        assert_eq!(
            d,
            AdversarialDecision::EscalateNoMutation {
                reason: "need-clarification",
                round: 2
            }
        );
    }

    #[test]
    fn decide_blocker_below_max_retries() {
        assert_eq!(
            decide(Verdict::Blocker, 1, 3),
            AdversarialDecision::Retry { next_round: 2 }
        );
        assert_eq!(
            decide(Verdict::Blocker, 2, 3),
            AdversarialDecision::Retry { next_round: 3 }
        );
    }

    #[test]
    fn decide_blocker_at_max_escalates() {
        assert_eq!(
            decide(Verdict::Blocker, 3, 3),
            AdversarialDecision::EscalateBlocker { round: 3 }
        );
        // Defensive: never retry past the ceiling even if input is malformed.
        assert_eq!(
            decide(Verdict::Blocker, 4, 3),
            AdversarialDecision::EscalateBlocker { round: 4 }
        );
    }
}
