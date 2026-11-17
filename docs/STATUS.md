# 진행 상태

계획: [PLAN.md](PLAN.md) · 근거 문서: [research/](research/) · 단계별 증거: [evidence/](evidence/)

| 단계 | 상태 | 비고 |
|---|---|---|
| 0. 골격·환경 | 완료(한글 입력 시험만 사용자 확인 대기) | 증거: [evidence/stage-0.md](evidence/stage-0.md). Windows에서 빌드·실행 확인됨(`-j`로 동시 작업 수 제한). `gen-ur`는 Stage 2b, `bench-quality`는 Stage 3a에서 추가 |
| 1. 기반 + 계약 동결 | 완료 | foundation 7개 + api 2개. CI: Windows 빌드·테스트 통과 확인 |
| 2a / 2b 헤드리스 도메인 | 진행 중 | `gc_solver`, `gc_sketch` 완료. 다음: `gc_doc` |
| 3a / 3b 통합 도메인 | 대기 | |
| 4. UI 키트·뷰포트 | 대기 | |
| 5a–5f 앱 조립 | 대기 | |
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
| gc_mesh | 완료 | 14 |
| gc_plugin_api | 완료 | 8 |
| gc_robotics_api | 완료 | 10 (API만으로 작성한 모의 플러그인 포함) |
| gc_solver | 완료(드래그 속도는 KNOWN_ISSUES 참고) | 35 (F1–F20, 드래그, 3D 포함) |
| gc_sketch | 완료(스냅·추론은 gc_tools에서) | 11 (G1–G3, G8, G12, 삭제 연쇄, 드래그) |
| 그 외 | 스텁 | |
