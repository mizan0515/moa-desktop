# T26 — Fugu API advisory-only 통합 검토

GitHub: #57 (https://github.com/mizan0515/moa-desktop/issues/57)

## Origin
RL Conductor 논문 (arXiv:2512.04388, ICLR 2026) 적용 가능성 리서치에서 도출.
Sakana AI의 Fugu (RL Conductor 상용화) 서비스.
MoA 흐름 D 종합 + Codex adversarial review (2026-05-10) 판정: **PREMATURE**.

## 새 Claude 창 만들기 가이드
Fugu GA + pricing 공개 후 재평가. Pi Runtime (T15) 안정화 이후.

---

```
[세션 부트]
- repo: D:\moa-desktop
- base branch: master
- 권장 분기: feat/T26-fugu-advisory
- 권위: DESIGN.md (## Safety invariants), src-tauri/src/safety/, src-tauri/src/orchestrator/
- 운영: MoA Flow C — § 2.6 템플릿 A

[의존성 self-check]
- T15 (Pi Runtime): 3rd worker 통합 패턴 확인
- Fugu API: GA 여부 + pricing + API 문서 확인 (현재 beta-only, pricing UNVERIFIED)

[INDEPENDENT FIRST-PASS — read-only]
```

## Goal
Sakana AI Fugu를 advisory-only 모드로 통합 검토.
Fugu가 제안하는 워크플로우를 orchestrator의 "참고 의견"으로만 활용하고, 실제 워커 할당/실행 결정은 moa-desktop의 기존 flow classifier가 유지.

핵심 제약: Fugu의 hidden internal topology가 moa-desktop의 safety gate (WorkerCommandGuard, output scanner)와 충돌 — Fugu가 내부적으로 재귀 호출이나 예상 외 워커 조합을 지시할 경우 safety invariant 위반 가능.

## Advisory-only 모드 정의
1. Fugu API에 task description 전송
2. Fugu가 추천 워크플로우 반환 (워커 할당, 서브태스크 분해, 접근 파일)
3. moa-desktop은 이를 **참고 정보로만** 표시 (UI에 "Fugu suggestion" 패널)
4. 실제 실행은 기존 flow classifier + orchestrator가 결정
5. Fugu 추천과 실제 실행의 차이를 로깅하여 Fugu 정확도 평가

## Success criteria
- [ ] Fugu API GA + pricing 확인 (현재 UNVERIFIED → EVIDENCED)
- [ ] Fugu API 호출 adapter 구현 (advisory-only, 실행 권한 없음)
- [ ] Fugu 추천 결과를 UI에 표시하는 패널
- [ ] Fugu 추천 vs 실제 실행 차이 로깅
- [ ] safety invariant 검증: Fugu 추천이 WorkerCommandGuard 위반하는 조합을 제안해도 실행 차단됨을 확인
- [ ] Fugu API 장애/지연 시 graceful degradation (Fugu 없이 기존 동작 유지)
- [ ] 비용 분석: Fugu API 호출 비용 vs 워크플로우 개선 효과

## Dependencies
- T15 (Pi Runtime — 3rd worker 통합 패턴 참조)
- Fugu GA + pricing 공개 (외부 의존)
- v1.0 이후 (기본 파이프라인 안정화 선행)

## Files owned (예상)
- `src-tauri/src/adapters/fugu.rs` (신규)
- `src/components/FuguAdvisoryPanel.tsx` (신규)

## NEVER
- Fugu 추천을 자동 실행하지 않음 — advisory-only
- `src-tauri/src/safety/` — guard/scanner 변경 금지
- `src-tauri/src/orchestrator/flow.rs` — Fugu가 flow 분류를 override하지 않음

## Risks
1. **hidden topology**: Fugu 내부에서 어떤 모델 조합/순서로 워크플로우를 생성하는지 불투명 → safety 검증 불가. **대응**: advisory-only로 실행 차단, 추천 내용만 로깅
2. **pricing**: beta 무료 → GA 시 과금 가능. moa-desktop은 로컬 앱이므로 사용자에게 API 비용 투명하게 공개 필요. **대응**: pricing 공개 후 비용 대비 효과 분석
3. **API 안정성**: beta/early GA 서비스의 API 변경 빈도. **대응**: adapter 패턴으로 격리, versioned client
4. **latency**: Fugu API 호출이 파이프라인에 지연 추가. **대응**: advisory는 비동기 — 메인 파이프라인 블로킹 없이 병렬 호출

## 재평가 시점
- Fugu GA 발표 + pricing 공개 시
- Pi Runtime (T15c) 안정화 이후
- 현재 판정: PREMATURE — 외부 서비스 의존도 + pricing 불확실성

## Estimated effort
Medium — API adapter + UI 패널 + 로깅 + safety 검증. 단, Fugu API 문서 공개 전까지 설계만 가능.
