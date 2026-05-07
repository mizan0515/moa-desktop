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
- `src/components/PiPackageTrustPanel.tsx`
- `src/lib/piPackagePolicy.ts`

## NEVER 영역

- floating latest/semver range package activation
- auto `pi install` / `pi update`
- user confirm 없는 project-local package activation
- capability manifest 없는 package enable
- mandatory review gate relaxation

## Worker prompt 6 mandatory fields

1. Success criteria: pinned source schema, sha256, capability manifest, preview/confirm, audit, autoUpdate=false.
2. NEVER 영역: floating versions, auto install/update, unconfirmed project-local activation, manifest-less enable, review gate relaxation.
3. Validation cmd:
   ```powershell
   cargo test --manifest-path src-tauri\Cargo.toml pi_package
   npm test -- --run PiPackageTrustPanel
   ```
4. Files + lines: this ticket Success criteria, `PROJECT-RULES.md` Pi HarnessRuntime invariant, T13 PolicyPack docs.
5. Alternatives 2개 + pros/cons + 선택 근거: block all third-party packages(very safe but disables Pi value) vs pinned capability-gated install(usable with audit). 선택은 pinned capability-gated install.
6. Tests-first: semver range denial, missing sha denial, project-local auto-install denial, capability escalation confirm tests 를 먼저 실패시킨다.
