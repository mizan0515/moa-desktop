# T2 — Process runner (Tauri Command 추상)

## 새 Claude 창 만들기 가이드
T0 RESULTS.md 의 S8 확정 명령 템플릿 + T1 통과 후. worktree: T2-process.

---

```
[세션 부트]
- repo: D:\moa-desktop
- base branch: master (T1 머지 후)
- 권장 분기: feat/T2-process
- 권위: spikes/RESULTS.md (S1, S3, S7 검증 결과), PLAN.md (§ F2 argv array, § F6 error 분류), DESIGN.md, ~/.claude/CODEX-MCP.md
- 운영: MoA Flow C — § 2.6 템플릿 A

[INDEPENDENT FIRST-PASS — read-only]

## Goal
generic CLI process runner. argv array 로 spawn, stdout/stderr stream-line emit, abort signal 으로 process tree kill, timeout, exit code/error 분류.

## Success criteria
- [ ] `src-tauri/src/process/runner.rs` — `spawn(argv: Vec<String>, cwd: PathBuf, env: HashMap<String,String>) -> Handle`. Handle 은 stdout/stderr line stream + abort fn + wait_exit
- [ ] argv 는 list 그대로 전달, shell escaping 안 함 (PowerShell quoting 사고 회피)
- [ ] Windows process tree kill (`taskkill /T /F /PID <pid>` 또는 win32 JobObject) — 좀비 0 검증
- [ ] timeout 도달 시 abort + 명확한 error
- [ ] error 분류 enum: `cli-missing | auth-expired | quota | network | sandbox-denied | malformed-json | timeout | oom | killed | test-fail` (PLAN.md F6) — exit code + stderr 분석으로 매핑
- [ ] T8 mock runner 와 동일 trait `ProcessRunner` 으로 swap 가능
- [ ] unit test: fake CLI (Node script 으로 1000 line 출력) 로 streaming, mid-cancel, timeout, 좀비 검사

## Files owned
- `src-tauri/src/process/{mod.rs,runner.rs,trait.rs,errors.rs,kill.rs}`
- `src-tauri/tests/process_runner_test.rs`
- `src/lib/processEvents.ts` (frontend 가 받을 event type)

## Read-only
- spikes/RESULTS.md, T1 의 src-tauri 구조, T8 mock runner (trait 합의)

## NEVER 영역
- src-tauri/src/adapters/* (T5a/T5b — runner 위에 builder)
- src-tauri/src/mock/* body (T8)
- src-tauri/src/orchestrator/* (T7)

## Stop conditions
- spike S7 (cancellation) 가 NO-GO 였다 → 작업 중단, alternative 사용자 보고
- Tauri v2 Command API 가 stream emit 부족 → sidecar mode 검토 후 사용자 보고

## Deliverable (first-pass)
1. Diagnosis: Tauri v2 Command API 의 streaming/cancel 동작 (spike S1 결과)
2. Approach: tauri-plugin-shell vs std::process::Command + tokio::process (대안 2개)
3. Risks
4. error 분류 매핑 표 (exit code, stderr substring → enum)
5. Open questions

## Constraints
- argv array 의무 (string concat 금지)
- 6 항목 의무
- 비밀 env 로깅 금지 (PROJECT-RULES 부재 — 대신 PLAN.md F6 의 redaction 정책)

[작업 완료 시]
- commit: `feat(T2): process runner + tree-kill + error classification`
- 보고: trait `ProcessRunner` 시그니처 (T5a/T5b 가 implement), error enum 최종, mock runner (T8) 와 swap 검증
```
