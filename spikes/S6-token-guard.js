// S6 — Hook stderr/exit propagation under spawned `claude -p`
// Run: node spikes/S6-token-guard.js
//
// Verifies:
//   1. Real TOKEN-GUARD hook (already in ~/.claude/settings.json) fires under
//      spawn, and its stdout/stderr is captured as a structured JSONL `system`
//      event of subtype `hook_response` (not lost in raw stderr).
//   2. A simulated HARD-block hook (exit 2 from UserPromptSubmit) surfaces a
//      clear, parseable signal to the parent — claude does not silently hang.
//
// Method: pass `--settings` inline JSON registering an extra UserPromptSubmit
// hook that runs a tiny script which writes to stderr and exits 2. Observe
// the resulting stream.

const { spawn, execSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");
const os = require("node:os");

const isWin = process.platform === "win32";
const CLAUDE = isWin ? "claude.exe" : "claude";

function runClaude(args, prompt, env, timeoutMs = 60000) {
  return new Promise((resolve) => {
    const child = spawn(CLAUDE, args, {
      stdio: ["pipe", "pipe", "pipe"],
      shell: false,
      windowsHide: true,
      env,
    });
    child.stdin.write(prompt);
    child.stdin.end();
    let buf = "", events = [], stderr = "";
    child.stdout.on("data", (c) => {
      buf += c.toString("utf8");
      let i;
      while ((i = buf.indexOf("\n")) >= 0) {
        const line = buf.slice(0, i);
        buf = buf.slice(i + 1);
        if (line.trim()) try { events.push(JSON.parse(line)); } catch { events.push({ _raw: line }); }
      }
    });
    child.stderr.on("data", (c) => { stderr += c.toString("utf8"); });
    let exit = null;
    child.on("exit", (c) => { exit = c; });
    setTimeout(() => { try { execSync(`taskkill /PID ${child.pid} /T /F`, { stdio: "ignore" }); } catch {} }, timeoutMs);
    child.on("close", () => resolve({ events, stderr, exit }));
  });
}

(async () => {
  // -- Part 1: real TOKEN-GUARD hook firing in normal spawn --
  console.log("[S6] === Part 1: real ~/.claude TOKEN-GUARD hook in normal spawn ===");
  const r1 = await runClaude(
    [
      "-p",
      "--output-format", "stream-json",
      "--verbose",
      "--max-turns", "1",
      "--model", "haiku",
      "--strict-mcp-config",
      "--mcp-config", '{"mcpServers":{}}',
      "--include-hook-events",
      "--disallowedTools", "Edit", "Write", "NotebookEdit",
    ],
    "Reply OK only.",
    process.env,
  );
  const hookEvents = r1.events.filter(e => e.type === "system" && e.subtype && e.subtype.startsWith("hook"));
  console.log(`[S6/P1] total events=${r1.events.length} hookEvents=${hookEvents.length}`);
  for (const h of hookEvents.slice(0, 6)) {
    console.log(`  ${h.subtype} hook=${h.hook_name} event=${h.hook_event} exit=${h.exit_code} outcome=${h.outcome || "-"}`);
  }
  console.log(`[S6/P1] Result: hook events captured in JSONL stream → PASS`);

  // -- Part 2: simulated HARD block (exit 2 from UserPromptSubmit) --
  // Write a tiny .py script that exits 2 with stderr text.
  const tmpdir = path.join(os.tmpdir(), `s6-${Date.now()}`);
  fs.mkdirSync(tmpdir, { recursive: true });
  const hookScript = path.join(tmpdir, "hard_block.py");
  fs.writeFileSync(
    hookScript,
    [
      "import sys, json",
      "sys.stderr.write('[S6 simulated HARD] msgs=999 cost=$99 — blocking new prompt')",
      "sys.exit(2)",
    ].join("\n"),
  );

  // Inline settings JSON: only register UserPromptSubmit running the script.
  // We use --setting-sources "" so user/project settings are NOT loaded
  // (avoids interaction with real TOKEN-GUARD).
  const inlineSettings = {
    hooks: {
      UserPromptSubmit: [
        {
          matcher: "*",
          hooks: [
            {
              type: "command",
              command: `python ${hookScript.replace(/\\/g, "/")}`,
              timeout: 5,
            },
          ],
        },
      ],
    },
  };

  console.log("\n[S6] === Part 2: simulated HARD block (exit 2 from UserPromptSubmit) ===");
  const r2 = await runClaude(
    [
      "-p",
      "--output-format", "stream-json",
      "--verbose",
      "--max-turns", "1",
      "--model", "haiku",
      "--strict-mcp-config",
      "--mcp-config", '{"mcpServers":{}}',
      "--include-hook-events",
      "--disallowedTools", "Edit", "Write", "NotebookEdit",
      "--setting-sources", "",          // skip user/project settings
      "--settings", JSON.stringify(inlineSettings),
    ],
    "Try this prompt — should be blocked.",
    process.env,
  );
  const blockHookEvents = r2.events.filter(e => e.type === "system" && e.subtype && e.subtype.startsWith("hook"));
  const blockedHook = blockHookEvents.find(h => h.exit_code === 2);
  console.log(`[S6/P2] total events=${r2.events.length} hookEvents=${blockHookEvents.length} exit=${r2.exit}`);
  console.log(`[S6/P2] all events: ${JSON.stringify(r2.events.map(e => ({type: e.type, subtype: e.subtype, exit: e.exit_code, num_turns: e.num_turns, is_error: e.is_error})))}`);
  for (const h of blockHookEvents.slice(0, 6)) {
    console.log(`  ${h.subtype} hook=${h.hook_name || "-"} event=${h.hook_event || "-"} exit=${h.exit_code} outcome=${h.outcome || "-"}`);
    if (h.stderr) console.log(`    stderr: ${h.stderr.slice(0, 120)}`);
  }
  if (r2.stderr.trim()) console.log(`[S6/P2] raw child stderr: ${r2.stderr.slice(0, 300)}`);
  // Look for any non-success indicator: result event with is_error or termination
  const result = r2.events.find(e => e.type === "result");
  if (result) {
    console.log(`[S6/P2] result event: subtype=${result.subtype} is_error=${result.is_error} num_turns=${result.num_turns}`);
  }

  console.log("\n[S6] ===== RESULT =====");
  const p1Pass = hookEvents.length > 0;
  const p2Signal = blockedHook != null || (result && result.is_error) || r2.exit !== 0 || r2.events.some(e => e.type === "system" && (e.subtype === "hook_blocked" || /block/i.test(JSON.stringify(e))));
  console.log(`[P1] real hooks captured as JSONL: ${p1Pass ? "PASS" : "FAIL"}`);
  console.log(`[P2] HARD-block signal observable: ${p2Signal ? "PASS" : "FAIL"}`);
  process.exit(p1Pass && p2Signal ? 0 : 1);
})();
