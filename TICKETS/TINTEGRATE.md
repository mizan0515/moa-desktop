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

[의존성 self-check — claim 직후, first-pass 시작 전 무조건 실행]
master 에 모든 선행 티켓 commit 이 있는지 확인 (T2~T12 + TINTEGRATE 제외):
```
cd D:\moa-desktop && git log master --oneline -200 | rg -i "feat\(T2\)|feat\(T3\)|feat\(T4\)|feat\(T5a\)|feat\(T5b\)|feat\(T6\)|feat\(T7-thin\)|feat\(T7-full\)|feat\(T8\)|feat\(T9\)|feat\(T10\)|feat\(T11\)|feat\(T12\)" | wc -l
```
- 결과 `13` 이면 OK — 작업 진행
- 13 미만이면 **STOP — "선행 티켓이 master 에 미머지" 사용자 보고** + 누락 commit 목록 작성.

[INDEPENDENT FIRST-PASS — read-only]

## Goal
모든 subsystem 통합 검증 + dep/script 정리 + README + dry-run 데모.

## Success criteria
- [ ] `src-tauri/Cargo.toml` deps 정리 — 중복 제거, version 통일. 누락 없음 (T2/T4/T5/T7/T9 의 추가 dep 모두 반영)
- [ ] `package.json` scripts 정리 — `tauri dev`, `tauri build`, `test`, `test:unit`, `test:e2e`, `lint`, `typecheck`
- [ ] DESIGN.md verification checklist 모두 통과 (앱 시작 → workbench / mock mode 동작 / Claude prompt 에 "Do not call Codex" 포함 / Codex prompt 에 "Do not call Claude" / output scanner 동작 / first-pass read-only / mutation 1 owner / same-file sequential lock transfer / 5칸 synthesis 렌더 / Claim Ledger 렌더 / 비밀 미저장 / 좁은 폭 OK)
- [ ] PLAN.md § F6 추가 항목 검증: recovery journal reconcile, multi-instance refusal, retry tracking, prompt cache awareness, version drift warning, error classification UX, concurrent log writes
- [ ] e2e test happy path:
  1. mock mode: 작업 입력 → 5칸 + Claim Ledger 렌더
  2. real mode (small synthetic project): first-pass × 2 → synthesis → adversarial → mutation worktree → patch apply → final report
- [ ] README.md: install, run, settings, troubleshooting (CLI missing, auth expired)
- [ ] dry-run demo 시나리오 문서 (`docs/demo.md`) — 사용자가 5분 안에 첫 실행 + 결과 보기

## Files owned
- `src-tauri/Cargo.toml` (T2~T9 가 추가한 deps 통합)
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

## Worker prompt 6 mandatory fields
1. Success criteria: `src-tauri/Cargo.toml` deps 정리, `package.json` scripts 정리, DESIGN.md verification checklist 전수 통과, PLAN.md § F6 검증, e2e test happy path (mock + real), README.md + docs/demo.md + docs/troubleshooting.md 를 구현한다.
2. NEVER 영역: 다른 ticket owned 파일 body 수정 (bug 발견 시 follow-up issue), 비밀 파일 commit, main repo `.git/` 직접 변경, worker 직접 peer 호출.
3. Validation cmd:
   ```powershell
   cargo test --manifest-path src-tauri\Cargo.toml
   npm test
   npm run lint && npm run typecheck
   rg -n "Do not call Codex|Do not call Claude|output scanner|mutation.*1.*owner|sequential lock" DESIGN.md src-tauri/src src
   ```
4. Files + lines: `src-tauri/Cargo.toml`, `package.json`, `README.md`, `docs/demo.md`, `docs/troubleshooting.md`, `e2e/*`, DESIGN.md verification checklist (현재 265-275줄 부근), PLAN.md § F6.
5. Alternatives 2개 + pros/cons + 선택 근거: per-ticket verification only(빠르지만 cross-ticket drift 놓침) vs final integration gate(느리지만 boundary regression 감지). 선택은 final integration gate. e2e framework: Playwright(cross-platform, web testing mature) vs Tauri WebDriver(native app 더 정확, API 미성숙). 선택은 first-pass 에서 결정.
6. Tests-first: DESIGN.md checklist 각 항목 자동 검증 스크립트, e2e mock mode happy path, e2e real mode synthetic project 를 먼저 실패시키고 구현한다.

## Worker prompt cross-contract fields
- GitHub/project handling: GitHub #13 / Project `MoA Desktop` card status 를 claim/complete 단계에서 갱신한다.
- Conflict matrix ownership: TINTEGRATE owns 는 `src-tauri/Cargo.toml` (deps 통합), `package.json`/`package-lock.json`, `README.md`, `docs/demo.md`, `docs/troubleshooting.md`, `e2e/*`, `.github/workflows/*` 로 한정한다. 다른 모든 ticket 결과는 read-only.
- Dependency/merge order: T2~T12 모든 선행 티켓 master 머지 후 시작. 최종 통합 ticket.
- Review gate warning: lead/orchestrator-owned `CodexAdversarialXHigh` 가 `Clean` 을 반환하고 `source_output_path` 가 persisted 되기 전에는 `pr_create`, `pr_merge`, `integrate_merge`, `main_apply` 진행 금지. worker 는 직접 peer review 를 실행하지 않는다.

[작업 완료 시 — 무조건 이 순서로]
1. commit: `feat(TINTEGRATE): final integration + verification + README + demo` (본문에 `Closes #13` 포함, push 금지)
2. **GitHub 카드 완료 처리 — 잊지 말고 무조건 실행** (안 하면 칠판 https://github.com/users/mizan0515/projects 에 status:doing 으로 남아 다른 세션이 또 잡을 수 있음):
   ```
   node ~/.claude/scripts/gh-tickets.mjs complete D:\moa-desktop 13
   ```
   - 출력에 `COMPLETED=13` 또는 `ALREADY_CLOSED=13` 가 보여야 OK.
   - 실패 시 사용자 보고 + STOP. gh 인증 오류면 `gh auth refresh -s project,read:project` 안내.
3. 보고: verification 통과 매트릭스, follow-up 안건 (있다면), v0.1.0 release 준비 상태, **GitHub 카드 close 결과 1줄**.
```
