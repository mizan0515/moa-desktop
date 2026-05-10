# T24 — T10 DAG metadata에 Conductor 3-list 관점 반영

GitHub: #55 (https://github.com/mizan0515/moa-desktop/issues/55)

## Origin
RL Conductor 논문 (arXiv:2512.04388, ICLR 2026) 적용 가능성 리서치에서 도출.
MoA 흐름 D 종합 + Codex adversarial review (2026-05-10) 판정: **기존 T10 포함** (T10 스키마가 이미 superset).

## 새 Claude 창 만들기 가이드
T10 착수 시 본 티켓을 참조 자료로 사용. 단독 착수 불필요.

---

````
[세션 부트]
- repo: D:\moa-desktop
- base branch: master
- 권장 분기: T10 분기 내에서 처리
- 권위: TICKETS/T10-ticket-decomposer.md, DESIGN.md
- 운영: T10 작업의 일부로 처리

[의존성 self-check]
- T10 착수 가능 상태 확인

[INDEPENDENT FIRST-PASS — read-only]
````

## Goal
T10 (ticket decomposer) 구현 시 RL Conductor의 3-list 워크플로우 표현 (model_id, subtasks, access_list)을 T10 DAG 스키마 설계에 참고.

현재 T10 스키마가 이미 `runtimeKind`, `deps`, `conflictMatrix`, `mergeOrder`, `reviewGate`, `sixMandatoryFields`를 포함 — Conductor 3-list는 이의 subset.

## 구체적 참조 사항
Conductor 3-list와 T10 스키마 대응:

| Conductor | T10 기존 스키마 | 비고 |
|---|---|---|
| `model_id` (워커 할당) | `runtimeKind` | T10이 더 넓음 (claude/codex/pi) |
| `subtasks` (서브태스크 분해) | `deps` + `mergeOrder` | T10이 DAG + 순서까지 포함 |
| `access_list` (파일 접근) | `filesOwned` + `conflictMatrix` | T10이 충돌 감지까지 포함 |

## T10 착수 시 확인 사항
- [ ] `runtimeKind` 필드가 워커별 강점 기반 할당을 지원하는지 (T21 역할 분화와 연계)
- [ ] subtask 간 dependency가 DAG로 표현되어 병렬 실행 가능 경로 자동 도출되는지
- [ ] access_list에 해당하는 `filesOwned`가 워커 간 파일 충돌 감지에 활용되는지

## Dependencies
- T10 (본 티켓은 T10의 참조 자료)

## Files owned
없음 — T10 작업 내에서 처리.

## NEVER
- T10 스키마를 Conductor 3-list로 축소하거나 대체하지 않음 (T10이 superset)

## Risks
1. **과잉 적용**: Conductor의 3-list가 T10보다 단순함을 인식하지 못하고 별도 구현 시도 → 중복. **대응**: 본 티켓은 "참조" 역할만, T10 스키마 변경은 T10 내에서 판단
2. **runtimeKind 할당 자동화 유혹**: Conductor처럼 RL로 워커 할당 최적화 시도 → 현재 2-worker에서는 과잉. **대응**: rule-based 할당 유지, 3+ worker 시 재검토

## 재평가 시점
T10 착수 시 자동 참조.

## Estimated effort
Minimal — T10 착수 시 본 문서 참조만. 별도 구현 없음.
