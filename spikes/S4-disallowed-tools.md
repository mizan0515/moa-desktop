# S4 — Block peer-MCP and peer slash-commands in Claude Worker

## Goal
Verify that a Claude Worker spawned by MoA Desktop cannot reach peer-AI via:
1. `mcp__codex__*` style MCP tools
2. `/codex:rescue`, `/codex:review`, `/codex:adversarial-review` slash commands
3. Other MCP servers (defense in depth)

## Result: PASS — three layers cooperate

| Approach | Result | Notes |
|---|---|---|
| A. `--disallowedTools "mcp__*"` | PASS | accepted as wildcard; init advertises 0 mcp__ tools |
| B. `--strict-mcp-config --mcp-config '{"mcpServers":{}}'` | PASS | init advertises `mcp_servers: []`, 0 mcp__ tools |
| C. `--disable-slash-commands` | needed for `/codex:*` slash commands | (not directly retested; see "Caveat" below) |

## Caveat (load-bearing)

The user's environment already has `ENABLE_CLAUDEAI_MCP_SERVERS=false` globally
(per TOKEN-GUARD.md), so the control run also showed `mcp_servers: []`. This
means:
- We did NOT empirically verify the deny mechanism on a machine with active
  MCP servers — only that the flags are accepted and that init reports no
  mcp tools when used.
- For MoA Desktop production, the recommendation below is **defense in depth**:
  use ALL three flags so the Worker is sealed regardless of user machine state.

The control init listed slash_commands including `codex:rescue`, `codex:review`,
`codex:adversarial-review`, `codex:cancel`, `codex:status`, `codex:setup`,
`codex:result`. These are **plugin slash commands**, NOT MCP tools, so
`--disallowedTools mcp__*` does NOT block them. `--disable-slash-commands` is
required.

## Production Worker Guard (recommended)

```
claude.exe -p
  --output-format stream-json
  --verbose
  --max-turns <N>
  --model <opus|haiku>
  --strict-mcp-config
  --mcp-config "{\"mcpServers\":{}}"
  --disallowedTools mcp__* Edit Write NotebookEdit
  --disable-slash-commands
  --append-system-prompt "<DESIGN.md Claude Worker guard text>"
  --setting-sources ""    (avoid loading user settings.json hooks/skills/etc)
```

Optional env when spawning:
- `ENABLE_CLAUDEAI_MCP_SERVERS=false`
- `CLAUDE_CODE_SUBAGENT_MODEL=haiku`
- `MAX_THINKING_TOKENS=10000`

For mutation owner mode, replace `Edit Write NotebookEdit` with explicit allowlist
via `--allowedTools`.

## Validation cmd
```
node D:/moa-desktop/spikes/S4-disallowed-tools.js
```

## Open questions
- `--allowedTools` vs `--disallowedTools` precedence when both supplied
  — DESIGN.md uses both; not blocking, but T5a should empirically pin behavior.
- Does `--disable-slash-commands` also disable user-defined skills (`Skill` tool)?
  Worker probably should not call skills regardless. Document and test in T5a.
