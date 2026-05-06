//! T2 fix/B-runner — regression tests for the 4 defects called out by the
//! orchestrator review:
//!   1. `kill_tree` failure must not hang the abort path forever — Tokio's
//!      direct `child.kill()` is a backstop with a bounded reap timeout.
//!   2. Timeout classification must be consistent across EVERY waiter — the
//!      supervisor publishes `timed_out=true` once and all `wait()` callers
//!      see the same `ProcessExit` (no per-caller mutation).
//!   3. UTF-8 multibyte chars must NOT be split mid-byte when a line
//!      overflows `max_line_bytes`. Force-split happens on a UTF-8 boundary;
//!      incomplete trailing bytes carry into the next chunk.
//!   4. Spawn errors must split into `CliMissing` (ENOENT), `PermissionDenied`
//!      (EACCES), and `Spawn` (everything else) — they no longer collapse to
//!      `CliMissing` for non-NotFound errors.

#![cfg(target_os = "windows")]

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use moa_desktop_lib::process::{
    ProcessErrorKind, ProcessHandle, ProcessRunner, ProcessSpec, Stream, TokioProcessRunner,
};

fn cwd() -> PathBuf {
    std::env::current_dir().expect("cwd available")
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

fn cmd_argv(line: &str) -> Vec<String> {
    vec!["cmd.exe".into(), "/c".into(), line.into()]
}

// ─────────────────────────────────────────────────────────────────────────
// Defect 1: kill_tree failure must not hang the abort path.
//
// We can't easily make the real `taskkill.exe` fail without admin
// privileges. The proxy assertion is timing: an aborted long-sleep must
// terminate within the supervisor's 5s reap budget regardless of any
// taskkill flakiness. The `child.start_kill()` backstop guarantees this.
// ─────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn abort_completes_within_reap_budget_even_if_kill_tree_is_slow() {
    let runner = TokioProcessRunner::new();
    let spec = ProcessSpec::new(
        vec![
            "powershell.exe".into(),
            "-NoProfile".into(),
            "-Command".into(),
            "Start-Sleep -Seconds 120".into(),
        ],
        cwd(),
    )
    .with_env(windows_env());

    let ProcessHandle { control, mut lines } = runner.spawn(spec).await.expect("spawn");
    let drainer = tokio::spawn(async move { while lines.recv().await.is_some() {} });

    tokio::time::sleep(Duration::from_millis(200)).await;
    let started = Instant::now();
    control.abort().await.expect("abort sends");

    // 8s budget = 5s supervisor reap + slack. Without the start_kill backstop
    // a kill_tree failure would let `child.wait()` block until the 120s sleep
    // ended, blowing this assertion by an order of magnitude.
    let exit = control
        .wait(Some(Duration::from_secs(8)))
        .await
        .expect("exit publishes within reap budget");
    let elapsed = started.elapsed();
    drainer.await.ok();

    assert!(exit.aborted, "exit must be marked aborted: {exit:?}");
    assert!(
        elapsed < Duration::from_secs(7),
        "abort path must reap within 7s; got {elapsed:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// Defect 2: timeout classification must be visible to every waiter.
//
// Pre-fix bug: `ProcessControl::wait()` mutated only its OWN returned
// `ProcessExit { timed_out, kind }` after the timer fired. The published
// `watch::Receiver` value still carried `timed_out=false, kind=Killed`, so
// any sibling waiter (T7's orchestrator state machine watches the same
// channel) saw a wrong classification.
// ─────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn timeout_is_seen_consistently_by_every_waiter() {
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

    // First waiter sets the timer.
    let c1 = control.clone();
    let waiter1 = tokio::spawn(async move { c1.wait(Some(Duration::from_millis(400))).await });

    // Second waiter has no deadline and observes the supervisor's published
    // exit. It must see timed_out=true, kind=Timeout — which only works if
    // the supervisor itself stamped those (not the per-caller mutation
    // pre-fix did).
    let c2 = control.clone();
    let waiter2 = tokio::spawn(async move { c2.wait(None).await });

    let r1 = waiter1.await.expect("join1").expect("exit1");
    let r2 = waiter2.await.expect("join2").expect("exit2");
    drainer.await.ok();

    for (label, e) in [("waiter1", &r1), ("waiter2", &r2)] {
        assert!(e.aborted, "{label}: aborted must be true: {e:?}");
        assert!(e.timed_out, "{label}: timed_out must be true: {e:?}");
        assert_eq!(
            e.kind,
            Some(ProcessErrorKind::Timeout),
            "{label}: kind must be Timeout: {e:?}"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Defect 3: UTF-8 multibyte split corruption.
//
// Korean syllables encode as 3 UTF-8 bytes. When a line exceeds
// `max_line_bytes` mid-character, the pre-fix reader cut at the byte
// boundary and `from_utf8_lossy` replaced the half-bytes with U+FFFD on
// BOTH sides of the split — corrupting every character that straddled
// any chunk boundary in a long line.
//
// The fix splits at the largest UTF-8-valid prefix and carries incomplete
// trailing bytes into the next chunk, so concatenating all `partial:true`
// chunks of a single line reconstructs the exact original text without any
// U+FFFD replacement chars.
// ─────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn utf8_multibyte_survives_overflow_split() {
    let runner = TokioProcessRunner::new();
    // "안녕하세요" (5 syllables × 3 bytes = 15 bytes) repeated 50× = 750
    // bytes — well above the 128-byte max_line_bytes we set below, forcing
    // multiple force-splits inside multibyte runs. Append \n so the line
    // ultimately terminates and we exit the read loop.
    //
    // PowerShell -Command supports UTF-8 output via `[Console]::OutputEncoding`.
    let cmd = "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; \
               $s = '안녕하세요' * 50; Write-Host $s";
    let spec = ProcessSpec::new(
        vec![
            "powershell.exe".into(),
            "-NoProfile".into(),
            "-Command".into(),
            cmd.into(),
        ],
        cwd(),
    )
    .with_env(windows_env());

    // Force overflow splits with a tiny max_line_bytes.
    let mut spec = spec;
    spec.max_line_bytes = 128;

    let ProcessHandle { control, mut lines } = runner.spawn(spec).await.expect("spawn");

    let mut joined = String::new();
    let mut saw_partial = false;
    while let Some(l) = lines.recv().await {
        if l.stream != Stream::Stdout {
            continue;
        }
        if l.partial {
            saw_partial = true;
        }
        joined.push_str(&l.line);
    }
    let _ = control.wait(Some(Duration::from_secs(10))).await.expect("exit");

    let expected: String = "안녕하세요".repeat(50);
    assert!(saw_partial, "test must actually exercise overflow splitting");
    assert!(
        !joined.contains('\u{FFFD}'),
        "U+FFFD replacement char indicates a multibyte split: {:?}",
        &joined.chars().take(40).collect::<String>()
    );
    assert!(
        joined.contains(&expected),
        "concatenated lines must contain the full Korean payload uncorrupted"
    );
}

// Defect 3 sub-cases c-1/c-2/c-3 are tested as unit tests inside
// `src/process/runner.rs` (using `Cursor<Vec<u8>>` for byte-level control)
// because Windows `echo` always emits CRLF — the trailing \r interacts with
// `max_line_bytes` boundaries in ways that make subprocess-based assertions
// non-deterministic for c-3 specifically. See `tests` module in runner.rs.

// Smoke regression: the integration path still doesn't emit empty-partial
// chunks on real PowerShell UTF-8 output across an overflow boundary.
#[tokio::test]
async fn utf8_overflow_does_not_emit_empty_partial_on_zero_cut() {
    let runner = TokioProcessRunner::new();
    // "안녕" — 6 bytes total. With max_line_bytes=4, after 4 bytes the cut
    // is 3 (one full "안"); we emit "안" partial. The carry is the first
    // 1 byte of "녕". Next iteration must NOT emit an empty chunk before
    // pulling the remaining 2 bytes of "녕".
    let cmd = "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; \
               Write-Host '안녕'";
    let mut spec = ProcessSpec::new(
        vec![
            "powershell.exe".into(),
            "-NoProfile".into(),
            "-Command".into(),
            cmd.into(),
        ],
        cwd(),
    )
    .with_env(windows_env());
    spec.max_line_bytes = 4;

    let ProcessHandle { control, mut lines } = runner.spawn(spec).await.expect("spawn");
    let mut emitted = Vec::new();
    while let Some(l) = lines.recv().await {
        if l.stream == Stream::Stdout {
            emitted.push(l);
        }
    }
    let _ = control.wait(Some(Duration::from_secs(5))).await.expect("exit");

    for l in &emitted {
        assert!(
            !(l.line.is_empty() && l.partial),
            "no empty partial chunks allowed (cut=0 case): {emitted:?}"
        );
    }
    let joined: String = emitted.iter().map(|l| l.line.as_str()).collect();
    assert!(
        joined.contains("안녕"),
        "concatenated emit must reconstruct payload: {joined:?}"
    );
    assert!(!joined.contains('\u{FFFD}'), "no replacement chars");
}

// Defect 3 — sub-case c-2: when CR lands at exactly the overflow boundary
// and \n follows in the next chunk, the CR must NOT be leaked into the
// concatenated line. The current implementation pushes the post-limit byte
// (here, \r) into the carry; the \n in the next iteration triggers
// got_newline and the existing trailing-CR strip emits "" without the CR.
#[tokio::test]
async fn crlf_at_overflow_boundary_strips_cr_in_concat() {
    let runner = TokioProcessRunner::new();
    // "abc\r\n" (CMD echo emits CRLF). With max=3, after pushing 'a','b','c'
    // (buf at the limit), the next byte is \r — pre-fix that emitted "abc\r"
    // partial and "" non-partial; concat yields "abc\r" (CR leaked).
    // Post-fix: \r becomes the carry byte, \n triggers got_newline next
    // iteration, the CR is stripped before emit, and concat yields "abc".
    let mut spec = ProcessSpec::new(
        vec!["cmd.exe".into(), "/c".into(), "echo abc".into()],
        cwd(),
    )
    .with_env(windows_env());
    spec.max_line_bytes = 3;

    let ProcessHandle { control, mut lines } = runner.spawn(spec).await.expect("spawn");
    let mut chunks: Vec<String> = Vec::new();
    while let Some(l) = lines.recv().await {
        if l.stream == Stream::Stdout {
            chunks.push(l.line);
        }
    }
    let _ = control.wait(Some(Duration::from_secs(5))).await.expect("exit");

    let joined = chunks.join("");
    assert!(
        !joined.contains('\r'),
        "CR must be stripped even when overflow falls on the \\r byte; got {joined:?}"
    );
    assert!(joined.contains("abc"), "payload preserved: {joined:?}");
}

// ─────────────────────────────────────────────────────────────────────────
// Defect 4: spawn errors must NOT all collapse to CliMissing.
// ─────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn spawn_enoent_is_cli_missing() {
    let runner = TokioProcessRunner::new();
    let spec = ProcessSpec::new(vec!["this-binary-does-not-exist-12345.exe".into()], cwd())
        .with_env(windows_env());
    let err = match runner.spawn(spec).await {
        Ok(_) => panic!("ENOENT spawn must fail"),
        Err(e) => e,
    };
    assert_eq!(err.kind, ProcessErrorKind::CliMissing);
}

#[tokio::test]
async fn spawn_eacces_is_permission_denied() {
    // On Windows, attempting to spawn a *directory* as a process returns
    // ERROR_ACCESS_DENIED (5), which Rust maps to ErrorKind::PermissionDenied.
    let dir = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
    let runner = TokioProcessRunner::new();
    let spec = ProcessSpec::new(vec![dir], cwd()).with_env(windows_env());
    match runner.spawn(spec).await {
        Ok(_) => panic!("expected spawn to fail on a directory path"),
        Err(e) => {
            // Some Windows versions/AVs may rewrite this to a different
            // ErrorKind; we accept either PermissionDenied (ideal) or Spawn
            // (still distinct from CliMissing — defect-4 hard requirement).
            assert!(
                matches!(
                    e.kind,
                    ProcessErrorKind::PermissionDenied | ProcessErrorKind::Spawn
                ),
                "directory spawn must classify as PermissionDenied or Spawn, not CliMissing: {e:?}"
            );
            assert_ne!(
                e.kind,
                ProcessErrorKind::CliMissing,
                "non-ENOENT must NOT collapse to CliMissing"
            );
        }
    }
}

#[tokio::test]
async fn cli_missing_kebab_wire_format_includes_new_variants() {
    // Belt-and-suspenders for the wire enum — the renderer (processEvents.ts)
    // depends on these exact kebab-case strings.
    assert_eq!(ProcessErrorKind::PermissionDenied.to_string(), "permission-denied");
    assert_eq!(ProcessErrorKind::Spawn.to_string(), "spawn");
}

// ─────────────────────────────────────────────────────────────────────────
// Sanity: byte-level smoke for the existing happy path still works.
// ─────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn happy_path_unchanged() {
    let runner = TokioProcessRunner::new();
    let spec = ProcessSpec::new(cmd_argv("echo hello"), cwd()).with_env(windows_env());
    let ProcessHandle { control, mut lines } = runner.spawn(spec).await.expect("spawn");
    let mut got = false;
    while let Some(l) = lines.recv().await {
        if l.stream == Stream::Stdout && l.line.contains("hello") {
            got = true;
        }
    }
    let exit = control
        .wait(Some(Duration::from_secs(5)))
        .await
        .expect("exit");
    assert!(got);
    assert!(exit.is_clean(), "{exit:?}");
}
