# T7-full — Orchestrator state machine (Flow A/B/C/D)

## 새 Claude 창 만들기 가이드
T3, T4, T5a, T5b 통과 후. worktree: T7full-orch.

---

```
[세션 부트]
- repo: D:\moa-desktop
- base branch: master (T3 + T4 + T5a + T5b 머지 후)
- 권장 분기: feat/T7-full
- 권위: ~/.claude/CODEX-MCP.md (§ 2.5 Flow 결정 트리 + § 2.6 템플릿), PLAN.md (§ 0 conflict protocol), DESIGN.md
- 운영: MoA Flow C — § 2.6 템플릿 A

[INDEPENDENT FIRST-PASS — read-only]

## Goal
실제 MoA flow state machine. 사용자 작업 입력 → Flow A/B/C/D 자동 분류 → first-pass × 2 (T5a + T5b 병렬) → synthesis (T3) → adversarial round (orchestrator 가 새 Worker 호출, Worker recursion X) → mutation owner 1명 (T4 lock) → verification → final report. 최대 3 round. T7-thin 의 dry-run path 와 통합 (mockMode flag 기준 분기).

## Success criteria
- [ ] Flow 분류: trivial(10줄 미만 + 단일 파일 + behavior-preserving) → A/B, 그 외 → C, research → D — heuristic + 사용자 override
- [ ] first-pass 병렬 호출: T5a + T5b 동시 spawn, 둘 다 완료까지 대기 (Promise.all)
- [ ] synthesis: T3 deterministic merge 호출 (LLM X)
- [ ] adversarial: synthesis JSON 을 prompt 에 embed → Worker 한쪽 (Codex default) 새로 호출. **Worker 가 peer 호출 X — orchestrator 가 호출자**
- [ ] 충돌 해결 protocol: 사실 충돌 → live verify (T2 git/test 호출), 구현 충돌 → blast radius/rollback/validation 비교, risk 충돌 → cheapest test, 아키텍처 → 사용자 escalate
- [ ] mutation: T4 lock acquire → 선택된 Worker mutation 모드 (T5a 또는 T5b) → patch 추출 → diff/test 검증 → reviewer 가 다른 Worker → confirm 후 T4 patch apply
- [ ] verification cmd 실행 (settings 의 project-specific cmd)
- [ ] final report = Claim Ledger + 5칸 + mutation 결과
- [ ] max 3 round, 초과 시 사용자 escalation
- [ ] T7-thin (dryRun) path 와 통합: settings.mockMode → T8 mock runner 로 swap, 같은 state machine 재사용
- [ ] integration test: small task end-to-end (mock Workers 로) + small real task (사용자 confirm 후)
- [ ] **Lane supervisor + panic boundary** (Phase 6 multi-project crash isolation 흡수책): 각 lane orchestrator instance 를 격리된 Tokio task 로 spawn, panic 감지 시 해당 lane 만 fail (다른 lane/UI 영향 0). lane drop 시 child process abort + lock release + journal close 의무. unit test: `lane_panic_does_not_kill_app`

## Files owned
- `src-tauri/src/orchestrator/{mod.rs,state.rs,flow.rs,adversarial.rs,verify.rs}` (단 dryrun.rs 는 T7-thin 영역, T7-full 이 import 만)
- `src/lib/orchestrator/stateMachine.ts` (frontend state)

## Read-only
- T2, T3, T4, T5a, T5b, T8 의 public API
- T7-thin 의 dryRun.ts (재사용 vs replace 결정)
- ~/.claude/CODEX-MCP.md (Flow 결정 + 충돌 protocol)

## NEVER 영역
- T2/T3/T4/T5a/T5b body 수정 (API 만 사용)
- src-tauri/src/orchestrator/dryrun.rs body (T7-thin 의 영역, integration 만)
- src-tauri/src/{process,adapters,safety,git,lock,journal,synthesis,mock}/ body

## Stop conditions
- T3/T4/T5a/T5b API 가 state machine 표현 부족 → 해당 worker 와 API 협의
- adversarial round 가 무한 루프 위험 (Worker 가 NEED_PEER_REVIEW 반복) → round counter ≤ 3 강제 + 사용자 escalation

## Deliverable (first-pass)
1. Diagnosis: ~/.claude/CODEX-MCP.md 의 Flow 결정 트리 + 충돌 protocol 그대로 옮긴 표
2. Approach: state machine 라이브러리 (xstate Rust port? actor model?) vs hand-rolled enum (대안 2개)
3. Risks (race condition, partial state on crash → T4 journal 의존)
4. adversarial prompt template (synthesis JSON embed + reality-check 요청 — § 2.6 템플릿 D 적용)
5. Open questions

## Constraints
- 6 항목 의무
- adversarial 호출 = 새 Worker spawn (orchestrator 가 호출, Worker recursion 금지)
- mutation 1명만 (T4 lock 강제)
- max 3 round

[작업 완료 시]
- commit: `feat(T7-full): orchestrator state machine + adversarial round`
- 보고: state diagram, edge cases (timeout, partial failure), Phase 3 demo milestone (real small task end-to-end) 달성 여부
```
