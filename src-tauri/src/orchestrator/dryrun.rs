//! T7-thin — dry-run orchestrator (walking skeleton).
//!
//! Drives the Phase 1 demo: user task → mock first-pass × 2 (parallel) →
//! mock synthesis → mock adversarial × 2 (parallel) → mock final report.
//! No real CLI is spawned — every phase reads a canned JSONL file from
//! `<repo>/mockResponses/` via `MockRunner` (T8).
//!
//! Events are emitted on the Tauri channel `dryrun://event` with shape:
//!   { session_id, phase, lane?, kind: "phase_start"|"line"|"phase_end"
//!                                     |"session_start"|"session_done"
//!                                     |"session_cancelled"|"session_error",
//!     payload? }
//!
//! State machine (hand-rolled, see ticket §Approach):
//!   idle → preflight → fp (claude || codex) → synth → adv (claude || codex)
//!        → final → done
//!
//! T7-full will replace this with the real adapter pipeline; this module is
//! intentionally narrow so the demo loop is debuggable end-to-end.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::{oneshot, Mutex};

/// FIX-C — monotonic suffix for dryrun session ids. See `new_session_id`.
static DRYRUN_SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

/// FIX-C — driver waits for `dryrun_ack` before its first emit. Same
/// rationale as the production orchestrator (`mod.rs::ACK_TIMEOUT`).
const DRYRUN_ACK_TIMEOUT: Duration = Duration::from_secs(5);

use crate::mock::MockRunner;
use crate::process::{ProcessControl, ProcessRunner, ProcessSpec};

pub const EVENT_NAME: &str = "dryrun://event";

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    Preflight,
    FirstPass,
    Synthesis,
    Adversarial,
    Final,
}

impl Phase {
    fn as_str(self) -> &'static str {
        match self {
            Phase::Preflight => "preflight",
            Phase::FirstPass => "first-pass",
            Phase::Synthesis => "synthesis",
            Phase::Adversarial => "adversarial",
            Phase::Final => "final",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Lane {
    System,
    Claude,
    Codex,
}

impl Lane {
    fn as_str(self) -> &'static str {
        match self {
            Lane::System => "system",
            Lane::Claude => "claude",
            Lane::Codex => "codex",
        }
    }
}

#[derive(Clone, Serialize)]
struct EventEnvelope<'a> {
    session_id: &'a str,
    phase: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    lane: Option<&'static str>,
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload: Option<serde_json::Value>,
}

fn emit(
    app: &AppHandle,
    session_id: &str,
    phase: Phase,
    lane: Option<Lane>,
    kind: &'static str,
    payload: Option<serde_json::Value>,
) {
    let env = EventEnvelope {
        session_id,
        phase: phase.as_str(),
        lane: lane.map(|l| l.as_str()),
        kind,
        payload,
    };
    let _ = app.emit(EVENT_NAME, env);
}

/// Per-session handle held by the coordinator. `cancelled` is checked between
/// phases; the active `ProcessControl` is aborted immediately on cancel.
struct SessionHandle {
    cancelled: Arc<AtomicBool>,
    active: Arc<Mutex<Option<ProcessControl>>>,
    /// FIX-C — same emit-ordering handshake as the production orchestrator.
    ack_tx: Mutex<Option<oneshot::Sender<()>>>,
}

#[derive(Default)]
pub struct DryRunCoordinator {
    sessions: Mutex<HashMap<String, SessionHandle>>,
}

impl DryRunCoordinator {
    pub fn new() -> Self {
        Self::default()
    }

    /// FIX-C — `dr-{ms}` collided when two `dryrun_start` calls landed in the
    /// same millisecond. Add a process-wide atomic counter suffix.
    fn new_session_id() -> String {
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let n = DRYRUN_SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("dr-{ms}-{n}")
    }

    async fn register(&self, sid: &str, handle: SessionHandle) {
        self.sessions.lock().await.insert(sid.to_string(), handle);
    }

    /// FIX-C — release the parked driver. Idempotent.
    pub async fn ack(&self, sid: &str) -> bool {
        let map = self.sessions.lock().await;
        let Some(h) = map.get(sid) else { return false };
        let mut slot = h.ack_tx.lock().await;
        match slot.take() {
            Some(tx) => tx.send(()).is_ok(),
            None => false,
        }
    }

    async fn unregister(&self, sid: &str) {
        self.sessions.lock().await.remove(sid);
    }

    async fn cancel(&self, sid: &str) -> bool {
        let map = self.sessions.lock().await;
        if let Some(h) = map.get(sid) {
            h.cancelled.store(true, Ordering::SeqCst);
            let active = h.active.lock().await;
            if let Some(ctl) = active.as_ref() {
                let _ = ctl.abort().await;
            }
            true
        } else {
            false
        }
    }
}

fn mock_dir() -> PathBuf {
    // Resolved at compile time; dev-tool path. Production would inject via
    // settings — out of scope for T7-thin.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().map(|p| p.join("mockResponses")).unwrap_or_else(|| manifest.join("mockResponses"))
}

fn mock_file(name: &str) -> PathBuf {
    mock_dir().join(name)
}

/// Stream every line of a canned mock file under one phase/lane. Stores the
/// `ProcessControl` in `active` while running so cancel can abort mid-stream.
async fn run_mock_phase(
    app: &AppHandle,
    sid: &str,
    phase: Phase,
    lane: Lane,
    file: &str,
    active: &Mutex<Option<ProcessControl>>,
    cancelled: &AtomicBool,
) -> Result<(), String> {
    if cancelled.load(Ordering::SeqCst) {
        return Err("cancelled".into());
    }

    emit(app, sid, phase, Some(lane), "phase_start", None);

    let runner = MockRunner::new(mock_file(file));
    let spec = ProcessSpec::new(
        vec![format!("mock:{}", file)],
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    );

    let handle = runner
        .spawn(spec)
        .await
        .map_err(|e| format!("mock spawn failed ({file}): {e}"))?;
    let crate::process::ProcessHandle { control, mut lines } = handle;

    {
        let mut slot = active.lock().await;
        *slot = Some(control.clone());
    }

    while let Some(pl) = lines.recv().await {
        let parsed: serde_json::Value =
            serde_json::from_str(&pl.line).unwrap_or(serde_json::Value::String(pl.line.clone()));
        emit(app, sid, phase, Some(lane), "line", Some(parsed));
    }

    let exit = control
        .wait(None)
        .await
        .map_err(|e| format!("mock wait failed ({file}): {e}"))?;

    {
        let mut slot = active.lock().await;
        *slot = None;
    }

    if exit.aborted || cancelled.load(Ordering::SeqCst) {
        return Err("cancelled".into());
    }
    if !exit.is_clean() {
        return Err(format!("mock {file} exited unclean: code={:?}", exit.code));
    }

    emit(app, sid, phase, Some(lane), "phase_end", None);
    Ok(())
}

/// Walking-skeleton state machine. Two-lane phases run serially in this thin
/// version (Claude then Codex, same phase) — the UI still treats them as
/// independent lanes via the `lane` field, and T7-full will parallelize them
/// once real adapters land.
async fn run_session(
    app: AppHandle,
    sid: String,
    task: String,
    handle: SessionHandle,
    ack_rx: oneshot::Receiver<()>,
) {
    // FIX-C — gate the first emit on the frontend's ack. Same rationale as
    // the production orchestrator. On timeout / sender-dropped the
    // frontend has no record of this sid, so exit silently.
    match tokio::time::timeout(DRYRUN_ACK_TIMEOUT, ack_rx).await {
        Ok(Ok(())) => {}
        _ => return,
    }

    emit(
        &app,
        &sid,
        Phase::Preflight,
        Some(Lane::System),
        "session_start",
        Some(serde_json::json!({ "task": task })),
    );

    // Preflight is a no-op in mock mode beyond the announce/ack pair.
    emit(&app, &sid, Phase::Preflight, Some(Lane::System), "phase_start", None);
    emit(&app, &sid, Phase::Preflight, Some(Lane::System), "phase_end", None);

    let cancelled = handle.cancelled.clone();
    let active = handle.active.clone();

    let phases: &[(Phase, Lane, &str)] = &[
        (Phase::FirstPass, Lane::Claude, "claude_firstpass.json"),
        (Phase::FirstPass, Lane::Codex, "codex_firstpass.json"),
        (Phase::Synthesis, Lane::System, "synthesis.json"),
        (Phase::Adversarial, Lane::Claude, "claude_adversarial.json"),
        (Phase::Adversarial, Lane::Codex, "codex_adversarial.json"),
        (Phase::Final, Lane::System, "final_report.json"),
    ];

    for (phase, lane, file) in phases {
        match run_mock_phase(&app, &sid, *phase, *lane, file, &active, &cancelled).await {
            Ok(()) => {}
            Err(e) if e == "cancelled" => {
                emit(&app, &sid, *phase, Some(*lane), "session_cancelled", None);
                return;
            }
            Err(e) => {
                emit(
                    &app,
                    &sid,
                    *phase,
                    Some(*lane),
                    "session_error",
                    Some(serde_json::json!({ "message": e })),
                );
                return;
            }
        }
    }

    emit(&app, &sid, Phase::Final, Some(Lane::System), "session_done", None);
}

#[tauri::command]
pub async fn dryrun_start(
    app: AppHandle,
    coordinator: State<'_, DryRunCoordinator>,
    task: String,
) -> Result<String, String> {
    let sid = DryRunCoordinator::new_session_id();
    let (ack_tx, ack_rx) = oneshot::channel::<()>();
    let handle = SessionHandle {
        cancelled: Arc::new(AtomicBool::new(false)),
        active: Arc::new(Mutex::new(None)),
        ack_tx: Mutex::new(Some(ack_tx)),
    };
    let handle_clone = SessionHandle {
        cancelled: handle.cancelled.clone(),
        active: handle.active.clone(),
        // The driver's clone never holds the ack sender — it is owned by
        // the registered handle so `dryrun_ack` can take it.
        ack_tx: Mutex::new(None),
    };
    coordinator.register(&sid, handle).await;

    let app_for_task = app.clone();
    let sid_for_task = sid.clone();

    tokio::spawn(async move {
        run_session(
            app_for_task.clone(),
            sid_for_task.clone(),
            task,
            handle_clone,
            ack_rx,
        )
        .await;
        if let Some(coord) = app_for_task.try_state::<DryRunCoordinator>() {
            coord.unregister(&sid_for_task).await;
        }
    });

    Ok(sid)
}

/// FIX-C — frontend acknowledges the session shell is in the store; releases
/// the driver to emit `session_start`.
#[tauri::command]
pub async fn dryrun_ack(
    coordinator: State<'_, DryRunCoordinator>,
    session_id: String,
) -> Result<bool, String> {
    Ok(coordinator.ack(&session_id).await)
}

#[tauri::command]
pub async fn dryrun_cancel(
    coordinator: State<'_, DryRunCoordinator>,
    session_id: String,
) -> Result<bool, String> {
    Ok(coordinator.cancel(&session_id).await)
}
