# Claude First-Pass Analysis — MoA Desktop Design Review
Date: 2026-05-06
Scope: D:\moa-desktop\DESIGN.md
Mode: Read-only, independent (Codex is doing parallel first-pass separately)

## A. Critical flaws / gaps / contradictions

### A1. The synthesis paradox — "누가 5칸 표를 만드는가?"
**문제**: 설계는 "execution owner가 종합" 이라고 하지만 MoA Desktop에는 execution owner라는 개념이 없음 (앱이 orchestrator). 종합을 LLM이 하면 — 어느 쪽 LLM? Claude면 Claude bias, Codex면 Codex bias. silent averaging 금지인데 종합 LLM이 자동으로 averaging.
**evidence**: DESIGN.md "Synthesis (5칸 schema, execution owner 담당)"
**risk**: high — MoA의 핵심 가치(독립 검증)이 무너짐
**해결 방향**: 각 Worker가 **structured JSON**으로 출력 → 앱이 mechanical merge:
- "Verified" = 두 Worker가 동일 claim을 string-similarity ≥0.85로 출력한 것
- "X-only" = 한쪽만 출력
- "Disagreement" = 동일 topic 다른 결론 (topic clustering 필요 — 또는 단순히 Workers에게 "topic" 필드 강제)
- "Open" = 양쪽 다 confidence=low로 표기한 것
실제 LLM 종합은 마지막 narrative 요약에서만 사용.

### A2. Adversarial review가 재귀를 만들 위험
**문제**: 설계는 Worker가 peer 호출 금지. 하지만 adversarial review는 "synthesis를 다른 AI에 보여주고 비판받기". orchestrator가 새 prompt로 Codex Worker를 다시 호출하는 거면 재귀가 아님. **하지만 설계 문서에 이게 명시 안 됨** — 구현 단계에서 잘못 짜면 Claude Worker가 결과를 보고 "이거 Codex 의견 듣자"라며 NEED_PEER_REVIEW 를 출력 → 앱이 Codex 호출 → Codex가 "이건 Claude 의견 필요" → 무한 루프.
**evidence**: DESIGN.md "Adversarial (비-종합자가 비판)"
**risk**: med — 명시 안 되면 구현자가 헤맴
**해결**: orchestrator state machine을 명시 — round counter ≤ 3, NEED_PEER_REVIEW 는 hint일 뿐 실제 다음 round 진입 결정은 orchestrator가.

### A3. `--append-system-prompt` 가 tool 호출을 진짜 막는가?
**문제**: Worker guard 텍스트로 "Do not call /codex:rescue" 라고 prose로 지시 → LLM이 그냥 무시할 수도 있음. 진짜 sandboxing은 `--disallowedTools` + plugin/MCP 비활성화.
**evidence**: Claude Code CLI의 --append-system-prompt는 prose-level 가이드. tool 차단은 별도.
**risk**: high — prose 가드만 믿으면 일부 응답에서 Worker가 peer 도구를 invoke함
**해결**: Claude Worker 호출 시 `--disallowedTools` 에 명시:
- 모든 `mcp__*` 패턴 (Codex MCP 차단)
- `Skill` (slash 우회 차단)
- `Agent` 또는 subagent_type 화이트리스트 강제
또한 Codex MCP 플러그인 자체를 Worker용 fork된 `~/.claude/` 로 분리하거나, env로 `ENABLE_CLAUDEAI_MCP_SERVERS=false` 같은 차단.

### A4. Codex CLI에는 `--disallowedTools` 같은 게 없을 수 있음
**문제**: Codex Worker 가드는 prose only. Codex CLI가 sandbox flag를 제공하는지 미검증.
**risk**: med-high — Codex가 MCP나 다른 AI를 부르는 경로가 있다면 가드 우회 가능
**해결**: Codex 실행 환경에서 외부 호출 가능한 MCP를 env로 비활성. 검증은 Codex의 web search 결과 기다림.

### A5. Output scanner는 trivial bypass 가능
**문제**: 문자열 매칭 (`/codex:`, `claude -p`)은:
- 띄어쓰기 우회 ("/codex :")
- Unicode lookalike ("⁄codex:")
- 코드블록 안 ("```/codex:rescue```" — 의도된 인용일 수도)
- 변수 분할 ("/co" + "dex:")
**evidence**: DESIGN.md "Forbidden output scanner"
**risk**: med — 일부러 우회하는 적대적 worker가 아니라도 false negative 발생
**해결**: scanner는 보조 신호로만 (block이 아니라 warning + human review), 실제 차단은 sandbox(A3) + plugin 비활성.

### A6. `~/.moa-desktop/` 경로가 Windows convention 위반
**문제**: Windows는 `%APPDATA%\moa-desktop\` (= `C:\Users\X\AppData\Roaming\moa-desktop\`) 가 표준. `~/.moa-desktop/`는 Unix 스타일.
**evidence**: DESIGN.md "Storage: ~/.moa-desktop/"
**risk**: low (작동은 하지만 anti-pattern)
**해결**: Tauri v2 `app_data_dir()` 사용 — cross-platform 자동.

### A7. Auth/env 상속 미명시
**문제**: Tauri 자식 프로세스가 OAuth 토큰, `CODEX_HOME`, `CLAUDE_CODE_*` env, plugin enabled 목록 등을 상속해야 함. 설계 문서는 이 부분 무.
**evidence**: DESIGN.md command adapter 섹션
**risk**: high — 테스트 못 거치면 "Worker가 인증 안 됨"으로 첫 실행 실패
**해결**: Tauri Command API에서 env 명시 전달, 첫 spike 단계에서 검증.

### A8. Cancellation / process kill 미설계
**문제**: 사용자가 "중단" 클릭 → 진행 중 Worker 자식 프로세스 + 그 자식 (subagent, MCP server 등) 전체 트리를 죽여야 함. Windows에서 process tree kill은 `taskkill /T /F`. 설계 무.
**risk**: high — 좀비 프로세스 누적 → 사용자 경험 박살
**해결**: Tauri Command의 abort signal + `--max-turns` 보수적 설정 + 명시적 process group 관리.

### A9. Streaming vs batch 출력
**문제**: `--output-format json` 은 batch (전부 끝나야 응답). UX 측면에서 progress lane을 보여주려면 `--output-format stream-json` 이 맞음.
**evidence**: DESIGN.md command adapter
**risk**: med — UX 만족도 낮아짐
**해결**: stream-json 사용 + JSONL 파싱.

### A10. Same-file sequential edit의 conflict detection 빈약
**문제**: 설계는 "git diff로 확인" → 프로젝트가 git repo가 아니면? uncommitted state면? 설계는 파일 hash 비교 같은 mechanism 무.
**risk**: med
**해결**: 각 Worker turn 전 file hash 저장, turn 후 hash 검증. git 의존성 제거.

### A11. Token cost / quota 텔레메트리 부재
**문제**: MoA round = Worker × 2 + 종합 + adversarial = 최소 3-4 LLM 호출. Opus 사용 시 한 작업이 $5-10. 설계는 cost cap 없음.
**risk**: high (운영) — 사용자가 비용 인지 못함
**해결**: 세션별 token/cost 누적 표시, 임계 도달 시 confirm.

### A12. Test runner 가정
**문제**: "Verification command runner" 로 npm test, pytest 가정. 프로젝트가 다른 stack (Rust cargo test, Go go test, Java mvn test) 이면? 설계 무.
**risk**: low-med — 처음에는 프로젝트별 verification cmd를 settings에 명시 받게.

### A13. UI 좁은 폭 + 카드 중첩 금지 + 두 lane + lock owner + round counter — 정보 밀도 과다
**risk**: med
**해결**: 첫 spike에서 wireframe → 사용자 confirm 후 코드.

### A14. Implementation steps 15개가 너무 큼
**문제**: spike (de-risk) 단계 없이 바로 scaffold → 첫 통합 단계에서 막힐 가능성.
**해결**: 추천 — § B에서 다시 정리.

## B. Recommended implementation order (Claude 안)

### Phase 0 — De-risk spikes (1주)
- **Spike A**: Node 스크립트로 `claude -p` spawn. stream-json 파싱. 인증/env 상속 확인.
- **Spike B**: 같은 방식으로 `codex exec` (또는 실제 headless cmd) spawn.
- **Spike C**: 둘을 병렬 실행, stdout 충돌·deadlock 없는지 확인.
- **Spike D**: Claude `--disallowedTools` 로 MCP 차단 시 실제 `/codex:rescue` 호출 막히는지 검증.

### Phase 1 — Skeleton (2주)
1. Tauri v2 scaffold + workbench static layout (랜딩 페이지 X)
2. Settings 화면 + 스토리지 (`%APPDATA%\moa-desktop\`)
3. Single-Worker Flow A end-to-end (Claude Worker만, mutation 가능, 결과 표시)

### Phase 2 — Dual-Worker (2주)
4. Codex Worker adapter + 병렬 first-pass (read-only)
5. Per-Worker JSON 출력 schema 정의
6. Mechanical 5칸 synthesis 엔진
7. 결과 SynthesisView UI

### Phase 3 — Adversarial + Lock (1.5주)
8. Adversarial round (orchestrator state machine)
9. Mutation lock manager
10. Same-file sequential edit (file hash diff)
11. Verification command runner

### Phase 4 — Hardening (1주)
12. Output scanner (warning level)
13. Cost telemetry
14. Cancellation / process tree kill
15. Final report (Claim Ledger)
16. Dry-run mode (mockResponses/)

총 ≈ 6-7주 (1인). 2인 병렬이면 4-5주.

## C. Parallel tickets (non-conflicting)

### T1 — Tauri shell + 정적 workbench UI
- owns: `src-tauri/`, `src/App.tsx`, `src/ui/Workbench/*` (SynthesisView, ClaimLedger 제외)
- reads: DESIGN.md
- deps: 없음
- accept: 앱 실행 → workbench 레이아웃, 더미 데이터, 폭 좁아도 깨지지 않음

### T2 — Worker spawn library
- owns: `src-tauri/src/workers/{claude,codex,scanner}.rs`, `src/lib/worker-types.ts`
- reads: DESIGN.md
- deps: 없음
- accept: unit test — spawn / stream-json 파싱 / kill 동작

### T3 — Synthesis engine (mechanical)
- owns: `src/lib/synthesis/*` 순수 함수
- reads: T2의 worker output schema
- deps: T2 schema 합의 후
- accept: 두 sample JSON → 5칸 deterministic 출력

### T4 — Lock manager + diff gate
- owns: `src-tauri/src/lock/*`, `src-tauri/src/diff/*`
- reads: 없음
- deps: 없음
- accept: lock acquire/release/transfer, file hash snapshot/verify unit test

### T5 — Settings + storage
- owns: `src/ui/Settings/*`, `src-tauri/src/settings/*`
- reads: DESIGN.md
- deps: T1 (앱 shell)
- accept: settings JSON round-trip, 비밀 미저장

### T6 — Synthesis view + Claim Ledger UI
- owns: `src/ui/SynthesisView.tsx`, `src/ui/ClaimLedger.tsx`
- reads: T3 schema
- deps: T3
- accept: sample synthesis JSON → 5칸 + ledger 렌더

### T7 — Orchestrator state machine
- owns: `src/lib/orchestrator/*`
- reads: T2, T3, T4 인터페이스
- deps: T2, T3, T4
- accept: Flow A/B/C/D 시작→종료 state transition

### T8 — Dry-run / mock mode
- owns: `src-tauri/src/mock/*`, `mockResponses/*.json`
- reads: T2 인터페이스
- deps: T2
- accept: settings.mockMode=true → Worker 응답이 canned JSON

### T9 — Cost telemetry + cancellation
- owns: `src-tauri/src/telemetry/*`, `src/ui/CostMeter.tsx`
- reads: T2
- deps: T2
- accept: 세션별 token/cost 누적, Stop 클릭 시 process tree kill

### Dependency graph
```
T1 ──┬─ T5
     └─ (UI 사용처)

T2 ──┬─ T3 ── T6
     ├─ T7
     ├─ T8
     └─ T9

T4 ── T7

T1 + T7 + T6 + T9 → 통합 (Phase 1-4 완료 시점)
```

병렬 가능 첫 스프린트: **T1, T2, T4** (서로 독립). 그 다음 T3, T5, T8, T9 동시. 마지막 T6, T7 통합.

## D. Open questions for synthesis with Codex
- Codex CLI의 실제 headless 명령은? (`codex exec` 검증 필요)
- Tauri v1 vs v2 — 현재 (2026-05) 권장은?
- `--allowedTools` 에 MCP 패턴 매칭 가능한가? (`--disallowedTools "mcp__*"` 같은 와일드카드)
- Claude Code CLI가 `--output-format stream-json` 을 정식 지원하는가?
- Codex CLI에 sandbox/permission flag가 있나?
