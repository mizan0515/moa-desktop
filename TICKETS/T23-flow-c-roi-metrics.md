# T23 — Flow C ROI 지표 (accept/reject + test + cost 피드백 루프)

GitHub: #54 (https://github.com/mizan0515/moa-desktop/issues/54)

## Origin
RL Conductor 논문 (arXiv:2512.04388, ICLR 2026) 적용 가능성 리서치에서 도출.
MoA 흐름 D 종합 + Codex adversarial review (2026-05-10) 판정: **LOW**.

## 새 Claude 창 만들기 가이드
telemetry 파이프라인 (T9) 완성 후 착수 가능.

---

````
[세션 부트]
- repo: D:\moa-desktop
- base branch: master
- 권장 분기: feat/T23-roi-metrics
- 권위: DESIGN.md, src-tauri/src/telemetry/ (cost.rs 포함)
- 운영: MoA Flow C — § 2.6 템플릿 A

[의존성 self-check — claim 직후, first-pass 시작 전 무조건 실행]
```
cd D:\moa-desktop && git log master --oneline -100 | rg -i "feat\(T9\)" | wc -l
```
- 결과 `1` 이상이면 OK — 작업 진행
- 0 이면 **STOP — "T9 가 master 에 미머지" 사용자 보고**.
- 추가 확인: `rg -c "cost\|telemetry" src-tauri/src/telemetry/cost.rs` — 1 이상이면 OK
- cost.rs: Codex cost 추적 현황 (현재 $0 문제)

[INDEPENDENT FIRST-PASS — read-only]
````

## Goal
Flow C (MoA 코드 변경) 실행 후 outcome 피드백을 수집하여 파이프라인 효과 측정.
RL Conductor가 보상 신호로 정확도를 사용한 것처럼, moa-desktop은 사용자 accept/reject + 테스트 결과 + 비용을 outcome 신호로 활용.

현재 상태: 실행 비용은 `src-tauri/src/telemetry/cost.rs`에서 부분 추적하나 Codex=$0 문제. verify verdict는 Skipped/Passed 구분 불명확. outcome→flow 연결 없음.

## Success criteria
- [ ] Flow C 실행마다 outcome record 저장: `{flow, task_hash, worker_claims_count, synthesis_result, adversarial_rounds, adversarial_verdict, user_accept, test_result, cost_claude, cost_codex, duration_ms}`
- [ ] Codex cost 추적 정상화 (cost.rs에서 Codex API 응답의 usage 필드 파싱)
- [ ] verify verdict 세분화: `Skipped | Passed | Failed | Timeout`
- [ ] outcome dashboard 또는 CLI 리포트: flow별 accept rate, 평균 round 수, 평균 비용
- [ ] 최소 20건 outcome 축적 후 패턴 분석 가능 확인

## Dependencies
- T9 (telemetry/cancel — cost 파이프라인)
- T7-full (orchestrator — outcome hook point)
- v1.0 출시 (실사용 데이터 수집 시작)

## Files owned (예상)
- `src-tauri/src/telemetry/outcome.rs` (신규)
- `src-tauri/src/telemetry/cost.rs` (Codex cost 수정)
- `src/components/OutcomeDashboard.tsx` (신규, 선택)

## NEVER
- `src/lib/synthesis/merge.ts` — synthesis 알고리즘 변경 금지
- `src-tauri/src/safety/` — guard/scanner 변경 금지

## Risks
1. **Codex cost=$0 문제**: OpenAI Codex CLI가 usage 응답을 제공하지 않으면 cost 추적 불가능. **대응**: adapter에서 token count estimation fallback
2. **task difficulty confound**: 같은 flow에서도 task 난이도에 따라 outcome이 크게 다름 → ROI 비교 의미 없을 수 있음. **대응**: task complexity proxy (file count, line count) 함께 기록
3. **사용자 accept/reject 피로**: 매번 명시적 피드백 요구 시 UX 저하. **대응**: implicit signal (사용자가 변경 유지 vs revert) 우선

## 재평가 시점
telemetry 파이프라인 (T9) 완성 + Codex cost 추적 정상화 후.

## Validation cmd

```powershell
cargo test --manifest-path src-tauri\Cargo.toml outcome
npm test -- --run "Outcome|Dashboard"
rg -n "OutcomeRecord|cost_codex|verify.*verdict|user_accept|flow_c.*roi" src-tauri/src src
```

## Worker prompt 6 mandatory fields
1. Success criteria: Flow C outcome record 스키마 (`flow, task_hash, worker_claims_count, synthesis_result, adversarial_rounds, adversarial_verdict, user_accept, test_result, cost_claude, cost_codex, duration_ms`), Codex cost 추적 정상화 (`telemetry/cost.rs` Codex API usage 필드 파싱), verify verdict 세분화 (`Skipped | Passed | Failed | Timeout`), outcome dashboard/CLI 리포트, 최소 20건 축적 후 패턴 분석 가능 확인을 구현한다.
2. NEVER 영역: `src/lib/synthesis/merge.ts` synthesis 알고리즘 변경, `src-tauri/src/safety/` guard/scanner 변경, worker 직접 peer 호출.
3. Validation cmd:
   ```powershell
   cargo test --manifest-path src-tauri\Cargo.toml outcome
   npm test -- --run "Outcome|Dashboard"
   rg -n "OutcomeRecord|cost_codex|verify.*verdict|user_accept|flow_c.*roi" src-tauri/src src
   ```
4. Files + lines: `src-tauri/src/telemetry/outcome.rs` (신규), `src-tauri/src/telemetry/cost.rs` (Codex cost 수정), `src/components/OutcomeDashboard.tsx` (신규, 선택), `src-tauri/src/orchestrator/mod.rs` (outcome hook point).
5. Alternatives 2개 + pros/cons + 선택 근거: outcome 수집 없이 감각적 판단(코드 없지만 효과 측정 불가) vs outcome record + dashboard(효과 측정 가능, telemetry 파이프라인 의존). 선택은 outcome record + dashboard. implicit signal(user accept/reject 명시 피로 감소, 정확도 낮을 수 있음) 는 보조로 사용.
6. Tests-first: outcome record 저장/조회, Codex cost 파싱 (mock usage response), verify verdict 세분화, dashboard rendering, 20건 축적 후 통계 계산 을 먼저 실패시키고 구현한다.

## Worker prompt cross-contract fields
- GitHub/project handling: GitHub #54 / Project `MoA Desktop` card status 를 claim/complete 단계에서 갱신한다.
- Conflict matrix ownership: T23 owns 는 `src-tauri/src/telemetry/outcome.rs` (신규), `src-tauri/src/telemetry/cost.rs` (Codex cost 수정 부분), `src/components/OutcomeDashboard.tsx` (신규) 로 한정한다. `src/lib/synthesis/merge.ts`, `src-tauri/src/safety/`, orchestrator 는 read-only (outcome hook point 만 추가).
- Dependency/merge order: T9 (telemetry/cancel) + T7-full 완료 후. v1.0 출시 후 실사용 데이터 수집 시작 시 착수 권장.
- Review gate warning: lead/orchestrator-owned `CodexAdversarialXHigh` 가 `Clean` 을 반환하고 `source_output_path` 가 persisted 되기 전에는 `pr_create`, `pr_merge`, `integrate_merge`, `main_apply` 진행 금지. worker 는 직접 peer review 를 실행하지 않는다.

## Paste-ready prompt

````text
[세션 부트]
- Prompt kind: Codex Desktop manual lead ticket session
- repo: D:\moa-desktop
- branch: codex/T23-flow-c-roi-metrics
- worktree required

[Goal]
Flow C 실행 후 outcome 피드백을 수집하여 파이프라인 효과를 측정한다.

[NEVER]
synthesis 알고리즘 변경, safety guard/scanner 변경 금지.

[Validation]
cargo test --manifest-path src-tauri\Cargo.toml outcome
npm test -- --run "Outcome|Dashboard"

[작업 완료 시]
outcome schema, Codex cost 정상화 결과, dashboard 를 보고한다.
````

[작업 완료 시 — 무조건 이 순서로]
1. commit: `feat(T23): Flow C ROI metrics + outcome record + Codex cost tracking` (본문에 `Closes #54` 포함, push 금지)
2. **GitHub 카드 완료 처리 — 잊지 말고 무조건 실행**:
   ```
   node ~/.claude/scripts/gh-tickets.mjs complete D:\moa-desktop 54
   ```
   - 출력에 `COMPLETED=54` 또는 `ALREADY_CLOSED=54` 가 보여야 OK.
   - 실패 시 사용자 보고 + STOP.
3. 보고: outcome schema, Codex cost 정상화 결과, dashboard, **GitHub 카드 close 결과 1줄**.

## Estimated effort
Medium — outcome record 스키마 + cost.rs 수정 + outcome hook + 대시보드 기본형.
