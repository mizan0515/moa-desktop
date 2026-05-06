// Shared types for SynthesisView, ClaimLedger, T3 (engine), T8 (mock).
// Stable contract: T3 produces SynthesisData; T6 renders it.

export type Confidence = "high" | "med" | "low";
export type EvidenceLevel = "L1" | "L2" | "L3" | "L4" | "none";
export type SynthesisColumn =
  | "verified"
  | "codex_only"
  | "claude_only"
  | "disagreement"
  | "open";

export interface VerifiedRow {
  kind: "verified";
  claim: string;
  sources: string[];
  confidence: Confidence;
}

export interface CodexOnlyRow {
  kind: "codex_only";
  claim: string;
  sources: string[];
  confidence: Confidence;
  note?: string;
}

export interface ClaudeOnlyRow {
  kind: "claude_only";
  claim: string;
  sources: string[];
  confidence: Confidence;
  note?: string;
}

export interface DisagreementRow {
  kind: "disagreement";
  topic: string;
  claudePosition: string;
  codexPosition: string;
  resolution?: string;
}

export interface OpenRow {
  kind: "open";
  question: string;
  raisedBy?: string;
}

export type SynthesisRow =
  | VerifiedRow
  | CodexOnlyRow
  | ClaudeOnlyRow
  | DisagreementRow
  | OpenRow;

export interface SynthesisData {
  verified: VerifiedRow[];
  codexOnly: CodexOnlyRow[];
  claudeOnly: ClaudeOnlyRow[];
  disagreement: DisagreementRow[];
  open: OpenRow[];
}

export interface ClaimLedgerEntry {
  claim: string;
  evidence: string;
  level: EvidenceLevel;
  confidence: Confidence;
  residualRisk?: string;
}

export const EMPTY_SYNTHESIS: SynthesisData = {
  verified: [],
  codexOnly: [],
  claudeOnly: [],
  disagreement: [],
  open: [],
};

// NDJSON event shape produced by T8 mock (and T3 engine).
export interface SynthesisEvent {
  event: "start" | "row" | "end";
  phase?: string;
  ts?: string;
  status?: string;
  column?: SynthesisColumn;
  claim?: string;
  sources?: string[];
  confidence?: Confidence;
  note?: string;
  topic?: string;
  claude_position?: string;
  codex_position?: string;
  resolution?: string;
  question?: string;
  raised_by?: string;
}

export function parseSynthesisNdjson(text: string): SynthesisData {
  const data: SynthesisData = {
    verified: [],
    codexOnly: [],
    claudeOnly: [],
    disagreement: [],
    open: [],
  };
  for (const line of text.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    let evt: SynthesisEvent;
    try {
      evt = JSON.parse(trimmed) as SynthesisEvent;
    } catch {
      continue;
    }
    if (evt.event !== "row" || !evt.column) continue;
    switch (evt.column) {
      case "verified":
        data.verified.push({
          kind: "verified",
          claim: evt.claim ?? "",
          sources: evt.sources ?? [],
          confidence: evt.confidence ?? "med",
        });
        break;
      case "codex_only":
        data.codexOnly.push({
          kind: "codex_only",
          claim: evt.claim ?? "",
          sources: evt.sources ?? [],
          confidence: evt.confidence ?? "med",
          note: evt.note,
        });
        break;
      case "claude_only":
        data.claudeOnly.push({
          kind: "claude_only",
          claim: evt.claim ?? "",
          sources: evt.sources ?? [],
          confidence: evt.confidence ?? "med",
          note: evt.note,
        });
        break;
      case "disagreement":
        data.disagreement.push({
          kind: "disagreement",
          topic: evt.topic ?? "",
          claudePosition: evt.claude_position ?? "",
          codexPosition: evt.codex_position ?? "",
          resolution: evt.resolution,
        });
        break;
      case "open":
        data.open.push({
          kind: "open",
          question: evt.question ?? "",
          raisedBy: evt.raised_by,
        });
        break;
    }
  }
  return data;
}

export const SYNTHESIS_COLUMN_LABEL: Record<SynthesisColumn, string> = {
  verified: "Verified",
  claude_only: "Claude-only",
  codex_only: "Codex-only",
  disagreement: "Disagreement",
  open: "Open",
};

export const SYNTHESIS_COLUMN_ORDER: SynthesisColumn[] = [
  "verified",
  "claude_only",
  "codex_only",
  "disagreement",
  "open",
];
