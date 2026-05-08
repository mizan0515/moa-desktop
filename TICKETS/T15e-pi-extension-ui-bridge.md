# T15e — Pi Extension UI Bridge

GitHub: #41 (https://github.com/mizan0515/moa-desktop/issues/41)

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
