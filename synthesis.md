# MoA Synthesis — Claude × Codex Independent First-Pass
Date: 2026-05-06
Sources: `analysis-claude.md` + Codex first-pass result (in-conversation)
Mode: Mechanical 5-column merge, **silent averaging 금지**

## 1. VERIFIED (양측 일치)

| # | Claim | Claude evidence | Codex evidence | Action |
|---|---|---|---|---|
| V1 | `--append-system-prompt` 는 prose 가이드일 뿐 보안 경계 아님 | A3 | DESIGN.md:74-86 + Claude CLI docs | OS/process-level 차단 필수 (deny rules + sandbox + plugin off) |
| V2 | Codex CLI 에는 per-tool allow/deny flag 없음 | A4 | DESIGN.md:117-120 + developers.openai.com/codex/cli/reference | Codex 안전성은 `--sandbox read-only` + env/PATH 격리에 의존 |
| V3 | Output scanner (string blocklist) 는 trivial bypass 가능 | A5 | OWASP LLM injection cheatsheet | scanner = warning level only, 실제 차단은 sandbox |
| V4 | `~/.moa-desktop` 는 Windows convention 위반 | A6 | DESIGN.md:60-64 + Tauri appConfigDir docs | Tauri v2 `appConfigDir`/`appDataDir` 사용 → `%APPDATA%` 자동 |
| V5 | 인증/env 상속 미명시 — preflight 필요 | A7 | Claude CLI auth docs (med conf) | Spike 단계에서 `claude --help`, `codex --help`, version, auth, PATH 기록 |
| V6 | Cancellation / process tree kill / 좀비 프로세스 미설계 | A8 | Tauri v2 lifecycle docs | abort signal + `--max-turns` + Windows process group 명시 |
| V7 | Synthesis 책임자 underspecified (execution owner vs orchestrator) | A1 | DESIGN.md:29-40, 136-150 | **결론**: deterministic JSON merge for 5-column + orchestrator-owned LLM call for adversarial only |
| V8 | Adversarial review = orchestrator 호출이지 Worker가 peer 호출 X — 명시 필요 | A2 | Codex 같은 결론 | state machine 명시: round counter ≤ 3, `NEED_PEER_REVIEW` 는 hint만 |
| V9 | "git diff 저장" 만으로 same-file conflict 못 잡음 | A10 | base commit/tree + file hashes + index status + dirty files policy 필요 | file hash snapshot + worktree-isolated patch |
| V10 | Tauri v2 stable (2026-05) | (assumed) | https://v2.tauri.app/blog/tauri-20/ | v2 채택 |

## 2. CODEX-ONLY (Claude 본인 verify 미실시 — spot-check 필요)

| # | Claim | Codex evidence | Claude action |
|---|---|---|---|
| C1 | **Command 은 argv array 로 빌드 (shell string X). Windows PowerShell 이 `Bash(git status:*)` 같은 tool name 인용 망가뜨림** | DESIGN.md:100-108, Tauri shell plugin docs | ⚠️ critical — DESIGN.md 의 prose 명령 표기를 모두 argv 배열로 바꿔야 함. spike 시 검증. |
| C2 | **`codex exec --reasoning-effort high` UNVERIFIED. 실제 flag: `-m <model> -s read-only --json --cd <repo> <prompt>`** | developers.openai.com/codex/cli/reference | ⚠️ critical — DESIGN.md 의 `commandTemplate` 를 사용자 명시 검증 후 수정. 이건 LOAD-BEARING claim. |
| C3 | `--allowedTools` 는 allowlist 지만 read-only hard guarantee 아님 | Claude CLI docs | deny rules 명시 + smoke test 로 Edit/Write 시도 차단 확인 |
| C4 | Mutation lock 이 file 만으로 부족 — git index, tests, formatters, package managers 도 mutate | DESIGN.md:11-13, 44-58 | lock scope 정의: project source globs + git index 상태 + 외부 side-effect 가능한 명령 격리 |
| C5 | Verification checklist 누락: CLI missing, auth expired, malformed JSON, cancellation, worker timeout, process leak, read-only violation, scanner false positive, generated dirty files | DESIGN.md:176-191 | 체크리스트 확장 의무 |
| C6 | Flow A "Claude direct Edit/Write" 도 lock + checkpoint + post-diff gate 필요 — 단순 path 도 안전 우회 X | DESIGN.md:22-25 vs 11-13 | Flow A 도 lock 적용 |
| C7 | Mutation 은 isolated patch/worktree flow — Worker 가 temp tree 에 쓰고 app 이 patch apply/reject | step 10 | adopt — 강력한 개선 (rollback 무료) |
| C8 | Codex CLI sandbox 는 `--sandbox read-only` 또는 `-s read-only` 가 실제 mechanism | docs | first-pass 명령에 명시 |

## 3. CLAUDE-ONLY (Codex 미커버)

| # | Claim | evidence | 검토 |
|---|---|---|---|
| L1 | `--output-format json` (batch) vs `stream-json` (streaming) — UX 측면에서 streaming 필요 | A9 | Codex 도 process runner 에서 stream stdout 강조 (T3) → 기능 자체는 합의, 구체 flag 만 Claude-only |
| L2 | Token cost / quota telemetry 부재 — 운영 위험 high | A11 | Codex 미언급 — 추가 권고 |
| L3 | Test runner 가 npm/pytest 가정 — Rust/Go/Java 미커버 | A12 | settings 에 verification cmd 명시 받기 |
| L4 | UI 정보 밀도 (좁은 폭 + lock owner + round counter + cost meter) — wireframe 선행 필요 | A13 | 디자인 작업 |
| L5 | Implementation steps 15개 너무 큼 — Phase 0 de-risk spikes 필요 | A14 | Codex 의 11-step 보다 spike 우선 권고 |

## 4. DISAGREEMENT (미해결)

| # | Topic | Claude 입장 | Codex 입장 | 해결 방향 |
|---|---|---|---|---|
| D1 | Synthesis 메커니즘 | Mechanical JSON merge — string-similarity ≥0.85 | Deterministic 5-column aggregation + 별도 orchestrator-owned LLM call | **합의**: 5-column 자체는 mechanical, adversarial round 만 LLM. Codex T6 도 본질적으로 같은 구조. 충돌 X |
| D2 | Ticket 분해 granularity | 9 tickets, T5 를 Claude/Codex adapter 로 분리 | 7 tickets, T5 에 두 adapter 통합 | Codex T5 는 동일 파일 owner 충돌 위험 → **합의**: T5 를 T5a (Claude adapter) / T5b (Codex adapter) 로 split |
| D3 | Phase 0 de-risk spike 명시 여부 | 1주 spike 권고 | 첫 step 이 scaffold (spike 없음) | **합의**: spike 를 step 0 으로 추가. CLI auth/env/streaming/sandbox bypass 검증 후 scaffold |

## 5. OPEN QUESTIONS

| # | 질문 | 누가 답해야? |
|---|---|---|
| O1 | Tauri v2 sidecar 와 Command API 중 어느 쪽이 long-running CLI child 에 적합? | Spike 결과 |
| O2 | TOKEN-GUARD 가 Tauri-spawned `claude -p` 안에서 발화하는가? HARD 차단 시 stderr 가 부모에 전달되는가? | Spike 결과 |
| O3 | Claude `--disallowedTools` 가 `mcp__*` 와일드카드 지원? Codex MCP 호출 실제 차단되는가? | Spike 결과 |
| O4 | `claude --output-format stream-json` 정식 지원? | docs spot-check |
| O5 | UI wireframe — 정보 밀도 견딜 수 있는 layout 합의 | 사용자 confirm 필요 |

## 6. 종합 판단 (mutation owner: 사용자에게 escalate 후 결정)

설계는 **방향성이 옳음** — sibling orchestrator 구조, MoA flow, lock-based mutation 모두 핵심 위에 서 있음. 다만 **operational details 가 verify 안 된 상태로 spec 화**되어 있어 그대로 구현하면 막힘:

1. **CRITICAL FIX (LOAD-BEARING)**: `codex exec` 명령 템플릿 (V2) — 사용자가 자신의 Codex CLI 버전에 맞춰 검증 후 DESIGN.md 수정. `--reasoning-effort` 는 unverified 이므로 빼거나 검증.
2. **CRITICAL FIX**: Worker 명령 빌드를 argv array 로 (C1) — Windows quoting 사고 회피.
3. **CRITICAL FIX**: Sandbox 는 `--disallowedTools` + Codex `--sandbox read-only` + plugin/MCP env 분리 (V1, V2, V3) — prose 가드만 믿지 말 것.
4. **STRONG RECOMMENDATION**: Mutation 은 isolated worktree/patch flow (C7) — 단순 file lock 보다 안전.
5. **STRONG RECOMMENDATION**: Phase 0 de-risk spikes 1주 (D3) — 이거 안 하면 Phase 1 에서 막힘.
6. **NEW**: Token/cost telemetry (L2), verification cmd 일반화 (L3), UI wireframe (L4) 추가.
