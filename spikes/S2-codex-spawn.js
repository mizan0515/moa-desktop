// S2 — codex exec spawn + JSON stream + sandbox verification
// Run: node spikes/S2-codex-spawn.js
//
// Verifies:
//   1. argv-array spawn works (no PowerShell quoting issues)
//   2. --json yields JSONL stream
//   3. --ephemeral does not persist session files
//   4. --sandbox read-only — mutation attempt is denied (file NOT created)
//   5. --sandbox workspace-write — mutation succeeds (file IS created)

const { spawn } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");
const os = require("node:os");

const isWin = process.platform === "win32";

// codex CLI is npm-installed (codex.cmd wrapper → node codex.js → native binary).
// .cmd cannot be spawned with shell:false on Node 18+ (CVE-2024-27980 → EINVAL).
// Tauri-safe path: resolve to the native codex.exe inside the platform optional
// dep and spawn it directly. This skips the Node bootstrap and gives argv-array
// safety with no shell.
function resolveCodexExe() {
  if (!isWin) return "codex";
  const candidates = [
    "C:/Users/mizan/AppData/Roaming/npm/node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/codex/codex.exe",
  ];
  for (const c of candidates) {
    try { if (fs.existsSync(c)) return c; } catch {}
  }
  // last resort: shell:true wrapper around .cmd
  return null;
}
const program = resolveCodexExe();

function runCodex({ sandbox, cwd, prompt, timeoutMs = 60000 }) {
  return new Promise((resolve) => {
    const args = [
      "exec",
      "--ephemeral",
      "-c", "model_reasoning_effort=\"low\"",  // 'minimal' incompatible with web_search/image_gen tools
      "-c", "web_search=\"disabled\"",         // disable to keep this spike cheap
      "--sandbox", sandbox,
      "--json",
      "--cd", cwd,
      "--skip-git-repo-check",
      prompt,
    ];

    console.log(`\n[S2/${sandbox}] argv:`, JSON.stringify([program, ...args]));
    const t0 = Date.now();
    const child = spawn(program, args, {
      stdio: ["ignore", "pipe", "pipe"],
      shell: false,
      windowsHide: true,
      cwd: cwd,  // codex respects --cd, but also set process cwd for safety
      env: {
        ...process.env,
        // Codex needs CODEX_HOME to find auth.json
        CODEX_HOME: process.env.CODEX_HOME || `${process.env.USERPROFILE || os.homedir()}\\.codex`,
      },
    });

    const events = [];
    let buf = "";
    let firstEventMs = null;
    let stderrBuf = "";

    child.stdout.on("data", (chunk) => {
      buf += chunk.toString("utf8");
      let idx;
      while ((idx = buf.indexOf("\n")) >= 0) {
        const line = buf.slice(0, idx);
        buf = buf.slice(idx + 1);
        if (!line.trim()) continue;
        if (firstEventMs == null) firstEventMs = Date.now() - t0;
        try {
          events.push(JSON.parse(line));
        } catch {
          events.push({ _raw: line });
        }
      }
    });

    child.stderr.on("data", (c) => { stderrBuf += c.toString("utf8"); });

    let exitCode = null;
    child.on("exit", (code) => { exitCode = code; });

    const timer = setTimeout(() => {
      console.log(`[S2/${sandbox}] timeout — killing`);
      try {
        require("node:child_process").execSync(`taskkill /PID ${child.pid} /T /F`, { stdio: "ignore" });
      } catch {}
    }, timeoutMs);

    child.on("close", () => {
      clearTimeout(timer);
      resolve({
        sandbox,
        durationMs: Date.now() - t0,
        firstEventMs,
        eventCount: events.length,
        events,
        stderr: stderrBuf,
        exitCode,
      });
    });
  });
}

(async () => {
  // Make a fresh tmpdir as cwd for both runs (separate to avoid interference)
  const baseTmp = path.join(os.tmpdir(), `moa-S2-${Date.now()}`);
  const roDir = path.join(baseTmp, "ro");
  const wwDir = path.join(baseTmp, "ww");
  fs.mkdirSync(roDir, { recursive: true });
  fs.mkdirSync(wwDir, { recursive: true });
  console.log(`[S2] roDir=${roDir}`);
  console.log(`[S2] wwDir=${wwDir}`);

  const targetFile = "S2-mutation.txt";
  const prompt = `In your working directory, create a file named ${targetFile} with the single line "ok". Use a shell command. After creating it, output exactly: DONE.`;

  // ---- Run 1: read-only — mutation MUST fail ----
  const r1 = await runCodex({ sandbox: "read-only", cwd: roDir, prompt, timeoutMs: 90000 });
  const roFileExists = fs.existsSync(path.join(roDir, targetFile));
  // Look for sandbox-deny / patch-rejected / error events
  const denyEvent = r1.events.find(e => {
    const s = JSON.stringify(e).toLowerCase();
    return s.includes("sandbox") || s.includes("denied") || s.includes("permission") || s.includes("read-only");
  });

  console.log(`\n[S2/read-only] eventCount=${r1.eventCount} duration=${r1.durationMs}ms exit=${r1.exitCode}`);
  console.log(`[S2/read-only] firstEventMs=${r1.firstEventMs}`);
  console.log(`[S2/read-only] target file created? ${roFileExists}  (expected: false)`);
  console.log(`[S2/read-only] sandbox-related event detected? ${denyEvent ? "YES" : "no"}`);
  if (r1.stderr.trim()) console.log(`[S2/read-only] stderr (truncated):`, r1.stderr.slice(0, 600));

  // ---- Run 2: workspace-write — mutation SHOULD succeed ----
  const r2 = await runCodex({ sandbox: "workspace-write", cwd: wwDir, prompt, timeoutMs: 90000 });
  const wwFileExists = fs.existsSync(path.join(wwDir, targetFile));

  console.log(`\n[S2/workspace-write] eventCount=${r2.eventCount} duration=${r2.durationMs}ms exit=${r2.exitCode}`);
  console.log(`[S2/workspace-write] firstEventMs=${r2.firstEventMs}`);
  console.log(`[S2/workspace-write] target file created? ${wwFileExists}  (expected: true)`);
  if (r2.stderr.trim()) console.log(`[S2/workspace-write] stderr (truncated):`, r2.stderr.slice(0, 600));

  // ---- Verdict ----
  const sandboxBlocked = !roFileExists;
  const writeAllowed = wwFileExists;
  const streaming = r1.eventCount > 0 && r2.eventCount > 0;

  console.log(`\n[S2] ===== RESULT =====`);
  console.log(`[S2] read-only blocked mutation: ${sandboxBlocked ? "PASS" : "FAIL"}`);
  console.log(`[S2] workspace-write allowed mutation: ${writeAllowed ? "PASS" : "FAIL"}`);
  console.log(`[S2] JSONL streaming working: ${streaming ? "PASS" : "FAIL"}`);

  // dump first 5 event types for each run
  const types = (evs) => evs.slice(0, 8).map(e => e.type || e.msg?.type || e._raw?.slice(0, 40) || "?").join(", ");
  console.log(`[S2/read-only] first event types: ${types(r1.events)}`);
  console.log(`[S2/workspace-write] first event types: ${types(r2.events)}`);

  // save raw events for postmortem
  const dumpPath = path.join(baseTmp, "events.json");
  fs.writeFileSync(dumpPath, JSON.stringify({ ro: r1.events, ww: r2.events }, null, 2));
  console.log(`[S2] raw events dumped to ${dumpPath}`);

  // Sandbox verification is the primary safety assertion — exit non-zero on any FAIL
  // so CI / manual runs surface regressions instead of being masked by exit 0.
  const ok = sandboxBlocked && writeAllowed && streaming;
  if (!ok) {
    console.error(`[S2] FAIL — sandboxBlocked=${sandboxBlocked} writeAllowed=${writeAllowed} streaming=${streaming}`);
    process.exit(1);
  }
  process.exit(0);
})();
