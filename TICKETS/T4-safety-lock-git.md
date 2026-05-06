# T4 — Safety / Git worktree / Lock / Recovery journal

## 새 Claude 창 만들기 가이드
T2 통과 후 (T5a/T5b 와 병렬 가능 — 다른 폴더). worktree: T4-safety.

---

```
[세션 부트]
- repo: D:\moa-desktop
- base branch: master (T2 머지 후)
- 권장 분기: feat/T4-safety
- 권위: PLAN.md (§ F4 worktree mutation, § F6 recovery journal, multi-instance, file hash), DESIGN.md (lock manager), ~/.claude/CODEX-MCP.md
- 운영: MoA Flow C — § 2.6 템플릿 A

[INDEPENDENT FIRST-PASS — read-only]

## Goal
Mutation 안전 인프라:
1. **Worktree-isolated patch flow** — Worker 가 직접 source 안 만지고 임시 worktree 에서 작업, app 이 patch 추출 → 검증 → apply 또는 reject
2. **2-layer lock manager** — (a) in-memory `LockManager` API `(projectId, lockKey)` for in-app N-tab safety + (b) **OS-level named mutex / lock file** for cross-process safety (Tauri single-instance plugin 이 Win11 24H2 등에서 실패해도 mutation safety 보장). 1 session 당 1 owner, transfer 가능.
3. **Recovery journal** — phase, owner, PID, base hashes, patch path JSONL. crash 후 startup reconcile. **Durability policy** 명시 (아래 success criteria).
4. **Repo-path canonicalization** — case fold, symlink, junction, UNC path 정규화 → 동일 repo 가 다른 path 표기로 두 탭에 열리는 것 차단.
5. **File hash snapshot/diff** — Worker turn 전후 hash 비교
6. **Lock ordering contract** — `repo-open canonical lock → project lock → session/lane mutation lock → journal append queue`. lane lock 보유 중 다른 project lock 획득 금지. cross-project 작업 (T11) 은 path/projectId 정렬 기반 2-phase `try_acquire_all` + retry. worker output 은 lock acquisition source 가 될 수 없음 (scheduler 만).

## Success criteria
- [ ] `git worktree add` 으로 임시 worktree 생성, Worker 종료 시 `git worktree remove`
- [ ] patch 추출 (`git -C <worktree> diff` 또는 `format-patch`) → in-memory + 파일 저장
- [ ] patch apply (`git apply --check` → `git apply`) — 실패 시 reject
- [ ] **lock manager API 가 `(projectId, lockKey)` 키로 받음** — v1 single-project 에서도 인터페이스만 미리 (Phase 6 multi-project 진입 시 backtrack 0). state machine: idle → acquired(claude|codex) → transferring → acquired(other) → released. transfer 시 audit log
- [ ] **Lock ordering contract 구현 + 검증**: `repo-open canonical lock → project lock → session/lane mutation lock → journal append queue` 순서. lane mutation lock 보유 중 다른 project lock 획득 시도 = 컴파일 타임 또는 런타임 panic. cross-project 는 정렬 기반 `try_acquire_all` (전부 또는 전무). worker output → lock command 변환 경로 차단 (스케줄러만 lock).
- [ ] **OS-level named mutex / lock file fallback**: Tauri plugin 실패 또는 사용자가 `--user-data-dir` 로 의도적 N 인스턴스 띄움 시에도 동일 repo 동시 mutation 차단. `~/.moa-desktop/locks/<repo-canonical-hash>.lock` (Windows: named mutex `Global\moa-desktop-<hash>`, Unix: flock). stale detection (lock holder PID 사망 시 cleanup).
- [ ] **Repo path canonicalization**: case fold, symlink resolve, junction resolve, UNC normalize. `D:\repo`, `d:\repo`, `\\?\D:\repo`, junction → 같은 canonical key.
- [ ] journal: **per-(projectId, sessionId) JSONL append-only** — 디렉토리 구조 `~/.moa-desktop/journals/<projectId>/<sessionId>.jsonl`. 각 entry = `{phase, owner, pid, ts, base_hashes, patch_path}`. startup 시 마지막 미완료 entry 발견하면 사용자에 reconcile 옵션 (cleanup / resume).
- [ ] **Journal durability policy**: session 시작 시 file pre-create + handle 유지, append 는 per-session single writer channel 로 직렬화, **mutation lock 밖에서 flush** (lock starvation 방지). critical phase transition 에만 bounded/batched `sync_all`. **reconcile 은 "마지막 entry 유실 가능" 을 정상 케이스로** 처리 — worktree/patch dir scan 결과와 결합해 truth 결정. OneDrive/Defender 경로 감지 시 사용자 경고.
- [ ] file hash: SHA-256 snapshot. transfer 시점에 hash 미스매치 = error
- [ ] unit test: worktree create/apply/reject, lock acquire/transfer/release (per project key), **lock ordering deadlock test**, **OS-level mutex second-process refusal**, **repo path canonicalization (case/symlink/junction/UNC)**, **journal parallel flush latency + crash reconcile**, hash mismatch

## Files owned
- `src-tauri/src/safety/*.rs` (mod.rs body 포함)
- `src-tauri/src/git/{worktree.rs,patch.rs,mod.rs}`
- `src-tauri/src/lock/{manager.rs,instance.rs,mod.rs}`
- `src-tauri/src/journal/{writer.rs,reader.rs,reconcile.rs,mod.rs}`
- `src-tauri/tests/{safety,git,lock,journal}_*.rs`

## Read-only
- T2 ProcessRunner (git 명령 실행 시 사용)
- DESIGN.md, PLAN.md

## NEVER 영역
- src-tauri/src/adapters/* (T5)
- src-tauri/src/orchestrator/* body (T7)
- src-tauri/src/process/* body (T2)
- 사용자 작업 repo 의 main worktree 내용 (worktree 안에서만 mutation)

## Stop conditions
- git CLI 미설치 → 사용자 보고 (Phase 1 prereq)
- multi-instance flock 이 Windows 에서 unreliable → fallback (named mutex) 검토
- worktree apply 가 충돌 → conflict UI 가 T7 책임이므로 T7 와 협의

## Deliverable (first-pass)
1. Diagnosis: git worktree on Windows 동작 (CRLF, longpath 등)
2. Approach: in-process git (libgit2/gitoxide) vs git CLI shell-out (대안 2개+pros/cons). 아마 CLI shell-out 우선 (단순 + 명확)
3. Risks (worktree 잔존, lock file 좀비)
4. journal schema sample
5. Open questions

## Constraints
- 6 항목 의무
- mutation 권한 부여는 명시적 lock acquire 후만
- 사용자 main repo 손상 0 (worktree 안에서만)

[작업 완료 시 — 무조건 이 순서로]
1. commit: `feat(T4): worktree mutation + lock + journal + multi-instance` (본문에 `Closes #10` 포함, push 금지)
2. **GitHub 카드 완료 처리 — 잊지 말고 무조건 실행** (안 하면 칠판 https://github.com/users/mizan0515/projects 에 status:doing 으로 남아 다른 세션이 또 잡을 수 있음):
   ```
   node ~/.claude/scripts/gh-tickets.mjs complete D:\moa-desktop 10
   ```
   - 출력에 `COMPLETED=10` 또는 `ALREADY_CLOSED=10` 가 보여야 OK.
   - 실패 시 사용자 보고 + STOP. gh 인증 오류면 `gh auth refresh -s project,read:project` 안내.
3. 보고: 4개 subsystem API, T7 가 호출할 시퀀스 (acquire → mutate → patch verify → apply 또는 reject → release), edge case 테스트 결과, **GitHub 카드 close 결과 1줄**.
```
