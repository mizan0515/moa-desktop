//! T5b — Codex CLI adapter.
//!
//! Builds argv for `codex exec ...` per `spikes/S8-final-templates.md` and
//! parses the line-delimited JSON ("--json" stream) output into
//! [`CodexEvent`]s.
//!
//! Two modes:
//! * [`firstpass`](CodexAdapter::firstpass) — `--sandbox read-only`. Mutation
//!   attempts are blocked by Codex's sandbox engine.
//! * [`mutation`](CodexAdapter::mutation) — Windows-specific:
//!   `--dangerously-bypass-approvals-and-sandbox` inside an isolated
//!   worktree (S2 finding #5: `workspace-write` is broken on Windows).
//!   Caller is responsible for ensuring `worktree_path` is under
//!   `~/.moa-desktop/worktrees/<sid>/`.
//!
//! Differences from the Claude adapter (T5a):
//! * Codex has no system-prompt flag — Worker guard is **prefixed** to the
//!   prompt body.
//! * Prompt is the **last argv element** (positional), not stdin. Stdin is
//!   piped and closed immediately to avoid the
//!   "Reading additional input from stdin..." infinite wait (S2 finding #4).
//! * `CODEX_HOME` must point at a non-temp directory (S2 finding #7) — the
//!   caller wires this through [`CodexConfig::env`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::process::traits::StdinPolicy;
use crate::process::{
    ProcessControl, ProcessError, ProcessExit, ProcessHandle, ProcessLine, ProcessRunner,
    ProcessSpec, Stream as PStream,
};

/// Static config built at startup from `~/.moa-desktop/settings.json` +
/// `prompts/workers/*.txt`.
#[derive(Debug, Clone)]
pub struct CodexConfig {
    /// Resolved `codex.exe` path (S2 finding #1: never `codex.cmd`).
    pub program: PathBuf,
    /// `-c model_reasoning_effort="<value>"`. `"minimal"` is rejected — it's
    /// API-incompatible with `web_search`/`image_gen` (S2 finding #3).
    pub reasoning_effort: String,
    /// `-c web_search="<value>"`. Spike-confirmed values: `"live"`,
    /// `"cached"`, `"disabled"`.
    pub web_search: String,
    /// `-c approval_policy="<value>"`. Default `"never"` (non-interactive).
    pub approval_policy: String,
    /// Worker guard text prefixed onto every prompt (Codex has no
    /// `--append-system-prompt` analogue).
    pub guard_text: String,
    /// First-pass prompt template; `{{task}}` and `{{files}}` substituted.
    pub firstpass_template: String,
    /// Mutation prompt template; `{{task}}` and `{{worktree}}` substituted.
    pub mutation_template: String,
    /// Spawn env. Must include `CODEX_HOME` pointing at a non-temp directory
    /// (S2 finding #7) plus the standard Windows env-inherit set
    /// (PATH, USERPROFILE, APPDATA, LOCALAPPDATA, SystemRoot, TEMP, TMP).
    pub env: HashMap<String, String>,
}

impl CodexConfig {
    /// Load templates + guard from `prompts/workers/`. `prompts_dir` must
    /// contain `codex_guard.txt`, `codex_firstpass_template.txt`,
    /// `codex_mutation_template.txt`.
    pub fn from_dir(
        program: PathBuf,
        prompts_dir: &Path,
        codex_home: PathBuf,
    ) -> std::io::Result<Self> {
        let read = |name: &str| std::fs::read_to_string(prompts_dir.join(name));
        let guard_text = read("codex_guard.txt")?;
        let firstpass_template = read("codex_firstpass_template.txt")?;
        let mutation_template = read("codex_mutation_template.txt")?;
        let mut env = HashMap::new();
        env.insert("CODEX_HOME".into(), codex_home.to_string_lossy().into_owned());
        Ok(Self {
            program,
            reasoning_effort: "high".into(),
            web_search: "live".into(),
            approval_policy: "never".into(),
            guard_text,
            firstpass_template,
            mutation_template,
            env,
        })
    }
}

/// Caller-supplied input for a read-only first-pass run.
#[derive(Debug, Clone)]
pub struct FirstPassRequest {
    pub task: String,
    /// Lines like `src/foo.rs:10-40` — substituted into the template's
    /// `{{files}}` token verbatim.
    pub files: Vec<String>,
    /// cwd (typically the session worktree root).
    pub cwd: PathBuf,
}

/// Caller-supplied input for a mutation-owner run.
#[derive(Debug, Clone)]
pub struct MutationRequest {
    pub task: String,
    /// Isolated worktree path. Caller is responsible for ensuring this is
    /// under `~/.moa-desktop/worktrees/<sid>/` (T4 lock manager).
    pub worktree_path: PathBuf,
}

/// Higher-level event type emitted to consumers (T7 orchestrator).
///
/// Mirrors the Codex `--json` line shapes documented in S2 / S8:
/// `thread.started`, `turn.started`, `item.started`, `item.completed`,
/// `turn.completed`, `turn.failed`, `error`. Plus runner-injected `Exit` /
/// `Stderr` / `MalformedJson`. Non-blocking warnings (e.g. `[features]
/// .web_search deprecated`) surface as `Other` with a normalized type.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CodexEvent {
    /// `type=thread.started` — first event Codex emits.
    ThreadStarted {
        thread_id: Option<String>,
        raw: Value,
    },
    /// `type=turn.started` — model began a turn.
    TurnStarted { raw: Value },
    /// `type=item.started` — a tool call / message item started.
    ItemStarted {
        item_type: Option<String>,
        raw: Value,
    },
    /// `type=item.completed`. `error_message` is set when the item carried
    /// a non-fatal error payload (e.g. deprecation warning, sandbox-deny).
    ItemCompleted {
        item_type: Option<String>,
        error_message: Option<String>,
        raw: Value,
    },
    /// `type=turn.completed` — terminal success for the turn.
    TurnCompleted { raw: Value },
    /// `type=turn.failed` — terminal failure. `error_message` extracted
    /// from `error.message` when present.
    TurnFailed {
        error_message: Option<String>,
        raw: Value,
    },
    /// `type=error` — fatal error event, distinct from `turn.failed`.
    Error {
        message: Option<String>,
        raw: Value,
    },
    /// Any other `type=*` line (forward-compat — schema is sparsely
    /// documented; we don't drop unknowns).
    Other { type_: String, raw: Value },
    /// stderr passthrough. Spike S2 noted benign warnings (chatgpt.com 403,
    /// PowerShell shell snapshot, MCP client missing) — orchestrator may
    /// log-only.
    Stderr { line: String },
    /// A line that did not parse as JSON.
    MalformedJson { line: String, error: String },
    /// Process terminated. `failed` is `true` when any `TurnFailed` /
    /// `Error` event was seen during the run, so consumers don't have to
    /// reconstruct it.
    Exit { exit: ProcessExit, failed: bool },
}

/// Stream returned to consumers — control + parsed events.
pub struct CodexStream {
    pub control: ProcessControl,
    pub events: mpsc::Receiver<CodexEvent>,
}

/// Adapter façade — holds the runner and the static config.
pub struct CodexAdapter {
    runner: Arc<dyn ProcessRunner>,
    config: CodexConfig,
}

impl CodexAdapter {
    pub fn new(runner: Arc<dyn ProcessRunner>, config: CodexConfig) -> Self {
        Self { runner, config }
    }

    pub fn config(&self) -> &CodexConfig {
        &self.config
    }

    /// Build the argv for a first-pass run. Pure — exposed for tests.
    /// `cwd` and `prompt` are baked into argv (Codex consumes them as
    /// `--cd <cwd>` and a trailing positional respectively).
    pub fn firstpass_argv(&self, cwd: &Path, prompt: &str) -> Vec<String> {
        vec![
            self.config.program.to_string_lossy().into_owned(),
            "exec".into(),
            "--ephemeral".into(),
            "-c".into(),
            format!("approval_policy=\"{}\"", self.config.approval_policy),
            "-c".into(),
            format!("model_reasoning_effort=\"{}\"", self.config.reasoning_effort),
            "-c".into(),
            format!("web_search=\"{}\"", self.config.web_search),
            "--sandbox".into(),
            "read-only".into(),
            "--json".into(),
            "--cd".into(),
            cwd.to_string_lossy().into_owned(),
            "--skip-git-repo-check".into(),
            prompt.to_string(),
        ]
    }

    /// Build the argv for a mutation run. Pure — exposed for tests.
    /// Uses `--dangerously-bypass-approvals-and-sandbox` (Windows-required;
    /// `workspace-write` is broken — S2 finding #5). Caller MUST ensure
    /// `worktree` is an isolated path under `~/.moa-desktop/worktrees/`.
    pub fn mutation_argv(&self, worktree: &Path, prompt: &str) -> Vec<String> {
        vec![
            self.config.program.to_string_lossy().into_owned(),
            "exec".into(),
            "--ephemeral".into(),
            "-c".into(),
            format!("approval_policy=\"{}\"", self.config.approval_policy),
            "-c".into(),
            format!("model_reasoning_effort=\"{}\"", self.config.reasoning_effort),
            "-c".into(),
            format!("web_search=\"{}\"", self.config.web_search),
            "--dangerously-bypass-approvals-and-sandbox".into(),
            "--json".into(),
            "--cd".into(),
            worktree.to_string_lossy().into_owned(),
            "--skip-git-repo-check".into(),
            prompt.to_string(),
        ]
    }

    /// Render the first-pass prompt body — guard prefix + template
    /// substitution. Codex has no system-prompt flag (S8) so the guard
    /// must live inside the prompt itself.
    pub fn render_firstpass_prompt(&self, req: &FirstPassRequest) -> String {
        let files = if req.files.is_empty() {
            "(none specified)".to_string()
        } else {
            req.files
                .iter()
                .map(|f| format!("- {f}"))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let body = self
            .config
            .firstpass_template
            .replace("{{task}}", &req.task)
            .replace("{{files}}", &files);
        format!("{}\n\n{}", self.config.guard_text.trim_end(), body)
    }

    pub fn render_mutation_prompt(&self, req: &MutationRequest) -> String {
        let body = self
            .config
            .mutation_template
            .replace("{{task}}", &req.task)
            .replace("{{worktree}}", &req.worktree_path.to_string_lossy());
        format!("{}\n\n{}", self.config.guard_text.trim_end(), body)
    }

    /// Spawn a read-only first-pass run. Returns immediately; events stream
    /// asynchronously through the returned [`CodexStream`].
    pub async fn firstpass(&self, req: FirstPassRequest) -> Result<CodexStream, ProcessError> {
        let prompt = self.render_firstpass_prompt(&req);
        let argv = self.firstpass_argv(&req.cwd, &prompt);
        self.spawn(argv, req.cwd).await
    }

    /// Spawn a mutation-owner run inside `req.worktree_path`.
    pub async fn mutation(&self, req: MutationRequest) -> Result<CodexStream, ProcessError> {
        let prompt = self.render_mutation_prompt(&req);
        let argv = self.mutation_argv(&req.worktree_path, &prompt);
        let cwd = req.worktree_path.clone();
        self.spawn(argv, cwd).await
    }

    async fn spawn(
        &self,
        argv: Vec<String>,
        cwd: PathBuf,
    ) -> Result<CodexStream, ProcessError> {
        // Codex requires stdin closed immediately or it prints
        // "Reading additional input from stdin..." and waits forever
        // (S2 finding #4). The runner closes the pipe synchronously when
        // CloseImmediately is set.
        let spec = ProcessSpec::new(argv, cwd)
            .with_env(self.config.env.clone())
            .with_stdin(StdinPolicy::CloseImmediately);

        let ProcessHandle { control, lines } = self.runner.spawn(spec).await?;

        let (tx, rx) = mpsc::channel::<CodexEvent>(256);
        let exit_watch = control.clone();
        tokio::spawn(parser_task(lines, tx, exit_watch));

        Ok(CodexStream { control, events: rx })
    }
}

async fn parser_task(
    mut lines: mpsc::Receiver<ProcessLine>,
    tx: mpsc::Sender<CodexEvent>,
    control: ProcessControl,
) {
    let mut failed = false;

    while let Some(pl) = lines.recv().await {
        match pl.stream {
            PStream::Stderr => {
                if tx.send(CodexEvent::Stderr { line: pl.line }).await.is_err() {
                    return;
                }
            }
            PStream::Stdout => {
                let event = parse_stdout_line(&pl.line, &mut failed);
                if tx.send(event).await.is_err() {
                    return;
                }
            }
        }
    }

    let exit = match control.wait(None).await {
        Ok(e) => e,
        Err(_) => ProcessExit {
            code: None,
            aborted: false,
            timed_out: false,
            stderr_tail: String::new(),
            kind: None,
        },
    };
    let _ = tx.send(CodexEvent::Exit { exit, failed }).await;
}

fn parse_stdout_line(line: &str, failed: &mut bool) -> CodexEvent {
    let raw: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => {
            return CodexEvent::MalformedJson {
                line: line.to_string(),
                error: e.to_string(),
            };
        }
    };

    let type_ = raw
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();

    match type_.as_str() {
        "thread.started" => {
            let thread_id = raw
                .get("thread_id")
                .or_else(|| raw.get("id"))
                .and_then(|s| s.as_str())
                .map(str::to_string);
            CodexEvent::ThreadStarted { thread_id, raw }
        }
        "turn.started" => CodexEvent::TurnStarted { raw },
        "item.started" => {
            let item_type = extract_item_type(&raw);
            CodexEvent::ItemStarted { item_type, raw }
        }
        "item.completed" => {
            let item_type = extract_item_type(&raw);
            // S8: deprecation/sandbox-deny warnings surface as
            // item.completed with an error payload but the turn continues.
            let error_message = raw
                .get("error")
                .and_then(|e| e.get("message").or_else(|| e.as_str().map(|_| e)))
                .and_then(|m| m.as_str())
                .map(str::to_string)
                .or_else(|| {
                    raw.get("item")
                        .and_then(|i| i.get("error"))
                        .and_then(|e| e.get("message"))
                        .and_then(|s| s.as_str())
                        .map(str::to_string)
                });
            CodexEvent::ItemCompleted {
                item_type,
                error_message,
                raw,
            }
        }
        "turn.completed" => CodexEvent::TurnCompleted { raw },
        "turn.failed" => {
            *failed = true;
            let error_message = raw
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|s| s.as_str())
                .map(str::to_string);
            CodexEvent::TurnFailed {
                error_message,
                raw,
            }
        }
        "error" => {
            *failed = true;
            let message = raw
                .get("message")
                .or_else(|| raw.get("error").and_then(|e| e.get("message")))
                .and_then(|s| s.as_str())
                .map(str::to_string);
            CodexEvent::Error { message, raw }
        }
        _ => CodexEvent::Other { type_, raw },
    }
}

fn extract_item_type(raw: &Value) -> Option<String> {
    raw.get("item")
        .and_then(|i| i.get("type"))
        .and_then(|t| t.as_str())
        .map(str::to_string)
        .or_else(|| {
            raw.get("item_type")
                .and_then(|t| t.as_str())
                .map(str::to_string)
        })
}

#[cfg(test)]
mod unit {
    use super::*;

    fn cfg() -> CodexConfig {
        let mut env = HashMap::new();
        env.insert("CODEX_HOME".into(), "C:/x/.moa-desktop/codex-home".into());
        CodexConfig {
            program: PathBuf::from("codex.exe"),
            reasoning_effort: "high".into(),
            web_search: "live".into(),
            approval_policy: "never".into(),
            guard_text: "GUARD".into(),
            firstpass_template: "T={{task}} F={{files}}".into(),
            mutation_template: "T={{task}} W={{worktree}}".into(),
            env,
        }
    }

    fn fake_runner() -> Arc<dyn ProcessRunner> {
        struct Never;
        #[async_trait::async_trait]
        impl ProcessRunner for Never {
            async fn spawn(&self, _: ProcessSpec) -> Result<ProcessHandle, ProcessError> {
                unreachable!("unit tests don't call spawn")
            }
        }
        Arc::new(Never)
    }

    #[test]
    fn firstpass_argv_has_required_flags_as_separate_elements() {
        let a = CodexAdapter::new(fake_runner(), cfg())
            .firstpass_argv(Path::new("C:/repo"), "PROMPT_BODY");
        assert_eq!(a[0], "codex.exe");
        assert_eq!(a[1], "exec");
        assert!(a.iter().any(|s| s == "--ephemeral"));
        assert!(a.iter().any(|s| s == "--json"));
        assert!(a.iter().any(|s| s == "--skip-git-repo-check"));

        // -c key="value" pairs each occupy their own slot, immediately after a "-c"
        for key in ["approval_policy", "model_reasoning_effort", "web_search"] {
            let needle = format!("{key}=");
            let pos = a
                .iter()
                .position(|s| s.starts_with(&needle))
                .unwrap_or_else(|| panic!("missing -c {key}"));
            assert_eq!(a[pos - 1], "-c", "{key} not preceded by -c");
        }

        // sandbox is read-only for first-pass
        let i = a.iter().position(|s| s == "--sandbox").expect("--sandbox");
        assert_eq!(a[i + 1], "read-only");

        // --cd <cwd>
        let i = a.iter().position(|s| s == "--cd").expect("--cd");
        assert_eq!(a[i + 1], "C:/repo");

        // prompt is the LAST element (positional)
        assert_eq!(a.last().unwrap(), "PROMPT_BODY");

        // mutation-only flag must not appear
        assert!(!a
            .iter()
            .any(|s| s == "--dangerously-bypass-approvals-and-sandbox"));
    }

    #[test]
    fn mutation_argv_swaps_to_dangerous_bypass_and_drops_sandbox_flag() {
        let a = CodexAdapter::new(fake_runner(), cfg())
            .mutation_argv(Path::new("C:/wt"), "PROMPT");
        assert!(a
            .iter()
            .any(|s| s == "--dangerously-bypass-approvals-and-sandbox"));
        // S2 finding #5: workspace-write is broken on Windows; we deliberately
        // do NOT carry --sandbox at all in mutation mode.
        assert!(
            !a.iter().any(|s| s == "--sandbox"),
            "mutation argv must not carry --sandbox"
        );
        let i = a.iter().position(|s| s == "--cd").unwrap();
        assert_eq!(a[i + 1], "C:/wt");
        assert_eq!(a.last().unwrap(), "PROMPT");
    }

    #[test]
    fn render_firstpass_prompt_prefixes_guard_then_substitutes() {
        let a = CodexAdapter::new(fake_runner(), cfg());
        let p = a.render_firstpass_prompt(&FirstPassRequest {
            task: "fix bug".into(),
            files: vec!["src/x.rs:1-10".into(), "src/y.rs".into()],
            cwd: PathBuf::from("."),
        });
        // guard text comes first (no flag delivery on Codex)
        assert!(p.starts_with("GUARD"));
        assert!(p.contains("T=fix bug"));
        assert!(p.contains("- src/x.rs:1-10"));
        assert!(p.contains("- src/y.rs"));
    }

    #[test]
    fn render_mutation_prompt_prefixes_guard_then_substitutes() {
        let a = CodexAdapter::new(fake_runner(), cfg());
        let p = a.render_mutation_prompt(&MutationRequest {
            task: "refactor".into(),
            worktree_path: PathBuf::from("/tmp/wt"),
        });
        assert!(p.starts_with("GUARD"));
        assert!(p.contains("T=refactor"));
        assert!(p.contains("W=/tmp/wt") || p.contains("W=\\tmp\\wt"));
    }

    #[test]
    fn parse_thread_started_extracts_id() {
        let mut f = false;
        let line = r#"{"type":"thread.started","thread_id":"thr-7"}"#;
        match parse_stdout_line(line, &mut f) {
            CodexEvent::ThreadStarted { thread_id, .. } => {
                assert_eq!(thread_id.as_deref(), Some("thr-7"));
            }
            other => panic!("expected ThreadStarted, got {other:?}"),
        }
        assert!(!f);
    }

    #[test]
    fn parse_turn_failed_sets_failed_flag_and_extracts_message() {
        let mut f = false;
        let line = r#"{"type":"turn.failed","error":{"message":"sandbox blocked"}}"#;
        match parse_stdout_line(line, &mut f) {
            CodexEvent::TurnFailed { error_message, .. } => {
                assert_eq!(error_message.as_deref(), Some("sandbox blocked"));
            }
            other => panic!("expected TurnFailed, got {other:?}"),
        }
        assert!(f, "turn.failed must set running failed flag");
    }

    #[test]
    fn parse_item_completed_with_error_payload() {
        let mut f = false;
        let line =
            r#"{"type":"item.completed","item":{"type":"error"},"error":{"message":"deprecated"}}"#;
        match parse_stdout_line(line, &mut f) {
            CodexEvent::ItemCompleted {
                item_type,
                error_message,
                ..
            } => {
                assert_eq!(item_type.as_deref(), Some("error"));
                assert_eq!(error_message.as_deref(), Some("deprecated"));
            }
            other => panic!("expected ItemCompleted, got {other:?}"),
        }
        // non-blocking warning — must NOT mark the run failed
        assert!(!f);
    }

    #[test]
    fn parse_turn_completed_is_terminal_success() {
        let mut f = false;
        let line = r#"{"type":"turn.completed"}"#;
        match parse_stdout_line(line, &mut f) {
            CodexEvent::TurnCompleted { .. } => {}
            other => panic!("expected TurnCompleted, got {other:?}"),
        }
        assert!(!f);
    }

    #[test]
    fn parse_error_event_sets_failed_flag() {
        let mut f = false;
        let line = r#"{"type":"error","message":"fatal"}"#;
        match parse_stdout_line(line, &mut f) {
            CodexEvent::Error { message, .. } => assert_eq!(message.as_deref(), Some("fatal")),
            other => panic!("expected Error, got {other:?}"),
        }
        assert!(f);
    }

    #[test]
    fn parse_unknown_type_falls_through_to_other() {
        let mut f = false;
        let line = r#"{"type":"future.event","payload":42}"#;
        match parse_stdout_line(line, &mut f) {
            CodexEvent::Other { type_, .. } => assert_eq!(type_, "future.event"),
            other => panic!("expected Other, got {other:?}"),
        }
    }

    #[test]
    fn parse_malformed_json_surfaces() {
        let mut f = false;
        match parse_stdout_line("not json {", &mut f) {
            CodexEvent::MalformedJson { line, .. } => assert_eq!(line, "not json {"),
            other => panic!("expected MalformedJson, got {other:?}"),
        }
    }
}
