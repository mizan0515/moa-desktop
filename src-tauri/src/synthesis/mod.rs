//! T3 — canonical worker NDJSON schema (Rust mirror of `src/lib/synthesis/types.ts`).
//!
//! FIX-D: this module owns the IPC boundary contract for `orch://event`
//! `kind="line"` payloads. Adapters and the orchestrator drain produce
//! [`WorkerEvent`] values; the frontend `parseWorkerNdjson` consumes the
//! same shape verbatim. The previous code leaked Serde-tagged adapter
//! envelopes (`{kind:"assistant", ...}`) across the boundary, which the
//! frontend parser silently dropped — empty synthesis was the result.
//!
//! Two extraction paths feed this canonical schema:
//! 1. `extract_from_text` — scans assistant text (Claude `stream-json`
//!    `assistant` turn body, Codex `item.completed` agent message) for
//!    NDJSON lines that round-trip through [`WorkerEvent`].
//! 2. `try_passthrough` — accepts a `serde_json::Value` whose top-level
//!    discriminator is already `event`. Mock fixtures (`mockResponses/
//!    *_firstpass.json`) are emitted by `MockRunner` as raw stdout lines
//!    that the adapter wraps as `Other { raw }`; the orchestrator drain
//!    unwraps the `raw` here without paying the parser tax twice.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerName {
    Claude,
    Codex,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    High,
    Med,
    Low,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Citation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<String>,
}

/// Discriminant matches `src/lib/synthesis/types.ts` 1:1.
///
/// The TS parser ignores unknown `event` values, and the Rust mirror does
/// the same on `try_passthrough` — so adding a new variant later is a
/// non-breaking change for older frontends as long as `start | claim |
/// open_question | end` keep their meaning.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum WorkerEvent {
    Start {
        #[serde(skip_serializing_if = "Option::is_none")]
        worker: Option<WorkerName>,
        #[serde(skip_serializing_if = "Option::is_none")]
        phase: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        attempt: Option<u32>,
    },
    Claim {
        id: String,
        text: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        citations: Vec<Citation>,
        #[serde(skip_serializing_if = "Option::is_none")]
        confidence: Option<Confidence>,
        #[serde(skip_serializing_if = "Option::is_none")]
        applicability: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        topic: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        attempt: Option<u32>,
    },
    OpenQuestion {
        id: String,
        text: String,
    },
    End {
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<String>,
    },
}

impl WorkerEvent {
    /// Try to interpret an arbitrary JSON value as a canonical WorkerEvent.
    /// Returns `None` if the discriminator is missing or unrecognised.
    ///
    /// Used by the orchestrator drain to passthrough mock fixture lines
    /// that the adapter wrapped as `Other { raw }`.
    pub fn try_passthrough(raw: &Value) -> Option<Self> {
        // Cheap discriminator probe before the full deserialize.
        let tag = raw.get("event")?.as_str()?;
        if !matches!(tag, "start" | "claim" | "open_question" | "end") {
            return None;
        }
        serde_json::from_value(raw.clone()).ok()
    }
}

/// Scan a free-form text body for NDJSON lines that round-trip as
/// [`WorkerEvent`]. Lines that are not JSON, or are JSON but not canonical,
/// are silently skipped — matching the TS parser's tolerance.
pub fn extract_from_text(text: &str) -> Vec<WorkerEvent> {
    let mut out = Vec::new();
    for line in text.split('\n') {
        let trimmed = line.trim();
        if trimmed.is_empty() || !trimmed.starts_with('{') {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
            if let Some(ev) = WorkerEvent::try_passthrough(&v) {
                out.push(ev);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn passthrough_canonical_claim() {
        let raw = json!({
            "event": "claim",
            "id": "c1",
            "text": "foo",
            "citations": [{"file":"x.rs","line":12}],
            "confidence": "high",
            "applicability": "scope",
            "topic": "t",
            "attempt": 1
        });
        let ev = WorkerEvent::try_passthrough(&raw).expect("canonical claim");
        match ev {
            WorkerEvent::Claim { id, text, citations, confidence, .. } => {
                assert_eq!(id, "c1");
                assert_eq!(text, "foo");
                assert_eq!(citations.len(), 1);
                assert_eq!(confidence, Some(Confidence::High));
            }
            _ => panic!("expected claim"),
        }
    }

    #[test]
    fn passthrough_rejects_adapter_envelope() {
        // The exact shape the orchestrator was leaking pre-FIX-D.
        let raw = json!({"kind":"assistant", "text":"hello", "raw":{}});
        assert!(WorkerEvent::try_passthrough(&raw).is_none());
    }

    #[test]
    fn passthrough_rejects_unknown_event() {
        let raw = json!({"event":"made_up", "id":"x", "text":"y"});
        assert!(WorkerEvent::try_passthrough(&raw).is_none());
    }

    #[test]
    fn extract_from_assistant_text_finds_canonical_lines() {
        let text = "Here is my analysis:\n\
            {\"event\":\"start\",\"worker\":\"claude\",\"phase\":\"firstpass\"}\n\
            {\"event\":\"claim\",\"id\":\"c1\",\"text\":\"x\",\"confidence\":\"med\"}\n\
            some prose\n\
            {\"event\":\"open_question\",\"id\":\"q1\",\"text\":\"why?\"}\n\
            {\"event\":\"end\",\"status\":\"ok\"}\n";
        let evs = extract_from_text(text);
        assert_eq!(evs.len(), 4, "got: {evs:#?}");
        assert!(matches!(evs[0], WorkerEvent::Start { .. }));
        assert!(matches!(evs[1], WorkerEvent::Claim { .. }));
        assert!(matches!(evs[2], WorkerEvent::OpenQuestion { .. }));
        assert!(matches!(evs[3], WorkerEvent::End { .. }));
    }

    #[test]
    fn extract_skips_non_json_and_envelope_lines() {
        let text = "{\"kind\":\"assistant\",\"text\":\"x\"}\n\
            not json {\n\
            {\"event\":\"claim\",\"id\":\"c1\",\"text\":\"y\"}\n";
        let evs = extract_from_text(text);
        assert_eq!(evs.len(), 1);
        assert!(matches!(evs[0], WorkerEvent::Claim { .. }));
    }

    #[test]
    fn mock_fixture_lines_round_trip_as_canonical_events() {
        // Lock the on-disk mock fixtures as canonical: every non-empty,
        // non-comment line must deserialize as a `WorkerEvent`. T8
        // contract — see `mockResponses/{claude,codex}_firstpass.json`.
        let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repo_root = manifest.parent().unwrap().to_path_buf();
        for name in ["claude_firstpass.json", "codex_firstpass.json"] {
            let path = repo_root.join("mockResponses").join(name);
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("read {path:?}: {e}"));
            let mut count = 0;
            for line in text.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let v: Value = serde_json::from_str(trimmed)
                    .unwrap_or_else(|e| panic!("parse {name} line: {e} — {trimmed}"));
                let ev = WorkerEvent::try_passthrough(&v).unwrap_or_else(|| {
                    panic!("{name}: line not canonical WorkerEvent — {trimmed}")
                });
                let _ = serde_json::to_value(&ev).unwrap();
                count += 1;
            }
            assert!(count > 0, "{name} must contain at least one canonical event");
        }
    }

    #[test]
    fn round_trip_serializes_with_event_tag() {
        let ev = WorkerEvent::Claim {
            id: "c1".into(),
            text: "hello".into(),
            citations: vec![],
            confidence: Some(Confidence::Low),
            applicability: None,
            topic: None,
            attempt: None,
        };
        let s = serde_json::to_string(&ev).unwrap();
        assert!(s.starts_with("{\"event\":\"claim\""), "got: {s}");
        // Frontend parser sees `event`, not `kind`.
        assert!(!s.contains("\"kind\""));
    }
}
