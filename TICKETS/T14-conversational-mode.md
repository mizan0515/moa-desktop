# T14 — Conversational Mode with Pi (Claude/Codex/Pi Desktop parity)

GitHub: #29 (https://github.com/mizan0515/moa-desktop/issues/29)

## 배경

본 앱은 자동화 mode (앱 orchestrator 가 Claude/Codex sibling worker 및 Pi harness runtime 을 spawn) 외에, 사용자가 현재 Claude Code Desktop 에서 Codex MCP 와 상호작용하듯 진행하는 conversational mode 도 지원해야 한다.

중요한 경계: 여기서 "peer" 는 worker 가 다른 worker 를 직접 호출한다는 뜻이 아니다. 모든 cross-AI dispatch 는 UI 또는 앱 orchestrator 가 수행한다. Worker 내부에서 `/codex:*`, `claude -p`, `codex exec`, Claude MCP/Codex MCP 를 호출하는 nested peer-call 은 T13 L2 scanner 가 차단한다.

## 의존성

- T13 L1-L5 통과 후 진입.
- Pi interactive lane 의 기본 후보는 T15b `PiRpcAdapter` 이후 가능하다. extension UI 는 T15e 이후, session tree/fork/compact mirror 는 T15f 이후 활성화한다. T15f 전에는 session tree 관련 동작을 blocked/stub event 로 기록한다.
- 특히 L2 scanner, L2.5 ReviewVerdict/ReviewInputStrategy, L4 slash dispatcher, L5 ResumePacket 이 토대.

## Goal

`settings.interactionMode: "automated" | "conversational"` 를 도입하고, conversational mode 에서 사용자가 Claude/Codex/Pi lane 을 대화형 harness 로 다루되, 권한·review gate·resume·thinking visibility 는 앱이 통제한다. 현재 T5a/T5b adapter 는 prompt 전달 후 stdin 을 닫는 one-shot 모델이므로 Claude/Codex 기본 설계는 **spawn-new-turn + ResumePacket state carryover** 다. Pi 는 T15b RPC session 또는 T15c SDK sidecar session 을 통해 `prompt`, `steer`, `followUp`, `abort`, `setModel`, `compact`, `fork` 를 노출할 수 있다. same-stream redirect/ASK_USER roundtrip 은 interactive/resumable adapter 가 구현된 뒤 확장한다.

## Success criteria

- [ ] Mode toggle: `automated` / `conversational`. 진행 중 세션에는 영향 없고 다음 세션부터 적용.
- [ ] Thinking visibility: Claude thinking summary / Codex reasoning summary 를 `thinking_chunk` 이벤트로 분리해 expand/collapse 표시.
- [ ] Turn-level intervention: 사용자가 진행 중 worker 를 stop/pause 하고, ResumePacket state carryover 로 새 worker turn 에 redirect 메시지를 전달 가능. mutation lock 보유 중 redirect 는 safety scanner 와 lock state 가 상태 확인.
- [ ] Worker-to-user question: worker 가 `<ASK_USER>...</ASK_USER>` marker 를 내보내면 UI input 으로 답변을 받아 새 worker turn 에 state + 답변을 전달. 동일 worker stream 주입은 interactive/resumable adapter 구현 전까지 scope 밖.
- [ ] Peer dispatch boundary: 사용자가 UI 에서 "Codex 검토" 또는 "Claude 검토" 를 누르면 앱/orchestrator 가 호출. worker output 의 nested peer-call 패턴은 차단.
- [ ] Pi conversational lane: 사용자가 UI 에서 Pi lane 을 선택하면 앱/orchestrator 가 `PiRuntimeAdapter` 로 dispatch 한다. Pi extension 이 Claude/Codex executable 또는 peer AI command 를 실행하려 하면 T13 command_guard 가 차단한다.
- [ ] Pi session tree mirror: T15f 이후 Pi session tree/fork/compact state 를 ResumePacket 에 mirror 하되 source of truth 는 MoA journal 이다. T15f 전에는 해당 UI/action 이 blocked/stub event 로 남는다.
- [ ] Pi extension UI: T15e 가 있으면 `ctx.ui` request 를 conversation panel/right widget slot 에 표시하고, 없으면 capability missing blocked event 로 처리한다.
- [ ] ResumePacket: conversational turn history, pending user questions, thinking collapsed state, review gate verdict 를 저장/복원.
- [ ] Review gate: `pr_create`, `pr_merge`, `integrate_merge`, `main_apply` 전 mandatory `CodexAdversarialXHigh` `ReviewVerdict::Clean` 만 진행. current-session advisory 는 gate 증거가 아니며, lead/orchestrator-owned `source_output_path` 가 남아야 한다.
- [ ] Integration test: mode toggle, thinking collapse, spawn-new-turn ask-user roundtrip, turn-level redirect, nested peer-call scanner/command_guard block.

## Files owned

- `src-tauri/src/conversation/*.rs`
- `src-tauri/src/commands/conversation.rs`
- `src/components/ConversationPanel.tsx`
- `src/components/ThinkingBlock.tsx`
- `src/lib/conversation/*.ts`
- `src-tauri/tests/conversation_*.rs`
- `src/components/__tests__/ConversationPanel.test.tsx`

## Read-only

- T13 `policy`, `commands`, `lifecycle`, `safety::scanner`
- T7-full orchestrator state machine
- T5a/T5b adapters
- T15b/T15c/T15e/T15f Pi runtime/session/UI contracts
- PLAN.md, DESIGN.md, PROJECT-RULES.md, AGENTS.md

## NEVER 영역

- Worker 가 peer AI 를 직접 호출하는 path 추가 금지.
- Pi extension/package 가 peer AI executable 또는 MCP 를 직접 호출하는 path 추가 금지.
- T13 scanner/review/lifecycle 본체 수정 금지. 필요한 API gap 은 follow-up 으로 분리.
- 인증 파일, settings raw secret, auth/cookie/token/session cache 저장 금지.

## Stop conditions

- Codex Desktop attach/shared instance 가 sandbox 분리를 보장하지 못하면 spawn-only 로 fallback 하고 attach 는 pending 처리.
- thinking/reasoning summary stream 이 CLI 별로 안정적이지 않으면 provider-specific adapter feature flag 로 격리.
- same-stream redirect/ASK_USER 구현을 요구받으면 STOP — 먼저 interactive/resumable adapter ticket 을 분리한다.
- Pi SDK sidecar 가 package trust/extension UI capability 없이 custom UI 를 요구하면 blocked event 로 처리하고 T15d/e follow-up 으로 분리한다.
- redirect 가 mutation lock 상태와 충돌하면 즉시 stop + 사용자 선택 요청.

## Deliverable (first-pass)

1. Diagnosis: T13 산출물 중 재사용할 API 목록.
2. Approach: spawn-new-turn state carryover vs interactive/resumable adapter (대안 2 개 + pros/cons). 기본 선택은 spawn-new-turn.
3. Risks: nested peer-call 재발, partial mutation redirect, thinking stream drift.
4. UI state schema sample.
5. Open questions.

## Constraints

- 6 항목 의무.
- UI/orchestrator-mediated peer 만 허용.
- Review gate 는 T13 L2.5/L4 와 같은 verdict schema 를 재사용.

## T15 Pi Runtime amend

- `runtimeKind="pi"` lane 을 conversational 후보로 포함한다.
- Pi model switch/thinking level/session fork/compact 는 T15f contract 를 통해 UI 에 표시한다. T15f 전에는 disabled control 또는 blocked/stub event 로만 표현한다.
- Pi extension UI 는 T15e capability allowlist 를 통과한 request 만 표시한다.
- Pi package install/update/hot reload 는 conversation 중 자동 실행하지 않는다. user confirm + T15d policy command 로만 처리한다.
- Pi review 는 conversation 중 참고 의견이 될 수 있지만 mandatory `CodexAdversarialXHigh` gate 를 대체하지 않는다.

## Worker prompt 6 mandatory fields
1. Success criteria: automated/conversational mode toggle, thinking summary chunks, stop/pause redirect, ASK_USER roundtrip, UI/orchestrator-mediated peer dispatch, Pi conversational lane, ResumePacket 복원을 구현한다.
2. NEVER 영역: worker 직접 peer AI 호출 path, Pi extension/package 직접 peer AI 호출 path, T13 scanner/review/lifecycle 본체, 인증 파일/settings raw secret/auth-cookie-token-session cache.
3. Validation cmd:
   ```powershell
   cargo test --manifest-path src-tauri\Cargo.toml conversation
   npm test -- --run ConversationPanel
   ```
4. Files + lines: `TICKETS/T14-conversational-mode.md` 의 Success criteria/Constraints/T15 amend, `DESIGN.md` 의 orchestrator/review gate/Pi Runtime/conversation 관련 section, `AGENTS.md` 의 worker peer boundary.
5. Alternatives 2개 + pros/cons + 선택 근거: Claude/Codex spawn-new-turn state carryover + Pi RPC session(현재 adapter 로 구현 가능, custom UI 는 제한) vs full interactive/resumable adapter + Pi SDK sidecar(UX 좋지만 adapter/sidecar 범위가 큼). 선택은 T15b 이후 Pi RPC lane 을 먼저 포함하고 T15c/e/f 이후 custom UI/session tree 를 확장.
6. Tests-first: mode toggle, thinking collapse, ASK_USER roundtrip, Pi lane prompt/abort, redirect safety, nested peer-call block, review gate source_output_path persistence test 를 먼저 실패시키고 구현한다.

## Worker prompt cross-contract fields
- GitHub/project handling: GitHub #29 / Project `MoA Desktop` card status 를 claim/complete 단계에서 갱신한다. conversational mode 는 PR/merge 를 직접 수행하지 않지만 close 전 review gate record 를 보고한다.
- Conflict matrix ownership: T14 owns 는 `src-tauri/src/conversation/*`, `src-tauri/src/commands/conversation.rs`, `src/components/ConversationPanel.tsx`, `src/components/ThinkingBlock.tsx`, `src/lib/conversation/*`, conversation tests 로 한정한다. T13 scanner/review/lifecycle 은 read-only 이며 API gap 은 follow-up 으로 분리한다.
- Dependency/merge order: T14 는 T13 이후 시작한다. Pi basic lane 은 T15b 이후 활성화할 수 있고, extension UI 는 T15e 이후, session tree/fork/compact mirror 는 T15f 이후에만 활성화한다. T15f 전 T14 구현은 session tree 동작을 blocked/stub event 로 처리한다. T10/T11/T12 merge order 를 생성하거나 변경하지 않으며, ResumePacket 안에서는 기존 `reviewRunRecords` 와 gate verdict 를 보존만 한다.
- Review gate warning: lead/orchestrator-owned `CodexAdversarialXHigh` 가 `Clean` 을 반환하고 `source_output_path` 가 persisted 되기 전에는 `pr_create`, `pr_merge`, `integrate_merge`, `main_apply` 진행 금지. worker 는 직접 peer review 를 실행하지 않는다.

[작업 완료 시]
1. commit: `feat(T14): conversational mode with mediated Claude/Codex/Pi UI` (본문에 `Closes #29` 포함, push 금지)
2. Review gate: T13 L2 scanner + command_guard 로 nested peer-call 0건 검증 후, lead/orchestrator-owned mandatory `CodexAdversarialXHigh` review gate `Clean` 전 `pr_create`, `pr_merge`, `integrate_merge`, `main_apply` 진행 금지. Codex Desktop 수동 개발이면 lead PowerShell 별도 리뷰 프로파일 결과를 `.moa-desktop/reviews/<stamp>.md` 에 capture 하고 `ReviewRunRecord.source_output_path` 로 남긴다.
3. GitHub 카드 완료:
   ```
   node ~/.claude/scripts/gh-tickets.mjs complete D:\moa-desktop 29
   ```
4. 보고: mode toggle, thinking visibility, Pi lane prompt/abort demo, spawn-new-turn ask-user/redirect demo, nested peer-call block test, GitHub 카드 close 결과.
