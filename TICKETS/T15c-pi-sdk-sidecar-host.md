# T15c — Pi SDK Sidecar Host

GitHub: #39 (https://github.com/mizan0515/moa-desktop/issues/39)

## 새 Claude 창 만들기 가이드
T15b + T13 통과 후. worktree: T15c-sidecar.

---

```
[세션 부트]
- repo: D:\moa-desktop
- base branch: master (T15b + T13 머지 후)
- 권장 분기: feat/T15c-pi-sdk-sidecar-host
- 권위: PROJECT-RULES.md, AGENTS.md, DESIGN.md, TICKETS/T15b-pi-rpc-runtime-adapter.md, TICKETS/T13-policy-lifecycle-epic.md
- 운영: MoA Flow C — § 2.6 템플릿 A

[의존성 self-check — claim 직후, first-pass 시작 전 무조건 실행]
master 에 선행 commit 2개 있는지 확인:
```
cd D:\moa-desktop && git log master --oneline -100 | rg -i "feat\(T15b\)|feat\(T13\)" | wc -l
```
- 결과 `2` 면 OK — 작업 진행
- 2 미만이면 **STOP — "선행 티켓이 master 에 미머지" 사용자 보고**.

[INDEPENDENT FIRST-PASS — read-only]

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

## Worker prompt 6 mandatory fields
1. Success criteria: `moa-pi-host` sidecar skeleton, `@earendil-works/pi-coding-agent` SDK host, Rust bridge `sidecar.rs`, IPC protocol (start_session/prompt/abort/set_model/compact/fork/reload_extensions), `DefaultResourceLoader` approved roots only, secret redaction, packaging docs 를 구현한다.
2. NEVER 영역: third-party Pi package auto install, project-local `.pi/settings.json` auto load, sidecar→Claude/Codex 직접 호출, Pi SDK host 가 MoA journal/ResumePacket 대체, worker 직접 peer 호출 패턴.
3. Validation cmd:
   ```powershell
   npm test --workspace sidecars/moa-pi-host
   cargo test --manifest-path src-tauri\Cargo.toml pi_sidecar
   rg -n "@earendil-works/pi-coding-agent|createAgentSession|DefaultResourceLoader|createEventBus|ModelRegistry|SessionManager|moa-pi-host" sidecars src-tauri docs
   ```
4. Files + lines: `TICKETS/T15c-pi-sdk-sidecar-host.md` 의 Success criteria/NEVER, `TICKETS/T15b-pi-rpc-runtime-adapter.md` 의 RPC protocol, `DESIGN.md` 의 Pi runtime section.
5. Alternatives 2개 + pros/cons + 선택 근거: Rust-only RPC path(Node packaging 없지만 custom UI/package/session 제한) vs Node sidecar SDK host(official SDK, full extension/custom UI 가능하지만 packaging 복잡). 선택은 Node sidecar SDK host.
6. Tests-first: IPC contract tests with fake SDK host — command correlation, abort, model switch, extension reload denied while mutation lock active 를 먼저 실패시키고 구현한다.

## Worker prompt cross-contract fields
- GitHub/project handling: GitHub #39 / Project `MoA Desktop` card status 를 claim/complete 단계에서 갱신한다.
- Conflict matrix ownership: T15c owns 는 `sidecars/moa-pi-host/*`, `src-tauri/src/pi/sidecar.rs`, `src-tauri/tests/pi_sidecar_*.rs`, `docs/pi-sidecar-packaging.md` 로 한정한다. T13 policy/safety, T15b RPC adapter 는 read-only.
- Dependency/merge order: T15b + T13 완료 후 시작. T15d/T15e/T15f 는 T15c 이후.
- Review gate warning: lead/orchestrator-owned `CodexAdversarialXHigh` 가 `Clean` 을 반환하고 `source_output_path` 가 persisted 되기 전에는 `pr_create`, `pr_merge`, `integrate_merge`, `main_apply` 진행 금지. worker 는 직접 peer review 를 실행하지 않는다.

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

[작업 완료 시 — 무조건 이 순서로]
1. commit: `feat(T15c): Pi SDK sidecar host + IPC protocol + packaging docs` (본문에 `Closes #39` 포함, push 금지)
2. **GitHub 카드 완료 처리 — 잊지 말고 무조건 실행**:
   ```
   node ~/.claude/scripts/gh-tickets.mjs complete D:\moa-desktop 39
   ```
   - 출력에 `COMPLETED=39` 또는 `ALREADY_CLOSED=39` 가 보여야 OK.
   - 실패 시 사용자 보고 + STOP.
3. 보고: IPC protocol, packaging limitations, T15d/e/f handoff, **GitHub 카드 close 결과 1줄**.
