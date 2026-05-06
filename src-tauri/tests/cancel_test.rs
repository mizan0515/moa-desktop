//! T9 cancel — integration test for the Stop button path.
//!
//! Spawns a real long-running child via `TokioProcessRunner`, registers it
//! into `CancelRegistry`, fires `abort_all`, and asserts the supervisor
//! reports the run as aborted with `kind = killed`.

#![cfg(target_os = "windows")]

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use moa_desktop_lib::cancel::CancelRegistry;
use moa_desktop_lib::process::{
    ProcessErrorKind, ProcessHandle, ProcessRunner, ProcessSpec, TokioProcessRunner,
};

fn cwd() -> PathBuf {
    std::env::current_dir().expect("cwd")
}

fn windows_env() -> HashMap<String, String> {
    let mut env = HashMap::new();
    for k in ["PATH", "SystemRoot", "USERPROFILE", "ComSpec", "TEMP"] {
        if let Ok(v) = std::env::var(k) {
            env.insert(k.to_string(), v);
        }
    }
    env
}

#[tokio::test]
async fn abort_all_kills_registered_runs() {
    let runner = TokioProcessRunner::new();
    // A 60s sleep — would never exit on its own within the test.
    let argv = vec![
        "powershell.exe".into(),
        "-NoProfile".into(),
        "-Command".into(),
        "Start-Sleep -Seconds 60".into(),
    ];
    let spec = ProcessSpec::new(argv, cwd()).with_env(windows_env());
    let ProcessHandle { control, lines: _lines } = runner.spawn(spec).await.expect("spawn");
    let pid = control.pid();

    let reg = CancelRegistry::new();
    reg.register("run-1", control.clone());
    assert_eq!(reg.count(), 1);
    assert!(reg.pids().contains(&pid));

    // Issue cancel.
    let n = reg.abort_all().await;
    assert_eq!(n, 1);

    // Wait for the supervisor to publish the exit. Bound it tight — taskkill
    // is normally instant.
    let exit = control
        .wait(Some(Duration::from_secs(10)))
        .await
        .expect("supervisor publishes exit");
    assert!(exit.aborted, "exit should be marked aborted");
    assert_eq!(exit.kind, Some(ProcessErrorKind::Killed));
}

#[tokio::test]
async fn abort_one_kills_specific_run() {
    let runner = TokioProcessRunner::new();
    let argv_long = vec![
        "powershell.exe".into(),
        "-NoProfile".into(),
        "-Command".into(),
        "Start-Sleep -Seconds 60".into(),
    ];
    let spec = ProcessSpec::new(argv_long, cwd()).with_env(windows_env());
    let ProcessHandle { control, lines: _ } = runner.spawn(spec).await.expect("spawn");

    let reg = CancelRegistry::new();
    reg.register("alpha", control.clone());
    reg.abort_one("alpha").await.expect("abort_one");
    let exit = control
        .wait(Some(Duration::from_secs(10)))
        .await
        .expect("exit");
    assert!(exit.aborted);
}
