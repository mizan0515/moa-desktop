//! Process runner error types (PLAN.md § F6).
//!
//! Strict to the F6 enum. The runner classifies a small subset of variants
//! itself (`cli-missing`, `timeout`, `killed`, plus `oom` from Windows
//! NTSTATUS). All other variants are adapter responsibility — the runner
//! returns a `ProcessExit { code, stderr_tail, .. }` and adapters refine
//! based on protocol-specific knowledge.

use serde::Serialize;
use std::fmt;
use thiserror::Error;

/// PLAN.md § F6 typed error categories.
///
/// Wire format is kebab-case to match the frontend `processEvents.ts` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProcessErrorKind {
    CliMissing,
    PermissionDenied,
    Spawn,
    AuthExpired,
    Quota,
    Network,
    SandboxDenied,
    MalformedJson,
    Timeout,
    Oom,
    Killed,
    TestFail,
}

impl fmt::Display for ProcessErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::CliMissing => "cli-missing",
            Self::PermissionDenied => "permission-denied",
            Self::Spawn => "spawn",
            Self::AuthExpired => "auth-expired",
            Self::Quota => "quota",
            Self::Network => "network",
            Self::SandboxDenied => "sandbox-denied",
            Self::MalformedJson => "malformed-json",
            Self::Timeout => "timeout",
            Self::Oom => "oom",
            Self::Killed => "killed",
            Self::TestFail => "test-fail",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Error, Serialize)]
pub struct ProcessError {
    pub kind: ProcessErrorKind,
    pub message: String,
    pub exit_code: Option<i32>,
    pub stderr_tail: String,
}

impl fmt::Display for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)
    }
}

impl ProcessError {
    pub fn cli_missing(program: &str, source: std::io::Error) -> Self {
        Self {
            kind: ProcessErrorKind::CliMissing,
            message: format!("failed to spawn {program:?}: {source}"),
            exit_code: None,
            stderr_tail: String::new(),
        }
    }

    pub fn permission_denied(program: &str, source: std::io::Error) -> Self {
        Self {
            kind: ProcessErrorKind::PermissionDenied,
            message: format!("permission denied spawning {program:?}: {source}"),
            exit_code: None,
            stderr_tail: String::new(),
        }
    }

    pub fn spawn_failed(program: &str, source: std::io::Error) -> Self {
        Self {
            kind: ProcessErrorKind::Spawn,
            message: format!("spawn {program:?} failed: {source}"),
            exit_code: None,
            stderr_tail: String::new(),
        }
    }

    pub fn empty_argv() -> Self {
        Self {
            kind: ProcessErrorKind::CliMissing,
            message: "argv is empty".to_string(),
            exit_code: None,
            stderr_tail: String::new(),
        }
    }

    pub fn timeout(stderr_tail: String) -> Self {
        Self {
            kind: ProcessErrorKind::Timeout,
            message: "wait timed out; child aborted".to_string(),
            exit_code: None,
            stderr_tail,
        }
    }

    pub fn killed(stderr_tail: String) -> Self {
        Self {
            kind: ProcessErrorKind::Killed,
            message: "child aborted by caller".to_string(),
            exit_code: None,
            stderr_tail,
        }
    }

    pub fn supervisor_dropped() -> Self {
        Self {
            kind: ProcessErrorKind::Killed,
            message: "process supervisor task ended without publishing exit".to_string(),
            exit_code: None,
            stderr_tail: String::new(),
        }
    }

    pub fn io(message: impl Into<String>) -> Self {
        Self {
            kind: ProcessErrorKind::Killed,
            message: message.into(),
            exit_code: None,
            stderr_tail: String::new(),
        }
    }
}

/// Best-effort OOM detection from a Windows NTSTATUS exit code.
///
/// Substring-based detection is deliberately rejected (false-positive risk
/// from prompts/output mentioning "out of memory"). Adapters may refine on
/// stderr_tail if needed.
#[cfg(target_os = "windows")]
pub(crate) fn is_windows_oom_exit(code: i32) -> bool {
    let u = code as u32;
    matches!(u, 0xC0000017 | 0xC000012D)
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn is_windows_oom_exit(_code: i32) -> bool {
    false
}

/// Redact a key/value diagnostic pair if the key matches secret patterns.
///
/// `raw stderr_tail` is left untouched — it carries process-emitted text and
/// is required intact for adapters' protocol parsing. This helper is only
/// for env-derived diagnostic logs.
pub fn redact_env_pair(key: &str, value: &str) -> (String, String) {
    let upper = key.to_ascii_uppercase();
    let secret = upper.contains("API_KEY")
        || upper.contains("TOKEN")
        || upper.contains("SECRET")
        || upper.contains("CREDENTIAL")
        || upper.contains("PASSWORD");
    if secret {
        (key.to_string(), "[redacted]".to_string())
    } else {
        (key.to_string(), value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_secret_keys() {
        assert_eq!(redact_env_pair("ANTHROPIC_API_KEY", "sk-x").1, "[redacted]");
        assert_eq!(redact_env_pair("MY_TOKEN", "abc").1, "[redacted]");
        assert_eq!(redact_env_pair("MY_SECRET", "abc").1, "[redacted]");
        assert_eq!(redact_env_pair("AWS_CREDENTIAL", "abc").1, "[redacted]");
        assert_eq!(redact_env_pair("PATH", "/usr/bin").1, "/usr/bin");
    }

    #[test]
    fn kind_display_is_kebab_case() {
        assert_eq!(ProcessErrorKind::CliMissing.to_string(), "cli-missing");
        assert_eq!(
            ProcessErrorKind::SandboxDenied.to_string(),
            "sandbox-denied"
        );
        assert_eq!(ProcessErrorKind::AuthExpired.to_string(), "auth-expired");
    }
}
