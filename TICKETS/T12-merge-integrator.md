# T12 — Merge Integrator (`/병행통합` 등가)

## 새 Claude 창 만들기 가이드
T11 통과 후 (Phase 6 마무리). worktree: T12-integrator.

---

```
[세션 부트]
- repo: D:\moa-desktop
- base branch: master (T11 머지 후)
- 권장 분기: feat/T12-integrator
- 권위: PROJECT-RULES.md, AGENTS.md, T13 PolicyPack/RuntimeProfile resolver, DESIGN.md (## v1.5 scope), PLAN.md (§ Phase 6), TICKETS/T4 (patch apply API), TICKETS/T11 (lane 결과 schema)
- 글로벌 reference: T13 이 resolve 한 현재 `/병행통합` source. 원문이 앱 정책과 충돌하면 policy resolver transform 이 우선하며, `~/.claude/plugins/...` raw path 를 직접 copy source 로 삼지 않는다.
- 운영: MoA Flow C — § 2.6 템플릿 A

[의존성 self-check — claim 직후, first-pass 시작 전 무조건 실행]
master 에 선행 commit 1개 있는지 확인:
```
cd D:\moa-desktop && git log master --oneline -100 | rg -i "feat\(T11\)" | wc -l
```
- 결과 `1` 이상이면 OK — 작업 진행
- 0 이면 **STOP — "선행 티켓이 master 에 미머지" 사용자 보고**.

[INDEPENDENT FIRST-PASS — read-only]

## T15 Pi runtime amend

T12 는 T11 lane result 의 `runtimeKind`, permission, capability manifest, package trust state 를 검증한다. Pi advisory review 나 Pi extension 결과는 merge 판단의 참고 신호일 수 있지만 mandatory `CodexAdversarialXHigh` gate 를 대체할 수 없다. Pi package install/update/hot reload 요청이 lane result 에 남아 있으면 user confirm/pinned source/sha256/capability manifest/mutation lock evidence 가 없을 때 `ReviewRunError` 또는 policy blocked 로 stop 한다.

## Goal
T11 의 N lane 이 모두 완료된 후:
1. T10 가 emit 한 **머지 순서** 따라 patch apply
2. 충돌 시 **즉시 stop + 한국어 보고** (자동 해결 시도 X — 사용자 명시 결정 필요)
3. 성공 시 worktree 정리 + lane 상태 archived
4. 실패 시 rollback (이미 apply 된 patch revert 또는 reset)
5. UI 에 IntegratePanel: progress (현재 N/M), 충돌 시 file path + diff 표시
6. PR 생성 전/머지 전/통합 merge 전/main 적용 전 T13 L2.5/L4 의 lead/orchestrator-owned mandatory `CodexAdversarialXHigh` review gate 통과. 앱 실행 시에는 orchestrator profile, Codex Desktop 수동 개발 시에는 lead PowerShell 별도 리뷰 프로파일과 `.moa-desktop/reviews/<stamp>.md` output capture 로 수행 가능. Worker 가 peer AI 를 직접 호출하지 않음.

## Success criteria
- [ ] `src-tauri/src/integrator/{apply.rs,conflict.rs,rollback.rs,cleanup.rs,mod.rs}` — patch apply 순서 실행, 충돌 detection, rollback, worktree 정리
- [ ] `src/components/IntegratePanel.tsx` — Run/Pause/Stop 버튼, progress bar, 충돌 시 conflict viewer (file path + diff hunk + base/our/their)
- [ ] 머지 순서 입력: T10 의 `mergeOrder: ["T1", "T2", ...]` 그대로 사용
- [ ] 각 단계: T4 의 `git apply --check` → 성공 시 `git apply` → 실패 시 즉시 stop + 한국어 보고
- [ ] 충돌 보고 schema:
  ```json
  {
    "stoppedAt": "T3",
    "conflict": {"files": [{"path": "src/foo.rs", "hunks": [...]}], "reason": "patch does not apply"},
    "applied": ["T1", "T2"],
    "remaining": ["T3", "T4", "T5"],
    "rollbackPlan": "revert T1, T2 in reverse order"
  }
  ```
- [ ] 한국어 보고: 충돌 시 "T3 적용 중 src/foo.rs 의 X-Y 라인에서 충돌. 직전까지 적용된 ticket: T1, T2. 진행 옵션: (1) 충돌 수동 해결 후 resume, (2) T1/T2 rollback + 전체 중단"
- [ ] Review gate: mandatory `CodexAdversarialXHigh` `ReviewVerdict::Clean` 인 경우만 PR create/merge/integrate/main apply 진행. `Concern`, `Block`, `ReviewRunError` 는 STOP + 한국어 보고. PrimaryRole=Codex 의 ClaudeSymmetry 는 추가 신호이며 `CodexAdversarialXHigh` 를 대체하지 않음.
- [ ] Review input: T13 `ReviewInputStrategy` 로 diff 크기별 chunking 수행. 50KB 미만 단일 review, 초과 시 critical files 우선 + omitted_files 표시.
- [ ] ReviewRunRecord persistence: 각 gate 의 T13 L2.5 full audit field set (`verdict`, `reviewer`, `review_kind`, `review_profile_id`, `reasoning_effort`, `model_or_profile_id`, `prompt_template_version`, `prompt_hash`, `command_source_adapter`, `primary_role`, `scope`, `gate`, `patch_hash`, `files_reviewed`, `omitted_files`, `limitations`, `evidence`, `required_actions`, `created_at`, `source_output_path`) 를 journal, ResumePacket, PR/merge 보고에 저장. lane result 에서 필드가 빠졌거나 `CodexAdversarialXHigh` 필수 field 가 비어 있으면 merge/apply 를 skip 하지 말고 `ReviewRunError` 로 stop.
- [ ] Runtime result validation: 모든 lane 의 `runtimeKind` 와 permission class 를 검증한다. `runtimeKind="pi"` lane 은 `PiRpcAdapter`/`PiSdkHost` capability evidence, package trust state, extension UI capability blocked/allowed record 를 포함해야 하며 누락 시 stop 한다.
- [ ] Nested peer-call 차단: integrator worker/lane output 에 peer AI 직접 호출 패턴이 있으면 T13 L2 scanner 가 차단하고 merge 중단.
- [ ] 성공 시 cleanup: 모든 N worktree `git worktree remove` (force flag 만 사용 X — 미커밋 변경 있으면 사용자 confirm)
- [ ] integration test: 4 lane → 5 ticket 머지 → 3 번째에서 충돌 → stop 보고 → 사용자 resume → 완료

## Files owned
- `src-tauri/src/integrator/*.rs` (mod.rs body 포함)
- `src/components/IntegratePanel.tsx` (T1 의 stub 채움)
- `src-tauri/tests/integrator_*.rs`

## Read-only
- T4: patch apply API, worktree remove API, journal API
- T11: lane 결과 schema (각 lane 의 final patch path + status)
- T10: mergeOrder
- T13 PolicyPack 이 resolve 한 `/병행통합` skill source — merge order + conflict stop pattern 참조

## NEVER 영역
- src-tauri/src/{decomposer,parallel}/ body (T10/T11)
- src-tauri/src/{policy,safety,commands,lifecycle}/ body (T13 owns)
- src-tauri/src/{adapters,orchestrator,safety,git,lock,journal,synthesis,process,telemetry}/ body
- main repo 직접 force overwrite (T4 patch apply API 만 사용)
- 미커밋 user 변경을 묻지 않고 force remove
- 비밀 파일

## Stop conditions
- T4 patch apply API 가 atomic 보장 안 함 (apply 도중 crash 시 partial state) → T4 와 협의
- 충돌 자동 해결 시도 (3-way merge 자동) — 본 ticket 범위 X, 사용자 명시 결정만
- worktree remove 가 Windows file lock 으로 실패 → retry + 사용자 보고

## Deliverable (first-pass)
1. Diagnosis: git apply atomicity (실패 시 partial state 남는지)
2. Approach: rebase vs cherry-pick vs apply (대안 2-3 개 + pros/cons)
3. Risks: 충돌 보고 한국어 명확성, rollback 안전성
4. 사용자 resume UX (충돌 수동 해결 후 어떻게 다시 본 ticket 에 control 돌려주는지)
5. Open questions

## Constraints
- 6 항목 의무
- 충돌 시 자동 해결 시도 금지 — 항상 stop + 보고
- rollback 은 명시적 사용자 결정 후만
- review gate 는 lead/orchestrator-owned 로만 수행한다. 앱 실행 시에는 source=orchestrator profile, Codex Desktop 수동 개발 시에는 lead PowerShell 별도 Codex review profile 과 `.moa-desktop/reviews/` output capture 로 수행 가능하다. worker prompt 안에 peer AI 직접 호출 명령을 넣지 않음. gate 시점은 `pr_create`, `pr_merge`, `integrate_merge`, `main_apply` 네 곳을 모두 schema 로 표현하고, profile/prompt 는 `CodexAdversarialXHigh(reasoning_effort=xhigh)` 로 fail-closed 검증한다.
- 비밀 파일 access X

## Worker prompt 6 mandatory fields
1. Success criteria: T10 `mergeOrder` 순서대로 T11 lane patch 를 apply/check/rollback 하고, conflict stop/report, cleanup, review gate verdict matrix, full ReviewRunRecord persistence 를 구현한다.
2. NEVER 영역: T10/T11/T13 owned body, adapters/orchestrator/safety/git/lock/journal/synthesis/process/telemetry body, main repo force overwrite, 사용자 확인 없는 force remove, 자동 conflict 해결, worker 직접 peer 호출 패턴.
3. Validation cmd:
   ```powershell
   cargo test --manifest-path src-tauri\Cargo.toml integrator
   npm test -- --run IntegratePanel
   ```
4. Files + lines: `TICKETS/T12-merge-integrator.md` 의 Goal/Success criteria/Constraints, `TICKETS/T10-ticket-decomposer.md` 의 `mergeOrder`/`reviewGate`, `TICKETS/T11-parallel-runner.md` 의 lane result schema, `TICKETS/T13-policy-lifecycle-epic.md` L2.5 의 ReviewInputStrategy/ReviewRunRecord.
5. Alternatives 2개 + pros/cons + 선택 근거: `git apply`(patch 중심, 예측 가능하지만 conflict 수동 처리 필요) vs cherry-pick/rebase(깃 히스토리 활용 가능하지만 자동 해결/side effect 위험). 선택은 T4 API 기반 `git apply --check` 후 `git apply`.
6. Tests-first: apply success, third-ticket conflict stop, rollback report, Clean/Concern/Block/ReviewRunError gate matrix, large diff chunk aggregation test 를 먼저 실패시키고 구현한다.

## Worker prompt cross-contract fields
- GitHub/project handling: GitHub #17 / Project `MoA Desktop` card status 를 claim/complete 단계에서 갱신한다. PR 생성/머지 step 이 포함되면 각 step 전 사용자 confirm 과 review gate record 를 남긴다.
- Conflict matrix ownership: T12 owns 는 `src-tauri/src/integrator/*`, `src/components/IntegratePanel.tsx`, `src-tauri/tests/integrator_*.rs` 로 한정한다. T10/T11/T13 및 shared adapter/orchestrator body 는 read-only 이며, conflict 발생 시 자동 해결하지 않고 한국어 stop report 로 넘긴다.
- Dependency/merge order: T10 `mergeOrder` 를 source of truth 로 사용하고, T11 lane result 의 `dependencyGraphRef`, `conflictMatrixRef`, `runtimeKind`, `runtimeCapabilities`, `reviewRunRecords` 를 검증한 뒤 적용한다. T11 완료 전 T12 시작 금지.
- Review gate warning: lead/orchestrator-owned `CodexAdversarialXHigh` 가 `Clean` 을 반환하고 `source_output_path` 가 persisted 되기 전에는 `pr_create`, `pr_merge`, `integrate_merge`, `main_apply` 진행 금지. worker 는 직접 peer review 를 실행하지 않는다.

[작업 완료 시 — 무조건 이 순서로]
1. commit: `feat(T12): merge integrator (병행통합 등가) + IntegratePanel + 충돌 한국어 보고` (본문에 `Closes #17` 포함, push 금지)
2. **Review gate 검증**: Clean/Concern/Block/ReviewRunError 4 케이스에서 다음 단계 진행/차단이 정확한지 테스트. 대형 diff chunk aggregate 가 `Block > Concern > Clean` 우선순위를 따르는지 확인. `pr_create`, `pr_merge`, `integrate_merge`, `main_apply` 네 gate 의 ReviewRunRecord persistence 를 확인.
3. **GitHub 카드 완료 처리 — 잊지 말고 무조건 실행** (안 하면 칠판 https://github.com/users/mizan0515/projects 에 status:doing 으로 남아 다른 세션이 또 잡을 수 있음):
   ```
   node ~/.claude/scripts/gh-tickets.mjs complete D:\moa-desktop 17
   ```
   - 출력에 `COMPLETED=17` 또는 `ALREADY_CLOSED=17` 가 보여야 OK.
   - 실패 시 사용자 보고 + STOP. gh 인증 오류면 `gh auth refresh -s project,read:project` 안내.
4. 보고: 머지 lifecycle 다이어그램, 충돌 보고 sample (한국어), review gate verdict matrix, Phase 6 (v1.5) 완료 — 다음 Phase 5 polish 진입 가능, **GitHub 카드 close 결과 1줄**.
```
