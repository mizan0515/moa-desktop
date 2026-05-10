# T15f — Pi Model Switch & Session Tree

GitHub: #42 (https://github.com/mizan0515/moa-desktop/issues/42)

## 새 Claude 창 만들기 가이드
T15c + T13 ResumePacket 통과 후. worktree: T15f-session-tree.

---

````
[세션 부트]
- repo: D:\moa-desktop
- base branch: master (T15c + T13 머지 후)
- 권장 분기: feat/T15f-pi-model-session-tree
- 권위: PROJECT-RULES.md, AGENTS.md, DESIGN.md, TICKETS/T15c-pi-sdk-sidecar-host.md, TICKETS/T13-policy-lifecycle-epic.md (L5 ResumePacket)
- 운영: MoA Flow C — § 2.6 템플릿 A

[의존성 self-check — claim 직후, first-pass 시작 전 무조건 실행]
master 에 선행 commit 2개 있는지 확인:
```
cd D:\moa-desktop && git log master --oneline -100 | rg -i "feat\(T15c\)|feat\(T13\)" | wc -l
```
- 결과 `2` 면 OK — 작업 진행
- 2 미만이면 **STOP — "선행 티켓이 master 에 미머지" 사용자 보고**.

[INDEPENDENT FIRST-PASS — read-only]
````

## Goal

Pi model switching, thinking level, session tree/fork/clone/compact 를 MoA UI 에 노출한다. MoA journal/ResumePacket 이 source of truth 이고 Pi session tree 는 lane-local mirror 이다.

## 의존성

- 선행: T15c SDK sidecar host.
- 선행: T13 ResumePacket.

## Success criteria

- [ ] `runtimeKind="pi"` lane 에 model/provider/thinking level controls 가 있다.
- [ ] model switch 는 read-only/research/conversation lane 에서는 즉시 허용, mutation lane 에서는 turn boundary only 로 제한한다.
- [ ] `fork`, `clone`, `compact`, `get_state` 가 UI action 과 event로 노출된다.
- [ ] Pi session tree 는 MoA `ResumePacket` 에 mirror/import/export 되지만 source of truth 는 아니다.
- [ ] compaction event 는 MoA journal entry 를 남기고 rollback/import 가능성을 보존한다.
- [ ] session tree node 는 lane id, runtime session id, parent node, created_at, model ref, compacted flag 를 가진다.
- [ ] T14 conversational mode 가 Pi interactive lane 후보로 소비할 수 있다.

## Files owned

- `src-tauri/src/pi/session_tree.rs`
- `src-tauri/tests/pi_session_tree_*.rs`
- `src/components/PiSessionTree.tsx`
- `src/components/PiModelSwitcher.tsx`
- `src/lib/piSessionTree.ts`

## Read-only

- T13 lifecycle/ResumePacket APIs
- T15c sidecar host IPC
- T14 conversation contracts

## NEVER 영역

- Pi session tree 가 MoA journal/ResumePacket 을 대체하지 않는다.
- review gate profile model 은 `CodexAdversarialXHigh` fixed profile 에서 Pi model switch 로 바꾸지 않는다.
- mutation lock 보유 중 hot reload/model switch/compact 를 무제한 허용하지 않는다.

## Validation cmd

```powershell
cargo test --manifest-path src-tauri\Cargo.toml pi_session_tree
npm test -- --run PiSession
rg -n "PiSessionTree|PiModelSwitcher|fork|compact|ResumePacket|source of truth|runtimeKind.*pi" src-tauri/src src TICKETS
```

## Alternatives

1. Hide Pi session tree
   - Pros: simpler.
   - Cons: loses core Pi UX.
2. Mirror Pi tree into MoA journal (선택)
   - Pros: preserves MoA lifecycle while exposing Pi features.
   - Cons: needs mapping/migration tests.
3. Make Pi tree source of truth
   - Pros: less duplication.
   - Cons: violates MoA orchestration/lifecycle boundary.

## Tests-first

Failing tests first: fork/clone lineage, compaction persistence, ResumePacket restore, mutation-boundary model switch reject, T14 consumption fixture.

## Worker prompt 6 mandatory fields
1. Success criteria: `runtimeKind="pi"` lane 에 model/provider/thinking level controls, read-only/research lane 즉시 model switch + mutation lane turn boundary 제한, fork/clone/compact/get_state UI action+event 노출, Pi session tree → MoA ResumePacket mirror (source of truth 아님), compaction journal entry + rollback 보존, session tree node schema (lane id, runtime session id, parent, created_at, model ref, compacted flag), T14 conversational mode consumption 을 구현한다.
2. NEVER 영역: Pi session tree 가 MoA journal/ResumePacket 대체, review gate profile model 을 Pi model switch 로 교체, mutation lock 보유 중 무제한 hot reload/model switch/compact 허용, worker 직접 peer 호출.
3. Validation cmd:
   ```powershell
   cargo test --manifest-path src-tauri\Cargo.toml pi_session_tree
   npm test -- --run PiSession
   rg -n "PiSessionTree|PiModelSwitcher|fork|compact|ResumePacket|source of truth|runtimeKind.*pi" src-tauri/src src TICKETS
   ```
4. Files + lines: `TICKETS/T15f-pi-model-session-tree.md` 의 Success criteria/NEVER, `TICKETS/T15c-pi-sdk-sidecar-host.md` 의 IPC event protocol (set_model/compact/fork), `TICKETS/T13-policy-lifecycle-epic.md` 의 ResumePacket, `DESIGN.md` 의 Pi runtime section.
5. Alternatives 2개 + pros/cons + 선택 근거: Pi session tree 숨기기(단순하지만 core Pi UX 손실) vs Pi tree 를 MoA journal 에 mirror(MoA lifecycle 보존 + Pi features 노출, mapping/migration 테스트 필요). 선택은 mirror. Make Pi tree source of truth(중복 줄지만 MoA orchestration/lifecycle boundary 위반) 은 기각.
6. Tests-first: fork/clone lineage, compaction persistence, ResumePacket restore, mutation-boundary model switch reject, T14 consumption fixture 를 먼저 실패시키고 구현한다.

## Worker prompt cross-contract fields
- GitHub/project handling: GitHub #42 / Project `MoA Desktop` card status 를 claim/complete 단계에서 갱신한다.
- Conflict matrix ownership: T15f owns 는 `src-tauri/src/pi/session_tree.rs`, `src-tauri/tests/pi_session_tree_*.rs`, `src/components/PiSessionTree.tsx`, `src/components/PiModelSwitcher.tsx`, `src/lib/piSessionTree.ts` 로 한정한다. T13 lifecycle/ResumePacket, T15c sidecar host IPC, T14 conversation contracts 는 read-only.
- Dependency/merge order: T15c + T13 완료 후 시작. T14 conversational mode 와는 병행 가능 (read-only 참조).
- Review gate warning: lead/orchestrator-owned `CodexAdversarialXHigh` 가 `Clean` 을 반환하고 `source_output_path` 가 persisted 되기 전에는 `pr_create`, `pr_merge`, `integrate_merge`, `main_apply` 진행 금지. worker 는 직접 peer review 를 실행하지 않는다.

## Paste-ready prompt

````text
[세션 부트]
- Prompt kind: Codex Desktop manual lead ticket session
- repo: D:\moa-desktop
- branch: codex/T15f-pi-model-session-tree
- worktree required

[Goal]
Pi model switching and session tree 를 MoA lifecycle 아래에 노출한다.

[NEVER]
Pi tree replacing MoA journal, review profile replacement, mutation-lock unsafe switch 금지.

[Validation]
cargo test --manifest-path src-tauri\Cargo.toml pi_session_tree
npm test -- --run PiSession

[작업 완료 시]
session mapping, compaction behavior, T14 handoff 를 보고한다.
````

[작업 완료 시 — 무조건 이 순서로]
1. commit: `feat(T15f): Pi model switch & session tree + ResumePacket mirror + compaction` (본문에 `Closes #42` 포함, push 금지)
2. **GitHub 카드 완료 처리 — 잊지 말고 무조건 실행**:
   ```
   node ~/.claude/scripts/gh-tickets.mjs complete D:\moa-desktop 42
   ```
   - 출력에 `COMPLETED=42` 또는 `ALREADY_CLOSED=42` 가 보여야 OK.
   - 실패 시 사용자 보고 + STOP.
3. 보고: session mapping, compaction behavior, T14 handoff, **GitHub 카드 close 결과 1줄**.
