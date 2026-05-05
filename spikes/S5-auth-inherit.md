# S5 — Auth/env inheritance

## Goal
Verify a Tauri-spawned child can authenticate without env-injected API keys.

## Result: PASS

```
~/.claude/.credentials.json exists: true (note dotted name)
~/.codex/auth.json exists:           true
env passed to children:              PATH, USERPROFILE, APPDATA, LOCALAPPDATA, SystemRoot, TEMP, TMP
ANTHROPIC_API_KEY in env:            false
OPENAI_API_KEY in env:               false
claude assistant/result events:       received → auth OK
codex thread.started + turn.completed: received → auth OK
```

## Findings

1. **Claude credential file is `~/.claude/.credentials.json` (dotted).**
   DESIGN.md `(line 64)` writes "credentials.json" without the dot — that's a
   doc bug. Update DESIGN.md or just keep this note pinned for ticket
   reference. The actual file is `.credentials.json` and Claude reads it
   automatically when `USERPROFILE` env is set.
2. **Codex auth: `~/.codex/auth.json` (no dot).** Read via `$CODEX_HOME` env
   (default `~/.codex`). MoA Desktop will use a dedicated `CODEX_HOME` per
   S2 finding to skip the user's global AGENTS.md preflight.
3. **No env-injected keys needed.** A minimal env (`PATH`, `USERPROFILE`,
   `APPDATA`, `LOCALAPPDATA`, `SystemRoot`, `TEMP`, `TMP`) is sufficient.
   The CLIs resolve credentials via filesystem, anchored on `USERPROFILE`.
4. **`ENABLE_CLAUDEAI_MCP_SERVERS=false`** does NOT come through implicitly —
   if MoA wants this for prompt-cache hygiene, set it explicitly per Worker.

## Tauri implication

```rust
let mut cmd = Command::new("claude.exe");
cmd.envs([
    ("PATH", path),
    ("USERPROFILE", home),
    ("APPDATA", appdata),
    ("LOCALAPPDATA", localappdata),
    ("SystemRoot", system_root),
    ("TEMP", temp),
    ("TMP", temp),
    ("ENABLE_CLAUDEAI_MCP_SERVERS", "false"),
    ("CLAUDE_CODE_SUBAGENT_MODEL", "haiku"),
    ("MAX_THINKING_TOKENS", "10000"),
]);
// no ANTHROPIC_API_KEY — credentials come from ~/.claude/.credentials.json
```

For Codex Worker:
```rust
cmd.env("CODEX_HOME", "~/.moa-desktop/codex-home");
// ~/.moa-desktop/codex-home/auth.json must be a copy of ~/.codex/auth.json,
// kept fresh by orchestrator on session start.
```

## Validation cmd
```
node D:/moa-desktop/spikes/S5-auth-inherit.js
```

## Open questions
- Token refresh: if `~/.codex/auth.json` is refreshed by Codex Desktop's running
  session, the **copied** copy in `~/.moa-desktop/codex-home/auth.json` goes
  stale. Orchestrator should re-copy on each session start (simple) or
  symlink/junction (Windows: `mklink /J`). Tabled to T7-full preflight.
- Claude `.credentials.json` is **not** dotted in DESIGN.md `(line 64)`
  — recommend updating DESIGN.md before T1.
