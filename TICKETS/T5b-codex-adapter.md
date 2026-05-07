# T5b — Codex adapter (`-s read-only` first-pass + mutation)

## 새 Claude 창 만들기 가이드
T2 통과 후. T5a 와 병렬 가능 (단 mod.rs coordinate). worktree: T5b-codex-adapter.

---

```
[세션 부트]
- repo: D:\moa-desktop
- base branch: master (T2 머지 후. T5a 와 분기 분리)
- 권장 분기: feat/T5b-codex
- 권위: spikes/RESULTS.md (S2, S8 — codex exec 확정 옵션), DESIGN.md (Codex Worker guard), PLAN.md (§ F1, F3, F4)
- 운영: MoA Flow C — § 2.6 템플릿 A

[INDEPENDENT FIRST-PASS — read-only]

## Goal
Codex CLI 전용 adapter. T2 의 ProcessRunner 위에서 `codex exec ...` argv 빌드 + Worker guard prompt 결합 + JSON output 파싱.

## Success criteria
- [ ] `src-tauri/src/adapters/codex.rs` — `CodexAdapter::firstpass(task, files, cwd) -> stream` and `CodexAdapter::mutation(task, worktree_path) -> stream`
- [ ] argv 빌드 (사용자 검증 완료, codex-cli 0.128.0):
  - read-only first-pass: `codex exec --ephemeral -c model_reasoning_effort='high' -c tools.web_search=true --sandbox read-only --json --cd <cwd> <prompt>`
  - mutation: `--sandbox` 제거 + `--dangerously-bypass-approvals-and-sandbox` 추가 (isolated worktree 안, Windows S2 #5: `workspace-write` is broken on Windows). Source of truth: `src-tauri/src/adapters/codex.rs::mutation_argv`.
  - ❌ `--reasoning-effort` 직접 flag 사용 금지 (unsupported)
- [ ] 비차단 경고 무시 (chatgpt.com 403, PowerShell shell snapshot, MCP client program not found) — 기록만
- [ ] Worker guard 는 prompt 안 prefix 로 주입 (Codex 는 system-prompt flag 부재) — `prompts/workers/codex_guard.txt` 로드 후 prefix
- [ ] CODEX_HOME 등 env 명시 전달 (Tauri spawn env)
- [ ] stream-json line 파싱
- [ ] integration test: fake codex binary 로 호출 → expected event sequence

## Files owned
- `src-tauri/src/adapters/codex.rs` (mod.rs 는 T5a 와 공유)
- `prompts/workers/codex_guard.txt`
- `prompts/workers/codex_firstpass_template.txt`
- `prompts/workers/codex_mutation_template.txt`
- `src-tauri/tests/adapter_codex_test.rs`

## ⚠️ 공유 파일 주의
- `src-tauri/src/adapters/mod.rs` — T5a 가 먼저 머지하면 `pub mod codex;` 만 추가. T5a 의 `pub mod claude;` 라인 건드리지 X.

## Read-only
- spikes/RESULTS.md (S2 — `codex exec --help` 확정 옵션, --reasoning-effort 가용 여부)
- T2 ProcessRunner trait
- DESIGN.md, PLAN.md

## NEVER 영역
- src-tauri/src/adapters/claude.rs (T5a)
- src-tauri/src/process/* body (T2)
- src-tauri/src/orchestrator/* (T7)
- ~/.codex/auth.json (read 만, write 절대 X)

## Stop conditions
- spike S2 가 NO-GO 또는 `codex exec` 명령이 다른 형태 → 즉시 보고
- read-only sandbox 가 spike 에서 mutation 시도를 진짜 차단 못함 → 사용자 보고 후 fallback

## Deliverable (first-pass)
1. Diagnosis: spike S2 결과 정리
2. Approach: prompt prefix 방식 (Codex 는 system-prompt 분리 flag 없음) (대안 2개)
3. Risks (Codex CLI 가 자동 update 로 옵션 변경 가능)
4. JSON event 매핑 (Codex stream-json shape → process_event)
5. Open questions

## Constraints
- argv array 의무
- guard text commit, secrets X
- 6 항목 의무

[작업 완료 시 — 무조건 이 순서로]
1. commit: `feat(T5b): codex adapter (firstpass + mutation)` (본문에 `Closes #8` 포함, push 금지)
2. **GitHub 카드 완료 처리 — 잊지 말고 무조건 실행** (안 하면 칠판 https://github.com/users/mizan0515/projects 에 status:doing 으로 남아 다른 세션이 또 잡을 수 있음):
   ```
   node ~/.claude/scripts/gh-tickets.mjs complete D:\moa-desktop 8
   ```
   - 출력에 `COMPLETED=8` 또는 `ALREADY_CLOSED=8` 가 보여야 OK.
   - 실패 시 사용자 보고 + STOP. gh 인증 오류면 `gh auth refresh -s project,read:project` 안내.
3. 보고: argv shape, T7 가 호출할 API, T5a 와 머지 순서 협의 (mod.rs), **GitHub 카드 close 결과 1줄**.
```
