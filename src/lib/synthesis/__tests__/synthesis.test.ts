import { describe, expect, it } from "vitest";
import {
  appendAttempt,
  jaccard,
  parseWorkerNdjson,
  SIMILARITY_THRESHOLD,
  synthesize,
} from "../index";
import type { WorkerOutput } from "../types";

function claudeWith(claims: WorkerOutput["claims"], openQs: WorkerOutput["openQuestions"] = []): WorkerOutput {
  return { worker: "claude", phase: "firstpass", claims, openQuestions: openQs };
}
function codexWith(claims: WorkerOutput["claims"], openQs: WorkerOutput["openQuestions"] = []): WorkerOutput {
  return { worker: "codex", phase: "firstpass", claims, openQuestions: openQs };
}

describe("similarity", () => {
  it("identical strings → 1", () => {
    expect(jaccard("REST endpoints use plural nouns", "REST endpoints use plural nouns")).toBe(1);
  });
  it("paraphrase above threshold", () => {
    const a = "REST endpoints use plural nouns per Fielding";
    const b = "REST endpoints conventionally use plural nouns per Fielding";
    expect(jaccard(a, b)).toBeGreaterThanOrEqual(SIMILARITY_THRESHOLD);
  });
  it("unrelated → near zero", () => {
    expect(jaccard("REST endpoints", "GraphQL eliminates over fetching")).toBeLessThan(0.1);
  });
});

describe("synthesize — identical inputs", () => {
  it("two equal high-conf claims → verified, no leftovers", () => {
    const cl = claudeWith([
      { id: "c1", text: "REST endpoints use plural nouns per Fielding", citations: [{ url: "https://a" }], confidence: "high", topic: "rest" },
    ]);
    const cx = codexWith([
      { id: "x1", text: "REST endpoints use plural nouns per Fielding", citations: [{ url: "https://b" }], confidence: "high", topic: "rest" },
    ]);
    const out = synthesize(cl, cx);
    expect(out.verified).toHaveLength(1);
    expect(out.verified[0].confidence).toBe("high");
    expect(out.verified[0].sources).toEqual(["https://a", "https://b"]);
    expect(out.claudeOnly).toHaveLength(0);
    expect(out.codexOnly).toHaveLength(0);
    expect(out.disagreement).toHaveLength(0);
    expect(out.open).toHaveLength(0);
  });
});

describe("synthesize — partial overlap", () => {
  it("one shared, one each side unique → 1 verified + 1 claude_only + 1 codex_only", () => {
    const cl = claudeWith([
      { id: "c1", text: "Idempotent methods are safe to retry per RFC 9110", citations: [], confidence: "high", topic: "http-methods" },
      { id: "c2", text: "Cursor pagination outperforms offset for large datasets", citations: [], confidence: "med", topic: "pagination" },
    ]);
    const cx = codexWith([
      { id: "x1", text: "Idempotent methods are safe to retry per RFC 9110", citations: [], confidence: "high", topic: "http-methods" },
      { id: "x2", text: "GraphQL eliminates over-fetching by exact field selection", citations: [], confidence: "med", topic: "graphql" },
    ]);
    const out = synthesize(cl, cx);
    expect(out.verified).toHaveLength(1);
    expect(out.claudeOnly).toHaveLength(1);
    expect(out.codexOnly).toHaveLength(1);
    expect(out.claudeOnly[0].claim).toMatch(/Cursor pagination/);
    expect(out.codexOnly[0].claim).toMatch(/GraphQL/);
  });
});

describe("synthesize — contradictions", () => {
  it("same topic, divergent texts → disagreement row", () => {
    const cl = claudeWith([
      { id: "c1", text: "Use cursor pagination always for sorted feeds", citations: [], confidence: "high", topic: "pagination" },
    ]);
    const cx = codexWith([
      { id: "x1", text: "Offset pagination is acceptable when total count exposed in UI", citations: [], confidence: "high", topic: "pagination" },
    ]);
    const out = synthesize(cl, cx);
    expect(out.disagreement).toHaveLength(1);
    expect(out.disagreement[0].topic).toBe("pagination");
    expect(out.disagreement[0].claudePosition).toMatch(/cursor/);
    expect(out.disagreement[0].codexPosition).toMatch(/Offset/);
    expect(out.verified).toHaveLength(0);
  });
});

describe("synthesize — both low confidence", () => {
  it("matched pair, both low → open", () => {
    const cl = claudeWith([
      { id: "c1", text: "Some untested claim about caching strategy", citations: [], confidence: "low", topic: "caching" },
    ]);
    const cx = codexWith([
      { id: "x1", text: "Some untested claim about caching strategy", citations: [], confidence: "low", topic: "caching" },
    ]);
    const out = synthesize(cl, cx);
    expect(out.open).toHaveLength(1);
    expect(out.open[0].question).toMatch(/Both workers low-confidence/);
    expect(out.verified).toHaveLength(0);
  });
});

describe("synthesize — mixed confidence pair", () => {
  it("claude high + codex low → claude_only with weak corroboration note", () => {
    const cl = claudeWith([
      { id: "c1", text: "REST endpoints use plural nouns per Fielding", citations: [{ url: "https://x" }], confidence: "high", topic: "rest" },
    ]);
    const cx = codexWith([
      { id: "x1", text: "REST endpoints use plural nouns per Fielding", citations: [], confidence: "low", topic: "rest" },
    ]);
    const out = synthesize(cl, cx);
    expect(out.claudeOnly).toHaveLength(1);
    expect(out.claudeOnly[0].note).toMatch(/weak corroboration/);
    expect(out.verified).toHaveLength(0);
  });
});

describe("synthesize — topic mismatch", () => {
  it("same text but different topics → both *_only (no pairing across topics)", () => {
    const cl = claudeWith([
      { id: "c1", text: "Cache must be invalidated on write", citations: [], confidence: "high", topic: "http-cache" },
    ]);
    const cx = codexWith([
      { id: "x1", text: "Cache must be invalidated on write", citations: [], confidence: "high", topic: "db-cache" },
    ]);
    const out = synthesize(cl, cx);
    expect(out.claudeOnly).toHaveLength(1);
    expect(out.codexOnly).toHaveLength(1);
    expect(out.verified).toHaveLength(0);
  });
});

describe("appendAttempt — retry tracking", () => {
  it("second attempt with same id appends new citation, keeps higher conf", () => {
    const a1 = claudeWith([
      { id: "c1", text: "claim v1", citations: [{ url: "https://a" }], confidence: "med", topic: "t" },
    ]);
    const a2 = claudeWith([
      { id: "c1", text: "claim v2", citations: [{ url: "https://b" }], confidence: "high", topic: "t" },
    ]);
    const merged = appendAttempt(a1, a2);
    expect(merged.claims).toHaveLength(1);
    expect(merged.claims[0].citations).toHaveLength(2);
    expect(merged.claims[0].confidence).toBe("high");
    expect(merged.claims[0].text).toBe("claim v2"); // higher conf wins for text
    expect(merged.attempt).toBeGreaterThanOrEqual(1);
  });

  it("second attempt with new id is added as separate claim", () => {
    const a1 = claudeWith([{ id: "c1", text: "x", citations: [], confidence: "high", topic: "t" }]);
    const a2 = claudeWith([{ id: "c2", text: "y", citations: [], confidence: "high", topic: "t" }]);
    const merged = appendAttempt(a1, a2);
    expect(merged.claims).toHaveLength(2);
  });

  it("worker mismatch throws (never silently coerce)", () => {
    expect(() => appendAttempt(claudeWith([]), codexWith([]))).toThrow(/worker mismatch/);
  });
});

describe("synthesize — open questions", () => {
  it("open questions from both sides land in open column with raisedBy", () => {
    const cl = claudeWith([], [{ id: "q1", text: "Error envelope: RFC 9457 or custom?" }]);
    const cx = codexWith([], [{ id: "q2", text: "URL versioning vs header versioning?" }]);
    const out = synthesize(cl, cx);
    expect(out.open).toHaveLength(2);
    expect(out.open.map((q) => q.raisedBy).sort()).toEqual(["claude", "codex"]);
  });
});

describe("synthesize — empty inputs", () => {
  it("empty workers → empty synthesis, all columns []", () => {
    const out = synthesize(claudeWith([]), codexWith([]));
    expect(out.verified).toHaveLength(0);
    expect(out.claudeOnly).toHaveLength(0);
    expect(out.codexOnly).toHaveLength(0);
    expect(out.disagreement).toHaveLength(0);
    expect(out.open).toHaveLength(0);
  });
});

describe("parseWorkerNdjson", () => {
  it("parses real mock fixture shape", () => {
    const text = [
      JSON.stringify({ event: "start", worker: "claude", phase: "firstpass" }),
      JSON.stringify({ event: "claim", id: "c1", text: "x", citations: [{ url: "https://a" }], confidence: "high", applicability: "rest" }),
      JSON.stringify({ event: "open_question", id: "q1", text: "?" }),
      JSON.stringify({ event: "end", status: "ok" }),
      "",
    ].join("\n");
    const out = parseWorkerNdjson(text, "claude");
    expect(out.worker).toBe("claude");
    expect(out.claims).toHaveLength(1);
    expect(out.claims[0].applicability).toBe("rest");
    expect(out.openQuestions).toHaveLength(1);
  });

  it("ignores malformed lines (does not throw)", () => {
    const text = "{not json}\n" + JSON.stringify({ event: "claim", id: "c1", text: "ok", citations: [], confidence: "med" });
    const out = parseWorkerNdjson(text, "claude");
    expect(out.claims).toHaveLength(1);
  });
});

describe("synthesize — determinism", () => {
  it("output is stable across repeated calls (no random ordering)", () => {
    const cl = claudeWith([
      { id: "c1", text: "alpha beta gamma delta", citations: [], confidence: "high", topic: "t1" },
      { id: "c2", text: "epsilon zeta eta", citations: [], confidence: "high", topic: "t2" },
    ]);
    const cx = codexWith([
      { id: "x1", text: "alpha beta gamma delta", citations: [], confidence: "high", topic: "t1" },
      { id: "x2", text: "different theta iota kappa", citations: [], confidence: "high", topic: "t2" },
    ]);
    const a = synthesize(cl, cx);
    const b = synthesize(cl, cx);
    expect(JSON.stringify(a)).toBe(JSON.stringify(b));
  });
});
