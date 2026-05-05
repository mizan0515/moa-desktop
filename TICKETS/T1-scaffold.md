# T1 — Tauri v2 scaffold + workbench static UI

## 새 Claude 창 만들기 가이드
1. T0 통과 후 (RESULTS.md PASS) 새 창 열기
2. 첫 입력으로 아래 프롬프트 통째 붙여넣기

---

```
[세션 부트]
- repo: D:\moa-desktop
- base branch: master
- 권장 분기: feat/T1-scaffold
- 권위: DESIGN.md, PLAN.md (§ F2 argv array, § F6 multi-instance), spikes/RESULTS.md, ~/.claude/CODEX-MCP.md, ~/.claude/KARPATHY.md
- 운영: MoA Flow C — § 2.6 템플릿 A 사용

[INDEPENDENT FIRST-PASS — read-only, mutation 금지]

## Goal
Tauri v2 + React + TypeScript scaffold + workbench 정적 UI. 다른 ticket 들이 충돌 없이 들어올 수 있도록 모듈 폴더 미리 생성.

## Success criteria
- [x] `npm install && npm run tauri dev` 실행 시 데스크탑 앱 창 띄워짐 (Windows)
- [x] 첫 화면 = workbench (랜딩 페이지 X). **상단 프로젝트 탭 바** (Codex/Claude Desktop 패턴, 처음에는 1 탭만 활성, "+" 버튼으로 새 프로젝트 탭 추가 stub OK), 왼쪽 세션 list, 가운데 작업 입력 + 진행 상태, 오른쪽 결과 영역, 아래 로그 영역 (모두 더미 데이터 OK)
- [x] 좁은 폭(800px) 에서도 layout 깨지지 않음
- [x] settings 화면 진입 가능 (빈 form 도 OK)
- [x] Tauri single-instance plugin 적용 — 두 번째 instance 실행 시 첫 instance 만 활성화
- [x] **Win11 24H2 fast double-launch stress test**: 50 회 30ms 간격으로 `Start-Process moa-desktop.exe` 후 `Get-Process moa-desktop` count = 1 검증. plugin 의 known issue (Win11 24H2 실패, fast-startup mutex+window race, callback main-thread hang) 대응 — callback 은 main event loop 로 enqueue 만, window/focus/state mutation 은 main thread handler 에서 처리
- [x] **Single-instance plugin 실패 fallback hook**: plugin 이 첫 등록 후 동작 안 하는 환경 감지 시 T4 의 OS-level named mutex 로 graceful fallback. `--user-data-dir <path>` CLI flag 인식 (safe-mode escape hatch — 명시적 N 인스턴스 허용)
- [x] `src-tauri/src/` 아래 다음 빈 module folder + mod.rs 미리 생성: process, adapters, safety, git, lock, journal, telemetry, cancel, mock, orchestrator, synthesis, settings, **decomposer, parallel, integrator** (Phase 6 v1.5 owns) — 각 mod.rs 는 `// owned by T<n>` 주석만
- [x] `Cargo.toml` 에 공통 deps 미리 등록: tokio (full), serde, serde_json, anyhow, thiserror, tauri-plugin-single-instance
- [x] SynthesisView.tsx, ClaimLedger.tsx, CostMeter.tsx, ErrorBanner.tsx, **TicketBoard.tsx, ParallelLanes.tsx, IntegratePanel.tsx** (v1.5 stub) 는 stub (placeholder div) 로 placeholder
- [x] **`tabRegistry` 패턴**: `src/lib/tabRegistry.ts` 에 `{ id, title, component, order }` 배열 + register/unregister API. App.tsx 가 본 registry 를 읽어 탭 렌더. Phase 6 의 T10/T11/T12 가 App.tsx 수정 없이 탭 등록 가능해야 함
- [x] **`projectId` context**: `src/lib/projectContext.tsx` 에 React context 제공 — 현재 활성 탭 projectId 를 하위 컴포넌트에 전달. v1 에서는 default project 1 개만 active 지만 인터페이스 미리

## 완료 (2026-05-06)
- 초기 구현: `cc8abaf feat(T1): tauri v2 scaffold + workbench static UI + module stubs`
- 후속 fix: `tabRegistry.getSnapshot` unstable reference 로 인한 빈 화면 — `useSyncExternalStore` 가 매 호출 새 배열을 받으면 무한 re-render 로 판단해 mount 실패. cached snapshot 으로 수정.
- 관리자 검증 통과: 앱 창 / project 탭 + "+" / 4구역 layout / settings 진입 / 800px narrow layout / single-instance.
- 다음 ticket 진입점: T2 (`src-tauri/src/process/`), T8 (`src-tauri/src/mock/`) — 폴더 칸막이 준비됨.

## Files owned (수정 허용)
- `package.json`, `package-lock.json`, `tsconfig.json`, `vite.config.ts`, `index.html`
- `src/main.tsx`, `src/App.tsx`, `src/styles/*`, `src/components/Workbench/*`
- `src/components/{SynthesisView,ClaimLedger,CostMeter,ErrorBanner,TicketBoard,ParallelLanes,IntegratePanel}.tsx` (stub only)
- `src/lib/tabRegistry.ts`, `src/lib/projectContext.tsx`
- `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/build.rs`
- `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`
- `src-tauri/src/{process,adapters,safety,git,lock,journal,telemetry,cancel,mock,orchestrator,synthesis,settings,decomposer,parallel,integrator}/mod.rs` (빈 stub)
- `README.md` (한 paragraph)
- `.gitignore`

## Read-only (참조만)
- DESIGN.md, PLAN.md, synthesis.md, spikes/RESULTS.md

## NEVER 영역
- spikes/* (T0 영역)
- mockResponses/, src-tauri/src/mock/*.rs body (T8)
- src/components/{SynthesisView,ClaimLedger}.tsx **body** (stub 만 OK, 실제 logic 은 T6)
- src-tauri/src/process/*.rs body (T2)
- src-tauri/src/adapters/*.rs body (T5a/T5b)
- src-tauri/src/{safety,git,lock,journal}/*.rs body (T4)
- src-tauri/src/synthesis/*.rs (T3)
- src-tauri/src/orchestrator/*.rs body (T7)
- src-tauri/src/{telemetry,cancel}/*.rs body (T9)
- src-tauri/src/{decomposer,parallel,integrator}/*.rs body (T10/T11/T12, v1.5)
- src/components/{TicketBoard,ParallelLanes,IntegratePanel}.tsx body (T10/T11/T12, v1.5)
- secret/.env 파일

## Stop conditions
- Tauri v2 plugin (single-instance) 가 Windows 에서 안 됨 → 사용자 보고
- T0 RESULTS 가 NO-GO 인데 시작했음 → 즉시 중단

## Deliverable (first-pass)
1. Diagnosis: 현재 빈 repo 상태, Tauri v2 scaffold 명령 (`npm create tauri-app@latest` 인지 별도 절차인지)
2. Approach: scaffold tool 사용 vs from-scratch (대안 2개)
3. Risks + validation
4. Module 미리 생성 전략 (mod.rs 빈 stub + lib.rs 에서 `pub mod x` 선언 vs 동적 등록)
5. Open questions

## Constraints
- argv array 빌드 의무 (PowerShell quoting 회피)
- 6 항목 의무
- 무관 dirty 파일 (DESIGN.md 등) 건드리지 X
- 비밀 파일 commit X

[작업 완료 시]
- commit: `feat(T1): tauri v2 scaffold + workbench static UI + module stubs`
- push 금지
- 보고: 변경 파일 목록, `npm run tauri dev` 동작 스크린샷 또는 글 묘사, 좁은 폭 검증 결과, 다음 ticket (T8/T2) worker 가 알아야 할 module 진입점
```
