# AGENTS — MoA Desktop

본 프로젝트에서 AI agent (Claude Code worker, Codex CLI worker, MoA Desktop orchestrator) 가 작동할 때의 공통 규칙.

권위·NEVER·운영 세부 규칙은 [PROJECT-RULES.md](PROJECT-RULES.md) 참조.

## Agent 역할 정의

### Claude Worker
- 역할: MoA Desktop 의 sibling worker (orchestrator 가 아님)
- 호출 방식: `claude -p ... --output-format json` (DESIGN.md "Claude Command Adapter" 참조)
- read-only first-pass: `--allowedTools Read Bash(git:*) Bash(rg:*) WebSearch WebFetch`, `--disallowedTools Edit Write NotebookEdit`
- mutation owner: `--allowedTools Read Edit Write Bash(git:*) Bash(npm test:*) Bash(pytest:*) WebSearch WebFetch`
- 금지: `/codex:rescue`, `/codex:review`, `/codex:adversarial-review`, Codex MCP, `claude_code_peer`, `TeamCreate`, `Agent` (peer 호출 금지)
- 필요 시: `NEED_PEER_REVIEW` 출력 후 stop

### Codex Worker
- 역할: MoA Desktop 의 sibling worker (orchestrator 가 아님)
- 호출 방식: `codex exec --ephemeral -c model_reasoning_effort='high' -c web_search="live" --sandbox <mode> --json --cd <cwd> <prompt>` (`src-tauri/src/adapters/codex.rs::firstpass_argv` 가 source of truth)
- read-only first-pass: `--sandbox read-only`
- mutation owner: `--dangerously-bypass-approvals-and-sandbox` inside isolated worktree (Windows S2 finding #5: `workspace-write` is broken; bypass-in-worktree 가 source of truth — `src-tauri/src/adapters/codex.rs::mutation_argv`). 단 lock owner=codex 일 때만.
- 금지: `claude` 직접 호출, Claude MCP, `claude_code_peer`, `TeamCreate`, `Agent`
- 필요 시: `NEED_PEER_REVIEW` 출력 후 stop

### MoA Desktop Orchestrator
- 역할: parent — Worker 두 개를 spawn, synthesis, adversarial, lock manage, mutation gate, verification
- Flow A/B/C/D 자동 판단 (DESIGN.md "Flows" 절)
- 사용자 개입 2 곳: 작업 입력 + 최종 patch apply confirm
- **PrimaryRole** (T13 L1, settings.primaryRole): "claude" | "codex". Codex 선택 시 synthesizer / default reviewer / Flow-C mutation owner 모두 Codex. lock state machine 무영향. destructive-network 슬래시 (예: `/메인동기화`) 는 PrimaryRole 무관 단계별 confirm.

## 모든 Agent 의 6 항목 의무 (프롬프트 빌드 시)
1. Success criteria (검증 가능)
2. NEVER 영역 (PROJECT-RULES.md 의 NEVER + ticket 별 NEVER)
3. Validation cmd (사용자가 돌릴 1-2 줄)
4. Files + lines (Worker 가 grep 부터 시작 안 하도록)
5. 대안 2 개 + pros/cons + 선택 근거
6. Tests-first (버그 수정/기능 추가는 failing test 먼저)

## Output 보안
- 모든 Worker output 은 [DESIGN.md "Output Scanner"](DESIGN.md) block list 통과 의무
- Block 시: orchestrator 가 자동 재시도 또는 사용자 보고

## 현재 진행 상태 (latest)
- Branch: `feat/T1-scaffold` (T1 in-flight)
- Phase: 1 (walking skeleton)
- 최근 결정: v1.5 에 multi-project tabs + 다중 티켓 동시 실행 (T10/T11/T12) 추가, single-app-instance 모델 채택
- 자세한 ticket dependency 는 [PLAN.md](PLAN.md) § 2, § 3, § Phase 6
