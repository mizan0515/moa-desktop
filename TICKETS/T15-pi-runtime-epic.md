# T15 — Pi Runtime EPIC (MoA parent-owned third harness)

GitHub: #36 (https://github.com/mizan0515/moa-desktop/issues/36)

## 배경

Pi 는 "아이디어 참고"가 아니라 MoA Desktop 안에 들어오는 세 번째 harness runtime 이다. 단, MoA 의 핵심 경계는 유지한다: Claude Worker 와 Codex Worker 는 sibling worker 이고, Pi 역시 worker 내부 peer-call 이 아니라 MoA parent orchestrator 가 소유하는 `HarnessRuntime` 이다. Pi package/extension/custom UI/hot reload 는 T13 policy/safety gate 아래에서만 활성화한다.

확인 근거(2026-05-08):
- `@earendil-works/pi-coding-agent` npm 최신: `0.74.0`
- deprecated: `@mariozechner/pi-coding-agent@0.73.1` 는 `@earendil-works/pi-coding-agent` 사용을 안내
- `earendil-works/pi` HEAD: `783e96a14431e9cb33299d8c5e162cc5ad6e7c69`
- Pi RPC docs: `pi --mode rpc` 는 stdin/stdout JSONL headless protocol
- Pi SDK docs: `createAgentSession()`, `DefaultResourceLoader`, `createEventBus`, `ModelRegistry`, `SessionManager`
- Pi packages docs: packages/extensions 는 full system access 위험이 있으므로 source review, pin, hash, capability manifest 필요

## 의존성

- 선행: T13 L1-L5. 특히 `WorkerCommandGuard`, `ReviewRunRecord`, `ResumePacket`, command permission class 가 필요하다.
- T15a -> T15b -> T15c -> T15d/T15e/T15f/T15g.
- T10/T11/T12/T14 는 `runtimeKind: "claude" | "codex" | "pi"` 를 소비하도록 amend 한다.

## Goal

MoA Desktop 이 Claude/Codex/Pi runtime 을 같은 parent orchestrator 아래에서 다룬다. Pi lane 은 초기에는 read-only/research/reviewer/conversational 용도로만 허용하고, mutation owner 승격은 T15g 이후 별도 opt-in setting 으로 미룬다.

## Architecture

```text
MoA Parent Orchestrator
  ├─ ClaudeRuntimeAdapter
  ├─ CodexRuntimeAdapter
  └─ PiRuntimeAdapter
       ├─ PiRpcAdapter: pi --mode rpc JSONL
       └─ PiSdkHost: sidecars/moa-pi-host using @earendil-works/pi-coding-agent
```

Pi extension 은 MoA orchestrator 위에 설 수 없다. tool call, package install, hot reload, custom UI request 는 모두 MoA policy gate 를 지나야 한다.

## Success criteria

- [ ] `DESIGN.md` 에 Pi Runtime / Harness Runtime section 이 있고 parent-owned third runtime 임을 명시한다.
- [ ] `PLAN.md` 에 T15/T15a-T15g/T16 phase 와 dependency graph 가 반영된다.
- [ ] T10/T11/T12/T14 가 Pi runtime 을 소비하는 amend section 을 가진다.
- [ ] 모든 T15 sub-ticket 이 6 항목 의무를 포함한다.
- [ ] Pi lane initial permission 은 read-only/research/reviewer/conversational 로 제한된다.
- [ ] mandatory `CodexAdversarialXHigh` gate 는 Pi review 로 대체되지 않는다.
- [ ] Pi package install/update/hot reload 는 user confirm + source review + pinned version + sha256 + capability manifest + mutation lock check 없이는 금지되고, `autoUpdate=false` 가 기본이다.

## Files owned

- `DESIGN.md` Pi Runtime / Harness Runtime vision section
- `PLAN.md` T15/T16 phase, dependency graph, final ticket list
- `TICKETS/T15*.md`, `TICKETS/T16-harness-marketplace-equipment-profiles.md`

## Read-only

- `src-tauri/src/*`, `src/*`, `package.json`, `Cargo.toml`
- T13 policy/safety implementation until actual T15 implementation tickets begin

## NEVER 영역

- Pi 를 Claude/Codex worker 내부 tool 로 넣지 않는다.
- Pi extension 이 `claude`, `codex exec`, `/codex:*`, Claude/Codex MCP, `TeamCreate`, `Agent` 를 직접 호출하게 하지 않는다.
- `CodexAdversarialXHigh` 를 Pi review, Pi package, Pi extension 으로 대체하지 않는다.
- Pi package 를 자동 설치/업데이트하지 않는다.
- production code 구현은 T15 EPIC 문서 작업에서 하지 않는다.

## Validation cmd

```powershell
git diff -- DESIGN.md PLAN.md PROJECT-RULES.md AGENTS.md TICKETS
rg -n "Pi Runtime|PiRpcAdapter|PiSdkHost|runtimeKind|@earendil-works/pi-coding-agent|CodexAdversarialXHigh" DESIGN.md PLAN.md TICKETS
rg -n "pi install|pi update|/reload|full system access|mutation lock|capability manifest|user confirm|source review|pinned version|sha256|autoUpdate=false" DESIGN.md PLAN.md PROJECT-RULES.md TICKETS
rg -n "worker.*(codex exec|claude -p|/codex:|Claude MCP|Codex MCP)" TICKETS DESIGN.md PLAN.md
```

## Alternatives

1. Pi RPC only
   - Pros: fastest MVP, Tauri/Rust `ProcessRunner` 와 잘 맞고 격리가 쉽다.
   - Cons: extension custom UI/custom renderer/package lifecycle 의 일부가 degraded 되거나 제한된다.
2. Pi SDK Node sidecar only
   - Pros: Pi 의 실제 강점인 extensions/packages/custom UI/session tree 를 처음부터 제어한다.
   - Cons: Node sidecar packaging, signing, IPC, permission boundary 를 초반부터 모두 해결해야 한다.
3. RPC MVP -> SDK sidecar 확장 (선택)
   - Pros: 실제 Pi lane 을 빨리 붙이고, 위험한 package/extension 기능은 T13 gate 이후 확장한다.
   - Cons: adapter 가 2단계로 진화하므로 capability mapping 과 migration test 가 필요하다.

선택: C. MoA 의 safety boundary 를 유지하면서 Pi 를 가장 빨리 실사용 lane 으로 붙일 수 있다.

## Tests-first

- T15a 는 smoke script/report 를 먼저 만든다.
- T15b 는 JSONL framing/correlation/parser tests 를 먼저 실패시킨다.
- T15c 는 sidecar IPC contract tests 를 먼저 실패시킨다.
- T15d/e/f/g 는 capability denial tests 를 먼저 실패시킨다.

## Paste-ready prompt

```text
[세션 부트]
- Prompt kind: Codex Desktop manual lead ticket session
- repo: D:\moa-desktop
- base branch: master
- 권장 분기: codex/T15-pi-runtime-epic
- 작업 종류: 백로그/설계 문서 수정. production code 구현 금지.

[Goal]
Pi 자체를 MoA Desktop parent-owned third harness runtime 으로 편입하는 T15 EPIC 을 문서/티켓에 반영한다.

[Success criteria]
- DESIGN.md/PLAN.md/TICKETS 에 Pi Runtime EPIC 과 sub-ticket graph 반영
- T10/T11/T12/T14 amend 반영
- GitHub issue/project body 로 옮길 title/body 목록 출력

[NEVER]
- production code, package install, Pi install/update, GitHub destructive mutation unless user explicitly asks
- worker nested peer-call 문구 삽입

[Validation]
위 Validation cmd 실행.

[작업 완료 시]
변경 파일, validation 결과, GitHub issue title/body 목록 보고.
```
