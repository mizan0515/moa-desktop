//! T4 — file hash snapshot/diff.

use moa_desktop_lib::safety::{diff, snapshot_dir};
use tempfile::tempdir;

#[test]
fn snapshot_diff_detects_add_modify_remove() {
    let td = tempdir().unwrap();
    std::fs::write(td.path().join("a.txt"), "1\n").unwrap();
    std::fs::write(td.path().join("b.txt"), "two\n").unwrap();

    let before = snapshot_dir(td.path()).unwrap();
    assert_eq!(before.len(), 2);

    // modify a, remove b, add c
    std::fs::write(td.path().join("a.txt"), "1+changed\n").unwrap();
    std::fs::remove_file(td.path().join("b.txt")).unwrap();
    std::fs::write(td.path().join("c.txt"), "3\n").unwrap();

    let after = snapshot_dir(td.path()).unwrap();
    let d = diff(&before, &after);
    assert_eq!(d.added, vec!["c.txt"]);
    assert_eq!(d.removed, vec!["b.txt"]);
    assert_eq!(d.modified, vec!["a.txt"]);
}

#[test]
fn snapshot_skips_dot_git() {
    let td = tempdir().unwrap();
    std::fs::create_dir(td.path().join(".git")).unwrap();
    std::fs::write(td.path().join(".git/HEAD"), "ref: x\n").unwrap();
    std::fs::write(td.path().join("real.txt"), "hi\n").unwrap();

    let snap = snapshot_dir(td.path()).unwrap();
    assert_eq!(snap.len(), 1);
    assert!(snap.contains_key("real.txt"));
}

#[test]
fn identical_content_yields_identical_hash() {
    let td = tempdir().unwrap();
    std::fs::write(td.path().join("x"), b"abc").unwrap();
    std::fs::write(td.path().join("y"), b"abc").unwrap();
    let snap = snapshot_dir(td.path()).unwrap();
    assert_eq!(snap["x"], snap["y"]);
    assert_eq!(snap["x"].len(), 64);
}
