//! Cost calculation + cap policy.
//!
//! Pricing values are public defaults from Anthropic's pricing page (USD per
//! 1M tokens, Sonnet 4.5 / Opus 4.x baseline). They are **estimates** — actual
//! billing is the source of truth. UI must surface "estimated".
//!
//! Codex (ChatGPT subscription) is reported as $0 — Codex usage is billed
//! against the user's subscription, not per-token. Tokens are still reported
//! as a usage signal.

use serde::{Deserialize, Serialize};

use super::counter::Usage;

/// Worker identity for pricing lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Worker {
    Claude,
    Codex,
}

/// USD per 1M tokens.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Pricing {
    pub input_per_mtok: f64,
    pub output_per_mtok: f64,
    pub cache_read_per_mtok: f64,
    pub cache_create_per_mtok: f64,
}

impl Pricing {
    /// Sonnet 4.x baseline (estimate; check Anthropic pricing page).
    pub const fn claude_sonnet() -> Self {
        Self {
            input_per_mtok: 3.0,
            output_per_mtok: 15.0,
            cache_read_per_mtok: 0.30,
            cache_create_per_mtok: 3.75,
        }
    }
    /// Opus 4.x baseline (estimate).
    pub const fn claude_opus() -> Self {
        Self {
            input_per_mtok: 15.0,
            output_per_mtok: 75.0,
            cache_read_per_mtok: 1.50,
            cache_create_per_mtok: 18.75,
        }
    }
    /// Codex via ChatGPT subscription — no per-token charge to user.
    pub const fn codex_subscription() -> Self {
        Self {
            input_per_mtok: 0.0,
            output_per_mtok: 0.0,
            cache_read_per_mtok: 0.0,
            cache_create_per_mtok: 0.0,
        }
    }
}

/// Compute estimated USD cost for a single `Usage`.
pub fn cost_usd(usage: &Usage, p: &Pricing) -> f64 {
    let m = 1_000_000.0;
    (usage.input as f64) * p.input_per_mtok / m
        + (usage.output as f64) * p.output_per_mtok / m
        + (usage.cache_read as f64) * p.cache_read_per_mtok / m
        + (usage.cache_create as f64) * p.cache_create_per_mtok / m
}

/// Cost cap policy.
///
/// Two independent gates: per-session and global-per-day. Reaching either
/// triggers `Exceeded` and the orchestrator must not start a new run without
/// user confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CostCap {
    /// USD cap for a single session.
    pub per_session_usd: f64,
    /// USD cap aggregated across all sessions for the current day (global).
    pub daily_usd: f64,
    /// Soft warning threshold as a fraction of either cap (e.g. 0.8 = 80%).
    pub warn_at: f64,
}

impl Default for CostCap {
    fn default() -> Self {
        // Ticket spec: per-session $10, global $30/day.
        Self {
            per_session_usd: 10.0,
            daily_usd: 30.0,
            warn_at: 0.8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CapStatus {
    Ok,
    Warn,
    Exceeded,
}

/// Evaluate a cap given a session-level cost and a daily-aggregate cost.
pub fn evaluate_cap(session_usd: f64, daily_usd: f64, cap: &CostCap) -> CapStatus {
    let exceeded = session_usd >= cap.per_session_usd || daily_usd >= cap.daily_usd;
    if exceeded {
        return CapStatus::Exceeded;
    }
    let session_ratio = session_usd / cap.per_session_usd.max(f64::EPSILON);
    let daily_ratio = daily_usd / cap.daily_usd.max(f64::EPSILON);
    if session_ratio >= cap.warn_at || daily_ratio >= cap.warn_at {
        return CapStatus::Warn;
    }
    CapStatus::Ok
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(input: u64, output: u64, cache_read: u64, cache_create: u64) -> Usage {
        Usage {
            input,
            output,
            cache_read,
            cache_create,
        }
    }

    #[test]
    fn cost_usd_for_sonnet_million_in_million_out() {
        let c = cost_usd(&u(1_000_000, 1_000_000, 0, 0), &Pricing::claude_sonnet());
        // $3 in + $15 out = $18
        assert!((c - 18.0).abs() < 1e-9);
    }

    #[test]
    fn cost_usd_for_codex_subscription_is_zero() {
        let c = cost_usd(
            &u(10_000_000, 10_000_000, 10_000_000, 10_000_000),
            &Pricing::codex_subscription(),
        );
        assert_eq!(c, 0.0);
    }

    #[test]
    fn evaluate_cap_ok_warn_exceeded_thresholds() {
        let cap = CostCap {
            per_session_usd: 10.0,
            daily_usd: 30.0,
            warn_at: 0.8,
        };
        assert_eq!(evaluate_cap(1.0, 1.0, &cap), CapStatus::Ok);
        assert_eq!(evaluate_cap(8.0, 0.0, &cap), CapStatus::Warn); // 80% session
        assert_eq!(evaluate_cap(0.0, 24.0, &cap), CapStatus::Warn); // 80% daily
        assert_eq!(evaluate_cap(10.0, 0.0, &cap), CapStatus::Exceeded);
        assert_eq!(evaluate_cap(0.0, 30.0, &cap), CapStatus::Exceeded);
    }

    #[test]
    fn cost_estimate_mixes_cache_lines() {
        // 1M cache_read on Sonnet = $0.30 ; 1M cache_create = $3.75
        let c = cost_usd(&u(0, 0, 1_000_000, 1_000_000), &Pricing::claude_sonnet());
        assert!((c - 4.05).abs() < 1e-9);
    }
}
