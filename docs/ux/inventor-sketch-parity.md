# Inventor 2026 2D 스케치 환경 동등성 명세 — GraphCTRL 스케치와의 차이

작성 2026-11-17. 근거: Autodesk Inventor 2026 공식 도움말(`help.autodesk.com`, 영문 ENU)을 이 세션에서 직접 내려받아 읽었다. 인용은 `[S#]`이며 URL은 문서 끝 "출처" 표에 있다. 2026 페이지가 있는 것은 2026, 없는 것은 **(미확인)** 으로 표시했다(2025 폴백으로 채우지 않았다). **도움말에 없는 동작은 추측하지 않았다.** GraphCTRL 쪽 사실은 `docs/PLAN.md` 5.4–5.5·부록 C, `docs/STATUS.md`, `crates/domain/gc_tools`·`gc_sketch`, `crates/ui/gc_viewport`, `crates/app/gc_app`, 스크린샷 `docs/evidence/img/stage4-sketch-ui.png`·`review-2-value-box.png`에서 읽었다. 한글 명령명은 ENU만 읽었으므로 관행 번역이며 공식 한글 UI 명칭은 (미확인).

## 1. 범위

- **"완전히 동일한 경험"의 뜻(이 문서의 판정 기준)**: 같은 입력(클릭·드래그·키·우클릭)에 같은 결과(어떤 기하·구속·치수가 생기는가), 같은 되돌리기 단위, 같은 피드백(HUD 입력창, 점선 정렬선, 구속 글리프, 색, 상태바 문구)을 낸다. 인벤터가 도움말로 규정한 것만 "동일"의 대상이다. 도움말에 없는 수치(예: 스냅 픽셀 허용오차, 정확한 색 코드)는 GraphCTRL 값(8/6/4 px 등)을 유지하고 **(미확인)** 으로 둔다.
- **그리지 않는 것**: Autodesk 아이콘·로고·상표·색 팔레트 자산은 쓰지 않는다. 아이콘은 직접 그린 것을 쓰고 이름은 일반 용어를 쓴다("Inventor"는 비교 설명에만 등장).
- **GraphCTRL에 해당 없는 것(복제 제외)**: 돌출·회전 등 피처가 소비하는 프로파일 개념, 파트/어셈블리/도면 환경 차이, 스케치 블록, 이미지 삽입, 등식 곡선, 3D 스케치 전용 명령(Bend 등), AutoCAD 데이터 가져오기, 허용오차 표시. 이 경계는 `inventor-parity.md`(셸 전반 비교)와 같다.
- **GraphCTRL 고유 요소는 유지**: 로봇 셀 맥락(스케치 점 → 웨이포인트), 작업 좌표계·3D 스케치(Tab 평면 전환), 일부 단축키(A, R)는 인벤터와 다르면 인벤터를 따르고 기존 키는 별칭으로 남긴다(5.2 참조).
- **용어**: 프론트엔드(FE) = `crates/ui`·`render`·`gc_app`·`gc_panels` (리본·HUD·캔버스 입력·표시). 백엔드(BE) = `gc_sketch`·`gc_solver`·`gc_tools` (기하·솔버·도구 상태기계). 우선순위 P1 = "기본 스케치가 인벤터와 같다"에 필요, P2 = 다음, P3 = 이후.

## 2. 명령별 비교

### 2.0 현재 GraphCTRL 요약 (비교의 기준)

- 리본 스케치 탭: 패널 3개 — 작성(선택·선·원·호·사각형), 구속(치수 + 구속 12종: 일치·동일선상·동심·고정·평행·직각·수평·수직·접선·대칭·동일·중점), 종료(스케치 마무리). 인벤터의 Modify·Pattern·Format·Insert 패널과 Cancel Sketch는 없다.
- 도구(`ToolKind`): Select, Line, Circle, Arc, Rectangle, Dimension, Constraint(kind). 스냅 종류는 `Free/Point/OnCurve/Horizontal/Vertical` 5개뿐. 구성선 플래그는 모델에 있다(`construction`)(UI 진입점 미확인).
- 키: L·C·A·R·D, Ctrl+Z/Y, Esc(도구에서 한 단계 물러나기·나가기), Delete, F6(Iso). F7·F8·F9·F10·Backspace·Tab(2D)·`-` 없음. 3D 스케치에서만 Tab = 평면 전환.
- 우클릭: 셀 항목(스케치 이름 등) 컨텍스트 메뉴만 있고, 그리는 중 우클릭 메뉴(Done·옵션)는 없다.
- 입력: 값 상자는 치수 배치 때만(`RequestValue`). 선·원·호·사각형에는 HUD 입력창이 없다.
- 표시: 구속 글리프(H, V, T 등)·치수 텍스트·상태바 "N개 치수 필요 / 완전 구속"은 있다.

### 2.1 시작·종료·환경 (Start 2D Sketch, Finish, Cancel, Slice, Look At)

| 명령 | 인벤터 2026 동작 | 단축키 | GraphCTRL 현재 | 차이 | 우선 | 담당 |
|---|---|---|---|---|---|---|
| Start 2D Sketch (스케치 시작) | 3D Model 탭 Sketch 패널 "Start 2D Sketch" → 평면(작업 평면·평면 면) 클릭 [S1]. 브라우저 우클릭 "New Sketch", 평면 클릭 후 미니 툴바 "Create Sketch"도 가능 [S1]. 새 2D 스케치가 시작되면 **Line 명령이 자동 활성화**(편집 진입 시에는 아님) [S27]. 평면 선택 후 UI 흐름의 세부(미리보기 하이라이트 등)는 (미확인) | S [S28,S29] | 모델 탭에 "XY/XZ/YZ에 스케치" 버튼 3개(`create_sketch(plane)`), 면 클릭으로 평면 고르기 없음. 시작 후 선택 도구 | 평면 선택 흐름 다름. Line 자동 활성 없음 | P1 | FE+BE |
| Look At on start (스케치 평면에 시선 맞춤) | 응용 옵션 "Look at sketch plane on sketch creation and edit": 켜면 새 스케치·편집 때 그래픽 창이 스케치 평면과 평행으로 재정렬, 끄면 현재 시점 유지 [S6] | (옵션) | 시작 시 시점 처리 (미확인)(코드에서 자동 정렬 미발견) | 확인 필요 | P1 | FE |
| Slice Graphics (그래픽 절단) | 상태바 버튼 또는 F7: 시점 쪽 모델을 잘라내 스케치 평면을 드러냄. 피처 명령 클릭·스케치 종료·F7 재입력으로 복원 [S8] | F7 [S28,S29] | 없음 | 로봇·설비가 평면을 가릴 때 필요 | P2 | FE(+render) |
| Finish Sketch (스케치 마무리) | 스케치 탭 Exit 패널 "Finish Sketch" 또는 그래픽 영역 우클릭 "Finish 2D Sketch" [S1]. Exit 패널의 한글 라벨은 (미확인) | (없음, 도움말에 단축키 없음) | 종료 패널 "스케치 마무리" 있음. 우클릭 "Finish 2D Sketch" 없음 | 우클릭 항목 추가 | P1 | FE |
| Cancel Sketch (스케치 취소) | 스케치 시작·편집 시 리본에 Finish와 함께 표시. 지금까지 한 모든 동작을 되돌림(생성 중이면 스케치 생성까지). Undo/Redo 목록에 동작이 쌓임 [S1,S33(2024.1)] | | 없음(Ctrl+Z 반복만) | 한 번에 되돌리는 명령 없음 | P2 | BE+FE |
| 다른 스케치 편집으로 전환 | 브라우저에서 스케치 아이콘 더블클릭, 우클릭 "Edit Sketch", 평면 위 우클릭 "Edit Sketch", 치수 우클릭 "Edit Sketch" [S1]. 2025.2부터 Finish/Cancel 없이 다른 스케치를 더블클릭해 곧바로 전환 [S33] | | 브라우저 행 더블클릭으로 진입(`enter_sketch`). 활성 스케치에서 다른 스케치로 직접 전환은 (미확인) | 직접 전환·치수 우클릭 진입 없음 | P2 | FE |
| 스케치 표시 | F10 "Sketch visibility" [S28]. 브라우저 스케치 우클릭 "Dimension Visibility", 뷰 탭 Object Visibility의 Sketch Dimensions [S14] | F10 | 브라우저 가시성 토글(스케치별) | 치수 표시 토글·F10 없음 | P3 | FE |
| 평면 좌표계 재정렬 | Constrain 패널 "Edit Coordinate System": 축 화살표 클릭 후 모서리 클릭으로 회전, 원점 점 클릭 후 꼭짓점 클릭으로 이동, 우클릭 Flip Axis/Done [S31] | | 없음 | 스케치 평면 정렬은 면 선택 없음 | P3 | BE+FE |

### 2.2 Create 패널 — 선·원·호 (Line, Circle, Arc)

공통(인벤터): 명령 종료는 Esc 또는 다른 명령 클릭 [S2,S3]. 우클릭 메뉴의 "Done"/"OK" 항목 문구는 Constrain 계열에서만 도움말에 명시되어 있고(§2.6) 그리기 명령의 전체 우클릭 메뉴는 (미확인). 선 그리기 중 Backspace는 마지막 선분 제거 [S28].

| 명령 | 인벤터 2026 동작 | 단축키 | GraphCTRL 현재 | 차이 | 우선 | 담당 |
|---|---|---|---|---|---|---|
| Line (선) — Create 패널 [S2] | 1번 클릭 = 시작점. 이후 클릭마다 이어지는 선분(끝점끼리 **자동 일치 구속**) [S2,S25]. **더블클릭 = 선 종료** [S2]. **끝점에서 드래그**: 호 미리보기가 나오고 놓으면 시작점에 접하는(또는 직각) 호가 되며, 이어서 클릭하면 다음 선분·호 [S2]. 우클릭 "Close"는 마지막 선분 뒤에 시작점으로 닫는 선을 그리고 명령은 유지 [S13]. 거의 수직·수평이면 자동으로 수직·수평 구속 [S27]. HUD가 켜져 있으면 길이·각도 입력(§3.2) [S4] | L | 클릭 연속 선(끝점 스냅 시 일치), 수평/수직 스냅(4 px)이 H/V 구속, Esc로 이탈. 끝점 드래그 호·더블클릭 종료·Backspace·Close 없음 | 접선/직각 호 전환, 더블클릭, Backspace, Close | P1 | BE(도구)+FE(입력) |
| Midpoint Line (중점 선) | 1번 클릭 = 선의 중점, 2번 클릭(또는 입력창 길이·각도) = 끝점. 중점과 선택한 기하 사이에 일치 구속 [S2,S33(2025.2)] | (미확인) | 없음 | 신규 | P2 | BE |
| Center Point Circle (중심점 원) [S3] | 1번 클릭 = 중심, 2번 클릭 = 반지름점. 2번째 점이 선·호·원·타원 위이면 **접선 구속** 적용. 반복해서 계속 그림 | C | 중심→림 두 번 클릭. 접선 추론(림이 곡선 위일 때)은 PLAN 부록 C에는 있으나 도구 구현 범위는 (미확인) | 접선 추론 확인 | P1 | BE |
| Tangent Circle (접선 원) [S3] | "세 선에 접하는 원": 선 3개를 선택(도움말 문장이 중심점 원과 중복되어 모호) — 세부 순서 (미확인) | (미확인) | 없음 | 신규 | P2 | BE |
| Three Point Arc (3점 호) [S2] | 1번 = 첫 끝점, 2번 = 다른 끝점(현), 3번 = 방향·반지름(호 위의 점). HUD 지원 [S4] | A는 별칭 아님(표에 없음) | 없음 | 신규 | P1 | BE |
| Center Point Arc (중심점 호) [S2] | 1번 = 중심, 2번 = 반지름·시작점, 3번 = 끝점. HUD 지원 [S4] | | 중심·시작·끝 3클릭 호(커서가 쓸어간 방향으로 CCW) = 같은 순서. 단축키 A | 순서 일치. 호 방향 규칙·키 확인 | P1 | BE |
| Tangent Arc (접선 호) [S2] | 기존 곡선 위에 커서를 올리면 끝점이 강조되고, 끝점 근처 클릭 = 호 시작(접선 구속), 이동 후 클릭 = 호 끝 | | 없음 | 신규 (선 도구의 끝점 드래그와 같은 결과) | P1 | BE |

### 2.3 Create 패널 — 사각형·슬롯·다각형·타원·스플라인·점

| 명령 | 인벤터 2026 동작 | 단축키 | GraphCTRL 현재 | 차이 | 우선 | 담당 |
|---|---|---|---|---|---|---|
| Two Point Rectangle (2점) [S3] | 대각 모서리 2클릭, 스케치 축에 정렬. HUD 지원 [S4] | (표에 없음) | 2클릭. 선4+일치4+H,V,H,V(PLAN 5.5). 단축키 R(인벤터 표에 없음) | 구속 구성은 같음으로 보이나 **(인벤터가 만드는 구속 목록은 도움말에 없음, 미확인)** | P1 | BE |
| Three Point Rectangle (3점) [S3] | 1=모서리, 2=한 변의 방향·길이, 3=인접 변 길이. 임의 방향. HUD 지원 [S4] | | 없음 | 신규 | P1 | BE |
| Two Point Center / Three Point Center Rectangle [S3] | 2점 중심: 1=중심, 2=모서리. 3점 중심: 1=중심, 2=한 변 방향·거리, 3=인접 변 거리 | | 없음 | 신규 | P2 | BE |
| Slot (슬롯) 5종 [S3] | Center to Center: 호 중심 2점 클릭 후 3번째로 너비. Overall: 중심선 시작·끝 2점 후 너비. Center Point: 슬롯 중심, 호 중심 1점, 너비. Three Point Arc Slot(호 슬롯): 중심 호의 시작·끝 2점, 3번째 = 호 중심, 마지막 = 너비. Center Point Arc Slot: 중심점, 중심 호의 시작·끝, 너비(4클릭). 값은 입력창·우클릭으로 직접 입력 가능 [S3] | (미확인) | 없음 | 신규 5종 | P2 | BE |
| Polygon (다각형) [S3] | 대화상자: Inscribed(꼭짓점 기준)/Circumscribed(변 중점 기준), 변 수(최대 120). 중심 클릭 후 드래그. Esc·다른 명령·대화상자 Done으로 종료 | (미확인) | **BE 완료**: `SketchOp::AddPolygon{centre, at, sides(3–120), circumscribed}` — 선 n개 + 구성 원(내접: 꼭짓점이 원 위, 외접: 변이 원에 접함), 꼭짓점 일치, 변 같음(짝수 외접은 첫 변 중점으로 가는 구성 반지름이 그 변과 직각). 남는 자유도 4(중심·크기·회전). 원문에 구속 목록은 없어 구성은 자체 결정 | 대화상자(내접/외접·변 수)·중심 클릭 후 드래그 도구는 FE | P2 | BE+FE |
| Ellipse (타원) [S3] | 1=중심, 2=첫 축의 방향·길이(중심선 표시), 3=두 번째 축 길이 | (미확인) | 없음 | 신규(타원 기하 자체가 `CurveGeom`에 없음) | P3 | BE |
| Interpolation Spline (보간) [S11] | 클릭마다 맞춤점. 끝점은 사각형, 맞춤점은 마름모. 시작점 우클릭 "Close Curve". 핸들 편집(Activate Handle), Insert Point, Split Spline, Spline Tension 등 [S11] | (미확인) | 없음 | 신규 | P3 | BE |
| Control Vertex Spline (제어점) [S11] | 클릭마다 제어점(제어 프레임은 구성선, 제어점은 원). 완료는 OK, 계속은 Apply | | 없음 | 신규 | P3 | BE |
| Point / Center Point (점) [S10] | 클릭으로 점 배치(기본은 Center Point = 십자, Format 패널 토글로 일반 점 = 점). 우클릭 Point Snaps(Midpoint·Center·Intersection 등 8종)로 정밀 배치. 구속 없이 생성 [S10]. HUD: Tab → X, Tab → Y, Enter [S4] | (미확인) | 선·원 끝점이 점. 독립 점 도구 없음 | **BE 완료**(`SketchOp::PlacePoint{pos, center}`: 구속 없음, 기본 센터 점은 도구가 `center: true`로; `SetCenterPoint{points, center}` 토글 — 독립 점만). 도구·HUD(X/Y)·우클릭 토글은 FE | P2 | BE |
| Fillet (모깎기) [S3] | 대화상자 반지름 + Equal. 선 두 개(또는 끝점 공유 모서리 위에 커서를 올려 미리보기) 선택. 교차하는 선은 호 끝으로 **트림**됨. 반지름은 다음 사용까지 유지, Esc·대화상자 닫기로 종료 | (미확인; 표의 F는 파트 Fillet) | 없음 | 신규(트림 의존) | P1 | BE |
| Chamfer (모따기) [S3] | 대화상자: Create Dimensions, Equal to Parameters(기본 켜짐). 방식 3: Equal Distance, Unequal Distance, Distance and Angle. 선 두 개 선택 | (미확인) | 없음 | 신규 | P2 | BE |
| Text / Geometry Text [S8] | Create 패널 Text: 클릭=삽입점, 드래그=텍스트 상자, Format Text 대화상자(글꼴·속성·매개변수 삽입). 생성 후 더블클릭 편집, 드래그 이동, Delete. 미리보기 갱신(2026.1) [S33]. Geometry Text는 선·호·원에 정렬 | T [S28,S29] | 없음 | 신규(렌더·입력 별도 작업, 필요성 낮음) | P3 | FE+BE |
| Project Geometry (형상 투영) [S20,S21] | Create 패널. 모서리·꼭짓점·작업 요소·루프·곡선 클릭 → 참조 형상(연결된 연관 형상, 고정). 면 클릭 = 그 면의 모든 루프. Shift+클릭 = 구성/일반 반전. 우클릭 Break Link, Replace Reference(2026.1) [S20,S33]. 종료: 우클릭 Done·Esc·다른 명령 | (미확인) | 원점 투영 점 자동 생성(PLAN 5.5 외부 참조: 원점·작업점·다른 스케치 점). 투영 명령 없음 | 명령 없음. 연관·Break Link·Broken 상태 일부 있음(PLAN) | P2 | BE+FE |
| Auto-project (자동 투영) [S6,S21] | 옵션 3가지: "Autoproject edges during curve creation"(기존 선을 커서로 훑으면 참조로 투영), "Autoproject edges for sketch creation and edit"(선택 면의 모서리를 스케치 시작·편집 때 자동 투영), "Autoproject part origin on sketch create"(기본 켜짐) [S6]. 면 위에 만든 스케치의 모서리는 자동 투영되며 이 자동 참조는 삭제 불가 [S21] | | 원점 자동 투영만(삭제 불가, 5.5) | 모서리·곡선 자동 투영 없음 | P2 | BE |

### 2.4 Modify 패널

일반: 이동·복사·회전·신축·축척은 같은 대화상자 패턴이다 — 선택 모드(클릭 / 드래그 상자 / 우클릭 Select All)로 고르고 기준점·끝점을 클릭하며, 끝점 입력 중 Backspace로 되돌린다. 미리보기는 원본 파선·결과 실선. 기준점은 가장 가까운 수평·수직 축에 스냅되며 Ctrl로 억제 [S17,S18]. "Optimize for Single Selection"은 하나 선택하면 곧바로 기준점 모드로 넘어감. 대화상자 More 옵션으로 치수 완화(Never/If No Equation/Always/Prompt)와 구속 해제(Never/Always/Prompt) 선택, 기본 Prompt [S17,S18]. 종료: Done·우클릭 Done·Esc.

| 명령 | 인벤터 2026 동작 | 단축키 | GraphCTRL 현재 | 차이 | 우선 | 담당 |
|---|---|---|---|---|---|---|
| Move (이동) [S17] | 위 일반 + 대화상자 Copy 체크(복사본 이동), Precise Input(X,Y) | (미확인; 표의 M은 어셈블리) | 선택 도구로 점·선 드래그(솔버 드래그)만 | 명령 없음. 드래그는 제약을 지키며 이동(릴렉스 모드 없음) | P2 | BE+FE |
| Copy (복사) [S17] | 위 일반 + Clipboard [Ctrl+C]로 다른 스케치에 붙여넣기. 복사 시 파선 원본·실선 복사본 | | 없음 | 신규 | P2 | BE |
| Rotate (회전) [S17] | 중심점 모드, 끝점으로 각도, 대화상자 Angle 직접 입력(고정됨) | | 없음 | 신규 | P2 | BE |
| Trim (트림) [S19] | 커서를 곡선 위에 올리고 잠시 멈추면 미리보기, 클릭 = 가장 가까운 교차 곡선까지 자름. **끝점이 경계 곡선에 일치 구속**. 교차가 없으면 곡선 삭제. **마우스 누른 채 이동 = 지나간 모든 곡선 트림(동적)**. Ctrl+선택 = 경계 지정. Shift 누르고 선택 = 임시 Extend. 우클릭 = Split/Extend로 전환 | TR [S28,S29] | 없음(STATUS "다음 후보 2번") | 신규 | P1 | BE+FE |
| Extend (연장) [S19] | 트림의 반대. 가장 가까운 교차 곡선까지 연장하고 일치 구속. 동적 연장, Ctrl로 경계 지정, Shift = 임시 Trim | (미확인) | 없음 | 신규 | P1 | BE+FE |
| Split (분할) [S19] | 선택 곡선을 가장 가까운 교차 곡선에서 분할(치수 유지). 두 조각이 H/V/평행/직각/동일선상 구속을 상속, 동일·대칭 구속은 필요 시 해제. 커서 근처 끝점 쪽 영향 | (미확인) | 없음 | 신규 | P2 | BE |
| Offset (오프셋) [S18] | 곡선 선택 후 커서 이동·클릭 또는 부동 입력창에 거리 입력. 기본 **루프 선택**(Loop Select, 우클릭 해제 가능), **동일 거리 구속**(Constrain Offset, 우클릭 해제 가능). 타원은 위치에 따라 타원/스플라인 [S3] | (미확인) | 없음 | 신규 | P2 | BE |
| Scale (축척) [S18] | 기준점 선택 후 끝점 클릭, 또는 Scale Factor 직접 입력 | | 없음 | 신규 | P3 | BE |
| Stretch (신축) [S18] | 기준점 지정 후 끝점. 선택 범위에 따라 1차·2차 형상 구분(구속 처리 규칙이 이를 따름) | | 없음 | 신규 | P3 | BE |

### 2.5 Pattern 패널

| 명령 | 인벤터 2026 동작 | 단축키 | GraphCTRL 현재 | 차이 | 우선 | 담당 |
|---|---|---|---|---|---|---|
| Rectangular Pattern (사각 패턴) [S12] | 대화상자: 기하 선택 → 방향 1 선택(화살표) + Count + Spacing(수식 가능), 방향 2 동일. 선택 Extents: Boundary(Include Geometry/Include Centroids/Base Points). 고급: Suppress, Associative, Fitted. OK로 생성. 패턴 형상은 완전 구속이며 구속은 그룹으로 유지. 경계 밖 형상은 다른 색·선종류(2026.2) | RP [S29] | 없음 | 신규 | P2 | BE+FE |
| Circular Pattern (원형 패턴) [S12] | 축(점·꼭짓점·작업축) 선택, Count, Angle(첫~끝 요소 사이 각도, 수식 가능) | CP [S29] | 없음 | 신규 | P2 | BE+FE |
| Mirror (대칭 복사) [S12] | 대화상자: 기하 선택 → Mirror Line 선택(파란 화살표). 스플라인이 대칭선과 교차하면 Self Symmetric. Apply로 계속·Done으로 종료 | (미확인; 표의 MI는 피처 미러) | 없음 | 신규(대칭 구속은 있음) | P2 | BE |
| 패턴 편집 | 요소 우클릭 Edit Pattern, 패턴 치수 더블클릭, Delete / Delete Pattern / Suppress Element(s) [S12] | | 없음 | | P3 | BE+FE |

### 2.6 Constrain 패널 — 구속조건

공통: 단축키 없음(수평 `-` 만 [S25,S29]). 구속 명령 안에서 클릭 순서대로 선택하고 **우클릭 "Done"(또는 Esc, 다른 명령)** 으로 끝낸다 [S25]. 명령은 연속 사용 가능(같은 구속을 여러 쌍에 계속 적용). 선택 순서·대상은 아래 표를 따른다. 수평/수직은 명령 선택 후 **Shift를 누르면 반대 구속으로 전환**(2023.1) [S33]. GraphCTRL은 이미 구속 12종 버튼이 있고 선택 순서 규칙은 구현 확인 범위 밖이다(PLAN 5.4 `try_add` 판정).

| 구속 (한글 관행명) | 인벤터 2026 선택 순서·규칙 [S25] | GraphCTRL 현재 | 차이 | 우선 | 담당 |
|---|---|---|---|---|---|
| Coincident (일치) | 점 클릭 → (점 또는 곡선) 클릭. 한 점이 곡선 위에 놓이면 곡선 연장선 위에도 있을 수 있음(곡선 범위 밖 가능). 선을 연속 그리면 끝점 일치 자동 생성. 노란 점 = 구속 존재, 초록 점 = 릴렉스 때 제거 대상 [S25] | 있음(`Coincident`, `PointOnCurve`) | 노란/초록 점 표시 규칙(미확인) | P1 | FE |
| Collinear (동일선상) | 선(또는 타원 축) 2개 | 있음 | | - | - |
| Concentric (동심) | 호·원·타원 2개(결과는 중심점 일치와 같음) | 있음 | | - | - |
| Fix (고정) | 곡선·중심점·중간점·점 클릭, 계속 클릭 가능. 선은 위치·각도 고정(끝점은 길이 변경 가능), 원·호는 중심·반지름 고정. 좌표계를 옮기면 따라옴 | 있음(점 고정). 선·원에 대한 의미가 같은지 (미확인) | 선 고정 의미 확인 | P2 | BE |
| Parallel (평행) / Perpendicular (직각) | 선·곡선·타원 축 2개. 스플라인 직각은 끝점끼리만 | 있음 | | - | - |
| Horizontal (수평) / Vertical (수직) | 선·타원 축 하나, 또는 **점 2개**. 계속 클릭 가능 | 있음(점쌍 형태 `Horizontal(p,q)`) | Shift 전환, `-` 키 | P2 | FE |
| Tangent (접선) | 곡선 2개 | 있음(선-원, 원-원, 끝 접선) | | - | - |
| Smooth G2 (스무스) | 스플라인과 그 끝점의 다른 곡선(선·호·스플라인) 선택. 곡률 연속 | 없음(스플라인 없음) | 스플라인 도입 후 | P3 | BE |
| Symmetric (대칭) | 곡선(또는 점) 1, 2 선택 후 **대칭선** 선택. 대칭선을 삭제하면 구속도 삭제. 대칭선은 어떤 선종류도 가능 | 있음(`Symmetric`) | | - | - |
| Equal (동일) | 같은 종류의 원·호·선 2개 | 있음 | | - | - |
| Midpoint (중점) | 도움말 S25 구속 목록에 없음(별도 중점 구속은 (미확인)). 점 스냅 "Midpoint"와 일치 구속으로 다룸 [S10] | 있음(`Midpoint`, 리본 버튼 "중점") | 용어·동작 정렬 확인 | P3 | BE |
| 구속 글리프 상호작용 | 호버 = 연관 형상·파트너 글리프 강조 [S24,S25]. 글리프 드래그로 이동(기하 이동 때 상대 위치 유지, 가시성 재토글 시 기본 위치로 복귀). 클릭 선택 후 **Delete = 구속 삭제**. 우클릭 글리프 "Hide". 기하 선택 후 우클릭 "Delete Constraints" [S24]. 참조(투영) 구속 삭제 = Break Link [S24] | 글리프 호버 강조·Delete는 있음(PLAN 5.5). 드래그 이동·우클릭 Hide·Delete Constraints는 (미확인) | 드래그·우클릭 항목 | P2 | FE |
| Show / Hide Constraints (F8 / F9) | 상태바 "Show All Constraints/Hide All Constraints" 또는 F8/F9. 일반 탭 옵션: Display Constraints on Creation(만들 때만 표시, 명령 끝나면 자동 숨김), Show Constraints for Selected Objects(선택한 기하의 구속 표시, 삭제 가능), Display Coincident Constraints in Sketch [S24,S29] | 글리프는 항상 표시되는 것으로 보임(스크린샷). F8/F9 없음 | 표시 모델 다름 | P1 | FE |
| Constraint Settings (구속 설정) | Constrain 패널 > Constraint Settings: General(위 3개 표시 옵션), Inference(Infer Constraints, Persist Constraints, 우선순위 "Parallel and Perpendicular"(기본) / "Horizontal and Vertical", 추론할 구속 종류 선택), Relax Mode 탭. Inference Scope(Geometry in Current Command(기본)/All Geometry/Select) [S23,S24] | 없음 | 옵션 UI 전체 | P2 | FE+BE |

### 2.7 Constrain 패널 — 치수 (Dimension)

| 명령 | 인벤터 2026 동작 | 단축키 | GraphCTRL 현재 | 차이 | 우선 | 담당 |
|---|---|---|---|---|---|---|
| General Dimension (일반 치수) [S14,S16] | Constrain 패널 Dimension. 기하를 클릭하고 클릭으로 배치. 다음 치수로 계속 선택, 끝은 우클릭 OK. 만들 수 있는 종류: 한 요소 선형, 두 요소 간 선형/정렬, 두 모서리 각도, 세 점 각도, 내부·외부 각도, 기준선 각도, 반지름, 지름 [S16]. 중점·끝점·중심 스냅 필터(2025.1) [S33]. 클릭한 대상과 커서 위치로 종류가 정해지는 구체 규칙은 도움말에 없음 (미확인) | D | 치수 도구: 선→길이, 두 점→수평/수직/정렬, 평행선→거리, 두 선→각도(사분면), 점+선→수직거리, 원→지름, 호→반지름(PLAN 5.5). 값 상자 즉시 표시, Enter 확정, 도구 유지(S2 시나리오) | 인벤터는 "클릭 후 배치 클릭"과 HUD 값 상자를 같이 씀. 중점 스냅 필터 없음 | P1 | BE+FE |
| 치수 편집 [S14] | 더블클릭, 또는 Dimension 명령 중 클릭, 또는 우클릭 메뉴. **Edit Dimension 대화상자**: 값 직접 입력, 화살표 메뉴에서 Measure(기하를 클릭해 측정값 사용)·Show Dimensions(브라우저에서 피처 선택 → 그 치수 클릭해 참조)·목록 값 선택. 체크 표시(OK)로 확정. 식 입력 가능: `HGHT = 5mm`은 매개변수 HGHT를 만들고 5 mm를 할당. 치수 텍스트 위에서 이동 커서가 나오면 드래그로 위치 변경 | 더블클릭 | 값 상자에서 수식·매개변수 지원(`gc_expr`, PLAN 5.5 `source`). 치수 더블클릭 편집(스케치 캔버스)은 (미확인). Measure·Show Dimensions 메뉴 없음 | 편집 진입점, 대화상자 구성 | P1 | FE |
| 구동/참조 치수 (Driven) [S14] | 완전 구속 스케치에 치수를 넣으면 **과구속 알림 후 Accept(= 괄호로 표시되는 참조 치수) / Cancel**. 구동 치수를 일반으로 바꾸려면 완전 구속이 아니어야 하며, 선택 후 Format 패널의 **Driven Dimension 토글**을 끈다. 구동 치수 값은 복사해 일반 치수 입력에 붙여넣기 가능(2024.1) [S33] | | 과구속 프롬프트(수락 = 괄호 참조 치수, 취소 = 문서 불변)(S3 시나리오). Format 토글 없음 | 토글 추가 | P1 | FE+BE |
| 치수 표시 형식 [S14] | 상태바 아이콘: Value / Name / Expression / Tolerance / Precise Value. 수식 치수는 `fx:` 접두. 색: 호버 = 주황, 선택 = 파랑, 기본 = 검정 [S27] | | 값만 표시(`"60 mm"`). `fx` 표시 (미확인) | 표시 모드 5종 | P2 | FE |
| 치수 방향 뒤집기 (Flip Direction) [S14,S33] | 우클릭 Flip Direction(평행선 거리·점-선 거리·점쌍 수평/수직·선-호 거리·호-호 거리·각도), 다중 선택 가능 | | 없음 | | P3 | BE+FE |
| 삭제·표시 [S14] | 선택 후 Delete 또는 우클릭 Delete | Delete | Delete 있음 | - | - |
| Automatic Dimensions and Constraints (자동 치수) [S15] | Constrain 패널. Auto Dimension 대화상자: Curves 선택, 체크박스 Dimensions/Constraints(기본 켜짐), "Dimensions Required"(필요 수, 제외 항목은 합계에서 뺌). 선택 없음 = 전체 스케치. Apply/Remove/Done | (미확인) | 없음 | 신규 | P2 | BE+FE |

## 3. 공통 동작

### 3.1 추론과 스냅

- **추론(inference)**: 기하를 만들거나 수정할 때 어떤 구속을 쓸 수 있는지 계속 판단하고, 구속 기호와 (해당될 때) **점선 정렬선**을 보여준다 [S22]. 구속은 도형을 만든 뒤에도 남는다(Persist) — 끄면 만들어지지 않지만 추론 위치는 맞다. 선 끝점끼리의 일치 구속은 열린 프로파일을 막기 위해 항상 유지되며, Selection for Constraint Inference에서 Coincident를 끄지 않는 한 변하지 않는다 [S23].
- **억제**: 기하를 만드는 동안 **Ctrl**을 누르고 있으면 구속 추론을 일시적으로 끈다 [S23,S25]. 이동·복사·회전의 기준점 스냅 억제도 Ctrl [S17].
- **우선순위(기본: 평행/직각 먼저, 그다음 스케치 격자 좌표)** 또는 수평/수직 먼저를 설정으로 고른다 [S23]. **범위**: 현재 명령에서 만든 기하(기본), 전체 기하, 선택한 기하. 범위와 상관없이 **어떤 기하든 그 위에 커서를 잠시 멈추면 그 기하에서 추론**한다 [S23].
- **점 정렬 (Point alignment)**: 켜면 새 기하의 끝점이 기존 점들과 정렬됨을 점선으로 보여준다. 끄면 점 위로 커서를 지나가야 일시 호출된다 [S6].
- **격자 스냅**: 상태바 Snap to Grid 버튼 또는 옵션. 격자가 안 보여도 스냅 가능 [S32]. 간격·격자 표시 수는 문서 설정 Sketch 탭(X, Y 스냅 간격, Snaps Per Minor, Major Every) [S7,S32].
- **점 스냅(우클릭 Point Snaps)**: Midpoint, Center, Apparent Intersection, Intersection, Endpoint, Quadrant, Tangent, Mid of 2 points [S10].
- 픽셀 허용오차·히스테리시스·스냅 마커 모양은 도움말에 없음 (미확인).
- **GraphCTRL 현재**: 순수 함수 `snap(...)`, 점(8 px)·곡선 위(6 px)·수평/수직(4 px)·Ctrl 억제 [pick.rs, tolerance]. PLAN 부록 C는 중점·교차·사분점·접선·직각/평행·정렬을 계획하지만 현재 `SnapKind`는 5종. **차이**: 중점·교차·사분점·접선·평행/직각 스냅과 마커, 점선 정렬선, 추론 설정(우선순위·범위), 점 스냅 우클릭 메뉴.

### 3.2 HUD(Dynamic Input)와 정밀 입력

- **켜기**: 응용 옵션 > Sketch > Heads-Up Display(HUD). 기본 설정과 세부는 Settings 대화상자 [S4,S5,S6].
- **지원 도구**: Line, Center Point Circle, Three Point Arc, Tangent Arc, Center Point Arc, Two Point Rectangle, Three Point Rectangle, Point [S4]. (Dynamic Input 개요 문서는 Line·Circle·Arc·Rectangle·Point라고만 적음 [S35].)
- **순서**: 도구 선택 → 커서 근처에 X, Y(Pointer Input) 표시 → 첫 점을 클릭하거나 값을 입력 → 강조된 입력창에 길이·각도·지름·반지름 등 입력 → **Tab으로 다음 입력창** → 값을 입력한 필드에는 **자물쇠 아이콘**(커서가 그 값에 구속됨) → 모든 값이 들어가면 **Enter로 도형 완성** → 도형은 입력한 값으로 **치수가 붙은 채** 그려짐 → 계속 그리거나 Esc [S4].
- **좌표계**: Pointer Input(첫 점)은 Cartesian(기본)/Polar를, Dimension Input(두 번째 점부터)은 Cartesian/Polar(기본: 마지막 점 기준 극좌표 길이·각도)를 고른다. "Enable Dimension Input where possible"을 끄면 ΔX/ΔY만 쓰고 **치수가 자동 배치되지 않는다** [S5].
- **점 입력**: Point 명령에서 Tab → X 입력 → Tab → Y 입력 → Enter [S4].
- **Precise Input 툴바**(Create 패널 확장 > Precise Input): X, Y(, Z) 입력 후 Enter, Relative/Absolute 선택, Reposition Triad/Reset to Origin, 종료는 우클릭 Close [S9]. Move/Copy/Rotate/Stretch/Scale 대화상자에서도 Precise Input으로 기준점·끝점 좌표 지정 [S17,S18].
- **상태바 버튼(3D)**: Ctrl+D = Dynamic Dimension(치수 HUD) 토글, Ctrl+R = 직교(Ortho), Ctrl+I = 구속 추론, Ctrl+Shift+P = 객체 스냅 [S25,S29]. 2D 스케치에서 같은 버튼의 단축키는 (미확인).
- **GraphCTRL 현재**: HUD 입력창 없음. 치수 값 상자만 있음(`review-2-value-box.png`: 커서 옆 단일 입력, Enter 확정, Esc 취소). 입력창 간 Tab 이동·자물쇠·극좌표·자동 치수는 모두 없음. 3D에서 Tab은 평면 전환(인벤터는 Shift+Space [S29]) — **충돌 규칙 필요**.

### 3.3 스케치 안의 선택

- 클릭 = 선택. 왼쪽→오른쪽 드래그 = 완전히 **포함된** 객체만(Window). 오른쪽→왼쪽 드래그 = 포함 + **교차**한 객체(Crossing) [S30].
- Shift+클릭 / Ctrl+클릭 = 추가·제거(이미 선택된 객체를 다시 클릭하면 해제). 상자 드래그 + Shift = 미선택 객체만 추가(나머지 유지), 상자 드래그 + Ctrl = 선택 상태 **반전**. 빈 곳 클릭 = 선택 해제 [S30].
- Select Other: 우클릭 메뉴 또는 Ctrl+A로 겹친 후보를 화살표로 순환 [S30,S29]. 결과 목록에 스케치 이름이 표시됨(2023.2) [S33].
- 우클릭 Find in Browser(선택한 기하의 스케치를 브라우저에서 강조) [S8].
- **GraphCTRL 현재**: 클릭 선택·Ctrl/modifier 토글, 점·곡선·치수 텍스트 피킹(8/6/14 px). **상자 선택(창/교차)·Select Other·Shift 규칙은 없음**(`ToolEvent`에 DragStart/Drag는 솔버 드래그용이며 상자 선택은 코드에서 미발견).

### 3.4 Esc와 우클릭의 의미

- **Esc** = 현재 명령 종료(그리기 명령 도움말 전반 [S2,S3]); 구속·수정 명령은 우클릭 Done과 동등 [S25,S17]. 명령 중 부분 입력이 있을 때 "먼저 입력만 취소하는지"는 도움말에 없음 (미확인).
- **우클릭** = 명령 중에는 컨텍스트 메뉴(Done, Close(선) [S13], Loop Select/Constrain Offset(오프셋) [S18], Select All(수정 명령) [S17], Point Snaps [S10] 등). 명령이 없을 때 객체 위에서는 객체별 메뉴(Edit Sketch, Finish 2D Sketch, Break Link, Delete Constraints, Display Degrees of Freedom 등 [S1,S20,S24,S26]). 메뉴의 정확한 항목 순서와 "Done [Esc]" 같은 단축키 표기는 도움말에 없음 (미확인).
- **Delete** = 선택 삭제 [S9,S28]. **Backspace** = 선 그리기 중 마지막 선분 제거 [S28].
- **GraphCTRL 현재**: Esc가 도구에서 한 단계 물러나거나 나감(PLAN 5.5, `Key::Escape`), 그리는 중 우클릭 메뉴 없음, 우클릭은 셀 항목 메뉴만. **차이**: 우클릭 = Done 메뉴(+도구별 항목), Esc 의미 통일.

### 3.5 색, 자유도 표시, 상태바

- **자유도(DOF) 표시**: 명령이 없을 때 상태바 "Show All Degrees of Freedom"(또는 개별 기하 우클릭 Display Degrees of Freedom). 새 기하는 자동 표시되지 않는다 — 다시 Hide → Show [S24,S26]. 글리프: 평행이동 1방향(점 = 축 방향 이동, 곡선 = 위치), 2D 평행이동(점이 평면에서 자유), 회전(곡선). 구속이 늘면 글리프가 사라지고 삭제하면 다시 생긴다 [S22,S26]. 예: 한 번에 하나씩 수직 → 고정 → 치수로 자유도 0 [S26].
- **완전 구속**: 모든 자유도가 없어지면 fully constrained, 아니면 under-constrained. 완전 구속 기하는 **다른 색**으로 표시되며 색은 현재 In-canvas Color scheme을 따른다 [S14,S22] — **구체 색 값은 (미확인)**. 상태바는 "완전 구속에 필요한 치수 수"를 보여준다 [S22] — 정확한 문구와 0일 때 문구는 (미확인).
- **치수 색**: 호버 = 주황, 선택 = 파랑, 기본 = 검정 [S27]. 참조(구동 아님) 치수는 괄호 [S14,S22]. 구성선·참조 형상의 색·선종류는 (미확인).
- **Construction / Centerline / Center Point / Driven**(Format 패널): Construction 토글 — 그리기 전에 켜면 새 기하가 구성선, 선택 후 켜면 변환, 다시 누르면 해제. 구성선은 프로파일·피처를 만들지 않고 다른 기하를 구속하는 용도 [S8]. Center Point 토글 — 점 종류(십자 = 센터 점) [S10]. Driven Dimension 토글 [S14]. 투영을 구성선으로 하는 옵션 "Project objects as construction geometry" [S6]. **Centerline 토글은 도움말에서 확인되지 않음 (미확인)**.
- **GraphCTRL 현재**: 엔티티별 색, 상태바 "N개 치수 필요"/"완전 구속"(`StatusFullyConstrained`), 스크린샷에서 선택 = 주황, 일반 형상 = 파랑, 완전 구속 = 검정으로 보임. F8/F9·DOF 글리프(평행이동·회전) 없음. **차이**: DOF 글리프 3종과 표시 토글, Format 패널(구성선·센터 점·참조 치수 토글), 색 체계를 인벤터 규칙(호버/선택/기본)에 맞춤.

### 3.6 릴렉스 모드 (Relax Mode)

- 상태바의 Relax Mode 버튼(상태는 세션 간 유지)으로 켠다. 켜면 구속된 기하도 드래그할 수 있다 — 충돌하는 **구속은 제거**(강조됨), 충돌 **치수는 Driven으로 전환**. 끄면 드래그가 구속·치수를 지우지 않으며 직접 찾아 지워야 한다. 구속·치수를 추가할 때도 같다(끄면 과구속 경고) [S24].
- 제외: Coincident, Tangent, Symmetric, Smooth는 릴렉스에서 바뀌지 않음. 수식으로 제어되는 치수는 변하지 않음. Pattern·Project 구속은 릴렉스 대상이 아님 [S24].
- 설정: Constraint Settings의 Relax Mode 탭 — Enable Relax Mode, 제거 대상 구속 종류, "Preserve Dimensions with Equations" [S23].
- **GraphCTRL 현재**: 없음. 완전 구속 형상은 드래그해도 안 움직임(PLAN 5.5, S5 시나리오)은 릴렉스 끔의 동작과 같다. **차이**: 릴렉스 모드 전체.

### 3.7 되돌리기(Undo) 단위

- 인벤터 Undo는 선분·호를 **하나씩 역순으로** 제거한다 [S9]. Cancel Sketch는 시작/편집 이후 모든 동작을 되돌리며 Undo/Redo 목록에 개별 동작이 쌓인다 [S1,S33]. 한 번의 드래그·한 번의 치수 편집이 몇 단계인지는 (미확인).
- **GraphCTRL 현재**: 드래그 한 번 = 1단계, Esc = 비트 복원(PLAN S4). 도구가 만든 편집을 한 Txn으로 묶는다. **차이**: 선 연속 그리기에서 선분당 1단계인지 확인, Cancel Sketch 신설.

### 3.8 응용 프로그램 옵션 > Sketch 탭, 문서 설정

| 옵션 | 인벤터 2026 [S6,S7] | GraphCTRL |
|---|---|---|
| Constraint Settings | 표시·생성·추론·릴렉스·과구속 설정 대화상자 | 없음 |
| Spline fit method | Standard / AutoCAD / Minimum Energy + 기본 장력 | 해당 없음(스플라인 없음) |
| Display | Grid lines, Minor grid lines, Axes, Coordinate system indicator | 격자·축 표시(토글 UI는 (미확인)) |
| Heads-Up Display (+Settings) | §3.2 | 없음 |
| Snap to grid | 켜기/끄기(상태바 버튼 포함) [S32] | 격자 스냅 규칙은 PLAN에 "그리드 → 암시 구속 없음" |
| Autoproject 3종 | §2.3 | 원점 자동 투영만 |
| Project objects as construction geometry | 기본 꺼짐 | 없음 |
| Look at sketch plane on sketch creation and edit | 기본값은 도움말에 없음(미확인) | (미확인) |
| Point alignment | 점선 정렬 | 없음(수평/수직 스냅만) |
| Auto-scale sketch geometry on initial dimension | 첫 치수에서 스케치 전체를 비례 확대 | 없음(미확인) |
| 3D: Auto-bend with 3D line creation | 기본 꺼짐 | 해당 없음 |
| Opacity of sketch displayed through shaded model | 기본 0 % (가려짐) | (미확인) |
| 문서 설정 Sketch 탭 | Snap Spacing X/Y, Snaps Per Minor, Major Every, 선 두께 표시 | 단위 설정 일부(`설정` 후보) |

## 4. 구현 우선순위

수용 기준은 "무엇을 하면 무엇이 보이는가"로 썼다. QA는 각 줄을 자동·수동 시나리오로 옮긴다. BE = `gc_sketch`·`gc_solver`·`gc_tools`, FE = `gc_viewport`·`gc_panels`·`gc_app`·`gc_ui`.

### P1 — "기본 스케치가 인벤터와 같다"

| # | 제목 | 담당 | 수용 기준 |
|---|---|---|---|
| P1-1 | 스케치 시작 시 Line 자동 활성 + 시점 맞춤 | FE | 새 스케치 시작 직후 선 도구가 켜져 있고(편집 재진입 때는 선택 도구), 옵션이 켜져 있으면 시점이 스케치 평면과 평행해진다. |
| P1-2 | 선 도구 완성(더블클릭 종료·Backspace·Close·끝점 드래그 호) | BE+FE | 끝점에서 드래그해 놓으면 접선 호가 생기고 이어서 클릭하면 다음 선분이 이어진다. 더블클릭하면 종료. Backspace가 마지막 선분을 지운다. 우클릭 Close가 닫는 선을 추가하고 명령은 유지. |
| P1-3 | 호 도구 3종(3점·중심점·접선) | BE | 3점 호는 3번째 클릭으로 방향·반지름이 정해지고, 접선 호는 시작점이 기존 끝점에 접선 구속으로 붙는다. 남은 DOF가 상태바 치수 수와 일치. |
| P1-4 | 사각형 2종 추가(3점, 2점 중심 선택은 P2) 와 구속 구성 | BE | 3점 사각형이 임의 각도로 그려지고 닫힌 선4 + 모서리 일치 + 직각 성질이 구속으로 저장되며, 상태바 필요 치수 수가 솔버 DOF와 같다. |
| P1-5 | 원 접선 추론, 점 정렬선, 추론 확장(중점·교차·사분점·접선) | BE+FE | 원의 림이 곡선 위이면 접선 구속이 저장되고, 점 위에서 점선 정렬선과 마커가 보인다. Ctrl 누르면 구속이 저장되지 않는다. |
| P1-6 | HUD 입력(선·원·호·사각형, Tab/Enter/자물쇠) | FE+BE | 도구 시작 후 커서 옆에 입력창이 나오고 값 입력+Tab으로 다음 칸으로 이동, 자물쇠 표시, Enter로 완성하면 값이 치수로 붙는다. Esc로 취소. |
| P1-7 | 우클릭 컨텍스트(Done, Close, Finish 2D Sketch, 구속 삭제) | FE | 명령 중 우클릭에 Done이 나오고 선택하면 명령이 끝난다. 빈 곳 우클릭에 "스케치 마무리"가 있다. |
| P1-8 | Trim / Extend / Fillet | BE+FE | Trim에서 호버 시 미리보기, 클릭하면 가장 가까운 교차까지 자르고 끝점 일치 구속이 생긴다. 누른 채 드래그하면 지나간 곡선 전부가 잘린다. Shift+클릭이 Extend로 일시 전환. Fillet은 반지름 입력 후 두 선 클릭으로 호+접선 구속이 생기고 교차 선은 트림된다. |
| P1-9 | 상자 선택(창/교차)과 Shift·Ctrl 규칙 | FE | 왼→오른쪽 드래그는 포함된 것만, 오른→왼쪽 드래그는 교차한 것도 선택. Ctrl+드래그는 선택 반전. |
| P1-10 | Show/Hide Constraints(F8/F9)와 표시 옵션 | FE | F8로 모든 구속 글리프가 보이고 F9로 숨겨진다. "만들 때만 표시" 옵션이 켜지면 명령 끝에 사라진다. 글리프 호버·클릭·Delete는 기존 동작 유지. |
| P1-11 | 치수 더블클릭 편집 + Edit Dimension(식·Measure) | FE | 치수 더블클릭 시 값 상자가 열리고 `HGHT = 5mm`가 매개변수를 만든다. Enter 확정, 체크로 확정, Esc 취소. |
| P1-12 | Driven Dimension 토글과 과구속 Accept/Cancel 흐름 | FE+BE | 완전 구속 상태에서 치수를 넣으면 대화상자, Accept가 괄호 치수를 만든다. 완전 구속이 아닐 때 토글로 일반↔구동 전환. |
| P1-13 | Format: Construction 토글(그리기 전·후) | BE+FE | 토글을 켜고 그린 선이 구성선(다른 선종류)이고 프로파일에서 제외된다. 선택 후 토글이 변환한다. |
| P1-14 | 인벤터 단축키(L, C, D, T, TR, `-`, F7(P2), F8, F9) 정렬 | FE | 표의 키가 같은 명령을 켠다. A·R은 별칭으로 유지. 3D Tab 충돌은 문서화. |

### P2

| # | 제목 | 담당 | 수용 기준 |
|---|---|---|---|
| P2-1 | Slot 5종·Polygon·Point/Center Point·Midpoint Line·Chamfer·Tangent Circle | BE+FE | 각 도구의 클릭 횟수·순서(§2.2–2.3)가 같다. |
| P2-2 | Move/Copy/Rotate(Precise Input·Copy 체크·Clipboard) | BE+FE | 선택 → 기준점 → 끝점 순서, 원본 파선/결과 실선 미리보기, Backspace로 끝점 취소. |
| P2-3 | Offset(Loop Select·Constrain Offset)·Split | BE | 루프를 한 번에 오프셋하고 기본으로 동일 거리 구속이 생긴다. Split이 치수를 유지. |
| P2-4 | Rectangular/Circular Pattern·Mirror | BE+FE | 개수·간격이 수식으로 편집되고 패턴 형상은 완전 구속, 패턴 삭제로 일괄 제거. |
| P2-5 | Project Geometry와 Auto-project 옵션, Break Link | BE+FE | 클릭한 형상이 참조(연관)로 들어오고 원본이 변하면 따라온다. Break Link로 일반 형상이 된다. |
| P2-6 | 릴렉스 모드와 Constraint Settings(추론 우선순위·범위·Persist) | BE+FE | 릴렉스 켬에서 구속된 선을 드래그하면 충돌 구속이 제거되고 치수가 Driven이 된다. |
| P2-7 | DOF 글리프(평행이동·2D 평행이동·회전)와 상태바 토글 | FE | "자유도 표시" 켜면 글리프가 보이고 구속을 더하면 줄어든다. |
| P2-8 | Cancel Sketch, Slice Graphics(F7), 스케치 간 더블클릭 전환 | FE+BE | 취소 시 스케치 시작 이전으로, F7로 가린 형상이 잘려 평면이 보인다. |
| P2-9 | Automatic Dimensions and Constraints | BE+FE | 선택 없음으로 실행하면 Dimensions Required 수만큼 치수·구속이 채워지고 Remove가 되돌린다. |
| P2-10 | 치수 표시 형식(Value/Name/Expression) | FE | 상태바 토글로 치수 텍스트가 바뀌고 수식 치수는 `fx:` 접두. |

### P3

| # | 제목 | 담당 | 수용 기준 |
|---|---|---|---|
| P3-1 | Ellipse, Interpolation/CV Spline, Smooth G2 | BE | 도구 순서(§2.3)와 핸들 편집이 같다. |
| P3-2 | Scale, Stretch, Text, Geometry Text | BE+FE | §2.4 순서. |
| P3-3 | 치수 Flip Direction, Edit Sketch Coordinate System, 패턴 편집 메뉴 | BE+FE | 우클릭 항목이 동작. |
| P3-4 | 응용 옵션 Sketch 탭 UI 전체(HUD 설정, 투영 구성선, 불투명도 등) | FE | §3.8 표의 항목이 설정 창에 있다. |

### 미확인 목록(도움말에 없어 사용자 확인·실측 필요)

Start 2D Sketch 평면 선택 UI 세부, Look at 기본값, 완전 구속/참조 형상 색 값과 상태바 문구(0일 때), 그리기 명령 우클릭 메뉴의 전체 항목과 "Done [Esc]" 표기, 스냅 픽셀 허용오차, Dimension 명령에서 대상·커서 위치에 따른 치수 종류 결정 규칙, 대부분 명령의 기본 단축키(도움말은 L, C, D, T, TR, CP, RP, S, `-`만 명시), Midpoint 구속의 존재, Centerline 토글, 한글 UI 명칭, Esc의 부분 입력 취소 순서, Undo 단위(선분 외).

## 출처 (Inventor 2026 도움말)

접두 `https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/` (S33은 `.../Inventor-WhatsNew/files/`).

| ID | 문서 | 파일 |
|---|---|---|
| S1 | To Create and Edit Sketches | GUID-E96FB3C4-136B-4551-BF45-F54594543ECB.htm |
| S2 | To Create Lines, Arcs, and Bends | GUID-4F861674-F74C-474E-869A-4C190ED6EC4B.htm |
| S3 | To Create 2D Shape Geometry | GUID-D489CE6D-7299-4211-A43A-F3580A4BA357.htm |
| S4 | To Draw 2D Geometry Using Dynamic Input | GUID-33A8B11D-A04C-4809-BC82-C0419215915F.htm |
| S5 | Heads-Up Display (HUD) Settings Reference | GUID-C75B225D-6257-4C64-A6D8-841FE1FB1F8C.htm |
| S6 | Sketch Tab Reference (Application Options) | GUID-1898C432-CFD2-46F1-8BAE-631392DFBF1D.htm |
| S7 | Sketch Tab Reference (Document Settings) | GUID-58DD9A89-84C7-48F3-8E4E-0303E755B10F.htm |
| S8 | To Work with Sketches (construction, Slice, Text, Image) | GUID-F5A5790A-F748-49F0-A81F-BCA56C7C1E89.htm |
| S9 | To Edit Sketch Geometry (Precise Input, Undo) | GUID-F57563E3-CB1B-4BE5-B7D1-025F4476A02A.htm |
| S10 | To Work with Points | GUID-D4A4288F-32F1-4E23-A382-930EBA152E87.htm |
| S11 | To Create, Lock Spline Length, and Edit Splines | GUID-DB15C479-66F2-456A-9E54-40A60F83DC92.htm |
| S12 | To Create and Edit Sketch Patterns | GUID-AAD3A699-FED8-4614-A31D-0A2316328CEE.htm |
| S13 | To Work with Sketch Profiles (Close) | GUID-4110FA02-0A8A-41F7-85B5-384819DAC528.htm |
| S14 | To Work with Sketch Dimensions | GUID-026FE8D3-AFBB-4834-BC18-73D9CFF78185.htm |
| S15 | To Apply 2D Sketch Dimensions Automatically | GUID-1DC71C6F-9F6D-4846-8E73-9CB7D3452882.htm |
| S16 | Sketch Dimension Reference | GUID-3F4E5140-259E-45F1-AD6B-2D31416E1138.htm |
| S17 | To Copy, Move, or Rotate 2D Sketch Geometry | GUID-F8114270-18F7-4C7A-947E-AC489EDACFA9.htm |
| S18 | To Stretch, Scale, or Offset 2D Sketch Geometry | GUID-962CEE71-8BE4-4DA6-B51E-341319C64FA4.htm |
| S19 | To Split, Trim, or Extend Curves | GUID-DD00037F-6BBA-4B95-AA90-4F99DCEB667D.htm |
| S20 | To Project Geometry into a 2D Sketch | GUID-B8D8B07B-86F2-4CB7-AB21-3131D94AD1CC.htm |
| S21 | About Projecting Sketch Geometry | GUID-B61431DA-FABC-41EE-945C-6E54B4806582.htm |
| S22 | About Sketch Constraints | GUID-FAF69614-E8F2-4763-975C-552E0BEA1DD1.htm |
| S23 | To Specify Geometric Constraint Default Behavior | GUID-653A9867-4F4F-48AE-B5DE-9DED76BD6DF2.htm |
| S24 | To Work with Geometric Constraints (show/hide, Relax) | GUID-B06FA7B7-5CD5-4D58-B1C5-0C5ABAFFD84E.htm |
| S25 | To Apply Geometric Constraints | GUID-A85A7A30-7D81-4C75-8769-CAD034EEA930.htm |
| S26 | Degrees of Freedom Glyph Reference | GUID-784929B0-3480-4FC0-8ED5-E3414248FB7F.htm |
| S27 | About Part Sketching (Line 자동 활성, 치수 색) | GUID-97C8CDC5-AAAD-4A6F-BB84-24DB5C90C7A3.htm |
| S28 | Predefined Command Alias and Shortcut Reference | GUID-428B9A8E-E513-44C8-888A-F6048AE89F8E.htm |
| S29 | Keyboard Shortcut Reference | GUID-DEEB0723-DFC7-4B28-95F4-099CC1E1EBF8.htm |
| S30 | To Select and Deselect Objects | GUID-B8F6E805-2E4B-4366-8FF6-33F0ED1C1C4A.htm |
| S31 | Realign sketch coordinate system | GUID-A3346785-37D7-4E52-B604-02C4F3AEBBE6.htm |
| S32 | Set sketch grid | GUID-D82007B4-4244-4319-9C3C-DBB76D3208F0.htm |
| S33 | What's New: 2026.1 `INV2026-1-SKETCH.htm`, 2026.2 `INV2026-2-SKETCH.htm`, 2025.2 `NEW-SKETCH-2025-2.htm`, 2025.1 `GUID-8B5B5012-90F9-47C0-AF7D-FA994C9F6B6D.htm`, 2024.2 `GUID-8111280A-9158-489F-90E8-19D7B214B7BB.htm`, 2024.1 `GUID-72134CCA-5594-4C1C-97C1-5DE6A1DC241E.htm`, 2023.1 `GUID-C302FB2E-DE01-4697-8D5F-8936CD369BC8.htm`, 2023.2 `GUID-1E4F8747-7C13-4544-A389-339EECF2C9E5.htm` | |
| S34 | To Convert Sketch Text to Sketch Geometry | GUID-99BC5307-460F-4EC7-8413-86874BA8E562.htm |
| S35 | Dynamic Input (개요) | GUID-5DC9930A-7A1F-4260-83E1-323858856056.htm |
