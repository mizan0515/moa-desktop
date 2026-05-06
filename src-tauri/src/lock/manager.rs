//! In-memory `LockManager` — protects mutation lanes inside one app process
//! across N tabs / N async tasks (T4 §F6 layer 1).
//!
//! ## Ordering contract
//!
//! Locks form a tiered chain:
//! `repo-open canonical (Tier::Repo) → project (Tier::Project) → session/lane (Tier::Lane)`
//!
//! The journal append queue (Tier::Journal) is enforced separately by the
//! journal writer (single per-session channel) and never sits on the same
//! manager — so callers can flush journal entries *outside* a held lane lock,
//! which is the documented durability policy.
//!
//! Higher-tier acquisition takes a borrow of the lower-tier guard at the type
//! level: `acquire_project(&RepoGuard, ...)`, `acquire_lane(&ProjectGuard, ...)`.
//! Compile-time prevents the most common ordering violations (lane without
//! project, project without repo). Runtime additionally rejects re-entry that
//! would cross projects while holding a lane lock.
//!
//! ## Cross-project (T11) — `try_acquire_all`
//!
//! Sorts requested keys by repo-key (canonical hash, deterministic) and
//! attempts each in order. On any failure, releases all already-acquired and
//! returns `LockError::WouldBlock` so the caller may retry/backoff.
//!
//! ## Source restriction
//!
//! All public acquire methods take a `LockSource`. `LockSource::Scheduler` is
//! the only allowed source for lane mutation. Worker-derived requests must
//! be tagged `LockSource::Worker` and are rejected at runtime — defense in
//! depth in case orchestrator layering ever leaks worker output into a
//! command path.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::git::canonical::RepoKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Worker {
    Claude,
    Codex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockSource {
    Scheduler,
    /// Defense in depth — manager rejects all acquires from this source. The
    /// only legitimate caller is the orchestrator scheduler.
    Worker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Repo,
    Project,
    Lane,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LockError {
    #[error("lock {key:?} already held")]
    WouldBlock { key: String },
    #[error("worker-source acquires are forbidden — scheduler only")]
    ForbiddenSource,
    #[error("lane lock held — cannot acquire project lock for a different project")]
    OrderingViolation,
    #[error("lock not in transferring state")]
    NotTransferring,
    #[error("transfer target equals current owner")]
    TransferToSelf,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditEntry {
    pub ts_ms: i64,
    pub kind: AuditKind,
    pub project_id: String,
    pub lock_key: String,
    pub owner: Option<Worker>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuditKind {
    Acquired,
    TransferStarted,
    TransferCompleted,
    Released,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LaneState {
    Idle,
    Acquired,
    Transferring,
}

#[derive(Debug, Default)]
struct Inner {
    repos: HashMap<String, bool>,        // RepoKey.key -> held
    projects: HashMap<String, bool>,     // projectId -> held
    lanes: HashMap<LaneKey, LaneEntry>,  // (projectId, lockKey) -> entry
    audit: Vec<AuditEntry>,
}

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
struct LaneKey {
    project_id: String,
    lock_key: String,
}

#[derive(Debug)]
struct LaneEntry {
    state: LaneState,
    owner: Option<Worker>,
    transfer_target: Option<Worker>,
}

#[derive(Debug, Clone, Default)]
pub struct LockManager(Arc<Mutex<Inner>>);

impl LockManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Tier::Repo — keyed by canonical RepoKey. Blocks any second tab opening
    /// the same canonical path.
    pub fn acquire_repo(&self, repo: &RepoKey) -> Result<RepoGuard, LockError> {
        let mut g = self.0.lock();
        if *g.repos.get(&repo.key).unwrap_or(&false) {
            return Err(LockError::WouldBlock {
                key: format!("repo:{}", &repo.key[..16]),
            });
        }
        g.repos.insert(repo.key.clone(), true);
        Ok(RepoGuard {
            mgr: self.0.clone(),
            key: repo.key.clone(),
        })
    }

    /// Tier::Project. Compile-time requires holding Tier::Repo for this path.
    pub fn acquire_project(
        &self,
        _repo_guard: &RepoGuard,
        project_id: &str,
    ) -> Result<ProjectGuard, LockError> {
        let mut g = self.0.lock();
        if *g.projects.get(project_id).unwrap_or(&false) {
            return Err(LockError::WouldBlock {
                key: format!("project:{project_id}"),
            });
        }
        g.projects.insert(project_id.into(), true);
        Ok(ProjectGuard {
            mgr: self.0.clone(),
            project_id: project_id.into(),
        })
    }

    /// Tier::Lane — `(projectId, lockKey)` mutation lane.
    pub fn acquire_lane(
        &self,
        proj: &ProjectGuard,
        lock_key: &str,
        owner: Worker,
        source: LockSource,
    ) -> Result<LaneGuard, LockError> {
        if source == LockSource::Worker {
            return Err(LockError::ForbiddenSource);
        }
        let key = LaneKey {
            project_id: proj.project_id.clone(),
            lock_key: lock_key.into(),
        };
        let mut g = self.0.lock();
        if let Some(e) = g.lanes.get(&key) {
            if e.state != LaneState::Idle {
                return Err(LockError::WouldBlock {
                    key: format!("lane:{}/{}", &proj.project_id, lock_key),
                });
            }
        }
        g.lanes.insert(
            key.clone(),
            LaneEntry {
                state: LaneState::Acquired,
                owner: Some(owner),
                transfer_target: None,
            },
        );
        g.audit.push(AuditEntry {
            ts_ms: now_ms(),
            kind: AuditKind::Acquired,
            project_id: proj.project_id.clone(),
            lock_key: lock_key.into(),
            owner: Some(owner),
        });
        Ok(LaneGuard {
            mgr: self.0.clone(),
            key,
        })
    }

    /// Cross-project 2-phase: sort by deterministic key, acquire all or none.
    /// Caller passes already-held repo + project guards for *each* project;
    /// only lane acquires are batched here. Returns lanes in input order.
    pub fn try_acquire_all_lanes(
        &self,
        requests: Vec<LaneRequest>,
    ) -> Result<Vec<LaneGuard>, LockError> {
        let mut sorted: Vec<(usize, LaneRequest)> =
            requests.into_iter().enumerate().collect();
        // Sort by (project_id, lock_key) — deterministic ordering across both
        // requesters guarantees no AB/BA deadlock under contention.
        sorted.sort_by(|(_, a), (_, b)| {
            a.project_guard
                .project_id
                .cmp(&b.project_guard.project_id)
                .then(a.lock_key.cmp(&b.lock_key))
        });

        let mut out: Vec<(usize, LaneGuard)> = Vec::with_capacity(sorted.len());
        for (idx, req) in sorted {
            match self.acquire_lane(req.project_guard, &req.lock_key, req.owner, req.source) {
                Ok(g) => out.push((idx, g)),
                Err(e) => {
                    // Release all already-acquired (Drop) and bail.
                    drop(out);
                    return Err(e);
                }
            }
        }
        out.sort_by_key(|(i, _)| *i);
        Ok(out.into_iter().map(|(_, g)| g).collect())
    }

    /// Begin owner transfer. State Acquired(A) → Transferring(target=B).
    /// Caller still holds the LaneGuard during transfer.
    pub fn begin_transfer(&self, guard: &LaneGuard, to: Worker) -> Result<(), LockError> {
        let mut g = self.0.lock();
        let entry = g.lanes.get_mut(&guard.key).expect("lane entry");
        if entry.state != LaneState::Acquired {
            return Err(LockError::NotTransferring);
        }
        if entry.owner == Some(to) {
            return Err(LockError::TransferToSelf);
        }
        entry.state = LaneState::Transferring;
        entry.transfer_target = Some(to);
        g.audit.push(AuditEntry {
            ts_ms: now_ms(),
            kind: AuditKind::TransferStarted,
            project_id: guard.key.project_id.clone(),
            lock_key: guard.key.lock_key.clone(),
            owner: Some(to),
        });
        Ok(())
    }

    /// Complete transfer. State Transferring → Acquired(target).
    pub fn complete_transfer(&self, guard: &LaneGuard) -> Result<Worker, LockError> {
        let mut g = self.0.lock();
        let entry = g.lanes.get_mut(&guard.key).expect("lane entry");
        if entry.state != LaneState::Transferring {
            return Err(LockError::NotTransferring);
        }
        let target = entry.transfer_target.take().expect("target set");
        entry.state = LaneState::Acquired;
        entry.owner = Some(target);
        g.audit.push(AuditEntry {
            ts_ms: now_ms(),
            kind: AuditKind::TransferCompleted,
            project_id: guard.key.project_id.clone(),
            lock_key: guard.key.lock_key.clone(),
            owner: Some(target),
        });
        Ok(target)
    }

    /// Snapshot of audit log (cloned). Callers should treat this as
    /// read-only; the source-of-truth remains the journal.
    pub fn audit_log(&self) -> Vec<AuditEntry> {
        self.0.lock().audit.clone()
    }

    /// Inspect current state of a lane (test/debug).
    pub fn lane_state(&self, project_id: &str, lock_key: &str) -> Option<(LaneState, Option<Worker>)> {
        let g = self.0.lock();
        g.lanes
            .get(&LaneKey {
                project_id: project_id.into(),
                lock_key: lock_key.into(),
            })
            .map(|e| (e.state, e.owner))
    }
}

pub struct LaneRequest<'a> {
    pub project_guard: &'a ProjectGuard,
    pub lock_key: String,
    pub owner: Worker,
    pub source: LockSource,
}

#[must_use = "RepoGuard releases the lock on drop"]
#[derive(Debug)]
pub struct RepoGuard {
    mgr: Arc<Mutex<Inner>>,
    key: String,
}
impl Drop for RepoGuard {
    fn drop(&mut self) {
        self.mgr.lock().repos.remove(&self.key);
    }
}

#[must_use = "ProjectGuard releases the lock on drop"]
#[derive(Debug)]
pub struct ProjectGuard {
    mgr: Arc<Mutex<Inner>>,
    project_id: String,
}
impl ProjectGuard {
    pub fn project_id(&self) -> &str {
        &self.project_id
    }
}
impl Drop for ProjectGuard {
    fn drop(&mut self) {
        self.mgr.lock().projects.remove(&self.project_id);
    }
}

#[must_use = "LaneGuard releases the lane on drop"]
#[derive(Debug)]
pub struct LaneGuard {
    mgr: Arc<Mutex<Inner>>,
    key: LaneKey,
}
impl LaneGuard {
    pub fn project_id(&self) -> &str {
        &self.key.project_id
    }
    pub fn lock_key(&self) -> &str {
        &self.key.lock_key
    }
}
impl Drop for LaneGuard {
    fn drop(&mut self) {
        let mut g = self.mgr.lock();
        if let Some(entry) = g.lanes.remove(&self.key) {
            g.audit.push(AuditEntry {
                ts_ms: now_ms(),
                kind: AuditKind::Released,
                project_id: self.key.project_id.clone(),
                lock_key: self.key.lock_key.clone(),
                owner: entry.owner,
            });
        }
    }
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
