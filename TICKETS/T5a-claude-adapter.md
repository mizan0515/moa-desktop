# T5a — Claude adapter (read-only first-pass + mutation owner)

## 새 Claude 창 만들기 가이드
T2 통과 후. worktree: T5a-claude-adapter.

---

```
[세션 부트]
- repo: D:\moa-desktop
- base branch: master (T2 머지 후)
- 권장 분기: feat/T5a-claude
- 권위: spikes/RESULTS.md (S1, S4, S8 명령 템플릿), DESIGN.md (Claude Worker guard), PLAN.md (§ F1, F3, F4), ~/.claude/CODEX-MCP.md
- 운영: MoA Flow C — § 2.6 템플릿 A

[INDEPENDENT FIRST-PASS — read-only]

## Goal
Claude CLI 전용 adapter. T2 의 ProcessRunner 위에서 `claude -p ...` argv 빌드 + Worker guard prompt 결합 + JSON output 파싱.

## Success criteria
- [ ] `src-tauri/src/adapters/claude.rs` — `ClaudeAdapter::firstpass(task, files, cwd) -> stream` and `ClaudeAdapter::mutation(task, worktree_path) -> stream`
- [ ] argv 빌드: `claude -p <prompt> --model <m> --permission-mode <p> --allowedTools <list> --disallowedTools <list> --append-system-prompt <guard> --max-turns N --output-format stream-json` — 모두 별 element
- [ ] Worker guard 텍스트는 `prompts/workers/claude_guard.txt` 에서 로드 (DESIGN.md 의 Claude Worker 가드 섹션)
- [ ] disallowedTools 에 `Edit, Write, NotebookEdit, mcp__*` 명시 (read-only 모드) — wildcard 동작이 spike S4 에서 검증됐다면 그대로, 안 됐으면 plugin env 분리 fallback
- [ ] mutation 모드는 worktree path 를 cwd 로, allowedTools 에 Edit/Write 추가
- [ ] stream-json line 파싱 → process_event::* 로 emit (T2 trait)
- [ ] integration test: fake claude binary (echo JSON line script) 로 firstpass + mutation 호출 → expected event sequence

## Files owned
- `src-tauri/src/adapters/{mod.rs (T5b 와 공유 — coordinate),claude.rs}`
- `prompts/workers/claude_guard.txt`
- `prompts/workers/claude_firstpass_template.txt`
- `prompts/workers/claude_mutation_template.txt`
- `src-tauri/tests/adapter_claude_test.rs`

## ⚠️ 공유 파일 주의
- `src-tauri/src/adapters/mod.rs` 는 T5a + T5b 양쪽이 owner. 분기 머지 시 conflict 가능.
  → 해결: T5a 가 먼저 머지하면 `pub mod claude;` 만 추가. T5b 가 나중에 `pub mod codex;` 추가. mod.rs 의 그 외 라인은 둘 다 손대지 X.

## Read-only
- spikes/RESULTS.md (확정된 명령 템플릿)
- T2 ProcessRunner trait
- DESIGN.md, PLAN.md

## NEVER 영역
- src-tauri/src/adapters/codex.rs (T5b)
- src-tauri/src/process/* body (T2)
- src-tauri/src/orchestrator/* (T7)

## Stop conditions
- spike S4 가 NO-GO (disallowedTools 로 mcp 차단 안 됨) → fallback 명시 후 사용자 보고
- Worker guard 가 prose only — sandbox 강화 안 되면 즉시 보고

## Deliverable (first-pass)
1. Diagnosis: 확정된 argv 구조 (spike S8)
2. Approach: prompt 빌드 — template substitution 방식 (대안 2개)
3. Risks
4. JSON event 매핑 (Claude stream-json shape → process_event)
5. Open questions

## Constraints
- argv array 의무
- guard text 는 commit 하지만 secrets 는 X
- 6 항목 의무

[작업 완료 시 — 무조건 이 순서로]
1. commit: `feat(T5a): claude adapter (firstpass + mutation)` (본문에 `Closes #7` 포함, push 금지)
2. **GitHub 카드 완료 처리 — 잊지 말고 무조건 실행** (안 하면 칠판 https://github.com/users/mizan0515/projects 에 status:doing 으로 남아 다른 세션이 또 잡을 수 있음):
   ```
   node ~/.claude/scripts/gh-tickets.mjs complete D:\moa-desktop 7
   ```
   - 출력에 `COMPLETED=7` 또는 `ALREADY_CLOSED=7` 가 보여야 OK.
   - 실패 시 사용자 보고 + STOP. gh 인증 오류면 `gh auth refresh -s project,read:project` 안내.
3. 보고: argv shape, T7 가 호출할 API, T5b 와 공유할 mod.rs 행 (충돌 회피), **GitHub 카드 close 결과 1줄**.
```
