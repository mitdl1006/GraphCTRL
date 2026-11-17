# 진행 상태

계획: [PLAN.md](PLAN.md) · 근거 문서: [research/](research/) · 단계별 증거: [evidence/](evidence/)

| 단계 | 상태 | 비고 |
|---|---|---|
| 0. 골격·환경 | 완료(한글 입력 시험만 사용자 확인 대기) | 증거: [evidence/stage-0.md](evidence/stage-0.md). Windows에서 빌드·실행 확인됨(`-j`로 동시 작업 수 제한). `gen-ur`는 Stage 2b, `bench-quality`는 Stage 3a에서 추가 |
| 1. 기반 + 계약 동결 | 완료 | foundation 7개 + api 2개. CI: Windows 빌드·테스트 통과 확인 |
| 2a / 2b 헤드리스 도메인 | 진행 중 | `gc_solver`, `gc_sketch`, `gc_doc`, `gc_tools`, `gc_session`(스케치 범위) 완료. 화면까지 연결됨. 로봇·경로 계획 헤드리스 쪽(`gc_robot`, `gc_plan`, `gc_collide`, `gc_traj`, `gc_motion`) 완료. 다음: 플러그인(플래너·메시·UR), 문서·세션에 로봇/경로 엔티티, 화면 연결 |
| 3a / 3b 통합 도메인 | 대기 | |
| 4. UI 키트·뷰포트 | UI 체크포인트 도달 — 사용자 확인 대기 | 증거: [evidence/stage-4.md](evidence/stage-4.md) |
| 5a–5f 앱 조립 | 진행 중 | 5c 첫 조각: 로봇 탭(샘플 로봇·바디·웨이포인트·경로·계획·재생)이 화면에서 동작 |
| 6. 하드닝 | 대기 | |

## 작업 순서

혼자 순차로 구현하므로, 눈에 보이는 기능이 먼저 나오도록 스케치 쪽을 먼저 세로로 완성한다:
`gc_solver → gc_sketch → gc_doc → gc_tools → gc_session → gc_draw → gc_render → gc_ui → gc_viewport → 앱(셸 + 2D 스케치)`.
그다음 로봇·경로 계획 쪽(`gc_robot → gc_collide → gc_plan → gc_traj → gc_motion → 플러그인`)을 붙인다.

## 크레이트별 상태

| 크레이트 | 상태 | 테스트 |
|---|---|---|
| gc_core | 완료 | 10 |
| gc_linalg | 완료 | 19 (QR 랭크 == SVD 랭크, Kahan 행렬 포함) |
| gc_math | 완료 | 16 |
| gc_expr | 완료 | 16 |
| gc_i18n | 완료(메시지는 UI 작업과 함께 추가) | 5 |
| gc_jobs | 완료 | 8 |
| gc_mesh | 완료 + 기본 도형(상자·원기둥·구), STL 리더(바이너리·텍스트) | 17 |
| gc_plugin_api | 완료 | 8 |
| gc_robotics_api | 완료 | 10 (API만으로 작성한 모의 플러그인 포함) |
| gc_solver | 완료(드래그 속도는 KNOWN_ISSUES 참고) | 35 (F1–F20, 드래그, 3D 포함) |
| gc_sketch | 완료(스냅·추론은 gc_tools에서) | 11 (G1–G3, G8, G12, 삭제 연쇄, 드래그) |
| gc_doc | 완료(스케치 + 로봇 셀: 바디·로봇·웨이포인트·경로 엔티티와 명령) | 11 |
| gc_tools | 완료(2D 도구·스냅; 트림·3D 배치는 이후) | 3 |
| gc_session | 스케치 범위 + 로봇 셀(로봇·메시 로드, 설비 STL 불러오기, 조그, 웨이포인트, 작업 스레드 계획·취소·오래됨 판정, 재생) | 13 |
| gc_draw | 카메라, 내비게이션 프리셋, 클립 행렬, ViewCube 기하(26방향 피킹) | 13 |
| gc_ui / gc_viewport / gc_panels / gc_app | 셸 + 2D 스케치 캔버스 | 4 + kittest 2 |
| gc_persist | 완료(`.gctrl` v1, 원자적 저장, `.bak`; 자동 저장·복구는 이후) | 4 (save∘load = id, 결정적 출력, 손상·상위 버전 거부) |
| gc_robot | URDF 리더·검증, 체인, FK, 자코비안, 수치 IK 완료(PlanningProfile·해석 IK 연결은 이후) | 8 (UR5e 공개 자세 FK, API 기준 FK 1e-12, 자코비안, IK, 오류 메시지) |
| gc_plan | 완료(RRT-Connect·RRT·RRT*·Informed RRT*·PRM, 숏컷·코너 절단·조밀화, 독립 재검증) | 12 (유효성, 시드 재현성, 최적성, 실패 사유, 다중 목표, 취소) |
| gc_collide | 완료(구·상자·원기둥·볼록 껍질·메시, 패딩 쌍 정책, 허용 쌍; BVH 가속·ACM 자동 생성은 이후) | 6 |
| gc_traj | 1차 완료(세그먼트별 동기 사다리꼴 — 웨이포인트마다 정지, 검증 타입스테이트. 블렌드·TOPP-RA는 이후, DEVIATIONS 참조) | 5 (닫힌 형태 소요 시간 6건 포함) |
| gc_motion | 1차 완료(로봇 장면 구성, 목표 해석(관절/자세 IK), 직선 우선 → 플래너 best-of-N → 숏컷 → 독립 재검증 → 타이밍 → 궤적 검증, 원인을 지목하는 실패) | 6 (비-UR 6축 + 기둥 장애물, 플래너 5종, 시드 재현성) |
| gc_panels | 브라우저(원점·스케치·바디·로봇·웨이포인트·경로), 속성, 조그, 바디 배치, 경로 계획·재생, 상태바 | (gc_app 테스트) |
| gc_plugin_ur | UR5e 제공(URDF + 충돌 STL, `RobotModelProvider`로 레지스트리 등록). UR3e/10e/16e, `gen-ur`, 해석적 IK는 이후 | 1 |
| gc_viewport | 내비게이션, 스케치 입력, 그리드(솔리드 아래) → GPU 솔리드 → 스케치·궤적·웨이포인트(위), ViewCube(클릭) | (gc_app 테스트) |
| gc_scene | 세션 → 솔리드·궤적·웨이포인트 | 1 |
| gc_render | 오프스크린 타깃 + 인스턴스 메시 렌더러(자산별 1회 업로드, 메시별 배치, reverse-Z, 반투명). 선 파이프라인·ViewCube는 이후 | 2 (GPU) |
| 그 외 | 스텁 | |
