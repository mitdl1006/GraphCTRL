# Inventor 2026 어셈블리(.iam) 환경 동등성 명세 — GraphCTRL 문서(어셈블리)와의 차이

작성 2026-11-17. 근거: Autodesk Inventor 2026 공식 도움말(`help.autodesk.com`, 영문 ENU)을 이 세션에서 `curl`로 내려받아 읽었다. 인용은 `[A#]`이며 URL은 문서 끝 "출처" 표에 있다. 2026 페이지가 있는 것은 2026을 읽었고, 도움말 텍스트에 없는 것은 **(미확인)** 으로 표시했다. **도움말에 없는 동작은 추측하지 않았다.** 이 문서는 도움말 텍스트 기준이며 실기 검증이 아니다. GraphCTRL 쪽 사실은 `docs/PLAN.md` 0·9, `docs/STATUS.md`, `crates/domain/gc_doc/src/{cell,document}.rs`, `crates/ui/gc_panels/src/lib.rs`에서 읽었다. 한글 명령명은 ENU만 읽었으므로 관행 번역이며 공식 한글 UI 명칭은 (미확인).

관련 문서: 셸 전반(리본·마킹 메뉴·ViewCube·선택·상태바)은 `inventor-parity.md`, 2D 스케치 명령은 `inventor-sketch-parity.md`에 있다. 이 문서는 **어셈블리 수준**(컴포넌트·원점·구속/조인트·어셈블리 스케치·단면·측정·간섭·작업 피처)만 다루고 위 두 문서와 겹치는 항목은 "→ 해당 문서"로 넘긴다.

## 1. 범위

- **GraphCTRL의 "어셈블리"**: 문서(셀)가 곧 어셈블리이고 컴포넌트는 불러온 설비 부품(GLB/STEP/STL), UR 로봇(URDF, 링크·툴 플랜지·TCP), 툴(그리퍼·드라이버), 필드/구역이다(사용자 지정). **파트 편집(피처 모델링)은 하지 않는다**(`inventor-parity.md` §1과 동일).
- **"동일한 경험"의 판정**: 같은 입력(클릭·드래그·키·우클릭)에 같은 결과(어떤 구속·위치가 생기는가), 같은 피드백(글리프, 브라우저 아이콘, 색, 미니 툴바). 도움말이 규정한 것만 "동일"의 대상이다. 도움말에 없는 수치·색 코드·픽셀 허용오차는 GraphCTRL 값을 유지하고 (미확인)으로 둔다.
- **복제 제외**: iMate, iAssembly/iPart, Content Center, Design Accelerator, Frame/Tube&Pipe/Weldment, Dynamic Simulation, Inventor Studio, 적응성(Adaptive), Make Components/Layout, BOM·도면. **Autodesk 상표·아이콘·색 팔레트 자산은 쓰지 않는다**(아이콘은 직접 그린다).
- **GraphCTRL 어댑테이션**(로봇·툴·필드)은 §4에 모았고 **인벤터 사실이 아니다**.
- **현재 상태의 한계**: `docs/handoff/backend.md`(2026-11-17)에는 Assembly/WorkFeature/Feature/Timeline 엔티티 설명이 없다(모션 계획·ACM·보증만 있음). 그래서 §5의 엔티티 이름은 **제안**이며 백엔드 확정 전까지 가칭이다. 코드(`gc_doc`)에도 해당 엔티티가 없다(`Kind`: Param, Frame, Sketch…, Body, RobotInstance, Tool, Waypoint, Path…).
- 우선순위: P1 = 인벤터 사용자가 어셈블리 첫 10분에 막히는 것, P2 = 다음, P3 = 이후. 담당: FE = `crates/ui`·`render`·`gc_app`·`gc_panels`, BE = `crates/domain`·`plugins`.

## 2. GraphCTRL 현재 요약 (비교의 기준)

> 갱신(2026-11-18): 이 문서의 "GraphCTRL 현재" 열은 `docs/handoff/backend.md`·`frontend.md`와 코드(i18n 문자열, 리본, 대화상자 모듈)를 기준으로 고쳤다. 확인하지 못한 것은 "확인 중"으로 표시했다. 이전 요약의 "없음"은 이 날짜 이전 상태다.

- **문서 구조**: 평평한 셀. `Body`(이름·`Placement{xyz, rpy}`·`Solid`·`Look`), `Robot`(`placement`, `tcp`, `joints`, `tool: Option<Tool>`; 툴은 팁 링크 프레임 안의 `placement`), `Frame`(`FrameKind::{World, OriginPlane, User}`, 부모 프레임 기준 포즈), 작업 좌표계, 웨이포인트, 경로. **컴포넌트 계층·서브어셈블리·패턴·관계(구속/조인트)·표현(representation)은 없다.**
- **원점**: 브라우저 "원점" 그룹에 **문서 전체 1세트**(원점 평면 3개)만 있다(`FrameKind::OriginPlane`). 축·중심점은 없고, 컴포넌트별 원점 폴더는 없다. 스케치는 XY/XZ/YZ 버튼(`create_sketch(plane)`)과 오프셋으로 만든다(PLAN 9.1).
- **표시**: 바디·로봇에 가시성 체크(`Look.hidden`, 숨겨도 장애물로 남음), 투명도(0~0.95). 억제·활성(enabled) 개념 없음.
- **배치/이동**: 바디·로봇·웨이포인트·좌표계에 이동/회전 기즈모. 로봇 베이스·바디 배치 패널(숫자 입력). 로봇 TCP는 팁 링크 기준 `Placement`, "TCP를 툴 끝으로" 있음.
- **측정·간섭**: 스케치 치수 측정만(`sketch.measure`). 3D 측정 없음. 충돌 엔진 `gc_collide`(파트 쌍당 `closest_points`, 겹침 판정)는 있으나 "간섭 분석" 화면은 없음.
- **단면·슬라이스**: 없음(`inventor-parity.md` P3에 슬라이스 F7만 항목).
- **불러오기**: STL/COLLADA 읽기 완료, GLB 계층 불러오기·STEP은 PLAN 9.1(P1 목록 14번)에서 예정.
- **관계**: 어셈블리 구속·조인트·DOF 표시 없음. 로봇 마운트는 숫자 배치(+기즈모)뿐.

## 3. 영역별 비교

### 3.1 어셈블리 브라우저 (컴포넌트·원점 폴더·관계·표현)

| 항목 | 인벤터 2026 동작 (출처) | GraphCTRL 현재 | 차이 | 우선 | 담당 |
|---|---|---|---|---|---|
| 컴포넌트 노드 | 컴포넌트(부품·서브어셈블리)가 occurrence로 나열. 부품/어셈블리/다중바디/표면/가상/표준 부품 아이콘, **고정(Grounded)·비활성(Disabled)·비가시(Invisible)** 변형 아이콘 [A1]. 고정 컴포넌트는 브라우저에 압정(pushpin) 아이콘 [A17] | 구현: 어셈블리 트리(`session.component_tree()`), 서브어셈블리 폴더, 컴포넌트 행(이름·표시·활성·접지·억제) | 계층·occurrence·고정 아이콘 없음 | P1 | FE+BE |
| **컴포넌트 Origin 폴더** | 모든 파트·어셈블리 파일에는 숨겨진 기준 평면(work plane)이 원점에서 교차하고, **작업 평면과 중심점(center point)이 브라우저의 Origin 아이콘 아래에 나열**된다 [A52]. 아이콘은 "Origin Plane", "Origin Point" [A55]. 브라우저 메뉴 "Hide Work Features"는 **모든 작업 피처와 Origin 폴더**의 표시를 끈다 [A2]. 폴더 안 항목명 `YZ/XZ/XY Plane`·`X/Y/Z Axis`·`Center Point`와 축 3개의 존재는 읽은 2026 페이지에 명시 없음 (미확인, 사용자 서술 기준) | 구현: 컴포넌트마다 원점 폴더(평면 3·축 3·중심점), 항목별 표시·선택, 평면에서 스케치 만들기 | 컴포넌트마다 Origin 폴더(평면 3 + 축 3 + 중심점) 필요 | P1 | BE(데이터)+FE(트리) |
| 원점 표시·크기 | 기본 숨김. 브라우저에서 Origin 옆 `+` 펼침 → 평면 선택 → 우클릭 **Visibility** [A53]. 표시되면 와이어프레임·셰이디드 모두에서 보임. 평면 모서리/가장자리에 커서를 올리면 크기 조절(Resize)·이동(Move) 기호 → 드래그 [A52,A53] | 구현: 원점 폴더 항목별 표시(개체 가시성의 원점 항목 포함) | 컴포넌트별 표시 토글, 평면 크기 조절 | P1 | FE |
| 원점 평면 선택 | 표시된 원점 평면을 **그래픽 창에서 선택**하고 스케치 명령으로 스케치 평면으로 쓸 수 있다 [A52,A53]. 브라우저 항목 선택도 가능 [A52]. 숨김 상태 평면의 그래픽 창 피킹 가능 여부 (미확인) | 구현: 뷰포트에서 원점 평면 클릭 선택, 우클릭 메뉴의 스케치 만들기 | 뷰포트에서 평면 클릭 선택 없음 | P1 | FE |
| 원점의 활용 | ① 스케치 평면 [A52,A11] ② 구속·조인트 입력("point, curve, face, or work feature" 선택) [A19,A28]; 컴포넌트 Origin 요소가 구속 선택 대상인지는 읽은 페이지에 명시 없음 (미확인) ③ 작업 피처 기준 [A49] ④ 스케치에 중심점 투영(Project Geometry) [A52,A53] ⑤ 컴포넌트 패턴 방향에 "origin axis" 선택 [A59] ⑥ 첫 컴포넌트의 원점·좌표축이 어셈블리 것과 정렬되어 자동 고정 [A11] | 구현: 스케치 평면, 구속·조인트 입력, 측정, 작업 피처 기준(구성요소 원점 기준) | ②③④⑤ 없음 | P1/P2 | BE+FE |
| 어셈블리 자체 Origin | 어셈블리 스케치는 **최상위 어셈블리의 Origin 폴더 아래**에 중첩, 피처에 소비되면 피처 아래로 이동 [A42]. Modeling View에서는 각 어셈블리의 구속이 어셈블리 origin 폴더 아래에 모인다 [A26] | 구현: 어셈블리 스케치의 소유자(최상위 어셈블리 원점 폴더 아래) | 폴더 구조(스케치가 원점 아래) 다름 | P2 | FE |
| 이름 바꾸기 | 노드 선택 후 **한 번 더 클릭**(더블클릭 아님)하면 편집, 전체 이름 선택 [A3]. **Rename Browser Nodes**(어셈블리 탭 Productivity): Filename / Part Number / Default 3가지 규칙을 모든 컴포넌트에 적용 [A4]. 불러온 부품 이름은 파일명(타 CAD는 part number 속성) [A8] | 구현: 브라우저 행에서 이름 바꾸기, 매개변수 이름은 표에서 Enter | 인라인 이름 바꾸기 → `inventor-parity.md` P1-3. 일괄 규칙은 없음 | P2 | FE |
| 가시성 | 컴포넌트 우클릭 **Visibility**. 보이지 않는 컴포넌트는 그래픽에서 선택 불가, 브라우저에서 선택 후 우클릭으로 변경, 브라우저 노드에 커서를 올리면 **경계 상자**가 그래픽에 표시. 더블클릭 편집하면 자동으로 보임 [A56,A57]. Alt+V = 선택한 솔리드·작업 피처 가시성 토글 → `inventor-parity.md` | 구현: 바디·로봇·컴포넌트 표시 체크, 숨긴 것도 충돌 장애물 유지(계획) | 경계 상자 힌트, 계층 단위(서브어셈블리) 토글 | P1 | FE |
| 활성(Enabled) | Enabled를 끄면 **보이지만 선택 불가**, 배경 스타일(기본 반투명)로 표시, 어셈블리 계산이 빨라짐 [A56,A57]. Visible+enabled / Invisible / Visible but not enabled 3상태 [A57] | 구현: 활성 체크(끄면 계산에서 빠짐) | 신규 상태. 큰 설비를 "배경"으로 두는 용도로 유용 | P2 | FE+BE |
| 억제(Suppress) | 패턴 요소·관계는 우클릭 **Suppress**: 억제 요소는 그래픽에 없고 갱신 때 계산되지 않으며 BOM에서 빠진다. 브라우저에 음영 기호+취소선 [A60]. 모델 상태에서 억제된 컴포넌트는 BOM 제외 [A65]. 컴포넌트 우클릭 메뉴의 정확한 문구 (미확인) | 구현: 바디·로봇·패턴 요소·관계 억제(해제 가능) | 억제 상태·평가 제외 | P2 | BE+FE |
| 서브어셈블리 | 서브어셈블리는 상위에서 한 단위로 취급. 활성화(더블클릭)하면 나머지는 음영 처리, **한 번에 하나만 활성** [A11,A66]. 컴포넌트를 선택해 **Demote**로 새 서브어셈블리로 묶기, 브라우저 드래그로 레벨 이동 [A66]. 활성 컴포넌트 편집은 더블클릭, 끝내면 Return / Return to Parent / Return to Top [A67] | 구현: 묶기(강등)·이동·해체·제자리 편집·복귀, 통째 이동, Flexible | 계층·활성화 모드 | P2 | BE+FE |
| 패턴 | Circular/Rectangular 컴포넌트 패턴, 요소는 패턴 아이콘 아래 나열. 요소 가시성·억제·독립화(independent), 수·간격 지정, 직사각형은 **원점 축 또는 모서리·작업 축·면(평면은 법선, 원통/회전면은 축)**으로 방향 [A58,A59,A60,A61]. 부품 피처 패턴과 연동(associative) [A58] | 구현: 컴포넌트 패턴(직사각형·원형), 요소 가시성·억제 — 독립화는 예정 | 신규 | P3 | BE+FE |
| Relationships 폴더 | **구속과 조인트를 순서대로 나열**. 선택하면 값(오프셋·각도)을 편집 상자에서 즉시 변경·측정(메뉴 "Edit Values in Browser" 필요) [A2]. 구속 유형별 아이콘(Mate/Flush/Angle/Insert/Tangent, 회전·병진 등) [A1]. 한계 있는 관계는 **+/-** 기호 [A30]. 구속 상태 점: 검은 점=완전 구속, 속 빈 점=미구속, 대시=알 수 없음 [A2,A27]. "Hide Fully Constrained"(문제 있는 것만 표시)·"Show Constraint State" [A2,A27]. 구속 클릭하면 관련 컴포넌트 강조(두 참가자 색 다름) [A26] | 구현: 관계 폴더(구속·조인트 나열, 삭제, 잠금·보호 체크) | 폴더·상태 점 전부 신규 | P1(구속 도입 시) | FE+BE |
| 표현(Representations) | **View(Design View)**: 가시성·작업 피처 표시·외관·확대 상태 저장. **Positional**: View 내용 + 서브어셈블리 유연성·활성 상태·오프셋·관계 억제까지 저장, 이름 `Primary`는 편집 불가·모든 새 표현의 기반 [A63]. **Model State**(Level of Detail의 후신): 한 파일 안 여러 크기·단계·단순화 상태, `Primary` 이름 변경·삭제 불가 [A65]. 브라우저 Representations 폴더 [A1,A2] | 구현: 뷰 표현·위치 표현(Primary, 복사로 새 위치 표현), 활성화 | 저장된 "뷰 상태" 개념 없음 | P2(View·Positional), P3(Model State) | BE+FE |
| 격리(Isolate) | 선택 컴포넌트 외 모두 Invisible, 관계에서 "Isolate Components"로 두 참가자만 표시, **Undo Isolate**로 복원(저장 전 반드시 복원) [A37] | 구현: Isolate / Undo Isolate(선택 외 숨김, 두 참가자만 표시) | 신규(구속 도입 후 유용) | P2 | FE |
| 선택 집합 | 크기·위치·"Constrained To" 등으로 컴포넌트 집합 선택, Isolate와 연계 [A68] | 미구현 | 신규 | P3 | FE |

### 3.2 컴포넌트 배치·이동·고정

| 항목 | 인벤터 2026 동작 (출처) | GraphCTRL 현재 | 차이 | 우선 | 담당 |
|---|---|---|---|---|---|
| Place | 어셈블리 탭 Component 패널 Place → 파일 선택 → 커서에 붙은 컴포넌트를 **클릭할 때마다 인스턴스 배치**, 우클릭 **OK**로 종료. 배치 전 우클릭으로 X/Y/Z 회전 방향 지정 또는 **Place Grounded at Origin** [A8]. 복사·붙여넣기로 같은 방향의 새 인스턴스 [A8] | 구현: 배치 명령, 커서에 매달아 연속 배치(STL 본체), 첫 구성요소 원점 고정 | 배치 중 커서 부착·연속 배치·축 회전·"원점에 고정 배치" 없음 | P1 | FE |
| 첫 컴포넌트 | **처음 배치한 컴포넌트는 자동 고정**, 원점·좌표축이 어셈블리 것과 정렬. 나머지는 미구속·미고정 [A11]. 신규(구 판 설명) 문서는 "구 릴리스에서는 첫 부품이 고정이었다"며 우클릭 Place Grounded at Origin을 안내 [A18] — 2026에서 자동 고정이 유지되는지 두 페이지가 어긋남 (미확인) | 구현: 첫 구성요소 자동 고정(원점·축 정렬) | 고정 상태 도입 | P1 | BE |
| Ground / Unground | 컴포넌트 우클릭 **Grounded** 체크. 고정 = 모든 DOF 제거·공간에 위치 고정. 해제 후 드래그 → 다시 Grounded [A10,A17]. 정밀 이동: Unground → iProperties > Occurrence 탭 **Current Offset from Parent Assembly Origin**에 x,y,z 입력 [A10]. 고정 컴포넌트에 구속된 것은 함께 이동 [A17] | 구현: 접지 토글(조립 탭 위치 그룹), 접지 해제 | 고정 상태·오프셋 편집 | P1 | BE+FE |
| Ground and Root | 어셈블리 탭 Productivity: **Ground at origin**(이동·고정·정렬), **Create origin flush constraints**(원점에 Flush 구속으로 뿌리내리기), **Reposition to the top of the browser** [A5]. 기존 구속과 충돌하면 빨간 Recover 표시 [A5] | 백엔드만 구현, 화면 연결 예정(프론트엔드 배정) | 신규 | P2 | BE |
| Place at Component Origin | 선택한 기존 컴포넌트의 원점에 새 컴포넌트를 정렬·일치시켜 배치 [A6] | 백엔드만 구현, 화면 연결 예정(프론트엔드 배정) | 신규. 로봇·툴·고정구를 "다른 컴포넌트 원점에" 놓을 때 유용 | P2 | BE+FE |
| Free Move | Position 패널. 컴포넌트를 **뷰 평면에서 임의 선형 방향으로 드래그**, 관계는 **고무줄**로, 관계 글리프·충돌 오류 글리프 표시. 관계 없는 컴포넌트는 그 자리에 남고, 자유도가 남은 것은 관계에 맞게 조정, **Update 시 관계 위치로 복귀** [A12]. 관계를 무시하려면 브라우저에서 관계 Suppress [A12] | 구현: 기즈모 이동, 자유 이동(마킹 메뉴 컴포넌트 이동, 구속 기준 Update 복귀 제외) | 뷰 평면 자유 이동, 관계 복귀·고무줄 | P1(자유 이동), P2(고무줄) | FE |
| Free Rotate | 3D 회전 기호: **안쪽 클릭=임의 방향, 위/아래 손잡이=수평축, 좌/우 손잡이=수직축, 테두리=화면 평면 회전**. Update 시 구속 위치로 복귀(고정·무관계 컴포넌트는 새 위치 유지) [A13] | 구현: 기즈모 회전, 자유 회전(마킹 메뉴 컴포넌트 회전) | 위 손잡이 모델 없음 | P1 | FE |
| Grip Snap(3D 그립) | 모서리를 클릭해 기준 → 이동 옵션 바에서 **"Move along ray using reference geometry" / "Rotate about line using reference geometry"** → 점 2개로 오프셋·각도 지정. HUD가 남은 DOF 안내, 임시 구속은 **녹색**. 끝날 때 우클릭 Done, 또는 **Commit and Ground** [A14] | 구현: 그립 스냅(모서리·꼭짓점·평면 기준, 축·점·면·법선 동작, 남은 자유도 검사) | 신규 | P2 | FE+BE |
| ALT+드래그 | ALT를 누른 채 드래그하면 대상 면·모서리에 따라 **Mate/Flush(평면·직선·축), Tangent(원통면), Insert(원형 모서리)**를 미리보기하고 놓으면 오프셋 0 구속 생성, 스페이스로 옵션 순환, 드래그 중 ALT를 떼고 M/1, A/2, T/3, I/4, R/5, S/6, X/8로 유형 변경 [A19,A25] | 구현: Alt + 드래그로 맞대기 구속 | §3.3 | P2 | FE |
| 자유도(DOF) 표시 | 컴포넌트는 처음에 6 DOF. **DOF 기호**: iProperties > Occurrence 탭 "Degrees of Freedom" 체크, 또는 활성 어셈블리에서 View 탭 > Visibility 패널 > Degrees of Freedom [A15,A16]. 미구속 부품에 상대 구속된 컴포넌트에는 **큐브**가 붙고 커서를 올리면 색이 변해 상대 부품 강조 [A16]. 기호의 모양(화살표 등) 자체는 (미확인) | 구현: 뷰 탭 가시성의 자유도 버튼(DOF 기호) | 신규 | P1(구속 도입 시) | FE+BE |

### 3.3 어셈블리 구속·조인트

| 항목 | 인벤터 2026 동작 (출처) | GraphCTRL 현재 | 차이 | 우선 | 담당 |
|---|---|---|---|---|---|
| 개요 | Constrain/Assemble은 "legacy" 방식으로 DOF를 점차 제거. **Joint는 위치와 운동을 한 번에 정의**해 구속 수가 적다. 둘을 함께 사용 가능 [A18]. 갱신 때마다 구속이 강제됨 [A18] | 구현: 구속 대화상자, 관계 폴더, 구속 상태, 조인트(5종)·한계·잠금·보호, 구동 — 세부는 아래 행 | 구속 솔버 신규(`gc_solver`는 2D 스케치용) | P1 | BE |
| Constrain 명령 | 어셈블리 탭 Relationships > Constrain. 대화상자 탭: Assembly / Motion / Transitional / Constraint Set [A29]. 입력: **First Selection, Second Selection**(색 막대가 선택 형상 색과 대응), **Pick Part First**(한 컴포넌트로 선택 제한), Offset/Angle, **Show Preview**(기본 켬), **Predict Offset and Orientation**(기본 켬), Suppress 체크, More(이름·한계) [A28] | 구현: 구속 대화상자(맞춤·플러시·삽입·각도·접선·대칭), 미니 툴바 흐름은 예정 | 신규 | P1 | FE+BE |
| Mate | 면 대 면(법선 반대) 또는 곡선·평면·모서리·점 일치. 평면에서 직선 이동 1 + 회전 2 제거 [A19,A28]. 축 2개: **Opposed(기본)/Aligned/Undirected** [A19]. 두 축 사이는 최소 회전 해를 미리 선택 [A19] | 구현: 맞춤(반대·정렬·방향 없음, 간격띄우기, 예측) | | P1 | BE |
| Flush (Mate의 솔루션) | 면·평면을 **법선이 같은 방향**으로 정렬 [A28]. 대화상자 Solutions에서 Mate/Flush 전환 [A19] | 구현: 플러시 | | P1 | BE |
| Predict offset | 오프셋 상자가 비어 있으면: 법선이 같은 방향이면 Flush로 추론·현재 거리 측정, 법선이 반대면 Mate로 추론, Angle은 현재 각 측정 [A28]. 편집 시에는 사용 불가("only available when creating") [A26] | 구현: 오프셋 및 방향 예측 | | P1 | BE |
| Offset 부호·측정 | 첫 번째로 고른 컴포넌트가 양의 방향을 정함, 음수로 방향 반전. 입력 상자의 화살표 메뉴로 거리·각도 **Measure**, 컴포넌트 치수 표시, 최근 값 [A28] | 구현: 간격띄우기 값 mm, 예측 값 표시 | | P1 | FE |
| Angle | 모서리·평면 사이 각. **Directed**(우수 규칙, 한계 불가), **Undirected**(방향 뒤집힘 해소, 한계 가능), **Explicit Reference Vector**(세 번째 선택으로 Z축 지정, 기본 해) [A20,A28]. 한계: Undirected ±180°, Explicit ±360° [A20] | 구현: 각도 구속(3해 — 직접·무방향·기준 벡터) | | P1 | BE |
| Tangent | 면·평면·원통·구·원뿔·ruled spline 접함. **Inside/Outside**(기본 Outside). 평면-원통은 병진 1 + 회전 1 제거 [A21,A28] | 구현: 접선 구속(면·평면·원통·구, 안·밖) | | P2 | BE |
| Insert | 원형 모서리 2개: **평면 Mate + 축 Mate** 한 번에, 회전 DOF는 열림. **Opposed/Aligned**, **Lock Rotation** 옵션(자물쇠 아이콘, 기본은 대화상자 닫으면 표시 안 됨, Show 명령으로 다시 표시) [A22,A28] | 구현: 삽입(반대·정렬) | | P1 | BE |
| Symmetry | 대칭 평면(작업 평면·면)을 기준으로 두 객체를 대칭 배치. 선택 순서: 객체1, 객체2, 대칭 평면 [A23] | 구현: 대칭 구속 | | P3 | BE |
| 선택 가능한 형상 | **점(중점 포함), 곡선, 면, 작업 피처**(작업 평면·축·점) [A19,A28]. 다른 컴포넌트가 가리면 선택 시 Visibility 잠시 끄기·**Pick Part First**·**Select Other** [A19,A21]. 원점 평면·축을 구속 선택으로 쓰는 것 자체는 Common Origin Modeling이 "assembly origin과 Predict Offset" 방식으로 언급 [A7] | 구현: 원점 요소, 메시 면·모서리 선택 중 일부(작업 피처 포함) | | P1 | FE |
| Assemble 명령 | **형상을 먼저, 유형은 나중**: 이동할 컴포넌트에서 선택 → 고정 컴포넌트에서 선택 → 오프셋/각도 → 솔루션 유형 → Apply/OK/Undo. 첫 컴포넌트가 두 번째로 **이동**하고(이미 완전 구속이거나 고정이어도 완화), 형상에 따라 가능한 유형만 목록에 표시(평면만 선택→Mate-Mate/Mate-Flush/Angle-Directed, 원형 모서리→Insert-Opposed/Aligned). 한 번에 컴포넌트 하나. **OK로 끝낼 때 새 구속을 기존과 비교**하고 충돌하면 Assemble Constraint Management 대화상자. 미지원: Motion·Translation·Angle-Undirected·Explicit Reference Vector, 이름·한계 [A18,A24,A29] | 구현: 어셈블 창(첫·두 번째 원점 → 가능한 유형) | 신규 | P1 | FE+BE |
| Show / 구속 표시 | Relationships 패널 **Show**: 선택 컴포넌트의 관계 **글리프** 표시(위에 멈추면 상세, 우클릭 메뉴), **Show Sick**: 문제 관계만, **Hide All**: 글리프 전부 제거 [A33]. Insert 구속 Lock 아이콘도 Show로 재표시 [A22] | 구현: 구속조건 기호(개체 가시성), 관계 행 | | P1 | FE |
| 구속 편집·이름·한계 | 브라우저에서 구속 우클릭 **Edit**(유형·오프셋·각·선택·방향). 오프셋/각만 바꾸려면 브라우저 편집 상자에 값 입력 후 Enter. Positional 표현에서는 **Modify (Override)** [A26]. **Name** 입력(비우면 기본 이름), **Maximum/Minimum/Use Offset As Resting Position** [A19,A28]. 한계 값은 체크를 해제해도 보존 [A19] | 구현: 이름, 더 보기(최대·최소·휴지 위치) | | P1(편집), P2(한계) | FE+BE |
| Drive 구속 | 브라우저에서 관계 우클릭 **Drive**: Start(기본=현재 값), End(기본=Start+10), Pause Delay, Increment(값 또는 총 단계 수), Repetitions(Start/End 또는 Start/End/Start+횟수), Drive Adaptivity, **Collision Detection**(간섭을 만나면 멈추고 표시), 버튼 Forward/Reverse/Stop/Step/Go to Start/End, Record(AVI/WMV). 한 번에 관계 하나 [A31,A32] | 구현: 관계 구동(로봇 관절 + 구속 값, 시작·끝 범위) | 신규. 로봇 관절/슬라이더 시연에 직결 | P2 | FE+BE |
| 구속 오류 | 해결 불가면 **Place/Edit Connection Error** 대화상자: Edit / Cancel / **Accept the relationship**(오류 아이콘과 함께 생성) / Diagnose [A34]. **Relationship Conflict Analysis**: 충돌 관계를 굵게, 관계별 Keep/Break, 컴포넌트 고정 상태 토글, Check 버튼, "Suppress/Delete broken relationships" [A35]. **Design Doctor**(관계 오류 우클릭 Recover): Edit/Delete/Suppress/Isolate and Edit/Isolate Related Failed/Diagnose [A36]. 오류 관계는 브라우저에 오류 기호 (정확한 아이콘 형태 미확인) | 구현: 구속 오류 대화상자(억제·삭제 후 다시 배치, 취소 시 미생성) | 신규 | P2 | FE+BE |
| 구속 상태 표시 | 위 §3.1 Relationships 행의 상태 점, 문서 설정 "Show Constraint State in Browser" 기본 켬 [A2,A27,A70] | 구현: 구속 상태·남은 자유도(상태 줄) | | P1 | FE |
| Suppress 체크 | 2023부터 Place/Edit Constraint 대화상자에서 **Suppress** 체크로 바로 억제(미리보기 꺼짐) [A28,A71] | 구현: 관계 억제 체크 | | P2 | FE+BE |
| **Joint** 명령 | Relationships > Joint (단축키 **J**, 2024에서 추가) [A30,A69]. 흐름: 이동 컴포넌트의 **원점(origin)** 선택 → 고정 컴포넌트의 원점 선택 → 유형 필요 시 변경 → **Flip component**(양의 방향 반전) → **Align 1/Align 2**(정렬 방향 선택: 평면·점·모서리·작업 평면/축/점) → **Invert alignment** → Apply/OK [A30]. 원점은 형상에서 추론, 안 되면 우클릭 **Between Two Faces**(면 2개+점 1개로 가상 중점) / **Offset Origin**(조작 화살표나 값으로 오프셋) [A30]. 끝점·중점·중심점(슬롯 중심 포함) 선택 가능 [A30]. 첫 컴포넌트가 두 번째로 이동, 필요하면 기존 관계·고정 완화, 미리보기 [A18] | 없음 | 신규. **로봇·툴 마운트의 핵심 명령** | P1 | FE+BE |
| Joint 유형 | **Automatic(기본)**: 선택한 두 원점이 원형이면 **Rotational**, 원통 위 점이면 **Cylindrical**, 구 위 점이면 **Ball**, 그 외 **Rigid**. **Slider는 선택 규칙으로 지정 불가**(유형을 직접 선택) [A30]. **Rigid**: 모든 DOF 제거(용접·볼트 연결), 가장 흔함 [A30,A18]. **Planar**: 읽은 2026 페이지에 언급 없음 (미확인, 사용자 요청 항목). 유형별 DOF 수(예: Rotational=회전 1)는 도움말 텍스트에 표로 없음 (미확인) | 구현: 5종(회전·슬라이더·원통·볼·강체) + 자동 | 6종 유형 | P1(Rigid·Rotational), P2(Slider·Cylindrical), P3(Planar·Ball) | BE |
| Joint 한계 | Joint 편집 → **Limits 탭**(미니 툴바가 없으면 점 3개로 표시). 유형에 따라 **Angular/Linear** 선택(Rotational=Angular만), **Start/Current/End** 값, **Rigid에는 한계 없음**. 한계 있는 관계는 브라우저에 **+/-** 기호 [A30]. Joint의 resting position은 "Current"이며 고정 위치가 아님 [A18] | 구현: 조인트 한계 탭(각도·선형 시작·끝·현재) | | P2 | BE+FE |
| Lock / Protect | 브라우저에서 조인트 우클릭 **Lock**(현재 위치 유지: 모든 운동 제거하되 관련 컴포넌트가 움직이면 따라 이동. **고정과 다름**), **Protect**(추가 관계가 필요한 DOF를 침해하면 경고). **구속에는 없음** [A30,A18] | 구현: 잠금·보호(보호 위반은 거부 — 인벤터는 경고 후 계속) | | P2 | BE |
| Joint 편집 | 브라우저에서 조인트 더블클릭 또는 그래픽의 조인트 글리프 더블클릭. 값 수정은 브라우저 편집 상자 [A30] | 부분: 관계 행에서 값 편집 — 조인트 편집 대화상자는 확인 중 | | P1 | FE |

### 3.4 어셈블리 안 스케치 · Project Cut Edges · Slice Graphics

| 항목 | 인벤터 2026 동작 (출처) | GraphCTRL 현재 | 차이 | 우선 | 담당 |
|---|---|---|---|---|---|
| 스케치 시작 | 3D Model 탭 Start 2D Sketch, 브라우저 우클릭 New Sketch, 또는 **평면 면·작업 평면 클릭 후 미니 툴바 Create Sketch** [A41]. 어셈블리 안 스케치 평면: **어셈블리 원점 평면 / 빈 공간 클릭(현재 카메라 평면) / 기존 컴포넌트 면에 구속** [A11,A66]. 평면 선택 UI 세부는 → `inventor-sketch-parity.md` §2.1 | 구현: 원점 평면·구성요소 면·작업 평면 위 스케치, S 키 | **컴포넌트 면·컴포넌트 원점 평면·작업 평면에 스케치** 없음(PLAN 9.1 P1-13 대응) | P1 | BE+FE |
| 어셈블리 스케치의 성질 | 어셈블리 파일 스케치는 **여러 부품에 영향을 주는 피처(컷·구멍·용접)용**, 다른 컴포넌트에서 형상 투영 가능, 스케치 구속 가능, 처음엔 최상위 어셈블리 Origin 폴더 아래, 다른 어셈블리 피처와 공유 불가 [A42]. 컴포넌트 안(in place)의 스케치는 파트 편집에 속함 [A67] | 구현: 어셈블리 스케치 소유자, 피처 아래 접힘 | 스케치 소유자(어셈블리 vs 컴포넌트) 구분 없음 | P2 | BE |
| Project Geometry | 스케치 평면 설정 후 Sketch 탭 Create 패널 **Project Geometry**: 모서리·꼭짓점·**작업 피처**·루프·곡선을 클릭해 평면에 투영(면 클릭=그 면의 모든 루프). 대상이 안 보이면 회전. **Shift+클릭**은 현재 구성선 토글과 반대로 투영. 종료: 우클릭 Done/Esc/다른 명령 [A38]. **투영은 원본과 연결**(원본이 바뀌면 갱신), 원본 삭제 시 고정 곡선으로 변환, 연결 해제 후 독립 편집 가능, 투영 형상의 크기·위치는 원본 스케치 기준 고정 [A39]. 기본 중심점을 투영해 스케치를 원점에 구속 [A39,A52]. 평면에 수직인 작업 평면/축/점은 치수·구속 시 자동 투영 [A38] | 구현: 형상 투영(연결 투영, 원본 삭제 시 고정 선) | 신규 | P1 | BE+FE |
| 교차 부품 투영 | 어셈블리에서 다른 부품의 모서리·루프·면·스케치를 부품 스케치에 투영하면 **Reference sketch** 기호로 브라우저에 표시되고 적응형(adaptive)이 되어 자동 갱신. **적응성 자체는 복제하지 않음** → 투영은 "연결 유지 투영"으로만 구현 [A43] | 부분: 면 위 스케치의 모서리 자동 투영 구현(옵션), 다른 부품 교차 투영은 확인 중 | 적응성 제외 | P3 | BE |
| **Project Cut Edges** | 2D 스케치에서 면 또는 작업 평면으로 스케치 평면 설정 → Sketch 탭 Create 패널 **Project Cut Edges** → "**단면 평면이 자르는 부품**"을 클릭하면 **평면이 자른 모든 모서리가 스케치 평면에 투영**. 종료는 우클릭 Done/Esc/다른 명령. 복합(composite) 피처와 표면은 교차 곡선 생성, 스케치를 옮기면 갱신 [A38]. 어셈블리에서 여러 부품을 한 번에 고르는지·창 선택·Done 이전 반복 선택은 (미확인) | 없음 | 신규. 설비 윤곽을 평면에서 따서 필드(구역) 프로파일을 그릴 때 핵심 | P1 | BE(메시·평면 교차)+FE |
| **Slice Graphics (F7)** | 스케치 편집 중 상태바 **Slice Graphics** 버튼 또는 **F7**: 시점 쪽을 향한 모델 일부를 **잘라내 스케치 평면을 드러낸다**. 복원: 피처 명령 클릭, Finish Sketch, 버튼 재클릭, F7 재입력 [A40]. 2D 스케치 전용 [A40] | 없음 | 신규 | P1 | FE(+render) |
| Look At / 평면 정렬 | → `inventor-sketch-parity.md` §2.1(Look At on start 옵션) | 구현: 면 보기(탐색 막대) | | | |

### 3.5 섹션 뷰 (View 탭 > Appearance 패널)

| 항목 | 인벤터 2026 동작 (출처) | GraphCTRL 현재 | 차이 | 우선 | 담당 |
|---|---|---|---|---|---|
| 종류 | **Half Section View / Quarter Section View / Three Quarter Section View**, **Delete Section View** [A44]. 도움말에 "End Section"이라는 이름의 명령은 읽은 2026 페이지에 없음 (미확인). 부품·어셈블리·가져온 모델·메시 모두 가능, 지오메트리 수정 없음 [A44] | 구현: 1/2·1/4·3/4 단면, 단면 삭제 | 신규 | P1 | FE(+render) |
| 평면 선택·오프셋 | Half: 임의의 **평면 또는 평면 면**을 절단 평면으로 선택 → 필요하면 우클릭 **Flip** → **드래그하거나 값 입력으로 위치 이동** → **Continue** → 우클릭 OK. Quarter/Three-Quarter: 평면을 **두 개** 순서대로 선택(각각 Flip·이동·Continue) → OK, 종류 변경 가능 [A44]. 원점 평면이 "any plane"에 포함되는지는 평면 선택 일반 규칙상 가능하나 명시는 없음 (미확인) | 구현: 단면 위치(오프셋), 뒤집기 | | P1 | FE |
| 미니 툴바 | **Flip**(절단 평면 법선 반전), **Section Plane control의 Move/Rotate 전환**, **값 상자**(이동=거리, 회전=각도), **Cap Preview** 켜기/끄기 [A44]. 휠 단계 크기: 우클릭 Virtual Movement > Scroll Step Size(이동=선형, 회전=각도) [A44] | 미구현 | | P2 | FE |
| 캡(단면 채움) | 절단된 지오메트리에 **끝 캡(end cap)**을 표시, 기본은 **부품 수 ≤ 500일 때 표시, 500 초과면 숨김**, 미니 툴바에서 재정의(드래그 중 미리보기 제어는 2024 개선) [A44,A45]. 캡 질감/색은 Application Options > Color 탭 **Section Solid Texture**(Bitmap Image=BMP)로 지정, 질감 이미지는 부품별로 스케일 [A44]. 캡의 기본 색·해칭 모양은 (미확인) | 구현: 단면 캡 | 캡 렌더(클리핑 + 단면 채움) 필요 | P1 | FE(render) |
| 컴포넌트별 참여 | **Content Center 부품은 기본적으로 단면에서 제외**, Application Options > Assembly 탭 **Section All Parts**로 포함 [A44]. 일반 컴포넌트를 개별로 제외하는 UI는 읽은 페이지에 없음 (미확인) | 부분: 단면 참여는 확인 중 | 개별 제외는 GraphCTRL 확장 후보(§4) | P2 | FE |
| 저장·편집 | 단면 정의는 **View(Design View) 표현에 저장**, 표현 노드 우클릭 **Section View > Edit / Suppress** [A44,A45] | 구현: 단면 저장 — 편집은 확인 중 | View 표현 도입 필요(§3.1) | P2 | BE+FE |

### 3.6 측정(Measure)·검사(Inspect/Interference)

| 항목 | 인벤터 2026 동작 (출처) | GraphCTRL 현재 | 차이 | 우선 | 담당 |
|---|---|---|---|---|---|
| Measure 패널 | Tools 탭 Measure 패널 **Measure**(마킹 메뉴에도 있음). 어셈블리에서 **선택 우선순위 Component / Part / Edges and Faces**(기본 Faces and Edges), 이중 단위·정밀도, UCS 선택, **Minimum Distance / Center to Center / Maximum Distance** 전환, 값 복사(Copy/Copy All). 선택 1·2는 **서로 다른 색**으로 유지 강조. 값 클릭하면 그래픽에 해당 측정 표시. 빈 곳 클릭 = Restart, Done = 닫기 [A46] | 구현: 측정 창(도구 메뉴, 컴포넌트 우선·모서리 및 면 우선) | 3D 측정 도구 없음 | P1 | FE+BE |
| 측정 종류 | 모서리 길이, 원/원통 지름·반지름, 점 위치(좌표축별), 점 사이 거리(두 번째 점은 첫 점 대비 델타도), **선/축·호/원·스플라인·모든 면 중 임의 2개 사이 거리**, 호 길이, **각도**(선·모서리·면 2개 / 점 2개+Shift 3번째 점 / 호·원 / 선·축과 평면), 면적·루프, 질량(어셈블리에서 부품·컴포넌트 선택 시) [A46] | 구현: 거리(최소·중심 간·최대), 각도, 모서리·면·점 값, 누적 | | P1(거리·각도), P2(면적·질량) | BE |
| 컴포넌트 간 최소 거리 | 컴포넌트 우선순위에서 **컴포넌트 2개를 선택하면 최소 거리** 표시(모서리·면·꼭짓점 중 가장 가까운 곳). 컴포넌트 우선순위에서는 면 선택 불가. 우선순위를 바꾸면 최소 거리 종류가 바뀜 [A46] | 구현: 컴포넌트 쌍 최소 거리(UI 연결) | 컴포넌트 쌍 최소 거리 화면 | P1 | BE(재사용)+FE |
| 값 입력용 측정 | 대화상자 값 상자의 드롭다운 **Measure**로 거리·각도를 그래픽에서 재서 입력(대화상자별로 없을 수 있음) [A46,A28] | 미구현(치수 상자 연동) | 구속 오프셋에도 필요 | P2 | FE |
| Analyze Interference | Inspect 탭 Interference 패널 **Analyze Interference**: **Define Set #1 / Set #2**(컴포넌트·그룹 선택; 한 세트만 주면 세트 내부 검사), **Treat subassemblies as components**, 나사 정밀 분석 옵션. 결과: 간섭 개수·총 체적, 각 항목의 **체적·중심(x,y,z)**, 더블클릭 시 해당 영역으로 줌, **간섭 체적을 임시 솔리드로 표시**. 필터 General/Threads/Non-threaded Content Center/AnyCAD, **Ignore/Un-ignore**(무시 항목은 취소선), 값 복사·인쇄. 오래 걸리면 중단 가능 [A47,A48] | 구현: 간섭 분석(세트 #1·#2, 결과 목록, 무시, 확대, 표시) | 임의 컴포넌트 쌍 분석·결과 목록·간섭 체적 표시 없음 | P2 | BE+FE |
| 접촉·충돌 감지 | Contact Solver: 접촉 집합(contact set)으로 드래그·구동 중 충돌 감지 [A18의 Contact Sets 서술, 별도 페이지 `GUID-4499BD14`/`FA1A8282`는 읽지 않음] | 구현: 접촉 솔버(끔·접촉 세트만·모든 컴포넌트), 접촉 세트(브라우저 우클릭) | 읽지 않은 부분 (미확인) | P3 | BE |

#### 3.6.1 원문과의 차이 — 측정·간섭·구동 (2026-11-17, 원문 재확인)

원문을 curl로 다시 읽어 확인한 항목만 적었다. 근거 GUID는 아래 출처 표 참고.

| 기능 | 원문에서 확인한 것 | GraphCTRL 구현 | 차이 |
|---|---|---|---|
| 측정 위치 | Tools 탭 > Measure 패널 > Measure [A46, GUID-1805AB8F] | 도구 메뉴 > 측정 | 위치가 다름(리본 탭이 아니라 메뉴). 기능 이름은 같음 |
| 측정 모드 전환 | Minimum Distance, Center to Center, **Maximum Distance** 세 가지 전환 [A46] | 최소 거리, 중심 간 거리, 최대 거리(세 가지) | **최대 거리 없음** |
| UCS 선택 | 문서에 UCS가 있으면 측정 기준 UCS를 고를 수 있음 [A46] | 없음 | 차이 |
| 값 복사 | 값 하나 또는 모두 우클릭 > Copy / Copy All [A46] | 모두 복사만 | **값 하나 복사 없음** |
| 다시 시작·완료 | 빈 곳 클릭으로 새 측정, Done으로 패널 닫기 [A46] | 빈 곳 클릭 = 다시 시작, 완료 = 닫기 | 같음 |
| 면적 | 면적·루프 측정 [A46] | 면 면적(mm²) | 루프는 없음 |
| 질량 | 부품·컴포넌트 선택 시 질량 [A46] | 없음 | 차이(P2 계획과 같음) |
| 간섭 필터 | General, Threads, Non-threaded Content Center, AnyCAD 필터와 **Ignored 필터 표시/숨김** [A47] | Ignored 표시 필터만 | 나사·콘텐츠 필터 없음 |
| 간섭 무시 | 우클릭으로 항목 무시, 또는 **지정 체적 이하 모두 무시** [A47] | 무시 / 이하 모두 무시 / 무시 해제 / 모두 해제 | 같음 |
| 간섭 확대 | 목록 항목을 두 번 클릭하면 해당 간섭 영역으로 줌 [A47] | 두 번 클릭 확대 | 같음 |
| 간섭 형상 | 원문 2026 항목은 간섭 결과를 **솔리드로 표시**한다고 요약(A48 요약, 원문 문장은 확인 못 함) | 뷰포트에 **표본점**(빨강) 표시 | 차이. 체적은 24³ 격자 추정(충돌 형상 기준), 원문은 정확한 체적 |
| Contact Solver | Inspect 탭 > Interference 패널 > **Activate Contact Solver** [GUID-FA1A8282] | 구현: 접촉 솔버(끔·접촉 세트만·모든 컴포넌트), 2 mm/1° 단계로 밀림·정지, 드래그와 구동, 한 번의 실행 취소 | 드래그·구동 중 접촉 해석 없음(P3) |
| 구동 범위 | 관계 하나 또는 조인트 하나씩. 여러 관계는 방정식으로 연결 [A31, GUID-B78E59C9] | 구현: 로봇 관절과 관계(구속 값) 구동, 구동 범위(시작·끝) | 일치. 방정식 연결은 없음 |
| 구동 적응성 | **Drive Adaptivity** 체크 — 관계를 유지하도록 컴포넌트를 적응시킴 [A31] | 없음 | 차이 |
| 구동 반복 | Start/End는 한 번 돌고 시작으로 복귀. Start/End/Start는 정·역방향 순서이고 **사이클 수**로 반복 [A31] | 시작/끝, 시작/끝/시작 + 반복 횟수 | 같음(사이클 정의 확인 필요) |
| 구동 기록 | Record(AVI/WMV)는 원문 항목 [A31] | 없음 | 차이(P3) |
| 구동 후 자세 | 원문은 구동 뒤 자세 유지 여부를 이 페이지에서 명시하지 않음(미확인) | 닫으면 원래 자세로 복귀 | 확인 필요. 원문으로 정해질 때까지 현행 유지 |

### 3.7 작업 피처 (3D Model 탭 Work Features 패널)

| 항목 | 인벤터 2026 동작 (출처) | GraphCTRL 현재 | 차이 | 우선 | 담당 |
|---|---|---|---|---|---|
| 정의 | **어셈블리의 작업 피처는 파트가 아니라 어셈블리에 속하며 기존 컴포넌트에 구속**된다. 브라우저 맨 아래에 나열, 만든 컴포넌트에 의존 [A49,A50]. 어셈블리 제약: in-line 생성 불가, 투영 형상 사용 불가, **중점(midpoint) 선택 불가**, 3D Move/Rotate 도구 불가, UCS를 기존 형상에 상대 배치 불가. Selection Priority "Select Features"로 선택 [A50] | 구현: 작업 피처 그룹(작업 평면·축·점), 정의 편집, 관계에서 참조 | 작업 평면·축·점 유형 없음 | P1(평면), P2(축·점) | BE |
| 작업 평면 | Plane, Offset from Plane, Parallel to Plane Through Point, **Midplane Between Two Planes**(두 컴포넌트의 평행한 면 사이도 가능), Midplane of Torus, Angle to Plane Around Edge, Three Points, Two Coplanar Edges, Tangent to Surface Through Edge / and Parallel to Plane, Normal to Axis Through Point. (Tangent Through Point, Normal to Curve at Point는 **Part 전용**) [A49,A50] | 구현: 간격띄우기·3점·평행(점 통과)·중간면·곡선·선에 수직·선과 평면 등 — 대화상자 | Torus·Tangent·Two Coplanar Edges·컴포넌트 면 기준은 없음 | P1(오프셋·중간·3점), P2(나머지) | BE |
| 작업 축 | On Line or Edge, Parallel to Line Through Point, Through Two Points(중점 제외), Intersection of Two Planes, Through Center of Circular or Elliptical Shape, Through Revolved Face or Feature. (Normal to Plane Through Point는 Part 전용) [A50] | 구현: 선 위·평행(점 통과)·두 점·두 평면 교선·원 중심·평면에 수직 | Revolved Face는 없음(회전 면 데이터 없음) | P2 | BE |
| 작업 점 | Point, On Vertex/Sketch Point/Midpoint, Intersection of Three Planes, Intersection of Two Lines, Center Point of Torus/Sphere. (평면/선 교점, 루프 중심은 Part 전용). 어셈블리에서 **Grounded** 작업 점은 브라우저 우클릭 Grounded로 만든다 [A49,A50] | 구현: 점, 두 선 교차, 평면과 선 교차, 세 평면 교차, 원·호·타원 중심 | Torus/Sphere 중심·Grounded 토글은 없음 | P2 | BE |
| 표시 | 브라우저/그래픽에서 선택 → 우클릭 **Visibility** 해제/체크, 또는 View 탭 Visibility 패널 **Object Visibility**(종류 전체). 두 방식이 충돌하면 브라우저 상태가 우선 확인 대상 [A51]. **Show Inputs**(우클릭): 이 작업 피처가 의존하는 평면·면 강조 [A49] | 구현: 개체 가시성의 작업 피처 항목(작업 평면·축·점) | Show Inputs·Object Visibility | P2 | FE |
| 스케치 평면·구속 기준 | 작업 평면은 스케치 평면·치수·구속 대상, 축 기준 [A49]. 스케치 구속에서 작업 축/점은 자동 투영 [A38] | 구현: 원점 평면, 구성요소 면, 작업 평면 위 스케치 | | P1 | BE |

## 4. GraphCTRL 어댑테이션 — 로봇·툴·필드 (**인벤터 사실 아님, 제안**)

| 대상 | 제안(인벤터 동작과의 대응) | 근거 | 우선 | 담당 |
|---|---|---|---|---|
| 로봇 = 컴포넌트 | 로봇은 **서브어셈블리**로 보이게 한다(인벤터의 서브어셈블리 활성화·Origin 폴더 구조를 차용 [A11,A66]). 노드 아래: 자체 Origin 폴더 + 링크 노드(`base_link` … `tip_link`) | 구현: 로봇 링크를 하위 컴포넌트로 | P1 | BE+FE |
| 로봇 베이스 원점 | **Robot Base**는 해당 컴포넌트의 Origin(중심점+평면+축)과 같이 표시·선택된다. UR 티치펜던트의 Base 좌표계와 일치(STATUS: 웨이포인트 기준 좌표계 "로봇 베이스") | 구현: 로봇 베이스 원점 폴더 | P1 | BE+FE |
| 링크별 프레임 | 각 링크가 자기 **링크 프레임**(축 삼색 트라이어드)을 가지며 **토글로 표시**(Origin 가시성 토글과 같은 UX [A53]). 링크 프레임은 Joint의 원점 선택 대상 | 구현: 링크마다 원점 폴더 | P2 | FE |
| 플랜지·TCP | **Tool Flange**(`tip_link` 프레임)와 **TCP**(`Robot.tcp`)를 "원점 점(Origin Point)"처럼 표시·선택 가능한 프레임으로. TCP는 웨이포인트·스케치 투영·측정의 기준점이 됨 | 구현: 플랜지·TCP(툴→플랜지) | P1 | FE+BE |
| 로봇을 설비에 마운트 | ① **Rigid 조인트**(베이스 Origin ↔ 설비의 작업 피처/원점) — 인벤터의 Rigid(모든 DOF 제거) [A30]. ② 고급: **Mate/Flush/Insert**로 베이스 플레이트 면과 설비 상판, 볼트 구멍 [A19,A22]. ③ 현재의 숫자 배치(`Robot.placement`)는 "고정(Grounded) 컴포넌트의 현재 오프셋"으로 해석(인벤터 iProperties Occurrence 오프셋 [A10] 대응) | 구현: 로봇 마운트(설비 부품과 함께 이동) | P1 | BE |
| 로봇 관절 | 6개 회전 관절은 **Rotational 조인트(각도 한계 포함)**로 모델링해 관절 한계 = Joint Limits(Angular) [A30]. 단, 관절값은 조인트 DOF가 아니라 `joints`(운동학)가 관리. 조인트 글리프는 읽기 전용 표시. **구속 솔버가 로봇 관절을 풀지 않는다**(IK/조그가 담당) | 구현: 관절 조인트(회전·슬라이더·원통·볼) | P2 | BE |
| 툴 부착 | 툴은 플랜지에 **Rigid 조인트**(툴 원점 ↔ 플랜지/TCP 프레임) [A30]. 그리퍼 손가락은 **Slider 조인트(+Linear 한계)**로 열림/닫힘 범위 [A30] — 손가락 구동은 Drive 대화상자(Forward/Reverse, Start/End) [A32]로 시연 | 구현: 툴 컴포넌트(툴→플랜지 조인트, 툴 형상·충돌 반영) | P2 | BE+FE |
| 필드·구역 | 필드는 컴포넌트가 아니라 **작업 피처 + 스케치 프로파일(돌출)**로 정의, 브라우저에서 컴포넌트 옆 "필드" 그룹. **Project Cut Edges로 설비 윤곽을 높이별 평면에서 따서** 프로파일에 사용(§3.4) | 미구현(PLAN 9.2) | P1 | BE+FE |
| 단면 + 필드 | 필드 볼륨도 **단면에서 반투명 캡**으로 보여 설비 안 구역을 확인(캡 질감은 필드 종류 색) — 인벤터 단면 캡 [A44]의 확장 | 미구현 | P2 | FE |
| 간섭 분석 | **로봇 링크·툴 ↔ 설비** 쌍을 Set #1/#2로 지정해 분석(`gc_collide` 재사용). 결과는 ACM(허용 충돌 행렬)과 연동 | 구현: 간섭 분석(세트 정의 → 결과) | P2 | BE+FE |
| 컴포넌트 단면 제외 | 단면에서 개별 컴포넌트 제외 토글(인벤터에는 Content Center 외 제외 UI 없음 [A44]) — 로봇은 기본 제외하지 않음 | 확인 중 | P3 | FE |
| Place Grounded at Origin | 설비 GLB의 **기본 배치 = 원점에 고정**(첫 컴포넌트 규칙 [A11]). 로봇은 고정하지 않음 | 구현: 접지 + 원점 맞춤(P2-16, 리드가 화면 연결 확인) | P1 | BE |

## 5. 구현 순서 (우선순위·수용 기준·담당)

수용 기준은 한 줄. P1을 위에서 아래로 순서대로 진행한다(앞 단계가 뒤 단계의 입력).

### P1 (어셈블리 첫 10분)
1. **컴포넌트 계층·고정 상태 모델(BE)**: `Assembly`(가칭) 안에서 컴포넌트가 부모-자식 occurrence를 갖고 `grounded`·`visible`·`enabled`·`suppressed` 플래그를 저장·복원. 수용: 저장→열기 후 계층·플래그 동일, 첫 컴포넌트는 자동 고정(§3.2), 실행 취소 한 단계.
2. **컴포넌트 Origin 폴더(BE+FE)**: 모든 컴포넌트(설비·로봇·툴·서브어셈블리)에 평면 3·축 3·중심점이 생기고 브라우저에 `원점` 폴더로 나열, 기본 숨김, 우클릭 가시성으로 표시. 수용: 컴포넌트 2개 불러온 뒤 각자 Origin을 펼쳐 평면을 켜면 컴포넌트 포즈에 맞춰 보이고 `원점 숨기기` 메뉴로 전체 끔 [A2,A52,A53]. 평면 크기 드래그 조절.
3. **원점 평면·축·점을 뷰포트에서 선택(FE)**: 표시된 원점 평면을 클릭하면 강조(+브라우저 동기화), 우클릭 메뉴에 `스케치 만들기`. 수용: 평면 클릭 → 속성에 소유 컴포넌트·좌표 표시, Esc로 해제 [A52,A53].
4. **컴포넌트 평면·작업 평면 위 스케치(BE+FE)**: 컴포넌트 Origin 평면·메시 평면 면·작업 평면(오프셋/중간)에 2D 스케치를 만들고 컴포넌트를 움직이면 스케치가 따라감. 수용: 설비 컴포넌트 평면 위 스케치를 만들고 설비를 이동해도 스케치가 그 평면에 유지, 저장·재열기 후 참조 유지(PLAN 9.1 P1-13) [A11,A41,A42].
5. **Project Geometry(+작업 피처 투영)(BE+FE)**: 컴포넌트 모서리·꼭짓점·작업 피처·메시 루프를 스케치에 연결 투영. 수용: 원본 컴포넌트 이동 시 투영 곡선이 갱신, 원본 삭제 시 고정 곡선으로 변환, Shift+클릭 구성선 토글 [A38,A39].
6. **Project Cut Edges(BE+FE)**: 스케치 평면이 메시를 자르는 모든 모서리를 스케치에 투영. 수용: 박스 메시 + 평면 스케치에서 4개 모서리 교차선이 생성되고 평면을 옮기면 갱신, 평면과 안 만나면 아무것도 안 만들고 상태바에 안내 [A38]. **상태(리드, 2026-11-17)**: 1차 완료 — 스케치 탭 > 작성 > 절단 모서리 투영: 활성 2D 스케치 평면이 선택한 컴포넌트(없으면 전체)를 자르는 모서리를 고정 선으로 한 번에(undo 1단계) 추가(`gc_mesh::slice`, `SketchOp::ProjectedLines`), 못 자르면 상태바 안내. 원문 GUID-B8D8B07B-86F2-4CB7-AB21-3131D94AD1CC("click a part that a section plane cuts"). 차이: 인벤터는 명령 후 부품을 클릭(여기서는 미리 선택), 평면 이동 시 갱신은 아직 없음(스냅샷), 투영 곡선 전용 표현(참조 색)은 백엔드 Project Geometry 표현이 정해지면 교체.
7. **Slice Graphics F7(FE+render)**: 스케치 편집 중 F7/상태바 버튼으로 시점 쪽 모델을 클립, 같은 키·Finish Sketch·피처 명령으로 복원. 수용: 스케치 평면을 가린 설비를 F7로 잘라 평면이 보이고 Finish에서 자동 복원 [A40].
8. **섹션 뷰 Half/Quarter/Three-Quarter(FE+render)**: 평면·평면 면·원점 평면 선택 → Flip·드래그/값 이동 → Continue → OK, **단면 캡 표시**, Delete Section View. 수용: 설비+UR 장면에서 Half 단면이 보이고 캡이 닫힌 면에 채워지며 부품 수 >500이면 캡 자동 숨김·미니 툴바로 재정의 [A44,A45].
9. **Mate/Flush/Insert 구속 + Predict Offset(BE+FE)**: Constrain 대화상자(First/Second Selection, Pick Part First, Offset, Show Preview, Predict Offset, Mate/Flush·Opposed/Aligned 솔루션), 컴포넌트 선택 형상에 원점 평면·작업 피처 포함. 수용: 상자 윗면과 로봇 베이스 바닥을 Mate로 구속하면 로봇이 이동하고 Flush 전환으로 방향이 뒤집히며 Relationships 폴더에 구속이 생기고 상태 점이 채워진다 [A19,A22,A28].
10. **Assemble·Show·구속 상태(FE+BE)**: Assemble(형상 먼저, 유형 목록 자동 제한), Show/Show Sick/Hide All 글리프, 구속 상태 점(검은/속 빈/대시), DOF 기호. 수용: 평면 2개만 고르면 Mate-Mate/Mate-Flush/Angle-Directed만 보이고, 구속 후 DOF 기호에 남은 자유도가 표시된다 [A16,A24,A27,A33].
11. **Joint: Rigid·Rotational + 로봇 마운트(BE+FE)**: 조인트 명령(J), 원점 선택·Flip component·Align 1/2·Invert, Automatic 유형(원형 원점=Rotational, 그 외 Rigid), Rotational 각도 한계 +/- 표시. 수용: 로봇 베이스 Origin을 설비 작업 평면 점에 Rigid로 붙이면 설비를 이동할 때 로봇이 따라가고, 툴 원점을 플랜지 프레임에 Rigid로 붙이면 툴이 TCP와 일치 [A30,A69].
12. **Measure 3D(BE+FE)**: 도구 > 측정 패널(점·모서리·면 거리, 각도, 컴포넌트 쌍 최소 거리, 최소/중심/최대 거리 전환, 선택 1·2 색 구분, 값 복사). 수용: 로봇 TCP와 설비 모서리 사이 거리가 mm로 표시되고 컴포넌트 2개 선택 시 `gc_collide` 최소 거리와 1e-6 m 이내 일치 [A46].
    - **상태(리드, 2026-11-17)**: 1차 완료 — 도구 > 측정 패널, 컴포넌트 우선순위(바디·로봇·웨이포인트·좌표계), 선택 1·2 색 구분, 최소 거리(충돌 형상, `Session::measure_pair`)/중심 간 거리 전환, 위치·Δ·Z축 각도, 모두 복사·다시 시작·완료, 빈 곳 클릭 = 다시 시작, 뷰포트에 최소 거리 선. 테스트 `gc_session/tests/measure.rs`, `shell.rs::measure_gives_the_minimum_distance_between_two_components`. 원문: https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-1805AB8F-F782-4B24-A332-B4F2E10A3D00.htm . 2차(리드): **모서리 및 면 우선** — 메시의 평평한 면(같은 평면 삼각형 영역)·외곽 모서리·꼭짓점 피킹(`gc_mesh::region`, `Picker::pick_surface`, `Viewport::pick_feature`), 점·모서리·면 조합의 거리/Δ/각도(`gc_session::measure_features`), 면적, 우선순위 바꾸면 명령 재시작. 3차(리드): 원통면 인식(`gc_mesh::region::cylindrical_region`, 축·반지름·길이 맞춤) → 지름·반지름·길이, 원통이 있는 쌍은 최소 거리와 중심 간 거리(축 기준)를 함께 표시(원문: 원·원통 선택 시 Minimum과 Center to Center 동시 표시). 4차(리드): 누적 특성(값 옆 +/−로 거리·면적·각도 그룹에 더하고 빼기, 그룹 합계, 값·그룹 삭제, 모두 복사; 원문 GUID-3610915E-112A-4BBD-8E28-820D33F2A717), 점 2개 뒤 Shift+세 번째 점 = 두 번째 점에서의 각도. 남음: 원형 모서리(호) 길이·지름, 체적·질량 누적(질량 개념 없음), 이중 단위·정밀도, 값 클릭 시 그래픽 강조, 리본 검사/도구 탭 버튼(프론트엔드 리본 정리 때), 시각 메시 기준 최대 거리.
13. **Free Move·Free Rotate + Update 복귀(FE)**: 뷰 평면 드래그와 3D 회전 기호(내부/상하/좌우/테두리), 관계 있는 컴포넌트는 Update 시 구속 위치로 복귀. 수용: 구속된 컴포넌트를 드래그 후 Update하면 원위치로 돌아오고 고정·무구속 컴포넌트는 새 위치 유지 [A12,A13].

### P2
14. **억제·활성(Enabled) + 격리(Isolate)(BE+FE)** — 컴포넌트 억제는 평가·계획에서 제외, Enabled 끄면 반투명·선택 불가 [A56,A57,A60,A37].
15. **서브어셈블리·Demote·활성화(BE+FE)** — 로봇을 서브어셈블리로 보이기(§4) [A66,A67].
16. **Ground and Root·Place at Component Origin·Rename Browser Nodes(BE+FE)** [A4,A5,A6].
17. **구속 완성(BE)**: Angle(Directed/Undirected/Explicit), Tangent(Inside/Outside), Symmetry, 한계·Resting Position, Suppress 체크, ALT+드래그 추론, 구속 오류 대화상자·Design Doctor·Conflict Analysis [A20,A21,A23,A25,A34,A35,A36].
18. **Joint 완성(BE)**: Slider·Cylindrical·Ball(·Planar 확인 후), Linear/Angular 한계, Lock/Protect, Between Two Faces·Offset Origin [A30].
19. **Drive 대화상자(FE+BE)**: 조인트·구속 시연(Start/End/Increment/Repetitions/Collision Detection) — UR 관절·그리퍼 손가락 시연에 사용 [A31,A32]. **상태(리드, 2026-11-17)**: 1차 완료(로봇 관절) — 도구 > 구동: 로봇·관절 선택, 시작(현재 값)/끝(시작+10), 일시 중지 지연, 자세히(충돌 감지, 증분: 값의 양/총 단계 수, 반복: 시작/끝·시작/끝/시작 + 횟수), 정방향·역방향·중지·한 단계·시작으로·끝으로, 충돌 시 멈춤과 원인 표시. 구동은 문서를 바꾸지 않음(`Session::drive_show`/`end_drive`), 닫으면 원래 자세. 원문 GUID-B78E59C9-F9FB-4ED5-B60E-C9198770B1AF, GUID-267E6CE6-7B8D-4972-A20A-350477A447DF. 2차(리드): 관계 구동 — 맞춤·플러시·삽입은 간격띄우기(mm), 회전 조인트는 각도(°), 강체는 제외(백엔드 `drive_relationship`), 대상 선택 콤보, 충돌 감지는 움직인 컴포넌트와 나머지의 간섭으로 판정, `GraphCtrlApp::drive_relationship(id)`(브라우저 관계 우클릭에서 호출할 진입점). 남음: 브라우저 관계 우클릭 > 구동 연결(프론트엔드), 녹화(AVI), 측정으로 값 입력.
20. **View·Positional 표현(BE+FE)**: 가시성·단면 정의(View), 관계 오프셋·억제(Positional) 저장, Primary는 편집 불가 [A44,A63].
21. **Interference 분석(BE+FE)** — 세트 지정·결과 목록·무시·줌 [A47,A48]. **상태(리드, 2026-11-17)**: 1차 완료 — 도구 > 간섭 분석: 세트 #1/#2 정의(선택으로 추가·제거, 세트 #2 비면 세트 #1 내부 검사), 모든 컴포넌트를 세트 #1에, 결과(개수·총 체적·항목별 체적·중심), 무시/이하 모두 무시/무시 해제/모두 해제, 무시 항목 취소선과 표시 필터, 두 번 클릭 확대, 모두 복사, 뷰포트에 간섭 영역 표본점(빨강). 체적은 충돌 형상 위 24³ 격자 추정. 원문 GUID-80DED0A6-AC65-4C7C-AD85-0144EE6DAB5B. 남음: 간섭 영역을 임시 솔리드로 렌더(지금은 점), 나사 필터(해당 없음), 인쇄, 검사 탭 리본 버튼.
22. **작업 축·점·Show Inputs·Object Visibility, Grip Snap(BE+FE)** [A14,A49,A50,A51].
23. **단면 미니 툴바(Move/Rotate 전환·값 상자·Cap Preview), 컴포넌트별 단면 참여(FE)** [A44].

### P3
- 패턴(원형·직사각형, 요소 억제·독립) [A58–A61], Model State, 선택 집합 [A65,A68], 교차 부품 투영의 적응성 없는 연결 투영 [A43], Planar 조인트·구속 한계 세부, 컨택 솔버 [A18].

## 6. 미확인·다음 확인 사항

- Origin 폴더 항목의 이름(YZ/XZ/XY Plane, X/Y/Z Axis, Center Point)과 **축 3개의 존재** — 2026 도움말은 "reference planes + center point"만 서술 [A52,A55]. 실기 확인 필요.
- 숨김 Origin 평면을 그래픽에서 바로 선택할 수 있는지, 컴포넌트 Origin 요소가 구속·조인트의 직접 선택 대상인지의 문구.
- **End Section**(사용자 요청 명칭): 읽은 2026 도움말에 없음. Half/Quarter/Three Quarter만 확인 [A44].
- **Planar 조인트**, 유형별 DOF 표, 조인트 글리프 모양, Automatic 유형의 임계 규칙 세부.
  - **Planar 조인트 상태(백엔드, 2026-11-18)**: 구현됨 — **원문 미확인, 일반 정의**(Z축 정렬 + 첫 원점이 둘째 원점 평면 위; 평면 위 병진 2 + 법선 축 회전 1 자유). 2026 도움말 조인트 페이지(A30)와 이웃 페이지에 Planar 서술이 없어 리드 지시로 일반 정의를 썼다. Gap 값·Ball 한계는 원문 확인 전까지 보류.
  - **Transitional 구속 상태(백엔드, 2026-11-18)**: 보류 — 원문(`GUID-CD8AE8A0-03F0-468A-BE85-D7C5B989BA0B`)은 "원통면이 이어진 면 집합에 접함"인데 선택 기하에 면 인접 정보가 없다. 근사 구현은 하지 않는다(리드 결정).
- 첫 컴포넌트 자동 고정 여부(두 페이지가 어긋남 [A11] 대 [A18]).
- 단면 캡의 기본 색·해칭, 단면 평면 핸들 색, 구속/조인트 글리프 색, DOF 기호 모양, 오류 관계 아이콘 형태.
- Project Cut Edges에서 다중 부품·창 선택, 단면 뷰와의 연동 방식.
- 컴포넌트 우클릭 메뉴의 정확한 항목 문구(Suppress/Enabled/Grounded 등), 한글 UI 명칭.
- `docs/handoff/backend.md`에 Assembly/WorkFeature/Feature/Timeline 엔티티 설명이 없음 — §5의 엔티티 이름은 가칭. 리드가 확정한 스키마가 있으면 이 문서의 담당 열을 맞출 것.

## 출처 (Inventor 2026 도움말)

접두 `https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/` (A45, A61, A69–A71은 `.../Inventor-WhatsNew/files/`). 전 문서 2026판 본문을 직접 읽었다.

| ID | 문서 | 파일 |
|---|---|---|
| A1 | Assembly Browser Reference | GUID-CB6B1637-71AD-4753-8CC9-19559BFEE3A5.htm |
| A2 | Working with the Assembly Browser | GUID-5F975252-BDB4-488C-A2C5-8B15C7D3C641.htm |
| A3 | Inventor Browser (Node Renaming) | GUID-EDF6B951-….htm |
| A4 | Rename Browser Nodes | GUID-64CEED31-D107-4AA2-A5F9-EBC9335D0B5A.htm |
| A5 | Ground and Root | GUID-98FA88B2-AA32-4A27-946A-DCECB36A6696.htm |
| A6 | Place at Component Origin | GUID-E7A45820-E297-449D-AFBF-3C8D8EAE8A0E.htm |
| A7 | Common Origin Modeling | GUID-D89AE80F-6AB7-4ACF-A015-47E3FA512DFE.htm |
| A8 | To Place Components in Assemblies | GUID-2C55B507-C204-4AD7-8804-BA9461306616.htm |
| A9 | To Place and Insert Parts (2026.2) | PLACE_AND_INSERT.htm |
| A10 | Relocate a grounded component | GUID-3D3382D0-7CBD-4201-84AD-93D9F638D337.htm |
| A11 | Place Components | GUID-2E0B9110-….htm |
| A12 | Move assembly components | GUID-9796C22B-23C1-4D69-BC2E-54056E46F255.htm |
| A13 | Rotate assembly components | GUID-7EAE9DFD-899A-4FD1-B63F-92DD41D0A916.htm |
| A14 | Grip Snap move or rotate | GUID-E1CA109A-D5A1-4131-AC3A-AE217716DD8C.htm |
| A15 | Degrees of freedom in assemblies | GUID-6E131E5A-438B-4008-94F3-D3A3E92EF44C.htm |
| A16 | View degrees of freedom in assemblies | GUID-3BCBF614-CF92-4980-A965-80AEAFEF768E.htm |
| A17 | Restore degrees of freedom to grounded components | GUID-4B644959-246B-4643-ADF6-C1516569DBB2.htm |
| A18 | Defining Positions and Relationships with Constraints and Joints | GUID-2DFEAF48-DCBF-4781-9AC7-7….htm |
| A19 | To Place Mate or Flush Constraints in Assemblies | GUID-0F500089-4F69-41C5-AFE8-B58AC9E8E15E.htm |
| A20 | To Place Angle Constraints in Assemblies (What's New 2026.1) | GUID-AD4679FD-EE9B-47C0-9E32-04B990BDD29E.htm |
| A21 | Place Tangent constraints in assemblies | GUID-FF270566-CA9E-420A-A00A-F52DD0FDFD8E.htm |
| A22 | To Place Insert Constraints in Assemblies | GUID-97DD5557-FAEE-46FA-845D-11123D725BA9.htm |
| A23 | Place Symmetry constraints in assemblies | GUID-237E701E-A16B-49D5-95C2-FB6528E41217.htm |
| A24 | Create assembly constraints using the Assemble command | GUID-C1E8D955-784D-47DE-9DD4-C9C695051320.htm |
| A25 | Place assembly constraints using ALT-drag | GUID-0D26AAEA-BBF0-4BC1-BDCA-A4C17CE1D842.htm |
| A26 | Edit assembly constraints | GUID-36315745-5FE6-4277-9F00-E410B962D83F.htm |
| A27 | To Manage the Display of Constraint Status in the Browser | GUID-A0573060-3778-4D6F-9431-28587D837CC7.htm |
| A28 | Place Constraint Reference - Assembly Tab | GUID-EBD2C784-0E71-452A-B5F2-22738D22D45A.htm |
| A29 | Place Constraint Reference | GUID-198F05E4-2973-4818-8F25-A0B8B9796D07.htm |
| A30 | To Define and Manage Joint Relationships | GUID-21DC3336-5C51-42C1-90FB-4299CD66E0….htm |
| A31 | To Drive Relationships in Assemblies | GUID-B78E59C9-….htm |
| A32 | Drive Relationship Reference | GUID-267E6CE6-….htm |
| A33 | Show and Hide relationships | GUID-841A4999-….htm |
| A34 | Place/Edit Connection Error Reference | GUID-23D0D333-….htm |
| A35 | Use Relationship Conflict Analysis dialog box… | GUID-456335A3-….htm |
| A36 | Use Design Doctor to repair relationship errors | GUID-8C95B0AB-….htm |
| A37 | Isolate assembly components | GUID-9D872034-….htm |
| A38 | To Project Geometry into a 2D Sketch (Project Cut Edges) | GUID-B8D8B07B-86F2-4CB7-AB21-3131D94AD1CC.htm |
| A39 | About Projecting Sketch Geometry | GUID-B61431DA-FABC-41EE-945C-6E54B4806582.htm |
| A40 | To Work with Sketches (Slice Graphics) | GUID-F5A5790A-F748-49F0-A81F-BCA56C7C1E89.htm |
| A41 | To Create and Edit Sketches | GUID-E96FB3C4-136B-4551-BF45-F54594543ECB.htm |
| A42 | Sketches in assemblies | GUID-2C180C3A-….htm |
| A43 | About Cross-Part Sketches in Assemblies | GUID-395DCA98-….htm |
| A44 | To Work with Section Views | GUID-9A47D4FC-….htm |
| A45 | What's New 2024: Assembly Section View Enhancements | GUID-EF37647C-4B81-42B1-AC6A-A4A30AF5A908.htm |
| A46 | To Measure Distance, Length, Angle, Loop, or Area… | GUID-1805AB8F-….htm |
| A47 | About Interference between Components | GUID-33B4E343-….htm |
| A48 | To Analyze Interference Among Components | GUID-80DED0A6-….htm |
| A49 | About Work Features | GUID-045A1EA5-….htm |
| A50 | To Create Work Features | GUID-38A9748E-….htm |
| A51 | To set visibility of work features… | GUID-BA193171-….htm |
| A52 | About Reference Planes | GUID-991276D7-D2BF-4266-9A57-E49146DC760C.htm |
| A53 | To Display Reference Planes | GUID-7FB44EFA-4D72-4D9B-8318-8294DE054EC7.htm |
| A54 | Part Browser Reference | GUID-36DB9904-….htm |
| A55 | Part Browser Icon Reference (Origin Plane/Point) | GUID-5F5D6502-….htm |
| A56 | Visibility of assembly components | GUID-BF99EA40-….htm |
| A57 | Component visibility and enabled status reference | GUID-19E2C95A-….htm |
| A58 | About Assembly Components in Patterns | GUID-ABC5607F-….htm |
| A59 | Create rectangular patterns of components | GUID-CF627358-….htm |
| A60 | Suppress elements in assembly patterns | GUID-FDD20E41-….htm |
| A61 | What's New 2024: Component Patterning Enhancements | GUID-A73AAF8C-50F2-4C66-860B-59FBFFF206A1.htm |
| A62 | Representations | GUID-A3060F8E-….htm |
| A63 | Positional representations in assemblies | GUID-C728DFAC-….htm |
| A64 | About View Representations | GUID-9A49CDC1-….htm |
| A65 | About Model States | GUID-8E771DBE-….htm |
| A66 | Create Subassemblies in Place | GUID-FCEE3F0E-….htm |
| A67 | Edit components in place | GUID-C79C36C9-….htm |
| A68 | About Component Selection Modes and Sets | GUID-E3E49BD5-….htm |
| A69 | What's New 2024: "J" to Directly Start Joints | GUID-231CB789-5C8E-49DE-ADAD-4F5398CC0A4F.htm |
| A70 | What's New 2022: Constraints Enhancements | GUID-D0812EFA-60BF-4BB7-9992-956CDC992192.htm |
| A71 | What's New 2023: Constraint Workflow Enhancement | GUID-F0E278CB-F99D-4EB6-BA02-D600E799D2F6.htm |

`…` 로 줄인 GUID는 TOC 목록(`/cloudhelp/2026/ENU/Inventor-Help/files/GUID-XXXXXXXX-…`)의 앞 8자리로 식별한 페이지이며 전체 파일명은 TOC JSON에서 앞 8자리로 조회하면 된다.
