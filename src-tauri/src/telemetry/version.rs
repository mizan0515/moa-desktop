//! CLI / app version snapshot + drift detection.
//!
//! Captured at session start. Two snapshots from different sessions in the
//! same project drifting on any field surfaces a dismissible warning so users
//! can correlate behavior changes with toolchain changes.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionSnapshot {
    pub claude_cli: Option<String>,
    pub codex_cli: Option<String>,
    pub app: String,
    /// e.g. `"openai-codex@1.0.4"` for the Codex MCP plugin.
    pub plugin: Option<String>,
    /// ISO-8601 capture time.
    pub captured_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriftItem {
    pub field: String,
    pub previous: Option<String>,
    pub current: Option<String>,
}

/// Compare two snapshots field-by-field. Returns drifted items (empty = no
/// drift). `captured_at` is intentionally excluded — it always differs.
pub fn detect_drift(prev: &VersionSnapshot, curr: &VersionSnapshot) -> Vec<DriftItem> {
    let mut out = Vec::new();
    macro_rules! cmp {
        ($field:literal, $prev:expr, $curr:expr) => {
            if $prev != $curr {
                out.push(DriftItem {
                    field: $field.to_string(),
                    previous: $prev.clone().map(|s: String| s),
                    current: $curr.clone().map(|s: String| s),
                });
            }
        };
    }
    cmp!("claude_cli", prev.claude_cli, curr.claude_cli);
    cmp!("codex_cli", prev.codex_cli, curr.codex_cli);
    if prev.app != curr.app {
        out.push(DriftItem {
            field: "app".into(),
            previous: Some(prev.app.clone()),
            current: Some(curr.app.clone()),
        });
    }
    cmp!("plugin", prev.plugin, curr.plugin);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(claude: &str, codex: &str, app: &str) -> VersionSnapshot {
        VersionSnapshot {
            claude_cli: Some(claude.into()),
            codex_cli: Some(codex.into()),
            app: app.into(),
            plugin: None,
            captured_at: "2026-05-07T00:00:00Z".into(),
        }
    }

    #[test]
    fn no_drift_when_identical_modulo_capture_time() {
        let mut a = snap("1.2.3", "0.9.0", "0.1.0");
        let mut b = a.clone();
        b.captured_at = "2026-05-07T01:00:00Z".into();
        assert!(detect_drift(&a, &b).is_empty());
        // sanity
        a.captured_at = "x".into();
        assert!(detect_drift(&a, &b).is_empty());
    }

    #[test]
    fn drift_reports_each_changed_field() {
        let prev = snap("1.2.3", "0.9.0", "0.1.0");
        let mut curr = snap("1.2.4", "0.9.0", "0.1.0");
        curr.app = "0.1.1".into();
        let d = detect_drift(&prev, &curr);
        assert_eq!(d.len(), 2);
        assert!(d.iter().any(|i| i.field == "claude_cli"));
        assert!(d.iter().any(|i| i.field == "app"));
    }

    #[test]
    fn drift_handles_none_to_some() {
        let mut prev = snap("1.0", "1.0", "0.1.0");
        prev.plugin = None;
        let mut curr = prev.clone();
        curr.plugin = Some("openai-codex@1.0.4".into());
        let d = detect_drift(&prev, &curr);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].field, "plugin");
        assert_eq!(d[0].previous, None);
        assert_eq!(d[0].current.as_deref(), Some("openai-codex@1.0.4"));
    }
}
