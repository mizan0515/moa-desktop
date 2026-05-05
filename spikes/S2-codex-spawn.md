# S2 — codex exec spawn + JSON stream + sandbox semantics (Windows)

## Goal
1. Verify `codex exec --json --sandbox <mode>` spawns from Node and streams JSONL.
2. Confirm `read-only` blocks mutation.
3. Confirm `workspace-write` allows mutation. **(Windows: this turned out NOT to be true with default settings — see findings.)**

## Result: PARTIAL PASS (with critical finding)

| Check | Result |
|---|---|
| Argv-array spawn (no shell) | PASS — must resolve to native `codex.exe`, not `.cmd` |
| JSONL streaming | PASS — events emit line-by-line on stdout, schema = `thread.started`, `turn.started`, `item.started`/`completed`, `turn.completed`/`failed` |
| `--sandbox read-only` blocks writes | PASS — file never created |
| `--sandbox workspace-write` allows writes | **FAIL on Windows** — codex's command-policy layer still rejects PowerShell `Set-Content` with `rejected: blocked by policy` |
| `--dangerously-bypass-approvals-and-sandbox` allows writes | PASS — file created (4B `mutation-test.txt`) |

## Critical findings (load-bearing for T1/T5b)

### 1. Native `.exe` resolution required
`codex` on Windows ships as an npm package with a `.cmd` wrapper that calls
`node codex.js` which dispatches to a platform-specific binary at
`C:/Users/<user>/AppData/Roaming/npm/node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/codex/codex.exe`.
Tauri must resolve this directly (Node 18+ blocks `.cmd` spawn with shell:false).

### 2. `[features].web_search=true` is deprecated
The form `-c tools.web_search=true` from PLAN.md F1 / DESIGN.md emits a deprecation
event. New form: top-level `-c web_search="live"` / `"cached"` / `"disabled"`.
Spike used `"disabled"` to keep cost minimal.

### 3. `model_reasoning_effort="minimal"` incompatible with web_search/image_gen
API returns `400 invalid_request_error` with `"The following tools cannot be used
with reasoning.effort 'minimal': image_gen, web_search."`. Use `"low"` or higher.
For our adapter: just don't expose `minimal` as an option.

### 4. **Global `~/.codex/AGENTS.md` injects forced preflight (BIGGEST GOTCHA)**
The user's `~/.codex/AGENTS.md` contains a "Codex MCP Windows Environment Guard"
that forces every Codex turn to first run a PowerShell preflight
(`Get-Location; ...; Get-Command pwsh,cmd,git`). This applies even to direct
`codex exec` calls because Codex loads `$CODEX_HOME/AGENTS.md` regardless.

`--ignore-rules` does NOT skip AGENTS.md (it's only for `.rules` files).
`--ignore-user-config` only skips `config.toml`, not `AGENTS.md`.

**Fix for MoA Desktop**: spawn Codex Worker with `CODEX_HOME` pointing to a
dedicated, minimal directory (e.g. `~/.moa-desktop/codex-home/`) that contains
only a copy of `auth.json` and a Worker-specific `AGENTS.md` with our own guard
text (DESIGN.md "Codex Worker guard"). This isolates the Worker from
user-machine instructions.

### 5. **`workspace-write` is broken on Windows for PowerShell writes**
With `--sandbox workspace-write` + `approval_policy="never"`, codex's command
policy layer still rejects `pwsh.exe -Command "Set-Content ..."` with
`rejected: blocked by policy`. The model also reports the session as "read-only"
in its narration despite the CLI flag.

`--dangerously-bypass-approvals-and-sandbox` is required on Windows. The user's
own `~/.codex/config.toml` confirms this: `sandbox_mode = "danger-full-access"`.

**Implication for MoA Desktop F4 (mutation = isolated worktree)**:
Codex mutation-owner mode on Windows MUST use
`--dangerously-bypass-approvals-and-sandbox` and rely on the worktree boundary
for safety, not Codex's sandbox. Blast radius is contained because:
- worktree is in a temp/dedicated path under `~/.moa-desktop/worktrees/<sid>/`
- final patch is reviewed before apply (DESIGN.md F4)
- patch apply is to main repo only on user confirm

### 6. Codex spawns helper child processes (descendants)
Even on a failed turn, `taskkill /T /F` cleanup reported 6+ descendant PIDs.
Process-tree-kill (S7) is mandatory.

### 7. Stderr noise
- "Reading additional input from stdin..." prints when stdin is `ignore` (harmless).
- `WARNING: proceeding, even though we could not update PATH: Refusing to create
  helper binaries under temporary dir` when `CODEX_HOME` is in `%TEMP%` — must
  use a non-temp directory in production (e.g. `~/.moa-desktop/codex-home/`).

## Confirmed working argv (read-only first-pass)

```
codex.exe (native)
  exec
  --ephemeral
  -c approval_policy="never"
  -c model_reasoning_effort="high"
  -c web_search="live"
  --sandbox read-only
  --json
  --cd <repo>
  --skip-git-repo-check
  <prompt>
```

`CODEX_HOME` env: `~/.moa-desktop/codex-home/` (non-temp, contains auth.json + worker AGENTS.md).
Stdin: ignore (or pipe + close immediately).

## Confirmed working argv (mutation owner — Windows)

```
codex.exe (native)
  exec
  --ephemeral
  -c approval_policy="never"
  -c model_reasoning_effort="high"
  -c web_search="live"
  --dangerously-bypass-approvals-and-sandbox
  --json
  --cd <isolated-worktree>
  --skip-git-repo-check
  <prompt>
```

⚠️ `--dangerously-bypass-approvals-and-sandbox` only inside `--cd <isolated-worktree>`
that the orchestrator created via `git worktree add`. Never use for first-pass.

## Validation cmd
```
node D:/moa-desktop/spikes/S2-codex-spawn.js
```

## Open questions
- `web_search="cached"` vs `"live"` cost trade-off (deferred to T5b).
- Does `--add-dir` widen workspace-write usefully on Linux/macOS?
  (Spike didn't test — Windows-first project.)
- Codex policy engine source: is the "blocked by policy" rule for pwsh-write
  configurable via `execpolicy`, or hardcoded? (Worth investigating in T5b
  if `--dangerously-bypass` proves too risky.)
