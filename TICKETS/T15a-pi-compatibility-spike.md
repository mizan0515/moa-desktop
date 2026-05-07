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
- [ ] `git ls-remote https://github.com/earendil-works/pi HEAD` 로 source HEAD 확인.
- [ ] `pi --version` 실행 가능 여부 확인. 미설치면 install 하지 않고 `cli-missing` 으로 기록.
- [ ] `pi --mode rpc --no-session` JSONL smoke 계획 또는 실행 결과 기록.
- [ ] RPC command smoke: `prompt`, `set_model`, `compact`, `abort`, `get_state` 가능성 확인.
- [ ] extension UI request smoke: `confirm`, `input`, `select`, `notify`, `setStatus`, `setWidget` mapping 가능성 확인.
- [ ] SDK smoke plan: `createAgentSession`, `DefaultResourceLoader`, `createEventBus`, `ModelRegistry`, `SessionManager` imports 확인.
- [ ] package/hot reload risk report: full system access, auto-update, project-local `.pi/settings.json` auto install 금지 필요성 기록.
- [ ] `spikes/pi-compatibility.md` 에 PASS/FAIL/UNVERIFIED 와 follow-up ticket mapping 기록.

## Files owned

- `spikes/pi-compatibility.md`
- `spikes/scripts/pi_*.ps1`

## Read-only

- DESIGN.md, PLAN.md, PROJECT-RULES.md, AGENTS.md
- T13 policy/safety docs

## NEVER 영역

- Pi package install/update
- production code 수정
- `package.json`, lockfiles 수정
- worker nested peer-call 추가
- 미설치 Pi 를 설치해서 smoke 를 진행

## Worker prompt 6 mandatory fields

1. Success criteria: metadata, CLI/RPC/SDK/UI/package/hot reload compatibility report.
2. NEVER 영역: install/update, production code, package manifests, nested peer-call.
3. Validation cmd:
   ```powershell
   npm view @earendil-works/pi-coding-agent version
   npm view @mariozechner/pi-coding-agent deprecated
   git ls-remote https://github.com/earendil-works/pi HEAD
   ```
4. Files + lines: `TICKETS/T15a-pi-compatibility-spike.md`, `DESIGN.md` Pi Runtime Adapter section, `PLAN.md` § 0.7.
5. Alternatives 2개 + pros/cons + 선택 근거: install smoke(정확하지만 forbidden) vs metadata/no-install smoke(제한적이지만 safe). 선택은 no-install first, local Pi 가 이미 있으면 read-only CLI smoke.
6. Tests-first: report template 과 expected findings matrix 를 먼저 만들고 command output 을 채운다.
