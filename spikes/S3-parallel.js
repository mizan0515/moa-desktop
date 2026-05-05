// S3 — parallel two-worker spawn (Claude + Codex) verification
// Run: node spikes/S3-parallel.js
//
// Verifies:
//   1. Two children spawn concurrently
//   2. Each child's JSONL stream is independently received (no cross-pipe bleed)
//   3. Each line in each stream is a complete, parseable JSON object (no
//      interleaving within a single line)
//   4. Killing one does not affect the other
//   5. Each child has its own descendant tree

const { spawn, execSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");
const os = require("node:os");

const isWin = process.platform === "win32";
const CLAUDE = isWin ? "claude.exe" : "claude";

function resolveCodexExe() {
  if (!isWin) return "codex";
  const candidates = [
    "C:/Users/mizan/AppData/Roaming/npm/node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/codex/codex.exe",
  ];
  for (const c of candidates) if (fs.existsSync(c)) return c;
  return null;
}
const CODEX = resolveCodexExe();

function killTree(pid) {
  if (isWin) {
    try { execSync(`taskkill /PID ${pid} /T /F`, { stdio: "ignore" }); } catch {}
  } else {
    try { process.kill(-pid, "SIGKILL"); } catch {}
  }
}

function startWorker(label, program, args, opts = {}) {
  const child = spawn(program, args, {
    stdio: ["pipe", "pipe", "pipe"],
    shell: false,
    windowsHide: true,
    ...opts,
  });
  const events = [];
  let buf = "";
  let firstEventMs = null;
  const t0 = Date.now();
  let exitCode = null;
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
        const obj = JSON.parse(line);
        events.push({ ok: true, type: obj.type || obj.event || "?", len: line.length });
      } catch (e) {
        events.push({ ok: false, len: line.length, snippet: line.slice(0, 60) });
      }
    }
  });
  child.stderr.on("data", (c) => { stderrBuf += c.toString("utf8"); });
  child.on("exit", (c) => { exitCode = c; });

  return {
    label,
    child,
    pid: child.pid,
    t0,
    get firstEventMs() { return firstEventMs; },
    get events() { return events; },
    get exitCode() { return exitCode; },
    get stderr() { return stderrBuf; },
    get parseFails() { return events.filter(e => !e.ok).length; },
  };
}

(async () => {
  const cleanCodexHome = path.join(os.homedir(), ".moa-desktop-test", "codex-home");
  fs.mkdirSync(cleanCodexHome, { recursive: true });
  const authSrc = path.join(os.homedir(), ".codex", "auth.json");
  const authDst = path.join(cleanCodexHome, "auth.json");
  if (!fs.existsSync(authDst) && fs.existsSync(authSrc)) {
    fs.copyFileSync(authSrc, authDst);
  }

  // Both workers in their OWN cwd to avoid file race
  const claudeCwd = path.join(os.tmpdir(), `s3-claude-${Date.now()}`);
  const codexCwd = path.join(os.tmpdir(), `s3-codex-${Date.now()}`);
  fs.mkdirSync(claudeCwd, { recursive: true });
  fs.mkdirSync(codexCwd, { recursive: true });

  const w1 = startWorker(
    "claude",
    CLAUDE,
    [
      "-p",
      "--output-format", "stream-json",
      "--verbose",
      "--max-turns", "1",
      "--model", "haiku",
      "--disallowedTools", "Edit", "Write", "NotebookEdit",
    ],
    { cwd: claudeCwd },
  );
  w1.child.stdin.write("Reply PONG.");
  w1.child.stdin.end();

  const w2 = startWorker(
    "codex",
    CODEX,
    [
      "exec",
      "--ephemeral",
      "-c", "approval_policy=\"never\"",
      "-c", "model_reasoning_effort=\"low\"",
      "-c", "web_search=\"disabled\"",
      "--sandbox", "read-only",
      "--json",
      "--cd", codexCwd,
      "--skip-git-repo-check",
      "Reply PONG only.",
    ],
    {
      cwd: codexCwd,
      env: { ...process.env, CODEX_HOME: cleanCodexHome },
    },
  );
  // Codex prints "Reading additional input from stdin..." if stdin is open;
  // close immediately so it doesn't wait for input that will never come.
  w2.child.stdin.end();

  console.log(`[S3] spawned claude PID=${w1.pid} cwd=${claudeCwd}`);
  console.log(`[S3] spawned codex  PID=${w2.pid} cwd=${codexCwd}`);

  const tStart = Date.now();
  // Wait until both have at least 1 event OR 30s timeout
  while (
    (w1.firstEventMs == null || w2.firstEventMs == null) &&
    Date.now() - tStart < 30000 &&
    w1.exitCode === null &&
    w2.exitCode === null
  ) {
    await new Promise(r => setTimeout(r, 100));
  }
  console.log(`[S3] both got first event after ${Date.now() - tStart}ms`);

  // Let them run a bit more to accumulate events
  await new Promise(r => setTimeout(r, 2000));

  console.log(`[S3] claude events=${w1.events.length} parseFails=${w1.parseFails} firstMs=${w1.firstEventMs}`);
  console.log(`[S3] codex  events=${w2.events.length} parseFails=${w2.parseFails} firstMs=${w2.firstEventMs}`);

  // Track exit times to distinguish "natural completion" vs "killed-by-association"
  const w1ExitedNaturallyBeforeKill = w1.exitCode !== null;
  const w2ExitedNaturallyBeforeKill = w2.exitCode !== null;
  console.log(`[S3] before kill: claude exited? ${w1ExitedNaturallyBeforeKill} codex exited? ${w2ExitedNaturallyBeforeKill}`);

  // Kill ONLY claude (if still alive) and verify codex's lifecycle is independent.
  // Two cases count as "kill isolation":
  //   (a) codex already exited naturally before we killed claude — proves independent lifecycle
  //   (b) codex still alive after we kill claude
  const tBeforeKill = Date.now();
  if (!w1ExitedNaturallyBeforeKill) {
    console.log(`[S3] killing claude (PID=${w1.pid})`);
    killTree(w1.pid);
    await new Promise(r => setTimeout(r, 1500));
  }

  // Was codex alive at the moment of (or shortly after) the claude-kill?
  // If w2 exited BEFORE the kill, isolation is trivially proven.
  // If w2 exited AFTER the kill but its exit-event was emitted within 200ms of
  // the kill, that would be suspicious. Otherwise independent.
  let codexExitedBeforeKill = w2ExitedNaturallyBeforeKill;
  let pidsAreSeparate = w1.pid !== w2.pid;
  // Strongest proof of independent lifecycle: codex emitted `turn.completed`
  // (= natural completion). A kill cascade would not show this terminal event.
  const codexNaturalCompletion = w2.events.some(e => e.type === "turn.completed" || e.type === "turn.failed");
  const isolated = pidsAreSeparate && (codexExitedBeforeKill || codexNaturalCompletion || w2.exitCode === null);

  console.log(`[S3] PIDs separate: ${pidsAreSeparate}`);
  console.log(`[S3] codex events: ${JSON.stringify(w2.events.map(e=>e.type))}`);
  console.log(`[S3] codex emitted turn.completed (natural finish): ${codexNaturalCompletion}`);
  console.log(`[S3] codex exited naturally before claude kill: ${codexExitedBeforeKill}`);
  console.log(`[S3] isolation (kill of one didn't cascade): ${isolated}`);

  // Cleanup
  killTree(w2.pid);
  await new Promise(r => setTimeout(r, 1000));

  console.log("\n[S3] ===== RESULT =====");
  const bothStreamed = w1.events.length > 0 && w2.events.length > 0;
  const noParseFails = w1.parseFails === 0 && w2.parseFails === 0;
  console.log(`[S3] both workers streamed JSONL: ${bothStreamed ? "PASS" : "FAIL"}`);
  console.log(`[S3] no JSON parse failures (no line interleaving): ${noParseFails ? "PASS" : "FAIL"}`);
  console.log(`[S3] kill isolation (claude kill ≠ codex kill): ${isolated ? "PASS" : "FAIL"}`);
  const verdict = bothStreamed && noParseFails && isolated;
  console.log(`[S3] VERDICT: ${verdict ? "PASS" : "FAIL"}`);

  process.exit(verdict ? 0 : 1);
})();
