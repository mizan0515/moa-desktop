# T11 — Parallel Session Runner

## 새 Claude 창 만들기 가이드
T4 + T7-full + T9 + T10 통과 후 (Phase 6). worktree: T11-parallel.

---

```
[세션 부트]
- repo: D:\moa-desktop
- base branch: master (T10 머지 후)
- 권장 분기: feat/T11-parallel
- 권위: PROJECT-RULES.md, AGENTS.md, DESIGN.md (## v1.5 scope, ## UI), PLAN.md (§ F6 lock ordering, § Phase 6 resource budget), TICKETS/T4 (lock manager + worktree pool API), TICKETS/T7-full (lane supervisor + panic boundary)
- 운영: MoA Flow C — § 2.6 템플릿 A

[의존성 self-check — claim 직후, first-pass 시작 전 무조건 실행]
master 에 선행 commit 4개 있는지 확인:
```
cd D:\moa-desktop && git log master --oneline -100 | rg -i "feat\(T4\)|feat\(T7-full\)|feat\(T9\)|feat\(T10\)" | wc -l
```
- 결과 `4` 면 OK — 작업 진행
- 4 미만이면 **STOP — "선행 티켓이 master 에 미머지" 사용자 보고**.

[INDEPENDENT FIRST-PASS — read-only]

## Goal
T10 가 분해한 N 티켓을 한 프로젝트 안에서 동시에 lane 으로 굴린다:
1. **Worktree pool** — N 티켓 → N 개 git worktree (T4 의 worktree.rs API 호출)
2. **Lane × N** — 각 lane 이 독립 T7-full orchestrator instance (panic boundary 격리, T7-full 의 lane supervisor)
3. **ParallelLanes UI** — N lane 동시 표시, 각 lane "Run/Pause/Cancel" 버튼 (사용자 명시 자동 실행 X — 자원 폭주 방지)
4. **Resource budget** (Codex amendment 의무): global `max_live_workers` (default 4), per-project `max_lanes` (default 2), bounded ring buffer for worker output (default 1MB) + disk spill, hidden tab idle throttling, RSS watchdog
5. **Lock ordering 준수** (Codex amendment 의무): § F6 의 contract — `repo-open canonical lock → project lock → session/lane mutation lock → journal append queue`. lane mutation lock 보유 중 다른 project lock 획득 금지. cross-project 작업 시 path/projectId 정렬 기반 2-phase `try_acquire_all`

## Success criteria
- [ ] `src-tauri/src/parallel/{pool.rs,lane.rs,worktree_pool.rs,budget.rs,supervisor.rs,mod.rs}` — worktree pool, lane orchestrator manager, resource budget enforcer
- [ ] `src/components/ParallelLanes.tsx` — N lane 카드 (lane id, ticket id, phase, progress, log preview), 각 lane Run/Pause/Cancel 버튼
- [ ] T10 의 분해 결과 JSON 을 input → worktree pool 이 base branch 에서 N worktree 생성 → 의존성 그래프 따라 ready 한 lane 만 active
- [ ] **Resource budget 강제**: 동시 활성 lane > `max_lanes` 시 queue, global worker > `max_live_workers` 시 queue. 대기 lane UI 에 "queued" 표시
- [ ] **Tab close drop**: tab close 시 React state + Rust `ProjectHandle/SessionHandle/LaneHandle` drop + child process abort + journal handle close + lock release 모두 수행. drop test 필수
- [ ] **Lock ordering deadlock test**: 2 lane 이 cross-project lock 시도 시 deadlock 없음 검증 (`try_acquire_all` 동작)
- [ ] **Bounded ring buffer**: 각 lane 의 worker stdout 1MB 까지 in-memory, 초과 시 disk spill (`~/.moa-desktop/lanes/<projectId>/<laneId>.log`). hidden tab 의 lane 은 throttle (UI poll 빈도 ↓)
- [ ] **RSS watchdog**: 5 초마다 child process RSS 합산, 사용자 설정 cap (default 6GB) 초과 시 가장 오래된 idle lane 부터 일시정지 + 사용자 alert
- [ ] integration test: 4 lane 동시 mock 실행 + lock ordering deadlock test + tab close drop test + RSS watchdog 발화 test

## Files owned
- `src-tauri/src/parallel/*.rs` (mod.rs body 포함)
- `src-tauri/src/parallel/worktree_pool.rs` (T4 의 worktree.rs 를 import 해서 pool layer)
- `src/components/ParallelLanes.tsx` (T1 의 stub 채움)
- `src-tauri/tests/parallel_*.rs`

## Read-only
- T4: LockManager API, worktree.rs (단일 worktree 생성), journal API
- T7-full: orchestrator instance 생성 API, lane supervisor / panic boundary
- T9: telemetry per (projectId, sessionId) — lane RSS aggregation 에 사용
- T10: 분해 결과 JSON schema

## NEVER 영역
- src-tauri/src/{decomposer,integrator}/ body (T10/T12)
- src-tauri/src/{policy,safety,commands,lifecycle}/ body (T13 owns)
- src-tauri/src/{adapters,orchestrator,safety,git,lock,journal,synthesis,process,telemetry}/ body
- T4 의 lock state machine 직접 mutate (API 만 사용)
- 비밀 파일

## Stop conditions
- T4 의 worktree.rs 가 pool 동시 생성에 race 있음 → T4 와 협의 (lock 으로 직렬화)
- T7-full panic boundary 가 N=2 이상에서 안 통하면 → T7-full 와 협의
- Windows worktree 동시 생성 시 file handle 누수 → 수정 (cleanup 강화)

## Deliverable (first-pass)
1. Diagnosis: git worktree pool concurrency (Windows 에서 동시 `git worktree add` 안전성)
2. Approach: 동기 acquire vs async semaphore (대안 2 개)
3. Risks: lock ordering 위반, RSS 누수, ring buffer disk spill 비용
4. Resource budget default 값 근거
5. Open questions

## Constraints
- 6 항목 의무
- lock ordering contract 위반은 panic 으로 즉시 잡힘 (debug build) / log+stop (release)
- 사용자 명시 N lane 동시 실행은 안전 (자동 N+1 시작 X)
- 비밀 파일 access X

[작업 완료 시 — 무조건 이 순서로]
1. commit: `feat(T11): parallel session runner + worktree pool + resource budget + lock ordering` (본문에 `Closes #16` 포함, push 금지)
2. **GitHub 카드 완료 처리 — 잊지 말고 무조건 실행** (안 하면 칠판 https://github.com/users/mizan0515/projects 에 status:doing 으로 남아 다른 세션이 또 잡을 수 있음):
   ```
   node ~/.claude/scripts/gh-tickets.mjs complete D:\moa-desktop 16
   ```
   - 출력에 `COMPLETED=16` 또는 `ALREADY_CLOSED=16` 가 보여야 OK.
   - 실패 시 사용자 보고 + STOP. gh 인증 오류면 `gh auth refresh -s project,read:project` 안내.
3. 보고: lane lifecycle 다이어그램, deadlock test 결과, RSS watchdog 발화 시나리오, T12 integrator 가 본 lane 결과를 어떻게 소비하는지, **GitHub 카드 close 결과 1줄**.
```
