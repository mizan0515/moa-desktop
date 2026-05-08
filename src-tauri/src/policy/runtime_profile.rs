//! T13 L3 — safe subset of external runtime settings.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeProfile {
    pub env_allowlist: BTreeMap<String, String>,
    pub always_thinking_enabled: Option<bool>,
    pub show_thinking_summaries: Option<bool>,
    pub permissions: Value,
    pub enabled_plugins: Vec<String>,
    pub extra_known_marketplaces: Vec<String>,
    pub auto_updates_channel: Option<String>,
    pub hooks_hash: BTreeMap<String, String>,
    pub status_line_hash: Option<String>,
    pub excluded_secret_keys: Vec<String>,
}

const ENV_ALLOWLIST: &[&str] = &[
    "CODEX_HOME",
    "CODEX_SHELL",
    "CODEX_INTERNAL_ORIGINATOR_OVERRIDE",
    "CODEX_COMPANION_FORCE_DIRECT_APP_SERVER",
    "ENABLE_CLAUDEAI_MCP_SERVERS",
    "MAX_THINKING_TOKENS",
    "MAX_MCP_OUTPUT_TOKENS",
];

const SECRET_MARKERS: &[&str] = &["token", "auth", "cookie", "secret", "credential", "session"];

pub fn import_safe_subset(raw: &Value) -> RuntimeProfile {
    let mut profile = RuntimeProfile::default();
    if let Some(env) = raw.get("env").and_then(Value::as_object) {
        for key in ENV_ALLOWLIST {
            if let Some(value) = env.get(*key).and_then(Value::as_str) {
                profile.env_allowlist.insert((*key).into(), value.into());
            }
        }
        for key in env.keys() {
            if SECRET_MARKERS
                .iter()
                .any(|m| key.to_lowercase().contains(m))
            {
                profile.excluded_secret_keys.push(format!("env.{key}"));
            }
        }
    }

    profile.always_thinking_enabled = raw.get("alwaysThinkingEnabled").and_then(Value::as_bool);
    profile.show_thinking_summaries = raw.get("showThinkingSummaries").and_then(Value::as_bool);
    profile.permissions = raw
        .get("permissions")
        .map(import_safe_permissions)
        .unwrap_or(Value::Null);
    profile.extra_known_marketplaces = string_array(raw.get("extraKnownMarketplaces"));
    profile.auto_updates_channel = raw
        .get("autoUpdatesChannel")
        .and_then(Value::as_str)
        .map(str::to_string);
    profile.enabled_plugins = raw
        .get("plugins")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|v| {
                    v.get("name")
                        .or_else(|| v.get("id"))
                        .and_then(Value::as_str)
                })
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    profile
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn import_safe_permissions(value: &Value) -> Value {
    let Some(map) = value.as_object() else {
        return Value::Null;
    };
    let mut out = serde_json::Map::new();
    for key in ["deny", "ask"] {
        if let Some(items) = map.get(key).and_then(Value::as_array) {
            let safe_items = items
                .iter()
                .filter_map(Value::as_str)
                .filter(|s| !contains_secret_marker(s))
                .map(|s| Value::String(s.to_string()))
                .collect::<Vec<_>>();
            out.insert(key.to_string(), Value::Array(safe_items));
        }
    }
    Value::Object(out)
}

fn contains_secret_marker(value: &str) -> bool {
    let lower = value.to_lowercase();
    SECRET_MARKERS.iter().any(|marker| lower.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn safe_subset_imports_allowlisted_env_and_excludes_secrets() {
        let raw = json!({
            "env": {
                "CODEX_HOME": "C:/x",
                "API_TOKEN": "do-not-copy",
                "RANDOM": "ignored"
            },
            "alwaysThinkingEnabled": true,
            "showThinkingSummaries": false,
            "extraKnownMarketplaces": ["internal"]
        });
        let profile = import_safe_subset(&raw);
        assert_eq!(profile.env_allowlist["CODEX_HOME"], "C:/x");
        assert!(!profile.env_allowlist.contains_key("API_TOKEN"));
        assert!(profile
            .excluded_secret_keys
            .contains(&"env.API_TOKEN".to_string()));
        assert_eq!(profile.extra_known_marketplaces, vec!["internal"]);
    }

    #[test]
    fn permissions_import_only_safe_deny_and_ask_string_arrays() {
        let raw = json!({
            "permissions": {
                "deny": ["Write"],
                "allow": ["Bash(curl https://example)"],
                "authToken": "do-not-copy",
                "ask": ["Bash(git status:*)", "TOKEN-bearing-command"]
            }
        });
        let profile = import_safe_subset(&raw);
        assert_eq!(profile.permissions["deny"], json!(["Write"]));
        assert_eq!(profile.permissions["ask"], json!(["Bash(git status:*)"]));
        assert!(profile.permissions.get("allow").is_none());
        assert!(profile.permissions.get("authToken").is_none());
    }
}
