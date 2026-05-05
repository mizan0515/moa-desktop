# S3 — parallel two-worker spawn (Claude + Codex)

## Goal
Spawn Claude Worker + Codex Worker concurrently from a single Node parent.
Verify:
1. Both stream JSONL independently
2. No line interleaving across pipes (each line is a complete JSON object)
3. Killing one does not cascade to the other
4. Each in its own cwd → no file race

## Result: PASS

```
claude events    = 3 (parseFails=0, firstMs=1068)
codex events     = 2 (parseFails=0, firstMs=129)
                   types: [thread.started, turn.started, item.completed, turn.completed]
PIDs separate                  : true
codex turn.completed naturally  : true
isolation (kill ≠ cascade)      : PASS
```

## Findings

1. **Pipe-isolation is OS-guaranteed.** Each child has its own stdout pipe;
   parent reads independently. No risk of stdout collision in the parent.
2. **Per-line JSON integrity preserved.** 0 parse failures across both
   streams under concurrent flush. The newline-delimited framing is robust.
3. **No process group / job object linking.** The Node parent does not place
   children in a single Windows job object, so killing one (`taskkill /T /F`)
   does NOT affect the other. (Tauri equivalent: `Command::new` does not
   auto-group children — each spawn is independent.)
4. **Cwd isolation prevents file race.** With `cwd: <unique-temp-dir>` per
   worker, no shared FS surface. MoA Desktop's worktree-per-Worker pattern
   (DESIGN.md F4) makes this even stronger.
5. **Codex's stdin must be closed**. If left open, codex prints
   `Reading additional input from stdin...` and waits forever. Tauri must
   `child.write_stdin("")` + `child.close_stdin()` immediately after spawn.

## Implications for orchestrator (T7)

- One `spawn` per Worker, no shared state inheritance.
- Distinct `cwd` (worktree path) per Worker.
- Per-worker JSONL event channel (Tauri `Event` API or Tokio `mpsc` per child).
- Cancellation API can target one Worker without affecting the other.

## Validation cmd
```
node D:/moa-desktop/spikes/S3-parallel.js
```

## Open questions
- None blocking. Production concern: Tauri v2 `Command` events are global by
  default — the adapter must tag emit events with `worker_id` to distinguish
  streams. Caught in T2 design.
