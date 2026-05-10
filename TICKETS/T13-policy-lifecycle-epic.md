# T13 — Policy & Session Lifecycle EPIC (v1.5 prequel, T10 진입 전 필수)

## 새 Claude 창 만들기 가이드
T7-full 머지 + #20 AppHandle integration test 보강 후. worktree: T13-policy-lifecycle.

---

````
[세션 부트]
- repo: D:\moa-desktop
- base branch: master (T7-full 머지 후)
- 권장 분기: feat/T13-policy-lifecycle-epic
- 권위: PROJECT-RULES.md, AGENTS.md, DESIGN.md, PLAN.md § 0.6
- 운영: MoA Flow C — § 2.6 템플릿 A

[의존성 self-check — claim 직후, first-pass 시작 전 무조건 실행]
master 에 T7-full commit 있는지 확인:
```
cd D:\moa-desktop && git log master --oneline -100 | rg -i "feat\(T7-full\)" | wc -l
```
- 결과 `1` 이상이면 OK — 작업 진행
- 0 이면 **STOP — "T7-full 이 master 에 미머지" 사용자 보고**.
- #20 AppHandle integration test 보강 여부도 확인:
```
cd D:\moa-desktop && gh issue view 20 --repo mizan0515/moa-desktop --json state -q .state
```
- `CLOSED` 면 OK. `OPEN` 이면 **경고** — T13 은 orchestrator/safety 경계를 건드리므로 #20 보강이 선행되어야 한다.

[INDEPENDENT FIRST-PASS — read-only]
````

## 배경 (2026-05-07 사용자 비전 검증 결과, 2026-05-07 글로벌 분리 반영)

글로벌 `~/.claude/{CLAUDE.md, RTK.md, TOKEN-GUARD.md, KARPATHY.md, TICKET-CLOSE.md, CODEX-MCP.md}` (Hot 룰 6) + on-demand 스킬 `skills/{codex-mcp-runtime,token-guard-internals}/SKILL.md` (2) + 한국어 슬래시 7 개 (`commands/{다음세션,쉽게,진행}.md` + `skills/{메인동기화,백로그,병행통합,병행티켓}/SKILL.md`) 의 운영체계를 본 앱의 **표준 session lifecycle** 로 승격. **총 15 파일**. 사용자 결정:
- **Q1 → B**: PrimaryRole=Codex 시 **mutation owner 까지** Codex 로 스왑.
- **Q2 → 단계별 confirm**: `/메인동기화` 류 destructive 명령은 4-5 step 별 사용자 확인 (자동화-2-개입 원칙 의 명시적 예외).
- **Q3 → 추천대로**: 앱 backlog = source of truth, `~/.claude/projects/<repo>/memory/` 는 mirror (앱 → 글로벌 단방향 sync).

## 의존성

GitHub: #35 (https://github.com/mizan0515/moa-desktop/issues/35)

선행: T7-full 머지 완료 (`bee18d9`) + #20 AppHandle integration test 보강. 본 ticket 은 orchestrator/safety 경계를 건드리므로 #20 을 또 미루면 안 된다. 본 ticket 통과 후에야 T10 진입.

## 진입 전 최신 코드 위치 preflight

본 ticket 의 line reference 는 T7-full 이후 fix PR (#30-#33) 로 drift 가능성이 있다. L1 착수 직전 반드시 최신 위치를 재캡처한다:

```powershell
rg -n "default_reviewer|mutation_owner|Flow::B|Lane::Claude|Lane::Codex|primaryRole" src-tauri/src/orchestrator src/lib/synthesis src-tauri/src/settings
rg -n "workspace-write|dangerously-bypass-approvals-and-sandbox|bypass-in-worktree" PLAN.md DESIGN.md AGENTS.md TICKETS src-tauri/src/adapters
gh issue view 35 --repo mizan0515/moa-desktop --json number,title,state,url
```

현재 관측 기준(2026-05-07): `default_reviewer(Lane::Claude)` 는 `src-tauri/src/orchestrator/mod.rs` 의 최신 위치를 `rg` 로 다시 찾아야 한다. stale line 번호만 믿고 수정 금지.

## 5 Phase 구조 (단일 ticket, phase 별 commit)

### L1 — PrimaryRole + ExecutionPolicy (선행, 다른 phase 의 토대)

**목표**: orchestrator/synthesis/adversarial/Flow-C-mutation-owner 의 Claude-main hardcode 제거. `settings.primaryRole: "claude" | "codex"` 도입.

**파일**:
- `src-tauri/src/policy/mod.rs` (신규) — `PrimaryRole` enum, `ExecutionPolicy { synthesizer, default_reviewer, mutation_owner_default(flow), label }`.
- `src-tauri/src/orchestrator/mod.rs` amend — `default_reviewer(Lane::Claude)` 류 hardcode → `policy.default_reviewer()`.
- `src-tauri/src/orchestrator/mod.rs` amend — `Flow::B → Codex, _ → Claude` 류 mutation-owner hardcode → `policy.mutation_owner_default(flow)`.
- `src-tauri/src/orchestrator/adversarial.rs` amend — synthesizer 가정 제거.
- `src/lib/synthesis/merge.ts` amend — Claude-first 가정 제거.
- `src-tauri/src/settings/` (신규 또는 stub 채움) — `primaryRole` 필드 + load/save.
- UI: 설정 화면에 PrimaryRole 토글 (Claude / Codex). 실시간 반영 안 함 — 다음 세션부터.

**Success criteria**:
- [ ] PrimaryRole 토글 후 Flow C 시작 → synthesizer/reviewer/mutation owner 모두 정확히 스왑.
- [ ] 기존 e2e (T7-full dry-run) 양 role 모두 통과.
- [ ] lock state machine 무영향 (`src-tauri/src/lock/manager.rs` state machine 그대로, preflight `rg` 로 정확한 줄 재확인).
- [ ] PrimaryRole 변경 시 진행중 세션 영향 X (다음 세션부터 적용 — UI 명시).
- [ ] 단위 테스트: `policy::tests` — Flow A/B/C/D × 양 PrimaryRole = 8 케이스 mutation_owner / synthesizer / reviewer 검증.

### L2 — SafetyPolicy + WorkerCommandGuard + Output Scanner (role-aware)

**목표**: `DESIGN.md` 의 "자동 중단 조건" block list 코드화 (현재 85-91줄 부근, preflight `rg` 로 재확인). **role-aware**: PrimaryRole 받아 정상 orchestration 문구 vs 워커 peer-call 위반 구분. 출력 스캐너는 2차 방어이며, worker context 에서는 process spawn/tool command 실행 **전** peer-recursion 을 차단한다.

**파일**:
- `src-tauri/src/safety/scanner.rs` (신규) — block list regex + role context input + `ScanResult { Clean | Violation { kind, evidence, role_context } }`.
- `src-tauri/src/safety/command_guard.rs` (신규) — `WorkerCommandGuard` / `SpawnGuard` 로 worker source 의 executable path, argv, shell command string 을 process spawn 또는 tool execution 전에 검사. Codex mutation 이 `--dangerously-bypass-approvals-and-sandbox` 를 쓰더라도 이 guard 를 통과하지 못하면 실행하지 않는다.
- `src-tauri/src/safety/mod.rs` amend — `pub mod scanner; pub use scanner::*;`.
- 통과 지점 (모두 같은 scanner 사용):
  1. Worker stdout (claude.rs / codex.rs streaming chunk).
  2. Slash command output (L4).
  3. Integrator / gh helper output (T12 진입 전 hook).
  4. UI 가 표시 직전 (final filter).

**Block list (DESIGN.md "자동 중단 조건" block, preflight rg 로 재확인 + adversarial 보강)**:
- 양방향 peer-call 패턴: `/codex:`, `claude -p`, `codex exec`, `Claude MCP`, `Codex MCP`, `claude_code_peer`, `TeamCreate`, `Agent`, `call Codex`, `call Claude`, `ask another AI`, `run another agent`.
- Role-aware 예외: orchestrator 자체가 워커 spawn 하는 정상 메시지 ("Spawning Claude worker for first-pass…") 는 source=orchestrator 일 때만 통과. source=worker 일 때는 차단.
- Pre-execution 예외 없음: source=worker 이면 `claude`, `codex`, `/codex:*`, Claude/Codex MCP peer, `TeamCreate`, `Agent` 계열 executable/argv/shell string 은 실행 전 차단. source=orchestrator 의 정상 worker spawn 과 review gate spawn 만 command capability allowlist 로 통과.

**Success criteria**:
- [ ] 워커 stdout 에 `/codex:rescue` 흔적 → 즉시 차단 + 사용자 alert.
- [ ] orchestrator 의 "claude worker spawning" 정상 메시지 → 통과.
- [ ] integrator stdout 에 `git push` 워커가 시도하면 → 차단 (워커는 orchestrator 권한 없음).
- [ ] PrimaryRole=Codex 시 Claude worker 의 `codex exec` 시도 차단 (방향 무관).
- [ ] worker source 에서 `claude`, `codex exec`, `/codex:*`, Claude/Codex MCP, `TeamCreate`, `Agent` 실행 시도는 spawn/tool execution 전 차단된다. Codex mutation bypass 모드에서도 동일.
- [ ] Codex mutation bypass 는 adapter/orchestrator 양쪽에서 repo-local literal `.moa-desktop/worktrees/<session-id>` shape 를 검증한다. main repo path, `.moa-desktop/worktrees` root 자체, stale/임의 path 는 spawn 전 reject 하고 `PermissionDenied`/safety violation 으로 기록한다. `.moa-desktop/` 는 git ignore 대상이다. Windows 에서는 junction/reparse-point 로 `.moa-desktop` 또는 `.moa-desktop/worktrees` 가 repo 밖을 가리키는 escape case 와 repo 안 alias (`.moa-desktop/worktrees -> <repo>/src/.worktrees`) 를 모두 전용 테스트로 검증한다.
- [ ] 단위 테스트: 12 블록패턴 × 4 source × 2 PrimaryRole = 96 케이스 + command_guard pre-execution 20 케이스 + Codex bypass worktree guard 의 Windows junction/reparse escape 케이스.

### L2.5 — ReviewVerdict schema + ReviewInputStrategy (review state machine 입력 통일)

**목표**: T10/T11/T12 가 흩어 쓰던 verdict 용어 (clean/concern/block/fail) 를 단일 enum 으로 통일. 대형 diff (PR 100+ 파일) 시 Codex/Claude reviewer context overflow 방지. PR/merge gate 는 항상 `CodexAdversarialXHigh` review profile 을 포함하며, PrimaryRole=Codex 에서 Claude review 를 추가하더라도 Codex review 를 대체하지 않는다.

**파일**:
- `src-tauri/src/policy/review.rs` (신규) — `ReviewVerdict { Clean, Concern, Block, ReviewRunError }`. 실행 자체 실패도 gate verdict 로 저장해 fail-closed 판단과 audit persistence 가 같은 필드를 사용한다.
- `ReviewProfile`: `{ id: CodexAdversarialXHigh | ClaudeSymmetry, reviewer: Lane, reasoning_effort, model_or_profile_id, prompt_template_version, prompt_hash, command_source_adapter, output_capture_required }`. `command_source_adapter` 는 gate 증거로 인정되는 `moa-orchestrator`, `codex-desktop-lead-powershell`, `codex-desktop-lead-powershell-controlled-bypass` 만 허용한다. lead PowerShell review 는 먼저 `--sandbox read-only` 와 `.moa-desktop/reviews/<stamp>.md` `--output-last-message` capture 를 사용한다. WindowsApps `pwsh.exe -Command ... rejected: blocked by policy` 로 formal read-only review 가 `ENV_BLOCKED` 를 내면 그 attempt 는 `ReviewRunError` 로 저장하고, Codex Desktop lead/manual session 은 `--dangerously-bypass-approvals-and-sandbox` 를 controlled-bypass review gate 로 1회 재시도할 수 있다. 이 fallback 은 READ-ONLY prompt, edit/create/delete/stage/commit/push/format/GitHub mutation 금지, before/after `git status`, review-caused mutation 0건, `Verdict: Clean`, failed read-only attempt path 기록이 모두 충족될 때만 gate 증거다. current-session review 는 advisory/non-gate 이며 mandatory gate 를 대체하지 않는다.
  - Controlled-bypass selector 는 loose regex 금지. output file 존재, 정확히 1개인 `Verdict: ReviewRunError` line, `ENV_BLOCKED`, `WindowsApps`, `pwsh.exe`, concrete policy-block text 가 모두 있을 때만 열어야 한다. arbitrary nonzero exit, missing output, timeout, model/tool failure, 또는 generic `blocked by policy` 단독 matching 은 fallback 이 아니라 fail-closed `ReviewRunError` 다. Clean verdict 도 정확히 1개인 `Verdict: Clean` line 으로만 gate pass 처리한다.
  - `CodexAdversarialXHigh`: `/codex:adversarial-review --effort xhigh` 와 동등한 앱 profile. app/orchestrator 실행 시 `reasoning_effort="xhigh"` 또는 CLI 등가 config 를 강제하고, output capture 는 `source_output_path` 필수.
- `ReviewRunRecord`: `{ verdict, reviewer: Lane, review_kind: CodexAdversarialXHigh | ClaudeSymmetry | Manual, review_profile_id, reasoning_effort, model_or_profile_id, prompt_template_version, prompt_hash, command_source_adapter, primary_role, scope, gate: PrCreate | PrMerge | IntegrateMerge | MainApply | TicketClose, patch_hash, files_reviewed: Vec<PathBuf>, omitted_files: Vec<PathBuf>, limitations: Vec<String>, evidence: String, required_actions: Vec<String>, created_at, source_output_path }`.
- Reviewer matrix:
  - PrimaryRole=Claude: mandatory `CodexAdversarialXHigh`. Claude self/symmetry review optional but merge gate decision 은 `CodexAdversarialXHigh` 포함 aggregate 로 판단.
  - PrimaryRole=Codex: mandatory `CodexAdversarialXHigh` + optional ClaudeSymmetry. ClaudeSymmetry 는 추가 신호이며 `CodexAdversarialXHigh` 를 대체하지 않는다.
  - Aggregate: `Block > Concern > Clean`; `ReviewRunError` 는 Block 과 동일하게 gate stop.
- `ReviewInputStrategy` (같은 파일):
  ```
  ReviewInput {
    patch_hash, diff_stat, changed_files, ticket_scope, critical_files,
    chunks: Vec<Chunk>,           // diff 가 max_context_bytes 초과 시 분할
    max_context_bytes,            // reviewer 별 default
    reviewed_subset_reason,       // 부분 review 이면 사유
    omitted_files,                // chunking 으로 빠진 파일
  }
  ```
- 대형 diff 처리: `max_context_bytes` 초과 시 critical_files 우선 + ticket_scope 기반 chunking → chunk 별 verdict aggregate (`Block ⊃ Concern ⊃ Clean`). reviewed_subset_reason 사용자 표시 의무.
- T10/T11/T12 의 review 의무 절은 본 schema 사용. "full diff 직접 투입 금지, chunked aggregate verdict 사용" 명시.
- Gate 시점: `pr_create` 전 local diff, `pr_merge` 전 PR diff, `integrate_merge` 전 통합 diff, `main_apply` 전 최종 diff. review 가 불가능한 시점 또는 `CodexAdversarialXHigh` audit field 누락은 skip 이 아니라 `ReviewRunError` 로 기록하고 gate stop.

**Success criteria**:
- [ ] 50KB 미만 diff: 단일 review.
- [ ] 50KB-500KB: critical_files 우선 1 chunk, 나머지 부속 chunk.
- [ ] 500KB+: ticket_scope 별 분할 + omitted_files 표시.
- [ ] verdict aggregate 정확성: chunk1=Clean + chunk2=Concern → 전체 Concern. chunk1=Block 하나라도 → 전체 Block.
- [ ] ReviewRunRecord 가 journal, lane result, ResumePacket, PR/merge report 에 모두 저장된다.
- [ ] 모든 gate 의 ReviewRunRecord 에 `verdict`, `reviewer`, `review_kind`, `review_profile_id=CodexAdversarialXHigh`, `reasoning_effort=xhigh`, `model_or_profile_id`, `prompt_template_version`, `prompt_hash`, `command_source_adapter`, `primary_role`, `scope`, `gate`, `patch_hash`, `files_reviewed`, `omitted_files`, `limitations`, `evidence`, `required_actions`, `created_at`, `source_output_path` 가 없으면 fail closed.
- [ ] 단위 테스트: 5 size class × 2 PrimaryRole = 10 케이스 + reviewer matrix/gate timing 8 케이스 + `CodexAdversarialXHigh` audit field 8 케이스.

### L3 — Policy Pack (executable schema, not markdown copy)

**목표**: 글로벌 `~/.claude/*.md` 를 markdown 채로 import 하지 않고 **구조화된 schema** 로 resolve. drift detect + 명시적 sync.

**파일**:
- `src-tauri/src/policy/pack.rs` (신규) — `PolicyPack { source_manifest: Vec<SourceEntry>, runtime_profile: RuntimeProfile, output_blocklist, role_bindings, token_thresholds, handoff_behavior, command_permission_classes, ticket_close_gate, version }`.
  - `SourceEntry { path: PathBuf, kind: SourceKind, sha256: String, size_bytes: u64, role: SourceRole }`
  - `SourceKind { HotRule, OnDemandSkill, TicketCloseRule, RuntimeHealthCheck, RuntimeSettings, CodexDesktopOverlay }` — hot/skill 분할 + TICKET-CLOSE.md 의 close-gate 역할 + codex-mcp-runtime 의 환경 health check 역할 + settings safe-subset 역할 + Codex Desktop 전역 skill overlay 역할 명시
  - `SourceRole { GuardClaude, GuardCodex, GuardShared, OutputScannerSource, TokenThreshold, HandoffPolicy, CloseGate, RuntimePatch, SlashCommandReference, RuntimeProfile }` — `guard_text_claude`/`guard_text_codex` 단일 string 모델 폐기. resolve 시 다중 source concat
- `~/.moa-desktop/policy.json` (런타임) — 사용자 PC 에서 resolve 된 active pack.
- `src-tauri/src/policy/sync.rs` (신규) — 글로벌 15 파일 (`~/.claude/{CLAUDE.md,RTK.md,TOKEN-GUARD.md,KARPATHY.md,TICKET-CLOSE.md,CODEX-MCP.md, skills/{codex-mcp-runtime,token-guard-internals}/SKILL.md, commands/{다음세션,쉽게,진행}.md, skills/{메인동기화,백로그,병행통합,병행티켓}/SKILL.md}`) + Codex Desktop overlay (`~/.codex/skills/병행티켓/SKILL.md` when present) 의 hash 비교 → drift 발견 시 UI notification → 사용자 명시 import 명령 → policy.json 갱신. Claude-side source 가 stale gate vocabulary 를 담고 Codex-side overlay 가 더 최신이면 resolver 는 silent merge 하지 않고 drift/conflict 로 fail closed 한다.
- `src-tauri/src/policy/runtime_profile.rs` (신규) — `~/.claude/settings.json` 의 safe subset 만 구조화 import. raw JSON 통째 복사 금지.
  - include: env allowlist (`CODEX_HOME`, `CODEX_SHELL`, `CODEX_INTERNAL_ORIGINATOR_OVERRIDE`, `CODEX_COMPANION_FORCE_DIRECT_APP_SERVER`, `ENABLE_CLAUDEAI_MCP_SERVERS`, `MAX_THINKING_TOKENS`, `MAX_MCP_OUTPUT_TOKENS`, output/timeout budget), `alwaysThinkingEnabled`, `showThinkingSummaries`, permissions deny/ask 패턴, enabled plugin 이름/autoUpdate, `extraKnownMarketplaces`, `autoUpdatesChannel`, hook command hash, statusLine command hash.
  - exclude: credentials/auth/cookie/token/path-secret, shell history, stats/cache/session payload, arbitrary unvalidated env.
- **syncMode** (PolicyPack 필드, default `manual`):
  - `manual` (기본 안전값): 모든 drift 사용자 명시 import.
  - `trusted-safe-auto`: schema 검증 통과 + active session 없음 + destructive permission scope 확대 없음 + safe fields (guard text, scanner blocklist 추가, token thresholds, handoff behavior) 만 자동 적용. **항상 manual confirm 의무**: GitHub/write 권한 추가, command permission class 상승, output blocklist **완화** (제거/약화), role binding 변경.
  - 사용자 의도 ("글로벌 개선분 앱에서 살아있게") 반영하되 destructive scope 는 안전 유지.
- UI: "Policy Pack" 패널 — 현재 active version, drift 표시 (어느 글로벌 파일이 변경됨), import 버튼.

**Success criteria**:
- [ ] 글로벌 ~/.claude/CODEX-MCP.md 변경 → 앱 재시작 시 drift 표시.
- [ ] 글로벌 ~/.claude/settings.json 의 safe subset 변경 → `RuntimeProfile` drift 표시. secret/auth 값은 캡처되지 않음.
- [ ] 사용자 import 클릭 → policy.json 갱신 + L1/L2/L4 가 새 값 사용 (다음 세션부터).
- [ ] policy.json 누락 시 안전 default 로 fallback (앱 동작 영속성).
- [ ] policy.json schema 검증 — 미지정 필드는 default, 잘못된 enum 은 reject.
- [ ] 단위 테스트: drift detection (hash 변경/누락/추가), runtime_profile safe-subset import/exclusion, import roundtrip, fallback.

### L4 — Privileged Slash Command Subsystem

**목표**: 앱 UI 슬래시 입력 → orchestrator dispatch (워커가 아니라). Tauri capability allowlist 기반. Permission class 별 분류.

**파일**:
- `src-tauri/src/commands/mod.rs` (신규) — `CommandRegistry`, `Permission { ReadOnly, MetaDispatch, SessionMgmt, DestructiveNetwork }`, `dispatch(name, args, permission_check) -> Result<Stream>`.
- `src-tauri/src/commands/{handoff,briefing,proceed,backlog,sync_main,decompose,integrate}.rs` (각 슬래시 1 파일):
  - `handoff` (`/다음세션`, SessionMgmt) — L5 ResumePacket emit.
  - `briefing` (`/쉽게`, ReadOnly) — Korean non-dev briefing 생성.
  - `proceed` (`/진행`, MetaDispatch) — briefing + auto-run 추천 명령. destructive 만 confirm.
  - `backlog` (`/백로그`, DestructiveNetwork, **3 step**) — (1) 앱 backlog DB write 미리보기 + 사용자 confirm, (2) GitHub Issue API 호출 + 사용자 confirm, (3) `~/.claude/projects/<repo>/memory/` mirror write. 각 step 별도 confirm.
  - `sync_main` (`/메인동기화`, DestructiveNetwork, **단계별 confirm**) — 글로벌 `/메인동기화` 는 raw import 하지 않고 policy resolver 가 앱 gate 로 변환한다. 5 step 각각 사용자 확인:
    1. 안전 가드 검증 (다른 ticket branch 오염, dirty file)
    2. **Pre-PR Review gate 의무 (lead/orchestrator-owned)** — local diff 에 대해 L2.5 ReviewInputStrategy 로 mandatory `CodexAdversarialXHigh` 실행. MoA Desktop 앱에서는 source=orchestrator, Codex Desktop 수동 개발에서는 lead PowerShell 별도 리뷰 프로파일과 `.moa-desktop/reviews/<stamp>.md` output capture 로 실행할 수 있다. PrimaryRole=Codex 이면 optional ClaudeSymmetry 를 추가할 수 있으나 `CodexAdversarialXHigh` 를 대체하지 않는다.
    3. push + PR 생성 (gh, force-push 금지)
    4. **Pre-merge Review gate 의무 (lead/orchestrator-owned)** — PR diff 에 대해 mandatory `CodexAdversarialXHigh` 실행. 대형 diff 는 chunking, full-diff 직접 투입 금지. 워커가 `/codex:*`, `claude -p`, `codex exec` 를 직접 호출하는 nested peer-call 은 L2 command_guard/scanner 가 차단. lead PowerShell 별도 review 는 worker 밖 gate 실행으로만 허용된다.
    5. main merge + local pull
    각 step **사용자 확인 후만** 다음 진행 (Q2 결정).
  - `decompose` (`/병행티켓`, SessionMgmt) — T10 본체 호출.
  - `integrate` (`/병행통합`, DestructiveNetwork) — T12 본체 호출 + **integrate_merge/main_apply 직전 mandatory `CodexAdversarialXHigh` review gate 의무** (Q 추가요구). 글로벌 `/병행통합` 도 policy resolver 가 앱 gate 로 변환한다. 생성된 worker prompt 는 "review gate 통과 전 merge/main apply 금지" 를 안내하지만 워커에게 `/codex:*` 직접 호출을 지시하지 않는다.
- UI: 슬래시 자동완성 + permission class 색상 구분 (read-only=회색, destructive=빨강).
- Tauri `tauri.conf.json` capabilities: 각 명령 별 allowlist (gh CLI / git push / 외부 API 호출 등).

**Success criteria**:
- [ ] 워커는 슬래시 안 받음 (`--disable-slash-commands` 그대로) — 슬래시 입력은 UI → orchestrator 만.
- [ ] DestructiveNetwork 명령은 매 step 사용자 confirm 통과해야 진행.
- [ ] `/메인동기화` step 2/4 review verdict (L2.5 enum: Clean | Concern | Block | ReviewRunError) 가 Concern, Block, ReviewRunError 시 다음 destructive step 자동 진행 차단.
- [ ] `/병행티켓` 이 생성한 paste-ready prompt 에 PR/merge review gate 안내가 포함되며, nested peer-call 문자열(`/codex:`, `claude -p`, `codex exec`) 은 worker 실행 영역에 포함되지 않는다.
- [ ] `/병행통합` 의 PR 생성/머지 step 은 ReviewVerdict=Clean 일 때만 다음 단계로 진행한다.
- [ ] 글로벌 `/메인동기화`, `/병행티켓`, `/병행통합` 원문이 앱 정책과 충돌하는 경우 resolver transform 이 적용되고, transform 결과가 policy.json 에 versioned record 로 남는다.
- [ ] `/진행` 은 추천 명령을 **MetaDispatch 로 wrap** — 추천 안에 DestructiveNetwork 가 있으면 사용자에 explicit 표시 + 개별 confirm 필요.
- [ ] integration test: 7 명령 each end-to-end mock.

### L5 — Resume Packet & Session Lifecycle

**목표**: `.claude-handoff.md` 흉내 X. journal + synthesis + claim ledger + open questions + lane status + pending approvals + command history → 단일 resume artifact. T11 multi-lane 의 토대.

**파일**:
- `src-tauri/src/lifecycle/mod.rs` (신규) — `ResumePacket { session_id, project_id, last_phase, journal_tail, synthesis_snapshot, claim_ledger, open_questions, lane_states, review_run_records, pending_approvals, command_history, timestamp, version_pin }`.
- `src-tauri/src/lifecycle/export.rs` — `/다음세션` 호출 시 markdown export (사용자가 다음 세션 첫 입력으로 paste 가능).
- `src-tauri/src/lifecycle/import.rs` — 새 세션 시작 시 ResumePacket import (선택) → 상태 복원.
- `~/.moa-desktop/sessions/<projectId>/<sessionId>/resume.json` (런타임).
- T11 hook: 각 lane 의 panic boundary 가 ResumePacket 자동 emit (panic 시 lane 별 보존).

**Success criteria**:
- [ ] `/다음세션` → markdown packet 생성 + 클립보드 복사 + 파일 저장.
- [ ] 새 세션에서 import 클릭 → state machine + journal + claim ledger 복원.
- [ ] T11 lane panic → 해당 lane resume.json emit, 다른 lane 영향 X.
- [ ] ReviewRunRecord 의 full audit field set (`verdict`, `reviewer`, `review_kind`, `review_profile_id`, `reasoning_effort`, `model_or_profile_id`, `prompt_template_version`, `prompt_hash`, `command_source_adapter`, `primary_role`, `scope`, `gate`, `patch_hash`, `files_reviewed`, `omitted_files`, `limitations`, `evidence`, `required_actions`, `created_at`, `source_output_path`) 이 resume/import 후에도 보존된다.
- [ ] packet schema version pin — 구버전 packet 은 migration 또는 fallback.

## 글로벌 변화 반영 (Q 추가요구)

L3 의 sync 메커니즘이 base. 단 사용자 명시 추가:
- 본 ticket 본문 자체에 `~/.claude/CODEX-MCP.md § 2.5/2.6 (2026-05-04 cross-verify default)` 반영 — Flow C/D 의 cross-verify 강제, Claim Ledger 형식, 6 항목 의무 모두 본 ticket 의 prompt 에 박힘.
- L3 import 시 **글로벌 변화 changelog** 도 같이 표시 (어느 줄이 추가/삭제).

### Baseline pin (2026-05-07 사용자 결정 — A안 베이스라인만 + B안 별도 파일, 글로벌 분리 반영)

**대상 파일 15 개** (Hot 룰 6 + On-demand 스킬 2 + 한국어 단축명령 7):
- Hot 룰: `~/.claude/CLAUDE.md`, `~/.claude/RTK.md`, `~/.claude/TOKEN-GUARD.md`, `~/.claude/KARPATHY.md`, `~/.claude/TICKET-CLOSE.md`, `~/.claude/CODEX-MCP.md`
- On-demand 스킬 (Hot 룰의 절차/진단 분리본):
  - `~/.claude/skills/codex-mcp-runtime/SKILL.md` (CODEX-MCP.md hot 의 4-layer 패치/검증 분리, kind=`RuntimeHealthCheck`)
  - `~/.claude/skills/token-guard-internals/SKILL.md` (TOKEN-GUARD.md hot 의 진단/Opus 4.7 회귀 분리, kind=`OnDemandSkill`)
- 한국어 단축명령:
  - `~/.claude/commands/다음세션.md`, `~/.claude/commands/쉽게.md`, `~/.claude/commands/진행.md`
  - `~/.claude/skills/메인동기화/SKILL.md`, `~/.claude/skills/백로그/SKILL.md`, `~/.claude/skills/병행통합/SKILL.md`, `~/.claude/skills/병행티켓/SKILL.md`
- Codex Desktop overlay:
  - `~/.codex/skills/병행티켓/SKILL.md` — Codex Desktop 수동 개발의 current source. Claude-side `/병행티켓` 과 review gate vocabulary 또는 `command_source_adapter` 가 다르면 T13 resolver 는 fail closed 하고 사용자 import/transform confirm 을 요구한다.

**추가 런타임 입력 1 개 (safe subset only)**:
- `~/.claude/settings.json` — `RuntimeProfile` 로 resolve. baseline 에 raw file body 를 저장하지 않고, safe subset + source sha256 + denied secret-key proof 만 저장한다.

**저장 위치**: `~/.moa-desktop/policy/baseline-2026-05-07.json` (별도 파일, B안). schema:
```
{
  "version": "2026-05-07",
  "created_at": <ISO-8601>,
  "source_manifest": [
    { "path": "~/.claude/CLAUDE.md", "kind": "HotRule", "role": "GuardShared", "sha256": "<hex>", "size_bytes": <n>, "captured_excerpt_first_500_chars": "..." },
    { "path": "~/.claude/TICKET-CLOSE.md", "kind": "TicketCloseRule", "role": "CloseGate", ... },
    { "path": "~/.claude/skills/codex-mcp-runtime/SKILL.md", "kind": "RuntimeHealthCheck", "role": "RuntimePatch", ... },
    { "path": "~/.codex/skills/병행티켓/SKILL.md", "kind": "CodexDesktopOverlay", "role": "SlashCommandReference", "sha256": "<hex-or-null-when-absent>", "size_bytes": <n-or-null>, "present": true },
    ...,
    { "path": "~/.claude/settings.json", "kind": "RuntimeSettings", "role": "RuntimeProfile", "sha256": "<hex>", "safe_subset": { "env_allowlist": {...}, "permissions": {...}, "hooks_hash": {...} } }
  ],
  "policy_pack_resolution": {
    // L3 PolicyPack 가 본 baseline 으로부터 resolve 한 첫 active pack — 같은 파일 안에 박아 단일 source
  }
}
```

**Open questions** (L3 phase 진입 시 확정):
- O1: Hot 룰 갱신 vs On-demand 스킬 갱신 빈도 차이 → rolling baseline (`baseline-<date>.json` archive) 정책으로 충분한가?
- O2: `codex-mcp-runtime/SKILL.md` 가 plugin 경로 `openai-codex/codex/1.0.4/...` hardcode — plugin 버전 업 시 stale. baseline 도 path glob 화 (`openai-codex/codex/*/...`) 또는 baseline 갱신 trigger?
- O3: TICKET-CLOSE.md 자체가 글로벌 룰 → **본 EPIC ticket 닫기 절차에도 본 룰 적용 (재귀)**. § 작업 완료 절 참조.
- O4: `~/.claude/settings.json` safe subset 자동 import 범위 — env allowlist 변경은 manual confirm, purely display setting 변경은 safe auto 후보인지 L3 에서 결정.

**Pin 시점**: L3 phase 의 sync.rs 첫 구현 + 첫 실행 시점. 즉 **본 EPIC 의 L3 phase commit 일자 = baseline 일자**. 명시적으로 사용자가 "Init baseline" 명령 호출. 자동 pin X (사용자 의도와 시점 일치 보장).

**Pin 후 운영**:
- `policy.json` (active) 은 baseline 으로부터 resolve 됨.
- Claude-side 15 개 파일 + Codex Desktop overlay + settings safe subset 변경 → sync.rs 가 baseline 의 sha256 와 비교 → drift report (kind 별 group: HotRule 6, OnDemandSkill 2 + 단축명령 7, CodexDesktopOverlay, RuntimeSettings).
- 사용자 import 시 새 buffered pack 으로 swap, 단 baseline 자체는 **불변** (history pin).
- 차후 baseline 갱신 필요 시: 새 파일 `baseline-<date>.json` 추가, 기존은 archive (rolling baseline).

**Success criteria 추가** (L3 phase):
- [ ] `Init baseline` 명령 → Claude-side 15 개 파일 (Hot 6 + on-demand 2 + 단축명령 7) + Codex Desktop overlay `~/.codex/skills/병행티켓/SKILL.md`(present/null semantics 포함) hash + excerpt + kind + role 캡처 → `baseline-<today>.json` 작성.
- [ ] `~/.claude/settings.json` safe subset 캡처 → `RuntimeProfile` 작성. raw settings body 와 secret/auth/cookie/token 값은 baseline 에 저장되지 않음.
- [ ] Claude-side `/병행티켓` 과 Codex Desktop overlay 가 모두 존재하고 review gate vocabulary 또는 `command_source_adapter` semantics 가 충돌하면 resolver output 은 fail closed 이며 사용자 import/transform confirm 없이는 active PolicyPack 으로 승격하지 않는다.
- [ ] baseline 누락 또는 schema 불일치 시 sync.rs fail-soft (manual mode 강제).
- [ ] 15 개 중 일부 파일 미존재 (예: `~/.claude/skills/codex-mcp-runtime/SKILL.md` 없는 환경 — Layer 2/3 patch 미적용) → null 로 기록 + warning + kind 별 critical 여부 판단 (HotRule/TicketCloseRule 누락 = blocker, OnDemandSkill/RuntimeHealthCheck/RuntimeSettings 누락 = warning; RuntimeSettings schema 불일치 또는 hook/statusLine hash drift = manual confirm).

## NEVER 영역 (본 ticket 내내)
- src-tauri/src/{adapters,git,journal,synthesis (T3 결정론적 merge 본체),process,telemetry,cancel,mock,lock (state machine 본체)}/ 본문 — 본 ticket 은 **policy/safety/commands/lifecycle 신규** 만.
- T10/T11/T12 의 owns 영역 (decomposer/parallel/integrator 본문) — 본 ticket 은 hook 만 박음, 본문 작성은 T10-T12 가.
- 비밀 파일.

## 작업 완료 시 (글로벌 `~/.claude/TICKET-CLOSE.md` 의무 적용 — 재귀 자기 적용)

### 1. TICKET-CLOSE § 1 — 사전 충돌 검사
- 본 프로젝트 메모리 (`~/.claude/projects/D--moa-desktop/memory/MEMORY.md`) 의 `project_*` / `feedback_*` 항목 전부 훑고, 변경 영역 (policy/safety/commands/lifecycle 신규 + L3 글로벌 sync 대상 15 파일 + settings safe subset) 과 겹치는 결정이 "보류·pending·deferred" 인지 확인.
- 충돌 시 → 변경 진행 자체 중단 + 사용자 보고. 현재 메모리 기준: #19 codex sandbox drift 는 resolved 로 취급, #20 orch AppHandle integration test 는 T13 L1 전 hard prerequisite, #26 policy source manifest / #28 SourceKind severity / #29 T14 conversational mode 는 본 ticket 과 충돌 여부 확인.

### 2. TICKET-CLOSE § 2 — Decisions 5 컬럼 schema
PR description 또는 본 ticket 의 phase 별 commit message body 에 다음 5 컬럼:

| # | 결정 | 채택 옵션 | 거부 옵션 | 근거 |
|---|---|---|---|---|
| 1 | PrimaryRole 도입 위치 | `settings.primaryRole` 단일 enum | per-flow override | 사용자 Q1·B 결정 (PLAN § 0.6) |
| 2 | DestructiveNetwork 슬래시 confirm 모델 | step-gate (4-5 step 각 confirm) | 단일 apply confirm | 사용자 Q2 결정 + blast radius |
| 3 | 글로벌 sync 모드 default | `manual` | `trusted-safe-auto` | 사용자 Q3 + destructive scope 보안 |
| 4 | PolicyPack source 표현 | `source_manifest[]` + kind discriminator | 단일 `guard_text_*` string | Codex 1차 검토 (글로벌 분리 후 schema 안정성) |
| 5 | baseline 파일 count | 15 + settings safe subset | 12 (구버전) / raw settings 복사 | 글로벌 신규 import (TICKET-CLOSE) + skill 분리 + runtime 설정 이식 요구 |

### 3. TICKET-CLOSE § 3 — CodexAdversarialXHigh review gate (의무, 본 EPIC 적용 대상)
phase 별 commit 직전 (L1~L5 각각) lead/orchestrator-owned `CodexAdversarialXHigh` review 를 1 회 실행한다. MoA Desktop 앱 안에서는 앱/orchestrator source 가 ReviewProfile 을 실행한다. Codex Desktop 수동 개발 흐름에서는 사용자가 명시한 lead PowerShell 별도 리뷰 프로파일과 `.moa-desktop/reviews/<stamp>.md` output capture 가 허용되며, 그 결과 파일을 `ReviewRunRecord.source_output_path` 로 남긴다. lead PowerShell review 는 먼저 `--sandbox read-only` 로 실행한다. WindowsApps `pwsh.exe -Command ... rejected: blocked by policy` 로 `ENV_BLOCKED` 가 나오면 failed read-only attempt 를 `ReviewRunError` 로 기록한 뒤, Codex Desktop lead/manual session 이 controlled-bypass review gate 를 1회 실행할 수 있다. controlled-bypass 는 `--dangerously-bypass-approvals-and-sandbox` 를 사용하지만 READ-ONLY prompt, mutation 금지, before/after `git status`, review-caused mutation 0건, `Verdict: Clean`, failed read-only attempt path 기록이 모두 있을 때만 gate 증거다. worker 는 `/codex:*`, `codex exec`, `claude -p`, Claude/Codex MCP 를 직접 호출하지 않는다. 수동 Claude Worker 문맥에서 peer review 가 필요하면 `NEED_PEER_REVIEW` 를 출력하고 stop 한다.

ReviewProfile 요청에는 다음 5 가지 focus 를 포함한다:
1. 변경 영역과 본 프로젝트 메모리 백로그 결정 충돌 여부
2. PR description / commit body 의 Decisions 5 컬럼 schema 충족 여부
3. 의도된 정책 vs 실제 코드 일치 (silent intent drift 검출)
4. 만지지 않은 영역의 회귀 가능성 (cross-cutting risk)
5. 다른 세션이 본 변경 인지 가능한가 (PR / 메모리 / 핸드오프 어디에 무엇이 있나)

`ReviewRunRecord` 는 full audit field set 을 저장한다. `Concern`, `Block`, `ReviewRunError`, 또는 필수 audit field 누락 → fail closed. `Clean` 일 때만 다음 phase commit / PR / merge 단계로 진행한다.

### 4. TICKET-CLOSE § 4 — 메모리 갱신
- 신규 결정 → `project_t13_*.md` 추가
- 시정 받은 행동 → `feedback_*.md` 추가
- 의도적으로 뺀 항목 (예: trusted-safe-auto safe field 범위 미확정) → `project_deferred_*.md`
- `MEMORY.md` 인덱스 1 줄 추가 (트리거 조건 description 에 명시)

### 5. 기존 절차
1. phase 별 commit 5 개 (L1~L5 각각 별도 commit, push 금지).
2. 최종 머지 commit: `feat(T13): policy + safety + slash registry + lifecycle prequel for v1.5` (본문에 `Closes #35` 포함, push 금지).
3. PLAN.md § 0.6 amend (이미 본 ticket 진입 전에 작성).
4. AGENTS.md amend (PrimaryRole 명시).
5. **GitHub 카드 close**: `node ~/.claude/scripts/gh-tickets.mjs complete D:\moa-desktop 35`.
6. 보고: 8 PrimaryRole × Flow 케이스 결과, scanner 96 케이스 결과, sync drift 시나리오, slash 7 명령 e2e mock 결과, ResumePacket roundtrip 결과.

## 적용 순서 권고
L1 → L2 → L3 → L4 → L5. L4 가 L1+L2+L3 모두 의존. L5 는 L4 의 `/다음세션` 명령이 호출자.
