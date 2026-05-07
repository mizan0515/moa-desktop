//! T7-full — orchestrator state machine + Tauri command surface.
//!
//! Sub-modules:
//! - `state`     — Phase / Lane / Flow / WorkerMode / SessionState enums.
//! - `flow`      — Flow A/B/C/D classifier (heuristic + override).
//! - `supervisor`— Lane panic boundary (`tokio::spawn` + JoinHandle/AbortHandle wrap).
//! - `adversarial` — adversarial-round prompt builder + verdict parser.
//! - `verify`    — post-mutation verification command runner.
//! - `dryrun`    — T7-thin walking skeleton (kept for legacy demo path).
//!
//! T7-thin's `dryrun://event` Tauri channel is left untouched. T7-full
//! emits on a separate channel `orch://event` with the same envelope shape
//! (session_id / phase / lane / kind / payload) — UI subscribes to either.

pub mod adversarial;
pub mod dryrun;
pub mod flow;
pub mod state;
pub mod supervisor;
pub mod verify;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::{mpsc, oneshot, Mutex};

use crate::adapters::{
    claude::{ClaudeAdapter, ClaudeEvent, FirstPassRequest as ClaudeFirstPass},
    codex::{CodexAdapter, CodexEvent, FirstPassRequest as CodexFirstPass},
};
use crate::cancel::CancelRegistry;
use crate::journal::schema::{Entry, Phase as JournalPhase};
use crate::journal::writer::JournalWriter;
use crate::lock::manager::{LockManager, LockSource};
use crate::process::{ProcessControl, ProcessRunner};
use crate::synthesis::{extract_from_text, WorkerEvent};

use self::flow::{classify, ClassifyInput};
use self::state::{Flow, Lane, Phase, SessionState, MAX_ADVERSARIAL_ROUNDS};
use self::supervisor::{LaneError, LaneSupervisor};

pub const EVENT_NAME: &str = "orch://event";

/// FIX-C — monotonic suffix for session ids (see `new_session_id`).
static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Upper bound on how long the driver waits for `orch_ack` before falling
/// through and emitting `session_start`. A frontend that crashed mid-handshake
/// would otherwise leak a parked driver task. 5 s is generous — the ack is a
/// single round-trip across a local IPC channel.
const ACK_TIMEOUT: Duration = Duration::from_secs(5);

// ─── event envelope (UI-compatible with dryrun://event) ────────────────────

#[derive(Clone, Serialize)]
struct EventEnvelope<'a> {
    session_id: &'a str,
    phase: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    lane: Option<&'static str>,
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload: Option<Value>,
}

fn emit(
    app: &AppHandle,
    sid: &str,
    phase: Phase,
    lane: Option<Lane>,
    kind: &'static str,
    payload: Option<Value>,
) {
    let env = EventEnvelope {
        session_id: sid,
        phase: phase.as_str(),
        lane: lane.map(|l| l.as_str()),
        kind,
        payload,
    };
    let _ = app.emit(EVENT_NAME, env);
}

// ─── public start / cancel / submit-synthesis / confirm-mutation API ───────

#[derive(Debug, Clone, Deserialize)]
pub struct OrchestrationStart {
    pub task: String,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub override_flow: Option<Flow>,
    /// Use T8 mock runner (dry-run mode). Settings.mockMode passes this.
    #[serde(default)]
    pub mock_mode: bool,
    /// cwd for adapter spawn.
    pub cwd: PathBuf,
    /// Project id (T4 lock key namespace).
    pub project_id: String,
    /// Verification command to run post-mutation. Empty = skip.
    #[serde(default)]
    pub verify_cmd: Option<String>,
}

// ─── coordinator state ────────────────────────────────────────────────────

/// External commands the frontend posts back during a session — synthesis
/// result (TS-side T3 merge), mutation confirm, abort.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SessionCommand {
    SubmitSynthesis { synthesis_json: String },
    ConfirmMutation { proceed: bool },
    Cancel,
}

struct SessionHandle {
    cancelled: Arc<AtomicBool>,
    /// FIX-F — registry of all live worker processes for this session.
    /// Pre-FIX-F this was `Mutex<Option<ProcessControl>>` — a single slot
    /// that only stored the *last* lane's handle. When two first-pass
    /// adapters ran in parallel, lane B overwrote lane A's handle and
    /// `cancel` only aborted lane B (lane A kept burning tokens until
    /// natural exit). The CancelRegistry holds Arc-cloned controls under
    /// stable run-ids ("claude-firstpass" / "codex-firstpass" / "mutation"
    /// / "claude-adv-r{n}" / "codex-adv-r{n}") so `abort_all` walks every
    /// lane.
    active: Arc<CancelRegistry>,
    /// Channel to the driver task — frontend commands are routed here.
    cmd_tx: mpsc::Sender<SessionCommand>,
    /// Latest published state (best-effort snapshot for `get_state`).
    state: Arc<Mutex<SessionState>>,
    /// FIX-C — gates the driver's first emit until the frontend has
    /// inserted the session shell into its store. `orch_ack` consumes
    /// this sender; on second ack the option is `None` (idempotent).
    ack_tx: Mutex<Option<oneshot::Sender<()>>>,
}

#[derive(Default)]
pub struct OrchestrationCoordinator {
    sessions: Mutex<HashMap<String, SessionHandle>>,
}

impl OrchestrationCoordinator {
    pub fn new() -> Self {
        Self::default()
    }

    /// FIX-C — ms+monotonic counter. Two `orch_start` calls inside the same
    /// millisecond used to produce identical sids and the second silently
    /// overwrote the first SessionHandle in the coordinator map. The atomic
    /// counter eliminates the collision; the ms prefix is kept so existing
    /// log/journal greps that key on a leading timestamp still work.
    fn new_session_id() -> String {
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let n = SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("orch-{ms}-{n}")
    }

    /// Public test hook for `tests/session_ordering_test.rs`. Not used in
    /// production paths.
    #[doc(hidden)]
    pub fn new_session_id_for_test() -> String {
        Self::new_session_id()
    }

    async fn register(&self, sid: &str, h: SessionHandle) {
        self.sessions.lock().await.insert(sid.to_string(), h);
    }

    /// Test helper — register a placeholder handle wired only with the ack
    /// channel. Returns the receiver so a test can verify ack delivery.
    #[doc(hidden)]
    pub async fn register_with_ack(&self, sid: &str) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();
        let (cmd_tx, _cmd_rx) = mpsc::channel::<SessionCommand>(1);
        let h = SessionHandle {
            cancelled: Arc::new(AtomicBool::new(false)),
            active: Arc::new(CancelRegistry::new()),
            cmd_tx,
            state: Arc::new(Mutex::new(SessionState::Idle)),
            ack_tx: Mutex::new(Some(tx)),
        };
        self.register(sid, h).await;
        rx
    }

    /// FIX-F test hook — register a session whose `active` registry is
    /// shared, so a test can pre-populate it with multiple `ProcessControl`
    /// clones and assert `Cancel` aborts every one (not just the last).
    #[doc(hidden)]
    pub async fn register_with_active(
        &self,
        sid: &str,
        active: Arc<CancelRegistry>,
    ) -> mpsc::Receiver<SessionCommand> {
        let (tx, rx) = oneshot::channel();
        let (cmd_tx, cmd_rx) = mpsc::channel::<SessionCommand>(8);
        let h = SessionHandle {
            cancelled: Arc::new(AtomicBool::new(false)),
            active,
            cmd_tx,
            state: Arc::new(Mutex::new(SessionState::Idle)),
            ack_tx: Mutex::new(Some(tx)),
        };
        // Drop the ack receiver — caller doesn't need it.
        drop(rx);
        self.register(sid, h).await;
        cmd_rx
    }

    /// FIX-F test hook — post a `Cancel` command. Mirrors the behaviour of
    /// the live cancel path (atomic flag + abort every registered lane).
    #[doc(hidden)]
    pub async fn cancel_for_test(&self, sid: &str) -> bool {
        self.post(sid, SessionCommand::Cancel).await
    }

    /// FIX-C — release the driver's first emit. Idempotent: returns true
    /// only on the first call per session.
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

    async fn post(&self, sid: &str, cmd: SessionCommand) -> bool {
        let map = self.sessions.lock().await;
        if let Some(h) = map.get(sid) {
            // Mark cancel flag eagerly so any non-channel-aware loop sees it.
            if matches!(cmd, SessionCommand::Cancel) {
                h.cancelled.store(true, Ordering::SeqCst);
                // FIX-F — abort every registered lane, not just the last
                // one stored in a single-slot Mutex<Option<...>>.
                let _ = h.active.abort_all().await;
            }
            h.cmd_tx.send(cmd).await.is_ok()
        } else {
            false
        }
    }

    async fn get_state(&self, sid: &str) -> Option<SessionState> {
        let map = self.sessions.lock().await;
        if let Some(h) = map.get(sid) {
            Some(h.state.lock().await.clone())
        } else {
            None
        }
    }
}

// ─── runtime injection (adapters + runners) ────────────────────────────────
//
// At app startup `lib.rs` constructs `OrchestrationDeps` with real adapters
// and registers it as Tauri state. Tests construct it with mock runners.

pub struct OrchestrationDeps {
    pub real_runner: Arc<dyn ProcessRunner>,
    pub mock_runner: Arc<dyn ProcessRunner>,
    pub lock_manager: LockManager,
    pub claude_config: crate::adapters::claude::ClaudeConfig,
    pub codex_config: crate::adapters::codex::CodexConfig,
    /// FIX-F — base dir for `JournalWriter`. Live: `~/.moa-desktop`. When
    /// `None` (legacy/test paths) journal is best-effort skipped — the
    /// orchestrator never panics on absent journal infrastructure.
    pub journal_base_dir: Option<PathBuf>,
}

impl OrchestrationDeps {
    fn pick_runner(&self, mock: bool) -> Arc<dyn ProcessRunner> {
        if mock {
            self.mock_runner.clone()
        } else {
            self.real_runner.clone()
        }
    }

    fn claude(&self, mock: bool) -> ClaudeAdapter {
        ClaudeAdapter::new(self.pick_runner(mock), self.claude_config.clone())
    }

    fn codex(&self, mock: bool) -> CodexAdapter {
        CodexAdapter::new(self.pick_runner(mock), self.codex_config.clone())
    }
}

// ─── driver task ──────────────────────────────────────────────────────────

struct DriverCtx {
    app: AppHandle,
    sid: String,
    cancelled: Arc<AtomicBool>,
    /// FIX-F — shared with `SessionHandle.active`. The driver registers a
    /// `ProcessControl` under a stable run-id (`<lane>-<phase>[-r{round}]`)
    /// before draining its events and unregisters on completion.
    active: Arc<CancelRegistry>,
    /// FIX-F — best-effort journal. `None` only when the deps did not
    /// configure a journal base dir (tests that don't touch durability).
    journal: Option<JournalWriter>,
    state: Arc<Mutex<SessionState>>,
    /// FIX-C — set true once the frontend has acked. Read by the cleanup
    /// task to decide whether a panic-path `session_error` should fire:
    /// if the frontend never acked, it has no record of this sid and
    /// surfacing an error would phantom a failed session into the store.
    acked: Arc<AtomicBool>,
}

impl DriverCtx {
    async fn set_state(&self, s: SessionState) {
        let kind = s.kind();
        *self.state.lock().await = s.clone();
        emit(
            &self.app,
            &self.sid,
            Phase::Preflight, // phase is overridden by per-step emits; this is a state-change ping
            None,
            "state",
            Some(serde_json::json!({ "state": s, "kind": kind })),
        );
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

async fn drive_session(
    ctx: DriverCtx,
    start: OrchestrationStart,
    deps: Arc<OrchestrationDeps>,
    mut cmd_rx: mpsc::Receiver<SessionCommand>,
    ack_rx: oneshot::Receiver<()>,
) {
    // FIX-C — wait for the frontend to confirm it has the session in its
    // store before emitting anything. The driver MUST NOT emit before the
    // ack: a `session_start` (or `session_error` on panic) leaking out
    // would land on a listener whose store has no entry, which is the
    // exact race we are fixing. On timeout / sender-dropped we exit
    // silently — the frontend never knew this sid existed, so there is
    // nothing to surface; the cleanup task will unregister.
    match tokio::time::timeout(ACK_TIMEOUT, ack_rx).await {
        Ok(Ok(())) => ctx.acked.store(true, Ordering::SeqCst),
        _ => return,
    }

    emit(
        &ctx.app,
        &ctx.sid,
        Phase::Preflight,
        Some(Lane::System),
        "session_start",
        Some(serde_json::json!({
            "task": start.task,
            "mock_mode": start.mock_mode,
            "project_id": start.project_id,
        })),
    );

    // 1. Classify ──────────────────────────────────────────────────────────
    ctx.set_state(SessionState::Classifying).await;
    emit(&ctx.app, &ctx.sid, Phase::Classify, Some(Lane::System), "phase_start", None);
    let flow = classify(&ClassifyInput {
        task: start.task.clone(),
        files: start.files.clone(),
        user_override: start.override_flow,
        research_only: false,
    });
    emit(
        &ctx.app,
        &ctx.sid,
        Phase::Classify,
        Some(Lane::System),
        "phase_end",
        Some(serde_json::json!({ "flow": flow.as_str() })),
    );

    if ctx.is_cancelled() {
        return finalize_cancelled(&ctx).await;
    }

    // 2. First-pass × 2 (Flow C/D) — Flow A/B short-circuit to mutation. ───
    let synth_input = if flow.needs_full_moa() {
        match run_first_pass_pair(&ctx, &start, &deps, flow).await {
            Ok(v) => Some(v),
            Err(e) => return finalize_failed(&ctx, e).await,
        }
    } else {
        None
    };

    if ctx.is_cancelled() {
        return finalize_cancelled(&ctx).await;
    }

    // 3. Synthesis (TS callback) ───────────────────────────────────────────
    let synthesis_json = if let Some(pair) = synth_input.as_ref() {
        ctx.set_state(SessionState::AwaitingSynthesis { flow }).await;
        emit(
            &ctx.app,
            &ctx.sid,
            Phase::Synthesis,
            Some(Lane::System),
            "phase_start",
            Some(serde_json::json!({
                "claude_lines": pair.claude_lines,
                "codex_lines": pair.codex_lines,
            })),
        );
        match await_synthesis(&ctx, &mut cmd_rx).await {
            Some(s) => {
                emit(&ctx.app, &ctx.sid, Phase::Synthesis, Some(Lane::System), "phase_end", None);
                s
            }
            None => return finalize_cancelled(&ctx).await,
        }
    } else {
        String::new()
    };

    // 4. Adversarial round(s) ──────────────────────────────────────────────
    // FIX-F — `Approved` is the only outcome that lets mutation run.
    // `Escalated` (Unknown / NeedClarification / max-rounds-exhausted-without-fail)
    // routes to `Final { ok: false }` instead of silently proceeding.
    let adv_outcome = if flow.needs_full_moa() {
        match run_adversarial_loop(&ctx, &start, &deps, flow, &synthesis_json).await {
            Ok(o) => o,
            Err(e) => return finalize_failed(&ctx, e).await,
        }
    } else {
        AdversarialResult::Approved
    };

    if let AdversarialResult::Escalated { reason, round } = adv_outcome {
        // Mutation forbidden. Surface a structured `Final` so the frontend
        // can render the escalation reason instead of a green check.
        if let Some(j) = ctx.journal.as_ref() {
            let _ = j.note(
                JournalPhase::SessionEnd,
                format!("escalated reason={reason} round={round}"),
            );
        }
        ctx.set_state(SessionState::Final { flow, ok: false }).await;
        emit(
            &ctx.app,
            &ctx.sid,
            Phase::Final,
            Some(Lane::System),
            "session_done",
            Some(serde_json::json!({
                "flow": flow.as_str(),
                "ok": false,
                "reason": reason,
                "round": round,
            })),
        );
        return;
    }

    // 5. Mutation (Flow A/B/C only) — D is read-only. ─────────────────────
    let mut session_ok = true;
    let mut session_reason: Option<String> = None;
    if flow.produces_mutation() {
        match run_mutation_phase(&ctx, &start, &deps, flow, &mut cmd_rx).await {
            Ok(MutationOutcome::Applied) => {
                let outcome = run_verify_phase(&ctx, &start, flow).await;
                // FIX-F — verify outcome is now load-bearing for `Final.ok`.
                // Pre-FIX-F the driver always emitted `ok: true`, even when
                // `cargo check` failed post-mutation.
                if !outcome.is_ok() {
                    session_ok = false;
                    session_reason = Some(format!("verify-failed: {outcome:?}"));
                }
            }
            Ok(MutationOutcome::Skipped) => {
                emit(
                    &ctx.app,
                    &ctx.sid,
                    Phase::Mutation,
                    Some(Lane::System),
                    "phase_end",
                    Some(serde_json::json!({ "skipped": true })),
                );
            }
            Err(e) => return finalize_failed(&ctx, e).await,
        }
    }

    // 6. Final ────────────────────────────────────────────────────────────
    if let Some(j) = ctx.journal.as_ref() {
        let _ = j.note(
            JournalPhase::SessionEnd,
            format!("ok={session_ok}"),
        );
    }
    ctx.set_state(SessionState::Final { flow, ok: session_ok }).await;
    let mut payload = serde_json::json!({
        "flow": flow.as_str(),
        "ok": session_ok,
    });
    if let Some(r) = session_reason {
        payload["reason"] = serde_json::Value::String(r);
    }
    emit(
        &ctx.app,
        &ctx.sid,
        Phase::Final,
        Some(Lane::System),
        "session_done",
        Some(payload),
    );
}

async fn finalize_cancelled(ctx: &DriverCtx) {
    ctx.set_state(SessionState::Cancelled).await;
    emit(
        &ctx.app,
        &ctx.sid,
        Phase::Final,
        Some(Lane::System),
        "session_cancelled",
        None,
    );
}

async fn finalize_failed(ctx: &DriverCtx, msg: String) {
    ctx.set_state(SessionState::Failed { message: msg.clone() })
        .await;
    emit(
        &ctx.app,
        &ctx.sid,
        Phase::Final,
        Some(Lane::System),
        "session_error",
        Some(serde_json::json!({ "message": msg })),
    );
}

// ─── first-pass × 2 in parallel ───────────────────────────────────────────

struct FirstPassPair {
    claude_lines: usize,
    codex_lines: usize,
}

async fn run_first_pass_pair(
    ctx: &DriverCtx,
    start: &OrchestrationStart,
    deps: &Arc<OrchestrationDeps>,
    flow: Flow,
) -> Result<FirstPassPair, String> {
    ctx.set_state(SessionState::FirstPassRunning {
        flow,
        claude_done: false,
        codex_done: false,
    })
    .await;
    emit(&ctx.app, &ctx.sid, Phase::FirstPass, Some(Lane::System), "phase_start", None);

    let claude_adapter = deps.claude(start.mock_mode);
    let codex_adapter = deps.codex(start.mock_mode);

    let req_claude = ClaudeFirstPass {
        task: start.task.clone(),
        files: start.files.clone(),
        cwd: start.cwd.clone(),
    };
    let req_codex = CodexFirstPass {
        task: start.task.clone(),
        files: start.files.clone(),
        cwd: start.cwd.clone(),
    };

    let app_a = ctx.app.clone();
    let sid_a = ctx.sid.clone();
    let cancelled_a = ctx.cancelled.clone();
    let active_a = ctx.active.clone();
    let claude_sup: LaneSupervisor<Result<usize, String>> = LaneSupervisor::spawn(async move {
        let stream = claude_adapter
            .firstpass(req_claude)
            .await
            .map_err(|e| format!("claude firstpass spawn: {e}"))?;
        track_active(&active_a, "claude-firstpass", stream.control.clone());
        let n = drain_claude_events(&app_a, &sid_a, Phase::FirstPass, Lane::Claude, stream.events, &cancelled_a)
            .await;
        untrack_active(&active_a, "claude-firstpass");
        Ok(n)
    });

    let app_b = ctx.app.clone();
    let sid_b = ctx.sid.clone();
    let cancelled_b = ctx.cancelled.clone();
    let active_b = ctx.active.clone();
    let codex_sup: LaneSupervisor<Result<usize, String>> = LaneSupervisor::spawn(async move {
        let stream = codex_adapter
            .firstpass(req_codex)
            .await
            .map_err(|e| format!("codex firstpass spawn: {e}"))?;
        // FIX-F — both lanes register independently in the CancelRegistry,
        // so a mid-stream cancel aborts both (pre-FIX-F we tracked only one
        // and let the other keep burning tokens until natural exit).
        track_active(&active_b, "codex-firstpass", stream.control.clone());
        let n = drain_codex_events(&app_b, &sid_b, Phase::FirstPass, Lane::Codex, stream.events, &cancelled_b)
            .await;
        untrack_active(&active_b, "codex-firstpass");
        Ok(n)
    });

    let (r_claude, r_codex) = tokio::join!(claude_sup.into_outcome(), codex_sup.into_outcome());

    let claude_n = lane_result(r_claude, "claude")?;
    let codex_n = lane_result(r_codex, "codex")?;

    emit(
        &ctx.app,
        &ctx.sid,
        Phase::FirstPass,
        Some(Lane::System),
        "phase_end",
        Some(serde_json::json!({
            "claude_lines": claude_n,
            "codex_lines": codex_n,
        })),
    );
    Ok(FirstPassPair {
        claude_lines: claude_n,
        codex_lines: codex_n,
    })
}

fn lane_result(r: Result<Result<usize, String>, LaneError>, lane: &str) -> Result<usize, String> {
    match r {
        Ok(Ok(n)) => Ok(n),
        Ok(Err(e)) => Err(format!("{lane} lane error: {e}")),
        Err(LaneError::Aborted) => Err(format!("{lane} lane aborted")),
        Err(LaneError::Panic(m)) => Err(format!("{lane} lane panic: {m}")),
        Err(e) => Err(format!("{lane} lane: {e}")),
    }
}

/// FIX-F — register/unregister a worker under a stable run-id so cancel
/// can abort every live lane. Replaces the single-slot
/// `Mutex<Option<ProcessControl>>` used pre-FIX-F.
fn track_active(reg: &CancelRegistry, run_id: &str, ctl: ProcessControl) {
    reg.register(run_id.to_string(), ctl);
}

fn untrack_active(reg: &CancelRegistry, run_id: &str) {
    reg.unregister(run_id);
}

/// Emit a canonical WorkerEvent on `kind="line"`. The TS-side
/// `parseWorkerNdjson` expects exactly this shape — see
/// [`crate::synthesis::WorkerEvent`].
fn emit_worker_event(
    app: &AppHandle,
    sid: &str,
    phase: Phase,
    lane: Lane,
    ev: &WorkerEvent,
) {
    if let Ok(payload) = serde_json::to_value(ev) {
        emit(app, sid, phase, Some(lane), "line", Some(payload));
    }
}

/// Emit the raw adapter envelope on `kind="worker_raw"`. The frontend
/// uses this for diagnostics only — the lane buffer that feeds synthesis
/// must come from `kind="line"`.
fn emit_worker_raw<T: Serialize>(
    app: &AppHandle,
    sid: &str,
    phase: Phase,
    lane: Lane,
    raw: &T,
) {
    if let Ok(payload) = serde_json::to_value(raw) {
        emit(app, sid, phase, Some(lane), "worker_raw", Some(payload));
    }
}

/// Pull canonical `WorkerEvent`s out of a Claude adapter event:
/// 1. `Assistant { text }` — scan the model's text output for NDJSON.
/// 2. `Other { raw }` — passthrough when `raw` already carries `event:`
///    (the mock fixture path: `MockRunner` emits canonical lines as
///    stdout, the adapter wraps them as `Other`).
fn worker_events_from_claude(ev: &ClaudeEvent) -> Vec<WorkerEvent> {
    match ev {
        ClaudeEvent::Assistant { text, .. } => extract_from_text(text),
        ClaudeEvent::Other { raw, .. } => {
            WorkerEvent::try_passthrough(raw).into_iter().collect()
        }
        _ => Vec::new(),
    }
}

/// Pull canonical `WorkerEvent`s out of a Codex adapter event:
/// 1. `ItemCompleted` with `item.text` (Codex `--json` `agent_message`
///    items carry the model body in `item.text`).
/// 2. `Other { raw }` — passthrough for mock fixture lines wrapped as
///    `Other`.
fn worker_events_from_codex(ev: &CodexEvent) -> Vec<WorkerEvent> {
    match ev {
        // Only `agent_message` items carry the model's text body. Tool /
        // reasoning items can also have JSON-looking payloads in
        // `item.text` that we must not mistake for canonical claims.
        CodexEvent::ItemCompleted { item_type, raw, .. } => {
            if item_type.as_deref() != Some("agent_message") {
                return Vec::new();
            }
            let text = raw
                .get("item")
                .and_then(|i| i.get("text"))
                .and_then(|t| t.as_str())
                .unwrap_or("");
            if text.is_empty() {
                Vec::new()
            } else {
                extract_from_text(text)
            }
        }
        CodexEvent::Other { raw, .. } => {
            WorkerEvent::try_passthrough(raw).into_iter().collect()
        }
        _ => Vec::new(),
    }
}

async fn drain_claude_events(
    app: &AppHandle,
    sid: &str,
    phase: Phase,
    lane: Lane,
    mut rx: mpsc::Receiver<ClaudeEvent>,
    cancelled: &AtomicBool,
) -> usize {
    let mut n = 0usize;
    while let Some(ev) = rx.recv().await {
        if cancelled.load(Ordering::SeqCst) {
            break;
        }
        n += 1;
        for we in worker_events_from_claude(&ev) {
            emit_worker_event(app, sid, phase, lane, &we);
        }
        emit_worker_raw(app, sid, phase, lane, &ev);
        if matches!(ev, ClaudeEvent::Exit { .. }) {
            break;
        }
    }
    n
}

async fn drain_codex_events(
    app: &AppHandle,
    sid: &str,
    phase: Phase,
    lane: Lane,
    mut rx: mpsc::Receiver<CodexEvent>,
    cancelled: &AtomicBool,
) -> usize {
    let mut n = 0usize;
    while let Some(ev) = rx.recv().await {
        if cancelled.load(Ordering::SeqCst) {
            break;
        }
        n += 1;
        for we in worker_events_from_codex(&ev) {
            emit_worker_event(app, sid, phase, lane, &we);
        }
        emit_worker_raw(app, sid, phase, lane, &ev);
        if matches!(ev, CodexEvent::Exit { .. }) {
            break;
        }
    }
    n
}

// ─── synthesis (frontend callback) ────────────────────────────────────────

async fn await_synthesis(
    ctx: &DriverCtx,
    cmd_rx: &mut mpsc::Receiver<SessionCommand>,
) -> Option<String> {
    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            SessionCommand::SubmitSynthesis { synthesis_json } => return Some(synthesis_json),
            SessionCommand::Cancel => {
                ctx.cancelled.store(true, Ordering::SeqCst);
                return None;
            }
            SessionCommand::ConfirmMutation { .. } => {
                // Out of order — ignore until we're at mutation step.
                continue;
            }
        }
    }
    None
}

// ─── adversarial round(s) ─────────────────────────────────────────────────

/// FIX-F — outcome of the adversarial loop seen by `drive_session`.
/// Pre-FIX-F the function returned `Result<(), String>` and treated
/// `Pass | Unknown` as "approved" — Unknown is *not* approval, the
/// reviewer just failed to emit a parseable verdict header. We now split
/// "approved" from "escalated, mutation forbidden" so the driver can
/// short-circuit cleanly and emit `Final { ok: false }` instead of
/// silently letting an unverified synthesis touch files.
enum AdversarialResult {
    Approved,
    Escalated { reason: &'static str, round: u32 },
}

async fn run_adversarial_loop(
    ctx: &DriverCtx,
    start: &OrchestrationStart,
    deps: &Arc<OrchestrationDeps>,
    flow: Flow,
    synthesis_json: &str,
) -> Result<AdversarialResult, String> {
    let mut round = 1u32;
    loop {
        if ctx.is_cancelled() {
            return Err("cancelled".into());
        }
        ctx.set_state(SessionState::AwaitingAdversarial { flow, round })
            .await;
        emit(
            &ctx.app,
            &ctx.sid,
            Phase::Adversarial,
            Some(Lane::System),
            "phase_start",
            Some(serde_json::json!({ "round": round })),
        );

        // Default reviewer: Codex (Claude is user-facing session holder).
        let reviewer = adversarial::default_reviewer(Lane::Claude);
        let prompt_body = adversarial::render_prompt(
            &start.task,
            synthesis_json,
            round,
            MAX_ADVERSARIAL_ROUNDS,
        );

        // Adversarial uses firstpass argv (read-only) but with the embedded
        // synthesis prompt body — we feed the body as the "task" so the
        // template substitutes it at the prompt position.
        let verdict = match reviewer {
            Lane::Codex => {
                let codex = deps.codex(start.mock_mode);
                let req = CodexFirstPass {
                    task: prompt_body.clone(),
                    files: start.files.clone(),
                    cwd: start.cwd.clone(),
                };
                let stream = codex
                    .firstpass(req)
                    .await
                    .map_err(|e| format!("codex adversarial spawn: {e}"))?;
                let run_id = format!("codex-adv-r{round}");
                track_active(&ctx.active, &run_id, stream.control.clone());
                let mut text = String::new();
                let mut rx = stream.events;
                while let Some(ev) = rx.recv().await {
                    if ctx.is_cancelled() {
                        break;
                    }
                    if let CodexEvent::Other { ref raw, .. } = ev {
                        if let Some(s) = raw.get("text").and_then(|v| v.as_str()) {
                            text.push_str(s);
                            text.push('\n');
                        }
                    }
                    for we in worker_events_from_codex(&ev) {
                        emit_worker_event(
                            &ctx.app,
                            &ctx.sid,
                            Phase::Adversarial,
                            Lane::Codex,
                            &we,
                        );
                    }
                    emit_worker_raw(
                        &ctx.app,
                        &ctx.sid,
                        Phase::Adversarial,
                        Lane::Codex,
                        &ev,
                    );
                    if matches!(ev, CodexEvent::Exit { .. }) {
                        break;
                    }
                }
                untrack_active(&ctx.active, &run_id);
                adversarial::parse_verdict(&text)
            }
            _ => {
                let claude = deps.claude(start.mock_mode);
                let req = ClaudeFirstPass {
                    task: prompt_body.clone(),
                    files: start.files.clone(),
                    cwd: start.cwd.clone(),
                };
                let stream = claude
                    .firstpass(req)
                    .await
                    .map_err(|e| format!("claude adversarial spawn: {e}"))?;
                let run_id = format!("claude-adv-r{round}");
                track_active(&ctx.active, &run_id, stream.control.clone());
                let mut text = String::new();
                let mut rx = stream.events;
                while let Some(ev) = rx.recv().await {
                    if ctx.is_cancelled() {
                        break;
                    }
                    if let ClaudeEvent::Assistant { text: ref t, .. } = ev {
                        text.push_str(t);
                        text.push('\n');
                    }
                    for we in worker_events_from_claude(&ev) {
                        emit_worker_event(
                            &ctx.app,
                            &ctx.sid,
                            Phase::Adversarial,
                            Lane::Claude,
                            &we,
                        );
                    }
                    emit_worker_raw(
                        &ctx.app,
                        &ctx.sid,
                        Phase::Adversarial,
                        Lane::Claude,
                        &ev,
                    );
                    if matches!(ev, ClaudeEvent::Exit { .. }) {
                        break;
                    }
                }
                untrack_active(&ctx.active, &run_id);
                adversarial::parse_verdict(&text)
            }
        };

        emit(
            &ctx.app,
            &ctx.sid,
            Phase::Adversarial,
            Some(reviewer),
            "phase_end",
            Some(serde_json::json!({ "round": round, "verdict": format!("{verdict:?}") })),
        );

        // FIX-F — pure decision (testable in `adversarial::tests`); the
        // I/O loop only emits + transitions based on its result.
        match adversarial::decide(verdict, round, MAX_ADVERSARIAL_ROUNDS) {
            adversarial::AdversarialDecision::Approved => {
                return Ok(AdversarialResult::Approved);
            }
            adversarial::AdversarialDecision::EscalateNoMutation { reason, round: r } => {
                emit(
                    &ctx.app,
                    &ctx.sid,
                    Phase::Adversarial,
                    Some(Lane::System),
                    "escalation",
                    Some(serde_json::json!({ "reason": reason, "round": r })),
                );
                return Ok(AdversarialResult::Escalated { reason, round: r });
            }
            adversarial::AdversarialDecision::EscalateBlocker { round: r } => {
                emit(
                    &ctx.app,
                    &ctx.sid,
                    Phase::Adversarial,
                    Some(Lane::System),
                    "escalation",
                    Some(serde_json::json!({
                        "reason": "max-rounds-exceeded",
                        "round": r,
                        "max": MAX_ADVERSARIAL_ROUNDS,
                    })),
                );
                return Err(format!("adversarial blocker after {r} rounds"));
            }
            adversarial::AdversarialDecision::Retry { next_round } => {
                round = next_round;
            }
        }
    }
}

// ─── mutation phase ───────────────────────────────────────────────────────

enum MutationOutcome {
    Applied,
    Skipped,
}

async fn run_mutation_phase(
    ctx: &DriverCtx,
    start: &OrchestrationStart,
    deps: &Arc<OrchestrationDeps>,
    flow: Flow,
    cmd_rx: &mut mpsc::Receiver<SessionCommand>,
) -> Result<MutationOutcome, String> {
    // Mutation owner — Flow A: Claude, Flow B: Codex, Flow C: Claude default.
    let owner = match flow {
        Flow::B => Lane::Codex,
        _ => Lane::Claude,
    };
    let owner_worker = owner.to_worker().ok_or_else(|| "system lane cannot mutate".to_string())?;

    ctx.set_state(SessionState::AwaitingMutationConfirm {
        flow,
        round: 1,
        mutation_owner: owner,
    })
    .await;
    emit(
        &ctx.app,
        &ctx.sid,
        Phase::Mutation,
        Some(owner),
        "awaiting_confirm",
        Some(serde_json::json!({ "owner": owner.as_str() })),
    );

    // Wait for user confirm via cmd_rx.
    let proceed = loop {
        match cmd_rx.recv().await {
            Some(SessionCommand::ConfirmMutation { proceed }) => break proceed,
            Some(SessionCommand::Cancel) => {
                ctx.cancelled.store(true, Ordering::SeqCst);
                return Err("cancelled".into());
            }
            Some(_) => continue,
            None => return Err("command channel closed".into()),
        }
    };
    if !proceed {
        return Ok(MutationOutcome::Skipped);
    }

    ctx.set_state(SessionState::Mutating { flow, owner }).await;
    emit(&ctx.app, &ctx.sid, Phase::Mutation, Some(owner), "phase_start", None);

    // T4 lock acquire — Repo → Project → Lane chain.
    let repo_key = crate::git::canonical::RepoKey::from_path(&start.cwd);
    let repo_guard = deps
        .lock_manager
        .acquire_repo(&repo_key)
        .map_err(|e| format!("acquire repo lock: {e}"))?;
    let project_guard = deps
        .lock_manager
        .acquire_project(&repo_guard, &start.project_id)
        .map_err(|e| format!("acquire project lock: {e}"))?;
    let lock_key = format!("session:{}", &ctx.sid);
    let _lane_guard = deps
        .lock_manager
        .acquire_lane(&project_guard, &lock_key, owner_worker, LockSource::Scheduler)
        .map_err(|e| format!("acquire lane lock: {e}"))?;

    // FIX-F — durability hook: the lane lock is now ours. Journal the
    // acquisition so a crash recovery can see "we held a lane on session X
    // when we died" before any FS-touching work begins.
    if let Some(j) = ctx.journal.as_ref() {
        let _ = j.append(Entry {
            seq: 0,
            ts_ms: 0,
            phase: JournalPhase::LockAcquired,
            owner: Some(owner_worker),
            pid: 0,
            base_hashes: Default::default(),
            patch_path: None,
            note: Some(format!("project={} lock_key={}", &start.project_id, &lock_key)),
        });
    }

    // Worktree creation: under <repo>/.moa-desktop/worktrees/<sid>
    let wt_root = start.cwd.join(".moa-desktop").join("worktrees").join(&ctx.sid);
    let branch = Some(format!("orch/{}", &ctx.sid));
    let wt = crate::git::Worktree::add(&start.cwd, &wt_root, branch.as_deref())
        .map_err(|e| format!("worktree add: {e}"))?;

    if let Some(j) = ctx.journal.as_ref() {
        let _ = j.note(
            JournalPhase::WorktreeCreated,
            wt.path.display().to_string(),
        );
    }

    emit(
        &ctx.app,
        &ctx.sid,
        Phase::Mutation,
        Some(owner),
        "worktree_created",
        Some(serde_json::json!({ "path": wt.path.display().to_string() })),
    );

    // Spawn mutation worker.
    let worker_result = match owner {
        Lane::Claude => {
            let adapter = deps.claude(start.mock_mode);
            let req = crate::adapters::claude::MutationRequest {
                task: start.task.clone(),
                worktree_path: wt.path.clone(),
            };
            let stream = adapter
                .mutation(req)
                .await
                .map_err(|e| format!("claude mutation spawn: {e}"))?;
            track_active(&ctx.active, "mutation", stream.control.clone());
            if let Some(j) = ctx.journal.as_ref() {
                let _ = j.note(JournalPhase::WorkerStarted, "claude-mutation");
            }
            let n = drain_claude_events(
                &ctx.app,
                &ctx.sid,
                Phase::Mutation,
                Lane::Claude,
                stream.events,
                &ctx.cancelled,
            )
            .await;
            untrack_active(&ctx.active, "mutation");
            n
        }
        Lane::Codex => {
            let adapter = deps.codex(start.mock_mode);
            let req = crate::adapters::codex::MutationRequest {
                task: start.task.clone(),
                worktree_path: wt.path.clone(),
            };
            let stream = adapter
                .mutation(req)
                .await
                .map_err(|e| format!("codex mutation spawn: {e}"))?;
            track_active(&ctx.active, "mutation", stream.control.clone());
            if let Some(j) = ctx.journal.as_ref() {
                let _ = j.note(JournalPhase::WorkerStarted, "codex-mutation");
            }
            let n = drain_codex_events(
                &ctx.app,
                &ctx.sid,
                Phase::Mutation,
                Lane::Codex,
                stream.events,
                &ctx.cancelled,
            )
            .await;
            untrack_active(&ctx.active, "mutation");
            n
        }
        Lane::System => return Err("system lane cannot mutate".into()),
    };

    if let Some(j) = ctx.journal.as_ref() {
        let _ = j.note(
            JournalPhase::WorkerFinished,
            format!("events={worker_result}"),
        );
    }

    emit(
        &ctx.app,
        &ctx.sid,
        Phase::Mutation,
        Some(owner),
        "worker_finished",
        Some(serde_json::json!({ "events": worker_result })),
    );

    if ctx.is_cancelled() {
        let _ = wt.remove();
        return Err("cancelled".into());
    }

    // Extract patch + check + apply.
    let patch_dir = start.cwd.join(".moa-desktop").join("patches").join(&ctx.sid);
    let patch = crate::git::patch::extract(&wt, &patch_dir, "mutation")
        .map_err(|e| format!("patch extract: {e}"))?;
    if let Some(j) = ctx.journal.as_ref() {
        let _ = j.append(Entry {
            seq: 0,
            ts_ms: 0,
            phase: JournalPhase::PatchExtracted,
            owner: Some(owner_worker),
            pid: 0,
            base_hashes: Default::default(),
            patch_path: Some(patch.path.display().to_string()),
            note: Some(format!("size={} empty={}", patch.text.len(), patch.is_empty())),
        });
    }
    emit(
        &ctx.app,
        &ctx.sid,
        Phase::Mutation,
        Some(Lane::System),
        "patch_extracted",
        Some(serde_json::json!({
            "path": patch.path.display().to_string(),
            "empty": patch.is_empty(),
            "size": patch.text.len(),
        })),
    );

    if patch.is_empty() {
        let _ = wt.remove();
        emit(
            &ctx.app,
            &ctx.sid,
            Phase::Mutation,
            Some(Lane::System),
            "phase_end",
            Some(serde_json::json!({ "applied": false, "reason": "empty-patch" })),
        );
        return Ok(MutationOutcome::Skipped);
    }

    crate::git::patch::check(&start.cwd, &patch).map_err(|e| {
        if let Some(j) = ctx.journal.as_ref() {
            let _ = j.note(JournalPhase::PatchRejected, format!("check failed: {e}"));
        }
        format!("patch check: {e}")
    })?;
    if let Some(j) = ctx.journal.as_ref() {
        let _ = j.note(JournalPhase::PatchVerified, "");
    }
    crate::git::patch::apply(&start.cwd, &patch).map_err(|e| format!("patch apply: {e}"))?;
    if let Some(j) = ctx.journal.as_ref() {
        let _ = j.append(Entry {
            seq: 0,
            ts_ms: 0,
            phase: JournalPhase::PatchApplied,
            owner: Some(owner_worker),
            pid: 0,
            base_hashes: Default::default(),
            patch_path: Some(patch.path.display().to_string()),
            note: None,
        });
    }
    let _ = wt.remove();
    if let Some(j) = ctx.journal.as_ref() {
        let _ = j.note(JournalPhase::WorktreeRemoved, "");
    }

    emit(
        &ctx.app,
        &ctx.sid,
        Phase::Mutation,
        Some(Lane::System),
        "phase_end",
        Some(serde_json::json!({ "applied": true })),
    );
    Ok(MutationOutcome::Applied)
}

// ─── verify phase ─────────────────────────────────────────────────────────

async fn run_verify_phase(
    ctx: &DriverCtx,
    start: &OrchestrationStart,
    flow: Flow,
) -> verify::VerifyOutcome {
    ctx.set_state(SessionState::Verifying { flow }).await;
    emit(&ctx.app, &ctx.sid, Phase::Verify, Some(Lane::System), "phase_start", None);

    let mut spec = verify::VerifySpec::new(&start.cwd);
    if let Some(c) = start.verify_cmd.as_ref() {
        spec = spec.with_command(c.clone());
    }
    let outcome = verify::run(spec).await;
    emit(
        &ctx.app,
        &ctx.sid,
        Phase::Verify,
        Some(Lane::System),
        "phase_end",
        Some(serde_json::json!({ "outcome": outcome })),
    );
    outcome
}

// ─── Tauri commands ───────────────────────────────────────────────────────

#[tauri::command]
pub async fn orch_start(
    app: AppHandle,
    coord: State<'_, OrchestrationCoordinator>,
    deps: State<'_, Arc<OrchestrationDeps>>,
    start: OrchestrationStart,
) -> Result<String, String> {
    let sid = OrchestrationCoordinator::new_session_id();
    let cancelled = Arc::new(AtomicBool::new(false));
    let active = Arc::new(CancelRegistry::new());
    let state = Arc::new(Mutex::new(SessionState::Idle));
    let (tx, rx) = mpsc::channel::<SessionCommand>(32);
    let (ack_tx, ack_rx) = oneshot::channel::<()>();

    // FIX-F — open the per-session journal up-front. Best-effort: if the
    // base dir is unset (test deps) or open() fails (FS issue), we
    // continue without journal rather than fail orch_start.
    let journal = deps.journal_base_dir.as_ref().and_then(|base| {
        match JournalWriter::open(base, &start.project_id, &sid) {
            Ok(w) => {
                let _ = w.note(JournalPhase::SessionStart, format!("task={}", start.task));
                Some(w)
            }
            Err(_) => None,
        }
    });

    coord
        .register(
            &sid,
            SessionHandle {
                cancelled: cancelled.clone(),
                active: active.clone(),
                cmd_tx: tx,
                state: state.clone(),
                ack_tx: Mutex::new(Some(ack_tx)),
            },
        )
        .await;

    let acked = Arc::new(AtomicBool::new(false));
    let ctx = DriverCtx {
        app: app.clone(),
        sid: sid.clone(),
        cancelled,
        active,
        journal,
        state,
        acked: acked.clone(),
    };
    let deps = deps.inner().clone();
    let sid_for_cleanup = sid.clone();
    let app_for_cleanup = app.clone();
    let acked_for_cleanup = acked;

    // Wrap the driver in a LaneSupervisor so a panic in the driver does not
    // tear down the runtime — failures bubble up as a lane error and we emit
    // an `orch://event session_error` instead.
    let driver_sup: LaneSupervisor<()> = LaneSupervisor::spawn(async move {
        drive_session(ctx, start, deps, rx, ack_rx).await;
    });

    tokio::spawn(async move {
        let outcome = driver_sup.into_outcome().await;
        if let Err(e) = outcome {
            // Driver panicked — emit a structured error so UI can surface
            // it, but only if the frontend has acked. Without an ack the
            // listener has no record of this sid and a `session_error`
            // would phantom a failed entry into the store (FIX-C).
            if acked_for_cleanup.load(Ordering::SeqCst) {
                let env = EventEnvelope {
                    session_id: &sid_for_cleanup,
                    phase: Phase::Final.as_str(),
                    lane: Some(Lane::System.as_str()),
                    kind: "session_error",
                    payload: Some(serde_json::json!({ "message": format!("driver panic: {e}") })),
                };
                let _ = app_for_cleanup.emit(EVENT_NAME, env);
            }
        }
        if let Some(coord) = app_for_cleanup.try_state::<OrchestrationCoordinator>() {
            coord.unregister(&sid_for_cleanup).await;
        }
    });

    Ok(sid)
}

/// FIX-C — frontend acknowledges that the session shell is in the store.
/// Until this fires the driver task is parked before its first emit, which
/// guarantees `session_start` (and every subsequent event) lands on a
/// session the frontend already has.
#[tauri::command]
pub async fn orch_ack(
    coord: State<'_, OrchestrationCoordinator>,
    session_id: String,
) -> Result<bool, String> {
    Ok(coord.ack(&session_id).await)
}

#[tauri::command]
pub async fn orch_cancel(
    coord: State<'_, OrchestrationCoordinator>,
    session_id: String,
) -> Result<bool, String> {
    Ok(coord.post(&session_id, SessionCommand::Cancel).await)
}

#[tauri::command]
pub async fn orch_submit_synthesis(
    coord: State<'_, OrchestrationCoordinator>,
    session_id: String,
    synthesis_json: String,
) -> Result<bool, String> {
    Ok(coord
        .post(&session_id, SessionCommand::SubmitSynthesis { synthesis_json })
        .await)
}

#[tauri::command]
pub async fn orch_confirm_mutation(
    coord: State<'_, OrchestrationCoordinator>,
    session_id: String,
    proceed: bool,
) -> Result<bool, String> {
    Ok(coord
        .post(&session_id, SessionCommand::ConfirmMutation { proceed })
        .await)
}

#[tauri::command]
pub async fn orch_get_state(
    coord: State<'_, OrchestrationCoordinator>,
    session_id: String,
) -> Result<Option<SessionState>, String> {
    Ok(coord.get_state(&session_id).await)
}

// `tauri::AppHandle::try_state` requires this trait import in scope.
use tauri::Manager;

#[cfg(test)]
mod fix_d_tests {
    //! FIX-D regression: the orchestrator must canonicalise worker output
    //! before it leaves the Rust side. These tests exercise the pure
    //! extraction helpers — the live Tauri `emit` path needs an `AppHandle`
    //! and is exercised via dryrun integration tests.

    use super::{worker_events_from_claude, worker_events_from_codex};
    use crate::adapters::claude::ClaudeEvent;
    use crate::adapters::codex::CodexEvent;
    use crate::synthesis::WorkerEvent;
    use serde_json::json;

    #[test]
    fn claude_assistant_text_with_canonical_ndjson_yields_worker_events() {
        let raw = json!({"type":"assistant","message":{"content":[]}});
        let text = "preamble\n\
            {\"event\":\"start\",\"worker\":\"claude\",\"phase\":\"firstpass\"}\n\
            {\"event\":\"claim\",\"id\":\"c1\",\"text\":\"x\",\"confidence\":\"high\"}\n\
            {\"event\":\"end\",\"status\":\"ok\"}";
        let ev = ClaudeEvent::Assistant {
            text: text.into(),
            raw,
        };
        let out = worker_events_from_claude(&ev);
        assert_eq!(out.len(), 3);
        assert!(matches!(out[0], WorkerEvent::Start { .. }));
        assert!(matches!(out[1], WorkerEvent::Claim { .. }));
        assert!(matches!(out[2], WorkerEvent::End { .. }));
    }

    #[test]
    fn claude_other_with_canonical_event_passthroughs() {
        // The mock-fixture path: MockRunner emits canonical NDJSON as
        // stdout, the adapter wraps each line as `Other { type_:"", raw }`.
        let raw = json!({
            "event":"claim",
            "id":"c1",
            "text":"y",
            "confidence":"med"
        });
        let ev = ClaudeEvent::Other {
            type_: String::new(),
            raw,
        };
        let out = worker_events_from_claude(&ev);
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], WorkerEvent::Claim { .. }));
    }

    #[test]
    fn claude_system_init_yields_no_worker_events() {
        let ev = ClaudeEvent::SystemInit {
            session_id: Some("s".into()),
            raw: json!({"type":"system","subtype":"init"}),
        };
        assert!(worker_events_from_claude(&ev).is_empty());
    }

    #[test]
    fn codex_item_completed_with_agent_text_extracts_canonical() {
        // Codex `--json` `agent_message` carries the model body in
        // `item.text`. Real CLI shape, observed via `codex exec --json`.
        let raw = json!({
            "type":"item.completed",
            "item":{
                "type":"agent_message",
                "text":"prose then\n{\"event\":\"claim\",\"id\":\"c1\",\"text\":\"k\"}"
            }
        });
        let ev = CodexEvent::ItemCompleted {
            item_type: Some("agent_message".into()),
            error_message: None,
            raw,
        };
        let out = worker_events_from_codex(&ev);
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], WorkerEvent::Claim { .. }));
    }

    #[test]
    fn codex_non_agent_message_item_does_not_extract() {
        // Tool / reasoning items can carry JSON-looking payloads in
        // `item.text`; the drain must not mistake them for canonical claims.
        let raw = json!({
            "type":"item.completed",
            "item":{
                "type":"command_execution",
                "text":"{\"event\":\"claim\",\"id\":\"x\",\"text\":\"masquerade\"}"
            }
        });
        let ev = CodexEvent::ItemCompleted {
            item_type: Some("command_execution".into()),
            error_message: None,
            raw,
        };
        assert!(worker_events_from_codex(&ev).is_empty());
    }

    #[test]
    fn codex_other_with_canonical_event_passthroughs() {
        let raw = json!({
            "event":"open_question",
            "id":"q1",
            "text":"why"
        });
        let ev = CodexEvent::Other {
            type_: String::new(),
            raw,
        };
        let out = worker_events_from_codex(&ev);
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], WorkerEvent::OpenQuestion { .. }));
    }
}
