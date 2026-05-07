// FIX-D: lane-buffer guard. The stateMachine must only accept canonical
// WorkerEvent payloads on `kind="line"`; raw adapter envelopes ride on
// `kind="worker_raw"` and are diagnostics-only.

import { describe, expect, it } from "vitest";

import { isCanonicalWorkerEventPayload } from "../stateMachine";

describe("isCanonicalWorkerEventPayload", () => {
  it("accepts canonical events", () => {
    for (const event of ["start", "claim", "open_question", "end"]) {
      expect(isCanonicalWorkerEventPayload({ event })).toBe(true);
    }
  });

  it("rejects Serde-tagged adapter envelopes", () => {
    expect(
      isCanonicalWorkerEventPayload({ kind: "assistant", text: "hi", raw: {} }),
    ).toBe(false);
    expect(isCanonicalWorkerEventPayload({ kind: "system_init" })).toBe(false);
    expect(isCanonicalWorkerEventPayload({ kind: "result" })).toBe(false);
    expect(isCanonicalWorkerEventPayload({ kind: "exit" })).toBe(false);
  });

  it("rejects unknown event values", () => {
    expect(isCanonicalWorkerEventPayload({ event: "made_up" })).toBe(false);
    expect(isCanonicalWorkerEventPayload({ event: 42 })).toBe(false);
  });

  it("rejects non-objects", () => {
    expect(isCanonicalWorkerEventPayload(null)).toBe(false);
    expect(isCanonicalWorkerEventPayload(undefined)).toBe(false);
    expect(isCanonicalWorkerEventPayload("claim")).toBe(false);
    expect(isCanonicalWorkerEventPayload(42)).toBe(false);
  });
});
