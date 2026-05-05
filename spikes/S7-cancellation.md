# S7 — Cancellation = Windows process tree kill

## Goal
Verify that on Tauri abort signal, all descendants of the Worker root process
terminate cleanly with zero zombies.

## Result: PASS

```
[A] taskkill /F (no /T) — surviving descendants: 0
[B] taskkill /T /F        — surviving descendants: 0
```

## Findings

1. **`taskkill /T /F /PID <root>`** is the canonical cleanup path. All
   descendants visible via `Get-CimInstance Win32_Process -Filter
   "ParentProcessId=$pid"` are killed in one syscall, no zombies.
2. **Codex spawns 1–2 descendant helpers per turn** (likely the actual
   model-streaming binary + a stdio relay). Snapshot times: usually present
   within the first 200ms of spawn.
3. **`taskkill /F` without `/T`** also cleaned up descendants in this run —
   suggesting Codex's helpers auto-exit on parent death OR are placed in a
   Windows job object by Codex itself. **However**, prior S2 spike runs
   showed taskkill /T /F killing 6+ descendants at cleanup, indicating
   not all descendants always die. **Always use `/T /F`** for safety.
4. **From S1**: Claude Worker (`claude.exe -p`) shows zero descendants for
   a single turn. Different process model from Codex. Tree-kill still
   correct (no-op for empty tree).
5. **Tauri v2** does not expose process-tree-kill directly. Implementation
   path:
   - On `Command::spawn()`, capture `child.pid()`.
   - On cancellation, call `std::process::Command::new("taskkill").args(["/T", "/F", "/PID", pid_str])` (Windows) or `process::kill(-pid)` (Unix).

## Tauri implementation snippet

```rust
#[cfg(target_os = "windows")]
fn kill_tree(pid: u32) -> Result<()> {
    std::process::Command::new("taskkill")
        .args(&["/T", "/F", "/PID", &pid.to_string()])
        .output()?;
    Ok(())
}

#[cfg(unix)]
fn kill_tree(pid: u32) -> Result<()> {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;
    kill(Pid::from_raw(-(pid as i32)), Signal::SIGKILL)?;
    Ok(())
}
```

Note: on Unix, requires `setpgid` so the child becomes its own process group.

## Validation cmd
```
node D:/moa-desktop/spikes/S7-cancellation.js
```

## Open questions
- Does Tauri's child stream-cleanup race with our explicit taskkill? Tauri's
  `Command::spawn` returns `(rx, child)` — we should `child.kill()` first
  (which on Windows does `TerminateProcess`), THEN `taskkill /T /F /PID` for
  any helpers. Belt-and-suspenders. T2 to finalize.
