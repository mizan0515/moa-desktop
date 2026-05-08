# T15a — Pi Compatibility Spike

GitHub: #37 (https://github.com/mizan0515/moa-desktop/issues/37)

## Goal

Windows/Tauri/MoA 환경에서 Pi CLI/RPC/SDK/package/extension/hot reload 기능이 MoA safety boundary 아래에서 사용 가능한지 검증한다. 산출물은 spike report 이며 production code 는 만들지 않는다.

## 의존성

- 선행: T13 L2/L3 최소 통과. command guard 와 package/source policy vocabulary 가 필요하다.
- 후속: T15b `PiRpcAdapter`, T15c `PiSdkHost`.

## Success criteria

- [ ] `npm view @earendil-works/pi-coding-agent version` 으로 latest metadata 확인.
- [ ] `npm view @mariozechner/pi-coding-agent deprecated` 로 deprecated migration 확인.
- [ ] `pi --version` 실행 가능 여부 확인. 미설치면 install 하지 않고 `cli-missing` 으로 기록.
- [ ] `pi --mode rpc --no-session` JSONL smoke 계획 또는 실행 결과 기록.
- [ ] RPC command smoke: `prompt`, `set_model`, `compact`, `abort`, `get_state` 가능성 확인.
- [ ] extension UI request smoke: `confirm`, `input`, `select`, `notify`, `setStatus`, `setWidget` mapping 가능성 확인.
- [ ] SDK smoke plan: `createAgentSession`, `DefaultResourceLoader`, `createEventBus`, `ModelRegistry`, `SessionManager` imports 확인.
- [ ] package/hot reload risk report: full system access, auto-update, project-local `.pi/settings.json` auto install 금지 필요성 기록.
- [ ] `spikes/T15a-pi-compatibility.md` 에 PASS/FAIL/UNVERIFIED matrix 작성.

## Files owned

- `spikes/T15a-pi-compatibility.md`
- `spikes/T15a-pi-rpc-smoke.ps1`
- `spikes/T15a-pi-sdk-smoke.mjs`

## Read-only

- `DESIGN.md`, `PLAN.md`, `PROJECT-RULES.md`, `AGENTS.md`, `TICKETS/T15*.md`
- `src-tauri/src/process/*`, `src-tauri/src/safety/*`, `src-tauri/src/policy/*`

## NEVER 영역

- Pi package 실제 설치 금지: `pi install`, `pi update`, `npm install -g` 금지.
- production code 수정 금지.
- `.pi/settings.json` 를 신뢰하거나 자동 적용하지 않는다.
- Pi 를 mutation owner 로 실행하지 않는다.
- worker prompt 안에 peer AI 직접 호출을 넣지 않는다.

## Validation cmd

```powershell
npm view @earendil-works/pi-coding-agent version name --json
npm view @mariozechner/pi-coding-agent version name deprecated --json
pi --version
pi --mode rpc --no-session
rg -n "PASS|FAIL|UNVERIFIED|@earendil-works/pi-coding-agent|full system access|hot reload" spikes/T15a-pi-compatibility.md
```

## Alternatives

1. CLI/RPC only spike
   - Pros: fastest, no sidecar dependency.
   - Cons: SDK/package/custom UI risks remain speculative.
2. SDK/package spike first
   - Pros: validates Pi's strongest extension surface early.
   - Cons: heavier setup and higher security risk.
3. Metadata + RPC smoke + SDK import-only smoke (선택)
   - Pros: enough evidence for T15b/T15c split without installing third-party packages.
   - Cons: package behavior remains partly UNVERIFIED until T15d.

## Tests-first

Create the spike matrix first with expected PASS/FAIL/UNVERIFIED rows, then run only read-only metadata/RPC/import smoke checks to fill it.

## Paste-ready prompt

```text
[세션 부트]
- Prompt kind: Codex Desktop manual lead ticket session
- repo: D:\moa-desktop
- branch: codex/T15a-pi-compatibility-spike
- worktree required

[Goal]
Pi CLI/RPC/SDK/package/hot reload compatibility 를 read-only spike 로 검증한다.

[NEVER]
production code, package install/update, global settings mutation, GitHub mutation 금지.

[Validation]
Validation cmd 를 실행하고 `spikes/T15a-pi-compatibility.md` 에 결과를 기록한다.

[작업 완료 시]
PASS/FAIL/UNVERIFIED matrix, T15b/T15c scope adjustment, unresolved risks 를 보고한다.
```
