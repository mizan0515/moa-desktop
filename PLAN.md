# MoA Desktop — Final Implementation Plan
Date: 2026-05-06
Source: `DESIGN.md` × `analysis-claude.md` × Codex first-pass × adversarial review
Status: Ready for ticket dispatch after critical-fix confirmation

## 0. Critical fixes from adversarial review (must apply before ship)

### F1. `codex exec` 명령 템플릿 — 확정 (2026-05-06 사용자 검증 완료, codex-cli 0.128.0)
- ❌ **불가**: `codex exec --reasoning-effort high ...` — `error: unexpected argument '--reasoning-effort' found`
- ✅ **확정 (read-only first-pass)**: `codex exec --ephemeral -c model_reasoning_effort='high' --sandbox read-only --cd <repo> <prompt>`
- ✅ **확정 (mutation owner)**: `codex exec --ephemeral -c model_reasoning_effort='high' --sandbox workspace-write --cd <worktree> <prompt>`
- ✅ JSON streaming: `--json` 추가 (라인 단위 emit)
- ✅ Web search 사용: config.toml 에서 `[tools.web_search] enabled = true` 또는 `-c tools.web_search=true`
- 비차단 경고 (T0 에서 기록만, 차단 X): `chatgpt.com` 플러그인/analytics 403, PowerShell shell snapshot 미지원, MCP client `program not found`

### F2. 명령 빌드는 argv array, shell string 금지
- Tauri v2 Command API: `Command.create(program, [args...])` 로 호출. PowerShell quoting 회피.
- `--allowedTools "Bash(git status:*)"` 같은 인자는 한 element 로 전달, 따옴표 내부 escape 신경 X.

### F3. Sandbox 는 3중 (prose 가드만 믿지 X)
- Claude Worker: `--allowedTools` allowlist + `--disallowedTools` deny list (Edit, Write, NotebookEdit 명시 + `mcp__*` wildcard 시도)
- Codex Worker: `-s read-only` (CLI sandbox flag)
- Plugin/MCP env 분리: Worker spawn 시 `ENABLE_CLAUDEAI_MCP_SERVERS=false`, `CODEX_HOME` 별도 prof
- Output scanner = warning level only (security 가드 X)

### F4. Mutation = isolated worktree/patch flow
- Worker 가 직접 source 수정 X
- 절차: app 이 `git worktree add` 또는 임시 디렉토리에 base 복사 → Worker 가 그 안에서 수정 → app 이 patch 추출 → diff/test 검증 → patch apply (또는 reject + cleanup)
- Rollback 무료, multi-step 추적 가능.

### F5. T0 spike 1주 — exit criteria hard
| # | 검증 항목 | 통과 조건 |
|---|---|---|
| S1 | `claude -p` Tauri spawn + stream-json | stdout JSONL 라인 단위 받음, kill 시 child 즉시 종료, 잔존 0 |
| S2 | `codex exec -s read-only --json --cd <repo>` Tauri spawn | 동일, `-s read-only` 가 mutation 시도 차단 검증 |
| S3 | 두 Worker 병렬 실행 | stdout 충돌 없음, 메모리/파일 race 없음 |
| S4 | Claude `--disallowedTools "mcp__*"` 검증 | Worker 가 `/codex:rescue` 호출 시도 → 차단 확인. 안 되면 plugin env 분리로 fallback |
| S5 | 인증/env 상속 | Tauri 자식이 `~/.claude/credentials.json`, `~/.codex/auth.json` 자동 사용 |
| S6 | TOKEN-GUARD 발화 동작 | spawned `claude -p` 안에서 hook 발화 시 stderr 부모에 전달, HARD 차단 시 명확한 exit code |
| S7 | Cancellation | Tauri abort signal → Windows process tree kill (`taskkill /T /F` 등가) → 0 좀비 |
| S8 | 명령 templates 확정 | argv array 형태로 빌드, settings 에 저장, smoke test 통과 |

### F6. 누락된 운영 항목 추가
- **Recovery journal**: session 별 JSONL journal (phase, owner, PID, base hashes, patch path) — crash 후 startup reconcile
- **Multi-instance**: 단일 instance app (Tauri single-instance plugin) 또는 repo-scoped OS file lock
- **Error 분류**: `cli-missing | auth-expired | quota | network | sandbox-denied | malformed-json | timeout | oom | killed | test-fail` typed errors → UI 에서 사용자 행동 가능한 메시지
- **Retry tracking**: LLM 비결정성 — 재시도는 새 attempt 로 기록 (prompt hash, argv, model, CLI version, cwd, env allowlist, raw output, attempt#)
- **Prompt cache awareness**: 매 `claude -p` 가 fresh session = cache reuse 0. 비용 multiplier 명시. 사용자에 표시.
- **Version pinning**: session metadata 에 CLI/plugin version 기록, drift warning UI
- **Concurrent log writes**: per-worker append-only JSONL + orchestrator single-writer index + monotonic seq ID

## 0.5 Automation Contract — 사용자 한 번 명령 → 끝까지 자동

**원칙**: 사용자는 작업 텍스트 1번 입력 + 마지막 apply confirm 1번 클릭. 그 사이 모든 결정은 orchestrator (T7-full) 가 자동. 관리자 중재는 **2개 trigger 에서만**: (1) 아키텍처 tradeoff 충돌, (2) max 3 round 초과.

### 자동 실행 sequence (Flow C 기준 — 큰 코드 변경)
1. preflight (CLI 검증, auth, version, sandbox) — 자동
2. **first-pass × 2 병렬** (T5a Claude read-only + T5b Codex read-only, 양쪽 web search + deep thinking 활성) — 자동
3. **synthesis** (T3 deterministic merge, no LLM, 5칸 schema) — 자동
4. **adversarial round** (orchestrator 가 Codex Worker 새로 spawn, synthesis embed) — 자동
5. **충돌 해결 protocol** 자동 적용:
   - 사실 충돌 → live verify (T2 가 git/test 호출) — 자동
   - 구현 충돌 → blast radius/rollback/validation 비교 후 자동 결정
   - risk 충돌 → cheapest decisive test 실행 — 자동
   - **아키텍처 tradeoff → 사용자 escalation (이때만 멈춤)**
6. **mutation owner 자동 선택** (mechanical/Windows shell → Codex, semantic refactor → Claude default. orchestrator heuristic + 사용자 override 가능)
7. T4 lock acquire → git worktree 생성 → mutation Worker 실행 (Claude `Edit/Write` 또는 Codex `--sandbox workspace-write`) → patch 추출
8. **Same-file 순차 편집 자동 처리**:
   - 첫 Worker 종료 → file hash snapshot (T4)
   - second Worker 가 review-only 모드로 최신 파일 re-read
   - second Worker review 에 추가 mutation 제안 ≥ 1개 + 동일 파일 영향 → orchestrator 가 lock transfer 자동 결정
   - second Worker 가 mutation owner 로 전환 (T4 lock state: `acquired(claude) → transferring → acquired(codex)`) → 추가 수정
   - max 1회 transfer (overflow 시 사용자 confirm)
9. 두 Worker mutation 합쳐 final patch
10. verification cmd 자동 실행 (settings 의 project-specific cmd, 또는 default `npm test` 등)
11. **final report (Claim Ledger) 사용자에 표시**
12. 사용자 confirm 클릭 → main repo 에 patch apply
13. lock release + journal flush + 세션 left panel 에 archived

### 사용자 개입 지점 (총 2 곳)
- **시작**: 작업 텍스트 입력 + Run 클릭 (1회)
- **끝**: final report 보고 patch apply 또는 reject (1회)

### Manager intervention 필요 시 (자동 멈춤 + UI 명시)
- 아키텍처 tradeoff 충돌 (Claude/Codex 가 근본적으로 다른 방향)
- max 3 round 초과 (수렴 실패)
- cost cap 도달 (T9 — default $10/session)
- preflight 실패 (CLI 미설치, auth 만료, sandbox NO-GO)
- multi-instance lock 거부

### "양측 모두 web search · deep thinking · file edit" 보장
- Claude Worker: read-only 모드 — `--allowedTools "Read" "WebSearch" "WebFetch" "Bash(git:*)" "Bash(rg:*)"`. Mutation 모드 — `+ "Edit" "Write"`. Deep thinking — prompt 에 "think hard" 또는 `MAX_THINKING_TOKENS=10000` env.
- Codex Worker: read-only — `--sandbox read-only -c model_reasoning_effort='high' -c tools.web_search=true`. Mutation — `--sandbox workspace-write` 동일 reasoning + web_search.

## 1. Implementation phases (value-incremental, 5-7주 1인)

### Phase 0 — Spike (T0, 5-7일)
F5 의 S1-S8 검증. 통과 못하면 Phase 1 진입 금지.

### Phase 1 — Walking skeleton (T1, T8, T6-render, T7-thin → 1-2주)
**목표**: 사용자가 더미 데이터로 MoA 흐름 전체를 화면에서 본다.
- T1: Tauri shell + workbench static UI
- T8: Mock mode + canned Claude/Codex JSON responses
- T6: Synthesis view + Claim Ledger UI (mock JSON 으로 렌더)
- T7-thin: dry-run orchestrator (mock workers 호출 → mock synthesis → mock adversarial → mock final report)

**Demo milestone (Phase 1 완료 시점)**: 사용자가 앱 실행 → 프로젝트 선택 → 작업 입력 → mock 으로 Flow C 시뮬레이션 → 5칸 + Claim Ledger 결과 본다. **No real AI call.** 이게 첫 가시적 가치.

### Phase 2 — Real Workers (T2, T5a, T5b → 1.5주)
- T2: Process runner (스트리밍, 취소, 타임아웃)
- T5a: Claude adapter (read-only first-pass + mutation owner 모드)
- T5b: Codex adapter (`-s read-only` first-pass + mutation 모드)
- 통합: Phase 1 의 mock 자리를 실제 Worker 로 교체

**Demo milestone**: 실제 Claude+Codex 가 read-only first-pass 실행, 결과 5칸 표 렌더. **No mutation yet.**

### Phase 3 — Mutation (T3, T4, T7-full → 1-1.5주)
- T3: Synthesis engine (deterministic JSON merge)
- T4: Safety/Git/Lock — worktree-isolated patch flow + recovery journal + multi-instance lock
- T7-full: Orchestrator state machine — mutation owner 선택, lock transfer, max 3 round, conflict protocol

**Demo milestone**: 사용자가 작업 입력 → MoA flow → 한쪽 mutation owner 가 worktree 에 수정 → app 이 patch 검증 → 사용자 confirm → main repo apply. End-to-end happy path.

### Phase 4 — Hardening (T9, error UX, version pinning → 1주)
- T9: Cost telemetry (token, cache_read, $) + cancellation
- Error 분류 + UI 표시
- Version drift warning
- Verification cmd 일반화 (npm/pytest 외 stack)
- `--output-format stream-json` 채택

### Phase 5 — Polish & verify (1주)
- DESIGN.md 의 verification checklist + adversarial F6 추가 항목 모두 통과
- UI wireframe 사용자 confirm
- README, dry-run 데모 영상 또는 GIF

## 2. Ticket dependency graph (value-incremental)

```
T0 (spike) ── 통과 후 Phase 1 진입
   │
   ├─ T1 (shell/UI)        ┐
   ├─ T8 (mock + responses) │
   ├─ T6 (synthesis view)   ├─ Phase 1 walking skeleton (병렬 가능)
   └─ T7-thin (dry-run)     ┘
        │
        ├─ T2 (process runner)  ┐
        ├─ T5a (Claude adapter) ├─ Phase 2 real Workers (T2 → T5a/T5b 병렬)
        └─ T5b (Codex adapter)  ┘
              │
              ├─ T3 (synthesis engine)  ┐
              ├─ T4 (safety/lock/git)   ├─ Phase 3 mutation (병렬 가능)
              └─ T7-full (orchestrator) ┘
                    │
                    └─ T9 + Phase 4 hardening
                          │
                          └─ Phase 5 verify
```

## 3. Final ticket list (paste-ready 다음 섹션에서 prompt 화)

| ID | Phase | Owns (no overlap) | Reads only | Deps |
|---|---|---|---|---|
| **T0** | 0 | `spikes/*.md`, `spikes/scripts/*.{js,ps1}` | DESIGN.md | — |
| T1 | 1 | `src-tauri/`, `src/App.tsx`, `src/components/Workbench/*` (단 SynthesisView, ClaimLedger 제외) | DESIGN.md, T0 결과 | T0 |
| T8 | 1 | `mockResponses/*.json`, `src-tauri/src/mock/*` | T0 결과 | T0 |
| T6 | 1 | `src/components/SynthesisView.tsx`, `src/components/ClaimLedger.tsx`, `src/lib/synthesisTypes.ts` | mock JSON | T8 (schema) |
| T7-thin | 1 | `src/lib/orchestrator/dryRun.ts`, `src-tauri/src/orchestrator_dryrun.rs` | T8 mock 인터페이스 | T8 |
| T2 | 2 | `src-tauri/src/process/*`, `src/lib/processEvents.ts` | T0 결과 | T0 |
| T5a | 2 | `src-tauri/src/adapters/claude.rs`, `prompts/workers/claude_*.txt` | T2 | T2 |
| T5b | 2 | `src-tauri/src/adapters/codex.rs`, `prompts/workers/codex_*.txt` | T2 | T2 |
| T3 | 3 | `src/lib/synthesis/merge.ts`, `src/lib/synthesis/__tests__/*` | T6 schema | T6 |
| T4 | 3 | `src-tauri/src/safety/*`, `src-tauri/src/git/worktree.rs`, `src-tauri/src/lock/*`, `src-tauri/src/journal/*` | T2 | — (T2 만) |
| T7-full | 3 | `src-tauri/src/orchestrator/*` (dryRun 제외), `src/lib/orchestrator/stateMachine.ts` | T2, T3, T4, T5a, T5b | T2, T3, T4, T5a, T5b |
| T9 | 4 | `src-tauri/src/telemetry/*`, `src/components/CostMeter.tsx`, `src-tauri/src/cancel/*` | T2 | T2 |

**병렬 가능한 첫 스프린트** (T0 통과 후): T1, T8, T2 (3 worker 동시 가능). T6, T7-thin 는 T8 schema 합의 후.
