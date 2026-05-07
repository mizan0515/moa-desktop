// Deterministic 5-column synthesis. Pure function, no LLM, no I/O.
// Rules (see TICKETS/T3-synthesis-engine.md success criteria):
//   - Token Jaccard >= SIMILARITY_THRESHOLD (0.85) classifies a Claude×Codex
//     pair as "same claim" → verified candidate.
//   - Confidence rule: both high → verified(high); both med → verified(med);
//     mixed high+med → verified(med); high+low → high side promotes to its
//     own column with a `weak corroboration` note (not verified — low side is
//     unreliable evidence); both low → open.
//   - Same topic, similarity below threshold → disagreement.
//   - Unmatched claims: low conf → open; otherwise *_only.
//   - Open questions from both workers → open column.

import type {
  ClaudeOnlyRow,
  CodexOnlyRow,
  Confidence,
  DisagreementRow,
  SynthesisData,
  VerifiedRow,
} from "../synthesisTypes";
import { jaccard, SIMILARITY_THRESHOLD } from "./similarity";
import type { ClaimAttempt, WorkerClaim, WorkerOutput } from "./types";
import { topicKey } from "./types";

const CONF_RANK: Record<Confidence, number> = { high: 2, med: 1, low: 0 };

function minConf(a: Confidence, b: Confidence): Confidence {
  return CONF_RANK[a] <= CONF_RANK[b] ? a : b;
}

// FIX-E: claims tagged UNVERIFIED by the worker must not enter the verified
// column even if both sides agree on the text. Coerce confidence to "low" so
// the existing both-low / mixed-low rules handle them deterministically.
const UNVERIFIED_RE = /\[UNVERIFIED\]|\bUNVERIFIED\s*:?/gi;
function isUnverified(claim: WorkerClaim): boolean {
  // Reset lastIndex — global flag retains state across .test() calls.
  UNVERIFIED_RE.lastIndex = 0;
  return UNVERIFIED_RE.test(claim.text);
}
function stripUnverifiedMarker(text: string): string {
  return text.replace(UNVERIFIED_RE, "").replace(/\s+/g, " ").trim();
}
function normalizeClaim(claim: WorkerClaim): WorkerClaim {
  if (!isUnverified(claim)) return claim;
  return {
    ...claim,
    text: stripUnverifiedMarker(claim.text),
    confidence: "low",
  };
}

function citationsToSources(claim: WorkerClaim): string[] {
  return claim.citations
    .map((c) => c.url ?? (c.file ? `${c.file}${c.line ? `:${c.line}` : ""}` : c.excerpt ?? ""))
    .filter((s) => s.length > 0);
}

interface Pairing {
  claude: WorkerClaim;
  codex: WorkerClaim;
  similarity: number;
}

/**
 * Greedy best-first matcher inside a topic cluster.
 * Returns matched pairs + leftover claims from each side.
 */
function pairWithinTopic(
  claudeClaims: WorkerClaim[],
  codexClaims: WorkerClaim[],
): { pairs: Pairing[]; claudeLeft: WorkerClaim[]; codexLeft: WorkerClaim[] } {
  const pairs: Pairing[] = [];
  const claudeUsed = new Set<number>();
  const codexUsed = new Set<number>();

  // Build all candidate similarities, sort desc, assign greedily.
  const cands: Array<{ ci: number; xi: number; sim: number }> = [];
  claudeClaims.forEach((cc, ci) => {
    codexClaims.forEach((xc, xi) => {
      cands.push({ ci, xi, sim: jaccard(cc.text, xc.text) });
    });
  });
  cands.sort((a, b) => b.sim - a.sim || a.ci - b.ci || a.xi - b.xi);

  for (const c of cands) {
    if (claudeUsed.has(c.ci) || codexUsed.has(c.xi)) continue;
    // Within a topic, *every* leftover claim eventually pairs (similarity may
    // be low → caller decides verified vs disagreement). We only stop when one
    // side is exhausted.
    pairs.push({ claude: claudeClaims[c.ci], codex: codexClaims[c.xi], similarity: c.sim });
    claudeUsed.add(c.ci);
    codexUsed.add(c.xi);
  }

  return {
    pairs,
    claudeLeft: claudeClaims.filter((_, i) => !claudeUsed.has(i)),
    codexLeft: codexClaims.filter((_, i) => !codexUsed.has(i)),
  };
}

function classifyPair(p: Pairing, out: SynthesisData): void {
  const { claude, codex, similarity } = p;
  const claudeSrc = citationsToSources(claude);
  const codexSrc = citationsToSources(codex);

  if (similarity >= SIMILARITY_THRESHOLD) {
    const both = [claude.confidence, codex.confidence];
    const lowCount = both.filter((c) => c === "low").length;
    if (lowCount === 2) {
      out.open.push({
        kind: "open",
        question: `Both workers low-confidence on: ${claude.text}`,
        raisedBy: "both",
      });
      return;
    }
    if (lowCount === 1) {
      // High side promotes; low side discarded as evidence.
      const promoteCodex = codex.confidence !== "low";
      if (promoteCodex) {
        const row: CodexOnlyRow = {
          kind: "codex_only",
          claim: codex.text,
          sources: codexSrc,
          confidence: codex.confidence,
          note: "claude weak corroboration (low conf)",
        };
        out.codexOnly.push(row);
      } else {
        const row: ClaudeOnlyRow = {
          kind: "claude_only",
          claim: claude.text,
          sources: claudeSrc,
          confidence: claude.confidence,
          note: "codex weak corroboration (low conf)",
        };
        out.claudeOnly.push(row);
      }
      return;
    }
    const verified: VerifiedRow = {
      kind: "verified",
      claim: claude.text, // Claude phrasing wins (orchestrator is Claude-side).
      sources: dedupe([...claudeSrc, ...codexSrc]),
      confidence: minConf(claude.confidence, codex.confidence),
    };
    out.verified.push(verified);
    return;
  }

  // Same topic, divergent conclusions → disagreement.
  const dis: DisagreementRow = {
    kind: "disagreement",
    topic: claude.topic ?? claude.applicability ?? codex.topic ?? codex.applicability ?? "",
    claudePosition: claude.text,
    codexPosition: codex.text,
  };
  out.disagreement.push(dis);
}

function classifySolo(claim: WorkerClaim, side: "claude" | "codex", out: SynthesisData): void {
  const sources = citationsToSources(claim);
  if (claim.confidence === "low") {
    out.open.push({
      kind: "open",
      question: `${side} unverified: ${claim.text}`,
      raisedBy: side,
    });
    return;
  }
  if (side === "claude") {
    out.claudeOnly.push({
      kind: "claude_only",
      claim: claim.text,
      sources,
      confidence: claim.confidence,
    });
  } else {
    out.codexOnly.push({
      kind: "codex_only",
      claim: claim.text,
      sources,
      confidence: claim.confidence,
    });
  }
}

function dedupe(arr: string[]): string[] {
  return Array.from(new Set(arr));
}

function snapshotOf(c: WorkerClaim): ClaimAttempt {
  return {
    text: c.text,
    confidence: c.confidence,
    citations: [...c.citations],
    attempt: c.attempt,
  };
}

/**
 * Append a retry attempt: same-`id` claims accumulate citations and take the
 * higher confidence on the top-level fields; full per-attempt history is
 * preserved on `attempts[]`. New ids are added. Open questions dedupe by id.
 * Pure — returns a new `WorkerOutput`.
 *
 * FIX-E: prior attempts are no longer overwritten. `attempts[]` holds a
 * chronological snapshot list so a downgrade or text revision is recoverable.
 */
export function appendAttempt(prev: WorkerOutput, next: WorkerOutput): WorkerOutput {
  if (prev.worker !== next.worker) {
    throw new Error(
      `appendAttempt: worker mismatch (${prev.worker} vs ${next.worker}) — refusing to merge`,
    );
  }
  const claimsById = new Map<string, WorkerClaim>();
  for (const c of prev.claims) {
    const cloned: WorkerClaim = {
      ...c,
      citations: [...c.citations],
      attempts: c.attempts ? [...c.attempts] : [snapshotOf(c)],
    };
    claimsById.set(c.id, cloned);
  }
  for (const c of next.claims) {
    const existing = claimsById.get(c.id);
    if (!existing) {
      claimsById.set(c.id, {
        ...c,
        citations: [...c.citations],
        attempts: c.attempts ? [...c.attempts] : [snapshotOf(c)],
      });
      continue;
    }
    // History: always record this attempt verbatim, regardless of conf delta.
    existing.attempts!.push(snapshotOf(c));
    // Citations: union by url+file+line+excerpt key.
    const seen = new Set(existing.citations.map((x) => `${x.url ?? ""}|${x.file ?? ""}|${x.line ?? ""}|${x.excerpt ?? ""}`));
    for (const cit of c.citations) {
      const k = `${cit.url ?? ""}|${cit.file ?? ""}|${cit.line ?? ""}|${cit.excerpt ?? ""}`;
      if (!seen.has(k)) {
        existing.citations.push(cit);
        seen.add(k);
      }
    }
    // Top-level fields: keep higher-confidence text/conf as the primary view.
    if (CONF_RANK[c.confidence] > CONF_RANK[existing.confidence]) {
      existing.confidence = c.confidence;
      existing.text = c.text;
    }
    existing.attempt = Math.max(existing.attempt ?? 0, c.attempt ?? 0, (prev.attempt ?? 0) + 1);
  }

  const qById = new Map<string, { id: string; text: string }>();
  for (const q of prev.openQuestions) qById.set(q.id, q);
  for (const q of next.openQuestions) if (!qById.has(q.id)) qById.set(q.id, q);

  return {
    worker: prev.worker,
    phase: next.phase || prev.phase,
    claims: Array.from(claimsById.values()),
    openQuestions: Array.from(qById.values()),
    attempt: Math.max(prev.attempt ?? 0, next.attempt ?? 0, (prev.attempt ?? 0) + 1),
  };
}

/** Synthesize Claude × Codex worker outputs into the 5-column schema. */
export function synthesize(claude: WorkerOutput, codex: WorkerOutput): SynthesisData {
  if (claude.worker !== "claude") {
    throw new Error(`synthesize: first arg must be claude worker (got ${claude.worker})`);
  }
  if (codex.worker !== "codex") {
    throw new Error(`synthesize: second arg must be codex worker (got ${codex.worker})`);
  }
  const out: SynthesisData = {
    verified: [],
    codexOnly: [],
    claudeOnly: [],
    disagreement: [],
    open: [],
  };

  // Cluster claims by topic key. UNVERIFIED claims are normalized to low conf
  // up front so downstream pair classification handles them correctly.
  const topics = new Set<string>();
  const claudeByTopic = new Map<string, WorkerClaim[]>();
  const codexByTopic = new Map<string, WorkerClaim[]>();
  for (const raw of claude.claims) {
    const c = normalizeClaim(raw);
    const k = topicKey(c);
    topics.add(k);
    (claudeByTopic.get(k) ?? claudeByTopic.set(k, []).get(k)!).push(c);
  }
  for (const raw of codex.claims) {
    const c = normalizeClaim(raw);
    const k = topicKey(c);
    topics.add(k);
    (codexByTopic.get(k) ?? codexByTopic.set(k, []).get(k)!).push(c);
  }

  // Stable iteration: sorted topic keys.
  const sortedTopics = Array.from(topics).sort();
  for (const t of sortedTopics) {
    const cls = claudeByTopic.get(t) ?? [];
    const cxs = codexByTopic.get(t) ?? [];
    if (cls.length === 0) {
      for (const c of cxs) classifySolo(c, "codex", out);
      continue;
    }
    if (cxs.length === 0) {
      for (const c of cls) classifySolo(c, "claude", out);
      continue;
    }
    const { pairs, claudeLeft, codexLeft } = pairWithinTopic(cls, cxs);
    for (const p of pairs) classifyPair(p, out);
    for (const c of claudeLeft) classifySolo(c, "claude", out);
    for (const c of codexLeft) classifySolo(c, "codex", out);
  }

  // Open questions: simple union, dedup by text.
  const seenQ = new Set<string>();
  for (const q of claude.openQuestions) {
    if (seenQ.has(q.text)) continue;
    seenQ.add(q.text);
    out.open.push({ kind: "open", question: q.text, raisedBy: "claude" });
  }
  for (const q of codex.openQuestions) {
    if (seenQ.has(q.text)) continue;
    seenQ.add(q.text);
    out.open.push({ kind: "open", question: q.text, raisedBy: "codex" });
  }

  return out;
}
