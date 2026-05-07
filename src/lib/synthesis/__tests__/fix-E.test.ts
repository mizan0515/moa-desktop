// FIX-E regression tests:
//  (1) negation-aware similarity — "X is safe" vs "X is not safe" must NOT match.
//  (2) UNVERIFIED guard — claim text marked UNVERIFIED forced to confidence=low,
//      cannot enter the verified column even if both sides agree.
//  (3) appendAttempt history — retries push to attempts[]; latest text/confidence
//      still on the top-level fields, but prior attempts are preserved.
//  (4) determinism — 100 randomized inputs → identical synthesis output.

import { describe, expect, it } from "vitest";
import { appendAttempt, jaccard, SIMILARITY_THRESHOLD, synthesize } from "../index";
import type { WorkerOutput } from "../types";

function claudeWith(claims: WorkerOutput["claims"], openQs: WorkerOutput["openQuestions"] = []): WorkerOutput {
  return { worker: "claude", phase: "firstpass", claims, openQuestions: openQs };
}
function codexWith(claims: WorkerOutput["claims"], openQs: WorkerOutput["openQuestions"] = []): WorkerOutput {
  return { worker: "codex", phase: "firstpass", claims, openQuestions: openQs };
}

describe("FIX-E negation-aware similarity", () => {
  it("'X is safe' vs 'X is not safe' must score below threshold", () => {
    const a = "Idempotent methods are safe to retry";
    const b = "Idempotent methods are not safe to retry";
    expect(jaccard(a, b)).toBeLessThan(SIMILARITY_THRESHOLD);
  });

  it("English 'never' polarity disagrees with affirmative", () => {
    const a = "REST endpoints expose internal state";
    const b = "REST endpoints never expose internal state";
    expect(jaccard(a, b)).toBeLessThan(SIMILARITY_THRESHOLD);
  });

  it("Korean 부정 표현 ('없') disagrees with affirmative", () => {
    const a = "캐시 무효화 정책이 일관되게 적용된다";
    const b = "캐시 무효화 정책이 일관되게 적용되지 않는다";
    // Even after stopword/length filtering, polarity flag forces sub-threshold.
    expect(jaccard(a, b)).toBeLessThan(SIMILARITY_THRESHOLD);
  });

  it("two negative claims still match each other", () => {
    const a = "REST endpoints should not expose internal IDs";
    const b = "REST endpoints should never expose internal IDs";
    // Both negative polarity → still similar (token overlap high).
    expect(jaccard(a, b)).toBeGreaterThanOrEqual(SIMILARITY_THRESHOLD);
  });

  it("synthesize: positive claude × negative codex → disagreement, not verified", () => {
    const cl = claudeWith([
      { id: "c1", text: "Idempotent methods are safe to retry per RFC 9110", citations: [], confidence: "high", topic: "http-methods" },
    ]);
    const cx = codexWith([
      { id: "x1", text: "Idempotent methods are not safe to retry per RFC 9110", citations: [], confidence: "high", topic: "http-methods" },
    ]);
    const out = synthesize(cl, cx);
    expect(out.verified).toHaveLength(0);
    expect(out.disagreement).toHaveLength(1);
  });
});

describe("FIX-E UNVERIFIED guard", () => {
  it("'UNVERIFIED:' prefix forces confidence=low → cannot enter verified column", () => {
    const cl = claudeWith([
      { id: "c1", text: "UNVERIFIED: REST endpoints use plural nouns per Fielding", citations: [], confidence: "high", topic: "rest" },
    ]);
    const cx = codexWith([
      { id: "x1", text: "UNVERIFIED: REST endpoints use plural nouns per Fielding", citations: [], confidence: "high", topic: "rest" },
    ]);
    const out = synthesize(cl, cx);
    expect(out.verified).toHaveLength(0);
    // Both forced to low → both-low pair → open.
    expect(out.open).toHaveLength(1);
    expect(out.open[0].question).toMatch(/Both workers low-confidence/);
  });

  it("'[UNVERIFIED]' inline marker also forces low", () => {
    const cl = claudeWith([
      { id: "c1", text: "Cursor pagination [UNVERIFIED] outperforms offset", citations: [], confidence: "high", topic: "pagination" },
    ]);
    const cx = codexWith([
      { id: "x1", text: "Cursor pagination [UNVERIFIED] outperforms offset", citations: [], confidence: "high", topic: "pagination" },
    ]);
    const out = synthesize(cl, cx);
    expect(out.verified).toHaveLength(0);
  });

  it("UNVERIFIED on one side only → mixed-low pair → *_only with weak corroboration", () => {
    const cl = claudeWith([
      { id: "c1", text: "REST endpoints use plural nouns", citations: [{ url: "https://x" }], confidence: "high", topic: "rest" },
    ]);
    const cx = codexWith([
      { id: "x1", text: "UNVERIFIED: REST endpoints use plural nouns", citations: [], confidence: "high", topic: "rest" },
    ]);
    const out = synthesize(cl, cx);
    expect(out.verified).toHaveLength(0);
    expect(out.claudeOnly).toHaveLength(1);
    expect(out.claudeOnly[0].note).toMatch(/weak corroboration/);
  });
});

describe("FIX-E appendAttempt — attempts[] history", () => {
  it("retry preserves prior attempt in attempts[] array", () => {
    const a1 = claudeWith([
      { id: "c1", text: "claim v1", citations: [{ url: "https://a" }], confidence: "med", topic: "t" },
    ]);
    const a2 = claudeWith([
      { id: "c1", text: "claim v2", citations: [{ url: "https://b" }], confidence: "high", topic: "t" },
    ]);
    const merged = appendAttempt(a1, a2);
    const c = merged.claims[0];
    // Latest text/confidence still wins (existing contract).
    expect(c.text).toBe("claim v2");
    expect(c.confidence).toBe("high");
    // History preserved.
    expect(c.attempts).toBeDefined();
    expect(c.attempts!.length).toBeGreaterThanOrEqual(2);
    expect(c.attempts![0].text).toBe("claim v1");
    expect(c.attempts![0].confidence).toBe("med");
    expect(c.attempts![1].text).toBe("claim v2");
    expect(c.attempts![1].confidence).toBe("high");
  });

  it("third attempt appends — does not overwrite history", () => {
    const a1 = claudeWith([{ id: "c1", text: "v1", citations: [], confidence: "low", topic: "t" }]);
    const a2 = claudeWith([{ id: "c1", text: "v2", citations: [], confidence: "med", topic: "t" }]);
    const a3 = claudeWith([{ id: "c1", text: "v3", citations: [], confidence: "high", topic: "t" }]);
    const m = appendAttempt(appendAttempt(a1, a2), a3);
    const c = m.claims[0];
    expect(c.attempts!.map((a) => a.text)).toEqual(["v1", "v2", "v3"]);
  });

  it("retry with lower confidence still records history (does not silently drop)", () => {
    const a1 = claudeWith([{ id: "c1", text: "v1", citations: [], confidence: "high", topic: "t" }]);
    const a2 = claudeWith([{ id: "c1", text: "v2", citations: [], confidence: "low", topic: "t" }]);
    const m = appendAttempt(a1, a2);
    const c = m.claims[0];
    // Top-level keeps higher conf (v1).
    expect(c.text).toBe("v1");
    expect(c.confidence).toBe("high");
    // But v2 is recorded in history.
    expect(c.attempts!.map((a) => a.text)).toEqual(["v1", "v2"]);
  });
});

describe("FIX-E determinism (property-style, 100 iterations)", () => {
  // Pseudo-random but reproducible: fixed seed via index-based shuffle.
  function shuffle<T>(arr: T[], seed: number): T[] {
    const a = [...arr];
    let s = seed;
    for (let i = a.length - 1; i > 0; i--) {
      s = (s * 1664525 + 1013904223) >>> 0;
      const j = s % (i + 1);
      [a[i], a[j]] = [a[j], a[i]];
    }
    return a;
  }
  const baseClaude = claudeWith([
    { id: "c1", text: "alpha beta gamma delta", citations: [{ url: "https://1" }], confidence: "high", topic: "t1" },
    { id: "c2", text: "epsilon zeta eta theta", citations: [{ url: "https://2" }], confidence: "med", topic: "t2" },
    { id: "c3", text: "iota kappa lambda mu", citations: [], confidence: "low", topic: "t3" },
  ]);
  const baseCodex = codexWith([
    { id: "x1", text: "alpha beta gamma delta", citations: [{ url: "https://3" }], confidence: "high", topic: "t1" },
    { id: "x2", text: "different nu xi omicron", citations: [], confidence: "high", topic: "t2" },
    { id: "x3", text: "iota kappa lambda mu", citations: [], confidence: "med", topic: "t3" },
  ]);
  const reference = JSON.stringify(synthesize(baseClaude, baseCodex));

  it("100 shuffled-input runs produce identical synthesis output", () => {
    for (let i = 0; i < 100; i++) {
      const cl: WorkerOutput = { ...baseClaude, claims: shuffle(baseClaude.claims, i + 1) };
      const cx: WorkerOutput = { ...baseCodex, claims: shuffle(baseCodex.claims, i + 7919) };
      expect(JSON.stringify(synthesize(cl, cx))).toBe(reference);
    }
  });
});
