# T15e — Pi Extension UI Bridge

GitHub: #41 (https://github.com/mizan0515/moa-desktop/issues/41)

## 새 Claude 창 만들기 가이드
T15c (또는 T15b) + T15d 통과 후. worktree: T15e-extension-ui.

---

````
[세션 부트]
- repo: D:\moa-desktop
- base branch: master (T15c + T15d 머지 후)
- 권장 분기: feat/T15e-pi-extension-ui-bridge
- 권위: PROJECT-RULES.md, AGENTS.md, DESIGN.md, TICKETS/T15c-pi-sdk-sidecar-host.md, TICKETS/T15d-pi-package-trust-installer.md
- 운영: MoA Flow C — § 2.6 템플릿 A

[의존성 self-check — claim 직후, first-pass 시작 전 무조건 실행]
master 에 선행 commit 2개 있는지 확인:
```
cd D:\moa-desktop && git log master --oneline -100 | rg -i "feat\(T15c\)|feat\(T15d\)" | wc -l
```
- 결과 `2` 면 OK — 작업 진행
- 2 미만이면 **STOP — "선행 티켓이 master 에 미머지" 사용자 보고**.

[INDEPENDENT FIRST-PASS — read-only]
````

## Goal

Pi extension `ctx.ui` 와 RPC extension UI requests 를 MoA React UI 로 매핑한다. UI injection 은 capability allowlist 없이는 금지한다.

## 의존성

- 선행: T15c SDK sidecar host 또는 T15b RPC event stream.
- 선행: T15d capability manifest for third-party extension UI.

## Success criteria

- [ ] `confirm`, `input`, `select`, `editor` dialog request 를 modal/inline input/select dialog 로 매핑한다.
- [ ] `notify`, `setStatus`, `setWidget`, `setTitle`, `setEditorText` 를 toast/timeline/lane status/widget slot 으로 매핑한다.
- [ ] custom UI/widget 은 named right-panel slot 과 renderer registry 로만 표시한다.
- [ ] capability allowlist 없는 extension UI request 는 blocked event 로 표시하고 agent 에 cancel/default response 를 보낸다.
- [ ] dialog timeout 은 agent-side timeout 을 존중하고 UI 에 stale prompt 를 남기지 않는다.
- [ ] extension UI event 는 lane journal 과 ResumePacket 에 기록된다.
- [ ] malicious HTML/script injection 은 escaped/sanitized renderer 에서 차단된다.

## Files owned

- `src-tauri/src/pi/extension_ui.rs`
- `src-tauri/tests/pi_extension_ui_*.rs`
- `src/components/PiExtensionPanel.tsx`
- `src/components/PiExtensionWidgetSlot.tsx`
- `src/lib/piExtensionUi.ts`

## Read-only

- T13 policy/safety APIs
- T15b/T15c event protocols
- T15d capability manifest

## NEVER 영역

- arbitrary DOM injection 금지.
- capability 없는 custom renderer 금지.
- extension UI 가 GitHub/network/filesystem permission confirm 을 우회하지 못하게 한다.
- worker nested peer-call 을 UI action 으로 숨기지 않는다.

## Validation cmd

```powershell
cargo test --manifest-path src-tauri\Cargo.toml pi_extension_ui
npm test -- --run PiExtension
rg -n "extension_ui_request|confirm|input|select|setWidget|capability|sanitize|PiExtension" src-tauri/src src
```

## Alternatives

1. Support only RPC dialog methods
   - Pros: minimal.
   - Cons: Pi custom UI value reduced.
2. Full custom renderer bridge (선택, allowlisted)
   - Pros: unlocks Pi extension UI in MoA.
   - Cons: requires strict renderer registry and sanitization.
3. Disable extension UI
   - Pros: safest.
   - Cons: packages/extensions feel broken.

## Tests-first

Failing UI tests first: confirm/select/input happy path, denied no-capability request, timeout cleanup, sanitized renderer, ResumePacket persistence.

## Worker prompt 6 mandatory fields
1. Success criteria: confirm/input/select/editor dialog mapping, notify/setStatus/setWidget/setTitle/setEditorText mapping, named right-panel widget slot + renderer registry, no-capability blocked event, dialog timeout cleanup, journal/ResumePacket 기록, HTML/script injection sanitization 을 구현한다.
2. NEVER 영역: arbitrary DOM injection, capability 없는 custom renderer, extension UI 가 permission confirm 우회, worker nested peer-call 을 UI action 으로 은닉.
3. Validation cmd:
   ```powershell
   cargo test --manifest-path src-tauri\Cargo.toml pi_extension_ui
   npm test -- --run PiExtension
   rg -n "extension_ui_request|confirm|input|select|setWidget|capability|sanitize|PiExtension" src-tauri/src src
   ```
4. Files + lines: `TICKETS/T15e-pi-extension-ui-bridge.md` 의 Success criteria/NEVER, `TICKETS/T15c-pi-sdk-sidecar-host.md` 의 IPC event protocol, `TICKETS/T15d-pi-package-trust-installer.md` 의 capability manifest.
5. Alternatives 2개 + pros/cons + 선택 근거: RPC dialog methods only(minimal, Pi custom UI 가치 감소) vs Full custom renderer bridge + allowlist(Pi extension UI unlock, strict registry/sanitization 필요). 선택은 Full custom renderer bridge.
6. Tests-first: confirm/select/input happy path, denied no-capability request, timeout cleanup, sanitized renderer, ResumePacket persistence 를 먼저 실패시키고 구현한다.

## Worker prompt cross-contract fields
- GitHub/project handling: GitHub #41 / Project `MoA Desktop` card status 를 claim/complete 단계에서 갱신한다.
- Conflict matrix ownership: T15e owns 는 `src-tauri/src/pi/extension_ui.rs`, `src-tauri/tests/pi_extension_ui_*.rs`, `src/components/PiExtensionPanel.tsx`, `src/components/PiExtensionWidgetSlot.tsx`, `src/lib/piExtensionUi.ts` 로 한정한다. T13 policy/safety, T15b/T15c event protocols, T15d capability manifest 는 read-only.
- Dependency/merge order: T15c + T15d 완료 후 시작. T15f 는 T15e 이후 또는 병행 가능.
- Review gate warning: lead/orchestrator-owned `CodexAdversarialXHigh` 가 `Clean` 을 반환하고 `source_output_path` 가 persisted 되기 전에는 `pr_create`, `pr_merge`, `integrate_merge`, `main_apply` 진행 금지. worker 는 직접 peer review 를 실행하지 않는다.

## Paste-ready prompt

```text
[세션 부트]
- Prompt kind: Codex Desktop manual lead ticket session
- repo: D:\moa-desktop
- branch: codex/T15e-pi-extension-ui-bridge
- worktree required

[Goal]
Pi extension UI requests 를 MoA UI 로 안전하게 bridge 한다.

[NEVER]
arbitrary DOM injection, no-capability UI, permission bypass, nested peer-call action 금지.

[Validation]
cargo test --manifest-path src-tauri\Cargo.toml pi_extension_ui
npm test -- --run PiExtension

[작업 완료 시]
mapping table, denied cases, renderer registry policy 를 보고한다.
```

[작업 완료 시 — 무조건 이 순서로]
1. commit: `feat(T15e): Pi extension UI bridge + renderer registry + sanitization` (본문에 `Closes #41` 포함, push 금지)
2. **GitHub 카드 완료 처리 — 잊지 말고 무조건 실행**:
   ```
   node ~/.claude/scripts/gh-tickets.mjs complete D:\moa-desktop 41
   ```
   - 출력에 `COMPLETED=41` 또는 `ALREADY_CLOSED=41` 가 보여야 OK.
   - 실패 시 사용자 보고 + STOP.
3. 보고: mapping table, denied cases, renderer registry policy, **GitHub 카드 close 결과 1줄**.
