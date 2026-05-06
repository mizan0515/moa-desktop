//! Verification step — runs a project-supplied command after mutation lands
//! and reports a structured outcome. The command lives in `settings.json`
//! (project-specific) and defaults to `cargo check && pnpm build`.
//!
//! No-op (`Ok(VerifyOutcome::Skipped)`) when no command is configured.

use std::path::Path;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::time::timeout;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum VerifyOutcome {
    Skipped,
    Passed { duration_ms: u128, stdout_tail: String },
    Failed { exit_code: Option<i32>, duration_ms: u128, stdout_tail: String, stderr_tail: String },
    TimedOut { after_secs: u64 },
}

impl VerifyOutcome {
    pub fn is_ok(&self) -> bool {
        matches!(self, VerifyOutcome::Passed { .. } | VerifyOutcome::Skipped)
    }
}

#[derive(Debug, Clone)]
pub struct VerifySpec {
    /// Shell command line. Run via the platform shell (Windows: `cmd /C`,
    /// otherwise `sh -c`) so users can chain with `&&`.
    pub command: Option<String>,
    pub cwd: std::path::PathBuf,
    /// Hard cap. Default 5 min.
    pub timeout: Duration,
    /// stdout/stderr tail char count returned in the outcome.
    pub tail_chars: usize,
}

impl VerifySpec {
    pub fn new(cwd: impl AsRef<Path>) -> Self {
        Self {
            command: None,
            cwd: cwd.as_ref().to_path_buf(),
            timeout: Duration::from_secs(300),
            tail_chars: 4_000,
        }
    }

    pub fn with_command(mut self, cmd: impl Into<String>) -> Self {
        let s = cmd.into();
        self.command = if s.trim().is_empty() { None } else { Some(s) };
        self
    }

    pub fn with_timeout(mut self, t: Duration) -> Self {
        self.timeout = t;
        self
    }
}

/// Run the configured verify command. Never panics on tool failure — every
/// failure mode is mapped to a `VerifyOutcome` variant.
pub async fn run(spec: VerifySpec) -> VerifyOutcome {
    let cmdline = match &spec.command {
        Some(c) => c.clone(),
        None => return VerifyOutcome::Skipped,
    };

    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/C", &cmdline]);
        c
    } else {
        let mut c = Command::new("sh");
        c.args(["-c", &cmdline]);
        c
    };
    cmd.current_dir(&spec.cwd);
    cmd.kill_on_drop(true);

    let start = Instant::now();
    let fut = cmd.output();

    let output = match timeout(spec.timeout, fut).await {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => {
            let dur = start.elapsed().as_millis();
            return VerifyOutcome::Failed {
                exit_code: None,
                duration_ms: dur,
                stdout_tail: String::new(),
                stderr_tail: format!("verify spawn error: {e}"),
            };
        }
        Err(_) => {
            return VerifyOutcome::TimedOut {
                after_secs: spec.timeout.as_secs(),
            };
        }
    };

    let dur_ms = start.elapsed().as_millis();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let stdout_tail = tail(&stdout, spec.tail_chars);
    let stderr_tail = tail(&stderr, spec.tail_chars);

    if output.status.success() {
        VerifyOutcome::Passed { duration_ms: dur_ms, stdout_tail }
    } else {
        VerifyOutcome::Failed {
            exit_code: output.status.code(),
            duration_ms: dur_ms,
            stdout_tail,
            stderr_tail,
        }
    }
}

fn tail(s: &str, chars: usize) -> String {
    if s.chars().count() <= chars {
        return s.to_string();
    }
    let n_skip = s.chars().count() - chars;
    s.chars().skip(n_skip).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn skipped_when_no_command() {
        let out = run(VerifySpec::new(".")).await;
        assert!(matches!(out, VerifyOutcome::Skipped));
    }

    #[tokio::test]
    async fn passes_on_zero_exit() {
        let cmd = if cfg!(windows) { "cmd /C exit 0" } else { "true" };
        let out = run(VerifySpec::new(".").with_command(cmd)).await;
        assert!(out.is_ok(), "expected Passed, got {out:?}");
    }

    #[tokio::test]
    async fn fails_on_non_zero() {
        let cmd = if cfg!(windows) { "cmd /C exit 1" } else { "false" };
        let out = run(VerifySpec::new(".").with_command(cmd)).await;
        assert!(matches!(out, VerifyOutcome::Failed { .. }));
    }

    #[tokio::test]
    async fn times_out() {
        let cmd = if cfg!(windows) {
            // ping with -n 100 is the conventional way to sleep on Windows
            // when timeout.exe is unreliable in CI.
            "ping -n 100 127.0.0.1 >nul"
        } else {
            "sleep 5"
        };
        let out = run(
            VerifySpec::new(".")
                .with_command(cmd)
                .with_timeout(Duration::from_millis(150)),
        )
        .await;
        assert!(matches!(out, VerifyOutcome::TimedOut { .. }));
    }

    #[test]
    fn tail_truncates() {
        let s = "abcdefghij";
        assert_eq!(tail(s, 4), "ghij");
        assert_eq!(tail(s, 100), "abcdefghij");
    }
}
