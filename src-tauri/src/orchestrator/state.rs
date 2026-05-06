//! T7-full — session state, phase, lane, flow, worker mode.
//!
//! Public types only. No transition logic — the driver lives in `mod.rs`.
//! Phase is a *superset* of `dryrun::Phase` (which we cannot extend without
//! touching the T7-thin NEVER 영역). Same kebab-case serde so UI envelope
//! shape is compatible.

use serde::{Deserialize, Serialize};

use crate::lock::manager::Worker;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    Preflight,
    Classify,
    FirstPass,
    Synthesis,
    Adversarial,
    Mutation,
    Verify,
    Final,
}

impl Phase {
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::Preflight => "preflight",
            Phase::Classify => "classify",
            Phase::FirstPass => "first-pass",
            Phase::Synthesis => "synthesis",
            Phase::Adversarial => "adversarial",
            Phase::Mutation => "mutation",
            Phase::Verify => "verify",
            Phase::Final => "final",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lane {
    System,
    Claude,
    Codex,
}

impl Lane {
    pub fn as_str(self) -> &'static str {
        match self {
            Lane::System => "system",
            Lane::Claude => "claude",
            Lane::Codex => "codex",
        }
    }

    pub fn to_worker(self) -> Option<Worker> {
        match self {
            Lane::Claude => Some(Worker::Claude),
            Lane::Codex => Some(Worker::Codex),
            Lane::System => None,
        }
    }
}

/// MoA flow classification (CODEX-MCP.md § 2.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Flow {
    /// Trivial — Claude author, optional Codex review.
    A,
    /// Trivial mechanical — Codex author, optional Claude review.
    B,
    /// Non-trivial code mutation — full MoA round.
    C,
    /// Research / read-only investigation.
    D,
}

impl Flow {
    pub fn as_str(self) -> &'static str {
        match self {
            Flow::A => "a",
            Flow::B => "b",
            Flow::C => "c",
            Flow::D => "d",
        }
    }

    /// Whether this flow requires the full first-pass × 2 → synthesis →
    /// adversarial → mutation pipeline. Flow A/B short-circuits.
    pub fn needs_full_moa(self) -> bool {
        matches!(self, Flow::C | Flow::D)
    }

    /// Whether mutation is part of the pipeline. D is read-only.
    pub fn produces_mutation(self) -> bool {
        matches!(self, Flow::A | Flow::B | Flow::C)
    }
}

/// Per-worker invocation mode. Maps to `adapter.firstpass()` vs
/// `adapter.mutation()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkerMode {
    /// Read-only first-pass (Flow C/D step 1, also adversarial review).
    FirstPass,
    /// Read-only adversarial review of synthesis (re-uses firstpass argv but
    /// prompt embeds synthesis JSON — see `adversarial.rs`).
    AdversarialReview,
    /// Write-enabled mutation owner. Holds T4 lane lock.
    Mutation,
}

impl WorkerMode {
    pub fn as_str(self) -> &'static str {
        match self {
            WorkerMode::FirstPass => "first-pass",
            WorkerMode::AdversarialReview => "adversarial-review",
            WorkerMode::Mutation => "mutation",
        }
    }
}

/// Top-level session state. Mutation/Verify/Final are reachable only when
/// the chosen Flow `produces_mutation()`.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SessionState {
    Idle,
    Classifying,
    FirstPassRunning {
        flow: Flow,
        claude_done: bool,
        codex_done: bool,
    },
    AwaitingSynthesis {
        flow: Flow,
    },
    AwaitingAdversarial {
        flow: Flow,
        round: u32,
    },
    AwaitingMutationConfirm {
        flow: Flow,
        round: u32,
        mutation_owner: Lane,
    },
    Mutating {
        flow: Flow,
        owner: Lane,
    },
    Verifying {
        flow: Flow,
    },
    Final {
        flow: Flow,
        ok: bool,
    },
    Failed {
        message: String,
    },
    Cancelled,
}

impl SessionState {
    pub fn kind(&self) -> &'static str {
        match self {
            SessionState::Idle => "idle",
            SessionState::Classifying => "classifying",
            SessionState::FirstPassRunning { .. } => "first-pass-running",
            SessionState::AwaitingSynthesis { .. } => "awaiting-synthesis",
            SessionState::AwaitingAdversarial { .. } => "awaiting-adversarial",
            SessionState::AwaitingMutationConfirm { .. } => "awaiting-mutation-confirm",
            SessionState::Mutating { .. } => "mutating",
            SessionState::Verifying { .. } => "verifying",
            SessionState::Final { .. } => "final",
            SessionState::Failed { .. } => "failed",
            SessionState::Cancelled => "cancelled",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            SessionState::Final { .. } | SessionState::Failed { .. } | SessionState::Cancelled
        )
    }
}

/// Constraint: max adversarial rounds before user escalation. Per ticket §
/// "max 3 round, 초과 시 사용자 escalation".
pub const MAX_ADVERSARIAL_ROUNDS: u32 = 3;
