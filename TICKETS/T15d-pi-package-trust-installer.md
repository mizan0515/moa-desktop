# T15d — Pi Package Trust & Installer

GitHub: #40 (https://github.com/mizan0515/moa-desktop/issues/40)

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
