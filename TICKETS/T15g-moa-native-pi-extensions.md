# T15g — MoA Native Pi Extensions

GitHub: #43 (https://github.com/mizan0515/moa-desktop/issues/43)

## Goal

MoA first-party Pi extensions 를 제공한다. 이 ticket 이후에만 Pi mutation owner 승격을 별도 opt-in setting 으로 검토한다.

## 의존성

- 선행: T15d package trust, T15e extension UI bridge, T15f session tree.
- 선행: T13 command guard/review gate.

## Success criteria

- [ ] `moa-tool-guard` extension: Pi tool calls before execution 을 MoA `WorkerCommandGuard`/capability policy 로 검사한다.
- [ ] `moa-review-gate` extension: Pi lane 결과가 mandatory `CodexAdversarialXHigh` gate 를 대체할 수 없음을 metadata 로 고정한다.
- [ ] `moa-ticket-context` extension: T10/T11/T12 ticket/lane context 를 read-only 로 주입한다.
- [ ] `moa-lane-telemetry` extension: token/tool/session events 를 MoA telemetry 로 publish 한다.
- [ ] first-party extension package 는 pinned local/bundled hash 와 source manifest 를 가진다.
- [ ] Pi mutation owner opt-in setting 은 off by default 이고, T4 worktree lock + T15d trust + T15e UI + review gate all-pass 일 때만 enabled 후보가 된다.

## Files owned

- `sidecars/moa-pi-host/extensions/moa-tool-guard/*`
- `sidecars/moa-pi-host/extensions/moa-review-gate/*`
- `sidecars/moa-pi-host/extensions/moa-ticket-context/*`
- `sidecars/moa-pi-host/extensions/moa-lane-telemetry/*`
- `docs/pi-native-extensions.md`

## NEVER 영역

- first-party extension 이 mandatory review gate 를 bypass
- Pi mutation owner default-on
- unpinned bundled extension
- ticket context write access
- WorkerCommandGuard bypass

## Worker prompt 6 mandatory fields

1. Success criteria: tool guard, review gate metadata, ticket context read-only, telemetry, pinned bundled manifest, mutation opt-in prerequisites.
2. NEVER 영역: gate bypass, mutation default-on, unpinned extension, writable ticket context, command guard bypass.
3. Validation cmd:
   ```powershell
   npm test -- --run moa-pi-host
   cargo test --manifest-path src-tauri\Cargo.toml pi_native_extensions
   ```
4. Files + lines: this ticket Success criteria, T15d/e/f outputs, T13 review gate invariant.
5. Alternatives 2개 + pros/cons + 선택 근거: no first-party extensions(safe but Pi integration remains shallow) vs small guard/context/telemetry set(useful and bounded). 선택은 small first-party set.
6. Tests-first: gate bypass denial, command guard denial, readonly ticket context, pinned manifest tests 를 먼저 실패시킨다.
