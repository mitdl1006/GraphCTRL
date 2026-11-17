# GraphCTRL M1 계획 — Inventor식 로봇 셀 프로그래밍 워크벤치 (100% Rust, egui + wgpu)

## 0. 현재 상태와 진행 방식 (2026-11-17 갱신, 리드가 관리)

**완료 기준 시나리오(3.1) 1–8은 화면에서 모두 동작한다**: 셸·내비게이션, 2D/3D 스케치와 치수·구속, 메시·기본 도형, 로봇
(UR3e/5e/10e/16e, 샘플 6R, URDF 불러오기), 웨이포인트(작업 좌표계·월드·베이스 기준)·경로·플래너 5종·비교·재생, 저장/열기·자동
저장 복구·예제. 그 위에 선택·기즈모·고스트·툴·TCP 속도 상한·포즈 계산기·UR 해석적 IK가 있다. 상세는 `STATUS.md`.

**진행 방식**: 역할 세션이 각자 브랜치에서 일하고 리드가 검사 후 병합한다(`STATUS.md` "팀 구성"). 범위 밖 변경은
`docs/handoff/<역할>.md`로 리드에게 요청한다.

**M2(9절)가 다음 마일스톤이다.** 아래 표의 M1 잔여 항목은 M2-1과 병행한다.

**남은 P1(3.2) — 담당**
| 항목 | 담당 | 비고 |
|---|---|---|
| 계획 품질 측정 `bench-quality`(부록 E Q1·Q3·Q5·Q9) | 백엔드 | 진행 중 |
| 재계획 사다리(기존 경로 재검증 우선, 5.8) | 백엔드 | 진행 중 |
| 내장 플래너를 `gc_plugin_planners`로 API 등록, `gc_motion`은 레지스트리 조회(4.3) | 백엔드 + 리드(API) | 대기 |
| TCP 구 모드(로봇 없이 구로 계획, 5.8) | 백엔드 → 프론트엔드 | 대기 |
| 보증 문구 `Guarantee`(5.7, Q2)를 결과에 기록·표시 | 백엔드 → 프론트엔드 | 대기 |
| 접촉 허용 `ContactAllowance`, ACM 목록 켜기/끄기(5.7) | 백엔드 → 프론트엔드 | 대기 |
| 도구 > 성능 오버레이(프레임 시간·드로 호출, 5.2) | 프론트엔드 | 대기 |
| 브라우저 아이콘 행, 콤보 라벨 정렬, 3D 스케치 점 드래그·글리프 | 프론트엔드 | 진행 중 |
| 시나리오 S1–S9·5.11 체크리스트 대조, 경계 사례, Windows CI | QA·빌드 | 진행 중 |
| 사용자 가이드·README·UI 문구 검토 | 문서·UX | 진행 중 |
| UR3e/10e/16e 시각 메시, 로봇을 실행 파일 밖 `robots/` 라이브러리로 | 리드 | 완료 |

## 1. Context — 왜, 무엇을 만드는가

- 저장소 `mitdl1006/GraphCTRL`은 README / LICENSE(MIT) / .gitignore만 있는 빈 상태다. 재사용할 기존 코드는 없다.
- 목표 제품: 설비·라인에 장착되는 로봇(첫 타깃 UR 시리즈)의 제어를 **시뮬레이션·프로그래밍하는 OS 네이티브 데스크톱 앱**. 장기적으로 PDDL/FSM/HFSM/BT를 무한 캔버스 노드 그래프로 편집하고, Inventor/SolidWorks식 3D 작업공간에서 설비 모델·로봇·경로·웨이포인트를 파라메트릭하게 다룬다.
- **M1 범위(사용자 확정)**: ① Inventor식 UI 셸과 3D 뷰 조작 ② 2D + 3D 스케치와 치수·구속조건(솔버 포함) ③ **URDF 같은 로봇 기술 파일 기반**의 **고품질 매니퓰레이터 경로 자동생성**(여러 알고리즘 중 선택 → 생성 경로 표시), UR은 내장 플러그인 ④ 이후 기능을 받을 플러그인 구조.
- 제어 방법(PDDL/FSM/HFSM/BT 편집기)은 M1에서 제외하고 확장 지점만 마련한다.
- 이 계획은 리서치 9건과 독립 리뷰 4건(아키텍처·스케치/솔버·로보틱스·실행/검증)을 반영했다. 세부 사양은 Stage 0에서 저장소 `docs/`로 옮기는 원문(부록 F)이 **규범 문서**이며, 충돌 시 이 계획서가 우선한다.

## 2. 확정된 결정

| 항목 | 결정 | 근거 요약 |
|---|---|---|
| 선택 기준(사용자 지정 순서) | **① 큰 장면 3D 성능**(여러 3D 파일을 올려도 렉 없이) → **② 유지보수 지속성** → ③ 주류이면서 프로젝트에 맞는 크레이트 | 프론트엔드와 이후 의존성 선택에 같은 순서를 적용 |
| 프론트엔드 | **egui/eframe 0.37 + wgpu 30 네이티브** (웹 기반 대안 재검토 후 사용자 재확정) | ① 네이티브 DX12/Vulkan 직접 렌더링, 큰 3D 장면에서 제품(Rerun)으로 검증된 유일한 후보. Linux 웹뷰(WebKitGTK)는 WebGPU 근거 없음·WebGL 불안정, wasm32 4 GB·단일 스레드 한계 ② 24개월 안정 릴리스 19회, 의존 크레이트 1,424개(Leptos는 2026-05 "가볍게만 유지보수" 공지, Yew는 800일 공백, Dioxus 네이티브 렌더러는 베타) ③ Rust GUI 중 다운로드 1위(90일 10.0M) |
| 프론트엔드 대비책 | 3D 렌더러(`gc_render`)는 egui에 의존하지 않고 `Device`/`Queue`/대상 `TextureView`만 받는다 → UI 계층만 교체 가능. 한글 입력 시험이 실패하면 Slint·Dioxus 네이티브를 재평가 | 교체 비용을 화면 계층으로 한정 |
| 3D 뷰포트 | 자체 **오프스크린 타깃**(MSAA 4x + Depth32F reverse-Z → `Rgba8Unorm` resolve)을 `register_native_texture`로 표시. **GPU는 선택 사항**: `wgpu_render_state`가 없어도 앱·도구·피킹·패널이 동작(뷰포트만 자리표시) | eframe·kittest·헤드리스에서 동일 동작, GPU 없는 환경에서도 시나리오 테스트 가능 |
| 뷰 조작 기본값 | **Inventor 프리셋** (SolidWorks 프리셋은 데이터로 제공) | 사용자 선택 |
| 스케치 | **2D 평면 + 3D 스케치** | 사용자 선택 |
| 경로 계획 | **URDF 기반 매니퓰레이터 계획**(UR 내장 플러그인, 비-UR 로봇도 같은 경로) + 로봇 없는 **TCP 구(球) 모드** | 사용자 선택 + 추가 요구 |
| UI 폰트 | **Jetendard**(kuskhan/jetendard v0.1.0, OFL 1.1) Regular·Bold를 무수정 번들, UI 전체 기본 폰트 | 사용자 지정. 고정폭(한글 = 라틴 2칸) |
| 대상 OS | **Windows, Linux** 우선 | 사용자 선택 |
| 구속 솔버 | **자체 구현** `gc_solver`(2D+3D) | ezpz는 2D 전용·DOF/충돌 진단 없음, slvs는 GPL |
| URDF 리더 | **자체 구현**(roxmltree, 파일:줄:열 진단). xacro는 런타임 미지원(명확한 안내) | urdf-rs는 누락값을 0과 구분 못 하고 헬퍼가 ROS 도구를 실행 |
| IK | 범용 **수치 IK(LM)** + UR **해석적 IK**(Hawkins, BSD-3 포팅·보강) | 성숙한 Rust UR IK 없음 |
| 충돌 | **parry3d-f64 0.31**(`enhanced-determinism`), `gc_collide` 뒤에 격리 | glamx 타입 공유 |
| 플래너·단순화·타이밍 | **자체 구현**(RRT 계열, PRM, 숏컷/스무딩, 동기 사다리꼴, TOPP-RA) | 관련 크레이트 미성숙 |
| 수학 | 기하: **glamx 0.3(glam 0.33) f64**, `gc_math`에서만 노출. N차원: **자체 `gc_linalg`**. 삼각함수는 `gc_math::fm`(libm) 경유 | parry가 glamx로 이전, 재현성 |
| 단위/좌표 | 내부 **SI(m, rad)**, 표시 mm/deg. 오른손 **Z-up** | URDF/URScript와 일치 |
| 플러그인 | M1은 **컴파일타임 플러그인**(명시적 생성자 목록). 내장 기능도 같은 API로 등록 | Rust 안정 ABI 부재 |
| 의존성 | crates.io만, GPL/LGPL/AGPL/MPL/EUPL 금지, egui·wgpu·glamx·parry는 `=` 고정, `Cargo.lock` 커밋, `--locked` | github.com 차단, 저장소가 MIT |
| 저장소에 넣는 서드파티 자산 | Jetendard 폰트(OFL), UR e-Series YAML·충돌 STL(BSD-3) — 출처·해시를 `assets/SOURCES.toml`에 기록 | 이 계획 승인으로 동의하는 항목 |

## 3. 범위와 완료 기준

### 3.1 완료 기준 시나리오
1. 앱 실행 → 리본 / 브라우저(Origin: YZ·XZ·XY 평면, X·Y·Z 축, Center Point) / 뷰포트(그리드, ViewCube) / 속성 / 상태바.
2. 휠 줌(커서 기준), 가운데 드래그 팬, Shift+가운데 드래그·F4 오빗, F6 홈, 가운데 더블클릭 전체보기, ViewCube 클릭.
3. XY 평면 2D 스케치 → 정면 뷰 전환 → 선/원/호/사각형을 추론 스냅과 함께 작성 → 구속 → 치수(`25 mm`, `d0*2`) → 상태바 "N개 치수 필요 → 완전 구속" → 드래그 → 과구속 시 참조 치수 전환 프롬프트 → Undo/Redo.
4. 3D 스케치: 축 잠금 추론과 Tab 평면 전환으로 3D 폴리라인, 3D 거리·평면 거리 치수, 평면 위·축 방향 구속.
5. 설비 메시(STL/OBJ) 불러오기·수치 배치, 박스/원기둥/구 장애물.
6. 로봇: 라이브러리의 UR5e 추가 → 마운트 → 관절 조그 → 툴/TCP 수치 정의. 임의 `.urdf` 불러오기(진단 표시).
7. 스케치 점 → 웨이포인트 → 경로 → 알고리즘 선택 → **계획** → TCP 궤적·고스트 로봇·품질 지표, 타임라인 재생, **알고리즘 비교** 표. **동봉한 비-UR 6R 샘플 로봇으로도 같은 계획·재생**. 스케치 치수를 바꾸면 웨이포인트가 따라 움직이고 경로가 "오래됨" → 재계획.
8. 저장/열기(`.gctrl`), 자동 저장/복구, 예제 프로젝트(UR5e 셀) 열기.

### 3.2 우선순위 — P1에 없는 것은 모두 P2
| 영역 | P1 (M1 필수) | P2 (다음으로 넘길 수 있음) |
|---|---|---|
| 셸/뷰 | 리본, 브라우저, 속성, 상태바, 로그, 파라미터 표, ko/en, Inventor 프리셋, ViewCube 클릭·드래그·홈, 윈도/크로싱 선택, 고정 도킹 레이아웃 | SolidWorks 프리셋, ViewCube 인접면 화살표·롤, 레이아웃 저장, 명령 팔레트, 다크 테마 |
| 스케치 2D | 선·원·호·사각형·점, 구속 13종, 스마트 치수, 수식/파라미터, 참조 치수, 추론 스냅, 드래그, DOF 색상, 오프셋 작업 평면 | 트림, 선→접선 호 제스처, HUD 수치 입력 |
| 스케치 3D | 3D 점·선, 축 잠금, Tab 평면 전환, 일치·축 방향·평면 위·거리·평면 거리·고정, 외부 참조(2D 스케치 점) | 3D 각도·동일·평행/직각, 3D 호 |
| 로봇 | URDF 임포트·검증, FK, 수치 IK, UR 해석적 IK, UR3e/5e/10e/16e 패키지(충돌 STL로 표시), 조그, 툴·TCP 수치 편집, 수치 배치 | 시각 메시(DAE 변환), 드래그 기즈모, 보정 YAML, 5자유도 이하의 위치 전용 IK 목표 |
| 충돌 | `<collision>` 요소별 형상(메시는 연결 성분별 볼록 껍질), ACM 자동 생성(읽기 전용 목록 + 켜기/끄기), 쌍별 패딩, 접촉 허용 | VHACD, 인증 거리 경계 검사 |
| 계획 | PTP/LIN/Free, RRT-Connect·RRT·RRT*·Informed RRT*·PRM, 숏컷·부분 숏컷·스무딩, 목표 집합 규칙, 시퀀스 해석(DP), 결정적 best-of-4, 지표, 알고리즘 비교, TCP 구 모드, 재검증 우선 재계획 | CIRC, PRM*·LazyPRM, 하이브리드화, 경로 수리(repair), 탐색 트리 오버레이 |
| 타이밍 | 동기 사다리꼴, 블렌드 경로 + TOPP-RA(관절 행 + TCP 속도 상한), 검증 타입스테이트, 재생 | TCP 가속도 행, 저크 완화, rsruckig |

- **절대 줄이지 않는 것**: Inventor 내비게이션 + ViewCube 클릭, 치수·구속이 되는 2D/3D 스케치, 선택 가능한 플래너 4종 이상과 경로 표시·비교, UR과 비-UR 로봇에서 "URDF 임포트 → 계획 → 검증된 궤적", `Registrar`를 통한 플러그인 등록.
- **축소 순서**(게이트가 3회 수정 후에도 실패할 때만 오케스트레이터가 적용, `docs/DEVIATIONS.md`에 기록): ① 대칭·중점 구속 ② 자동저장 복구 UI ③ Informed RRT* ④ LIN의 TCP 속도 행 ⑤ 접촉 허용 UI(오류 구분은 유지).
- **제외**: 노드 그래프 편집기, STEP, 컨트롤러 연결·URScript 내보내기, 스플라인/패턴/오프셋/미러, WASM·프로세스 플러그인, 동역학, 설치 패키지.

## 4. 아키텍처

### 4.1 워크스페이스
레이어 순서 `foundation < api < domain < render < ui < app`. 같은 레이어 안의 의존은 아래 표에 적힌 것만 허용(비순환). `plugins/`는 foundation + api만 의존(예외: `gc_plugin_planners → gc_plan, gc_traj`). dev-의존은 예외. `cargo xtask check-layers`가 커밋된 `layers.toml`로 검사한다.

```
GraphCTRL/
├─ Cargo.toml  rust-toolchain.toml(1.97.0)  clippy.toml  deny.toml  layers.toml  kittest.toml
├─ assets/{fonts,ur}/ + SOURCES.toml     fixtures/{urdf,scenes,sketch}/     docs/{PLAN.md,research/,evidence/,STATUS.md,DEVIATIONS.md,KNOWN_ISSUES.md}
├─ xtask/        # gate, check-layers, check-licences, test-gpu, fetch-assets, gen-ur, bench-quality
├─ tools/oracles # 별도 워크스페이스(자체 Cargo.lock): sketch-solver·urdf-rs 차분 테스트
└─ crates/
   ├─ foundation/  gc_core  gc_linalg  gc_math  gc_mesh  gc_expr  gc_i18n  gc_jobs
   ├─ api/         gc_plugin_api  gc_robotics_api
   ├─ domain/      gc_solver  gc_sketch  gc_robot  gc_collide  gc_plan  gc_traj  gc_motion  gc_doc  gc_tools  gc_persist  gc_session
   ├─ render/      gc_draw  gc_render  gc_scene
   ├─ ui/          gc_ui  gc_viewport  gc_panels
   ├─ app/         gc_app (lib)  graphctrl (bin)  gc_test_support (dev)
   └─ plugins/     gc_plugin_ur  gc_plugin_mesh  gc_plugin_planners
```

| 크레이트 | 책임 | 워크스페이스 의존 (+ 외부) |
|---|---|---|
| `gc_core` | `Id<T>`/`AnyId`/`Kind`, 문서 전역 `IdAlloc`(단조 증가, 재사용 없음), `Entity`, `Revision`, `Tolerance`, 단위 뉴타입, `StableHasher` | — (serde, thiserror) |
| `gc_linalg` | dense 행렬, `qr`(LM 스택용), `qr_pivoted(Global \| Blocked{block_ends})` → 랭크·순열, `q_columns`(영공간), `solve_r11`, `lstsq_min_norm`, 대칭 Jacobi 고유값, Jacobi SVD(테스트 오라클), Cholesky | — (libm) |
| `gc_math` | glamx 재노출, 평면·광선·AABB, UR 회전벡터/URDF RPY 변환, `fm`(libm 삼각함수) | gc_core (glamx) |
| `gc_mesh` | `MeshData`(f32 위치·인덱스·법선), 내용 해시, `AssetStore`(Arc), 광선 캐스트용 삼각형 BVH, 연결 성분 분리 | gc_core, gc_math |
| `gc_expr` | 치수 수식(렉서 + Pratt 파서), 런타임 단위 검사, 의존 그래프·순환 검출 | gc_core |
| `gc_i18n` | 컴파일 시 검사되는 메시지 카탈로그(ko/en), `tr!` | — |
| `gc_jobs` | 작업·취소·진행률, `Executor`(`Thread`/`Inline`/`Manual`) | — |
| `gc_plugin_api` | `Plugin`, `Registrar`, `ExtensionPoint<T>`, `CommandSpec`, `Importer`, `ParamSchema`/`ParamValues` | gc_core, gc_mesh (serde) |
| `gc_robotics_api` | `Pose`, `ChainSpec`(+ 기준 FK), `KinematicsFactory/Solver`, `SpaceSpec`, `StateChecker`, `MotionPlanner`, `TimeParameterizer`, `RobotModelProvider`, 확장 지점 상수 | gc_core, gc_plugin_api |
| `gc_solver` | 구속 솔버(2D+3D), 진단, 드래그 타입스테이트 | gc_linalg |
| `gc_sketch` | `Sketch`(점·곡선·구속·치수·외부 참조), 솔버 바인딩, 추론/스냅, 3D 배치, 측정 | gc_core, gc_math, gc_expr, gc_solver |
| `gc_robot` | URDF 리더·검증 → 불변 `RobotModel`, `Chain`, FK, 자코비안, `IkSolver`(`solve_all`/`solve_near`/`track`), `NumericLm`, `Refined`, `PlanningProfile` | gc_core, gc_math, gc_linalg, gc_mesh, gc_robotics_api (roxmltree) |
| `gc_collide` | 로봇과 무관한 충돌 월드(바디 = 포즈 + 형상 목록), 장면 BVH, 쌍별 패딩, ACM 분류, 여유 거리 | gc_core, gc_math, gc_mesh (parry3d-f64) |
| `gc_plan` | `StateSpace`·유효성·`Planner` 트레잇, `JointSpace`, 플래너, 최근접 탐색, 경로 처리기, 고정 PRNG | — (smallvec) |
| `gc_traj` | C² 경로, 동기 사다리꼴, TOPP-RA, `Trajectory<Unvalidated/Validated>`, 통계 | gc_linalg |
| `gc_motion` | 모션 파이프라인(목표 집합 → 시퀀스 해석 → 세그먼트 계획 → 후처리 → 타이밍 → 검증·보증·지표). 플래너·IK·리타이머는 **API 트레잇 객체로만** 호출 | gc_core, gc_math, gc_jobs, gc_robot, gc_collide, gc_plan(경로 처리기), gc_traj, gc_robotics_api |
| `gc_doc` | 제네릭 `Store`/`Txn`/`History` + 구체 `Document`(Param, Frame, Sketch, Body, RobotInstance, Tool, Waypoint, Path), `Command` | gc_core, gc_math, gc_expr, gc_mesh, gc_sketch |
| `gc_tools` | UI 비의존 도구 상태기계, `ToolEvent`(미리 계산된 호버·월드 광선·픽셀 스케일 포함) | gc_core, gc_math, gc_sketch, gc_doc |
| `gc_persist` | 버전드 DTO(v1), 결정적 JSON, 원자적 저장, 자동저장, OS별 경로(`fs` 모듈) | gc_core, gc_doc, gc_sketch, gc_mesh (serde_json, tempfile) |
| `gc_session` | **egui 없는 앱 코어**: `Session`(문서·히스토리·캐시·레지스트리·작업), `Intent` 적용, 드래그 세션, 문서 → 장면 스냅샷/모션 요청, 작업 결과 수신, API ↔ 도메인 어댑터 | gc_doc, gc_persist, gc_motion, gc_collide, gc_robot, gc_jobs, gc_mesh, gc_tools, gc_plugin_api, gc_robotics_api |
| `gc_draw` | 카메라, 내비게이션 상태기계/프리셋, ViewCube 기하, 피킹(`PickId = AnyId`), 드로 리스트, 테셀레이션 | gc_core, gc_math, gc_mesh |
| `gc_render` | wgpu 30 렌더러(오프스크린 타깃, 파이프라인, 리드백) | gc_draw, gc_mesh, gc_math (wgpu, bytemuck) |
| `gc_scene` | `Session` 뷰 + 도구 미리보기 → `DrawList` + 2D 오버레이 항목 | gc_draw, gc_session, gc_doc, gc_tools, gc_sketch, gc_robot |
| `gc_ui` | UI 키트: 테마, 리본, 속성 그리드, 트리 래퍼, 벡터 아이콘, 텍스트 필드 래퍼 | gc_i18n (egui, egui_tiles, egui_ltreeview, egui-notify) |
| `gc_viewport` | 뷰포트 위젯: 입력 → 내비게이션/도구, 오프스크린 렌더, 오버레이 | gc_ui, gc_render, gc_draw, gc_scene, gc_session, gc_tools (egui-wgpu) |
| `gc_panels` | 브라우저, 속성, 파라미터, 로그, 경로/플래너, 타임라인, 조그 | gc_ui, gc_i18n, gc_session, gc_doc, gc_robotics_api |
| `gc_app` / `graphctrl` | eframe 배선, 설정, 파일 대화상자, 크래시 처리. `gc_app::run(plugins)`; `BUILTIN` 목록은 `graphctrl` bin에 | 전부 (eframe, rfd, anyhow, log) |
| `gc_plugin_ur` | UR 패키지(생성 URDF·충돌 STL·프로파일), `ur6r` 해석적 IK 계열 | gc_robotics_api, gc_plugin_api, gc_math, gc_linalg, gc_mesh |
| `gc_plugin_mesh` | STL/OBJ 임포터 | gc_plugin_api, gc_mesh (mesh-loader) |
| `gc_plugin_planners` | 내장 플래너·리타이머를 API 트레잇으로 감싸 등록 | gc_robotics_api, gc_plugin_api, gc_plan, gc_traj |

- **린트**: `unsafe_code = "deny"`(foundation/domain 루트는 `forbid`), clippy `all` warn. deny: `unwrap_used, expect_used, panic, todo, unimplemented, dbg_macro, exit, mem_forget, undocumented_unsafe_blocks`. warn: `iter_over_hash_type, float_cmp, map_err_ignore, let_underscore_must_use, print_stdout, print_stderr`. `pedantic`·`indexing_slicing`·`allow_attributes`는 켜지 않는다. `clippy.toml`: 테스트에서 unwrap/expect/panic/indexing 허용, `disallowed-methods`로 `f64::{sin,cos,tan,asin,acos,atan,atan2,exp,ln,powf,hypot}` 금지("gc_math::fm 사용"), ui/app/gc_session에서 `std::time::Instant` 금지(egui 입력 시간 사용).
- **프로필**: dev `opt-level = 1`, `debug = "line-tables-only"`; 의존성 `opt-level = 2`, `debug = false`; 수치 크레이트(`gc_linalg, gc_solver, gc_robot, gc_collide, gc_plan, gc_traj, gc_motion, gc_plugin_planners, gc_plugin_ur`) `opt-level = 3`; release `panic = "unwind"`, `lto = "thin"`.

### 4.2 핵심 패턴 (Rust 장점 활용)
- **타입드 ID**: `Id<Sketch>`와 `Id<Waypoint>`를 섞으면 컴파일 오류. 모든 id(스케치 내부 점·곡선 포함)는 문서 하나의 `IdAlloc`에서 나와 문서 전역 유일. `BTreeMap` 저장으로 결정적 순서.
- **변경은 `Txn`으로만**: Txn 엔티티는 최상위 항목(Param, Frame, **Sketch 전체**, Body, RobotInstance, Tool, Waypoint, Path). `Txn`이 최초 변경 전 이미지를 기록 → undo/redo는 같은 swap, 커밋 없이 drop되면 롤백(롤백은 swap만 하며 패닉하지 않음). `commit`이 참조 무결성 검증.
- **불변식 I1**: 커밋된 모든 스케치는 구동 구속을 허용오차 내에서 만족한다. 솔브가 `Inconsistent/NotConverged/Flipped`로 끝난 명령은 Txn을 버려 **통째로 롤백**하고 `EditError::SketchUnsolved{blame}`를 돌려준다(값 입력 상자는 인라인 오류와 함께 열린 채 유지). 로드는 솔브하지 않고 검사만 하며 위반 스케치는 `needs_repair`.
- **명령은 데이터**: `enum Command`(serde). UI는 `&Document`만 읽고 `Intent`를 큐에 넣으며 `gc_session`이 프레임당 한 번 적용.
- **연속 상호작용은 프레임마다 커밋하지 않는다**: 스케치 드래그·조그 슬라이더·수치 배치 미리보기는 `gc_session`의 세션 상태(미리보기 좌표)에만 반영하고 문서·`Revision`은 그대로다. `EndDrag`에서 Txn 하나로 최종 좌표를 기록(`Command::SetSketchCoords`) → undo 1단계, `Revision` +1. 시작과 비트 동일하면 커밋하지 않는다. Esc는 세션만 버린다. 드래그 중 Undo/Redo·다른 편집·작업 결과 반영은 보류.
- **삭제 연쇄**: 구속·치수까지만. 웨이포인트는 연쇄 삭제하지 않고, 참조 점이 삭제되면 같은 Txn에서 마지막 위치의 `Explicit`으로 **분리(detached)** 하고 경로를 "오래됨"으로 표시(일치 형제 점이 있으면 그쪽으로 재지정).
- **수식 한 번, 미분 자동**: 구속 잔차를 `Scalar` 제네릭으로 쓰고 `f64`와 `Dual<N>`으로 평가. `match`에 와일드카드 금지.
- **오류 정책**: 라이브러리는 `thiserror` enum, 앱 경계만 `anyhow`. 퇴화는 오류가 아니라 결과값.
- **결정성**: `HashMap` 순회 금지, 자체 PRNG, `fm` 삼각함수, 반복·검사 횟수 예산. 보장 수준은 "같은 플랫폼에서 비트 동일"(플랫폼 간은 최선). 커밋하는 골든 파일은 허용오차 비교.
- **패닉 격리**: `catch_unwind`는 세 곳만 — 작업 경계, 플러그인 호출 경계, `Session::apply_command`. 패닉 훅은 살아 있는 상태를 건드리지 않고, 자동저장 경로가 갱신해 둔 "마지막 커밋 문서의 직렬화 바이트"와 로그만 기록한다.

### 4.3 플러그인 구조와 API 계약 (Stage 1에서 동결)
- `trait Plugin { fn manifest(&self) -> &PluginManifest; fn register(&self, reg: &mut Registrar<'_>) -> Result<(), PluginError>; }`. M1 매니페스트는 `const` 구조체(`id, name, version, api: u32`), `api == GC_PLUGIN_API_VERSION` 검사. `plugin.toml`·semver 범위는 WASM/프로세스 단계로 미룬다. 0.0.x 동안 API 크레이트에 `#[non_exhaustive]`를 쓰지 않는다.
- `ExtensionPoint<T: ?Sized> { id, PhantomData<fn() -> Arc<T>> }`; Registrar는 `Arc<T>`를 `Box<dyn Any + Send + Sync>`로 저장. 확장 지점 상수는 트레잇을 가진 크레이트에 둔다(예: `gc_robotics_api::MOTION_PLANNERS`).
- 경계 타입은 `&[f64]`, serde POD, API 트레잇 객체뿐(glam/parry/egui 타입 금지):

```rust
pub struct Pose { pub xyz: [f64; 3], pub quat_xyzw: [f64; 4] }
pub struct JointSpec { pub name: String, pub kind: JointKind /* Revolute | Prismatic | Continuous */,
                       pub axis: [f64; 3], pub lower: f64, pub upper: f64, pub max_vel: Option<f64>, pub to_next: Pose }
pub struct ChainSpec { pub base_to_first: Pose, pub joints: Vec<JointSpec>, pub tip: Pose }   // 고정 조인트는 합쳐짐
impl ChainSpec { pub fn fk(&self, q: &[f64]) -> Pose }                // 기준 FK: gc_robot의 FK와 1e-12 일치 테스트
pub trait KinematicsFactory: Send + Sync { fn id(&self) -> &str;
    fn probe(&self, c: &ChainSpec, meta: Option<&FamilyMetadata>) -> Option<Fit>;   // Exact | NearNominal{pos_err, rot_err}
    fn build(&self, c: &ChainSpec) -> Result<Box<dyn KinematicsSolver>, KinError>; }
pub trait KinematicsSolver: Send + Sync {
    fn solve_all(&self, target: &Pose, seed: Option<&[f64]>, out_q: &mut Vec<f64>, out_branch: &mut Vec<u32>) -> Result<usize, IkFailure>;
    fn classify(&self, q: &[f64]) -> Option<u32>; fn branch_label(&self, branch: u32) -> String; }
pub struct SpaceSpec { pub lo: Vec<f64>, pub hi: Vec<f64>, pub weights: Vec<f64>, pub wrapping: Vec<bool> }
pub trait StateChecker: Sync { fn state_valid(&self, q: &[f64]) -> bool; fn motion_valid(&self, a: &[f64], b: &[f64]) -> bool;
                               fn clearance(&self, q: &[f64]) -> Option<f64>; }
pub trait MotionPlanner: Send + Sync { fn descriptor(&self) -> &PlannerDescriptor;
    fn plan(&self, req: &PlanRequest<'_>, chk: &dyn StateChecker, ctl: &dyn PlanControl) -> Result<PlanOutput, PlanFailure>; }
pub trait RobotModelProvider: Send + Sync { fn catalog(&self) -> Vec<RobotModelInfo>; fn urdf(&self, model: &str) -> Result<String, ProviderError>;
    fn resource(&self, model: &str, uri: &str) -> Result<Vec<u8>, ProviderError>; fn profile(&self, model: &str) -> Option<PlanningProfileData>; }
```
- **내장 기능이 API를 실제로 거치는 방식**: 로봇은 제공자가 준 **URDF 텍스트**를 사용자 파일과 같은 `gc_robot` 리더로 파싱. 해석적 IK는 `probe`로 스스로 적합 판정을 하고, 호스트(`gc_robot::Refined`)가 그 해를 실제 체인에서 LM으로 다듬는다(`gc_session`이 API 솔버 ↔ `IkSolver` 어댑터 소유). 플래너는 `gc_plan`에서 제네릭으로 작성하고 `gc_plugin_planners`가 `MotionPlanner`로 감싼다. `gc_motion`은 구체 플래너 이름을 모른다(문서의 `StrategyRef { planner_id, params }`로 레지스트리 조회).
- 수치 IK·경로 처리기·`PlanningProfile` 자동 유도는 호스트 기능으로 명시(플러그인 지점 아님). 실패한 플러그인의 기여만 비활성화.

### 4.4 한 프레임의 흐름
입력(egui) → 내비게이션 또는 활성 도구(`gc_tools`) → `Intent` 큐 → `gc_session::apply_intents`(`Command` → Txn + 솔브 → 커밋, `ChangeSet`, `Revision`, History) → 리비전·입력 해시 기반 캐시 무효화 → `gc_scene`이 드로 리스트·오버레이 생성 → `gc_render` 오프스크린 렌더 → egui 이미지 + 2D 오버레이. 무거운 일(계획, ACM, 임포트)은 `gc_jobs`; 결과는 문서가 아니라 `(JobId, Revision, 입력 해시)`가 붙은 파생 캐시로 들어온다.

## 5. 서브시스템 설계

### 5.1 문서 모델 · 저장
- **프레임 그래프**: World → 설비 바디 → 마운트 프레임 → 로봇 base_link → … → tip → TCP. 스케치(2D·3D 모두)는 `FrameId` 위에 놓인다. 평면 규약: 스케치는 프레임의 로컬 XY(U = +X, V = +Y, 법선 +Z). Origin 평면: XY(U+X, V+Y, n+Z), XZ(U+X, V+Z, n−Y), YZ(U+Y, V+Z, n+X). "정면 보기"는 −n 방향, up = V.
- **로봇 인스턴스**: 모델 참조(패키지 id 또는 URDF 자산 해시), 마운트 프레임 + 오프셋, 관절 상태, 한계 프로파일, 충돌 설정(패딩, ACM 재정의), 계획 체인.
- **툴**: `Tool { name, attach_link, mount: Pose, geometry: Vec<(Pose, Mesh | Box | Cylinder | Sphere)>, tcps: Vec<{name, pose}>, active_tcp }`. 목표 변환은 한 곳에서: `T_base_tip = T_world_base⁻¹ · T_world_target · T_tip_tcp⁻¹`.
- **웨이포인트**: `WaypointTarget::{Cartesian{position: SketchPoint | Explicit | Offset, orientation: Explicit | PlaneNormal{twist} | SameAs}, Joints(Vec<f64>)}`, 구성 힌트(자동 / 분기 번호 — 계열 중립적인 `u32` + 라벨), 블렌드, 속도.
- **경로**: `Path { subject: Robot{robot, tool, tcp} | ToolSphere{radius}, start: CurrentRobotState | NamedPose | Waypoint, segments: Vec<Segment{to, motion: Ptp | Lin | Free, strategy: StrategyRef, limits, contact: Option<ContactAllowance>}> }`.
- 파생 데이터(솔브 리포트, FK, 충돌 월드, 계획 결과)는 캐시. **세그먼트 입력 해시**: 로봇 모델 해시, 체인, 마운트, 툴·TCP, 한계 프로파일, ACM 해시, 패딩 정책, 장애물 집합(메시 해시·포즈), 해석된 시작 q, 해석된 목표, 세그먼트 파라미터, 플래너 id·버전, 시드, 예산. 세그먼트 k의 끝 구성이 바뀌면 k+1 이후가 "오래됨".
- 갱신 흐름: 파라미터 수정 → 의존 순서(파라미터 → 구동 치수 → 스케치 → 참조 치수/외부 참조, 순환은 인라인 오류)로 스케치 재솔브 → 프레임·웨이포인트 재계산 → 해시가 바뀐 세그먼트만 "오래됨".
- 저장: `.gctrl` = 버전 헤더가 있는 결정적 pretty JSON(DTO `v1`, `From` 마이그레이션), 메시는 상대 경로 + 해시. 임시 파일 → fsync → rename, `.bak` 1개. 지원 버전보다 새 파일은 거부.

### 5.2 뷰포트 · 렌더러 · 내비게이션
- `gc_draw::Camera`: f64 피벗·자세·거리(또는 ortho 높이), 직교↔원근 전환 시 화면 크기 유지, reverse-Z. 커서 기준 줌, 잡은 점이 커서 아래 유지되는 팬, 드래그 시작 시 커서 아래 모델 점을 피벗으로 하는 오빗(없으면 뷰 중심), 표준 뷰 전환 애니메이션(시간은 egui 입력 시간).
- **내비게이션 프리셋은 데이터**(`NavPreset { bindings, wheel_in_toward_user, zoom_to_cursor, orbit_mode, pivot_policy, .. }`, 부록 B). 뷰 변경은 undo에 넣지 않고 이전/다음 뷰 스택.
- ViewCube: 26개 클릭 대상, 드래그 오빗, 홈 버튼. 미니 뷰포트 렌더, CPU 피킹.
- 파이프라인: 메시(면 법선 음영, 틴트/반투명: 고스트·충돌 강조), 화면공간 일정 두께 라인·포인트(인스턴스 쿼드, SDF AA), 무한 그리드, 반투명 작업 평면, 스케치용 depth bias. 카메라 상대 f64→f32.
- **큰 장면 성능(1순위 요구)**: 메시 자산마다 정적 정점·인덱스 버퍼를 한 번만 올리고(같은 자산은 인스턴싱), 프레임마다 바뀌는 것은 인스턴스 변환·틴트뿐. 바디 단위 절두체 컬링, 드로 호출은 자산별 배치. 임포트·법선 계산·BVH 구축은 작업 스레드에서 하고 GPU 업로드만 UI 스레드에서 분할 수행(프레임당 예산). 카메라만 움직일 때는 CPU 쪽 재계산이 없어야 한다(드로 리스트 캐시, 변경된 것만 갱신). 정점 축소 LOD는 P2.
- **성능 목표와 측정**: 합계 500만 삼각형·바디 50개 장면에서 오빗/팬/줌 60 fps(일반 외장 GPU 기준, 사용자 PC에서 확인). 샌드박스에는 GPU가 없으므로 여기서는 프레임당 CPU 준비 시간(드로 리스트 갱신 + 컬링 ≤ 2 ms @ 바디 50개), 드로 호출 수, 임포트 처리량(삼각형/초)을 `bench-quality`로 측정·보고한다. 앱에 프레임 시간 오버레이(도구 > 성능)를 넣어 사용자 PC 측정을 쉽게 한다.
- 피킹은 전부 CPU: 스케치 엔티티는 화면공간 거리(점 > 곡선, 일치 클래스는 최저 id), 메시·링크는 `gc_mesh` BVH 광선.
- 2D 오버레이(치수, 글리프, 스냅 마커, 선택 박스)는 egui `Painter`. 뷰포트는 dirty일 때만 다시 그린다.

### 5.3 UI 셸 (Inventor식)
- 레이아웃(고정): 상단 메뉴 + **리본**, 좌 **브라우저**, 중앙 뷰포트, 우 **속성**, 하단 탭(로그 · 파라미터 · 경로 결과 · 타임라인), **상태바**(프롬프트 · 좌표 · 구속 상태 · 단위).

| 리본 탭 | 명령 그룹 |
|---|---|
| 3D 모델 | 스케치(2D, 3D 시작) · 작업 피처(오프셋 평면) · 장면(메시 불러오기, 박스/원기둥/구) · 파라미터 |
| 스케치 | 작성(선, 원, 호 3점/중심, 사각형, 점) · 구속(치수, 일치, 동일선상, 동심, 고정, 평행, 직각, 수평, 수직, 접선, 대칭, 동일, 중점) · 구성선 · 구속 표시(F8)/숨김(F9) · 마무리 |
| 3D 스케치 | 작성(선, 점) · 구속(치수, 일치, X/Y/Z 방향, 평면 위, 고정) · 마무리 |
| 로봇 | 추가(라이브러리, URDF 불러오기) · 배치(마운트, 툴/TCP) · 조그 · 충돌(ACM, 패딩, 충돌 형상 보기) |
| 경로 | 웨이포인트 추가 · 경로 만들기 · 계획 · 알고리즘 비교 · 재생 |
| 뷰 / 도구 | 표준 뷰, 직교/원근, 전체 보기, 표시 토글, 프리셋 / 설정, 언어, 로그, 라이선스 |

- 리본·속성 그리드·ViewCube·아이콘(코드로 그리는 벡터)은 자체 구현. 트리(`egui_ltreeview`)·토스트(`egui-notify`)·하단 탭(`egui_tiles`)은 `gc_ui` 뒤에 격리.
- **모든 동작은 명령**이고 리본·메뉴·단축키는 레지스트리의 뷰. 단축키는 `Event::Key`의 `physical_key`로 매칭(한글 자판에서도 동작), 텍스트 필드 포커스 중에는 단일 문자 단축키 비활성. 컨텍스트별 키맵 표(부록 B)와 "같은 컨텍스트에 같은 키가 둘이면 실패"하는 테스트.
- 폰트: **Jetendard**를 Proportional·Monospace 두 패밀리의 1순위로 등록(Regular = 본문, Bold = 강조), 없는 글리프는 egui 기본 → 시스템 폰트 폴백. 모든 텍스트 입력은 `gc_ui` 래퍼를 거친다. 아이콘 버튼에는 접근성 이름 필수.
- 선택: 클릭 / Ctrl 토글 / Shift 추가, 좌→우 윈도(실선)·우→좌 크로싱(점선), 호버 프리하이라이트, Esc는 한 단계 뒤로, Delete.

### 5.4 구속조건 솔버 (`gc_solver`)
1. **전처리**: 고정 파라미터 제거 → 가중 union-find로 `a=b`, `a=b+c`(일치·수평·수직·축 방향·수평/수직 거리)를 치환하며 **출처(구속 id)** 기록 → 이미 같은 클래스인 병합은 오프셋 일치면 중복, 아니면 충돌(병합 경로 지목) → 양 끝이 같은 클래스인 선은 `degenerate` → 연결 성분 분할. 스케치 구속은 치환되더라도 1급 엔티티로 남고 모든 보고는 스케치 id로 한다.
2. **풀이**: 성분마다 원점 이동·스케일 프레임(파라미터 종류별: 좌표는 이동+스케일, 길이는 스케일만, 비율·각도는 그대로)에서 **모든 행**에 LM(스택 `[J; √λ·I]`의 QR). 잔차는 길이 단위, 구속별 고정 스케일. NaN 금지(가드 + 대체 기울기 + `degenerate`). 수렴 상태에서 다시 풀면 한 스텝도 가지 않는다(멱등, 비트 동일).
3. **판정**: `|r| ≤ tol` → Solved / 정류점인데 `|r| > tol` → **영공간에 투영한 결정적 의사난수 방향**(강체 이동 방향 하나가 아님)으로 한 번 섭동 후 재시도, 실패 시 섭동 전 좌표로 `Inconsistent` / 예산 소진 → `NotConverged`.
4. **분기 유지**: 구속된 것만 서명에 넣는다 — 반지름 부호, 접선 쪽, 끝점 접선의 매끄러움/첨점, 부호 있는 치수의 쪽, 각도 분기, 호 스윕 연속성. 미구속 형상에는 서명이 없다(자유 폴리라인 꼭짓점은 일직선을 넘어 드래그 가능). 불일치 시 연속법(변경을 1/1024까지 반분), 그래도면 `Flipped`.
5. **진단은 해에서만**: `Jᵀ`의 클래스 블록 피벗 QR → 랭크, 종속 행과 계수 → `combo = r_k − Σc_j r_j`로 분류. 보고 단위는 **행**: `Dependency { row: (ConstraintId, u8), with: Vec<Blame>, kind: Redundant | Conflicting | PartiallyRedundant | SingularHere }`. `DOF = n − rank`, 영공간으로 파라미터별 이동성(보조 미지수는 색상·"치수 필요" 계산에서 제외).
6. **`try_add`는 DOF로 정의**: 사본에서 후보를 치환하지 않고 마지막 순서의 명시 행으로 추가. 솔브가 `Inconsistent`이거나 후보 행의 `|combo| > 1e-7`이면 Conflicting(후보가 주체). `dof_after < dof_before`면 Added(일부 행이 종속이어도). 같으면 증인 검사(영공간으로 1e-3·S 이동 후 재투영, 랭크 재계산): 랭크가 늘면 `AddedSingular`, 아니면 Redundant. 스케치 정책: 치수가 중복/충돌 → "참조 치수로 전환" 프롬프트, 명시적 기하 구속 → 거절 + 상태바에 관련 구속 표시, 추론 구속 → Added가 아니면 조용히 버림.
7. **드래그**(소유 타입스테이트 `Idle(System)` ↔ `Dragging`): 잡기 종류 `Point`(목표 = 커서) / `LineBody·ArcBody`(끝점 목표 = 시작 위치 + 커서 변위) / `CircleRim`(반지름 목표). 이동성 0인 파라미터만 빼고, 남는 게 없을 때만 거부. 프레임마다 **사전식 최적화**: ① 구속 만족 ② `‖목표 − 현재‖²` 최소 ③ `‖x − x_start‖²` 최소(**드래그 시작 상태 기준**, 직전 프레임 아님 → 경로 독립). 커서 이동은 성분 내 최소 엔티티 길이의 1/4 이하로 서브스텝. 스케일·서명은 시작 시 동결. 결과 `Moved | Clamped | RolledBack`.
- 구속 목록·저장 데이터·허용오차 표·추론 규칙은 부록 C. 2D: 일치, 점–곡선 위, 중점, 수평, 수직, 평행, 직각, 동일선상, 접선, 동일, 동심, 대칭, 고정 + 치수(거리, 수평/수직 거리, 점–선, 선–선, 각도, 반지름/지름). 3D(P1): 일치, 점–점 거리, 점–평면 위/거리, 축 방향, 고정.
- GPL/LGPL 솔버는 수학만 참고(코드 이식 금지). 차분 오라클은 `tools/oracles`(sketch-solver, 상태·DOF만 비교).
- 대비책: Stage 2a 끝에 F1–F20·G1–G7이 통과하지 못하면 2D는 `SolverBackend` 트레잇 뒤 sketch-solver 어댑터로 잇고 자체 백엔드는 트레잇 뒤에서 계속한다.

### 5.5 스케치 (`gc_sketch`, `gc_tools`)
- `Sketch { space: Planar(FrameId) | Spatial(FrameId), points, curves, constraints, dimensions }`. `SkPoint { pos: Uv | Xyz, kind: Free | External(FrameOrigin | WorkPoint | SketchPoint{sketch, point} | Broken{last}) }`. 곡선: `Line{a, b}`, `Circle{c, r}`, `Arc{c, s, e}`(CCW), 구성선 플래그. **각 곡선은 자기 끝점을 소유**하고 공유 모서리는 `Coincident` 구속(솔버에서 치환되어 좌표는 비트 동일, 글리프로 삭제 가능).
- 외부 참조 점은 출처가 있는 고정 파라미터이며 매 솔브 전에 원본에서 다시 계산된다(다른 스케치의 치수가 바뀌면 따라온다). 스케치 간 참조는 DAG(순환 생성은 거절), 원본이 삭제되면 `Broken{last}`. 원점 투영점은 스케치 진입 시 자동 생성되는 외부 참조로 삭제 불가.
- 치수: `{ id, param, source(수식 문자열), driving, text_pos, kind }`. 종류별로 분기 안정에 필요한 부호·쪽을 생성 시 저장(부록 C). 값 영역: 길이 > 0, 각도 (0, 2π). 참조 치수의 값은 저장하지 않고 `measure()`로 계산(괄호 표시). 참조되는 치수·파라미터를 삭제하면 같은 Txn에서 참조를 현재 숫자로 치환하고 알림.
- **스마트 치수 매핑**: 선→길이(커서로 정렬/수평/수직), 두 점→수평/수직/정렬, 평행 두 선→거리(평행 행 없이 1행), 그 외 두 선→각도(커서 사분면), 점+선→수직 거리, 원→지름, 호→반지름. 3D: 선→길이, 두 점→거리, 점+원점 평면→평면 거리. 배치 즉시 값 상자(선택된 상태), Enter 확정, 도구 유지.
- **추론/스냅은 순수 함수** `snap(cursor, px_scale, geometry, ctx{prev_point, drawing, memory, suppress}) -> SnapResult`: 스냅 종류별 조건·픽셀 허용오차·암시 구속 표(부록 C), 위치 스냅 1개 + 방향 스냅 최대 1개, 히스테리시스, Ctrl 억제. 암시 구속은 `try_add`를 거쳐 Added만 저장.
- **3D 점 배치** `place_3d(ray, prev, plane, ..)`: ① 기존 점 8 px 이내 → 그 위치 + 일치 ② 직전 점을 지나는 축에 8 px 이내이고 축이 시선과 10° 이상 벌어져 있으면 축 잠금 + 축 방향 구속 ③ 활성 주평면과 광선 교차(평면이 시선과 10° 이내로 누우면 배치 거부 + "Tab으로 평면 전환" 안내). Tab은 XY → YZ → ZX, 초기 평면은 시선과 가장 정렬된 평면.
- **도구는 UI 비의존 상태기계**: `ToolEvent → ToolFlow::{Continue, Commit(Command), Cancel}` + 미리보기 + 상태바 프롬프트. 사각형 도구 출력 = 선 4 + 일치 4 + H, V, H, V(DOF 4).
- 상태 표시: 엔티티별 색, 상태바 "N개 치수 필요"/"완전 구속", F8/F9 글리프, 글리프 호버 강조·Delete. 완전 구속 형상은 드래그해도 움직이지 않는다(히스토리·리비전 변화 없음).

### 5.6 로봇 모델(URDF) · 기구학 (`gc_robot`, `gc_plugin_ur`)
- **URDF가 기구학의 단일 원천**. 자체 리더가 URDF 1.0–1.2를 읽어 `Option`이 정확한 중간 표현 → 검증 진단(데이터, 파일:줄:열) → 불변·`Sync` `RobotModel` + 스레드별 스크래치. 의미 규칙: `R = Rz(yaw)·Ry(pitch)·Rx(roll)`, `T(q) = T_origin · T_motion(q)`, 축 정규화, mimic `q = m·q_master + o`(마스터 한계에 mimic 한계를 교차), 원기둥 Z축(parry는 Y → 보정 포즈), 메시 scale·단위 경고. 리소스 URI는 `ResourceResolver`로 해석. 오류·경고 목록은 `docs/research/robot-model.md` §3.
- **`PlanningProfile`**(URDF 밖 정보, 패키지 데이터 + 자동 기본값, 출처 표시): 계획 체인(tip은 `tool0`가 있으면 그것, 없으면 가동 조인트가 가장 많은 리프; 후보가 여럿이면 사용자가 선택), 가속도 한계(**가정값**이면 UI에 표시), **비용 가중치 = 시간 비례** `w_j = (1/v_max_j) / max_k(1/v_max_k)`, ACM, 패딩, 이름 있는 자세, (UR) 팔꿈치 한계 `Official(±π) | Hardware(±2π)`.
- FK: 변환 체인, 호출자 소유 스크래치. 자코비안: revolute `Jv = z × (p_tip − p_i), Jω = z`, prismatic `Jv = z, Jω = 0`, mimic은 마스터 열에 `m·열` 가산. 특이점 지표: 정규화한 `J·Jᵀ`의 Jacobi 고유값 σ_min.
- **`IkSolver`**: `solve_all`, `solve_near`, `track(target, prev_q)`(직교 경로 추종; 기본 구현은 감쇠 최소제곱 연속 + 샘플당 `‖Δq‖∞` 상한 + 3회 이분, 해석적 계열은 재정의). UI에 "IK: 해석적, N분기" / "IK: 수치(샘플링)" 표시.
- **`NumericLm`**: 오차 `[(p*−p)/L ; log(R*·Rᵀ)]`, 위치/자세 허용오차 분리(1e-6 m, 1e-6 rad), Sugihara식 감쇠, 게인 비율 수락, 한계는 active-set, 시드 고정 재시작, 해 중복 제거, `IkFailure{best_q, pos_err, rot_err, reason}`.
- **UR 해석적 IK**(`gc_plugin_ur`): Hawkins `ur_kin.cpp`(BSD-3, 고지 유지)를 `T_DH`에 직접 작용하도록 포팅. 모든 `acos/asin` 범위 검사·클램프(원본은 NaN 가능), 스케일 상대 임계값, 손목 특이점에서 `q6`는 시드에서. 최대 8분기를 (−π, π]로 정규화, 분기 번호와 라벨(어깨·팔꿈치·손목), 한계 내 2π 턴 변형 열거.
- **계열 선택**: 패키지 메타데이터 → `probe`(구조 검사 후 DH 적합, 64개 무작위 q에서 FK 일치: <1e-9 Exact, <5 mm/0.5° NearNominal) → 그 외 수치 IK.
- **UR 패키지**: `cargo xtask fetch-assets`가 고정 커밋 SHA의 공식 YAML 4종과 충돌 STL을 받아 `assets/ur/` + `SOURCES.toml`(URL·SHA-256·라이선스)에 기록(Stage 0에서 1회). `cargo xtask gen-ur`(오프라인, 결정적)가 `ur_macro.xacro`의 고정 토폴로지를 재현해 `.urdf` 생성: 루트는 `base_link`(`world` 없음), `base`/`flange`/`tool0`/`ft_frame` 포함, 계획 체인 `base_link → tool0`. YAML은 xtask의 작은 들여쓰기 리더(`!degrees` → ×π/180). 테스트: 커밋 파일의 SHA-256 검사, 생성 결과 == 커밋본, URDF FK == DH, 적합 결과가 pre = Rz(π)·post = I·부호 +1·오프셋 0. e-Series 4종만(BSD-3), Smart Panel·최신 기종 메시는 동봉하지 않음.
- **비-UR 검증용 픽스처**(자체 작성, 박스/원기둥/구 형상만, `fixtures/urdf/`): ① UR이 아닌 오프셋의 6R(해석 계열 판정이 "해당 없음") ② continuous 조인트가 있는 7자유도 ③ prismatic이 있는 4자유도(관절 공간 목표로 계획). ①은 샘플 로봇으로 라이브러리에도 노출.

### 5.7 충돌 (`gc_collide`)
- 로봇: **`<collision>` 요소마다 형상 하나**(box → cuboid, sphere → ball, cylinder → 보정 포즈, mesh → 정점 용접 후 **연결 성분별 볼록 껍질**; 퇴화 시 AABB + 경고). 링크 = (포즈, 형상) 목록. 툴 형상은 부착 링크의 추가 바디(툴 ↔ 부착 링크는 항상 허용, 툴을 붙이거나 떼면 툴 쌍만 ACM 증분 재계산).
- 환경: 정적 `TriMesh`(정점 용접, 퇴화 삼각형 제거)·기본 도형, 장면 `Bvh`. 임포트 시 단위 의심(경계 상자 >50 m 또는 <1 mm) 경고와 mm→m 변환 제안.
- **패딩은 쌍 정책**: 로봇 바디 ↔ 장애물은 로봇 기본 5 mm(링크별 재정의), 자기 충돌 쌍은 0. `closest_points(max_dist = pad)`로 판정.
- **접촉 허용**: `ContactAllowance { body, obstacle, scope: Waypoint | Segment, radius(기본 50 mm), allow_touch }` — 웨이포인트 TCP 주변 반경 안에서는 그 쌍을 패딩 0으로 검사(관통은 `allow_touch`가 아니면 금지). 오류를 `GoalWithinPadding{distance}`와 `GoalInCollision{depth}`로 구분하고 전자에 허용 추가를 제안한다.
- **ACM**: 인접(Adjacent) / 기본 자세 충돌(Default) / 10,000 샘플 중 ≥95%(Always) / 0회(Never, 패딩 포함 측정) / 그 외 Sometimes(p). 20% ≤ p < 95%는 "볼록 껍질 과대 근사 가능" 경고. 사유와 함께 저장, UR은 `gen-ur --acm`으로 미리 계산해 동봉.
- 상태 검사: 링크 AABB → 장면 BVH → 겹치는 쌍만 정밀. 원위 링크·환경·직전 충돌 쌍 먼저. 결과는 최초 충돌 `Hit`.
- **엣지 검사**: 스텝 수 = `ceil(max(max_j|Δq_j| / 1°, sweep / δ))`, `sweep(a,b) = max_i Σ_{j≤i} r_ij·|Δq_j|`(`r_ij` = 조인트 축 j에서 링크 i 바디 정점까지 거리의 상계, 임포트 시 계산), prismatic은 10 mm. 불변식 `δ ≤ 2·pad_min`(기본 δ = 10 mm). 이분 순서.
- **보증 문구(결과에 기록)**: "패딩 p, 간격 δ ≤ 2p에서 패딩 없는 볼록 모델이 장애물에 닿지 않음. 제외: ACM으로 끈 쌍, 닫힌 메시 내부에서 시작한 바디, 컨트롤러 추종 오차." 패딩 0이면 "샘플링만". 보증은 최종 `q(s)`에 대해 `validate()`가 계산한 것 하나만 쓴다.
- parry `Unsupported`는 타입드 오류(절대 "충돌 없음"으로 취급하지 않음).

### 5.8 경로 계획 (`gc_plan`, `gc_motion`)
- `gc_plan`은 로봇을 모른다: `JointSpace { kinds: Bounded{lo, hi} | Wrapping, weights }`(상태 = `SmallVec<[f64; 8]>`). UR 조인트(±2π)는 상자이며 `q`와 `q+2π`는 다른 상태. Wrapping 조인트가 있으면 경로는 unwrap해서 돌려주고 Informed 샘플링은 끈다(안내 표시).
- 플래너(`PlannerDescriptor`: id, 이름, `ParamSchema`, 능력 플래그): **RRT-Connect(기본)**(양방향, greedy CONNECT, 다중 목표 시드), RRT, RRT\*(k-최근접 `k = ⌈(e + e/d)·ln n⌉`, 재배선·비용 전파·가지치기), Informed RRT\*, PRM(k = 10, A\*). 공통: 확장당 `|Δq_j| ≤ 0.5 rad`, **턴 국소 샘플링**(`bbox(start ∪ goals) ± π`, 실패 시 전체 한계로 1회 재시도 후 보고), 최근접은 선형 스캔으로 시작, 종료는 반복·검사 횟수·비용·취소(256 상태 검사마다 폴링), PRNG는 자체(SplitMix64 → xoshiro256++).
- **경로 처리**: 정점 축소 → 숏컷 → 부분 숏컷(조인트별) → B-스플라인 코너 절단(유효할 때만) → 조밀화 → **독립 재검증**(필수).
- **목표 집합 규칙**: IK 분기마다 시작에 가장 가까운 턴 변형(최대 8) → 한계·충돌로 거름 → `cost(start, g) + 힌트 벌점 + λ·Σmax(0, |q_j| − π)`로 순위(±2π 쪽으로 흘러가지 않게) → 남는 자리는 차순위 턴 변형 → 동률은 (분기, 턴) 순. 도달한 목표의 분기 라벨·턴·순위를 보고.
- **시퀀스 해석(샘플링 전에 수행)**: 직교 웨이포인트마다 후보 구성 최대 16개 → LIN 세그먼트는 후보 쌍마다 추종기를 돌려 가능한 쌍과 비용 산출 → 동적 계획법으로 웨이포인트당 구성 하나 확정(Free/PTP 간선은 직선 거리 비용). 실패 시 `SequenceInfeasible { segment, tried }`. Free는 다음 세그먼트가 제약 없을 때만 다중 목표.
- **Free 기본 파이프라인**: 확정 목표 → 직선 관절 세그먼트 먼저 시도 → 막히면 선택한 플래너를 **시드 4개로 끝까지 돌려 최소 비용 선택**(동률은 낮은 인덱스; 결정적) → 경로 처리 → 타이밍.
- **PTP**: 직선 관절 보간 검증, 막히면 충돌 보고(옵션 "자동 회피"). **LIN**: 1–2 mm·0.5° 샘플(자세 slerp), `track`으로 분기 유지, 점프·한계·특이점(σ_min)·충돌 시 달성 비율과 원인 보고.
- **시작 상태**: 한계를 ≤1e-3 rad 벗어나면 클램프(보고). 패딩 안쪽 시작은 **옮기지 않고** 여유가 패딩 이상이 될 때까지 첫 구간을 줄인 패딩으로 계획(보증에 구간 기록). 관통 시작은 오류.
- **재계획 사다리**: ① 기존 경로를 새 입력에 재검증 → 유효하고 끝점이 같으면 유지("유효, 재최적화 안 함") ② 아니면 이전 목표 분기·턴을 1순위로 전체 계획. (기존 내부 정점을 재연결하는 수리는 P2.)
- **오류**: `StartInCollision`, `GoalInCollision`, `GoalWithinPadding`, `GoalUnreachable{OutOfReach | JointLimit{joint} | NoIkBranchMatchesHint}`, `SequenceInfeasible`, `NoPathWithinBudget{stats, nearest_goal_distance}`, `CartesianIncomplete{fraction, cause}`, `Cancelled`.
- **TCP 구 모드**: 로봇 없이 반지름 r의 구를 R³에서 같은 플래너·파이프라인으로 계획.
- 품질 기준은 부록 E(결정적 단위).

### 5.9 궤적 · 작업 (`gc_traj`, `gc_jobs`)
- **경로와 시간 법칙 분리**: 궤적 = 기하 경로 `q(s)`(C²) + 시간 법칙 `s(t)`.
- PTP **동기 사다리꼴** 오라클: `V = min_j v_j/|Δq_j|`, `A = min_j a_j/|Δq_j|`, `T = 1/V + V/A`(`V² ≤ A`일 때) 아니면 `2/√A`.
- 다점 경로: 편차 한계가 있는 블렌드로 C² 경로 → **충돌 재검사**(실패 시 블렌드 반경을 최대 4회 반분 → 0, `BlendReduced` 보고) → **TOPP-RA**(격자 N ∈ [100, 2000], 블렌드 경계 포함, 양 끝 `ṡ = 0`, 관절 속도·가속도 행, TCP 속도는 `ṡ` 상한). 검증 실패 시 N 2배(3회) → 한계 0.95배(3회) → `TimingFailed{joint, s, kind}`.
- **대비책**: TOPP-RA가 수용 테스트를 통과하지 못하면 "세그먼트별 동기 사다리꼴(정점에서 정지)"을 P1 타이머로 출하하고 TOPP-RA는 P2.
- LIN 타이밍: `ds = max(dp / 1 m, dθ / π)` 파라미터에 같은 TOPP-RA. 특이점 근처에서는 실패 대신 TCP 속도가 떨어지고 "프로그래밍 속도 미달 구간 x%" 보고.
- 검증 타입스테이트: `Trajectory<Unvalidated>` → `validate()` → `Trajectory<Validated>`(+ `Guarantee { padding_nominal, padding_min, reduced_intervals, step_deg, step_mm, model, acm_hash }`). 2 ms 샘플에서 한계, 적응 간격 충돌 재검사.
- 한계: 속도는 URDF에서(하드코딩 금지), 가속도는 로봇별 프로파일(UR: "MoveIt 보수값 5 rad/s²" / "URScript 기본 1.4 rad/s²·1.05 rad/s" / 사용자 정의). 결과는 "**예상** 사이클 타임".
- 작업: 스레드 + 불변 `Arc<SceneSnapshot>` + 취소 + 진행률, `Notifier`로 repaint. 실행기는 주입(`Thread` / `Inline` / 테스트가 한 틱씩 진행시키는 `Manual`). 같은 종류의 새 작업은 이전 작업 취소. 취소는 100 ms 내 반영(테스트).

### 5.10 웨이포인트 · 경로 UX
- 웨이포인트 추가(스케치 점 선택, 자세 기본값: 평면 스케치는 툴 Z = −평면 법선, 3D 스케치는 툴 Z = −월드 Z; RPY/회전벡터 편집, 쿼터니언이 원본), 관절 웨이포인트(현재 조그 자세 저장).
- 경로 만들기(웨이포인트 순서 선택 또는 스케치 체인: 선 → LIN), 세그먼트 속성(모션 종류, 플래너·파라미터(스키마로 자동 생성되는 편집기)·시드·예산, 속도·가속도 스케일, 블렌드, 접촉 허용).
- **계획** → 세그먼트별 상태·진단·지표·보증. **알고리즘 비교**: 선택한 플래너들을 시드 스윕(기본 20)으로 돌려 성공률, 상태 검사 수, 계획 시간(참고), 관절·TCP 길이, 최소 여유, 노드 수 표.
- 표시: 세그먼트별 색 TCP 궤적, 시작/목표/호버 웨이포인트의 고스트 로봇, 타임라인 스크러버(재생 중에만 repaint), 충돌·패딩 축소 구간 눈금과 해당 링크·장애물 강조, 웨이포인트 도달성 색, 조그 슬라이더의 한계 근접 표시.

### 5.11 앱 배관 (P1, Stage 5e 게이트 체크리스트)
1. 경로: 자체 조회(Windows `%APPDATA%`/`%LOCALAPPDATA%`, Linux XDG) — `settings.json`(스키마 버전, 언어, 프리셋, 단위, 최근 파일, 창), `recovery/`, `logs/`. 도움말 > 정보에 경로 표시.
2. `trait FileDialogs`(`RfdDialogs` / 테스트용 `ScriptedDialogs`); kittest는 rfd를 건드리지 않는다.
3. 더티 추적(현재 리비전 ≠ 저장 리비전), 제목 `이름* — GraphCTRL`, 닫기·새로 만들기·열기 시 저장/버리기/취소 모달.
4. 오류 정책: 복구 가능 → 토스트 + 로그 / 결정 필요 → 모달 / 진단 목록(URDF·솔버·플래너) → 해당 패널 / 패닉 → 복구 저장 + 네이티브 메시지 + 로그.
5. 로그: stderr + 파일 + 로그 패널 링 버퍼.
6. `windows_subsystem = "windows"`(release), 창 아이콘(내장 PNG).
7. 도움말 > 라이선스: `THIRD_PARTY_NOTICES` 내장 표시(Jetendard OFL + 원 폰트 고지, UR BSD-3, Hawkins BSD-3).
8. 파일 > 예제 열기: 동봉 UR5e 셀(골든 장면 픽스처), 빈 상태에서 제안.
9. 자동저장 `recovery/<문서 uuid>.gctrl` + 정상 종료 표시.

## 6. 구현 단계와 게이트

| 단계 | 내용 | 게이트(증거) |
|---|---|---|
| **0. 골격·환경** | 워크스페이스·린트·프로필, 전 크레이트 스텁과 의존성(부록 A 전체 목록) → `Cargo.lock`. `docs/`에 계획·리서치·리뷰 복사, STATUS/DEVIATIONS/KNOWN_ISSUES 생성. `fetch-assets`(폰트, UR) 커밋. CI 워크플로를 첫 커밋으로 푸시(거부되면 `ci/` 템플릿). `rustup target add x86_64-pc-windows-msvc`. 최소 앱 + **한글 입력 시험용 화면**(텍스트 필드, 치수 입력 상자 모형, 단축키 표시) | ① 스텁 워크스페이스 build·clippy 통과, `cargo tree -d`에 wgpu/naga/glam/egui/winit 단일 ② `gc_render` clear + 리드백 픽셀 단언 ③ 오프스크린 파이프라인의 kittest 스냅샷 PNG ④ `xvfb-run graphctrl --smoke-frames 5 --screenshot`(실패 시 "사용자 PC에서만 확인"으로 기록) ⑤ 콜드 빌드 시간·`du -sh target` 기록 ⑥ **한글 입력 시험을 사용자에게 요청**(Windows·Linux에서 최소 앱 실행, 체크리스트 5항목) — 결과가 "사용 불가"면 Stage 4 전에 프론트엔드 대비책을 재평가(그동안 UI와 무관한 Stage 1–3은 계속 진행) |
| **1. 기반 + 계약 동결** | foundation 7개, api 2개. 나머지 크레이트의 **공개 타입·트레잇 시그니처**를 본문 없이 커밋(이후 변경은 오케스트레이터만). 수용 테스트용 **픽스처 작성**(장면 S1–S4, URDF 3종, 스케치 F/G) — 구현자와 다른 에이전트가 사양에서 작성 | QR 랭크 == SVD 랭크 속성 테스트, 수식 파서, 작업 취소·패닉 격리, API 트레잇 객체 안전 컴파일 테스트 |
| **2a** | `gc_solver` ∥ `gc_robot` ∥ `gc_plan` ∥ `gc_traj` ∥ `gc_draw` ∥ `gc_doc`(제네릭 store/txn/history) | F·G 솔버 픽스처, 솔브 성능(50/100/300 미지수; 100에서 드래그 프레임 ≤ 4 ms 목표), 픽스처 URDF의 FK·IK, 플래너 속성 테스트(해석 세계), 사다리꼴·TOPP-RA 오라클, 카메라·피킹 수치 |
| **2b** | `gc_sketch` ∥ `gc_collide` ∥ `gc_render` ∥ `gc_plugin_mesh` ∥ `gc_plugin_ur` | 스냅·배치·측정 테스트, 충돌 vs 무차별 대입 비교, 렌더 리드백, UR 4종 "생성 == 커밋 / FK == DH / IK 왕복"(부록 D 값) |
| **3a** | `gc_doc`(구체 모델·Command·연쇄, **스키마 동결**) ∥ `gc_motion` ∥ `gc_plugin_planners` | undo∘redo = id, I1 속성 테스트, **범용 URDF 게이트**(픽스처 3종이 임포트 → 자동 프로파일·ACM → 계획 → 후처리 → 타이밍 → `Validated`; 로봇 이름으로 분기하는 코드 금지), UR5e 장면 S1–S4 |
| **3b** | `gc_persist` ∥ `gc_tools` ∥ `gc_session` ∥ `gc_scene`, UR ACM 골든 생성 | **헤드리스 시나리오 테스트**: 3.1의 3–8을 ToolEvent/Command와 `InlineExecutor`로 실행, 문서 상태·계획 지표 단언(GPU 불필요). save∘load = id |
| **4** | `gc_ui`, `gc_viewport` | kittest 입력 시뮬레이션, 스냅샷. **UI 확인 지점**: 리본 + 브라우저 + 속성 그리드 + 상태바의 실제 화면 스크린샷을 사용자에게 보여 주고, 승인 후 Stage 5로 진행(불만족 시 UI 계층만 교체하는 대안을 그 시점에 결정 — 코어·렌더러는 유지) |
| **5a → 5f** | 5a 셸·명령·내비게이션(3.1 #1–2) → 5b 2D 스케치 UI(#3) → **5c 로봇·장면·웨이포인트·계획·재생(#5–7)** → 5d 3D 스케치 UI(#4) → 5e 저장·설정·배관(#8, 5.11) → 5f 알고리즘 비교와 남은 P1 | 하위 단계마다 kittest 시나리오 + 스냅샷 체크리스트, 따로 커밋 |
| **6. 하드닝** | 코드 전반 반박 리뷰, 수정, README(한국어), 고지, P2·보류 목록 | 전체 게이트 + Tier B 보고서 |

**게이트 규약**
- 한 명령 `cargo xtask gate`: fmt, clippy(호스트 + `--target x86_64-pc-windows-msvc`), Tier A 테스트, `test-gpu`, `check-layers`, `check-licences`, 디스크 점검(target 20 GB 상한).
- 단계마다 `docs/evidence/stage-N.md`: 실행 명령과 요약, `#[ignore]` 목록과 사유, 새 스냅샷 PNG별 "보여야 할 것" 체크리스트와 확인 결과, Tier B 수치, 리뷰 지적과 처리. `docs/STATUS.md` 갱신 후 브랜치 `claude/ur-robot-control-simulator-jsmhze`에 커밋·푸시(PR은 요청 시에만).
- 수용 테스트(F/G 픽스처, Q1, undo∘redo, save∘load, 시나리오)는 구현자가 고칠 수 없다. 허용오차·기대값 변경은 `DEVIATIONS.md`에 이전 값·새 값·근거를 남기고 오케스트레이터만.
- **면제 불가**: Q1 유효성, 어떤 입력에도 패닉 없음, undo∘redo = id, save∘load = id, 같은 시드 재실행 비트 동일, 레이어 검사, Windows 교차 검사. 그 외 항목은 수정 3회 후에도 실패하면 `KNOWN_ISSUES.md`에 재현 절차와 함께 기록하고 보고한 뒤 진행.
- **병렬 규약**: 동시 구현 에이전트 최대 4. 각 에이전트는 자기 크레이트에서만 `cargo check|clippy|test -p <crate> --locked`, 헤드리스 크레이트는 `CARGO_TARGET_DIR=target/agents/<crate>`, egui/wgpu 의존 크레이트는 기본 target에서 한 번에 하나. 루트 매니페스트·`Cargo.lock`·API 크레이트·계약 모듈은 오케스트레이터만 수정(의존성 요청은 서면으로). 크레이트는 의존하는 워크스페이스 크레이트가 모두 이전 웨이브에서 병합·통과된 뒤에만 시작.

## 7. 검증

- **Tier A** = `cargo test --workspace --locked`(dev 프로필, `PROPTEST_CASES=64`, 크레이트당 ≤ 60 s, 전체 ≤ 10분, 플래너 질의당 시드 ≤ 5, 벽시계 단언 없음). **Tier B** = `cargo xtask bench-quality --release`(시드 100 스윕, `PROPTEST_CASES=2048`, `docs/quality/report-<stage>.md`; Stage 3a·5c·6에서 실행). 퍼징은 proptest의 시드 기반 무작위 입력(URDF 텍스트, 수식, 솔버 시스템)으로 대체하고 회귀 파일 커밋.
- **GPU 테스트**: `cargo xtask test-gpu`가 SwiftShader ICD를 glob(`/opt/pw-browsers/chromium-*/chrome-linux*/vk_swiftshader_icd.json`)으로 찾아 자식 프로세스에만 `VK_ICD_FILENAMES`·`WGPU_BACKEND=vulkan`·`GC_REQUIRE_GPU=1`을 준다(이 변수를 `.cargo/config.toml`에 커밋 금지). `gc_test_support::gpu()`가 어댑터를 먼저 탐지해 없으면 SKIP(단 `GC_REQUIRE_GPU=1`이면 실패). GPU 테스트는 전용 테스트 바이너리, `--test-threads=1`. SwiftShader가 안 되면: lavapipe 설치 시도 → 그래도 안 되면 Stage 4·5 게이트를 "GPU 없는 시나리오 테스트 + naga 검증 + 사용자 수동 체크리스트"로 바꾸고 최종 보고 맨 위에 명시.
- **UI 테스트 규칙**: 애니메이션·재생·디바운스는 egui 입력 시간만 사용. 정착이 필요할 때만 `run()`, 재생·작업 진행 중에는 `run_steps(n)`과 헬퍼 `pump_until(cond, max_steps)`. 하네스는 1280×800, ppp 1.0, 라이트 테마, 로케일 `ko` 고정, 번들 폰트만. 위젯은 `tr!(key)`나 명령 id로 찾는다. 스냅샷 기준 렌더러는 SwiftShader(어댑터 이름에 SwiftShader가 있을 때만 엄격 비교, 그 외에는 `.new/.diff`를 남기고 통과), 3D 정확성은 수치 단언. `max_failed_pixels` 사용. 모든 i18n 문자열의 글리프가 번들 폰트에 있는지 검사하는 단위 테스트. 기준 이미지 갱신은 증거 문서에 "무엇을 눈으로 확인했는지" 한 줄과 함께 기록(무검토 `UPDATE_SNAPSHOTS`는 게이트 실패).
- **시나리오**(헤드리스 + kittest): 부록 C의 S1–S9, 5.11 항목, "UR5e·비-UR 로봇 추가 → 웨이포인트 → 계획 → 재생".
- **정적 검사**: clippy, rustfmt, `check-layers`, `check-licences`(cargo metadata 라이선스 허용 목록 + 금지 목록 + git 소스 금지). cargo-deny는 CI에서만.
- **Windows**: 매 게이트에서 MSVC 타깃 `cargo check`·clippy(C 빌드 스크립트가 막는 크레이트만 예외 기록). OS별 코드는 `gc_persist::fs`, `gc_app::platform`에 격리. CI가 돌면 실행 URL을 증거 문서에 첨부.
- **사용자 PC 확인**(샌드박스에서 검증 불가로 보고): `cargo run -p graphctrl --release` 후 3.1 시나리오, Windows 한글 입력 체크리스트(치수 입력, 이름 변경, 한글 자판 상태의 도구 단축키).

## 8. 위험과 대응
| 위험 | 대응 |
|---|---|
| 범위가 매우 큼(약 30개 크레이트) | 수직 슬라이스 순서(셸 → 2D 스케치 → 로봇·계획 → 3D 스케치 → 배관), P1/P2와 축소 순서, 단계별 증거·커밋, `docs/STATUS.md`로 재개 가능 |
| egui 0.37 API가 기억과 다름 | Stage 0에서 실제 소스 기준 컴파일, UI 크레이트 `#![deny(deprecated)]` |
| SwiftShader에서 wgpu 30 미실행 | Stage 0의 세 가지 증명, GPU 선택 사항 설계, 대체 게이트 명시 |
| 솔버 수치 강건성 | 리뷰 사양(부록 C) + 픽스처 + 오라클 + sketch-solver 어댑터 대비책 |
| 계획 성능 미측정 | Stage 2a·3a에서 조기 측정, 느리면 캡슐 프록시·환경 메시 단순화 |
| 볼록 껍질 과대 근사, ACM "Never"는 통계적 | 요소·성분별 껍질, Sometimes 경고, 켜기/끄기 UI, 보증 문구에 제외 사항 명시 |
| 실제 UR 컨트롤러와의 차이 | "예상" 표기, 한계 프로파일 편집, URSim 비교는 후속 |
| 파괴적 릴리스(egui/wgpu/glam/parry) | `=` 고정, 격리 크레이트(`gc_ui`, `gc_math`, `gc_collide`, `gc_render`) |
| 한글 IME(winit 미해결 이슈, 샌드박스에서 검증 불가) | Stage 0에서 사용자 PC 시험을 먼저 요청, 텍스트 래퍼 단일화, 물리 키 매칭. 실패 시 UI 계층만 교체(렌더러·코어는 유지) |
| 큰 장면 3D 성능을 샌드박스에서 실측 불가(GPU 없음) | 정적 버퍼·인스턴싱·컬링 설계, CPU 준비 시간과 드로 호출 수를 측정, 프레임 시간 오버레이로 사용자 PC에서 확인 |
| egui 메인테이너 의존(0.x, 연 3–4회 파괴적 릴리스) | `=` 고정, 분기별 업그레이드, 서드파티 위젯은 `gc_ui` 뒤, 렌더러는 egui 비의존 |
| 라이선스 | 허용 목록 검사, 자산 출처·해시 기록, 고지 내장 |
| `.github/workflows` 푸시 거부 | Stage 0 첫 커밋으로 확인, 거부 시 템플릿 + "Windows는 교차 검사만" 보고 |
| 세션 유실 | 계획·리서치·상태를 저장소 `docs/`에 보관 |

## 9. M2 — 인벤터식 설계 환경 위에서 작업을 정의하고 다수 UR을 동적으로 스케줄링

**목표(사용자 확정, 2026-11-17)**: 최신 Autodesk Inventor와 최대한 같은 설계 경험을 제공하되, 우리가 다루는 것은 UR이다. 설계
파일(설비 어셈블리)을 불러와 여러 UR을 마운트하고, 설비·환경의 특정 지점에 **작업**을 정의하면, **동적 스케줄러**가 상황에 맞춰
알맞은 UR에 작업을 할당해 시뮬레이션하고, 필요하면 URScript·RTDE로 실제 로봇(URSim 먼저)을 제어한다.

**원칙**
- **작업이 중심이다.** 웨이포인트·경로·IO 변경·대기는 작업을 정의하는 단계(부품)일 뿐이다. 사용자가 웨이포인트·경로를 UR에
  직접 주는 수동 모드는 작업을 만들고 확인하는 도구로 남긴다.
- **작업은 로봇이 아니라 구조에 붙는다.** 작업의 위치는 설비(불러온 어셈블리의 부품)나 스케치에 고정된 작업 좌표계로 정의하고,
  어느 UR이 할지는 정하지 않는다. 실행 시 스케줄러가 UR별 비용(IK 도달성, 계획한 동작 시간, 현재 위치, 대기, 다른 UR과의 간섭)을
  비교해 할당한다.
- **제어 메커니즘(PDDL 참고)**: 상태가 조건식을 만족할 때, 특정 구역에서 특정 행동을 하면, 상태가 특정 값으로 바뀐다. 행동을
  UR1·UR2가 할 때의 비용을 비교해 적합한 UR에 할당하고, 실패하면 복구 행동으로 해소한다.
- 근거 조사: `docs/research/m2-references.md`(CAD 불러오기, 필드 의미론, PDDL·MRTA·실행 감시, UR 인터페이스, 모델 트리 UX).

**확정된 결정**
| 항목 | 결정 |
|---|---|
| 단계 순서 | 설계 환경 → 필드 → 다수 UR → 작업 모델 → 스케줄러·시뮬레이션 → 실제 제어 |
| 기본 도형(상자·원기둥·구) | 리본에서 제거. 설비는 파일로만 불러온다. 간단한 장애물은 접근 제한 필드(스케치 돌출)로 그린다. 기존 문서의 기본 도형은 열 때 접근 제한 필드로 변환 |
| CAD 형식 | glTF/GLB(`gltf` 1.4, MIT/Apache: 어셈블리 계층·이름·색·인스턴싱 유지)를 주 경로로. STEP AP214는 `step-io`(MIT/Apache, 실험적)로 읽고 테셀레이션은 자체 구현(foxtrot·truck의 수학 참고, 스파이크로 검증). STL/OBJ 유지. Inventor `.ipt/.iam`은 비공개 형식이라 GLB/STEP 내보내기로 받는다 |
| 실제 제어 | URSim(도커, 테스트 전용·배포물 제외) 먼저. RTDE·Dashboard·스크립트 전송은 자체 구현(Universal_Robots_Client_Library Apache-2.0, ur_rtde MIT를 이식 참고). 실제 로봇은 명시적 확인·속도 제한을 거친다 |
| 금지 | `pddl`/`miniplan`(EUPL), `opencascade`(LGPL), python-urx(LGPL). FreeCAD·SolveSpace는 동작만 참고 |

### 9.1 M2-1 인벤터식 설계 환경
- **모델 트리(브라우저)**: 원점(평면·축·중심점) / 스케치 / 작업 피처(작업 평면·축·점·좌표계) / 피처(돌출) / 설비(불러온 어셈블리,
  부품 계층) / 로봇(마운트) / 필드 / 상태 / 작업. 동작: 뷰포트와 선택 동기화, 더블클릭 편집, F2 이름 바꾸기, 우클릭(스케치 편집·
  표시·억제·삭제와 종속 항목 안내), 소비된 스케치는 피처 아래에 접힘, 롤백 마커(파트 끝), 끌어서 순서 바꾸기(종속 검사), 오류 배지,
  불러온 설비는 고정(freeze) 참조.
- **스케치**: 임의의 평면·작업 평면·불러온 부품 면 위 2D 스케치(지금은 원점 평면·오프셋). 도구: 점, 선, 원(중심·3점), 호, 사각형
  (2점·3점·중심), 다각형, 슬롯, 모깎기, 트림·연장·간격띄우기, 형상 투영, 구성선. 구속·치수는 기존 솔버. 3D 스케치 유지.
- **피처**: 닫힌 스케치 프로파일의 **돌출**(거리·대칭·양방향) — M2에서는 필드 볼륨을 만든다(설계용 솔리드 모델링은 하지 않음).
- **입력 경험**: 마킹 메뉴(우클릭 드래그 방사형), 값 입력 미니 툴바, 명령 중 상태바 안내, 단축키 표(부록 B 확장).
- **CAD 불러오기**: GLB → 어셈블리 트리·이름·색·인스턴스. STEP 스파이크 후 결정. 큰 어셈블리는 작업 스레드에서 읽고 GPU 업로드를 나눠 한다.

### 9.2 M2-2 필드 (2D·3D)
- 모델: `Field { name, kind, profile(스케치의 닫힌 영역), extent(2D=평면 위 높이 범위, 3D=돌출 범위), params, active_when(상태식, M2-4) }`.
- 종류와 계획 의미: **접근 제한**(링크·툴 충돌 볼륨 또는 TCP 전용) / **감속**(TCP 속도 상한, 진입 전 미리 감속) / **가속**(구역 안에서
  관절 한계 내 상향) / **단방향**(TCP 이동 방향이 허용 원뿔 안) / **선호**(플래너 비용 가중).
- 시간 법칙: 경로를 따라 구간별 TCP 속도 상한을 TOPP-RA 제약 행으로 넣는다(지금의 고른 감속을 대체).
- UR 안전 설정으로 내보낼 수 있는 것(반공간 평면, Reduced 모드, 툴 방향)만 내보내고 나머지는 "계획 시에만 보장"으로 표시한다.
- 화면: 종류별 반투명 색, 화살표(단방향), 모델 트리 "필드" 그룹.

### 9.3 M2-3 다수 UR
- 여러 UR을 설비 부품·작업 좌표계에 마운트(마운트를 따라 움직임). UR마다 툴·TCP·IO 이름표.
- 충돌 검사에 로봇 간 충돌을 넣는다. 각 UR의 경로 계획 시 다른 UR은 현재(또는 시간에 따른) 자세의 장애물.

### 9.4 M2-4 작업 모델
- **상태 변수**: `State { name, type(bool·int·real·enum), initial }`. 조건식은 `gc_expr` 확장(비교·논리·상태 참조).
- **작업(행동)**: `Task { name, location(설비 부품·작업 좌표계·필드), precondition, steps, effects, robots(전체 또는 조건), priority,
  on_failure }`. 단계: `MoveJ(웨이포인트)`, `MoveL(웨이포인트)`, `FollowPath(경로)`, `SetIo(이름, 값)`, `WaitIo(조건, 시간 제한)`,
  `Gripper(열기·닫기)`, `Wait(초)`. 웨이포인트·경로는 작업 위치의 좌표계 기준(기존 기능)이라 어느 UR에서나 IK로 풀린다.
- **목표**: 목표 상태식 또는 작업 대기열. **복구**: 단계 실패·시간 초과 시 복구 작업 실행 또는 상태 설정 후 재할당.
- 형식 근거: PDDL2.1 수치·지속 행동(전제조건·효과·지속시간), 할당 분류는 ST-SR-TA(Gerkey–Matarić).
- 편집 화면: 모델 트리 "상태"·"작업" 그룹 + 속성 창 단계 목록(끌어서 순서). 노드 그래프 편집기는 이후.

### 9.5 M2-5 동적 스케줄러와 시뮬레이션
- 이산 사건 시뮬레이션: 실행 가능한 작업(전제조건 충족·자원 비어 있음)마다, 쉬는 UR별 **입찰 = 현재 자세에서 작업 시작점까지 계획한
  동작 시간 + 작업 동작 시간 + 대기**(계획 결과 캐시)를 계산해 순차 단일 항목 경매로 할당(CBBA는 참고).
- 공유 구역은 상호 배제 자원으로 먼저 처리하고, 이미 계획된 경로를 시간 조정(coordination diagram)으로 비키게 하며, 안 되면 우선순위
  재계획 → 복구·재경매.
- 실행 감독은 자체 행동 트리 실행기(BehaviorTree.CPP·PlanSys2의 구조 참고): 단계 진행, 실패 감지, 복구.
- 화면: 시간축(간트)·재생, UR별 상태, 처리량·가동률, 고장 주입으로 복구 시험.

### 9.6 M2-6 실제 제어 (URSim 먼저)
- 작업 단계를 URScript로 생성(movej/movel·블렌드, set_digital_out, 대기). Dashboard(29999)로 프로그램 제어, RTDE(30004)로 관절·IO
  상태 수신, 1차 클라이언트(30001/30002)로 스크립트 전송. External Control URCap으로 온라인 궤적 실행은 그다음.
- 안전: 실제 로봇은 명시적 "연결·무장" 단계, 속도 비율 제한, 드라이런(시뮬레이션과 RTDE 상태 비교) 후에만 실행.

### 9.7 담당과 검증
| 단계 | 백엔드 | 프론트엔드 | QA | 문서 |
|---|---|---|---|---|
| M2-1 | 문서 모델(피처·작업 피처·불러온 어셈블리), GLB/STEP 읽기, 스케치 도구 확장 | 모델 트리 동작, 마킹 메뉴, 스케치 도구 UI, 리본 정리 | 트리 편집 시나리오 | 인벤터 사용자용 가이드 |
| M2-2 | 필드 모델·계획 의미·TOPP-RA 속도 행 | 필드 그리기·표시 | 필드별 계획 테스트 | 필드 설명 |
| M2-3 | 다중 로봇 충돌·마운트 | 마운트 UI | 다중 로봇 시나리오 | — |
| M2-4 | 상태·식·작업 모델·저장 | 작업 편집 화면 | 작업 정의 시나리오 | 작업 작성법 |
| M2-5 | 스케줄러·시뮬레이터·행동 트리 | 간트·재생 | 스케줄링 회귀·결정성 | — |
| M2-6 | URScript·RTDE·Dashboard | 연결·모니터 화면 | URSim 통합 테스트 | 안전 안내 |
- 단계마다: 헤드리스 시나리오 테스트 + kittest 화면 테스트·스크린샷, `cargo xtask gate`, 사용자 PC 확인. 리드는 계약(`gc_robotics_api`,
  문서 스키마)을 먼저 정하고 역할 세션에 배정한다.

## 부록 A. 의존성과 검증된 API 사실 (2026-11-16)
- **`=` 고정**: eframe / egui / egui-wgpu / egui_kittest 0.37.0 · wgpu 30.0.2 · egui_tiles 0.18.0 · egui_ltreeview 0.10.0 · glamx 0.3.1(`default-features = false`, `std, f64, libm, scalar-math`; glam 0.33, **0.34 금지**) · parry3d-f64 0.31.1(`enhanced-determinism`; `simd8`·`parallel` 금지).
- **캐럿 + lock**: egui-notify 0.24 · rfd 0.18 · roxmltree 0.21 · mesh-loader 0.1.13(`default-features = false`, `stl, obj`) · smallvec 1 · libm 0.2 · bytemuck 1 · pollster 1 · image 0.25(png) · serde 1 · serde_json 1(`float_roundtrip`) · thiserror 2 · anyhow 1 · log 0.4 · tempfile 3 · rayon 1(ACM·시드 스윕 전용 풀) · proptest 1(dev) · insta 1(dev). xtask 전용: yaml 리더는 자체 구현.
- **라이선스 허용**: MIT, Apache-2.0(+LLVM 예외), BSD-2/3, ISC, Zlib, BSL-1.0, CC0-1.0, Unicode-3.0, OFL-1.1, Ubuntu-font-1.0. **금지 크레이트**: evalexpr(AGPL), slvs(GPL), brepkit-sketch(AGPL), `directories`(MPL 전이 의존 → 자체 경로 조회). **아직 egui 0.36 전용(사용 금지)**: egui_dock, egui-snarl, transform-gizmo-egui, egui-phosphor.
- **eframe 0.37**: `App`의 필수 메서드는 `fn ui(&mut self, ui: &mut egui::Ui, frame: &mut Frame)`뿐(`update` 없음). `Panel::{top,bottom,left,right}(id).show(ui, ..)`, `CentralPanel::no_frame()`, `MenuBar::new().ui(..)`, `Id::unique`, `Modal`, `Popup`, `Scene`. `SidePanel`/`TopBottomPanel`/`raw_scroll_delta` 없음, `Id::new` deprecated. `cc.wgpu_render_state: Option<_>`. `Event::Key { key, physical_key: Option<Key>, pressed, repeat, modifiers }`.
- **egui-wgpu**: `Renderer::register_native_texture(&device, &view, FilterMode)`(반드시 `Rgba8Unorm`), `update_egui_texture_from_wgpu_texture`.
- **wgpu 30**: `InstanceDescriptor::new_without_display_handle_from_env()`, `VertexState::buffers: &[Option<_>]`, 정수 varying `@interpolate(flat)` 필수, `get_mapped_range()`가 `Result`, `PollType::Wait{submission_index, timeout}`, `depth_slice`/`multiview_mask`/`cache`, `depth_write_enabled: Option<bool>`.
- **egui_kittest**: `Harness::builder().with_size(..).wgpu().build_eframe(..)`(`.wgpu()`는 어댑터가 없으면 패닉 → 먼저 탐지), `run()`은 즉시 repaint가 멈출 때까지(기본 `max_steps` 4, `step_dt` 0.25 s), `run_steps(n)`, `get_by_label`, `render()`, 스냅샷 옵션은 `max_failed_pixels`. 그릴 수 없는 글리프·이름 없는 입력 위젯은 실패.
- **egui_tiles 0.18**: `Tree::new(id, root, tiles)`, `Behavior::{pane_ui, tab_title_for_pane}`, `is_tab_closable` 기본 false.
- **glamx 0.3.1**: `DRot3 = glam::DQuat`, `DPose3 { rotation, translation }` — `from_parts`, `new(translation, axisangle)`, `inverse`, `inv_mul`, `transform_point`, `to_mat4`, `Pose * Pose`.
- **parry3d-f64 0.31.1**: `math::Pose = DPose3`, `SharedShape::{ball, cuboid, cylinder(half_height, radius), convex_hull -> Option, trimesh_with_flags}`, **Cylinder는 로컬 Y축**, 질의는 `Result<_, Unsupported>`: `intersection_test -> ShapeIntersection{intersecting,..}`, `distance -> ShapeDistance{distance,..}`, `closest_points(.., max_dist) -> ClosestPoints`. `partitioning::Bvh::{from_leaves, intersect_aabb, refit}`.
- **폰트**: Jetendard = JetBrainsMono Nerd Font Mono + Pretendard(1.15배, 라틴 2칸 폭). OFL 1.1, Reserved Font Name "Jetendard" → **수정·서브셋 금지**. 빌드본은 릴리스 자산에만: `https://github.com/kuskhan/jetendard/releases/download/v0.1.0/Jetendard-TTF.zip`(35.7 MB, 16종, 다운로드 가능 확인). `Jetendard-Regular.ttf`·`Jetendard-Bold.ttf`만 커밋. 리거처 없는 변형이 없으므로 UI에서 거슬리면 egui에서 리거처를 끄는 방법을 Stage 0에서 확인.
- **환경**: Rust 1.97.0, 4코어, 디스크 약 30 GB, GPU 없음. Vulkan 로더 + Chromium SwiftShader ICD(Vulkan 1.3), libEGL 없음, Xvfb 있음. crates.io·raw.githubusercontent.com·GitHub 릴리스 자산·static.rust-lang.org 접근 가능, github.com HTML·docs.rs 차단.

## 부록 B. 내비게이션 프리셋 · 키맵
| 동작 | Inventor 프리셋(기본) | SolidWorks 프리셋(P2) |
|---|---|---|
| 팬 | 가운데 드래그 / F2 + 좌드래그 | Ctrl + 가운데 드래그 |
| 줌 | 휠(사용자 쪽으로 굴리면 확대, 반전 옵션) / F3 + 좌드래그 | 휠 / Shift + 가운데 드래그 |
| 오빗 | Shift + 가운데 드래그 / F4 + 좌드래그 | 가운데 드래그 / 방향키 15° |
| 전체 보기 | 가운데 더블클릭 / Home | 가운데 더블클릭 / F |
| 선택 항목 줌 / 홈 / 이전·다음 뷰 / 정면 보기 | End / F6 / F5·Shift+F5 / PageUp | — / Ctrl+7 / 뷰포트에서 Ctrl+Shift+Z / Ctrl+8, Ctrl+1~6 |
- 전역: 실행 취소 Ctrl+Z, 다시 실행 Ctrl+Y, 저장 Ctrl+S, 열기 Ctrl+O, 새로 만들기 Ctrl+N, Delete, Esc. 스케치: L 선, C 원, A 호, D 치수, S 스케치 시작/마무리, F8/F9 구속 표시. Home/End/PageUp은 텍스트 필드 포커스 중에는 편집 키.
- 표준 뷰(Z-up): Front = +Y를 바라봄(up +Z), Right = −X, Top = −Z(up +Y), 등각 = (−1, +1, −1) 방향.
- 스케치 색: 미구속 `#0000FF`, 완전구속 `#000000`, 과구속/미해결 `#FF0000`, 무효 `#FFD400`, 참조 치수 `#808080`, 호버 `#FF8000`, 선택 `#3FA9F5`. 배경 `#E6EAEE → #C3CAD3`.

## 부록 C. 솔버·스케치 세부 사양과 수용 테스트
- **허용오차(스케일 프레임, S = 1)**: S = max(성분 점들의 경계 상자 대각선, 최대 길이 치수, 1e-3 m). 잔차 1e-10 · 정류 `‖Jᵀr‖ ≤ 1e-9‖J‖_F‖r‖` 또는 수락 스텝 < 1e-13 · 랭크 `|R_kk| < 1e-8|R_11|`(피벗 비 1e-10..1e-6은 `ill_conditioned`에 기재) · combo 충돌 1e-7 · 영기울기 1e-12 · 퇴화 길이 1e-9 · 이동성 1e-6 · LM: λ₀ 1e-6, 수락 ×0.1 / 거절 ×10, 단계당 50회 · 섭동 크기 1e-3.
- **고정 스케일**: 평행·직각·대칭은 `max(|d1|₀, |d2|₀)`로 나눔, 각도 잔차는 `min(|d1|₀, |d2|₀)·atan2(..)`. 각도 절단: 오차가 ±π에서 1e-12 이내면 +π, 각도 잔차를 π/2 넘게 바꾸는 스텝은 반분.
- **치수 저장 데이터**: `Distance{p, q}` · `HDistance/VDistance{p, q, sign}` · `PointLineDistance{p, line, side}` · `LineLineDistance{l1, l2, side}`(1행) · `Angle{l1, rev1, l2, rev2}`(반시계, 생성 시 (0, π]) · `Radius/Diameter` · (3D) `Distance3`, `PointPlaneDistance{p, FramePlane, side}`. 구속: `TangentLineCircle{side}`, `TangentCircleCircle{External | Internal}`, `EndTangent{smooth}`.
- **스냅 표**(조건 → 암시 구속): 점·끝점 8 px → 일치 · 중심 8 px → 일치 · 선 중점 → 중점 · 두 곡선 교차 → 곡선 위 ×2 · 사분점 → 원 위 · 접선 → 곡선 위 + 접선(쪽 저장) · 기준선에 직각/평행 → 해당 구속 · 곡선 위 6 px → 선 위/원 위 · 그리는 선분이 수평/수직(커서에서 4 px) → 수평/수직 · 다른 점과 정렬 → 위치만 · 그리드 → 없음. 현재 스냅은 허용오차의 1.5배를 벗어날 때까지 유지.
- **2D 커널 F1–F20**: F1 H+H+평행 → 평행 중복 · F2 H 중복 · F3 삼각형 부등식 위반 → 충돌 집합 지목, 상태 불변 · F4 일직선 시작 3-4-5 → 섭동 후 해결(F4b: 고정점 없이도) · F5 양 끝에 일치한 점 → degenerate · F6 고정+고정+거리 · F7 수평 + 수직거리 5 → 충돌 · F8 직각 4개 → 중복 1, DOF 5 · F9/F10/F13 접선 쪽·스윕·반지름 부호 유지(호 끝점을 170° → 190°로 끄는 것은 허용) · F11 각도 0 / 1e-6 / 90 / 179.999999 / 180 / 270° 풀랭크 · F12 원 중심의 점 → NaN 없음 · F14 4절 링크 DOF 4/1 · F15 특이 배치 `AddedSingular` · F16 드래그 경로 독립(4/64/512 샘플 순회 후 회전 < 1e-9 rad), F16b 한 끝이 고정된 선을 몸통으로 잡으면 회전 · F17 완전구속은 드래그해도 비트 불변 · F18 ×1e-3 / ×1e3 스케일·(1e6, 1e6) 이동 불변 · F19 짧은 선 평행 ≤ 1e-6 rad · F20 호 + 반지름 = 1행.
- **스케치 계층 G1–G13**: G1 사각형 도구 DOF 4 → 모서리를 원점에 일치 2 → 폭·높이 0, 대각선 거리는 Redundant · G2 거리 10 + 수평, 수평거리 7 후보 → Conflicting(주체 = 후보) · G3 선–접선 호–선 DOF 5, 반지름 추가 4, 첨점 없음 · G4 중점 후 선 위 → Redundant · G5 수평 두 선 + 동일선상 → Added, 한 행 PartiallyRedundant · G6 동심 등반지름 원 + 접선 → degenerate · G7 재솔브 비트 동일 · G8 (3D) 고정 O, 자유 P: DOF 3 → 거리 2 → 평면 위 1 / 대신 X 방향이면 0 · G9–G11 (3D P2 구속용, 구현 시 활성화) · G12 폭 10 → 0.001, 1000 뒤집힘 없음, `-5`는 인라인 오류 · G13 같은 스케치의 참조 치수를 참조하는 구동 치수 → 순환 오류 · 추가: 평행한 두 수평선 사이 폭 치수는 Added, 3D 점이 외부 참조(2D 스케치 점)를 따라 같은 Txn에서 이동.
- **UI/도구 시나리오 S1–S9**: S1 L, 원점 클릭, 오른쪽 클릭 → 일치 + 수평 저장(Ctrl이면 둘 다 없음) · S2 D, 선, 배치, `25 mm`, Enter → 도구 유지, "치수 필요" −1 · S3 과구속 치수 → 프롬프트(수락 = 괄호 참조 치수, 취소 = 문서 불변) · S4 30프레임 드래그 → undo 1단계, Esc는 비트 복원, 경로 "오래됨"은 EndDrag 전에 안 바뀜 · S5 완전구속 드래그 → 히스토리·리비전 불변 · S6 삼각형을 불가능하게 만드는 fx 수정 → 인라인 오류, 문서 불변 · S7 3D 축 잠금 클릭, Tab 순환, 평면 위 1e-12, 누운 평면은 거부 · S8 웨이포인트가 치수 수정을 따라가고 경로가 오래됨, 점 삭제 시 웨이포인트는 분리(삭제 안 됨) · S9 H 글리프·모서리 일치 글리프 Delete → DOF +1 / +2.

## 부록 D. UR 데이터와 기대값 (공식 ROS2 Description, 고정 커밋 기준)
- 표준 DH `A_i = Rz(θ)·Tz(d)·Tx(a)·Rx(α)`: a = [0, a2, a3, 0, 0, 0], d = [d1, 0, 0, d4, d5, d6], α = [π/2, 0, 0, π/2, −π/2, 0].

| 모델 | d1 | a2 | a3 | d4 | d5 | d6 | 속도 한계(°/s) |
|---|---|---|---|---|---|---|---|
| UR3e | 0.15185 | −0.24355 | −0.2132 | 0.13105 | 0.08535 | 0.0921 | 180×3, 360×3 (손목3 continuous) |
| UR5e | 0.1625 | −0.425 | −0.3922 | 0.1333 | 0.0997 | 0.0996 | 180×6 |
| UR10e | 0.1807 | −0.6127 | −0.57155 | 0.17415 | 0.11985 | 0.11655 | 120, 120, 180×4 |
| UR16e | 0.1807 | −0.4784 | −0.36 | 0.17415 | 0.11985 | 0.11655 | 120, 120, 180×4 |

- **프레임**: `T(base → tool0)_URDF(q) = A_1···A_6`(오프셋 0, 부호 동일). `base_link = Rz(π)·base`. `tool0` ≡ `wrist_3_link` ≡ DH 프레임 6(Z가 플랜지 밖), `flange`는 X가 밖. 한계: 공식 파일에서 팔꿈치 ±π, 나머지 ±2π. 가속도 한계 없음. 허용오차 1e-9(YAML에 변환 잔재).
- **UR5e 조인트 원점**(xyz / rpy, 축 0 0 1): shoulder_pan (0 0 0.1625 / 0 0 0) · shoulder_lift (0 0 0 / π/2 0 0) · elbow (−0.425 0 0 / 0 0 0) · wrist_1 (−0.3922 0 0.1333 / 0 0 0) · wrist_2 (0 −0.0997 ~0 / π/2 0 0) · wrist_3 (0 0.0996 ~0 / π/2 π π). 고정: base_link→base_link_inertia (0 0 π), base_link→base (0 0 π), wrist_3→flange (0 −π/2 −π/2), flange→tool0 (π/2 0 π/2).
- **FK 기대값**(UR5e, `base → tool0`): q = 0 → p (−0.8172, −0.2329, 0.0628), R 행 (1,0,0), (0,0,−1), (0,1,0) · q = (0, −π/2, 0, −π/2, 0, 0) → p (0, −0.2329, 1.0794), R 행 (−1,0,0), (0,0,−1), (0,−1,0) · q = (0, −π/2, π/2, −π/2, −π/2, 0) → p (−0.4919, −0.1333, 0.4879), R 행 (0,1,0), (1,0,0), (0,0,−1)(툴 Z 아래). q = 0은 팔꿈치·손목 모두 특이점이라 IK 픽스처로 쓰지 않는다.
- **IK 기대값**: 세 번째 자세를 목표로 하면 정확히 8개 정규 해와 8개 서로 다른 분기 라벨. 일반 자세는 필터 전 후보 256개(8분기 × 2⁵ 턴, 팔꿈치는 ±π). 나머지 기대값은 `docs/research/review-robotics.md` 12번.
- **Hawkins 주의**: 원본 `forward()`는 `Rz(π)·T_DH·M`(레거시 ee_link) 기준이라 그대로 쓰면 틀린다. q5의 `acos`에 범위 검사가 없다.
- **타이밍 기대값**: UR5e(v = π, a = 5) Δq1 = 1 rad → 0.894427 s, Δq1 = π → 1.628319 s · URScript 프로파일(v = 1.05, a = 1.4) Δ = π/2 → 2.245997 s, Δ = 0.5 → 1.195229 s · UR10e(a = 5) Δq = (1,0,2,0,0,0) → 1.264938 s, (2,0,1,0,0,0) → 1.373809 s · TOPP-RA는 같은 경로에서 N = 200일 때 0.5% 이내.
- **메시·라이선스**: UR5e 충돌 STL 합계 약 378 kB(7,558 삼각형). UR16e는 5개 부품이 UR10e 메시 공유. BSD-3는 e-Series까지. 상위 LICENSE 원문을 그대로 커밋하고 고지에 "Universal Robots A/S and contributors, Universal_Robots_ROS2_Description, BSD-3-Clause" 표기.

## 부록 E. 계획 품질 기준 (release, 단일 스레드 플래너, 시드 0..99, 상태 검사 수로 판정 — 시간은 보고만)
장면(`fixtures/scenes/`, Stage 1에서 치수·포즈·질의를 확정): S1 빈 공간 · S2 테이블 + 펜스 · S3 기계 문(400 × 400 mm 개구부, TCP 150 mm 진입) · S4 슬롯(툴 폭 + 2·패딩 + 30 mm), 그리고 구 모드용 대응 장면.

| # | 기준 | 목표 |
|---|---|---|
| Q1 | 유효성 | 반환된 모든 경로가 독립 재검증에서 위반 0(속성 테스트, 면제 불가) |
| Q2 | 보증 | 5.7의 보증 문구와 `Guarantee`가 결과에 붙는다 |
| Q3 | 성공률 | S1 100%, S2 ≥ 99%(2만 상태 검사 내) · S3 ≥ 95%, S4 ≥ 80%(20만 내) |
| Q4 | 경로 길이 | 커밋된 기준 비용(`bench-quality --reference`: Informed RRT*, 200만 검사 × 8 시드, 후처리, 최솟값) 대비 중앙값 ≤ 1.20배(S2, S3). S1은 직선과 일치. 기준을 2% 넘게 이기는 시드가 있으면 기준 갱신 필요로 실패 |
| Q5 | 일관성 | best-of-4 후 변동계수 ≤ 10%(S1, S2), ≤ 20%(S3, S4) |
| Q6 | 매끄러움 | 스무딩 후 설정 각도 이상의 내부 코너 없음, 내부 정점에서 정지 없이 타이밍 가능 |
| Q7 | 여유 | 최소 여유 거리 보고 |
| Q8 | 지연 | 계획 + 단순화 시간을 release에서 측정해 보고(게이트 아님) |
| Q9 | 결정성 | 같은 요청·시드·예산·빌드 → 같은 플랫폼에서 비트 동일 |
| Q10 | 설명 가능성 | 실패가 원인(링크 쌍/장애물, 조인트와 한계, 분기, 예산 소진과 통계)을 지목 |
| Q11 | 안정성 | S2/S3에서 웨이포인트를 1 mm 옮겨도 목표 분기·턴 유지, 비용 변화 ≤ 5%(10회) |
- 범용 URDF 픽스처 3종은 Q1, Q2, Q9, Q10을 만족하고 Q3/Q5를 보고한다(임계값은 UR5e 전용).
- 임계값·픽스처 변경은 오케스트레이터만, `DEVIATIONS.md`에 근거와 함께.

## 부록 F. 규범 문서 — Stage 0에서 `docs/research/`로 복사
세션 경로 `/root/.claude/projects/-home-user-GraphCTRL/01a2db8f-d059-557f-8af5-8c0778f1f0ce/` 아래 원문을 저장소로 옮긴다(이후 구현은 저장소 경로만 참조).

| 저장소 파일 | 원본 | 내용 |
|---|---|---|
| `docs/PLAN.md` | 이 계획서 | — |
| `egui-wgpu.md` | `tool-results/bm8miguoz.txt` | egui/wgpu API, 골격 코드 |
| `architecture.md` | `tool-results/br9p5r4t0.txt` | Txn 코드 스케치, 크레이트 선택 |
| `solver.md` | `tool-results/bknk32u07.txt` | 솔버 리서치 |
| `robot-model.md` | `tool-results/bla8svt8f.txt` | URDF·FK·IK, UR5e 전개 표 |
| `planning.md` | `tool-results/b9811oomf.txt` | parry API, 플래너, RRT-Connect 코드 |
| `solver-review.md`, `robotics-domain.md` | `subagents/workflows/wf_785bb0fc-76b/journal.jsonl`의 result 2건 | 솔버 반박 리뷰, 로보틱스 도메인 |
| `review-architecture.md`, `review-delivery.md` | `subagents/workflows/wf_56667075-6c9/journal.jsonl` | 계획 리뷰(레이어·API 계약·병렬 규약 / 게이트·검증) |
| `review-sketch-solver.md`, `review-robotics.md` | `subagents/workflows/wf_26a291f9-11e/journal.jsonl` | 계획 리뷰(드래그 알고리즘·트림 규칙·픽스처 / 목표 집합·툴·보증·기대값) |
| `frontend-decision.md` | `/tmp/claude-0/-home-user-GraphCTRL/01a2db8f-d059-557f-8af5-8c0778f1f0ce/tasks/a553c0c8796a22050.output`의 최종 결과 | Leptos·Dioxus·Yew·Tauri·egui 비교(3D 성능, 유지보수 이력, 혼합 구조 평가)와 재검토 조건 |
