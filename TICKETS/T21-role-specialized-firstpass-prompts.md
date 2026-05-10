# T21 — Worker별 역할 특화 first-pass 프롬프트

## Origin
RL Conductor 논문 (arXiv:2512.04388, ICLR 2026) 적용 가능성 리서치에서 도출.
MoA 흐름 D 종합 + Codex adversarial review (2026-05-10) 판정: **HIGH**.

## 새 Claude 창 만들기 가이드
현재 master 에서 즉시 착수 가능 (선행 없음). worktree: T21-role-prompts.

---

```
[세션 부트]
- repo: D:\moa-desktop
- base branch: master
- 권장 분기: feat/T21-role-prompts
- 권위: PROJECT-RULES.md, AGENTS.md, DESIGN.md (## Flows), prompts/workers/*.txt
- 운영: MoA Flow C — § 2.6 템플릿 A

[의존성 self-check]
없음 — 현재 master 에서 즉시 가능.

[INDEPENDENT FIRST-PASS — read-only]
```

## Goal
Claude 와 Codex 의 first-pass 프롬프트를 역할 분화하여 MoA 다양성을 높인다.
현재 두 워커는 동일한 deliverable 6 항목을 동일한 관점으로 생산 → 중복 분석, correlated blind spot 위험.

- **Claude first-pass**: architecture reasoning, edge case analysis, design-level risk, API contract 검증
- **Codex first-pass**: mechanical correctness, Windows runtime behavior, sandbox/permission 검증, concrete repro/test scenarios

output contract (WorkerClaim NDJSON schema) 는 변경 없음 — 역할 분화는 관점의 차이이지 출력 포맷의 차이가 아님.

## Success criteria
- [ ] `prompts/workers/claude_firstpass_template.txt` — Claude 역할 특화 deliverable 가이드 추가 (architecture + edge case focus)
- [ ] `prompts/workers/codex_firstpass_template.txt` — Codex 역할 특화 deliverable 가이드 추가 (mechanical + runtime focus)
- [ ] **output contract 보존**: 양측 WorkerClaim 의 `id, text, confidence, citations, topic` 필드 및 NDJSON 포맷 불변
- [ ] **adversarial review 프롬프트 분리**: `src-tauri/src/orchestrator/mod.rs` 에서 first-pass 프롬프트와 adversarial 프롬프트가 별도 경로로 구성됨을 확인 (현재 first-pass argv 재사용 경로 `mod.rs:1062-1064` 검증). adversarial reviewer 는 역할 bias 없이 synthesis 결과만 비판해야 함
- [ ] **synthesis engine 무변경**: `src/lib/synthesis/merge.ts` 의 Jaccard pairing 이 역할 분화된 claim 을 정상 처리 (topic vocabulary 차이로 인한 false disagreement 없음)
- [ ] A/B 검증: 기존 동일-프롬프트 vs 역할-분화 프롬프트로 동일 task 에 대해 synthesis 결과 비교. `codexOnly`/`claudeOnly`/`disagreement` 비율 변화 기록

## Files owned
- `prompts/workers/claude_firstpass_template.txt`
- `prompts/workers/codex_firstpass_template.txt`

## NEVER
- `src/lib/synthesis/merge.ts` — synthesis 알고리즘 변경 금지
- `src/lib/synthesis/types.ts` — WorkerClaim schema 변경 금지
- `src-tauri/src/orchestrator/adversarial.rs` — adversarial 로직 변경 금지
- `src-tauri/src/safety/` — guard/scanner 변경 금지

## Risks
1. **topic vocabulary 분기**: Claude 가 "architectural coupling" 관점, Codex 가 "runtime permission" 관점으로 같은 문제를 기술하면 Jaccard similarity < 0.85 → 실제 합의인데 disagreement 로 분류. **대응**: A/B 검증에서 false disagreement rate 측정, threshold 조정 검토 (별도 티켓)
2. **adversarial prompt contamination**: first-pass 역할 bias 가 adversarial review 로 누출되면 reviewer 가 편향. **대응**: adversarial 프롬프트 구성 경로 분리 확인
3. **coverage gap**: 양측 모두 놓치는 영역 발생 가능 (역할 분화가 너무 좁으면). **대응**: deliverable 6 항목 구조는 유지, 역할은 "emphasis" 레벨이지 "restriction" 이 아님

## Validation cmd

```powershell
cargo test --manifest-path src-tauri\Cargo.toml firstpass
npm test -- --run "firstpass|synthesis"
rg -n "claude_firstpass_template|codex_firstpass_template|WorkerClaim|adversarial.*argv|firstpass.*role" prompts src-tauri/src src/lib/synthesis
```

## Worker prompt 6 mandatory fields
1. Success criteria: Claude first-pass 역할 특화 (architecture reasoning, edge case, design risk, API contract), Codex first-pass 역할 특화 (mechanical correctness, Windows runtime, sandbox/permission, repro/test), WorkerClaim NDJSON output contract 보존, adversarial review 프롬프트 분리 확인 (first-pass argv 재사용 경로 `mod.rs:1062-1064`), synthesis engine 무변경 (Jaccard pairing 정상), A/B 검증 1건 을 구현한다.
2. NEVER 영역: `src/lib/synthesis/merge.ts` synthesis 알고리즘 변경, `src/lib/synthesis/types.ts` WorkerClaim schema 변경, `src-tauri/src/orchestrator/adversarial.rs` adversarial 로직 변경, `src-tauri/src/safety/` guard/scanner 변경, worker 직접 peer 호출.
3. Validation cmd:
   ```powershell
   cargo test --manifest-path src-tauri\Cargo.toml firstpass
   npm test -- --run "firstpass|synthesis"
   rg -n "claude_firstpass_template|codex_firstpass_template|WorkerClaim|adversarial.*argv|firstpass.*role" prompts src-tauri/src src/lib/synthesis
   ```
4. Files + lines: `prompts/workers/claude_firstpass_template.txt`, `prompts/workers/codex_firstpass_template.txt`, `src-tauri/src/orchestrator/mod.rs:1062-1064` (adversarial argv 재사용 경로), `src/lib/synthesis/merge.ts` (Jaccard pairing), `src/lib/synthesis/types.ts` (WorkerClaim schema).
5. Alternatives 2개 + pros/cons + 선택 근거: 동일 프롬프트 유지(중복 분석 + correlated blind spot 위험) vs 역할 분화 프롬프트(MoA 다양성 향상, topic vocabulary 분기로 false disagreement 가능). 선택은 역할 분화. deliverable 6 항목 구조 유지 + 역할은 emphasis 이지 restriction 아님.
6. Tests-first: Jaccard false disagreement rate 측정 (A/B baseline), adversarial prompt contamination 부재 확인, output contract NDJSON schema 보존 검증 을 먼저 설정하고 구현한다.

## Worker prompt cross-contract fields
- GitHub/project handling: GitHub #52 / Project `MoA Desktop` card status 를 claim/complete 단계에서 갱신한다.
- Conflict matrix ownership: T21 owns 는 `prompts/workers/claude_firstpass_template.txt`, `prompts/workers/codex_firstpass_template.txt` 로 한정한다. `src/lib/synthesis/merge.ts`, `src/lib/synthesis/types.ts`, `src-tauri/src/orchestrator/adversarial.rs`, `src-tauri/src/safety/` 는 read-only.
- Dependency/merge order: 선행 없음 — 현재 master 에서 즉시 가능. T22 (adaptive depth) 는 T21 이후 또는 병행 가능.
- Review gate warning: lead/orchestrator-owned `CodexAdversarialXHigh` 가 `Clean` 을 반환하고 `source_output_path` 가 persisted 되기 전에는 `pr_create`, `pr_merge`, `integrate_merge`, `main_apply` 진행 금지. worker 는 직접 peer review 를 실행하지 않는다.

## Paste-ready prompt

```text
[세션 부트]
- Prompt kind: Codex Desktop manual lead ticket session
- repo: D:\moa-desktop
- branch: codex/T21-role-specialized-firstpass-prompts
- worktree required

[Goal]
Claude 와 Codex 의 first-pass 프롬프트를 역할 분화하여 MoA 다양성을 높인다.

[NEVER]
synthesis 알고리즘 변경, WorkerClaim schema 변경, adversarial 로직 변경, safety guard/scanner 변경 금지.

[Validation]
cargo test --manifest-path src-tauri\Cargo.toml firstpass
npm test -- --run "firstpass|synthesis"

[작업 완료 시]
역할 분화 내용, A/B 검증 결과, false disagreement rate 를 보고한다.
```

[작업 완료 시 — 무조건 이 순서로]
1. commit: `feat(T21): role-specialized first-pass prompts for Claude/Codex MoA diversity` (본문에 `Closes #52` 포함, push 금지)
2. **GitHub 카드 완료 처리 — 잊지 말고 무조건 실행**:
   ```
   node ~/.claude/scripts/gh-tickets.mjs complete D:\moa-desktop 52
   ```
   - 출력에 `COMPLETED=52` 또는 `ALREADY_CLOSED=52` 가 보여야 OK.
   - 실패 시 사용자 보고 + STOP.
3. 보고: 역할 분화 내용, A/B 검증 결과, false disagreement rate, **GitHub 카드 close 결과 1줄**.

## Estimated effort
Small — 프롬프트 텍스트 변경 + adversarial 경로 분리 확인 + A/B 검증 1건. 코드 변경 최소.
