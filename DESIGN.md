# MoA Desktop — Design Document (User-authored, 2026-05-06)

## Vision
Claude Code + Codex CLI를 sibling worker로 실행하는 **별도 데스크탑 앱**. 액자형 호출(Codex→Claude→Codex 또는 그 반대) 회피. 사용자는 한 번 명령, 앱이 MoA 흐름(first-pass 병렬 → synthesis → adversarial → mutation 1명 → review → verification)을 자동 진행.

## Architecture (sibling, not nested)
```
MoA Desktop (parent orchestrator)
  ├─ Claude Worker (claude -p ...)
  ├─ Codex Worker (codex exec ...)
  ├─ Orchestrator (Flow A/B/C/D 판단)
  ├─ Lock Manager (mutation owner: none|claude|codex)
  ├─ Diff Gate (순차 수정 시 최신 재확인)
  ├─ Consensus Engine (5칸 schema)
  └─ UI (Codex Desktop 같은 실무 화면)
```

**금지 구조**: Codex→Claude→Codex, Claude→Codex→Claude, Worker가 peer 호출.

## Flows

### Flow A — Trivial (Claude author)
1. Claude 직접 Edit/Write
2. 사용자 review 명시 또는 의심 시 Codex review
3. blocker → 적용 또는 escalate

### Flow B — Trivial mechanical (Codex author)
Windows shell, sandbox, 대량 mechanical edit. Codex가 작성, Claude가 spot review.

### Flow C — Non-trivial code (MoA)
1. 양측 병렬 read-only first-pass (diagnosis + approach + risks + alternatives + validation plan)
2. Synthesis (5칸 schema, execution owner 담당)
3. Adversarial (비-종합자가 비판)
4. 충돌 해결 (사실/구현/risk/아키텍처)
5. mutation owner 1명만 수정, 다른 쪽 review
6. 최대 3라운드

### Flow D — Research/debug (MoA)
1. 양측 병렬 read-only first-pass (claims + citations + confidence + applicability)
2. Synthesis (verified / Codex-only / Claude-only / disagreement / open)
3. Adversarial (반례·맹점 요청)
4. Claim Ledger (max 5)

## Same-file sequential edit (허용)
조건:
1. 첫 수정자 끝 → git diff 저장
2. 두 번째 AI 최신 파일 re-read
3. review-only 또는 owner transfer 명시
4. transfer 시 lock owner 변경
5. 변경 후 테스트 재실행

자동 중단 조건:
- Worker 출력에 peer 호출 흔적
- 둘 다 동시 mutation 시도
- 비밀 파일 접근 시도
- lock 없이 수정 시도
- 테스트 실패 후 계속 진행 시도
- 3라운드 초과

## Tech Stack
- Tauri + React + TypeScript
- Windows 우선
- 로컬 JSON 저장 (`~/.moa-desktop/sessions, logs, prompts, results, settings.json`)
- 비밀 파일 절대 저장 금지 (API key, cookies, .env, pipeline_config.json)

## UI
- 첫 화면 = workbench (랜딩 페이지 금지)
- **상단: 프로젝트 탭 바** (Codex/Claude Desktop 패턴 — 한 앱 인스턴스에 N 프로젝트 동시 활성, 탭 전환으로 컨텍스트 스왑)
- 왼쪽: 현재 탭 프로젝트의 세션/lane 리스트
- 가운데: 작업 입력 + 자동 실행 상태 (preflight → first-pass → synthesis → adversarial → mutation → verification → final)
- 오른쪽: 5칸 synthesis + 충돌 + 결론
- 아래: Claude/Codex lane 별 로그 타임라인
- 좁은 폭 지원, 카드 중첩 금지
- 프로젝트 탭별 lock/journal/telemetry 격리, 같은 repo 가 두 탭에 중복 열림 금지 (repo-path scoped lock)

## Worker Guard (system-append)

### Claude Worker:
- You are Claude Worker in MoA Desktop. Independent peer, not orchestrator.
- Do NOT call Codex. Do NOT use /codex:rescue, /codex:review, /codex:adversarial-review.
- Do NOT call Codex MCP, Claude MCP, claude_code_peer, TeamCreate, Agent, AI-peer routing.
- If peer needed, write `NEED_PEER_REVIEW` and stop.

### Codex Worker:
- You are Codex Worker in MoA Desktop. Independent peer, not orchestrator.
- Do NOT call Claude. Do NOT run claude.
- Do NOT run nested Codex (`codex exec`, `/codex:*`) from inside the worker.
- Do NOT call Claude MCP, Codex MCP, claude_code_peer, TeamCreate, Agent.
- If peer needed, write `NEED_PEER_REVIEW` and stop.

### Review Gate Owner:
- Mandatory `CodexAdversarialXHigh` review gates are lead/orchestrator-owned, never worker-owned. In the app this means source=orchestrator ReviewProfile. During Codex Desktop manual development, a user-requested lead PowerShell `codex exec --ephemeral --sandbox read-only ... --output-last-message <repo>/.moa-desktop/reviews/<stamp>.md` review is allowed and must be recorded as review evidence. `--dangerously-bypass-approvals-and-sandbox` is mutation-in-worktree only, not review-gate evidence.

## WorkerCommandGuard + Output Scanner

`WorkerCommandGuard` / `SpawnGuard` is the primary defense: source=worker process/tool execution is blocked before spawn if argv or shell text attempts peer recursion. Output scanner is the second defense for streamed text and final UI display.

### Block list
- `/codex:`, `codex exec`, `claude -p`, `claude_code_peer`, `Claude MCP`, `Codex MCP`
- `TeamCreate`, `Agent`, `call Codex`, `call Claude`, `ask another AI`, `run another agent`

## Claude Command Adapter

### Research / first-pass (read-only):
```
claude -p <prompt>
  --model opus
  --permission-mode default
  --allowedTools Read Bash(git status:*) Bash(git log:*) Bash(git diff:*) Bash(rg:*) WebSearch WebFetch
  --disallowedTools Edit Write NotebookEdit
  --append-system-prompt <Claude Worker guard>
  --max-turns 20
  --output-format json
```

### Mutation owner:
```
claude -p <prompt>
  --model opus
  --permission-mode acceptEdits
  --allowedTools Read Edit Write Bash(git status:*) Bash(git diff:*) Bash(npm test:*) Bash(pytest:*) WebSearch WebFetch
  --append-system-prompt <Claude Worker guard>
  --max-turns 30
  --output-format json
```

## Codex Command Adapter (configurable) — codex-cli 0.128.0 검증
Default template (read-only first-pass):
```
codex exec --ephemeral -c model_reasoning_effort='high' -c 'web_search="live"' \
  --sandbox read-only --json --cd <repo> <prompt>
```
Mutation template (lock owner=codex 일 때만):
```
codex exec --ephemeral -c model_reasoning_effort='high' -c 'web_search="live"' \
  --dangerously-bypass-approvals-and-sandbox --json --cd <worktree> <prompt>
```
- ⚠️ Windows S2 finding #5: `--sandbox workspace-write` is BROKEN on Windows (codex command-policy rejects PowerShell writes with `blocked by policy`). Mutation MUST use `--dangerously-bypass-approvals-and-sandbox` inside an **isolated repo-local worktree** (`<repo>/.moa-desktop/worktrees/<session-id>/`) — orchestrator T4 lock + git worktree + adapter worktree-path guard are the safety boundary, not the codex sandbox. Source of truth: `src-tauri/src/adapters/codex.rs::mutation_argv`.
- ❌ `--reasoning-effort` 직접 flag 는 unsupported (`error: unexpected argument`)
- 6 항목 의무: success criteria, NEVER 영역, validation cmds, files+lines, alternatives 2개, tests-first
- mutation: lock owner=codex 일 때만, 최신 re-read + diff checkpoint 후
- 비차단 경고 (기록만): chatgpt.com 403, PowerShell shell snapshot 미지원, MCP client `program not found`

## Settings (~/.moa-desktop/settings.json)
```json
{
  "moaDefault": true,
  "defaultFlow": "auto",
  "primaryRole": "claude",
  "_primaryRoleNote": "claude|codex. Codex 선택 시 synthesizer/default reviewer/Flow-C mutation owner 기본값까지 Codex 로 스왑하되 Claude 는 sibling worker 로 유지. 진행 중 세션에는 영향 없고 다음 세션부터 적용.",
  "maxRounds": 3,
  "mutationPolicy": "single-owner-with-transfer",
  "sameFileSequentialEdit": true,
  "claude": {"enabled": true, "command": "claude", "model": "opus", "maxTurns": 20, "allowWeb": true, "allowEditWhenOwner": true},
  "codex": {"enabled": true, "commandTemplate": "codex exec --ephemeral -c model_reasoning_effort='high' -c web_search=\"live\" --sandbox {{sandboxMode}} --json --cd {{cwd}} {{prompt}}", "_commandTemplateNote": "read-only first-pass 전용 template ({{sandboxMode}}=read-only). Mutation 은 별도 빌더 (`src-tauri/src/adapters/codex.rs::mutation_argv`) — `--sandbox` 제거 + `--dangerously-bypass-approvals-and-sandbox` 추가, isolated worktree 안 (Windows S2 #5).", "allowWeb": true, "allowEditWhenOwner": true},
  "safety": {"blockPeerRecursion": true, "blockSecrets": true, "requireGitCheckpoint": true, "stopOnTestFailure": true}
}
```

## First-version scope (in)
- Tauri 앱
- Claude/Codex 자동 실행 adapter
- Flow A/B/C/D 자동 판단
- 병렬 first-pass
- 5칸 synthesis
- adversarial review
- mutation owner lock
- 순차 파일 수정
- git diff checkpoint
- 테스트 실행
- output guard
- max 3 round
- final report
- dry-run mode

## First-version scope (out)
- GitHub issue/PR 자동화
- 장기 백그라운드 큐
- cloud sync
- 무인 push, main 직접 push

## v1.5 scope (decomposer + parallel + integrator + multi-project)
- 작업 분해기 (`/병행티켓` 등가, T10): 큰 작업 → 충돌 없는 N 티켓 + paste-ready prompt + 의존성 그래프 + 머지 순서
- 다중 세션 병렬 실행 (T11): 한 프로젝트 안 N 티켓 동시 lane (worktree pool, 각 lane = 독립 T7-full orchestrator instance)
- 머지 통합기 (`/병행통합` 등가, T12): 머지 순서대로 patch apply → 충돌 시 stop + 한국어 보고
- **다중 프로젝트 동시** (단일 앱 인스턴스 + 최상위 프로젝트 탭, Codex/Claude Desktop 패턴): repo-scoped lock manager, per-project settings/journal/telemetry, 한 프로젝트는 한 탭에서만 활성

## Implementation steps (사용자 제시)
1. Tauri + React + TS scaffold
2. Static workbench UI
3. Session state model
4. Settings 화면
5. Flow A/B/C/D prompt builder
6. Claude command adapter
7. Codex command adapter
8. Output scanner
9. Synthesis engine (5칸)
10. Mutation lock manager
11. Git diff checkpoint helper
12. Verification command runner
13. Final report renderer
14. Dry-run mode
15. Local app verify

## Verification checklist
- 앱 시작 → 바로 workbench
- 세션 생성 가능
- Auto mode trivial vs MoA 분류
- Claude/Codex lane 독립 실행
- Claude prompt에 "Do not call Codex" 포함
- Codex prompt에 "Do not call Claude" 포함
- command guard가 worker-source `/codex:`, `codex exec`, `claude -p` 실행 시도를 spawn/tool execution 전에 차단
- command guard가 worker-source Claude/Codex MCP, `TeamCreate`, `Agent` 실행 시도를 spawn/tool execution 전에 차단
- output scanner가 `/codex:` Claude 출력에서 2차 차단
- output scanner가 `claude -p` Codex 출력에서 2차 차단
- first-pass = read-only
- mutation = 정확히 1 owner
- same-file 순차 = lock transfer 필요
- 5칸 synthesis 렌더링
- final Claim Ledger 렌더링
- secret 파일 미저장
- desktop/narrow width 작동
