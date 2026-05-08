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

- `src-tauri/src/process/*`, `src-tauri/src/policy/*`, `src-tauri/src/safety/*`
- `src-tauri/tauri.conf.json` until allowlist implementation is explicit in this ticket

## NEVER 영역

- Third-party Pi package auto install 금지.
- Project-local `.pi/settings.json` auto load/install 금지.
- Sidecar 가 Claude/Codex executable 을 직접 호출하게 하지 않는다.
- Pi SDK host 가 MoA journal/ResumePacket 을 대체하지 않는다.

## Validation cmd

```powershell
npm test --workspace sidecars/moa-pi-host
cargo test --manifest-path src-tauri\Cargo.toml pi_sidecar
rg -n "@earendil-works/pi-coding-agent|createAgentSession|DefaultResourceLoader|createEventBus|ModelRegistry|SessionManager|moa-pi-host" sidecars src-tauri docs
```

## Alternatives

1. Keep Rust-only RPC path
   - Pros: no Node packaging.
   - Cons: custom UI/package/session APIs remain constrained.
2. Node sidecar SDK host (선택)
   - Pros: official SDK surface, full extension/custom UI control.
   - Cons: packaging/signing/versioning overhead.
3. Frontend direct SDK
   - Pros: UI integration easy.
   - Cons: browser/Tauri frontend should not own agent runtime or filesystem/tool permissions.

## Tests-first

Define IPC contract tests with a fake SDK host before real SDK wiring: command correlation, abort, model switch, extension reload denied while mutation lock is active.

## Paste-ready prompt

```text
[세션 부트]
- Prompt kind: Codex Desktop manual lead ticket session
- repo: D:\moa-desktop
- branch: codex/T15c-pi-sdk-sidecar-host
- worktree required

[Goal]
`moa-pi-host` Node sidecar 를 추가해 Pi SDK 기반 runtime host 를 만든다.

[NEVER]
package auto install, project-local package trust, peer AI command, frontend-owned SDK runtime 금지.

[Validation]
npm test --workspace sidecars/moa-pi-host
cargo test --manifest-path src-tauri\Cargo.toml pi_sidecar

[작업 완료 시]
IPC protocol, packaging limitations, T15d/e/f handoff 를 보고한다.
```
