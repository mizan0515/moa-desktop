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
2. **Mutation lock manager** — 1 session 당 1 owner. transfer 가능 (lock acquire/release/transfer + audit log)
3. **Recovery journal** — phase, owner, PID, base hashes, patch path JSONL. crash 후 startup reconcile
4. **Multi-instance lock** — repo 단위 OS file lock (두 번째 MoA Desktop instance 가 같은 repo 작업 시 거부)
5. **File hash snapshot/diff** — Worker turn 전후 hash 비교

## Success criteria
- [ ] `git worktree add` 으로 임시 worktree 생성, Worker 종료 시 `git worktree remove`
- [ ] patch 추출 (`git -C <worktree> diff` 또는 `format-patch`) → in-memory + 파일 저장
- [ ] patch apply (`git apply --check` → `git apply`) — 실패 시 reject
- [ ] lock state machine: idle → acquired(claude|codex) → transferring → acquired(other) → released. transfer 시 audit log
- [ ] journal: per-session JSONL append-only. 각 entry = `{phase, owner, pid, ts, base_hashes, patch_path}`. startup 시 마지막 미완료 entry 발견하면 사용자에 reconcile 옵션 (cleanup / resume)
- [ ] multi-instance: `~/.moa-desktop/locks/<repo-hash>.lock` (또는 Tauri appDataDir) — flock/exclusive open. 두 번째 instance 거부
- [ ] file hash: SHA-256 snapshot. transfer 시점에 hash 미스매치 = error
- [ ] unit test: worktree create/apply/reject, lock acquire/transfer/release, journal reconcile, multi-instance refusal, hash mismatch

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

[작업 완료 시]
- commit: `feat(T4): worktree mutation + lock + journal + multi-instance`
- 보고: 4개 subsystem API, T7 가 호출할 시퀀스 (acquire → mutate → patch verify → apply 또는 reject → release), edge case 테스트 결과
```
