# PROJECT-RULES — MoA Desktop

본 프로젝트의 권위·NEVER·운영 규칙은 분산 정의되어 있다. 본 파일은 **인덱스** 다.

## 권위 문서 (변경은 사용자 승인 필요)
- [DESIGN.md](DESIGN.md) — 비전·아키텍처·UI·Worker guard·WorkerCommandGuard·Output scanner·Settings·scope in/out
- [PLAN.md](PLAN.md) — 구현 phase·F1-F6 critical fixes·Automation Contract·ticket dependency graph
- [synthesis.md](synthesis.md) — Claude+Codex first-pass 종합본 (참조용)
- [analysis-claude.md](analysis-claude.md) — Claude first-pass 원본 (참조용)
- [TICKETS/](TICKETS/) — 12 paste-ready 작업 티켓 (T0-T9 + TINTEGRATE) + v1.5 prequel (T13 EPIC) + v1.5 본체 (T10/T11/T12) + conversational mode pending (T14)

## 글로벌 권위 (`~/.claude/`)

**Hot 룰** (매 세션 cache_read 로 항상 로드, kind=`HotRule`):
- `CLAUDE.md` — 글로벌 인덱스 (RTK / TOKEN-GUARD / KARPATHY / TICKET-CLOSE / CODEX-MCP import)
- `CODEX-MCP.md` — Codex MCP 페어 모드, MoA 흐름 C/D, 6 항목 프롬프트, Claim Ledger
- `KARPATHY.md` — coding guideline (think before, simplicity, surgical, goal-driven)
- `TOKEN-GUARD.md` — 토큰 임계 운영
- `RTK.md` — Rust Token Killer CLI
- `TICKET-CLOSE.md` — 티켓 닫기 절차 의무 (메모리 충돌 검사 / 5 컬럼 Decisions / Codex adversarial gate / 메모리 갱신)

**On-demand 스킬** (트리거 발화 시 자동 invoke, kind=`OnDemandSkill`):
- `skills/codex-mcp-runtime/SKILL.md` — 4-layer Windows Desktop-equivalent 패치, 검증 스크립트, 재적용
- `skills/token-guard-internals/SKILL.md` — HARD 차단 진단·1M-context routing·cache_read 7x·Opus 4.7 회귀

**한국어 단축명령** (T13 L4 가 앱 dispatcher 로 흡수, kind=`OnDemandSkill`):
- `commands/다음세션.md`, `commands/쉽게.md`, `commands/진행.md` — session UX
- `skills/메인동기화/SKILL.md`, `skills/백로그/SKILL.md` — destructive-network
- `skills/병행통합/SKILL.md`, `skills/병행티켓/SKILL.md` — T10/T12 reference

위 **Claude-side 15 파일** (Hot 룰 6 + On-demand 스킬 2 + 한국어 단축명령 7) 과 Codex Desktop overlay `~/.codex/skills/병행티켓/SKILL.md`(존재 시) 는 T13 L3 의 baseline pin 대상 — `~/.moa-desktop/policy/baseline-<date>.json` 에 hash 로 박힘. 각 파일은 `source_manifest[]` 의 entry 로 `{path, kind, role, sha256, size_bytes}` 형태 저장 (`role` ∈ `SourceRole` enum: GuardClaude / GuardCodex / GuardShared / OutputScannerSource / TokenThreshold / HandoffPolicy / CloseGate / RuntimePatch / SlashCommandReference / RuntimeProfile — T13 ticket § L3 schema 참조). Claude-side `/병행티켓` 과 Codex overlay 의 review gate vocabulary 또는 `command_source_adapter` 가 충돌하면 resolver 는 silent merge 하지 않고 fail closed + 사용자 import/transform confirm 을 요구한다.

추가로 `~/.claude/settings.json` 은 raw file copy 가 아니라 safe subset 만 `RuntimeProfile` 로 resolve 한다. 포함 가능: Codex/Claude 런타임 env allowlist, thinking/output budget, deny/ask permission 패턴, enabled plugin metadata, `autoUpdatesChannel`, marketplace metadata, hook/statusLine command hash. 제외: credentials/auth/cookie/token/session/cache/history payload, arbitrary executable body.

`RuntimeSettings` 누락 severity: settings 파일 자체가 없으면 warning + safe default, safe subset schema 불일치는 blocker, plugin/runtime patch auto-update 상태 누락은 health finding, hook/statusLine hash 변경은 manual confirm 전까지 inactive.

## NEVER 영역 (모든 worker 공통)
- 비밀 파일: `.env`, `*credentials*`, `*cookie*`, `pipeline_config.json`, `auth.json`
- 다른 ticket 의 owns 영역 (각 TICKETS/T*.md 의 "Files owned" 절 + "NEVER 영역" 절 참조)
- T0 spike 영역 (`spikes/`) — T0 외 ticket 은 read 만
- 사용자 작업 repo 의 main worktree 직접 수정 — 모든 mutation 은 git worktree 격리 (PLAN.md § F4)
- output 에 `claude -p`, `codex exec`, `/codex:`, `claude_code_peer`, `Claude MCP`, `Codex MCP`, `TeamCreate`, `Agent`, `call Codex`, `call Claude` 등 peer 호출 흔적 (DESIGN.md "Output Scanner" 절)
- worker context 에서 peer AI executable/command 를 spawn 하거나 tool command 로 실행하는 것. 출력 스캐너는 2차 방어이며, T13 SafetyPolicy 는 process spawn/tool 실행 **전** `claude`, `codex exec`, `/codex:*`, Claude/Codex MCP, `TeamCreate`, `Agent` 계열을 차단해야 한다. 단, lead Codex Desktop 세션/사용자 PowerShell/MoA orchestrator 가 별도 read-only `CodexAdversarialXHigh` review gate 를 실행하는 것은 worker nested peer-call 이 아니다.

## 운영 원칙
- **MoA Flow C** default (큰 코드 변경): 양측 병렬 first-pass → synthesis → adversarial → 충돌 해결 → mutation 1 명만 → max 3 round
- **MoA Flow D** default (조사·디버깅 가설): 양측 병렬 first-pass → synthesis → adversarial → Claim Ledger
- **6 항목 의무** (모든 Codex/Claude worker 프롬프트): success criteria, NEVER 영역, validation cmd, files+lines, 대안 2 개, tests-first
- **Mutation = 1 owner**, lock transfer 는 명시적 protocol (DESIGN.md "Same-file sequential edit")
- **사용자 개입 기본 2 곳**: 작업 입력 + 최종 patch apply confirm (PLAN.md § 0.5 Automation Contract). **예외**: T13 의 DestructiveNetwork slash 명령 (`/메인동기화` 4-5 step, `/백로그` 3 step) 은 step-gate confirm 우선. PLAN.md § 0.6.
- **Review gate invariant**: PR 생성 전, PR 머지 전, 병행통합 merge 전, main 적용 전에는 lead/orchestrator-owned `CodexAdversarialXHigh` review gate 가 항상 필요하다. MoA Desktop 앱 안에서는 source=orchestrator 의 review profile 이 실행하고, Codex Desktop 수동 개발 흐름에서는 사용자가 명시한 lead PowerShell `codex exec --ephemeral --sandbox read-only ... --output-last-message <repo>/.moa-desktop/reviews/<stamp>.md` 별도 review 가 같은 gate 증거가 될 수 있다. WindowsApps `pwsh.exe -Command ... rejected: blocked by policy` 로 formal read-only review 가 `ENV_BLOCKED` 를 내면 그 attempt 는 `ReviewRunError` 로 저장한다. 그 뒤 lead/manual session 은 controlled-bypass review gate 를 1회 실행할 수 있다: `--dangerously-bypass-approvals-and-sandbox` + READ-ONLY prompt + edit/create/delete/stage/commit/push/format/GitHub mutation 금지 + `--output-last-message` + before/after `git status` + review-caused mutation 0건 + 정확히 1개인 `Verdict: Clean` line 이 모두 필요하다. 이 경우 `command_source_adapter=codex-desktop-lead-powershell-controlled-bypass` 와 failed read-only attempt path 를 limitations/evidence 에 남긴다. controlled-bypass selector 는 output file 존재, 정확히 1개인 `Verdict: ReviewRunError` line, `ENV_BLOCKED`, `WindowsApps`, `pwsh.exe`, concrete policy-block text 가 모두 있을 때만 열리며 generic `blocked by policy` 단독 matching, arbitrary nonzero exit, missing output, timeout, model/tool failure 는 fallback 이 아니라 fail-closed 다. controlled-bypass 는 worker mutation 권한이 아니며, worker prompt 내부 peer 직접 호출은 계속 금지다. 현재 세션 자기검토는 gate evidence 가 아니다. 이 profile/prompt 는 `/codex:adversarial-review --effort xhigh` 와 동등한 의도와 강도여야 하며, `reasoning_effort=xhigh`, prompt template version/hash, model/profile id, command/source adapter, source output path 를 감사 가능하게 남긴다. PrimaryRole=Codex 여도 Codex review 는 빠지지 않으며, Claude review 는 추가 대칭 검토일 뿐 대체물이 아니다. ReviewRunRecord 는 journal/lane result/ResumePacket/PR 또는 merge 보고에 저장한다. profile 실행 불가, `Concern`/`Block`, 또는 audit field 누락은 `ReviewRunError`/fail-closed 로 취급한다.

## Codex preflight (본 파일 존재 자체가 preflight 통과 조건)
Codex MCP 호출 시 워크스페이스 root 에 본 파일이 있어야 NEVER 영역 인식이 시작된다. 본 파일을 삭제하지 말 것.
