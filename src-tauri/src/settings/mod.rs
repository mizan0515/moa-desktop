//! T13 settings safe subset exposed to the UI.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::policy::PrimaryRole;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default)]
    pub primary_role: PrimaryRole,
    #[serde(default)]
    pub policy_sync_mode: PolicySyncMode,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            primary_role: PrimaryRole::Claude,
            policy_sync_mode: PolicySyncMode::Manual,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PolicySyncMode {
    #[default]
    Manual,
    TrustedSafeAuto,
}

pub fn settings_path() -> PathBuf {
    home_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(".moa-desktop")
        .join("settings.json")
}

pub fn load_settings_from_disk() -> AppSettings {
    let path = settings_path();
    let Ok(text) = fs::read_to_string(path) else {
        return AppSettings::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn save_settings_to_disk(settings: &AppSettings) -> Result<(), String> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("settings mkdir: {e}"))?;
    }
    let text = serde_json::to_string_pretty(settings).map_err(|e| format!("settings json: {e}"))?;
    fs::write(path, text).map_err(|e| format!("settings write: {e}"))
}

#[tauri::command]
pub fn settings_load() -> Result<AppSettings, String> {
    Ok(load_settings_from_disk())
}

#[tauri::command]
pub fn settings_save(settings: AppSettings) -> Result<AppSettings, String> {
    save_settings_to_disk(&settings)?;
    Ok(settings)
}

fn home_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    } else {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_settings_defaults_to_claude_manual_policy() {
        let s = AppSettings::default();
        assert_eq!(s.primary_role, PrimaryRole::Claude);
        assert_eq!(s.policy_sync_mode, PolicySyncMode::Manual);
    }

    #[test]
    fn app_settings_rejects_unknown_primary_role() {
        let raw = r#"{"primaryRole":"pi"}"#;
        assert!(serde_json::from_str::<AppSettings>(raw).is_err());
    }
}
