//! Integration tests for T5a — `ClaudeAdapter`.
//!
//! Uses an in-test `ScriptRunner` (stdin-capable analogue of `MockRunner`)
//! to stream canned `claude -p --output-format stream-json` lines and
//! assert the adapter's argv shape, prompt routing, and event sequence.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use parking_lot::Mutex as PlMutex;
use tokio::sync::{mpsc, watch, Mutex};

use moa_desktop_lib::adapters::claude::{
    ClaudeAdapter, ClaudeConfig, ClaudeEvent, FirstPassRequest, MutationRequest,
};
use moa_desktop_lib::process::traits::{ProcessControlInner, StdinCommand};
use moa_desktop_lib::process::{
    ProcessControl, ProcessError, ProcessExit, ProcessHandle, ProcessLine, ProcessRunner,
    ProcessSpec, Stream as PStream,
};

// ---- ScriptRunner ----------------------------------------------------------

/// What the runner captured from one spawn call.
#[derive(Debug, Clone, Default)]
struct Captured {
    spec: Option<ProcessSpec>,
    stdin_chunks: Vec<Vec<u8>>,
    stdin_closed: bool,
}

#[derive(Clone)]
struct ScriptRunner {
    /// Raw JSONL the runner will stream as stdout, one line per emit.
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

        // Capture spec.
        self.captured.lock().spec = Some(spec.clone());

        let inner = Arc::new(ProcessControlInner {
            pid: 1,
            aborted: AtomicBool::new(false),
            abort_tx,
            stdin_tx: Mutex::new(Some(stdin_tx)),
            exit_watch: exit_rx,
        });
        let control = ProcessControl { inner };

        // Stdin pump task — record every write/close.
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

fn cfg() -> ClaudeConfig {
    let mut env = std::collections::HashMap::new();
    env.insert("ENABLE_CLAUDEAI_MCP_SERVERS".into(), "false".into());
    ClaudeConfig {
        program: PathBuf::from("claude.exe"),
        model: "opus".into(),
        max_turns_firstpass: 20,
        max_turns_mutation: 30,
        guard_text: "GUARD-TEXT".into(),
        firstpass_template: "FP task={{task}} files=\n{{files}}".into(),
        mutation_template: "MUT task={{task}} wt={{worktree}}".into(),
        env,
    }
}

async fn drain(rx: &mut mpsc::Receiver<ClaudeEvent>) -> Vec<ClaudeEvent> {
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
        r#"{"type":"system","subtype":"init","session_id":"sess-1"}"#,
        r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hello"}]}}"#,
        r#"{"type":"result","is_error":false,"num_turns":3}"#,
    ];
    let runner = Arc::new(ScriptRunner::new(script));
    let adapter = ClaudeAdapter::new(runner.clone(), cfg());

    let mut stream = adapter
        .firstpass(FirstPassRequest {
            task: "diagnose flaky test".into(),
            files: vec!["src/foo.rs:1-10".into()],
            cwd: PathBuf::from("."),
        })
        .await
        .expect("firstpass spawn");

    let events = drain(&mut stream.events).await;

    // Sequence: SystemInit, Assistant, Result, Exit
    assert_eq!(events.len(), 4, "got {events:#?}");
    assert!(matches!(events[0], ClaudeEvent::SystemInit { .. }));
    match &events[1] {
        ClaudeEvent::Assistant { text, .. } => assert_eq!(text, "hello"),
        other => panic!("idx 1 not Assistant: {other:?}"),
    }
    match &events[2] {
        ClaudeEvent::Result {
            is_error,
            num_turns,
            hook_blocked,
            ..
        } => {
            assert!(!is_error);
            assert_eq!(*num_turns, Some(3));
            assert!(!hook_blocked);
        }
        other => panic!("idx 2 not Result: {other:?}"),
    }
    match &events[3] {
        ClaudeEvent::Exit { exit, hook_blocked } => {
            assert!(exit.is_clean());
            assert!(!hook_blocked);
        }
        other => panic!("last not Exit: {other:?}"),
    }

    // Argv captured: program first, then -p, with required flags as
    // separate elements. Prompt MUST NOT appear in argv (S1 finding #2).
    let cap = runner.captured();
    let spec = cap.spec.expect("spawn captured");
    assert_eq!(spec.argv[0], "claude.exe");
    assert!(spec.argv.iter().any(|s| s == "-p"));
    assert!(!spec.argv.iter().any(|s| s.contains("diagnose flaky test")));

    // Prompt routed via stdin then closed.
    assert!(cap.stdin_closed, "stdin must be closed after prompt write");
    let prompt = String::from_utf8(cap.stdin_chunks.concat()).unwrap();
    assert!(prompt.contains("FP task=diagnose flaky test"));
    assert!(prompt.contains("- src/foo.rs:1-10"));
}

#[tokio::test]
async fn hook_block_propagates_to_result_and_exit() {
    // S8 critical signal: hook_response exit_code=2 → result num_turns=0.
    let script = vec![
        r#"{"type":"system","subtype":"init","session_id":"sess-2"}"#,
        r#"{"type":"system","subtype":"hook_response","hook_event":"UserPromptSubmit","exit_code":2}"#,
        r#"{"type":"result","is_error":false,"num_turns":0}"#,
    ];
    let runner = Arc::new(ScriptRunner::new(script));
    let adapter = ClaudeAdapter::new(runner, cfg());

    let mut stream = adapter
        .firstpass(FirstPassRequest {
            task: "x".into(),
            files: vec![],
            cwd: PathBuf::from("."),
        })
        .await
        .expect("spawn");

    let events = drain(&mut stream.events).await;
    let hook_ev = events
        .iter()
        .find(|e| matches!(e, ClaudeEvent::HookEvent { .. }))
        .expect("HookEvent emitted");
    match hook_ev {
        ClaudeEvent::HookEvent { blocked, .. } => assert!(*blocked),
        _ => unreachable!(),
    }
    let result_ev = events
        .iter()
        .find(|e| matches!(e, ClaudeEvent::Result { .. }))
        .unwrap();
    match result_ev {
        ClaudeEvent::Result { hook_blocked, .. } => {
            assert!(*hook_blocked, "result must inherit hook_blocked")
        }
        _ => unreachable!(),
    }
    let exit_ev = events.last().unwrap();
    match exit_ev {
        ClaudeEvent::Exit { hook_blocked, .. } => {
            assert!(*hook_blocked, "exit must carry hook_blocked")
        }
        other => panic!("last not Exit: {other:?}"),
    }
}

#[tokio::test]
async fn mutation_uses_worktree_cwd_and_acceptedits_argv() {
    let script = vec![r#"{"type":"result","is_error":false,"num_turns":1}"#];
    let runner = Arc::new(ScriptRunner::new(script));
    let adapter = ClaudeAdapter::new(runner.clone(), cfg());

    let wt = PathBuf::from("C:/temp/moa-desktop-wt-test").is_absolute();
    let wt_path = if wt {
        PathBuf::from("C:/temp/moa-desktop-wt-test")
    } else {
        PathBuf::from("/tmp/moa-desktop-wt-test")
    };

    let mut stream = adapter
        .mutation(MutationRequest {
            task: "apply patch".into(),
            worktree_path: wt_path.clone(),
        })
        .await
        .expect("spawn");

    // Drain to completion so spec capture is stable.
    let _ = drain(&mut stream.events).await;

    let cap = runner.captured();
    let spec = cap.spec.expect("spec captured");
    assert_eq!(spec.cwd, wt_path);

    let i = spec
        .argv
        .iter()
        .position(|s| s == "--permission-mode")
        .expect("--permission-mode present");
    assert_eq!(spec.argv[i + 1], "acceptEdits");
    assert!(
        !spec.argv.iter().any(|s| s == "--disallowedTools"),
        "mutation must not carry disallowedTools"
    );
    let prompt = String::from_utf8(cap.stdin_chunks.concat()).unwrap();
    assert!(prompt.contains("MUT task=apply patch"));
    assert!(prompt.contains("apply patch"));
}

#[tokio::test]
async fn malformed_json_line_surfaces_event_then_continues() {
    let script = vec![
        r#"{"type":"system","subtype":"init"}"#,
        r#"NOT JSON"#,
        r#"{"type":"result","is_error":false,"num_turns":1}"#,
    ];
    let runner = Arc::new(ScriptRunner::new(script));
    let adapter = ClaudeAdapter::new(runner, cfg());

    let mut stream = adapter
        .firstpass(FirstPassRequest {
            task: "t".into(),
            files: vec![],
            cwd: PathBuf::from("."),
        })
        .await
        .expect("spawn");

    let events = drain(&mut stream.events).await;
    let mj = events
        .iter()
        .find(|e| matches!(e, ClaudeEvent::MalformedJson { .. }))
        .expect("MalformedJson emitted");
    if let ClaudeEvent::MalformedJson { line, .. } = mj {
        assert_eq!(line, "NOT JSON");
    }
    // Must still see a Result and an Exit afterward (parser does not
    // bail on a single bad line).
    assert!(events.iter().any(|e| matches!(e, ClaudeEvent::Result { .. })));
    assert!(matches!(events.last(), Some(ClaudeEvent::Exit { .. })));
}
