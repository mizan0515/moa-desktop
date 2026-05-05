# T12 — Merge Integrator (`/병행통합` 등가)

## 새 Claude 창 만들기 가이드
T11 통과 후 (Phase 6 마무리). worktree: T12-integrator.

---

```
[세션 부트]
- repo: D:\moa-desktop
- base branch: master (T11 머지 후)
- 권장 분기: feat/T12-integrator
- 권위: PROJECT-RULES.md, AGENTS.md, DESIGN.md (## v1.5 scope), PLAN.md (§ Phase 6), TICKETS/T4 (patch apply API), TICKETS/T11 (lane 결과 schema)
- 글로벌 reference: ~/.claude/plugins/...skills/병행통합 (해당 skill 의 머지 순서 + 충돌 stop 패턴)
- 운영: MoA Flow C — § 2.6 템플릿 A

[INDEPENDENT FIRST-PASS — read-only]

## Goal
T11 의 N lane 이 모두 완료된 후:
1. T10 가 emit 한 **머지 순서** 따라 patch apply
2. 충돌 시 **즉시 stop + 한국어 보고** (자동 해결 시도 X — 사용자 명시 결정 필요)
3. 성공 시 worktree 정리 + lane 상태 archived
4. 실패 시 rollback (이미 apply 된 patch revert 또는 reset)
5. UI 에 IntegratePanel: progress (현재 N/M), 충돌 시 file path + diff 표시

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
- 글로벌 `/병행통합` skill 정의 — pattern 참조

## NEVER 영역
- src-tauri/src/{decomposer,parallel}/ body (T10/T11)
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
- 비밀 파일 access X

[작업 완료 시]
- commit: `feat(T12): merge integrator (병행통합 등가) + IntegratePanel + 충돌 한국어 보고`
- push 금지
- 보고: 머지 lifecycle 다이어그램, 충돌 보고 sample (한국어), Phase 6 (v1.5) 완료 — 다음 Phase 5 polish 진입 가능
```
