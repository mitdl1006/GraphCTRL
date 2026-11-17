# 백엔드 인계 (2026-11-17)

## 2026-11-17 밤 2 — 어셈블리 P1-2 컴포넌트 원점 폴더(+P1-4 일부), 리드 b02ac34 병합

- `Document::component_origin(occurrence) -> Option<ComponentOrigin{pose}>`; `ComponentOrigin::{point(), axis(0|1|2), plane(OriginPlane)}`.
  바디·로봇(베이스)·어셈블리는 placement, 부품은 파일의 부품 행렬(배율이 있으면 정규직교화) × 어셈블리 placement.
  평면 방향은 문서 원점 평면과 같다(XY 법선 +Z, XZ 법선 −Y, YZ 법선 +X).
- 참조: `PlaneRef::Component(occurrence, OriginPlane)`, `PointRef::Component(occurrence)`(작업 피처 기준),
  `PlaneSource::Component{occurrence, plane}`(스케치 평면, 컴포넌트를 따라 움직임, 쓰는 동안 그 컴포넌트 삭제 거부).
- 세션: `session.component_origin(occurrence) -> Option<gc_doc::ComponentOrigin>`,
  `session.create_sketch_on_component(occurrence, plane: gc_doc::OriginPlane) -> Result<Id<Sketch>, EditError>`.
  원점 폴더의 표시(기본 숨김, 컴포넌트별 토글, 전체 숨기기)는 프론트엔드 상태로 둔다(문서 저장이 필요하면 알려 주세요).
- 근거: 명세의 A52 About Reference Planes / A53 To Display Reference Planes(명세 표의 GUID가 잘려 원문을 열지 못함 — 전체 GUID를
  알려 주면 확인하겠다), A11 Place Components.
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
