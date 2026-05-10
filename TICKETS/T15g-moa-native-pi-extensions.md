# T15g — MoA Native Pi Extensions

GitHub: #43 (https://github.com/mizan0515/moa-desktop/issues/43)

## 새 Claude 창 만들기 가이드
T15d + T15e + T15f + T13 통과 후. worktree: T15g-native-ext.

---

````
[세션 부트]
- repo: D:\moa-desktop
- base branch: master (T15d + T15e + T15f + T13 머지 후)
- 권장 분기: feat/T15g-moa-native-pi-extensions
- 권위: PROJECT-RULES.md, AGENTS.md, DESIGN.md, TICKETS/T15d-pi-package-trust-installer.md, TICKETS/T15e-pi-extension-ui-bridge.md, TICKETS/T15f-pi-model-session-tree.md, TICKETS/T13-policy-lifecycle-epic.md
- 운영: MoA Flow C — § 2.6 템플릿 A

[의존성 self-check — claim 직후, first-pass 시작 전 무조건 실행]
master 에 선행 commit 4개 있는지 확인:
```
cd D:\moa-desktop && git log master --oneline -100 | rg -i "feat\(T15d\)|feat\(T15e\)|feat\(T15f\)|feat\(T13\)" | wc -l
```
- 결과 `4` 면 OK — 작업 진행
- 4 미만이면 **STOP — "선행 티켓이 master 에 미머지" 사용자 보고**.

[INDEPENDENT FIRST-PASS — read-only]
````

## Goal

MoA first-party Pi extensions 를 제공한다. 이 ticket 이후에만 Pi mutation owner 승격을 별도 opt-in setting 으로 검토한다.

## 의존성

- 선행: T15d package trust, T15e extension UI bridge, T15f session tree.
- 선행: T13 command guard/review gate.

## Success criteria

- [ ] `moa-tool-guard` extension: Pi tool calls before execution 을 MoA `WorkerCommandGuard`/capability policy 로 검사한다.
- [ ] `moa-review-gate` extension: Pi lane 결과가 mandatory `CodexAdversarialXHigh` gate 를 대체할 수 없음을 metadata 로 고정한다.
- [ ] `moa-ticket-context` extension: T10/T11/T12 ticket/lane context 를 read-only 로 주입한다.
- [ ] `moa-lane-telemetry` extension: token/tool/session events 를 MoA telemetry 로 publish 한다.
- [ ] first-party extension package 는 pinned local/bundled hash 와 source manifest 를 가진다.
- [ ] Pi mutation owner opt-in setting 은 off by default 이고, T4 worktree lock + T15d trust + T15e UI + review gate all-pass 일 때만 enabled 후보가 된다.

## Files owned

- `sidecars/moa-pi-host/extensions/moa-tool-guard/*`
- `sidecars/moa-pi-host/extensions/moa-review-gate/*`
- `sidecars/moa-pi-host/extensions/moa-ticket-context/*`
- `sidecars/moa-pi-host/extensions/moa-lane-telemetry/*`
- `src-tauri/tests/pi_native_extensions_*.rs`

## Read-only

- T13 safety/review APIs
- T15d/e/f APIs
- T4 lock/worktree APIs

## NEVER 영역

- first-party extension 이 command guard 를 우회하지 않는다.
- Pi review 를 mandatory Codex review gate 로 대체하지 않는다.
- mutation owner 승격을 default on 으로 하지 않는다.
- third-party package 를 first-party 로 가장하지 않는다.

## Validation cmd

```powershell
npm test --workspace sidecars/moa-pi-host
cargo test --manifest-path src-tauri\Cargo.toml pi_native_extensions
rg -n "moa-tool-guard|moa-review-gate|moa-ticket-context|moa-lane-telemetry|mutation owner|CodexAdversarialXHigh" sidecars src-tauri/src TICKETS
```

## Alternatives

1. No first-party extensions
   - Pros: less maintenance.
   - Cons: Pi integration remains generic and less safe.
2. First-party guard/review/context extensions (선택)
   - Pros: makes Pi extension power align with MoA policy.
   - Cons: extension API version drift to maintain.
3. Allow community extensions directly
   - Pros: fastest ecosystem adoption.
   - Cons: high bypass/supply-chain risk.

## Tests-first

Failing tests first: guard blocks peer command, review gate cannot be replaced, ticket context read-only, telemetry redaction, mutation owner setting remains off by default.

## Worker prompt 6 mandatory fields
1. Success criteria: `moa-tool-guard` extension (WorkerCommandGuard/capability 검사), `moa-review-gate` extension (CodexAdversarialXHigh 대체 불가 metadata 고정), `moa-ticket-context` extension (T10/T11/T12 context read-only 주입), `moa-lane-telemetry` extension (token/tool/session events publish), first-party pinned local/bundled hash + source manifest, Pi mutation owner opt-in off default + 전제조건 gate 를 구현한다.
2. NEVER 영역: first-party extension 이 command guard 우회, Pi review 가 mandatory Codex review gate 대체, mutation owner 승격 default on, third-party 를 first-party 로 가장, worker 직접 peer 호출.
3. Validation cmd:
   ```powershell
   npm test --workspace sidecars/moa-pi-host
   cargo test --manifest-path src-tauri\Cargo.toml pi_native_extensions
   rg -n "moa-tool-guard|moa-review-gate|moa-ticket-context|moa-lane-telemetry|mutation owner|CodexAdversarialXHigh" sidecars src-tauri/src TICKETS
   ```
4. Files + lines: `TICKETS/T15g-moa-native-pi-extensions.md` 의 Success criteria/NEVER, `TICKETS/T15d-pi-package-trust-installer.md` 의 trust policy, `TICKETS/T15e-pi-extension-ui-bridge.md` 의 renderer registry, `TICKETS/T13-policy-lifecycle-epic.md` 의 WorkerCommandGuard/ReviewVerdict.
5. Alternatives 2개 + pros/cons + 선택 근거: first-party extension 없이 generic(유지보수 적지만 Pi integration 이 generic/unsafe) vs first-party guard/review/context extensions(Pi 를 MoA policy 와 정렬, extension API version drift 관리 필요). 선택은 first-party extensions. community extensions 직접 허용(ecosystem 빠르지만 bypass/supply-chain 위험) 기각.
6. Tests-first: guard blocks peer command, review gate cannot be replaced, ticket context read-only, telemetry redaction, mutation owner off default 를 먼저 실패시키고 구현한다.

## Worker prompt cross-contract fields
- GitHub/project handling: GitHub #43 / Project `MoA Desktop` card status 를 claim/complete 단계에서 갱신한다.
- Conflict matrix ownership: T15g owns 는 `sidecars/moa-pi-host/extensions/moa-tool-guard/*`, `sidecars/moa-pi-host/extensions/moa-review-gate/*`, `sidecars/moa-pi-host/extensions/moa-ticket-context/*`, `sidecars/moa-pi-host/extensions/moa-lane-telemetry/*`, `src-tauri/tests/pi_native_extensions_*.rs` 로 한정한다. T13 safety/review, T15d/e/f, T4 lock/worktree 는 read-only.
- Dependency/merge order: T15d + T15e + T15f + T13 완료 후 시작. T16 은 T15g 이후.
- Review gate warning: lead/orchestrator-owned `CodexAdversarialXHigh` 가 `Clean` 을 반환하고 `source_output_path` 가 persisted 되기 전에는 `pr_create`, `pr_merge`, `integrate_merge`, `main_apply` 진행 금지. worker 는 직접 peer review 를 실행하지 않는다.

## Paste-ready prompt

````text
[세션 부트]
- Prompt kind: Codex Desktop manual lead ticket session
- repo: D:\moa-desktop
- branch: codex/T15g-moa-native-pi-extensions
- worktree required

[Goal]
MoA first-party Pi extensions 를 구현해 Pi runtime 을 MoA policy 와 연결한다.

[NEVER]
guard bypass, Codex gate replacement, default mutation owner, third-party as first-party 금지.

[Validation]
npm test --workspace sidecars/moa-pi-host
cargo test --manifest-path src-tauri\Cargo.toml pi_native_extensions

[작업 완료 시]
extension list, safety tests, mutation-owner opt-in prerequisites 를 보고한다.
````

[작업 완료 시 — 무조건 이 순서로]
1. commit: `feat(T15g): MoA native Pi extensions + guard/review-gate/context/telemetry` (본문에 `Closes #43` 포함, push 금지)
2. **GitHub 카드 완료 처리 — 잊지 말고 무조건 실행**:
   ```
   node ~/.claude/scripts/gh-tickets.mjs complete D:\moa-desktop 43
   ```
   - 출력에 `COMPLETED=43` 또는 `ALREADY_CLOSED=43` 가 보여야 OK.
   - 실패 시 사용자 보고 + STOP.
3. 보고: extension list, safety tests, mutation-owner opt-in prerequisites, **GitHub 카드 close 결과 1줄**.
