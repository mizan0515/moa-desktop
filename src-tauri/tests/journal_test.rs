//! T4 — journal append / fsync / reconcile / torn-tail tolerance.

use std::collections::BTreeMap;

use moa_desktop_lib::journal::schema::{Entry, Phase};
use moa_desktop_lib::journal::{read_all, scan, JournalWriter};
use moa_desktop_lib::lock::manager::Worker;
use tempfile::tempdir;

fn entry(phase: Phase) -> Entry {
    Entry {
        seq: 0,
        ts_ms: 0,
        phase,
        owner: Some(Worker::Claude),
        pid: 0,
        base_hashes: BTreeMap::new(),
        patch_path: None,
        note: None,
    }
}

#[test]
fn append_assigns_seq_and_persists() {
    let td = tempdir().unwrap();
    let w = JournalWriter::open(td.path(), "proj", "sess1").unwrap();
    let s1 = w.append(entry(Phase::SessionStart)).unwrap();
    let s2 = w.append(entry(Phase::WorkerStarted)).unwrap();
    let s3 = w.append(entry(Phase::PatchVerified)).unwrap();
    assert_eq!((s1, s2, s3), (1, 2, 3));

    let entries = read_all(w.path()).unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[2].phase, Phase::PatchVerified);
    assert!(entries[2].pid > 0);
    assert!(entries[2].ts_ms > 0);
}

#[test]
fn reopen_resumes_seq() {
    let td = tempdir().unwrap();
    {
        let w = JournalWriter::open(td.path(), "p", "s").unwrap();
        w.append(entry(Phase::SessionStart)).unwrap();
        w.append(entry(Phase::WorkerStarted)).unwrap();
    }
    let w2 = JournalWriter::open(td.path(), "p", "s").unwrap();
    let s = w2.append(entry(Phase::WorkerFinished)).unwrap();
    assert_eq!(s, 3);
}

#[test]
fn reader_tolerates_torn_tail() {
    let td = tempdir().unwrap();
    let w = JournalWriter::open(td.path(), "p", "s").unwrap();
    w.append(entry(Phase::SessionStart)).unwrap();
    w.append(entry(Phase::WorkerStarted)).unwrap();
    drop(w);

    // Append a partial line (no newline + invalid JSON tail).
    use std::io::Write;
    let path = td.path().join("journals/p/s.jsonl");
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap();
    f.write_all(b"{\"seq\":3,\"ts_ms\":").unwrap();
    drop(f);

    let entries = read_all(&path).unwrap();
    assert_eq!(entries.len(), 2, "torn tail dropped, prefix survives");
}

#[test]
fn reconcile_lists_unfinished_sessions() {
    let td = tempdir().unwrap();

    // Session A — ended cleanly.
    let a = JournalWriter::open(td.path(), "proj", "a").unwrap();
    a.append(entry(Phase::SessionStart)).unwrap();
    a.append(entry(Phase::SessionEnd)).unwrap();
    drop(a);

    // Session B — abnormal exit (no SessionEnd).
    let b = JournalWriter::open(td.path(), "proj", "b").unwrap();
    b.append(entry(Phase::SessionStart)).unwrap();
    b.append(entry(Phase::WorktreeCreated)).unwrap();
    b.append(entry(Phase::WorkerStarted)).unwrap();
    drop(b);

    let unfinished = scan(td.path()).unwrap();
    assert_eq!(unfinished.len(), 1);
    assert_eq!(unfinished[0].session_id, "b");
    assert_eq!(unfinished[0].last_phase, Some(Phase::WorkerStarted));
}

#[test]
fn parallel_appends_serialize_via_per_session_writer() {
    use std::sync::Arc;
    use std::thread;

    let td = tempdir().unwrap();
    let w = Arc::new(JournalWriter::open(td.path(), "p", "s").unwrap());

    let mut handles = Vec::new();
    for _ in 0..8 {
        let w = w.clone();
        handles.push(thread::spawn(move || {
            for _ in 0..32 {
                w.append(entry(Phase::WorkerStarted)).unwrap();
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }

    let entries = read_all(w.path()).unwrap();
    assert_eq!(entries.len(), 8 * 32);
    // seq is monotonic and dense.
    let mut seqs: Vec<u64> = entries.iter().map(|e| e.seq).collect();
    seqs.sort();
    for (i, s) in seqs.iter().enumerate() {
        assert_eq!(*s, (i as u64) + 1);
    }
}
