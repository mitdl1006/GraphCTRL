# 백엔드 인계 (최종 갱신 2026-11-18)

브랜치 `claude/ur-robot-control-simulator-jsmhze-backend`. 아래 "현황"이 요약, 그 다음 "작업 기록"이 최신 순 상세(근거 링크·API·차이).
인벤터 원문은 `https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-….htm`.

## 인수인계 (2026-11-18, 이전 백엔드 세션 → 새 백엔드 세션)

### 지금 상태

- 진행 중인 작업 없음. WIP 없음. 마지막 커밋은 0f749eb다.
- 리드 브랜치 77b81f9를 병합했다(5dd4a32). 병합 후 workspace check와 clippy가 통과했다.
- 리드 지시 ①②③을 모두 끝내고 푸시했다.
  - ① 문서 설정: a9a0473.
  - ② BOM 기초: 94c69ad.
  - ③ Bend Equal: 5df259d.
- 0f749eb: Tangent·Smooth에서 같은 곡선 쌍을 거부한다(QA `qa_sketch_new_curves` "ellipse with itself").

### 최근 API (자세한 것은 아래 각 절)

- **BOM** (GUID-A0A95805):
  - `session.bill_of_materials(BomView::{Structured, PartsOnly}) -> Vec<BomRow { item, level, part_number, description, quantity, structure, keys }>`
  - `session.bom_csv(view)`
  - `session.bom_structure(TreeKey)`, `session.set_bom_structure(TreeKey, BomStructure)`
  - `gc_session::structure_name`
  - 저장: `CellOp::SetBomStructure`, `Components.bom`
  - Reference는 `physical_properties(None)` 질량·부피에서도 빠진다.
- **Bend Equal** (GUID-4F861674): `SketchOp::BendEqual { lines, to }`. 첫 굽힘 호와 Equal로 묶이고 치수는 붙지 않는다. 프론트 대화상자 배선은 아직 안 됐다.
- **iProperties**:
  - `session.properties(target)`, `set_properties`, `part_number`, `physical_properties`
  - `CellOp::SetProperties`
- **문서 설정**:
  - `session.doc_settings()`, `set_doc_settings(DocSettings)`(`CellOp::SetDocSettings`)
  - `DocSettings::{format_length, format_angle, dimension_text, auto_bend_text}`
  - `session.set_snap_to_grid(bool)`, `snap_to_grid()`
  - `set_auto_bend(bool)`, `auto_bend() -> bool`. 리드가 이 bool 시그니처에 맞췄으니 바꾸지 말 것.

### 보류 목록

- `DocSettings.persist_constraints`: 저장만 되고 동작에는 반영되지 않는다. 꺼도 추론된 구속이 남는다.
- 도구 미리보기 내부 숫자: 아직 mm 고정이다(문서 단위 미반영).
- 격자 그리기, 입력 상자의 식/값 표시(`input_as_expression`): 프론트 몫.
- BOM 남은 것:
  - 문서 설정 Bill of Materials 탭(기본 BOM 구조)
  - 단위(Base Quantity)
  - 열 사용자 정의
  - First Level 보기(지금 구조 보기는 All Levels만)
- Bend Equal 프론트 배선(Equal 체크, 기본 켬).
- 이 파일 아래 "남은 백엔드 일 — 우선순위" 표의 나머지.

### 알려진 함정

- 공용 타입·시그니처를 바꾸기 전에는 다음 세 브랜치를 `git grep`한다.
  - `origin/claude/ur-robot-control-simulator-jsmhze-qa`
  - `origin/...-frontend`
  - 리드 브랜치 `origin/claude/ur-robot-control-simulator-jsmhze`
  - 깨지는 곳이 있으면 필드 추가 대신 새 variant나 새 함수로 비켜 간다(예: `Bend` 대신 `BendEqual`).
  - 영향은 handoff와 리드 메시지에 적는다.
- 리드가 이미 맞춘 API는 되돌리지 않는다. `set_auto_bend`를 `Option<String>`으로 되돌렸다가 다시 고친 적이 있다.
- 리드 브랜치 병합은 컴파일될 때만 한다. 리드가 수정을 푸시했다고 알린 뒤에 한다.
- clippy가 f64의 `hypot`/`acos`/`tan`/`sin`/`atan2`를 금지한다(테스트에서도). `gc_math::fm::*`를 쓴다.
- `type_complexity` 경고는 타입 별칭으로 푼다.
- 테스트나 빌드가 도는 중에 파일을 고치면 결과가 거짓으로 실패한다.
- 인벤터 동작은 먼저 원문을 curl로 받는다(scratchpad `help/`, `python3 -I`로 파싱). 원문 대조를 handoff에 쓰고 시작한다. 커밋에 GUID를 남긴다.
- `try_add`는 새로 퇴화하는 후보를 Conflicting으로 거부한다. 테스트에서 구속 순서를 바꾸면 결과가 달라질 수 있다.

### 축소 검사 절차 (리드 지시, 전체 gate 대신)

1. `cargo fmt --all`
2. `cargo clippy -p <바뀐 크레이트> --all-targets --locked -- -D warnings`
3. `cargo test -p <바뀐 크레이트와 그 직접 사용자> --locked`
4. 공용 타입이 바뀌었으면 `cargo check --workspace --all-targets --locked`

전체 `cargo xtask gate`는 리드가 병합할 때 돌린다.

## 남은 백엔드 일 — 우선순위(리드 ②, 2026-11-18)

`docs/ux/inventor-sketch-parity.md`·`inventor-assembly-parity.md`에서 "없음/신규/남음"이고 담당에 BE가 있는 행을 코드(`ToolKind`·`SketchOp`·`CellOp`)와 맞춰
본 결과. 문서의 "현재" 열이 오래된 행(Trim·Fillet·3점 호·관계·조인트·표현·측정 등 이미 된 것)은 뺐다. 위에서부터 진행.

| # | 항목 | 문서 행 | 지금 | 할 일 |
|---|---|---|---|---|
| 1 | ~~3D 스케치 구속: 평행·직각·동일(선 길이)·동일선상·점-선 위·중점, 3D 각도 치수~~ **완료** | 스케치 §3D(리드 지정) | — | 아래 작업 기록 |
| 2 | ~~3D 호~~ **완료**(반원 미만) | 리드 지정 | 3D 원은 아직 `WrongSpace` | 아래 작업 기록 |
| 3 | ~~Cancel Sketch~~ **완료** | 스케치 32 (P2) | — | 아래 작업 기록 |
| 4 | ~~Tangent Circle·Midpoint Line·중심 사각형 2종~~ **완료** | 44, 46, 57 (P2) | — | 아래 작업 기록 |
| 5 | ~~Chamfer·Split~~ **완료** | 65, 81 (P2) | — | 아래 작업 기록 |
| 6 | ~~Move·Copy·Rotate / Scale·Stretch~~ **완료** | 76–78 (P2) / 83–84 (P3) | — | 아래 작업 기록 |
| 7 | Auto-project — **면 스케치의 면 윤곽 자동 투영 완료** / 그리기 중 문질러 투영 남음 | 68 (P2) | 문질러 투영(rubbing)은 뷰포트 스냅 작업 | 아래 작업 기록 |
| 8 | ~~Automatic Dimensions and Constraints~~ **완료** | 126 (P2) | — | 아래 작업 기록 |
| 9 | ~~치수 방향 뒤집기(Flip)~~ **완료** | 124 (P3) | 선-호·호-호 거리 치수 종류는 없음 | 아래 작업 기록 |
| 10 | ~~타원~~(**완료**)·~~스플라인~~(**보간·열린 곡선 완료**, Smooth G2·제어점은 P3) | 60, 61, 108 (P3) | `CurveGeom`에 없음 | 새 곡선 종류 — 영향 큼 |
| 11 | 어셈블리: ~~서브어셈블리 계층·활성화~~(**완료**, 단위 이동·Flexible 포함), ~~조인트 Lock/Protect~~(**완료**), ~~3D 그립 스냅~~(**계산 완료**, 드래그 UI는 FE), ~~어셈블리 스케치 소유자~~(**완료**), ~~작업 피처 종류~~(**완료**: 평면 8종·축 6종·점 5종) | 어셈블리 43, 90, 61, 98, 157 | 회전 면 축·토러스·접평면은 없음 | 행마다 원문 확인 후 범위 결정 |

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
| 스케치 → 웨이포인트·경로(계획 9.1·9.4) | 스케치 점 = 웨이포인트 역할, 곡선 사슬 = 경로 역할(직선/관절/곡선 추종, 속도, 툴 자세), 역할을 따라가는 로봇 경로, 두 점 사이 경로 생성 | `add_waypoint_role`, `add_path_role`, `assign_roles`, `chain_at`, `set_role_*`, `follow_roles`, `route_between`, `path_from_sketch` / `CellOp::{AddRole, SetRole, DeleteRole, SetPathFollow, AddFollowPath, AddPathThrough}` | `gc_session/tests/sketch_roles.rs` |
| 모션 | 플래너·재계획 사다리·ACM·보증(M1–M2, 아래 오래된 절) | `plan`, `planned(path)` | `gc_motion`, `gc_plan` |

## 현황 — 남음

| 항목 | 상태 / 다음 |
|---|---|
| STEP B-rep(정확 형상) | 테셀레이션 없는 솔리드는 경고 후 건너뜀. 평면·원통부터 자체 테셀레이터 필요. 앱의 가져오기 배선(확장자 → `read_step`)은 리드/FE. 실제 대형 파일(NIST CTC 등) 확인은 네트워크 허용 후 |
| 미러의 진짜 반사 | 파일을 만들지 않아 Mirror = Reuse(프리미티브만 정확). 메시 반사는 렌더/메시 쪽 배율 음수 지원 필요 |
| 로봇 교체, 패턴·표현의 미러/복사 | 미지원 |
| 스케치 패턴 | Associative 해제·Boundary 없음. 패턴 치수 직접 편집 값이 편집 창에 안 보임 |
| 접촉 솔버 | 밀린 부품은 병진만(회전 없음), 배율 있는 어셈블리 부품 제외, Surface Complexity 없음 |
| 메시 로드 | `add_mesh_body`는 mm/m 판정용 STL 파싱만 동기 |
| DOF 기호 | 기호마다 관계를 모음(수백 개 동시 표시 시 캐시 필요) |
| Unsolvable | 인벤터의 Recover 상태 대신 거부(원문과 차이, 아래 기록) |
| 에이전트 도구 | `docs/handoff/backend-agent-tools.md`에 코드(gc_agent 수정은 권한상 리드) |
| 모션(오래된 남은 일) | 맨 아래 "남은 일(백엔드)" 절 그대로 |

# 작업 기록(최신 순)

## 2026-11-18 — URScript 프로그램 생성 (PLAN 9.6, 오프라인 텍스트만)

근거: `docs/research/robotics-domain.md`의 URScript 절(SI 단위, `movej` 기본 a = 1.4 rad/s²·v = 1.05 rad/s, `movel` 기본 a = 1.2 m/s²·v = 0.25 m/s, 자세 `p[x, y, z, rx, ry, rz]` 회전벡터), `m2-references.md`. **URSim·실제 로봇으로 실행해 본 적 없다**(이 환경에 URSim 없음). 전송·RTDE·Dashboard는 없다.

- `session.task_urscript(task, robot) -> Result<String, String>`: 그 로봇으로 작업을 계획해 `def <작업 이름>(): … end` 텍스트로. 문서 불변.
- `session.schedule_urscript(&Schedule) -> Vec<(Id<Robot>, String)>`: 할 일이 있는 로봇마다 프로그램 하나(`def <로봇 이름>()`). 작업·비키기 이동을 시작 순서로 잇고 사이의 쉬는 시간은 `sleep`. 로봇 간 동기화는 시간뿐이다.
- 변환 규칙(`gc_session/src/urscript.rs`):
  - 머리: `set_tcp(p[…])`(로봇 TCP), 이어서 계획 시작 관절값으로 `movej`.
  - 자유 이동(MoveJ, 경로 역할 접근, 비키기): 계획이 찾은 관절 경유점 그대로 `movej([q…], a=1.4, v=1.05, r=…)` — 시뮬레이션과 같은 팔 자세·우회로. 중간 경유점 `r=0.01`, 마지막 `r=0`.
  - 직선·곡선(MoveL, Linear/Follow 역할): `movel(p[…], a=1.2, v=<역할 속도 또는 0.25>, r=…)`, 자세는 **로봇 베이스 좌표계**(`robot_base_frame`의 역변환 적용). 곡선 표본은 `r=0.001`, 마지막 `r=0`.
  - `Wait` → `sleep(s)`. `SetIo` → `set_digital_out(pin, True/False)`(핀은 로봇 툴 종류의 출력 신호; 없으면 주석). `ToolAction` → 주석 + 신호 set + 확인 입력을 `timeout`까지 기다리는 `while`(확인이 없으면 `sleep(duration)`). `WaitIo` → `# TODO wait for: …` 주석(신호 모델 없음).
  - 이름의 식별자 밖 글자는 `_`.
- 출력은 결정적이다: 관절 6자리, 위치(m) 5자리, 회전벡터 6자리, 시간·속도·가속도·블렌드 3자리 고정 소수점(`-0.000` 없음). 첫 줄은 `# Generated by GraphCTRL <버전>, document revision <n>`. 골든 테스트 `urscript.rs::the_program_text_is_the_same_every_time`.
- 툴 동작의 확인 입력이 `timeout` 안에 안 오면 `popup("<동작>: no confirmation on input <pin>", "GraphCTRL", False, True)` 뒤 `halt`(조용히 진행하지 않는다).
- `TaskPlan.program: Vec<ProgramStep>`(새 필드): 계획된 작업을 명령 순서로 본 것 — `Joints`, `Linear { poses, speed }`, `Pause`, `Output`, `Action`, `WaitFor`. 다른 내보내기(다른 제조사, 온라인 실행)가 여기서 출발할 수 있다.
- 한계: 블렌드 반지름은 고정값(계획의 블렌드와 같지 않다 → 실제 경로가 시뮬레이션과 조금 다르고, 충돌 보증은 시뮬레이션 궤적에만 해당), 툴 플랜지 IO(`set_tool_digital_out`)·아날로그 미구분, 속도·가속도는 URScript 기본값(로봇별 한계·문서의 가속 프로파일 미반영), `movep`/`movec` 미사용.
- 테스트: `gc_session/tests/urscript.rs`(명령 순서, 숫자, 베이스 좌표계, 핀).

## 2026-11-18 — 교착 해소: 막는 로봇을 대기 자세로

- 대기 자세: `WorkOp::SetRobotPark { robot, joints: Option<Vec<f64>> }`(관절 수가 다르면 `BadValue`), `doc().robot_park(robot)` — 지정이 없으면 문서의 관절 자세(시작 자세). `Robot` 구조체는 그대로이고 `Components.parks`에 저장.
- 스케줄: 아무것도 움직이지 않고 더 줄 작업도 없는데 **아직 한 번도 못 한 준비된 작업**이 남아 있으면, 대기 자세에 있지 않은 쉬는 로봇 하나(로봇 추가 순서)를 대기 자세로 보낸다(다른 로봇은 서 있는 장애물). 그동안과 도착 후 작업을 다시 본다. 같은 자리에서 같은 로봇을 두 번 보내지 않으므로 끝난다.
- 결과: `Schedule.parks: Vec<ParkMove { robot, start, end, plan }>`. `joints_at`·`robots_at`·`makespan`·재생(`set_schedule_playback`)에 포함된다. 프론트는 간트에 "비키기" 막대로 그리면 된다.
- 한계: 실제로 길을 막는 로봇인지 따지지 않고 대기 자세에 없는 첫 로봇부터 보낸다(불필요한 이동이 있을 수 있음). 대기 자세로 가는 길도 막혀 있으면 그 로봇은 건너뛴다. 두 로봇이 서로의 대기 자세를 막는 경우는 풀지 못한다.
- 테스트: `task_schedule.rs::robots_keep_clear_of_each_other_…`.

## 2026-11-18 — 숫자 효과 식, 작업 반복, 대기 기록, 개별 계획에도 다른 로봇

- **숫자 변수 효과는 식**: `Effect.value`가 Int·Real 변수 대상이면 수치 식이다(`count + 1`, `2 * n`; 기존 `gc_expr::parse`). 이름은 숫자 상태 변수. 틀리면 `EditError::Expr { message, start, end }`(전에는 `BadValue`). 스케줄에서는 그 시점의 상태 값으로 계산하고 Int는 반올림한다. 변수 이름을 바꾸면 식도 따라 바뀐다. 한계: 공백이 든 변수 이름은 식에서 쓸 수 없다(조건식의 따옴표 문법이 수치 식에는 없다) — 그런 이름으로 바꾸려 하면 식이 깨지므로 `InUse`로 거부된다.
  - `gc_doc::effect_number(text, &lookup) -> Result<f64, EditError>`: 입력 칸 검증에 쓸 수 있다.
- **작업 반복**: `WorkOp::SetTaskRepeat { task, times: Option<u32> }` — `None` 한 번(기본), `Some(0)` 전제조건이 참이 될 때마다, `Some(n)` 최대 n번. `doc().task_repeat(task)`. `Task` 구조체는 안 건드렸다(다른 브랜치에 리터럴이 많다) → `Components.task_repeats`에 저장.
  - 스케줄: 끝난 작업이 반복 대상이면 우선순위 자리에 다시 넣는다. 전체 할당 `MAX_ASSIGNMENTS`(1000)에서 끊고 `Schedule.cut_short = true`.
  - 한 번 이상 한 반복 작업은 더 못 해도 `blocked`에 넣지 않는다.
- **대기 기록**: `Schedule.waits: Vec<Wait { task, robot, seconds, cause }>` — 움직이는 로봇 때문에 미뤄졌다가 시작한 작업.
- **개별 계획에도 다른 로봇**: `task_plan`·`task_plan_from`·`task_candidates`가 다른 로봇(억제 제외)을 문서의 관절 자세로 서 있는 장애물로 넣는다 → 스케줄의 첫 라운드와 같은 기준. 다른 로봇이 목표 앞에 서 있으면 이유에 그 로봇 이름이 나온다. 기존 경로 계획(`plan`)은 그대로(다른 로봇 안 봄).

## 2026-11-18 — 스케줄에서 로봇 간 회피 (PLAN 9.3·9.5)

- **쉬는 로봇 = 정지 장애물**: 입찰을 계획할 때 그 시점에 쉬고 있는 다른 로봇을 서 있는 자세 그대로 장애물로 넣는다(`RobotScene::as_obstacles(q, name)`를 `gc_motion`에 추가, `Planning::task_plan_among`). 그래서 스케줄의 입찰은 `task_candidates`(다른 로봇을 안 넣음)와 다를 수 있다.
- **움직이는 로봇 = 시간 표본 검사**: 계획된 동작을 50 ms마다, 그 시각에 다른 로봇이 있는 자세(진행 중이면 궤적 위, 끝났으면 끝 자세)와 링크·툴 형상끼리 거리 검사한다. 패딩(`PADDING`)보다 가까우면 그 로봇에는 **지금 주지 않는다(대기)**: 작업은 준비 상태로 남고 다음 작업이 끝나는 시점에 다시 본다. 그때 상대는 쉬는 로봇이므로 정지 장애물로 다시 계획된다(= 재계획).
- 결과: 대기는 `Assignment.start`가 늦어지는 것으로 나타난다. 끝까지 못 가면 `blocked`에 이유가 남는다(예: "W1: the goal is not valid: `link_4` collides with `Left`" — 상대가 목표 위에 서 있음, "it waits for Left to be out of the way").
- 기본 정책을 "대기"로 한 이유: 결정적이고, 이미 확정된 다른 로봇의 계획을 건드리지 않는다. 우회 재계획은 상대가 멈춘 뒤에만 한다.
- 한계: 표본 시각 사이는 검사하지 않는다(패딩으로 보완). 움직이는 상대를 피해 돌아가는 계획은 없다(기다림). 목표 위에 서 있는 로봇을 비키게 하는 작업을 스스로 만들지 않는다(교착은 `blocked`). `task_plan`·`task_candidates`·경로 계획(`plan`)은 여전히 다른 로봇을 보지 않는다.
- 테스트: `task_schedule.rs::robots_keep_clear_of_each_other_…`.

## 2026-11-18 — 스케줄 재생 (여러 로봇 동시)

- `Schedule.plans: Vec<TaskPlan>`(할당과 같은 순서) 추가. `Schedule`의 `==`는 할당과 막힌 작업만 비교한다(궤적은 비교 안 함).
- `TaskPlan`에 `start`(시작 관절값), `pauses: Vec<(몇 번째 동작 뒤, 초)>`(대기·툴 동작으로 서 있는 구간), `sample(t)`(작업 시작 후 t초의 관절값) 추가.
- `Schedule::joints_at(robot, t) -> Option<Vec<f64>>`: 그 시각 하고 있는 작업의 동작 위, 또는 마지막 작업이 끝난 자세. 첫 작업 전·작업 없는 로봇은 `None`. `Schedule::robots_at(t)`: 작업이 있는 모든 로봇의 자세.
- 뷰포트용: `session.set_schedule_playback(Some(t))` → `session.robot_joints(robot)`(뷰포트가 이미 쓰는 함수)이 모든 로봇을 그 시각 자세로 돌려준다. 문서는 안 바뀐다. `None`으로 해제, `session.schedule_playback()`으로 조회. 대상은 `request_schedule`로 받은 마지막 스케줄(`schedule_result`)이며 스케줄이 없으면 무시된다. 켜면 경로 재생(`set_playback`)은 꺼진다.
- 프론트 할 일: 시간축 슬라이더·재생 버튼에서 `set_schedule_playback(t)`를 부르고 `0..=schedule.makespan()` 범위로 움직이면 된다.
- 테스트: `task_schedule.rs::a_schedule_is_played_back_…`.

## 2026-11-18 — 조건식 길이 상한, 빈 로봇 조건 (QA)

- `gc_expr::Condition`: `and`/`or`를 256개까지만 받는다(넘으면 그 자리의 `CondError` "a condition can combine at most 256 terms"). 긴 연쇄는 깊이가 길이만큼인 트리가 되어 평가·순회·해제가 재귀로 스택을 다 쓰던 것(약 5000항에서 프로세스 종료)을 막는다. 괄호·`not` 중첩 상한 64는 그대로.
- `RobotChoice::When("")`(빈 조건)는 `BadValue`. 빈 전제조건(`Task.precondition`)은 여전히 "항상".
- QA 브랜치의 `qa_work_model.rs::a_long_chain_of_and_terms…`는 이 브랜치에 없어 `#[ignore]`를 못 뗐다(QA/리드가 병합 때).

## 2026-11-18 — 스케줄을 작업 스레드에서 (`request_schedule` / `schedule_result`)

- `session.request_schedule()`: 지금 문서의 스케줄을 실행기(앱은 스레드, 기본은 그 자리에서)에서 계산하기 시작한다. 같은 리비전에 대해 이미 계산 중이거나 결과가 있으면 아무것도 하지 않는다. 더 옛 리비전의 계산은 취소한다.
- 결과는 `session.poll_jobs()`(기존, 매 프레임 부르는 것)로 도착한다 → `session.schedule_result() -> Option<(&Schedule, bool)>`. `bool`은 "요청한 뒤 문서가 안 바뀌었다"(리비전 일치). 바뀌었으면 옛 결과를 흐리게 보여 주고 다시 `request_schedule()`.
- `session.schedule_pending()`: 계산 중인지.
- 계산은 문서·로봇 모델·메시·설비 장애물의 사본으로 한다(계산 중 편집해도 결과는 요청한 문서의 것). 취소는 작업 사이·계획 단계 사이에서 반영된다.
- `session.schedule()`(동기)와 결과가 같다. `task_plan`·`task_candidates`는 여전히 동기다.
- 내부: 작업 계획·스케줄 함수들을 `Planning<'_>`(문서·로봇·메시·장애물·취소를 빌려 보는 문맥)의 메서드로 옮겼고 `Session`의 공개 함수는 그대로 얇게 감싼다(시그니처 불변).
- 테스트: `task_schedule.rs::a_schedule_asked_for_in_the_background_…`.

## 2026-11-18 — 툴 종류(ToolKit)·툴 동작·IO 이름표 (PLAN 9.4, GraphCTRL 고유)

기존 `gc_doc::Tool { solid, placement }`(형상)과 `Robot`은 그대로다. 장치로서의 툴은 새 엔티티다.

- `ToolKit { name, signals: Vec<IoSignal { name, direction: In|Out, pin }>, states: Vec<String>, actions: Vec<ToolAction> }` (`Kind::ToolKit`, `gc_doc::toolkit`).
- `ToolAction { name, target: Option<Id<ObjectType>>, precondition, set: Option<(신호, bool)>, confirm: Option<(신호, bool)>, tool_state, target_state, duration, timeout }`.
  - `precondition`은 조건식이고 여기서만 `tool.state`·`target.state`를 쓸 수 있다(작업 조건식에서 쓰면 `check_condition`이 오류). 그래서 객체 이름 `tool`·`target`은 `BadName`으로 거부한다.
  - 검사: 이름 유일, `set`은 Out 신호·`confirm`은 In 신호, 같은 방향·같은 핀 중복 거부, `tool_state`는 툴 상태, `target_state`는 대상 타입의 상태, 시간은 0 이상.
- 장착: `WorkOp::SetRobotTool { robot, kit: Option<_>, state: Option<String> }` → `Components.tool_kits`(`FittedTool`). 조회 `session.robot_tool_kit(robot) -> Option<(Id<ToolKit>, Option<상태>)>`(상태 생략 시 첫 상태).
- 편집: `session.edit_work(WorkOp::{AddToolKit, SetToolKit, DeleteToolKit, SetRobotTool})`, 이름 바꾸기 `Command::Rename`. 각 Undo 1단계, 저장.
  - `SetToolKit`: 동작(신호) 개수가 같으면 자리별로 이름이 달라진 것을 이름 바꾸기로 보고 작업 단계(`ToolAction.action`, `SetIo.name`)를 고친다(다른 툴 종류가 옛 이름을 아직 갖고 있으면 그대로 둠).
  - 단계가 쓰는 동작·출력 신호를 없애면 `InUse`. 로봇에 장착된 툴 종류 삭제도 `InUse`. 동작의 대상 타입은 삭제가 거부된다.
- 작업과의 연결(기존 `Step`·`Task` 타입 불변):
  - 문서는 `ToolAction`·`SetIo`의 이름을 강제하지 않는다(툴 종류가 없는 예전 문서가 그대로 열리게). **툴 종류가 하나라도 있으면** `task_problems`가 "그 동작/출력 신호를 가진 툴이 없다", "대상 객체 타입이 다르다"를 알려 준다.
  - `task_plan`: 툴 종류가 하나라도 있으면 그 로봇의 툴 종류에 동작·출력 신호가 있어야 한다(없으면 이유와 함께 실패 → `task_candidates`에서 뒤로). 동작의 `duration`이 `TaskPlan.waiting`에 더해진다.
  - `schedule`: 로봇별 툴 상태를 추적한다. 로봇에 주기 전에 작업의 툴 동작을 순서대로 따져 전제조건(`tool.state`, `target.state`, 전역 상태)이 맞아야 하고, 할당하면 툴 상태가 바뀌며, 작업이 끝날 때 대상 객체 상태가 바뀐다(그 뒤 작업의 효과).
- 없는 것: 실제 신호 송수신, 확인 신호·시간 초과 실패의 시뮬레이션, `WaitIo` 조건의 검사(글자 그대로), 아날로그 IO, 툴 교환.
- 테스트: `gc_session/tests/tool_kits.rs`.
- 다음 후보(리드 메모): 로봇 간 회피 — 다른 로봇을 그 시간대 자세로 장애물에 넣기.

## 2026-11-18 — 작업 → 로봇별 계획(입찰)과 최소 스케줄러 (PLAN 9.4·9.5, GraphCTRL 고유)

### 작업 계획 (`gc_session/src/cell.rs` 끝)

- `session.task_plan(task, robot) -> Result<TaskPlan, String>` / `task_plan_from(task, robot, &start_joints)`: 작업 단계를 그 로봇의 동작으로 계획한다. 문서는 바뀌지 않는다.
  - `MoveJ` → 자유 이동, `MoveL` → TCP 직선, `FollowPath(role)` → 경로 역할 그대로(시작점까지 자유 이동 + 추종). `Wait`는 시간만 더하고, `SetIo`·`WaitIo`·`ToolAction`은 0초(모델 없음).
  - `TaskPlan { robot, segments: Vec<MotionPlan>, waiting, end }`, `duration()` = 동작 시간 + 대기.
  - 기존 경로 계획 본체를 나눠 재사용한다: `robot_scene`(로봇·툴·장애물), `role_steps`, `waypoint_goal`, `run_steps`. 기본 플래너(`PLANNERS[0]`), 시드 1.
  - **툴 TCP**: 목표는 TCP 자세이고 플랜지는 `scene.flange_target`으로 물러난다(테스트: 100 mm 툴이면 플랜지가 벽 앞 100 mm).
  - **기준 좌표계**: 역할의 자세는 계획 시점의 스케치에서 읽는다 — 스케치가 컴포넌트 면·작업 평면 위에 있으면 그것을 따라가고, 역할의 `aim`이 작업 좌표계 기준이면 그 좌표계를 따른다. `Task.location`의 자세를 역할 자세에 **또 곱하지 않는다**(역할이 이미 월드에 고정돼 있어 두 번 옮기게 됨). `location`은 표시·`Location::Robot` 판정에만 쓴다.
  - 이미 목표 자세에 서 있으면(1 µm·1 µrad) 다른 IK 해로 바꾸지 않고 그 자리에 머문다(`run_steps`, 기존 경로 계획에도 적용됨).
  - 실패 이유는 단계 이름으로 시작한다(`W2: …`).
- `session.task_robots(task)`: 할당 가능한 로봇(로봇 자신 작업이면 그 로봇, `Only`, `Any`, `When`은 조건이 참일 때 전부; 억제된 로봇 제외).
- `session.task_candidates(task) -> Vec<TaskCandidate { robot, time: Result<f64, String> }>`: 현재 자세 기준 입찰. 빠른 순, 못 하는 로봇은 뒤에 이유와 함께.
- 동기 호출이다(호출한 스레드에서 계획). 작업 스레드·캐시는 아직 없다 → 화면에서 매 프레임 부르지 말 것.

### 최소 스케줄러 (`gc_session/src/schedule.rs`)

- `session.schedule() -> Schedule { assignments: Vec<Assignment { task, robot, start, end }>, blocked: Vec<(Id<Task>, String)> }`, `makespan()`. 동기·결정적, 실행 없음, 문서 불변.
- 규칙: 각 작업은 한 번. 매 시점 전제조건이 참인 작업을 우선순위(큰 것 먼저) → 만든 순서로 보고, **그때 비어 있는** 허용 로봇 중 입찰 시간이 가장 짧은 로봇(그 로봇이 그때 서 있는 관절값에서 계획)에 준다. 작업이 끝나는 시점에 효과를 상태에 적용하고 다시 본다. 진행 중인 작업이 없고 더 줄 것도 없으면 끝.
- 막힌 작업의 이유: "its precondition does not hold", "no robot may do it", 또는 로봇의 계획 실패 이유.
- 한계: 로봇끼리 서로를 피하지 않는다(각자 지금의 셀을 기준으로 계획, 다른 로봇은 현재 자세의 장애물도 아님). 공유 구역·시간 조정·재경매·복구 없음. 숫자 변수 효과는 값 대입만(증감 없음). 큰 셀에서는 입찰 계획 때문에 느리다.
- `gc_session`이 `gc_expr`에 직접 의존하게 됐다(Cargo.lock 변경: 워크스페이스 내부 항목만).
- 테스트: `gc_session/tests/task_schedule.rs`.

## 2026-11-18 — 작업 모델 첫 데이터층 (PLAN 9.4, GraphCTRL 고유, 원문 없음)

데이터와 검증만이다. 스케줄러·실행·시뮬레이션, Tool/ToolAction 정의, IO 모델, 이벤트, 목표식, 복구는 없다.

### 모델 (`gc_doc::work`, 엔티티 4종: `Kind::{ObjectType, WorkObject, StateVar, Task}`)

- `ObjectType { name, states: Vec<String> }` — 열거 상태(1개 이상, 중복 없음).
- `WorkObject { name, ty, location, state }` — `state`는 타입의 상태 중 하나(초기 상태).
- `StateVar { name, value: StateValue::{Bool, Int, Real, Enum { choices, value }} }` — 전역 상태(초기값이 종류를 정함).
- `Task { name, location, precondition, steps, effects, robots, priority }`
  - `Location::{None, Component(Occurrence), Frame(작업 좌표계), Sketch, Robot}`
  - `Step::{MoveJ(MoveTarget), MoveL(MoveTarget), FollowPath(Id<SketchRole>), SetIo { name, value }, WaitIo { condition, timeout }, ToolAction { action, target: Option<Id<WorkObject>> }, Wait { seconds }}`
  - `MoveTarget::{Role(Id<SketchRole>), Waypoint(Id<Waypoint>)}` — `Role`은 웨이포인트 역할만, `FollowPath`는 경로 역할만. `Waypoint`(로봇 소유)는 `Location::Robot(그 로봇)`인 작업에서만.
  - `Effect { target: EffectTarget::{Object, Var}, value: String }` — 객체는 타입의 상태 이름, 변수는 종류에 맞는 값(`true`, `3`, `0.5`, 선택지 이름).
  - `RobotChoice::{Any, Only(Vec<Id<Robot>>), When(조건식)}`, `priority: i32`(클수록 먼저).
  - `WaitIo.condition`과 `ToolAction.action`, `SetIo.name`은 지금은 글자 그대로 저장만 한다(IO·툴 동작 모델이 생기면 검사).
- 이름(타입·객체·변수·상태): 비어 있지 않고 `"`·제어 문자가 없고 앞뒤 공백 없음. 한글·공백 가능. 타입끼리, 객체끼리, 변수끼리 유일.

### 조건식 (`gc_expr::Condition`, 새 모듈 — 기존 수치 식은 그대로)

- `== != < <= > >=`, `and or not`(`&& || !`도), 괄호, 숫자, `true`/`false`, 이름. 우선순위: 비교 > not > and > or.
- 참조: `변수이름`, `객체이름.state`. 공백 등이 든 이름은 큰따옴표: `"홀 A".state == "체결 완료" and count < 3`.
- 낱말은 그 이름의 상태 변수가 있으면 변수, 없으면 이름(상태·선택지). `==`·`!=`는 같은 종류끼리, 대소 비교는 숫자만.
- 문서에는 원문 문자열로 저장. 쓸 때 검사: 읽히는지, 객체·필드·낱말을 아는지(낱말은 변수이거나 어떤 타입의 상태이거나 어떤 열거의 선택지), 초기 상태로 참/거짓이 나오는지. 틀리면 `EditError::Expr { message, start, end }`(바이트 범위).
- 빈 전제조건 = 항상.

### 명령 `CellOp::Work(WorkOp)` — 각 Undo 1단계

`Add*/Set*/Delete*` × {ObjectType, Object, StateVar, Task}. 모든 변경 뒤 전체 작업을 다시 검사해, 깨지는 것이 있으면 `InUse`로 거부한다.

- 이름 바꾸기(`Set*`로 이름이 바뀌거나 `Command::Rename`): 객체·변수 이름을 쓰는 조건식이 같은 단계에서 바뀐다(필요하면 따옴표를 붙여 씀).
- `SetObjectType`: 상태 개수가 같으면 자리별로 이름이 달라진 상태를 "이름 바꾸기"로 보고 그 타입 객체의 초기 상태, 효과, 조건식(그 타입 객체의 `.state`와 비교된 낱말)을 고친다. 상태를 빼면(개수 감소) 쓰이는 곳이 있을 때 `InUse`.
- 삭제: 타입은 객체가 있으면, 객체는 조건식·효과·단계가 참조하면, 변수는 조건식·효과가 참조하면 `InUse`.
- `SketchRole`: 작업 단계가 쓰면 `DeleteRole` = `InUse`, 종류를 바꾸는 `SetRole`도 `InUse`, 스케치 요소가 지워져도 역할은 남는다(→ `task_problems`). 로봇·작업 좌표계·로봇 웨이포인트·스케치는 참조되면 삭제가 거부된다(`Dangling`).
- 열거 변수의 선택지 이름 바꾸기는 조건식을 따라 고치지 않는다(쓰이면 `InUse`로 거부됨).

### 세션 API (프론트엔드용, `gc_session/src/work.rs`)

| 무엇 | 호출 |
|---|---|
| 목록 | `doc().object_types()`, `doc().objects()`, `doc().state_vars()`, `doc().tasks()`(모두 만든 순서), `session.tasks() -> Vec<Id<Task>>` |
| 추가·편집·삭제 | `session.edit_work(WorkOp) -> Result<Option<AnyId>, EditError>`(추가면 새 id) |
| 이름 바꾸기(브라우저 F2) | `Command::Rename { item, name }` — 4종 모두 |
| 억제 | 작업은 `set_suppressed`(기존) 가능 |
| 조건식 입력 검증 | `session.check_condition(text)` → `Err(EditError::Expr { message, start, end })` |
| 준비 여부(초기 상태 기준) | `session.task_ready(task) -> Result<bool, EditError>` |
| 끊긴 참조 안내 | `session.task_problems(task) -> Vec<String>` |
| 위치 표시 | `session.location_pose(&Location) -> Option<DPose3>` |
| 단계에 넣을 역할 | `sketch_roles(sketch)`, `doc().roles()`(웨이포인트/경로는 `SketchRole::is_path`) |

### 설계 메모 — 작업 대상이 로봇 자신(`Location::Robot`)

- 그 작업은 그 로봇만 할 수 있다(`robots`는 보지 않는다). 스케줄러는 입찰 없이 그 로봇에 바로 준다.
- 단계는 그 로봇의 `Waypoint`(`Joints`·`InBase`·`InWorld`·`InFrame`·`SketchPoint`)를 직접 가리킬 수 있다(`MoveTarget::Waypoint`) → 기존 이동 제어(툴 TCP, 기준 좌표계 = 베이스/월드/작업 좌표계)가 그대로 해석한다.
- 계획은 새 플래너를 만들지 않고 `gc_session/src/cell.rs`의 `path_steps`가 만드는 단계 목록(`Step { goal, travel, tcp_speed, contacts }`)으로 변환한다: `MoveJ` → `Travel::Free`, `MoveL` → `Travel::Linear`, `FollowPath(role)` → 지금 `follow` 경로와 같은 "시작점까지 자유 이동 + 역할 추종". 즉 `Task` + 로봇 → 임시 단계 목록 → 기존 `plan_path` 본체. 스케줄러의 입찰(계획한 동작 시간)도 같은 함수를 쓴다.
- 구조에 붙은 작업(`Role`만 쓰는 작업)은 어느 로봇에나 같은 변환이 적용된다: 역할의 월드 자세가 로봇과 무관하기 때문이다.

### 다른 브랜치 영향

추가뿐이다: `Kind` 4개, `Created` 4개, `CellOp::Work`, `Contents` 필드 4개, `gc_expr`의 새 공개 항목. 기존 시그니처는 그대로. `Document::role_followed`가 이제 "작업 단계가 쓰는 역할"에도 참이다.

## 2026-11-18 — BOM CSV·부품 번호 (QA)

- `bom_csv`: 탭(`\t`)·CR(`\r`)로 시작하는 칸도 수식 시작으로 보고 앞에 `'`를 붙인다(OWASP CSV 인젝션 문자 집합: `=` `+` `-` `@` 탭 CR).
- 부품 번호: 끝 공백을 떼고 묶는다(`"BOLT "` = `"BOLT"`). 대소문자는 구분한다. About the BOM Structure(GUID-A0A95805-CBF6-4524-8314-4AC8B53C19CF)에는 대소문자·공백 규칙이 없다(원문 근거 없음, 리드 지시대로).
- QA 브랜치의 `qa_bom_bend_equal.rs::csv_also_guards_cells_that_start_with_a_tab_or_a_carriage_return`는 이 브랜치에 없어 `#[ignore]`를 여기서 못 뗐다. 같은 단언을 `gc_session/tests/bom.rs`에 넣었다.

## 2026-11-18 — 경로 역할: 닫힌 곡선, 접선 방향 툴 자세 (GraphCTRL 고유, 원문 없음)

- `SketchRole.travel: PathTravel { tangent: bool, start: Option<Id<SkPoint>> }` 추가(`#[serde(default)]`, 옛 파일은 기본값). `RoleKind`·`ToolAim`은 그대로(다른 브랜치의 패턴·리터럴을 깨지 않으려고 새 필드를 `SketchRole`에 뒀다).
- **닫힌 곡선**: 원·전체 타원·닫힌 스플라인 하나가 경로 역할이 된다(한 바퀴 돌아 제자리, 원·타원은 스케치에서 반시계, 스플라인은 그린 방향, `reverse_role`로 반대). 다른 곡선과 사슬로 묶는 것은 여전히 `BadValue`. `chain_at(닫힌 곡선)` = `[그 곡선]`(전에는 빈 목록).
  - 시작점: `set_role_start(role, Some(스케치 점))` → 그 점에서 가장 가까운 경로 위 위치에서 시작·종료(원·타원은 정확한 위치, 닫힌 스플라인은 표본점, 선·호로 닫힌 고리는 가장 가까운 이음점). `None`이면 원은 +X 점, 타원은 장축 끝, 스플라인은 첫 맞춤점. 그 점이 지워지면 기본 시작으로 돌아간다.
  - `role_closed(role)`: 닫힌 경로인지(시작점 UI를 낼지).
- **접선 방향 자세**: `set_role_tangent(role, true)` → 툴 X축이 진행 방향(자세 기준 평면에 투영)을 보고, 툴 Z는 그대로 기준 Z 반대, `aim.rpy`는 그 위에. Follow는 곡선 접선, Linear는 현 방향(모서리에서 제자리 회전 동작이 따로 생김: 세그먼트 수가 늘어난다), Joint는 무시(한 자세).
  - 닫힌 원을 접선으로 한 바퀴 돌면 툴이 축 둘레로 360° 돈다 → 손목 관절 한계에 걸리면 계획이 이유와 함께 실패할 수 있다.
- `Document::role_poses(role)`: 곡선별 툴 자세열(계획과 `role_ends`가 쓴다). `role_polyline`은 그대로.
- `path_from_sketch`는 닫힌 곡선을 여전히 건너뛴다(닫힌 곡선만 있는 스케치는 거부 그대로).
- **QA 테스트 수정**: `qa_sketch_roles.rs::closed_curves_branches_and_gaps_are_refused`가 "닫힌 곡선 하나는 거부, `chain_at`은 빈 목록"을 단언하고 있어, 리드 지시(닫힌 경로)에 맞춰 "닫힌 곡선은 혼자서 경로가 된다"로 바꿨다. QA 브랜치의 `gc_app/tests/qa_role_screens.rs` 100–103행은 "원은 경로로 제안되지 않는다"를 단언한다 — 프론트 메뉴가 `chain_at`이 비어 있지 않으면 경로를 제안하므로 병합 뒤 실패할 것이다(QA가 단언을 "원도 경로로 제안된다"로 바꿔야 함). `sketch_shape_tools.rs`에는 해당 단언이 없다.
- 남은 것: 닫힌 곡선을 `path_from_sketch`에 넣기, 구간별 속도·블렌드, 역할 편집 미리보기.

## 2026-11-18 — 브라우저 사용자 정의 폴더

원문: Assembly Browser Reference GUID-CB6B1637-71AD-4753-8CC9-19559BFEE3A5("User-defined Folder groups assembly nodes for better browser
organization"), Working with the Assembly Browser GUID-5F975252-BDB4-488C-A2C5-8B15C7D3C641("Create folders to organize the browser nodes
into groups or to communicate design intent"). 원문은 이 두 문장뿐이라 세부 규칙(한 수준, 한 행은 한 폴더)은 인벤터 동작에 맞춘 것이고 원문 근거는 없다.

- 폴더는 브라우저 정리용일 뿐이다: `component_tree()`(수준·부모·깊이), 관계, BOM은 그대로. 프론트가 `folders()`를 보고 구성원 행을 폴더 노드 아래로 그린다.
- `session.folders() -> Vec<BrowserFolder { id, name, members: Vec<TreeKey> }>`, `session.folder_of(TreeKey) -> Option<u32>`.
- `session.edit_folders(FolderOp)` — 각 Undo 1단계, 문서에 저장(`Components.folders`, `CellOp::Folder`).
  - `Add { name, members }`: 새 폴더(빈 이름 → "Folder:N", 구성원 없이도 가능). 새 id를 돌려준다.
  - `Move { members, into: Option<id> }`: 끌어 넣기 / 꺼내기(`None`).
  - `Rename { id, name }`(빈 이름 `BadValue`, 겹치면 `NameTaken`), `Delete(id)`(구성원은 그 자리에 남음).
- 규칙: 구성원은 트리의 한 수준 행(`TreeKey`: 컴포넌트·서브어셈블리)이어야 하고 한 행은 한 폴더에만(다른 폴더에 넣으면 앞 폴더에서 빠짐). 다른 수준의 행을 섞으면 `BadValue`. 지워진 행, 다른 수준으로 옮겨진 행은 조회 때 빠진다(폴더 수준 = 첫 구성원의 수준).
- 없는 것: 폴더 안의 폴더, 관계·스케치·작업 피처를 폴더에 넣기, 폴더 순서 바꾸기.
- 테스트: `gc_session/tests/folders.rs`.

## 2026-11-18 — Isolate / Undo Isolate

원문: Frame Generator browser GUID-36AE9264-912D-4D3C-BB24-7EC16FA8777E("Isolate turns off visibility of components … Undo Isolate, instead of Undo,
restores visibility of components to the pre-isolation state. It also keeps intact those operations performed in the isolated state"),
Working with the Assembly Browser GUID-5F975252-BDB4-488C-A2C5-8B15C7D3C641("Select All Visible … to restore an Isolate Components operation").

- `session.isolate(&[Occurrence])`: 보이는 컴포넌트 중 선택한 것, 그것을 담은 것(부모), 그 안의 것(자식)을 뺀 나머지를 숨긴다(바깥쪽 것만 끄면 안쪽도 안 보임). Undo 1단계.
- `session.undo_isolate()`: Isolate가 숨긴 것만 다시 보인다. 그 사이에 한 일(손으로 보이게/숨긴 것, 이동 등)은 그대로. Isolate 전에 숨겨져 있던 것은 계속 숨김. 격리 중이 아니면 `BadValue`.
- `session.is_isolated()`: Undo Isolate 메뉴를 낼지.
- 저장: `Components.isolated`(문서에 저장 — 원문은 저장 여부를 말하지 않는다. 가시성이 문서에 저장되므로 같이 저장했다). `CellOp::{Isolate, UndoIsolate}`.
- 대상은 `Occurrence`(본체·로봇·어셈블리·부품·링크·툴). 셀에서 묶은 서브어셈블리는 프론트가 구성원으로 펼쳐서 넘긴다. 스케치·작업 피처는 숨기지 않는다.
- 테스트: `gc_session/tests/isolate.rs`.

## 2026-11-18 — 리드 덧붙임·QA 관찰 2건

- **작업 좌표계 삭제**: 역할의 `aim`이 그 좌표계를 기준으로 하면 `CellOp::DeleteFrame`이 `InUse`로 거부(전에는 `Dangling`).
- **정밀도 반올림**(QA): `DocSettings::format_length/angle`이 반(.5)을 0에서 먼 쪽으로 올린다(정밀도 0에서 12.5 mm → "13 mm", 전에는 짝수 반올림으로 "12 mm"). 원문 To Change Document Settings(GUID-FB49837F-7832-4713-928E-1CEDBDB0C3BD)에는 반올림 방식이 적혀 있지 않아 통상의 사사오입으로 했다(원문 근거 없음).
- **열린 메시**(QA): `PhysicalProperties.open: bool` 추가 — 메시의 모서리 중 면이 정확히 둘이 아닌 것이 있으면(면이 빠진 STL) `true`. 부피·질량·무게중심은 그대로 계산해 주되 믿을 수 없다는 표시이고, 면적은 맞다. 문서 합계(`None`)에도 전파. 로봇 링크의 시각 메시도 닫혀 있지 않으면 `true`가 된다. 프론트는 Physical 탭에 경고를 띄우면 된다. 다른 브랜치에 `PhysicalProperties` 리터럴은 없다.

## 2026-11-18 — 스케치 요소의 웨이포인트·경로 역할 (사용자 최우선, 계획 9.1 "스케치 → 로봇 경로·웨이포인트", 9.4)

인벤터에 없는 GraphCTRL 고유 기능이라 Autodesk 원문 근거는 없다. 근거는 PLAN 9.1·9.4·5.10.

### 모델 (`gc_doc::role`, 새 엔티티 `SketchRole`, `Kind::SketchRole`)

- 역할은 **로봇에 속하지 않는다**(작업은 구조에 붙는다). 문서에 저장되고, 편집은 각각 Undo 1단계, 이름 바꾸기(`Command::Rename`)·억제(`SetSuppressed`)·숨김은 다른 브라우저 항목과 같다.
- `SketchRole { name, sketch, kind, aim }`
  - `RoleKind::Waypoint { point }` — 2D·3D 스케치 점. 한 점에 하나(`InUse`).
  - `RoleKind::Path { curves, reversed, motion, tcp_speed }` — 끝과 끝이 이어진(0.1 µm 이내) 열린 곡선: 선·호·타원호·열린 스플라인. 아무 순서로 줘도 사슬 순서로 저장된다. 시작은 "준 목록에서 자유 끝이 있는 첫 곡선의 자유 끝". 원·전체 타원·닫힌 스플라인, 세 곡선이 만나는 분기, 끊긴 묶음은 `BadValue`. 한 곡선은 한 경로 역할에만(`InUse`).
  - `PathMotion::{Follow(기본, 곡선 추종), Linear(곡선 끝점 사이 직선), Joint(끝점 사이 관절 이동)}`, `tcp_speed: Option<f64>`(m/s).
  - `aim: ToolAim { reference: AimReference::{SketchPlane, Frame(작업 좌표계)}, rpy }` — 툴 Z가 기준의 Z 반대(기준의 X 유지)를 향한 뒤 `rpy`. 기본값은 예전과 같은 "스케치 평면 법선 반대"(3D 스케치는 −Z). 경로는 전 구간 같은 자세.
- 스케치 편집으로 점·곡선이 없어지면 같은 Undo 단계에서 역할이 정리된다(`role::prune`, `apply` 끝). 경로 역할은 남은 곡선이 여전히 한 사슬이면 그것만 유지, 아니면 삭제. **로봇 경로가 따라가는 역할은 지우지 않는다**(그 경로는 계획 때 "its sketch curves no longer exist or no longer join end to end"로 실패). 스케치 자체 삭제는 따라가는 경로가 있으면 `Dangling`으로 거부.
- 치수·드래그로 사슬 끝이 벌어져도 역할은 남는다(깨진 상태: `role_polyline` = `None`).

### 로봇에 붙이기 (수동 모드. 나중에 스케줄러가 같은 것을 만든다)

- 웨이포인트: 기존 `WaypointTarget::SketchPoint` 그대로. 그 점에 웨이포인트 역할이 있으면 자세는 역할의 `aim`(`Document::sketch_point_pose`).
- 경로: `MotionPath.follow: Vec<Id<SketchRole>>`(새 필드, `#[serde(default)]`). 비어 있지 않으면 `waypoints`는 비어 있고, **계획할 때** 역할에서 곡선·이동 방식·속도·자세를 읽는다. 역할마다 "시작점까지 자유 이동(선택한 플래너)" 뒤 Follow는 사슬 전체를 한 동작으로(멈춤 없이), Linear/Joint는 곡선마다 한 동작. 속도는 역할의 `tcp_speed`, 없으면 경로의 `tcp_speed`. `ContactAllowance.index`는 `follow`에서의 위치.
- 스케치 치수·역할 설정이 바뀌면 `plan_status` = `Stale`(기존 규칙 그대로, `plan_inputs`에 역할의 월드 곡선·자세·이동 방식 포함).
- 곡선 추종은 `gc_motion::plan_follow(scene, request, via, control)`(새 함수): 기존 직선 이동을 "여러 자세를 잇는 TCP 추종"으로 일반화. 호는 3° 간격, 스플라인은 구간당 16점, 타원호 96점으로 샘플(`Sketch::curve_run`).

### 세션 API (프론트엔드용, `gc_session/src/role.rs`)

| 무엇 | 호출 |
|---|---|
| 점 → 웨이포인트 | `add_waypoint_role(sketch, point) -> Id<SketchRole>` |
| 곡선들 → 경로 | `add_path_role(sketch, &[curve]) -> Id<SketchRole>` |
| 스케치 선택 한 번에(Undo 1단계) | `assign_roles(sketch, &points, &curves) -> Vec<Id<SketchRole>>` (이미 웨이포인트인 점은 건너뜀, 곡선은 경로 하나) |
| 곡선 하나 눌러 사슬 찾기 | `chain_at(sketch, curve) -> Vec<Id<Curve>>` (자유 끝·분기·이미 경로인 곡선에서 멈춤) |
| 조회 | `waypoint_role(sketch, point)`, `path_role(sketch, curve)`, `sketch_roles(sketch)`, `doc().roles()`, `doc().get(role)` |
| 속성 편집 | `set_role_motion(id, PathMotion)`, `set_role_speed(id, Option<f64>)`, `reverse_role(id)`, `set_role_aim(id, ToolAim)`, `set_role(id, SketchRole)`(통째), `role_aim_references()` |
| 해제 | `delete_role(id)` (따라가는 로봇 경로가 있으면 `InUse`) |
| 화면 표시 | `role_pose(id)`(웨이포인트 자세 / 경로 시작 자세), `role_ends(id)`, `role_polyline(id)`(진행 방향 월드 점열; Linear·Joint는 끝점만) |
| 로봇이 따라가기 | `follow_roles(robot, &[role]) -> Id<MotionPath>`, `set_path_follow(path, &[role])`, 이후 `plan(path)`·재생은 기존과 같음 |
| "경로 생성" 모드 | `route_between(robot, (sketch, p1), (sketch, p2), planner_id, linear) -> Id<MotionPath>` — 로봇의 그 점 웨이포인트를 재사용하거나 만들고 경로를 추가(Undo 1단계)한 뒤 `plan`까지 시작. `planner_id`는 `planner_choices()`의 id |
| 스케치 전체 → 경로 | `path_from_sketch(robot, sketch)` — 시그니처 그대로. 이제 사슬마다 경로 역할을 만들고(이미 있으면 재사용) 그것을 따라가는 경로 하나를 만든다(Undo 1단계, 예전엔 N+1단계). 호·스플라인도 따라간다 |

이름: 웨이포인트 역할 `W{n}`, 경로 역할 `Route{n}`.

### 기존 기능 정리

- "스케치 점 → 웨이포인트"(`add_waypoint(robot, SketchPoint)`)는 그대로 로봇 쪽 바인딩으로 남고 자세만 역할을 따른다.
- "폴리라인 → 경로"(`path_from_sketch`)는 위 흐름으로 다시 구현. 결과 경로는 `waypoints`가 비고 `follow`가 채워진다 → **경로 패널의 단계 목록·직선 토글이 비어 보인다**(프론트가 `follow`를 보여 주도록 바꿔야 함).
- 세션의 `pose_at_point`·`frame_pose_of`는 문서로 옮겼다(`Document::sketch_point_pose`, `work_frame_pose_of`).

### 다른 브랜치 영향 (리드·프론트·QA 브랜치 `git grep` 결과)

- `MotionPath`에 `follow` 필드 추가: 구조체 리터럴은 `gc_doc/tests/cell.rs`, `gc_persist`, `gc_session` 안뿐이고 모두 고쳤다. 다른 브랜치에 새 리터럴이 생기면 `follow: Vec::new()`를 넣어야 한다.
- `Kind::SketchRole`, `Created::Role`, `CellOp` 6종 추가: 세 브랜치에 와일드카드 없는 match는 gc_doc·gc_core 안에만 있었다(고침).
- `gc_doc::Contents`에 `roles` 필드 추가(만드는 곳은 gc_persist뿐).
- `CellItem`에는 역할을 **넣지 않았다**(UI에 match가 많다). 브라우저·뷰포트 선택에 필요하면 프론트와 맞춰 추가.

### 남은 것

- **후속(리드 지정)**: 닫힌 곡선(원·전체 타원·닫힌 스플라인) 경로 — 시작점을 지정해 닫힌 경로로.
- 경로를 따라 툴이 접선 방향을 보는 옵션, 구간별 속도, 블렌드 반지름.
- 역할 편집의 미리보기(드래그 중 문서 불변) 없음 — `set_role_*`는 바로 Undo 단계가 된다.
- 9.4 작업 단계(`MoveL(웨이포인트)`, `FollowPath(경로)`)가 `Id<SketchRole>`을 참조하도록 잇는 것은 작업 모델 구현 때.

## 2026-11-18 — Bend Equal 옵션(리드 ③)

원문: To Create Lines, Arcs, and Bends, GUID-4F861674-F74C-474E-869A-4C190ED6EC4B — "(Optional) Deselect Equal to prevent automatic
constraints. If selected, the Equal option constrains successively placed bends of the same radius."

- 새 연산 `SketchOp::BendEqual { lines, to }`:
  - 반지름은 호 `to`(대화상자에서 처음 놓은 굽힘)의 반지름을 쓴다.
  - 치수 대신 `Equal(새 호, to)` 구속을 건다. 첫 굽힘의 치수를 바꾸면 둘 다 따라간다.
  - 실행 취소 한 단계. `to`가 호·원이 아니면 거부한다.
- `SketchOp::Bend`에 필드를 더하지 않은 이유: 뷰포트(`gc_viewport/src/lib.rs`)와 QA 테스트가 리터럴로 쓴다.
- 프론트 배선:
  - Bend 대화상자의 Equal 체크(기본 켬)를 둔다.
  - 켜져 있고 반지름 칸이 바뀌지 않았으면 두 번째부터 `BendEqual { lines, to: 첫 굽힘 호 }`를 보낸다.
  - 꺼져 있거나 반지름을 바꿨으면 지금처럼 `Bend`를 보낸다. 반지름을 바꿨으면 그 굽힘이 새 기준이 된다.
  - 첫 굽힘 호는 `Bend` 결과 `Created::Curve`의 첫 항목이다.
- Auto-Bend(선 그리기 중 자동 굽힘)는 원문에 Equal 언급이 없어 그대로 둔다(굽힘마다 치수).

## 2026-11-18 — BOM 기초(리드 ②)

원문: About the BOM Structure, GUID-A0A95805-CBF6-4524-8314-4AC8B53C19CF.

| 원문 | 구현 |
|---|---|
| Normal: "numbered and included in quantity calculations" | 기본값. 항목 번호, 수량 |
| Phantom: 행 아님, 자식은 상위로 올라가 수량이 곱해지고 같은 항목과 합쳐짐, 질량에는 포함 | 자식을 부모 수준으로 올려 부품 번호로 합침 |
| Reference: BOM에서 자식과 함께 무시, 질량·부피에서 제외 | 두 보기에서 빠짐. 문서 전체 `physical_properties(None)`에서도 제외 |
| Purchased: 하나의 줄. 구조 보기에는 Normal 자식이 나오고 부품 전용에서는 자식이 숨음 | 그대로 |
| Inseparable: 부품 전용에서 부품처럼 취급, Normal·Inseparable 자식은 숨고 Purchased 자식은 나옴 | 그대로 |
| 기본 BOM 구조는 문서 설정 Bill of Materials 탭 | 미구현. 컴포넌트마다 Normal이 기본 |

- 같은 부품 번호는 한 줄로 합치고 수량을 더한다. 같은 파일은 같은 부품 번호라는 인벤터 동작을 따른 것이다.
  - 부품 번호가 비어 있으면 이름을 쓴다(iProperties 원문).
  - 하위 어셈블리는 iProperties가 없어 이름이 부품 번호다.
- 구조 보기는 모든 수준(All Levels)을 보이고 항목 번호는 "1", "1.2" 꼴이다.
  - 합쳐진 줄의 자식은 첫 인스턴스 기준이다(단위당 수량).
- 부품 전용 보기에는 자식이 없는 컴포넌트와 Purchased·Inseparable 어셈블리가 나온다. 일반 어셈블리는 나오지 않는다.
- 다르게 한 점: 단위(Base Quantity "Each")와 열 사용자 정의는 없다. CSV 열은 Item, Part Number, Quantity, Description, BOM Structure 고정이다.
- 프론트용 API:
  - `session.bill_of_materials(BomView::{Structured, PartsOnly}) -> Vec<BomRow { item, level, part_number, description, quantity, structure, keys }>`
  - `session.bom_csv(view) -> String`: RFC 4180, CRLF. 수식처럼 보이는 값(`= + - @`로 시작)은 앞에 `'`를 붙인다.
  - `session.bom_structure(TreeKey)`, `session.set_bom_structure(TreeKey, BomStructure)`: 실행 취소 한 단계. 트리에 없는 행은 거부한다.
  - `gc_session::structure_name(BomStructure)`: 표시 이름.
  - 저장: `Components.bom`(Normal이 아닌 것만). 사라진 행의 값은 정리된다.
  - 공용 타입 영향: `CellOp::SetBomStructure` 추가, `Components.bom` 필드 추가(serde default). 리드·QA·프론트 브랜치에서
    `CellOp`를 빠짐없이 매치하거나 `Components { .. }` 리터럴을 쓰는 곳은 없다(grep 확인).

## 2026-11-18 — 2D 구속·치수 원문 대조(리드 다음 ①, 시작 전)

원문: To Apply Geometric Constraints GUID-A85A7A30-7D81-4C75-8769-CAD034EEA930, Sketch Dimension Reference GUID-3F4E5140-259E-45F1-AD6B-2D31416E1138,
To Work with Sketch Dimensions GUID-026FE8D3-AFBB-4834-BC18-73D9CFF78185.

| 원문 | 지금 | 할 일 |
|---|---|---|
| Fix: "click a curve, center point, midpoint, or point". 선은 위치·각도, 원·호는 중심·반지름, 타원은 위치·크기·방향 | 점만 | **아래에서 완료** |
| Collinear·Parallel·Perpendicular·Horizontal·Vertical: "lines or ellipse axes" | 타원 축은 H/V(점 쌍)만 | 타원 주축을 선으로 |
| Symmetric: "selected lines or curves … about a selected line" | 점 쌍만 | 선·원·호 쌍 |
| Tangent: 곡선 2개(타원·스플라인 포함) | 선·원·호 | 타원·스플라인 |
| Smooth (G2): 스플라인과 끝점의 곡선 | 없음 | 스플라인 끝 조건 필요(마지막) |
| Equal: 원·호·선 | 있음 | — |
| 치수: 선형(한 요소·두 요소)·정렬·각도(두 모서리·세 점·내/외각·기준선)·반지름·지름 | 있음(각도는 사분면 선택) | — |
| Flip Direction 대상의 "line to an arc (the arc itself)"·"between 2 arcs" 거리 | 없음 | 선-호·호-호 거리 치수 |
| 구동 치수 토글 | 있음(`SketchOp::SetDriving`) | — |

## 2026-11-18 — 문서 설정 마무리: 단위 해석·격자 스냅·구속 추론(리드 ①)

근거: About Units of Measure, GUID-671A8679-AEC1-413A-A23B-428967648CE6 — "When you change the units setting, all existing values in the file display as the new
units"; "You can also override the default units in any value box. For example, you can type 12mm in the dimension value box of a file using an inch unit".
Sketch Tab Reference(Document Settings), GUID-58DD9A89-84C7-48F3-8E4E-0303E755B10F(Snap Spacing).

- 단위 없이 친 값은 문서 단위로 읽는다.
  - 대상: 치수 상자, HUD, 대화상자 값(`evaluate_value`), 필렛·모따기·패턴 등 `SketchOp` 식.
  - 방법: `gc_expr::Scope`에 기본 메서드 `default_units()`를 추가했다(기존 구현은 그대로 동작). 문서 스코프가 길이·각 단위를 돌려주고, 길이 필드(mm 자리)·각 필드(deg 자리)를 그 단위로 바꾼다.
  - 단위를 쓰면 그 단위가 이긴다.
  - 매개변수 표(Param)는 자기 단위를 그대로 쓴다(`evaluate_exact`).
- 문서 단위를 바꾸면 크기가 그대로다(`command::keep_sizes`). 새 단위에서 값이 달라지는 구동 치수의 식(맨 숫자 등)은 옛 단위를 붙인 값으로 바꿔 적는다(예: `25` → `25 mm`). 한 번의 실행 취소에 함께 들어간다.
- 구속 추론 끔(`infer_constraints = false`): 그리기 도구의 Move·Click을 Ctrl을 누른 것처럼 다룬다(스냅·추론 없음). 선택 도구는 영향 없음.
- Snap to grid: `Session::set_snap_to_grid(bool)` / `snap_to_grid()`(응용 옵션, 기본 끔). 켜면 그리기 도구의 스냅 안 된 커서 위치를 문서 `snap_spacing` 격자점으로 옮긴다.
- 아직 안 됨:
  - `persist_constraints = false`: 추론 구속과 도구 고유 구속을 구분해야 해서 저장만 한다.
  - 격자 그리기(`snaps_per_minor`·`minor_per_major`): FE.
  - `input_as_expression`: FE가 입력 상자에 식/값 중 무엇을 보일지 읽으면 된다.
  - 도구 미리보기의 숫자 해석(`gc_tools` 내부)은 아직 mm·deg. 확정 값은 문서 단위로 들어간다.
- 테스트: `gc_session/tests/doc_settings.rs`(`a_bare_number_means_the_documents_unit`, `changing_units_keeps_every_size`, `inference_and_grid_shape_what_the_tools_place`).
- QA·FE·리드 브랜치 영향: 시그니처 변경 없음(트레이트 기본 메서드, 새 세션 메서드만).

## 2026-11-18 — 원-타원·타원-타원 Tangent(리드 다음 ③)

근거: GUID-A85A7A30-7D81-4C75-8769-CAD034EEA930(Tangent: "click the first curve … Click the second curve").

- `GeoConstraint::Tangent`가 타원 + 원(호)과 타원 + 타원을 받는다.
- 닫힌 식이 없어서 풀이에 숨은 접점 P(매개변수 2개)를 넣는다. 자유도 −1.
  - 원-타원: P가 원 위·타원 위, 그리고 P의 타원 법선이 원 중심을 지난다(솔버 `TangentCircleEllipse`, 새 행 `EllipseNormalThrough`).
  - 타원-타원: P가 두 타원 위, 법선이 평행(`TangentEllipseEllipse`, 행 `EllipseNormals`).
  - P는 바인딩마다 지금 모양에서 "타원 위에서 상대 곡선에 가장 가까운 점"으로 시작한다(`bind.rs::contact_start`). 그래서 바깥·안쪽 접촉은 지금 놓인 쪽을 따른다.
- 접선 도구가 타원 + 원/타원 클릭을 받는다.
- 기울기 검사에 새 행 2개를 넣었다.
- 차이: 접점이 문서에 저장되지 않고 매번 가장 가까운 점에서 다시 시작한다. 크게 끌면 다른 접촉 쪽으로 넘어갈 수 있다.
- 테스트: `gc_session/tests/ellipse_round_tangent.rs`(원 바깥·안쪽, 두 타원, 도구, 저장·불러오기).
- QA 브랜치 영향: 없음(새 솔버 변형; QA 테스트에 원-타원 접선 거부 단언 없음).

## 2026-11-18 — iProperties 기초(리드 다음 ②)

근거:
- iProperties Reference, GUID-8ABF17C5-B9EB-47B5-92CF-9FBD335334E3(Summary·Project 탭: "to classify and manage files, search for files, create reports").
- iProperties - Physical Tab - Reference, GUID-877564AC-78CE-4216-9913-003A2DE8D698 — "Calculates mass, surface area, and volume", "Center of Gravity …
  relative to the assembly origin", "The density of the default material is 1 kg/m^3", 질량·부피 사용자 재정의.

데이터(저장·실행 취소):
- `gc_doc::IProperties`
  - Summary: `title`, `subject`, `author`, `company`, `comments`
  - Project: `part_number`, `description`, `revision`, `project`, `designer`, `vendor`
  - Physical: `density: Option<f64>`(kg/m³, 없으면 1), `mass_override: Option<f64>`(kg)
- 문서 것은 Timeline, 컴포넌트 것은 `Components.properties`. 컴포넌트가 지워지면 다음 쓰기 때 정리된다(다른 상태와 같은 규칙).
- `CellOp::SetProperties { target: Option<Occurrence>, properties }`(유효성 검사, 한 번의 실행 취소), `Document::properties(target)`.

세션 API:
- `Session::{properties, set_properties, part_number}`. `part_number`는 비어 있으면 이름(원문: 파일 이름).
- `Session::physical_properties(target: Option<Occurrence>) -> Option<PhysicalProperties { mass, volume, area, centre, density, overridden }>`
  - 바디: 기본형은 해석식, 메시는 발산 정리(부호 사면체)로 부피·무게중심, 면적은 삼각형 합.
  - 로봇: 링크 시각 형상을 지금 자세로 합산.
  - 문서(`None`): 모든 바디·로봇을 각자 밀도·재정의로 합산하고 질량 가중 무게중심.
  - 메시를 아직 못 읽었거나 가져온 어셈블리·파트·링크 단위면 `None`.

FE 대화상자 필드: Summary(Title, Subject, Author, Company, Comments), Project(Part Number, Description, Revision Number, Project, Designer, Vendor), Physical(Density, Mass 재정의, 읽기 전용 Mass·Volume·Area·Center of Gravity — `physical_properties`, 단위는 `DocSettings::format_length` 등).

차이·없음: Material 라이브러리(밀도만 입력), Requested Accuracy와 상대 오차 표시, 관성 모멘트, 가져온 어셈블리 메시, Custom·Status 탭, 부피 재정의, Clipboard 보고서.

고친 것: `Components::is_empty`가 관절 조건·서브어셈블리·배치·스케치 소유자를 빠뜨려서, 그것들만 있는 문서는 저장할 때 상태가 통째로 빠질 수 있었다. 모든 필드를 보도록 고쳤다.

- 테스트: `gc_session/tests/iproperties.rs`(저장·불러오기, 실행 취소, 거부, 기본형·재정의·문서 합산, 로봇).
- QA 브랜치 영향: 없음(`Components` 구조체 리터럴 없음; 새 `CellOp` 변형).

## 2026-11-18 — 문서 설정(Document Settings, 리드 다음 ①)

근거:
- To Change Document Settings, GUID-FB49837F-7832-4713-928E-1CEDBDB0C3BD — 탭 목록. Units: "Units of measure, Modeling Dimension Display, Default Parameter Input
  Display"; Sketch: snap spacing, grid display, lineweight.
- Sketch Tab Reference(Document Settings), GUID-58DD9A89-84C7-48F3-8E4E-0303E755B10F — "3D Sketch Auto-Bend Radius".
- About Units of Measure, GUID-671A8679-AEC1-413A-A23B-428967648CE6 — "Unit value settings are saved with the file."
- 표시 모드: To Work with Sketch Dimensions, GUID-026FE8D3-AFBB-4834-BC18-73D9CFF78185(Value/Name/Expression/Tolerance/Precise Value).

데이터·API:
- `gc_doc::DocSettings`는 Timeline에 저장되고 실행 취소된다. 기본값이면 저장하지 않으므로 옛 파일은 기본값으로 읽힌다.
- `Document::settings()`, `CellOp::SetDocSettings(DocSettings)`(유효하지 않으면 거부, 한 번의 실행 취소).
- `Session::{doc_settings, set_doc_settings}`.
- 도우미: `DocSettings::{format_length, format_angle, dimension_text, length(), angle(), auto_bend_text, is_valid}`.
- 뷰포트 치수 글자가 단위·자릿수·표시 모드를 따른다(`dimension_text`).
- Auto-Bend: `Session::set_auto_bend(bool)` / `auto_bend() -> bool`(응용 옵션, 세션 값). 반지름은 문서의 `auto_bend_radius`. **API 바뀜**: 이전 `Option<String>`.

FE 대화상자 필드(Tools > Document Settings):

| 탭 | 필드 | 타입·범위 | 기본 |
|---|---|---|---|
| Units | Length | `LengthUnit::{Mm, Cm, M, In}` | Mm |
| Units | Angle | `AngleUnit::{Deg, Rad}` | Deg |
| Units | Modeling Dimension Display: Linear 자릿수 | 0–8 | 3 |
| Units | Angular 자릿수 | 0–8 | 3 |
| Units | Dimension display | `DimensionDisplay::{Value, Name, Expression, PreciseValue}` | Value |
| Units | Default Parameter Input Display: 식으로 표시 | bool | true |
| Sketch | Snap Spacing X, Y | m, > 0 | 1 mm |
| Sketch | Snaps per minor | 1–100 | 10 |
| Sketch | Major every N minor lines | 1–100 | 10 |
| Sketch | 3D Sketch Auto-Bend Radius | m, > 0 | 10 mm |
| Sketch(구속 추론) | Infer constraints / Persist constraints | bool | true / true |

아직 연결 안 됨(필드만 저장):
- 단위 없이 친 값이 문서 단위로 읽히는 것. 지금은 mm·deg로 고정되어 있고 `DocSettings::length()`로 바꿀 자리만 마련했다.
- `input_as_expression`, 스냅·격자 값(격자 표시는 FE).
- 구속 추론 두 값: FE가 `suppress_snap`이나 도구 옵션으로 넘기면 된다.

없음: Tolerance 표시(치수에 허용차 없음), Modeling·Drawing·BOM·Default Tolerance 탭, Lineweight.

- 테스트: `gc_session/tests/doc_settings.rs`, `gc_doc` 단위 테스트 `settings::tests`.
- QA 브랜치 영향: QA 브랜치의 `sketch_bend.rs`는 내 파일의 옛 사본이라 병합하면 내 쪽(`set_auto_bend(bool)`)으로 바뀐다. 다른 영향 없음(`Timeline` 구조체 리터럴 없음).

## 2026-11-18 — 스플라인 Tangent·Smooth(G2)(① 6번)

근거: GUID-A85A7A30-7D81-4C75-8769-CAD034EEA930 — "The Smooth (G2) constraint causes a continuous curvature (G2) condition between a spline and another curve, such
as a line, arc, or spline. … click the spline and the curve attached to the spline endpoint. The spline curvature adjusts as required to create a G2 condition."

- 새 `GeoConstraint::Smooth(spline, other)`. `Tangent { a, b }`도 한쪽이 스플라인이면 받는다.
- 둘 다 스플라인 끝의 모양을 정하는 조건이다(솔버에 들어가지 않음: `Sketch::shapes_spline`). 스플라인은 원래 맞춤점만 따르고 풀 것이 없다.
  - Tangent: 그 끝을 붙은 곡선의 진행 방향으로 클램프.
  - Smooth: 클램프 + 곡률 일치(끝 구간 1/3 지점에 보조 매듭을 넣고, 곡률이 맞도록 선형으로 위치를 정함).
  - 붙은 곡선: 선(곡률 0), 호(1/r, 중심 쪽), 열린 스플라인(그 자체 모양의 끝 방향·곡률).
  - `gc_sketch::{SplineEnd, shaped_spline_points3}`, `Sketch::{spline_ends, spline_ends_with}`. `spline_outline(3)`과 뷰포트 그리기가 끝 조건을 쓴다.
- 추가 조건: 스플라인 끝이 다른 곡선의 끝과 같은 점이거나 일치 구속이어야 한다. 아니면 거부. 이미 조건이 걸린 끝에 또 걸면 Redundant. 자유도는 바뀌지 않는다.
- 도구:
  - `ConstraintKind::Smooth`(새 변형): 스플라인 + 끝에 붙은 곡선 두 클릭.
  - 접선 도구도 스플라인이 끼면 이 경로를 쓴다.
  - FE: 구속 패널에 Smooth(G2) 버튼(`ribbon.rs` CONSTRAINTS)과 글리프 "G2"(paint 반영됨).
- 저장 호환: 새 변형만 추가했고 옛 파일은 그대로 읽힌다. 저장·불러오기 뒤 모양이 같은 것을 테스트로 확인.
- 차이: 원문 스플라인은 핸들(크기·방향)이 있는 곡선이다. 여기는 끝 조건만 있고 핸들 편집·장력은 없다. 닫힌 스플라인 끝 조건은 해당 없음. Trim 등으로 이음이 끊긴 조건은 남지만 효과가 없다.
- 테스트: `gc_session/tests/spline_smooth.rs`(선·호 Smooth, Tangent, 중복·미연결 거부, 저장·불러오기, 도구).
- QA 브랜치 영향: 없음(`GeoConstraint`·`ConstraintKind` 변형 추가뿐; QA 테스트에 스플라인 접선 거부 단언 없음).

## 2026-11-18 — QA: 풀리지 않는 접선이 퇴화로 수락되던 것

- 증상: 고정 직선이 고정 원·타원을 가로지르는 상태에서 Tangent를 걸면 수락되고 Solved, 두 구속이 degenerate로만 보고된다.
- 원인: 솔버가 고정선의 두 끝을 교점으로 모아 길이 0인 선을 만들고, 방향이 정해지지 않은 그 선으로 접선을 "만족"시켰다.
- 고침(`gc_solver::System::try_add`): 후보 구속이 추가 전에는 아니었는데 풀린 결과에서 퇴화(`degenerate`)로 나오면 `Conflicting`으로 거부한다. 흔적은 남지 않는다. `with`에는 함께 새로 퇴화한 구속이 들어간다.
- 테스트: `gc_session/tests/tangent_conflict.rs`(원·타원).
- QA 브랜치 영향: `qa_sketch_new_curves.rs` 312행 "fixed crossing line made tangent"는 `step`(수락 또는 거부 모두 허용)이라 그대로 통과. 관찰 메모를 "거부됨"으로 바꿔도 된다.

## 2026-11-18 — Helical Curve(리드 ③ 4번)

근거: To Draw 3D Curves, GUID-37BA1DCE-6BC4-41CD-A36D-577F31B304FC — Type(Constant/Variable), Definition(Pitch and Revolution, Revolution and Height,
Pitch and Height, Spiral), Diameter, Taper, Left/Right hand, Helix Ends(Natural/Flat), "click to define the start point of the curve and then click to define
the end point".

- `SketchOp::AddHelix { start, end, diameter, shape: HelixShape, taper, right_hand }`(3D 스케치만):
  - 축은 `start`에서 `end` 쪽. 오른손 = 축을 따라 오르며 반시계.
  - `HelixShape::{PitchRevolution, RevolutionHeight, PitchHeight, Spiral}`. Spiral은 평면, 회전마다 반지름이 피치만큼 커진다.
  - 테이퍼는 높이당 `tan(taper)`만큼 반지름 증가.
  - 결과는 회전당 8개 맞춤점의 3D 보간 스플라인(최대 4096점). 한 번의 실행 취소.
- 거부: 양수·유한이 아닌 값, 축 길이 0, 반지름이 0 이하로 줄어드는 테이퍼, 4096점 초과, 2D 스케치.
- 차이:
  - 원문은 수학적 나선이다. 여기는 맞춤점 스플라인 근사라 회전당 8점에서 반지름 오차가 반지름의 약 0.1% 이하.
  - Variable(행 표), Helix Ends(Natural/Flat 전이), 반지름 점을 시작점 평면에 투영하는 입력은 없다.
  - 결과가 일반 스플라인이라 피치 등을 다시 편집할 수 없다.
- FE: 3D Draw 패널 Helical Curve 대화상자(Helical Shape 탭 값) + 두 점 클릭 → `SketchOp::AddHelix`.
- 테스트: `gc_session/tests/helix.rs`.
- QA 브랜치 영향: 없음.

## 2026-11-18 — 치수 도구: 원 테두리 두 개(리드 화면 점검 보고)

- 보고: 원 테두리 두 개를 차례로 클릭하면 두 원 사이 거리가 아니라 지름 치수가 생긴다.
- 확인: 지금 백엔드에서는 떨어진 두 원과 한쪽이 안에 있는 두 원 모두 도구 흐름(ToolEvent)으로 `ArcArc`가 나온다. 테스트를 추가해 확인했다.
  - 보고된 동작은 `ArcArc` 커밋(8bbde33) 전 빌드이거나, 두 원이 겹친 경우와 맞는다. 겹치면 두 원 조합 치수가 없어 클릭이 첫 선택(지름)을 놓았다.
- 고침: 겹치거나 맞닿은 두 원은 중심 사이 거리(`Distance`)로 잡는다. 이제 지름으로 떨어지지 않는다.
- 테스트: `gc_session/tests/sketch_arc_distance.rs::{the_dimension_tool_on_two_circle_rims_gives_their_distance, the_dimension_tool_on_two_crossing_circles_gives_the_centre_distance}`.

## 2026-11-18 — 3D 선 Auto-Bend(리드 ③ 3번)

근거:
- Sketch Tab Reference(Application Options), GUID-1898C432-CFD2-46F1-8BAE-631392DFBF1D — "3D Sketch Auto-bend with 3D line creation. Automatically places
  tangent corner bends on 3D lines as you sketch them. Default setting is off."
- Document Settings, GUID-58DD9A89-84C7-48F3-8E4E-0303E755B10F — "3D Sketch Auto-Bend Radius. Sets the default radius for corner bends".

- `Session::set_auto_bend(Option<String>)` / `auto_bend()`: `Some("10 mm")`이면 켜짐(그 반지름), `None`이면 꺼짐(기본).
- 켜져 있으면 3D 선 도구가 이어 그리는 선마다 앞 선과의 모서리에 Bend를 건다.
  - `SketchOp::AddLineBent { line, previous, radius }`: 선 + Bend가 한 번의 실행 취소.
  - 안 들어가는 모서리(짧음·일직선)에서는 Bend 없이 선만 남는다.
- 차이: 원문은 켜고 끄는 것이 응용 옵션, 반지름이 문서 설정이다. 여기에는 아직 문서 설정 저장소가 없어서 둘 다 세션 값이다(저장되지 않음). FE가 옵션 창에 연결하면 된다.
- 테스트: `gc_session/tests/sketch_bend.rs::auto_bend_places_a_bend_at_each_corner_as_lines_are_drawn`.
- QA 브랜치 영향: 없음(새 `SketchOp` 변형).

## 2026-11-18 — 3D 보간 스플라인(리드 ③ 2번)

근거: GUID-DB15C479-66F2-456A-9E54-40A60F83DC92 — "Interpolation splines can be created in both 2D and 3D sketches".

- `SketchOp::AddSpline`이 3D 스케치에서도 된다. 맞춤점은 공간 어디든, 현 길이도 공간에서 잰다.
  - `gc_sketch::{spline_points3, closed_spline_points3}`, `Sketch::spline_outline3`.
  - 솔버: 스플라인은 맞춤점만 따르므로 3D에서도 더할 것이 없다(이전에는 `WrongSpace`).
- Close Curve는 3D에서도 된다.
- 평면에서만 뜻이 있는 것은 3D 스플라인에서 거부하거나 건너뛴다: Insert Point(거부), Extend·Trim·Split·교점(평면 곡선이 아니라 다루지 않음), Offset(3D 거부, 기존).
- 3D 도구(`gc_session/src/spatial.rs`): `ToolKind::Spline`에서 `spatial_click`이 맞춤점을 쌓는다.
  - 마지막 점을 다시 클릭하거나 `Session::spatial_finish()`(Enter)로 끝낸다. Esc는 기존 `drop_line`.
  - 미리보기는 `spatial_spline_preview()`.
- 뷰포트 `paint.rs`가 스플라인을 3D로 그린다(닫힌 스플라인 포함; 2D는 z = 0이라 같다).
- FE: 3D Draw 패널에 Spline 버튼을 두고, Enter를 `spatial_finish`에, 미리보기를 `spatial_spline_preview`에 연결하면 된다.
- 테스트: `gc_session/tests/spline_3d.rs`.
- **QA 브랜치 영향**: `qa_spline.rs` 140–141행 "a spline in a 3D sketch"가 거부를 단언한다. 원문대로 이제 받으므로 `refused`를 `step`(한 단계로 받아짐)으로 바꿔야 한다.

## 2026-11-18 — 3D Bend와 3D 끝 접선(리드 ③ 1번)

근거: To Create Lines, Arcs, and Bends, GUID-4F861674-F74C-474E-869A-4C190ED6EC4B — "The Bend tool lets you place tangent corner bends on lines in a 3D sketch.
… specify a Radius. … click any line".

- `SketchOp::Bend { lines: [l1, l2], radius: "20 mm" }`: 3D 스케치에서 모서리(같은 점, 또는 같은 자리의 끝점)로 만나는 두 선을 반지름만큼 잘라 그 평면의 접하는 3D 호로 잇는다.
  - 반지름 치수가 붙는다. 한 번의 실행 취소.
  - 두 선이 한 점을 공유하면 두 번째 선에 끝점을 따로 준다.
  - 거부: 짧아서 안 들어감, 만나지 않음, 일직선, 2D 스케치.
- 3D `Tangent`(이전에는 `WrongSpace`): 끝이 이어진 선-호(일치 구속 또는 같은 점)만 받는다. 솔버 `EndTangent3` = 이음점의 반지름에 직각(`Dot3`) + 선의 먼 끝이 호의 평면 위(새 행 `Coplanar`, 기울기 검사 포함).
- 없음(원문에 있음): Bend 대화상자의 Equal(같은 반지름 연속 구속), 선을 클릭할 때마다 연속 적용하는 도구 흐름. FE가 `ToolKind`를 만들면 두 선 클릭 → `SketchOp::Bend`.
- 테스트: `gc_session/tests/sketch_bend.rs`.
- QA 브랜치 영향: 없음(새 `SketchOp`·솔버 변형 추가뿐).

## 2026-11-18 — 3D 스케치 원문 대조(리드 ③, 시작 전)

원문:
- To Draw 3D Curves, GUID-37BA1DCE-6BC4-41CD-A36D-577F31B304FC: Helical Curve, Curve on Face, Silhouette, Intersection, 복합체 와이어.
- To Create Lines, Arcs, and Bends, GUID-4F861674-F74C-474E-869A-4C190ED6EC4B: 3D Line(Auto-Bend), 3D Arc, "Bend … tangent corner bends on lines in a 3D sketch", 반지름, Equal 옵션.
- To Create … Splines, GUID-DB15C479-66F2-456A-9E54-40A60F83DC92: "Interpolation splines can be created in both 2D and 3D sketches".

| 원문 3D 도구 | 지금 | 할 일(순서) |
|---|---|---|
| Line(+ Auto-Bend) | 선 있음, Auto-Bend 없음 | ~~3. Auto-Bend~~ **완료** |
| Arc(3점·중심) | 있음(반원 미만) | — |
| Bend(반지름, Equal) | 없음 | ~~1. 두 선의 모서리에 접하는 3D 호~~ **완료**(Equal 없음) |
| Interpolation Spline | 2D만(3D는 `WrongSpace`) | ~~2. 3D 맞춤점 스플라인~~ **완료** |
| Helical Curve(피치·회전·높이·나선, 테이퍼, 좌·우) | 없음 | ~~4. 나선~~ **완료**(맞춤점 스플라인 근사) |
| Point, 구속(평행·직각·동일·동일선상·점-선·중점·X/Y/Z 평행·평면 평행), 치수 | 있음 | — |
| Tangent(3D) | `WrongSpace` | 1번과 함께(선-호 끝 접선) |
| Curve on Face, Project to Surface, Intersection, Silhouette, Equation Curve | 없음 | 해당 없음/보류: 솔리드 면 데이터 없음. Equation Curve는 요청 시 |

## 2026-11-18 — Look At·Slice Graphics의 세션 쪽(리드 ②)

원문 대조:
- Navigation Bar, GUID-3B926737-92BA-46CF-B722-97F68651772B — Look At "zooms and rotates the model to display the selected element planar to the screen or
  a selected edge or line horizontal to the screen".
- To Work with Sketches, GUID-F5A5790A-F748-49F0-A81F-BCA56C7C1E89 — Slice Graphics: F7 또는 상태바. 복원은 "Click a feature command. Click the Finish Sketch …
  Click Slice Graphics … Press F7".

지금:
- 뷰포트에 Look At 버튼(선택한 원점 평면 또는 활성 스케치 평면만)과 F7 Slice(활성 2D 스케치 평면, 시점 쪽 절단, 스케치 종료 시 해제)가 이미 있다.
- Slice는 렌더·FE 몫이라 세션 쪽에 할 일이 없다. 피처 명령 클릭 시 해제는 FE에서 확인할 것.

새 세션 API:
- `Session::look_target(LookItem, forward) -> Option<LookTarget { toward_eye, up, centre, reach }>`.
- `LookItem::{WorkFeature, Sketch, SketchCurve, ComponentPlane, Face(Selection)}`.
  - 평면(작업 평면, 스케치, 컴포넌트 원점 평면, 평면 면, 원형 모서리의 평면): 눈이 있는 쪽에서 정면. 평면 프레임의 Y가 위, 면은 월드 Z(안 되면 Y)에 가깝게.
  - 선(작업 축, 직선 모서리, 원통 축, 3D 스케치 선): 지금 시선에 가장 가까운, 선에 직각인 방향에서 수평으로.
  - 2D 스케치 선: 스케치 위에서, 선이 수평.
  - 점·구는 `None`.
  - `reach`는 줌 맞춤용 크기(스케치는 그린 점들의 반대각선, 선은 반 길이, 원통은 반지름).
- FE: Look At 버튼이 선택(작업 피처·스케치·곡선·면)을 `look_target`으로 넘기고 카메라를 `toward_eye`·`up`·`centre`(`reach`가 있으면 줌)로 맞추면 된다.
- 테스트: `gc_session/tests/look_at.rs`.

## 2026-11-18 — 선과 타원의 Tangent(① 5번)

근거: GUID-A85A7A30-7D81-4C75-8769-CAD034EEA930(Tangent: "click the first curve … Click the second curve").

- `GeoConstraint::Tangent`가 선 + 타원(타원 호 포함)을 받는다.
  - 떨어진 선: 타원 중심의 선까지 부호 거리 = 선 쪽으로의 타원 도달 거리. 솔버 `TangentLineEllipse`, 행 `LineEllipseTangent`(지지 함수 `sqrt((n·w)² + b²(d·w/|w|)²)`).
  - 선 끝이 타원 위(호 끝, 그와 일치, 또는 점-타원 위 구속): 그 점에서 타원 법선과 직각(`EndTangentEllipse`). 일반 식은 공유점에서 미분이 0이라 자유도를 못 줄이기 때문(원과 같은 이유).
- 접선 도구가 선 + 타원 클릭에서 타원이 선 어느 쪽인지 잡아 건다.
- 기울기 검사(`analytic_gradients_match_finite_differences`)에 두 행을 더함.
- 없음: 원-타원, 타원-타원 접선. 스플라인 접선은 스플라인 끝 조건이 필요해 Smooth(G2)와 함께 한다(6번).
- 테스트: `gc_session/tests/sketch_ellipse_tangent.rs`.
- QA 브랜치 영향: 없음(새 솔버 변형; QA 테스트에 타원 접선 거부 단언 없음).

## 2026-11-18 — 선-호·호-호 거리 치수(① 4번)

근거: To Work with Sketch Dimensions, GUID-026FE8D3-AFBB-4834-BC18-73D9CFF78185 — Flip Direction이 받는 치수 종류에 "Distance from a line to an arc (the
arc itself, not to the arc center point)", "Distance between 2 arcs".

- `DimKind::LineArc { line, arc, side }`: 선에서 원·호 자체까지 거리(`side`는 원이 선의 왼쪽이면 +1).
- `DimKind::ArcArc { a, b, external }`: 두 원·호 자체 사이 거리(떨어짐, 또는 한쪽이 안).
- 솔버: `Constraint::{LineCircleDistance, CircleCircleDistance}`. 접선 행(`RowKind::LineCircleTangent`·`CircleCircle`)에 `gap` 필드를 더한 것이라 새 잔차는 없다.
- 치수 도구 판단:
  - 선 + 원(호) 클릭: 선이 원과 떨어져 있으면 `LineArc`.
  - 원 + 원 클릭: 떨어져 있거나 한쪽이 안이면 `ArcArc`. 겹치면 치수 없음.
  - 이전에는 둘 다 치수가 없었다.
- Flip Direction: `LineArc`는 원을 선 반대쪽으로, `ArcArc`는 떨어짐 ↔ 안 쪽으로 바꾼다. 뷰포트는 가장 가까운 두 점 사이 치수선을 그린다. 매개변수 표에서는 길이.
- 3D 스케치는 거부(`WrongSpace`).
- 테스트: `gc_session/tests/sketch_arc_distance.rs`.
- QA 브랜치 영향: 없음(`DimKind` 변형 추가뿐, QA 테스트에 `DimKind`·`RowKind` 빠짐없는 match 없음).

## 2026-11-18 — 선·곡선 쌍 Symmetric(① 3번)

근거: GUID-A85A7A30-7D81-4C75-8769-CAD034EEA930 — "click the first line or curve. Click the second line or curve. Click the line of symmetry." /
"Selected lines and arcs are constrained about a line to create a symmetrical shape."

- `SketchOp::SymmetricCurves { first, second, axis }`: 점 쌍 `Symmetric` 구속으로 푼다(`Sketch::symmetric_pairs`). 한 번의 실행 취소.
  - 두 선: 끝점끼리, 대칭선에 비춰 가까운 쪽과 짝짓는다.
  - 두 원: 중심 대칭 + 같은 반지름.
  - 두 호: 중심 대칭, 한쪽 시작 ↔ 다른 쪽 끝(거울은 방향을 뒤집음).
  - 이미 대칭인 쌍은 건너뛰고, 충돌하면 거부되며 흔적이 남지 않는다.
- 대칭 구속 도구가 곡선 3개(곡선, 곡선, 대칭선) 클릭을 받는다. 점 2개 + 선은 그대로.
- 대칭선을 지우면 그 구속들도 지워진다(점 쌍 Symmetric이 대칭선을 참조하므로 기존 규칙).
- 없음: 타원·스플라인 쌍(원문 문구는 "lines and arcs").
- 테스트: `gc_session/tests/sketch_symmetric_curves.rs`.
- QA 브랜치 영향: 없음(새 `SketchOp` 변형만, QA 테스트에 `SketchOp` 빠짐없는 match 없음).

## 2026-11-18 — 타원 축을 선 구속에(① 2번)

근거: GUID-A85A7A30-7D81-4C75-8769-CAD034EEA930 — Collinear "an ellipse axis", Parallel/Perpendicular "lines, curves, or ellipse axes",
Horizontal/Vertical "a line, an ellipse axis, or two points".

- `Parallel`·`Perpendicular`·`Collinear`가 타원(타원 호 포함)을 주축(중심→주축 끝) 선으로 받는다(바인딩 `axis_line`).
- Collinear는 타원 쪽 점을 선 위로 올리게 순서를 바꿔 낮춘다. 반대로 하면 축이 0으로 줄어드는 퇴화 해가 나온다.
- 수평·수직 구속 도구가 타원 클릭을 받아 중심-주축 끝 점 쌍에 건다.
- 차이: 원문은 두 축 중 어느 것이든 고를 수 있다. 여기는 주축만 쓴다(단축은 주축에 Perpendicular로 대신).
- 테스트: `gc_session/tests/sketch_ellipse_axis.rs`.
- QA 브랜치 영향: 없음(새 타입 없음; QA 테스트에 타원 Parallel 거부 단언 없음).

## 2026-11-18 — Fix를 곡선에(① 1번)

- `GeoConstraint::FixCurve(curve)`:
  - 선은 지금 자리의 숨은 고정 복사선에 동일선상(끝점은 미끄러짐, 자유도 4→2).
  - 원·호는 중심·반지름(호 끝은 원 위로 미끄러짐), 타원은 중심·축 끝·단반경, 스플라인은 모든 맞춤점.
  - 3D 스케치는 곡선의 점 전부.
- 이미 고정된 곡선에 다시 걸면 거부된다(중복).
- 구속 도구 Fix가 곡선 클릭을 받는다(`ConstraintKind::Fix` + 곡선 1개). 뷰포트 글리프 "fix"는 곡선 가운데에.
- Move·Rotate 등 변형: 엄격 모드에서는 고정 곡선을 움직이지 못하고, Relax(Always)에서는 그 Fix를 지운다(점 Fix와 같은 규칙). 복사본에는 Fix를 옮기지 않는다.
- 테스트: `gc_session/tests/sketch_fix_curve.rs`.

## 2026-11-18 — QA: 불러오기 경고가 다음 문서로 넘어감

- `new_document`·`open_recovered`가 이전 `open`의 `load_warnings`를 그대로 두던 것을 고침. 문서를 바꿀 때(`replace_document`) 비우고, `open_recovered`는 복구 파일 자체의 경고로 채운다.
- QA `gc_session/tests/qa_paste_flexible.rs::load_warnings_belong_to_the_document_that_was_opened`의 `#[ignore]` 해제. 파일은 QA 브랜치에서 가져왔고 ignore 줄만 다르다.

## 2026-11-18 — 사분점 사이 타원 Offset → 스플라인(리드 다음 ③)

근거: To Create 2D Shape Geometry, GUID-D489CE6D-7299-4211-A43A-F3580A4BA357 — "If you select an ellipse near a quadrant and an axis is visible, the offset
geometry is an ellipse. If you select an ellipse between quadrants and an axis is not visible, the offset geometry is a spline. … An offset spline is
associative to the shape of the original ellipse."

- `SketchOp::Offset`에 `picked: Option<[f64; 2]>`(serde 기본 `None`, 옛 파일 그대로)를 추가.
  - 타원을 사분점에서 10°(이심 각) 밖에서 고르면 스플라인, 안쪽이거나 `None`이면 지금처럼 타원.
  - 스플라인은 참 오프셋 곡선 위 점으로 만든다. 온 타원은 16점 닫힌 스플라인, 호는 비슷한 밀도의 열린 스플라인(끝 포함).
  - 안쪽으로 가장 작은 곡률 반지름(b²/a) 이상 들어가면 거부.
- 간격띄우기 도구가 곡선을 고른 위치를 `picked`로 넘긴다. 다른 생성 지점은 `picked: None`(QA 테스트 포함, 기계적으로만 바꿈).
- 차이: 원문의 스플라인은 원본 타원에 연관되고 둘 사이에 치수를 걸 수 있다. 여기 스플라인은 비연관이다. 연관에는 스케치 파생 형상(원본을 따라 맞춤점 갱신)이 필요해 남겨 둠.
- 테스트: `gc_session/tests/ellipse_spline_edit.rs::an_ellipse_picked_between_quadrants_offsets_to_a_spline`.

## 2026-11-18 — 스플라인 Close Curve·Insert Point·Extend(리드 다음 ②)

근거: To Create, Lock Spline Length, and Edit Splines, GUID-DB15C479-66F2-456A-9E54-40A60F83DC92 — "To add a point, right-click the spline and choose
Insert Point. Then click the curve to add one or more points." / "To close a curve, right-click the starting point and choose Close Curve."
To Split, Trim, or Extend Curves, GUID-DD00037F-6BBA-4B95-AA90-4F99DCEB667D — "the Extend command extends a curve to the nearest intersecting curve".

- `SketchOp::CloseSpline { curve }`: 맞춤점 3개 이상인 열린 스플라인을 닫는다.
  - 닫힌 스플라인은 첫 점 id를 끝에 한 번 더 둔다(`gc_sketch::spline_closed`). 저장 형식은 그대로.
  - 곡선은 주기적으로 매끄럽다(`closed_spline_points`: 앞뒤로 세 점씩 감아 붙여 맞춘 뒤 가운데를 씀).
  - `Curve::points()`는 중복 없이 돌려준다.
- `SketchOp::InsertSplinePoint { curve, at }`: 클릭에 가장 가까운 곡선 위 점에 맞춤점을 끼운다(결과 `Created::Point`). 기존 맞춤점 위면 거부. 닫는 구간에도 된다.
- `SketchOp::Extend`가 열린 스플라인을 받는다. 클릭에 가까운 끝에서 그 방향으로 곧게 나아가 처음 만나는 곡선까지 새 끝 맞춤점을 두고 묶는다. 미리보기는 그 직선(`extend_preview` → `Piece::Segment`). 끝이 다른 형상에 일치되어 있으면 거부(선·호와 같은 규칙).
- 닫힌 스플라인: Trim·Split·Split Spline·Extend는 거부. 다른 곡선의 경계로는 쓰인다. Offset하면 닫힌 스플라인이 된다.
- FE: 스플라인 우클릭 "Insert Point"(클릭마다 `InsertSplinePoint`, Esc로 끝), 시작점 우클릭 "Close Curve". 그리기 중 Close Curve는 `AddSpline` 다음 `CloseSpline`(지금은 실행 취소 두 단계).
- 차이: 원문 Extend가 스플라인을 곡률을 따라 늘리는지는 원문에 없음 — 여기서는 끝 방향 직선. 끼운 점·닫기 뒤에는 다시 맞춰서 모양이 조금 바뀐다(원문은 핸들로 유지).
- 테스트: `gc_session/tests/ellipse_spline_edit.rs`(`close_curve_and_insert_point`, `a_spline_extends_to_the_nearest_curve`).

## 2026-11-18 — 작업 피처 종류 확장(리드 다음 ①)

근거: To Create Work Features, GUID-38A9748E-FA2D-44D3-928C-DFE1326A9385(작업 평면·축·점 유형 목록). 인용 예: "Midplane Between Two Planes. … Select two
parallel planar faces or work planes. The new work plane … has the same outward normal of the first selected plane." / "Normal to Axis Through Point.
… The positive X axis is oriented from the intersection of the plane and axis to the point."

새 `WorkGeometry` 변형(FE 대화상자에 바로 붙일 수 있게 입력이 곧 필드):

| 변형 | 입력 | 원문 유형 |
|---|---|---|
| `PlaneParallelThrough { base, through }` | 평면, 점 | Parallel to Plane Through Point |
| `PlaneMidplane { planes: [_; 2] }` | 평행한 두 평면(첫 평면의 법선) | Midplane Between Two Planes |
| `PlaneAngleToPlane { base, line, angle }` | 평면, 그에 평행한 선, 각(rad; 0=평행, π/2=직각) | Angle to Plane Around Edge |
| `PlaneLineAndPoint { line, through }` | 선, 선 밖의 점 | Plane(모서리+점) |
| `PlaneNormalToLine { line, through }` | 선, 점(X축은 교점→점) | Normal to Axis Through Point |
| `PlaneNormalToCurve { curve, at }` | 스케치 호·원·타원·스플라인, 곡선 위 점 | Normal to Curve at Point(원문은 Part 전용) |
| `PointLinesIntersection { lines: [_; 2] }` | 두 선(1 µm 안에서 만나야 함) | Intersection of Two Lines |
| `PointPlaneLine { plane, line }` | 평면, 평행하지 않은 선 | Intersection of Plane and Line(원문은 Part 전용) |
| `PointThreePlanes { planes: [_; 3] }` | 세 평면 | Intersection of Three Planes |
| `PointCurveCentre { curve }` | 원·호·타원 | (리드 요청: 원 중심) |
| `AxisThroughCircle`가 타원도 받음 | | Through Center of Circular or Elliptical Shape |

- 새 참조 `LineRef::{Sketch(CurveRef), Work(축 id), Origin(OriginAxis), Component(Occurrence, OriginAxis)}`, `OriginAxis::{X, Y, Z}`. `WorkGeometry::{lines(), is_axis()}`.
- 풀리지 않는 정의(평행하지 않은 두 평면의 중간, 꼬인 두 선, 평면과 평행한 선, 곡선 밖의 점 등)는 `AddWorkFeature`/`SetWorkFeature`에서 거부(기존 규칙).
- 참조된 작업 축은 지울 수 없다(`refs`에 `LineRef::Work`·컴포넌트 포함). 저장·불러오기 됨.
- FE: `gc_app/src/work_feature.rs` 대화상자에 새 `Kind`를 붙이면 된다. 지금은 편집 시 `_ => return`이 새 변형을 건너뛴다.
- 없음: Midplane of Torus, Tangent to Surface 계열, Two Coplanar Edges, Through Revolved Face, Center of Torus/Sphere, 컴포넌트 면(메시 면) 기준.
- 테스트: `gc_session/tests/work_feature_kinds.rs`(3개).

## 2026-11-18 — 붙여넣기 고스트 미리보기(리드 요청)

Ctrl+V 뒤 복사본 모양이 커서를 따라다니게(인벤터 Paste 동작). 문서는 바꾸지 않는다.

- `Session::preview_paste(offset: Placement)`: 클립보드 내용을 `offset`에 보여 준다. 남은 복사 대상이 없거나 값이 유한하지 않으면 거부하고 미리보기도 끈다. 다시 부르면 바뀐다.
- `Session::end_paste_preview()`: 미리보기를 끈다.
- `Session::paste_ghosts() -> Vec<(AnyId, DPose3)>`: 고스트로 그릴 원본 컴포넌트와, 지금 모습에 앞에서 곱할 이동(모두 같은 `offset`, 월드 기준 — 실제 `paste`와 같은 식). 서브어셈블리는 안의 컴포넌트 전부.
- `Document::pasted_components(keys)`: Paste가 복사할 컴포넌트(`paste`와 같은 수집 코드).
- `gc_scene::Scene::of`가 고스트를 그린다. 바디는 반투명 하늘색 솔리드, 로봇은 링크·툴을 같은 색으로. `owner: None`이라 피킹되지 않는다.
- FE: Ctrl+V → 커서 위치로 `offset`을 계산해 매 프레임 `preview_paste`, 클릭하면 `paste(offset)` 다음 `end_paste_preview`, Esc면 `end_paste_preview`.
- 테스트: `gc_scene/tests/scene.rs::a_paste_preview_draws_ghosts_at_the_offset`.

## 2026-11-18 — 스플라인 교점·Trim·Split(리드 다음 ③ 2차)

근거: To Split, Trim, or Extend Curves, GUID-DD00037F-6BBA-4B95-AA90-4F99DCEB667D(Trim·Split은 가장 가까운 교차 곡선까지/에서).

- 스플라인이 교점 계산에 들어간다. 그린 폴리라인(맞춤점 사이 32단계)으로 다른 곡선(선·원·호·타원·스플라인)과 만나는 곳을 구한다.
  - `Sketch::curve_crossings`가 스플라인을 받고, 교점 스냅(`gc_tools` pick)도 스플라인 교점을 잡는다.
  - 다른 곡선의 Trim·Split·Extend는 스플라인을 경계로 쓴다(선이 스플라인까지 잘리고 늘어남).
- 스플라인 자체의 Trim과 `SketchOp::Split`:
  - 끝 쪽 Trim: 교점 너머의 맞춤점을 지우고 교점에 새 끝 맞춤점을 둔다.
  - 가운데 Trim: 두 스플라인이 된다.
  - Split: 교점에서 둘로 나누고 두 조각이 교점 맞춤점을 공유한다.
  - 새 끝점은 자른 곡선에 묶인다(`attach`: 점 일치 또는 점-곡선 위). 아무것도 안 걸치면 Trim은 통째 삭제, Split은 거부.
- `Piece::Polyline(Vec<[f64; 2]>)` 추가(스플라인 Trim 미리보기). 그래서 `Piece`는 더 이상 `Copy`가 아니다. `gc_tools` 미리보기는 폴리라인으로 그린다.
- 차이: 자른 조각은 남은 맞춤점으로 다시 맞춰서 원래 곡선과 모양이 조금 다를 수 있다(원문은 모양 유지).
  스플라인 위 점 구속은 솔버에 없어서, 스플라인이 경계일 때 새 끝점은 묶이지 않을 수 있다(구속 추가가 거부되면 조용히 생략). 스플라인 Extend는 없음.
- 테스트: `gc_session/tests/ellipse_spline_edit.rs`(`splines_cut_and_are_cut_where_they_cross`, `spline_against_spline_and_lone_splines`).

## 2026-11-18 — 타원·스플라인 Offset, Split Spline(리드 다음 ③ 1차)

근거: To Create 2D Shape Geometry, GUID-D489CE6D-7299-4211-A43A-F3580A4BA357 — "When you Offset an ellipse, the result is either a
mathematical ellipse or an associative spline. If you select an ellipse near a quadrant and an axis is visible, the offset geometry is an
ellipse. If you select an ellipse between quadrants … the offset geometry is a spline. An offset ellipse is a nonassociative mathematical
ellipse." / To Create, Lock Spline Length, and Edit Splines, GUID-DB15C479-66F2-456A-9E54-40A60F83DC92 — "To split a spline, right-click a
point and choose Split Spline."

- `SketchOp::Offset`이 타원·타원 호·스플라인을 받는다(루프에는 끼지 않고 각각 따로).
  - 타원: 같은 중심·축 방향, 두 반지름에 거리를 더한 새 타원(구속 없음 = 원문의 nonassociative). 호는 끝 각(이심 각)을 유지. 반지름이 0 이하가 되면 거부. (사분점 사이를 고르면 스플라인 — 위 ③ 항목)
  - 스플라인: 맞춤점마다 진행 방향 왼쪽(양수)으로 거리만큼 옮긴 새 스플라인(구속 없음).
  - 간격띄우기 도구(`ToolKind::Offset`)는 그대로 동작(커서로 거리·방향).
- `SketchOp::SplitSpline { curve, point }`: 안쪽 맞춤점에서 둘로. 원래 스플라인은 그 점까지, 새 스플라인(결과 `Created::Curve`)은 그 점부터. 두 조각이 점을 공유. 끝점·다른 점·스플라인 아님은 거부. 한 번의 실행 취소.
- FE: 스플라인 맞춤점 우클릭 메뉴 "Split Spline" → `SketchOp::SplitSpline`.
- 차이: 원문은 사분점 근처를 고르면 타원, 사분점 사이면 연관 스플라인 — 여기는 언제나 타원(`SketchOp::Offset`에 고른 위치가 없음). 스플라인 오프셋은 연관 아님.
  나눈 두 조각은 각자 자연 3차로 다시 맞춰져 나눈 점 근처 모양이 조금 달라질 수 있다(원문은 모양 유지).
- QA `qa_ellipse`의 Offset 거부 단언을 "거부 또는 정상, 한 단계"로 바꿈(Split·Fillet은 여전히 거부).
- 테스트: `gc_session/tests/ellipse_spline_edit.rs`(5개).

## 2026-11-18 — Flexible 서브어셈블리(리드 다음 ②)

근거: Create and edit flexible components, GUID-30817F08-A5C2-44B7-B237-4791ACA3F854 —
"right-click the placed subassembly … and then select Flexible. The freedom of movement allowed in the subassembly is available in the
parent assembly. An icon denoting the flexible state marks the subassembly in the browser." / "Click Save to preserve flexible states in the
assembly file." / "To remove flexibility, right-click a flexible subassembly, and then clear the check mark."

- **강체(기본)**: 서브어셈블리는 상위에서 한 단위. 관계 풀이에서 단위 전체가 솔버 소유자 하나(첫 컴포넌트)로 합쳐지고, 단위 안끼리의 관계는
  건너뛴다. 구성원 하나에 관계를 걸면 단위 전체가 같은 이동으로 움직인다. 자유도도 단위가 하나를 공유(구성원 하나라도 접지면 0).
  중첩: 바깥에서부터 처음 만나는 강체 서브어셈블리가 단위(`rigid_unit`). 바깥이 Flexible이면 안쪽 강체가 각각 단위.
- **Flexible**: 구성원이 상위에서 각자 움직인다(이전 동작). 클릭도 서브어셈블리가 아니라 구성원을 고른다(`pick_unit`이 통과).
- API: `SubassemblyOp::SetFlexible { id, flexible }`(한 번의 실행 취소, 레이블 "Flexible"/"Rigid", 바꾼 뒤 관계 다시 풀이),
  `Session::set_flexible(id, bool)`, `Subassembly.flexible`(저장됨, 기본 false라 옛 파일 그대로), `Document::rigid_unit(item)`,
  `rigid_unit_members(item)`, `Session::unit_moves(item, pose)`(끌기 미리보기용 — 단위 구성원 전부의 새 자세, `preview_components`에 그대로).
- FE: 브라우저 우클릭 메뉴 "Flexible" 체크 토글 + 서브어셈블리 행 아이콘(원문: 상태 아이콘). 컴포넌트 끌기는 `unit_moves` 결과로 미리보기.
- 차이/남음: 제자리 편집(edit in place) 중에도 강체 단위의 내부 관계는 단위로 합쳐 푼다(인벤터는 하위 어셈블리 파일 안에서 풀지만 여기는 한 문서).
  Flexible은 서브어셈블리 인스턴스별 상태 — 여기는 서브어셈블리가 문서 안 그룹이라 그룹별.
- 테스트: `gc_session/tests/subassembly.rs::rigid_subassemblies_move_as_one_and_flexible_ones_let_members_move`.

## 2026-11-18 — 컴포넌트 복사·붙여넣기(Ctrl+C/V, 리드 다음 ①)

근거: About Placing Components(GUID-249C50F9-5A8C-48BD-8341-D1BDB7A16B19)의 "Use Copy and Paste to place new instances using the same orientation".
- `SubassemblyOp::Paste{keys, offset, into}`:
  - 셀에 놓인 컴포넌트와 서브어셈블리를 새 인스턴스로 만든다. 같은 방향으로 놓고 `offset`만큼 옮긴다. 관계는 복사하지 않는다.
  - 서브어셈블리는 구조째로 복사한다(중첩 포함). 새 id(`next_subassembly`)와 빈 이름 `이름:N`을 준다.
  - 맨 위 복사본은 `into` 레벨에 넣는다. 로봇은 툴째로 복사된다(`Robot` 복제).
  - 실행 취소 한 단계다. 없어진 행은 건너뛰고, 남은 것이 없으면 `BadValue`.
- `replicate::copy_mapped`: 원본→복사본 짝을 돌려주는 복사. 기존 `CopyComponents`와 같은 인스턴스 규칙(이름 `:N`, 상태 복사, 접지 안 함).
- 세션:
  - `copy_to_clipboard(&[TreeKey])`: 부품·링크·없는 행은 빼고, 남는 것이 없으면 `BadValue`.
  - `clipboard()`.
  - `paste(offset) -> Vec<Created>`: 서브어셈블리를 제자리 편집 중이면 그 안으로, 아니면 최상위로 넣는다.
  - 클립보드는 세션 상태다(문서·저장과 무관). OS 클립보드와는 연결하지 않는다.
- 테스트: `gc_session/tests/subassembly.rs::copy_and_paste_places_new_instances_and_keeps_subassembly_structure`.

## 2026-11-18 — 긴급: 손상된 조인트 범위로 인한 패닉(QA)

- 원인: `JointRange::target`이 `f64::clamp(start, end)`를 썼다. 손상·손편집된 파일에서 Start > End(또는 NaN)면 패닉했다.
- `gc_math::clamp_between(value, a, b)`: 경계 순서와 상관없고, 경계가 NaN이면 값을 그대로 둔다. 패닉하지 않는다.
  데이터에서 오는 경계를 쓰는 곳을 이것으로 바꿨다.
  - 조인트 범위의 목표, 조인트 Drive.
  - 로봇 관절 한계로 깎기(`gc_session` 두 곳, `gc_robot/src/ik.rs`). URDF가 손상돼 lower > upper일 수 있다.
  - 재생 시간·궤적 시간·호 길이(`gc_session` 재생, `gc_traj` 다섯 곳). 길이가 NaN일 때 대비.
  - `gc_traj`는 `gc_math` 의존이 없어서 같은 함수를 crate 안에 두었다(Cargo.lock 변화 없음).
- 불러오기: `gc_persist::load`가 조인트 범위의 `is_valid()`를 검사한다. 잘못된 범위는 버리고(한계 없음) `Loaded.warnings`에 남긴다.
  열기는 계속한다. 세션 `load_warnings()`(상태 표시줄용, 프론트엔드).
- 검색 결과: 나머지 `.clamp(`는 상수 경계뿐이다(0..1, -1..1, 개수 상한 등).
- 테스트: `gc_session/tests/joint_limits.rs::a_damaged_joint_range_in_a_file_is_dropped_not_a_panic`, `gc_math` `clamp_between_never_panics`.
  QA 테스트 `qa_joint_limits.rs::a_damaged_joint_range_in_a_file_does_not_crash_the_solver`가 통과해서 `#[ignore]`를 풀었다.

## 2026-11-18 — 스플라인(보간, 리드 다음 ②)

근거: To Create, Lock Spline Length, and Edit Splines(GUID-DB15C479-66F2-456A-9E54-40A60F83DC92).
- 보간 스플라인은 맞춤점들을 지난다.
- 첫 점은 클릭하거나 기존 점을 클릭한다. 이어서 클릭하면 맞춤점이 된다.
- 스플라인 점에는 구속을 일부 또는 전부 걸 수 있다.

**범위(리드 결정대로)**: 열린 보간 스플라인 하나, 맞춤점 = 스케치 점. 끝 접선 핸들·제어점 스플라인은 P3다.

**모델**
- `CurveGeom::Spline{points: Vec<Id<SkPoint>>}`. 그래서 `CurveGeom`과 `Curve`에서 `Copy`를 뺐다
  (`.clone()` 10여 곳, `map(|c| c.geom.clone())`).
- `Curve::points()`는 맞춤점 전부다. 그래서 지우기·이동·복사·패턴·미러가 맞춤점을 따라간다.
- 모양: `gc_sketch::spline_points(fit, per_span)`. 현 길이 매개변수의 자연 3차 스플라인이고 맞춤점을 정확히 지난다(점 2개면 선분).
  `Sketch::spline_outline(geom, n)`.
- 솔버: 스플라인은 맞춤점이 정하므로 따로 더할 것이 없다(맞춤점마다 자유도 2). 3D 스케치에서는 `WrongSpace`.
- 미지원(거부하거나 무시): Trim·Extend·Split·Fillet·Offset·교점 스냅·접선, Close Curve, 핸들, Insert Point, Tension, 치수 직접(맞춤점에 건다).
- `SketchOp::AddSpline{points, snaps}`: 점 2개 미만, 연속한 같은 점, 유한하지 않은 값은 `InvalidValue`. 맞춤점마다 스냅 구속을 건다.
- 도구 `ToolKind::Spline`:
  - 클릭할 때마다 맞춤점(스냅 포함). Enter나 마지막 점을 다시 클릭하면 마친다. Esc는 버린다.
  - 미리보기는 지금까지의 점과 커서를 지나는 곡선이다.
  - 프롬프트 `SplineFirst/Next`, i18n `ToolSpline`·`PromptSpline*`. 리본은 선▾ split에 한 줄 넣었다(아이콘 "~").
- 그리기·피킹·상자 선택·드래그(맞춤점 전부를 몸통으로)를 지원한다.
- 저장: 모델을 그대로 직렬화한다. 스플라인이 없는 옛 파일은 그대로 열린다.
- 테스트: `gc_persist/tests/spline.rs`(맞춤점 통과, 자유도, 구속, 복사·삭제, 거부, save∘load), `gc_session/tests/spline_tool.rs`.

## 2026-11-18 — 타원 Trim·Extend, 교점·사분점 스냅(리드 다음 ①)

근거: To Create 2D Shape Geometry(GUID-D489CE6D-7299-4211-A43A-F3580A4BA357) "You can trim and extend ellipses like any other curve".
- 모델: `CurveGeom::Ellipse`에 `arc: Option<(시작, 끝)>`을 추가했다(serde default, 옛 파일 그대로).
  타원호는 이심각(eccentric angle)으로 반시계 방향이다. 바인딩이 두 끝점을 그 타원 위에 둔다
  (`PointOnEllipse`, 끝점 id로 키를 단다). `Sketch::ellipse_arc`.
- cut.rs `Shape::Oval{c, u, a, b, arc}`:
  - 매개변수는 이심각이다.
  - 선과의 교점: 타원을 단위원으로 펴는 좌표에서 선-원으로 정확히 구한다.
  - 원·타원과의 교점: 둘째 곡선 전체를 720칸으로 돌며 첫째의 음함수 부호가 바뀌는 곳을 이분법으로 찾는다. 접하기만 하는 교점은 못 찾는다.
- Trim:
  - 온 타원(교점 2개 이상)은 남는 타원호가 되고, 끝은 자른 곡선에 붙는다.
  - 타원호의 가운데를 자르면 두 타원호가 되고 중심·축 끝 점을 공유한다. 단반경은 `Equal`로 묶는다(두 타원의 Equal = 단반경 같음).
  - 끝쪽을 자르면 끝이 옮겨진다.
- Extend: 타원호의 열린 끝을 다음 곡선까지 늘린다. `Piece::EllipseArc`(미리보기)와 `Piece::polyline(n)`(모든 조각을 점열로)을 추가했다.
- 스냅:
  - `Sketch::curve_crossings(a, b)`: 모든 곡선 쌍. pick.rs는 타원이 낀 쌍에 이것을 쓴다.
  - `Sketch::ellipse_quadrants(id)`: 축 끝 4점 중 곡선 위의 것. Quadrant 추론으로 쓰고, 수평·수직 정렬 표시(`aligned`)는 붙이지 않는다.
- 그리기: `ellipse_points(c, m, b, ends, n) -> (점열, 닫힘)`, `ellipse_param`, `Sketch::ellipse_geom_outline`, `gc_tools::push_polyline`.
  미리보기·뷰포트·피킹이 타원호를 그린다.
- 프론트엔드 참고: `Preview.cut`(Trim이 지울 조각)은 지금 UI 어디에서도 그리지 않는다(원래부터). `Piece::polyline`으로 그리면 된다.
- 남은 것: Split·Fillet·Chamfer·Offset의 타원은 아직 거부한다. 타원 접선 구속·접하는 교점도 없다.
- 테스트: `gc_persist/tests/ellipse.rs`(자르기·나누기·늘리기·save∘load, 원-타원 교점).

## 2026-11-18 — 타원(리드 ④)

근거: To Create 2D Shape Geometry(GUID-D489CE6D-7299-4211-A43A-F3580A4BA357)의 Ellipse.
1=중심, 2=첫 축 끝, 3=타원 위 점(둘째 축 길이).

**모델·솔버**
- `CurveGeom::Ellipse{c, major, minor}`: 중심 점, 장축 끝 점(장반경 = 둘의 거리, 축 방향), 단반경 값.
  `Curve::points()` = [c, major]. 3D 스케치에서는 `WrongSpace`.
- gc_solver: `Constraint::PointOnEllipse{p, c, major, minor}`, 행 `RowKind::PointEllipse`.
  잔차는 `(|(x/a, y/b)| − 1)·b`(길이 단위)이고, 장반경이나 단반경이 0에 가까우면 퇴화로 표시한다. 기울기 검사 테스트에 넣었다.
- 단반경은 원 반지름처럼 반지름 변수다(`binding.radii`).
  - `DimKind::Radius(ellipse)` = 단반경 치수. 장반경은 `DimKind::Distance{c, major}`로 잰다.
  - `Sketch::round()`는 타원에 `None`을 돌려준다. 그래서 접선·지름 치수는 거부된다.
- 구속: 중심 일치(점), `PointOnCurve(p, ellipse)` → 점-타원 위, 주축 수평·수직 = `Horizontal`·`Vertical(c, major)`.
- 미지원(거부하거나 무시): Trim·Extend·Split·Fillet·Chamfer·Offset의 타원, 교점 스냅, 사분점 스냅, 타원 접선. 원문은 "You can trim and extend ellipses like any other curve"라서 Trim·Extend는 원문과 다른 점이다.
- 복사·패턴·미러·이동·축척은 점을 옮긴다(축척은 단반경도 곱한다). 자동 치수는 장반경 거리와 단반경 반지름을 넣는다.
- `gc_sketch::ellipse_points(c, m, b, n)`, `Sketch::ellipse(id)`, `Sketch::ellipse_outline(id, n)`: 그리기·피킹용 점열.

**문서·도구·UI**
- `SketchOp::AddEllipse{centre, major, minor, centre_snap}`: 축 길이가 0이거나, 단반경이 0 이하이거나, 값이 유한하지 않으면 `InvalidValue`.
- `ToolKind::Ellipse`(shapes.rs 흐름): 중심 → 첫 축 끝(중심선 미리보기) → 커서(타원 미리보기) → 클릭.
  단반경은 커서와 첫 축 사이 거리다. 프롬프트 `EllipseCentre/Axis/Minor`, i18n `PromptEllipse*`, `ToolEllipse`.
- 리본: 원▾ split에 한 줄 넣었다(리드 지시대로). 프론트엔드 화면 쪽은 최소한만 넣었다.
  - `gc_viewport` 그리기·상자 선택: 큰 반경의 원 상자로 잡는다.
  - `gc_panels` 속성 줄은 없음.
- 저장: 모델을 그대로 직렬화한다. 새 변이 하나이고, 타원이 없는 옛 파일은 그대로 열린다.
- 테스트: `gc_persist/tests/ellipse.rs`(구속으로 완전 정의, 수직 축, 점-위 유지, 거부, save∘load),
  `gc_session/tests/ellipse_tool.rs`(세 클릭, 미리보기, 프롬프트, Esc).
- 스플라인은 리드 결정대로 보류했다.

## 2026-11-18 — 서브어셈블리 통째 이동(리드 ③)

근거: Create Subassemblies in Place(GUID-FCEE3F0E-AAE2-4619-8645-2588FEECD353)의 "treated as a single unit in the parent assembly".
Grip Snap도 서브어셈블리를 움직일 수 있다(Move and Rotate Assembly Components, GUID-BB632466-C77C-4BFB-9C81-67653D1C2ED3).
- `Document::subassembly_components(id)`: 그 서브어셈블리 아래(깊이 상관없이) 셀에 놓인 컴포넌트들, 모델 순서.
- 세션 `subassembly_moved(id, delta: DPose3) -> Option<Vec<(AnyId, DPose3)>>`: 단위 드래그 미리 보기.
  각 컴포넌트를 `delta * 지금 자세`로 옮긴다. 없거나 비었거나 고정된 멤버가 있으면 `None`.
- 세션 `move_subassembly(id, delta)`: 커밋한다(`MoveComponents`, 실행 취소 한 단계). 묶인 다른 컴포넌트는 따라온다.
- 그립 스냅으로 서브어셈블리를 움직일 때는 `grip_snap_pose_on`으로 한 멤버의 새 자세를 구한다.
  `delta = 새 자세 × 지금 자세⁻¹`를 `move_subassembly`에 넘기면 된다.
- 남은 것: 서브어셈블리 안 관계로 멤버끼리 움직이는 "유연(flexible)" 서브어셈블리는 없다(모두 강체 단위).
- 여러 컴포넌트 동시 미리보기(프론트엔드 요청): `preview_components(&[(AnyId, DPose3)])`, `end_component_preview()`.
  Drive 미리보기와 같은 표시 경로라서 `driven_placement`, `shown_body`, `shown_base`에 나타난다. 문서·실행 취소에는 손대지 않는다.
  컴포넌트가 아니거나 자세가 유한하지 않으면 `BadValue`. 단위 드래그는 미리 보기로 `subassembly_moved` → `preview_components`, 놓을 때 `move_subassembly`.
- 테스트: `gc_session/tests/subassembly.rs::a_subassembly_moves_as_one_unit`.

## 2026-11-18 — 그립 스냅 점·면 기준(리드 ②)

근거: Grip Snap Move Options Bar Reference(GUID-C030C32C-7E1E-4F37-BBDF-120412898D00), "Reference Geometry …" 행들.
- 점: Displacement(두 점의 방향·크기), Rotate(두 점의 외적 축, 내적 각).
- 선: Displacement Along Ray, Rotate(이미 있음).
- 면(평면 면·작업 평면): Displacement Within Plane(두 점 성분 중 평면 안 성분), Along Normal(법선 성분).

**API**
- `GripReference::{Point, Line(GripLine), Plane{point, normal}}`.
- `GripMotion`에 `Displace`, `RotateAboutPoint`, `MoveWithinPlane`, `MoveAlongNormal`을 추가했다.
- `grip_pose_on(pose, motion, reference, from, to)`, `Document::grip_snap_pose_on`, 세션 `grip_snap_pose_on`.
  기준 종류와 안 맞는 움직임, 퇴화한 기준, 축이 안 나오는 점은 `None`이다.
- `grip_snap_allowed(item, motion, reference, from, to)`: 그 스냅의 실제 변화(이동 + 회전 벡터)가 남은 자유도로 되는지
  1차로 본다. 접지면 false다. 기존 선 전용 `grip_allows`는 그대로 둔다.
- Free Drag·스냅점 계열(커서 위치로 놓기, 임시 구속)은 화면 몫이라 넣지 않았다. 원통·구 등 해석 면 기준도 아직이다.
- 테스트: `gc_session/tests/grip_snap.rs::point_and_plane_references_move_and_turn_as_the_options_bar_says`.

## 2026-11-18 — 조인트 한계(Limits, 리드 ①)

근거: To Define and Manage Joint Relationships(GUID-21DC3336-5C51-42C1-90FB-4299CD66E0C6) Limits 탭.
- Angular·Linear 가능 여부는 조인트 종류가 정한다. Rotational은 Angular만, Rigid는 없음.
- Start·Current·End 값을 넣는다. 한계가 있는 관계는 브라우저에 +/- 표시가 붙는다.
- 구속(Mate·Flush 등)의 최소·최대·휴지 위치(`Relationship.limits`)는 예전부터 있다. 이번 일은 조인트 쪽이다.

**모델**
- `JointCondition`에 `angular`, `linear: Option<JointRange{start, end, current: Option<f64>}>`를 추가했다.
  Lock·Protect와 같은 곳(`Components.joint_conditions`)에 저장한다. 옛 파일은 비어 있는 채로 열린다.
- 값의 뜻(`joint_values`): angular는 둘째 원점 Z축 둘레로 첫째 X축이 돈 각(라디안), linear는 둘째 Z축을 따라 첫째 원점이 미끄러진 거리(m).
- 솔버: 조인트 잔차 뒤에 범위마다 한 행을 더한다. `current`가 있으면 그 값에서 벗어난 만큼(자유도 1개를 없앰),
  없으면 범위 밖으로 벗어난 만큼(안에서는 0, 자유도를 없애지 않음). 각도는 감아서 잰다.
- `takes_limits(kind, angular)`: angular는 Rotational·Cylindrical, linear는 Slider·Cylindrical. Ball·Rigid·구속은 안 받는다.
- `CellOp::SetJointLimits{id, angular, linear}`: 종류에 안 맞거나, Start > End, Current가 범위 밖이거나 유한하지 않으면 `BadValue`.
  적용 뒤 관계를 다시 맞추고, 안 맞으면 거부한다. 실행 취소 한 단계다. `SetJointCondition`(Lock·Protect)은 범위를 보존한다.
- Drive: 한계가 있는 조인트를 구동하면 범위 안으로 깎은 값을 `current`로 둔 미리 보기를 낸다(각도 범위가 우선).
  그래서 Slider·Cylindrical도 한계가 있으면 구동된다. 한계가 없는 조인트는 예전 동작 그대로다(Rotational은 상대 회전).
- `Document::joint_position(id) -> Option<(angle, slide)>`: 대화상자의 Current 표시용.

**세션(프론트엔드)**
- `set_joint_limits(id, angular, linear)`, `joint_limits(id) -> (Option<JointRange>, Option<JointRange>)`(브라우저 +/- 표시), `joint_position(id)`.

**원문과 다른 점·남은 것**
- 원문의 Current는 지금 위치를 바꾸는 값이다. 우리는 `current: None`이면 그 자리에 두고 범위만 지킨다.
- 로봇 관절 한계는 로봇 설명(URDF, `gc_robot`)에서 오고 관계 한계와는 따로다.
- 테스트: `gc_session/tests/joint_limits.rs`.

## 2026-11-18 — 치수 상자의 `이름 = 값`을 한 단계로(프론트엔드 요청)

- `Command::Sequence(Vec<Command>)`: 명령 여러 개를 한 트랜잭션에서 차례로 실행한다. 실행 취소 한 단계이고,
  하나라도 거부되면 전체를 거부한다. 레이블은 마지막 명령의 것이다. 빈 목록은 `BadValue`.
- `session.edit_dimension_with_param(dim, name, value)`: 사용자 매개변수 `name = value`를 만든다(각도 치수면 deg, 아니면 mm).
  그리고 치수 식을 `name`으로 바꾼다. 실행 취소 한 단계다. 이름이 틀리거나, 이미 있거나, 값이 틀리면 아무것도 남지 않는다.
- 근거: To Work with Sketch Dimensions(GUID-026FE8D3-AFBB-4834-BC18-73D9CFF78185)의 "enter HGHT = 5mm".
- 테스트: `gc_session/tests/dimension_param.rs`.

## 2026-11-18 — 관계에 어셈블리 작업 피처(프론트엔드 요청)

근거: To Place Mate or Flush Constraints in Assemblies(GUID-0F500089-4F69-41C5-AFE8-B58AC9E8E15E)의 "Select the first point, curve, face, or work feature".
Place Constraint 참조(GUID-EBD2C784-0E71-452A-B5F2-22738D22D45A)는 Angle의 셋째 선택으로 "work plane, or work axis"를 받는다.

**설계**
- 작업 피처 선택은 `Selection{occurrence: Occurrence::Item(<작업 피처 id>), geometry}`다.
  `Geometry`에 새 종류를 넣지 않아서 기존 형상 매치와 저장 포맷이 그대로다. 옛 파일은 영향이 없다.
- 솔버에서 작업 피처는 월드에 고정된 소유자다(항상 고정, 자세 = 단위).
  형상은 풀 때마다 `work_shape`에서 다시 읽는다(평면 = Plane, 축 = Axis, 점 = Point).
  그래서 작업 피처를 고치면(`SetWorkFeature`, 이제 관계를 다시 맞춤) 묶인 컴포넌트가 따라온다. 못 따라오면 그 편집을 거부한다.
- `AddRelationship`·`SetRelationship` 검사는 `relatable(occ)`를 쓴다: 컴포넌트이거나, 모양이 있는 작업 피처면 된다.
- 관계가 쓰는 작업 피처를 지우면 `InUse`다(예전에는 커밋 때 `Dangling`). `Relationship::references(item)` 추가.

**API(프론트엔드)**
- `session.work_feature_selection(id) -> Option<Selection>`: 작업 평면·축·점을 관계의 첫째나 둘째 선택으로 쓴다.
  `possible_relations`, `predict_relationship`, `add_relationship`에 그대로 넘긴다.
- 작업 피처는 컴포넌트가 아니다: `components()`·`component_tree()`에 나오지 않고 DOF·접지 대상도 아니다.

**남은 것**
- 테스트: `gc_session/tests/work_feature_relations.rs`.

## 2026-11-18 — 그립 스냅 계산(우선순위 표 11 일부, 백엔드는 계산 함수까지)

근거: Grip Snap move or rotate(GUID-E1CA109A-D5A1-4131-AC3A-AE217716DD8C).
- 모서리를 기준으로 고른 뒤 "Move along ray using reference geometry"나 "Rotate about line using reference geometry"를 고른다.
- 점 2개로 오프셋이나 각도를 정한다. HUD가 그 선에 대해 남은 자유도를 알려 준다.
- Done으로 끝내거나, Commit and Ground로 움직인 것을 고정한다.

**gc_doc**
- `GripLine{point, direction}`, `GripLine::through(a, b)`: 모서리 두 끝점으로 만든다. 길이가 0이면 `None`.
- `GripMotion::{MoveAlong, RotateAbout}`.
- `grip_pose(pose, motion, line, from, to)`, `Document::grip_snap_pose(item, …)`:
  - 이동: `to − from`의 선 방향 성분만큼 옮긴다.
  - 회전: 선을 따라 본 `from → to` 각도만큼 선 둘레로 돌린다.
  - 점이 선 위에 있거나 값이 유한하지 않으면 `None`.
- `Document::grip_allows(item, motion, line)`: 관계가 남긴 자유도로 그 움직임이 되는지 본다(HUD 보고).
  관계 야코비안 × 움직임 속도(twist) ≈ 0인지로 판정한다. 접지면 false, 관계가 없으면 true.
- `CellOp::MoveAndGround(Vec<(AnyId, Placement)>)`: Commit and Ground. 실행 취소 한 단계다.

**세션(프론트엔드가 드래그 UI에 쓸 것)**
- `grip_snap_pose(item, motion, line, from, to) -> Option<DPose3>`: 미리 보기.
- `grip_allows(item, motion, line)`: HUD.
- `grip_snap_commit(&[(item, pose)], ground)`: Done(`false`, `MoveComponents`)이나 Commit and Ground(`true`).
  여러 컴포넌트를 한 번에 넘길 수 있다(세션 동안 움직인 것들). 묶인 컴포넌트는 따라온다.

**원문과 다른 점**
- 임시 구속을 녹색으로 보여 주는 것과 이동 옵션 바는 화면 몫이다.
- 기준 형상은 모서리(선)만 받는다. 원문의 점·면 기준 이동은 아직 없다.
- 테스트: `gc_session/tests/grip_snap.rs`.

## 2026-11-18 — 어셈블리 스케치 소유자(우선순위 표 11 일부)

근거: Sketches in assemblies(GUID-2C180C3A-2064-4671-B1C2-F74B383B5D39).
- 어셈블리에서 만든 스케치는 최상위 어셈블리 Origin 폴더 아래에 놓인다. 다른 컴포넌트 형상을 투영할 수 있고, 스케치 구속을 쓴다.
- 부품 피처용 스케치는 그 부품(제자리 편집, GUID-C79C36C9)에 속한다.

**모델**
- `Components.sketch_owners: Vec<(Id<Sketch>, TreeKey)>`: 제자리 편집 중에 만든 스케치와 그 소유자(컴포넌트나 서브어셈블리).
  목록에 없는 스케치는 최상위 어셈블리 소유다. 타임라인 상태라서 저장·실행 취소된다.
- `Command::Owned{owner: Option<TreeKey>, command: Box<Command>}`: 스케치를 만드는 명령에 소유자를 붙인다. 실행 취소 한 단계다.
  없는 소유자면 `BadValue`.
- 소유자가 사라지면(해체·삭제) 그 스케치는 최상위로 간다. 정리는 `set_timeline`에서 하고, 질의는 존재하는 소유자만 돌려준다.
- 질의: `Document::sketch_owner(id) -> Option<TreeKey>`(`None` = 최상위), `sketches_of(owner)`.
- 리드 결정(QA 관찰): 컴포넌트를 지우면 그 컴포넌트와 부품이 소유한 스케치를 같은 트랜잭션에서 함께 지운다(`delete_owned_sketches`).
  `DeleteBody`, `DeleteRobot`, `DeleteAssembly`가 이 규칙을 따르고, 실행 취소 한 단계로 모두 돌아온다.
  다른 것(피처, 작업 피처 등)이 그 스케치를 쓰면 `InUse`로 거부한다. 전에는 `Doc(Dangling)` 오류가 났다.
  QA 테스트 `qa_sketch_owner.rs`의 삭제 거부 단언은 새 동작(`deleting_a_component_deletes_the_sketches_it_owns`)으로 바꿨다.
- `CreateSketch`·`CreateSketchOn`의 이름은 앞뒤 공백을 뗀다. 비면 첫 빈 `SketchN`을 붙인다(`sketch_name`).
  중복 이름은 아직 막지 않는다. 세션은 지금도 `Sketch{개수+1}`을 넘긴다.

**세션**
- `create_sketch`, `create_sketch_on_*`, `create_sketch_3d`는 편집 대상이 있으면 그것을 소유자로 붙인다(`execute_owned`).
- 컴포넌트를 제자리 편집하는 중에 `create_sketch("XY"|"XZ"|"YZ")`를 부르면 그 컴포넌트의 원점 평면(`PlaneSource::Component`)에 만든다.
  그래서 스케치가 컴포넌트를 따라 움직인다. 서브어셈블리를 편집할 때는 원점이 따로 없어서 문서 원점 평면에 만든다.
- `sketch_owner(id)`, `sketches_of(owner)`: 브라우저에서 스케치를 최상위 Origin 폴더 아래 둘지, 컴포넌트 아래 둘지 정할 때 쓴다(프론트엔드).

**원문과 다른 점**
- 피처가 소비한 스케치를 피처 아래로 옮기는 것은 브라우저 표시 몫이다(타임라인에 이미 피처가 스케치를 참조함).
- "어셈블리 스케치는 다른 피처와 공유할 수 없음"은 따로 검사하지 않았다.
- 테스트: `gc_session/tests/subassembly.rs::sketches_belong_to_the_assembly_or_to_the_component_being_edited`.

## 2026-11-18 — 서브어셈블리 계층·제자리 편집(우선순위 표 11 일부) — 세션 API 확정

근거 원문:
- Create Subassemblies in Place(GUID-FCEE3F0E-AAE2-4619-8645-2588FEECD353): Component Demote, 브라우저 드래그로 레벨 이동,
  부모에서 서브어셈블리는 한 단위, 여러 층 중첩.
- Edit components in place(GUID-C79C36C9-A6DB-46C5-8864-41F66C750231): 더블클릭으로 편집, 활성 컴포넌트만 음영·나머지 투명,
  Return / Return to Parent / Return to Top.

**모델(gc_doc)**
- `Components.subassemblies: Vec<Subassembly{id: SubassemblyId, name, parent}>`, `Components.placed_in: Vec<(Occurrence, SubassemblyId)>`.
  타임라인 상태라서 실행 취소되고 저장된다. 옛 파일은 비어 있는 채로 열린다(serde default).
- 서브어셈블리는 `Occurrence`가 아니다. 관계·원점·충돌은 지금처럼 컴포넌트 단위이고, 그룹은 브라우저·선택·편집 범위에만 쓴다.
- `CellOp::Subassembly(SubassemblyOp)`, 각각 실행 취소 한 단계:
  - `Demote{members: Vec<TreeKey>, name}`: 같은 레벨의 컴포넌트·서브어셈블리를 새 서브어셈블리로 묶는다.
    `Created::Subassembly(id)`를 돌려준다. 빈 이름이면 `Subassembly:N`, 이미 있는 이름이면 `NameTaken`.
  - `Move{members, into: Option<SubassemblyId>}`: 브라우저 드래그. `None`이 최상위다. 자기 자신이나 자기 하위로 넣으면 거부한다.
  - `Dissolve(id)`: 담긴 것은 부모로 올라간다.
  - `Rename{id, name}`.
- 옮길 수 있는 것은 셀에 놓인 컴포넌트(바디·로봇·가져온 어셈블리)와 서브어셈블리뿐이다. 부품·링크·툴은 각자의 트리에 그대로 있다.
- 컴포넌트를 지우면 소속도 정리된다(`set_timeline` 정리).
- 질의: `component_tree() -> Vec<TreeRow{key: TreeKey, parent, depth, name, component: Option<ComponentNode>}>`.
  레벨마다 서브어셈블리(만든 순서) 다음 컴포넌트(모델 순서)이고, 각 컴포넌트 뒤에 부품·로봇 링크·툴이 들여쓰기되어 붙는다.
  그 밖에 `tree_parent(key)`, `has_tree_row`, `is_under(key, ancestor)`, `subassemblies()`, `subassembly_of(occ)`.

**세션 API(프론트엔드가 화면 상호작용에 쓸 것)**
- `component_tree()`: 브라우저 들여쓰기용. 서브어셈블리 행은 `component == None`.
- `edit_subassemblies(SubassemblyOp)`: Demote 메뉴, 드래그 이동, 해제, 이름 바꾸기.
- `edit_component(TreeKey)`: 더블클릭. 없는 행이면 `BadValue`.
- `edit_target() -> Option<TreeKey>`: 지워졌거나 실행 취소된 대상은 `None`.
- `return_edit()`(Return), `return_to_parent()`, `return_to_top()`.
- `in_edit_scope(Occurrence) -> bool`: false면 흐리게 그리고 선택하지 않는다.
- `pick_unit(Occurrence) -> Option<TreeKey>`: 클릭한 컴포넌트를 담은 행 중 편집 대상 바로 아래 행이다(서브어셈블리를 한 단위로 선택).
  편집 대상 밖이면 `None`이다. 가져온 어셈블리의 부품이나 로봇 링크는 그 자신이다.
- 편집 대상은 세션의 보기 상태다(저장·실행 취소 안 함).
- QA 수정: 서브어셈블리 id는 `Components.next_subassembly`로 단조 증가해서, 해체한 id를 다시 쓰지 않는다.
  선택(`select_occurrence`, `select_item`)은 편집 범위 밖 컴포넌트를 거부한다.
  `selected_occurrence`, `selected_item`도 범위 밖 선택은 없는 것으로 본다. 그래서 들어가기 전에 고른 것으로도 접지·삭제가 안 된다.

**원문과 다른 점·남은 것**
- 서브어셈블리를 드래그로 통째로 옮기기(유연성 없는 단위 이동)는 아직 없다. 지금은 컴포넌트마다 따로 움직인다.
- Demote 때 원문은 새 파일 이름·템플릿·BOM 구조를 묻는다. 우리는 한 파일 안의 그룹이라서 이름만 받는다.
- 제자리 편집 중 피처 편집(Edit Feature)은 해당 없음(우리 컴포넌트는 메시·기본 도형).
- 테스트: `gc_session/tests/subassembly.rs`.

## 2026-11-18 — 관계 사슬 성능(KNOWN_ISSUES "컴포넌트 사슬 관계 추가가 급격히 느려짐")

- `solve_relationships_holding`은 섬마다 먼저 **단계 풀이**(`staged`)를 한다. 고정 컴포넌트에서 너비 우선으로 한 컴포넌트씩 놓는다.
  고정 컴포넌트가 없는 섬은 첫 컴포넌트를 세워 둔다. 각 단계는 이미 놓인 컴포넌트와의 관계만 6변수로 푼다.
  이미 만족하면 건너뛴다(이전 해를 초기값으로 씀).
  끝나고 섬의 관계가 하나라도 안 맞으면(루프, 나중 컴포넌트가 있어야 정해지는 경우, 충돌) 시작 자세로 되돌린다.
  그다음 예전처럼 섬 전체를 푼다. 진단(`sick`, `conflicting_relationships`)은 그대로다.
- 섬 전체 풀이(`least_squares_on`)를 희소하게 바꿨다.
  - 야코비안: 움직인 컴포넌트에 닿는 관계만 다시 계산한다.
  - 정규 행렬: 블록 조립, 너비 우선 순서의 envelope(skyline) 저장, envelope Cholesky(`envelope_solve`)로 푼다(예전: 밀집 가우스 소거).
  - 멈춤 규칙은 예전 그대로(200회 또는 개선 없음). 정체 시 멈춤은 진단 목록을 바꿔서(QA) 뺐다.
- 측정(QA `relationship_build_times`, 디버그): 30개 사슬 426 ms → 0.8 ms.
  새 테스트 `gc_session/tests/relationship_chain.rs`(접지된 바탕 위에 Mate·Rigid를 번갈아 건 100개 사슬, 1 s 상한, 디버그에서 통과).
  그 사슬 끝에 충돌 관계를 넣으면: 릴리스에서 거부 48 s → 54 ms, 진단 106 ms.

## 2026-11-18 — QA 버그 3건(작업 축, qa_joint_assemble_axes.rs)

- `CellOp::AddWorkFeature`·`SetWorkFeature`는 넣은 뒤 `work_shape`가 `None`이면 `BadValue`로 거부한다.
  평행한 두 평면의 교선 축, 길이 0인 선 위 축이 이렇게 거부된다. 같은 직선 위 세 점을 지나는 평면도 이제 거부된다
  (`gc_session/tests/cell.rs`의 기존 단언을 바꿈).
- 스케치 작업(`Command::Sketch`)이 작업 피처가 쓰는 곡선이나 스케치 점을 없애면 `EditError::InUse`로 거부한다.
  작업 전에 있던 참조만 본다. 그래서 이미 끊긴 참조가 있는 옛 파일의 스케치도 계속 편집할 수 있다.
  피처가 쓰는 스케치 자체의 삭제 거부와 같은 방식이다.
- `WorkGeometry::sketch_points()`를 추가했다. 테스트 3개의 `#[ignore]`를 풀었다.

## 2026-11-18 — QA 버그 3건(transform.rs): 고정 곡선 이동, 투영 선 이동, 중복 복사

- relax 없이 Move·Rotate·Stretch하면(`MoveRules::break_ties == false`) 움직일 점(일치 구속으로 함께 가는 점 포함) 중에
  Fix 구속이 있거나 참조점(`PointKind::External`)이 있으면 `SketchError::Unsolved(Inconsistent)`로 거부한다.
  예전에는 Fix가 점을 따라가서 고정 곡선도 움직였다. 투영된 선은 끝점이 Fix라서 같은 규칙으로 거부된다.
  근거: To Copy, Move, or Rotate 2D Sketch Geometry(GUID-F8114270-18F7-4C7A-947E-AC489EDACFA9)의 기하 구속 "Never"인
  "The operation obeys all existing geometric constraints".
- `copy_placed`: 곡선 목록의 중복을 없애고 복사한다(한 번만 복사).
- relax(기하 구속 "Always")로 이동하면 원문 "Deletes only fixed constraints that are associated with the selected geometry"대로
  움직이는 점의 Fix 구속을 지우고 움직인다. 참조점(`External`)은 relax여도 거부한다.
  QA 테스트 `copying_a_fixed_curve_leaves_the_original`의 "rotate, relax 후 제자리" 가정은 원문과 달라서 뺐다.
  대신 `relaxed_move_of_a_fixed_curve_deletes_its_fix`를 추가했다.
  아직 원문과 다른 점: "Always"에서 선택·비선택 사이 구속을 지울 때 Move는 Parallel·Perpendicular·Equal을, Rotate는 Equal을 남겨야 하는데,
  지금은 수평·수직만 따로 다룬다(`break_level`).
- `gc_session/tests/qa_sketch_transform_auto.rs`의 `#[ignore]` 3개를 풀었다.

## 2026-11-18 — QA 버그 2건(qa.md 151행): 패턴 방향선 삭제, HUD 식

- **패턴 방향선 삭제**: `Sketch::prune_patterns`가 원본 곡선뿐 아니라 직사각형 패턴의 방향선(`first.line`, `second.line`)과
  원형 패턴의 축 점도 확인한다. 하나라도 없으면 패턴 기록을 풀고, 단계 작도선·구속·간격 치수만 지운다. 복사본 곡선과 점은
  일반 곡선으로 남긴다. 실행 취소 한 번이면 패턴이 돌아온다.
  근거: Sketch Patterns 원문(GUID-AAD3A699-FED8-4614-A31D-0A2316328CEE)의 "If you remove the associative relationship among
  pattern elements, the geometry is no longer a pattern, but geometric elements that you can edit individually".
  원형 패턴의 축 점은 패턴의 바큇살 작도선이 쓰고 있어서 그 점만 지우면 지워지지 않는다(기록이 매달리지 않음).
- **HUD 식**: 세션이 `ToolEvent::Hud`로 들어온 글을 문서 매개변수 범위로 평가한다(`gc_doc::evaluate_value(doc, text, unit)`,
  `NAME = 식` 형태도 받음). 결과는 `Tool::set_hud_value(field, Option<f64>)`로 도구에 넘긴다.
  단위는 각도 → deg, 배율 → 단위 없음, 나머지 → mm. 평가에 실패하면 도구가 예전처럼 글을 직접 읽는다(`plain_value`).
  유한하지 않은 값은 버린다. 입력한 글은 그대로 치수의 식으로 남는다. `gc_doc`이 `Unit`(gc_expr)을 다시 내보낸다.
  근거: Dynamic Input 원문(GUID-33A8B11D-A04C-4809-BC82-C0419215915F)은 "enter a value"까지만 말한다. 식·매개변수는
  Sketch Dimensions 원문(GUID-026FE8D3-AFBB-4834-BC18-73D9CFF78185)의 "enter or edit an expression that defines a parameter"를 근거로 삼았다.
  입력한 값은 치수가 되므로 같은 식 규칙을 따른다.
- 테스트: `gc_session/tests/qa_sketch_pick_arcslots.rs`에서 `#[ignore]` 2개를 풀었다. 새 테스트 2개:
  `deleting_the_guide_releases_the_pattern_and_keeps_the_copies`, `hud_boxes_take_units_in_expressions_and_parameter_names`.

## 2026-11-18 — 작업 축 정의 보강(우선순위 표 11 일부)

- 원문: To Create Work Features `GUID-38A9748E-FA2D-44D3-928C-DFE1326A9385.htm`, Create Work Axes.
  - On Line or Edge, Parallel to Line Through Point, Through Two Points, Intersection of Two Planes, Normal to Plane Through Point(파트 전용), Through Center of Circular or Elliptical Shape, Through Revolved Face or Feature.
- `WorkGeometry` 추가:
  - `AxisOnLine{line: CurveRef}`, `AxisParallelThrough{line, through: PointRef}`, `AxisPlanesIntersection{planes}`, `AxisThroughCircle{curve}`.
  - `CurveRef{sketch, curve}`는 스케치 곡선을 가리키고 스케치를 따라간다. 참조된 스케치는 지울 수 없다.
  - 원 축: 2D 스케치는 원·호 중심을 지나는 스케치 법선, 3D 호는 세 점 평면의 법선.
  - 검사: 선 자리에 원, 원 자리에 선, 같은 평면 두 번은 거부한다.
- 원문과 다른 점: 모델 모서리·회전 면은 없다. 메시라 정확한 모서리·원통 면이 없어서, 스케치 선·원(투영된 선 포함)만 받는다.
- 테스트: `gc_doc/tests/design.rs` work_axes_on_a_line….

## 2026-11-18 — 조인트 Lock·Protect(우선순위 표 11 일부)

- **원문**: To Define and Manage Joint Relationships `GUID-21DC3336-5C51-42C1-90FB-4299CD66E0C6.htm`.
  - Lock: 현재 위치를 유지해 모든 운동을 없애지만, 관련 컴포넌트가 움직이면 따라간다. 고정(Ground)과 다르다.
  - Protect: 추가 관계가 필요한 자유도를 침해하면 경고한다.
  - 구속(Constraint)에는 둘 다 없다.
- **gc_doc**:
  - `Components::joint_conditions: BTreeMap<Id<Relationship>, JointCondition{locked: Option<LockedPose>, protected}>`. 직렬화 기본값이 있어 예전 파일도 열린다.
  - `CellOp::SetJointCondition{id, lock, protect}`: 조인트가 아니면 `BadValue`.
  - Lock은 잠그는 순간 첫 조인트 원점을 둘째 원점 좌표로 저장(`LockedPose`)하고, 잠긴 동안 잔차가 그 상대 자세(원점·Z·X)를 요구한다. 그래서 따라 움직이고 자유도는 0이다.
  - Protect: 관계 추가·편집 전후로 보호된 조인트의 움직이는 컴포넌트 자유도를 비교해, 줄면 `EditError::Protected(이름)`으로 거부한다.
  - 조인트를 지우면 조건도 정리된다(`set_timeline`, `prune_states`).
  - **원문과 다른 점**: 원문은 Protect 침해 시 경고한 뒤 계속할 수 있게 한다. 우리 앱에는 경고 대화상자가 없어서 거부를 유지한다(리드 결정, 2026-11-18). 경고 대화상자가 생기면 `EditError::Protected`를 확인 후 강행하는 경로로 바꾼다.
- **세션**: `joint_condition(id)`, `set_joint_lock(id, on)`, `set_joint_protect(id, on)`.
- **상태 표시줄**: Protected 거부 문구(ErrProtected).
- **UI**: 브라우저 관계 우클릭 Lock/Protect 체크는 프론트엔드 몫(모델 트리 메뉴).
- **원문과 다른 점**: 원문은 경고(warn)라고 했지만 여기서는 거부한다(되돌릴 단계가 남지 않게).
- **테스트**: `gc_session/tests/qa_joints.rs` lock_and_protect_a_joint.

## 2026-11-18 — 스케치 축척·신축(우선순위 표 6 나머지)

- **원문**: To Stretch, Scale, or Offset 2D Sketch Geometry `GUID-962CEE71-8BE4-4DA6-B51E-341319C64FA4.htm`. 이동과 같은 흐름(선택 → 기준점 → 끝점, 미리보기, More), 축척 비율은 직접 입력할 수 있다.
- **gc_sketch**:
  - `MoveRules{break_ties, break_level, recompute_crossing, recompute_inside, radius}`, `move_with(curves, points, place, rules)`. 이동·회전은 `MoveRules::moving`을 쓴다.
  - 선택 점과 일치로 묶인 점도 함께 옮기고, 치수를 다시 계산할지 판단할 때도 함께 센다. 직사각형의 변 끝이 그렇다.
- **gc_doc**:
  - `Motion::Scale{centre, factor}`: 원 반지름도 배율. 풀면(relax) 선택 안의 치수도 새 값.
  - `SketchOp::Stretch{curves, points, by, relax}`: 연결을 끊지 않아 이어진 형상이 늘어난다. 풀면 경계를 넘는 치수가 새 값.
- **도구**:
  - `ToolKind::{Scale, Stretch}`.
  - 축척 비율 = 기준점에서 커서까지 ÷ 기준점에서 가장 먼 선택 점까지. 세션이 `set_tool` 때 선택 점을 넘긴다. `HudField::Scale`에 숫자로 입력할 수 있다.
  - 신축은 `ToolOutput::Stretch(by)`를 내고, 세션이 선택한 곡선과 따로 선택한 점(`selected_points_only`)으로 명령을 만든다.
- **UI**: 리본 '수정' 패널 4×2(간격띄우기·분할·이동·축척 / 모따기·복사·회전·신축). 이동 창을 같이 쓴다(복사 상자는 이동·회전에만). 1280 px에서 취소 버튼이 1261에서 끝난다.
- **원문과 다른 점**:
  - 원문은 커서로 정하는 축척 비율의 기준을 적지 않았다. 여기서는 가장 먼 선택 점까지의 거리를 1로 본다.
  - 풀지 않은 신축을 치수가 막으면, 자유로운 형상은 늘어나는 대신 통째로 미끄러진다(구속은 지킴).
- **테스트**: `gc_doc/tests/sketch_shapes.rs` scale_and_stretch…, 세션 scale_and_stretch_tools, 앱 kittest scale_and_stretch_on_the_ribbon.

## 2026-11-18 — 치수 방향 뒤집기(우선순위 표 9)

- **원문**: To Work with Sketch Dimensions `GUID-026FE8D3-AFBB-4834-BC18-73D9CFF78185.htm`. 치수 우클릭 Flip Direction(여러 개 가능)으로 형상이 반대쪽으로 간다.
  - 대상: 평행선 거리, 점-선 거리, 두 점의 수평·수직 거리, 선-호, 호-호, 각도.
  - 선택에 대상이 아닌 것이 있으면 메뉴가 안 나온다.
- **gc_doc**:
  - `SketchOp::FlipDimensions(ids)`: `PointLine.side`, `HDistance/VDistance.sign`의 부호를 바꾸고, 각도는 두 선을 맞바꿔 반대로 잰다. 그다음 풀이한다.
  - 대상이 아닌 종류가 섞이면 거부한다. `gc_doc::flipped(kind)`.
- **세션**: `selected_dimensions()`, `can_flip_selected()`, `flip_selected_dimensions()`.
- **리본**: 구속 패널 끝 작은 버튼 '방향 뒤집기'. 뒤집을 수 있는 치수가 선택됐을 때만 켜진다(우클릭 메뉴 대신).
- **원문과 다른 점**:
  - 선-호·호-호 거리 치수는 이 앱에 그런 종류가 없다.
  - 풀이가 반대쪽 해로 못 가면 거부한다.
- **테스트**: `gc_doc/tests/sketch_shapes.rs` flip_direction…(점-선 거리), 앱 kittest flip_direction_on_the_ribbon(수직 거리).

## 2026-11-18 — 자동 치수 및 구속(우선순위 표 8)

- **원문**: To Apply 2D Sketch Dimensions Automatically `GUID-1DC71C6F-9F6D-4846-8E73-9CB7D3452882.htm`. Auto Dimension 대화상자: Curves(선택 없음 = 전체), Dimensions/Constraints(기본 켬), Dimensions Required, Apply/Remove/Done.
- **gc_doc `SketchOp::AutoDimension{curves, dimensions, constraints}`**:
  - 구속을 먼저 넣는다: 거의 수평·수직인 선, 겹친 끝점의 일치.
  - 그다음 치수: 선 길이·원 지름·호 반지름, 그리고 각 점의 스케치 원점에서 가로·세로 거리.
  - 값은 지금 잰 값이다. 후보를 하나씩 시도해 자유도를 줄이는 것만 남긴다(중복 검사).
- **세션**:
  - `auto_dimension(dimensions, constraints)`: 선택한 곡선, 없으면 전체.
  - `remove_auto_dimensions(dimensions, constraints)`: 마지막 적용분.
  - `dimensions_required()`: 남은 자유도. 선택과 관계없이 스케치 전체 기준이다.
- **UI**: 리본 구속 패널 끝에 '자동 치수 및 구속' 작은 버튼, '자동 치수' 창(곡선 수, 치수/구속, 필요한 치수, 적용/제거/완료).
- **원문과 다른 점**:
  - 창 안 Curves 선택기가 없다. 창을 열기 전에 고르거나, 열린 뒤 선택 도구로 고른다.
  - 필요한 치수는 선택이 아니라 스케치 전체 기준이다.
  - 치수 배치·종류 선택은 인벤터와 다를 수 있다(원문에 규칙이 없음).
- **테스트**: `gc_doc/tests/sketch_shapes.rs` automatic_dimensions…(자유도 0, 위치 그대로, 수평 추론, 구속만), 앱 kittest auto_dimension_applies_and_removes.

## 2026-11-18 — 면 위 스케치에서 면 모서리 자동 투영(우선순위 표 7 일부)

- **원문**:
  - Sketch Tab Reference(Application Options) `GUID-1898C432-CFD2-46F1-8BAE-631392DFBF1D.htm`: "Autoproject edges for sketch creation and edit".
  - About Projecting Sketch Geometry `GUID-B61431DA-FABC-41EE-945C-6E54B4806582.htm`: 모델의 평면 면에 스케치를 만들면 면의 모든 모서리가 참조로 들어온다.
- **세션**:
  - `create_sketch_on_face`가 스케치를 만든 뒤, 고른 삼각형과 같은 평면에서 이웃으로 이어진 삼각형 영역의 윤곽을 `ProjectedLines`(고정 선)로 넣는다. 윤곽은 영역 안에서 한 번만 쓰인 모서리이고, 곧은 줄은 이어 붙인다.
  - 메시는 꼭짓점이 겹쳐 있을 수 있어 나노미터로 반올림해 맞춘다.
  - 옵션: `autoproject_face_edges()/set_autoproject_face_edges(on)`, 기본 켬. 응용 프로그램 옵션 화면은 FE.
  - 윤곽은 별도 실행 취소 단계다. Cancel Sketch는 스케치와 함께 되돌린다.
- **원문과 다른 점**:
  - 원문은 자동 투영 참조를 지울 수 없다. 여기서는 고정 선이라 지울 수 있다.
  - 원본이 바뀌어도 따라가지 않는다(Project Cut Edges와 같은 스냅샷).
  - 편집할 때(enter_sketch) 다시 투영하지 않는다.
  - "Autoproject edges during curve creation"(선을 문질러 투영)은 아직 없다.
- **테스트**: `gc_session/tests/sketch_planes.rs` a_sketch_on_a_face_takes_the_face_outline(정사각형 면 = 선 4개, 대각선·옆면 제외, 취소, 옵션 끔).

## 2026-11-18 — 스케치 이동·복사·회전(우선순위 표 6)

- **원문**: To Copy, Move, or Rotate 2D Sketch Geometry `GUID-F8114270-18F7-4C7A-947E-AC489EDACFA9.htm`.
  - 선택 → 기준점(회전은 중심점) → 끝점 순서. 미리보기가 있고, 회전 각도는 직접 입력할 수 있다.
  - 이동·회전에는 Copy 체크가 있다.
  - More에서 치수 완화(Never/If No Equation/Always/Prompt)와 구속 끊기(Never/Always/Prompt)를 고른다.
- **gc_sketch `transform.rs`**:
  - `copy_placed`: 곡선·점과 그 안끼리의 구속(고정 제외)을 복사한다. 돌린 복사본에 맞지 않는 수평·수직은 빠진다.
  - `move_placed`: 선택한 점을 목표 위치에 임시 고정한 채 풀고, 이어진 것이 따라오게 한다. 풀리지 않으면 거부한다.
  - relax: 선택과 바깥을 잇는 구속을 끊고(회전이면 선택 안의 수평·수직도), 선택과 바깥을 잇는 치수는 이동 후 잰 값으로 다시 쓴다(식 → 값).
- **gc_doc**: `Motion::{Translate{by}, Rotate{centre, angle}}`(`place`), `SketchOp::Transform{curves, motion, copy, relax}`.
- **도구**:
  - `ToolKind::{Move, Copy, Rotate}`는 두 번 클릭 후 `ToolOutput::Transform(motion)`을 낸다. 회전은 HUD 각도 입력이 이긴다.
  - 세션이 현재 선택한 곡선과 창 옵션(`transform_options`: Copy, relax)으로 명령을 만든다.
  - 미리보기: 세션 `preview()`가 선택을 옮긴 자리에 그린다.
- **UI**:
  - '이동'·'복사'·'회전' 창: 선택 수, 복사본으로(이동·회전), 구속 풀기, 완료.
  - 스케치 명령 창들은 이제 오른쪽 리본 아래에 처음 뜬다(리본을 가리지 않게).
  - 리본 '수정' 패널: 간격띄우기·분할·이동 / 모따기·복사·회전.
  - 작성 패널의 슬롯(중심 대 중심)은 작은 버튼 3×2 묶음으로 옮겼다. 1280 px에서 마무리·취소까지 1230에서 끝난다.
- **원문과 다른 점**:
  - 창에 선택 모드가 없다. 명령 전에 고르고, 창은 선택 수만 보여 준다.
  - More는 상자 하나(Always/Never). If No Equation·Prompt는 없다.
  - Precise Input, Optimize for Single Selection, Clipboard 붙여넣기, Backspace 되돌리기는 없다.
  - 기준점의 축 스냅(Ctrl로 끄기)은 일반 스냅 그대로다.
- **테스트**: `gc_doc/tests/sketch_shapes.rs` move_copy_and_rotate…(이동·Never 거부·Always 치수 갱신·회전·복사 자유도), 세션 move_copy_and_rotate_tools…, 앱 kittest move_copy_and_rotate_on_the_ribbon….

## 2026-11-18 — 분할(Split)·2D 모따기(Chamfer)(우선순위 표 5)

- **원문**:
  - To Split, Trim, or Extend Curves `GUID-DD00037F-6BBA-4B95-AA90-4F99DCEB667D.htm`: 2D에서 곡선을 가장 가까운 교차 곡선에서 나누고, 두 조각이 원래 구속을 잇는다.
  - Create Chamfers(`GUID-D489CE6D-7299-4211-A43A-F3580A4BA357.htm`): 2D Chamfer 대화상자. Create Dimensions, Equal to Parameters, Equal Distance / Unequal Distance / Distance and Angle.
- **gc_sketch `cut.rs`**:
  - `split_preview`, `split_at`: 클릭에 가장 가까운 교차점 한 곳에서 자른다. 기존 Trim의 내부 `split`을 틈 없이 쓰고, 두 조각을 일치로 잇는다. 선은 동일선상, 호는 같은 원.
  - `chamfer_plan`, `chamfer`: 모서리와 집은 쪽 판정은 모깎기와 같다. 두 선을 거리만큼 줄이거나 늘이고 새 선으로 잇는다.
- **gc_doc**:
  - `SketchOp::Split{curve, at}`.
  - `SketchOp::Chamfer{lines, kind: ChamferKind::{Equal, Unequal, DistanceAngle{distance, angle}}, dimensions}`. 크기는 식이다.
  - 치수를 켜면 모서리에 구성용 점을 두 선 위에 두고, 그 점에서 각 자른 점까지 거리 치수를 단다. 거리와 각도 형식은 둘째 선 거리 + 첫 선과 모따기 선 사이 각도 치수.
- **도구·UI**:
  - `ToolKind::{Split, Chamfer}`. Split은 마우스를 올리면 자를 곳에 × 표시. Chamfer는 첫 선 → 둘째 선, 모따기 선을 미리 보여 준다.
  - 세션 `chamfer_options()/set_chamfer_options()`는 도구를 바꿔도 유지된다.
  - gc_app '2D 모따기' 창: 치수 만들기, 세 형식, 크기, 확인 = 도구 끝.
  - 리본 '수정' 패널은 작은 버튼 두 줄(간격띄우기·분할 / 모따기)로 바꿨다. 인벤터는 모따기가 작성 패널에 있지만, 1280 px에 맞추려고 이렇게 했다.
- **원문과 다른 점**:
  - Equal to Parameters는 없다(모따기마다 그때 값).
  - 3D Split(면·작업 평면까지)은 없다. Split의 오른쪽 클릭 Trim/Extend 전환도 없다.
  - 원 분할은 거부한다(교차점 한 곳으로는 원을 나눌 수 없음).
- **테스트**: `gc_doc/tests/sketch_shapes.rs` split_cuts…(교차점·자유도·세 형식·치수 구동), 세션 split_and_chamfer_tools, 앱 kittest split_and_chamfer_on_the_ribbon….

## 2026-11-18 — 중점 선·접선 원·중심 직사각형 2종(우선순위 표 4)

- **원문**: To Create Lines, Arcs, and Bends `GUID-4F861674-F74C-474E-869A-4C190ED6EC4B.htm`(Midpoint Line), To Create 2D Shape Geometry `GUID-D489CE6D-7299-4211-A43A-F3580A4BA357.htm`(Two/Three Point Center Rectangle, Tangent Circle).
- **명령(gc_doc)**:
  - `AddMidpointLine{mid, end, snap_mid, snap_end}`: 선 + 중점에 놓인 점(Midpoint 구속). 그 점이 스냅 대상과 일치 등을 받는다.
  - `AddCenterRectangle{centre, corner, snap_centre}`, `AddCenterRectangle3{centre, side, width, snap_centre}`: 기존 직사각형 명령 + 구성 대각선 + 그 중점의 중심 점.
  - 자유도: 2점 4, 3점 5 — 기존 직사각형과 같다.
  - `AddTangentCircle{lines, near}`: 세 선(무한선)에 접하는 원(최대 4개) 중 접점이 클릭 위치에 가장 가까운 것 + 접선 구속 3개.
- **도구**:
  - `ToolKind::{MidpointLine, CenterRectangle, CenterRectangle3, TangentCircle}`, `Prompt` 7개.
  - 미리보기는 같은 명령을 사본에 적용해 그린다(`draw_new`).
- **리본**:
  - 작성 패널에 작은 버튼 3×2를 넣었다(점·중점 선·접선 원 / 다각형·2점·3점 중심 직사각형).
  - 점·다각형은 큰 버튼에서 작은 버튼으로 옮겼다. 그대로면 1280 px에서 '스케치 마무리'(1231–1331)와 '스케치 취소'가 창 밖으로 밀렸다. 지금은 취소가 1275에서 끝난다.
  - 더 넣으려면 인벤터식 분할 버튼(선 ▾·원 ▾·직사각형 ▾·슬롯 ▾)이 필요하다(FE).
- **원문과 다른 점**:
  - 원문의 Tangent Circle 문장은 중심점 원과 같은 문장이 반복되어 모호하다. parity 문서의 "세 선에 접하는 원"을 따랐다.
  - 중점 선의 HUD 길이·각도 입력은 없다.
- **테스트**: `gc_doc/tests/sketch_shapes.rs` midpoint_lines_center_rectangles…(모양·자유도·내접원), 세션 midpoint_line_center_rectangles…_tools, 앱 kittest the_ribbon_starts….

## 2026-11-18 — Cancel Sketch(우선순위 표 3)

- 원문: To Create and Edit Sketches `GUID-E96FB3C4-136B-4551-BF45-F54594543ECB.htm`, Cancel a Sketch. 새로 만들던 스케치면 만든 것까지, 편집 중이면 그 편집에서 한 일을 모두 버린다.
- **세션 `cancel_sketch() -> bool`**:
  - 스케치에 들어갈 때 실행 취소 깊이를 기록한다(새 스케치면 만든 단계 하나 앞).
  - 취소하면 그 깊이까지 되돌리고, 다시 실행 목록을 비우고(`History::drop_redo`), 스케치를 나간다.
  - 기록이 200단계 한도로 잘린 경우는 `History::trimmed()`로 기준을 옮긴다. 기록이 시작점까지 닿지 않으면 남은 만큼 되돌리고 false를 돌려준다.
- **리본**: 종료 그룹에 '스케치 취소' 작은 버튼.
- **테스트**: 세션 cancel_sketch_discards…(새 스케치·기존 스케치 편집), 앱 kittest cancel_sketch_on_the_ribbon….

## 2026-11-18 — 3D 호: 3점 호·중심점 호(우선순위 표 2)

- **원문**: To Create Lines, Arcs, and Bends `GUID-4F861674-F74C-474E-869A-4C190ED6EC4B.htm`.
  - 3D Sketch 탭 Draw 패널에 Three Point Arc(두 끝, 호 위의 점)와 Center Point Arc(중심, 시작, 끝)가 있다.
- **모델**:
  - 3D 호도 `CurveGeom::Arc{c, s, e}` 그대로다. 세 점의 평면 위에서 시작 → 끝으로 짧은 쪽으로 돈다.
  - 그래서 **3D 호는 반원 미만**이다. 세 점만으로는 더 큰 호를 구별할 수 없고, 원문에는 이 제한이 없다.
  - 3점 호의 세 번째 점이 긴 쪽에 있으면 거부(`InvalidValue`)한다.
  - `gc_sketch::Arc3`(new, through, at)를 그리기·고르기·미리보기가 함께 쓴다.
- **gc_solver**: `Constraint::Arc3{c, r, start, end}`, 행 `PointSphere`(|p − c| − r). 호 하나 = 자유도 8(평면 3, 평면 안 중심 2, 반지름, 두 각도).
- **gc_sketch**:
  - 3D 호 바인딩. 3D 원은 아직 `WrongSpace`(법선이 없어서).
  - 3D 호의 반지름 치수·지름 치수·같음(반지름)은 반지름 매개변수만 쓴다(`radius_only`).
  - `Sketch::round`의 호 반지름을 3D 거리로 바꿨다(2D는 z = 0이라 같음).
- **명령·세션**:
  - `SketchOp::AddSpatialArc{centre, start, end, snaps}`.
  - 세션 `spatial_click`이 `ToolKind::Arc`(중심점)와 `Arc3`(3점)에서 세 점을 모은다. `spatial_arc_preview()`가 호 고무줄을 준다.
  - Esc는 진행 중인 호를 버린다.
- **뷰포트**:
  - 3D 호를 3D로 그리고, 3D 선택에서 호를 고를 수 있다.
  - 3D 클릭 경로가 호 도구도 받는다.
  - 3D 치수 글자: 반지름 `R…`, 각도 `…°`. 반지름·각도 치수의 글자 위치도 추가했다.
- **리본**: 3D 스케치 작성 패널에 호(중심점)·3점 호.
- **테스트**: 세션 three_dimensional_arcs…(기울어진 평면, 자유도 8, 반지름 치수, 긴 쪽 거부), 앱 kittest(3D 리본의 호 버튼).
- **남음**: 3D 원, 반원 이상 3D 호(법선 저장 필요), 3D 접선, Bend.

## 2026-11-18 — 3D 스케치 구속: 평행·직각·동일선상·동일·점-선 위·중점·각도(우선순위 표 1)

- **원문**: To Apply Geometric Constraints `GUID-A85A7A30-7D81-4C75-8769-CAD034EEA930.htm`.
  - 3D Sketch 탭 Constrain 패널에 Coincident(점-곡선 포함), Collinear, Parallel, Perpendicular, Equal, Fix, 축·평면 평행이 있다.
  - Tangent는 3D에서 스플라인이 먼저여야 해서 해당 없음.
- **gc_solver**:
  - `Line3` 추가.
  - 구속: `Constraint::{Parallel3, Perpendicular3, Collinear3, EqualLength3, PointOnLine3, Midpoint3, Angle3}`.
  - 행: `Cross3`, `Dot3`, `EqualLen3`, `PointLine3`, `Angle3`. 각도는 atan2(|u×w|, u·w)로 0~180°.
  - 평행·점-선은 외적 성분 중 방향의 가장 큰 축을 뺀 두 개(내리기 때문에 정함)라서, 행 수가 곧 없애는 자유도다.
  - 외적·내적·점-선은 현재 길이로 나눈다(사인·코사인·거리). 고정 길이로 나누던 첫 구현에서는 솔버가 선을 점으로 줄여 동일선상을 "만족"시켰다.
  - `MAX_VARS` 8 → 12(3D 선 두 개). solver perf 보고(release)는 100·300 미지수에서 차이 없음.
- **gc_sketch `bind.rs`**:
  - 3D 갈래 추가. 3D의 Horizontal·Vertical·Tangent·Concentric·Symmetric은 `WrongSpace`.
  - 3D 각도 치수는 180° 이상이면 거부한다. `measure`도 3D 각도를 잰다.
- **gc_tools**: 3D 치수 도구에서 두 선은 3D 각도. 평행선과 점-선 조합은 3D에서 치수 없음.
- **리본 3D 구속 패널**: 일치·동일선상·평행·직각·동일·중점·고정·X/Y/Z·XY/YZ/ZX(13개, 두 줄).
- **테스트**: `gc_sketch/tests/sketch.rs` three_dimensional_line_relations(제거 자유도·성립·각도·거부), 앱 kittest the_three_dimensional_sketch_offers….
- **다음(표 2)**: 3D 호.

## 2026-11-18 — 큰 스케치 패턴 지연 해결(리드 ①, KNOWN_ISSUES "직사각형 스케치 패턴 60×60 이상 지연")

- **측정**(QA `pattern_times`, 테스트 프로필): 10×10 11.5 s, 12×12 48 s.
- **원인**:
  - 패턴 구속 하나마다 `try_add_constraint`가 스케치 전체를 다시 묶고 순위 검사를 했다. 요소당 구속이 여러 개라 요소 수 n에 대해 n × (밀집 QR)이었다.
  - 모든 복사본이 앞 복사본·첫 간격선에 묶여 한 성분이 되었다. 검사를 마지막 한 번으로 줄여도 100×100은 약 4만 미지수의 밀집 QR이라 1 s가 불가능했다.
- **바꾼 구성**:
  - 풀이에 남는 것은 방향마다 구성 간격선 하나(첫 원본 점 → 구성 점, 방향선과 평행, 간격 치수)다. 원형은 축 → 원본 점 바큇살과 같은 길이로 돌린 바큇살 하나(각도 치수)다.
  - 복사본 점과 복사된 원의 반지름은 바인딩 때 솔버 안에서 고정된다(`Sketch::pattern_held`, 사용자에게 보이는 Fix 구속 아님).
  - `Sketch::solve` 뒤에 `refresh_patterns`가 원본과 간격선/바큇살에서 복사본 위치를 다시 계산한다. 움직였으면 한 번 더 풀어서 복사본에 걸린 사용자 형상도 따라오게 한다.
  - 기록에 `step: PatternStep::{Translate, Rotate}`, `places`, `copies`, `circles`가 추가되었다.
- **결과**:
  - 자유도는 같다(간격선 2 미지수, 평행 + 치수 2개).
  - 간격·각도 치수는 그대로 있어 더블클릭 편집과 식(매개변수)이 된다.
  - 10×10 0.5 ms, 60×60 생성 20 ms·간격 편집 29 ms, 100×100 생성 56 ms·간격 편집 88 ms(`gc_session/tests/sketch_shape_tools.rs` large_rectangular…, 1 s 단언).
- **달라진 점**:
  - 원본을 끄는 동안(드래그 프레임) 복사본이 따라오지 않고, 놓으면 따라간다.
  - ba1c3ab 형식(1시간 전)으로 저장한 패턴 기록은 새 필드가 없어 열리지 않는다(그 이전 파일은 기록이 없어 예전 구속식 그대로 동작).
- **KNOWN_ISSUES 항목은 리드가 닫아 주세요**(백엔드는 그 파일을 고치지 않음).

## 2026-11-18 — 점 도구 HUD X/Y(리드 ④)

- 원문: Dynamic Input `GUID-33A8B11D-A04C-4809-BC82-C0419215915F.htm`, Draw Points Using Dynamic Input. 점에 치수를 붙인다는 말은 없어서 붙이지 않는다.
- 동작: `HudField::{X, Y}`. 상자에 커서 좌표가 보이고, 입력한 값이 클릭·Enter 때 커서 좌표 대신 쓰인다. 점을 놓으면 입력이 지워진다.
- 원문과 다른 점: 원문은 Tab을 먼저 눌러야 X 입력이 시작되지만, 여기서는 다른 도구처럼 바로 X 상자에 입력된다.
- 테스트: 세션 `the_point_tool_shows_x_and_y…`, 앱 kittest `the_point_tool_takes_typed_x_and_y…`.

## 2026-11-18 — 스케치 패턴 개체: 편집·삭제·요소 억제(리드 ②)

원문: To Create and Edit Sketch Patterns `GUID-AAD3A699-FED8-4614-A31D-0A2316328CEE.htm`, Edit Rectangular and Circular Patterns.

- **모델(gc_sketch `pattern.rs`)**:
  - `Sketch::patterns: Vec<SketchPattern{definition: PatternDefinition, elements, items, suppressed}>`. serde 기본값이라 예전 파일도 열린다.
  - `PatternDirection`을 gc_sketch로 옮겼다. `gc_doc::SketchPatternDirection`은 같은 타입의 재노출이라 API 변화 없음.
  - `pattern_of(curve)`, `pattern_element_of(curve)`, `suppress_pattern_elements`, `remove_exactly(items)`, `prune_patterns()`.
  - `remove_exactly`가 따로 있는 이유: `Sketch::delete`는 곡선의 점까지 지워서, 원본 점을 공유하는 구성선을 지우면 원본이 망가진다.
- **명령(gc_doc)**:
  - 직사각형·원형 패턴이 기록을 남긴다.
  - `EditSketchPattern{element, definition, suppressed}`: 지우고 다시 만든다.
  - `DeleteSketchPattern{element}`: 원본은 남는다.
  - `SuppressPatternElements{curves, suppress}`: 그 요소의 곡선이 구성선이 되고 구속은 유지. 되돌리면 원본이 구성선일 때만 구성선으로.
  - `Delete`: 패턴이 만든 항목은 건너뛴다(원문 "Delete removes nonpattern geometry"). 원본을 지우면 그 패턴도 통째로 지운다.
    전에는 원본을 지우면 구성선이 끊겨 풀이 오류가 났다.
- **세션**: `selected_pattern()`(선택된 곡선이 속한 패턴).
- **UI**: 패턴의 곡선을 선택한 채 리본 직사각형/원형을 누르면 그 패턴을 편집하는 창이 열린다.
  - 값이 채워져 있고 '억제: 요소 n개' 선택기가 켜져 있다. 클릭한 요소가 억제 대상이 된다.
  - 확인 = 다시 만들기, 패턴 삭제 = 패턴만 지우기.
- **원문과 다른 점**:
  - 우클릭 메뉴(Edit Pattern·Delete Pattern·Suppress Element(s)) 대신 리본 버튼으로 연다(뷰포트에 컨텍스트 메뉴가 없음).
  - 편집은 복사본을 새로 만든다(복사본에 손으로 단 구속은 사라짐).
  - 패턴 치수를 직접 바꾼 값은 기록의 식에 반영되지 않는다.
  - Associative 해제와 Boundary는 없다.
- **테스트**: `gc_doc/tests/sketch_shapes.rs` sketch_patterns_can_be_edited…, 앱 kittest a_selected_pattern….

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
