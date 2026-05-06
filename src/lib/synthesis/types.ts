// T3 synthesis engine — input types.
// Worker NDJSON shape (matches mockResponses/{claude,codex}_firstpass.json).
// Output types are re-used from src/lib/synthesisTypes.ts (T6 contract).

import type { Confidence } from "../synthesisTypes";

export type WorkerName = "claude" | "codex";

export interface Citation {
  url?: string;
  file?: string;
  line?: number;
  excerpt?: string;
}

export interface WorkerClaim {
  id: string;
  text: string;
  citations: Citation[];
  confidence: Confidence;
  applicability?: string;
  /** Explicit topic key for clustering. Falls back to `applicability` when absent. */
  topic?: string;
  /** Per-claim attempt counter. Higher attempt # appends evidence rather than overwriting. */
  attempt?: number;
}

export interface WorkerOpenQuestion {
  id: string;
  text: string;
}

export interface WorkerOutput {
  worker: WorkerName;
  phase: string;
  claims: WorkerClaim[];
  openQuestions: WorkerOpenQuestion[];
  /** Worker-level attempt counter — for diagnostics/retry tracking. */
  attempt?: number;
}

interface RawWorkerEvent {
  event?: string;
  worker?: WorkerName;
  phase?: string;
  attempt?: number;
  // claim fields
  id?: string;
  text?: string;
  citations?: Citation[];
  confidence?: Confidence;
  applicability?: string;
  topic?: string;
}

/** Parse Worker NDJSON (start/claim/open_question/end) into a `WorkerOutput`. */
export function parseWorkerNdjson(text: string, fallbackWorker: WorkerName): WorkerOutput {
  const out: WorkerOutput = {
    worker: fallbackWorker,
    phase: "firstpass",
    claims: [],
    openQuestions: [],
  };
  for (const line of text.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    let evt: RawWorkerEvent;
    try {
      evt = JSON.parse(trimmed) as RawWorkerEvent;
    } catch {
      continue;
    }
    switch (evt.event) {
      case "start":
        if (evt.worker) out.worker = evt.worker;
        if (evt.phase) out.phase = evt.phase;
        if (typeof evt.attempt === "number") out.attempt = evt.attempt;
        break;
      case "claim":
        if (!evt.id || !evt.text) continue;
        out.claims.push({
          id: evt.id,
          text: evt.text,
          citations: evt.citations ?? [],
          confidence: evt.confidence ?? "med",
          applicability: evt.applicability,
          topic: evt.topic,
          attempt: evt.attempt,
        });
        break;
      case "open_question":
        if (!evt.id || !evt.text) continue;
        out.openQuestions.push({ id: evt.id, text: evt.text });
        break;
    }
  }
  return out;
}

/** Stable topic key for clustering. Worker `topic` wins; else `applicability`; else id-anchored. */
export function topicKey(claim: WorkerClaim): string {
  return (claim.topic ?? claim.applicability ?? `__topicless__:${claim.id}`).trim().toLowerCase();
}
