// T7-full — frontend orchestrator state machine.
//
// Subscribes to `orch://event` (Rust-side T7-full pipeline). Mirrors the
// Rust SessionState. Owns the TS-side T3 synthesis step: when the Rust
// driver enters `synthesis/phase_start`, this file invokes
// `synthesize(...)` (existing TS T3) and posts the JSON back via
// `orch_submit_synthesis`.
//
// Designed to coexist with `dryRun.ts` — different event channel, separate
// store, no shared mutable state.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { synthesize } from "../synthesis";
import { parseWorkerNdjson, type WorkerName, type WorkerOutput } from "../synthesis/types";

export type Phase =
  | "preflight"
  | "classify"
  | "first-pass"
  | "synthesis"
  | "adversarial"
  | "mutation"
  | "verify"
  | "final";

export type Lane = "system" | "claude" | "codex";

export type Flow = "a" | "b" | "c" | "d";

export type EventKind =
  | "session_start"
  | "session_done"
  | "session_cancelled"
  | "session_error"
  | "phase_start"
  | "phase_end"
  | "line"
  | "worker_raw"
  | "state"
  | "escalation"
  | "safety_violation"
  | "awaiting_confirm"
  | "worktree_created"
  | "worker_finished"
  | "patch_extracted";

export interface OrchEvent {
  session_id: string;
  phase: Phase;
  lane?: Lane;
  kind: EventKind;
  payload?: unknown;
}

export interface OrchSessionState {
  kind: string;
  flow?: Flow;
  round?: number;
  mutation_owner?: Lane;
  ok?: boolean;
  message?: string;
}

export interface OrchSession {
  sessionId: string;
  task: string;
  flow?: Flow;
  state: OrchSessionState;
  /// Raw lines per lane — accumulated for the TS-side synthesizer.
  lanes: Record<Lane, string[]>;
  /// Most recent verdict from adversarial round.
  lastVerdict?: string;
  /// Final report payload, if present.
  finalPayload?: unknown;
  /// User-facing log strings for the workbench transcript.
  log: string[];
}

interface Store {
  sessions: Record<string, OrchSession>;
  active?: string;
}

type Listener = () => void;

const store: Store = { sessions: {} };
const listeners = new Set<Listener>();
let unlisten: UnlistenFn | null = null;
// FIX-C — concurrent first callers used to each invoke `listen()` (the
// existing `if (unlisten) return` only guards after the await resolved).
// Memoize the in-flight subscription so all callers share a single bus.
let subscribePromise: Promise<void> | null = null;

function notify(): void {
  for (const l of listeners) l();
}

function freshSession(sid: string, task: string): OrchSession {
  return {
    sessionId: sid,
    task,
    state: { kind: "idle" },
    lanes: { system: [], claude: [], codex: [] },
    log: [],
  };
}

function ensureSession(sid: string, task = ""): OrchSession {
  let s = store.sessions[sid];
  if (!s) {
    s = freshSession(sid, task);
    store.sessions[sid] = s;
  }
  return s;
}

async function ensureSubscribed(): Promise<void> {
  if (unlisten) return;
  if (subscribePromise) return subscribePromise;
  subscribePromise = (async () => {
    unlisten = await listen<OrchEvent>("orch://event", (e) => {
      void onEvent(e.payload);
    });
  })();
  return subscribePromise;
}

async function onEvent(ev: OrchEvent): Promise<void> {
  const s = ensureSession(ev.session_id);

  switch (ev.kind) {
    case "session_start": {
      const payload = (ev.payload ?? {}) as { task?: string };
      if (payload.task) s.task = payload.task;
      store.active = ev.session_id;
      s.log.push(`[${ev.session_id}] start: ${s.task}`);
      break;
    }
    case "state": {
      const payload = (ev.payload ?? {}) as { state?: OrchSessionState; kind?: string };
      if (payload.state) s.state = payload.state;
      if (payload.state?.flow) s.flow = payload.state.flow;
      break;
    }
    case "phase_start": {
      s.log.push(`→ ${ev.phase}${ev.lane ? ` (${ev.lane})` : ""}`);
      // T3 synthesis hook — when Rust enters synthesis phase_start, call TS T3
      // and submit the result back.
      if (ev.phase === "synthesis") {
        await runSynthesisAndSubmit(s);
      }
      break;
    }
    case "phase_end": {
      s.log.push(`✓ ${ev.phase}${ev.lane ? ` (${ev.lane})` : ""}`);
      break;
    }
    case "line": {
      const lane = (ev.lane ?? "system") as Lane;
      // FIX-D: only canonical WorkerEvent payloads (see
      // `src/lib/synthesis/types.ts` — `event: start|claim|open_question|end`)
      // feed the lane buffer. The Rust orchestrator emits raw adapter
      // envelopes under `kind="worker_raw"` instead. Pre-FIX-D this branch
      // accepted anything stringifiable, which silently buried Serde-tagged
      // adapter envelopes (`{kind:"assistant",...}`) in the buffer; the
      // synthesis parser then dropped them all and returned an empty result.
      if (typeof ev.payload === "string") {
        // Defensive: dryrun and legacy mock paths emit a raw NDJSON string.
        // Trust the producer to keep the canonical shape.
        s.lanes[lane].push(ev.payload);
      } else if (isCanonicalWorkerEventPayload(ev.payload)) {
        s.lanes[lane].push(JSON.stringify(ev.payload));
      }
      break;
    }
    case "escalation": {
      const reason = ((ev.payload ?? {}) as { reason?: string }).reason ?? "unknown";
      s.log.push(`! escalation: ${reason}`);
      break;
    }
    case "safety_violation": {
      const payload = (ev.payload ?? {}) as { evidence?: string; violation_kind?: string };
      const reason = payload.evidence ?? payload.violation_kind ?? "policy violation";
      s.log.push(`! safety violation: ${reason}`);
      s.state = { kind: "failed", message: `safety violation: ${reason}` };
      break;
    }
    case "awaiting_confirm": {
      s.log.push(`? awaiting mutation confirm`);
      break;
    }
    case "session_done": {
      s.log.push(`✔ session done`);
      s.finalPayload = ev.payload;
      break;
    }
    case "session_error": {
      const msg = ((ev.payload ?? {}) as { message?: string }).message ?? "unknown error";
      s.log.push(`✗ ${msg}`);
      s.state = { kind: "failed", message: msg };
      break;
    }
    case "session_cancelled": {
      s.log.push(`◼ cancelled`);
      s.state = { kind: "cancelled" };
      break;
    }
    case "worktree_created":
    case "worker_finished":
    case "patch_extracted":
      s.log.push(`· ${ev.kind}`);
      break;
  }
  notify();
}

/// TS-side T3 synthesis. Called when Rust driver enters `synthesis/phase_start`.
/// Reads accumulated worker output from the session, runs `synthesize()`, and
/// posts the result back via `orch_submit_synthesis`.
async function runSynthesisAndSubmit(s: OrchSession): Promise<void> {
  // Parse accumulated NDJSON-ish lines per lane into WorkerOutput.
  const claude = parseWorkerLanes(s.lanes.claude, "claude");
  const codex = parseWorkerLanes(s.lanes.codex, "codex");
  const data = synthesize(claude, codex);
  const json = JSON.stringify(data);
  try {
    await invoke("orch_submit_synthesis", {
      sessionId: s.sessionId,
      synthesisJson: json,
    });
  } catch (err) {
    s.log.push(`✗ submit_synthesis: ${String(err)}`);
  }
}

function parseWorkerLanes(lines: string[], who: WorkerName): WorkerOutput {
  // Concatenate as ND-JSON so the existing parser does the work. Best-effort
  // — invalid lines are skipped by `parseWorkerNdjson`.
  return parseWorkerNdjson(lines.join("\n"), who);
}

/// Canonical `WorkerEvent` discriminator — must match
/// `src/lib/synthesis/types.ts` and `src-tauri/src/synthesis/mod.rs`.
const CANONICAL_WORKER_EVENTS = ["start", "claim", "open_question", "end"] as const;

/// Returns true when the payload is shaped like a `WorkerEvent` (top-level
/// `event` discriminator with one of the canonical values). Used to gate
/// the lane buffer so raw adapter envelopes (`kind="assistant"`, etc.) do
/// not poison the synthesis input.
export function isCanonicalWorkerEventPayload(payload: unknown): boolean {
  if (!payload || typeof payload !== "object") return false;
  const tag = (payload as { event?: unknown }).event;
  if (typeof tag !== "string") return false;
  return (CANONICAL_WORKER_EVENTS as readonly string[]).includes(tag);
}

// ─── public API ────────────────────────────────────────────────────────────

export interface OrchStartArgs {
  task: string;
  files?: string[];
  cwd: string;
  projectId: string;
  overrideFlow?: Flow;
  mockMode?: boolean;
  verifyCmd?: string;
  primaryRole?: "claude" | "codex";
}

export const orchStore = {
  subscribe(l: Listener): () => void {
    listeners.add(l);
    return () => listeners.delete(l);
  },
  getSnapshot(): Store {
    return store;
  },
  getActive(): OrchSession | undefined {
    return store.active ? store.sessions[store.active] : undefined;
  },
  get(sid: string): OrchSession | undefined {
    return store.sessions[sid];
  },
};

export async function orchStart(args: OrchStartArgs): Promise<string> {
  await ensureSubscribed();
  const primaryRole = args.primaryRole ?? readPrimaryRoleSetting();
  const sid = await invoke<string>("orch_start", {
    start: {
      task: args.task,
      files: args.files ?? [],
      cwd: args.cwd,
      project_id: args.projectId,
      override_flow: args.overrideFlow,
      mock_mode: args.mockMode ?? false,
      verify_cmd: args.verifyCmd,
      primary_role: primaryRole,
    },
  });
  // FIX-C — register the session in our store BEFORE telling the backend
  // it is safe to emit. The Rust driver parks on a oneshot until the ack
  // below fires, which guarantees `session_start` lands on a session shell
  // we already have. `onEvent` still self-heals via `ensureSession` if any
  // event slips through out of order (e.g. backend timeout fallback).
  ensureSession(sid, args.task);
  store.active = sid;
  notify();
  await invoke("orch_ack", { sessionId: sid });
  return sid;
}

function readPrimaryRoleSetting(): "claude" | "codex" {
  if (typeof window === "undefined") return "claude";
  try {
    const raw = window.localStorage.getItem("moa.settings");
    if (!raw) return "claude";
    const parsed = JSON.parse(raw) as { primaryRole?: unknown };
    return parsed.primaryRole === "codex" ? "codex" : "claude";
  } catch {
    return "claude";
  }
}

export async function orchCancel(sessionId: string): Promise<boolean> {
  return invoke<boolean>("orch_cancel", { sessionId });
}

export async function orchConfirmMutation(
  sessionId: string,
  proceed: boolean,
): Promise<boolean> {
  return invoke<boolean>("orch_confirm_mutation", { sessionId, proceed });
}

export async function orchGetState(sessionId: string): Promise<OrchSessionState | null> {
  return invoke<OrchSessionState | null>("orch_get_state", { sessionId });
}

/// Test hook — reset all state. Not for production use.
export function __resetForTests(): void {
  for (const sid of Object.keys(store.sessions)) delete store.sessions[sid];
  store.active = undefined;
  listeners.clear();
  unlisten = null;
  subscribePromise = null;
}
