# 백엔드 인계 (2026-11-17)

## M2-1 (진행 중)

| 항목 | 위치 | 상태 |
|---|---|---|
| GLB/glTF 읽기 | `gc_plugin_mesh::read_gltf(bytes, base_dir) -> Assembly { nodes(이름·부모·로컬 행렬·메시), meshes(정점·삼각형·색), warnings }` | 완료. 인스턴싱 유지(같은 메시는 한 번), Y-up → Z-up, 점·선 프리미티브는 경고 후 건너뜀 |
| 문서: 불러온 설비 | `gc_doc::Assembly { name, file, placement, parts: Vec<AssemblyPart{name, parent, matrix, meshes}>, meshes: Vec<AssemblyMesh{name, color, triangles}>, look }` | 완료. 고정 참조(부품 편집 없음), 삼각형은 파일에 남김. `CellOp::{AddAssembly, PlaceAssembly, SetAssemblyLook, DeleteAssembly}` |
| 문서: 작업 피처 | `gc_doc::WorkFeature { name, geometry: WorkGeometry::{PlaneOffset, PlaneThroughPoints, AxisThroughPoints, AxisNormalToPlane, Point}, visible }` | 완료(정의·검증·저장). **월드 좌표 계산(평면 포즈·축·점)은 아직** — 화면 표시 전에 세션 함수로 추가 예정 |
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
