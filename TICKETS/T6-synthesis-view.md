# T6 — Synthesis view + Claim Ledger UI

## 새 Claude 창 만들기 가이드
1. T1 통과 후 (T8 와 병렬 가능 — 다른 폴더). worktree: T6-synthview
2. 프롬프트 붙여넣기

---

```
[세션 부트]
- repo: D:\moa-desktop
- base branch: master (T1 머지 후)
- 권장 분기: feat/T6-synthview
- 권위: DESIGN.md (UI 섹션), synthesis.md (5칸 schema 예시), PLAN.md, ~/.claude/CODEX-MCP.md
- 운영: MoA Flow C — § 2.6 템플릿 A

[INDEPENDENT FIRST-PASS — read-only]

## Goal
5칸 synthesis 표 + Claim Ledger (max 5) UI 컴포넌트. mock JSON 으로 렌더 가능.

## Success criteria
- [ ] SynthesisView.tsx: 5칸 (Verified / Claude-only / Codex-only / Disagreement / Open) 표. 좁은 폭에서는 column 을 collapsible accordion 으로 fallback
- [ ] ClaimLedger.tsx: claim 1줄 + evidence/level/conf/risk 한 줄로 inline. card 중첩 X
- [ ] src/lib/synthesisTypes.ts: 두 컴포넌트 + T3 (engine) + T8 (mock) 가 공유할 type 정의
- [ ] storybook 또는 dev route (`/dev/synthview-demo`) 로 mock JSON 렌더 시각 검증
- [ ] 회귀 0 (T1 의 workbench 가 빈 placeholder 였던 자리에 이 컴포넌트가 들어가도 layout 깨지지 않음)

## Files owned
- `src/components/SynthesisView.tsx` (T1 의 stub 을 채움)
- `src/components/ClaimLedger.tsx` (T1 의 stub 을 채움)
- `src/lib/synthesisTypes.ts`
- `src/dev/SynthViewDemo.tsx` (dev-only route)

## Read-only
- T1 의 src/App.tsx, src/components/Workbench/*
- T8 의 mockResponses/synthesis.json, final_report.json
- DESIGN.md, synthesis.md

## NEVER 영역
- src/lib/synthesis/* (T3 — synthesis engine 로직)
- src-tauri/* (T6 는 frontend 전용)
- T1 영역의 App.tsx 구조 변경 (route 추가만)

## Stop conditions
- mock JSON schema 가 5칸 + Claim Ledger 표현 부족 → T8 worker 에 schema 추가 요청 후 진행
- 좁은 폭 design 합의 안 됨 → wireframe 1차 안 작성 후 사용자 confirm

## Deliverable (first-pass)
1. Diagnosis: DESIGN.md UI 요구사항 정리 + synthesis.md § 1-§ 5 의 표 구조 분석
2. Approach: 표 vs accordion vs split panel (대안 2개+pros/cons)
3. Risks
4. wireframe 텍스트 묘사
5. Open questions (col span, mobile breakpoint 등)

## Constraints
- 6 항목 의무
- 카드 중첩 금지, 텍스트 overlap 금지 (DESIGN.md UI 원칙)
- emoji 금지 (사용자 명시 없으면)

[작업 완료 시 — 무조건 이 순서로]
1. commit: `feat(T6): synthesis view + claim ledger UI` (본문에 `Closes #4` 포함, push 금지)
2. **GitHub 카드 완료 처리 — 잊지 말고 무조건 실행** (안 하면 칠판 https://github.com/users/mizan0515/projects 에 status:doing 으로 남아 다른 세션이 또 잡을 수 있음):
   ```
   node ~/.claude/scripts/gh-tickets.mjs complete D:\moa-desktop 4
   ```
   - 출력에 `COMPLETED=4` 또는 `ALREADY_CLOSED=4` 가 보여야 OK.
   - 실패 시 사용자 보고 + STOP. gh 인증 오류면 `gh auth refresh -s project,read:project` 안내.
3. 보고: dev demo route, 좁은 폭 검증, T3 synthesis engine 이 만족해야 할 type contract, **GitHub 카드 close 결과 1줄 (`COMPLETED=4` 출력 그대로)**.
```
