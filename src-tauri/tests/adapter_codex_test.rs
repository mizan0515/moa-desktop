//! Integration tests for T5b — `CodexAdapter`.
//!
//! Uses an in-test `ScriptRunner` (analogue of T5a's) to stream canned
//! `codex exec --json` lines and assert argv shape, prompt routing
//! (positional, NOT stdin), and event sequence.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use parking_lot::Mutex as PlMutex;
use tokio::sync::{mpsc, watch, Mutex};

use moa_desktop_lib::adapters::codex::{
    CodexAdapter, CodexConfig, CodexEvent, FirstPassRequest, MutationRequest,
};
use moa_desktop_lib::process::traits::{ProcessControlInner, StdinCommand};
use moa_desktop_lib::process::{
    ProcessControl, ProcessError, ProcessExit, ProcessHandle, ProcessLine, ProcessRunner,
    ProcessSpec, StdinPolicy, Stream as PStream,
};

// ---- ScriptRunner ----------------------------------------------------------

#[derive(Debug, Clone, Default)]
struct Captured {
    spec: Option<ProcessSpec>,
    stdin_chunks: Vec<Vec<u8>>,
    stdin_closed: bool,
}

#[derive(Clone)]
struct ScriptRunner {
    script: Vec<String>,
    delay: Duration,
    captured: Arc<PlMutex<Captured>>,
}

impl ScriptRunner {
    fn new(script: Vec<&str>) -> Self {
        Self {
            script: script.into_iter().map(String::from).collect(),
            delay: Duration::from_millis(5),
            captured: Arc::new(PlMutex::new(Captured::default())),
        }
    }

    fn captured(&self) -> Captured {
        self.captured.lock().clone()
    }
}

#[async_trait]
impl ProcessRunner for ScriptRunner {
    async fn spawn(&self, spec: ProcessSpec) -> Result<ProcessHandle, ProcessError> {
        if spec.argv.is_empty() {
            return Err(ProcessError::empty_argv());
        }

        let line_buf = spec.line_buf.max(1);
        let (line_tx, line_rx) = mpsc::channel::<ProcessLine>(line_buf);
        let (abort_tx, mut abort_rx) = mpsc::channel::<()>(1);
        let (exit_tx, exit_rx) = watch::channel::<Option<ProcessExit>>(None);
        let (stdin_tx, mut stdin_rx) = mpsc::channel::<StdinCommand>(8);

        self.captured.lock().spec = Some(spec.clone());

        // Mirror real runner contract: with CloseImmediately, the runner —
        // not the adapter — closes the stdin pipe right after spawn.
        let stdin_handle = if spec.stdin == StdinPolicy::CloseImmediately {
            let _ = stdin_tx.send(StdinCommand::Close).await;
            None
        } else {
            Some(stdin_tx)
        };

        let inner = Arc::new(ProcessControlInner {
            pid: 1,
            aborted: AtomicBool::new(false),
            abort_tx,
            timed_out_pending: Arc::new(AtomicBool::new(false)),
            stdin_tx: Mutex::new(stdin_handle),
            exit_watch: exit_rx,
        });
        let control = ProcessControl { inner };

        // Stdin pump — for Codex we expect a single Close (CloseImmediately).
        let cap = self.captured.clone();
        tokio::spawn(async move {
            while let Some(cmd) = stdin_rx.recv().await {
                match cmd {
                    StdinCommand::Write(bytes) => cap.lock().stdin_chunks.push(bytes),
                    StdinCommand::Close => {
                        cap.lock().stdin_closed = true;
                        break;
                    }
                }
            }
        });

        let script = self.script.clone();
        let delay = self.delay;
        tokio::spawn(async move {
            let mut seq: u64 = 0;
            let mut aborted = false;
            for line in script {
                tokio::select! {
                    _ = tokio::time::sleep(delay) => {}
                    _ = abort_rx.recv() => { aborted = true; break; }
                }
                let pl = ProcessLine {
                    seq,
                    stream: PStream::Stdout,
                    line,
                    partial: false,
                };
                seq += 1;
                if line_tx.send(pl).await.is_err() {
                    aborted = true;
                    break;
                }
            }
            drop(line_tx);
            let _ = exit_tx.send(Some(ProcessExit {
                code: if aborted { None } else { Some(0) },
                aborted,
                timed_out: false,
                stderr_tail: String::new(),
                kind: None,
            }));
        });

        Ok(ProcessHandle {
            control,
            lines: line_rx,
        })
    }
}

// ---- helpers --------------------------------------------------------------

fn cfg() -> CodexConfig {
    let mut env = std::collections::HashMap::new();
    env.insert("CODEX_HOME".into(), "C:/x/.moa-desktop/codex-home".into());
    CodexConfig {
        program: PathBuf::from("codex.exe"),
        reasoning_effort: "high".into(),
        web_search: "live".into(),
        approval_policy: "never".into(),
        guard_text: "GUARD-TEXT".into(),
        firstpass_template: "FP task={{task}} files=\n{{files}}".into(),
        mutation_template: "MUT task={{task}} wt={{worktree}}".into(),
        env,
    }
}

async fn drain(rx: &mut mpsc::Receiver<CodexEvent>) -> Vec<CodexEvent> {
    let mut out = Vec::new();
    while let Some(e) = rx.recv().await {
        out.push(e);
    }
    out
}

// ---- tests ----------------------------------------------------------------

#[tokio::test]
async fn firstpass_emits_expected_event_sequence() {
    let script = vec![
        r#"{"type":"thread.started","thread_id":"thr-A"}"#,
        r#"{"type":"turn.started"}"#,
        r#"{"type":"item.started","item":{"type":"agent_message"}}"#,
        r#"{"type":"item.completed","item":{"type":"agent_message"}}"#,
        r#"{"type":"turn.completed"}"#,
    ];
    let runner = Arc::new(ScriptRunner::new(script));
    let adapter = CodexAdapter::new(runner.clone(), cfg());

    let mut stream = adapter
        .firstpass(FirstPassRequest {
            task: "diagnose flaky test".into(),
            files: vec!["src/foo.rs:1-10".into()],
            cwd: PathBuf::from("C:/repo"),
        })
        .await
        .expect("firstpass spawn");

    let events = drain(&mut stream.events).await;

    // Sequence: ThreadStarted, TurnStarted, ItemStarted, ItemCompleted,
    // TurnCompleted, Exit.
    assert_eq!(events.len(), 6, "got {events:#?}");
    assert!(matches!(events[0], CodexEvent::ThreadStarted { .. }));
    assert!(matches!(events[1], CodexEvent::TurnStarted { .. }));
    assert!(matches!(events[2], CodexEvent::ItemStarted { .. }));
    assert!(matches!(events[3], CodexEvent::ItemCompleted { .. }));
    assert!(matches!(events[4], CodexEvent::TurnCompleted { .. }));
    match &events[5] {
        CodexEvent::Exit { exit, failed } => {
            assert!(exit.is_clean());
            assert!(!failed, "no turn.failed seen");
        }
        other => panic!("last not Exit: {other:?}"),
    }

    // Argv shape: program, then exec, with --cd <cwd> and prompt as last
    // positional. Guard text MUST appear in the prompt body (Codex has no
    // system-prompt flag).
    let cap = runner.captured();
    let spec = cap.spec.expect("spawn captured");
    assert_eq!(spec.argv[0], "codex.exe");
    assert_eq!(spec.argv[1], "exec");

    let i = spec
        .argv
        .iter()
        .position(|s| s == "--cd")
        .expect("--cd present");
    assert_eq!(spec.argv[i + 1], "C:/repo");

    let prompt = spec.argv.last().expect("prompt last").clone();
    assert!(prompt.starts_with("GUARD-TEXT"), "guard prefix missing");
    assert!(prompt.contains("FP task=diagnose flaky test"));
    assert!(prompt.contains("- src/foo.rs:1-10"));

    // sandbox: read-only present, dangerous bypass absent
    let i = spec.argv.iter().position(|s| s == "--sandbox").unwrap();
    assert_eq!(spec.argv[i + 1], "read-only");
    assert!(!spec
        .argv
        .iter()
        .any(|s| s == "--dangerously-bypass-approvals-and-sandbox"));

    // Stdin policy: closed immediately (Codex requirement, S2 finding #4).
    assert_eq!(spec.stdin, StdinPolicy::CloseImmediately);
    assert!(cap.stdin_closed, "stdin must be closed for Codex");
    assert!(
        cap.stdin_chunks.is_empty(),
        "Codex prompt must NOT be written to stdin (it's argv-positional)"
    );
}

#[tokio::test]
async fn turn_failed_propagates_to_exit_failed_flag() {
    let script = vec![
        r#"{"type":"thread.started","thread_id":"thr-B"}"#,
        r#"{"type":"turn.started"}"#,
        r#"{"type":"turn.failed","error":{"message":"sandbox blocked write"}}"#,
    ];
    let runner = Arc::new(ScriptRunner::new(script));
    let adapter = CodexAdapter::new(runner, cfg());

    let mut stream = adapter
        .firstpass(FirstPassRequest {
            task: "x".into(),
            files: vec![],
            cwd: PathBuf::from("C:/repo"),
        })
        .await
        .expect("spawn");

    let events = drain(&mut stream.events).await;
    let failed_ev = events
        .iter()
        .find(|e| matches!(e, CodexEvent::TurnFailed { .. }))
        .expect("TurnFailed emitted");
    match failed_ev {
        CodexEvent::TurnFailed { error_message, .. } => {
            assert_eq!(error_message.as_deref(), Some("sandbox blocked write"))
        }
        _ => unreachable!(),
    }
    let exit_ev = events.last().unwrap();
    match exit_ev {
        CodexEvent::Exit { failed, .. } => assert!(*failed, "exit must carry failed=true"),
        other => panic!("last not Exit: {other:?}"),
    }
}

#[tokio::test]
async fn mutation_uses_worktree_cwd_and_dangerous_bypass_argv() {
    let script = vec![r#"{"type":"turn.completed"}"#];
    let runner = Arc::new(ScriptRunner::new(script));
    let adapter = CodexAdapter::new(runner.clone(), cfg());

    let tmp = tempfile::tempdir().expect("tempdir");
    let (repo_root, wt_path) = create_repo_with_registered_moa_worktree(tmp.path(), "session-1");

    let mut stream = adapter
        .mutation(MutationRequest {
            task: "apply patch".into(),
            repo_root: repo_root.clone(),
            worktree_path: wt_path.clone(),
        })
        .await
        .expect("spawn");

    let _ = drain(&mut stream.events).await;

    let cap = runner.captured();
    let spec = cap.spec.expect("spec captured");
    assert_eq!(spec.cwd, wt_path);

    assert!(spec
        .argv
        .iter()
        .any(|s| s == "--dangerously-bypass-approvals-and-sandbox"));
    assert!(
        !spec.argv.iter().any(|s| s == "--sandbox"),
        "mutation must not carry --sandbox (workspace-write broken on Windows)"
    );

    let i = spec.argv.iter().position(|s| s == "--cd").unwrap();
    assert_eq!(spec.argv[i + 1], wt_path.to_string_lossy());

    let prompt = spec.argv.last().unwrap().clone();
    assert!(prompt.starts_with("GUARD-TEXT"));
    assert!(prompt.contains("MUT task=apply patch"));
    assert!(prompt.contains(&wt_path.to_string_lossy().into_owned()));
}

#[tokio::test]
async fn mutation_rejects_unregistered_existing_moa_worktree_dir() {
    let runner = Arc::new(ScriptRunner::new(vec![r#"{"type":"turn.completed"}"#]));
    let adapter = CodexAdapter::new(runner.clone(), cfg());

    let tmp = tempfile::tempdir().expect("tempdir");
    let repo_root = create_git_repo(tmp.path(), "repo");
    let wt_path = repo_root
        .join(".moa-desktop")
        .join("worktrees")
        .join("not-registered");
    std::fs::create_dir_all(&wt_path).expect("fake worktree dir");

    let result = adapter
        .mutation(MutationRequest {
            task: "apply patch".into(),
            repo_root,
            worktree_path: wt_path,
        })
        .await;
    let err = match result {
        Ok(_) => panic!("unregistered directory must be rejected"),
        Err(err) => err,
    };

    assert_eq!(
        err.kind,
        moa_desktop_lib::process::ProcessErrorKind::PermissionDenied
    );
    assert!(
        runner.captured().spec.is_none(),
        "guard must reject before spawning Codex bypass mode"
    );
}

#[tokio::test]
async fn mutation_rejects_unregistered_git_repo_under_moa_worktree_parent() {
    let runner = Arc::new(ScriptRunner::new(vec![r#"{"type":"turn.completed"}"#]));
    let adapter = CodexAdapter::new(runner.clone(), cfg());

    let tmp = tempfile::tempdir().expect("tempdir");
    let repo_root = create_git_repo(tmp.path(), "repo");
    let worktrees_parent = repo_root.join(".moa-desktop").join("worktrees");
    std::fs::create_dir_all(&worktrees_parent).expect("worktree parent");
    let wt_path = create_git_repo(&worktrees_parent, "not-registered");

    let result = adapter
        .mutation(MutationRequest {
            task: "apply patch".into(),
            repo_root,
            worktree_path: wt_path,
        })
        .await;
    let err = match result {
        Ok(_) => panic!("unregistered nested git repository must be rejected"),
        Err(err) => err,
    };

    assert_eq!(
        err.kind,
        moa_desktop_lib::process::ProcessErrorKind::PermissionDenied
    );
    assert!(
        err.message.contains("not registered in git worktree list"),
        "expected registration-branch denial, got: {}",
        err.message
    );
    assert!(
        runner.captured().spec.is_none(),
        "guard must reject before spawning Codex bypass mode"
    );
}

#[tokio::test]
async fn mutation_rejects_registered_worktree_from_different_repo_root() {
    let runner = Arc::new(ScriptRunner::new(vec![r#"{"type":"turn.completed"}"#]));
    let adapter = CodexAdapter::new(runner.clone(), cfg());

    let tmp = tempfile::tempdir().expect("tempdir");
    let (_repo_a, wt_path) = create_repo_with_registered_moa_worktree(tmp.path(), "session-1");
    let repo_b = create_git_repo(tmp.path(), "repo-b");
    std::fs::create_dir_all(repo_b.join(".moa-desktop").join("worktrees"))
        .expect("repo-b worktree parent");

    let result = adapter
        .mutation(MutationRequest {
            task: "apply patch".into(),
            repo_root: repo_b,
            worktree_path: wt_path,
        })
        .await;
    let err = match result {
        Ok(_) => panic!("worktree from another repo must be rejected"),
        Err(err) => err,
    };

    assert_eq!(
        err.kind,
        moa_desktop_lib::process::ProcessErrorKind::PermissionDenied
    );
    assert!(
        runner.captured().spec.is_none(),
        "guard must reject before spawning Codex bypass mode"
    );
}

#[tokio::test]
async fn mutation_rejects_traversal_path_before_spawn() {
    let runner = Arc::new(ScriptRunner::new(vec![r#"{"type":"turn.completed"}"#]));
    let adapter = CodexAdapter::new(runner.clone(), cfg());

    let tmp = tempfile::tempdir().expect("tempdir");
    let repo_root = create_git_repo(tmp.path(), "repo");
    let wt_path = repo_root
        .join(".moa-desktop")
        .join("worktrees")
        .join("..")
        .join("session-1");

    let result = adapter
        .mutation(MutationRequest {
            task: "apply patch".into(),
            repo_root,
            worktree_path: wt_path,
        })
        .await;
    let err = match result {
        Ok(_) => panic!("traversal worktree path must be rejected"),
        Err(err) => err,
    };

    assert_eq!(
        err.kind,
        moa_desktop_lib::process::ProcessErrorKind::PermissionDenied
    );
    assert!(
        runner.captured().spec.is_none(),
        "guard must reject traversal before spawning Codex bypass mode"
    );
}

#[tokio::test]
async fn mutation_rejects_repo_root_that_is_git_subdirectory() {
    let runner = Arc::new(ScriptRunner::new(vec![r#"{"type":"turn.completed"}"#]));
    let adapter = CodexAdapter::new(runner.clone(), cfg());

    let tmp = tempfile::tempdir().expect("tempdir");
    let (repo_root, wt_path) = create_repo_with_registered_moa_worktree(tmp.path(), "session-1");
    let subdir = repo_root.join("nested");
    std::fs::create_dir_all(&subdir).expect("repo subdir");

    let result = adapter
        .mutation(MutationRequest {
            task: "apply patch".into(),
            repo_root: subdir,
            worktree_path: wt_path,
        })
        .await;
    let err = match result {
        Ok(_) => panic!("repo_root subdirectory must be rejected"),
        Err(err) => err,
    };

    assert_eq!(
        err.kind,
        moa_desktop_lib::process::ProcessErrorKind::PermissionDenied
    );
    assert!(
        runner.captured().spec.is_none(),
        "guard must reject subdirectory repo_root before spawning Codex bypass mode"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn mutation_rejects_symlinked_worktree_parent_escape() {
    use std::os::unix::fs::symlink;

    let runner = Arc::new(ScriptRunner::new(vec![r#"{"type":"turn.completed"}"#]));
    let adapter = CodexAdapter::new(runner.clone(), cfg());

    let tmp = tempfile::tempdir().expect("tempdir");
    let repo_root = create_git_repo(tmp.path(), "repo");
    let outside = tmp.path().join("outside-worktrees");
    std::fs::create_dir_all(&outside).expect("outside dir");
    std::fs::create_dir_all(repo_root.join(".moa-desktop")).expect("moa dir");
    symlink(&outside, repo_root.join(".moa-desktop").join("worktrees")).expect("symlink");
    let wt_path = repo_root
        .join(".moa-desktop")
        .join("worktrees")
        .join("session-escape");
    run_git(
        &repo_root,
        &[
            "worktree",
            "add",
            "-b",
            "test-session-escape",
            path_str(&wt_path),
        ],
    );

    let result = adapter
        .mutation(MutationRequest {
            task: "apply patch".into(),
            repo_root,
            worktree_path: wt_path,
        })
        .await;
    let err = match result {
        Ok(_) => panic!("symlinked worktree parent escape must be rejected"),
        Err(err) => err,
    };

    assert_eq!(
        err.kind,
        moa_desktop_lib::process::ProcessErrorKind::PermissionDenied
    );
    assert!(
        runner.captured().spec.is_none(),
        "guard must reject canonical parent escape before spawning Codex bypass mode"
    );
}

#[cfg(windows)]
#[tokio::test]
async fn mutation_rejects_junctioned_worktree_parent_escape() {
    let runner = Arc::new(ScriptRunner::new(vec![r#"{"type":"turn.completed"}"#]));
    let adapter = CodexAdapter::new(runner.clone(), cfg());

    let tmp = tempfile::tempdir().expect("tempdir");
    let repo_root = create_git_repo(tmp.path(), "repo");
    let outside = tmp.path().join("outside-worktrees");
    std::fs::create_dir_all(&outside).expect("outside dir");
    std::fs::create_dir_all(repo_root.join(".moa-desktop")).expect("moa dir");
    create_junction(&outside, &repo_root.join(".moa-desktop").join("worktrees"));
    let wt_path = repo_root
        .join(".moa-desktop")
        .join("worktrees")
        .join("session-escape");
    run_git(
        &repo_root,
        &[
            "worktree",
            "add",
            "-b",
            "test-session-escape",
            path_str(&wt_path),
        ],
    );

    let result = adapter
        .mutation(MutationRequest {
            task: "apply patch".into(),
            repo_root,
            worktree_path: wt_path,
        })
        .await;
    let err = match result {
        Ok(_) => panic!("junctioned worktree parent escape must be rejected"),
        Err(err) => err,
    };

    assert_eq!(
        err.kind,
        moa_desktop_lib::process::ProcessErrorKind::PermissionDenied
    );
    assert!(
        err.message.contains("escapes the git top-level")
            || err.message.contains("not a direct child")
            || err.message.contains("reparse/junction alias"),
        "unexpected error: {}",
        err.message
    );
    assert!(
        runner.captured().spec.is_none(),
        "guard must reject canonical parent escape before spawning Codex bypass mode"
    );
}

#[cfg(windows)]
#[tokio::test]
async fn mutation_rejects_in_repo_junctioned_worktree_parent_alias() {
    let runner = Arc::new(ScriptRunner::new(vec![r#"{"type":"turn.completed"}"#]));
    let adapter = CodexAdapter::new(runner.clone(), cfg());

    let tmp = tempfile::tempdir().expect("tempdir");
    let repo_root = create_git_repo(tmp.path(), "repo");
    let alias = repo_root.join("src").join(".worktrees");
    std::fs::create_dir_all(&alias).expect("in-repo alias target");
    std::fs::create_dir_all(repo_root.join(".moa-desktop")).expect("moa dir");
    create_junction(&alias, &repo_root.join(".moa-desktop").join("worktrees"));
    let wt_path = repo_root
        .join(".moa-desktop")
        .join("worktrees")
        .join("session-alias");

    let result = adapter
        .mutation(MutationRequest {
            task: "apply patch".into(),
            repo_root,
            worktree_path: wt_path,
        })
        .await;
    let err = match result {
        Ok(_) => panic!("in-repo junction alias must be rejected"),
        Err(err) => err,
    };

    assert_eq!(
        err.kind,
        moa_desktop_lib::process::ProcessErrorKind::PermissionDenied
    );
    assert!(
        err.message.contains("reparse/junction alias"),
        "unexpected error: {}",
        err.message
    );
    assert!(
        runner.captured().spec.is_none(),
        "guard must reject alias before spawning Codex bypass mode"
    );
}

fn create_repo_with_registered_moa_worktree(base: &Path, session: &str) -> (PathBuf, PathBuf) {
    let repo_root = create_git_repo(base, "repo");
    let wt_path = repo_root
        .join(".moa-desktop")
        .join("worktrees")
        .join(session);
    std::fs::create_dir_all(wt_path.parent().expect("worktree parent"))
        .expect("worktree parent dir");
    run_git(
        &repo_root,
        &[
            "worktree",
            "add",
            "-b",
            &format!("test-{session}"),
            path_str(&wt_path),
        ],
    );
    (repo_root, wt_path)
}

fn create_git_repo(base: &Path, name: &str) -> PathBuf {
    let repo_root = base.join(name);
    std::fs::create_dir_all(&repo_root).expect("repo dir");
    run_git(&repo_root, &["init"]);
    std::fs::write(repo_root.join("README.md"), "test\n").expect("seed file");
    run_git(&repo_root, &["add", "README.md"]);
    run_git(
        &repo_root,
        &[
            "-c",
            "user.name=MoA Test",
            "-c",
            "user.email=moa-test@example.invalid",
            "commit",
            "-m",
            "init",
        ],
    );
    repo_root
}

fn run_git(cwd: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("spawn git");
    assert!(
        output.status.success(),
        "git {:?} failed\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(windows)]
fn create_junction(target: &Path, link: &Path) {
    let output = Command::new("cmd")
        .args(["/C", "mklink", "/J", path_str(link), path_str(target)])
        .output()
        .expect("spawn mklink");
    assert!(
        output.status.success(),
        "mklink /J failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn path_str(path: &Path) -> &str {
    path.to_str().expect("path is valid UTF-8")
}

#[tokio::test]
async fn malformed_json_line_surfaces_event_then_continues() {
    let script = vec![
        r#"{"type":"thread.started","thread_id":"thr-C"}"#,
        r#"NOT JSON"#,
        r#"{"type":"turn.completed"}"#,
    ];
    let runner = Arc::new(ScriptRunner::new(script));
    let adapter = CodexAdapter::new(runner, cfg());

    let mut stream = adapter
        .firstpass(FirstPassRequest {
            task: "t".into(),
            files: vec![],
            cwd: PathBuf::from("C:/repo"),
        })
        .await
        .expect("spawn");

    let events = drain(&mut stream.events).await;
    let mj = events
        .iter()
        .find(|e| matches!(e, CodexEvent::MalformedJson { .. }))
        .expect("MalformedJson emitted");
    if let CodexEvent::MalformedJson { line, .. } = mj {
        assert_eq!(line, "NOT JSON");
    }
    assert!(events
        .iter()
        .any(|e| matches!(e, CodexEvent::TurnCompleted { .. })));
    assert!(matches!(events.last(), Some(CodexEvent::Exit { .. })));
}

#[tokio::test]
async fn item_completed_with_warning_payload_does_not_mark_failed() {
    // S8: deprecation warnings emit as item.completed with an error
    // payload — the run continues normally.
    let script = vec![
        r#"{"type":"thread.started","thread_id":"thr-D"}"#,
        r#"{"type":"item.completed","item":{"type":"error"},"error":{"message":"[features].web_search deprecated"}}"#,
        r#"{"type":"turn.completed"}"#,
    ];
    let runner = Arc::new(ScriptRunner::new(script));
    let adapter = CodexAdapter::new(runner, cfg());

    let mut stream = adapter
        .firstpass(FirstPassRequest {
            task: "t".into(),
            files: vec![],
            cwd: PathBuf::from("C:/repo"),
        })
        .await
        .expect("spawn");

    let events = drain(&mut stream.events).await;
    let warn = events
        .iter()
        .find(|e| matches!(e, CodexEvent::ItemCompleted { .. }))
        .expect("ItemCompleted emitted");
    match warn {
        CodexEvent::ItemCompleted {
            error_message,
            item_type,
            ..
        } => {
            assert_eq!(item_type.as_deref(), Some("error"));
            assert!(error_message
                .as_deref()
                .unwrap_or("")
                .contains("deprecated"));
        }
        _ => unreachable!(),
    }
    let exit_ev = events.last().unwrap();
    match exit_ev {
        CodexEvent::Exit { failed, .. } => {
            assert!(!failed, "non-blocking warning must NOT fail the run")
        }
        other => panic!("last not Exit: {other:?}"),
    }
}

#[tokio::test]
async fn firstpass_spec_carries_env_inherit_allowlist() {
    // Regression: see CodexConfig.env contract — the runner must inherit
    // USERPROFILE / APPDATA / PATH / PATHEXT / SystemRoot etc. so codex.exe
    // can find auth.json + config.toml + npm shims on Windows.
    let script = vec![r#"{"type":"thread.started","thread_id":"thr-env"}"#];
    let runner = Arc::new(ScriptRunner::new(script));
    let adapter = CodexAdapter::new(runner.clone(), cfg());

    let _ = adapter
        .firstpass(FirstPassRequest {
            task: "t".into(),
            files: vec![],
            cwd: PathBuf::from("C:/repo"),
        })
        .await
        .expect("spawn");

    let cap = runner.captured();
    let spec = cap.spec.expect("spec captured");
    let inherited: std::collections::HashSet<&str> =
        spec.env_inherit.iter().map(String::as_str).collect();

    #[cfg(windows)]
    {
        for key in [
            "USERPROFILE",
            "APPDATA",
            "LOCALAPPDATA",
            "PATH",
            "PATHEXT",
            "SystemRoot",
            "TEMP",
        ] {
            assert!(
                inherited.contains(key),
                "env_inherit must include {key}; got {inherited:?}"
            );
        }
    }
    #[cfg(unix)]
    {
        for key in ["PATH", "HOME"] {
            assert!(
                inherited.contains(key),
                "env_inherit must include {key}; got {inherited:?}"
            );
        }
    }

    // Plugin-specific env still wins (CODEX_HOME present from cfg()).
    assert!(spec.env.contains_key("CODEX_HOME"));
}
