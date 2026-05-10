# T15b — Pi RPC Runtime Adapter

GitHub: #38 (https://github.com/mizan0515/moa-desktop/issues/38)

## Goal

Tauri/Rust `ProcessRunner` 위에 `PiRpcAdapter` 를 구현해 `pi --mode rpc --no-session` 기반 Pi lane 을 첫 실행한다. 초기 permission 은 read-only/research/reviewer only 이다.

## 의존성

- 선행: T15a PASS 또는 acceptable UNVERIFIED with fallback.
- 선행: T13 L2 command guard, L2.5 ReviewRunRecord vocabulary.
- 후속: T15c SDK sidecar, T11 Pi lane budget support.

## Success criteria

- [ ] `src-tauri/src/adapters/pi_rpc.rs` 가 argv array 로 `pi --mode rpc --no-session` 을 spawn 한다.
- [ ] `src-tauri/src/pi/rpc.rs` 가 strict JSONL framing 을 구현한다.
- [ ] LF delimiter 만 record 로 인정하고 trailing CR 은 strip 한다. Unicode separator 를 newline 으로 취급하지 않는다.
- [ ] 모든 command 는 optional `id` 로 request/response correlation 된다.
- [ ] event stream 은 `agent_start`, `turn_start`, `message_update`, `tool_execution_*`, `extension_ui_request`, `agent_end` 를 typed event 로 매핑한다.
- [ ] supported commands: `prompt`, `steer`, `follow_up`, `abort`, `set_model`, `compact`, `get_state`.
- [ ] extension UI RPC request 는 T15e 전까지 safe stub: dialog methods 는 capability missing 으로 user-visible blocked event, fire-and-forget 는 timeline event.
- [ ] Pi lane 은 `runtimeKind="pi"` 로 lane result 에 기록되지만 mutation owner 는 reject 된다.
- [ ] malformed JSON, duplicate id, unknown response id, process exit, timeout, queue overflow tests 가 있다.

## Files owned

- `src-tauri/src/adapters/pi_rpc.rs`
- `src-tauri/src/pi/{mod.rs,rpc.rs,types.rs}`
- `src-tauri/tests/pi_rpc_*.rs`

## Read-only

- `src-tauri/src/process/*`
- `src-tauri/src/safety/*`, `src-tauri/src/policy/*`
- T5a/T5b adapter patterns

## NEVER 영역

- Pi package install/update 실행 금지.
- Pi mutation owner 허용 금지.
- Node sidecar 구현 금지 (T15c).
- T13 safety/policy 본체를 우회하지 않는다.
- worker-source peer AI command 허용 금지.

## Validation cmd

```powershell
cargo test --manifest-path src-tauri\Cargo.toml pi_rpc
rg -n "PiRpcAdapter|--mode rpc|runtimeKind.*pi|extension_ui_request|mutation.*reject" src-tauri/src src-tauri/tests
```

## Alternatives

1. Direct subprocess RPC client (선택)
   - Pros: existing ProcessRunner reuse, clear process isolation.
   - Cons: custom JSONL framing/correlation required.
2. Use Pi TypeScript RPC client through Node immediately
   - Pros: typed client may reduce parser work.
   - Cons: introduces sidecar before policy is ready.
3. Shell out per prompt using print/json mode
   - Pros: simpler.
   - Cons: loses session/control/extension UI semantics.

## Tests-first

Implement fake Pi RPC process fixtures first: valid response, streaming events, malformed JSON, id mismatch, abort race, extension UI request. Adapter code follows tests.

## Paste-ready prompt

````text
[세션 부트]
- Prompt kind: Codex Desktop manual lead ticket session
- repo: D:\moa-desktop
- branch: codex/T15b-pi-rpc-runtime-adapter
- worktree required

[Goal]
`PiRpcAdapter` 를 ProcessRunner 위에 구현한다. Pi lane 은 read-only/research/reviewer only.

[NEVER]
Pi package install/update, mutation owner, Node sidecar, T13 bypass 금지.

[Validation]
cargo test --manifest-path src-tauri\Cargo.toml pi_rpc

[작업 완료 시]
adapter API, JSONL framing guarantees, denied mutation-owner evidence, T15c handoff 를 보고한다.
````
