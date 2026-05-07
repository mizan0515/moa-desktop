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

## NEVER 영역

- arbitrary HTML/script injection
- capability allowlist 없는 custom UI render
- extension UI 가 mutation lock/review gate 를 우회
- package capability manifest 없는 third-party extension activation

## Worker prompt 6 mandatory fields

1. Success criteria: dialog/fire-and-forget mapping, custom slot registry, blocked events, timeout, journal/ResumePacket persistence, sanitizer.
2. NEVER 영역: unsafe injection, unallowed custom UI, lock/review bypass, manifest-less extension.
3. Validation cmd:
   ```powershell
   cargo test --manifest-path src-tauri\Cargo.toml pi_extension_ui
   npm test -- --run PiExtension
   ```
4. Files + lines: this ticket Success criteria, T15d package capability schema, DESIGN.md Pi Runtime Adapter.
5. Alternatives 2개 + pros/cons + 선택 근거: fixed dialog-only mapping(very safe but loses Pi UI value) vs allowlisted slot registry(safe enough and extensible). 선택은 allowlisted slot registry.
6. Tests-first: blocked unallowed UI, timeout cleanup, sanitizer, ResumePacket persistence tests 를 먼저 실패시킨다.
