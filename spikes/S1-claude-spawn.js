// S1 — claude -p spawn + stream-json + kill verification
// Run: node spikes/S1-claude-spawn.js
// Goal: spawn claude -p, receive JSONL stdout line-by-line, kill before completion,
//       verify child + descendants exit cleanly (no zombies).

const { spawn } = require("node:child_process");
const { exec } = require("node:child_process");
const { promisify } = require("node:util");
const execP = promisify(exec);

const isWin = process.platform === "win32";

async function listDescendants(rootPid) {
  // PowerShell: get all descendant PIDs recursively via CIM
  const ps = `
    function Get-Descendants($pid) {
      $kids = Get-CimInstance Win32_Process -Filter "ParentProcessId=$pid" -ErrorAction SilentlyContinue
      foreach ($k in $kids) {
        $k.ProcessId
        Get-Descendants $k.ProcessId
      }
    }
    Get-Descendants ${rootPid}
  `;
  try {
    const { stdout } = await execP(`powershell -NoProfile -Command "${ps.replace(/"/g, '\\"').replace(/\n/g, ' ')}"`);
    return stdout.trim().split(/\s+/).filter(Boolean).map(Number).filter(n => !Number.isNaN(n));
  } catch {
    return [];
  }
}

async function pidAlive(pid) {
  try {
    const { stdout } = await execP(`powershell -NoProfile -Command "Get-Process -Id ${pid} -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id"`);
    return stdout.trim().length > 0;
  } catch {
    return false;
  }
}

async function killTree(pid) {
  // Windows: taskkill /T /F kills the entire tree
  if (isWin) {
    try {
      await execP(`taskkill /PID ${pid} /T /F`);
    } catch (e) {
      // ignore "process not found"
    }
  } else {
    try { process.kill(-pid, "SIGKILL"); } catch {}
  }
}

(async () => {
  console.log("[S1] spawn claude -p with --output-format stream-json --verbose");

  // argv array (no shell string) — Tauri-style
  // KEY: use .exe on Windows. .cmd files trigger Node EINVAL with shell:false (CVE-2024-27980).
  // Tauri v2 Command::new must also resolve to .exe for argv-array safety.
  const program = isWin ? "claude.exe" : "claude";
  const args = [
    "-p",
    "--output-format", "stream-json",
    "--verbose",
    "--max-turns", "1",
    "--model", "haiku",
    "--disallowedTools", "Edit", "Write", "NotebookEdit",
  ];

  // KEY: pass prompt via stdin, not argv. claude's variadic flags
  // (--allowedTools / --disallowedTools <tools...>) absorb subsequent positional
  // args, so an argv-positional prompt gets eaten as a tool name. Stdin is the
  // deterministic path Tauri should use.
  const prompt = "Reply with exactly: PONG. Nothing else.";

  console.log("[S1] argv:", JSON.stringify([program, ...args]));

  const t0 = Date.now();
  const child = spawn(program, args, {
    stdio: ["pipe", "pipe", "pipe"],
    shell: false,
    windowsHide: true,
  });
  child.stdin.write(prompt);
  child.stdin.end();

  console.log(`[S1] spawned PID=${child.pid}`);

  let lineCount = 0;
  let firstLineMs = null;
  let buf = "";
  child.stdout.on("data", (chunk) => {
    buf += chunk.toString("utf8");
    let idx;
    while ((idx = buf.indexOf("\n")) >= 0) {
      const line = buf.slice(0, idx);
      buf = buf.slice(idx + 1);
      if (!line.trim()) continue;
      lineCount += 1;
      if (firstLineMs == null) firstLineMs = Date.now() - t0;
      // print short summary of each JSONL line
      try {
        const obj = JSON.parse(line);
        const t = obj.type || obj.event || "?";
        console.log(`[S1] line#${lineCount} type=${t} (${line.length}B)`);
      } catch {
        console.log(`[S1] line#${lineCount} (non-json, ${line.length}B): ${line.slice(0, 80)}`);
      }
    }
  });

  child.stderr.on("data", (c) => {
    process.stderr.write(`[S1][child stderr] ${c.toString("utf8")}`);
  });

  let exitCode = null;
  let exitSignal = null;
  child.on("exit", (code, signal) => {
    exitCode = code;
    exitSignal = signal;
    console.log(`[S1] child exit code=${code} signal=${signal} (after ${Date.now() - t0}ms)`);
  });

  // wait either for first JSONL line or 8s
  const startWait = Date.now();
  while (lineCount === 0 && Date.now() - startWait < 12000 && exitCode === null) {
    await new Promise(r => setTimeout(r, 100));
  }

  console.log(`[S1] firstLine after ${firstLineMs}ms, totalLines=${lineCount}`);

  // Snapshot descendants BEFORE kill
  const descBefore = await listDescendants(child.pid);
  console.log(`[S1] descendants before kill: [${descBefore.join(", ")}]`);

  // Kill the tree
  console.log(`[S1] killing tree of PID=${child.pid}`);
  await killTree(child.pid);

  // Wait up to 3s for parent exit
  const killStart = Date.now();
  while (exitCode === null && Date.now() - killStart < 3000) {
    await new Promise(r => setTimeout(r, 100));
  }

  // Re-check descendants (filter to those that were alive before)
  await new Promise(r => setTimeout(r, 500));
  const stillAlive = [];
  for (const pid of [child.pid, ...descBefore]) {
    if (await pidAlive(pid)) stillAlive.push(pid);
  }

  console.log("[S1] ===== RESULT =====");
  console.log(`[S1] firstLineMs=${firstLineMs}`);
  console.log(`[S1] linesReceived=${lineCount}`);
  console.log(`[S1] exitCode=${exitCode} exitSignal=${exitSignal}`);
  console.log(`[S1] descendantsBeforeKill=${descBefore.length}`);
  console.log(`[S1] stillAliveAfterKill=${stillAlive.length} ${JSON.stringify(stillAlive)}`);

  const pass = (
    lineCount > 0 &&
    exitCode !== null &&
    stillAlive.length === 0
  );
  console.log(`[S1] VERDICT: ${pass ? "PASS" : "FAIL"}`);
  process.exit(pass ? 0 : 1);
})();
