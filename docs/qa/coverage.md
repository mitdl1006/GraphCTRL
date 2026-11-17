# 시나리오·체크리스트 대 테스트 대조 (QA)

기준: PLAN.md 부록 C(S1–S9), 5.11(앱 배관). 갱신: 이 문서를 만든 QA 세션.

## 부록 C — UI/도구 시나리오

| 시나리오 | 테스트 | 상태 |
|---|---|---|
| S1 선 + 원점 → 일치·수평, Ctrl이면 없음 | `gc_session/tests/scenarios.rs` s1_* (2개) | 있음 |
| S2 치수 입력 후 도구 유지 | s2_* | 있음 |
| S3 과구속 → 참조 치수 | s3_* | 있음 |
| S4 드래그 = undo 1단계, Esc 복원 | s4_* (경로 "오래됨"이 EndDrag 전에 안 바뀜은 미검증) | 일부 |
| S5 완전구속 드래그 불변 | s5_* | 있음 |
| S6 잘못된 수식 → 인라인 오류, 문서 불변 | s6_* | 있음 |
| S7 3D 축 잠금·Tab·평면 위 | `a_3d_polyline_...`(축 잠금·평면 구속), `qa_scenarios.rs::s7_tab_cycles_the_working_plane`(추가) | 있음. "누운 평면 거부"는 잠금 무시 경로로만 확인 |
| S8 웨이포인트가 치수를 따라감·오래됨 / 점 삭제 시 분리 | `cell.rs::a_waypoint_on_a_sketch_point_follows_the_sketch`, `qa_scenarios.rs::s8_deleting_a_sketch_point_keeps_the_waypoint`(추가) | 있음 |
| S9 글리프 Delete → DOF +1/+2 | `qa_scenarios.rs::s9_deleting_constraint_glyphs_frees_degrees_of_freedom`(추가) | 있음 (헤드리스. 화면 글리프 클릭은 kittest 없음) |

## 5.11 앱 배관

| # | 항목 | 대조 | 상태 |
|---|---|---|---|
| 1 | 경로 조회, 도움말 > 정보 | 코드 `settings_dir`, `gc_app` 정보 창 테스트 있음. Windows `%APPDATA%` 분기는 Linux에서 실행 불가(CI Windows에서 간접) | 일부. `logs/` 폴더는 없음 |
| 2 | `FileDialogs` 트레잇 / 테스트 대역 | shell.rs `Scripted` | 있음 |
| 3 | 더티 추적, 새로 만들기 모달 | shell.rs 1408~ | 있음(닫기·열기 시 모달은 미확인) |
| 4 | 오류 정책(토스트/모달/패널/패닉 복구 저장) | 패닉 훅·네이티브 메시지 코드 없음 | **빠짐**(제품 코드) |
| 5 | 로그: stderr + 파일 + 패널 링 버퍼 | 로깅 코드 없음 | **빠짐**(제품 코드) |
| 6 | `windows_subsystem`, 창 아이콘 | `main.rs`에 `windows_subsystem` 있음, 창 아이콘 없음 | 아이콘 **빠짐**(제품 코드) |
| 7 | 도움말 > 라이선스 | shell.rs | 있음 |
| 8 | 예제 열기 | `the_example_cell_opens_planned` | 있음 |
| 9 | 자동 저장·복구, 정상 종료 표시 | shell.rs 1301, `gc_persist/tests/qa_edges.rs`(추가: 왕복·삭제·한글 이름) | 있음 |

## 최근 기능 경계 사례(추가한 테스트)

- `gc_persist/tests/qa_edges.rs`: 잘린 `.gctrl`(모든 접두 길이)·쓰레기 입력 거부, 없는 파일·폴더, 폴더 위에 저장, 최근 목록(중복·상한·삭제된 파일), 손상된 설정, 복구 파일.
- `gc_plugin_ur/tests/qa_ik_edges.rs`: 특이 자세(손목·어깨·각도 감김)의 해 유효성, NaN/무한/영 쿼터니언 대상, 도달 한계. 버그 1건(아래).
- `gc_session/tests/qa_cell_edges.rs`: TCP 속도 한계 NaN/무한/0, 아주 작은 한계, 길이 0 직선 이동, 손상된 툴·배치.
- 미검증 후보: 좌표계 기준 웨이포인트 + 좌표계가 퇴화(축 점 일치)할 때, 여러 번 저장·열기에서 메시 경로 이동.

## CI

`.github/workflows/ci.yml`: Linux 게이트(`cargo xtask gate`, 교차 clippy 포함)와 `windows-2025`의 빌드·테스트(`--locked`)를 도는 것을 확인(정적 확인; 이 샌드박스에서는 실행 결과를 볼 수 없음).
