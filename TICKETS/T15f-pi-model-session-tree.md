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
- [ ] `fork`, `clone`, `compact`, `get_state` 가 UI action 과 event 로 노출된다.
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

## NEVER 영역

- Pi session tree 를 MoA journal/ResumePacket source of truth 로 승격
- active mutation turn 중 model switch
- compact 가 audit/journal 없이 context 를 폐기
- package install/update side effect

## Worker prompt 6 mandatory fields

1. Success criteria: model controls, turn-boundary rule, fork/clone/compact/get_state, ResumePacket mirror, journal entries, T14 consumption.
2. NEVER 영역: Pi tree source-of-truth 승격, active mutation switch, unaudited compact, package side effects.
3. Validation cmd:
   ```powershell
   cargo test --manifest-path src-tauri\Cargo.toml pi_session_tree
   npm test -- --run PiSessionTree
   ```
4. Files + lines: this ticket Success criteria, T13 ResumePacket schema, T14 Pi lane amend.
5. Alternatives 2개 + pros/cons + 선택 근거: Pi tree as source of truth(simple integration but unsafe resume semantics) vs MoA source with Pi mirror(more mapping but preserves MoA recovery). 선택은 MoA source with Pi mirror.
6. Tests-first: turn-boundary model switch denial, compact journal, import/export, T14 handoff tests 를 먼저 실패시킨다.
