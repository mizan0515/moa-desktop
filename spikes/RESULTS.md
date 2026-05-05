# T0 Spike — Results matrix

Date: 2026-05-06
Branch: `feat/T0-spike`
Environment: Windows 11 Pro 10.0.26200, Node v24.15.0, claude-cli 2.1.126,
codex-cli 0.128.0

## PASS/FAIL summary

| # | Spike | Verdict | One-line outcome |
|---|---|---|---|
| S1 | `claude -p` Tauri spawn + stream-json | ✅ PASS | Use `claude.exe` (not `.cmd`); prompt via stdin; argv-array safe; tree-kill works |
| S2 | `codex exec` sandbox read-only/workspace-write | ⚠️ PASS w/ caveat | read-only blocks ✓; workspace-write BROKEN on Windows → must use `--dangerously-bypass-approvals-and-sandbox` inside isolated worktree |
| S3 | Parallel two-worker | ✅ PASS | Pipe-isolated, no line interleaving, kill of one ≠ cascade |
| S4 | `--disallowedTools mcp__*` blocks peer | ✅ PASS | Plus `--strict-mcp-config + empty mcpServers` + `--disable-slash-commands` for defense in depth |
| S5 | Auth/env inheritance | ✅ PASS | Filesystem auth (`.credentials.json`, `auth.json`) — no env keys needed; minimal env (PATH+USERPROFILE…) sufficient |
| S6 | TOKEN-GUARD hook propagation | ✅ PASS | `--include-hook-events` exposes `system.hook_response` JSONL with `exit_code` — HARD block detectable via `exit_code:2` + `result.num_turns:0` |
| S7 | Cancellation = process tree kill | ✅ PASS | `taskkill /T /F /PID <root>` cleans codex's 1–2 helper descendants reliably |
| S8 | Argv-array templates | ✅ DONE | See [S8-final-templates.md](S8-final-templates.md) |

## Major load-bearing findings

1. **`codex.cmd` cannot be spawned with `shell: false`** (Node 18+ EINVAL,
   CVE-2024-27980). Resolve to native `codex.exe` inside the platform
   optional-dep tree.
2. **`claude.cmd` likewise** — use `claude.exe`. Tauri v2 `Command::new`
   follows the same Rust safe-spawn rule.
3. **Pass prompt via stdin, not argv** for Claude. `--allowedTools`
   /`--disallowedTools` are variadic and absorb subsequent positional args.
4. **Close codex stdin immediately** or it prints `Reading additional input
   from stdin...` and waits forever.
5. **PLAN.md F1 needs update**: the form `-c tools.web_search=true` is
   deprecated. New form: top-level `-c web_search="live"|"cached"|"disabled"`.
   `--reasoning-effort` flag is unsupported (must use `-c
   model_reasoning_effort="high"`).
6. **`model_reasoning_effort="minimal"`** is API-incompatible with
   `web_search` and `image_gen` tools (HTTP 400). Use `"low"` or higher.
7. **`workspace-write` sandbox is unreliable on Windows** with default
   policy. Codex's command-policy engine still rejects PowerShell writes
   with `rejected: blocked by policy`. Mutation owner must use
   `--dangerously-bypass-approvals-and-sandbox` inside an isolated worktree
   (DESIGN.md F4 covers this — adversarial F3 sandbox-defense-in-depth idea
   loses one layer on Windows).
8. **Global `~/.codex/AGENTS.md` injects forced PowerShell preflight**
   (user's "Codex MCP Windows Environment Guard"). This makes Codex bail
   under sandbox with `ENV_BLOCKED`. Fix: use a dedicated `CODEX_HOME` for
   Workers (`~/.moa-desktop/codex-home/`) with only a copy of `auth.json`
   and a Worker-specific `AGENTS.md` (DESIGN.md guard text). `--ignore-rules`
   does NOT skip AGENTS.md — only `.rules` files. `--ignore-user-config`
   skips only `config.toml`.
9. **Auth via filesystem, not env**. `~/.claude/.credentials.json` (note
   leading dot — DESIGN.md line 64 has a typo, says `credentials.json`).
   `~/.codex/auth.json` (no dot, default `$CODEX_HOME`).
10. **`--include-hook-events`** is required to see `system.hook_started`/
    `system.hook_response` events in the JSONL stream. Without it, a hook
    that exits 2 silently blocks the prompt and `result.num_turns:0` is the
    only signal. `result.is_error` is `false` even when blocked.
11. **`--bare` is incompatible with filesystem auth** — `--bare` mode
    requires `ANTHROPIC_API_KEY` (no OAuth/keychain). MoA Desktop cannot use
    `--bare` for the Worker; must accept user hooks may fire and parse the
    JSONL stream to interpret outcomes.
12. **`taskkill /T /F`** is the safe cancellation path. Codex spawns 1–2
    helper descendants; without `/T` they MAY linger.

## PLAN.md / DESIGN.md update recommendations (NOT done in this spike)

The ticket scopes Worker mutation to `spikes/`. The following authority-doc
changes are recommended but are out of scope here — flag them in T1
ticket discussion:

- DESIGN.md `(line 64)`: `credentials.json` → `.credentials.json`.
- DESIGN.md `(line 120-121)`: Codex first-pass template — replace
  `-c tools.web_search=true` with `-c web_search="live"`.
- DESIGN.md `(line 124-125)`: Codex mutation template on **Windows** — replace
  `--sandbox workspace-write` with `--dangerously-bypass-approvals-and-sandbox`
  AND require isolated worktree cwd.
- PLAN.md § F1: same updates as DESIGN.md above. Add note about
  `--reasoning-effort` not being supported (already noted in F1 — keep).
- PLAN.md § F1: add a CODEX_HOME bootstrap step (auth.json copy + Worker
  AGENTS.md) to preflight.
- PLAN.md § 0.5 step 1 (preflight): include "resolve native codex.exe path"
  and "ensure CODEX_HOME bootstrapped".

## Phase 1 GO / NO-GO

**GO.** All 8 spikes pass with caveats that are documented and actionable.
No spike requires PLAN architecture rework — only template/config
adjustments captured in S8.

## T1 worker — one-line spike conclusion

> **Resolve native `claude.exe` and `codex.exe` (skip `.cmd` wrappers), spawn
> with stdin-piped prompt for Claude / immediately-closed stdin for Codex,
> bootstrap a dedicated `CODEX_HOME` with copied `auth.json` + Worker
> `AGENTS.md`, parse stream-json including `--include-hook-events`, and
> cancel via `taskkill /T /F /PID`.**

See [S8-final-templates.md](S8-final-templates.md) for the full settings.json
shape.

## Spike artifacts

- [S1-claude-spawn.md](S1-claude-spawn.md) + [S1-claude-spawn.js](S1-claude-spawn.js)
- [S2-codex-spawn.md](S2-codex-spawn.md) + [S2-codex-spawn.js](S2-codex-spawn.js)
- [S3-parallel.md](S3-parallel.md) + [S3-parallel.js](S3-parallel.js)
- [S4-disallowed-tools.md](S4-disallowed-tools.md) + [S4-disallowed-tools.js](S4-disallowed-tools.js)
- [S5-auth-inherit.md](S5-auth-inherit.md) + [S5-auth-inherit.js](S5-auth-inherit.js)
- [S6-token-guard.md](S6-token-guard.md) + [S6-token-guard.js](S6-token-guard.js)
- [S7-cancellation.md](S7-cancellation.md) + [S7-cancellation.js](S7-cancellation.js)
- [S8-final-templates.md](S8-final-templates.md)
