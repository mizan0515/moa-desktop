# PROJECT-RULES — MoA Desktop

본 프로젝트의 권위·NEVER·운영 규칙은 분산 정의되어 있다. 본 파일은 **인덱스** 다.

## 권위 문서 (변경은 사용자 승인 필요)
- [DESIGN.md](DESIGN.md) — 비전·아키텍처·UI·Worker guard·Output scanner·Settings·scope in/out
- [PLAN.md](PLAN.md) — 구현 phase·F1-F6 critical fixes·Automation Contract·ticket dependency graph
- [synthesis.md](synthesis.md) — Claude+Codex first-pass 종합본 (참조용)
- [analysis-claude.md](analysis-claude.md) — Claude first-pass 원본 (참조용)
- [TICKETS/](TICKETS/) — 12 paste-ready 작업 티켓 (T0-T9 + TINTEGRATE) + v1.5 추가 예정 (T10/T11/T12)

## 글로벌 권위 (`~/.claude/`)
- `CODEX-MCP.md` — Codex MCP 페어 모드, MoA 흐름 C/D, 6 항목 프롬프트, Claim Ledger
- `KARPATHY.md` — coding guideline (think before, simplicity, surgical, goal-driven)
- `TOKEN-GUARD.md` — 토큰 임계 운영
- `RTK.md` — Rust Token Killer CLI

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
- **사용자 개입 2 곳만**: 작업 입력 + 최종 patch apply confirm (PLAN.md § 0.5 Automation Contract)

## Codex preflight (본 파일 존재 자체가 preflight 통과 조건)
Codex MCP 호출 시 워크스페이스 root 에 본 파일이 있어야 NEVER 영역 인식이 시작된다. 본 파일을 삭제하지 말 것.
