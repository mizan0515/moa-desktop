# T16 — Harness Marketplace / Equipment Profiles

GitHub: #44 (https://github.com/mizan0515/moa-desktop/issues/44)

## 새 Claude 창 만들기 가이드
T15 전체 + T13 통과 후. worktree: T16-harness.

---

````
[세션 부트]
- repo: D:\moa-desktop
- base branch: master (T15 전체 + T13 머지 후)
- 권장 분기: feat/T16-harness-marketplace-equipment-profiles
- 권위: PROJECT-RULES.md, AGENTS.md, DESIGN.md, TICKETS/T15*.md, TICKETS/T13-policy-lifecycle-epic.md
- 운영: MoA Flow C — § 2.6 템플릿 A

[의존성 self-check — claim 직후, first-pass 시작 전 무조건 실행]
master 에 T15 series + T13 commit 있는지 확인:
```
cd D:\moa-desktop && git log master --oneline -100 | rg -i "feat\(T15b\)|feat\(T15c\)|feat\(T15d\)|feat\(T15e\)|feat\(T15f\)|feat\(T15g\)|feat\(T13\)" | wc -l
```
- 결과 `7` 면 OK — 작업 진행
- 7 미만이면 **STOP — "선행 티켓이 master 에 미머지" 사용자 보고**.

[INDEPENDENT FIRST-PASS — read-only]
````

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

## Worker prompt 6 mandatory fields
1. Success criteria: Equipment profile schema (`id`, `label`, `runtimeMix`, `models`, `thinking`, `toolset`, `budget`, `extensionPacks`, `safetyLevel`, `allowedFlow`), built-in profiles 5개, profile change 는 next turn/lane/session boundary 적용, destructive change user confirm, `CodexAdversarialXHigh` mandatory gate disable 불가, extension pack trust policy 통과만 포함, cost/budget + safety level UI 표시를 구현한다.
2. NEVER 영역: profile 로 mandatory CodexAdversarialXHigh gate 끄기, untrusted Pi package/extension auto-activate, active mutation lock 중 runtime/model/toolset 교체, profile 이 worker nested peer-call 허용, worker 직접 peer 호출.
3. Validation cmd:
   ```powershell
   cargo test --manifest-path src-tauri\Cargo.toml harness_profiles
   npm test -- --run "HarnessMarketplace|EquipmentProfile"
   rg -n "HarnessMarketplace|EquipmentProfile|CodexAdversarialXHigh|runtimeMix|extensionPacks|safetyLevel" src-tauri/src src TICKETS DESIGN.md PLAN.md
   ```
4. Files + lines: `TICKETS/T16-harness-marketplace-equipment-profiles.md` 의 Success criteria/NEVER, T15 series 의 Pi runtime/package/session APIs, `TICKETS/T13-policy-lifecycle-epic.md` 의 settings/policy lifecycle, T10/T11 runtimeKind schema.
5. Alternatives 2개 + pros/cons + 선택 근거: Settings-only toggles(UI 비용 적지만 equipment mix 추론 어려움) vs Equipment profiles(workflow 보존 + runtime/model/tool 선택 명시, profile migration/policy validation 필요). 선택은 Equipment profiles. Marketplace-first package browser(Pi UX 매력적이지만 trust/policy 미성숙 단계에서 위험) 기각.
6. Tests-first: mandatory gate disable 불가, untrusted extension pack 거절, active mutation lock 시 profile change 차단, profile migration roundtrip, budget display 를 먼저 실패시키고 구현한다.

## Worker prompt cross-contract fields
- GitHub/project handling: GitHub #44 / Project `MoA Desktop` card status 를 claim/complete 단계에서 갱신한다.
- Conflict matrix ownership: T16 owns 는 `src-tauri/src/harness_profiles/*.rs`, `src-tauri/tests/harness_profiles_*.rs`, `src/components/HarnessMarketplace.tsx`, `src/components/EquipmentProfilePicker.tsx`, `src/lib/harnessProfiles.ts` 로 한정한다. T13 settings/policy, T15 Pi runtime/package/session, T10/T11 runtimeKind schema 는 read-only.
- Dependency/merge order: T15 전체 + T13 완료 후 시작.
- Review gate warning: lead/orchestrator-owned `CodexAdversarialXHigh` 가 `Clean` 을 반환하고 `source_output_path` 가 persisted 되기 전에는 `pr_create`, `pr_merge`, `integrate_merge`, `main_apply` 진행 금지. worker 는 직접 peer review 를 실행하지 않는다.

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

[작업 완료 시 — 무조건 이 순서로]
1. commit: `feat(T16): Harness Marketplace + Equipment Profiles + policy schema` (본문에 `Closes #44` 포함, push 금지)
2. **GitHub 카드 완료 처리 — 잊지 말고 무조건 실행**:
   ```
   node ~/.claude/scripts/gh-tickets.mjs complete D:\moa-desktop 44
   ```
   - 출력에 `COMPLETED=44` 또는 `ALREADY_CLOSED=44` 가 보여야 OK.
   - 실패 시 사용자 보고 + STOP.
3. 보고: profile schema, built-in profiles, safety validation, migration notes, **GitHub 카드 close 결과 1줄**.
