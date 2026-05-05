# S6 — Hook stderr/exit propagation under spawned `claude -p`

## Goal
Verify that hooks (TOKEN-GUARD, claude-mem, etc.) fire under spawn AND that
their output / exit code reaches the parent process in a parseable form.
Special case: HARD block (exit 2 from UserPromptSubmit) must produce a
deterministic signal — no silent hangs.

## Result: PASS

```
Part 1 (real ~/.claude TOKEN-GUARD hook):
  total events = 19
  hook events  = 12 (hook_started + hook_response pairs across SessionStart and
                     UserPromptSubmit, including one real hook that exited 49
                     surfaced as outcome=error)

Part 2 (simulated HARD block — UserPromptSubmit exit 2):
  hook_response exit_code=2 outcome=error  ✓
  hook stderr captured: "[S6 simulated HARD] msgs=999 cost=$99 ..."  ✓
  result event num_turns=0  ✓
  parent process exit=0
```

## Findings (load-bearing)

1. **`--include-hook-events` is REQUIRED.** Without it, hook lifecycle
   events are filtered out of stream-json. With it, every hook fires emits
   `system.hook_started` then `system.hook_response` JSONL events with
   fields: `hook_id`, `hook_name`, `hook_event`, `exit_code`, `outcome`,
   `stdout`, `stderr`, `output`.
2. **HARD block signature** (orchestrator should match):
   ```
   { type: "system", subtype: "hook_response",
     hook_event: "UserPromptSubmit", exit_code: 2, outcome: "error",
     stderr: "<detail>" }
   ```
   PLUS the subsequent `{ type: "result", num_turns: 0, ... }`.
3. **`is_error=false` is misleading after HARD block.** Claude reports
   `result.is_error=false` even when a UserPromptSubmit hook blocked the
   prompt. Do NOT rely on `is_error`. Use `num_turns === 0` + the matching
   hook_response with `exit_code === 2`.
4. **Process exit code is 0 after HARD block.** No clear OS-level signal —
   the orchestrator MUST parse the stream.
5. **User's environment has at least one hook that exits 49** (outcome=error)
   on UserPromptSubmit even in normal flow. This is a non-blocking
   `outcome=error` and claude continues. Implication for orchestrator: don't
   treat ANY hook failure as fatal — only `exit_code === 2` blocks.
6. **`--bare` would skip all hooks** but also disables OAuth/keychain
   credential reading (per `claude --help`). That breaks S5 finding
   (filesystem-anchored auth). So MoA Desktop CANNOT use `--bare` for the
   Worker — it must accept that user's hooks may run and parse the stream.
7. **`--setting-sources ""`** combined with `--settings <inline-json>`
   provides a clean override — useful for testing and for production if
   MoA Desktop wants to enforce its own minimal settings (recommended for
   Workers to keep behavior deterministic across user machines).

## Recommended Worker stream parser

```typescript
type HookEvent = {
  type: "system";
  subtype: "hook_started" | "hook_response";
  hook_event: "SessionStart" | "UserPromptSubmit" | "PreToolUse" | "PostToolUse" | "Stop";
  exit_code?: number;
  outcome?: "success" | "error";
  stderr?: string;
};

function classifyHookResponse(ev: HookEvent): "ok" | "warn" | "hard-block" {
  if (ev.subtype !== "hook_response") return "ok";
  if (ev.exit_code === 2 && ev.hook_event === "UserPromptSubmit") return "hard-block";
  if (ev.outcome === "error") return "warn";  // log but continue
  return "ok";
}
```

## Validation cmd
```
node D:/moa-desktop/spikes/S6-token-guard.js
```

## Open questions
- Does PreToolUse `exit 2` propagate the same way? (Likely yes, but didn't
  test — lower priority for our Worker since Workers run with
  `--disallowedTools`/sandbox already.)
- Does Stop hook `exit 2` block the response from completing? — TOKEN-GUARD.md
  documents Stop hook exits 0 only; verified consistent with our P1 capture.
