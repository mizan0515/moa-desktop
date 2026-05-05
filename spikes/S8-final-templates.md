# S8 — Final argv-array command templates (Windows-verified)

Authority for `commandTemplate` field in `~/.moa-desktop/settings.json`.
All templates expressed as **argv arrays** (no PowerShell quoting). Tauri v2
`Command::new(program).args(...)` consumes these directly.

---

## Claude Worker — read-only first-pass

### program (resolved at startup)
```
where claude.exe   →   C:\Users\<user>\.local\bin\claude.exe   (or PATH-resolved equivalent)
```

### argv (Tauri shape)
```json
[
  "-p",
  "--output-format", "stream-json",
  "--verbose",
  "--include-hook-events",
  "--max-turns", "20",
  "--model", "opus",
  "--strict-mcp-config",
  "--mcp-config", "{\"mcpServers\":{}}",
  "--disable-slash-commands",
  "--disallowedTools", "mcp__*", "Edit", "Write", "NotebookEdit",
  "--allowedTools", "Read", "Glob", "Grep", "WebSearch", "WebFetch", "Bash(git status:*)", "Bash(git log:*)", "Bash(git diff:*)", "Bash(rg:*)",
  "--append-system-prompt", "<DESIGN.md Claude Worker guard text>",
  "--setting-sources", ""
]
```

### stdin
- write `<prompt>` then close stdin

### env
```
PATH, USERPROFILE, APPDATA, LOCALAPPDATA, SystemRoot, TEMP, TMP
ENABLE_CLAUDEAI_MCP_SERVERS=false
CLAUDE_CODE_SUBAGENT_MODEL=haiku
MAX_THINKING_TOKENS=10000
```
No `ANTHROPIC_API_KEY` — auth comes from `~/.claude/.credentials.json`.

### cwd
session worktree root (read-only mode does not need write access; orchestrator
points cwd at the worktree so the model has correct context).

---

## Claude Worker — mutation owner

Same as above, but replace tool flags with:

```json
{
  "remove": ["--disallowedTools", "mcp__*", "Edit", "Write", "NotebookEdit"],
  "add": [
    "--permission-mode", "acceptEdits",
    "--allowedTools", "Read", "Edit", "Write", "Glob", "Grep", "WebSearch", "WebFetch", "Bash(git status:*)", "Bash(git diff:*)", "Bash(npm test:*)", "Bash(pytest:*)", "Bash(cargo test:*)", "Bash(rg:*)",
    "--max-turns", "30"
  ]
}
```

Cwd: dedicated worktree under `~/.moa-desktop/worktrees/<sid>/`.

---

## Codex Worker — read-only first-pass

### program (resolved at startup; do NOT spawn `codex.cmd` — Node EINVAL)
```
candidates (probe in order, use first existing):
  C:/Users/<user>/AppData/Roaming/npm/node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/codex/codex.exe
  C:/Users/<user>/AppData/Local/Programs/codex/codex.exe   (future installer location, if added)
```

If none found → preflight error `cli-missing` (DESIGN.md F6 error class).

### argv
```json
[
  "exec",
  "--ephemeral",
  "-c", "approval_policy=\"never\"",
  "-c", "model_reasoning_effort=\"high\"",
  "-c", "web_search=\"live\"",
  "--sandbox", "read-only",
  "--json",
  "--cd", "<worktree-or-repo-cwd>",
  "--skip-git-repo-check",
  "<prompt>"
]
```

### stdin
- close immediately (`child.stdin.end()`). Otherwise codex prints
  `Reading additional input from stdin...` and waits forever.

### env
```
PATH, USERPROFILE, APPDATA, LOCALAPPDATA, SystemRoot, TEMP, TMP
CODEX_HOME=<HOME>/.moa-desktop/codex-home   (NOT a temp dir — codex refuses)
```
No `OPENAI_API_KEY` — auth from `<CODEX_HOME>/auth.json` (orchestrator ensures
this is a fresh copy of `~/.codex/auth.json` at session start).

`<HOME>/.moa-desktop/codex-home/AGENTS.md` should exist with MoA Desktop's
own Codex Worker guard text (DESIGN.md). This isolates the Worker from the
user's global `~/.codex/AGENTS.md` (which forces a Windows preflight that
breaks under sandbox — see S2 finding #4).

### cwd
session worktree root (set OS cwd AND `--cd` to the same path — defensive).

---

## Codex Worker — mutation owner (Windows)

⚠️ **Windows-specific**: `--sandbox workspace-write` is broken on Windows
(codex policy engine still rejects pwsh writes — see S2 finding #5).
The mutation owner template MUST use `--dangerously-bypass-approvals-and-sandbox`
inside an isolated worktree. Blast radius is contained because the worktree is
under `~/.moa-desktop/worktrees/<sid>/`, never the main repo.

### argv
```json
[
  "exec",
  "--ephemeral",
  "-c", "approval_policy=\"never\"",
  "-c", "model_reasoning_effort=\"high\"",
  "-c", "web_search=\"live\"",
  "--dangerously-bypass-approvals-and-sandbox",
  "--json",
  "--cd", "<isolated-worktree-path>",
  "--skip-git-repo-check",
  "<prompt>"
]
```

NEVER use `--dangerously-bypass-approvals-and-sandbox` outside an isolated
worktree. Orchestrator MUST verify cwd is under `~/.moa-desktop/worktrees/`
before constructing this template.

---

## Cancellation (both Workers)

Tauri `kill_tree(pid)` =
```
Windows: spawn `taskkill /T /F /PID <pid>` and wait.
Unix:    process::kill(-pid, SIGKILL) (requires child spawned with setpgid).
```

Always `/T /F`.

---

## Stream parser — common JSONL events

| Worker | Event types observed |
|---|---|
| Claude | `system.hook_started`, `system.hook_response`, `system.init`, `assistant`, `user` (tool_result), `result` |
| Codex  | `thread.started`, `turn.started`, `item.started`, `item.completed`, `turn.completed`, `turn.failed`, `error` |

Critical signals:
- Claude HARD-block: `system.hook_response{exit_code:2, hook_event:"UserPromptSubmit"}` + subsequent `result{num_turns:0}`. Do NOT rely on `result.is_error` — it is `false` even after block.
- Codex turn end: `turn.completed` (success) or `turn.failed` (with `error.message`).
- Codex non-blocking warnings (e.g., `[features].web_search deprecated`) emit as `item.completed{type:"error"}` — log but continue.

---

## Settings template (`~/.moa-desktop/settings.json`)

```json
{
  "claude": {
    "enabled": true,
    "exePath": "auto",
    "argvFirstPass": [
      "-p", "--output-format", "stream-json", "--verbose",
      "--include-hook-events", "--max-turns", "20", "--model", "opus",
      "--strict-mcp-config", "--mcp-config", "{\"mcpServers\":{}}",
      "--disable-slash-commands",
      "--disallowedTools", "mcp__*", "Edit", "Write", "NotebookEdit",
      "--allowedTools", "Read", "Glob", "Grep", "WebSearch", "WebFetch",
        "Bash(git status:*)", "Bash(git log:*)", "Bash(git diff:*)", "Bash(rg:*)",
      "--setting-sources", ""
    ],
    "argvMutation": ["...similar with permission-mode acceptEdits + Edit Write allowed..."],
    "stdin": "prompt",
    "env": {
      "ENABLE_CLAUDEAI_MCP_SERVERS": "false",
      "CLAUDE_CODE_SUBAGENT_MODEL": "haiku",
      "MAX_THINKING_TOKENS": "10000"
    },
    "envInherit": ["PATH", "USERPROFILE", "APPDATA", "LOCALAPPDATA", "SystemRoot", "TEMP", "TMP"]
  },
  "codex": {
    "enabled": true,
    "exePathCandidates": [
      "{npmGlobal}/node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/codex/codex.exe"
    ],
    "argvFirstPass": [
      "exec", "--ephemeral",
      "-c", "approval_policy=\"never\"",
      "-c", "model_reasoning_effort=\"high\"",
      "-c", "web_search=\"live\"",
      "--sandbox", "read-only", "--json",
      "--cd", "{cwd}", "--skip-git-repo-check",
      "{prompt}"
    ],
    "argvMutation": [
      "exec", "--ephemeral",
      "-c", "approval_policy=\"never\"",
      "-c", "model_reasoning_effort=\"high\"",
      "-c", "web_search=\"live\"",
      "--dangerously-bypass-approvals-and-sandbox",
      "--json", "--cd", "{worktree}", "--skip-git-repo-check",
      "{prompt}"
    ],
    "stdin": "close-immediately",
    "env": {
      "CODEX_HOME": "{home}/.moa-desktop/codex-home"
    },
    "envInherit": ["PATH", "USERPROFILE", "APPDATA", "LOCALAPPDATA", "SystemRoot", "TEMP", "TMP"],
    "codexHomeBootstrap": {
      "copyAuthFrom": "{home}/.codex/auth.json",
      "writeAgentsMd": "<DESIGN.md Codex Worker guard text>"
    }
  }
}
```

`{cwd}`, `{worktree}`, `{prompt}`, `{home}`, `{npmGlobal}` are tokens
substituted by the orchestrator at spawn time — kept as separate argv
elements (NOT substituted into a quoted shell string).
