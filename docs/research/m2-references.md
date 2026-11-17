# M2 참고 자료 조사 — CAD 가져오기 · 필드/구역 · 작업 모델/다중 로봇 · UR 제어 · Inventor식 트리 UX

조사일 2026-11-17. 대상: GraphCTRL M2(egui 0.37 + wgpu 30, Rust 1.97, 레이어 `foundation < api < domain < render < ui < app`).
판정 기준은 PLAN 2장과 같다: ① 큰 장면 3D 성능 → ② 유지보수 지속성 → ③ 주류. 라이선스 허용: MIT, Apache-2.0, BSD-2/3, ISC,
Zlib, BSL-1.0, CC0, Unicode-3.0. 금지: GPL, LGPL, AGPL, MPL, EUPL. GPL 코드는 수식만 읽고 코드는 옮기지 않는다.

**검증 방법과 한계**
- crates.io 수치(최신 버전·릴리스일·라이선스·다운로드)는 crates.io API로 직접 확인했다(2026-11-17 기준, "최근" = 90일).
- 저장소 활동은 github.com 커밋 페이지, 라이선스는 raw.githubusercontent.com·gitlab.com 원문으로 확인했다.
- 이 세션에서 **docs.rs, docs.nav2.org, wiki.freecad.org, janjericevic.github.io(MiR 문서 사본)는 네트워크 정책으로 열리지 않았다.**
  해당 내용은 소스 코드(raw GitHub)나 검색 요약으로 대신 확인했고, 그것도 못 한 항목은 **(미확인)** 으로 표시했다.
- 논문 서지 중 직접 확인한 것은 ✔, 기억에 의존한 것은 **(서지 미확인)** 으로 표시했다.

---

## 1. Rust에서 CAD 가져오기 (STEP · glTF)

### 1.1 배경
- STEP 파일은 ISO 10303-21(Part 21) 텍스트 인코딩 위에 AP203/AP214/AP242 스키마 엔티티를 담는다. 부품 형상은 B-rep
  (`MANIFOLD_SOLID_BREP`, `ADVANCED_FACE`, `B_SPLINE_SURFACE_WITH_KNOTS` …)이라 **화면에 그리려면 직접 테셀레이션**해야 한다.
- 어셈블리 구조는 `PRODUCT`/`PRODUCT_DEFINITION` + `NEXT_ASSEMBLY_USAGE_OCCURRENCE`(NAUO, 부모-자식 인스턴스) +
  `CONTEXT_DEPENDENT_SHAPE_REPRESENTATION`/`ITEM_DEFINED_TRANSFORMATION`(인스턴스 변환)으로 표현된다. 부품 이름은 `PRODUCT.name`,
  인스턴스 이름은 NAUO의 `name`/`id`. 색은 `STYLED_ITEM` → `PRESENTATION_STYLE_ASSIGNMENT` → `SURFACE_STYLE_RENDERING`/
  `FILL_AREA_STYLE_COLOUR` → `COLOUR_RGB`(AP214 이상; **AP203에는 색이 없다**).
- **Inventor 내보내기**: STEP 저장 옵션에서 AP203/AP214/AP242를 고른다. Inventor 외관(appearance)은 STEP 솔리드의 RGB 색으로
  변환된다. Autodesk 2024 도움말에 "AP242는 어셈블리에 사용할 수 없다"는 주석이 있으나 다른 표와 충돌하므로 실제 버전에서 확인
  필요. **어셈블리 + 색은 AP214가 현실적인 기본값**이다.
  Inventor는 작업 스케줄러(Task Scheduler)의 Export Files에서 **glTF(.gltf/.glb, 기본 .glb)** 도 내보낸다(대화형 메뉴 지원 여부는
  버전별로 (미확인)). 포럼에 GLB에서 일부 부품 메시가 빠지는 사례 보고가 있다.

### 1.2 후보

| 이름 | 종류 | 라이선스 | 유지보수 | 활용 방법 | 권고 |
|---|---|---|---|---|---|
| [`gltf`](https://crates.io/crates/gltf) 1.4.1 (+`gltf-json`) | glTF 2.0 로더 | MIT OR Apache-2.0 | 최신 릴리스 2024-05-10, 저장소 마지막 커밋 2026-05-11(패닉 수정 PR), 누적 12.0M·최근 3.15M 다운로드, 이슈 66·PR 24 열림 | 노드 트리(이름·로컬 변환) → 문서의 Body 계층, 메시 프리미티브 → `gc_mesh::MeshData`, `pbrMetallicRoughness.baseColorFactor` → 색. 인스턴싱(같은 mesh를 여러 노드가 참조)을 그대로 GPU 인스턴싱에 연결 가능 | **사용(1순위)**. 릴리스 간격이 길지만 생태계 표준이고 API 안정. `gc_plugin_mesh`에 GLB 임포터로 추가 |
| [`step-io`](https://crates.io/crates/step-io) 0.3.0 ([repo](https://github.com/elgar328/step-io)) | STEP 읽기(모든 주류 AP)/쓰기(AP242e2) | MIT OR Apache-2.0 | 2026-04 첫 릴리스, 9개 버전, 최신 2026-10-21, 1인 개발, README에 "Experimental — 언제든 API 깨짐" | [엔티티 표](https://github.com/elgar328/step-io/blob/main/docs/entities.md) 직접 확인: NAUO·CDSR·`styled_item`·`colour_rgb`·`b_spline_surface_with_knots` **읽음**, `triangulated_face`·`tessellated_shell`은 **안 읽음**(B-rep만). 2,195 중 501 엔티티 읽기. 테셀레이터는 없음 | **시험 후 채택 검토**. 어셈블리·이름·색을 얻는 가장 유망한 순수 Rust 파서. 테셀레이션은 우리가 구현해야 함. `=` 고정 + 어댑터 뒤 격리 |
| [`truck`](https://github.com/ricosjp/truck) (`truck-stepio` 0.3.0, `truck-meshalgo` 0.4.0, `truck-modeling` 0.6.0) | B-rep 커널 + STEP 입출력 + 테셀레이션 | Apache-2.0 | crates.io 마지막 릴리스 **2024-09-20**, 저장소는 2026-11-02까지 커밋(주로 `cargo upgrade`), 최근 다운로드 ~10만 | `truck-stepio`(master)는 NAUO·`MANIFOLD_SOLID_BREP`·`SHELL_BASED_SURFACE_MODEL`·(rational) B-spline을 처리. **색(`styled_item`)은 소스에 없음**. `truck-meshalgo`로 STEP→메시 일괄 변환 가능 | **스파이크 비교용 / 테셀레이션 수식 참고**. 릴리스가 2년 정체 → 의존성으로 장기 고정하기 어렵다. 트림 NURBS 테셀레이션 수학은 Apache라 포팅 가능 |
| [`ruststep`](https://crates.io/crates/ruststep) 0.4.0 / `espr` | Part 21 파서 + EXPRESS 컴파일러 | Apache-2.0 | 릴리스 2024-09-20, 마지막 커밋 2025-03-20(step-p21 README 기준) | truck-stepio의 하부 | **피함**(정체). 필요한 수정이 미배포 |
| [`step-p21`](https://crates.io/crates/step-p21) 0.1.1 | ruststep 포크(Part 21 구문만) | Apache-2.0 | 2026-08 생성, 3개 버전 | `''` 이스케이프·빈 집합 `()` 등 실제 CAD 파일에서 깨지는 ruststep 버그를 고침(README) | **학습**. step-io가 막히면 대안 하부 파서 |
| [Formlabs/foxtrot](https://github.com/Formlabs/foxtrot) | STEP 뷰어 데모(파서+삼각분할, wgpu) | MIT OR Apache-2.0 | crates.io 미배포(crates.io의 `foxtrot`은 **무관한 Bevy 데모**), "proof-of-concept, 유지보수 기대 낮음" 명시 | 곡면별 삼각분할·CDT 구현(`triangulate`, `cdt` 서브크레이트)이 참고할 만함 | **수식·구조 포팅 가능(허용 라이선스)**, 의존은 불가 |
| [`iso-10303`](https://crates.io/crates/iso-10303) 0.5.0 | EXPRESS 기반 파서 생성기 | MIT | 2021-05 이후 정체 | — | **피함** |
| `oxideav-step`, `stepperoni`, `cadcore-step`, `BREP_*` 등 | 2026 하반기 신생 STEP/B-rep 크레이트 | 대부분 MIT/Apache | 생성 1–3개월, 다운로드 수백 | 검증된 사용처 없음 | **관찰만**. 6개월 뒤 재평가 |
| `openbim-step` | Part 21/EXPRESS 인프라 | **AGPL-3.0** OR 상용 | — | — | **금지** |
| [`opencascade`](https://crates.io/crates/opencascade) 0.3.0 (OCCT 바인딩) | 산업용 커널 | **LGPL-2.1** | 2026-08 릴리스 | — | **금지**(라이선스) |
| [`mesh-loader`](https://crates.io/crates/mesh-loader) 0.1.13 | STL/OBJ/COLLADA | Apache-2.0 | 2024-10 | 이미 사용 중 | 유지 |

### 1.3 결론 — Inventor 설비 어셈블리에 현실적인 경로
1. **M2 기본 경로 = GLB**(Inventor가 테셀레이션까지 끝낸 결과). `gltf` 크레이트로 노드 계층·이름·색·인스턴싱을 보존해 가져온다.
   큰 장면 성능(①)에 가장 유리하다: 테셀레이션 비용 0, 같은 부품 반복을 인스턴스로 그린다. 단위(glTF는 m, Y-up)는 Z-up으로 변환.
2. **STEP AP214 = 2단계 스파이크**: `step-io`로 구조·이름·색·B-rep을 읽고, 평면/원통/원뿔/구/토러스 면부터 자체 테셀레이터
   (`gc_mesh`의 CDT, foxtrot·truck의 수식 참고)를 만든다. B-spline 면은 매개변수 격자 + 트림 루프 CDT. 실제 Inventor 내보내기
   파일 3–5개로 `truck-stepio`와 결과·시간을 비교해 결정한다. 테셀레이션은 `gc_jobs`에서 부품 단위 병렬, 내용 해시로 캐시.
3. 가져온 어셈블리는 **편집 불가 참조 Body 트리**로 두고(Inventor의 "derived/reference" 느낌), 충돌용으로는 부품별 볼록 분해나
   AABB 단순화를 선택하게 한다(parry). 이름이 비어 있으면 `PRODUCT.name` → 파일명 순으로 대체.

---

## 2. 필드/구역 — AMR 지도 편집기에서 로봇 팔로

### 2.1 AMR·내비게이션 쪽 의미론

| 이름 | 종류 | 라이선스 | 유지보수 | 활용 방법 | 권고 |
|---|---|---|---|---|---|
| MiR 지도 편집기(MiR Robot Interface Reference Guide) | 상용 제품 문서 | 독점(문서 공개) | 현행 제품 | 지도 객체: virtual wall, **forbidden zone**(진입 금지, 계단·낮은 설비 아래), **preferred / unpreferred drive zone**(경로 비용 가감), **directional zone**(일방통행, 참조 가이드 후기 판에 추가), speed zone, sound/light·I/O·planner zone, Fleet zone(교차로 상호 배제). 구역 = 지도 위 다각형 + 유형별 파라미터 | **학습**. 개별 유형의 정확한 파라미터는 공개 문서 접근 불가로 **(미확인)** |
| [Nav2 costmap filters](https://github.com/ros-navigation/navigation2/tree/main/nav2_costmap_2d/plugins/costmap_filters) (Keepout/Speed/Binary) | ROS 2 오픈소스 | 저장소 혼합(LGPL-2.1+/Apache-2.0/BSD-3), **costmap filters는 Apache-2.0**(파일별 확인 필요) | 활발(2026) | 데이터 모델 `CostmapFilterInfo`(직접 확인): `type` 0=keepout/lanes, 1=속도 % , 2=속도 m/s; 마스크 값 → `space = data * multiplier + base`. Keepout은 마스크 셀을 lethal로, **lanes**는 중간 비용(선호 경로). Speed filter는 현재 위치뿐 아니라 **경로를 lookahead 거리만큼 따라가며 최솟값**을 적용(진입 전 감속) | **수식·의미론 포팅 가능**(Apache 파일만). 특히 lookahead 최솟값 규칙 |
| [Nav2 Route Server](https://github.com/ros-navigation/navigation2/tree/main/nav2_route) | 그래프 경로 계획 | Apache-2.0(package.xml 확인) | 활발 | **모든 간선이 방향성** → 일방통행 차선. `PenaltyScorer`(간선 정적 벌점 = 선호/비선호), 방향 점수기(진행 방향 허용 각도 밖이면 거부 또는 가중) | **의미론 차용**: one-way = 허용 방향 + 허용 각, preferred = 비용 가중 |
| Omron / Locus / NVIDIA Isaac semantic areas | 상용 | 독점 | — | Isaac: limited speed zone(구역 최대 속도), directed area(권장 진행 방향). Omron·Locus는 공개 문서 미확인 | **학습**(미확인 다수) |

정리한 공통 의미론: **구역 = (영역 형상, 유형, 파라미터, 활성 조건)**. 유형은 ① 금지(하드 제약) ② 속도 상한(절대값 또는 비율,
진입 전 감속) ③ 방향(허용 방향 벡터 + 허용 각, 하드 또는 소프트) ④ 선호/비선호(비용 가중, 소프트) ⑤ 상호 배제(한 번에 한 대).

### 2.2 로봇 팔 쪽 대응물

| 이름 | 종류 | 라이선스 | 유지보수 | 활용 방법 | 권고 |
|---|---|---|---|---|---|
| UR 안전 설정: [Safety Planes](https://www.universal-robots.com/manuals/EN/HTML/SW10_10/Content/prod-usr-man/complianceUR5e/PolyX_SW/safety_planes.htm), [Tool Direction](https://www.universal-robots.com/manuals/EN/HTML/SW10_13/Content/prod-usr-man/complianceUR5e/PolyX_SW/polyx-ToolDirection.htm), 안전 기능 표 | 제조사 매뉴얼 | 독점(공개) | 현행(PolyScope 5 / X) | 안전 평면은 **반공간**이며 **툴 구(tool spheres)와 엘보만** 제한(팔 전체 아님). 모드: Normal/Reduced/Both/**Trigger Reduced**(넘으면 Reduced 모드 전환), Restrict Elbow, 툴 플랜지 제한. Tool direction limit = 베이스 기준 고정 원뿔. 그 밖에 TCP 속도·힘·운동량·파워·관절 위치/속도·정지 시간/거리 한계(Normal/Reduced 두 세트) | **대응 모델로 채택**: keep-out 볼록 영역 → 안전 평면 근사로 내보내기, 감속 구역 → Trigger Reduced 평면 + Reduced TCP 속도. 안전 설정은 URScript로 바꿀 수 없고 PolyScope(암호)에서 설정한다고 알려짐 **(미확인, 매뉴얼 재확인 필요)** |
| ISO 10218-1/-2(산업용 로봇 안전), ISO/TS 15066(협동 로봇) | 표준 | 유료 문서 | 10218은 2025 개정판 발행, TS 15066 내용 일부 흡수 **(미확인)** | 협동 운전 4방식: 안전 정격 모니터 정지, 핸드 가이딩, **속도·분리 감시(SSM)**, 동력·힘 제한(PFL). SSM 보호 분리 거리 `S_p = S_h + S_r + S_s + C + Z_d + Z_r`(사람 이동분, 로봇 반응·정지 중 이동분, 침입 거리, 측정 불확도) **(기억 기반)** | **수식 학습**: 시뮬레이터에서 "사람 구역" 근접 시 TCP 속도 상한을 거리 함수로 계산하는 기능의 근거. 인증 도구로 주장하지 않는다 |
| [MoveIt](https://moveit.ros.org) kinematic constraints (`PositionConstraint`, `OrientationConstraint`, `JointConstraint`, `VisibilityConstraint`, path constraints) | 오픈소스 | BSD-3 | 활발 | Position constraint = 링크 점이 **볼륨(primitive/mesh) 안에** 있어야 함 + 가중치, path constraint는 경로 전체에 적용, 제약 샘플러·제약 계획(OMPL constrained) | **의미론 포팅**: "허용 볼륨" 구역(keep-in), 경로 제약 개념 |
| TOPP-RA(이미 `gc_traj`에 있음) | 알고리즘 | (자체 구현) | — | 경로 매개변수 s에서 TCP 속도 제약 `‖J(q(s)) q'(s)‖·ṡ ≤ v_max(s)`는 ṡ² 상한(1차 제약)으로 바로 들어간다 | **사용**: 구역별 `v_max(s)`를 경로 샘플마다 계산해 TOPP-RA에 투입 |

### 2.3 GraphCTRL 구역 모델 제안(조사 결과 반영)
- **형상**: 2D = 스케치 프로파일(닫힌 루프) + 놓인 평면, 3D = 그 프로파일을 `[z_min, z_max]`로 돌출한 프리즘(필요하면 메시 볼륨).
  다각형 연산은 [`i_overlay`](https://crates.io/crates/i_overlay) 9.2.0(MIT/Apache, 2026-11-15, 최근 5.4M) 또는
  [`geo`](https://crates.io/crates/geo) 0.33.1(MIT/Apache), 삼각분할은 [`spade`](https://crates.io/crates/spade) 2.15(MIT/Apache) — 모두 활발.
- **적용 대상**: TCP 점 / 툴 구 / 엘보 / 팔 전체(충돌체), 대상 로봇 필터, 활성 조건(작업 모델 상태 식, 3장).
- **계획에 미치는 영향**
  - keep-out: 팔 전체 대상이면 `gc_collide`의 정적 바디(프리즘 볼록 분해), TCP/툴 대상이면 상태 유효성 검사 항목.
  - speed: 경로 후처리 후 `v_max(s)` = 겹치는 구역 상한의 최솟값, Nav2처럼 **진입 전 lookahead**로 감속 시작 → TOPP-RA.
  - one-way: 간선 유효성 검사에서 구역 안 TCP 변위 `Δp`가 `Δp·d ≥ cos θ·‖Δp‖`를 만족해야 함(하드). 소프트 모드는 위반 각 비례 비용.
  - preferred/unpreferred: 최적화 플래너(RRT*·PRM) 비용 = 길이 × (1 + w·구역 내 비율). 하드 제약이 아니므로 보증 문구에 표시하지 않음.
  - mutex(공유 구역): 3장의 다중 팔 조정에서 자원으로 사용.
- **UR 내보내기**: 하드웨어가 표현 가능한 것(평면·원뿔·속도 세트)만 안전 설정 제안으로 출력하고, 나머지는 "계획 시에만 보장"으로
  명시한다.

---

## 3. 작업 모델과 동적 다중 로봇 스케줄링

우리 메커니즘(상태 변수, 식 전제조건, 구역/웨이포인트, 효과, 로봇별 비용, 싼 로봇 배정, 실패 시 복구)은 **PDDL2.1 수치 +
지속 행동**에 가깝고, 배정은 **MRTA의 ST-SR-IA/TA**, 실행은 **BT/감독 실행기**에 해당한다.

### 3.1 형식·이론

| 이름 | 종류 | 라이선스 | 유지보수 | 활용 방법 | 권고 |
|---|---|---|---|---|---|
| PDDL 2.1 — Fox & Long, "PDDL2.1: An Extension to PDDL for Expressing Temporal Planning Domains", JAIR 20, 2003 (서지 미확인) | 언어 사양 | 논문 | 고전 | 수치 fluent(`increase`, 비교 전제), **지속 행동**(`at start`/`over all`/`at end` 조건·효과), plan metric | **의미론 차용**: 행동 = 시작 조건/진행 중 불변식(구역 점유)/종료 효과, 지속 시간 = 계획된 모션 시간 |
| PDDL+ — Fox & Long, JAIR 27, 2006 (서지 미확인) | 사양 | 논문 | 고전 | process/event로 연속 변화 모델 | **학습만**(M2 범위 밖) |
| PDDL 3 (preferences, trajectory constraints) | 사양 | — | — | 소프트 목표·선호 | 학습 |
| Gerkey & Matarić, "A formal analysis and taxonomy of task allocation in multi-robot systems", IJRR 23(9), 2004 (서지 미확인) | MRTA 분류 | 논문 | 고전 | ST/MT × SR/MR × IA/TA. 우리 = **ST-SR-TA**(한 로봇 한 작업, 시간 확장 배정) → 일반적으로 NP-hard, 탐욕/경매가 표준 근사 | **용어·문제 정의 차용** |
| Korsah, Stentz, Dias, "iTax", IJRR 2013 (서지 미확인) | 상호의존 작업 분류 | 논문 | — | 작업 간 의존(순서·공유 자원)이 있는 경우 분류(XD/CD) | 학습 |
| CBBA — Choi, Brunet, How, IEEE T-RO 25(4):912–926, 2009, doi:10.1109/TRO.2009.2022423 ✔ ([MIT DSpace 전문](https://dspace.mit.edu/handle/1721.1/52330)) | 분산 경매 | 논문 | 고전 | 번들 구성 + 합의로 충돌 없는 배정, 수렴·최악 성능 보장. 우리는 중앙 집중이라 **CBAA/SSI의 중앙 버전**이면 충분 | **수식 포팅**(점수 함수 diminishing marginal gain 조건) |
| 순차 단일 품목 경매(SSI) — Lagoudakis et al. RSS 2005 / Koenig et al. 2006 (서지 미확인) | 경매 | 논문 | — | 작업을 하나씩 경매, 입찰 = 기존 일정에 끼워 넣을 때 **추가 비용**(우리: 추가 모션 시간). 단순·재계획 쉬움 | **채택 권장(M2 기본 배정기)** |
| 시간 계획기 POPF (Coles et al., ICAPS 2010), OPTIC (Benton, Coles, Coles, ICAPS 2012), Temporal Fast Downward (Eyerich, Mattmüller, Röger, ICAPS 2009) (서지 미확인) | C++ 계획기 | 대부분 GPL 계열로 알려짐 **(미확인)** | 연구 코드, 정체 | 부분 순서 + 시간 제약(STN) | **학습만**(코드 금지 가정) |

### 3.2 실행·감독·BT

| 이름 | 종류 | 라이선스 | 유지보수 | 활용 방법 | 권고 |
|---|---|---|---|---|---|
| [ROSPlan](https://github.com/KCL-Planning/ROSPlan) (Cashmore et al., ICAPS 2015, 서지 미확인) | 계획·디스패치·감시 | BSD 계열(LICENSE 원문 확인, 2/3조항 구분은 미확인) | ROS1, 정체 | knowledge base ↔ planner ↔ dispatcher(행동 완료/실패 피드백) ↔ **재계획** 구조 | **구조 학습** |
| [PlanSys2](https://github.com/PlanSys2/ros2_planning_system) (Martín et al., IROS 2021, 서지 미확인) | ROS 2 계획 시스템 | Apache-2.0 ✔ | ROS 2 rolling | 계획을 **BT로 변환해 실행**, 행동 = BT 노드, 실패 시 재계획 | **구조·의미론 포팅 가능** |
| [BehaviorTree.CPP](https://github.com/BehaviorTree/BehaviorTree.CPP) | BT 엔진(C++) | MIT ✔ | 활발 | Sequence/Fallback/ReactiveSequence/Parallel, Decorator(Retry, Timeout), blackboard, XML 포맷(Groot) | **의미론·포맷 포팅**(복구 = Fallback + RetryUntilSuccessful) |
| Colledanchise & Ögren, *Behavior Trees in Robotics and AI*, CRC 2018 (arXiv 1709.00084, 서지 미확인) | 교재 | — | — | BT ↔ 계획(PPA: 후조건-전제-행동 역연쇄로 BT 자동 생성) | **학습**: 전제·효과 행동 집합 → BT 자동 구성 |
| [`bonsai-bt`](https://crates.io/crates/bonsai-bt) 0.14.0 | Rust BT | MIT | 2022~, 29개 버전, 최신 2026-09-22, 최근 16k | 틱 기반, Sequence/Select/While/Wait, blackboard | **참고 또는 선택적 사용**. 직렬화·편집기 연결이 필요하면 자체 구현이 낫다 |
| [`behavior-machines`](https://crates.io/crates/behavior-machines) 0.5.0 | Rust BT(no_std, halt 의미론) | BSD-3-Clause | 2026-10 생성 | — | 관찰 |
| `arora-behavior-tree` 10.1.0 | Rust BT(Groot 호환) | Apache-2.0 | 2026-07 생성, 저장소 링크 없음 | — | 피함(출처 불명) |
| [`pddl`](https://crates.io/crates/pddl) 0.4.0, `miniplan` | PDDL 3.1 파서/계획기 | **EUPL-1.2** | 활발 | — | **금지**(라이선스) |
| [`pddl-parser`](https://crates.io/crates/pddl-parser) 0.2.3 | PDDL 파서 | MIT OR Apache-2.0 | 2024-02 이후 정체, 최근 10 다운로드 | — | 피함 |
| [`ferroplan`](https://crates.io/crates/ferroplan) 0.30.0 | PDDL 계획기(FF, 수치, PDDL2.1 시간) | MIT OR Apache-2.0 | 2026-06 생성, 4개월에 32개 버전, 1인 | IPC 벤치 자체 보고(검증 안 함) | **관찰**. 나중에 PDDL 내보내기 검증용 외부 도구 후보 |

### 3.3 다중 팔 공유 작업공간 조정

| 이름 | 종류 | 라이선스 | 유지보수 | 활용 방법 | 권고 |
|---|---|---|---|---|---|
| O'Donnell & Lozano-Pérez, "Deadlock-free and collision-free coordination of two robot manipulators", ICRA 1989, pp. 484–489, doi:10.1109/ROBOT.1989.100033 ✔ | 논문 | — | 고전 | **조정 다이어그램**(s₁×s₂ 평면의 충돌 영역을 피하는 단조 경로 = 각 팔의 대기/진행 스케줄), 교착 회피 | **수식 포팅(M2 2팔 기본)** |
| Kant & Zucker, path-velocity decomposition, IJRR 1986 (서지 미확인) | 논문 | — | 고전 | 경로는 고정, **속도 프로파일만** 조정해 충돌 회피 | **채택**: 이미 계획된 경로를 재계획 없이 시간 스케일링 |
| Erdmann & Lozano-Pérez, prioritized planning, Algorithmica 1987 (서지 미확인) | 논문 | — | 고전 | 우선순위 순으로 계획, 앞선 로봇 궤적을 시간 변화 장애물로 | **채택**(3대 이상, 불완전성 경고 표시) |
| Siméon, Leroy, Laumond, "Path coordination for multiple mobile robots: a resolution-complete algorithm", IEEE TRA 2002 (서지 미확인) | 논문 | — | — | 다차원 조정 공간의 해상도-완전 탐색 | 학습 |
| Shome et al., dRRT* (Autonomous Robots 2020, 서지 미확인) | 논문 | — | — | 합성 로드맵 위 다중 팔 점근 최적 계획 | 학습(비용 큼) |

권장 조정 방식: ① 공유 구역을 **상호 배제 자원**(PDDL `over all` 점유)으로 두어 배정 단계에서 충돌 대부분을 예방,
② 남는 팔-팔 충돌은 고정 경로 위 **시간 스케일링/대기 삽입**(조정 다이어그램, 2팔), ③ 실패 시 우선순위 재계획 →
④ 그래도 실패하면 복구 행동(BT Fallback) 또는 다른 로봇 재경매.

---

## 4. UR 제어 인터페이스

### 4.1 프로토콜·개념

| 이름 | 종류 | 라이선스 | 유지보수 | 활용 방법 | 권고 |
|---|---|---|---|---|---|
| URScript Manual(제어기 버전별) | 언어 사양 | 독점(공개) | 현행 | `movej`(관절 보간, `a`,`v`,`t`,`r`), `movel`(TCP 직선), `movep`(공정 이동, 일정 TCP 속도 + 원호 블렌딩), `movec`, `servoj`(고주기 관절 목표, `lookahead_time`, `gain`), `speedj/speedl`(속도 명령), 블렌드 반경 `r`(연속 이동 간 모서리 둥글기, 겹치면 실패) | **사용**: `gc_traj` 결과를 ① 웨이포인트 + 블렌드 `movej/movel` 프로그램 또는 ② `servoj` 스트리밍으로 내보내기 |
| [RTDE Guide](https://docs.universal-robots.com/tutorials/communication-protocol-tutorials/rtde-guide.html) | 이진 프로토콜 사양 | 독점(공개) | 현행 | TCP **30004**, e-Series/ur-Series 최대 **500 Hz**(CB3 125 Hz). 패킷: 프로토콜 버전 요청, 제어기 버전, 출력/입력 레시피 설정, 시작/정지, 데이터 패키지, 텍스트 메시지(v1/v2 차이 있음). 레시피 = 변수명 목록 → 고정 레이아웃 | **직접 구현**(작고 명확). 실시간 상태(관절·TCP·안전 모드) 수신, 입력 레지스터로 경량 제어 |
| Primary/Secondary client interface | 프로토콜 | 독점(공개) | 현행 | 30001(primary, 10 Hz 상태 + URScript 수신), 30002(secondary), 30003(realtime, **폐기 예정**, PolyScope X 없음) | URScript 전송·버전 정보용으로만 |
| Dashboard server | 텍스트 프로토콜 | 독점(공개) | 현행 | TCP **29999**: `load`, `play`, `stop`, `pause`, `robotmode`, `safetystatus`, `power on`, `brake release` 등 | **직접 구현**(줄 단위 텍스트) |
| [External Control URCap](https://github.com/UniversalRobots/Universal_Robots_ExternalControl_URCap) | 로봇 측 플러그인 | Apache-2.0 ✔ | 활발 | 티치 펜던트 프로그램 안 노드가 외부 PC의 **reverse interface**(기본 50001), script sender(50002), trajectory forward(50003)에 접속 → 외부 궤적 실행 | **사용 권장**(로봇에 설치하는 UR 공식 URCap) |

### 4.2 오픈소스 클라이언트와 시뮬레이터

| 이름 | 종류 | 라이선스 | 유지보수 | 활용 방법 | 권고 |
|---|---|---|---|---|---|
| [Universal_Robots_Client_Library](https://github.com/UniversalRobots/Universal_Robots_Client_Library) (URCL) | C++ 공식 클라이언트 | **Apache-2.0**(+ 큐 BSD-2, atomicops Zlib, httplib MIT; package.xml 확인 ✔) | 활발, ROS 2 드라이버의 핵심 | RTDE·primary 파서, reverse interface, trajectory forwarding(관절 스플라인), script sender, dashboard | **코드 포팅 가능**(라이선스 고지 유지). 1순위 참조 구현 |
| [ur_rtde](https://gitlab.com/sdurobotics/ur_rtde) (SDU) | C++/Python RTDE 클라이언트 | **MIT** ✔ | 활발(gitlab) | `RTDEControlInterface`(로봇에 제어 스크립트 업로드 후 레지스터로 명령), `RTDEReceiveInterface`, `RTDEIOInterface`, dashboard | **포팅 가능**. 특히 제어 스크립트 + 레지스터 방식 |
| [python-urx](https://github.com/SintefManufacturing/python-urx) | Python 클라이언트 | **LGPL-3**(setup.py ✔) | 정체 | — | **학습만**(코드 금지) |
| [`ur-driver`](https://crates.io/crates/ur-driver) 0.1.0 ([repo](https://github.com/physwkim/ur-driver)) | Rust URCL 포팅(v2.14.1 기준) | Apache-2.0 AND BSD-3-Clause | 2026-08 1회 릴리스, 21 다운로드, 커밋 19 | RTDE·primary·dashboard·reverse/trajectory, URSim을 오라클로 비교 테스트 | **관찰·참고**. 의존하기엔 미성숙. 구조 비교 자료로 유용 |
| `rur` 0.1.0(2020), `ur-script` 0.2.0(2022) | Rust UR 크레이트 | — | 정체 | — | **피함** |
| [URSim e-Series Docker](https://hub.docker.com/r/universalrobots/ursim_e-series) | 시뮬레이터 이미지 | **라이선스 명시 없음**(UR 소프트웨어 EULA 적용으로 추정, 미확인) | `latest`만, 약 20일 전 갱신, 928 MB | `ROBOT_MODEL=UR3/UR5/UR10/UR16/UR20/UR30…`, VNC 5900, 웹 6080, 29999 등 포트 노출, `/ursim/programs` 마운트 | **테스트 전용**: CI·개발 PC에서 통합 테스트. 저장소·배포물에 포함하지 않음 |

권장 구조: `gc_plugin_ur` 아래(또는 새 `gc_plugin_ur_link`, foundation+api 의존만) **블로킹 `std::net` + 전용 스레드**로
RTDE·dashboard·script 전송을 자체 구현한다(tokio 불필요). URCL(Apache)·ur_rtde(MIT)를 참조 구현으로 두고, URSim 도커로 왕복 테스트
(GPU 테스트처럼 없으면 건너뛰기). 실행 모드 두 가지: ① **오프라인** = URScript 프로그램 생성(블렌드 `movej/movel`), ② **온라인** =
External Control URCap + trajectory forwarding으로 계획 궤적 실행, RTDE로 실제 상태를 받아 3D 고스트로 표시.

---

## 5. Inventor식 모델 트리 UX 참고

| 이름 | 종류 | 라이선스 | 유지보수 | 활용 방법 | 권고 |
|---|---|---|---|---|---|
| FreeCAD PartDesign / Sketcher | 파라메트릭 CAD(C++/Python) | **LGPL-2.1+** | 활발(1.0 = 2024-11) | Body의 **Tip**(이후 피처는 계산 제외 = 롤백), 피처 아래 스케치 중첩, 의존 그래프 보기, 1.0의 **Toggle freeze**(파라메트릭 갱신 중지), 오류 피처 표시 전파 | **동작만 학습**(코드 금지) |
| [SolveSpace](https://github.com/solvespace/solvespace) | 파라메트릭 CAD | **GPL-3** ✔ | 활발 | 그룹(=피처) 순차 목록, 그룹 활성화 시 이후 그룹 숨김, DOF 표시 | **학습만** |
| [Fornjot](https://github.com/hannobraun/fornjot) (`fj-core` 0.49.0) | Rust 코드 CAD 커널 | 0BSD(허용 목록 밖이지만 사실상 무제한) | **2026-06-19 아카이브, "프로젝트 종료"** | 코드 기반이라 트리 UX 없음 | **피함** |
| [truck](https://github.com/ricosjp/truck) | Rust B-rep 커널 | Apache-2.0 | 1장 참고 | 피처 트리 없음(커널만) | 1장 용도로만 |
| [CadQuery](https://github.com/CadQuery/cadquery) | Python 코드 CAD | Apache-2.0 ✔(단, OCP/OCCT는 LGPL) | 활발 | Workplane 체인 = 선택자 기반 피처 연쇄, 이름 붙은 태그 | **학습**(선택자 개념) |
| Autodesk Inventor 브라우저 / SolidWorks FeatureManager / Onshape 피처 목록 | 상용 | 독점(도움말 공개) | 현행 | 아래 동작 목록의 기준 | **동작 기준** |

목표 동작(우선순위 순, 우리 트리 = 피처·구역·작업 노드 공통):
1. **이름 바꾸기**: F2/느린 두 번 클릭, 내부 `Id`는 불변, 이름 중복 허용하되 경고. 수식 참조는 Id로 저장.
2. **스케치 소비**: 돌출·구역 피처가 스케치를 소비하면 스케치가 그 피처 **아래로 중첩**(Inventor). 우클릭 "스케치 편집"으로 바로 진입,
   종료 시 소비 피처 재계산. 스케치 공유(Share Sketch)는 P2.
3. **롤백 마커**(Inventor End of Part / SolidWorks rollback bar / FreeCAD Tip): 마커를 끌어 이후 항목을 회색·계산 제외.
   마커 아래에서 새 항목 생성 시 마커 위치에 삽입. 저장 시 마커 위치 보존.
4. **억제(Suppress)**: 계산에서 제외하되 삭제 아님, 의존 항목도 연쇄 억제(확인 대화상자). 구역 억제 = 계획에서 제외, 작업 억제 = 스케줄 제외.
5. **의존성**: 부모/자식 보기(SolidWorks), 의존 그래프(FreeCAD). 끌어서 재정렬할 때 의존 위반이면 거부·이유 표시.
6. **오류 표시**: 실패 피처에 경고 아이콘, 하위로 전파 표시, 툴팁에 원인(솔버 진단 재사용).
7. **동결(freeze)**: 외부 참조(가져온 어셈블리) 갱신을 멈추는 FreeCAD식 토글 — 가져온 CAD 재로드 시 유용.

기존 `docs/research/cad-ux-conventions.md`(내비게이션·스케치·구속)와 겹치지 않도록 트리 편집 동작만 다뤘다.

---

## 권고 요약

**의존성으로 채택**
- `gltf` 1.4.x (MIT/Apache) — Inventor GLB 가져오기, M2 기본 CAD 경로.
- `i_overlay` 또는 `geo` + `spade` (MIT/Apache) — 구역 다각형 불리언·삼각분할.
- 조건부: `step-io` (MIT/Apache, 실험적) — STEP AP214 스파이크에서 실제 Inventor 파일로 검증 통과 시 `=` 고정 채택.

**수식·코드 포팅(허용 라이선스만)**
- URCL(Apache-2.0)·ur_rtde(MIT) → 자체 RTDE·dashboard·script sender·trajectory forwarding.
- Nav2 costmap filter(Apache 파일)·Route Server(Apache) → 구역 의미론(속도 lookahead 최솟값, 방향 간선, 벌점).
- foxtrot(MIT/Apache)·truck(Apache) → B-rep 면 테셀레이션·CDT 수식.
- BehaviorTree.CPP(MIT)·PlanSys2(Apache) → BT 노드 의미론(Fallback/Retry/Reactive), 계획 → BT 변환·재계획 구조.
- CBBA/SSI(논문) → 중앙 경매 배정기, 조정 다이어그램·path-velocity decomposition(논문) → 다중 팔 시간 스케일링.
- MoveIt(BSD-3) 제약 의미론 → keep-in/경로 제약.

**학습만**
- FreeCAD·SolveSpace·python-urx·시간 계획기(POPF/OPTIC/TFD)는 GPL/LGPL(또는 미확인) → 코드 금지, 동작·수식만.
- MiR·Omron·Isaac·UR 안전 매뉴얼, ISO 10218/TS 15066(SSM 식) → 데이터 모델·의미론 근거.
- PDDL2.1/PDDL+ 논문, Gerkey–Matarić 분류, Colledanchise–Ögren 교재.

**피함**: `pddl`/`miniplan`(EUPL), `opencascade`(LGPL), `openbim-step`(AGPL), `ruststep`/`iso-10303`/`rur`/`ur-script`(정체),
Fornjot(아카이브), 신생 STEP 크레이트 다수(검증 부족, 6개월 뒤 재평가).

**후속 확인 필요**: docs.rs·docs.nav2.org·wiki.freecad.org·MiR 문서 접근 허용 후 (미확인) 항목 재검증, Inventor 실제 버전의
AP242 어셈블리 내보내기·glTF 메뉴 지원, URSim EULA, UR 안전 설정의 원격 변경 가능 여부.
