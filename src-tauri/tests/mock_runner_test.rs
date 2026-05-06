//! Integration tests for T8 — `MockRunner`.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use moa_desktop_lib::mock::MockRunner;
use moa_desktop_lib::process::{
    ProcessErrorKind, ProcessHandle, ProcessRunner, ProcessSpec, Stream,
};

fn repo_root() -> PathBuf {
    // Cargo runs the test binary with CARGO_MANIFEST_DIR = src-tauri/. Project
    // root (where mockResponses/ lives) is its parent.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().expect("repo root above src-tauri").to_path_buf()
}

fn mock_path(name: &str) -> PathBuf {
    repo_root().join("mockResponses").join(name)
}

#[tokio::test]
async fn streams_six_canned_lines_with_100ms_delay() {
    // claude_firstpass.json has exactly 6 non-empty JSONL lines (start +
    // 3 claims + 1 open_question + end). The mock runner should emit each
    // one as a stdout line spaced by ~100 ms.
    let path = mock_path("claude_firstpass.json");
    assert!(path.exists(), "missing canned file: {:?}", path);

    let runner = MockRunner::new(&path);
    let spec = ProcessSpec::new(vec!["mock".into()], repo_root());

    let ProcessHandle { control, mut lines } = runner.spawn(spec).await.expect("spawn");

    let start = Instant::now();
    let mut received: Vec<String> = Vec::new();
    let mut last_seq: Option<u64> = None;

    while let Some(pl) = lines.recv().await {
        assert_eq!(pl.stream, Stream::Stdout, "mock emits stdout only");
        assert!(!pl.partial, "canned lines fit within max_line_bytes");
        if let Some(prev) = last_seq {
            assert_eq!(pl.seq, prev + 1, "seq must be monotonic + dense");
        } else {
            assert_eq!(pl.seq, 0, "first seq is 0");
        }
        last_seq = Some(pl.seq);
        // Validate each line parses as JSON (worker schema invariant).
        let v: serde_json::Value =
            serde_json::from_str(&pl.line).expect("canned line is valid JSON");
        assert!(v.get("event").is_some(), "every event has an `event` field");
        received.push(pl.line);
    }
    let elapsed = start.elapsed();

    assert_eq!(received.len(), 6, "claude_firstpass.json has 6 events");

    // 6 lines × 100 ms = 600 ms minimum. Allow generous upper bound for CI.
    assert!(
        elapsed >= Duration::from_millis(550),
        "streaming took only {:?} — delay not honored",
        elapsed
    );
    assert!(
        elapsed < Duration::from_millis(2_500),
        "streaming took {:?} — too slow",
        elapsed
    );

    let exit = control
        .wait(Some(Duration::from_secs(2)))
        .await
        .expect("exit");
    assert!(exit.is_clean(), "mock should exit cleanly: {:?}", exit);
}

#[tokio::test]
async fn all_six_canned_files_exist_and_parse() {
    for name in [
        "claude_firstpass.json",
        "codex_firstpass.json",
        "synthesis.json",
        "claude_adversarial.json",
        "codex_adversarial.json",
        "final_report.json",
    ] {
        let p = mock_path(name);
        let raw = std::fs::read_to_string(&p)
            .unwrap_or_else(|e| panic!("read {:?}: {e}", p));
        let line_count = raw.lines().filter(|l| !l.trim().is_empty()).count();
        assert!(line_count >= 4, "{name} too sparse ({line_count} lines)");
        for (i, line) in raw.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            serde_json::from_str::<serde_json::Value>(line)
                .unwrap_or_else(|e| panic!("{name} line {i} invalid JSON: {e}\n{line}"));
        }
    }
}

#[tokio::test]
async fn abort_stops_streaming_early() {
    let path = mock_path("claude_firstpass.json");
    // Use a long delay so we can reliably abort mid-stream.
    let runner = MockRunner::new(&path).with_delay(Duration::from_millis(500));
    let spec = ProcessSpec::new(vec!["mock".into()], repo_root());

    let ProcessHandle { control, mut lines } = runner.spawn(spec).await.expect("spawn");

    let abort_ctl = control.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(150)).await;
        let _ = abort_ctl.abort().await;
    });

    let start = Instant::now();
    let mut count = 0;
    while let Some(_) = lines.recv().await {
        count += 1;
    }
    let elapsed = start.elapsed();

    assert!(count < 6, "abort did not interrupt: got {count} lines");
    assert!(
        elapsed < Duration::from_millis(1_500),
        "abort did not shorten run: {:?}",
        elapsed
    );

    let exit = control
        .wait(Some(Duration::from_secs(2)))
        .await
        .expect("exit");
    assert!(exit.aborted, "exit must be marked aborted: {:?}", exit);
    assert_eq!(exit.code, None, "aborted exit has no code");
    assert_eq!(
        exit.kind,
        Some(ProcessErrorKind::Killed),
        "aborted mock exit must classify as Killed (parity with TokioProcessRunner)"
    );
}

#[tokio::test]
async fn rejects_empty_argv() {
    let runner = MockRunner::new(mock_path("claude_firstpass.json"));
    let spec = ProcessSpec::new(vec![], repo_root());
    let err = match runner.spawn(spec).await {
        Ok(_) => panic!("empty argv must error"),
        Err(e) => e,
    };
    // empty_argv is classified as CliMissing by ProcessError::empty_argv().
    assert_eq!(err.kind, ProcessErrorKind::CliMissing);
}

#[tokio::test]
async fn synthesis_open_question_count_matches_final_report() {
    // Schema invariant: final_report.summary.open_questions must equal the
    // number of `column=open` rows in synthesis.json.
    let synth = std::fs::read_to_string(mock_path("synthesis.json")).expect("read synth");
    let open_rows = synth
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|v| {
            v.get("event").and_then(|e| e.as_str()) == Some("row")
                && v.get("column").and_then(|c| c.as_str()) == Some("open")
        })
        .count();

    let final_raw =
        std::fs::read_to_string(mock_path("final_report.json")).expect("read final");
    let summary = final_raw
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .find(|v| v.get("event").and_then(|e| e.as_str()) == Some("summary"))
        .expect("summary event");
    let declared = summary
        .get("open_questions")
        .and_then(|n| n.as_u64())
        .expect("open_questions number") as usize;

    assert_eq!(
        open_rows, declared,
        "synthesis open rows ({open_rows}) must match final_report.open_questions ({declared})"
    );
}
