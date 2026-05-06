//! T4 — worktree create/extract/apply/reject + repo-path canonicalization.
//!
//! These tests shell out to a real `git`. They are skipped silently if `git
//! --version` is not available (Phase 1 prereq surfaces a clear error
//! elsewhere).

use std::path::Path;
use std::process::Command;

use moa_desktop_lib::git::{self, RepoKey};
use moa_desktop_lib::git::patch;
use moa_desktop_lib::git::worktree::Worktree;
use tempfile::tempdir;

fn git_available() -> bool {
    git::probe().is_ok()
}

fn init_repo(dir: &Path) {
    run(Command::new("git").arg("-C").arg(dir).args(["init", "-q", "-b", "main"]));
    run(Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["config", "user.email", "t4@test"]));
    run(Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["config", "user.name", "T4"]));
    // Pin LF so tests don't depend on the host's core.autocrlf default.
    run(Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["config", "core.autocrlf", "false"]));
    std::fs::write(dir.join("README.md"), "hello\n").unwrap();
    run(Command::new("git").arg("-C").arg(dir).args(["add", "."]));
    run(Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["commit", "-q", "-m", "init"]));
}

fn run(cmd: &mut Command) {
    let out = cmd.output().expect("spawn git");
    assert!(
        out.status.success(),
        "git failed: {:?} stderr={}",
        cmd,
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn worktree_extract_check_apply_roundtrip() {
    if !git_available() {
        eprintln!("skip: git not on PATH");
        return;
    }
    let td = tempdir().unwrap();
    let repo = td.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    init_repo(&repo);

    let wt_path = td.path().join("wt");
    let wt = Worktree::add(&repo, &wt_path, Some("t4-feat")).unwrap();

    // Worker mutates inside the worktree.
    std::fs::write(wt.path.join("README.md"), "hello\nworld\n").unwrap();
    std::fs::write(wt.path.join("new.txt"), "added\n").unwrap();

    let out_dir = td.path().join("patches");
    let p = patch::extract(&wt, &out_dir, "session1").unwrap();
    assert!(!p.is_empty(), "patch must not be empty");
    assert!(p.path.exists(), "patch file persisted");
    assert!(p.text.contains("README.md"));
    assert!(p.text.contains("new.txt"));

    // Verify against main repo: must apply cleanly.
    patch::check(&repo, &p).unwrap();

    // Apply.
    patch::apply(&repo, &p).unwrap();
    let main_readme = std::fs::read_to_string(repo.join("README.md")).unwrap();
    assert_eq!(main_readme, "hello\nworld\n");
    assert!(repo.join("new.txt").exists());

    wt.remove().unwrap();
    assert!(!wt_path.exists());
}

#[test]
fn patch_check_rejects_when_main_diverged() {
    if !git_available() {
        eprintln!("skip: git not on PATH");
        return;
    }
    let td = tempdir().unwrap();
    let repo = td.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    init_repo(&repo);

    let wt = Worktree::add(&repo, td.path().join("wt"), Some("t4-feat")).unwrap();
    std::fs::write(wt.path.join("README.md"), "hello\nfrom-worker\n").unwrap();
    let p = patch::extract(&wt, td.path().join("patches"), "s").unwrap();

    // User edited the main repo while the Worker was running.
    std::fs::write(repo.join("README.md"), "hello\nfrom-user\n").unwrap();
    run(Command::new("git").arg("-C").arg(&repo).args(["add", "."]));
    run(Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["commit", "-q", "-m", "user edit"]));

    let res = patch::check(&repo, &p);
    assert!(res.is_err(), "check must reject when context diverged");
}

#[test]
fn repo_key_canonicalization_collapses_case() {
    let td = tempdir().unwrap();
    let p = td.path().join("Repo");
    std::fs::create_dir(&p).unwrap();

    let upper = RepoKey::from_path(&p);
    let lower_str = p.to_string_lossy().to_lowercase();
    let lower = RepoKey::from_path(std::path::PathBuf::from(lower_str));
    assert_eq!(upper.key, lower.key);
}
