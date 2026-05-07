# T13 — Policy & Session Lifecycle EPIC (v1.5 prequel, T10 진입 전 필수)

## 배경 (2026-05-07 사용자 비전 검증 결과, 2026-05-07 글로벌 분리 반영)

글로벌 `~/.claude/{CLAUDE.md, RTK.md, TOKEN-GUARD.md, KARPATHY.md, TICKET-CLOSE.md, CODEX-MCP.md}` (Hot 룰 6) + on-demand 스킬 `skills/{codex-mcp-runtime,token-guard-internals}/SKILL.md` (2) + 한국어 슬래시 7 개 (`commands/{다음세션,쉽게,진행}.md` + `skills/{메인동기화,백로그,병행통합,병행티켓}/SKILL.md`) 의 운영체계를 본 앱의 **표준 session lifecycle** 로 승격. **총 15 파일**. 사용자 결정:
- **Q1 → B**: PrimaryRole=Codex 시 **mutation owner 까지** Codex 로 스왑.
- **Q2 → 단계별 confirm**: `/메인동기화` 류 destructive 명령은 4-5 step 별 사용자 확인 (자동화-2-개입 원칙 의 명시적 예외).
- **Q3 → 추천대로**: 앱 backlog = source of truth, `~/.claude/projects/<repo>/memory/` 는 mirror (앱 → 글로벌 단방향 sync).

## 의존성

선행: T7-full 머지 완료 (`bee18d9`). 본 ticket 통과 후에야 T10 진입.

## 5 Phase 구조 (단일 ticket, phase 별 commit)

### L1 — PrimaryRole + ExecutionPolicy (선행, 다른 phase 의 토대)

**목표**: orchestrator/synthesis/adversarial/Flow-C-mutation-owner 의 Claude-main hardcode 제거. `settings.primaryRole: "claude" | "codex"` 도입.

**파일**:
- `src-tauri/src/policy/mod.rs` (신규) — `PrimaryRole` enum, `ExecutionPolicy { synthesizer, default_reviewer, mutation_owner_default(flow), label }`.
- `src-tauri/src/orchestrator/mod.rs:589` amend — `default_reviewer(Lane::Claude)` → `policy.default_reviewer()`.
- `src-tauri/src/orchestrator/mod.rs:738-741` amend — `Flow::B → Codex, _ → Claude` → `policy.mutation_owner_default(flow)`.
- `src-tauri/src/orchestrator/adversarial.rs:68-74` amend — synthesizer 가정 제거.
- `src/lib/synthesis/merge.ts:123, 222-227` amend — Claude-first 가정 제거.
- `src-tauri/src/settings/` (신규 또는 stub 채움) — `primaryRole` 필드 + load/save.
- UI: 설정 화면에 PrimaryRole 토글 (Claude / Codex). 실시간 반영 안 함 — 다음 세션부터.

**Success criteria**:
- [ ] PrimaryRole 토글 후 Flow C 시작 → synthesizer/reviewer/mutation owner 모두 정확히 스왑.
- [ ] 기존 e2e (T7-full dry-run) 양 role 모두 통과.
- [ ] lock state machine 무영향 (`lock/manager.rs:45-47` 그대로).
- [ ] PrimaryRole 변경 시 진행중 세션 영향 X (다음 세션부터 적용 — UI 명시).
- [ ] 단위 테스트: `policy::tests` — Flow A/B/C/D × 양 PrimaryRole = 8 케이스 mutation_owner / synthesizer / reviewer 검증.

### L2 — SafetyPolicy + Output Scanner (role-aware)

**목표**: [DESIGN.md:90-92](../DESIGN.md) 의 block list 코드화. **role-aware**: PrimaryRole 받아 정상 orchestration 문구 vs 워커 peer-call 위반 구분.

**파일**:
- `src-tauri/src/safety/scanner.rs` (신규) — block list regex + role context input + `ScanResult { Clean | Violation { kind, evidence, role_context } }`.
- `src-tauri/src/safety/mod.rs` amend — `pub mod scanner; pub use scanner::*;`.
- 통과 지점 (모두 같은 scanner 사용):
  1. Worker stdout (claude.rs / codex.rs streaming chunk).
  2. Slash command output (L4).
  3. Integrator / gh helper output (T12 진입 전 hook).
  4. UI 가 표시 직전 (final filter).

**Block list (DESIGN.md:91-92 + adversarial 보강)**:
- 양방향 peer-call 패턴: `/codex:`, `claude -p`, `codex exec`, `Claude MCP`, `Codex MCP`, `claude_code_peer`, `TeamCreate`, `Agent(`, `call Codex`, `call Claude`, `ask another AI`, `run another agent`.
- Role-aware 예외: orchestrator 자체가 워커 spawn 하는 정상 메시지 ("Spawning Claude worker for first-pass…") 는 source=orchestrator 일 때만 통과. source=worker 일 때는 차단.

**Success criteria**:
- [ ] 워커 stdout 에 `/codex:rescue` 흔적 → 즉시 차단 + 사용자 alert.
- [ ] orchestrator 의 "claude worker spawning" 정상 메시지 → 통과.
- [ ] integrator stdout 에 `git push` 워커가 시도하면 → 차단 (워커는 orchestrator 권한 없음).
- [ ] PrimaryRole=Codex 시 Claude worker 의 `codex exec` 시도 차단 (방향 무관).
- [ ] 단위 테스트: 12 블록패턴 × 4 source × 2 PrimaryRole = 96 케이스.

### L2.5 — ReviewVerdict schema + ReviewInputStrategy (review state machine 입력 통일)

**목표**: T10/T11/T12 가 흩어 쓰던 verdict 용어 (clean/concern/block/fail) 를 단일 enum 으로 통일. 대형 diff (PR 100+ 파일) 시 Codex/Claude reviewer context overflow 방지.

**파일**:
- `src-tauri/src/policy/review.rs` (신규) — `ReviewVerdict { Clean, Concern, Block }` + `ReviewRunError` (실행 자체 실패 — schema 외).
- payload: `{ verdict, reviewer: Lane, primary_role, scope, patch_hash, files_reviewed: Vec<PathBuf>, limitations: Vec<String>, evidence: String, required_actions: Vec<String>, created_at }`.
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

**Success criteria**:
- [ ] 50KB 미만 diff: 단일 review.
- [ ] 50KB-500KB: critical_files 우선 1 chunk, 나머지 부속 chunk.
- [ ] 500KB+: ticket_scope 별 분할 + omitted_files 표시.
- [ ] verdict aggregate 정확성: chunk1=Clean + chunk2=Concern → 전체 Concern. chunk1=Block 하나라도 → 전체 Block.
- [ ] 단위 테스트: 5 size class × 2 PrimaryRole = 10 케이스.

### L3 — Policy Pack (executable schema, not markdown copy)

**목표**: 글로벌 `~/.claude/*.md` 를 markdown 채로 import 하지 않고 **구조화된 schema** 로 resolve. drift detect + 명시적 sync.

**파일**:
- `src-tauri/src/policy/pack.rs` (신규) — `PolicyPack { source_manifest: Vec<SourceEntry>, output_blocklist, role_bindings, token_thresholds, handoff_behavior, command_permission_classes, ticket_close_gate, version }`.
  - `SourceEntry { path: PathBuf, kind: SourceKind, sha256: String, size_bytes: u64, role: SourceRole }`
  - `SourceKind { HotRule, OnDemandSkill, TicketCloseRule, RuntimeHealthCheck }` — hot/skill 분할 + TICKET-CLOSE.md 의 close-gate 역할 + codex-mcp-runtime 의 환경 health check 역할 명시
  - `SourceRole { GuardClaude, GuardCodex, GuardShared, OutputScannerSource, TokenThreshold, HandoffPolicy, CloseGate, RuntimePatch, SlashCommandReference }` — `guard_text_claude`/`guard_text_codex` 단일 string 모델 폐기. resolve 시 다중 source concat
- `~/.moa-desktop/policy.json` (런타임) — 사용자 PC 에서 resolve 된 active pack.
- `src-tauri/src/policy/sync.rs` (신규) — 글로벌 15 파일 (`~/.claude/{CLAUDE.md,RTK.md,TOKEN-GUARD.md,KARPATHY.md,TICKET-CLOSE.md,CODEX-MCP.md, skills/{codex-mcp-runtime,token-guard-internals}/SKILL.md, commands/{다음세션,쉽게,진행}.md, skills/{메인동기화,백로그,병행통합,병행티켓}/SKILL.md}`) 의 hash 비교 → drift 발견 시 UI notification → 사용자 명시 import 명령 → policy.json 갱신.
- **syncMode** (PolicyPack 필드, default `manual`):
  - `manual` (기본 안전값): 모든 drift 사용자 명시 import.
  - `trusted-safe-auto`: schema 검증 통과 + active session 없음 + destructive permission scope 확대 없음 + safe fields (guard text, scanner blocklist 추가, token thresholds, handoff behavior) 만 자동 적용. **항상 manual confirm 의무**: GitHub/write 권한 추가, command permission class 상승, output blocklist **완화** (제거/약화), role binding 변경.
  - 사용자 의도 ("글로벌 개선분 앱에서 살아있게") 반영하되 destructive scope 는 안전 유지.
- UI: "Policy Pack" 패널 — 현재 active version, drift 표시 (어느 글로벌 파일이 변경됨), import 버튼.

**Success criteria**:
- [ ] 글로벌 ~/.claude/CODEX-MCP.md 변경 → 앱 재시작 시 drift 표시.
- [ ] 사용자 import 클릭 → policy.json 갱신 + L1/L2/L4 가 새 값 사용 (다음 세션부터).
- [ ] policy.json 누락 시 안전 default 로 fallback (앱 동작 영속성).
- [ ] policy.json schema 검증 — 미지정 필드는 default, 잘못된 enum 은 reject.
- [ ] 단위 테스트: drift detection (hash 변경/누락/추가), import roundtrip, fallback.

### L4 — Privileged Slash Command Subsystem

**목표**: 앱 UI 슬래시 입력 → orchestrator dispatch (워커가 아니라). Tauri capability allowlist 기반. Permission class 별 분류.

**파일**:
- `src-tauri/src/commands/mod.rs` (신규) — `CommandRegistry`, `Permission { ReadOnly, MetaDispatch, SessionMgmt, DestructiveNetwork }`, `dispatch(name, args, permission_check) -> Result<Stream>`.
- `src-tauri/src/commands/{handoff,briefing,proceed,backlog,sync_main,decompose,integrate}.rs` (각 슬래시 1 파일):
  - `handoff` (`/다음세션`, SessionMgmt) — L5 ResumePacket emit.
  - `briefing` (`/쉽게`, ReadOnly) — Korean non-dev briefing 생성.
  - `proceed` (`/진행`, MetaDispatch) — briefing + auto-run 추천 명령. destructive 만 confirm.
  - `backlog` (`/백로그`, DestructiveNetwork, **3 step**) — (1) 앱 backlog DB write 미리보기 + 사용자 confirm, (2) GitHub Issue API 호출 + 사용자 confirm, (3) `~/.claude/projects/<repo>/memory/` mirror write. 각 step 별도 confirm.
  - `sync_main` (`/메인동기화`, DestructiveNetwork, **단계별 confirm**) — 4-5 step 각각 사용자 확인:
    1. 안전 가드 검증 (다른 ticket branch 오염, dirty file)
    2. push (브랜치 새로 만들기 + force-push 금지)
    3. PR 생성 (gh)
    4. **Codex review trigger 의무** — PR 생성 직후 Codex adversarial-review (L2.5 ReviewInputStrategy 사용 — 대형 diff chunking, full-diff 직접 투입 금지). L1 PrimaryRole=Codex 시 Claude review 가 대체.
    5. main merge + local pull
    각 step **사용자 확인 후만** 다음 진행 (Q2 결정).
  - `decompose` (`/병행티켓`, SessionMgmt) — T10 본체 호출.
  - `integrate` (`/병행통합`, DestructiveNetwork) — T12 본체 호출 + **PR 생성/머지 step 에서 Codex review 의무 prompt** (Q 추가요구).
- UI: 슬래시 자동완성 + permission class 색상 구분 (read-only=회색, destructive=빨강).
- Tauri `tauri.conf.json` capabilities: 각 명령 별 allowlist (gh CLI / git push / 외부 API 호출 등).

**Success criteria**:
- [ ] 워커는 슬래시 안 받음 (`--disable-slash-commands` 그대로) — 슬래시 입력은 UI → orchestrator 만.
- [ ] DestructiveNetwork 명령은 매 step 사용자 confirm 통과해야 진행.
- [ ] `/메인동기화` step 4 review verdict (L2.5 enum: Clean | Concern | Block) 가 Concern 또는 Block 시 step 5 (merge) 자동 진행 차단. ReviewRunError (실행 실패) 도 차단.
- [ ] `/진행` 은 추천 명령을 **MetaDispatch 로 wrap** — 추천 안에 DestructiveNetwork 가 있으면 사용자에 explicit 표시 + 개별 confirm 필요.
- [ ] integration test: 7 명령 each end-to-end mock.

### L5 — Resume Packet & Session Lifecycle

**목표**: `.claude-handoff.md` 흉내 X. journal + synthesis + claim ledger + open questions + lane status + pending approvals + command history → 단일 resume artifact. T11 multi-lane 의 토대.

**파일**:
- `src-tauri/src/lifecycle/mod.rs` (신규) — `ResumePacket { session_id, project_id, last_phase, journal_tail, synthesis_snapshot, claim_ledger, open_questions, lane_states, pending_approvals, command_history, timestamp, version_pin }`.
- `src-tauri/src/lifecycle/export.rs` — `/다음세션` 호출 시 markdown export (사용자가 다음 세션 첫 입력으로 paste 가능).
- `src-tauri/src/lifecycle/import.rs` — 새 세션 시작 시 ResumePacket import (선택) → 상태 복원.
- `~/.moa-desktop/sessions/<projectId>/<sessionId>/resume.json` (런타임).
- T11 hook: 각 lane 의 panic boundary 가 ResumePacket 자동 emit (panic 시 lane 별 보존).

**Success criteria**:
- [ ] `/다음세션` → markdown packet 생성 + 클립보드 복사 + 파일 저장.
- [ ] 새 세션에서 import 클릭 → state machine + journal + claim ledger 복원.
- [ ] T11 lane panic → 해당 lane resume.json emit, 다른 lane 영향 X.
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

**저장 위치**: `~/.moa-desktop/policy/baseline-2026-05-07.json` (별도 파일, B안). schema:
```
{
  "version": "2026-05-07",
  "created_at": <ISO-8601>,
  "source_manifest": [
    { "path": "~/.claude/CLAUDE.md", "kind": "HotRule", "role": "GuardShared", "sha256": "<hex>", "size_bytes": <n>, "captured_excerpt_first_500_chars": "..." },
    { "path": "~/.claude/TICKET-CLOSE.md", "kind": "TicketCloseRule", "role": "CloseGate", ... },
    { "path": "~/.claude/skills/codex-mcp-runtime/SKILL.md", "kind": "RuntimeHealthCheck", "role": "RuntimePatch", ... },
    ...
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

**Pin 시점**: L3 phase 의 sync.rs 첫 구현 + 첫 실행 시점. 즉 **본 EPIC 의 L3 phase commit 일자 = baseline 일자**. 명시적으로 사용자가 "Init baseline" 명령 호출. 자동 pin X (사용자 의도와 시점 일치 보장).

**Pin 후 운영**:
- `policy.json` (active) 은 baseline 으로부터 resolve 됨.
- 글로벌 15 개 파일 변경 → sync.rs 가 baseline 의 sha256 와 비교 → drift report (kind 별 group: HotRule 6, OnDemandSkill 2 + 단축명령 7).
- 사용자 import 시 새 buffered pack 으로 swap, 단 baseline 자체는 **불변** (history pin).
- 차후 baseline 갱신 필요 시: 새 파일 `baseline-<date>.json` 추가, 기존은 archive (rolling baseline).

**Success criteria 추가** (L3 phase):
- [ ] `Init baseline` 명령 → 15 개 파일 (Hot 6 + on-demand 2 + 단축명령 7) hash + excerpt + kind + role 캡처 → `baseline-<today>.json` 작성.
- [ ] baseline 누락 또는 schema 불일치 시 sync.rs fail-soft (manual mode 강제).
- [ ] 15 개 중 일부 파일 미존재 (예: `~/.claude/skills/codex-mcp-runtime/SKILL.md` 없는 환경 — Layer 2/3 patch 미적용) → null 로 기록 + warning + kind 별 critical 여부 판단 (HotRule 누락 = blocker, OnDemandSkill 누락 = warning).

## NEVER 영역 (본 ticket 내내)
- src-tauri/src/{adapters,git,journal,synthesis (T3 결정론적 merge 본체),process,telemetry,cancel,mock,lock (state machine 본체)}/ 본문 — 본 ticket 은 **policy/safety/commands/lifecycle 신규** 만.
- T10/T11/T12 의 owns 영역 (decomposer/parallel/integrator 본문) — 본 ticket 은 hook 만 박음, 본문 작성은 T10-T12 가.
- 비밀 파일.

## 작업 완료 시 (글로벌 `~/.claude/TICKET-CLOSE.md` 의무 적용 — 재귀 자기 적용)

### 1. TICKET-CLOSE § 1 — 사전 충돌 검사
- 본 프로젝트 메모리 (`~/.claude/projects/D--moa-desktop/memory/MEMORY.md`) 의 `project_*` / `feedback_*` 항목 전부 훑고, 변경 영역 (policy/safety/commands/lifecycle 신규 + L3 글로벌 sync 대상 15 파일) 과 겹치는 결정이 "보류·pending·deferred" 인지 확인.
- 충돌 시 → 변경 진행 자체 중단 + 사용자 보고. (현재 알려진 pending: #19 codex sandbox drift, #20 orch integration test, #신규 T13 policy source manifest)

### 2. TICKET-CLOSE § 2 — Decisions 5 컬럼 schema
PR description 또는 본 ticket 의 phase 별 commit message body 에 다음 5 컬럼:

| # | 결정 | 채택 옵션 | 거부 옵션 | 근거 |
|---|---|---|---|---|
| 1 | PrimaryRole 도입 위치 | `settings.primaryRole` 단일 enum | per-flow override | 사용자 Q1·B 결정 (PLAN § 0.6) |
| 2 | DestructiveNetwork 슬래시 confirm 모델 | step-gate (4-5 step 각 confirm) | 단일 apply confirm | 사용자 Q2 결정 + blast radius |
| 3 | 글로벌 sync 모드 default | `manual` | `trusted-safe-auto` | 사용자 Q3 + destructive scope 보안 |
| 4 | PolicyPack source 표현 | `source_manifest[]` + kind discriminator | 단일 `guard_text_*` string | Codex 1차 검토 (글로벌 분리 후 schema 안정성) |
| 5 | baseline 파일 count | 15 (Hot 6 + on-demand 2 + 단축명령 7) | 12 (구버전) | 글로벌 신규 import (TICKET-CLOSE) + skill 분리 반영 |

### 3. TICKET-CLOSE § 3 — Codex adversarial-review gate (의무, 본 EPIC 적용 대상)
phase 별 commit 직전 (L1~L5 각각) `/codex:adversarial-review --effort xhigh` 1 회. 다음 5 가지 명시 요청:
1. 변경 영역과 본 프로젝트 메모리 백로그 결정 충돌 여부 (Codex 가 메모리 파일 직접 read 하도록 경로 제공)
2. PR description / commit body 의 Decisions 5 컬럼 schema 충족 여부
3. 의도된 정책 vs 실제 코드 일치 (silent intent drift 검출)
4. 만지지 않은 영역의 회귀 가능성 (cross-cutting risk)
5. 다른 세션이 본 변경 인지 가능한가 (PR / 메모리 / 핸드오프 어디에 무엇이 있나)
- BLOCKER → 수정 후 재검토. MINOR → cheap fix 또는 follow-up issue 명시. PASS → 머지 진행.

### 4. TICKET-CLOSE § 4 — 메모리 갱신
- 신규 결정 → `project_t13_*.md` 추가
- 시정 받은 행동 → `feedback_*.md` 추가
- 의도적으로 뺀 항목 (예: trusted-safe-auto safe field 범위 미확정) → `project_deferred_*.md`
- `MEMORY.md` 인덱스 1 줄 추가 (트리거 조건 description 에 명시)

### 5. 기존 절차
1. phase 별 commit 5 개 (L1~L5 각각 별도 commit, push 금지).
2. 최종 머지 commit: `feat(T13): policy + safety + slash registry + lifecycle prequel for v1.5`.
3. PLAN.md § 0.6 amend (이미 본 ticket 진입 전에 작성).
4. AGENTS.md amend (PrimaryRole 명시).
5. **GitHub 카드 close**: `node ~/.claude/scripts/gh-tickets.mjs complete D:\moa-desktop 13` (카드 신규 — 본 ticket 작성 시 같이 생성).
6. 보고: 8 PrimaryRole × Flow 케이스 결과, scanner 96 케이스 결과, sync drift 시나리오, slash 7 명령 e2e mock 결과, ResumePacket roundtrip 결과.

## 적용 순서 권고
L1 → L2 → L3 → L4 → L5. L4 가 L1+L2+L3 모두 의존. L5 는 L4 의 `/다음세션` 명령이 호출자.
