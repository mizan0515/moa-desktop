# T15c — Pi SDK Sidecar Host

GitHub: #39 (https://github.com/mizan0515/moa-desktop/issues/39)

## Goal

Node sidecar `moa-pi-host` 에서 `@earendil-works/pi-coding-agent` SDK 를 직접 구동해 Pi packages/extensions/custom UI/session tree 의 기반을 만든다. Rust/Tauri 는 sidecar 와 JSONL 또는 local IPC 로 통신한다.

## 의존성

- 선행: T15b Pi RPC MVP.
- 선행: T13 command guard/policy pack.
- 후속: T15d package trust, T15e extension UI bridge, T15f session tree.

## Success criteria

- [ ] `sidecars/moa-pi-host/*` 에 SDK host package skeleton 이 있다.
- [ ] host 는 `@earendil-works/pi-coding-agent` 를 사용한다. deprecated package import 금지.
- [ ] SDK imports: `createAgentSession`, `DefaultResourceLoader`, `createEventBus`, `ModelRegistry`, `SessionManager`.
- [ ] Rust bridge `src-tauri/src/pi/sidecar.rs` 는 argv array 로 sidecar 를 spawn 한다.
- [ ] IPC protocol 은 `start_session`, `prompt`, `abort`, `set_model`, `compact`, `fork`, `reload_extensions` command 와 response/event correlation 을 가진다.
- [ ] `DefaultResourceLoader` 는 MoA approved resource roots 만 본다.
- [ ] sidecar stdout/stderr 는 secret redaction 과 bounded log 정책을 따른다.
- [ ] packaging note: Tauri sidecar binary/Node runtime bundling/signing/update strategy 를 `docs/pi-sidecar-packaging.md` 에 기록한다.

## Files owned

- `sidecars/moa-pi-host/*`
- `src-tauri/src/pi/sidecar.rs`
- `src-tauri/tests/pi_sidecar_*.rs`
- `docs/pi-sidecar-packaging.md`

## Read-only

- T15b RPC event vocabulary
- T13 PolicyPack/CommandGuard
- package manifests outside sidecar scope

## NEVER 영역

- root `package.json`/lockfile 무관 변경
- deprecated `@mariozechner/pi-coding-agent` import
- arbitrary filesystem resource loader
- sidecar-origin peer AI command execution
- package install/update without T15d policy

## Worker prompt 6 mandatory fields

1. Success criteria: sidecar skeleton, SDK imports, IPC correlation, approved resource roots, bounded/redacted logs, packaging note.
2. NEVER 영역: root manifest churn, deprecated package, arbitrary resource loader, peer command execution, untrusted package install/update.
3. Validation cmd:
   ```powershell
   cargo test --manifest-path src-tauri\Cargo.toml pi_sidecar
   npm test -- --run moa-pi-host
   ```
4. Files + lines: this ticket Success criteria, `DESIGN.md` Pi Runtime Adapter, T15b event protocol.
5. Alternatives 2개 + pros/cons + 선택 근거: JSONL sidecar IPC(consistent with ProcessRunner, easier logs) vs named pipe/local socket(richer but more packaging complexity). 선택은 JSONL unless perf/security test proves insufficient.
6. Tests-first: sidecar IPC contract, redaction, approved resource root denial, command correlation tests 를 먼저 실패시킨다.
