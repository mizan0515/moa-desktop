# T22 — 난이도 적응형 adversarial 깊이

GitHub: #53 (https://github.com/mizan0515/moa-desktop/issues/53)

## Origin
RL Conductor 논문 (arXiv:2512.04388, ICLR 2026) 적용 가능성 리서치에서 도출.
MoA 흐름 D 종합 + Codex adversarial review (2026-05-10) 판정: **MEDIUM**.

## 새 Claude 창 만들기 가이드
v1.0 출시 후 실사용 데이터 축적 필요. 선행: T7-full (orchestrator), T3 (synthesis engine).

---

````
[세션 부트]
- repo: D:\moa-desktop
- base branch: master
- 권장 분기: feat/T22-adaptive-depth
- 권위: DESIGN.md (## Flows, ## Adversarial), src-tauri/src/orchestrator/mod.rs, src/lib/synthesis/merge.ts
- 운영: MoA Flow C — § 2.6 템플릿 A

[의존성 self-check — claim 직후, first-pass 시작 전 무조건 실행]
```
cd D:\moa-desktop && git log master --oneline -100 | rg -i "feat\(T7-full\)|feat\(T3\)" | wc -l
```
- 결과 `2` 면 OK — 작업 진행
- 2 미만이면 **STOP — "T7-full 또는 T3 가 master 에 미머지" 사용자 보고** + 누락 commit 목록 작성.
- 추가 확인:
  - T7-full: `rg -c "adversarial_round\|round_loop\|max_rounds" src-tauri/src/orchestrator/` — 1 이상이면 OK
  - T3: `rg -c "OpenRow\|WorkerClaim" src/lib/synthesis/` — 1 이상이면 OK
- v1.0 실사용 데이터: confidence 분포, round별 verdict 패턴 (v1.0 이전이면 synthetic data 로 대체 가능)

[INDEPENDENT FIRST-PASS — read-only]
````

## Goal
현재 고정 3-round adversarial review를 task 난이도에 따라 1~3 round 동적 조절.
trivial 합의는 1 round에서 조기 종료, 복잡한 disagreement는 3 round 전부 사용.

RL Conductor의 핵심 교훈: "쉬운 문제에 깊은 파이프라인 = 토큰 낭비, 어려운 문제에 얕은 파이프라인 = 품질 저하".
단, RL이 아닌 rule-based heuristic으로 구현 (학습 데이터 부재).

## 현재 상태 및 제약
- `OpenRow` (synthesis output)에 confidence 필드 없음 — 깊이 결정에 필요한 신호 부재
- TS→Rust metadata 경계가 opaque JSON — structured 필드 전달 lossy
- fixed 3-round ceiling이 현재 안전한 default

## Success criteria
- [ ] `OpenRow` 또는 synthesis output에 aggregate confidence / disagreement severity 메트릭 추가
- [ ] orchestrator round loop가 early-exit 조건 지원 (e.g., round 1에서 전원 Pass + high confidence → 종료)
- [ ] round 수 동적 결정 heuristic: `(disagreement_count, max_severity, min_confidence)` 기반
- [ ] TS→Rust 경계에서 structured metadata 전달 (T25 선행 또는 동시)
- [ ] A/B 검증: 고정 3-round vs 적응형에서 동일 task set 비교 — verdict 일치율 + 토큰 절감율 측정
- [ ] safety invariant 유지: 적응형이 3-round 초과하는 경우 없음 (ceiling 보존)

## Dependencies
- T7-full (orchestrator round loop)
- T3 (synthesis engine)
- T25 (TS→Rust structured metadata — 동시 진행 가능)
- v1.0 실사용 데이터 (confidence 분포 패턴)

## Files owned (예상)
- `src-tauri/src/orchestrator/depth.rs` (신규)
- `src-tauri/src/orchestrator/mod.rs` (round loop 수정)

## NEVER
- `src/lib/synthesis/merge.ts` — synthesis 알고리즘 변경 금지 (메트릭 추출만 허용)
- `src/lib/synthesis/types.ts` — WorkerClaim schema 변경 금지 (OpenRow 확장은 별도 필드)
- `src-tauri/src/safety/` — guard/scanner 변경 금지

## Risks
1. **false early-exit**: confidence가 높지만 실제로는 양측이 같은 blind spot 공유 → adversarial이 잡아야 할 것을 놓침. **대응**: early-exit은 unanimous Pass일 때만, Concern/Block 있으면 무조건 다음 round
2. **heuristic overfitting**: 소수 사례에 튜닝하면 edge case 취약. **대응**: v1.0 이후 최소 100건 데이터 축적 후 착수
3. **TS→Rust metadata 누락**: T25 미완료 시 depth 결정에 필요한 신호 전달 불가. **대응**: T25와 동시 진행 또는 T25 선행

## 재평가 시점
v1.0 출시 후 실사용 데이터 축적 시. `OpenRow`에 confidence 필드 없음, TS→Rust metadata lossy 문제가 해결 전제.

## Validation cmd

```powershell
cargo test --manifest-path src-tauri\Cargo.toml adversarial_depth
npm test -- --run "adversarial|depth"
rg -n "early.exit|depth.*heuristic|round.*loop|OpenRow.*confidence|disagreement.*severity" src-tauri/src/orchestrator src/lib/synthesis
```

## Worker prompt 6 mandatory fields
1. Success criteria: `OpenRow` 또는 synthesis output 에 aggregate confidence / disagreement severity 메트릭 추가, orchestrator round loop early-exit 조건 (전원 Pass + high confidence → 종료), round 수 동적 결정 heuristic (`disagreement_count, max_severity, min_confidence`), TS→Rust structured metadata 전달 (T25), A/B 검증 (고정 3-round vs 적응형), safety invariant 유지 (3-round ceiling 보존) 를 구현한다.
2. NEVER 영역: `src/lib/synthesis/merge.ts` synthesis 알고리즘 변경 (메트릭 추출만 허용), `src/lib/synthesis/types.ts` WorkerClaim schema 변경 (OpenRow 확장은 별도 필드), `src-tauri/src/safety/` guard/scanner 변경, worker 직접 peer 호출.
3. Validation cmd:
   ```powershell
   cargo test --manifest-path src-tauri\Cargo.toml adversarial_depth
   npm test -- --run "adversarial|depth"
   rg -n "early.exit|depth.*heuristic|round.*loop|OpenRow.*confidence|disagreement.*severity" src-tauri/src/orchestrator src/lib/synthesis
   ```
4. Files + lines: `src-tauri/src/orchestrator/depth.rs` (신규), `src-tauri/src/orchestrator/mod.rs` (round loop), `src/lib/synthesis/merge.ts` (메트릭 추출 지점), `src/lib/synthesis/types.ts` (OpenRow 확인).
5. Alternatives 2개 + pros/cons + 선택 근거: 고정 3-round 유지(안전하지만 trivial 합의에 토큰 낭비) vs rule-based heuristic 적응형 깊이(토큰 절감 + 적절한 깊이, heuristic overfitting 위험). 선택은 rule-based heuristic. RL 학습 기반(최적이지만 학습 데이터 부재) 기각.
6. Tests-first: early-exit 정확성 (unanimous Pass 일 때만), false early-exit 방지 (Concern/Block 있으면 다음 round 강제), 3-round ceiling 유지, heuristic edge case (min confidence 변동) 를 먼저 실패시키고 구현한다.

## Worker prompt cross-contract fields
- GitHub/project handling: GitHub #53 / Project `MoA Desktop` card status 를 claim/complete 단계에서 갱신한다.
- Conflict matrix ownership: T22 owns 는 `src-tauri/src/orchestrator/depth.rs` (신규), `src-tauri/src/orchestrator/mod.rs` (round loop 수정 부분만) 로 한정한다. `src/lib/synthesis/merge.ts` (메트릭 추출만), `src/lib/synthesis/types.ts`, `src-tauri/src/safety/` 는 read-only.
- Dependency/merge order: T7-full + T3 완료 후. T25 (TS→Rust metadata) 와 동시 진행 가능. v1.0 실사용 데이터 축적 후 착수 권장.
- Review gate warning: lead/orchestrator-owned `CodexAdversarialXHigh` 가 `Clean` 을 반환하고 `source_output_path` 가 persisted 되기 전에는 `pr_create`, `pr_merge`, `integrate_merge`, `main_apply` 진행 금지. worker 는 직접 peer review 를 실행하지 않는다.

## Paste-ready prompt

````text
[세션 부트]
- Prompt kind: Codex Desktop manual lead ticket session
- repo: D:\moa-desktop
- branch: codex/T22-adaptive-adversarial-depth
- worktree required

[Goal]
현재 고정 3-round adversarial review를 task 난이도에 따라 1~3 round 동적 조절한다.

[NEVER]
synthesis 알고리즘 변경, WorkerClaim schema 변경, safety guard/scanner 변경, 3-round ceiling 초과 금지.

[Validation]
cargo test --manifest-path src-tauri\Cargo.toml adversarial_depth
npm test -- --run "adversarial|depth"

[작업 완료 시]
depth heuristic, A/B 검증 결과, 토큰 절감율 을 보고한다.
````

[작업 완료 시 — 무조건 이 순서로]
1. commit: `feat(T22): adaptive adversarial depth with rule-based heuristic` (본문에 `Closes #53` 포함, push 금지)
2. **GitHub 카드 완료 처리 — 잊지 말고 무조건 실행**:
   ```
   node ~/.claude/scripts/gh-tickets.mjs complete D:\moa-desktop 53
   ```
   - 출력에 `COMPLETED=53` 또는 `ALREADY_CLOSED=53` 가 보여야 OK.
   - 실패 시 사용자 보고 + STOP.
3. 보고: depth heuristic, A/B 검증 결과, 토큰 절감율, **GitHub 카드 close 결과 1줄**.

## Estimated effort
Medium — orchestrator round loop 수정 + depth heuristic 설계 + A/B 검증. synthesis engine은 메트릭 추출만.
