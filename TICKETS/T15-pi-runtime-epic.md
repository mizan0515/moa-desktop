# T15 — Pi Runtime EPIC (MoA parent-owned third harness)

GitHub: #36 (https://github.com/mizan0515/moa-desktop/issues/36)

## Goal

Pi 자체를 MoA Desktop parent-owned third `HarnessRuntime` 으로 편입하는 백로그/설계 문서를 정리한다. 본 ticket 은 docs/backlog only 이며 production code 를 수정하지 않는다.

## Decision

Pi 는 Claude/Codex worker 내부 tool 이 아니고, MoA parent orchestrator 가 소유하는 sibling harness runtime 이다.

```text
MoA Parent Orchestrator
  ├─ ClaudeRuntimeAdapter
  ├─ CodexRuntimeAdapter
  └─ PiRuntimeAdapter
       ├─ PiRpcAdapter: pi --mode rpc --no-session JSONL
       └─ PiSdkHost: sidecars/moa-pi-host using @earendil-works/pi-coding-agent
```

선택지는 RPC only, SDK only, RPC MVP -> SDK sidecar 였고, 선택은 RPC MVP -> SDK sidecar 다. 빠른 lane integration 을 먼저 얻고 packages/extensions/custom UI/session tree 는 T13 policy boundary 아래에서 확장한다.

## Verified metadata

설치/업데이트 없이 2026-05-08 확인:
- `@earendil-works/pi-coding-agent` npm latest: `0.74.0`.
- `@mariozechner/pi-coding-agent` is deprecated and points to `@earendil-works/pi-coding-agent`.
- `earendil-works/pi` remote HEAD: `3421726e8629d4cd344e75b94bdf9d7412dfddca`.

## Success criteria

- [ ] `DESIGN.md` 에 Pi Runtime / Harness Runtime section 이 있고 parent-owned third runtime 임을 명시한다.
- [ ] `PLAN.md` 에 T15/T15a-T15g/T15INTEGRATE/T16 phase 와 dependency graph 가 반영된다.
- [ ] `PROJECT-RULES.md`/`AGENTS.md` 에 Pi invariant/latest status 가 반영된다.
- [ ] T10/T11/T12/T14 가 Pi runtime 을 소비하는 amend section 을 가진다.
- [ ] T15/T15a-T15g/T15INTEGRATE/T16 ticket files 가 존재한다.
- [ ] 모든 T15 sub-ticket 이 6 항목 의무를 포함한다.
- [ ] Pi lane initial permission 은 read-only/research/reviewer/conversational 로 제한된다.
- [ ] mandatory `CodexAdversarialXHigh` gate 는 Pi review 로 대체되지 않는다.
- [ ] Pi package install/update/hot reload 는 user confirm + pinned source + sha256 + capability manifest + mutation lock check 없이는 금지된다.
- [ ] production code 수정 0.

## Files owned

- `DESIGN.md`
- `PLAN.md`
- `PROJECT-RULES.md`
- `AGENTS.md`
- `TICKETS/T10-ticket-decomposer.md`
- `TICKETS/T11-parallel-runner.md`
- `TICKETS/T12-merge-integrator.md`
- `TICKETS/T14-conversational-mode.md`
- `TICKETS/T15*.md`
- `TICKETS/T16-harness-marketplace-equipment-profiles.md`

## NEVER 영역

- `src-tauri/src/*`
- `src/*`
- `package.json`, `Cargo.toml`, lockfiles
- Pi package install/update
- Pi 를 worker nested peer-call 로 설명
- `CodexAdversarialXHigh` 를 Pi review, Pi package, Pi extension 으로 대체

## Validation cmd

```powershell
git diff -- DESIGN.md PLAN.md PROJECT-RULES.md AGENTS.md TICKETS
rg -n "Pi Runtime|PiRpcAdapter|PiSdkHost|runtimeKind|@earendil-works/pi-coding-agent|CodexAdversarialXHigh" DESIGN.md PLAN.md TICKETS
rg -n "pi install|pi update|/reload|full system access|mutation lock|capability manifest" DESIGN.md PLAN.md TICKETS
rg -n "worker.*(codex exec|claude -p|/codex:|Claude MCP|Codex MCP)" TICKETS DESIGN.md PLAN.md
```

## Worker prompt 6 mandatory fields

1. Success criteria: 위 Success criteria 전체, 특히 docs/tickets only 와 production code 0.
2. NEVER 영역: production code, manifests/lockfiles, Pi install/update, nested peer-call, mandatory review gate relaxation.
3. Validation cmd: 위 Validation cmd.
4. Files + lines: `DESIGN.md` Harness Runtime/Pi section, `PLAN.md` § 0.7/Phase graph, T10/T11/T12/T14 amend sections.
5. Alternatives 2개 + pros/cons + 선택 근거: RPC only(빠르지만 extension degraded), SDK only(기능 완전하지만 packaging/safety blast radius 큼), RPC MVP -> SDK sidecar(선택).
6. Tests-first: docs ticket 이므로 failing code test 대신 validation rg/diff 를 먼저 정의하고 생산 코드 diff 0 을 검증한다.

## 작업 완료 시

1. commit: `docs(T15): add Pi runtime backlog and ticket graph` (본문에 `Closes #36` 포함)
2. push + PR 생성.
3. lead/orchestrator-owned `CodexAdversarialXHigh` docs review gate.
4. GitHub 카드 #36 complete.
