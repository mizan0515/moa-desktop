# T16 — Harness Marketplace / Equipment Profiles

GitHub: #44 (https://github.com/mizan0515/moa-desktop/issues/44)

## Goal

"workflow 는 유지, 장비 구성만 교체"하는 UI 를 만든다. MoA orchestrator 의 flow/gate 는 그대로 두고 Claude/Codex/Pi runtime, model, thinking, toolset, extension pack, budget, safety level 을 profile 로 선택한다.

## 의존성

- 선행: T15 전체.
- 선행: T13 policy/settings lifecycle.

## Success criteria

- [ ] Equipment profile schema: `id`, `label`, `runtimeMix`, `models`, `thinking`, `toolset`, `budget`, `extensionPacks`, `safetyLevel`, `allowedFlow`.
- [ ] built-in profiles:
  - Cheap/Fast research: Pi + low-cost model
  - Deep challenge: `CodexAdversarialXHigh`
  - Claude semantic author
  - Codex mechanical author
  - Pi exploratory tool-rich lane
- [ ] profile change 는 active session 에 즉시 적용하지 않고 next turn/lane/session boundary 에 적용한다.
- [ ] destructive profile change 는 user confirm 필요.
- [ ] `CodexAdversarialXHigh` profile 은 mandatory gate profile 이며 marketplace profile 로 disable 할 수 없다.
- [ ] extension pack 은 T15d trust policy 통과 package 만 포함한다.
- [ ] cost/budget estimate 와 safety level 이 UI 에 표시된다.

## Files owned

- `src-tauri/src/harness_profiles/*.rs`
- `src-tauri/tests/harness_profiles_*.rs`
- `src/components/HarnessProfilePanel.tsx`
- `src/lib/harnessProfiles.ts`

## NEVER 영역

- mandatory `CodexAdversarialXHigh` disable
- active mutation turn 에 destructive profile 즉시 적용
- untrusted extension pack activation
- profile 이 T13/T15 safety policy 보다 높은 권한 획득

## Worker prompt 6 mandatory fields

1. Success criteria: schema, built-ins, boundary application, destructive confirm, mandatory gate immutable, trusted extension packs, budget/safety UI.
2. NEVER 영역: gate disable, active mutation destructive apply, untrusted extension pack, policy privilege escalation.
3. Validation cmd:
   ```powershell
   cargo test --manifest-path src-tauri\Cargo.toml harness_profiles
   npm test -- --run HarnessProfilePanel
   ```
4. Files + lines: this ticket Success criteria, T15 package/session/profile invariants, T13 policy/settings lifecycle.
5. Alternatives 2개 + pros/cons + 선택 근거: raw settings editor(fast but unsafe/opaque) vs curated equipment profiles(clear and policy-aware). 선택은 curated profiles.
6. Tests-first: mandatory gate immutable, next-boundary application, untrusted extension pack denial, destructive confirm tests 를 먼저 실패시킨다.
