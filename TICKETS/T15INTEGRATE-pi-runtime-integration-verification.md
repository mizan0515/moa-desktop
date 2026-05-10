# T15INTEGRATE — Pi Runtime Integration Verification

GitHub: #45 (https://github.com/mizan0515/moa-desktop/issues/45)

## 새 Claude 창 만들기 가이드
T15b~T15g + T14 + T16 모두 머지 후. lead 세션에서 직접 진행 권장.

---

```
[세션 부트]
- repo: D:\moa-desktop
- base branch: master (T15b~T15g + T14 + T16 모두 머지 후)
- 권장 분기: feat/T15INTEGRATE-pi-verification
- 권위: PROJECT-RULES.md, AGENTS.md, DESIGN.md, TICKETS/T15*.md, TICKETS/T14-conversational-mode.md, TICKETS/T16-harness-marketplace-equipment-profiles.md
- 운영: MoA Flow C — § 2.6 템플릿 A

[의존성 self-check — claim 직후, first-pass 시작 전 무조건 실행]
master 에 T15 series + T14 + T16 commit 있는지 확인:
```
cd D:\moa-desktop && git log master --oneline -200 | rg -i "feat\(T15b\)|feat\(T15c\)|feat\(T15d\)|feat\(T15e\)|feat\(T15f\)|feat\(T15g\)|feat\(T14\)|feat\(T16\)" | wc -l
```
- 결과 `8` 면 OK — 작업 진행
- 8 미만이면 **STOP — "선행 티켓이 master 에 미머지" 사용자 보고** + 누락 commit 목록 작성.

[INDEPENDENT FIRST-PASS — read-only]

## Goal

T15b/T15c/T15d/T15e/T15f/T15g/T14/T16 completion after merge verification. Validate that Pi is a MoA parent-owned `HarnessRuntime`, not a worker nested peer-call, and that package/extension/session/profile safety gates remain intact.

## Success criteria

- [ ] Pi RPC, SDK sidecar, package trust, extension UI, model/session tree, native extensions, conversational mode, and equipment profiles interoperate.
- [ ] Mandatory `CodexAdversarialXHigh` gate cannot be disabled or replaced by Pi review.
- [ ] Pi mutation owner remains off unless T15g opt-in prerequisites are all present.
- [ ] Project-local package auto-install and package auto-update are blocked.
- [ ] ResumePacket/journal remain source of truth.
- [ ] Integration report records PASS/FAIL/UNVERIFIED for every T15 capability.

## Files owned

- integration tests/docs only, exact paths selected after T15g/T16 merge

## NEVER 영역

- 새 feature scope 구현
- T13/T15 safety policy relaxation
- auto-install/update Pi packages
- mandatory review gate bypass

## Worker prompt 6 mandatory fields

1. Success criteria: above integration matrix and report.
2. NEVER 영역: new feature scope, policy relaxation, auto install/update, gate bypass.
3. Validation cmd:
   ```powershell
   cargo test --manifest-path src-tauri\Cargo.toml pi
   npm test -- --run Pi
   rg -n "runtimeKind|CodexAdversarialXHigh|capability manifest|pi install|pi update" DESIGN.md PLAN.md TICKETS src src-tauri
   ```
4. Files + lines: T15b-g, T14, T16 final reports and integration tests.
5. Alternatives 2개 + pros/cons + 선택 근거: per-ticket verification only(fast but misses cross-ticket drift) vs final integration gate(slower but catches boundary regressions). 선택은 final integration gate.
6. Tests-first: gate replacement denial, package auto-install denial, ResumePacket source-of-truth, mutation opt-in prerequisites tests 를 먼저 실패시킨다.

## Worker prompt cross-contract fields
- GitHub/project handling: GitHub #45 / Project `MoA Desktop` card status 를 claim/complete 단계에서 갱신한다.
- Conflict matrix ownership: T15INTEGRATE owns 는 integration tests/docs only (exact paths selected after T15g/T16 merge). 새 feature scope 구현 금지. T13/T15 safety policy relaxation 금지.
- Dependency/merge order: T15b~T15g + T14 + T16 모두 완료 후 시작. TINTEGRATE 이전.
- Review gate warning: lead/orchestrator-owned `CodexAdversarialXHigh` 가 `Clean` 을 반환하고 `source_output_path` 가 persisted 되기 전에는 `pr_create`, `pr_merge`, `integrate_merge`, `main_apply` 진행 금지. worker 는 직접 peer review 를 실행하지 않는다.

## Paste-ready prompt

```text
[세션 부트]
- Prompt kind: Codex Desktop manual lead ticket session
- repo: D:\moa-desktop
- branch: codex/T15INTEGRATE-pi-verification
- worktree required

[Goal]
T15 series + T14 + T16 통합 검증. Pi 가 MoA parent-owned HarnessRuntime 이며 safety gates 가 intact 인지 확인한다.

[NEVER]
새 feature scope, T13/T15 safety policy relaxation, auto-install/update, mandatory review gate bypass 금지.

[Validation]
cargo test --manifest-path src-tauri\Cargo.toml pi
npm test -- --run Pi

[작업 완료 시]
integration report (PASS/FAIL/UNVERIFIED per capability), follow-up issues 를 보고한다.
```

[작업 완료 시 — 무조건 이 순서로]
1. commit: `feat(T15INTEGRATE): Pi runtime integration verification report` (본문에 `Closes #45` 포함, push 금지)
2. **GitHub 카드 완료 처리 — 잊지 말고 무조건 실행**:
   ```
   node ~/.claude/scripts/gh-tickets.mjs complete D:\moa-desktop 45
   ```
   - 출력에 `COMPLETED=45` 또는 `ALREADY_CLOSED=45` 가 보여야 OK.
   - 실패 시 사용자 보고 + STOP.
3. 보고: integration report (PASS/FAIL/UNVERIFIED per capability), follow-up issues, **GitHub 카드 close 결과 1줄**.
