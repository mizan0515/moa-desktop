//! T9 telemetry — integration-style tests across the public module surface.

use moa_desktop_lib::telemetry::{
    cost_usd, detect_drift, evaluate_cap, extract_usage, AggKey, CapStatus, CostCap, Pricing,
    TelemetryStore, Usage, VersionSnapshot, Worker,
};
use serde_json::json;

#[test]
fn extract_usage_from_claude_result_event() {
    let raw = json!({
        "type": "result",
        "is_error": false,
        "num_turns": 1,
        "usage": {
            "input_tokens": 1234,
            "output_tokens": 567,
            "cache_creation_input_tokens": 200,
            "cache_read_input_tokens": 0
        }
    });
    let u = extract_usage(&raw).unwrap();
    assert_eq!(u.input, 1234);
    assert_eq!(u.output, 567);
    assert_eq!(u.cache_create, 200);
    // Fresh-session reuse is 0 (TOKEN-GUARD § prompt cache awareness).
    assert_eq!(u.cache_read, 0);
}

#[test]
fn extract_usage_from_codex_turn_completed() {
    let raw = json!({
        "type": "turn.completed",
        "usage": {
            "input_tokens": 90,
            "output_tokens": 110,
            "cached_input_tokens": 4
        }
    });
    let u = extract_usage(&raw).unwrap();
    assert_eq!(u.input, 90);
    assert_eq!(u.output, 110);
    assert_eq!(u.cache_read, 4);
}

#[test]
fn project_scoped_aggregation_separates_projects() {
    let store = TelemetryStore::new();
    let p1 = AggKey { project_id: "p1".into(), session_id: "s1".into() };
    let p2 = AggKey { project_id: "p2".into(), session_id: "s1".into() };
    let one_million_in = Usage { input: 1_000_000, output: 0, cache_read: 0, cache_create: 0 };
    store.record(&p1, Worker::Claude, one_million_in, &Pricing::claude_sonnet(), "2026-05-07");
    store.record(&p2, Worker::Claude, one_million_in, &Pricing::claude_sonnet(), "2026-05-07");

    let s1 = store.session(&p1).unwrap();
    assert_eq!(s1.claude.input, 1_000_000);
    let proj1 = store.project_total("p1");
    assert_eq!(proj1.claude.input, 1_000_000);
    assert_eq!(proj1.codex.input, 0);
    let all = store.all_total();
    assert_eq!(all.claude.input, 2_000_000);
}

#[test]
fn cost_cap_default_matches_ticket_spec() {
    let cap = CostCap::default();
    assert_eq!(cap.per_session_usd, 10.0);
    assert_eq!(cap.daily_usd, 30.0);
}

#[test]
fn cost_cap_evaluates_either_gate() {
    let cap = CostCap { per_session_usd: 10.0, daily_usd: 30.0, warn_at: 0.8 };
    assert_eq!(evaluate_cap(0.0, 0.0, &cap), CapStatus::Ok);
    assert_eq!(evaluate_cap(8.0, 0.0, &cap), CapStatus::Warn);
    assert_eq!(evaluate_cap(0.0, 30.0, &cap), CapStatus::Exceeded);
}

#[test]
fn cost_for_codex_subscription_is_zero() {
    let u = Usage { input: 10_000_000, output: 10_000_000, cache_read: 0, cache_create: 0 };
    assert_eq!(cost_usd(&u, &Pricing::codex_subscription()), 0.0);
}

#[test]
fn version_drift_detected_across_fields() {
    let prev = VersionSnapshot {
        claude_cli: Some("1.0.0".into()),
        codex_cli: Some("0.5.0".into()),
        app: "0.1.0".into(),
        plugin: None,
        captured_at: "2026-05-06T00:00:00Z".into(),
    };
    let mut curr = prev.clone();
    curr.claude_cli = Some("1.0.1".into());
    curr.captured_at = "2026-05-07T00:00:00Z".into();
    let d = detect_drift(&prev, &curr);
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].field, "claude_cli");
}
