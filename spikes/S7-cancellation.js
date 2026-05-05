// S7 — Cancellation = Windows process-tree kill
// Run: node spikes/S7-cancellation.js
//
// Verifies:
//   1. Codex (and possibly Claude) spawn descendant processes during a turn
//   2. `taskkill /F` (without /T) leaves zombies — DEMONSTRATE this hazard
//   3. `taskkill /T /F` cleans entire tree — REQUIRED approach
//   4. After tree-kill, no orphan PID remains alive

const { spawn, execSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");
const os = require("node:os");

const isWin = process.platform === "win32";
const CLAUDE = isWin ? "claude.exe" : "claude";
const CODEX_NATIVE = "C:/Users/mizan/AppData/Roaming/npm/node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/codex/codex.exe";

function listDescendants(rootPid) {
  const ps = `function Get-Descendants($p) { $kids = Get-CimInstance Win32_Process -Filter "ParentProcessId=$p" -ErrorAction SilentlyContinue; foreach ($k in $kids) { $k.ProcessId; Get-Descendants $k.ProcessId } }; Get-Descendants ${rootPid}`;
  try {
    const out = execSync(`powershell -NoProfile -Command "${ps}"`, { stdio: ["ignore", "pipe", "pipe"] }).toString();
    return out.trim().split(/\s+/).filter(Boolean).map(Number).filter(n => !Number.isNaN(n));
  } catch { return []; }
}

function pidsAlive(pids) {
  if (pids.length === 0) return [];
  const filter = pids.map(p => `Id -eq ${p}`).join(" -or ");
  try {
    const out = execSync(`powershell -NoProfile -Command "Get-Process | Where-Object { ${filter} } | Select-Object -ExpandProperty Id"`, { stdio: ["ignore", "pipe", "pipe"] }).toString();
    return out.trim().split(/\s+/).filter(Boolean).map(Number).filter(n => !Number.isNaN(n));
  } catch { return []; }
}

async function spawnCodexLongRunning(label) {
  const home = os.homedir();
  const codexHome = path.join(home, ".moa-desktop-test", "codex-home");
  fs.mkdirSync(codexHome, { recursive: true });
  if (!fs.existsSync(path.join(codexHome, "auth.json"))) {
    fs.copyFileSync(path.join(home, ".codex", "auth.json"), path.join(codexHome, "auth.json"));
  }
  const cwd = path.join(os.tmpdir(), `s7-${label}-${Date.now()}`);
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
    // Long-ish prompt to keep codex alive long enough to snapshot descendants
    "Tell me a story in 3 paragraphs about a robot. Each paragraph at least 5 sentences.",
  ], {
    stdio: ["pipe", "pipe", "pipe"],
    shell: false,
    windowsHide: true,
    env: { ...process.env, CODEX_HOME: codexHome },
  });
  child.stdin.end();
  let exited = false;
  child.on("exit", () => { exited = true; });
  return { child, isExited: () => exited };
}

(async () => {
  // ---- Test A: taskkill /F without /T (demonstrate hazard) ----
  console.log("[S7] === A: taskkill /F (no /T) leaves descendants ===");
  const a = await spawnCodexLongRunning("A");
  console.log(`[S7/A] codex PID=${a.child.pid}`);
  // Wait until codex spawns descendants
  let descA = [];
  for (let i = 0; i < 30 && descA.length === 0 && !a.isExited(); i++) {
    await new Promise(r => setTimeout(r, 200));
    descA = listDescendants(a.child.pid);
  }
  console.log(`[S7/A] descendants before kill: ${descA.length} ${JSON.stringify(descA)}`);

  if (descA.length > 0) {
    console.log(`[S7/A] taskkill /F /PID ${a.child.pid} (no /T)`);
    try { execSync(`taskkill /F /PID ${a.child.pid}`, { stdio: "ignore" }); } catch {}
    await new Promise(r => setTimeout(r, 1500));
    const survivorsA = pidsAlive(descA);
    console.log(`[S7/A] surviving descendants: ${survivorsA.length} ${JSON.stringify(survivorsA)}`);
    // cleanup any leaks
    for (const p of survivorsA) {
      try { execSync(`taskkill /F /PID ${p}`, { stdio: "ignore" }); } catch {}
    }
    var hazardConfirmed = survivorsA.length > 0;
    console.log(`[S7/A] hazard (zombies after non-tree kill) confirmed: ${hazardConfirmed}`);
  } else {
    console.log(`[S7/A] codex emitted no descendants in time window — skipping hazard demo`);
    var hazardConfirmed = "skipped";
    try { execSync(`taskkill /T /F /PID ${a.child.pid}`, { stdio: "ignore" }); } catch {}
  }

  // ---- Test B: taskkill /T /F cleans all ----
  console.log("\n[S7] === B: taskkill /T /F cleans entire tree ===");
  const b = await spawnCodexLongRunning("B");
  console.log(`[S7/B] codex PID=${b.child.pid}`);
  let descB = [];
  for (let i = 0; i < 30 && descB.length === 0 && !b.isExited(); i++) {
    await new Promise(r => setTimeout(r, 200));
    descB = listDescendants(b.child.pid);
  }
  console.log(`[S7/B] descendants before kill: ${descB.length} ${JSON.stringify(descB)}`);

  console.log(`[S7/B] taskkill /T /F /PID ${b.child.pid}`);
  try { execSync(`taskkill /T /F /PID ${b.child.pid}`, { stdio: "ignore" }); } catch {}
  await new Promise(r => setTimeout(r, 1500));
  const survivorsB = pidsAlive([b.child.pid, ...descB]);
  console.log(`[S7/B] survivors after tree kill: ${survivorsB.length} ${JSON.stringify(survivorsB)}`);

  console.log("\n[S7] ===== RESULT =====");
  const treeKillPasses = survivorsB.length === 0;
  console.log(`[A] non-tree kill leaves zombies (hazard demo): ${hazardConfirmed}`);
  console.log(`[B] /T /F leaves zero survivors: ${treeKillPasses ? "PASS" : "FAIL"}`);
  process.exit(treeKillPasses ? 0 : 1);
})();
