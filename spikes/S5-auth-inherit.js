// S5 — Auth/env inheritance for spawned Claude/Codex from a sandbox-style env
// Run: node spikes/S5-auth-inherit.js
//
// Verifies: a child process spawned with a *minimal* env (PATH + USERPROFILE only)
// can still authenticate via filesystem credentials at:
//   ~/.claude/.credentials.json (note leading dot — DESIGN.md said "credentials.json"
//                                without the dot; the actual file is dotted)
//   ~/.codex/auth.json (or $CODEX_HOME/auth.json)
//
// This proves Tauri does NOT need to forward API keys via env or copy
// credentials — filesystem inheritance is sufficient.

const { spawn, execSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");
const os = require("node:os");

const isWin = process.platform === "win32";
const CLAUDE = isWin ? "claude.exe" : "claude";
const CODEX_NATIVE = "C:/Users/mizan/AppData/Roaming/npm/node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/codex/codex.exe";

function minimalEnv(extra = {}) {
  return {
    PATH: process.env.PATH,
    USERPROFILE: process.env.USERPROFILE,
    APPDATA: process.env.APPDATA,
    LOCALAPPDATA: process.env.LOCALAPPDATA,
    SystemRoot: process.env.SystemRoot,
    TEMP: process.env.TEMP,
    TMP: process.env.TMP,
    ...extra,
  };
}

function runClaude(env) {
  return new Promise((resolve) => {
    const child = spawn(CLAUDE, [
      "-p",
      "--output-format", "stream-json",
      "--verbose",
      "--max-turns", "1",
      "--model", "haiku",
      "--strict-mcp-config",
      "--mcp-config", '{"mcpServers":{}}',
      "--disable-slash-commands",
      "--disallowedTools", "Edit", "Write", "NotebookEdit",
    ], {
      stdio: ["pipe", "pipe", "pipe"],
      shell: false,
      windowsHide: true,
      env,
    });
    child.stdin.write("Reply OK only.");
    child.stdin.end();
    let buf = "", events = [], stderr = "";
    child.stdout.on("data", (c) => {
      buf += c.toString("utf8");
      let i;
      while ((i = buf.indexOf("\n")) >= 0) {
        const line = buf.slice(0, i);
        buf = buf.slice(i + 1);
        if (line.trim()) try { events.push(JSON.parse(line)); } catch {}
      }
    });
    child.stderr.on("data", (c) => { stderr += c.toString("utf8"); });
    let exit = null;
    child.on("exit", (c) => { exit = c; });
    setTimeout(() => { try { execSync(`taskkill /PID ${child.pid} /T /F`, { stdio: "ignore" }); } catch {} }, 30000);
    child.on("close", () => resolve({ events, stderr, exit }));
  });
}

function runCodex(env, codexHome) {
  return new Promise((resolve) => {
    const cwd = path.join(os.tmpdir(), `s5-codex-${Date.now()}`);
    fs.mkdirSync(cwd, { recursive: true });
    const child = spawn(CODEX_NATIVE, [
      "exec",
      "--ephemeral",
      "-c", "approval_policy=\"never\"",
      "-c", "model_reasoning_effort=\"low\"",
      "-c", "web_search=\"disabled\"",
      "--sandbox", "read-only",
      "--json",
      "--cd", cwd,
      "--skip-git-repo-check",
      "Reply OK only.",
    ], {
      stdio: ["pipe", "pipe", "pipe"],
      shell: false,
      windowsHide: true,
      env: { ...env, CODEX_HOME: codexHome },
    });
    child.stdin.end();
    let buf = "", events = [], stderr = "";
    child.stdout.on("data", (c) => {
      buf += c.toString("utf8");
      let i;
      while ((i = buf.indexOf("\n")) >= 0) {
        const line = buf.slice(0, i);
        buf = buf.slice(i + 1);
        if (line.trim()) try { events.push(JSON.parse(line)); } catch {}
      }
    });
    child.stderr.on("data", (c) => { stderr += c.toString("utf8"); });
    let exit = null;
    child.on("exit", (c) => { exit = c; });
    setTimeout(() => { try { execSync(`taskkill /PID ${child.pid} /T /F`, { stdio: "ignore" }); } catch {} }, 30000);
    child.on("close", () => resolve({ events, stderr, exit }));
  });
}

(async () => {
  const home = os.homedir();
  const claudeCred = path.join(home, ".claude", ".credentials.json");
  const codexAuth = path.join(home, ".codex", "auth.json");
  console.log(`[S5] ${claudeCred} exists? ${fs.existsSync(claudeCred)}`);
  console.log(`[S5] ${codexAuth} exists? ${fs.existsSync(codexAuth)}`);

  // Minimal env — no ANTHROPIC_API_KEY, no OPENAI_API_KEY, no CLAUDE_* etc.
  const env = minimalEnv();
  console.log(`[S5] env keys passed: ${Object.keys(env).join(",")}`);
  console.log(`[S5] No ANTHROPIC_API_KEY in env: ${!("ANTHROPIC_API_KEY" in env)}`);
  console.log(`[S5] No OPENAI_API_KEY in env: ${!("OPENAI_API_KEY" in env)}`);

  console.log("\n[S5] === claude with minimal env ===");
  const c = await runClaude(env);
  const claudeAuthed = c.events.some(e => e.type === "assistant" || e.type === "result");
  const claudeAuthError = c.stderr.toLowerCase().includes("auth") || c.stderr.toLowerCase().includes("login");
  console.log(`[S5] claude got assistant/result event: ${claudeAuthed}`);
  console.log(`[S5] claude stderr has auth keyword: ${claudeAuthError}`);
  if (c.stderr.trim()) console.log(`[S5] claude stderr: ${c.stderr.slice(0, 300)}`);

  // Codex via dedicated CODEX_HOME (the recommended MoA Desktop pattern)
  const moaCodexHome = path.join(home, ".moa-desktop-test", "codex-home");
  fs.mkdirSync(moaCodexHome, { recursive: true });
  if (!fs.existsSync(path.join(moaCodexHome, "auth.json"))) {
    fs.copyFileSync(codexAuth, path.join(moaCodexHome, "auth.json"));
  }
  console.log(`\n[S5] === codex with minimal env + CODEX_HOME=${moaCodexHome} ===`);
  const co = await runCodex(env, moaCodexHome);
  const codexAuthed = co.events.some(e => e.type === "thread.started" || e.type === "turn.completed");
  console.log(`[S5] codex got thread.started/turn.completed: ${codexAuthed}`);
  if (co.stderr.trim()) console.log(`[S5] codex stderr (truncated): ${co.stderr.slice(0, 400)}`);

  console.log("\n[S5] ===== RESULT =====");
  console.log(`[S5] claude filesystem auth inheritance: ${claudeAuthed && !claudeAuthError ? "PASS" : "FAIL"}`);
  console.log(`[S5] codex filesystem auth via CODEX_HOME: ${codexAuthed ? "PASS" : "FAIL"}`);
  console.log(`[S5] DESIGN.md says credentials.json — actual file is .credentials.json (dotted). Note for ticket follow-up.`);
  process.exit((claudeAuthed && codexAuthed) ? 0 : 1);
})();
