//! Process tree kill — Windows `taskkill /T /F` and Unix process-group SIGKILL.
//!
//! S7 verified `taskkill /T /F /PID <root>` reliably kills Codex's 1–2 helper
//! descendants on Windows. Spike S7 left an open question on kill order
//! relative to Tokio's `child.kill()`; T2 resolves it by tree-killing FIRST,
//! then awaiting `child.wait()` to reap. Otherwise descendants can lose their
//! parent before traversal completes.

use std::time::Duration;

#[cfg(target_os = "windows")]
pub async fn kill_tree(pid: u32) -> Result<(), String> {
    use tokio::process::Command;

    let pid_str = pid.to_string();
    // Prefer absolute path so a minimal Worker PATH still resolves taskkill.
    let abs = std::env::var_os("SystemRoot")
        .map(|root| {
            let mut p = std::path::PathBuf::from(root);
            p.push("System32");
            p.push("taskkill.exe");
            p
        })
        .filter(|p| p.exists());

    let mut cmd = match abs {
        Some(p) => Command::new(p),
        None => Command::new("taskkill"),
    };
    cmd.args(["/T", "/F", "/PID", &pid_str]);
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());
    cmd.stdin(std::process::Stdio::null());

    let child = cmd.spawn().map_err(|e| format!("spawn taskkill: {e}"))?;
    // Bound the wait — taskkill is normally instant.
    let waited = tokio::time::timeout(Duration::from_secs(5), wait_child(child)).await;
    match waited {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(e)) => Err(format!("taskkill wait: {e}")),
        Err(_) => Err("taskkill exceeded 5s".into()),
    }
}

#[cfg(target_os = "windows")]
async fn wait_child(mut child: tokio::process::Child) -> Result<(), String> {
    child.wait().await.map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(unix)]
pub async fn kill_tree(pid: u32) -> Result<(), String> {
    // Unix path is a placeholder for v1 (project is Windows-first per spikes).
    // The runner sets `process_group(0)` so a future implementation can SIGKILL
    // the negative PID to reach the whole group. Until then, fall back to a
    // best-effort SIGKILL via /usr/bin/kill on the root only.
    use tokio::process::Command;
    let _ = Command::new("/bin/kill")
        .args(["-KILL", &format!("-{pid}")])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await;
    Ok(())
}
