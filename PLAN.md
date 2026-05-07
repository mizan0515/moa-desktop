# MoA Desktop — Final Implementation Plan
Date: 2026-05-06
Source: `DESIGN.md` × `analysis-claude.md` × Codex first-pass × adversarial review
Status: Ready for ticket dispatch after critical-fix confirmation

## 0. Critical fixes from adversarial review (must apply before ship)

### F1. `codex exec` 명령 템플릿 — 확정 (2026-05-06 사용자 검증 완료, codex-cli 0.128.0)
- ❌ **불가**: `codex exec --reasoning-effort high ...` — `error: unexpected argument '--reasoning-effort' found`
- ✅ **확정 (read-only first-pass)**: `codex exec --ephemeral -c model_reasoning_effort='high' -c web_search="live" --sandbox read-only --json --cd <repo> <prompt>` (`src-tauri/src/adapters/codex.rs::firstpass_argv` 가 source of truth)
- ✅ **확정 (mutation owner, Windows S2 #5)**: `codex exec --ephemeral -c model_reasoning_effort='high' -c web_search="live" --dangerously-bypass-approvals-and-sandbox --json --cd <worktree> <prompt>` (isolated worktree 안. `--sandbox workspace-write` 는 Windows 에서 broken — `src-tauri/src/adapters/codex.rs::mutation_argv` 가 source of truth)
- ✅ JSON streaming: `--json` 사용 (라인 단위 emit)
- ✅ Web search 사용: 현재 adapter source of truth 는 CLI config `-c web_search="live"` (`src-tauri/src/adapters/codex.rs::firstpass_argv`). 구형 `-c tools.web_search=true` 문구로 되돌리지 않는다.
- 비차단 경고 (T0 에서 기록만, 차단 X): `chatgpt.com` 플러그인/analytics 403, PowerShell shell snapshot 미지원, MCP client `program not found`

### F2. 명령 빌드는 argv array, shell string 금지
- Tauri v2 Command API: `Command.create(program, [args...])` 로 호출. PowerShell quoting 회피.
- `--allowedTools "Bash(git status:*)"` 같은 인자는 한 element 로 전달, 따옴표 내부 escape 신경 X.

### F3. Sandbox 는 3중 (prose 가드만 믿지 X)
- Claude Worker: `--allowedTools` allowlist + `--disallowedTools` deny list (Edit, Write, NotebookEdit 명시 + `mcp__*` wildcard 시도)
- Codex Worker: `-s read-only` (CLI sandbox flag)
- Plugin/MCP env 분리: Worker spawn 시 `ENABLE_CLAUDEAI_MCP_SERVERS=false`, `CODEX_HOME` 별도 prof
- WorkerCommandGuard/SpawnGuard = primary security guard: worker-source peer AI executable/argv/shell execution is blocked before spawn/tool execution.
- Output scanner = blocking second defense for worker stdout/slash/final UI, not warning-only.

### F4. Mutation = isolated worktree/patch flow
- Worker 가 직접 source 수정 X
- 절차: app 이 `git worktree add` 또는 임시 디렉토리에 base 복사 → Worker 가 그 안에서 수정 → app 이 patch 추출 → diff/test 검증 → patch apply (또는 reject + cleanup)
- Rollback 무료, multi-step 추적 가능.

### F5. T0 spike 1주 — exit criteria hard
| # | 검증 항목 | 통과 조건 |
|---|---|---|
| S1 | `claude -p` Tauri spawn + stream-json | stdout JSONL 라인 단위 받음, kill 시 child 즉시 종료, 잔존 0 |
| S2 | `codex exec -s read-only --json --cd <repo>` Tauri spawn | 동일, `-s read-only` 가 mutation 시도 차단 검증 |
| S3 | 두 Worker 병렬 실행 | stdout 충돌 없음, 메모리/파일 race 없음 |
| S4 | Claude `--disallowedTools "mcp__*"` 검증 | Worker 가 `/codex:rescue` 호출 시도 → 차단 확인. 안 되면 plugin env 분리로 fallback |
| S5 | 인증/env 상속 | Tauri 자식이 `~/.claude/credentials.json`, `~/.codex/auth.json` 자동 사용 |
| S6 | TOKEN-GUARD 발화 동작 | spawned `claude -p` 안에서 hook 발화 시 stderr 부모에 전달, HARD 차단 시 명확한 exit code |
| S7 | Cancellation | Tauri abort signal → Windows process tree kill (`taskkill /T /F` 등가) → 0 좀비 |
| S8 | 명령 templates 확정 | argv array 형태로 빌드, settings 에 저장, smoke test 통과 |

### F6. 누락된 운영 항목 추가
- **Recovery journal**: session 별 JSONL journal (phase, owner, PID, base hashes, patch path) — crash 후 startup reconcile
- **Multi-instance / Multi-project**: 단일 앱 인스턴스 (Tauri single-instance plugin) + 최상위 프로젝트 탭 바 (Codex/Claude Desktop 패턴). 한 인스턴스 안 N 프로젝트 동시 활성, 탭 전환으로 컨텍스트 스왑.
  - **같은 repo 중복 차단 = 2-layer lock**:
    1. **In-memory repo-path lock** (앱 안 N 탭 사이) — lock manager 가 path canonicalize (case fold, symlink/junction/UNC 정규화) 후 in-memory map.
    2. **OS-level named mutex / lock file** (프로세스 경계) — Tauri single-instance plugin 이 Win11 24H2 등에서 실패해도 mutation safety 보장. global app-identity mutex 1 개 + per-repo named mutex N 개. stale detection (PID 사망 감지 후 cleanup).
  - **Lock ordering contract** (deadlock 방지): `repo-open canonical lock → project lock → session/lane mutation lock → journal append queue`. lane mutation lock 보유 중 다른 project lock 획득 **금지**. cross-project 작업 (T11 multi-project 시나리오) 은 canonical path 또는 projectId 정렬 기반 2-phase `try_acquire_all`, 실패 시 전부 release + retry/stop. **worker output 은 절대 lock acquisition command source 가 될 수 없음** — scheduler 만 project/lane lock 잡음.
  - **N 인스턴스 모델 정책**: primary architecture 로는 reject (settings/단축키/telemetry fragmentation), 단 **safe-mode escape hatch** 로 `--user-data-dir <path>` flag 보존 — crash isolation/debug/profile 분리가 필요한 사용자가 N 인스턴스로 띄울 수 있음. mutation safety 는 위 OS-level mutex 가 보장.
  - **Single-app crash isolation 흡수책** (lane supervisor): 한 lane panic 이 앱 전체를 죽이지 않도록 T7-full 에 panic boundary + lane supervisor (lane 별 task 격리, panic 감지 후 lane 만 fail, 다른 lane 영향 0).
- **Error 분류**: `cli-missing | auth-expired | quota | network | sandbox-denied | malformed-json | timeout | oom | killed | test-fail` typed errors → UI 에서 사용자 행동 가능한 메시지
- **Retry tracking**: LLM 비결정성 — 재시도는 새 attempt 로 기록 (prompt hash, argv, model, CLI version, cwd, env allowlist, raw output, attempt#)
- **Prompt cache awareness**: 매 `claude -p` 가 fresh session = cache reuse 0. 비용 multiplier 명시. 사용자에 표시.
- **Version pinning**: session metadata 에 CLI/plugin version 기록, drift warning UI
- **Concurrent log writes**: per-worker append-only JSONL + orchestrator single-writer index + monotonic seq ID

## 0.5 Automation Contract — 사용자 한 번 명령 → 끝까지 자동

**원칙**: 사용자는 작업 텍스트 1번 입력 + 마지막 apply confirm 1번 클릭. 그 사이 모든 결정은 orchestrator (T7-full) 가 자동. 관리자 중재는 **2개 trigger 에서만**: (1) 아키텍처 tradeoff 충돌, (2) max 3 round 초과.

**예외 (§ 0.6 T13)**: DestructiveNetwork class slash 명령 (`/메인동기화`, `/백로그`, `/병행통합` 의 PR 생성/머지 step) 은 step-gate confirm 우선. 예외 사유: 외부 시스템 (GitHub, 원격 main) 변경은 단일 apply confirm 로 batch 하기엔 blast radius 큼. step 별 사용자 결정 필요.

### 자동 실행 sequence (Flow C 기준 — 큰 코드 변경)
1. preflight (CLI 검증, auth, version, sandbox) — 자동
2. **first-pass × 2 병렬** (T5a Claude read-only + T5b Codex read-only, 양쪽 web search + deep thinking 활성) — 자동
3. **synthesis** (T3 deterministic merge, no LLM, 5칸 schema) — 자동
4. **adversarial round** (orchestrator 가 Codex Worker 새로 spawn, synthesis embed) — 자동
5. **충돌 해결 protocol** 자동 적용:
   - 사실 충돌 → live verify (T2 가 git/test 호출) — 자동
   - 구현 충돌 → blast radius/rollback/validation 비교 후 자동 결정
   - risk 충돌 → cheapest decisive test 실행 — 자동
   - **아키텍처 tradeoff → 사용자 escalation (이때만 멈춤)**
6. **mutation owner 자동 선택** (mechanical/Windows shell → Codex, semantic refactor → Claude default. orchestrator heuristic + 사용자 override 가능)
7. T4 lock acquire → git worktree 생성 → mutation Worker 실행 (Claude `Edit/Write` 또는 Codex `--dangerously-bypass-approvals-and-sandbox` inside isolated worktree, Windows S2 #5) → patch 추출
8. **Same-file 순차 편집 자동 처리**:
   - 첫 Worker 종료 → file hash snapshot (T4)
   - second Worker 가 review-only 모드로 최신 파일 re-read
   - second Worker review 에 추가 mutation 제안 ≥ 1개 + 동일 파일 영향 → orchestrator 가 lock transfer 자동 결정
   - second Worker 가 mutation owner 로 전환 (T4 lock state: `acquired(claude) → transferring → acquired(codex)`) → 추가 수정
   - max 1회 transfer (overflow 시 사용자 confirm)
9. 두 Worker mutation 합쳐 final patch
10. verification cmd 자동 실행 (settings 의 project-specific cmd, 또는 default `npm test` 등)
11. **final report (Claim Ledger) 사용자에 표시**
12. 사용자 confirm 클릭 → main repo 에 patch apply
13. lock release + journal flush + 세션 left panel 에 archived

### 사용자 개입 지점 (총 2 곳, T13 DestructiveNetwork 명령 예외)
- **시작**: 작업 텍스트 입력 + Run 클릭 (1회)
- **끝**: final report 보고 patch apply 또는 reject (1회)
- **예외**: `/메인동기화` (4-5 step), `/백로그` (3 step), T12 `/병행통합` PR 생성·머지 step — 각 step 사용자 confirm. 단일 apply confirm 와 별개.

### Manager intervention 필요 시 (자동 멈춤 + UI 명시)
- 아키텍처 tradeoff 충돌 (Claude/Codex 가 근본적으로 다른 방향)
- max 3 round 초과 (수렴 실패)
- cost cap 도달 (T9 — default $10/session)
- preflight 실패 (CLI 미설치, auth 만료, sandbox NO-GO)
- multi-instance lock 거부

### "양측 모두 web search · deep thinking · file edit" 보장
- Claude Worker: read-only 모드 — `--allowedTools "Read" "WebSearch" "WebFetch" "Bash(git:*)" "Bash(rg:*)"`. Mutation 모드 — `+ "Edit" "Write"`. Deep thinking — prompt 에 "think hard" 또는 `MAX_THINKING_TOKENS=10000` env.
- Codex Worker: read-only — `--sandbox read-only -c model_reasoning_effort="high" -c web_search="live"`. Mutation — `--dangerously-bypass-approvals-and-sandbox` (isolated worktree 안, Windows S2 #5) + 동일 reasoning + web_search.

## 0.6 v1.5 Prequel — Policy & Lifecycle EPIC (T13, 2026-05-07 사용자 비전 검증 결과)

T10 진입 전 필수 보강. GitHub EPIC: #35 (https://github.com/mizan0515/moa-desktop/issues/35). 사용자 결정:
- **Q1·B**: `settings.primaryRole = "claude" | "codex"`. Codex 선택 시 synthesizer / default reviewer / Flow-C mutation owner 까지 Codex 로 스왑. lock state machine 은 이미 대칭 (`lock/manager.rs:45-47`) — 변경 X.
- **Q2·단계별 confirm**: `/메인동기화` 류 destructive-network 명령은 4-5 step 사용자 확인. **자동화-2-개입 원칙 (§ 0.5)** 의 명시적 예외 — destructive scope 가 큰 명령은 step gate 우선.
- **Q3·앱 backlog SOT**: 앱 내부 backlog 가 source of truth, `~/.claude/projects/<repo>/memory/` 는 단방향 mirror (앱 → 글로벌). 사용자가 다른 프로젝트에서 글로벌 read 가능.
- **추가요구**: 병행티켓 흐름 (T10/T11/T12) 의 PR 생성/머지/통합/main 적용 단계마다 **lead/orchestrator-owned review gate** 를 둔다. 이 gate 는 **항상 `CodexAdversarialXHigh` review profile/prompt** 를 포함한다. MoA Desktop 앱 안에서는 source=orchestrator 의 review profile 이 실행하고, Codex Desktop 수동 개발 흐름에서는 사용자가 명시한 lead PowerShell `codex exec --ephemeral --sandbox read-only ... --output-last-message <repo>/.moa-desktop/reviews/<stamp>.md` 별도 review 가 같은 gate 증거가 될 수 있다. 앱 profile/PowerShell prompt 는 `/codex:adversarial-review --effort xhigh` 와 동등한 의도/강도이며 `reasoning_effort=xhigh`, prompt template version/hash, model/profile id, command/source adapter, source output path 를 `ReviewRunRecord` 에 남긴다. `--dangerously-bypass-approvals-and-sandbox` 는 mutation-in-worktree 전용이고 review gate 성공 증거로 쓰지 않는다. PrimaryRole=Codex 인 경우 Claude review 는 추가 대칭 검토로 붙일 수 있지만 Codex review 를 대체하지 않는다. gate 시점은 `pr_create` 전 local diff, `pr_merge` 전 PR diff, `integrate_merge` 전 통합 diff, `main_apply` 전 최종 diff 로 기록한다. 워커 prompt 에 `/codex:*`, `claude -p`, `codex exec` 직접 호출을 박는 nested peer-call 방식은 금지.
- **글로벌 sync**: Claude-side 글로벌 **15 파일** (Hot 룰 6 = `CLAUDE/RTK/KARPATHY/TOKEN-GUARD/TICKET-CLOSE/CODEX-MCP` + On-demand 스킬 2 = `skills/{codex-mcp-runtime,token-guard-internals}/SKILL.md` + 한국어 단축명령 7) + Codex Desktop overlay `~/.codex/skills/병행티켓/SKILL.md`(존재 시) 변경분은 hash drift detect → 사용자 명시 import. 자동 적용 X. 추가로 `~/.claude/settings.json` 은 raw copy 가 아니라 safe subset 을 `RuntimeProfile` 로 import. PolicyPack 은 `source_manifest[]` + kind discriminator (HotRule / OnDemandSkill / TicketCloseRule / RuntimeHealthCheck / RuntimeSettings / CodexDesktopOverlay) 로 표현 — 글로벌이 또 분리되어도 schema 변경 불요. 글로벌 슬래시/스킬은 executable truth 가 아니라 **policy resolver 입력**이며, 현재 글로벌 명령이 앱 정책과 충돌하면 T13 L4 의 변환/override 규칙이 우선한다. Claude-side `/병행티켓` 과 Codex overlay 가 review gate vocabulary 또는 `command_source_adapter` 에서 충돌하면 fail closed 로 사용자 import/transform confirm 을 요구한다.

EPIC 구조 (단일 ticket T13, 5 phase):
- **L1** PrimaryRole + ExecutionPolicy (orchestrator hardcode 제거)
- **L2** SafetyPolicy + WorkerCommandGuard + Role-aware Output Scanner (DESIGN.md:90-98 코드화)
- **L3** Policy Pack (executable schema, markdown copy 아님)
- **L4** Privileged Slash Command Subsystem (UI dispatcher, 워커 슬래시는 disabled 유지)
- **L5** Resume Packet & Session Lifecycle (`.claude-handoff.md` 대체, T11 multi-lane 토대)

상세: [TICKETS/T13-policy-lifecycle-epic.md](TICKETS/T13-policy-lifecycle-epic.md).

T10/T11/T12 는 본 EPIC 통과 후 Phase 6 원안대로 진행. 단 T10/T11/T12 본문에는 "PR 생성 전/머지 전/통합 전/main 적용 전 Codex adversarial review gate 의무" 를 **lead/orchestrator-owned gate** 로 명시한다. 앱 실행 시에는 orchestrator source, Codex Desktop 수동 개발 시에는 lead PowerShell 별도 review 로 수행할 수 있다. worker prompt 는 review gate 통과 전 PR 생성/merge/main apply 금지만 안내하고, 워커가 peer AI 를 직접 호출하는 문구는 포함하지 않는다.

## 0.7 v1.6 Prequel — Pi Runtime EPIC (T15, 2026-05-08 사용자 비전 반영)

Pi 는 "Pi-inspired layer" 가 아니라 MoA Desktop 안에 들어오는 parent-owned third harness runtime 이다. Claude/Codex worker 내부 tool 이 아니며, worker nested peer-call 금지와 mandatory `CodexAdversarialXHigh` review gate 는 그대로 유지한다.

확인 근거(2026-05-08):
- `@earendil-works/pi-coding-agent` npm latest = `0.74.0`.
- `@mariozechner/pi-coding-agent` 는 deprecated 이며 `@earendil-works/pi-coding-agent` 사용을 안내한다.
- `earendil-works/pi` HEAD = `783e96a14431e9cb33299d8c5e162cc5ad6e7c69`.
- Pi RPC docs 는 `pi --mode rpc` 의 stdin/stdout JSONL headless protocol 을 제공한다.
- Pi SDK docs 는 `createAgentSession`, `DefaultResourceLoader`, `createEventBus`, `ModelRegistry`, `SessionManager` 를 제공한다.
- Pi packages docs 는 packages/extensions 의 full system access 위험을 명시한다.

핵심 결정:
- **T15a/T15b**: RPC MVP 먼저. `PiRpcAdapter` 는 `pi --mode rpc --no-session` 를 strict JSONL 로 붙인다.
- **T15c**: SDK sidecar 확장. `moa-pi-host` Node sidecar 에서 `@earendil-works/pi-coding-agent` SDK 를 직접 구동한다.
- **T15d/e/f/g**: package trust, extension UI, model/session tree, MoA native Pi extensions 를 T13 policy 아래에 넣는다.
- **초기 권한**: Pi lane 은 read-only/research/reviewer/conversational only. mutation owner 승격은 T15g 이후 별도 opt-in.
- **Hot reload**: mutation lock 보유 중 금지, package manifest/hash 재검증 후 다음 turn/lane 부터 적용.
- **Review gate**: Pi review 는 보조 신호일 뿐 mandatory `CodexAdversarialXHigh` 를 대체하지 않는다.

상세: [TICKETS/T15-pi-runtime-epic.md](TICKETS/T15-pi-runtime-epic.md).

## 1. Implementation phases (value-incremental, 5-7주 1인)

### Phase 0 — Spike (T0, 5-7일)
F5 의 S1-S8 검증. 통과 못하면 Phase 1 진입 금지.

### Phase 1 — Walking skeleton (T1, T8, T6-render, T7-thin → 1-2주)
**목표**: 사용자가 더미 데이터로 MoA 흐름 전체를 화면에서 본다.
- T1: Tauri shell + workbench static UI
- T8: Mock mode + canned Claude/Codex JSON responses
- T6: Synthesis view + Claim Ledger UI (mock JSON 으로 렌더)
- T7-thin: dry-run orchestrator (mock workers 호출 → mock synthesis → mock adversarial → mock final report)

**Demo milestone (Phase 1 완료 시점)**: 사용자가 앱 실행 → 프로젝트 선택 → 작업 입력 → mock 으로 Flow C 시뮬레이션 → 5칸 + Claim Ledger 결과 본다. **No real AI call.** 이게 첫 가시적 가치.

### Phase 2 — Real Workers (T2, T5a, T5b → 1.5주)
- T2: Process runner (스트리밍, 취소, 타임아웃)
- T5a: Claude adapter (read-only first-pass + mutation owner 모드)
- T5b: Codex adapter (`-s read-only` first-pass + mutation 모드)
- 통합: Phase 1 의 mock 자리를 실제 Worker 로 교체

**Demo milestone**: 실제 Claude+Codex 가 read-only first-pass 실행, 결과 5칸 표 렌더. **No mutation yet.**

### Phase 3 — Mutation (T3, T4, T7-full → 1-1.5주)
- T3: Synthesis engine (deterministic JSON merge)
- T4: Safety/Git/Lock — worktree-isolated patch flow + recovery journal + multi-instance lock
- T7-full: Orchestrator state machine — mutation owner 선택, lock transfer, max 3 round, conflict protocol

**Demo milestone**: 사용자가 작업 입력 → MoA flow → 한쪽 mutation owner 가 worktree 에 수정 → app 이 patch 검증 → 사용자 confirm → main repo apply. End-to-end happy path.

### Phase 4 — Hardening (T9, error UX, version pinning → 1주)
- T9: Cost telemetry (token, cache_read, $) + cancellation
- Error 분류 + UI 표시
- Version drift warning
- Verification cmd 일반화 (npm/pytest 외 stack)
- `--output-format stream-json` 채택

### Phase 5 — Polish & verify (1주)
- DESIGN.md 의 verification checklist + adversarial F6 추가 항목 모두 통과
- UI wireframe 사용자 confirm
- README, dry-run 데모 영상 또는 GIF

### Phase 6 (v1.5/v1.6 bridge) — Pi-aware Multi-ticket / Multi-project (T15a/T15b → T10/T11/T12 → 2-3주)
**비전 충족 단계**: 글로벌 `/병행티켓` + `/병행통합` 등가 + Codex/Claude Desktop 동등 multi-project.
- T15a: Pi Compatibility Spike — Windows/Tauri/MoA 환경에서 Pi CLI/RPC/SDK/package/hot reload 검증. production code 없음.
- T15b: Pi RPC Runtime Adapter — `pi --mode rpc` 를 `PiRpcAdapter` 로 붙여 `runtimeKind="pi"` read-only/research/reviewer lane MVP 를 만든다.
- T10: Ticket Decomposer — 큰 작업 입력 → 양측 MoA first-pass 로 충돌 없는 N 티켓 + paste-ready prompt + 의존성 그래프 + 머지 순서 emit. UI 에 "Decompose" 버튼 + TicketBoard 컴포넌트.
- T10 amend: output schema 는 `runtimeKind`, `allowedHarnesses`, `piExtensionPolicyRef` 를 포함한다.
- T11: Parallel Session Runner — 티켓 N 개 → worktree pool N → 각 lane 이 독립 T7-full orchestrator instance. UI 에 ParallelLanes (N 개 lane 동시 표시). 사용자가 각 lane "Run" 클릭 (자동 실행 X — 자원 폭주 방지).
  - T11 amend: Pi lane 도 queue/resource budget 대상으로 다루되 initial permission 은 read-only/research/reviewer only.
  - **Resource budget** (필수): global `max_live_workers` (default 4), per-project `max_lanes` (default 2), bounded ring buffer for worker output (default 1MB) + disk spill, hidden tab idle throttling, RSS watchdog. tab close 가 React state + Rust `ProjectHandle/SessionHandle` drop + process abort + journal handle close + lock release 모두 수행 (drop test 필수).
  - **Lock ordering 준수**: § F6 의 contract 따름. cross-project 작업 시 `try_acquire_all` 2-phase, 실패 시 전부 release.
- T12: Merge Integrator — 모든 lane 완료 후 머지 순서대로 patch apply → 충돌 시 stop + 한국어 보고. 성공 시 worktree 정리.
- T12 amend: Pi lane result 도 `ReviewRunRecord`, patch metadata, omitted files, limitations 를 동일하게 검증한다.
- **Multi-project 활성화**: T1 의 single-instance + tab 인프라 + T4 의 OS-level mutex 인프라가 이미 깔려 있다는 가정 하에 본 phase 에서 lock manager 를 repo-path scoped 로 확장. per-project journal/telemetry 격리는 T1/T4/T9 가 Phase 1/3/4 시점부터 project-id 를 키로 받도록 미리 설계 (Phase 6 backtrack 방지).

**Phase 6 진입 전 체크**: T1 가 상단 프로젝트 탭 바 + tabRegistry 패턴을 포함했는가, T4 lock 이 project-id 키로 분리됐는가, T9 telemetry 가 project-scoped 인가. 셋 중 하나라도 누락이면 backtrack 비용 큼 — **T1/T4/T9 ticket 본문을 본 결정에 맞춰 amend 필요** (다음 단계).

### Phase 7 (v1.6) — Pi SDK / Conversational Mode (T15c-T15g, T14, T16)
**비전 확장 단계**: 자동화 mode 외에 Claude Code Desktop + Codex MCP 처럼 사용자가 conversational peer workflow 를 직접 운전하는 mode.
- T15c: Pi SDK Sidecar Host — `moa-pi-host` Node sidecar 에서 SDK 직접 구동.
- T15d: Pi Package Trust & Installer — pinned version, sha256, source review, capability manifest, autoUpdate=false.
- T15e: Pi Extension UI Bridge — `ctx.ui` / extension UI requests 를 React/Tauri UI 로 매핑.
- T15f: Pi Model Switch & Session Tree — Pi model/thinking/fork/clone/compact 를 MoA UI 에 노출하되 ResumePacket 이 source of truth.
- T15g: MoA Native Pi Extensions — tool guard, review gate, ticket context, lane telemetry. 이후에만 Pi mutation owner opt-in 검토.
- T14: `settings.interactionMode = "automated" | "conversational"`.
- peer dispatch 는 UI/orchestrator-mediated 만 허용. worker 가 peer AI 를 직접 호출하는 nested peer-call 은 금지.
- Pi interactive lane 도 conversational 후보에 포함한다. Pi session tree 는 MoA journal/ResumePacket 을 대체하지 않고 mirror 된다.
- thinking/reasoning summary 를 `thinking_chunk` 이벤트로 분리해 expand/collapse 표시.
- 현재 adapter 가 stdin 을 닫는 one-shot 모델이면 same-stream redirect/`<ASK_USER>` 답변 주입을 주장하지 않는다. 우선 spawn-new-turn + ResumePacket state carryover 로 conversational turn 을 구현하고, interactive/resumable adapter 가 생긴 뒤 same-stream redirect 를 확장한다.
- 상세: [TICKETS/T14-conversational-mode.md](TICKETS/T14-conversational-mode.md), GitHub #29.
- T16: Harness Marketplace / Equipment Profiles — workflow 는 유지하고 runtime/model/toolset/extension pack/safety level 만 profile 로 바꾼다.

## 2. Ticket dependency graph (value-incremental)

```
T0 (spike) ── 통과 후 Phase 1 진입
   │
   ├─ T1 (shell/UI)        ┐
   ├─ T8 (mock + responses) │
   ├─ T6 (synthesis view)   ├─ Phase 1 walking skeleton (병렬 가능)
   └─ T7-thin (dry-run)     ┘
        │
        ├─ T2 (process runner)  ┐
        ├─ T5a (Claude adapter) ├─ Phase 2 real Workers (T2 → T5a/T5b 병렬)
        └─ T5b (Codex adapter)  ┘
              │
              ├─ T3 (synthesis engine)  ┐
              ├─ T4 (safety/lock/git)   ├─ Phase 3 mutation (병렬 가능)
              └─ T7-full (orchestrator) ┘
                    │
                    └─ T9 + Phase 4 hardening
                          │
                          └─ Phase 5 verify
                                │
                                ├─ T20-GATE (#20 AppHandle integration test prerequisite for next orchestrator-touching ticket)
                                │     │
                                │     └─ T13 (Policy & Lifecycle EPIC)
                                │            │
                                │            └─ T15a (Pi Compatibility Spike)
                                │                  │
                                │                  └─ T15b (Pi RPC Runtime Adapter)
                                │                        │
                                │                        └─ T10 (Ticket Decomposer, pi-aware)
                                │                              │
                                │                              └─ T11 (Parallel Runner, pi lane budget)
                                │                                    │
                                │                                    └─ T12 (Merge Integrator, pi result records)
                                │
                                └─ T15c (Pi SDK Sidecar Host)
                                      ├─ T15d (Package Trust)
                                      ├─ T15e (Extension UI Bridge)
                                      ├─ T15f (Model Switch & Session Tree)
                                      └─ T15g (MoA Native Pi Extensions)
                                            ├─ T14 (Conversational Mode with Pi)
                                            └─ T16 (Harness Marketplace / Equipment Profiles)
```

## 3. Final ticket list (paste-ready 다음 섹션에서 prompt 화)

| ID | Phase | Owns (no overlap) | Reads only | Deps |
|---|---|---|---|---|
| **T0** | 0 | `spikes/*.md`, `spikes/scripts/*.{js,ps1}` | DESIGN.md | — |
| T1 | 1 | `src-tauri/`, `src/App.tsx`, `src/components/Workbench/*` (단 SynthesisView, ClaimLedger 제외) | DESIGN.md, T0 결과 | T0 |
| T8 | 1 | `mockResponses/*.json`, `src-tauri/src/mock/*` | T0 결과 | T0 |
| T6 | 1 | `src/components/SynthesisView.tsx`, `src/components/ClaimLedger.tsx`, `src/lib/synthesisTypes.ts` | mock JSON | T8 (schema) |
| T7-thin | 1 | `src/lib/orchestrator/dryRun.ts`, `src-tauri/src/orchestrator_dryrun.rs` | T8 mock 인터페이스 | T8 |
| T2 | 2 | `src-tauri/src/process/*`, `src/lib/processEvents.ts` | T0 결과 | T0 |
| T5a | 2 | `src-tauri/src/adapters/claude.rs`, `prompts/workers/claude_*.txt` | T2 | T2 |
| T5b | 2 | `src-tauri/src/adapters/codex.rs`, `prompts/workers/codex_*.txt` | T2 | T2 |
| T3 | 3 | `src/lib/synthesis/merge.ts`, `src/lib/synthesis/__tests__/*` | T6 schema | T6 |
| T4 | 3 | `src-tauri/src/safety/*`, `src-tauri/src/git/worktree.rs`, `src-tauri/src/lock/*`, `src-tauri/src/journal/*` | T2 | — (T2 만) |
| T7-full | 3 | `src-tauri/src/orchestrator/*` (dryRun 제외), `src/lib/orchestrator/stateMachine.ts` | T2, T3, T4, T5a, T5b | T2, T3, T4, T5a, T5b |
| T9 | 4 | `src-tauri/src/telemetry/*`, `src/components/CostMeter.tsx`, `src-tauri/src/cancel/*` | T2 | T2 |
| **T20-GATE** | 5.5 | AppHandle 통합 테스트 보강 (#20) | orchestrator/Tauri test harness | T7-full |
| **T13** | 5.5 | `src-tauri/src/{policy,safety,commands,lifecycle}/*`, settings/policy UI hooks | 글로벌 15 파일 + settings safe subset, T7-full | T20-GATE, T7-full |
| **T15** | 6/7 | `DESIGN.md`, `PLAN.md`, `TICKETS/T15*.md`, Pi runtime architecture | T13 policy/review/lifecycle | T13 |
| **T15a** | 6 | `spikes/T15a-pi-*` | Pi docs/npm metadata, T13 policy vocabulary | T13 |
| **T15b** | 6 | `src-tauri/src/adapters/pi_rpc.rs`, `src-tauri/src/pi/rpc.rs`, `src-tauri/tests/pi_rpc_*.rs` | T2 ProcessRunner, T13 safety/review | T15a |
| **T10** | 6 | `src-tauri/src/decomposer/*`, `prompts/decomposer.txt`, `src/components/TicketBoard.tsx` | T7-full, T5a, T5b, T13 review/policy schema, T15b runtimeKind=pi | T13, T15b |
| **T11** | 6 | `src-tauri/src/parallel/*`, `src-tauri/src/parallel/worktree_pool.rs`, `src/components/ParallelLanes.tsx` | T4 (worktree.rs API), T7-full, T9, T10 schema, T15b Pi lane | T4, T7-full, T9, T10 |
| **T12** | 6 | `src-tauri/src/integrator/*`, `src/components/IntegratePanel.tsx` | T4 (patch apply), T10 merge order, T11 lane results, T13 review gate, Pi lane result records | T11 |
| **T15c** | 7 | `sidecars/moa-pi-host/*`, `src-tauri/src/pi/sidecar.rs`, `docs/pi-sidecar-packaging.md` | T15b, T13 policy | T15b |
| **T15d** | 7 | `src-tauri/src/pi/package_*.rs`, `src/components/PiPackageManager.tsx` | T15c, T13 policy/safety | T15c |
| **T15e** | 7 | `src-tauri/src/pi/extension_ui.rs`, `src/components/PiExtension*.tsx` | T15c, T15d capability manifest | T15c, T15d |
| **T15f** | 7 | `src-tauri/src/pi/session_tree.rs`, `src/components/PiSessionTree.tsx`, `src/components/PiModelSwitcher.tsx` | T15c, T13 ResumePacket | T15c |
| **T15g** | 7 | `sidecars/moa-pi-host/extensions/moa-*/*` | T15d/e/f, T13 review/safety | T15d, T15e, T15f |
| **T14** | 7 | `src-tauri/src/conversation/*`, `src/components/ConversationPanel.tsx`, `src/components/ThinkingBlock.tsx` | T13 commands/lifecycle/scanner, adapters, T15f Pi session tree | T13, T15b or T15c |
| **T16** | 7 | `src-tauri/src/harness_profiles/*`, `src/components/HarnessMarketplace.tsx`, `src/components/EquipmentProfilePicker.tsx` | T13 settings/policy, T15 package/session APIs | T15g |

**병렬 가능한 첫 스프린트** (T0 통과 후): T1, T8, T2 (3 worker 동시 가능). T6, T7-thin 는 T8 schema 합의 후.

**Phase 6 ticket 들 사이 병렬 가능성**: T15a/T15b 를 T10 전에 두는 것이 기본. 이후 T10 단독 → T11 단독 → T12 (T11 의존). T15c 이후 T15d/T15e/T15f 는 병렬 가능하되 T15d package trust 와 T13 policy/safety 충돌을 피한다. T15g/T16 은 뒤에 둔다.

## 4. T1/T4/T9 amend 필요 사항 (Phase 6 backtrack 방지)
v1 ticket 본문에 multi-project 인프라 hook 미리 박아둔다:

- **T1**: 상단 프로젝트 탭 바 컴포넌트 + `tabRegistry` 패턴 (T10/T11/T12 가 App.tsx 수정 없이 탭 등록 가능). 첫 v1 출시에는 탭 1개 (현재 프로젝트) 만 활성, multi 는 v1.5 에서 enable.
- **T4**: `LockManager` API 가 `(projectId, lockKey)` 키로 받음. v1 에서는 projectId 가 항상 단일이지만 인터페이스만 미리. `journal` 도 per-project subdir.
- **T9**: `Telemetry` 가 project-scoped aggregation. CostMeter UI 가 현재 탭 프로젝트의 cost 만 표시 (전역 합산 별도 탭).

위 amend 는 v1 출시 일정에 영향 0 (모두 인터페이스 디자인 디테일, 구현 추가 코드 한 자리수 줄). T1 는 현재 in-flight (`feat/T1-scaffold`) 이므로 본 결정 즉시 T1 ticket 본문에 반영 필요.
