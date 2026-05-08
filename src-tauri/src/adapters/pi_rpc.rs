//! T15b — Pi RPC adapter.
//!
//! Spawns `pi --mode rpc --no-session` through [`ProcessRunner`] and keeps the
//! MVP lane read-only/research/reviewer only. Pi mutation ownership is rejected
//! before any process spawn.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, Mutex};

use crate::pi::rpc::{
    parse_record, runtime_kind_event, timeout_duration_ms, PendingError, PendingRequests,
};
use crate::pi::types::{PiCommand, PiEvent, PiRequestId};
use crate::process::traits::StdinPolicy;
use crate::process::{
    ProcessControl, ProcessError, ProcessErrorKind, ProcessExit, ProcessHandle, ProcessLine,
    ProcessRunner, ProcessSpec, Stream as PStream,
};

#[derive(Debug, Clone)]
pub struct PiRpcConfig {
    pub program: PathBuf,
    pub env: HashMap<String, String>,
    pub request_timeout: Duration,
    pub max_pending_requests: usize,
    pub event_buffer: usize,
}

impl PiRpcConfig {
    pub fn new(program: PathBuf) -> Self {
        Self {
            program,
            env: HashMap::new(),
            request_timeout: Duration::from_secs(30),
            max_pending_requests: 128,
            event_buffer: 256,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PiRpcStartRequest {
    pub cwd: PathBuf,
}

#[derive(Debug, Clone)]
pub struct PiMutationRequest {
    pub cwd: PathBuf,
    pub task: String,
}

pub struct PiRpcStream {
    pub session: PiRpcSession,
    pub events: mpsc::Receiver<PiEvent>,
}

#[derive(Clone)]
pub struct PiRpcSession {
    pub control: ProcessControl,
    pending: Arc<Mutex<PendingRequests>>,
    next_id: Arc<AtomicU64>,
    tx: mpsc::Sender<PiEvent>,
    request_timeout: Duration,
}

impl PiRpcSession {
    pub async fn send_command(
        &self,
        command: PiCommand,
        id: Option<PiRequestId>,
    ) -> Result<Option<PiRequestId>, ProcessError> {
        if let Some(id) = &id {
            self.pending
                .lock()
                .await
                .begin(id)
                .map_err(pending_error_to_process_error)?;
        }

        let payload = command.to_rpc_request(id.as_ref());
        let mut bytes = serde_json::to_vec(&payload).map_err(|error| ProcessError {
            kind: ProcessErrorKind::MalformedJson,
            message: format!("failed to encode Pi RPC request: {error}"),
            exit_code: None,
            stderr_tail: String::new(),
        })?;
        bytes.push(b'\n');
        if let Err(error) = self.control.write_stdin(bytes).await {
            if let Some(id) = &id {
                self.pending.lock().await.cancel(id.as_str());
            }
            return Err(error);
        }

        if let Some(id) = id.clone() {
            self.spawn_timeout_watch(id);
        }

        Ok(id)
    }

    pub async fn send_command_auto_id(
        &self,
        command: PiCommand,
    ) -> Result<PiRequestId, ProcessError> {
        let id = PiRequestId::new(format!(
            "pi-{}",
            self.next_id.fetch_add(1, Ordering::SeqCst)
        ));
        self.send_command(command, Some(id.clone())).await?;
        Ok(id)
    }

    pub async fn abort_pi_turn(&self, id: Option<PiRequestId>) -> Result<(), ProcessError> {
        let _ = self.send_command(PiCommand::Abort, id).await?;
        self.control.abort().await
    }

    fn spawn_timeout_watch(&self, id: PiRequestId) {
        let pending = self.pending.clone();
        let tx = self.tx.clone();
        let timeout = self.request_timeout;
        tokio::spawn(async move {
            tokio::time::sleep(timeout).await;
            if pending.lock().await.timeout(id.as_str()) {
                let _ = tx
                    .send(PiEvent::ResponseTimeout {
                        id: id.0,
                        timeout_ms: timeout_duration_ms(timeout),
                    })
                    .await;
            }
        });
    }
}

pub struct PiRpcAdapter {
    runner: Arc<dyn ProcessRunner>,
    config: PiRpcConfig,
}

impl PiRpcAdapter {
    pub fn new(runner: Arc<dyn ProcessRunner>, config: PiRpcConfig) -> Self {
        Self { runner, config }
    }

    pub fn config(&self) -> &PiRpcConfig {
        &self.config
    }

    pub fn rpc_argv(&self) -> Vec<String> {
        vec![
            self.config.program.to_string_lossy().into_owned(),
            "--mode".into(),
            "rpc".into(),
            "--no-session".into(),
        ]
    }

    pub async fn start(&self, req: PiRpcStartRequest) -> Result<PiRpcStream, ProcessError> {
        let spec = ProcessSpec::new(self.rpc_argv(), req.cwd)
            .with_env(self.config.env.clone())
            .with_stdin(StdinPolicy::Pipe);
        let ProcessHandle { control, lines } = self.runner.spawn(spec).await?;

        let (tx, rx) = mpsc::channel::<PiEvent>(self.config.event_buffer.max(1));
        let pending = Arc::new(Mutex::new(PendingRequests::new(
            self.config.max_pending_requests,
        )));
        let session = PiRpcSession {
            control: control.clone(),
            pending: pending.clone(),
            next_id: Arc::new(AtomicU64::new(1)),
            tx: tx.clone(),
            request_timeout: self.config.request_timeout,
        };

        tokio::spawn(parser_task(lines, tx, pending, control));

        Ok(PiRpcStream {
            session,
            events: rx,
        })
    }

    pub async fn mutation(&self, req: PiMutationRequest) -> Result<PiRpcStream, ProcessError> {
        Err(ProcessError {
            kind: ProcessErrorKind::PermissionDenied,
            message: format!(
                "Pi mutation owner reject: runtimeKind=\"pi\" is read-only/research/reviewer only before T15g (cwd={}, task={})",
                req.cwd.display(),
                req.task
            ),
            exit_code: None,
            stderr_tail: String::new(),
        })
    }
}

async fn parser_task(
    mut lines: mpsc::Receiver<ProcessLine>,
    tx: mpsc::Sender<PiEvent>,
    pending: Arc<Mutex<PendingRequests>>,
    control: ProcessControl,
) {
    let _ = tx.send(runtime_kind_event()).await;

    while let Some(pl) = lines.recv().await {
        match pl.stream {
            PStream::Stderr => {
                if tx.send(PiEvent::Stderr { line: pl.line }).await.is_err() {
                    return;
                }
            }
            PStream::Stdout => {
                let event = if pl.partial {
                    PiEvent::MalformedJson {
                        line: pl.line,
                        error: "partial Pi RPC frame rejected; strict JSONL requires LF delimiter"
                            .into(),
                    }
                } else {
                    let mut guard = pending.lock().await;
                    parse_record(pl.line.trim_end_matches('\r'), &mut guard)
                };
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
    let _ = tx.send(PiEvent::Exit { exit }).await;
}

fn pending_error_to_process_error(error: PendingError) -> ProcessError {
    let message = match error {
        PendingError::DuplicateId(id) => format!("duplicate Pi RPC request id {id}"),
        PendingError::UnknownResponseId(id) => format!("unknown Pi RPC response id {id}"),
        PendingError::QueueOverflow { max } => {
            format!("Pi RPC pending request queue overflow at max {max}")
        }
    };
    ProcessError {
        kind: ProcessErrorKind::MalformedJson,
        message,
        exit_code: None,
        stderr_tail: String::new(),
    }
}

#[cfg(test)]
mod unit {
    use super::*;
    use async_trait::async_trait;
    use std::path::Path;

    struct Never;

    #[async_trait]
    impl ProcessRunner for Never {
        async fn spawn(&self, _: ProcessSpec) -> Result<ProcessHandle, ProcessError> {
            unreachable!("unit test does not spawn")
        }
    }

    #[test]
    fn pi_rpc_argv_uses_no_session_as_separate_elements() {
        let adapter = PiRpcAdapter::new(Arc::new(Never), PiRpcConfig::new(PathBuf::from("pi")));
        assert_eq!(
            adapter.rpc_argv(),
            vec!["pi", "--mode", "rpc", "--no-session"]
        );
    }

    #[tokio::test]
    async fn pi_rpc_mutation_owner_is_rejected_before_spawn() {
        let adapter = PiRpcAdapter::new(Arc::new(Never), PiRpcConfig::new(PathBuf::from("pi")));
        let err = match adapter
            .mutation(PiMutationRequest {
                cwd: Path::new("C:/repo").to_path_buf(),
                task: "edit file".into(),
            })
            .await
        {
            Ok(_) => panic!("Pi mutation owner must reject"),
            Err(err) => err,
        };
        assert_eq!(err.kind, ProcessErrorKind::PermissionDenied);
        assert!(err.message.contains("Pi mutation owner reject"));
        assert!(err.message.contains("runtimeKind=\"pi\""));
    }
}
