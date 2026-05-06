# T3 — Synthesis engine (deterministic JSON merge)

## 새 Claude 창 만들기 가이드
T6 schema 합의 후 (T6 와 병렬 가능). worktree: T3-synthesis.

---

```
[세션 부트]
- repo: D:\moa-desktop
- base branch: master (T6 머지 후 — schema 의존)
- 권장 분기: feat/T3-synth-engine
- 권위: synthesis.md (실제 5칸 merge 예시), PLAN.md (§ F6 retry tracking, V7 mechanical merge), ~/.claude/CODEX-MCP.md § 2.6 (5칸 schema)
- 운영: MoA Flow C — § 2.6 템플릿 A

[INDEPENDENT FIRST-PASS — read-only]

## Goal
두 Worker JSON 출력을 받아 5칸 (verified / claude-only / codex-only / disagreement / open) 으로 deterministic merge. **LLM 호출 없음.** 순수 함수.

## Success criteria
- [ ] `src/lib/synthesis/merge.ts` — `synthesize(claude: WorkerOutput, codex: WorkerOutput): Synthesis`
- [ ] string-similarity ≥0.85 → "verified" 분류 (algorithm: token Jaccard or trigram cosine — 하나 골라 명시)
- [ ] topic clustering — Worker output 의 `topic` field 기준 (Worker template 에서 강제)
- [ ] confidence: 양측 모두 high → verified, 한쪽 low → "Codex-only" 등
- [ ] disagreement: 동일 topic, 다른 결론 → 별도 칸
- [ ] open: 양쪽 confidence=low 또는 "UNVERIFIED" 표기
- [ ] retry tracking: 같은 worker 의 2번째 attempt 가 들어오면 새 evidence 로 추가 (덮어쓰기 X)
- [ ] unit test 10+: identical inputs / partial overlap / contradictions / both low confidence / topic mismatch / retry attempt

## Files owned
- `src/lib/synthesis/{merge.ts,similarity.ts,types.ts,index.ts}`
- `src/lib/synthesis/__tests__/*.test.ts`

## Read-only
- T6 의 src/lib/synthesisTypes.ts (TypeScript type 합의)
- synthesis.md (sample)

## NEVER 영역
- src-tauri/src/synthesis/* (Rust 측 — 본 ticket 은 frontend pure logic)
- src/components/* (T6)
- src-tauri/* (T2/T4/T5/T7)

## Stop conditions
- T6 의 synthesisTypes 가 merge logic 표현 부족 → T6 와 협의해서 type 확장
- string-similarity threshold 0.85 가 sample 에서 false positive 다수 → 사용자에 알고리즘 변경 제안

## Deliverable (first-pass)
1. Diagnosis: synthesis.md 의 sample claim 들로 verify/disagreement 분류 mental run
2. Approach: similarity algorithm (Jaccard vs cosine vs Levenshtein) (대안 2개+pros/cons)
3. Risks (false positive — silent averaging 방지)
4. test fixtures 목록
5. Open questions

## Constraints
- 순수 함수, side effect X
- LLM 호출 X (synthesis 는 mechanical, adversarial 만 LLM)
- 6 항목 의무

[작업 완료 시 — 무조건 이 순서로]
1. commit: `feat(T3): deterministic synthesis engine` (본문에 `Closes #9` 포함, push 금지)
2. **GitHub 카드 완료 처리 — 잊지 말고 무조건 실행** (안 하면 칠판 https://github.com/users/mizan0515/projects 에 status:doing 으로 남아 다른 세션이 또 잡을 수 있음):
   ```
   node ~/.claude/scripts/gh-tickets.mjs complete D:\moa-desktop 9
   ```
   - 출력에 `COMPLETED=9` 또는 `ALREADY_CLOSED=9` 가 보여야 OK.
   - 실패 시 사용자 보고 + STOP. gh 인증 오류면 `gh auth refresh -s project,read:project` 안내.
3. 보고: similarity threshold 결정 근거, T7 가 호출할 API, edge case coverage, **GitHub 카드 close 결과 1줄**.
```
