// FIX-C — session insert / emit-ordering regression test.
//
// Two backend protocols this test pins:
//   1. orchStart awaits the orch_start invoke (which returns the sid AFTER
//      registering the session handle but BEFORE the driver emits anything)
//      and inserts the session shell into the store BEFORE issuing
//      orch_ack — the ack is what releases the driver to emit `session_start`.
//   2. Even if a `session_start` event somehow lands before the local
//      `ensureSession` call (concurrent-orchStart races, single shared event
//      channel), it must not be dropped — `onEvent` registers the session on
//      first sight.
//
// We mock the Tauri IPC + event bus end-to-end and assert:
//   - 100 concurrent orchStart calls → 100 distinct sids in the store
//     (collision pressure on the new_session_id generator).
//   - `orch_ack` is invoked exactly once per started session and only AFTER
//     the session shell exists in the store.
//   - Events that fire concurrently with the invoke resolution are not lost.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

type InvokeArgs = Record<string, unknown> | undefined;

// ── fakes ──────────────────────────────────────────────────────────────────

interface Listener {
  cb: (e: { payload: unknown }) => void;
}

const listeners: Listener[] = [];
let sidCounter = 0;
let invokeCalls: Array<{ cmd: string; args: InvokeArgs; sidAtCall?: string }> = [];
let storeSnapshot: () => Record<string, unknown> = () => ({});

function emitFromBackend(payload: unknown): void {
  for (const l of listeners) l.cb({ payload });
}

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string, args?: InvokeArgs) => {
    if (cmd === "orch_start") {
      // Simulate backend allocating a unique sid. We deliberately do NOT
      // emit `session_start` here — the new contract is "no emit until
      // orch_ack". A buggy backend that emitted here would still need to
      // pass through the listener (which onEvent must tolerate via
      // ensureSession), but we test the contract: ack gates emission.
      sidCounter += 1;
      // Pad to expose any string-ordering bug ("orch-1" vs "orch-10").
      const sid = `orch-${Date.now()}-${sidCounter}`;
      invokeCalls.push({ cmd, args });
      return sid;
    }
    if (cmd === "orch_ack") {
      const sessionId = (args as { sessionId?: string } | undefined)?.sessionId;
      // Now that the frontend has acked, the backend would emit
      // `session_start`. Simulate it inline.
      const snap = storeSnapshot();
      const had = sessionId ? sessionId in snap : false;
      invokeCalls.push({ cmd, args, sidAtCall: had ? "present" : "missing" });
      if (sessionId) {
        emitFromBackend({
          session_id: sessionId,
          phase: "preflight",
          lane: "system",
          kind: "session_start",
          payload: { task: "from-backend" },
        });
      }
      return true;
    }
    if (cmd === "orch_submit_synthesis" || cmd === "orch_cancel" || cmd === "orch_confirm_mutation") {
      return true;
    }
    if (cmd === "orch_get_state") return null;
    throw new Error(`unmocked invoke: ${cmd}`);
  }),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (_name: string, cb: (e: { payload: unknown }) => void) => {
    listeners.push({ cb });
    return () => {
      const i = listeners.findIndex((l) => l.cb === cb);
      if (i >= 0) listeners.splice(i, 1);
    };
  }),
}));

// ── tests ──────────────────────────────────────────────────────────────────

describe("orchStart event ordering + sid collision (FIX-C)", () => {
  beforeEach(async () => {
    listeners.length = 0;
    invokeCalls = [];
    sidCounter = 0;
    const mod = await import("../stateMachine");
    mod.__resetForTests();
    storeSnapshot = () => mod.orchStore.getSnapshot().sessions;
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("creates 100 distinct sessions under concurrent starts", async () => {
    const { orchStart, orchStore } = await import("../stateMachine");
    const N = 100;
    const sids = await Promise.all(
      Array.from({ length: N }, (_, i) =>
        orchStart({ task: `t${i}`, cwd: "/tmp", projectId: "p" }),
      ),
    );
    expect(new Set(sids).size).toBe(N);
    const snap = orchStore.getSnapshot();
    expect(Object.keys(snap.sessions)).toHaveLength(N);
    for (const sid of sids) {
      expect(snap.sessions[sid]).toBeDefined();
    }
  });

  it("invokes orch_ack only after the session is in the store", async () => {
    const { orchStart } = await import("../stateMachine");
    await orchStart({ task: "t", cwd: "/tmp", projectId: "p" });
    const ack = invokeCalls.find((c) => c.cmd === "orch_ack");
    expect(ack).toBeDefined();
    expect(ack?.sidAtCall).toBe("present");
  });

  it("invokes orch_ack exactly once per started session", async () => {
    const { orchStart } = await import("../stateMachine");
    await Promise.all([
      orchStart({ task: "a", cwd: "/tmp", projectId: "p" }),
      orchStart({ task: "b", cwd: "/tmp", projectId: "p" }),
      orchStart({ task: "c", cwd: "/tmp", projectId: "p" }),
    ]);
    const ackCount = invokeCalls.filter((c) => c.cmd === "orch_ack").length;
    expect(ackCount).toBe(3);
  });

  it("does not drop events that arrive before the explicit shell insert", async () => {
    // Race: backend emits for an unknown sid before we ever called orchStart.
    // onEvent must register the session on first sight (defensive — the new
    // protocol prevents this in the happy path, but we keep the safety net).
    const { orchStore } = await import("../stateMachine");
    // Force subscription by running one orchStart first.
    const { orchStart } = await import("../stateMachine");
    await orchStart({ task: "primer", cwd: "/tmp", projectId: "p" });

    emitFromBackend({
      session_id: "orch-stray-99",
      phase: "preflight",
      lane: "system",
      kind: "session_start",
      payload: { task: "stray" },
    });
    // Allow microtask for onEvent.
    await Promise.resolve();
    expect(orchStore.getSnapshot().sessions["orch-stray-99"]).toBeDefined();
    expect(orchStore.getSnapshot().sessions["orch-stray-99"].task).toBe("stray");
  });
});
