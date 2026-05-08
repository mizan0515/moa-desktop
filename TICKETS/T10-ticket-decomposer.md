# T10 — Pi-aware Ticket Decomposer (`/병행티켓` 등가)

## 새 Claude 창 만들기 가이드
T7-full + T5a + T5b 통과 후 (Phase 6 v1.5 진입). worktree: T10-decomposer.

---

```
[세션 부트]
- repo: D:\moa-desktop
- base branch: master (T7-full + T5a + T5b 머지 후)
- 권장 분기: feat/T10-decomposer
- 권위: PROJECT-RULES.md, AGENTS.md, T13 PolicyPack/RuntimeProfile resolver, DESIGN.md (## v1.5 scope), PLAN.md (§ Phase 6, § F6 lock ordering)
- 글로벌 reference: T13 이 resolve 한 현재 `/병행티켓` source. Codex Desktop 수동 개발에서는 `C:\Users\mizan\.codex\skills\병행티켓\SKILL.md` 를 현 스킬 패턴으로 참조한다. `~/.claude/plugins/...` raw path 를 직접 copy source 로 삼지 않는다.
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
6. **reviewGate 계획** 출력 — PR 생성 전/머지 전/통합 전/main 적용 전 lead/orchestrator-owned mandatory `CodexAdversarialXHigh` review gate 가 필요한 지점. 앱 실행 시에는 orchestrator review profile, Codex Desktop 수동 개발 시에는 lead PowerShell 별도 리뷰 프로파일과 `.moa-desktop/reviews/<stamp>.md` output capture 가 gate 증거가 될 수 있다. PrimaryRole=Codex 인 경우 ClaudeSymmetry 는 추가 검토일 뿐 `CodexAdversarialXHigh` 를 대체하지 않는다. worker 가 peer AI 를 직접 호출하는 nested peer-call 은 생성 prompt 에 넣지 않는다.
7. UI 에 TicketBoard 컴포넌트로 표시 + 사용자 검토/수정/승인 → settings 에 저장
8. **Pi-aware runtime metadata** 출력 — 각 ticket/lane 이 `runtimeKind: "claude" | "codex" | "pi"`, `allowedHarnesses`, `piExtensionPolicyRef` 를 명시한다. T15b 이전이면 Pi 는 `allowedHarnesses` 후보가 아니라 future capability 로만 표시한다.

## Success criteria
- [ ] `src-tauri/src/decomposer/{prompt.rs,decompose.rs,graph.rs,mod.rs}` — prompt builder + MoA orchestrator 호출 + dependency graph 분석 + 머지 순서 결정
- [ ] `prompts/decomposer.txt` — first-pass 양측에 던지는 prompt (글로벌 § 2.6 템플릿 A 형식)
- [ ] `src/components/TicketBoard.tsx` — 분해 결과 카드 N 개 표시 (id, title, owns, deps, prompt preview), 사용자 edit/reorder 가능
- [ ] 분해 결과 schema (JSON):
  ```json
  {
    "phaseGuide": [{"phase": 1, "action": "창 N개 동시 실행", "betweenPhaseAction": "PR 머지 + git pull origin main"}],
    "inventory": [{"id": "T1", "issue": 123, "type": "feature", "phase": 1, "note": "..."}],
    "tickets": [{
      "id": "T1",
      "title": "...",
      "runtimeKind": "claude",
      "allowedHarnesses": ["claude", "codex", "pi"],
      "piExtensionPolicyRef": null,
      "owns": [...],
      "deps": [...],
      "sixMandatoryFields": {"successCriteria": [...], "neverAreas": [...], "validationCmd": "...", "filesAndLines": [...], "alternatives": [...], "testsFirst": true},
      "github": {"issue": 123, "project": "MoA Desktop", "status": "Todo"},
      "prompt": "..."
    }],
    "conflictMatrix": [{"surface": "src/foo", "T1": "owner", "T2": "금지"}],
    "dependencyGraph": {"nodes": [...], "edges": [{"from": "T1", "to": "T2", "reason": "T2 reads T1 API"}]},
    "mergeOrder": ["T1", "T2", ...],
    "reviewGate": {
      "mode": "lead-or-orchestrator-owned",
      "mandatoryCodexAdversarial": true,
      "reviewProfileId": "CodexAdversarialXHigh",
      "reasoningEffort": "xhigh",
      "requiredAuditFields": ["verdict", "reviewer", "review_kind", "review_profile_id", "reasoning_effort", "model_or_profile_id", "prompt_template_version", "prompt_hash", "command_source_adapter", "primary_role", "scope", "gate", "patch_hash", "files_reviewed", "omitted_files", "limitations", "evidence", "required_actions", "created_at", "source_output_path"],
      "optionalClaudeSymmetryWhenPrimaryCodex": true,
      "before": ["pr_create", "pr_merge", "integrate_merge", "main_apply"],
      "workerDirectPeerCall": false,
      "allowedExecutors": ["moa-orchestrator", "codex-desktop-lead-powershell", "codex-desktop-lead-powershell-controlled-bypass"],
      "outputLastMessageRequired": true
    },
    "reviewRunRecords": [],
    "mergeGuidance": {"betweenPhases": "PR 머지 + git pull origin main", "manualMergeOwner": "user"}
  }
  ```
- [ ] 충돌 검증: 분해 결과의 각 ticket owns 가 disjoint (집합 intersection 0), 모든 NEVER 영역에 다른 ticket 의 owns 포함, 의존 그래프에 cycle 없음 → 깨지면 사용자에 보고
- [ ] 생성된 paste-ready worker prompt 의 [작업 완료 시] 에 "lead/orchestrator-owned mandatory `CodexAdversarialXHigh` review gate Clean 전 `pr_create`, `pr_merge`, `integrate_merge`, `main_apply` 진행 금지" 안내가 들어감.
- [ ] 생성된 paste-ready worker prompt 의 worker 실행 영역에는 peer AI 직접 호출 패턴(`/codex:`, `claude -p`, `codex exec`, `Claude MCP`, `Codex MCP`) 이 들어가지 않음. 앱 review 는 T13 L2.5/L4 가 source=orchestrator 로 수행하고, Codex Desktop 수동 개발 review 는 lead PowerShell 에서 worker 밖 별도 프로세스로 수행한다.
- [ ] Pi-aware amend: decomposer output 의 모든 ticket 에 `runtimeKind`, `allowedHarnesses`, `piExtensionPolicyRef` 가 있다. Pi runtime 을 선택한 ticket 은 initial scope 가 read-only/research/reviewer/conversational 인지 검증하고 mutation owner 는 T15g opt-in 전까지 reject 한다.
- [ ] 글로벌 `/병행티켓` 출력 계약 10 섹션(현재 상태 요약, inventory, 분해 전략, 의존 그래프, 충돌 매트릭스, 진행 가이드, Phase 별 prompt, 머지 순서, Open questions, 사고 방지)을 schema 로 보존한다.
- [ ] GitHub issue/project 등록 metadata 와 Phase 사이 행동(`PR 머지 + git pull origin main`) 이 누락되지 않는다.
- [ ] settings 에 분해 결과 저장 (`~/.moa-desktop/decompositions/<projectId>/<timestamp>.json`)
- [ ] integration test: 가짜 큰 작업 입력 → 분해 → schema 검증 → graph cycle 없음 + owns disjoint 확인

## Files owned
- `src-tauri/src/decomposer/*.rs` (mod.rs body 포함)
- `prompts/decomposer.txt`
- `src/components/TicketBoard.tsx` (T1 의 stub 채움)
- `src-tauri/tests/decomposer_*.rs`

## Read-only
- T5a/T5b adapter (worker 호출), T7-full orchestrator (MoA flow 재사용)
- T13 PolicyPack 이 resolve 한 `/병행티켓` skill source — pattern 참조. Codex Desktop 수동 개발 시 현재 source 는 `C:\Users\mizan\.codex\skills\병행티켓\SKILL.md`.
- DESIGN.md, PLAN.md, PROJECT-RULES.md, AGENTS.md

## NEVER 영역
- src-tauri/src/{parallel,integrator}/ body (T11/T12)
- src-tauri/src/{policy,safety,commands,lifecycle}/ body (T13 owns)
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
- 생성 prompt 는 `/병행티켓` 글로벌 스킬의 paste-ready 규칙(본문 코드블록, [세션 부트]~[작업 완료 시], Phase 가이드, conflict matrix, dependency graph, GitHub issue/project metadata, tests-first, Phase 사이 `PR 머지 + git pull origin main`)을 지키되, PR/merge review 는 worker 직접 호출이 아니라 lead/orchestrator-owned gate 로 표현한다.
- 비밀 파일 access X

## T15 Pi Runtime amend

- `HarnessRuntimeKind = "claude" | "codex" | "pi"` 를 schema vocabulary 로 사용한다.
- `runtimeKind` 는 preferred lane runtime 이고, `allowedHarnesses` 는 fallback 후보 목록이다.
- `piExtensionPolicyRef` 는 T15d package/capability manifest reference 이며, 없으면 Pi extension UI/custom tool 은 disabled 상태로 생성한다.
- Pi runtime 은 decomposer 관점에서 "장비 후보"다. workflow, review gate, dependency graph, conflict matrix 는 MoA 가 계속 소유한다.
- generated prompt 에 Pi package install/update/hot reload 지시를 넣지 않는다. 필요한 경우 "T15d policy confirm 필요" 로만 표기한다.

## Worker prompt 6 mandatory fields
1. Success criteria: 큰 작업 입력을 T10 schema 로 분해하고 `phaseGuide`, `inventory`, `tickets`, `runtimeKind`, `allowedHarnesses`, `piExtensionPolicyRef`, `conflictMatrix`, `dependencyGraph`, `mergeOrder`, `reviewGate`, `reviewRunRecords`, `mergeGuidance` 를 모두 채운다.
2. NEVER 영역: T11/T12/T13 owned body, adapters/orchestrator/safety/git/lock/journal/synthesis/process body, 비밀 파일, worker 직접 peer 호출 패턴.
3. Validation cmd:
   ```powershell
   cargo test --manifest-path src-tauri\Cargo.toml decomposer
   npm test -- --run TicketBoard
   ```
4. Files + lines: `TICKETS/T10-ticket-decomposer.md` 의 Goal/Success criteria/Constraints/T15 amend, `C:\Users\mizan\.codex\skills\병행티켓\SKILL.md` 의 출력 계약과 paste-ready 필수 내용, `DESIGN.md` 의 policy/review gate/Pi Runtime section.
5. Alternatives 2개 + pros/cons + 선택 근거: 단일 LLM 분해(빠르지만 owns 충돌 위험) vs MoA first-pass + deterministic validator(느리지만 conflict matrix/reviewGate 누락 방지). 선택은 MoA first-pass + deterministic validator.
6. Tests-first: schema validator, owns disjoint/cycle 검증, nested peer-call scanner dry-run, TicketBoard render/edit test 를 먼저 실패시키고 구현한다.

## Worker prompt cross-contract fields
- GitHub/project handling: 생성된 모든 ticket 에 GitHub issue number, Project `MoA Desktop` status, claim/complete command 또는 수동 처리 방식을 포함한다. T10 자체는 GitHub #15 card 를 완료 처리한다.
- Conflict matrix ownership: 각 ticket 의 `owns` 는 disjoint 이어야 하며, 다른 ticket owns 는 해당 prompt 의 NEVER 영역과 conflict matrix 에 금지로 들어간다. 충돌이 있으면 사용자에게 수정 UI 를 표시하고 자동 병행 실행하지 않는다.
- Dependency/merge order: `dependencyGraph` cycle 이 없어야 하며, `mergeOrder` 와 `phaseGuide.betweenPhaseAction = "PR 머지 + git pull origin main"` 을 T11/T12 입력으로 보존한다.
- Review gate warning: lead/orchestrator-owned `CodexAdversarialXHigh` 가 `Clean` 을 반환하고 `source_output_path` 가 persisted 되기 전에는 `pr_create`, `pr_merge`, `integrate_merge`, `main_apply` 진행 금지. worker prompt 는 직접 peer review 를 실행하지 않는다.

[작업 완료 시 — 무조건 이 순서로]
1. commit: `feat(T10): ticket decomposer (병행티켓 등가) + TicketBoard UI` (본문에 `Closes #15` 포함, push 금지)
2. **Review gate dry-run**: 생성된 sample ticket prompt 1개에 대해 T13 L2 scanner + command_guard 를 실행해 worker 실행 영역의 nested peer-call 패턴이 0건인지 검증. 동시에 reviewGate metadata 가 `lead-or-orchestrator-owned`, `mandatoryCodexAdversarial=true`, `reviewProfileId=CodexAdversarialXHigh`, `reasoningEffort=xhigh`, `before=["pr_create","pr_merge","integrate_merge","main_apply"]`, `allowedExecutors=["moa-orchestrator","codex-desktop-lead-powershell","codex-desktop-lead-powershell-controlled-bypass"]`, `requiredAuditFields` 에 `verdict` 포함으로 저장됐는지 확인.
3. **GitHub 카드 완료 처리 — 잊지 말고 무조건 실행** (안 하면 칠판 https://github.com/users/mizan0515/projects 에 status:doing 으로 남아 다른 세션이 또 잡을 수 있음):
   ```
   node ~/.claude/scripts/gh-tickets.mjs complete D:\moa-desktop 15
   ```
   - 출력에 `COMPLETED=15` 또는 `ALREADY_CLOSED=15` 가 보여야 OK.
   - 실패 시 사용자 보고 + STOP. gh 인증 오류면 `gh auth refresh -s project,read:project` 안내.
4. 보고: 분해 결과 schema, graph cycle 검증 알고리즘, reviewGate metadata, T11 parallel runner 가 본 결과를 어떻게 소비하는지, **GitHub 카드 close 결과 1줄**.
```
