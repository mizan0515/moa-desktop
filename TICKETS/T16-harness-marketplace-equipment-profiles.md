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
- `src/components/HarnessMarketplace.tsx`
- `src/components/EquipmentProfilePicker.tsx`
- `src/lib/harnessProfiles.ts`

## Read-only

- T13 settings/policy
- T15 Pi runtime/package/session APIs
- T10/T11 runtimeKind schema

## NEVER 영역

- profile 로 mandatory `CodexAdversarialXHigh` gate 를 끄지 않는다.
- untrusted Pi package/extension pack 을 profile 에 자동 활성화하지 않는다.
- active mutation lock 중 runtime/model/toolset 을 바꾸지 않는다.
- profile 선택이 worker nested peer-call 을 허용하지 않는다.

## Validation cmd

```powershell
cargo test --manifest-path src-tauri\Cargo.toml harness_profiles
npm test -- --run "HarnessMarketplace|EquipmentProfile"
rg -n "HarnessMarketplace|EquipmentProfile|CodexAdversarialXHigh|runtimeMix|extensionPacks|safetyLevel" src-tauri/src src TICKETS DESIGN.md PLAN.md
```

## Alternatives

1. Settings-only toggles
   - Pros: low UI cost.
   - Cons: hard for users to reason about equipment mix.
2. Equipment profiles (선택)
   - Pros: preserves workflow while making runtime/model/tool choices explicit.
   - Cons: needs profile migration and policy validation.
3. Marketplace-first package browser
   - Pros: attractive Pi UX.
   - Cons: too risky before trust/policy is mature.

## Tests-first

Failing tests first: mandatory gate cannot be disabled, untrusted extension pack rejected, active mutation lock blocks profile change, profile migration roundtrip, budget display.

## Paste-ready prompt

```text
[세션 부트]
- Prompt kind: Codex Desktop manual lead ticket session
- repo: D:\moa-desktop
- branch: codex/T16-harness-marketplace-equipment-profiles
- worktree required

[Goal]
Harness Marketplace / Equipment Profiles UI 와 policy schema 를 구현한다.

[NEVER]
disable CodexAdversarialXHigh, auto-enable untrusted package, active mutation lock switch 금지.

[Validation]
cargo test --manifest-path src-tauri\Cargo.toml harness_profiles
npm test -- --run "HarnessMarketplace|EquipmentProfile"

[작업 완료 시]
profile schema, built-in profiles, safety validation, migration notes 를 보고한다.
```
