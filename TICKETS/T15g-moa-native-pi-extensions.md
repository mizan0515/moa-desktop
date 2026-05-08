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
- `src-tauri/tests/pi_native_extensions_*.rs`

## Read-only

- T13 safety/review APIs
- T15d/e/f APIs
- T4 lock/worktree APIs

## NEVER 영역

- first-party extension 이 command guard 를 우회하지 않는다.
- Pi review 를 mandatory Codex review gate 로 대체하지 않는다.
- mutation owner 승격을 default on 으로 하지 않는다.
- third-party package 를 first-party 로 가장하지 않는다.

## Validation cmd

```powershell
npm test --workspace sidecars/moa-pi-host
cargo test --manifest-path src-tauri\Cargo.toml pi_native_extensions
rg -n "moa-tool-guard|moa-review-gate|moa-ticket-context|moa-lane-telemetry|mutation owner|CodexAdversarialXHigh" sidecars src-tauri/src TICKETS
```

## Alternatives

1. No first-party extensions
   - Pros: less maintenance.
   - Cons: Pi integration remains generic and less safe.
2. First-party guard/review/context extensions (선택)
   - Pros: makes Pi extension power align with MoA policy.
   - Cons: extension API version drift to maintain.
3. Allow community extensions directly
   - Pros: fastest ecosystem adoption.
   - Cons: high bypass/supply-chain risk.

## Tests-first

Failing tests first: guard blocks peer command, review gate cannot be replaced, ticket context read-only, telemetry redaction, mutation owner setting remains off by default.

## Paste-ready prompt

```text
[세션 부트]
- Prompt kind: Codex Desktop manual lead ticket session
- repo: D:\moa-desktop
- branch: codex/T15g-moa-native-pi-extensions
- worktree required

[Goal]
MoA first-party Pi extensions 를 구현해 Pi runtime 을 MoA policy 와 연결한다.

[NEVER]
guard bypass, Codex gate replacement, default mutation owner, third-party as first-party 금지.

[Validation]
npm test --workspace sidecars/moa-pi-host
cargo test --manifest-path src-tauri\Cargo.toml pi_native_extensions

[작업 완료 시]
extension list, safety tests, mutation-owner opt-in prerequisites 를 보고한다.
```
