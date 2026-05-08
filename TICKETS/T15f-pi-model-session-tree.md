# T15f — Pi Model Switch & Session Tree

GitHub: #42 (https://github.com/mizan0515/moa-desktop/issues/42)

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

## Paste-ready prompt

```text
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
```
