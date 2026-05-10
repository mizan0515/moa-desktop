# T25 — TS→Rust structured synthesis metadata 경계

GitHub: #56 (https://github.com/mizan0515/moa-desktop/issues/56)

## Origin
RL Conductor 논문 (arXiv:2512.04388, ICLR 2026) 적용 가능성 리서치에서 도출.
MoA 흐름 D 종합 + Codex adversarial review (2026-05-10) 판정: **MEDIUM**.

## 새 Claude 창 만들기 가이드
B1 (T22 적응형 깊이) 재평가 시 함께 착수. T22의 선행 조건.

---

````
[세션 부트]
- repo: D:\moa-desktop
- base branch: master
- 권장 분기: feat/T25-synthesis-metadata
- 권위: DESIGN.md, src/lib/synthesis/merge.ts, src/lib/synthesis/types.ts, src-tauri/src/orchestrator/mod.rs
- 운영: MoA Flow C — § 2.6 템플릿 A

[의존성 self-check — claim 직후, first-pass 시작 전 무조건 실행]
```
cd D:\moa-desktop && git log master --oneline -100 | rg -i "feat\(T3\)|feat\(T7-full\)" | wc -l
```
- 결과 `2` 면 OK — 작업 진행
- 2 미만이면 **STOP — "T3 또는 T7-full 이 master 에 미머지" 사용자 보고** + 누락 commit 목록 작성.
- 추가 확인:
  - T3: `rg -c "merge\|synthesis" src/lib/synthesis/merge.ts` — 1 이상이면 OK
  - T7-full: `rg -c "invoke\|ipc\|tauri::command" src-tauri/src/orchestrator/mod.rs` — 1 이상이면 OK

[INDEPENDENT FIRST-PASS — read-only]
````

## Goal
TS synthesis engine의 출력 → Rust orchestrator로 전달 시 structured metadata를 보존하는 versioned contract 도입.

현재 상태: synthesis 결과가 opaque JSON으로 Rust에 전달되어, aggregate confidence, disagreement severity 등 메트릭이 Rust 측에서 활용 불가. T22 (적응형 adversarial 깊이)가 이 메트릭에 의존.

## Success criteria
- [ ] TS→Rust IPC에 versioned synthesis metadata schema 정의 (e.g., `SynthesisMetadata v1`)
- [ ] schema 필드: `{version, total_claims, verified_count, claude_only_count, codex_only_count, disagreement_count, avg_confidence, min_confidence, max_severity}`
- [ ] TS synthesis engine이 merge 결과와 함께 metadata를 생성
- [ ] Rust orchestrator가 metadata를 typed struct로 deserialize
- [ ] version mismatch 시 graceful fallback (opaque JSON으로 degraded 동작)
- [ ] 기존 synthesis 알고리즘 무변경 — metadata 추출은 결과 후처리

## Dependencies
- T3 (synthesis engine)
- T7-full (orchestrator IPC)

## Files owned (예상)
- `src/lib/synthesis/metadata.ts` (신규)
- `src-tauri/src/orchestrator/synthesis_metadata.rs` (신규)

## NEVER
- `src/lib/synthesis/merge.ts` — synthesis 알고리즘 변경 금지 (metadata 추출 함수만 추가)
- `src/lib/synthesis/types.ts` — WorkerClaim schema 변경 금지
- `src-tauri/src/safety/` — guard/scanner 변경 금지

## Risks
1. **schema version 관리 복잡성**: TS와 Rust 양측에서 schema 동기화 필요. **대응**: single source of truth (TS 측 JSON Schema → Rust codegen) 또는 shared schema 파일
2. **metadata 계산 오버헤드**: claim 수가 많으면 메트릭 계산 비용. **대응**: O(n) scan이면 무시 가능, 현재 claim 수 << 100
3. **opaque JSON fallback 시 T22 동작 불가**: metadata 없으면 depth heuristic에 입력 없음. **대응**: fallback 시 fixed 3-round (현재 동작) 유지

## 재평가 시점
T22 (적응형 adversarial 깊이) 착수 결정 시 함께.

## Validation cmd

```powershell
cargo test --manifest-path src-tauri\Cargo.toml synthesis_metadata
npm test -- --run "metadata|synthesis"
rg -n "SynthesisMetadata|avg_confidence|min_confidence|disagreement_count|version.*mismatch" src-tauri/src/orchestrator src/lib/synthesis
```

## Worker prompt 6 mandatory fields
1. Success criteria: TS→Rust IPC versioned synthesis metadata schema (`SynthesisMetadata v1`), schema 필드 (`version, total_claims, verified_count, claude_only_count, codex_only_count, disagreement_count, avg_confidence, min_confidence, max_severity`), TS synthesis engine 에서 merge 결과와 함께 metadata 생성, Rust orchestrator 에서 typed struct deserialize, version mismatch graceful fallback, 기존 synthesis 알고리즘 무변경 을 구현한다.
2. NEVER 영역: `src/lib/synthesis/merge.ts` synthesis 알고리즘 변경 (metadata 추출 함수만 추가), `src/lib/synthesis/types.ts` WorkerClaim schema 변경, `src-tauri/src/safety/` guard/scanner 변경, worker 직접 peer 호출.
3. Validation cmd:
   ```powershell
   cargo test --manifest-path src-tauri\Cargo.toml synthesis_metadata
   npm test -- --run "metadata|synthesis"
   rg -n "SynthesisMetadata|avg_confidence|min_confidence|disagreement_count|version.*mismatch" src-tauri/src/orchestrator src/lib/synthesis
   ```
4. Files + lines: `src/lib/synthesis/metadata.ts` (신규), `src-tauri/src/orchestrator/synthesis_metadata.rs` (신규), `src/lib/synthesis/merge.ts` (metadata 추출 지점), `src-tauri/src/orchestrator/mod.rs` (IPC 경계).
5. Alternatives 2개 + pros/cons + 선택 근거: opaque JSON 유지(변경 없지만 T22 depth heuristic 구현 불가) vs versioned schema contract(structured metadata 활용 가능, schema 동기화 관리 필요). 선택은 versioned schema. single source of truth 로 TS JSON Schema → Rust codegen 또는 shared schema 파일 방식 검토.
6. Tests-first: metadata 생성 정확성 (claim 수 / confidence 통계), Rust deserialization roundtrip, version mismatch fallback (fixed 3-round), schema 동기화 drift detection 을 먼저 실패시키고 구현한다.

## Worker prompt cross-contract fields
- GitHub/project handling: GitHub #56 / Project `MoA Desktop` card status 를 claim/complete 단계에서 갱신한다.
- Conflict matrix ownership: T25 owns 는 `src/lib/synthesis/metadata.ts` (신규), `src-tauri/src/orchestrator/synthesis_metadata.rs` (신규) 로 한정한다. `src/lib/synthesis/merge.ts` (metadata 추출 함수 추가만), `src/lib/synthesis/types.ts`, `src-tauri/src/safety/` 는 read-only.
- Dependency/merge order: T3 + T7-full 완료 후. T22 (adaptive depth) 와 동시 진행 가능 — T22 가 이 metadata 에 의존.
- Review gate warning: lead/orchestrator-owned `CodexAdversarialXHigh` 가 `Clean` 을 반환하고 `source_output_path` 가 persisted 되기 전에는 `pr_create`, `pr_merge`, `integrate_merge`, `main_apply` 진행 금지. worker 는 직접 peer review 를 실행하지 않는다.

## Paste-ready prompt

````text
[세션 부트]
- Prompt kind: Codex Desktop manual lead ticket session
- repo: D:\moa-desktop
- branch: codex/T25-ts-rust-structured-synthesis-metadata
- worktree required

[Goal]
TS synthesis engine → Rust orchestrator 간 structured metadata 를 versioned contract 로 전달한다.

[NEVER]
synthesis 알고리즘 변경, WorkerClaim schema 변경, safety guard/scanner 변경 금지.

[Validation]
cargo test --manifest-path src-tauri\Cargo.toml synthesis_metadata
npm test -- --run "metadata|synthesis"

[작업 완료 시]
schema definition, fallback behavior, T22 handoff 를 보고한다.
````

[작업 완료 시 — 무조건 이 순서로]
1. commit: `feat(T25): TS→Rust structured synthesis metadata with versioned contract` (본문에 `Closes #56` 포함, push 금지)
2. **GitHub 카드 완료 처리 — 잊지 말고 무조건 실행**:
   ```
   node ~/.claude/scripts/gh-tickets.mjs complete D:\moa-desktop 56
   ```
   - 출력에 `COMPLETED=56` 또는 `ALREADY_CLOSED=56` 가 보여야 OK.
   - 실패 시 사용자 보고 + STOP.
3. 보고: schema definition, fallback behavior, T22 handoff, **GitHub 카드 close 결과 1줄**.

## Estimated effort
Small-Medium — schema 정의 + TS metadata 추출 + Rust deserialization. 알고리즘 변경 없음.
