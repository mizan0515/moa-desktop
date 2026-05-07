// T7-thin — dry-run orchestrator (frontend store).
//
// Mirrors the Rust state machine in `src-tauri/src/orchestrator/dryrun.rs`.
// Subscribes to the `dryrun://event` channel and accumulates per-session
// state for the Workbench panes. No persistence — sessions live in memory
// for the demo (T7-full or T9 will add disk).

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  EMPTY_SYNTHESIS,
  type ClaimLedgerEntry,
  type Confidence,
  type EvidenceLevel,
  type SynthesisData,
} from "../synthesisTypes";

export type Phase =
  | "preflight"
  | "first-pass"
  | "synthesis"
  | "adversarial"
  | "final";

export type Lane = "system" | "claude" | "codex";

export type EventKind =
  | "session_start"
  | "phase_start"
  | "line"
  | "phase_end"
  | "session_done"
  | "session_cancelled"
  | "session_error";

export interface DryRunEvent {
  session_id: string;
  phase: Phase;
  lane?: Lane;
  kind: EventKind;
  payload?: unknown;
}

export type SessionStatus =
  | "idle"
  | "running"
  | "done"
  | "cancelled"
  | "error";

export interface SynthesisRow {
  column: string;
  // shape varies by column — keep raw event for the view
  raw: Record<string, unknown>;
}

export interface ClaimRow {
  id?: string;
  text: string;
  citations?: Array<{ url?: string; excerpt?: string }>;
  confidence?: string;
  applicability?: string;
  lane: Lane;
}

export interface FinalRecommendation {
  id?: string;
  text: string;
}

export interface FinalReport {
  summary?: Record<string, unknown>;
  recommendations: FinalRecommendation[];
  residual_risks: FinalRecommendation[];
  verdict?: string;
}

export interface PhaseState {
  status: "pending" | "active" | "done" | "error";
  lanesActive: Set<Lane>;
}

export interface Session {
  id: string;
  task: string;
  status: SessionStatus;
  currentPhase: Phase;
  phases: Record<Phase, PhaseState>;
  // Aggregated data for the Results pane:
  claims: ClaimRow[];
  synthesisRows: SynthesisRow[];
  finalReport: FinalReport;
  adversarialCritiques: Array<{ lane: Lane; raw: Record<string, unknown> }>;
  // Raw log entries for LogPane:
  logs: Array<{ ts: string; lane: Lane; msg: string }>;
  errorMessage?: string;
}

interface Store {
  sessions: Session[];
  activeSessionId: string | null;
}

type Listener = () => void;

const PHASES: Phase[] = [
  "preflight",
  "first-pass",
  "synthesis",
  "adversarial",
  "final",
];

function emptyPhases(): Record<Phase, PhaseState> {
  const out = {} as Record<Phase, PhaseState>;
  for (const p of PHASES) {
    out[p] = { status: "pending", lanesActive: new Set() };
  }
  return out;
}

function emptySession(id: string, task: string): Session {
  return {
    id,
    task,
    status: "running",
    currentPhase: "preflight",
    phases: emptyPhases(),
    claims: [],
    synthesisRows: [],
    finalReport: { recommendations: [], residual_risks: [] },
    adversarialCritiques: [],
    logs: [],
  };
}

const store: Store = { sessions: [], activeSessionId: null };
const listeners = new Set<Listener>();

let unlistenPromise: Promise<UnlistenFn> | null = null;

function notify() {
  for (const l of listeners) l();
}

function getSession(id: string): Session | undefined {
  return store.sessions.find((s) => s.id === id);
}

function tsNow(): string {
  return new Date().toISOString().slice(11, 19);
}

function ingest(ev: DryRunEvent) {
  // FIX-C — defensive: if an event somehow arrives for a sid we have not
  // inserted yet, register a placeholder so it is not dropped. The new
  // ack handshake (`dryrun_ack`) makes this a cold path on the happy
  // flow, but the safety net mirrors the production orchestrator store.
  let sess = getSession(ev.session_id);
  if (!sess) {
    if (ev.kind !== "session_start" && ev.kind !== "phase_start") return;
    sess = emptySession(ev.session_id, "");
    store.sessions.unshift(sess);
  }
  const lane = ev.lane ?? "system";

  switch (ev.kind) {
    case "session_start":
      sess.logs.push({ ts: tsNow(), lane, msg: `session start — ${sess.task}` });
      break;
    case "phase_start":
      sess.currentPhase = ev.phase;
      sess.phases[ev.phase].status = "active";
      sess.phases[ev.phase].lanesActive.add(lane);
      sess.logs.push({ ts: tsNow(), lane, msg: `${ev.phase} start` });
      break;
    case "line":
      sess.logs.push({
        ts: tsNow(),
        lane,
        msg: summarizeLine(ev.payload),
      });
      absorbLine(sess, ev.phase, lane, ev.payload);
      break;
    case "phase_end":
      sess.phases[ev.phase].lanesActive.delete(lane);
      if (sess.phases[ev.phase].lanesActive.size === 0) {
        sess.phases[ev.phase].status = "done";
      }
      sess.logs.push({ ts: tsNow(), lane, msg: `${ev.phase} end` });
      break;
    case "session_done":
      sess.status = "done";
      sess.logs.push({ ts: tsNow(), lane, msg: "session done" });
      break;
    case "session_cancelled":
      sess.status = "cancelled";
      sess.phases[ev.phase].status = "error";
      sess.logs.push({ ts: tsNow(), lane, msg: "session cancelled" });
      break;
    case "session_error":
      sess.status = "error";
      sess.phases[ev.phase].status = "error";
      sess.errorMessage =
        (ev.payload as { message?: string })?.message ?? "unknown error";
      sess.logs.push({
        ts: tsNow(),
        lane,
        msg: `error: ${sess.errorMessage}`,
      });
      break;
  }
  notify();
}

function summarizeLine(payload: unknown): string {
  if (payload && typeof payload === "object") {
    const p = payload as Record<string, unknown>;
    if (typeof p.event === "string") {
      const fields: string[] = [String(p.event)];
      for (const k of ["id", "column", "topic", "verdict", "status"]) {
        if (typeof p[k] === "string") fields.push(`${k}=${p[k] as string}`);
      }
      return fields.join(" ");
    }
  }
  return String(payload).slice(0, 120);
}

function absorbLine(
  sess: Session,
  phase: Phase,
  lane: Lane,
  payload: unknown,
) {
  if (!payload || typeof payload !== "object") return;
  const p = payload as Record<string, unknown>;
  const event = p.event as string | undefined;
  if (!event) return;

  if (phase === "first-pass" && event === "claim") {
    sess.claims.push({
      id: p.id as string | undefined,
      text: (p.text as string) ?? "",
      citations: (p.citations as ClaimRow["citations"]) ?? [],
      confidence: p.confidence as string | undefined,
      applicability: p.applicability as string | undefined,
      lane,
    });
  } else if (phase === "synthesis" && event === "row") {
    sess.synthesisRows.push({
      column: (p.column as string) ?? "?",
      raw: p,
    });
  } else if (phase === "adversarial" && event === "critique") {
    sess.adversarialCritiques.push({ lane, raw: p });
  } else if (phase === "final") {
    if (event === "summary") {
      sess.finalReport.summary = p;
    } else if (event === "recommendation") {
      sess.finalReport.recommendations.push({
        id: p.id as string | undefined,
        text: (p.text as string) ?? "",
      });
    } else if (event === "residual_risk") {
      sess.finalReport.residual_risks.push({
        id: p.id as string | undefined,
        text: (p.text as string) ?? "",
      });
    } else if (event === "end" && typeof p.verdict === "string") {
      sess.finalReport.verdict = p.verdict;
    }
  }
}

async function ensureListener() {
  if (unlistenPromise) return unlistenPromise;
  unlistenPromise = listen<DryRunEvent>("dryrun://event", (e) => {
    ingest(e.payload);
  });
  return unlistenPromise;
}

export const dryRunStore = {
  subscribe(l: Listener): () => void {
    listeners.add(l);
    return () => listeners.delete(l);
  },
  getSnapshot(): Store {
    return store;
  },
  getActive(): Session | null {
    if (!store.activeSessionId) return null;
    return getSession(store.activeSessionId) ?? null;
  },
  setActive(id: string) {
    store.activeSessionId = id;
    notify();
  },
  async start(task: string): Promise<string> {
    await ensureListener();
    const sid = await invoke<string>("dryrun_start", { task });
    // FIX-C — insert the session shell BEFORE the ack so the backend's
    // first emit lands on a record we already have.
    store.sessions.unshift(emptySession(sid, task));
    store.activeSessionId = sid;
    notify();
    await invoke("dryrun_ack", { sessionId: sid });
    return sid;
  },
  async cancel(sid: string): Promise<boolean> {
    return invoke<boolean>("dryrun_cancel", { sessionId: sid });
  },
};

export const PHASE_ORDER: Phase[] = PHASES;

// T7-full → T6 component adapters.
// dryRunStore accumulates raw event payloads; T6 components want typed shapes.

function asConfidence(v: unknown): Confidence {
  return v === "high" || v === "low" ? v : "med";
}

export function toSynthesisData(session: Session | null): SynthesisData {
  if (!session) return EMPTY_SYNTHESIS;
  const out: SynthesisData = {
    verified: [],
    codexOnly: [],
    claudeOnly: [],
    disagreement: [],
    open: [],
  };
  for (const row of session.synthesisRows) {
    const p = row.raw;
    const sources = Array.isArray(p.sources) ? (p.sources as string[]) : [];
    switch (row.column) {
      case "verified":
        out.verified.push({
          kind: "verified",
          claim: (p.claim as string) ?? "",
          sources,
          confidence: asConfidence(p.confidence),
        });
        break;
      case "codex_only":
        out.codexOnly.push({
          kind: "codex_only",
          claim: (p.claim as string) ?? "",
          sources,
          confidence: asConfidence(p.confidence),
          note: p.note as string | undefined,
        });
        break;
      case "claude_only":
        out.claudeOnly.push({
          kind: "claude_only",
          claim: (p.claim as string) ?? "",
          sources,
          confidence: asConfidence(p.confidence),
          note: p.note as string | undefined,
        });
        break;
      case "disagreement":
        out.disagreement.push({
          kind: "disagreement",
          topic: (p.topic as string) ?? "",
          claudePosition: (p.claude_position as string) ?? "",
          codexPosition: (p.codex_position as string) ?? "",
          resolution: p.resolution as string | undefined,
        });
        break;
      case "open":
        out.open.push({
          kind: "open",
          question: (p.question as string) ?? "",
          raisedBy: p.raised_by as string | undefined,
        });
        break;
    }
  }
  return out;
}

export function toClaimLedger(session: Session | null): ClaimLedgerEntry[] {
  if (!session) return [];
  return session.claims.map((c) => {
    const cite = c.citations?.[0];
    const evidence = cite?.url ?? cite?.excerpt ?? "—";
    const level: EvidenceLevel = "L3";
    return {
      claim: c.text,
      evidence,
      level,
      confidence: asConfidence(c.confidence),
    };
  });
}
