# TINTEGRATE — Final integration + verification + README

## 새 Claude 창 만들기 가이드
모든 ticket 머지 후. lead 세션에서 직접 진행 권장 (단일 세션 1개).

---

```
[세션 부트]
- repo: D:\moa-desktop
- base branch: master (T0~T9 모두 머지 후)
- 권장 분기: feat/TINTEGRATE
- 권위: PLAN.md verification checklist + adversarial review 의 § F6 추가 항목, DESIGN.md verification, ~/.claude/CODEX-MCP.md
- 운영: MoA Flow C — § 2.6 템플릿 A

[INDEPENDENT FIRST-PASS — read-only]

## Goal
모든 subsystem 통합 검증 + dep/script 정리 + README + dry-run 데모.

## Success criteria
- [ ] `Cargo.toml` deps 정리 — 중복 제거, version 통일. 누락 없음 (T2/T4/T5/T7/T9 의 추가 dep 모두 반영)
- [ ] `package.json` scripts 정리 — `tauri dev`, `tauri build`, `test`, `test:unit`, `test:e2e`, `lint`, `typecheck`
- [ ] DESIGN.md verification checklist 모두 통과 (앱 시작 → workbench / mock mode 동작 / Claude prompt 에 "Do not call Codex" 포함 / Codex prompt 에 "Do not call Claude" / output scanner 동작 / first-pass read-only / mutation 1 owner / same-file sequential lock transfer / 5칸 synthesis 렌더 / Claim Ledger 렌더 / 비밀 미저장 / 좁은 폭 OK)
- [ ] PLAN.md § F6 추가 항목 검증: recovery journal reconcile, multi-instance refusal, retry tracking, prompt cache awareness, version drift warning, error classification UX, concurrent log writes
- [ ] e2e test happy path:
  1. mock mode: 작업 입력 → 5칸 + Claim Ledger 렌더
  2. real mode (small synthetic project): first-pass × 2 → synthesis → adversarial → mutation worktree → patch apply → final report
- [ ] README.md: install, run, settings, troubleshooting (CLI missing, auth expired)
- [ ] dry-run demo 시나리오 문서 (`docs/demo.md`) — 사용자가 5분 안에 첫 실행 + 결과 보기

## Files owned
- `Cargo.toml` (workspace + sub-Cargo.toml — T2~T9 가 추가한 deps 통합)
- `package.json`, `package-lock.json` (deps 통합)
- `README.md`, `docs/demo.md`, `docs/troubleshooting.md`
- `e2e/*` (Playwright 또는 Tauri WebDriver 스크립트)
- `.github/workflows/*` (없어도 OK — 있다면 lint+test)

## Read-only
- 모든 다른 ticket 의 결과
- DESIGN.md, PLAN.md, synthesis.md

## NEVER 영역
- 다른 ticket 의 owned 파일 body 수정 (bug 발견 시 해당 ticket 으로 follow-up issue)
- 비밀 파일 commit
- main repo 의 .git/ 직접 변경

## Stop conditions
- 검증 checklist 항목 1개라도 FAIL → 해당 ticket 에 follow-up 안건 만들고 사용자 보고
- e2e real mode 가 비용 폭주 (cost cap 도달) → mock 으로 대체 + 사용자 confirm 후만 real

## Deliverable (first-pass)
1. Diagnosis: 모든 subsystem 의 통합 지점 (T7-full 이 T2/T3/T4/T5 호출하는 시퀀스)
2. Approach: e2e test framework 선택 (Playwright vs Tauri WebDriver) (대안 2개)
3. Risks (Cargo dep 충돌, version skew)
4. checklist 통과 매트릭스 sample
5. Open questions

## Constraints
- 6 항목 의무
- e2e 비용 < $5 — real mode 작은 task만
- 비밀 commit 절대 X

[작업 완료 시]
- commit: `feat(TINTEGRATE): final integration + verification + README + demo`
- 보고: verification 통과 매트릭스, follow-up 안건 (있다면), v0.1.0 release 준비 상태
```
