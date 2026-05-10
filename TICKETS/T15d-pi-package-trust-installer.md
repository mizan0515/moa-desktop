# T15d — Pi Package Trust & Installer

GitHub: #40 (https://github.com/mizan0515/moa-desktop/issues/40)

## 새 Claude 창 만들기 가이드
T15c + T13 통과 후. worktree: T15d-package-trust.

---

```
[세션 부트]
- repo: D:\moa-desktop
- base branch: master (T15c + T13 머지 후)
- 권장 분기: feat/T15d-pi-package-trust-installer
- 권위: PROJECT-RULES.md, AGENTS.md, DESIGN.md, TICKETS/T15c-pi-sdk-sidecar-host.md, TICKETS/T13-policy-lifecycle-epic.md
- 운영: MoA Flow C — § 2.6 템플릿 A

[의존성 self-check — claim 직후, first-pass 시작 전 무조건 실행]
master 에 선행 commit 2개 있는지 확인:
```
cd D:\moa-desktop && git log master --oneline -100 | rg -i "feat\(T15c\)|feat\(T13\)" | wc -l
```
- 결과 `2` 면 OK — 작업 진행
- 2 미만이면 **STOP — "선행 티켓이 master 에 미머지" 사용자 보고**.

[INDEPENDENT FIRST-PASS — read-only]

## Goal

npm/git/local Pi package 설치를 MoA trust policy 로 감싼다. Pi packages 는 full system access 위험이 있으므로 자동 설치/자동 업데이트를 기본 금지한다.

## 의존성

- 선행: T15c SDK sidecar host.
- 선행: T13 PolicyPack/CommandGuard.

## Success criteria

- [ ] `PiPackagePolicy` schema: source, resolvedVersion, sha256, capabilities, trust metadata.
- [ ] source types: `npm:<name>@<version>`, `git:<url>#<sha>`, `local:<path>`.
- [ ] npm package 는 version pin 필수. semver range 와 latest floating 금지.
- [ ] git package 는 commit SHA pin 필수.
- [ ] local package 는 path canonicalization + hash manifest 필수.
- [ ] `autoUpdate=false` default. `pi update` 자동 실행 금지.
- [ ] install preview 는 diff + manifest + capability request + source review checkbox 를 보여준다.
- [ ] uninstall/disable/enable 이 audit record 를 남긴다.
- [ ] project-local `.pi/settings.json` 이 package 를 요구해도 user confirm 없이는 설치하지 않는다.
- [ ] package capability 가 command/network/filesystem/UI 권한을 확장하면 T13 policy confirm 없이는 inactive.

## Files owned

- `src-tauri/src/pi/package_policy.rs`
- `src-tauri/src/pi/package_installer.rs`
- `src-tauri/tests/pi_package_*.rs`
- `src/components/PiPackageManager.tsx`

## Read-only

- T13 policy/safety APIs
- T15c sidecar host IPC

## NEVER 영역

- unpinned install/update 금지.
- `pi install` / `pi update` 자동 실행 금지.
- capability manifest 없는 package 활성화 금지.
- source review 없이 third-party package 활성화 금지.
- worker/lane 이 package install 을 command source 로 직접 요청하지 못하게 한다.

## Validation cmd

```powershell
cargo test --manifest-path src-tauri\Cargo.toml pi_package
npm test -- --run PiPackageManager
rg -n "PiPackagePolicy|autoUpdate|sha256|capability manifest|pi install|pi update|sourceReviewed" src-tauri/src src components TICKETS
```

## Alternatives

1. Delegate to Pi package manager
   - Pros: least code.
   - Cons: MoA safety boundary bypass risk.
2. MoA trust wrapper around Pi install (선택)
   - Pros: Pi ecosystem usable with explicit user trust.
   - Cons: manifest/hash/cache complexity.
3. Disable packages entirely
   - Pros: safest.
   - Cons: removes a major reason to embed Pi.

## Tests-first

Failing tests first: floating version reject, missing sha reject, project-local auto install reject, capability escalation requires confirm, uninstall disables resource, audit record persistence.

## Worker prompt 6 mandatory fields
1. Success criteria: `PiPackagePolicy` schema, source types (npm/git/local) 각각 pin 필수, `autoUpdate=false` default, install preview UI, uninstall/disable/enable audit, project-local auto install 차단, capability escalation confirm 을 구현한다.
2. NEVER 영역: unpinned install/update, `pi install`/`pi update` 자동 실행, capability manifest 없는 package 활성화, source review 없이 third-party 활성화, worker/lane 이 package install 직접 요청.
3. Validation cmd:
   ```powershell
   cargo test --manifest-path src-tauri\Cargo.toml pi_package
   npm test -- --run PiPackageManager
   rg -n "PiPackagePolicy|autoUpdate|sha256|capability manifest|pi install|pi update|sourceReviewed" src-tauri/src src components TICKETS
   ```
4. Files + lines: `TICKETS/T15d-pi-package-trust-installer.md` 의 Success criteria/NEVER, `TICKETS/T15c-pi-sdk-sidecar-host.md` 의 sidecar IPC, `TICKETS/T13-policy-lifecycle-epic.md` 의 PolicyPack/CommandGuard.
5. Alternatives 2개 + pros/cons + 선택 근거: Pi package manager 에 위임(코드 적지만 MoA safety boundary bypass 위험) vs MoA trust wrapper(Pi ecosystem 사용 가능 + explicit user trust, manifest/hash/cache 복잡). 선택은 MoA trust wrapper.
6. Tests-first: floating version reject, missing sha reject, project-local auto install reject, capability escalation requires confirm, uninstall disables resource, audit record persistence 를 먼저 실패시키고 구현한다.

## Worker prompt cross-contract fields
- GitHub/project handling: GitHub #40 / Project `MoA Desktop` card status 를 claim/complete 단계에서 갱신한다.
- Conflict matrix ownership: T15d owns 는 `src-tauri/src/pi/package_policy.rs`, `src-tauri/src/pi/package_installer.rs`, `src-tauri/tests/pi_package_*.rs`, `src/components/PiPackageManager.tsx` 로 한정한다. T13 policy/safety, T15c sidecar host IPC 는 read-only.
- Dependency/merge order: T15c + T13 완료 후 시작. T15e 는 T15d capability manifest 이후.
- Review gate warning: lead/orchestrator-owned `CodexAdversarialXHigh` 가 `Clean` 을 반환하고 `source_output_path` 가 persisted 되기 전에는 `pr_create`, `pr_merge`, `integrate_merge`, `main_apply` 진행 금지. worker 는 직접 peer review 를 실행하지 않는다.

## Paste-ready prompt

```text
[세션 부트]
- Prompt kind: Codex Desktop manual lead ticket session
- repo: D:\moa-desktop
- branch: codex/T15d-pi-package-trust-installer
- worktree required

[Goal]
Pi package install/update 를 MoA trust policy 로 통제한다.

[NEVER]
unpinned install/update, automatic `pi install`, automatic `pi update`, no-manifest activation 금지.

[Validation]
cargo test --manifest-path src-tauri\Cargo.toml pi_package
npm test -- --run PiPackageManager

[작업 완료 시]
trust schema, denied cases, UI confirm flow, audit fields 를 보고한다.
```

[작업 완료 시 — 무조건 이 순서로]
1. commit: `feat(T15d): Pi package trust & installer + policy schema + audit` (본문에 `Closes #40` 포함, push 금지)
2. **GitHub 카드 완료 처리 — 잊지 말고 무조건 실행**:
   ```
   node ~/.claude/scripts/gh-tickets.mjs complete D:\moa-desktop 40
   ```
   - 출력에 `COMPLETED=40` 또는 `ALREADY_CLOSED=40` 가 보여야 OK.
   - 실패 시 사용자 보고 + STOP.
3. 보고: trust schema, denied cases, UI confirm flow, audit fields, **GitHub 카드 close 결과 1줄**.
