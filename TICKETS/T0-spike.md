# T0 — De-risk spike (1주, 단독 실행)

## 새 Claude 창 만들기 가이드
1. 새 Claude Code 창 열기 (작업 폴더: `D:\moa-desktop`)
2. worktree 사용 안 함 (이 ticket 은 spike 결과만 만드는 거라 main repo 에서 직접 작업해도 OK)
3. 첫 입력으로 아래 프롬프트 통째 (Ctrl+V) 붙여넣기

---

## Paste-ready 프롬프트

```
[세션 부트]
- repo: D:\moa-desktop
- base branch: master
- 이 티켓의 권장 분기: feat/T0-spike
- GitHub 카드: 미등록 (이 프로젝트 remote 없음 — 1개씩 순차 실행 default)
- 권위 파일:
  - D:\moa-desktop\DESIGN.md (원본)
  - D:\moa-desktop\PLAN.md (§ F5 spike exit criteria)
  - D:\moa-desktop\synthesis.md (검토 합본)
  - ~/.claude/CODEX-MCP.md (§ 2.5+§ 2.6 페어 모드)
  - ~/.claude/KARPATHY.md
- 운영: MoA Flow D (research/spike) — § 2.6 템플릿 B 사용

[INDEPENDENT FIRST-PASS — read-only investigation, spike scripts 만 작성 허용]

## Goal
PLAN.md § F5 의 spike S1-S8 8개 모두 검증. 하나라도 FAIL 이면 alternative 제안. 통과 시 Phase 1 진입 GO sign.

## Success criteria (verifiable)
- [ ] S1: `claude -p` 를 Node 스크립트에서 spawn → stdout JSONL 라인 단위 수신 → kill 시 child 즉시 종료, 잔존 PID 0
- [ ] S2: 사용자 사전 검증 완료 (2026-05-06, codex-cli 0.128.0). 확정 명령: `codex exec --ephemeral -c model_reasoning_effort='high' -c tools.web_search=true --sandbox read-only --json --cd <repo> <prompt>`. 본 spike 에서 추가 검증: read-only 가 실제 mutation 시도를 차단하는지 (touch/echo > file 시도해서 deny 확인) + workspace-write 모드에서 mutation 정상
- [ ] S3: 두 Worker 병렬 실행 시 stdout 충돌 없음, 메모리/파일 race 없음
- [ ] S4: Claude `--disallowedTools "mcp__*"` 가 Codex MCP 호출 실제 차단하는지 (worker 가 `/codex:rescue` 시도하는 fake prompt 로 검증). 안 되면 plugin env 분리 fallback 정의
- [ ] S5: Tauri spawn 자식이 `~/.claude/credentials.json`, `~/.codex/auth.json` 자동 사용 — 별도 env 주입 없이
- [ ] S6: spawned `claude -p` 안에서 TOKEN-GUARD hook 발화 시 stderr 부모에 전달, HARD 차단 시 명확한 exit code
- [ ] S7: Tauri abort signal → Windows process tree kill (`taskkill /T /F` 등가) → 좀비 프로세스 0
- [ ] S8: 위 검증 결과로 settings 의 `claude.commandTemplate`, `codex.commandTemplate` 를 argv array 형태로 확정 (PowerShell quoting 회피)

## Files in scope (이 spike 가 만들 것)
- `spikes/S1-claude-spawn.md` (검증 노트)
- `spikes/S1-claude-spawn.{js,ps1}` (실험 스크립트)
- `spikes/S2-codex-spawn.{md,js,ps1}`
- `spikes/S3-parallel.{md,js}`
- `spikes/S4-disallowed-tools.{md,js}`
- `spikes/S5-auth-inherit.{md,js}`
- `spikes/S6-token-guard.{md,js}`
- `spikes/S7-cancellation.{md,ps1}`
- `spikes/S8-final-templates.md` (확정된 argv array)
- `spikes/RESULTS.md` (S1-S8 PASS/FAIL summary)

## NEVER 영역
- src-tauri/, src/, package.json, Cargo.toml, vite.config.ts (T1 영역)
- mockResponses/ (T8 영역)
- DESIGN.md, PLAN.md, synthesis.md, analysis-claude.md (참조만)
- 글로벌 ~/.claude/ 설정 변경 금지
- pipeline_config.json, secrets, .env

## Stop conditions
- spike 가 글로벌 설정 (~/.claude/settings.json) 변경 필요해질 때 → 사용자에 confirm
- S2 에서 codex CLI 가 실제로 다른 명령 형태 요구 (예: `codex exec` 대신 `codex run`) → 사용자에 보고 후 PLAN.md F1 업데이트 권고
- TOKEN-GUARD WARN 발화 시 안내 1회 후 작업 계속

## Deliverable (first-pass + spike 실행)
1. **Diagnosis**: Claude/Codex CLI 의 실제 가용 옵션, Tauri v2 spawn 패턴
2. **Approach**: 각 spike 별 검증 방법 (대안 2개 제시 후 justified path)
3. **Risks + rejected alts + validation plan**
4. **실험 실행** + spikes/RESULTS.md 작성
5. **Open questions** — 미해결 항목 (PLAN.md 의 § 5 open questions 답변 시도)

## Constraints
- spike scripts 는 spikes/ 폴더에만. 그 외 파일 mutation 금지.
- 6 항목 의무 (성공기준 / NEVER / 검증명령 / 파일·라인 / 대안 / tests-first)
- 모호하면 UNVERIFIED 명시

## Web search 의무
- "Claude Code CLI --disallowedTools mcp", "claude-code stream-json output format", "codex cli exec sandbox flag", "Tauri v2 Command argv array" 등 공식 docs 우선

[작업 완료 시]
- spikes/RESULTS.md 에 PASS/FAIL 매트릭스 + S8 확정 명령 템플릿
- commit: `feat(T0): spike S1-S8 검증 결과` (master 또는 feat/T0-spike 분기)
- push 금지
- 마지막 응답에 다음 보고:
  - PASS/FAIL 항목
  - PLAN.md § F1 (codex exec 명령) 확정 결과
  - Phase 1 진입 GO/NO-GO
  - Phase 1 진입 시 T1 worker 가 알아야 할 spike 결론 1줄
```
