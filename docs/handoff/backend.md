# 백엔드 인계 (최종 갱신 2026-11-18)

브랜치 `claude/ur-robot-control-simulator-jsmhze-backend`. 아래 "현황"이 요약, 그 다음 "작업 기록"이 최신 순 상세(근거 링크·API·차이).
인벤터 원문은 `https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-….htm`.

## 현황 — 완료

| 영역 | 무엇 | 세션/문서 API(대표) | 테스트 |
|---|---|---|---|
| 스케치 | Trim·Extend·Fillet, Project Geometry(연관), 치수 이름·매개변수(d0…, `NAME = 식`) | `SketchOp::{Trim, Extend, Fillet, Project, RenameDimension}`, `Command::RenameParam` | `gc_doc/tests/sketch_ops.rs`, `gc_session/tests/project_geometry.rs` |
| 스케치 도형·패턴 | 점(중심점 토글)·다각형·직선 슬롯 3종·간격띄우기 도구, 미러·직사각형·원형 패턴 대화상자, 리본 작성·패턴·수정 패널 | `SketchOp::{PlacePoint, AddPolygon, AddSlot, Mirror, RectangularPattern, CircularPattern, Offset}`, `ToolKind::{Point, Polygon, Slot, Offset}`, `edit_active_sketch(op)` | `gc_doc/tests/sketch_shapes.rs`, `gc_session/tests/sketch_shape_tools.rs`, 앱 `sketch_shape_tools.rs` |
| 컴포넌트 트리 | 접지·가시·활성·억제, 원점 폴더, 로봇 링크·툴을 하위 컴포넌트로 | `components()`, `robot_links(robot)`(링크 + `Occurrence::Tool`), `component_origin(occ)`, `set_component` | `robot_links.rs`, `qa_origin_folder.rs` |
| 구속·조인트 | Mate·Flush·Insert·Angle·Tangent·Symmetry, Rigid·Rotational·Slider·Cylindrical·Ball, 한계, Predict, Assemble 유형 제한 | `add_relationship`, `predict_relationship`, `automatic_joint`, `possible_relations(a, b)` | `gc_doc/tests/relate.rs` |
| 상태·DOF | 상태 점, 남은 자유도, DOF 기호(이동 방향·회전축·큐브), 충돌 관계 진단 | `component_statuses()`, `component_dof`, `dof_symbol(s)`, `conflicting_relationships` | `relate.rs`, `dof_symbol.rs` |
| 구동·접촉 | Drive(값 재생), 접촉 솔버(밀기·정지, 드래그 포함), Drive 충돌 감지 | `drive_relationship`, `contact_stop()`, `drive_collision()`, `set_contact_set/solver` | `contact.rs`, 앱 `shell.rs` |
| 배치 명령 | Ground and Root, Place at Component Origin, 패턴(직사각형·원형), 미러·복사·교체(Replace All) | `ground_at_origin`, `add_pattern`, `mirror_components`, `copy_components`, `replace_component` | `pattern.rs`, `replicate.rs` |
| 표현 | 뷰(가시·활성·투명도·카메라), 위치(Primary + 재정의) | `CellOp::{Add,Set,Activate}{View,Positional}Representation` | `relate.rs` |
| 가져오기·성능 | 메시 로드 잡(자리표시), GLB 및 **STEP AP242 테셀레이션** 읽기, 충돌 형상 캐시, 대형 장면 벤치 | `mesh_pending`, `gc_plugin_mesh::{read_gltf, read_step}`, `bench_scene` | `mesh_jobs.rs`, `gc_plugin_mesh/tests/step.rs` |
| 모션 | 플래너·재계획 사다리·ACM·보증(M1–M2, 아래 오래된 절) | `plan`, `planned(path)` | `gc_motion`, `gc_plan` |

## 현황 — 남음

| 항목 | 상태 / 다음 |
|---|---|
| STEP B-rep(정확 형상) | 테셀레이션 없는 솔리드는 경고 후 건너뜀. 평면·원통부터 자체 테셀레이터 필요. 앱의 가져오기 배선(확장자 → `read_step`)은 리드/FE. 실제 대형 파일(NIST CTC 등) 확인은 네트워크 허용 후 |
| 미러의 진짜 반사 | 파일을 만들지 않아 Mirror = Reuse(프리미티브만 정확). 메시 반사는 렌더/메시 쪽 배율 음수 지원 필요 |
| 로봇 교체, 패턴·표현의 미러/복사 | 미지원 |
| 스케치 패턴 개체 | 스케치 패턴 편집(개수·Delete Pattern·Suppress)은 없음 — 다시 만들어야 함 |
| 접촉 솔버 | 밀린 부품은 병진만(회전 없음), 배율 있는 어셈블리 부품 제외, Surface Complexity 없음 |
| 메시 로드 | `add_mesh_body`는 mm/m 판정용 STL 파싱만 동기 |
| DOF 기호 | 기호마다 관계를 모음(수백 개 동시 표시 시 캐시 필요) |
| Unsolvable | 인벤터의 Recover 상태 대신 거부(원문과 차이, 아래 기록) |
| 에이전트 도구 | `docs/handoff/backend-agent-tools.md`에 코드(gc_agent 수정은 권한상 리드) |
| 모션(오래된 남은 일) | 맨 아래 "남은 일(백엔드)" 절 그대로 |

# 작업 기록(최신 순)

## 2026-11-18 — 스케치 창의 뷰포트 선택 · 호 슬롯 2종(리드 ③①)

- 창이 열린 채 선택: 세션 `SketchPick{Curves, Line, Point}`, `set_sketch_pick(Some(..), 현재 항목)`/`sketch_pick()`. 선택 도구의
  클릭·상자 선택이 선택기 규칙을 따른다(아래 도구 절). gc_viewport는 고칠 필요가 없었다(상자 선택도 `ToolOutput::Select`를 거침).
- 호 슬롯(원문 GUID-D489CE6D-7299-4211-A43A-F3580A4BA357, Create Slots):
  - Three Point Arc Slot: 중심 호의 시작·끝, 세 번째 클릭, 너비.
  - Center Point Arc Slot: 중심, 시작·끝, 너비.
  - `SketchOp::AddArcSlot{centre, start, end, width}`(반시계), `ToolKind::ArcSlot(gc_doc::ArcSlotKind)`, `Prompt::{ArcSlotCentre, ArcSlotThrough, ArcSlotEnd}`.
  - 구성: 구성 중심 호 + 동심 바깥·안쪽 호 + 반원 끝 2개 + 끝마다 구성 반지름 선(중심 → 바깥 호 끝, 그 위에 끝 중심·안쪽 호 끝). 자유도 6.
- 원문과 다른 점:
  - 원문은 3점 호 슬롯의 세 번째 클릭을 "슬롯 호의 중심(center of the slot arc)"이라 적었다. 여기서는 3점 호 도구처럼 호가 지나는 점으로 읽는다.
  - 중심점 호 슬롯은 커서가 쓸어간 방향 대신 시작에서 끝까지 짧은 쪽으로 돈다.
- 테스트: `gc_doc/tests/sketch_shapes.rs` arc_slots…, 세션 `the_arc_slot_tools…`, `a_dialog_selector…`, 앱 kittest 9개.

## 2026-11-18 — 스케치 도형·패턴 도구의 화면 연결(리드 허락, 프론트엔드 대신)

근거(원문): To Work with Points `GUID-D4A4288F-32F1-4E23-A382-930EBA152E87.htm`, To Create 2D Shape Geometry
`GUID-D489CE6D-7299-4211-A43A-F3580A4BA357.htm`, To Stretch, Scale, or Offset 2D Sketch Geometry
`GUID-962CEE71-8BE4-4DA6-B51E-341319C64FA4.htm`, To Create and Edit Sketch Patterns `GUID-AAD3A699-FED8-4614-A31D-0A2316328CEE.htm`.

- **도구(gc_tools `shapes.rs`)**: `ToolKind::{Point, Polygon, Slot(SlotKind), Offset}`.
  - 점: 클릭마다 점(중심점 토글 기본 켬).
  - 다각형: 중심 → 꼭짓점(내접)/변 중점(외접), 미리보기가 커서를 따라감.
  - 슬롯: 종류대로 두 점 → 너비(입력창 `HudField::Width`).
  - 간격띄우기: 곡선 클릭 → 커서 쪽·거리(입력창 `HudField::Distance`) → 클릭. 쪽은 ±거리 결과 중 커서에 가까운 쪽.
  - `Prompt::{PointPlace, PolygonCentre, PolygonCorner, SlotFirst, SlotSecond, SlotWidth, OffsetPick, OffsetPlace}`.
  - 설정(중심점·다각형·간격띄우기 토글)은 도구를 바꿔도 유지(`Tool::renewed`).
- **세션**: `center_point()/set_center_point(on)`(선택된 독립 점도 바꿈), `polygon_settings()/set_polygon_settings(sides, circ)`,
  `offset_options()/set_offset_options(loop, constrain)`, `edit_active_sketch(op)`, `selected_curves()`, `selected_points()`.
- **리본 스케치 탭**(2D):
  - 작성 패널: 점·다각형·슬롯(중심 대 중심) 큰 버튼 + 작은 버튼(전체 슬롯·중심점 슬롯 / 중심점 토글).
  - '패턴' 패널: 직사각형·원형·미러 작은 버튼(두 줄, 1280 px에서 '스케치 마무리'가 밀리지 않게).
  - '수정' 패널: 간격띄우기.
  - '예정' 그룹과 `GroupPlanned` 문구는 삭제.
- **창(gc_app `sketch_dialogs.rs`)**:
  - '미러 패턴'·'직사각형 패턴'·'원형 패턴': 창이 열린 채 뷰포트 클릭이 눌린 선택기(형상·대칭선·방향 1/2·축)로 들어감, 적용(계속)/완료.
    세션 `set_sketch_pick(Some(SketchPick::{Curves, Line, Point}), 현재 항목)`: 선택 도구의 클릭·상자 선택이 선택기 규칙을 따름
    (형상은 Ctrl 없이 클릭마다 토글·빈 곳 클릭은 유지, 선 선택기는 선만, 점 선택기는 점 또는 원·호의 중심). 그동안 끌기·Delete 없음,
    상태 표시줄은 `Prompt::{PickGeometry, PickLine, PickPoint}`. 다른 명령(set_tool)·스케치 종료가 선택을 끝내고 창이 닫힘.
  - '다각형 옵션'(내접/외접·변 수·완료 = 도구 끝), '간격띄우기 옵션'(Loop Select·Constrain Offset).
  - 스케치를 나가면 패턴 창은 닫힘.
- **원문과 다른 점**:
  - 간격띄우기 토글은 우클릭 메뉴 대신 창.
  - 점 HUD(X/Y)는 없음.
- **테스트**: 앱 `tests/sketch_shape_tools.rs` 8개(도구 4, 대화상자 4), 세션 `sketch_shape_tools.rs` 5개.
- **남음**: 호 슬롯 2종, 스케치 패턴 개체(편집·삭제).

## 2026-11-18 — 컴포넌트(occurrence) 선택·접지 토글 · 표현 폴더 · 최대 거리(리드 ①②③)

- ① `session.selected_occurrence()`, `select_occurrence(Option<Occurrence>) -> bool`(없는 것은 거부·유지). CellItem 선택과의 관계:
  바디·로봇은 둘 다(같은 것), 로봇 링크·툴을 고르면 CellItem은 그 로봇(로봇 패널), 어셈블리·부품은 CellItem 없음; `select_item`은 바디·로봇이면
  occurrence도 그것, 웨이포인트·좌표계·경로면 없음. 접지 토글: `can_ground(occ)`(바디·로봇·어셈블리만), `toggle_ground_selected() -> Result<bool>`
  (Undo 1단계). 문서는 링크·툴 접지를 거부(부품 접지는 저장되지만 효과 없음 — 기존 동작 유지).
- ② 표현 폴더: `session.representation_folder() -> RepresentationFolder{views, positions(Primary 먼저)}`, 각 `RepresentationNode{name, active}`.
  `Representations.active_view`(마지막으로 활성화한 뷰, 삭제 시 조정) 추가. 래퍼: `add_view_representation`, `activate_view_representation`,
  `add_positional_representation`, `activate_positional_representation(Option)`, `delete_representation`.
- ③ 측정 최대 거리(원문 GUID-1805AB8F-F782-4B24-A332-B4F2E10A3D00 "Toggle between Minimum Distance, Center to Center, and Maximum Distance"):
  `PairMeasure.maximum: Option<Closest>`(점 두 개 포함), gc_collide `Geometry::farthest` — 상자·구·원통은 해석적 최원점, 메시·헐은 정점,
  여러 시작점에서 번갈아 최원점 갱신(돌린 상자 쌍에서 전수 조사와 일치 테스트).
- 테스트 `gc_session/tests/{occurrence_selection, measure_maximum}.rs`, gc_collide `farthest_is_the_largest_distance_between_the_shapes`.

## 2026-11-18 — 로봇 셀: 설비에 붙은 로봇 · 설비 기준 웨이포인트(리드 ①②)

- ① end-to-end(`gc_session/tests/robot_mount.rs`): 로봇을 받침대에 Mate 또는 Rigid 조인트로 붙이고 받침대를 옮기면(크기도 바꿈) 로봇 베이스가 즉시 따라가고,
  `robot_base_frame`·InBase 웨이포인트가 함께 이동, InWorld 웨이포인트는 그대로, 그 IK는 새 베이스에서 풀려 TCP가 정확히 도달, 이전 계획은 Stale,
  Undo 두 번이면 베이스·계획(Ready) 모두 원상. **코드 수정 불필요**(관계 settle이 Robot.placement를 바꾸고 IK·계획이 그 값을 씀).
- ② 설비 기준 웨이포인트: 이미 연관됨 — 컴포넌트 원점 평면/부품 면 위 스케치 → 작업 좌표계 → `InFrame` 웨이포인트가 설비 이동·회전·구속을 따라감(투영과 같은 원리, 테스트).
  한 번에 만들 수 있게 `session.add_component_frame(occurrence, OriginPlane) -> Id<WorkFrame>`(`CellOp::AddComponentFrame`, Undo 1단계, 링크·툴·부품 occurrence도 가능).
  새 `WaypointTarget::InComponent` 변형은 만들지 않음 — `gc_panels`·`gc_agent`가 WaypointTarget/Reference를 빠짐없이 match해서 UI·앱 수정이 필요(원하면 리드 결정).
  테스트 `gc_session/tests/component_frames.rs`.
- ③ 큰 셀 저장/열기: `bench_scene` "Many relationships" 표에 행 추가(1000 컴포넌트 + 로봇 5대, 관계 1900개, 2.2 MiB): 저장 ~50 ms, 열기 ~7 ms(release).
  개선 불필요로 판단. 메시 바디가 있으면 열기 뒤 메시는 잡으로 들어온다(화면 안 막음). 로봇이 많으면 열 때 URDF 재해석 비용이 로봇 수에 비례.

## 2026-11-18 — QA 버그 5건 · 관계가 많을 때의 속도

- QA(eec466e) 버그 수정, `#[ignore]` 해제: read_step 중첩 256단 상한(스택 오버플로 중단 → `Unreadable`), 미러 비유한 평면 거부,
  접촉 집합(및 DOF 표시) 중복 저장 안 함, 매 명령 후 없는 컴포넌트의 저장 상태 정리(`tree::prune_states`), predict 비유한 선택 → None.
- KNOWN_ISSUES "관계 추가 시간 급증"(400개 9.5 s, 1000개 5분+): 풀이를 **섬**(관계로 묶인 자유 컴포넌트 묶음)으로 나누고 이미 만족된 섬은 건너뜀
  → 관계 하나 추가 시 그 섬만 다시 푼다. `bench_scene --components 1000`으로 셀 구성 전체 ~0.15 s(release).
  프레임 질의: 세션이 `component_statuses()`를 문서 리비전별로 캐시(문서 교체 시 비움) → 1000개 p95 3.8 ms → 1.7 ms(예산 2 ms).
  테스트 `gc_session/tests/relation_scale.rs`(300개 메이트 디버그에서 0.05 s).
  `bench_scene`에 "Many relationships" 표(1000 컴포넌트·1900 관계, 10개씩 섬): 관계 1개 추가 7.8 ms, 오프셋 편집 8.0 ms, 삭제 0.1 ms(목표 100 ms, release). KNOWN_ISSUES 항목은 리드가 닫아 주세요(문서 소유 아님).

## 2026-11-18 — 툴 컴포넌트 · STEP AP242 · Assemble 유형(리드 ①②④)

- ① `Occurrence::Tool { robot }`: 툴이 팁 링크 아래 하위 컴포넌트(`robot_links` 끝). 원점 = 플랜지 프레임 × 툴 장착 배치라 `mount_tool`·세션 운동학과 일치(조인트 값과 무관하게 테스트).
  숨김·비활성 가능, 단독 접지 불가. 구속·조인트·스케치·투영이 툴을 참조 가능. 툴을 떼면(`SetTool None`) 그 관계도 삭제(컴포넌트 삭제와 같게), 다시 장착하면 묶인 것이 따라옴.
- ② `gc_plugin_mesh::read_step(bytes) -> Assembly`: 자체 Part 21 파서(새 의존성 없음). TRIANGULATED_FACE/SURFACE_SET(+COMPLEX_ 스트립·팬), TESSELLATED_SOLID/SHELL,
  제품 트리(NAUO + CDSR/ITEM_DEFINED_TRANSFORMATION, 공유 정의 인스턴스), 색(STYLED_ITEM → COLOUR_RGB), 길이 단위 → m. B-rep은 경고.
  픽스처 `fixtures/step/two_blocks_ap242.stp`는 손으로 작성(라이선스 문제 없음). 생성한 8만 삼각형 파일 0.07 s(release).
- ④ Assemble(원문 GUID-C1E8D955-784D-47DE-9DD4-C9C695051320 "The geometry you select determines the available constraint type"):
  `gc_doc::possible_relations(first, second) -> Vec<AssembleSolution>`(세션 래퍼 동명), `AssembleSolution::kind(값)`. 평면 둘 → Mate-Mate·Mate-Flush·Angle-Directed(원문 예),
  원형 모서리 → Insert 둘, 구/원통-평면 → Tangent 둘. 솔버가 받는 조합만(`Relationship::is_valid`). 조인트·Symmetry·Angle-Undirected는 원문대로 Assemble에 없음.

## 2026-11-18 — 메시 로드 잡 · 어셈블리 접촉 · Drive 충돌 감지(리드 ①②③)

- ① 바디·툴 메시 파일은 `gc_jobs` 잡에서 읽고(배율 적용 + 충돌 형상 BVH까지) `poll_jobs`가 받아들인다. 바디는 문서에 즉시 들어가고(undo 1단계),
  메시가 오는 것은 편집이 아니다. 자리표시용 `session.mesh_pending(&solid)`, `meshes_loading()`. 실패는 그대로 `mesh_error(file)`.
  기본(인라인) 실행기에서는 반환 시 이미 와 있어 기존 동작과 같다. 어셈블리 가져오기 잡도 메시 충돌 형상을 그 스레드에서 미리 만든다(열 때의 sync 경로도).
  벤치(스레드 실행기): 20×10만 삼각형 추가가 화면을 막는 시간 1.3 s → 0.03 s, 전부 도착까지 ~0.55 s(병렬).
  남은 것: `add_mesh_body`는 mm/m 판정 때문에 STL을 한 번 동기로 읽는다(파싱만, BVH는 잡).
- ② 어셈블리(가져온 GLB/STEP)도 접촉 솔버 참가: 보이는 부품 중 강체 행렬인 것(배율 있는 부품은 제외). 접지·관계 없으면 밀리고(`driven_placement`, `assembly_parts`가 밀린 위치로 그림), 아니면 정지.
- ③ Drive 대화상자(`gc_app/src/drive.rs`, 리드 허락): 원문상 Collision Detection은 접촉 솔버와 별개(접촉 세트 없이도 검사, 충돌 시 정지)라서
  둘 다 둔다 — 먼저 `session.contact_stop()`(솔버가 켜져 있을 때 스스로 멈춘 경우), Collision Detection이 켜져 있으면 `session.drive_collision()`.
  `drive_collision`은 프레임마다 `interference`(24³ 격자 부피 추정)를 부르던 것을 대신해, 구동으로 실제 움직인 컴포넌트만 AABB+침투로 검사하고
  문서 위치보다 더 깊어질 때만 충돌(닿아 있는 것은 아님). 숨김·비활성·억제 제외. 이름은 `session.component_name(AnyId)`.
  `shell.rs::drive_moves_a_mated_block_and_stops_at_a_collision` 통과.
- 테스트: `gc_session/tests/mesh_jobs.rs`, `contact.rs`(어셈블리, 충돌 감지 추가).

## 2026-11-18 — 대형 메시 성능(리드 ③)

`bench_scene`에 "Large meshes" 표 추가(`--meshes 20 --triangles 100000` 기본, `cargo xtask bench-quality`가 보고서에 덧붙임).
처음 돌렸을 때 접촉 구동 한 단계 21 s, Measure 103 ms, Interference(190쌍) 200 s — 원인과 수정:
- gc_collide가 질의마다 TriMesh(BVH)를 새로 만들었다 → 메시별 공유 캐시(`MESH_SHAPES`, 메시가 사라지면 항목도 정리),
  세션이 메시 파일을 읽을 때 `gc_collide::prepare_mesh`로 미리 만든다(드래그·구동 중이 아니라 불러올 때 비용).
- 간섭이 `closest`(여백 무한, 큰 메시 둘 사이 전체 거리 계산)로 겹침을 판정 → `Geometry::intersects`(AABB 선검사 + intersection_test).
  `penetration`도 AABB 선검사.
- 접촉 솔버: 솔버가 반올림만큼 건드린 컴포넌트까지 "움직임"으로 셌다 → 실제로 움직인 것만; 이미 겹쳐 있던 쌍은 더 깊어질 때만 접촉.
결과(이 샌드박스, release): 프레임 질의 p95 0.09 ms, 드래그 0.06 ms, 풀이 0.01 ms, 접촉 구동 한 단계 2.5–4 ms, Measure 0.05 ms,
Interference 190쌍 ~0.12–0.15 s(명령), 0.15 m 구동을 한 번에(2 mm × ~50 단계, 접촉 정지) ~0.26 s.
남은 것: 메시 20개 추가(파일 읽기 + BVH) ~1.3 s가 동기 — 불러오기 작업(job)으로 옮기면 좋다. 구동 대화상자는 프레임마다 값을 조금씩 넘길 것.
어셈블리(GLB/STEP) 부품은 세션의 간섭·측정·접촉 대상이 아니라 이 표에 없음(구속 풀이는 포즈만 써서 메시 크기와 무관).

## 2026-11-18 — 자유도(DOF) 기호 API(리드 ②)

근거(원문): Degrees of freedom in assemblies `GUID-6E131E5A-438B-4008-94F3-D3A3E92EF44C.htm`,
View degrees of freedom in assemblies `GUID-3BCBF614-CF92-4980-A965-80AEAFEF768E.htm`.
- `doc.dof_symbol(item) / session.dof_symbol(item) -> Option<DofSymbol{origin, translations: Vec<[f64;3]>(단위 방향),
  rotations: Vec<DofAxis{point, direction}>, relative_to: Vec<AnyId>}>`. 다른 컴포넌트는 고정으로 보고, 구속 야코비안의 영공간에서
  순수 이동 방향을 먼저 뽑고, 남은 운동을 나사축(점 = 원점 + ω×v/|ω|²)으로 낸다 — 힌지는 축이 힌지 선 위에 온다. 개수는 `component_dof`와 같다.
  `relative_to` = 원문의 큐브(덜 구속된 부품에 상대 구속된 경우, 그 부품들).
- 표시 상태: iProperties Occurrence 탭 체크 = `CellOp::SetDofShown{items, on}`(세션 `set_dof_shown`), View 탭 전체 = `CellOp::SetDofShownAll(bool)`.
  `doc.dof_shown(item)`, 그릴 것: `session.dof_symbols() -> Vec<(item, DofSymbol)>`(억제 제외).
- 기호 모양(화살표·원호)은 원문에 그림만 있어 프론트엔드 결정. 성능: 기호 하나마다 관계를 모으므로 수백 개를 동시에 켜면 느릴 수 있음(필요하면 캐시).
- 테스트 `gc_session/tests/dof_symbol.rs`(4).

## 2026-11-18 — 접촉 솔버(Contact Solver, 리드 ①)

근거(원문): Detect contact between assembly components `GUID-FA1A8282-3A7E-45BE-8DB3-F637FB1C3292.htm`,
About the Contact Solver and Component Collision `GUID-0BBD4ACE-7BD1-40BF-A011-74416B229035.htm`.
- 저장: `Components.contact_set`(Occurrence), `Components.contact_solver: ContactSolver::{Off(기본), ContactSet, All}`(Interactive Contact).
  `CellOp::SetContactSet{items, on}`, `CellOp::SetContactSolver(mode)`; 세션 `set_contact_set`, `set_contact_solver`.
  문서 `contact_set()`, `contact_solver()`, `in_contact_set(item)`, `contact_participants()`(숨김·비활성·억제 제외 — 원문).
- 구동: 솔버가 켜져 있고 참가자가 둘 이상이면 `drive_relationship`이 현재 값에서 목표까지 2 mm/1° 단계로 진행.
  접촉 시 밀 수 있는 컴포넌트(바디, 접지 아님, 활성 관계 없음)는 침투 깊이만큼 법선 방향으로 밀고(연쇄 가능),
  밀 수 없으면(접지·관계로 묶임·구동 중·로봇) 직전 값에서 멈춤. `session.contact_stop() -> Option<ContactStop{value, first, second}>`,
  `contact_pushed()`. 원문: "contact can push another component … exception … cannot move (grounded) → stops".
- 드래그: `preview_body`가 집합 안 바디면 같은 규칙(멈추면 미리보기 위치 유지), `end_body_preview`가 밀린 것까지 `CellOp::MoveComponents` 한 undo로 커밋.
- gc_collide `Geometry::penetration(&other, tol) -> Option<Penetration{depth, normal}>`(parry contact).
- 아직: 어셈블리 메시 참가(세션에 형상 없음), 밀린 부품의 회전(병진만), Surface Complexity 설정, 별도 Collision Detection 옵션(구동 시 충돌 정지 —
  접촉 솔버 밖의 간섭으로 멈추기)은 프론트엔드 Drive 대화상자 설계 후.
- 테스트 `gc_session/tests/contact.rs`(3), gc_collide `penetration_gives_the_depth_and_the_way_out`.

## 2026-11-18 — 컴포넌트 미러·복사·교체(리드 ②③)

근거(원문, `https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/`):
About Mirrored Assembly Components `GUID-66AA4A06-F63F-49C3-8F39-42FE4999F1FC.htm`, To Mirror Assembly Components `GUID-C5B3D3DF-27B3-4B4C-9058-A949138D8B8A.htm`,
Mirror Components: Reference `GUID-7B19CA69-8D14-40DA-9543-115A8AFE6336.htm`, About Copied Assembly Components `GUID-0E21F4A0-DE77-469D-9381-9F55CFA6BF49.htm`,
Copy assembly components `GUID-5AAA6CB1-7BB2-4AF0-9A3D-AB729E5160C5.htm`, Replace assembly components `GUID-E5FAA224-7332-4AC6-A78E-793FDACFEEDD.htm`.
- 미러: `CellOp::MirrorComponents{items, plane: Selection(평면), relationships, ground}`, 세션 `mirror_components(items, plane, rel, ground) -> 억제된 관계 이름`.
  원문의 Reuse 방식(회전+이동): 컴포넌트의 대칭면 = 미러 법선에 가장 가까운 로컬 좌표 평면, R' = M·R·F. 이름 `Block:1`.
  "두 참조가 모두 선택 집합에 있는 관계"만 다시 만들고, 바깥 참조(대칭면·각도 기준)가 있으면 방향이 모호 → 억제, 풀리지 않는 것도 억제.
  Directed 각도는 부호 반전. Ground New Components 지원.
- 복사: `CellOp::CopyComponents{items, offset: Placement, relationships, ground}`, 세션 `copy_components(..)`. 모양·외관·enabled 상태 복사, 비연관.
- 교체: `CellOp::ReplaceComponents{items, with: Replacement::{Solid(Solid), Assembly(Assembly)}}`, 세션 `replace_component(item, with, all)`
  (`all` = Replace All, `doc.same_source(item)`: 같은 메시 파일/같은 프리미티브, 같은 어셈블리 파일, 같은 URDF). 같은 위치·원점 일치.
  관계 유지: 박스·원기둥·구는 선택 기하가 새 모양에 그대로 있는지 검사(원점 기하는 항상 유지), 메시는 문서가 모양을 몰라 유지로 본다,
  어셈블리 부품은 이름으로 다시 찾는다. 사라진 것과 풀리지 않는 것은 억제하고 이름을 돌려준다(원문: "must be reapplied").
- 원문과 다른 점: 파일을 만들지 않으므로 Mirror = Reuse(프리미티브는 정확히 대칭, 메시·어셈블리·로봇은 반사된 모양이 아니라 회전 배치).
  미러 대칭면 변경(우클릭), 패턴·표현 복제, 파일 이름 대화상자 없음. 로봇 교체는 아직(URDF 다시 읽기 필요 — 세션에서 add_robot 후 관계 재지정).
- 테스트 `gc_doc/tests/replicate.rs`(5), `gc_session/tests/replicate.rs`(2).

## 2026-11-18 — P2-20 표현(Representations): 뷰·위치

근거(원문): To Create, Edit, or Copy Positional Representations `GUID-34C28C35-45CA-4476-9336-30146F7C9471.htm`([Primary] = 기본 상태,
새 위치 표현은 Primary 복사, 관계·컴포넌트 재정의, 하나만 활성). About View Representations `GUID-9A49CDC1-4DC7-4462-81D7-E18B8339DE0F.htm`(가시성·enabled 상태·투명도·카메라) — 33a40fa에서 enabled·투명도도 저장.
- 뷰: `CellOp::AddViewRepresentation{name, camera, section}`(지금 가시성 저장; camera·section은 뷰포트가 쓰고 읽는 문자열 — 형식은 프론트엔드/리드 결정),
  `SetViewRepresentation{index, view}`, `ActivateViewRepresentation(index)`(숨김·표시를 한 undo 단계로 적용, 부품·로봇 링크 포함).
- 위치: `AddPositionalRepresentation(name)`("Primary" 예약), `SetPositionalRepresentation{index, position: PositionalRepresentation{values:
  [(관계, 값)], suppression: [(관계, bool)], placements: [(컴포넌트, Placement)]}}`, `ActivatePositionalRepresentation(Some(i) | None=Primary)`.
  활성 위치의 재정의는 풀이 때 적용되고 Primary의 관계 정의는 그대로; Primary로 돌아가면 컴포넌트가 Primary 위치로 돌아간다.
  `DeleteRepresentation{view, position}`. 세션 `representations()`. 저장: `Timeline.representations`.
- 아직: 엑셀 편집, Verify, 표현 복사(Copy = 지금은 Set으로 복제 가능), Model State.
- 테스트 `gc_doc/tests/relate.rs`(view_representations_…, positional_representations_…).

## 2026-11-18 — 컴포넌트 패턴(직사각형·원형, 리드 순서 ②)

근거(원문): Create rectangular patterns of components `GUID-CF627358-120F-4142-A9D0-13217BD3432D.htm`, Create circular patterns of
components `GUID-FF7B08E3-624B-4382-B4E5-304C0354C048.htm`.
- `PatternLayout::Rectangular{column: PatternDirection{along: Selection, flip, count, spacing}, row: Option<..>}` — 방향은 원점 축·모서리·작업 축·면
  (평면 = 법선, 원통 = 축)을 `Selection`으로; `PatternLayout::Circular{axis, flip, count, angle, fitted, rotational, midplane}` —
  Incremental(사이 각) / Fitted(전체 각, 360°면 count로 나눔), Rotational(돌며 방향도 회전) / Fixed(방향 유지), Midplane(양쪽 번갈아).
- 명령: `CellOp::{AddPattern{name, parents, layout}, SetPattern{index, layout}, DeletePattern{index, keep_copies},
  MakePatternElementIndependent{index, element}}`, 세션 `add_pattern`, `patterns() -> Vec<ComponentPattern{name, parents, layout,
  elements: [PatternElement{index, items}], independent}>`. 복사본은 실제 바디·로봇·어셈블리(이름 `원본:2` …)이고 모든 명령 뒤 부모와 배치를
  따라간다. 개수 변경 시 추가·삭제, 독립화한 자리는 다시 채우지 않음, 부모가 지워지면 연결만 끊김. 저장: `Timeline.patterns`.
- 아직: Associative(피처 패턴 연동), Base Point, 요소별 억제(컴포넌트 억제로 대신 가능), 관계 복제. 인덱스로 패턴을 가리키므로 삭제 시 뒤 인덱스가 당겨진다.
- 테스트 `gc_doc/tests/pattern.rs`.

## 2026-11-18 — 대형 장면: 프레임당 세션 질의 벤치(리드 순서 ③)

- `cargo run --release -p gc_session --example bench_scene [-- --components N --frames N --append FILE]`; `cargo xtask bench-quality`가
  계획 벤치 뒤에 실행해 보고서에 표를 덧붙인다. 측정 범위: 한 프레임에 장면·브라우저가 세션에 묻는 것(바디 표시, 로봇 링크 자세, 충돌 상태
  (캐시), TCP, 모든 컴포넌트의 구속 상태 점). 렌더러의 드로 리스트·BVH 자체는 리드의 gc_scene 쪽에서 따로 재야 한다.
- 결과(이 샌드박스, release): 50개 p95 0.105 ms(예산 2 ms 통과), 200개 p95 0.565 ms.
- 고친 것: 구속 상태 점 계산이 컴포넌트마다 관계를 여러 번 다시 모아 200개에서 4.7 ms(이차)였다 → 한 번 모아 재사용. 브라우저는 매 프레임
  **`session.component_statuses()`**(한 번에 전부)를 쓰면 된다(행마다 `component_status` 대신).

## 구속 오류 대화상자(Design Doctor식) — 프론트엔드용 사용 예

관계 추가·수정·배치 변경이 `EditError::Unsolvable{ids, names}`로 거부되면:

```rust
match session.add_relationship(candidate.clone()) {
    Ok(_) => {}
    Err(EditError::Unsolvable { names, .. }) => {
        // 1. 무엇이 문제인지: 거부 메시지(이름 포함)와, 하나만 억제하면 풀리는 기존 관계들.
        let culprits = session.conflicting_relationships(Some(&candidate));
        // 2. 대화상자: "Mate:1, Flush:2를 함께 만족할 수 없습니다" + culprits 행마다 [억제] [편집] [삭제].
        //    culprits가 비면 "한 관계만으로는 풀리지 않음 — 새 관계를 바꾸세요"(둘 이상이 엉킨 경우).
        // 3. [억제]: session.set_relationship_suppressed(id, true) 뒤 다시 add_relationship(candidate).
        //    [편집]: 구속 대화상자를 그 관계로 열기. [삭제]: session.delete_relationship(id).
    }
    Err(other) => { /* 다른 오류 */ }
}
```

- 배치 이동(기즈모·숫자)이 거부된 경우는 `conflicting_relationships(None)`이 비어 있다(지금 상태는 만족되므로) — 그 이동이 고정된 관계와
  맞지 않는다는 뜻이니 "이 컴포넌트는 구속돼 있어 그 위치로 갈 수 없음"과 `relationships()` 중 그 컴포넌트를 쓰는 것을 보여 주면 된다
  (`r.first.occurrence.owner() == item || r.second.occurrence.owner() == item`).
- 억제된 관계는 `doc.is_inactive(id.erase())`가 true — Relationships 폴더에서 회색·취소선(원문 A60: 억제 요소 표시).
- 브라우저 상태 점은 `component_status(item)`(Full/Under/Unknown), 관계 행 표시는 `relationship_status(id)`(Full/Under/Sick).

## 2026-11-18 — 구속 한계(Limits) · 구속 오류 회복

- `Relationship.limits: Option<Limits{min, max, resting}>`(+`Relationship::new(name, kind, first, second)` — app·테스트 리터럴을 생성자로 바꿈,
  리드 허락). 대상: Mate·Flush·Insert·Tangent(오프셋 m)·Angle(rad). 의미(원문 To Place Mate or Flush의 Maximum/Minimum/Use Offset As
  Resting Position): `resting=false` → 값이 한계 안 어디든 머무름(밖으로 끌면 한계에서 멈춤), `resting=true` → 관계 값에 고정(구동만 한계 안에서
  움직임). `drive_relationship`은 항상 한계로 자른다. 순서가 뒤집힌 한계·값 없는 종류(조인트·대칭)의 한계는 거부.
  구동 대화상자: Start/End를 min/max로 쓰면 된다(조인트 각 한계는 아직 — Rotational에 각도 기준이 없어서).
- 구속 오류: `session.conflicting_relationships(candidate: Option<&Relationship>) -> Vec<Id<Relationship>>` — 새 관계(또는 지금 상태)를
  막는 관계 중 **하나만 억제하면** 모두 풀리는 것들. `session.set_relationship_suppressed(id, bool)`(한 undo 단계, 다시 풀이; 되살렸을 때
  충돌하면 거부). 억제된 관계는 풀이에서 빠진다. 대화상자는 `Unsolvable{names}` + 이 목록 + 억제 버튼이면 Design Doctor 흐름이 된다.
- 테스트 `gc_doc/tests/relate.rs`(limits_…, a_conflict_names_…).

## 에이전트 도구 후보(리드 요청, PLAN 9.8 2단계) — 백엔드 세션 API 정리

모두 `gc_session::Session` 메서드, 문서를 바꾸는 것은 한 undo 단계. 인자 타입은 `gc_doc`의 평범한 자료(serde 가능).

| 묶음 | 메서드 |
|---|---|
| 컴포넌트 트리 | `components() -> Vec<ComponentNode>`, `robot_links(robot) -> Vec<ComponentNode>`, `set_component(occ, ComponentFlag, bool)` |
| 원점 폴더 | `component_origin(occ) -> Option<ComponentOrigin>`, `set_origin_visible(occ, &[OriginElement], bool)`, `origin_visible(occ, el)` |
| 배치 | `ground_at_origin(&[AnyId])`, `place_at_component_origin(item, target: Occurrence)`, `mount_tool(robot, Geometry, flip)` |
| 구속·조인트 | `add_relationship(Relationship) -> Id`, `set_relationship(id, Relationship)`, `delete_relationship(id)`, `relationships()`, `predict_relationship(&Selection, &Selection)`, `automatic_joint(&a, &b)` |
| 상태 | `component_dof(item) -> Option<Dof>`, `component_status(item) -> Option<ComponentStatus>`, `relationship_status(id)` |
| 구동 | `drive_relationship(id, value)`, `end_drive_relationship()`, `driven_placement(item)` |
| 스케치 평면 | `create_sketch_on_component(occ, OriginPlane)`, `create_sketch_on_work_plane(id)`, `create_sketch_on_face(..)` |
| 투영 | `project_geometry(ProjectSource, shift) -> Vec<Created>` |
| 매개변수 | `execute(Command::{AddParam, SetParam, RenameParam, DeleteParam})`, 스케치: `SketchOp::{AddDimension, SetDimension("NAME = 값" 가능), RenameDimension, SetDriving}` |
| 스케치 도구 | `tool_event(ToolEvent)`, `tool_hud()`, `set_tool_construction(bool)`; Trim/Extend/Fillet은 `SketchOp::{Trim, Extend, Fillet}`로 직접도 가능 |

에이전트가 쓰기 좋은 순서: `components()`로 대상 찾기 → `component_origin`으로 좌표 확인 → `add_relationship`/`ground_at_origin`으로 배치 →
`component_status`로 확인. 실패는 `EditError`(Display가 사람이 읽는 문장, `Unsolvable`은 관계 이름 포함).

## 2026-11-18 — 모델 매개변수(d0, d1 …) — 리드 요청

근거(원문): To Work with Sketch Dimensions `GUID-026FE8D3-AFBB-4834-BC18-73D9CFF78185.htm` — Edit Dimension에서 식 입력 가능,
"enter HGHT = 5mm … creates the parameter HGHT and assigns the value of 5 mm".
- `Dimension.name`(gc_sketch, 저장됨): 모든 치수가 문서 전역 이름 `dN`(쓰는 중 가장 큰 번호 + 1)을 받는다 — 모든 명령 뒤 자동 부여.
- 수식에서 매개변수와 똑같이 참조: 구동 치수는 값, 참조(driven) 치수는 지금 측정값. 매개변수·구동 치수를 한 의존 그래프로 순서대로 계산,
  순환은 `EditError::Cycle`. 치수를 바꾸면 그것을 쓰는 치수·매개변수가 따라간다.
- `NAME = 식`: AddDimension/SetDimension의 source가 이 꼴이면 치수 이름을 NAME으로(기존 참조도 바꿈), 식만 값으로.
  `SketchOp::RenameDimension{id, name}`(참조 갱신). 이름은 매개변수·치수 통틀어 유일(AddParam/RenameParam도 치수 이름과 충돌 거부).
- 테스트 `gc_doc/tests/document.rs::dimensions_are_model_parameters_…`.
- 아직: 매개변수 대화상자에 모델 매개변수 행 표시(리드 parameters.rs) — `doc.sketches()`의 `dimensions[*].name/source/value`로 읽으면 된다.

## 2026-11-18 — P2-17 구속 완성(Angle·Tangent·Symmetry) · P2-18 조인트 완성(Slider·Cylindrical·Ball)

근거(원문): To Place Angle Constraints `GUID-AD4679FD-EE9B-47C0-9E32-04B990BDD29E.htm`(Directed = 항상 오른손 법칙, 축은 선택에서 추론;
Undirected = 어느 쪽이든; Explicit Reference Vector = 세 번째 선택이 외적(Z) 방향), Place Tangent constraints
`GUID-FF270566-CA9E-420A-A00A-F52DD0FDFD8E.htm`(면·평면·원통·구, Inside/Outside, 오프셋), Place Symmetry constraints
`GUID-237E701E-A16B-49D5-95C2-FB6528E41217.htm`(두 대상이 평면 기준 대칭), Create Joints Reference `GUID-6AA68E8F-…`(Slider = 병진 1,
Cylindrical = 병진 1 + 회전 1, Ball = 회전 3).
- `RelationKind::{Angle{angle, solution: AngleSolution::{Directed, Undirected, Explicit(Selection)}}, Tangent{offset, inside},
  Symmetry{plane: Selection}, Slider{flip}, Cylindrical{flip}, Ball}`; `Geometry::{Cylinder{point, direction, radius}, Sphere{centre, radius}}`
  (Mate에서는 원통 = 축, 구 = 중심). `Relationship::third()` = 대칭 평면·명시 기준. 세 번째 선택의 컴포넌트도 삭제 보호(refs).
- 각도는 평면 법선·모서리/축 방향 사이(rad). Directed의 축은 풀이 시작 때 두 방향의 외적으로 읽는다.
- `drive_relationship`: Angle(각도)·Tangent(오프셋)도 구동.
- 아직: 한계(Maximum/Minimum/Resting) — 관계 구조체에 필드를 늘리면 app의 리터럴이 깨지므로 리드와 표현 합의 후.
- 테스트 `gc_doc/tests/relate.rs`(각도 3방식·자유도, 공·롤러 접선·inside, 대칭, 세 조인트 자유도).

## 2026-11-18 — P2-16 Ground and Root(원점에 고정) · Place at Component Origin

근거(원문): Ground and Root `GUID-98FA88B2-AA32-4A27-946A-DCECB36A6696.htm`, Place at Component Origin
`GUID-E7A45820-E297-449D-AFBF-3C8D8EAE8A0E.htm`.
- `session.ground_at_origin(&[AnyId])`(`CellOp::GroundAtOrigin`): 어셈블리 원점으로 옮기고 정렬한 뒤 고정, 한 undo 단계. 묶인 컴포넌트는
  다시 풀어 따라온다. **원문과 차이**: 기존 구속과 충돌하면 원문은 고정하고 빨간 Recover를 띄우지만, 우리는 `Unsolvable`로 거부한다
  (아픈 상태를 허용하지 않는 설계). "Create origin flush constraints"(어셈블리 원점은 컴포넌트가 아니라 지금 표현 불가)와
  "Reposition to the top of the browser"(컴포넌트 순서 개념 없음)는 아직 없다.
- `session.place_at_component_origin(item, target: Occurrence)`(`CellOp::PlaceAtComponentOrigin`): 컴포넌트를 대상(부품·로봇 링크 포함)의
  원점에 맞춰 놓고 원점 평면 3쌍 Flush로 구속 → 완전 구속, 대상을 따라감. 파일 열기 대화상자 흐름은 프론트엔드(추가 후 이 함수 호출).
- 억제 로봇은 `item_geometry`·`interference`에서도 빠진다(measure.rs 한 곳).
- P2-17/P2-18은 `RelationKind` 변형 추가가 필요 — app의 constrain.rs가 전수 match라 리드에게 `_ =>` 갈래를 요청해 둠.
- 테스트 `gc_doc/tests/relate.rs`(ground_at_origin…, place_at_component_origin…).

## 2026-11-18 — 바디·로봇 억제(리드 순서 ⑤)

- `session.set_component(Occurrence::Item(body|robot), ComponentFlag::Suppressed, on)` 이제 허용(한 undo 단계, 저장). 억제된 바디:
  `shown_body` = None(그리지 않음·선택 안 됨), 장애물·계획·지문에서 빠짐. 억제된 로봇: `link_poses`/`tcp_pose` = None(그리지 않음),
  `plan`은 NotPlanned. 문서에는 그대로 남는다. `Document::is_component_suppressed(item)`.
- 근거: 원문 A60(억제 요소는 그래픽에 없고 갱신 때 계산되지 않음) — 명세 표 인용.
- 테스트 `gc_session/tests/robot_links.rs::suppressed_bodies_and_robots_are_kept_but_not_drawn_checked_or_planned`.

## 2026-11-18 — P1-6은 리드 구현(e25a650)으로 통일

- 백엔드의 중복 구현(e1b73ae: session.project_cut_edges, SketchOp::AddFixedSegments)은 리드 결정에 따라 제거했다.

## 2026-11-18 — 어셈블리 P1-5 Project Geometry

근거(원문): To Project Geometry into a 2D Sketch `GUID-B8D8B07B-86F2-4CB7-AB21-3131D94AD1CC.htm`, About Projecting Sketch Geometry
`GUID-B61431DA-FABC-41EE-945C-6E54B4806582.htm` — 투영은 원본과 연결(원본이 바뀌면 갱신), 원본 삭제 시 고정 스케치 곡선으로 바뀌고
구속·치수는 유지, Shift+클릭은 구성선 토글의 반대로 투영, Break Link로 연결 해제.
**명세와 차이**: 원문은 Project **Cut Edges**는 연관되지 않는 스냅샷("if the parent geometry changes, the projected geometry does not
update")이라고 한다 — 명세 P1-6 수용 기준("평면을 옮기면 갱신")과 다르다. P1-6은 원문대로 스냅샷으로 구현할 예정(리드 판단 필요하면 알려 주세요).

- `SketchOp::Project{source: ProjectSource, construction}` / `session.project_geometry(source, shift) -> Result<Vec<Created>, EditError>`
  (구성선 = 서식 토글 XOR shift). `ProjectSource::{Point{occurrence, local}, Edge{occurrence, a, b}, OriginAxis{occurrence, axis},
  WorkPoint(id), WorkAxis(id)}` — 컴포넌트(부품·로봇 링크 포함) 프레임 좌표. 점은 참조점, 모서리·축은 참조점 2개 + 선(축은 ±0.1 m,
  끝에서 보면 점 하나). 메시 꼭짓점·모서리는 프론트엔드가 피킹 좌표를 컴포넌트 프레임으로 바꿔 Point/Edge로 넘긴다.
- 연결: `ExternalRef::Projected{owner, part, local_nm}`(gc_sketch). 모든 명령 뒤 `apply`가 투영점을 원본 위치로 옮기고 스케치를 다시 푼다.
  원본이 없어지면 점은 Free + `Fix` 구속(구속 유지). 원본 삭제를 막지 않는다.
- 아직: Break Link 명령, 면 클릭 = 모든 루프(메시 루프 추출), 치수·구속 시 자동 투영.
- 테스트 `gc_session/tests/project_geometry.rs`.

## 2026-11-18 — 관계 구동(Drive) — 리드 요청

- `session.drive_relationship(id, value) -> Result<(), EditError>`: 문서를 바꾸지 않고 관계를 `value`로 구동했을 때의 위치로 보여 준다
  (Mate·Flush·Insert = 오프셋 m, Rotational = 지금 자세에서의 회전 rad; Rigid는 거부). 풀 수 없는 값은 거부하고 직전 표시 유지.
  `session.end_drive_relationship()`로 문서 위치로 복귀. `session.driven_placement(item) -> Option<Placement>`.
- 표시 연결: `shown_body`(바디), `shown_base`(로봇), `assembly_parts`(어셈블리)가 구동 위치를 쓴다 — 렌더가 이 셋을 쓰면 추가 작업 없음.
  계산은 `Document::driven(id, value) -> Result<Vec<(AnyId, DPose3)>, EditError>`(문서 사본에 같은 명령을 적용해 풀이).
- 아직: 한계(Start/End)·단계·반복 재생은 대시보드 쪽(리드). 구동 중 문서가 바뀌면 표시가 낡을 수 있으니 대화상자를 닫을 때 end 호출.
  원문 A31(To Drive Relationships)의 GUID가 명세 표에서 잘려 있어 원문 대조는 아직.
- 테스트 `gc_session/tests/robot_links.rs::drive_plays_a_relationship_through_values_without_changing_the_document`.

## 2026-11-18 — 로봇 링크를 하위 컴포넌트로(리드 순서 ②)

사용자 요구 "로봇조차도 xyz가 있으니 xy평면 같은 것들에 접근 가능해야".
- 문서: `Robot.links: Vec<RobotLink{name, parent, origin, axis, motion: LinkMotion::{Fixed, Turn{joint, scale, offset}, Slide{..}}}>` —
  설명(URDF)에서 뽑은 링크 트리를 평범한 자료로 저장(문서가 URDF를 읽지 않고 링크 프레임을 계산). `Robot::link_frames()`(로봇 배치 기준).
  로봇을 추가할 때 채우고, 옛 프로젝트는 열 때 채운다(편집이 아니며 '변경됨'으로 표시 안 함). `CellOp::SetRobotLinks{id, links}`.
  미믹 관절은 구동 관절의 배율·오프셋으로, 계획 체인 밖 관절은 0으로 고정.
- `Occurrence::Link{robot, index}` — `component_origin`(관절값을 따라 움직이는 링크 원점 폴더: 평면 3·축 3·점), 구속·조인트 선택,
  `create_sketch_on_component`, 원점 표시, 가시성(숨김) 모두 같은 API. 억제는 거부.
- **조인트 값이 바뀌면(SetJoints) 관계를 다시 풀되 그 로봇의 베이스는 그대로**: 링크에 Rigid로 붙인 바디(툴·공작물)가 팔을 따라간다.
- 세션: `session.robot_links(robot) -> Vec<ComponentNode>`(로봇 노드 아래 표시용; `components()`에는 넣지 않음 — 상태바 개수·기존 트리 유지).
  미리보기 관절(조그·구동·재생)은 문서 값이 아니므로 링크 원점은 문서 관절값 기준이다.
- 테스트 `gc_session/tests/robot_links.rs`(세션 FK와 일치, 관절 변경 추종, 링크에 붙인 바디·링크 평면 스케치 추종, 옛 프로젝트 채움).

## 2026-11-18 — 어셈블리 P1-10 구속 상태·남은 자유도

근거(원문): Degrees of freedom in assemblies `GUID-6E131E5A-438B-4008-94F3-D3A3E92EF44C.htm`(처음 6 = 병진 3 + 회전 3, 구속은 자유도를
줄이고 조인트는 자유도를 명시), To Manage the Display of Constraint Status in the Browser `GUID-A0573060-3778-4D6F-9431-28587D837CC7.htm`
(**컴포넌트별** 점: 검은 점 = 완전 구속, 속 빈 점 = 미구속, 대시 = 알 수 없음; Hide Fully Constrained / Show Constraint State 메뉴).

- `session.component_dof(item) -> Option<gc_doc::Dof{total, translation}>`(+`rotation()`): 다른 컴포넌트를 고정했을 때 남은 독립 운동 수
  (관계 잔차의 자코비안 계수로 계산). 고정 = 0, 관계 없음 = 6/3. 예: 면 Mate → 3(병진 2 + 회전 1), Insert·Rotational → 1, Rigid → 0.
- `session.component_status(item) -> Option<gc_doc::ComponentStatus::{Full, Under, Unknown}>` — 브라우저 점(검은/속 빈/대시).
  Unknown = 그 컴포넌트에 걸린 관계가 만족되지 않음.
- `session.relationship_status(id) -> Option<gc_doc::RelationStatus::{Full, Under, Sick}>` — Relationships 폴더 행 표시용(원문의 점은
  컴포넌트 단위라는 점 주의).
- 테스트 `gc_doc/tests/relate.rs::remaining_freedom_and_status_points_follow_the_relationships`.

## 2026-11-18 — QA 버그 2건(원점 표시), Unsolvable 메시지

- 리드 브랜치 병합. `qa_origin_folder`의 두 `#[ignore]` 해제: ① 이미 보이는 원점 요소를 다시 보이면 중복 저장 → 처음 보인 순서대로 한 번만,
  ② 삭제된 컴포넌트의 `origin_visible`이 true → `has_occurrence` 확인 + 모델 트리 상태를 쓸 때 없는 컴포넌트의 상태(고정·비활성·숨김·억제·원점 표시)를
  정리(삭제를 되돌리면 그 쓰기 이전 상태가 함께 돌아옴).
- `EditError::Unsolvable{ids, names}` — Display: "these assembly constraints cannot be satisfied together: Mate:1, Flush:2"(이름 포함).
  UI 문구로 쓰려면 `names`를 i18n 문장에 넣으면 된다.

## 2026-11-18 — 매개변수 이름 바꾸기(리드 요청)

- `Command::RenameParam{id, name}`: 이름 형식·중복 검사(`EditError::BadName`), 이 매개변수를 쓰는 다른 매개변수와 모든 스케치 치수의 수식 안
  이름을 같은 트랜잭션에서 바꿈(구문 트리의 이름 위치만 바꾸므로 `w`가 `width`의 일부일 때 등은 건드리지 않음), undo 1단계, 값은 그대로.
  세션 래퍼 없이 `session.execute(&Command::RenameParam{..})`(매개변수 명령은 원래 execute로 씀). 테스트 `gc_doc/tests/document.rs`.
- 모델 매개변수(d0, d1 …, 치수를 수식에서 참조) 검토: 가능하다. 제안 — 치수마다 문서 전역 이름(`Dimension.name`, 새 치수에 다음 `dN`)을
  두고 `scope_of`에 치수 값을 넣으며, `propagate`의 의존 순서에 치수↔매개변수를 함께 넣는다(순환 금지). 스케치 저장 형식에 필드 하나 추가
  (serde 기본값으로 기존 파일은 열 때 이름 부여). 인벤터 원문(매개변수 대화상자) 확인 후 진행 — 지금 구현하지 않았다.

## 2026-11-18 새벽 2 — 어셈블리 P1-11 Rigid·Rotational 조인트, 로봇 마운트, 툴→플랜지

- `RelationKind::{Rigid{flip}, Rotational{flip}}` — 구속과 같은 `Relationship`/`add_relationship`로 추가, 같은 솔버로 풀림.
  조인트 원점 `Geometry::Frame{origin, z, x}`(컴포넌트 프레임), `Geometry::origin_frame()` = 컴포넌트 자기 원점(중심점+Z+X).
  평면·원·축·점도 원점으로 쓸 수 있다(Z = 법선/방향, X = 컴포넌트 X를 수직화).
- 의미: Rigid = 두 원점 일치 + Z 반대(flip이면 같은 방향) + X 같은 방향 → 모든 자유도 제거. Rotational = 원점 일치 + Z 정렬, Z축 회전 자유.
  Automatic = 원형 원점이면 Rotational, 아니면 Rigid(`session.automatic_joint(&a, &b)`).
- **로봇 마운트**: `Selection{occurrence: Item(robot), geometry: Geometry::origin_frame()}`를 설비의 원점/작업 프레임에 Rigid로 → 설비를
  옮기면 로봇이 따라감(테스트는 바디로; 로봇도 같은 owner 경로).
- **툴→플랜지**: 툴은 아직 별도 컴포넌트가 아니라(`Robot.tool`) `session.mount_tool(robot, origin: Geometry, flip) -> Result<(), EditError>`가
  같은 Rigid 규칙으로 툴 배치를 계산한다(`Geometry::rigid_mount(flip) -> DPose3`). 툴 원점 = 툴 장착면 중심, Z는 면 밖(플랜지 쪽).
- 원문 확인(2026-11-18): To Define and Manage Joint Relationships `GUID-21DC3336-5C51-42C1-90FB-4299CD66E0C6.htm`, Create Joints Reference
  `GUID-6AA68E8F-7C97-4806-8483-3941DE915E70.htm`. 맞음: 첫 원점 = 움직이는 컴포넌트(두 번째로 이동), Rigid는 한계 없음, Flip component =
  방향 반전. **고침**: Automatic은 두 원점이 **모두** 원형일 때만 Rotational(전에는 한쪽만이어도). 원문에 기본 Z 방향 규정은 없다 —
  우리 기본(Z 반대, 면 대 면)은 유지. 아직 없는 것: Gap 값, Align 1/2·Invert alignment, Lock/Protect, Cylindrical·Ball·Slider·Planar, 한계.
- 아직: 각도 한계(Angular Start/End)·Drive, Slider·Cylindrical·Ball, 링크 프레임(플랜지·TCP) 선택, 툴의 독립 컴포넌트화.
- 테스트: `gc_doc/tests/relate.rs`(마운트 추종·Rigid 복귀·flip, Rotational 회전 유지, Automatic, 툴 마운트).

## 2026-11-18 새벽 — 어셈블리 P1-9 Mate·Flush·Insert(+Predict Offset) 솔버

근거(원문 확인, `https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/`): To Place Mate or Flush Constraints
`GUID-0F500089-4F69-41C5-AFE8-B58AC9E8E15E.htm`, To Place Insert Constraints `GUID-97DD5557-FAEE-46FA-845D-11123D725BA9.htm`,
Place Constraint Reference - Assembly Tab `GUID-EBD2C784-0E71-452A-B5F2-22738D22D45A.htm`.

- 엔티티 `gc_doc::Relationship{name, kind, first: Selection, second: Selection}` — **gc_core `Kind::Relationship`(StableHash 18) 추가, 리드 확인 부탁**.
  `Selection{occurrence, geometry}`, `Geometry::{Plane{point, normal}, Axis{point, direction}, Point, Circle{centre, normal}}`(컴포넌트 자기 프레임 =
  원점 폴더 프레임; 부품이면 부품 프레임), `Geometry::origin(OriginElement)`로 원점 평면·축·점 선택.
  `RelationKind::{Mate{offset, sense: AxisSense::{Opposed, Aligned, Undirected}}, Flush{offset}, Insert{offset, aligned}}`.
- 의미(원문 그대로): Mate = 면 대 면(법선 반대) + 오프셋, 곡선·축·점은 일치; Flush = 법선 같은 방향; 두 축 Mate는 Opposed가 기본;
  Insert = 원형 모서리 축 일치 + 평면 Mate, 축 회전 자유, Opposed 기본. Predict Offset and Orientation = 평행하고 법선이 반대면 Mate,
  아니면 Flush, 현재 거리를 오프셋으로(`predict_relationship`).
- 풀이: 움직이는 것은 선택 기하를 가진 컴포넌트(바디·로봇·어셈블리; 부품은 그 어셈블리와 함께). 고정(grounded) 컴포넌트는 그대로.
  모든 관계를 한꺼번에 감쇠 최소제곱(자세 6자유도/컴포넌트, 결정적)으로 풀고, 만족하는 자세 중 가까운 쪽을 고른다(덜 구속된 방향의 이동은 유지).
  방향이 반대로 놓인 경우는 먼저 반 바퀴 돌려 안장점을 피한다.
- 갱신: 관계 추가·수정, 그리고 배치를 바꾸는 명령(SetBody·PlaceRobot·PlaceAssembly) 뒤에 다시 푼다. 고정 컴포넌트를 옮기면 묶인 것이
  따라오고, 구속된 컴포넌트를 구속 방향으로 끌어내면 돌아온다(원문 Free Move의 Update 복귀). 만족 못 하면 `EditError::Unsolvable(관계들)`로
  거부, 문서 그대로. 관계가 쓰는 컴포넌트는 삭제 거부(InUse).
- 세션 API(에이전트 노출용):
  `session.add_relationship(Relationship) -> Result<Id<Relationship>, EditError>`,
  `session.set_relationship(id, Relationship) -> Result<(), EditError>`, `session.delete_relationship(id) -> Result<(), EditError>`,
  `session.relationships() -> Vec<(Id<Relationship>, &Relationship)>`,
  `session.predict_relationship(&Selection, &Selection) -> Option<RelationKind>`.
  프론트엔드: 면 피킹 결과를 컴포넌트 프레임 좌표로 넣으면 된다(`component_origin(occ).pose.inverse()`로 변환). `Unsolvable`은 지금 UI에서 ErrOther.
- 아직: 한계(Maximum/Minimum/Resting), 관계 억제, 구속 상태 점·DOF 표시(P1-10), Angle·Tangent, Drive. 테스트 `gc_doc/tests/relate.rs`(6),
  `gc_persist/tests/components.rs`(저장·열기).

## 2026-11-17 밤 2 — 어셈블리 P1-2 컴포넌트 원점 폴더(+P1-4 일부), 리드 b02ac34 병합

- `Document::component_origin(occurrence) -> Option<ComponentOrigin{pose}>`; `ComponentOrigin::{point(), axis(0|1|2), plane(OriginPlane)}`.
  바디·로봇(베이스)·어셈블리는 placement, 부품은 파일의 부품 행렬(배율이 있으면 정규직교화) × 어셈블리 placement.
  평면 방향은 문서 원점 평면과 같다(XY 법선 +Z, XZ 법선 −Y, YZ 법선 +X).
- 참조: `PlaneRef::Component(occurrence, OriginPlane)`, `PointRef::Component(occurrence)`(작업 피처 기준),
  `PlaneSource::Component{occurrence, plane}`(스케치 평면, 컴포넌트를 따라 움직임, 쓰는 동안 그 컴포넌트 삭제 거부).
- 세션: `session.component_origin(occurrence) -> Option<gc_doc::ComponentOrigin>`,
  `session.create_sketch_on_component(occurrence, plane: gc_doc::OriginPlane) -> Result<Id<Sketch>, EditError>`.
  원점 요소 표시(프론트엔드 요청): `gc_doc::OriginElement::{Plane(OriginPlane), Axis(0..=2), Point}`,
  `session.set_origin_visible(occurrence, &[OriginElement], bool) -> Result<(), EditError>`(여러 개를 한 undo 단계, 저장됨, 기본 숨김),
  `session.origin_visible(occurrence, element) -> bool`. 폴더 전체 토글은 7개를 한꺼번에 넘긴다.
- 근거(원문 확인): A52 About Reference Planes `GUID-991276D7-D2BF-4266-9A57-E49146DC760C.htm`(모든 파트·어셈블리에 원점에서 만나는
  숨은 작업 평면 + 중심점, 표시하면 와이어·셰이딩 모두 보임, 평면 선택 후 Sketch), A53 To Display Reference Planes
  `GUID-7FB44EFA-4D72-4D9B-8318-8294DE054EC7.htm`(Origin 펼침 → 평면 선택 → Visibility, 모서리 드래그로 크기·이동, 중심점은 Project Geometry로
  스케치에 투영). 명세와 차이 없음. 크기·이동 드래그와 중심점 투영(P1-5)은 아직.
- 테스트: `gc_session/tests/sketch_planes.rs::every_component_has_an_origin_and_a_sketch_on_its_plane_moves_with_it`.

**구동(Drive) 제안 — 리드 요청**: 어셈블리 구속·조인트가 생기면 `session.drive_relationship(id: Id<Relationship>, value: f64)`로
구속의 오프셋/각도를 문서를 바꾸지 않고 미리보기 값으로 바꿔 풀고(리드의 `drive_show`와 같은 CellState.drive 계층),
`end_drive()`로 원래 값에 돌아간다. 값 범위는 조인트 한계(있으면)로 자르고, 풀 수 없는 값이면 직전 자세를 유지하고 `Err(Unsolved)`.
로봇 관절은 구속 솔버가 풀지 않으므로(명세 4절) 로봇 마운트 조인트 구동은 로봇 전체를 옮기는 것으로 끝난다.

## 2026-11-17 밤 — 어셈블리 P1-1 컴포넌트 계층·상태

리드 브랜치(93a1a73) 병합 후 진행. 근거: Inventor 2026 도움말 A11 Place Components(첫 컴포넌트 자동 고정),
A17 Restore DOF to grounded components, A56/A57 Visibility·Enabled, A60 Suppress
(`https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/` + docs/ux/inventor-assembly-parity.md 출처 표의 파일명).

- 모델: `gc_doc::Occurrence::{Item(AnyId) /* Body·Robot·Assembly */, Part{assembly, index}}`,
  `ComponentFlag::{Grounded, Visible, Enabled, Suppressed}`. 상태는 `Timeline.components`(grounded·disabled·부품 hidden·부품 suppressed)에
  저장. 바디·로봇·어셈블리의 가시성은 기존 `Look.hidden`, 어셈블리 억제는 기존 `Timeline.suppressed` 그대로(같은 API로 보임).
- 첫 컴포넌트(바디·로봇·어셈블리 중 처음 추가된 것)는 자동 고정. 같은 명령 안이라 실행 취소하면 고정도 사라진다.
- 명령: `Command::SetComponent{occurrence, flag, on}` — 실행 취소 1단계. 바디·로봇 억제는 아직 거부(BadValue: 장면·계획의 바디/로봇 반복이
  여러 곳이라 별도 작업). 툴 occurrence는 조인트(P1-11)와 함께.
- 질의: `Document::{components() -> Vec<ComponentNode>, component_flag, has_occurrence, parts_shown(assembly, for_obstacles)}`.
  `ComponentNode{occurrence, parent, name, grounded, visible, enabled, suppressed}` — 평평한 목록, 부모가 먼저(프론트엔드가 트리로).
- 효과: 숨긴 부품은 그리지 않지만 장애물로 남음, 억제한 부품은 둘 다 빠짐, 부모가 숨김/억제면 자식도. 고정은 어셈블리 구속(P1-9)이
  쓸 상태(지금 기즈모 이동은 막지 않음). Enabled=false는 표시 쪽이 배경 스타일·선택 불가로 처리.
- 세션 API(에이전트 도구로도 노출 가능, PLAN 9.8):
  `session.components() -> Vec<gc_doc::ComponentNode>`,
  `session.set_component(occurrence: gc_doc::Occurrence, flag: gc_doc::ComponentFlag, on: bool) -> Result<(), EditError>`.
- 테스트: `gc_doc/tests/design.rs`(첫 고정·플래그별 undo·부모 상속·없는 부품 거부), `gc_persist/tests/components.rs`(저장→열기 동일).
- 다음: 원점 폴더(P1-2) `component_origin(occurrence) -> {planes, axes, point}`, 스케치 P1 잔여, Mate/Flush/Insert·조인트.

## 2026-11-17 저녁 — 인벤터 스케치 P1 (BE 몫): 도구 API 합의와 Trim·Extend·Fillet

프론트엔드와 합의한 도구 API(메시지로 합의, 여기 기록). UI가 `Prompt`·`SnapKind`·`GeoConstraint`·`Solid`를 빠짐없이 match하므로
이 enum들에는 변형을 추가하지 않았다. 새 정보는 필드로 준다. 프론트엔드 요청: 앞으로 `Prompt`에 변형을 더할 수 있게 UI 쪽 match에
`_ =>` 갈래를 넣어 달라(그 전까지 새 도구는 기존 Prompt를 쓴다: Trim·Extend·Fillet → `ConstraintPick`).

| 항목 | API |
|---|---|
| 새 도구 | `ToolKind::{Arc3, ArcTangent, Rectangle3, Trim, Extend, Fillet}` |
| 이벤트 | `ToolEvent::{DoubleClick, Command(ToolCommand), Hud{field, value}}`, `Key::{Backspace, Enter}` |
| 명령 | `ToolCommand::{Close, Done, Alternate(bool)}` — `Alternate`는 Shift 누름/뗌: Trim↔Extend 임시 전환 |
| HUD | `session.tool_hud() -> Vec<HudSlot{field, measured, locked}>`, 값 입력은 `ToolEvent::Hud`. 잠근 값은 치수가 된다(P1-6) |
| 추론 | `Snap::inference: Option<Inference>`, `Preview::alignments`(점선) |
| 구성선 | `session.tool_construction()`, `set_tool_construction(bool)`; 이미 그린 곡선은 `SketchOp::SetConstructionOf` |
| 트림 미리보기 | `Preview::cut: Vec<Piece>` — 클릭하면 사라질 조각(강조 표시). Extend가 더할 조각과 Fillet 호는 `Preview::lines/arcs` |
| Fillet 반지름 | `HudField::Radius` 한 칸이 항상 보인다(기본 "2 mm", 쓴 값은 다음 모깎기까지 유지) |

**Trim**: 호버 시 미리보기, 클릭 = 양쪽 가장 가까운 교차 곡선 사이 조각 제거. 잘린 끝은 자른 곡선에 묶인다(그 곡선의 점에 닿으면
`Coincident`, 아니면 `PointOnCurve`). 가운데를 자르면 선은 두 조각 + `Collinear`, 호는 두 조각 + 중심 `Coincident` + `Equal`.
원은 교차 2개 이상이면 호가 된다(곡선 id·치수 유지). 교차가 없으면 곡선 삭제. 누른 채 드래그하면 지나간 곡선을 각각 그 자리에서 트림.
선의 수평/수직 구속은 남고, 잘린 끝에 걸린 구속·치수와 길이에 기대는 `Equal`·`Midpoint`는 지워진다.
**Extend**: 커서에 가까운 끝을 선/원의 연장선이 처음 만나는 곡선까지. 다른 곡선과 이어진 끝은 거부.
**Fillet**: 선 두 개 클릭(또는 두 선이 만나는 모서리 점 클릭) → 접하는 호 + 접선 2 + 끝점 일치 2 + 반지름 치수(입력한 글자 그대로).
두 선은 교차점에서 클릭한 쪽을 남기고 호까지 트림·연장. 지금은 선-선만(호·원 모깎기는 아직).
SketchOp: `Trim{curve, at}`, `Extend{curve, at}`, `Fillet{lines: [(curve, at); 2], radius: String}`. 기하는 `gc_sketch::cut`
(`Sketch::{trim, extend, fillet, trim_preview, extend_preview, fillet_preview}`).
시나리오 테스트: `gc_doc/tests/sketch_ops.rs`(자유도 포함 6개), `gc_session/tests/inventor_sketch.rs`(p1_8 3개).

남은 P1(BE): P1-12 과구속 시 Accept 흐름(지금은 기존 참조 치수 제안 경로, `SetDriving`은 있음), Arc3·ArcTangent의 HUD, Ctrl 경계 지정.

## 2026-11-17 오후 — QA 버그·트리 API·작업 평면 스케치·S2 품질

**QA 버그 3건(수정, 테스트의 `#[ignore]` 제거)**: ① UR IK가 팔을 곧게 위로 세운 자세(손목 중심이 d4 원기둥 위)를 반올림 때문에
도달 불가로 판정 → 허용오차 ② 툴은 별도 바디("tool"): tip 링크와 장착 링크만 허용, 나머지 링크와는 검사(영자세 자동 허용 없음),
tip 링크의 접촉 허용은 툴에도 적용 ③ `library::scan`이 `..`·절대 경로 description을 거부. `StateChecker::motion_cost` 전달 완료.

**프론트엔드용 트리 API**

| 요청 | API |
|---|---|
| 이름 바꾸기(F2) | `Command::Rename { item: AnyId, name }` — 앞뒤 공백 제거, 같은 종류의 이름 중복이면 `EditError::NameTaken`, 파라미터는 거부(수식이 이름으로 참조) |
| 가시성 전 노드 저장 | `Command::SetVisible { item, visible }`, 읽기 `doc.is_visible(item)` (바디·로봇·설비는 look, 나머지는 트리 상태에 저장) |
| 억제 | `CellOp::SetSuppressed { item, suppressed }` — 스케치·작업 피처·피처·설비 + **웨이포인트·경로**. 읽기 `doc.is_inactive(item)`(억제 또는 롤백 뒤). 억제한 웨이포인트는 계획에서 건너뛰고, 억제한 경로는 계획 거부 |
| 삭제 시 종속 안내 | `doc.dependents_of(item) -> Vec<AnyId>` (간접 포함, 직접 먼저). 종속이 있는 삭제는 커밋에서 거부됨 |
| 순서 바꾸기 | `CellOp::MoveInTree { item, before }` — 종속 위반 시 `EditError::InUse`; 순서 읽기 `doc.model_order()` |
| 롤백 위치 | `doc.rollback_index() -> Option<usize>`(model_order에서 마커가 선 항목의 위치), 옮기기 `CellOp::SetRollback { after }` |
| 3D 스케치 점 드래그 | `session.spatial_drag_start(point, cursor3) -> bool`, `spatial_drag_move(cursor3) -> Option<FrameOutcome>`, `spatial_drag_end()`(실행 취소 1단계), `spatial_drag_cancel()`. 미리보기는 `point_pos`. 하위: `Sketch::begin_drag_3d`, `SketchDrag::update_3d`, `gc_solver::Grab::Point3`·`begin_drag_3d`·`update_3d` |

**프론트엔드와 합의한 세션 래퍼**(항목 id는 모두 `gc_core::AnyId` — `CellItem`/`SketchRef` 대신 하나로; `Id::erase()`로 얻음):
`session.rename(item, name)`, `set_visible(item, bool)`/`is_visible(item)`, `set_suppressed(item, bool)`/`is_suppressed(item)`(억제 또는 롤백 뒤),
`dependents_of(item) -> Vec<AnyId>`, `model_order()`, `move_item(item, before: Option<AnyId>)`, `rollback_index()`/`set_rollback(Option<usize>)`,
`spatial_drag_start/move/end/cancel`. 이름 중복은 같은 종류 안에서만 금지. `gc_session`이 `Guarantee, LinkPair, PairReason, StateProblem`을 re-export.

**13 임의 면·작업 평면 위 스케치**: `session.create_sketch_on_work_plane(id)`, `session.create_sketch_on_face(assembly, part, mesh, triangle)`
(피킹한 삼각형: 원점=삼각형 중심, 법선=앞면, X=첫 모서리). 스케치 평면 프레임이 `Frame::source`(`PlaneSource::Work | Face`)로 붙어
작업 평면·설비가 움직이면 따라가고, 저장·재열기 후에도 유지, 쓰는 동안 그 작업 평면·설비는 삭제 거부, 스케치를 지우면 프레임도 지워짐.
문서 명령 `Command::CreateSketchOn { name, source }`. 작업 피처 위치 계산은 `doc.work_shape(id)`(세션 `work_shape`는 위임).

**S2 경로 품질**: 부분(조인트별) 숏컷 추가(`gc_plan::partial_shortcut`, 파이프라인·벤치 공통 `gc_motion::simplify_path`). S2 RRT-Connect
20시드: 중앙값 2.50 → 1.90(최선 대비 1.07배), best-of-4 변동 2.9 %. 전체 보고서는 재측정 후 갱신.

**확정된 규칙(리드)**: 프로젝트의 `Robot.urdf` 사본이 기구학의 기준, 메시만 라이브러리에서 찾는다 — 현재 구현이 그렇게 동작(세션은 문서의 urdf로 모델을
만들고 `resource_dir`/`library://`로 메시만 찾음). 라이브러리 패키지와 다를 때 알림은 아직 없음.

## M2-1 (1차)

| 항목 | 위치 | 상태 |
|---|---|---|
| GLB/glTF 읽기 | `gc_plugin_mesh::read_gltf(bytes, base_dir) -> Assembly { nodes(이름·부모·로컬 행렬·메시), meshes(정점·삼각형·색), warnings }` | 완료. 인스턴싱 유지(같은 메시는 한 번), Y-up → Z-up, 점·선 프리미티브는 경고 후 건너뜀 |
| 문서: 불러온 설비 | `gc_doc::Assembly { name, file, placement, parts: Vec<AssemblyPart{name, parent, matrix, meshes}>, meshes: Vec<AssemblyMesh{name, color, triangles}>, look }` | 완료. 고정 참조(부품 편집 없음), 삼각형은 파일에 남김. `CellOp::{AddAssembly, PlaceAssembly, SetAssemblyLook, DeleteAssembly}` |
| 문서: 작업 피처 | `gc_doc::WorkFeature { name, geometry: WorkGeometry::{PlaneOffset, PlaneThroughPoints, AxisThroughPoints, AxisNormalToPlane, Point}, visible }` | 완료. 월드 위치: `session.work_shape(id) -> Option<WorkShape::{Plane(pose), Axis{point, direction}, Point}>`(정의가 깨지면 None) |
| 문서: 돌출 | `gc_doc::Feature { name, kind: FeatureKind::Extrude(Extrude { sketch, profile(곡선 id, 비면 모든 닫힌 루프), extent: Extent::{Distance, Symmetric, TwoSided}, reversed }) }` | 완료(정의·검증·저장). 볼륨 계산(닫힌 루프 추출)은 M2-2 필드와 함께 |
| 모델 트리 순서 | `doc.model_order()`, `doc.is_inactive(id)`, `CellOp::{MoveInTree, SetSuppressed, SetRollback}` | 완료. 종속을 거스르는 이동은 `EditError::InUse`, 새 항목은 롤백 마커 바로 뒤에 들어가고 마커가 따라감 |
| 세션 | `add_assembly(assembly, meshes)`, `import_assembly(file, reader)`(작업 스레드) + `import_status()`, `set_assembly_reader(reader)`(프로젝트 다시 열 때), `assembly_parts(id) -> Vec<ShownAssemblyPart{part, matrix, mesh, color}>` | 완료. 활성(억제·롤백 아님) 설비 부품은 계획·자세 검사의 장애물, 계획을 Stale로 만듦. 메시가 없으면 계획은 거부(설비를 무시하지 않음) |
| STEP 스파이크 | 아래 | 조사만(실행 못 함) |
| 기본 도형 | 변경 없음 | 기존 문서 그대로 열림 |

### 앱 배선(리드/프론트엔드) — 플러그인은 도메인에서 부를 수 없어 앱이 이어 준다

```rust
// gc_plugin_mesh::Assembly → (gc_doc::Assembly, Vec<gc_mesh::MeshData>)
fn to_doc(path: &str, a: gc_plugin_mesh::Assembly) -> Result<(gc_doc::Assembly, Vec<gc_mesh::MeshData>), String> {
    let meshes = a.meshes.iter().map(|m| gc_mesh::MeshData::new(m.positions.clone(), m.indices.clone()).map_err(|e| e.to_string())).collect::<Result<Vec<_>, _>>()?;
    let doc = gc_doc::Assembly {
        name: file_stem(path), file: path.to_owned(), placement: Default::default(),
        parts: a.nodes.iter().map(|n| gc_doc::AssemblyPart { name: n.name.clone(), parent: n.parent.map(|p| p as u32), matrix: n.matrix, meshes: n.meshes.iter().map(|m| *m as u32).collect() }).collect(),
        meshes: a.meshes.iter().map(|m| gc_doc::AssemblyMesh { name: m.name.clone(), color: m.color, triangles: (m.indices.len() / 3) as u32 }).collect(),
        look: Default::default(),
    };
    Ok((doc, meshes))
}
// 불러오기: session.import_assembly(path, Arc::new(|p| to_doc(p, gc_plugin_mesh::read_gltf(&std::fs::read(p)?, dir(p))?)))
// 열기 전: session.set_assembly_reader(Arc::new(|p| Ok(to_doc(p, read_gltf(..)?)?.1)))
```

### 리드에게 요청(M2-1)

1. **계약 제안**: 설비 리더를 확장점으로 — `gc_plugin_api`(또는 새 api)에 `ASSEMBLY_READERS: ExtensionPoint<dyn AssemblyReader>`,
   `trait AssemblyReader { fn extensions(&self) -> &[&str]; fn read(&self, bytes: &[u8], base_dir: Option<&Path>) -> Result<AssemblyData, String>; }`,
   `AssemblyData`는 위 `gc_plugin_mesh::Assembly`와 같은 평범한 자료. 그러면 앱 배선 없이 세션이 레지스트리로 GLB·STEP·OBJ를 고른다.
2. **gc_core `Kind`에 4개 추가**(WorkFeature, Feature, Assembly, Timeline; `StableHash` 번호 14–17) — foundation 크레이트라 검토 바람.
   추가만이고 도메인 밖에는 영향 없음(전체 빌드·테스트 통과).
3. 렌더러(gc_scene)·브라우저(gc_panels): `session.assembly_parts(id)`로 그리기, `doc.model_order()`/`is_inactive`로 트리·억제·롤백 표시.

### STEP 스파이크(step-io 0.3.0) — 문서·API 조사, 실행은 못 함

- MIT OR Apache-2.0, 2026-10-21 릴리스, 의존성 트리 16개(가벼움), 자체 표기 "Experimental — breaking API changes at any time".
- 읽기: 주요 AP 전부(AP203/214/242) → 단일 스키마 모델 + 보고서(고친 것·버린 것과 이유). `model.scene()`으로 솔리드 → 면(곡면 종류: 평면·원통·원뿔·구·토러스·B-스플라인·오프셋 …) → 경계 → 모서리(직선·원·B-스플라인 곡선 …) 순회.
- 어셈블리: `ProductDef::children()`, `occurrences()`(공유 정의의 여러 인스턴스 배치 지원), 색(`color()`), 단위(`units()`).
- **테셀레이션 없음**: 파일에 AP242 테셀레이션 표현이 들어 있을 때만 `meshes()`/`triangles()`로 삼각형을 준다. 일반 B-rep은 면별 테셀레이션을 우리가 구현해야 한다(평면 다각형 → 이어 붙이기, 원통·원뿔·구는 매개변수 격자, B-스플라인은 균일 매개변수 격자 + 트리밍 루프 클리핑). 계획서 판단(자체 구현)과 맞다.
- 미확인: 실제 파일 읽기 속도·견고성. 샘플(NIST CTC) 다운로드 호스트가 이 환경의 네트워크 허용 목록 밖이고, 내려받은 크레이트의 테스트 실행은 권한 정책상 하지 않았다. **결정 전 사용자 PC나 허용된 환경에서 NIST CTC 01–05로 읽기·시간 측정 필요.**
- 제안: M2-1은 GLB를 주 경로로 확정, STEP은 (1) 테셀레이션 표현이 있는 AP242 파일만 먼저 지원(step-io의 `meshes()` 그대로), (2) B-rep 테셀레이터는 평면·원통부터 별도 작업으로.


브랜치 `claude/ur-robot-control-simulator-jsmhze-backend`. 리드 브랜치(70b995b, `robots/` 라이브러리)를 병합해 두었다.

## 한 일

| 항목 | 위치 | 비고 |
|---|---|---|
| Tier B 벤치 `cargo xtask bench-quality [--seeds N] [--only S2] [--out F]` | `crates/domain/gc_motion/examples/bench_quality.rs`, `xtask/src/bench.rs` | xtask에 의존성을 늘리지 않으려고 벤치 본체는 gc_motion 예제. 결과 `docs/quality/report-<날짜>.md` |
| 장면 S1–S4 | `fixtures/scenes/*.scene` | 부록 E 설명대로 작성(Stage 1 픽스처가 없었음). **리드 검토 필요** |
| 상태 검사 2배 빠르게 | `gc_collide` | 파트 쌍당 `closest_points` 한 번, 경계 상자 갱신 단순화(UR5e 28 → 14 µs) |
| 정적 링크 ↔ 장애물 쌍 제외 | `gc_motion::RobotScene` | 테이블 위 로봇이 계획 여유 때문에 전부 무효가 되던 문제 |
| 플래너 개선 | `gc_plan` | RRT/RRT* 목표 greedy connect, RRT*는 첫 경로 뒤부터 재배선, Lazy PRM(배치 추가) |
| 목표 순위 | `gc_motion::rank_goals`, `near_goals` | IK 분기당 가장 가까운 턴 변형 먼저(5.8), 가까운 목표 먼저 계획(예산 절반씩) |
| 먼 목표 직선 이동 | `gc_motion` | 가장 가까운 목표가 막혔을 때 다른 목표로의 직선 이동은 계획 결과와 길이로 경쟁 |
| 재계획 사다리(5.8) | `gc_motion::replan`, 세션 `plan()` | ① 같은 종류·같은 시작·같은 목표·유효 → 유지(`MotionPlan::reused`) ② 아니면 이전 끝 분기·턴 우선. 우회 경로는 직선이 새로 가능하면 유지 안 함. 플래너·시드를 바꾸면 유지 안 함 |
| 보증(5.7, Q2) | `gc_motion::Guarantee`, `MotionPlan::guarantee`, `PlannedPath::guarantee()` | 검사 상태 사이에서 어떤 점도 패딩 이상 움직이지 않게 세분 검사 |
| 접촉 허용(5.7) | `gc_motion::ContactAllowance`, 문서 `MotionPath::contacts` | 웨이포인트 반경(기본 50 mm) 또는 세그먼트 전체에서 그 쌍만 패딩 0, `allow_touch`면 겹침도 허용 |
| ACM 목록·켜기/끄기 | `RobotScene::link_pairs`, 문서 `Robot::acm_overrides` | 사유: 인접 / 영자세에서 닿음 / 검사 |
| TCP 구 모드(5.8) | `gc_motion::plan_sphere` | 같은 플래너·숏컷·재검증·타이밍 |
| 플래너 레지스트리(4.3) | `gc_plugin_planners::PlannersPlugin`, `gc_motion::plan_motion_using` | 문서 id 그대로(`rrt_connect` …). 세션 `set_planners` |

측정 결과와 해석: `docs/quality/report-2026-11-17.md`(수정 전 기준선: `docs/quality/baseline-2026-11-17.md`).

## 프론트엔드가 쓸 세션 API

- **보증**: `session.planned(path)?.guarantee()` → `Guarantee { padding_nominal(m), padding_min(m), step_deg, step_mm, clearance(m), states, model, acm_hash }`.
  `holds_between_states()`가 참이면 문구: "패딩 {padding_nominal·1000} mm, 검사 간격 {step_mm} mm·{step_deg}°에서 볼록 충돌 모델이 장애물과
  {clearance·1000} mm 이상 떨어짐. 제외: 검사하지 않는 쌍(ACM), 닫힌 메시 안에서 시작한 바디, 컨트롤러 추종 오차."
  `padding_min < padding_nominal`이면 "접촉 허용 구간에서는 패딩 0". 세그먼트별 값은 `segments[i].guarantee`.
- **재사용 표시**: `segments[i].reused`가 참이면 "유효, 재최적화 안 함".
- **ACM**: `session.link_pairs(robot) -> Option<Vec<LinkPair { a, b, reason: PairReason::{Adjacent, TouchingAtZero, Checked}, checked }>>`,
  `session.set_pair_checked(robot, a, b, checked)`(실행 취소 한 단계, 기본값과 같으면 재정의 삭제). 바꾸면 계획이 Stale.
- **접촉 허용**: `session.allow_contact(path, index, link, obstacle_body_id)`(웨이포인트 반경 50 mm, 겹침 금지). 다른 옵션은
  `CellOp::SetContacts { id, contacts: Vec<gc_doc::ContactAllowance { index, link, obstacle, whole_motion, radius, allow_touch }> }`.
  `GoalInvalid(StateProblem::WithinPadding { a, b, gap })` 오류가 나면 "a ↔ b 접촉 허용 추가"를 제안하면 된다(`Collision`이면 겹침).
- **구 모드**: `gc_motion::plan_sphere(&obstacles, &SphereRequest { radius, padding, start, goal, algorithm, options, seed, runs, max_vel, max_acc }, &control)`
  → `SpherePlan { path, trajectory, length, straight, stats }`. 세션 래퍼는 아직 없음(화면 설계가 정해지면 추가).
- **플래너 목록**: `session.planner_choices() -> Vec<(id, 이름 ko, 이름 en)>`.

## 리드에게 요청

1. **계약 변경(gc_robotics_api)**: `StateChecker`에 기본 메서드 추가
   `fn motion_cost(&self, _a: &[f64], _b: &[f64]) -> u64 { 1 }` — 모션 검사 한 번이 보는 상태 수. 없으면 플러그인 플래너의 예산
   (`max_state_checks`)이 모션 검사를 1로 세어 내장 경로와 의미가 달라진다. 반영되면 `gc_plugin_planners`의 어댑터와
   `gc_motion::SharedChecker`가 전달하도록 바꾸겠다(그때까지 세션은 내장 id를 `gc_plan`으로 직접 실행).
2. **앱 배선(gc_app)**: `builtin_plugins()`에 `gc_plugin_planners::PlannersPlugin` 추가, `registry.get(MOTION_PLANNERS)`를
   `session.set_planners(...)`로 전달. (gc_app에 `gc_plugin_planners` 의존성이 필요하면 루트 매니페스트도.)
3. 장면 픽스처 S1–S4와 부록 E 임계값 해석(Q3를 기본 플래너 기준으로 볼지) 확정.
4. DEVIATIONS 2026-11-17 "플래너 변형과 재계획 사다리 세부" 항목 검토.

## 남은 일(백엔드)

- RRT·RRT*·Informed RRT*가 S3(기계 문)에서 성공률이 낮다(보고서 참조). 단일 트리라 좁은 개구부로 들어가기 어렵다. 후보: 목표 쪽에서도
  자라는 양방향 RRT*, 또는 RRT-Connect 해로 초기화한 뒤 최적화.
- Q4 기준 비용(`bench-quality --reference`, Informed RRT* 200만 검사 × 8 시드) 미구현. 지금 보고서는 "모든 실행 중 최선" 대비 비율.
- Q11(웨이포인트 1 mm 이동 시 분기 유지·비용 변화 ≤ 5 %)을 벤치에 넣기. 사다리 ②로 분기는 유지된다(테스트 있음).
- ACM 표본 분류(Always/Never/Sometimes(p), 10,000 표본)와 `gen-ur --acm` 골든은 아직 없음(지금 사유는 인접·영자세 접촉만).
- 정적 링크 제외의 대가: 베이스가 장애물에 묻혀 있어도 경고하지 않음 → 로봇 배치 시 한 번 검사해 경고하는 API가 있으면 좋겠다.
- 예산 초과: 한 반복의 모션 검사 때문에 예산을 최대 ~8 % 넘길 수 있다(보고서의 max 열).
