# T8 — Mock mode + canned worker responses

> ✅ **DONE** — 2026-05-06, commit `44dae08`. 6 canned JSONL fixtures + `MockRunner` (`ProcessRunner` impl) + 5 tests passing. Codex adversarial review applied (abort→Killed kind parity, empty-argv parity, synthesis open-row count invariant locked by test).

## 새 Claude 창 만들기 가이드
1. T1 통과 후 새 창 (worktree 권장: T8-mock)
2. 아래 프롬프트 붙여넣기

---

```
[세션 부트]
- repo: D:\moa-desktop
- base branch: master (T1 머지 후)
- 권장 분기: feat/T8-mock
- 권위: DESIGN.md, PLAN.md, synthesis.md, ~/.claude/CODEX-MCP.md
- 운영: MoA Flow C — § 2.6 템플릿 A

[INDEPENDENT FIRST-PASS — read-only]

## Goal
settings.mockMode=true 시 worker spawn 호출이 실제 CLI 대신 canned JSON 을 stream 형태로 반환. dry-run flow 의 데이터 소스.

## Success criteria
- [ ] mockResponses/ 에 canned 파일 6종: claude_firstpass.json, codex_firstpass.json, synthesis.json, claude_adversarial.json, codex_adversarial.json, final_report.json — 실제 worker 가 만들 schema 와 동일
- [ ] src-tauri/src/mock/runner.rs: mockMode flag 받으면 파일 읽어 line-by-line 으로 emit (실제 spawn 대체). 100ms delay 로 streaming 흉내
- [ ] src-tauri/src/mock/mod.rs export
- [ ] settings.mockMode 진입점은 T2 의 process runner trait 를 implement (T2 가 정의할 trait 시그니처는 src-tauri/src/process/mod.rs 의 stub comment 로 합의 — T1 에서 미리 작성된 stub 참조)
- [ ] unit test: mock runner 가 canned JSON line 6개 → 100ms 간격으로 emit 검증

## Files owned
- `mockResponses/*.json` (6종)
- `src-tauri/src/mock/*.rs` (mod.rs body 포함)
- `src-tauri/tests/mock_runner_test.rs`

## Read-only
- T1 의 process/mod.rs stub (trait 시그니처 합의용)
- DESIGN.md, PLAN.md, synthesis.md

## NEVER 영역
- src-tauri/src/process/*.rs body (T2)
- src-tauri/src/adapters/*.rs (T5a/T5b)
- src/components/* (T1, T6)
- 실제 CLI 호출 코드

## Stop conditions
- T2 trait 시그니처가 mock 으로 implement 어렵다 → T2 worker 와 schema 협의 후 진행 (PLAN.md 업데이트)
- canned JSON schema 가 T6 (UI) 가 기대하는 모양과 다름 → T6 schema 우선 합의

## Deliverable (first-pass)
1. Diagnosis: mock runner 의 trait shape 추론 (T1 stub 참조)
2. Approach: file-based canned vs in-code constant (대안 2개)
3. Risks
4. canned JSON 6종 의 sample claim 1개씩 (verified/codex-only/claude-only/disagreement/open 각각 cover)
5. Open questions

## Constraints
- 6 항목 의무
- canned JSON 의 claim 텍스트는 본 프로젝트 자체 (MoA Desktop) 가 아닌 임의 sample 로 (혼동 방지)
- 비밀 파일 미포함

[작업 완료 시]
- commit: `feat(T8): mock mode + 6 canned worker responses`
- 보고: schema 합의 항목, T2/T6 worker 가 알아야 할 contract
```
