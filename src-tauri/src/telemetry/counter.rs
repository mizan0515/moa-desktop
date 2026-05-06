//! Token usage counter — extracts `Usage` from Claude / Codex stream-json
//! events and accumulates it.
//!
//! Source shapes (verified against existing adapter parsers):
//! * Claude `result` line carries `{"usage": { "input_tokens", "output_tokens",
//!   "cache_creation_input_tokens", "cache_read_input_tokens" }}`.
//! * Codex `turn.completed` carries `{"usage": { "input_tokens",
//!   "output_tokens", "cached_input_tokens" }}` (S2 spike). Cache-creation
//!   is not reported by Codex — we leave it 0.
//!
//! Estimates only — Anthropic billing is authoritative. The UI must say so.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_create: u64,
}

impl Usage {
    pub fn add(&mut self, other: Usage) {
        self.input = self.input.saturating_add(other.input);
        self.output = self.output.saturating_add(other.output);
        self.cache_read = self.cache_read.saturating_add(other.cache_read);
        self.cache_create = self.cache_create.saturating_add(other.cache_create);
    }
    pub fn total_tokens(&self) -> u64 {
        self.input
            .saturating_add(self.output)
            .saturating_add(self.cache_read)
            .saturating_add(self.cache_create)
    }
}

/// Extract `Usage` from any JSON object that carries a `usage` sub-object.
/// Returns `None` if no `usage` key exists or all fields are missing.
pub fn extract_usage(raw: &Value) -> Option<Usage> {
    let usage = raw.get("usage").or_else(|| {
        // Some Codex variants nest under `turn.usage` or `result.usage`.
        raw.get("turn")
            .and_then(|t| t.get("usage"))
            .or_else(|| raw.get("result").and_then(|r| r.get("usage")))
    })?;

    let get = |k: &str| usage.get(k).and_then(|v| v.as_u64()).unwrap_or(0);
    let input = get("input_tokens");
    let output = get("output_tokens");
    // Claude
    let claude_cache_read = get("cache_read_input_tokens");
    let claude_cache_create = get("cache_creation_input_tokens");
    // Codex
    let codex_cache_read = get("cached_input_tokens");

    let cache_read = claude_cache_read.max(codex_cache_read);
    let cache_create = claude_cache_create;

    if input == 0 && output == 0 && cache_read == 0 && cache_create == 0 {
        return None;
    }
    Some(Usage {
        input,
        output,
        cache_read,
        cache_create,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_claude_result_usage() {
        let v = json!({
            "type": "result",
            "is_error": false,
            "num_turns": 1,
            "usage": {
                "input_tokens": 100,
                "output_tokens": 50,
                "cache_creation_input_tokens": 200,
                "cache_read_input_tokens": 0
            }
        });
        let u = extract_usage(&v).unwrap();
        assert_eq!(u.input, 100);
        assert_eq!(u.output, 50);
        assert_eq!(u.cache_create, 200);
        assert_eq!(u.cache_read, 0); // S8: fresh session = 0 cache reuse
    }

    #[test]
    fn extract_codex_turn_completed_usage() {
        let v = json!({
            "type": "turn.completed",
            "usage": {
                "input_tokens": 80,
                "output_tokens": 120,
                "cached_input_tokens": 5
            }
        });
        let u = extract_usage(&v).unwrap();
        assert_eq!(u.input, 80);
        assert_eq!(u.output, 120);
        assert_eq!(u.cache_read, 5);
        assert_eq!(u.cache_create, 0);
    }

    #[test]
    fn extract_returns_none_when_no_usage() {
        let v = json!({"type": "assistant"});
        assert!(extract_usage(&v).is_none());
    }

    #[test]
    fn extract_returns_none_when_all_zero() {
        let v = json!({"usage": {"input_tokens": 0, "output_tokens": 0}});
        assert!(extract_usage(&v).is_none());
    }

    #[test]
    fn usage_add_saturates() {
        let mut a = Usage {
            input: u64::MAX - 1,
            ..Default::default()
        };
        a.add(Usage { input: 10, ..Default::default() });
        assert_eq!(a.input, u64::MAX);
    }

    #[test]
    fn total_tokens_sums_all_fields() {
        let u = Usage {
            input: 1,
            output: 2,
            cache_read: 4,
            cache_create: 8,
        };
        assert_eq!(u.total_tokens(), 15);
    }
}
