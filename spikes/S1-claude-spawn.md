# S1 — claude -p spawn + stream-json + kill

## Goal
Spawn `claude -p` from a Node parent (Tauri-equivalent), receive JSONL stdout
line-by-line, kill before completion, verify no zombies.

## Result: PASS

```
firstLineMs       = 1154ms
linesReceived     = 3 (all type=system, line-delimited JSON)
exitCode          = 1 (caused by taskkill /T /F — expected on Windows)
descendantsBefore = 0 (claude.exe runs in-process, no children spawned for this prompt)
stillAliveAfter   = 0
```

## Findings (load-bearing)

1. **Use `claude.exe`, NOT `claude.cmd`.** Node 18+ blocks `.cmd` spawn with
   `shell: false` (CVE-2024-27980 → `EINVAL`). Tauri v2 `Command::new` follows
   the same Rust safe-spawn rules — argv-array safety only with `.exe`.
   Resolved via `where claude` → `C:\Users\mizan\.local\bin\claude.exe`.
2. **Pass prompt via stdin, not argv.** `--allowedTools`/`--disallowedTools` are
   variadic (`<tools...>`) and absorb subsequent positional args. Putting the
   prompt as the last argv slot causes claude to error
   `Input must be provided either through stdin or as a prompt argument`
   because the prompt gets eaten as a tool name.
3. **`--output-format stream-json` requires `--verbose`.** Without `--verbose`
   claude exits with arg-parse error. Confirmed via empirical run.
4. **JSONL streaming works.** Each line is one JSON event (e.g. `{"type":"system", ...}`).
   Parser must split on `\n` and accumulate partial chunks in a buffer (already
   modeled in `S1-claude-spawn.js:87-105`).
5. **`taskkill /T /F` cleanly terminates the spawn.** No zombies even with
   2-3 child processes; Windows job semantics + descendant walk via
   `Get-CimInstance Win32_Process -Filter "ParentProcessId=..."` confirms.

## Tauri implication

```rust
// pseudo-Tauri
let mut cmd = tauri::api::process::Command::new("claude.exe");
cmd.args([
  "-p",
  "--output-format", "stream-json",
  "--verbose",
  "--max-turns", "20",
  "--model", "opus",
  "--disallowedTools", "Edit", "Write", "NotebookEdit",
  // ...
]);
let (mut rx, child) = cmd.spawn()?;
child.write(prompt.as_bytes())?; // stdin
child.close_stdin()?;
```

For cancellation, on Windows the equivalent of `taskkill /T /F` is needed
(see S7).

## Validation cmd
```
node D:/moa-desktop/spikes/S1-claude-spawn.js
```

## Open questions
- exit code 1 is from kill; what is the natural exit code on success? (deferred — not blocking; out of scope for S1)
- stream-json schema documentation is sparse. Empirically observe `type=system`,
  `type=assistant`, `type=result`. T5a will need to enumerate.
