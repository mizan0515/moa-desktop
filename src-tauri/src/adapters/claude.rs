//! T5a — Claude CLI adapter.
//!
//! Builds argv for `claude -p ...` per `spikes/S8-final-templates.md` and
//! parses the line-delimited JSON ("stream-json") output into
//! [`ClaudeEvent`]s.
//!
//! Two modes:
//! * [`firstpass`](ClaudeAdapter::firstpass) — read-only. `Edit`/`Write`/
//!   `NotebookEdit` and all `mcp__*` tools denied; cwd is the session
//!   worktree but no mutations expected.
//! * [`mutation`](ClaudeAdapter::mutation) — owner mode. Runs in an
//!   isolated worktree under `~/.moa-desktop/worktrees/<sid>/`; allows
//!   Edit/Write plus a curated Bash test allowlist.
//!
//! The adapter is decoupled from the actual process implementation: it
//! takes any [`ProcessRunner`], so tests substitute a [`MockRunner`].

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
pub struct ClaudeConfig {
    /// Resolved `claude.exe` path (S1: never `claude.cmd`).
    pub program: PathBuf,
    /// `--model` value.
    pub model: String,
    pub max_turns_firstpass: u32,
    pub max_turns_mutation: u32,
    /// `--append-system-prompt` payload (Worker guard, DESIGN.md § Worker Guard).
    pub guard_text: String,
    /// First-pass prompt template; `{{task}}` and `{{files}}` substituted at
    /// spawn time.
    pub firstpass_template: String,
    /// Mutation prompt template; `{{task}}` and `{{worktree}}` substituted.
    pub mutation_template: String,
    /// Worker-specific env (S8): `ENABLE_CLAUDEAI_MCP_SERVERS=false`,
    /// `CLAUDE_CODE_SUBAGENT_MODEL=haiku`, `MAX_THINKING_TOKENS=10000`.
    pub env: HashMap<String, String>,
}

impl ClaudeConfig {
    /// Load templates + guard from `prompts/workers/`. `prompts_dir` is the
    /// directory that contains `claude_guard.txt`,
    /// `claude_firstpass_template.txt`, `claude_mutation_template.txt`.
    pub fn from_dir(
        program: PathBuf,
        prompts_dir: &Path,
        model: impl Into<String>,
    ) -> std::io::Result<Self> {
        let read = |name: &str| std::fs::read_to_string(prompts_dir.join(name));
        let guard_text = read("claude_guard.txt")?;
        let firstpass_template = read("claude_firstpass_template.txt")?;
        let mutation_template = read("claude_mutation_template.txt")?;
        let mut env = HashMap::new();
        env.insert("ENABLE_CLAUDEAI_MCP_SERVERS".into(), "false".into());
        env.insert("CLAUDE_CODE_SUBAGENT_MODEL".into(), "haiku".into());
        env.insert("MAX_THINKING_TOKENS".into(), "10000".into());
        Ok(Self {
            program,
            model: model.into(),
            max_turns_firstpass: 20,
            max_turns_mutation: 30,
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
/// Mirrors the Claude `stream-json` line shapes documented in S1 / S8 plus
/// runner-injected `Exit` / `Stderr` / `MalformedJson`. A `result` line
/// with `is_error=false` AND `num_turns=0` is interpreted as a hook block
/// and surfaces `Result { hook_blocked: true, .. }` (S8 critical signal).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ClaudeEvent {
    /// `type=system, subtype=init` — first event Claude emits.
    SystemInit { session_id: Option<String>, raw: Value },
    /// Hook event (only seen with `--include-hook-events`). `blocked=true`
    /// when `exit_code=2` on `UserPromptSubmit` (S8 HARD-block signal).
    HookEvent {
        hook_event: Option<String>,
        exit_code: Option<i64>,
        blocked: bool,
        raw: Value,
    },
    /// `type=assistant` — model output turn.
    Assistant { text: String, raw: Value },
    /// `type=user` — tool_result echo (model received tool output).
    User { raw: Value },
    /// `type=result` — terminal turn. `num_turns=0` after a hook block.
    Result {
        is_error: bool,
        num_turns: Option<u64>,
        hook_blocked: bool,
        raw: Value,
    },
    /// Any other `type=*` line (forward-compat — schema is sparsely
    /// documented; we don't drop unknowns).
    Other { type_: String, raw: Value },
    /// stderr passthrough (typically auth / spawn diagnostics).
    Stderr { line: String },
    /// A line that did not parse as JSON. Rare — surfaced so the
    /// orchestrator can classify as `malformed-json` per F6.
    MalformedJson { line: String, error: String },
    /// Process terminated. `hook_blocked` mirrors the most recent
    /// `Result.hook_blocked` so consumers don't have to reconstruct it.
    Exit { exit: ProcessExit, hook_blocked: bool },
}

/// Stream returned to consumers — control + parsed events.
pub struct ClaudeStream {
    pub control: ProcessControl,
    pub events: mpsc::Receiver<ClaudeEvent>,
}

/// Adapter façade — holds the runner and the static config.
pub struct ClaudeAdapter {
    runner: Arc<dyn ProcessRunner>,
    config: ClaudeConfig,
}

impl ClaudeAdapter {
    pub fn new(runner: Arc<dyn ProcessRunner>, config: ClaudeConfig) -> Self {
        Self { runner, config }
    }

    pub fn config(&self) -> &ClaudeConfig {
        &self.config
    }

    /// Build the argv for a first-pass run. Pure — exposed for tests.
    pub fn firstpass_argv(&self) -> Vec<String> {
        let max_turns = self.config.max_turns_firstpass.to_string();
        let mut a: Vec<String> = vec![
            self.config.program.to_string_lossy().into_owned(),
            "-p".into(),
            "--output-format".into(),
            "stream-json".into(),
            "--verbose".into(),
            "--include-hook-events".into(),
            "--max-turns".into(),
            max_turns,
            "--model".into(),
            self.config.model.clone(),
            "--strict-mcp-config".into(),
            "--mcp-config".into(),
            r#"{"mcpServers":{}}"#.into(),
            "--disable-slash-commands".into(),
            // S4: deny mcp__* + Edit/Write/NotebookEdit. Variadic — every
            // tool is its own argv element.
            "--disallowedTools".into(),
            "mcp__*".into(),
            "Edit".into(),
            "Write".into(),
            "NotebookEdit".into(),
            "--allowedTools".into(),
            "Read".into(),
            "Glob".into(),
            "Grep".into(),
            "WebSearch".into(),
            "WebFetch".into(),
            "Bash(git status:*)".into(),
            "Bash(git log:*)".into(),
            "Bash(git diff:*)".into(),
            "Bash(rg:*)".into(),
            "--append-system-prompt".into(),
            self.config.guard_text.clone(),
            "--setting-sources".into(),
            String::new(),
        ];
        // Defensive: ensure no element accidentally collapsed into another.
        debug_assert!(a.iter().all(|s| !s.contains('\0')));
        // Move ownership through.
        a.shrink_to_fit();
        a
    }

    /// Build the argv for a mutation run. Pure — exposed for tests.
    pub fn mutation_argv(&self) -> Vec<String> {
        let max_turns = self.config.max_turns_mutation.to_string();
        vec![
            self.config.program.to_string_lossy().into_owned(),
            "-p".into(),
            "--output-format".into(),
            "stream-json".into(),
            "--verbose".into(),
            "--include-hook-events".into(),
            "--max-turns".into(),
            max_turns,
            "--model".into(),
            self.config.model.clone(),
            "--strict-mcp-config".into(),
            "--mcp-config".into(),
            r#"{"mcpServers":{}}"#.into(),
            "--disable-slash-commands".into(),
            "--permission-mode".into(),
            "acceptEdits".into(),
            "--allowedTools".into(),
            "Read".into(),
            "Edit".into(),
            "Write".into(),
            "Glob".into(),
            "Grep".into(),
            "WebSearch".into(),
            "WebFetch".into(),
            "Bash(git status:*)".into(),
            "Bash(git diff:*)".into(),
            "Bash(npm test:*)".into(),
            "Bash(pytest:*)".into(),
            "Bash(cargo test:*)".into(),
            "Bash(rg:*)".into(),
            "--append-system-prompt".into(),
            self.config.guard_text.clone(),
            "--setting-sources".into(),
            String::new(),
        ]
    }

    /// Render the first-pass prompt body (substituted into the template,
    /// written to stdin per S1 finding #2 — never argv).
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
        self.config
            .firstpass_template
            .replace("{{task}}", &req.task)
            .replace("{{files}}", &files)
    }

    pub fn render_mutation_prompt(&self, req: &MutationRequest) -> String {
        self.config
            .mutation_template
            .replace("{{task}}", &req.task)
            .replace("{{worktree}}", &req.worktree_path.to_string_lossy())
    }

    /// Spawn a read-only first-pass run. Returns immediately; events stream
    /// asynchronously through the returned [`ClaudeStream`].
    pub async fn firstpass(&self, req: FirstPassRequest) -> Result<ClaudeStream, ProcessError> {
        let argv = self.firstpass_argv();
        let prompt = self.render_firstpass_prompt(&req);
        self.spawn_with_prompt(argv, req.cwd, prompt).await
    }

    /// Spawn a mutation-owner run inside `req.worktree_path`.
    pub async fn mutation(&self, req: MutationRequest) -> Result<ClaudeStream, ProcessError> {
        let argv = self.mutation_argv();
        let prompt = self.render_mutation_prompt(&req);
        let cwd = req.worktree_path.clone();
        self.spawn_with_prompt(argv, cwd, prompt).await
    }

    async fn spawn_with_prompt(
        &self,
        argv: Vec<String>,
        cwd: PathBuf,
        prompt: String,
    ) -> Result<ClaudeStream, ProcessError> {
        let spec = ProcessSpec::new(argv, cwd)
            .with_env(self.config.env.clone())
            .with_stdin(StdinPolicy::Pipe);

        let ProcessHandle { control, lines } = self.runner.spawn(spec).await?;

        // Write prompt to stdin then close (S1 finding #2: prompt is stdin,
        // not argv — the variadic --allowedTools eats positional args).
        control.write_stdin(prompt.into_bytes()).await?;
        control.close_stdin().await?;

        // Spawn parser task.
        let (tx, rx) = mpsc::channel::<ClaudeEvent>(256);
        let exit_watch = control.clone();
        tokio::spawn(parser_task(lines, tx, exit_watch));

        Ok(ClaudeStream { control, events: rx })
    }
}

async fn parser_task(
    mut lines: mpsc::Receiver<ProcessLine>,
    tx: mpsc::Sender<ClaudeEvent>,
    control: ProcessControl,
) {
    let mut last_hook_blocked = false;

    while let Some(pl) = lines.recv().await {
        match pl.stream {
            PStream::Stderr => {
                if tx.send(ClaudeEvent::Stderr { line: pl.line }).await.is_err() {
                    return;
                }
            }
            PStream::Stdout => {
                let event = parse_stdout_line(&pl.line, &mut last_hook_blocked);
                if tx.send(event).await.is_err() {
                    return;
                }
            }
        }
    }

    // Lines channel closed — supervisor has reached EOF / abort. Wait for
    // the published exit (already there or about to be).
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
    let _ = tx
        .send(ClaudeEvent::Exit {
            exit,
            hook_blocked: last_hook_blocked,
        })
        .await;
}

fn parse_stdout_line(line: &str, last_hook_blocked: &mut bool) -> ClaudeEvent {
    let raw: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => {
            return ClaudeEvent::MalformedJson {
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
        "system" => {
            let subtype = raw.get("subtype").and_then(|s| s.as_str()).unwrap_or("");
            // Hook events come in as type=system + (sub)event identifiers when
            // --include-hook-events is set. S8 critical signal: hook_response
            // with exit_code=2 on UserPromptSubmit.
            if subtype == "hook_started" || subtype == "hook_response" {
                let exit_code = raw.get("exit_code").and_then(|c| c.as_i64()).or_else(|| {
                    raw.get("response")
                        .and_then(|r| r.get("exit_code"))
                        .and_then(|c| c.as_i64())
                });
                let hook_event = raw
                    .get("hook_event")
                    .and_then(|e| e.as_str())
                    .map(str::to_string);
                let blocked = exit_code == Some(2)
                    && hook_event.as_deref() == Some("UserPromptSubmit");
                if blocked {
                    *last_hook_blocked = true;
                }
                ClaudeEvent::HookEvent {
                    hook_event,
                    exit_code,
                    blocked,
                    raw,
                }
            } else if subtype == "init" {
                let session_id = raw
                    .get("session_id")
                    .and_then(|s| s.as_str())
                    .map(str::to_string);
                ClaudeEvent::SystemInit { session_id, raw }
            } else {
                ClaudeEvent::Other { type_, raw }
            }
        }
        "assistant" => {
            let text = extract_assistant_text(&raw);
            ClaudeEvent::Assistant { text, raw }
        }
        "user" => ClaudeEvent::User { raw },
        "result" => {
            let is_error = raw
                .get("is_error")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let num_turns = raw.get("num_turns").and_then(|v| v.as_u64());
            // S8: hook block manifests as is_error=false + num_turns=0
            // following a hook_response with exit_code=2. Cross-correlate.
            let hook_blocked = *last_hook_blocked || (!is_error && num_turns == Some(0));
            if hook_blocked {
                *last_hook_blocked = true;
            }
            ClaudeEvent::Result {
                is_error,
                num_turns,
                hook_blocked,
                raw,
            }
        }
        "" => ClaudeEvent::Other {
            type_: String::new(),
            raw,
        },
        _ => ClaudeEvent::Other { type_, raw },
    }
}

fn extract_assistant_text(raw: &Value) -> String {
    // Stream-json shape (observed): { "type":"assistant",
    //   "message": { "content": [{ "type":"text", "text":"..." }, ...] } }
    let mut out = String::new();
    if let Some(content) = raw
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array())
    {
        for block in content {
            if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                    if !out.is_empty() {
                        out.push('\n');
                    }
                    out.push_str(t);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod unit {
    use super::*;

    fn cfg() -> ClaudeConfig {
        ClaudeConfig {
            program: PathBuf::from("claude.exe"),
            model: "opus".into(),
            max_turns_firstpass: 20,
            max_turns_mutation: 30,
            guard_text: "GUARD".into(),
            firstpass_template: "T={{task}} F={{files}}".into(),
            mutation_template: "T={{task}} W={{worktree}}".into(),
            env: HashMap::new(),
        }
    }

    fn fake_runner() -> Arc<dyn ProcessRunner> {
        // Unit tests don't spawn — use a panicking runner placeholder.
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
        let a = ClaudeAdapter::new(fake_runner(), cfg()).firstpass_argv();
        assert_eq!(a[0], "claude.exe");
        assert!(a.iter().any(|s| s == "-p"));
        // every flag literal occupies its own slot
        let pairs = [
            ("--output-format", "stream-json"),
            ("--max-turns", "20"),
            ("--model", "opus"),
            ("--mcp-config", r#"{"mcpServers":{}}"#),
        ];
        for (flag, val) in pairs {
            let i = a.iter().position(|s| s == flag).expect(flag);
            assert_eq!(a[i + 1], val, "{flag}");
        }
        // disallowedTools must list mcp__* Edit Write NotebookEdit as
        // distinct contiguous elements (S4 + S8).
        let i = a.iter().position(|s| s == "--disallowedTools").unwrap();
        assert_eq!(a[i + 1], "mcp__*");
        assert_eq!(a[i + 2], "Edit");
        assert_eq!(a[i + 3], "Write");
        assert_eq!(a[i + 4], "NotebookEdit");
        // setting-sources empty string is preserved
        let j = a.iter().position(|s| s == "--setting-sources").unwrap();
        assert_eq!(a[j + 1], "");
        // guard text ends up as one element after --append-system-prompt
        let k = a.iter().position(|s| s == "--append-system-prompt").unwrap();
        assert_eq!(a[k + 1], "GUARD");
    }

    #[test]
    fn mutation_argv_swaps_to_acceptedits_and_allows_edit_write() {
        let a = ClaudeAdapter::new(fake_runner(), cfg()).mutation_argv();
        assert!(a.iter().any(|s| s == "--permission-mode"));
        let i = a.iter().position(|s| s == "--permission-mode").unwrap();
        assert_eq!(a[i + 1], "acceptEdits");
        // Edit + Write must appear in the allowedTools span after the flag
        let i = a.iter().position(|s| s == "--allowedTools").unwrap();
        let span: &[String] = &a[i + 1..];
        assert!(span.contains(&"Edit".to_string()));
        assert!(span.contains(&"Write".to_string()));
        // mutation must NOT carry --disallowedTools (S8: replace, not append)
        assert!(
            !a.iter().any(|s| s == "--disallowedTools"),
            "mutation argv leaked read-only denylist"
        );
        // max-turns moves to 30
        let j = a.iter().position(|s| s == "--max-turns").unwrap();
        assert_eq!(a[j + 1], "30");
    }

    #[test]
    fn render_firstpass_prompt_substitutes_tokens() {
        let a = ClaudeAdapter::new(fake_runner(), cfg());
        let p = a.render_firstpass_prompt(&FirstPassRequest {
            task: "fix bug".into(),
            files: vec!["src/x.rs:1-10".into(), "src/y.rs".into()],
            cwd: PathBuf::from("."),
        });
        assert!(p.contains("T=fix bug"));
        assert!(p.contains("- src/x.rs:1-10"));
        assert!(p.contains("- src/y.rs"));
    }

    #[test]
    fn render_mutation_prompt_substitutes_tokens() {
        let a = ClaudeAdapter::new(fake_runner(), cfg());
        let p = a.render_mutation_prompt(&MutationRequest {
            task: "refactor".into(),
            worktree_path: PathBuf::from("/tmp/wt"),
        });
        assert!(p.contains("T=refactor"));
        assert!(p.contains("W=/tmp/wt") || p.contains("W=\\tmp\\wt"));
    }

    #[test]
    fn parse_system_init() {
        let mut hb = false;
        let line = r#"{"type":"system","subtype":"init","session_id":"abc"}"#;
        match parse_stdout_line(line, &mut hb) {
            ClaudeEvent::SystemInit { session_id, .. } => {
                assert_eq!(session_id.as_deref(), Some("abc"));
            }
            other => panic!("expected SystemInit, got {other:?}"),
        }
        assert!(!hb);
    }

    #[test]
    fn parse_hook_block_sets_flag() {
        let mut hb = false;
        let line = r#"{"type":"system","subtype":"hook_response","hook_event":"UserPromptSubmit","exit_code":2}"#;
        let ev = parse_stdout_line(line, &mut hb);
        match ev {
            ClaudeEvent::HookEvent { blocked, exit_code, hook_event, .. } => {
                assert!(blocked);
                assert_eq!(exit_code, Some(2));
                assert_eq!(hook_event.as_deref(), Some("UserPromptSubmit"));
            }
            other => panic!("expected HookEvent, got {other:?}"),
        }
        assert!(hb, "hook block must set running flag");
    }

    #[test]
    fn parse_result_inherits_hook_block_when_num_turns_zero() {
        let mut hb = false;
        let line = r#"{"type":"result","is_error":false,"num_turns":0}"#;
        match parse_stdout_line(line, &mut hb) {
            ClaudeEvent::Result {
                is_error,
                num_turns,
                hook_blocked,
                ..
            } => {
                assert!(!is_error);
                assert_eq!(num_turns, Some(0));
                assert!(hook_blocked, "S8 critical signal");
            }
            other => panic!("expected Result, got {other:?}"),
        }
    }

    #[test]
    fn parse_assistant_extracts_text_blocks() {
        let mut hb = false;
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hi"},{"type":"tool_use","id":"x"},{"type":"text","text":"there"}]}}"#;
        match parse_stdout_line(line, &mut hb) {
            ClaudeEvent::Assistant { text, .. } => assert_eq!(text, "hi\nthere"),
            other => panic!("expected Assistant, got {other:?}"),
        }
    }

    #[test]
    fn parse_malformed_json_surfaces() {
        let mut hb = false;
        match parse_stdout_line("not json {", &mut hb) {
            ClaudeEvent::MalformedJson { line, .. } => assert_eq!(line, "not json {"),
            other => panic!("expected MalformedJson, got {other:?}"),
        }
    }
}
