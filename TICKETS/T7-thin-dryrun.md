# T7-thin — Dry-run orchestrator (walking skeleton)

## 새 Claude 창 만들기 가이드
T8, T6 통과 후 새 창. worktree: T7thin-dryrun.

---

```
[세션 부트]
- repo: D:\moa-desktop
- base branch: master (T1 + T6 + T8 머지 후)
- 권장 분기: feat/T7-thin
- 권위: DESIGN.md, PLAN.md (Phase 1 demo milestone), synthesis.md, ~/.claude/CODEX-MCP.md
- 운영: MoA Flow C — § 2.6 템플릿 A

[의존성 self-check — claim 직후, first-pass 시작 전 무조건 실행]
master 에 선행 commit 2개 있는지 확인:
```
cd D:\moa-desktop && git log master --oneline -50 | rg -i "feat\(T6\)|feat\(T8\)" | wc -l
```
- 결과 `2` 면 OK — 작업 진행
- 2 미만이면 **STOP — "선행 티켓이 master 에 미머지" 사용자 보고**.

[INDEPENDENT FIRST-PASS — read-only]

## Goal
**Phase 1 Demo Milestone**: 사용자가 작업 입력 → mock first-pass × 2 → mock synthesis (T8 의 synthesis.json) → mock adversarial → mock final report → SynthesisView + ClaimLedger 렌더. **No real AI call.** End-to-end happy path 가시화.

## Success criteria
- [ ] 사용자가 workbench 에서 작업 텍스트 입력 + "Run dry-run" 클릭 → 진행 단계 (preflight → first-pass → synthesis → adversarial → final) 가 lane 별로 timeline 에 표시
- [ ] 각 단계가 mock runner (T8) 에서 데이터 받아 SynthesisView + ClaimLedger 에 최종 표시
- [ ] 중간 cancel 버튼 작동 (mock runner abort)
- [ ] 같은 작업 다시 실행하면 새 session 으로 left panel 에 추가됨
- [ ] 회귀 0

## Files owned
- `src/lib/orchestrator/dryRun.ts` (state machine: idle → preflight → fp1 → fp2 → synth → adv → final)
- `src-tauri/src/orchestrator/dryrun.rs` (T7-full 의 orchestrator/mod.rs 와 분리)
- `src/components/Workbench/RunButton.tsx`
- `src/components/Workbench/Timeline.tsx` (단계 표시)

## Read-only
- T8 mock runner API
- T6 SynthesisView, ClaimLedger
- T1 workbench layout

## NEVER 영역
- src-tauri/src/process/* (T2)
- src-tauri/src/adapters/* (T5a/T5b)
- src-tauri/src/orchestrator/mod.rs body (T7-full)
- src-tauri/src/{safety,git,lock,journal}/* (T4)
- 실제 worker spawn 코드

## Stop conditions
- mock runner trait 시그니처가 dryRun state machine 과 안 맞음 → T8 와 협의
- Timeline UI 가 좁은 폭 안 들어감 → T6 와 layout 합의

## Deliverable (first-pass)
1. Diagnosis: T8 mock + T6 view + T1 layout 의 결합 지점 정리
2. Approach: state machine 라이브러리 (xstate) vs hand-rolled (대안 2개)
3. Risks
4. session storage shape (left panel 의 list 데이터)
5. Open questions

## Constraints
- 6 항목 의무
- mock 만 호출, 실제 CLI 안 됨
- session metadata 는 in-memory 우선 (persistence 는 T7-full 또는 T9)

[작업 완료 시 — 무조건 이 순서로]
1. commit: `feat(T7-thin): dry-run orchestrator end-to-end` (본문에 `Closes #5` 포함, push 금지)
2. **GitHub 카드 완료 처리 — 잊지 말고 무조건 실행** (안 하면 칠판 https://github.com/users/mizan0515/projects 에 status:doing 으로 남아 다른 세션이 또 잡을 수 있음):
   ```
   node ~/.claude/scripts/gh-tickets.mjs complete D:\moa-desktop 5
   ```
   - 출력에 `COMPLETED=5` 또는 `ALREADY_CLOSED=5` 가 보여야 OK.
   - 실패 시 사용자 보고 + STOP. gh 인증 오류면 `gh auth refresh -s project,read:project` 안내.
3. 보고: Phase 1 demo milestone 달성 여부, 다음 단계 (Phase 2) 진입 GO/NO-GO, T7-full state machine 이 추가로 다룰 phase 목록, **GitHub 카드 close 결과 1줄**.
```
