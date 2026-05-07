# T15INTEGRATE — Pi Runtime Integration Verification

GitHub: #45 (https://github.com/mizan0515/moa-desktop/issues/45)

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
