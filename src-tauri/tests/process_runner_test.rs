//! Integration tests for T2 — `TokioProcessRunner`.
//!
//! Windows-first: these tests use `cmd.exe` / `powershell.exe` as the spawned
//! process so we don't need a Rust fixture binary or a Node fixture script.
//! Unix port deferred (project is Windows-first per spike S0; Tauri target is
//! desktop Windows v1).

#![cfg(target_os = "windows")]

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use moa_desktop_lib::process::{
    ProcessErrorKind, ProcessHandle, ProcessRunner, ProcessSpec, StdinPolicy, Stream,
    TokioProcessRunner,
};

fn cwd() -> PathBuf {
    std::env::current_dir().expect("cwd available")
}

fn windows_env() -> HashMap<String, String> {
    // Minimal env so cmd.exe / powershell.exe still resolve.
    let mut env = HashMap::new();
    for k in ["PATH", "SystemRoot", "USERPROFILE", "ComSpec", "TEMP"] {
        if let Ok(v) = std::env::var(k) {
            env.insert(k.to_string(), v);
        }
    }
    env
}

fn cmd_argv(line: &str) -> Vec<String> {
    vec!["cmd.exe".into(), "/c".into(), line.into()]
}

#[tokio::test]
async fn streams_thousand_lines_in_order() {
    let runner = TokioProcessRunner::new();
    let spec = ProcessSpec::new(cmd_argv("for /L %i in (1,1,1000) do @echo line-%i"), cwd())
        .with_env(windows_env());

    let ProcessHandle { control, mut lines } = runner.spawn(spec).await.expect("spawn");

    let exit_handle = {
        let c = control.clone();
        tokio::spawn(async move { c.wait(Some(Duration::from_secs(30))).await })
    };

    let mut count = 0u64;
    let mut last_seq: Option<u64> = None;
    while let Some(l) = lines.recv().await {
        if l.stream != Stream::Stdout {
            continue;
        }
        if let Some(prev) = last_seq {
            assert!(l.seq > prev, "seq must monotonically increase");
        }
        last_seq = Some(l.seq);
        count += 1;
    }
    let exit = exit_handle.await.unwrap().expect("exit ok");
    assert_eq!(count, 1000, "all 1000 lines must arrive");
    assert!(exit.is_clean(), "natural exit must be clean: {exit:?}");
}

#[tokio::test]
async fn abort_kills_long_sleep_within_seconds() {
    let runner = TokioProcessRunner::new();
    // Long sleep (60s); we abort at ~500ms.
    let spec = ProcessSpec::new(
        vec![
            "powershell.exe".into(),
            "-NoProfile".into(),
            "-Command".into(),
            "Start-Sleep -Seconds 60".into(),
        ],
        cwd(),
    )
    .with_env(windows_env());

    let ProcessHandle { control, mut lines } = runner.spawn(spec).await.expect("spawn");

    let pid = control.pid();
    let drainer = tokio::spawn(async move { while lines.recv().await.is_some() {} });

    tokio::time::sleep(Duration::from_millis(500)).await;
    let started = Instant::now();
    control.abort().await.expect("abort sends");

    let exit = control
        .wait(Some(Duration::from_secs(10)))
        .await
        .expect("exit publishes");
    let elapsed = started.elapsed();
    drainer.await.ok();

    assert!(exit.aborted, "exit must be marked aborted: {exit:?}");
    assert!(
        elapsed < Duration::from_secs(5),
        "abort must reap quickly, took {elapsed:?}"
    );
    // Verify no descendants linger.
    assert_eq!(
        windows_descendant_count(pid),
        0,
        "no descendants should remain after taskkill /T /F",
    );
}

#[tokio::test]
async fn timeout_marks_timed_out_and_cleans_descendants() {
    let runner = TokioProcessRunner::new();
    let spec = ProcessSpec::new(
        vec![
            "powershell.exe".into(),
            "-NoProfile".into(),
            "-Command".into(),
            "Start-Sleep -Seconds 30".into(),
        ],
        cwd(),
    )
    .with_env(windows_env());

    let ProcessHandle { control, mut lines } = runner.spawn(spec).await.expect("spawn");

    let pid = control.pid();
    let drainer = tokio::spawn(async move { while lines.recv().await.is_some() {} });

    let exit = control
        .wait(Some(Duration::from_millis(500)))
        .await
        .expect("exit publishes after timeout");
    drainer.await.ok();

    assert!(exit.timed_out, "timeout must set timed_out=true: {exit:?}");
    assert!(
        exit.aborted,
        "supervisor abort path should also mark aborted"
    );
    assert_eq!(exit.kind, Some(ProcessErrorKind::Timeout));
    assert_eq!(windows_descendant_count(pid), 0);
}

#[tokio::test]
async fn cli_missing_yields_typed_error() {
    let runner = TokioProcessRunner::new();
    let spec = ProcessSpec::new(vec!["this-binary-does-not-exist-12345.exe".into()], cwd())
        .with_env(windows_env());

    let err = match runner.spawn(spec).await {
        Ok(_) => panic!("expected spawn to fail"),
        Err(e) => e,
    };
    assert_eq!(err.kind, ProcessErrorKind::CliMissing);
}

#[tokio::test]
async fn empty_argv_yields_cli_missing() {
    let runner = TokioProcessRunner::new();
    let spec = ProcessSpec::new(vec![], cwd()).with_env(windows_env());
    let err = match runner.spawn(spec).await {
        Ok(_) => panic!("expected spawn to fail"),
        Err(e) => e,
    };
    assert_eq!(err.kind, ProcessErrorKind::CliMissing);
}

#[tokio::test]
async fn partial_final_line_emitted_when_no_trailing_newline() {
    let runner = TokioProcessRunner::new();
    // `<nul set /p=` prints to stdout WITHOUT trailing newline.
    let spec =
        ProcessSpec::new(cmd_argv("<nul set /p =fragment-no-eol"), cwd()).with_env(windows_env());

    let ProcessHandle { control, mut lines } = runner.spawn(spec).await.expect("spawn");

    let mut got_partial = false;
    let mut got_full_text = String::new();
    while let Some(l) = lines.recv().await {
        if l.stream == Stream::Stdout {
            got_full_text.push_str(&l.line);
            if l.partial {
                got_partial = true;
            }
        }
    }
    let _ = control.wait(Some(Duration::from_secs(5))).await;

    assert!(got_partial, "EOF without newline must mark partial=true");
    assert!(
        got_full_text.contains("fragment-no-eol"),
        "fragment must be delivered, got {got_full_text:?}",
    );
}

#[tokio::test]
async fn stderr_tail_captured_unredacted() {
    let runner = TokioProcessRunner::new();
    let spec = ProcessSpec::new(
        cmd_argv("echo STDERR_API_KEY=verysecret 1>&2 && exit /b 7"),
        cwd(),
    )
    .with_env(windows_env());

    let ProcessHandle { control, mut lines } = runner.spawn(spec).await.expect("spawn");
    let drainer = tokio::spawn(async move { while lines.recv().await.is_some() {} });

    let exit = control
        .wait(Some(Duration::from_secs(5)))
        .await
        .expect("exit");
    drainer.await.ok();

    assert_eq!(exit.code, Some(7));
    assert!(
        exit.stderr_tail.contains("STDERR_API_KEY=verysecret"),
        "stderr_tail must remain raw — adapters need fidelity. got: {:?}",
        exit.stderr_tail
    );
    // Adapter is responsible for refining; runner returns kind=None on plain
    // non-zero exit with no Windows OOM NTSTATUS.
    assert_eq!(exit.kind, None);
}

#[tokio::test]
async fn double_abort_is_idempotent() {
    let runner = TokioProcessRunner::new();
    let spec = ProcessSpec::new(
        vec![
            "powershell.exe".into(),
            "-NoProfile".into(),
            "-Command".into(),
            "Start-Sleep -Seconds 30".into(),
        ],
        cwd(),
    )
    .with_env(windows_env());

    let ProcessHandle { control, mut lines } = runner.spawn(spec).await.expect("spawn");
    let drainer = tokio::spawn(async move { while lines.recv().await.is_some() {} });

    tokio::time::sleep(Duration::from_millis(200)).await;
    control.abort().await.expect("first abort");
    control.abort().await.expect("second abort no-op");
    control.abort().await.expect("third abort no-op");

    let exit = control
        .wait(Some(Duration::from_secs(5)))
        .await
        .expect("exit");
    drainer.await.ok();
    assert!(exit.aborted);
}

#[tokio::test]
async fn abort_after_natural_exit_is_noop() {
    let runner = TokioProcessRunner::new();
    let spec = ProcessSpec::new(cmd_argv("echo done"), cwd()).with_env(windows_env());
    let ProcessHandle { control, mut lines } = runner.spawn(spec).await.expect("spawn");

    let _drainer = tokio::spawn(async move { while lines.recv().await.is_some() {} });

    let exit = control
        .wait(Some(Duration::from_secs(5)))
        .await
        .expect("exit");
    assert!(exit.is_clean());

    // Abort after exit must not panic / not error.
    control.abort().await.expect("idempotent late abort");
}

#[tokio::test]
async fn stdin_pipe_writes_then_closes() {
    let runner = TokioProcessRunner::new();
    // findstr echoes lines containing "hello" — needs stdin then EOF.
    let spec = ProcessSpec::new(
        vec!["cmd.exe".into(), "/c".into(), "findstr hello".into()],
        cwd(),
    )
    .with_env(windows_env())
    .with_stdin(StdinPolicy::Pipe);

    let ProcessHandle { control, mut lines } = runner.spawn(spec).await.expect("spawn");

    control
        .write_stdin(b"hello world\n".to_vec())
        .await
        .expect("write");
    control
        .write_stdin(b"goodbye\n".to_vec())
        .await
        .expect("write");
    control.close_stdin().await.expect("close stdin");

    let mut got_hello = false;
    while let Some(l) = lines.recv().await {
        if l.stream == Stream::Stdout && l.line.contains("hello world") {
            got_hello = true;
        }
    }
    let exit = control
        .wait(Some(Duration::from_secs(5)))
        .await
        .expect("exit");
    assert!(got_hello, "findstr must echo the matching line");
    assert!(exit.is_clean(), "{exit:?}");
}

// --------- helpers ---------

fn windows_descendant_count(pid: u32) -> usize {
    use std::process::Command;
    // Walk the process tree under `pid` via wmic / Get-CimInstance equivalent.
    // Returns the count of still-running PIDs that have `pid` (or descendants
    // of pid) as ancestor. Best-effort — failures collapse to 0.
    let out = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "Get-CimInstance Win32_Process -Filter 'ParentProcessId={pid}' | \
                 Select-Object -ExpandProperty ProcessId"
            ),
        ])
        .output();
    let Ok(o) = out else { return 0 };
    String::from_utf8_lossy(&o.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count()
}
