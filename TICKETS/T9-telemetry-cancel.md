# T9 — Telemetry + cancellation + error UX

## 새 Claude 창 만들기 가이드
T2 통과 후 (Phase 4). worktree: T9-telemetry.

---

```
[세션 부트]
- repo: D:\moa-desktop
- base branch: master (T2 + T7-full 통합 후)
- 권장 분기: feat/T9-telemetry
- 권위: PLAN.md (§ F6 prompt cache awareness, version pinning, error 분류, retry tracking, L2 cost telemetry), DESIGN.md
- 운영: MoA Flow C — § 2.6 템플릿 A

[INDEPENDENT FIRST-PASS — read-only]

## Goal
운영 가시성 + 안정성:
1. **Cost telemetry** — session 당 token 누적 (input, output, cache_read, cache_create), $ 환산, cap warning
2. **Cancellation UI** — Stop 버튼 → orchestrator 가 모든 진행 중 Worker process tree kill (T2 abort 호출)
3. **Error classification UI** — T2 의 error enum 받아 사용자에 actionable 메시지 (`gh auth refresh`, `claude login` 등)
4. **Version drift warning** — session 시작 시 CLI/plugin version 기록, 다른 session 과 차이 나면 경고
5. **Prompt cache awareness** — 매 `claude -p` 가 fresh session = cache reuse 0. 비용 multiplier 명시 + UI 경고

## Success criteria
- [ ] `src-tauri/src/telemetry/{counter.rs,cost.rs,version.rs,mod.rs}` — token counter, cost calculator, CLI version snapshot
- [ ] `src-tauri/src/cancel/{tree_kill.rs,mod.rs}` — process tree kill (T2 의 abort 와 통합)
- [ ] `src/components/CostMeter.tsx` — running token + $ 표시, cap 도달 시 warning banner
- [ ] `src/components/ErrorBanner.tsx` — T2 error enum 별 actionable 메시지
- [ ] `src/components/VersionDriftWarning.tsx` — version drift 시 dismissible warning
- [ ] cost cap setting (default $10/session) — 도달 시 사용자 confirm 없이 진행 안 함
- [ ] cache_read 가 0 인 게 정상임을 사용자 안내 (fresh session 매번)
- [ ] unit + UI test

## Files owned
- `src-tauri/src/telemetry/*.rs`
- `src-tauri/src/cancel/*.rs`
- `src/components/CostMeter.tsx` (T1 의 stub 채움)
- `src/components/ErrorBanner.tsx` (T1 의 stub 채움)
- `src/components/VersionDriftWarning.tsx`
- `src-tauri/tests/{telemetry,cancel}_test.rs`

## Read-only
- T2 ProcessRunner (abort 신호), error enum
- T7-full orchestrator (cost 누적 hook 지점)

## NEVER 영역
- src-tauri/src/{process,adapters,safety,git,lock,journal,synthesis,orchestrator}/ body
- T1, T6 영역의 layout 변경 (component 추가만)

## Stop conditions
- T2 abort 가 process tree kill 보장 안 됨 (spike S7 NO-GO) → 사용자 보고
- token counter 가 Claude/Codex stream-json 에서 추출 가능한지 (TOKEN-GUARD.md 의 input/output/cache 토큰 측정 참고)

## Deliverable (first-pass)
1. Diagnosis: stream-json 의 token 필드 위치 (Claude Code docs 검증)
2. Approach: cost cap 정책 (hard stop vs warning) (대안 2개)
3. Risks
4. error enum → message 매핑
5. Open questions

## Constraints
- 6 항목 의무
- 비밀 (token, key) UI 표시 X
- 비용은 추정치 명시 (정확치 X — Anthropic billing 과 다를 수 있음)

[작업 완료 시]
- commit: `feat(T9): telemetry + cancellation + error UX + version drift`
- 보고: cost cap 기본값, error message 카탈로그, Phase 4 hardening 완료
```
