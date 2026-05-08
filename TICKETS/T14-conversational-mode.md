# T14 — Conversational Mode (Claude/Codex Desktop parity)

GitHub: #29 (https://github.com/mizan0515/moa-desktop/issues/29)

## 배경

본 앱은 자동화 mode (앱 orchestrator 가 Claude/Codex sibling worker 를 spawn) 외에, 사용자가 현재 Claude Code Desktop 에서 Codex MCP 와 상호작용하듯 진행하는 conversational mode 도 지원해야 한다.

중요한 경계: 여기서 "peer" 는 worker 가 다른 worker 를 직접 호출한다는 뜻이 아니다. 모든 cross-AI dispatch 는 UI 또는 앱 orchestrator 가 수행한다. Worker 내부에서 `/codex:*`, `claude -p`, `codex exec`, Claude MCP/Codex MCP 를 호출하는 nested peer-call 은 T13 L2 scanner 가 차단한다.

T15 이후 Pi 도 conversational candidate runtime 이다. Pi 는 parent-owned `HarnessRuntime` 이며 worker 내부 peer-call 이 아니다. Pi conversational lane 은 `runtimeKind="pi"` 로 표시되고 `PiRpcAdapter` 또는 `PiSdkHost` capability gate 가 available 일 때만 활성화된다.

## 의존성

- T13 L1-L5 통과 후 진입.
- 특히 L2 scanner, L2.5 ReviewVerdict/ReviewInputStrategy, L4 slash dispatcher, L5 ResumePacket 이 토대.
- Pi lane support 는 T15f model/session tree 이후 활성화한다. 그 전에는 schema/commentary 만 보존한다.

## Goal

`settings.interactionMode: "automated" | "conversational"` 를 도입하고, conversational mode 에서 사용자가 Claude/Codex 양쪽을 대화형 peer 로 다루되, 권한·review gate·resume·thinking visibility 는 앱이 통제한다. 현재 T5a/T5b adapter 는 prompt 전달 후 stdin 을 닫는 one-shot 모델이므로, v1.6 기본 설계는 **spawn-new-turn + ResumePacket state carryover** 다. same-stream redirect/ASK_USER roundtrip 은 interactive/resumable adapter 가 구현된 뒤 확장한다.

## Success criteria

- [ ] Mode toggle: `automated` / `conversational`. 진행 중 세션에는 영향 없고 다음 세션부터 적용.
- [ ] Thinking visibility: Claude thinking summary / Codex reasoning summary 를 `thinking_chunk` 이벤트로 분리해 expand/collapse 표시.
- [ ] Turn-level intervention: 사용자가 진행 중 worker 를 stop/pause 하고, ResumePacket state carryover 로 새 worker turn 에 redirect 메시지를 전달 가능. mutation lock 보유 중 redirect 는 safety scanner 와 lock state 가 상태 확인.
- [ ] Worker-to-user question: worker 가 `<ASK_USER>...</ASK_USER>` marker 를 내보내면 UI input 으로 답변을 받아 새 worker turn 에 state + 답변을 전달. 동일 worker stream 주입은 interactive/resumable adapter 구현 전까지 scope 밖.
- [ ] Peer dispatch boundary: 사용자가 UI 에서 "Codex 검토" 또는 "Claude 검토" 를 누르면 앱/orchestrator 가 호출. worker output 의 nested peer-call 패턴은 차단.
- [ ] Pi conversational lane: `runtimeKind="pi"` lane 은 read-only/research/conversational permission 으로만 시작하며 package install/update/hot reload 요청은 blocked/confirm-needed event 로 표시한다.
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
- PLAN.md, DESIGN.md, PROJECT-RULES.md, AGENTS.md

## NEVER 영역

- Worker 가 peer AI 를 직접 호출하는 path 추가 금지.
- T13 scanner/review/lifecycle 본체 수정 금지. 필요한 API gap 은 follow-up 으로 분리.
- 인증 파일, settings raw secret, auth/cookie/token/session cache 저장 금지.

## Stop conditions

- Codex Desktop attach/shared instance 가 sandbox 분리를 보장하지 못하면 spawn-only 로 fallback 하고 attach 는 pending 처리.
- thinking/reasoning summary stream 이 CLI 별로 안정적이지 않으면 provider-specific adapter feature flag 로 격리.
- same-stream redirect/ASK_USER 구현을 요구받으면 STOP — 먼저 interactive/resumable adapter ticket 을 분리한다.
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

## Worker prompt 6 mandatory fields
1. Success criteria: automated/conversational mode toggle, thinking summary chunks, stop/pause redirect, ASK_USER roundtrip, UI/orchestrator-mediated peer dispatch, ResumePacket 복원을 구현한다.
2. NEVER 영역: worker 직접 peer AI 호출 path, T13 scanner/review/lifecycle 본체, 인증 파일/settings raw secret/auth-cookie-token-session cache.
3. Validation cmd:
   ```powershell
   cargo test --manifest-path src-tauri\Cargo.toml conversation
   npm test -- --run ConversationPanel
   ```
4. Files + lines: `TICKETS/T14-conversational-mode.md` 의 Success criteria/Constraints, `DESIGN.md` 의 orchestrator/review gate/conversation 관련 section, `AGENTS.md` 의 worker peer boundary.
5. Alternatives 2개 + pros/cons + 선택 근거: spawn-new-turn state carryover(현재 adapter 로 구현 가능, same-stream UX 는 제한) vs interactive/resumable adapter(UX 좋지만 adapter 범위가 커짐). 선택은 spawn-new-turn state carryover.
6. Tests-first: mode toggle, thinking collapse, ASK_USER roundtrip, redirect safety, nested peer-call block, review gate source_output_path persistence test 를 먼저 실패시키고 구현한다.

## Worker prompt cross-contract fields
- GitHub/project handling: GitHub #29 / Project `MoA Desktop` card status 를 claim/complete 단계에서 갱신한다. conversational mode 는 PR/merge 를 직접 수행하지 않지만 close 전 review gate record 를 보고한다.
- Conflict matrix ownership: T14 owns 는 `src-tauri/src/conversation/*`, `src-tauri/src/commands/conversation.rs`, `src/components/ConversationPanel.tsx`, `src/components/ThinkingBlock.tsx`, `src/lib/conversation/*`, conversation tests 로 한정한다. T13 scanner/review/lifecycle 은 read-only 이며 API gap 은 follow-up 으로 분리한다.
- Dependency/merge order: T14 는 T13 이후 시작한다. T10/T11/T12 merge order 를 생성하거나 변경하지 않으며, ResumePacket 안에서는 기존 `reviewRunRecords` 와 gate verdict 를 보존만 한다.
- Review gate warning: lead/orchestrator-owned `CodexAdversarialXHigh` 가 `Clean` 을 반환하고 `source_output_path` 가 persisted 되기 전에는 `pr_create`, `pr_merge`, `integrate_merge`, `main_apply` 진행 금지. worker 는 직접 peer review 를 실행하지 않는다.

[작업 완료 시]
1. commit: `feat(T14): conversational mode with mediated Claude/Codex peer UI` (본문에 `Closes #29` 포함, push 금지)
2. Review gate: T13 L2 scanner + command_guard 로 nested peer-call 0건 검증 후, lead/orchestrator-owned mandatory `CodexAdversarialXHigh` review gate `Clean` 전 `pr_create`, `pr_merge`, `integrate_merge`, `main_apply` 진행 금지. Codex Desktop 수동 개발이면 lead PowerShell 별도 리뷰 프로파일 결과를 `.moa-desktop/reviews/<stamp>.md` 에 capture 하고 `ReviewRunRecord.source_output_path` 로 남긴다.
3. GitHub 카드 완료:
   ```
   node ~/.claude/scripts/gh-tickets.mjs complete D:\moa-desktop 29
   ```
4. 보고: mode toggle, thinking visibility, spawn-new-turn ask-user/redirect demo, nested peer-call block test, GitHub 카드 close 결과.
