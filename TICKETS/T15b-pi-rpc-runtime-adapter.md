# T15b — Pi RPC Runtime Adapter

GitHub: #38 (https://github.com/mizan0515/moa-desktop/issues/38)

## Goal

Tauri/Rust `ProcessRunner` 위에 `PiRpcAdapter` 를 구현해 `pi --mode rpc --no-session` 기반 Pi lane 을 첫 실행한다. 초기 permission 은 read-only/research/reviewer/conversational only 이다.

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
- [ ] Pi advisory review result 는 mandatory `CodexAdversarialXHigh` gate 를 대체하지 않는다.

## Files owned

- `src-tauri/src/adapters/pi_rpc.rs`
- `src-tauri/src/pi/rpc.rs`
- `src-tauri/src/pi/mod.rs`
- `src-tauri/tests/pi_rpc_*.rs`
- `src/lib/piRpcEvents.ts`

## Read-only

- T2 ProcessRunner
- T13 WorkerCommandGuard/ReviewRunRecord
- T15a spike report

## NEVER 영역

- Pi package install/update/hot reload
- mutation owner permission
- T13 safety/review gate relaxation
- shell string command builder
- worker nested peer-call

## Worker prompt 6 mandatory fields

1. Success criteria: strict JSONL, command correlation, typed event mapping, blocked extension UI stubs, read-only permission.
2. NEVER 영역: package install/update, mutation owner, T13 relaxation, shell string command, nested peer-call.
3. Validation cmd:
   ```powershell
   cargo test --manifest-path src-tauri\Cargo.toml pi_rpc
   npm test -- --run piRpcEvents
   ```
4. Files + lines: this ticket Success criteria, `DESIGN.md` Pi Runtime Adapter, `PLAN.md` § 0.7, T15a report.
5. Alternatives 2개 + pros/cons + 선택 근거: direct ProcessRunner adapter(단순/빠름) vs generic JSONL harness trait first(재사용 좋지만 larger blast radius). 선택은 PiRpcAdapter local implementation + later trait extraction only if duplication appears.
6. Tests-first: framing/correlation/parser/event mapping/blocked UI request tests 를 먼저 실패시킨다.
