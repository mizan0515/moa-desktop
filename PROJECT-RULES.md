# PROJECT-RULES — MoA Desktop

본 프로젝트의 권위·NEVER·운영 규칙은 분산 정의되어 있다. 본 파일은 **인덱스** 다.

## 권위 문서 (변경은 사용자 승인 필요)
- [DESIGN.md](DESIGN.md) — 비전·아키텍처·UI·Worker guard·Output scanner·Settings·scope in/out
- [PLAN.md](PLAN.md) — 구현 phase·F1-F6 critical fixes·Automation Contract·ticket dependency graph
- [synthesis.md](synthesis.md) — Claude+Codex first-pass 종합본 (참조용)
- [analysis-claude.md](analysis-claude.md) — Claude first-pass 원본 (참조용)
- [TICKETS/](TICKETS/) — 12 paste-ready 작업 티켓 (T0-T9 + TINTEGRATE) + v1.5 prequel (T13 EPIC) + v1.5 본체 (T10/T11/T12)

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

위 **15 파일** (Hot 6 + On-demand 9) 은 T13 L3 의 baseline pin 대상 — `~/.moa-desktop/policy/baseline-<date>.json` 에 hash 로 박힘. 각 파일은 `source_manifest[]` 의 entry 로 `{path, kind, sha256, size_bytes}` 형태 저장.

## NEVER 영역 (모든 worker 공통)
- 비밀 파일: `.env`, `*credentials*`, `*cookie*`, `pipeline_config.json`, `auth.json`
- 다른 ticket 의 owns 영역 (각 TICKETS/T*.md 의 "Files owned" 절 + "NEVER 영역" 절 참조)
- T0 spike 영역 (`spikes/`) — T0 외 ticket 은 read 만
- 사용자 작업 repo 의 main worktree 직접 수정 — 모든 mutation 은 git worktree 격리 (PLAN.md § F4)
- output 에 `claude -p`, `codex exec`, `/codex:`, `claude_code_peer`, `Claude MCP`, `Codex MCP`, `TeamCreate`, `Agent`, `call Codex`, `call Claude` 등 peer 호출 흔적 (DESIGN.md "Output Scanner" 절)

## 운영 원칙
- **MoA Flow C** default (큰 코드 변경): 양측 병렬 first-pass → synthesis → adversarial → 충돌 해결 → mutation 1 명만 → max 3 round
- **MoA Flow D** default (조사·디버깅 가설): 양측 병렬 first-pass → synthesis → adversarial → Claim Ledger
- **6 항목 의무** (모든 Codex/Claude worker 프롬프트): success criteria, NEVER 영역, validation cmd, files+lines, 대안 2 개, tests-first
- **Mutation = 1 owner**, lock transfer 는 명시적 protocol (DESIGN.md "Same-file sequential edit")
- **사용자 개입 기본 2 곳**: 작업 입력 + 최종 patch apply confirm (PLAN.md § 0.5 Automation Contract). **예외**: T13 의 DestructiveNetwork slash 명령 (`/메인동기화` 4-5 step, `/백로그` 3 step) 은 step-gate confirm 우선. PLAN.md § 0.6.

## Codex preflight (본 파일 존재 자체가 preflight 통과 조건)
Codex MCP 호출 시 워크스페이스 root 에 본 파일이 있어야 NEVER 영역 인식이 시작된다. 본 파일을 삭제하지 말 것.
