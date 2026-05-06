//! Integration tests for T5b — `CodexAdapter`.
//!
//! Uses an in-test `ScriptRunner` (analogue of T5a's) to stream canned
//! `codex exec --json` lines and assert argv shape, prompt routing
//! (positional, NOT stdin), and event sequence.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use parking_lot::Mutex as PlMutex;
use tokio::sync::{mpsc, watch, Mutex};

use moa_desktop_lib::adapters::codex::{
    CodexAdapter, CodexConfig, CodexEvent, FirstPassRequest, MutationRequest,
};
use moa_desktop_lib::process::traits::{ProcessControlInner, StdinCommand};
use moa_desktop_lib::process::{
    ProcessControl, ProcessError, ProcessExit, ProcessHandle, ProcessLine, ProcessRunner,
    ProcessSpec, StdinPolicy, Stream as PStream,
};

// ---- ScriptRunner ----------------------------------------------------------

#[derive(Debug, Clone, Default)]
struct Captured {
    spec: Option<ProcessSpec>,
    stdin_chunks: Vec<Vec<u8>>,
    stdin_closed: bool,
}

#[derive(Clone)]
struct ScriptRunner {
    script: Vec<String>,
    delay: Duration,
    captured: Arc<PlMutex<Captured>>,
}

impl ScriptRunner {
    fn new(script: Vec<&str>) -> Self {
        Self {
            script: script.into_iter().map(String::from).collect(),
            delay: Duration::from_millis(5),
            captured: Arc::new(PlMutex::new(Captured::default())),
        }
    }

    fn captured(&self) -> Captured {
        self.captured.lock().clone()
    }
}

#[async_trait]
impl ProcessRunner for ScriptRunner {
    async fn spawn(&self, spec: ProcessSpec) -> Result<ProcessHandle, ProcessError> {
        if spec.argv.is_empty() {
            return Err(ProcessError::empty_argv());
        }

        let line_buf = spec.line_buf.max(1);
        let (line_tx, line_rx) = mpsc::channel::<ProcessLine>(line_buf);
        let (abort_tx, mut abort_rx) = mpsc::channel::<()>(1);
        let (exit_tx, exit_rx) = watch::channel::<Option<ProcessExit>>(None);
        let (stdin_tx, mut stdin_rx) = mpsc::channel::<StdinCommand>(8);

        self.captured.lock().spec = Some(spec.clone());

        // Mirror real runner contract: with CloseImmediately, the runner —
        // not the adapter — closes the stdin pipe right after spawn.
        let stdin_handle = if spec.stdin == StdinPolicy::CloseImmediately {
            let _ = stdin_tx.send(StdinCommand::Close).await;
            None
        } else {
            Some(stdin_tx)
        };

        let inner = Arc::new(ProcessControlInner {
            pid: 1,
            aborted: AtomicBool::new(false),
            abort_tx,
            timed_out_pending: Arc::new(AtomicBool::new(false)),
            stdin_tx: Mutex::new(stdin_handle),
            exit_watch: exit_rx,
        });
        let control = ProcessControl { inner };

        // Stdin pump — for Codex we expect a single Close (CloseImmediately).
        let cap = self.captured.clone();
        tokio::spawn(async move {
            while let Some(cmd) = stdin_rx.recv().await {
                match cmd {
                    StdinCommand::Write(bytes) => cap.lock().stdin_chunks.push(bytes),
                    StdinCommand::Close => {
                        cap.lock().stdin_closed = true;
                        break;
                    }
                }
            }
        });

        let script = self.script.clone();
        let delay = self.delay;
        tokio::spawn(async move {
            let mut seq: u64 = 0;
            let mut aborted = false;
            for line in script {
                tokio::select! {
                    _ = tokio::time::sleep(delay) => {}
                    _ = abort_rx.recv() => { aborted = true; break; }
                }
                let pl = ProcessLine {
                    seq,
                    stream: PStream::Stdout,
                    line,
                    partial: false,
                };
                seq += 1;
                if line_tx.send(pl).await.is_err() {
                    aborted = true;
                    break;
                }
            }
            drop(line_tx);
            let _ = exit_tx.send(Some(ProcessExit {
                code: if aborted { None } else { Some(0) },
                aborted,
                timed_out: false,
                stderr_tail: String::new(),
                kind: None,
            }));
        });

        Ok(ProcessHandle {
            control,
            lines: line_rx,
        })
    }
}

// ---- helpers --------------------------------------------------------------

fn cfg() -> CodexConfig {
    let mut env = std::collections::HashMap::new();
    env.insert("CODEX_HOME".into(), "C:/x/.moa-desktop/codex-home".into());
    CodexConfig {
        program: PathBuf::from("codex.exe"),
        reasoning_effort: "high".into(),
        web_search: "live".into(),
        approval_policy: "never".into(),
        guard_text: "GUARD-TEXT".into(),
        firstpass_template: "FP task={{task}} files=\n{{files}}".into(),
        mutation_template: "MUT task={{task}} wt={{worktree}}".into(),
        env,
    }
}

async fn drain(rx: &mut mpsc::Receiver<CodexEvent>) -> Vec<CodexEvent> {
    let mut out = Vec::new();
    while let Some(e) = rx.recv().await {
        out.push(e);
    }
    out
}

// ---- tests ----------------------------------------------------------------

#[tokio::test]
async fn firstpass_emits_expected_event_sequence() {
    let script = vec![
        r#"{"type":"thread.started","thread_id":"thr-A"}"#,
        r#"{"type":"turn.started"}"#,
        r#"{"type":"item.started","item":{"type":"agent_message"}}"#,
        r#"{"type":"item.completed","item":{"type":"agent_message"}}"#,
        r#"{"type":"turn.completed"}"#,
    ];
    let runner = Arc::new(ScriptRunner::new(script));
    let adapter = CodexAdapter::new(runner.clone(), cfg());

    let mut stream = adapter
        .firstpass(FirstPassRequest {
            task: "diagnose flaky test".into(),
            files: vec!["src/foo.rs:1-10".into()],
            cwd: PathBuf::from("C:/repo"),
        })
        .await
        .expect("firstpass spawn");

    let events = drain(&mut stream.events).await;

    // Sequence: ThreadStarted, TurnStarted, ItemStarted, ItemCompleted,
    // TurnCompleted, Exit.
    assert_eq!(events.len(), 6, "got {events:#?}");
    assert!(matches!(events[0], CodexEvent::ThreadStarted { .. }));
    assert!(matches!(events[1], CodexEvent::TurnStarted { .. }));
    assert!(matches!(events[2], CodexEvent::ItemStarted { .. }));
    assert!(matches!(events[3], CodexEvent::ItemCompleted { .. }));
    assert!(matches!(events[4], CodexEvent::TurnCompleted { .. }));
    match &events[5] {
        CodexEvent::Exit { exit, failed } => {
            assert!(exit.is_clean());
            assert!(!failed, "no turn.failed seen");
        }
        other => panic!("last not Exit: {other:?}"),
    }

    // Argv shape: program, then exec, with --cd <cwd> and prompt as last
    // positional. Guard text MUST appear in the prompt body (Codex has no
    // system-prompt flag).
    let cap = runner.captured();
    let spec = cap.spec.expect("spawn captured");
    assert_eq!(spec.argv[0], "codex.exe");
    assert_eq!(spec.argv[1], "exec");

    let i = spec
        .argv
        .iter()
        .position(|s| s == "--cd")
        .expect("--cd present");
    assert_eq!(spec.argv[i + 1], "C:/repo");

    let prompt = spec.argv.last().expect("prompt last").clone();
    assert!(prompt.starts_with("GUARD-TEXT"), "guard prefix missing");
    assert!(prompt.contains("FP task=diagnose flaky test"));
    assert!(prompt.contains("- src/foo.rs:1-10"));

    // sandbox: read-only present, dangerous bypass absent
    let i = spec.argv.iter().position(|s| s == "--sandbox").unwrap();
    assert_eq!(spec.argv[i + 1], "read-only");
    assert!(!spec
        .argv
        .iter()
        .any(|s| s == "--dangerously-bypass-approvals-and-sandbox"));

    // Stdin policy: closed immediately (Codex requirement, S2 finding #4).
    assert_eq!(spec.stdin, StdinPolicy::CloseImmediately);
    assert!(cap.stdin_closed, "stdin must be closed for Codex");
    assert!(
        cap.stdin_chunks.is_empty(),
        "Codex prompt must NOT be written to stdin (it's argv-positional)"
    );
}

#[tokio::test]
async fn turn_failed_propagates_to_exit_failed_flag() {
    let script = vec![
        r#"{"type":"thread.started","thread_id":"thr-B"}"#,
        r#"{"type":"turn.started"}"#,
        r#"{"type":"turn.failed","error":{"message":"sandbox blocked write"}}"#,
    ];
    let runner = Arc::new(ScriptRunner::new(script));
    let adapter = CodexAdapter::new(runner, cfg());

    let mut stream = adapter
        .firstpass(FirstPassRequest {
            task: "x".into(),
            files: vec![],
            cwd: PathBuf::from("C:/repo"),
        })
        .await
        .expect("spawn");

    let events = drain(&mut stream.events).await;
    let failed_ev = events
        .iter()
        .find(|e| matches!(e, CodexEvent::TurnFailed { .. }))
        .expect("TurnFailed emitted");
    match failed_ev {
        CodexEvent::TurnFailed { error_message, .. } => {
            assert_eq!(error_message.as_deref(), Some("sandbox blocked write"))
        }
        _ => unreachable!(),
    }
    let exit_ev = events.last().unwrap();
    match exit_ev {
        CodexEvent::Exit { failed, .. } => assert!(*failed, "exit must carry failed=true"),
        other => panic!("last not Exit: {other:?}"),
    }
}

#[tokio::test]
async fn mutation_uses_worktree_cwd_and_dangerous_bypass_argv() {
    let script = vec![r#"{"type":"turn.completed"}"#];
    let runner = Arc::new(ScriptRunner::new(script));
    let adapter = CodexAdapter::new(runner.clone(), cfg());

    let wt_path = PathBuf::from("C:/temp/moa-desktop-wt-test");

    let mut stream = adapter
        .mutation(MutationRequest {
            task: "apply patch".into(),
            worktree_path: wt_path.clone(),
        })
        .await
        .expect("spawn");

    let _ = drain(&mut stream.events).await;

    let cap = runner.captured();
    let spec = cap.spec.expect("spec captured");
    assert_eq!(spec.cwd, wt_path);

    assert!(spec
        .argv
        .iter()
        .any(|s| s == "--dangerously-bypass-approvals-and-sandbox"));
    assert!(
        !spec.argv.iter().any(|s| s == "--sandbox"),
        "mutation must not carry --sandbox (workspace-write broken on Windows)"
    );

    let i = spec.argv.iter().position(|s| s == "--cd").unwrap();
    assert_eq!(spec.argv[i + 1], wt_path.to_string_lossy());

    let prompt = spec.argv.last().unwrap().clone();
    assert!(prompt.starts_with("GUARD-TEXT"));
    assert!(prompt.contains("MUT task=apply patch"));
    assert!(prompt.contains(&wt_path.to_string_lossy().into_owned()));
}

#[tokio::test]
async fn malformed_json_line_surfaces_event_then_continues() {
    let script = vec![
        r#"{"type":"thread.started","thread_id":"thr-C"}"#,
        r#"NOT JSON"#,
        r#"{"type":"turn.completed"}"#,
    ];
    let runner = Arc::new(ScriptRunner::new(script));
    let adapter = CodexAdapter::new(runner, cfg());

    let mut stream = adapter
        .firstpass(FirstPassRequest {
            task: "t".into(),
            files: vec![],
            cwd: PathBuf::from("C:/repo"),
        })
        .await
        .expect("spawn");

    let events = drain(&mut stream.events).await;
    let mj = events
        .iter()
        .find(|e| matches!(e, CodexEvent::MalformedJson { .. }))
        .expect("MalformedJson emitted");
    if let CodexEvent::MalformedJson { line, .. } = mj {
        assert_eq!(line, "NOT JSON");
    }
    assert!(events
        .iter()
        .any(|e| matches!(e, CodexEvent::TurnCompleted { .. })));
    assert!(matches!(events.last(), Some(CodexEvent::Exit { .. })));
}

#[tokio::test]
async fn item_completed_with_warning_payload_does_not_mark_failed() {
    // S8: deprecation warnings emit as item.completed with an error
    // payload — the run continues normally.
    let script = vec![
        r#"{"type":"thread.started","thread_id":"thr-D"}"#,
        r#"{"type":"item.completed","item":{"type":"error"},"error":{"message":"[features].web_search deprecated"}}"#,
        r#"{"type":"turn.completed"}"#,
    ];
    let runner = Arc::new(ScriptRunner::new(script));
    let adapter = CodexAdapter::new(runner, cfg());

    let mut stream = adapter
        .firstpass(FirstPassRequest {
            task: "t".into(),
            files: vec![],
            cwd: PathBuf::from("C:/repo"),
        })
        .await
        .expect("spawn");

    let events = drain(&mut stream.events).await;
    let warn = events
        .iter()
        .find(|e| matches!(e, CodexEvent::ItemCompleted { .. }))
        .expect("ItemCompleted emitted");
    match warn {
        CodexEvent::ItemCompleted {
            error_message,
            item_type,
            ..
        } => {
            assert_eq!(item_type.as_deref(), Some("error"));
            assert!(error_message.as_deref().unwrap_or("").contains("deprecated"));
        }
        _ => unreachable!(),
    }
    let exit_ev = events.last().unwrap();
    match exit_ev {
        CodexEvent::Exit { failed, .. } => {
            assert!(!failed, "non-blocking warning must NOT fail the run")
        }
        other => panic!("last not Exit: {other:?}"),
    }
}
