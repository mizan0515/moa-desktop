use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use parking_lot::Mutex as PlMutex;
use tokio::sync::{mpsc, watch, Mutex};

use moa_desktop_lib::adapters::pi_rpc::{PiRpcAdapter, PiRpcConfig, PiRpcStartRequest};
use moa_desktop_lib::pi::types::{PiCommand, PiEvent, PiRequestId};
use moa_desktop_lib::process::traits::{ProcessControlInner, StdinCommand};
use moa_desktop_lib::process::{
    ProcessControl, ProcessError, ProcessExit, ProcessHandle, ProcessLine, ProcessRunner,
    ProcessSpec, StdinPolicy, Stream as PStream,
};

#[derive(Debug, Clone, Default)]
struct Captured {
    spec: Option<ProcessSpec>,
    stdin_chunks: Vec<Vec<u8>>,
    stdin_closed: bool,
}

#[derive(Clone)]
struct ScriptRunner {
    script: Vec<ScriptLine>,
    delay: Duration,
    exit_delay: Duration,
    captured: Arc<PlMutex<Captured>>,
}

#[derive(Clone)]
struct ScriptLine {
    stream: PStream,
    line: String,
    partial: bool,
}

impl ScriptRunner {
    fn stdout(script: Vec<&str>) -> Self {
        Self {
            script: script
                .into_iter()
                .map(|line| ScriptLine {
                    stream: PStream::Stdout,
                    line: line.into(),
                    partial: false,
                })
                .collect(),
            delay: Duration::from_millis(20),
            exit_delay: Duration::from_millis(0),
            captured: Arc::new(PlMutex::new(Captured::default())),
        }
    }

    fn with_exit_delay(mut self, delay: Duration) -> Self {
        self.exit_delay = delay;
        self
    }

    fn captured(&self) -> Captured {
        self.captured.lock().clone()
    }
}

#[async_trait]
impl ProcessRunner for ScriptRunner {
    async fn spawn(&self, spec: ProcessSpec) -> Result<ProcessHandle, ProcessError> {
        let line_buf = spec.line_buf.max(1);
        let (line_tx, line_rx) = mpsc::channel::<ProcessLine>(line_buf);
        let (abort_tx, mut abort_rx) = mpsc::channel::<()>(1);
        let (exit_tx, exit_rx) = watch::channel::<Option<ProcessExit>>(None);
        let (stdin_tx, mut stdin_rx) = mpsc::channel::<StdinCommand>(8);

        self.captured.lock().spec = Some(spec.clone());
        let stdin_handle = if spec.stdin == StdinPolicy::Pipe {
            Some(stdin_tx)
        } else {
            None
        };

        let inner = Arc::new(ProcessControlInner {
            pid: 42,
            aborted: AtomicBool::new(false),
            abort_tx,
            timed_out_pending: Arc::new(AtomicBool::new(false)),
            stdin_tx: Mutex::new(stdin_handle),
            exit_watch: exit_rx,
        });
        let control = ProcessControl { inner };

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
        let exit_delay = self.exit_delay;
        tokio::spawn(async move {
            let mut seq = 0;
            let mut aborted = false;
            for line in script {
                tokio::select! {
                    _ = tokio::time::sleep(delay) => {}
                    _ = abort_rx.recv() => {
                        aborted = true;
                        break;
                    }
                }
                let pl = ProcessLine {
                    seq,
                    stream: line.stream,
                    line: line.line,
                    partial: line.partial,
                };
                seq += 1;
                if line_tx.send(pl).await.is_err() {
                    aborted = true;
                    break;
                }
            }
            if !aborted && !exit_delay.is_zero() {
                tokio::select! {
                    _ = tokio::time::sleep(exit_delay) => {}
                    _ = abort_rx.recv() => {
                        aborted = true;
                    }
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

fn cfg() -> PiRpcConfig {
    let mut cfg = PiRpcConfig::new(PathBuf::from("pi"));
    cfg.request_timeout = Duration::from_millis(500);
    cfg.max_pending_requests = 2;
    cfg
}

fn timeout_cfg() -> PiRpcConfig {
    let mut cfg = cfg();
    cfg.request_timeout = Duration::from_millis(10);
    cfg
}

async fn drain_until_exit(rx: &mut mpsc::Receiver<PiEvent>) -> Vec<PiEvent> {
    let mut out = Vec::new();
    while let Some(e) = rx.recv().await {
        let is_exit = matches!(e, PiEvent::Exit { .. });
        out.push(e);
        if is_exit {
            break;
        }
    }
    out
}

#[tokio::test]
async fn pi_rpc_start_spawns_and_maps_valid_response_and_events() {
    let runner = Arc::new(ScriptRunner::stdout(vec![
        r#"{"type":"agent_start"}"#,
        r#"{"type":"turn_start"}"#,
        r#"{"type":"message_update","delta":"hi"}"#,
        r#"{"type":"tool_execution_start","name":"read"}"#,
        r#"{"type":"tool_execution_end","name":"read"}"#,
        r#"{"type":"extension_ui_request","method":"confirm"}"#,
        r#"{"id":"req-1","result":{"ok":true}}"#,
        r#"{"type":"agent_end"}"#,
    ]));
    let adapter = PiRpcAdapter::new(runner.clone(), cfg());
    let mut stream = adapter
        .start(PiRpcStartRequest {
            cwd: PathBuf::from("C:/repo"),
        })
        .await
        .expect("start");

    stream
        .session
        .send_command(
            PiCommand::Prompt {
                prompt: "inspect".into(),
            },
            Some(PiRequestId::from("req-1")),
        )
        .await
        .expect("send");

    let events = drain_until_exit(&mut stream.events).await;
    assert!(matches!(
        events[0],
        PiEvent::RuntimeKind { runtime_kind: "pi" }
    ));
    assert!(events
        .iter()
        .any(|e| matches!(e, PiEvent::AgentStart { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, PiEvent::TurnStart { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, PiEvent::MessageUpdate { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, PiEvent::ToolExecutionStart { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, PiEvent::ToolExecutionEnd { .. })));
    assert!(events.iter().any(|e| matches!(
        e,
        PiEvent::ExtensionUiBlocked {
            method: Some(method),
            ..
        } if method == "confirm"
    )));
    assert!(events.iter().any(|e| matches!(
        e,
        PiEvent::Response { id, .. } if id == "req-1"
    )));
    assert!(events.iter().any(|e| matches!(e, PiEvent::AgentEnd { .. })));
    assert!(matches!(events.last(), Some(PiEvent::Exit { .. })));

    let captured = runner.captured();
    let spec = captured.spec.expect("spawn spec");
    assert_eq!(spec.argv, vec!["pi", "--mode", "rpc", "--no-session"]);
    assert_eq!(spec.stdin, StdinPolicy::Pipe);
    assert_eq!(spec.cwd, PathBuf::from("C:/repo"));
    assert!(!captured.stdin_closed);
    let stdin = String::from_utf8(captured.stdin_chunks.concat()).unwrap();
    assert!(stdin.ends_with('\n'));
    assert!(stdin.contains(r#""id":"req-1""#));
    assert!(stdin.contains(r#""method":"prompt""#));
}

#[tokio::test]
async fn pi_rpc_malformed_json_duplicate_id_and_unknown_response_id_surface_as_events() {
    let runner = Arc::new(ScriptRunner::stdout(vec![
        "not json",
        r#"{"id":"req-1","result":{"ok":true}}"#,
        r#"{"id":"req-1","result":{"ok":true}}"#,
        r#"{"id":"ghost","result":{}}"#,
    ]));
    let adapter = PiRpcAdapter::new(runner, cfg());
    let mut stream = adapter
        .start(PiRpcStartRequest {
            cwd: PathBuf::from("C:/repo"),
        })
        .await
        .expect("start");
    stream
        .session
        .send_command(PiCommand::GetState, Some(PiRequestId::from("req-1")))
        .await
        .expect("send");

    let events = drain_until_exit(&mut stream.events).await;
    assert!(events
        .iter()
        .any(|e| matches!(e, PiEvent::MalformedJson { .. })));
    assert!(events.iter().any(|e| matches!(
        e,
        PiEvent::ProtocolError { reason, .. } if reason.contains("duplicate")
    )));
    assert!(events.iter().any(|e| matches!(
        e,
        PiEvent::ProtocolError { reason, .. } if reason.contains("unknown")
    )));
}

#[tokio::test]
async fn pi_rpc_request_timeout_emits_timeout_event_and_late_response_is_unknown() {
    let runner = Arc::new(
        ScriptRunner::stdout(vec![r#"{"id":"slow","result":{"late":true}}"#])
            .with_exit_delay(Duration::from_millis(100)),
    );
    let adapter = PiRpcAdapter::new(runner, timeout_cfg());
    let mut stream = adapter
        .start(PiRpcStartRequest {
            cwd: PathBuf::from("C:/repo"),
        })
        .await
        .expect("start");
    stream
        .session
        .send_command(PiCommand::GetState, Some(PiRequestId::from("slow")))
        .await
        .expect("send");

    let events = drain_until_exit(&mut stream.events).await;
    assert!(events.iter().any(|e| matches!(
        e,
        PiEvent::ResponseTimeout { id, .. } if id == "slow"
    )));
    assert!(events.iter().any(|e| matches!(
        e,
        PiEvent::ProtocolError { reason, .. } if reason.contains("unknown")
    )));
}

#[tokio::test]
async fn pi_rpc_abort_command_race_aborts_process_without_losing_written_rpc() {
    let runner = Arc::new(ScriptRunner::stdout(vec![]).with_exit_delay(Duration::from_secs(5)));
    let adapter = PiRpcAdapter::new(runner.clone(), cfg());
    let mut stream = adapter
        .start(PiRpcStartRequest {
            cwd: PathBuf::from("C:/repo"),
        })
        .await
        .expect("start");

    stream
        .session
        .abort_pi_turn(Some(PiRequestId::from("abort-1")))
        .await
        .expect("abort");

    let events = drain_until_exit(&mut stream.events).await;
    assert!(matches!(events.last(), Some(PiEvent::Exit { exit }) if exit.aborted));
    let stdin = String::from_utf8(runner.captured().stdin_chunks.concat()).unwrap();
    assert!(stdin.contains(r#""id":"abort-1""#));
    assert!(stdin.contains(r#""method":"abort""#));
}

#[tokio::test]
async fn pi_rpc_duplicate_request_id_and_pending_queue_overflow_reject_before_write() {
    let runner = Arc::new(ScriptRunner::stdout(vec![]).with_exit_delay(Duration::from_millis(120)));
    let adapter = PiRpcAdapter::new(runner.clone(), cfg());
    let stream = adapter
        .start(PiRpcStartRequest {
            cwd: PathBuf::from("C:/repo"),
        })
        .await
        .expect("start");

    stream
        .session
        .send_command(PiCommand::GetState, Some(PiRequestId::from("same")))
        .await
        .expect("first send");
    let duplicate = stream
        .session
        .send_command(PiCommand::Compact, Some(PiRequestId::from("same")))
        .await
        .expect_err("duplicate id rejected");
    assert!(duplicate.message.contains("duplicate"));

    stream
        .session
        .send_command(
            PiCommand::SetModel { model: "p".into() },
            Some(PiRequestId::from("two")),
        )
        .await
        .expect("second pending");
    let overflow = stream
        .session
        .send_command(
            PiCommand::Prompt { prompt: "x".into() },
            Some(PiRequestId::from("three")),
        )
        .await
        .expect_err("queue overflow rejected");
    assert!(overflow.message.contains("queue overflow"));

    stream.session.control.abort().await.unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    let stdin = String::from_utf8(runner.captured().stdin_chunks.concat()).unwrap();
    assert_eq!(
        stdin.matches('\n').count(),
        2,
        "only accepted requests write"
    );
}
