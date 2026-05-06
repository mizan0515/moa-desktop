# T10 — Ticket Decomposer (`/병행티켓` 등가)

## 새 Claude 창 만들기 가이드
T7-full + T5a + T5b 통과 후 (Phase 6 v1.5 진입). worktree: T10-decomposer.

---

```
[세션 부트]
- repo: D:\moa-desktop
- base branch: master (T7-full + T5a + T5b 머지 후)
- 권장 분기: feat/T10-decomposer
- 권위: PROJECT-RULES.md, AGENTS.md, ~/.claude/CODEX-MCP.md (§ 2.5 흐름 C/D, § 2.6 템플릿), DESIGN.md (## v1.5 scope), PLAN.md (§ Phase 6, § F6 lock ordering)
- 글로벌 reference: ~/.claude/plugins/...skills/병행티켓 (해당 skill 의 prompt 패턴 본받기)
- 운영: MoA Flow C — § 2.6 템플릿 A

[의존성 self-check — claim 직후, first-pass 시작 전 무조건 실행]
master 에 선행 commit 3개 있는지 확인:
```
cd D:\moa-desktop && git log master --oneline -100 | rg -i "feat\(T7-full\)|feat\(T5a\)|feat\(T5b\)" | wc -l
```
- 결과 `3` 면 OK — 작업 진행
- 3 미만이면 **STOP — "선행 티켓이 master 에 미머지" 사용자 보고**.

[INDEPENDENT FIRST-PASS — read-only]

## Goal
사용자 입력 = 큰 작업 텍스트 ("백로그 정리", "전체 refactor", "feature X 추가") → 본 ticket 의 decomposer 가:
1. 양측 MoA first-pass (T5a + T5b) 로 작업 분석
2. **충돌 없는 N 티켓** 으로 분해 (각 ticket 의 owns 영역 disjoint, NEVER 영역 일관)
3. 각 티켓에 paste-ready worker prompt 생성 (T1-T12 의 형식 mirror)
4. **의존성 그래프** 출력 (어떤 ticket 이 어떤 ticket 끝나야 시작 가능한지)
5. **머지 순서** 출력 (T12 integrator 가 사용)
6. UI 에 TicketBoard 컴포넌트로 표시 + 사용자 검토/수정/승인 → settings 에 저장

## Success criteria
- [ ] `src-tauri/src/decomposer/{prompt.rs,decompose.rs,graph.rs,mod.rs}` — prompt builder + MoA orchestrator 호출 + dependency graph 분석 + 머지 순서 결정
- [ ] `prompts/decomposer.txt` — first-pass 양측에 던지는 prompt (글로벌 § 2.6 템플릿 A 형식)
- [ ] `src/components/TicketBoard.tsx` — 분해 결과 카드 N 개 표시 (id, title, owns, deps, prompt preview), 사용자 edit/reorder 가능
- [ ] 분해 결과 schema (JSON):
  ```json
  {
    "tickets": [{"id": "T1", "title": "...", "owns": [...], "neverAreas": [...], "deps": [...], "prompt": "..."}],
    "graph": {"nodes": [...], "edges": [{"from": "T1", "to": "T2", "reason": "T2 reads T1 API"}]},
    "mergeOrder": ["T1", "T2", ...]
  }
  ```
- [ ] 충돌 검증: 분해 결과의 각 ticket owns 가 disjoint (집합 intersection 0), 모든 NEVER 영역에 다른 ticket 의 owns 포함, 의존 그래프에 cycle 없음 → 깨지면 사용자에 보고
- [ ] settings 에 분해 결과 저장 (`~/.moa-desktop/decompositions/<projectId>/<timestamp>.json`)
- [ ] integration test: 가짜 큰 작업 입력 → 분해 → schema 검증 → graph cycle 없음 + owns disjoint 확인

## Files owned
- `src-tauri/src/decomposer/*.rs` (mod.rs body 포함)
- `prompts/decomposer.txt`
- `src/components/TicketBoard.tsx` (T1 의 stub 채움)
- `src-tauri/tests/decomposer_*.rs`

## Read-only
- T5a/T5b adapter (worker 호출), T7-full orchestrator (MoA flow 재사용)
- 글로벌 `/병행티켓` skill 정의 (~/.claude/plugins/cache/.../skills/병행티켓/*) — pattern 참조
- DESIGN.md, PLAN.md, PROJECT-RULES.md, AGENTS.md

## NEVER 영역
- src-tauri/src/{parallel,integrator}/ body (T11/T12)
- src-tauri/src/{adapters,orchestrator,safety,git,lock,journal,synthesis,process}/ body
- main repo 의 다른 ticket owns 영역
- 비밀 파일

## Stop conditions
- 양측 MoA 가 같은 분해를 못 만들면 (충돌 해결 protocol 무한 루프) → 사용자 escalation
- 분해 결과의 owns 충돌이 자동 해결 안 됨 → 사용자에 충돌 표시 + 수동 수정 요청
- T7-full 의 orchestrator API 가 decomposer 재사용에 안 맞음 → T7-full 와 API 협의

## Deliverable (first-pass)
1. Diagnosis: 글로벌 `/병행티켓` skill 의 분해 휴리스틱 (어떻게 owns disjoint 보장하는지)
2. Approach: prompt 1 회 호출 vs iterative refine (대안 2 개 + pros/cons)
3. Risks: LLM 비결정성으로 분해 결과 불안정 → snapshot + 사용자 lock 가능 여부
4. Schema example (JSON sample 1 개)
5. Open questions

## Constraints
- 6 항목 의무
- 분해 prompt 자체에 6 항목 형식 강제 (생성된 N ticket 모두 6 항목 포함)
- read-only first-pass + mutation owner 분리 (decomposer 자체는 mutation 만드는 게 아니라 ticket 정의만)
- 비밀 파일 access X

[작업 완료 시 — 무조건 이 순서로]
1. commit: `feat(T10): ticket decomposer (병행티켓 등가) + TicketBoard UI` (본문에 `Closes #15` 포함, push 금지)
2. **GitHub 카드 완료 처리 — 잊지 말고 무조건 실행** (안 하면 칠판 https://github.com/users/mizan0515/projects 에 status:doing 으로 남아 다른 세션이 또 잡을 수 있음):
   ```
   node ~/.claude/scripts/gh-tickets.mjs complete D:\moa-desktop 15
   ```
   - 출력에 `COMPLETED=15` 또는 `ALREADY_CLOSED=15` 가 보여야 OK.
   - 실패 시 사용자 보고 + STOP. gh 인증 오류면 `gh auth refresh -s project,read:project` 안내.
3. 보고: 분해 결과 schema, graph cycle 검증 알고리즘, T11 parallel runner 가 본 결과를 어떻게 소비하는지, **GitHub 카드 close 결과 1줄**.
```
