// S4 — --disallowedTools "mcp__*" blocks peer-MCP calls in claude -p
// Run: node spikes/S4-disallowed-tools.js
//
// Verifies the Claude Worker cannot call Codex MCP or any MCP tool when:
//   approach A: --disallowedTools "mcp__*" (wildcard)
//   approach B: --strict-mcp-config --mcp-config '{"mcpServers":{}}'
//   approach C: --disable-slash-commands (also blocks /codex:rescue style)
//
// Test method: prompt claude to call mcp__codex__rescue (or any mcp tool).
// Inspect stream-json events for tool_use blocks. PASS if no mcp__ tool_use
// is permitted (either denied permission or model declines after seeing
// tool list).

const { spawn, execSync } = require("node:child_process");
const fs = require("node:fs");

const isWin = process.platform === "win32";
const CLAUDE = isWin ? "claude.exe" : "claude";

function runClaude(args, prompt, timeoutMs = 60000) {
  return new Promise((resolve) => {
    const child = spawn(CLAUDE, args, {
      stdio: ["pipe", "pipe", "pipe"],
      shell: false,
      windowsHide: true,
    });
    child.stdin.write(prompt);
    child.stdin.end();

    const events = [];
    let buf = "";
    let stderrBuf = "";
    child.stdout.on("data", (chunk) => {
      buf += chunk.toString("utf8");
      let idx;
      while ((idx = buf.indexOf("\n")) >= 0) {
        const line = buf.slice(0, idx);
        buf = buf.slice(idx + 1);
        if (!line.trim()) continue;
        try { events.push(JSON.parse(line)); } catch { events.push({ _raw: line }); }
      }
    });
    child.stderr.on("data", (c) => { stderrBuf += c.toString("utf8"); });

    let exitCode = null;
    child.on("exit", (c) => { exitCode = c; });

    const timer = setTimeout(() => {
      try { execSync(`taskkill /PID ${child.pid} /T /F`, { stdio: "ignore" }); } catch {}
    }, timeoutMs);

    child.on("close", () => {
      clearTimeout(timer);
      resolve({ events, stderr: stderrBuf, exitCode });
    });
  });
}

function inspectForMcpToolUse(events) {
  // Flatten tool_use blocks across messages.
  const toolUses = [];
  const toolResults = [];
  for (const ev of events) {
    if (ev.type === "assistant" && ev.message?.content) {
      for (const block of ev.message.content) {
        if (block.type === "tool_use") {
          toolUses.push({ name: block.name, id: block.id });
        }
      }
    }
    if (ev.type === "user" && ev.message?.content) {
      for (const block of ev.message.content) {
        if (block.type === "tool_result") {
          const isErr = block.is_error === true;
          const text = (Array.isArray(block.content) ? block.content : [block.content])
            .map(c => (typeof c === "string" ? c : (c?.text || "")))
            .join(" ");
          toolResults.push({ tool_use_id: block.tool_use_id, error: isErr, snippet: text.slice(0, 120) });
        }
      }
    }
    if (ev.type === "system" && ev.subtype === "init") {
      // Grab the available tools list announced at init
      ev._availableToolsCount = (ev.tools || []).length;
    }
  }
  return { toolUses, toolResults };
}

(async () => {
  const prompt =
    "Use the tool named mcp__codex__rescue (or any mcp__ tool you have) " +
    "to run a brief task. After you have either run it or determined you " +
    "cannot run it, output the literal text: REPORT_DONE";

  console.log("[S4] === Approach A: --disallowedTools mcp__* ===");
  const aArgs = [
    "-p",
    "--output-format", "stream-json",
    "--verbose",
    "--max-turns", "3",
    "--model", "haiku",
    "--disallowedTools", "mcp__*",
  ];
  const aRes = await runClaude(aArgs, prompt);
  const aInspect = inspectForMcpToolUse(aRes.events);
  const aMcpCalls = aInspect.toolUses.filter(t => t.name?.startsWith("mcp__"));
  const aMcpDenied = aInspect.toolResults.filter(r => r.error && /permission|disallowed|denied/i.test(r.snippet));
  console.log(`[A] events=${aRes.events.length} toolUses=${aInspect.toolUses.length} mcpCalls=${aMcpCalls.length} mcpDenied=${aMcpDenied.length}`);
  if (aMcpCalls.length) console.log(`[A] mcpCalls names: ${aMcpCalls.map(c => c.name).join(", ")}`);
  if (aMcpDenied.length) console.log(`[A] mcpDenied snippets: ${aMcpDenied.map(d => d.snippet).join(" | ")}`);

  console.log("\n[S4] === Approach B: --strict-mcp-config --mcp-config {} ===");
  const bArgs = [
    "-p",
    "--output-format", "stream-json",
    "--verbose",
    "--max-turns", "3",
    "--model", "haiku",
    "--strict-mcp-config",
    "--mcp-config", '{"mcpServers":{}}',
  ];
  const bRes = await runClaude(bArgs, prompt);
  const bInspect = inspectForMcpToolUse(bRes.events);
  const bMcpCalls = bInspect.toolUses.filter(t => t.name?.startsWith("mcp__"));
  console.log(`[B] events=${bRes.events.length} toolUses=${bInspect.toolUses.length} mcpCalls=${bMcpCalls.length}`);

  // Also: did init advertise zero MCP tools?
  const bInit = bRes.events.find(e => e.type === "system" && e.subtype === "init");
  const bMcpToolsInList = bInit?.tools?.filter(n => n.startsWith?.("mcp__")) || [];
  console.log(`[B] init advertised ${bMcpToolsInList.length} mcp__ tools`);

  console.log("\n[S4] ===== RESULT =====");
  const aPass = aMcpCalls.length === 0 || aMcpDenied.length > 0;
  const bPass = bMcpCalls.length === 0 && bMcpToolsInList.length === 0;
  console.log(`[A] --disallowedTools mcp__*: ${aPass ? "PASS (no successful mcp call)" : "FAIL"}`);
  console.log(`[B] --strict-mcp-config + empty mcpServers: ${bPass ? "PASS (zero mcp tools advertised)" : "FAIL"}`);

  // Also dump the init advertisement to confirm
  const aInit = aRes.events.find(e => e.type === "system" && e.subtype === "init");
  const aMcpAdvertised = aInit?.tools?.filter(n => n.startsWith?.("mcp__")) || [];
  console.log(`[A] init advertised ${aMcpAdvertised.length} mcp__ tools (informational)`);

  process.exit((aPass && bPass) ? 0 : 1);
})();
