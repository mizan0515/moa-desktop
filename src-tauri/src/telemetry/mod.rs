//! T9 — telemetry: token counter, cost estimator, version snapshot.
//!
//! Aggregation key = `(project_id, session_id)`. v1 is single-project, but the
//! key is exposed now so Phase 6 multi-project work has zero backtracking.

pub mod cost;
pub mod counter;
pub mod version;

use std::collections::BTreeMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

pub use cost::{cost_usd, evaluate_cap, CapStatus, CostCap, Pricing, Worker};
pub use counter::{extract_usage, Usage};
pub use version::{detect_drift, DriftItem, VersionSnapshot};

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct AggKey {
    pub project_id: String,
    pub session_id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionTelemetry {
    pub claude: Usage,
    pub codex: Usage,
    pub claude_usd: f64,
    pub codex_usd: f64,
}

impl SessionTelemetry {
    pub fn total_usd(&self) -> f64 {
        self.claude_usd + self.codex_usd
    }
    pub fn total_tokens(&self) -> u64 {
        self.claude
            .total_tokens()
            .saturating_add(self.codex.total_tokens())
    }
}

/// Process-wide telemetry aggregator. Keyed by (project_id, session_id).
///
/// This struct is small + cheap to lock; sessions update at most once per
/// turn.completed / result line.
pub struct TelemetryStore {
    inner: Mutex<TelemetryInner>,
}

#[derive(Default)]
struct TelemetryInner {
    by_session: BTreeMap<(String, String), SessionTelemetry>,
    /// USD spent today across all sessions (calendar-day; caller passes a
    /// `today` key so the store stays pure / testable).
    daily_usd: BTreeMap<String, f64>,
    cap: CostCap,
}

impl Default for TelemetryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TelemetryStore {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(TelemetryInner::default()),
        }
    }

    pub fn set_cap(&self, cap: CostCap) {
        self.inner.lock().unwrap().cap = cap;
    }

    pub fn cap(&self) -> CostCap {
        self.inner.lock().unwrap().cap
    }

    /// Record a `Usage` against a session for a given worker, applying the
    /// matching pricing. `today` is an ISO-8601 date (YYYY-MM-DD) used as the
    /// daily aggregation key — kept as a parameter so tests are deterministic.
    pub fn record(
        &self,
        key: &AggKey,
        worker: Worker,
        usage: Usage,
        pricing: &Pricing,
        today: &str,
    ) {
        let mut inner = self.inner.lock().unwrap();
        let entry = inner
            .by_session
            .entry((key.project_id.clone(), key.session_id.clone()))
            .or_default();
        let usd = cost_usd(&usage, pricing);
        match worker {
            Worker::Claude => {
                entry.claude.add(usage);
                entry.claude_usd += usd;
            }
            Worker::Codex => {
                entry.codex.add(usage);
                entry.codex_usd += usd;
            }
        }
        *inner.daily_usd.entry(today.to_string()).or_insert(0.0) += usd;
    }

    pub fn session(&self, key: &AggKey) -> Option<SessionTelemetry> {
        self.inner
            .lock()
            .unwrap()
            .by_session
            .get(&(key.project_id.clone(), key.session_id.clone()))
            .cloned()
    }

    /// Aggregate across one project (sum every session under it).
    pub fn project_total(&self, project_id: &str) -> SessionTelemetry {
        let inner = self.inner.lock().unwrap();
        let mut acc = SessionTelemetry::default();
        for ((pid, _), v) in &inner.by_session {
            if pid == project_id {
                acc.claude.add(v.claude);
                acc.codex.add(v.codex);
                acc.claude_usd += v.claude_usd;
                acc.codex_usd += v.codex_usd;
            }
        }
        acc
    }

    /// Aggregate across all projects.
    pub fn all_total(&self) -> SessionTelemetry {
        let inner = self.inner.lock().unwrap();
        let mut acc = SessionTelemetry::default();
        for v in inner.by_session.values() {
            acc.claude.add(v.claude);
            acc.codex.add(v.codex);
            acc.claude_usd += v.claude_usd;
            acc.codex_usd += v.codex_usd;
        }
        acc
    }

    pub fn daily_usd(&self, today: &str) -> f64 {
        self.inner
            .lock()
            .unwrap()
            .daily_usd
            .get(today)
            .copied()
            .unwrap_or(0.0)
    }

    /// Evaluate cap status for a session against the per-session and daily
    /// caps stored in this telemetry store.
    pub fn cap_status(&self, key: &AggKey, today: &str) -> CapStatus {
        let inner = self.inner.lock().unwrap();
        let session_usd = inner
            .by_session
            .get(&(key.project_id.clone(), key.session_id.clone()))
            .map(|s| s.total_usd())
            .unwrap_or(0.0);
        let daily = inner.daily_usd.get(today).copied().unwrap_or(0.0);
        evaluate_cap(session_usd, daily, &inner.cap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(p: &str, s: &str) -> AggKey {
        AggKey {
            project_id: p.into(),
            session_id: s.into(),
        }
    }

    #[test]
    fn project_scoped_aggregation() {
        let t = TelemetryStore::new();
        let p1s1 = key("p1", "s1");
        let p1s2 = key("p1", "s2");
        let p2s1 = key("p2", "s1");
        let one_million = Usage {
            input: 1_000_000,
            output: 0,
            cache_read: 0,
            cache_create: 0,
        };
        t.record(&p1s1, Worker::Claude, one_million, &Pricing::claude_sonnet(), "2026-05-07");
        t.record(&p1s2, Worker::Claude, one_million, &Pricing::claude_sonnet(), "2026-05-07");
        t.record(&p2s1, Worker::Claude, one_million, &Pricing::claude_sonnet(), "2026-05-07");

        let p1 = t.project_total("p1");
        assert_eq!(p1.claude.input, 2_000_000);
        assert!((p1.claude_usd - 6.0).abs() < 1e-9); // 2M * $3/M

        let p2 = t.project_total("p2");
        assert_eq!(p2.claude.input, 1_000_000);

        let all = t.all_total();
        assert_eq!(all.claude.input, 3_000_000);
        assert!((t.daily_usd("2026-05-07") - 9.0).abs() < 1e-9);
    }

    #[test]
    fn cap_status_reflects_session_and_daily() {
        let t = TelemetryStore::new();
        t.set_cap(CostCap {
            per_session_usd: 1.0,
            daily_usd: 10.0,
            warn_at: 0.8,
        });
        let k = key("p", "s");
        let small = Usage {
            input: 100_000, // $0.30 on Sonnet
            output: 0,
            cache_read: 0,
            cache_create: 0,
        };
        t.record(&k, Worker::Claude, small, &Pricing::claude_sonnet(), "d");
        assert_eq!(t.cap_status(&k, "d"), CapStatus::Ok);

        // Push session over warn threshold (0.8 * $1)
        let push = Usage {
            input: 200_000, // +$0.60 → $0.90 session
            output: 0,
            cache_read: 0,
            cache_create: 0,
        };
        t.record(&k, Worker::Claude, push, &Pricing::claude_sonnet(), "d");
        assert_eq!(t.cap_status(&k, "d"), CapStatus::Warn);

        // Exceed
        let over = Usage {
            input: 100_000, // +$0.30 → $1.20 session
            output: 0,
            cache_read: 0,
            cache_create: 0,
        };
        t.record(&k, Worker::Claude, over, &Pricing::claude_sonnet(), "d");
        assert_eq!(t.cap_status(&k, "d"), CapStatus::Exceeded);
    }

    #[test]
    fn codex_subscription_does_not_inflate_usd() {
        let t = TelemetryStore::new();
        let k = key("p", "s");
        let big = Usage {
            input: 5_000_000,
            output: 5_000_000,
            cache_read: 0,
            cache_create: 0,
        };
        t.record(&k, Worker::Codex, big, &Pricing::codex_subscription(), "d");
        let s = t.session(&k).unwrap();
        assert_eq!(s.codex.input, 5_000_000);
        assert_eq!(s.codex_usd, 0.0);
        assert_eq!(t.daily_usd("d"), 0.0);
    }
}
