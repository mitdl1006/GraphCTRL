# 프론트엔드 인수인계

## 한 일
- 브라우저: `gc_ui::panel::tree_row`(종류 아이콘·선택/호버 배경·가시성 체크·삭제 ×). × 는 호버·선택 행에서만 보이고, 접근성 이름("X 삭제")은 항상 존재. 원점·스케치·파라미터·셀 항목 모두 이 행을 사용. 스크린샷: `docs/evidence/img/review-25-browser-rows.png`
- 속성 창: 콤보박스(좌표계 축·티치 좌표계·툴 형상·경로 선택·플래너·웨이포인트 추가)와 시드 행을 `panel::combo_row`/`panel::label` 왼쪽 라벨 열로 정렬(라벨이 접근성 이름). `review-26-properties-combo.png`

- 3D 스케치 구속 글리프: 축 방향("ax X/Y/Z")·일치·고정 등을 3D 스케치에도 표시(`paint::glyph_spots`로 배치 계산을 그리기·피킹이 공유). 선택 도구로 글리프를 클릭하면 구속이 선택되고 Delete로 삭제(테스트 `a_3d_sketch_is_drawn_in_the_viewport`). `review-27-3d-sketch-glyphs.png`

- 도구 > 성능 오버레이: 뷰포트 왼쪽 아래에 프레임 시간(ms, 지수 평균)·fps·드로 호출·삼각형. 켜면 계속 다시 그림. `review-28-perf-overlay.png`(스크린샷의 250 ms는 테스트 하네스의 고정 dt)

- M2-1 첫 조각: 스케치 행에 우클릭 메뉴(스케치 편집·삭제)·삭제 ×·더블클릭 편집. 스케치 리본 끝의 "예정" 그룹에 점·다각형·슬롯 자리(비활성).

- 경로 결과 카드: 보증 문구(패딩·검사 간격·여유, 상태 사이 보증 없음일 때 표본 문구, 접촉 허용 구간 패딩 0, 제외 항목). 로봇 속성: "충돌 검사 쌍(ACM)" 체크 목록(바꾸면 계획이 오래됨). 경로 속성: "접촉 허용" 목록(이동 전체·겹침 허용·삭제)과 추가 폼(이동·링크·장애물). 테스트: `a_robot_path_is_planned_through_the_ribbon`
- 5.11: `gc_app::install_diagnostics`(로그 stderr+`logs/graphctrl.log`+링 버퍼 500줄, 패닉 훅: 로그에 백트레이스·마지막 자동 저장을 `recovery/crash-<초>.gctrl`로 보존·rfd 네이티브 메시지), 도움말 > 로그 창, 창 아이콘(내장 `assets/icon.png`)

- 떠 있는 반투명 패널(사용자 최우선 요청): 뷰포트가 창 전체를 차지하고 모델 브라우저·속성 창은 `egui::Area` 오버레이(`app.rs side_panel`; 알파 214=`colors::PANEL_ALPHA`, 여백 8=`PANEL_MARGIN`, `gc_ui::panel_frame/panel_fill`). 안쪽 가장자리 드래그로 폭 조절(왼 180–360, 오른 280–520), 모서리 버튼으로 접기/펼치기(접으면 44px 예약). 패널 영역은 `ui.interact`로 마우스를 가로채 뷰포트로 안 넘어감. `Viewport::clear`(패널이 가리지 않는 영역)를 ViewCube·성능 오버레이·전체 보기(fit, 해당 영역 중앙으로 이동)가 기준으로 사용. 뷰 탭 > 패널 > "패널 반투명"(기본 켬), `Settings::opaque_panels`로 저장. 테스트 `the_side_panels_float_over_the_view`; 좌표로 뷰포트를 클릭하던 3D 스케치 글리프 테스트는 그림을 패널 밖으로 옮김(`viewport_mut()`). 스크린샷 `review-29-floating-panels.png`

- 인벤터 동등성 P1(`docs/ux/inventor-parity.md` 3절) 진행: #1 다크 테마 완료(뷰 탭 > 패널 > "어두운 테마", `Settings::dark`, `colors::*()` 함수로 전환 — 테마 의존 색은 `colors::text()` 등 함수, 스레드 로컬 스위치라 테스트끼리 간섭 없음, `review-30-dark-theme.png`). #6 문맥 탭 자동 전환 완료(스케치 시작 시 모델 탭이면 스케치 탭, 마무리 시 이전 탭 복귀; 로봇 탭에서 시작하면 그대로 둠 — 스케치 점으로 좌표계 만들기 때문).

- 기본 테마를 Dark로(검증된 인벤터 2026 기준): 설정 폴더가 있는 실제 앱은 저장된 `light_theme`이 없으면 어둡게 시작, 설정 폴더가 없는 테스트는 밝게(스크린샷·픽셀 단언 안정). 뷰 탭 "어두운 테마" 토글은 그대로.

- 백엔드 API 연결(백엔드 브랜치 7826efd 병합): 브라우저 노드마다 가시성 체크(`Command::SetVisible`, 스케치·웨이포인트·경로·좌표계 포함, 숨김·억제·롤백 뒤 항목은 3D에서 그리지 않음), 이름 바꾸기 인라인(F2는 마우스가 브라우저 위일 때, 선택된 노드 다시 클릭, 우클릭 "이름 바꾸기"; Enter 확정·Esc 취소; 중복은 상태바 "같은 이름이 이미 있습니다"), 우클릭 "억제/억제 해제"(웨이포인트·경로·스케치, 억제 항목은 흐리게), 3D 스케치 점 드래그(선택 도구, 작업 평면 법선 방향 평면 위에서 `spatial_drag_*`). 테스트: `browser_nodes_are_renamed_and_hidden`, `a_3d_sketch_point_is_dragged`. 파라미터는 백엔드가 이름 변경을 거부하므로 인라인 편집 없음.

- 스케치 P1(FE): P1-1 새 스케치는 선 도구로 시작, P1-10 F8/F9(`Viewport::show_constraints`), P1-9 창(왼→오른쪽, 완전히 포함)/교차(오른→왼쪽, 점선 상자) 선택 + Shift/Ctrl 유지·토글(뷰포트가 `ClickItem`으로 적용), P1-7 우클릭 "완료"(지금은 Esc)·"스케치 마무리". 테스트 `sketch_basics_follow_the_cad_conventions`. 백엔드가 올릴 도구 API(HUD·ToolCommand·정렬선·스냅 종류)는 합의했고 올라오면 화면을 붙임. P1-14 단축키: L·C·A·R·D·F8·F9 있음, T·TR·`-`는 도구가 생기면.
- 리드 공지: GraphCTRL은 어셈블리(.iam) 수준 — 브라우저 트리는 구성요소(설비 부품·UR·툴·필드) 아래 각자의 원점 폴더(평면·축·중심점)를 가져야 하고 뷰포트에서 평면 호버·선택으로 스케치 시작. 트리 구조를 이 계층으로 확장 예정(명세 `docs/ux/inventor-assembly-parity.md` 대기).

- 어셈블리 P1-7 Slice Graphics(F7): 2D 스케치 편집 중 F7 또는 상태바 "그래픽 자르기 (F7)"가 시점 쪽 모델을 잘라 스케치 평면을 드러냄(렌더러 `View::slice` 평면 + 셰이더 discard, 피처 엣지는 양 끝이 잘린 쪽이면 생략; 평면에 놓인 면도 잘림), 스케치 마무리 시 자동 복원, 3D 스케치는 해당 없음. 근거: 인벤터 2026 "To Work with Sketches"의 Slice Graphics 절 — https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-F5A5790A-F748-49F0-A81F-BCA56C7C1E89.htm (원문의 복원 조건 중 "피처 명령 클릭"은 피처가 없어 미구현). 테스트 `slice_graphics_reveals_the_sketch_plane`(GPU), `review-31-slice-on/off.png`.

- 스케치 P1-6 HUD 입력(동적 입력): 도구가 다음 점을 기다리는 동안 커서 옆에 길이·각도·반지름 등 상자(`gc_viewport/src/hud.rs`, 백엔드 `tool_hud()`/`ToolEvent::Hud`/`Key::Enter`). 숫자를 치면 강조된 상자에 들어가고 Tab이 잠그고(자물쇠 아이콘) 다음 상자로, Shift+Tab은 이전, Enter가 잠근 값으로 도형 완성, Esc가 취소. 근거: 인벤터 2026 "To Draw 2D Geometry Using Dynamic Input" https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-33A8B11D-A04C-4809-BC82-C0419215915F.htm . 원문과의 차이: HUD는 인벤터에서 응용 옵션으로 켜는 선택 기능(기본 꺼짐)인데 여기서는 기본 켬(`Viewport::hud_on`, 설정 화면 없음); 점 도구의 X/Y 좌표 입력과 "Enable Pointer Input"(시작점 좌표 입력)은 백엔드에 점 도구가 없어 미구현. 테스트 `the_heads_up_display_draws_a_line_from_typed_values`.

- 리본 정리 1차: "검사" 탭 신설(측정, 간섭 분석), "구동"은 로봇 탭의 "관계" 그룹으로 이동, 도구 메뉴에서 세 항목 제거(근거 `docs/ux/inventor-assembly-ui.md` §2). "로봇" 탭은 Inventor에 없는 추가 기능이라 그대로 두고, 나머지 탭(설계·관리·환경 등)은 원문을 못 구해 만들지 않음. 단면도(Half/Quarter/Three-Quarter): 도움말 A44의 GUID가 문서에 잘려 있어(`GUID-9A47D4FC-….htm`) 원문을 못 읽었고 Quarter/Three-Quarter가 잘라 내는 쪽(1/4 vs 3/4)이 불확실해 보류 — 리드가 전체 GUID를 주면 구현. 컴포넌트 원점 폴더(#2)는 백엔드 `component_origin` 대기.

- 어셈블리 P1-2/3: 바디·로봇마다 브라우저에 "원점" 폴더(YZ 평면, XZ 평면, XY 평면, X축, Y축, Z축, 중심점; 기본 숨김, 항목마다 체크, 폴더 우클릭 "원점 표시/숨기기"), 보인 요소는 뷰포트에 컴포넌트 포즈로 그려짐(`gc_viewport/src/origin.rs`: 평면 반투명 정사각형, 축 색선, 점), 클릭 선택(점→축→평면 순, 숨긴 요소는 뷰포트에서 선택 안 됨·브라우저에서는 선택 가능, 브라우저와 선택 동기화, Esc 해제), 평면 우클릭 "스케치 만들기"(`create_sketch_on_component`). 테스트 `a_component_origin_plane_is_shown_picked_and_sketched_on`, 스크린샷 `review-32-origin-plane.png`. 근거는 `docs/ux/inventor-assembly-parity.md`의 A52/A53 요약뿐(도움말 원문은 직접 확인 못 함). 미구현: 평면 크기 드래그 조절(지금 고정 한 변 0.3 m라 작은 부품에는 큼), 속성 창에 소유 컴포넌트·좌표 표시, 설비(assemblies)·부품 트리와 그 원점 폴더, 로봇 링크 프레임.

- 브라우저를 어셈블리 트리로(리드 요청 1·4·3): 맨 위에 문서 노드("Cell1" 또는 파일 이름) 아래에 관계·표현(아직 빈 자리)·원점·구성요소("Body1:1", "UR5e_1:1", 각자 원점 폴더). 스케치·파라미터·좌표계·웨이포인트·경로는 GraphCTRL 고유 폴더로 아래에 유지(파라미터 폴더는 리드의 매개변수 대화상자가 들어오면 제거). 상태바 맨 오른쪽에 "구성요소 N"·"문서 1"(원문: 활성 문서의 발생(occurrence) 수와 열린 문서 수). 오른쪽 탐색 막대(모두 확대·초점 이동·확대/축소·궤도·면 보기; 초점 이동·확대/축소·궤도는 Esc까지 켜져 있는 왼쪽 버튼 도구, 면 보기는 선택한 원점 평면 또는 편집 중 2D 스케치 평면으로 시점 정렬)와 ViewCube 옆 홈 버튼(코드로 그린 아이콘). 근거: 인벤터 2026 Inventor Browser·Assembly Browser Reference(GUID-EDF6B951…, GUID-CB6B1637-71AD-4753-8CC9-19559BFEE3A5), Navigation Bar(GUID-FAAB453E…), Graphics Windows 상태바(GUID-7544AABF…). 원문과 다른 점: 탐색 막대 사용자 정의·도킹·불투명도·전체 탐색 휠은 없음. 스크린샷 `review-33-nav-bar-tree.png`, 테스트 `the_navigation_bar_and_the_assembly_tree`. 미구현: 설비(어셈블리)·부품 occurrence 노드, 접지·비활성·보이지 않음 아이콘 상태, 사용자 정의 폴더.

- 어셈블리 구속 대화상자(P1-9, 백엔드 f4460bd): 로봇 탭 > 관계 > "구속"(`gc_app/src/constrain.rs`). 유형 맞춤/플러시/삽입, 첫 번째·두 번째 선택은 뷰포트 클릭으로(대화상자가 열려 있는 동안 모든 컴포넌트의 원점 평면·축·점이 그려지고 숨긴 것도 잡힘), "오프셋 및 방향 예측" 체크(백엔드 `predict_relationship`), 맞춤의 축 해(반대/정렬/방향 없음), 오프셋(mm), 확인(닫기)/적용(계속)/취소. 구속은 브라우저 "관계" 폴더에 이름과 삭제 ×로 나열, 풀 수 없으면 상태바 "구속을 풀 수 없습니다". 근거: Inventor 2026 To Place Mate or Flush Constraints in Assemblies, GUID-0F500089-4F69-41C5-AFE8-B58AC9E8E15E (Constrain 대화상자의 Type·Predict Offset and Orientation·First/Second Selection·Solutions·Offset·Apply/OK). 원문과 다른 점·미구현: 면·모서리·꼭짓점 같은 메시 형상 선택(지금은 원점 요소만), Show Preview, More(이름·한계·휴식 위치), Select Other, 구속 편집·상태 점. 테스트 `the_constrain_dialog_mates_two_components`.

- 단면도(어셈블리 #8): 뷰 탭 > 외형 그룹 "1/2 단면·1/4 단면·3/4 단면·단면 삭제". 시작하면 캔버스 위 미니 툴바(평면 YZ/XZ/XY 선택, 위치 mm, 뒤집기, 계속(1/4·3/4은 두 번째 평면)/확인, 취소)와 절단 평면 사각형이 보이고, 렌더러(`View::slice/slice2/slice_union`, 셰이더 discard)가 절단 쪽을 그리지 않음(`gc_viewport/src/section.rs`). 근거: Inventor 2026 To Work with Section Views, GUID-9A47D4FC-30CE-422D-86EC-5E4AFC624C2D(원문 확인: 흐름 Half = 평면 선택·Flip·위치·Continue/OK, Quarter·Three Quarter = 두 평면, 미니 툴바 Flip·Move/Rotate·값·Cap Preview, 부품 수 ≤ 500이면 캡 기본 표시, 뷰 표현에 저장). **원문에 1/4과 3/4이 잘라 내는 쪽이 적혀 있지 않아 가정**: 1/4 = 두 평면의 절단 쪽이 겹친 4분의 1을 제거(교집합), 3/4 = 두 평면 중 어느 한쪽이라도 절단 쪽이면 제거(합집합, 4분의 1만 남음) — 원문 그림이나 실기로 확인 필요, 반대면 `SectionKind::slicing`의 `union` 한 줄. 미구현: 단면 캡(절단면 채움 렌더), 캡 미리보기 토글·500개 규칙, 임의 평면·평면 면 선택(지금은 월드 원점 평면 3개), 드래그로 평면 이동, 이동/회전 전환, 뷰 표현에 저장(백엔드), 우클릭 Flip·OK, 스크롤 단계. 테스트 `section_views_cut_the_model`, 스크린샷 `review-34-half-section.png`.

- 마킹 메뉴(리드의 `gc_ui::marking`)를 뷰포트 우클릭에 연결(`gc_viewport/src/radial.rs`): 어셈블리 환경 = 북 구속(구속 대화상자), 동 컴포넌트 이동·남 컴포넌트 회전(둘 다 Free Move/Rotate가 없어 비활성), 서 부품 배치(메시 불러오기); 선택·원점 요소 고유 명령(표시/숨기기, 계획, 삭제, 스케치 만들기)과 홈 뷰·모두 확대는 overflow. 스케치 환경 = 북 선, 동 원, 서 사각형, 남 스케치 마무리(3D 스케치는 선·마무리), overflow에 완료. 근거: Inventor 2026 About Marking Menus(GUID-4B081B74-45EA-451E-84BE-9FFE0E2ACAE3). **섹터 배치는 원문이 말하지 않아 임의로 정함**(원문 확인: 스케치는 중심점 원·선·두 점 사각형·스케치 마무리, 어셈블리는 구속·부품 배치·컴포넌트 이동·컴포넌트 회전). 기존 우클릭 컨텍스트 메뉴를 대체(`item_menu`는 남아 있으나 뷰포트가 더는 부르지 않음). 앱 연결: `Viewport::take_request`(Constrain/PlaceComponent). 테스트 `the_marking_menu_picks_by_mark_and_by_menu`(마크로 구속 대화상자 열기, 메뉴 모드의 명령 이름), 기존 우클릭 메뉴 테스트 3개는 마우스 클릭(`mouse_click_label`)으로 바꿈 — 접근성 트리의 `.click()`은 뷰가 직접 그리는 위젯에 닿지 않음. 미구현: 메뉴 모드 항목 아이콘, 위/아래 키, "고전 메뉴로 전환" 옵션, 사용자 정의.

- 조인트 대화상자(어셈블리 P1-11): 로봇 탭 > 관계 > "조인트", 단축키 J(스케치 밖). 유형 자동(두 원점이 원이면 회전, 아니면 강체)/강체/회전, 첫 번째 원점(움직일 쪽)·두 번째 원점(고정 쪽)은 뷰포트 클릭(중심점 = 컴포넌트 자체 원점 프레임 `Geometry::origin_frame()`, 평면·축 = 그 요소), 구성요소 뒤집기(flip), 확인/적용/취소; 브라우저 "관계" 폴더에 나열(`gc_app/src/constrain.rs` `Joint`). 근거: Inventor 2026 Create Joints Reference(GUID-6AA68E8F-7C97-4806-8483-3941DE915E70)와 To Define and Manage Joint Relationships(GUID-21DC3336-5C51-42C1-90FB-4299CD66E0C6) — 원문의 Type·Connect(First/Second origin)·Flip component·Gap·Name 중 Gap·Name 입력·Suppress·Align 1/2·Invert alignment·Limits 탭·Animate·Pick part first·Between Two Faces/Offset Origin·슬라이더/원통/평면/볼 유형은 미구현(백엔드가 강체·회전만). 로봇 마운트와 "툴을 플랜지에"(`mount_tool`) 화면도 아직 없음. 테스트 `the_joint_dialog_joins_two_components_rigidly`.

- 로봇 링크 하위 노드·구속 상태 점(백엔드 3a481c2): 로봇 행 아래 "링크" 폴더에 `robot_links`의 링크를 사슬 깊이만큼 들여 "이름:1"로 나열하고 링크마다 원점 폴더(뷰포트가 링크 원점도 그리고 고름, 구속·조인트 선택에도 그대로). 구성요소 행과 "관계" 폴더 행 오른쪽에 구속 상태 점: 검은 점(완전 구속/고정), 속 빈 점(움직일 수 있음), 붉은 대시(만족하지 않음). 근거는 `docs/ux/inventor-parity.md`·Assembly Browser Reference의 상태 설명뿐(아이콘 모양 원문 미확인). 미구현: 링크별 가시성·선택, 점 호버 설명, DOF 기호. 테스트 `robot_links_are_components_below_the_robot`.

- Free Move / Free Rotate(어셈블리 #13): 마킹 메뉴의 "컴포넌트 이동"·"컴포넌트 회전"이 도구를 켜고(`Viewport::free_tool`), 왼쪽 드래그가 바디·로봇을 뷰 평면에서 옮기거나(광선과 컴포넌트를 지나는 뷰 평면의 교점 차이) 자기 원점 기준으로 돌림(화면 가로·세로 드래그 = 카메라 up·right 축 회전, 픽셀당 0.01 rad), 드래그 한 번이 실행 취소 한 단계, Esc로 종료(`gc_viewport/src/freemove.rs`). 근거: `docs/ux/inventor-assembly-parity.md`의 A12/A13 요약만(도움말 원문 확인 못 함). 원문과 다른 점·미구현: 3D 회전 기호(내부/상하/좌우/테두리), 구속된 컴포넌트의 Update 복귀(드래그 후 구속 위치로 돌아가기), 접지 컴포넌트 이동 막기, 설비·부품 occurrence 드래그, 리본 버튼. 테스트 `free_move_and_free_rotate_drag_a_component`.

- 관계 구동 연결(리드 요청): 브라우저 "관계" 폴더의 구속/조인트 노드 우클릭 메뉴에 "구동"(강체 조인트는 비활성)과 삭제, 선택하면 `drive.start_relationship`. 리본 관계 그룹의 구동 버튼은 로봇이 없어도 관계가 있으면 활성이고, 로봇이 없으면 첫 번째 구동 가능한 관계를 엶. 마킹 메뉴 overflow에는 넣지 않음. 테스트 `a_relationship_is_driven_from_the_browser`(메뉴 항목이 눌리는 것까지만 확인, 구동 동작 자체는 리드의 구동 테스트).

- 관계 억제·구성요소 상태 1회 조회(리드 배치 ②⑥): 브라우저가 프레임마다 `session.component_statuses()`를 한 번만 부르고(`ParamsUi.statuses`) 행은 그 맵을 읽음. "관계" 폴더 행은 억제되면 흐리게 그리고 상태 점을 숨기며, 우클릭 메뉴에 "억제/억제 해제"(`BrowserAction::SuppressRelationship` → `set_relationship_suppressed`, 한 번의 실행 취소 단계). 근거: Inventor 억제 도움말 원문은 아직 확인하지 못함(GUID 미확인) — 흐림 표시는 기존 노드 억제 표시를 따른 것. 테스트 `a_relationship_is_suppressed_from_the_browser`.

- 구속 오류 대화상자(리드 배치 ③): 구속·조인트 대화상자에서 관계가 `EditError::Unsolvable`로 거부되면 `gc_app/src/conflict.rs`의 대화상자가 열려 만족되지 않는 이름들(`names`)과 `conflicting_relationships(Some(&후보))`의 관계 행마다 [억제][삭제]를 보여 주고, 누르면 `set_relationship_suppressed`/`delete_relationship` 뒤 후보를 다시 `add_relationship`(성공하면 닫힘, 아니면 남은 원인을 다시 표시). 한 관계로는 풀리지 않으면 안내 문구. 근거: 백엔드 handoff의 Design Doctor 흐름 + `docs/ux/inventor-assembly-parity.md` A34/A36 요약(전체 GUID가 문서에 잘려 있어 원문은 못 읽음). 원문과 다른 점·미구현: Accept the relationship(오류 기호와 함께 생성), Diagnose, 관계 충돌 분석(Keep/Break), [편집](그 관계로 구속 대화상자 열기), 배치 이동 거부 안내. 테스트는 `conflict::tests`(대화상자 화면 자체는 kittest·스크린샷 없음).

- 패턴 대화상자·조립 탭(리드 배치 ④⑤): 새 리본 탭 "조립"(3D 모델·스케치 다음): 컴포넌트(배치 = 메시·어셈블리 가져오기) / 패턴 / 관계(구속·조인트·구동, 로봇 탭에서 옮김) / 위치(컴포넌트 이동·회전 = 마킹 메뉴와 같은 `free_tool`). 로봇 탭에는 로봇 전용(추가·경로·계획)만 남음. 패턴 대화상자(`gc_app/src/pattern.rs`): 사각형/원형 탭, 패턴화할 컴포넌트 체크 목록, 열·행 방향(원점 축을 뷰에서 클릭, 뒤집기·개수·간격 mm), 원형은 회전 축·뒤집기·중간 평면·개수·각도°·회전/고정·증분/맞춤, 확인 → `session.add_pattern`(거부되면 열린 채 상태바). 브라우저에 "패턴" 폴더(이름 (개수), 삭제 = `DeletePattern`). 근거: Inventor 2026 Create rectangular/circular patterns of components(GUID-CF627358-120F-4142-A9D0-13217BD3432D, GUID-FF7B08E3-624B-4382-B4E5-304C0354C048) 원문 확인. 원문과 다른 점·미구현: 방향을 모서리·작업 축·면에서 고르기(원점 축만), Associative 탭, 기준점(Base Point), 패턴 편집(`SetPattern`)·요소 독립화, 패턴 요소 노드(브라우저에는 패턴 한 줄만), 부모를 브라우저·뷰 선택에서 가져오기(체크 목록), 접지(Ground) 버튼(컴포넌트 선택 개념이 아직 없어 미구현). 스크린샷 `docs/evidence/img/review-40-assemble-pattern.png`. 테스트 `pattern::tests`, `the_pattern_dialog_lists_the_components`; 구속·조인트·구동을 누르는 기존 테스트는 "조립" 탭으로 옮김.

- 단면 캡(리드 배치 ①): 단면도의 잘린 면을 평평한 색으로 채움. 셰이더가 슬라이스 중 `front_facing`이 아닌 면(잘린 틈으로 보이는 먼 벽의 안쪽)을 캡 색으로 그림(`View::slice_cap`, 단면도에서만 켜고 F7 스케치 슬라이스에서는 끔). 닫힌 메시(바깥을 향한 감는 방향)를 전제 — 열린 메시·뒤집힌 법선은 캡이 잘못 보일 수 있음. 겹친 컴포넌트가 서로 속을 파고들면 겹친 부분도 캡 색. 불투명(알파 1) 메시에만 적용. 원문(Section Views, GUID-9A47D4FC-30CE-422D-86EC-5E4AFC624C2D)과 다른 점: 해칭 무늬 없음, 캡 색은 내 선택(베이지). 스크린샷 `docs/evidence/img/review-41-section-cap.png`, 테스트 `a_slice_through_a_solid_shows_a_cap`(gc_render GPU), `section_views_cut_the_model`.

- DOF 기호(리드 큐): 뷰 탭 > 가시성 > "자유도" 버튼이 모든 컴포넌트의 기호를 켜고 끔(`session.set_dof_shown`). 뷰포트(`gc_viewport/src/dof.rs`)가 `session.dof_symbols()`를 컴포넌트 원점에 그림: 미끄러질 수 있는 방향마다 양쪽 파란 화살표, 돌 수 있는 축마다 4분의 3 원호(주황, 끝에 화살촉), 구속 상대가 미구속이면 작은 와이어 큐브. 모양·색은 내 선택(원문에 그림 없음; 백엔드가 적은 원문 GUID-3BCBF614-CF92-4980-A965-80AEAFEF768E, GUID-6E131E5A-438B-4008-94F3-D3A3E92EF44C는 읽지 못함). 미구현: 컴포넌트별 'DOF 표시' 우클릭 항목, 큐브에 커서를 올리면 상대 부품 강조, 기호 크기 조절(카메라 거리의 12%). 스크린샷 `docs/evidence/img/review-42-dof.png`, 테스트 `degrees_of_freedom_symbols_are_shown_from_the_view_tab`.

- 접촉 세트·접촉 솔버(리드 큐): 브라우저 바디·로봇 행 우클릭 > "접촉 세트"(체크, `BrowserAction::ContactSet` → `set_contact_set`), 검사 탭 분석 그룹 "접촉 솔버" 메뉴(끔/접촉 세트만/모든 컴포넌트 = 문서 설정의 Interactive Contact 선택지, `set_contact_solver`; 켜져 있으면 버튼이 눌린 상태). 원문 Detect contact between assembly components(GUID-FA1A8282-3A7E-45BE-8DB3-F637FB1C3292)와 About the Contact Solver(GUID-0BBD4ACE-7BD1-40BF-A011-74416B229035) 확인: 리본 위치(Inspect 탭 > Interference 패널 > Activate Contact Solver)와 우클릭 Contact Set은 원문대로. 원문과 다른 점·미구현: iProperties > Occurrence 탭의 Contact Set, 문서 설정 대화상자(Surface Complexity), Visibility/Enabled 해제 시 분석 제외 표시, 여러 컴포넌트 동시 선택 적용(한 행씩), 접촉 세트 구성원 표식(브라우저 아이콘), Drive 대화상자의 Collision Detection 옵션. 접촉으로 멈춘 구동은 리드의 drive.rs(`contact_stop`)가 표시. 스크린샷 `docs/evidence/img/review-43-contact-solver.png`, 테스트 `the_contact_set_and_the_contact_solver_are_set_from_the_ui`.

- 미러·복사·교체 대화상자(리드 큐 ①): 조립 탭 패턴 그룹에 "미러"·"복사", 컴포넌트 그룹에 "교체" 버튼 — 하나의 대화상자(`gc_app/src/replicate.rs`, 탭으로 전환). 컴포넌트 체크 목록(교체는 하나), 미러는 뷰에서 원점 평면 클릭, 복사는 이동 X/Y/Z mm, 미러·복사는 "관계 복사"·"새 컴포넌트 고정", 교체는 박스/원기둥/구 치수 mm와 "모두 교체"(Replace All). 확인 → `mirror_components`/`copy_components`/`replace_component`; 억제된 관계가 있으면 이름을 대화상자에 나열(다시 적용하라는 안내)하고 열려 있음, 없으면 닫힘. 근거 원문: To Mirror Assembly Components(GUID-C5B3D3DF-27B3-4B4C-9058-A949138D8B8A), Copy assembly components(GUID-5AAA6CB1-7BB2-4AF0-9A3D-AB729E5160C5), Replace assembly components(GUID-E5FAA224-7332-4AC6-A78E-793FDACFEEDD) 확인. 원문과 다른 점·미구현: 복사 Status 열(Copy/Reuse/Exclude)·파일 이름 대화상자, 교체는 파일 선택 없이 기본 도형만(파일로 교체는 메시 가져오기 흐름과 연결해야 함), Replace를 뷰에서 컴포넌트 클릭으로 고르기(체크 목록), 미러 대칭면 변경(우클릭), 연관 미러(Associative), 패턴·표현 복제. 스크린샷 `docs/evidence/img/review-44-copy.png`. 테스트 `replicate::tests`, `copy_and_replace_work_from_the_assemble_tab`.

- 브라우저 표현 폴더(리드 큐 ②): 정적이던 "표현" 행을 폴더로 교체 — "뷰 표현"과 "위치 표현"([Primary] 먼저) 하위 폴더, 활성인 것은 ●와 선택 색, 클릭 = 활성화(`ActivateViewRepresentation`/`ActivatePositionalRepresentation`), 우클릭 활성화·삭제([Primary]는 삭제 불가), "새 뷰 표현"/"새 위치 표현"(`AddViewRepresentation`은 지금의 가시성 저장, 위치는 Primary 복사). 근거: About View Representations(GUID-9A49CDC1-4DC7-4462-81D7-E18B8339DE0F), To Create, Edit, or Copy Positional Representations(GUID-34C28C35-45CA-4476-9336-30146F7C9471)는 백엔드 handoff에 적힌 요약만 따름(원문 직접 읽지 않음). 미구현: 이름 바꾸기(F2는 `Rename`이 표현에 안 통함), 뷰 표현에 카메라·단면 저장(`camera`/`section` 문자열 형식 미정 — 지금은 None), 위치 표현의 재정의 편집(구동 값·억제 기록), 표현 복사, Model State. 리본 아이콘은 새 벡터 아이콘으로(패턴 = RectPattern, 미러 = Mirror, 복사 = Cuboid). 스크린샷 `docs/evidence/img/review-45-representations.png`, 테스트 `the_representations_folder_creates_activates_and_deletes`.

- 뷰 표현의 카메라·단면 저장(리드 결정): `gc_viewport/src/repr.rs`가 카메라(pivot[3], rotation xyzw[4], distance, fov_y, ortho)와 단면(kind, planes[[축, 오프셋, 뒤집힘]])을 JSON 문자열로 쓰고 읽음(`camera_to_json/from_json`, `section_to_json/from_json`). 새 의존성(serde_json)을 못 쓰므로 평평한 객체만 직접 쓰고 파싱 — 형식은 유효한 JSON이라 나중에 바꿔도 호환. 새 뷰 표현은 지금 카메라·단면을 저장(`BrowserAction::NewView`), 활성화하면 가시성과 함께 카메라·단면을 복원(`ActivateView`; 파싱 실패하면 현재 뷰 유지, 저장된 카메라가 있는 표현은 단면이 없으면 단면도 해제). 미구현: 단면 편집 중 상태 저장, 표현 갱신(Set) 버튼. 테스트 `repr::tests`, `a_view_representation_restores_the_camera_and_the_section`.
- 3D 스케치 리본 확인(리드 요청): 구속 13개가 두 줄이 된 3D 스케치 탭이 1280 px에서 "스케치 마무리"를 창 안에 둠을 `the_3d_sketch_ribbon_keeps_finish_sketch_on_screen`로 확인(스크린샷 `docs/evidence/img/review-46-3d-sketch-ribbon.png`).

- 스케치 리본 분할 버튼(리드 새 1순위): `gc_ui::ribbon_split_button`(+ `SplitItem`) — 큰 버튼 본체 = 마지막으로 쓴 변형(아이콘·이름), 아래 ▾ 띠 = 목록 팝업, 고른 변형이 본체가 됨(세션 동안 기억, 실행 중인 도구가 있으면 그 변형을 보임). 접근성 이름: 본체 = 변형 이름, ▾ = "<그룹> 더 보기". 스케치 탭 작성 패널: 선택 | 선▾(선·중점 선) | 원▾(원·접선 원) | 호▾(3점 호·접선 호·중심점 호) | 직사각형▾(2점·3점·2점 중심·3점 중심) | 슬롯▾(중심 대 중심·전체·중심점·3점 호·중심점 호 슬롯), 점·다각형·중심점 토글은 작은 버튼 유지, 3D 스케치는 선과 호▾(3점·중심점). 폭은 현재 이름만큼이라 변형에 따라 바뀜(인벤터도 그러함). 1280 px에서 리본이 약 1190 px에서 끝나 타원·스플라인을 넣을 여유 ~90 px; 테스트 `the_sketch_ribbon_has_split_buttons_and_fits_1280_px`가 2D·3D 스케치에서 "스케치 마무리"·"스케치 취소"가 창 안에 있고 변형 고르기→본체 변경→다른 도구 뒤 본체로 재실행됨을 확인. 문구 변경: "호"→"중심점 호", "사각형"→"2점 직사각형"(마킹 메뉴 항목은 "직사각형"). 새 도구 추가 시 `ribbon.rs`의 해당 `split(...)` 변형 목록에 한 줄만 추가. 근거: Inventor 2026 To Create Lines, Arcs, and Bends(GUID-4F861674-F74C-474E-869A-4C190ED6EC4B), To Create 2D Shape Geometry(GUID-D489CE6D-7299-4211-A43A-F3580A4BA357)의 Create 패널 구성(선·원·호·직사각형·슬롯 변형 묶음) 원문 확인; 리본/분할 버튼 자체의 전용 문서는 찾지 못함. 미구현: 마지막 변형의 설정 파일 저장, 3D 모델·조립 탭 적용, 타원·스플라인 자리. 스크린샷 `docs/evidence/img/review-47-split-button.png`.

- Place 흐름(리드 ③): 조립 탭 "배치"와 마킹 메뉴 "배치"가 파일을 고르면(`App::place_component`) 첫 컴포넌트는 그대로 원점(세션이 고정), 이후 메시(STL) 본체는 커서에 매달려 지면(z=0)을 따라다님(`gc_viewport/src/place.rs`, 뷰가 지면과 평행이면 컴포넌트를 지나는 화면 평면; 컴포넌트 원점이 커서 위치). 왼쪽 클릭 = 그 자리에 놓고(실행 취소 한 단계) 복사본이 다시 매달림(여러 개 연속), Esc·오른쪽 클릭 = 끝내고 매달려 있던 복사본은 버림. 근거: Inventor 2026 Place Components(GUID-2E0B9110-BBAE-482B-A9AF-953BC40E8F2A)에서 확인한 것은 "첫 컴포넌트는 자동 고정되어 원점·축을 어셈블리와 맞추고, 나머지는 구속 없고 고정 안 됨"뿐 — 커서에 붙이기·연속 배치·Esc/우클릭 종료는 리드 요청과 Inventor 동작 지식(원문 페이지는 'Drag components into assemblies'로 넘김, 그 페이지는 읽지 못함)을 따름. 미구현: 어셈블리(STEP/glTF) 파일은 잡이 읽고 원점에 나타남(CellItem이 어셈블리를 몰라 매달리지 못함), 기본 도형·라이브러리 로봇 배치, 놓을 때 스냅·면 위 배치, Place 후 자동 구속 제안(Assemble 미니 툴바 ④), 우클릭 메뉴(확인/취소) — 지금은 우클릭이 바로 끝냄. 테스트 `place_hangs_a_body_on_the_cursor_and_sets_it_down_by_clicks`, 스크린샷 `docs/evidence/img/review-48-place.png`.

- Assemble 명령(리드 ④): 조립 탭 관계 그룹 첫 버튼 "어셈블" → 창(`gc_app/src/assemble.rs`): 첫 번째(움직일 쪽)·두 번째(고정 쪽) 형상을 뷰에서 원점 요소로 고르면 `session.possible_relations`가 돌려준 해만 버튼으로 나열(맞춤(마주봄)·플러시·맞춤(정렬)·각도(방향)·접선(안쪽/바깥쪽)·삽입(반대/정렬)), 기본은 첫 해, 오프셋(mm) 또는 각도(°) 입력, 적용(계속)/확인(끝)/선택 지우기/취소, 풀 수 없으면 구속 오류 대화상자로. 근거: Inventor 2026 Create assembly constraints using the Assemble command(GUID-C1E8D955-784D-47DE-9DD4-C9C695051320) 원문 확인 — 흐름(움직일 쪽 먼저, 형상이 유형을 결정, Apply/OK/Undo)은 원문대로. 원문과 다른 점·미구현: 인벤터는 뷰 안 미니 툴바에 해를 띄우지만 여기는 창에 나열, 새 구속을 OK 때까지 기존 것과 비교하지 않는 동작(우리는 적용 즉시 풀이)·Assemble Constraint Management 대화상자(우리 구속 오류 대화상자가 대신), 한 번에 한 컴포넌트만 구속하는 제한 없음, 원점 요소 외 형상(모서리·면) 선택. 스크린샷 `docs/evidence/img/review-49-assemble.png`, 테스트 `assemble::tests`, `the_assemble_command_opens_from_the_assemble_tab`.

- 조인트 Lock·Protect(리드 큐 ①): 브라우저 관계 폴더 행 우클릭 메뉴에 조인트(`gc_doc::is_joint_kind`)만 "잠금"·"보호" 체크 항목(`BrowserAction::JointLock/JointProtect` → `set_joint_lock/set_joint_protect`, 체크 해제 = 해제), 잠긴 조인트는 행 아이콘이 자물쇠(`gc_ui::Icon::Lock`, 새 벡터 아이콘). 구속에는 항목 없음. 근거: Inventor 2026 To Define and Manage Joint Relationships(GUID-21DC3336-5C51-42C1-90FB-4299CD66E0C6) 원문 확인 — "브라우저에서 관계를 우클릭 > Lock 또는 Protect, 체크 해제로 조건 제거, 구속에는 없음"과 같음. 원문과 다른 점·미구현: 보호된 조인트 때문에 거부된 관계의 경고 문구는 상태바의 일반 오류 문구뿐, 보호 표시 아이콘 없음(잠금만), 조인트 글리프 더블클릭 편집. 스크린샷 `docs/evidence/img/review-50-joint-lock.png`, 테스트 `lock_and_protect_are_set_on_a_joint_from_the_browser`.

- 응용 프로그램 옵션 · 스케치 탭 모서리 자동 투영(리드 큐 ②): 파일 메뉴 "응용 프로그램 옵션"(Inventor: File > Options) 창, 탭 줄에 "스케치" 하나, 체크박스 "스케치 작성·편집 시 모서리 자동 투영"(`session.autoproject_face_edges` / `set_autoproject_face_edges`, 기본 켜짐). 근거: Inventor 2026 Sketch Tab Reference(Application Options, GUID-1898C432-CFD2-46F1-8BAE-631392DFBF1D) 원문의 "Autoproject edges for sketch creation and edit" 항목 확인. 설정 파일 저장: 리드가 `gc_persist::places::Settings`에 `autoproject_edges_off`(기본 false = 켜짐)를 넣어 줘서, 시작 때 읽어 세션에 적용하고 체크가 바뀌면 `save_settings`로 저장(테스트가 설정 폴더를 임시 폴더로 줘서 확인). 원문의 나머지 항목(Autoproject edges during curve creation, 스플라인 맞춤 방식, 구속 설정 등)은 미구현. 테스트 `the_application_options_switch_autoproject_of_face_edges`.

- 서브어셈블리·제자리 편집 UI(리드 새 1순위): 브라우저 컴포넌트 목록을 `session.component_tree()`로 그림(`gc_panels::component_rows`): 서브어셈블리는 접히는 폴더(편집 중이면 ● 표시), 안에 컴포넌트 행(원점·링크 폴더 포함)이 들여쓰기로 중첩. 컴포넌트 행 우클릭: 제자리 편집, 서브어셈블리로 묶기(강등; 브라우저 위쪽에 이름 입력 줄 → 확인/Enter), 최상위로 이동, 서브어셈블리로 이동(하위 메뉴); 서브어셈블리 폴더 우클릭: 제자리 편집, 이름 바꾸기(같은 입력 줄), 최상위로 이동, 해체. 더블클릭(브라우저의 폴더·컴포넌트 행, 뷰의 컴포넌트) = `edit_component`(뷰에서는 `pick_unit`으로 서브어셈블리를 한 단위로 — `ViewportRequest::EditUnit`). 범위 밖(`in_edit_scope` false)은 브라우저 행이 흐리게, 뷰의 바디·로봇이 알파 0.25로 흐리게(`gc_scene`의 `OUT_OF_SCOPE`); 선택은 백엔드가 거부. 조립 탭 "편집" 그룹: 복귀·상위로·최상위로(편집 대상이 있을 때만 활성). 근거(원문을 curl로 읽음): Inventor 2026 Create Subassemblies in Place(GUID-FCEE3F0E-AAE2-4619-8645-2588FEECD353), Edit components in place(GUID-C79C36C9-A6DB-46C5-8864-41F66C750231). 원문과 일치하는 것: 브라우저에서 같은 레벨 컴포넌트를 우클릭 > Component Demote로 새 서브어셈블리에 담기, 브라우저에서 부품·컴포넌트 더블클릭(또는 우클릭 Edit)으로 제자리 편집, 편집 중 활성 컴포넌트만 음영·나머지는 투명, Return / Return to Parent / Return to Top, 부모 어셈블리에서 서브어셈블리는 한 단위, 여러 층 중첩. 원문과 다른 점: ① 원문은 서브어셈블리를 "빈 그룹으로 먼저 만들고 곧바로 활성 어셈블리가 되어 채워 넣는" 명령(Create Subassembly in place)도 갖는데 우리는 Demote(기존 컴포넌트 묶기)만 있고 빈 서브어셈블리 생성·자동 활성화가 없음 ② 원문 Demote는 여러 컴포넌트를 선택해 한 번에 묶고 새 파일 이름·템플릿·위치·BOM 구조를 물음 — 우리는 행 하나씩, 이름만 ③ 레벨 이동은 원문이 브라우저 끌어 놓기 — 우리는 우클릭 메뉴 ④ Return 계열은 원문이 빠른 실행 도구 모음(Quick Access toolbar)에 있고 우클릭 "Finish Edit"로도 끝냄 — 우리는 조립 탭 "편집" 그룹뿐이고 뷰 우클릭 "편집 끝내기"는 없음 ⑤ 편집 중 허용 동작(부품 삭제·접지 변경·자유도 표시·적응형 지정)은 백엔드 범위 규칙을 따를 뿐 화면에서 따로 막거나 안내하지 않음 ⑥ 원문은 부품(파트)을 직접 편집하지 못하고 활성화만 하며 Edit Feature를 띄움 — 우리 컴포넌트는 메시·기본 도형이라 해당 없음. 그 밖의 미구현: 브라우저 끌어 놓기로 레벨 이동(우클릭 메뉴로 대체), 여러 행 선택해 한 번에 강등, 스케치 소유자(`sketch_owner`/`sketches_of`는 아직 내 브랜치에 없음 — 들어오면 소유자 행 아래에 스케치 표시), 편집 중 뷰 우클릭 "복귀" 항목, 뷰에서 서브어셈블리를 한 덩어리로 선택·강조(클릭 선택은 컴포넌트 하나), 가져온 어셈블리·부품 행은 여전히 목록에 없음. 스크린샷 `docs/evidence/img/review-51-edit-in-place.png`, 테스트 `subassemblies_are_made_edited_in_place_and_dissolved_from_the_browser`.

- 어셈블리 스케치 소유자 표시(리드 큐): 최상위 "스케치" 폴더는 `sketches_of(None)`만, 제자리 편집 중에 만든 스케치는 소유자(컴포넌트·서브어셈블리) 행 아래에 들여써 표시(`sketch_row`). 근거: 백엔드 handoff의 소유자 API(`sketch_owner`/`sketches_of`). 원문 확인 없음(Inventor는 최상위 스케치를 Origin 폴더 아래에 둠 — 우리는 "스케치" 폴더). 테스트 `a_component_sketch_hangs_below_its_component_in_the_browser`.

- 작업 피처 UI(리드 큐 ③ — 앱에 작업 피처 화면이 전혀 없어서 새로 만듦): 3D 모델 탭 "작업 피처" 그룹 "작업 축" → 대화상자(`gc_app/src/work_axis.rs`): 유형 선 위 / 선에 평행·점 통과 / 두 점 통과 / 두 평면의 교선 / 원의 중심 통과, 스케치 선·원(호)·점과 원점 평면을 목록(콤보)에서 고름 → 확인 = `CellOp::AddWorkFeature`(풀리지 않는 정의 — 같은 평면 두 번·길이 0인 선 — 는 백엔드가 거부하고 상태바가 설명). 브라우저 "작업 피처" 폴더(보이기 체크·삭제), 뷰는 보이는 작업 피처를 청록색으로 그림(`gc_viewport/src/workfeat.rs`: 축 = 긴 선+이름, 평면 = 윤곽 사각형, 점 = 십자). 근거: Inventor 2026 To Create Work Features(GUID-38A9748E-FA2D-44D3-928C-DFE1326A9385) 원문을 읽음 — 어셈블리의 작업 축 유형(On Line or Edge, Parallel to Line Through Point, Through Two Points, Intersection of Two Planes, Through Center of Circular Shape)과 같음, "Normal to Plane Through Point"는 원문이 파트 전용이라 뺌, Revolved Face는 모델 면이 없어 뺌. 원문과 다른 점·미구현: 형상을 뷰에서 클릭해 고르지 않고 목록에서 고름, 어셈블리 작업 피처는 원문에서 "브라우저 맨 아래" — 우리는 스케치 폴더 뒤, 원문은 어셈블리에서 투영 형상을 못 쓴다는데 백엔드는 받음, 작업 평면·작업 점 만들기 화면 없음(백엔드에 데이터는 있음), 편집(`SetWorkFeature`)·이름 바꾸기·컴포넌트 원점 평면/점 선택, 작업 피처 선택·강조. 스크린샷 `docs/evidence/img/review-52-work-axis.png`, 테스트 `work_axis::tests`, `a_work_axis_is_made_listed_hidden_and_deleted`.

- 서브어셈블리 폴더 우클릭 "서브어셈블리로 묶기"(QA 관찰 3, 리드 요청): 폴더 메뉴에도 강등 항목을 넣어 폴더 중첩이 끌어 놓기 없이 됨(`Demote{members:[폴더]}`). 테스트 `a_subassembly_folder_is_nested_from_its_menu`.

- 작업 피처 마무리(리드 새 1순위 ①): `work_axis.rs`를 `work_feature.rs`로 일반화 — 3D 모델 탭 "작업 피처" 그룹에 "작업 평면"·"작업 축"·"작업 점". 대화상자가 백엔드 `WorkGeometry` 전부를 다룸: 평면 = 평면에서 간격띄우기(평면 목록+거리 mm)·세 점; 축 = 선 위·선에 평행+점·두 점·두 평면 교선·평면에 수직+점(원문은 파트 전용이지만 백엔드에 있어 넣음)·원 중심; 점 = 목록에서 고르거나 좌표(mm) 입력. 목록에는 원점 평면 외에 작업 평면, 점 목록에는 작업 점이 들어가 작업 피처끼리 이어 만들 수 있음. 같은 대화상자가 기존 피처의 정의를 편집(`SetWorkFeature`): 브라우저 행 더블클릭·우클릭 "정의 편집", 뷰에서 더블클릭. 브라우저 행 우클릭: 정의 편집·보이기/숨기기·삭제. 뷰: 작업 피처를 클릭해 고르면(선 6 px 이내) 주황 굵은 선으로 강조(`Viewport::work_selected`), Delete로 삭제, Esc로 선택 해제; 작업 피처는 원점 요소·솔리드보다 먼저 집힘. 근거: Inventor 2026 To Create Work Features(GUID-38A9748E-FA2D-44D3-928C-DFE1326A9385) — 평면 유형 목록(Offset from Plane, Three Points 등)과 점 유형 목록(On Vertex/Sketch Point, Intersection of Three Planes 등)까지 읽음. 원문과 다른 점·미구현: 평면은 2종만(원문은 Parallel to Plane Through Point, Midplane Between Two Planes, Angle to Plane Around Edge, Two Coplanar Edges, Normal to Axis Through Point, 곡면 접평면 등 다수), 점은 "선택/좌표"만(원문은 정점·선 끝점/중점·세 평면 교점·두 선 교점 등; 좌표 입력은 원문에 없음), 형상을 뷰에서 클릭해 고르지 않고 목록에서 고름, 작업 피처 이름 바꾸기·선택 시 브라우저 행 강조·뷰 우클릭 메뉴(우클릭은 마킹 메뉴가 씀)는 없음. **미완: 어셈블 명령에서 작업 피처를 형상으로 고르기** — `Selection`이 컴포넌트 `Occurrence`의 프레임에서만 형상을 받아 어셈블리 최상위(작업 피처)를 가리킬 수 없음 → 백엔드 요청: 최상위 어셈블리를 가리키는 `Occurrence`(예: `Occurrence::Root`) 또는 `Geometry::Work(Id<WorkFeature>)`. 테스트 `work_feature::tests`(2), `work_features_are_edited_from_the_browser_and_selected_in_the_view`, `a_work_axis_is_made_listed_hidden_and_deleted`.

- 3D 모델·조립 탭 분할 버튼(리드 새 1순위 ②): 3D 모델 탭 스케치 그룹의 네 큰 버튼(XY/XZ/YZ/3D 스케치)을 분할 버튼 하나로(본체 = 마지막으로 시작한 스케치, ▾ = 평면 3종·3D), 조립 탭 편집 그룹(복귀·상위로·최상위로)을 분할 버튼 하나로, 위치 그룹의 컴포넌트 이동·회전을 분할 버튼 하나로(실행 중인 도구가 본체에 눌려 보이고 본체를 다시 누르면 끔). 분할 버튼 아이콘·이름·화살표 띠가 겹치지 않도록 간격 조정(아이콘 22 px, 이름 아래 16 px). 1280 px 단언: `every_ribbon_tab_fits_1280_px`가 모든 탭의 리본 버튼 오른쪽 끝이 창 안이고 3D 모델·조립 탭은 150 px 이상 여유(지금 291·832 px 지점에서 끝남)를 가짐을 확인. 테스트 수정: 분할 버튼으로 바뀐 버튼을 누르던 shell·sketch_shape_tools·QA(qa_assembly_ui의 Return 계열) 테스트는 각 파일의 `pick_variant`(▾ 포인터 클릭 → 변형 이름 클릭)로 바꿈. 원문 근거는 앞의 분할 버튼 항목과 같음(Inventor 2026 리본의 분할 버튼 동작은 전용 문서를 못 찾음). 미구현: 작업 피처 3버튼·구속/조인트도 분할로 묶기(원문 3D 모델 탭 Work Features 패널은 평면▾·축▾·점▾ 각각 유형 목록), 마지막 변형의 설정 저장. 스크린샷 `docs/evidence/img/review-53-assemble-tab.png`.

- 스케치 P1 잔여(리드 새 1순위 ③·④의 일부): ① Trim·Extend·Fillet 리본 버튼("자르기"·"연장"·"모깎기")을 스케치 탭 수정 그룹 작은 버튼 줄에 추가(`ToolKind::Trim/Extend/Fillet`; 반지름 상자는 백엔드 HUD `Radius` 필드가 처리). 폭을 위해 "절단 모서리 투영"을 큰 버튼에서 중심점 토글 아래 작은 버튼으로 옮김 — 스케치 탭에서 "스케치 마무리"가 1280 px 안에 40 px 이상 여유(테스트 단언). 원문 To Split, Trim, or Extend Curves(GUID-DD00037F-6BBA-4B95-AA90-4F99DCEB667D) 읽음: Split·Trim·Extend 모두 Modify 패널, 커서를 올리면 미리보기, 클릭, 우클릭으로 Split/Trim/Extend 전환·Done. 원문과 다른 점: Fillet이 원문은 Create 패널(우리는 수정 그룹), 우클릭 전환 메뉴는 없음(마킹 메뉴가 우클릭을 씀), Shift 전환 시 도구 이름 갱신·Fillet 반지름 상자 유지·미리보기는 백엔드가 하는 부분만 확인하지 않음(QA `p1_8…` 보류 테스트가 확인). ② 치수 편집 상자: 더블클릭하면 종속(참조) 치수도 열리고 "종속 치수" 체크박스로 `SetDriving` 토글(실행 취소 1단계, 구속 과다가 되는 되돌리기는 거부되어 상태바가 설명; 종속 치수는 표현식 칸 없이 체크박스만), `이름 = 값` 입력은 매개변수를 만들고(이미 있으면 값을 바꾸고) 치수가 그 이름을 참조 — `Command::AddParam/SetParam` 후 `edit_dimension`이라 실행 취소는 두 단계(백엔드가 한 단계로 묶어 주면 좋음), 숫자로 시작하는 이름·예약어는 거부되고 상자가 열린 채 남음. 근거 원문 To Add Dimensions(GUID-026FE8D3-AFBB-4834-BC18-73D9CFF78185)의 위 동작은 QA 보류 테스트 설명을 따름(원문 페이지 자체는 읽지 않음). ③ 단축키: 스케치 밖에서 C = 구속(어셈블리), P = 배치, 스케치 안 C = 원 유지 — 원문 Predefined Command Alias and Shortcut Reference(GUID-428B9A8E-E513-44C8-888A-F6048AE89F8E)는 C=조립 구속, D=치수, L=선, P=컴포넌트 배치, R=회전체(스케치 안 R은 우리 사각형이라 다름), S=스케치, T=프레젠테이션 트윅만 적혀 있고 **TR(자르기)·T 같은 스케치 별칭은 이 문서에 없어** 만들지 않음(QA 설명의 TR 근거 확인 불가). 테스트 `trim_extend_and_fillet_have_ribbon_buttons_and_c_means_constrain_outside_sketches`, `the_dimension_box_toggles_driven_and_driving`, `a_parameter_is_made_from_the_dimension_box`.

### 다른 항목 선택 (Select Other)

- 근거: 인벤터 도움말 Select Other (선택 항목 겹침 목록). 원문 GUID는 커밋 메시지 참고.
- 뷰 위에서 Ctrl+A를 누르면 커서 아래에 겹친 구성요소를 거리순으로 나열한다(`Picker::pick_all`, 중복 제거). 항목에 마우스를 올리면 강조, 클릭하면 선택하고 목록이 닫힌다.
- 증거: `docs/evidence/img/review-54-select-other.png`, 테스트 `select_other_lists_what_lies_under_the_cursor`.
- Ctrl+A 근거: 단축키 원문 GUID-428B9A8E-E513-44C8-888A-F6048AE89F8E에 "Ctrl + A Activates Select Other." (재확인). Shift+우클릭은 Select 명령 메뉴(아래)를 연다. 일반 우클릭(마킹 메뉴) 목록에도 "다른 항목 선택" 항목이 있다.
- 원문과 차이: 보통 우클릭 메뉴(마킹 메뉴)에는 "다른 항목 선택" 없음, 멈춤 지연 설정 없음, 화살표 키 순환 없음, 구성요소만 대상.

### 그립 스냅 (Grip Snap)

- 근거: 인벤터 도움말 Grip Snap move or rotate, GUID-E1CA109A-D5A1-4131-AC3A-AE217716DD8C (백엔드 `grip_snap_pose` / `grip_allows` / `grip_snap_commit`).
- 조립 탭 > 위치 > "그립 스냅". 흐름: 구성요소의 모서리 클릭(기준선) → "축을 따라 이동"/"선 기준 회전" 선택 → 시작점 클릭 → 끝점 위로 움직이면 미리보기, 클릭하면 확정 → 완료 / 접지 후 완료 / 취소(Esc·우클릭). 완료는 Undo 한 단계.
- HUD 줄: `grip_allows`로 남은 자유도가 이 동작을 허용하는지 초록/빨강으로 표시.
- 이동은 시작점을 지나는 기준선과 평행한 선 위에서 커서에 가장 가까운 점을 끝점으로 써서 커서를 따라가고, 회전은 시점 평면 위의 점을 쓴다.
- 미리보기는 세션 미리보기를 쓰므로 `CellUi::hold_previews`로 속성 패널이 마우스를 뗀 순간 커밋하지 않게 했다. 그립 스냅 중에는 이동/회전 핸들(기즈모)을 끈다.
- 증거: `docs/evidence/img/review-55-grip-snap.png`, 테스트 `grip_snap_moves_a_component_along_a_picked_edge`.
- 원문과 차이: (점·면 기준은 아래 항목) 인벤터의 화면 내 조작 핸들·HUD 아이콘 대신 상단 패널, 시작점은 표면 클릭(스냅 점 보조 없음), 여러 구성요소 동시 조작 없음.

### 작업 피처를 어셈블/구속의 뷰 선택으로

- 근거: To Place Mate or Flush Constraints, GUID-0F500089-4F69-41C5-AFE8-B58AC9E8E15E ("Select the first point, curve, face, or work feature"). 백엔드 `work_feature_selection(id)`.
- 어셈블 명령과 구속 대화상자가 뷰에서 보이는 작업 피처(축 선 / 평면 테두리 / 점 십자)를 누르면 `work_feature_selection`을 첫째·둘째 선택으로 쓴다. 작업 피처가 원점 기하보다 먼저 잡힌다. 선택 이름은 작업 피처 이름(예: `Point1 · 점`).
- 테스트 `a_work_feature_clicked_in_the_view_fills_the_assemble_selection`.
- 원문과 차이: 조인트·패턴·복사·미러 대화상자는 원점 기하만 받는다(작업 피처 선택 연결 없음). 곡선·면 선택은 아직 없음.
- 같이: Shift+우클릭 = Select 명령 메뉴(아래 항목).

### `이름 = 값` 한 단계 Undo

- 치수 상자의 새 매개변수 이름은 백엔드 `edit_dimension_with_param`(한 Txn)으로 만든다. 이미 있는 이름은 값 변경(SetParam) 후 치수 연결의 두 단계로 남는다(백엔드 API가 추가만 지원). 테스트 `a_parameter_is_made_from_the_dimension_box`가 한 번의 Undo로 매개변수와 연결이 함께 사라지는지 확인.

### Select 명령 메뉴 (Shift+우클릭)

- 근거: 단축키 원문 GUID-428B9A8E-E513-44C8-888A-F6048AE89F8E "Shift+ right-click Activates the Select command menu."; 선택 모드는 GUID-E3E49BD5-3F55-4836-8CD1-2C1855CF9986(Select Bodies/Features/Faces/Sketches 등), 선택 방법 GUID-B8F6E805-2E4B-4366-8FF6-33F0ED1C1C4A("right-click, choose Select Other").
- 메뉴: 구성요소 우선(기본) / 작업 피처 우선 + 다른 항목 선택. 작업 피처 우선에서는 평범한 클릭이 작업 피처·원점 기하만 잡고 본체는 선택하지 않는다(`Viewport::select_priority`). 마킹 메뉴(일반 우클릭) 목록에도 "다른 항목 선택".
- 테스트 `shift_right_click_opens_the_select_menu`, `the_work_feature_priority_leaves_bodies_unselected`.
- 원문과 차이: 원문의 모드(본체·피처·면·스케치 선택, 부품 우선 등)는 앱에 해당 선택 대상이 없어 둘만 둔다. 상태 표시(어느 우선순위인지)는 메뉴에서만 보인다. 면과 모서리 선택은 측정 명령에서만.

### 조인트 한계 (Limits 탭)

- 근거: To Define and Manage Joint Relationships, GUID-21DC3336-5C51-42C1-90FB-4299CD66E0C6 (Limits 탭: Start / Current / End). 백엔드 `set_joint_limits` / `joint_limits` / `joint_position`.
- 브라우저 조인트 행 우클릭 > "한계…" 로 "조인트 한계" 창. 회전 조인트는 각도(°), 슬라이더는 선형(mm), 원통형은 둘 다(`takes_limits`). 각 한계는 체크로 켜고 시작·끝, "현재"를 체크하면 그 값에 고정. 현재 위치(`joint_position`)를 창에 표시. 확인/적용/취소. 값이 숫자가 아니거나 시작>끝, 현재가 범위 밖이면 버튼이 막힌다.
- 한계가 있는 조인트 행은 이름 뒤에 `+/-`. 구동(Drive) 대화상자의 시작·끝 기본값이 조인트 한계(시작·끝)가 된다.
- 증거: `docs/evidence/img/review-56-joint-limits.png`, 테스트 `a_rotational_joint_gets_angular_limits_from_its_dialog`.
- 원문과 차이: 원문은 조인트 편집 대화상자의 탭이지만 우리는 조인트 만들기 대화상자에 탭이 없어 별도 창으로 둔다. 슬라이더·원통형 입력은 같은 코드지만 테스트는 회전 조인트만. 한계에서 막힌 위치를 뷰에 그리지 않는다.

### 그립 스냅: 점·면 기준

- 근거: Grip Snap Move Options Bar Reference, GUID-C030C32C-7E1E-4F37-BBDF-120412898D00 (백엔드 `grip_snap_pose_on` / `grip_snap_allowed`, `GripReference::{Point, Line, Plane}`).
- 첫 선택으로 모서리(선), 꼭짓점(점), 평평한 면(평면)을 고른다. 이동 옵션은 기준에 따라: 점 = 점 기준 이동 / 점 기준 회전, 선 = 축을 따라 이동 / 선 기준 회전, 면 = 면 안에서 이동 / 법선 방향 이동. 법선·선 방향 이동은 끝점이 커서를 따라가고 나머지는 시점 평면의 점을 쓴다. HUD는 두 점이 있으면 `grip_snap_allowed`로 판정.
- 테스트 `grip_snap_moves_a_component_along_the_normal_of_a_picked_face`.
- 원문과 차이: Free Drag·Free Rotate(참조 기하 없는 옵션), 두 번째·세 번째 선택으로 점·선·면을 고르는 방식(두 점으로만), 원형 모서리·곡면·NURB 첫 선택(원통 면은 선택 불가), 오프셋 값 입력은 없다. 꼭짓점은 측정 명령의 점 줍기(8 px)를 쓴다.

### 서브어셈블리를 통째로 이동

- 근거: Create Subassemblies in Place, GUID-FCEE3F0E-AAE2-4619-8645-2588FEECD353 (서브어셈블리는 부모 안에서 하나의 단위). 백엔드 `subassembly_moved` / `move_subassembly`.
- 자유 이동·자유 회전: 서브어셈블리에 속한 구성요소를 끌면 서브어셈블리 전체가 움직이고(`delta = 새 포즈 × 시작 포즈⁻¹`) 놓을 때 Undo 한 단계. 접지된 구성요소가 있는 서브어셈블리는 끌려가지 않는다(드래그 시작 안 함). 그립 스냅: 서브어셈블리의 구성요소를 잡으면 확정 때 나머지도 함께 이동(접지 후 완료는 모두 접지).
- 같이 고침: 자유 이동/회전이 켜져 있으면 이동·회전 핸들(기즈모)을 끈다. 선택한 구성요소의 핸들 위로 끌면 핸들이 드래그를 가로채 자유 이동이 끝나지 않던 버그.
- 끄는 동안 서브어셈블리 전체가 `preview_components`로 미리보기되고, 놓을 때 `end_component_preview` 후 `move_subassembly`(문서·Undo는 놓을 때만).
- 원문과 차이: 핸들(기즈모) 드래그와 속성 패널 위치 입력은 서브어셈블리 단위가 아니다. 그립 스냅의 미리보기는 잡은 구성요소만 보이고 나머지는 확정 때 따라온다.
- 테스트 `free_move_drags_a_whole_subassembly`, `grip_snap_moves_the_whole_subassembly_of_the_picked_component`.

### Trim 미리보기 (지울 조각 강조)

- 근거: Trim/Extend/Split 원문 GUID-DD00037F(원문 목록은 앞 항목 참고): 지워질 부분이 클릭 전에 강조된다.
- `Preview.cut`(Trim이 지울 조각)을 스케치 뷰에서 빨간 점선(2.6 px)으로 그린다. `Piece::polyline`으로 선분·호·원·타원호(`EllipseArc`)를 모두 그린다.
- 증거 `docs/evidence/img/review-57-trim-preview.png`, 테스트 `trim_highlights_the_piece_that_would_go`.

### 단축키: F5 / Shift+F5, S

- 근거: 단축키 원문 GUID-428B9A8E-E513-44C8-888A-F6048AE89F8E ("F5 Returns to the previous view", "Shift + F5 Switches to the next view", "S Creates a 2D sketch on a face or plane").
- F5 이전 뷰 / Shift+F5 다음 뷰: 카메라가 0.35초 이상 멈춘 자세를 기록(50개)해 오간다. S(스케치 밖): 선택한 원점 평면·작업 평면, 없으면 XY에 2D 스케치를 만들고 선 도구로 시작. 테스트 `f5_walks_the_views_and_s_makes_a_sketch`.
- 아직 없는 원문 단축키: E·H 등 파트 모델링 명령(앱에 해당 기능 없음), Alt+F8·Alt+F11(매크로·VBA), Spacebar(3D 회전 전환), F1 도움말.

### 단축키: Tab / Shift+Tab, Alt+드래그, Ctrl+C/V, F10

- 근거: 단축키 원문 GUID-428B9A8E-E513-44C8-888A-F6048AE89F8E ("Tab Demote", "Shift + Tab Promote", "Alt + drag mouse: In assemblies, applies a mate constraint", "F10 Sketch visibility", Windows 키 Ctrl+C/V), Alt+드래그 본문 GUID-0F500089-4F69-41C5-AFE8-B58AC9E8E15E ("To drag a component into position and place a Mate constraint").
- **Tab**: 선택한 구성요소의 Demote(브라우저 메뉴와 같이 새 서브어셈블리 이름 입력 줄), **Shift+Tab**: 한 단계 위로 Promote(소속 서브어셈블리의 부모로 이동, 최상위면 최상위로). 테스트 `tab_demotes_and_shift_tab_promotes_the_selected_component`.
- **Alt+드래그**: Alt를 누른 채 구성요소의 면·원통을 눌러 다른 구성요소의 면·원통으로 끌어 놓으면 미리보기 선(초록=가능, 회색=불가)이 보이고, 놓으면 `possible_relations`의 첫 해(평면끼리 Mate, 원통끼리 축 Mate)를 오프셋 0으로 만든다(풀 수 없으면 구속 오류 대화상자). 테스트 `alt_drag_places_a_mate_between_two_faces`. 원문과 차이: 끄는 동안 구성요소 자체는 움직이지 않고 놓을 때 해에 따라 이동하며(원문은 접근하는 위치를 미리 보여 준다), 평면·원통 외 형상(모서리·점)과 평면의 Flush 자동 선택은 없다.
- **Ctrl+C / Ctrl+V**: Ctrl+C는 선택한 구성요소(본체·로봇과 툴·가져온 어셈블리)를 `copy_to_clipboard`로 기억한다. Ctrl+V는 아직 아무것도 만들지 않고 복사본의 반투명 고스트가 커서를 따라다닌다(`start_paste`, 매 프레임 `preview_paste(offset)`, 증거 `docs/evidence/img/review-64-paste-ghost.png`); 클릭하면 클릭한 점과 복사본 첫 구성요소 위치의 차이를 offset으로 `paste(offset)`을 한 번만 호출해(관계 없이, 같은 방향) **Undo 한 단계**로 붙여넣는다. Esc·우클릭은 `end_paste_preview`만 부른다. 원문 GUID-249C50F9-5A8C-48BD-8341-D1BDB7A16B19("Use Copy and Paste to place new instances using the same orientation"). 테스트 `ctrl_c_and_ctrl_v_paste_a_copy_of_the_selected_body`. 원문과 차이: 서브어셈블리 행 자체를 복사하는 키 경로는 없다(API는 지원), 한 번 붙이면 한 번만 놓인다.
- **F10**: 편집 중인 스케치(없으면 마지막 스케치)의 가시성을 토글. 테스트 `f10_toggles_the_visibility_of_the_latest_sketch`.

### 리본 아이콘 정리

- 3D 모델 탭의 작업 평면·작업 축·작업 점과 조립 탭의 그립 스냅이 모두 같은 "이동(Pan)" 아이콘이어서 구분이 안 됐다. 각각 직접 그린 아이콘(`Icon::WorkPlane` / `WorkAxis` / `WorkPoint` / `GripSnap`, 인벤터 그림 사용 안 함)으로 바꿨다. 증거 `docs/evidence/img/review-58-*.png`.
- 큰/작은 버튼 혼합: 7개 탭을 모두 렌더해 보니 큰 버튼 탭(3D 모델·조립·로봇·검사·관리·뷰)과 큰+작은 혼합 탭(스케치: 작성·수정·구속)이 인벤터처럼 역할별로 나뉘어 있고 폭도 1280 px 안에 들어간다(`every_ribbon_tab_fits_1280_px`). 분할 버튼은 아래쪽 화살표 때문에 이름이 한 줄 위에 앉는 점이 인벤터와 같다. 따로 더 합칠 항목은 찾지 못했다.

### 구속 한계 / 조인트 한계 탭 / 리본 큰·작은 버튼

- **구속 대화상자 "더 보기"**: 이름, 최대·최소(체크+값, mm), "오프셋을 휴지 위치로" → `Relationship.limits`. 원문 GUID-0F500089-4F69-41C5-AFE8-B58AC9E8E15E("To change the name of the constraint or set limits, click the More button… Check Maximum… Check Minimum… Use Offset As Resting Position"). 최소>최대나 숫자가 아닌 값이면 확인이 막힌다. 테스트 `the_constrain_dialogs_more_section_sets_the_name_and_limits`. 원문과 차이: 값은 오프셋(mm)만, 각도 구속의 한계는 아직 UI 없음, 체크를 풀어도 값이 유지되는("inactive state") 동작은 대화상자를 닫으면 사라진다.
- **조인트 대화상자에 "조인트 / 한계" 탭**(GUID-21DC3336-5C51-42C1-90FB-4299CD66E0C6 Limits 탭): 만들 때 한계 탭의 값은 조인트가 만들어진 뒤 `set_joint_limits`로 함께 저장. 브라우저 우클릭 "한계…"는 같은 대화상자를 한계 탭으로 연다(이미 있는 조인트는 원점·종류 읽기 전용, 한계만 수정). 별도 "조인트 한계" 창은 없앴다. 테스트 `a_rotational_joint_gets_angular_limits_from_its_dialog`, `the_joint_dialogs_limits_tab_goes_with_a_new_rotational_joint`.
- **리본 큰/작은 버튼**: 인벤터처럼 패널마다 가장 쓰는 명령은 큰 버튼, 나머지는 아이콘+이름 작은 버튼 세로 묶음(`gc_ui::ribbon_stack`, 최대 3줄). 조립 탭: 배치 / 패턴 / 구속·조인트는 큰 버튼, 교체 / 미러·복사 / 어셈블·구동 / 그립 스냅·접지는 작은 버튼. 3D 모델 탭: 작업 평면 큰 버튼, 작업 축·점 작은 버튼. 1280 px 단언은 그대로 통과. 증거 `docs/evidence/img/review-59-*.png`. 접근성 이름은 이전과 같다(버튼 이름 = 라벨).
- 스케치 탭은 원래 큰+작은 혼합 구성이라 그대로 두었다.

### 열기 경고 토스트 / 스플라인 화면 점검

- 파일을 열 때(열기·복구 모두) `Session::load_warnings()`가 비어 있지 않으면 상태 표시줄에 "열었지만 쓸 수 없는 데이터 N건을 버렸습니다: …"를 보인다(수리 안내보다 우선). 테스트 `opening_a_file_with_a_damaged_joint_range_reports_the_warning`(조인트 한계 시작>끝으로 손상시킨 파일).
- 스플라인(보간): 스케치 탭 선▾에서 고르고, 클릭으로 맞춤점, Enter로 마침 — 점들을 지나는 매끄러운 곡선이 그려진다(테스트 `the_spline_tool_is_in_the_line_split_button_and_draws_a_curve`, 증거 `docs/evidence/img/review-60-spline.png`). 이상 없음. 참고: 아이콘은 글자 "~"(전용 아이콘 아님).

### 글자 아이콘을 벡터로 / 로봇·검사·뷰 탭의 큰·작은 버튼

- 스케치 탭의 글자 아이콘(`-+-`, `~`, `(/)`, `[+]`, 슬롯 변형, 분할·이동·축척·트림·모깎기·모따기·복사·회전·늘이기·연장 등 20개)을 직접 그린 벡터 `Icon`으로 교체했다(인벤터 그림 사용 안 함). 구속 기호(`::`, `--`, `(o)` 등)와 `fx`, `x`는 글자 그대로 둔다(인벤터도 구속 기호는 글자 모양 아이콘).
- 로봇 탭: 로봇 추가·경로 만들기·계획·재생은 큰 버튼, 가져오기 2개 / 모양 3개 / 웨이포인트 추가 3종 / 좌표계·스케치 경로는 작은 버튼 묶음. 검사 탭: 측정·간섭 분석 큰 버튼, 접촉 솔버 작은 버튼(`ribbon_stack_responses`로 메뉴 유지). 뷰 탭: 홈·전체 보기·자유도·단면 삭제 큰 버튼, 정면·평면·우측 / 직교·원근 / 단면 3종 / 패널 토글은 작은 버튼. 1280 px 단언 유지. 증거 `docs/evidence/img/review-61-ribbon-tabs.png`.
- 접근성 이름은 그대로(버튼 이름 = 라벨).

### Flexible 서브어셈블리 / 스플라인 분할 / 타원·스플라인 간격띄우기

- **Flexible**: 원문 Create and edit flexible components, GUID-30817F08-A5C2-44B7-B237-4791ACA3F854("Right-click the placed subassembly in the graphics window or the browser, and then select Flexible ... An icon denoting the flexible state marks the subassembly in the browser ... clear the check mark"). 브라우저 서브어셈블리 행 우클릭 메뉴에 "유연" 체크(`SubassemblyOp::SetFlexible`, Undo 1단계), 유연한 행에는 용수철 아이콘(`Icon::Flexible`)이 오른쪽에 붙는다. 그래픽 창 우클릭 마킹 메뉴 목록에도 "유연"(현재 상태면 ✓)이 서브어셈블리 소속 구성요소에서 나온다. 자유 이동/회전과 그립 스냅은 `Session::unit_moves`로 강체 서브어셈블리만 통째로 움직이고(미리보기도 `preview_components`), 유연한 서브어셈블리는 구성요소별로 끌린다. 테스트 `flexible_subassemblies_are_dragged_component_by_component`(+ 기존 통째 이동 테스트).
- **스플라인 분할**: 원문 GUID-DB15C479-66F2-456A-9E54-40A60F83DC92("To split a spline, right-click a point and choose Split Spline"). 스케치에서 스플라인의 안쪽 맞춤점을 우클릭(멈춤 메뉴 모드)하면 목록에 "스플라인 분할"이 나오고 `SketchOp::SplitSpline`을 실행한다. 테스트 `the_right_click_menu_of_a_spline_fit_point_splits_the_spline`. 차이: 끝점에서는 항목이 나오지 않는다(백엔드가 거부), 핸들 재설정 메뉴는 없다.
- **간격띄우기**(GUID-D489CE6D…): 타원·스플라인도 받는다. 화면 점검 — 안쪽 타원과 스플라인 사본이 그려진다(`docs/evidence/img/review-62-offset.png`, 테스트 `offset_works_on_an_ellipse_and_a_spline_on_screen`). 이상 없음.

### 브라우저 동작 점검 (구성요소 메뉴·행 아이콘·찾기)

- 원문: Working with the Assembly Browser GUID-5F975252-BDB4-488C-A2C5-8B15C7D3C641(브라우저 메뉴: Find, Edit Values in Browser, Display Preferences = Hide Work Features / Hide Fully Constrained / Show Constraint State …, 우클릭 메뉴는 구성요소·관계에 대한 작업), Assembly Browser Reference GUID-CB6B1637-71AD-4753-8CC9-19559BFEE3A5(접지 = 압정 아이콘 변형, Disabled·Invisible 변형, 구속 상태 점).
- 점검 결과 이미 있던 것: 가시성 체크, 억제(+흐린 이름), 이름 바꾸기, 삭제, 제자리 편집(더블클릭), 구속 상태 점(검은/속 빈/대시), 관계 행의 잠금 아이콘·+/-.
- 이번에 채운 것: ① 구성요소 행 우클릭 메뉴에 **활성(Enabled)**·**접지(Grounded)** 체크 항목(`Command::SetComponent`, 항목 순서: 메뉴 고유 항목 → 이름 바꾸기 → 가시성 → 활성 → 접지 → 억제 → 삭제). ② 접지된 구성요소 아이콘 모서리에 **압정**(`Icon::Pin`), 비활성(Disabled)이나 숨긴(Invisible) 구성요소 이름은 배경 스타일(흐림). ③ 브라우저 위 **찾기** 상자(이름의 일부, 대소문자 무시): 구성요소·서브어셈블리 폴더(이름이 맞으면 안의 것 전부)·스케치·관계 행만 남긴다. 테스트 `the_browser_menu_enables_and_grounds_components_and_find_filters_the_rows`, 증거 `docs/evidence/img/review-63-browser-pin.png`.
- 아직 없는 것(앱에 대상이 없거나 보류): 적응(Adaptive)·BOM 제외 메뉴(앱에 적응성·BOM 없음), 사용자 정의 폴더, 찾기의 조건 검색(문제 있는 관계·억제된 항목 등 조합), Hide UCS·Hide Notes·Hide Documents·Show Children Only·Show Extended Names(대상 없음).

### 브라우저 메뉴: Display Preferences / Edit Values in Browser

- 원문: Working with the Assembly Browser GUID-5F975252-BDB4-488C-A2C5-8B15C7D3C641("Hide Work Features turns the display of all work features and the Origin folder off or on", "Hide Fully Constrained only displays components that have constraint issues", "Show Constraint State displays all constraint status", "Edit Values in Browser turns the display of the browser edit box off or on ... used for changing or measuring constraint geometry").
- 찾기 상자 옆 "…"(접근성 이름 "브라우저 메뉴") 메뉴에 네 개 체크: **작업 피처 숨기기**(문서·구성요소 원점 폴더와 작업 피처 목록을 감춤), **완전 구속 숨기기**(완전 구속 구성요소·관계 행을 감춤), **구속 상태 표시**(점 표시, 기본 켬), **브라우저에서 값 편집**(기본 켬). 값 편집: 구속 행을 클릭하면 그 아래 값 상자("관계 값", 오프셋 mm / 각도 °)가 나타나고 Enter로 `SetRelationship` 적용(Undo 1단계). 조인트·강체 같은 값 없는 관계는 상자가 없다. 테스트 `the_browser_menu_hides_work_features_and_edits_a_constraint_value_in_place`.
- 원문과 차이: 값을 바꾸면 인벤터처럼 상자가 닫히지 않고 선택이 유지된다, 측정 전용 동작(값 읽기)은 없다. 설정은 세션 동안만 유지(파일에 저장하지 않음).

### 작업 평면·점 종류 확장

- 원문: To Create Work Features, GUID-38A9748E-FA2D-44D3-928C-DFE1326A9385. 작업 평면 대화상자: 평면에서 간격띄우기 / 세 점 / **평면에 평행·점 통과 / 두 평면의 중간면 / 평면에 대한 각도(선 기준) / 선과 점 / 선에 수직·점 통과 / 곡선에 수직·점 통과**. 작업 점 대화상자: 점 / **두 선의 교차 / 평면과 선의 교차 / 세 평면의 교차 / 곡선의 중심**. 작업 축의 "원 중심 통과"는 타원도 받는다. 선은 스케치 선·작업 축·조립 원점 축·구성요소 원점 축(`LineRef`) 목록에서 고른다. 편집 시 정의를 대화상자로 되읽고(`_ => return` 정리), 풀 수 없는 정의는 문서가 거부해 상태 표시줄에 이유가 나온다.
- 테스트: `midplanes_and_intersection_points_are_made_and_read_back`(대화상자 단위), `the_work_plane_dialog_offers_the_newer_definitions_and_makes_a_midplane`, 증거 `docs/evidence/img/review-65-work-plane-kinds.png`.
- 원문과 차이: 대상은 뷰에서 클릭하지 않고 목록에서 고른다(앞 항목과 같음). 어셈블리에서 인벤터가 허용하는 종류 제한은 두지 않았다.

### 스플라인: 점 삽입 / 곡선 닫기, 간격띄우기 선택 강조, 열린 스플라인 연장

- 원문 GUID-DB15C479-66F2-456A-9E54-40A60F83DC92("To close a curve, right-click the starting point and choose Close Curve", "right-click the spline and choose Insert Point. Then click the curve"). 스케치에서 스플라인 위를 멈춤 우클릭(누르고 떼기)하면 목록에: 안쪽 맞춤점 위 = "스플라인 분할", **시작 맞춤점 위(열린 스플라인, 맞춤점 3개 이상) = "곡선 닫기"**(`SketchOp::CloseSpline`), **곡선 위 = "점 삽입"**: 고르면 곡선을 클릭할 때마다 가장 가까운 자리에 맞춤점을 넣고(`InsertSplinePoint`, 클릭마다 Undo 한 단계) 위쪽에 안내가 보이며 Esc로 끝낸다. 테스트 `the_spline_menu_inserts_points_and_closes_the_curve`.
- 간격띄우기 등 도구가 고른 곡선(`Preview.picks`의 `Item::Curve`)을 이제 굵은 미리보기 색으로 그린다(전에는 점만 그렸다) — 타원·스플라인의 선택이 보인다(`review-66-offset-pick.png`). 열린 스플라인 연장: 끝쪽에 커서를 대면 경계까지의 선이 미리보기되고 클릭하면 새 끝 맞춤점이 경계에 묶여 이어진다(`review-67-extend-spline.png`). 테스트 `offset_highlights_its_pick_and_extend_takes_an_open_spline_to_a_boundary`.
- 원문과 차이: 점 삽입 중 커서 옆에 삽입 위치 미리보기는 없다, 곡선 닫기/점 삽입/분할의 핸들 재설정 메뉴(Reset Handle)는 없다.

### 선-호 / 호-호 치수 화면 점검

- 원문 GUID-026FE8D3-AFBB-4834-BC18-73D9CFF78185(선과 호 자체 사이 거리, 두 호 사이 거리). 치수 도구로 선 → 원 테두리 → 글자 위치를 클릭하면 `DimKind::LineArc`가 만들어진다(테스트 `the_dimension_tool_measures_a_line_to_a_circle_itself_on_screen`, 증거 `docs/evidence/img/review-68-arc-distance.png`).
- 발견: 원 테두리를 차례로 두 번 클릭해 두 원 사이 거리(`ArcArc`)를 만들려 하면 도구는 두 번째 원의 지름 치수를 만든다 — 도구가 호-호 두 선택을 아직 받지 않는 것으로 보인다(`gc_tools`). 백엔드 확인 요청.

### Look At 연결, 3D Bend 버튼

- 근거: Inventor 2026 도움말 GUID-3B926737-92BA-46CF-B722-97F68651772B (Look At), GUID-4F861674-F74C-474E-869A-4C190ED6EC4B (Bend).
- Look At: 뷰 탭 "면 보기" 큰 버튼, 마킹 메뉴, 탐색 막대, PageUp 모두 `Viewport::look_at`로 연결(작업 피처·원점 평면·활성 스케치/선택 곡선). 마킹 메뉴 항목은 항상 활성(알려진 차이).
- Bend: 3D 스케치 그리기 패널의 작은 버튼. 선 두 개를 클릭 → 반지름 입력(기본 10 mm) → Enter로 `SketchOp::Bend`. 실패하면 상태 표시줄에 사유, 명령 유지. Esc로 종료. 호 미리보기는 없음(알려진 차이).
- 테스트: `look_at_turns_the_view_square_to_the_sketch_plane`, `the_3d_draw_panel_has_a_bend_button_that_esc_ends`.

### 3D 스플라인 버튼

- 근거: Inventor 2026 도움말 GUID-DB15C479-66F2-456A-9E54-40A60F83DC92 (보간 스플라인은 3D 스케치에서도 만들 수 있음).
- 3D 스케치 그리기 패널의 선 분할 버튼에 "스플라인(보간)" 추가. 뷰포트 클릭마다 `spatial_click`로 맞춤점, Enter로 `spatial_finish`, 진행 중 곡선은 `spatial_spline_preview`로 미리보기. Esc는 기존 도구 취소.
- 테스트: `a_3d_spline_is_placed_with_fit_points_and_finished_with_enter`.

### Auto-Bend 옵션, 두 원 치수 재확인

- 근거: Inventor 2026 도움말 GUID-1898C432-CFD2-46F1-8BAE-631392DFBF1D (Sketch 탭, 3D Sketch "Auto-bend with 3D line creation": 기본 꺼짐, 3D 선을 그릴 때 접하는 모서리 굽힘을 자동 배치), GUID-58DD9A89.
- 응용 프로그램 옵션 스케치 탭에 "3D 선 그릴 때 자동 굽힘" 체크 + 굽힘 반지름 칸(기본 10 mm). 설정 파일(`auto_bend`, `auto_bend_radius`)에 저장되고 다음 실행에서 `Session::set_auto_bend`로 복원.
- 원문과 차이: 원문 옵션은 체크 상자뿐이다. 반지름 칸은 리드 요청으로 추가.
- 겹친 두 원: 백엔드 테스트(`sketch_arc_distance`, 5개)가 리드 브랜치에서 통과 — 치수 도구 경로는 UI도 동일.
- 테스트: `the_auto_bend_option_is_off_by_default_saved_and_read_back`.

### 3D 스케치 Helical Curve 대화상자

- 근거: Inventor 2026 도움말 GUID-37BA1DCE-6BC4-41CD-A36D-577F31B304FC (To Draw 3D Curves, Draw a Helical Curve).
- 3D 스케치 그리기 패널의 "나선 곡선" 버튼 → 대화상자(정의: 피치 및 회전 / 회전 및 높이 / 피치 및 높이 / 나선형, 지름, 피치·회전·높이, 테이퍼(나선형 제외), 왼손·오른손) → 확인 → 뷰포트에서 시작점, 끝점(축 방향) 클릭 → `SketchOp::AddHelix`. 값은 `evaluate_value`로 해석(단위·매개변수 가능). 시작~끝 점 사이 고무줄 선을 보여 준다. 실패하면 명령 유지, Esc 종료.
- 원문과 차이: Variable helical curve(표), Helix Ends 탭(Natural/Flat 전이), 지름 생략(자동 계산)은 없음 — 지름은 필수(기본 20 mm). 끝점은 축 방향만 정한다(높이는 값에서).
- 테스트: `the_helical_curve_dialog_then_two_clicks_make_a_helix`.

### 스플라인 점 삽입 보정 (QA 관찰 2건)

- 점 삽입 모드는 곡선에서 픽 허용 거리(`CURVE_PX`) 안을 클릭할 때만 점을 넣는다(원문 "곡선을 클릭"). 멀리 누르면 무시.
- 안내 글자는 그려진 텍스트 대신 레이블(Area)이라 접근성 노드가 있다 → 화면 테스트에서 모드 확인 가능.
- 테스트: `the_spline_menu_inserts_points_and_closes_the_curve`에 안내 레이블·먼 클릭 무시 단언 추가.

### 뷰 탭: 시각 스타일(Visual Style) 분할 버튼

- 근거: Inventor 2026 도움말 GUID-8D59A8BF-49A8-4214-A3A7-F184DB73E68F (About Visual Styles in Models: View 탭 Appearance 패널에서 스타일 선택), 그림자·바닥 GUID-3395B47F-7D8B-47B4-ACDD-0A9489948DB4 / GUID-AC7970C5-72DB-409E-AED4-96B9C5E6C4A0.
- 외형 패널의 분할 버튼: 음영 처리(모서리 없음) / 모서리가 있는 음영 처리(기본, 현재 렌더) / 와이어프레임(면 없이 모서리 전체). 면·모서리 그리기 여부만으로 되는 것이라 렌더러 변경 없이 `VisualStyle`(gc_viewport)로 처리.
- 렌더러 지원이 필요한 나머지: 숨은 모서리 표시(음영/와이어프레임 + 숨은 모서리 점선·옅은 색: 깊이 반전 패스), 가시 모서리만 있는 와이어프레임(색 쓰기 없는 깊이 전용 패스), 사실적(재질), 흑백·수채화·스케치 일러스트.
- 테스트: `the_visual_style_drop_down_switches_shaded_edges_and_wireframe`.

### 뷰 탭: 개체 가시성(Object Visibility) 드롭리스트

- 근거: Inventor 2026 도움말 GUID-BA193171-FAFC-4150-9222-CC43D8D4AA54 (View 탭 Visibility 패널 Object Visibility: 드롭리스트에서 체크 해제=끄기, 선택=켜기, 같은 종류 전체에 적용. 컨텍스트 메뉴 가시성과 충돌하면 개별 설정이 우선이라는 주의 문구).
- 구현: `ribbon_drop_button`(gc_ui, 체크 클릭에는 열린 채, 바깥 클릭에 닫힘) + `ObjectVisibility`(gc_viewport). 종류: 원점 평면·축·점, 작업 평면·축·점, 스케치(비활성 2D), 3D 스케치, 구속조건 기호. 그리기와 선택(pick) 둘 다 같은 필터를 따른다.
- 의미: 작업 피처·스케치는 기본 켜짐이고 끄면 전부 숨김(개별로 숨긴 것은 그대로 숨김). 원점 종류는 기본 꺼짐(Inventor 원점 폴더처럼)이고 켜면 모든 컴포넌트의 해당 원점 요소가 개별 표시에 더해 보인다.
- 원문과 차이: 이 설정은 실행 중에만 유지(문서 저장 안 됨). UCS·스케치 치수·와이어 등 우리에게 없는 종류는 없음. 스케치 치수 일괄 표시/숨김(GUID-026FE8D3)은 구속 기호와 별개이므로 아직 없음.
- 테스트: `the_object_visibility_drop_list_switches_kinds_on_and_off`; 기존 `qa_assembly_ui` 15개 통과.

### 뷰 탭: 바닥 평면, 패널 순서(Visibility · Appearance · Windows · Navigate)

- 근거: Inventor 2026 도움말 GUID-AC7970C5-72DB-409E-AED4-96B9C5E6C4A0 (바닥 평면의 켜기/끄기는 View 탭 Appearance 패널, 세션 단위), GUID-3395B47F-7D8B-47B4-ACDD-0A9489948DB4 (그림자 목록도 Appearance 패널).
- 바닥 평면 버튼 = 바닥 격자(`Viewport::show_ground`). 스케치를 편집하는 동안에는 스케치 자체의 격자가 남는다. 바닥 평면 설정(위치·불투명도·색·반사)과 문서 저장은 없음.
- 그림자(전체/바닥/물체/주변)는 렌더러에 그림자 맵이 없어 없음 — 렌더러 작업이 필요하다(리드에 보고).
- 패널 순서: 가시성(개체 가시성, 자유도) · 외형(시각 스타일, 바닥 평면, 투영, 단면) · 창(패널 반투명, 어두운 테마) · 탐색(홈, 표준 뷰, 전체 보기, 면 보기). 투영 패널은 외형으로 합쳤고, "표준 뷰"는 "탐색", "패널"은 "창"으로 이름을 바꿨다. 단면(Section)을 외형에 둔 것은 이전 배치를 유지한 것(원문 위치 미확인).
- 테스트: `the_ground_plane_button_switches_the_floor_grid`, `the_view_tab_panels_run_visibility_appearance_windows_navigate`; shell 106개 통과.

### 렌더러: 숨은 모서리 패스와 깊이 전용 솔리드 패스

- 근거: Inventor 2026 도움말 GUID-8D59A8BF-49A8-4214-A3A7-F184DB73E68F (Shaded with Hidden Edges, Wireframe with Hidden Edges, Wireframe with visible Edges Only의 면·가시/숨은 모서리 표).
- `gc_render::View`에 `depth_only`(불투명 솔리드를 색 없이 깊이만 기록, 반투명은 건너뜀), `hidden_edges`(시각 깊이 반대 비교 `Less`로 면 뒤의 모서리를 점선·옅게 먼저 그림) 추가. 파이프라인은 시작 때 한 번 만들고, 두 플래그가 꺼져 있으면 기본 스타일의 드로 호출·정점 처리는 그대로다(GPU 테스트가 기본 = 메시 1 + 모서리 1 = 2회, 숨은 모서리 = 3회를 확인).
- 시각 스타일 6종: 음영 / 모서리가 있는 음영(기본) / 숨은 모서리가 있는 음영 / 와이어프레임(모든 모서리, 가림 없음) / 숨은 모서리가 있는 와이어프레임 / 보이는 모서리만 있는 와이어프레임. 사실적·흑백·수채화·스케치 일러스트는 보류(리드 지시).
- 점선은 화면 대각선 4 px 켜짐/4 px 꺼짐(fs_edge_hidden의 discard), 투명도 0.55. 원문에는 점선 규격이 없다.
- 테스트: `gc_render` GPU `hidden_edges_and_depth_only_solids`(큐브 뒤 모서리: 기본 안 보임, 숨은 모서리 점선, 깊이 전용에서 큐브 색 없음·모서리는 여전히 가려짐), 셸 `the_visual_style_drop_down_…`에 새 스타일 선택 추가.

### 뷰 탭 창(Windows) 패널: 사용자 인터페이스 드롭다운, Clean Screen

- 근거: Inventor 2026 도움말 GUID-285B9036-0E97-4F18-A6D2-EF78B8B8FAA9 (ViewCube Menu Reference: "View tab > Windows pane > User Interface > ViewCube", 끄면 탐색 막대에서 계속 사용), GUID-84B053B0-E83B-4D9C-8436-1BE9AD5E9EBE (To Work with Graphics Windows: Clean Screen은 View 탭 Windows 패널, 브라우저와 리본을 숨김). Ctrl+0은 구판 GUID-5ED57F9B-6FA8-440B-94F3-0B92CACBE295(Increase Screen Space)에서 확인한 것이라 2026 쪽 문서로는 못 찾음.
- 사용자 인터페이스 드롭리스트: 뷰큐브·탐색 막대·브라우저·속성·상태 표시줄 체크(`ribbon_drop_button`). 켜고 끄는 즉시 설정 파일(`hide_view_cube`, `hide_nav_bar`, `hide_status_bar`, `browser_closed`, `properties_closed`)에 저장되고 다음 실행에서 복원.
- Clean Screen: 외형 아래 스택의 한 줄(1280 px 안에 들어가게). 리본을 없애고 브라우저를 접으며, 다시 누르거나 Ctrl+0이면 이전 상태로 복원. 저장되지 않는다(세션 단위). 원문의 "Inventor 창 최대화"·"패널 최소화 상태 복원"은 해당 없음.
- 원문에서 확인 못 한 것: 2026 드롭다운의 전체 항목 목록(확인한 것은 ViewCube, 탐색 막대). 브라우저·속성·상태 표시줄은 앱에 있는 요소를 기준으로 붙였다.
- 테스트: `the_user_interface_list_hides_elements_and_clean_screen_toggles_with_ctrl_0`; shell 108개 통과(1280 px 폭 검사 포함).

### 구속 패널: Smooth(G2) 버튼

- 근거: Inventor 2026 도움말 GUID-A85A7A30-7D81-4C75-8769-CAD034EEA930 (To Apply Geometric Constraints: Smooth).
- 2D 스케치 구속 패널에 "부드럽게(G2)" 소형 버튼(기호 G2) 추가: `ToolKind::Constraint(ConstraintKind::Smooth)`. 스플라인 끝과 이어진 선·호·스플라인을 클릭하는 흐름은 gc_tools가 처리하고, 뷰포트의 G2 기호는 기존 그리기를 따른다. 3D 스케치 패널에는 두지 않았다(원문 목록 미확인).
- 테스트: `the_constrain_panel_has_a_smooth_g2_button`; 폭 검사(`every_ribbon_tab_fits_1280_px`) 통과.

### 문서 설정 대화상자, 옵션 창 Auto-Bend 정리

- 근거: Inventor 2026 도움말 GUID-FB49837F-7832-4713-928E-1CEDBDB0C3BD (To Change Document Settings: 탭 목록), GUID-58DD9A89-84C7-48F3-8E4E-0303E755B10F (Sketch 탭: 3D Sketch Auto-Bend Radius), 옵션 창은 GUID-1898C432-CFD2-46F1-8BAE-631392DFBF1D(체크 상자만).
- 도구 > 문서 설정: 단위 탭(길이·각도 단위, 선형·각도 자릿수, 치수 표시 값/이름/식/정밀 값, 입력 상자에 식 표시)과 스케치 탭(스냅 간격 X·Y, 보조선당 스냅 수, 주선당 보조선 수, 3D 스케치 자동 굽힘 반지름, 구속조건 추론·유지). 확인/적용/취소. 길이 칸은 문서 단위로 보여 주고 식·단위를 해석(`evaluate_value`); 0 이하나 해석 안 되는 값은 대화상자에 사유를 보이고 닫지 않는다. 적용은 `Session::set_doc_settings`(한 번의 실행 취소).
- 응용 프로그램 옵션 스케치 탭: 자동 굽힘 반지름 칸을 뺐다(원문대로 체크만). `Settings.auto_bend_radius` 필드도 제거.
- 백엔드가 아직 연결하지 않았다고 한 것(단위 없는 값의 문서 단위 해석, 스냅·격자 값, 구속 추론)은 대화상자에서 값만 저장한다.
- 원문에 있고 여기 없는 것: Modeling·Drawing·BOM·Default Tolerance 탭, Lineweight, 허용차 표시.
- 테스트: `the_document_settings_dialog_changes_the_units_and_the_auto_bend_radius`(잘못된 값 거부, 좋은 값 저장, 확인으로 닫힘); `the_auto_bend_option_is_off_by_default_saved_and_read_back` 유지.

### iProperties 대화상자

- 근거: Inventor 2026 도움말 GUID-8ABF17C5-B9EB-47B5-92CF-9FBD335334E3 (iProperties Reference: 탭 목록, 브라우저 파일 이름 우클릭 iProperties), GUID-877564AC-78CE-4216-9913-003A2DE8D698 (Physical 탭: 질량·면적·부피 계산, Update, 밀도, 단위는 문서 설정 Units 탭).
- 진입: 브라우저의 컴포넌트(바디·로봇·어셈블리) 우클릭 "iProperties…", 도구 메뉴 "iProperties…"(문서). 제목에 컴포넌트 이름.
- 탭: 요약(제목·주제·작성자·회사·설명), 프로젝트(부품 번호·설명·개정 번호·프로젝트·설계자·공급업체), 물리적(밀도 kg/m³·질량 재정의 입력, 계산값 질량·부피·면적·무게중심 읽기 전용 + 업데이트 버튼). 확인/적용/취소. 값은 `Session::set_properties`(한 번의 실행 취소). 밀도·질량 재정의는 비우면 없음, 0 이하나 숫자가 아니면 대화상자에 사유를 보이고 닫지 않는다.
- 단위: 길이는 문서 설정의 길이 단위(부피는 ³, 면적은 ²), 질량은 kg.
- 업데이트: 계산 결과는 열 때·적용할 때·업데이트를 눌렀을 때만 다시 계산한다(메시가 큰 장면에서 매 프레임 계산하지 않기 위해). 원문의 "값이 최신이면 업데이트 비활성"은 두지 않았다(항상 활성).
- 원문에 있고 여기 없는 것: General·Custom·Weld Bead·Status·Details·Occurrence·Thumbnail·Save 탭, 재료 목록·Requested Accuracy·상대 오차, 클립보드 보고서, 관성 모멘트. 모양을 알 수 없는 항목(가져온 어셈블리 등)은 "계산할 수 없습니다"를 보인다.
- 테스트: `the_iproperties_dialog_saves_the_summary_and_checks_the_density`(요약 저장, 밀도 0 거부, 비우면 저장); shell 110개 통과.

### 메뉴·브라우저 전수 대조 1차, Find in Window, 빠른 실행 도구 모음

- 대조표: `docs/ux/inventor-menu-browser-audit.md` (파일 메뉴·QAT·브라우저 자체·노드별 우클릭·끌어 놓기·마킹 메뉴, 항목마다 ✓/△/✗/⛔). 읽은 원문: GUID-EDF6B951-C309-44A8-868C-B72D99F6D5A7(Inventor Browser), GUID-5F975252-BDB4-488C-A2C5-8B15C7D3C641(Assembly Browser), GUID-CB6B1637-71AD-4753-8CC9-19559BFEE3A5(아이콘), GUID-3BF60B85-4BE9-4619-9FD3-0A179FA680E0(QAT), GUID-36AE9264-912D-4D3C-BB24-7EC16FA8777E(Find in Window·Isolate), GUID-39DE5C99-A7AD-41CD-AB63-DEA72BFCFC28(애플리케이션 메뉴, 2014 쪽만 열림).
- 한계: 2026 원문에는 브라우저 노드별 우클릭 항목·순서 표와 마킹 메뉴 방향별 표가 없다(원문은 "some or all options are available"). 완전한 표는 실제 Inventor 화면 확인이 필요 — 리드에게 요청.
- 채운 것: 브라우저 우클릭 "창에서 찾기"(바디·로봇: 경계 상자로 카메라를 맞춤, `Viewport::find_in_window`), 메뉴 줄 오른쪽 빠른 실행 도구 모음(새로 만들기·열기·저장·실행 취소·다시 실행·복귀; 복귀는 제자리 편집 중에만 활성, `Session::return_edit`). QAT 버튼은 글자 단추(+ O S ← → ↑)이고 접근성 이름은 "<이름> (빠른 실행)" — 메뉴 항목과 겹치지 않게 했다. 글꼴에 ↵ 글리프가 없어 ↑를 썼다.
- 브라우저 "…" 메뉴에 "경고 숨기기"(Hide Warnings, GUID-5F975252): 관계의 sick 표시만 숨기고 실패 상태는 유지. `qa_frontend_screens`의 메뉴 옵션 테스트에 항목 추가.
- 테스트: `find_in_window_centres_the_view_on_a_component`, `the_quick_access_toolbar_undoes_redoes_and_returns`; shell 112개, qa_*·sketch_shape_tools 통과.

### BOM 대화상자

- 근거: Inventor 2026 도움말 About the BOM Structure, GUID-A0A95805-CBF6-4524-8314-4AC8B53C19CF ("To change the BOM Structure, double-click the BOM Structure row in the Bill of Materials"; Normal·Phantom·Reference·Purchased·Inseparable).
- 진입: 도구 > 자재 명세서(BOM), 관리 탭 "자재 명세서(BOM)" 버튼. 구조화/부품 전용 탭, 열: 항목(들여쓰기)·부품 번호·수량·설명·BOM 구조. 행 클릭 → 그 줄의 첫 트리 행이 컴포넌트면 브라우저·뷰에서 선택(`select_occurrence`). BOM 구조 열은 더블클릭 대신 줄 안의 드롭다운으로 바꾼다(`set_bom_structure`, 한 번의 실행 취소). "CSV로 내보내기…"는 `bom_csv`를 파일 대화상자(`FileDialogs::save_csv`)로 고른 곳에 쓰고 상태 줄에 알린다.
- 원문과 차이: 더블클릭이 아니라 드롭다운. 문서 설정의 Bill of Materials 탭(기본 BOM 구조)·열 편집·정렬·Excel 내보내기·BOM 편집 중 부품 번호 수정은 없음(설명·부품 번호 편집은 iProperties 대화상자에서).
- 테스트: `the_bom_dialog_lists_components_selects_and_exports_csv`; shell 113개 통과.

### 브라우저 노드 호버 경계 상자

- 근거: docs/ux/inventor-assembly-parity.md(기록): "브라우저 노드에 커서를 올리면 경계 상자가 그래픽에 표시"(2026 도움말의 해당 쪽은 읽지 못함).
- 브라우저의 바디·로봇 행 위에 커서를 올리면 `ParamsUi.hovered` → `Viewport.browser_hover`로 전달되어 뷰에 그 컴포넌트의 경계 상자(월드 축 정렬, 12개 모서리)를 강조색으로 그린다. 브라우저를 벗어나거나 접으면 사라진다. 경계 계산은 `find_in_window`와 같은 함수(`item_bounds`)를 쓴다.
- 테스트: `hovering_a_browser_row_boxes_the_component_in_the_view`.

### 문서 설정 남은 것: 격자에 스냅, 스케치 격자 간격, 입력 식 표시

- 근거: Inventor 2026 도움말 Sketch Tab Reference(Application Options, GUID-1898C432-CFD2-46F1-8BAE-631392DFBF1D: Snap to grid), Sketch Tab Reference(Document Settings, GUID-58DD9A89-84C7-48F3-8E4E-0303E755B10F: Snap Spacing, Grid Display), To Change Document Settings(GUID-FB49837F-7832-4713-928E-1CEDBDB0C3BD: Units 탭 Default Parameter Input Display).
- 응용 프로그램 옵션 스케치 탭 "격자에 스냅"(`Session::set_snap_to_grid`, 기본 꺼짐, `settings.json`의 `snap_to_grid`에 저장·복원).
- 스케치를 편집하는 동안 격자는 문서 설정을 따른다: 가는 선 = 스냅 간격 × 보조선당 스냅 수(축별), 굵은 선 = 주선당 보조선 수마다. 너무 멀리 축소해 가는 선이 6 px보다 가까워지면 주선 간격씩 키운다. 스케치가 없을 때의 바닥 격자는 이전의 1-2-5 자동 간격 그대로.
- 치수 입력 상자: 문서 설정 "입력 식 표시"가 켜져 있으면(기본) 쓴 식, 꺼져 있으면 값(문서 단위·자릿수)으로 시작한다(`Viewport::edit_dimension`, 뷰에서 더블클릭). 새 치수를 만들 때의 값 입력 상자(도구가 요청하는 것)는 이번에 바꾸지 않았다.
- 테스트: `the_snap_to_grid_option_is_off_by_default_saved_and_read_back`, `editing_a_dimension_starts_from_the_expression_or_the_value_as_the_document_says`, gc_viewport `the_sketch_grid_follows_the_document_settings`.

### 3D Bend: Equal 체크

- 근거: Inventor 2026 도움말 To Create Lines, Arcs, and Bends, GUID-4F861674-F74C-474E-869A-4C190ED6EC4B (Bend 대화상자의 Equal).
- 굽힘 상자에 "같음" 체크(기본 켬). 첫 굽힘은 `SketchOp::Bend`(그 결과의 첫 `Created::Curve`와 입력한 반지름을 기억), 이후 굽힘은 같음이 켜져 있고 반지름 칸이 그대로면 `SketchOp::BendEqual{lines, to}`, 끄거나 반지름을 바꾸면 `Bend`(새 기준이 됨). 굽힘이 끝나도 명령은 열려 있어 다음 선 두 개를 이어서 고를 수 있다(Esc로 끝). 이전에는 한 번 만들면 끝났다.
- 굽힘 상자는 뷰 왼쪽 아래로 옮겼다(위쪽 중앙은 고를 선을 가렸다). 반지름 칸은 선 두 개를 고를 때마다 다시 키보드를 받는다.
- 테스트: `the_bend_command_keeps_the_first_radius_with_equal_on`(선 세 개 계단, 두 번 굽힘, 두 호의 반지름이 같음, 명령 유지). 테스트의 선은 속성 패널(오른쪽)에 가리지 않는 위치에 둔다.

### 스케치 역할 UI(웨이포인트·경로 지정, 브라우저 역할, 속성, 경로 생성, 경로 패널 follow)

- 인벤터 원문이 없는 GraphCTRL 고유 기능(백엔드 handoff "스케치 요소의 웨이포인트·경로 역할", 계획 9.1·9.4). 근거는 그 문서의 API 표.
- ① 스케치 우클릭(눌렀다 떼면 나오는 목록): 점 위 "웨이포인트로 지정"(`assign_roles(sketch,&[점],&[])`), 곡선 위 "경로로 지정"(`chain_at`으로 이어진 사슬 전체를 `assign_roles`로 한 경로). 이미 역할이 있는 점·곡선에는 나오지 않는다. 사슬 미리 강조는 아직 없다.
- ② 브라우저: 스케치 노드 아래에 역할 행(웨이포인트=웨이포인트 아이콘, 경로=경로 아이콘). 이름 바꾸기·삭제(`CellOp::DeleteRole`)·억제는 다른 항목과 같은 메뉴. 선택하면(`BrowserAction::SelectRole`) 뷰가 표시: 웨이포인트=마름모+툴 방향선, 경로=선+진행 화살표, 선택된 것은 굵게(`role_pose`·`role_polyline`). 숨겨진 스케치·억제된 역할은 그리지 않는다. 역할은 `CellItem`에 넣지 않고 `Viewport.role_selected`/`ParamsUi.role`로 연결.
- ③ 속성 패널(`gc_panels::role_properties`): 이동 방식(곡선 추종·직선·관절), TCP 속도(mm/s, 비우면 제한 없음), 경로 방향 뒤집기, 툴 자세(스케치 평면/작업 좌표계 기준, 롤·피치·요 — 끌기 한 번이 되돌리기 한 단계).
- ④ 경로 생성: 로봇 탭 "스케치 경로" 스택의 세 번째 줄. 로봇 → 스케치 점 1 → 점 2 → `route_between`(첫 계획기, "두 번째 이동을 직선으로" 체크), 만들어진 경로가 패널의 현재 경로가 된다(Esc·우클릭으로 끝). 점은 보이는 모든 스케치에서 가장 가까운 것.
- ⑤ 경로 패널: 경로가 `follow`를 가지면 웨이포인트 단계 대신 역할 목록(이름 — 이동 방식·속도·방향)을 보이고 위/아래/제거(`set_path_follow`).
- 알려진 차이: 사슬 미리 강조 없음, 뷰에서 역할을 직접 클릭해 선택하는 기능 없음(브라우저만), 속도·자세는 입력할 때마다 되돌리기 한 단계, 닫힌 곡선 경로·구간별 속도는 백엔드에 없음.
- 테스트: `the_sketch_menu_makes_waypoints_and_paths`, `the_browser_lists_sketch_roles_and_selects_them`, `the_properties_panel_edits_a_path_role`(sketch_shape_tools), `create_path_makes_a_planned_path_between_two_clicked_sketch_points`와 기존 `the_ur5e_follows_a_sketched_polyline`에 follow 목록 단언(shell).

### iProperties Physical 탭: 열린 메시 경고

- `PhysicalProperties.open`이 true이면(면이 정확히 둘이 아닌 모서리가 있는 메시) 계산값 위에 호박색 경고 "닫히지 않은 메시가 있어 부피·질량이 정확하지 않을 수 있습니다."를 보인다(면적은 맞다). 열린 메시를 불러오는 UI 테스트는 못 만들었다(메시 읽기가 작업 스레드) — 코드로만 확인.

### 스케치 역할 남은 차이, 브라우저 메뉴 보강

- "경로로 지정" 메뉴가 열려 있는 동안 그 명령이 가져갈 사슬(`chain_at`)을 굵은 선택색으로 미리 그린다.
- 뷰에서 역할의 표시(웨이포인트 마름모 9 px, 경로 선 6 px 안)를 클릭하면 그 역할이 선택된다(브라우저 따라감). 다른 항목이 잡히면 그쪽이 우선, 빈 곳 클릭은 선택 해제. 테스트 `clicking_a_roles_mark_in_the_view_selects_it`.
- 브라우저 "…" 메뉴: "자식만 표시"(Show Children Only, GUID-5F975252: 제자리 편집 중 편집 대상·그 안·그 위 행만), "억제된 항목만"(Find 설명의 "suppressed components"; 억제된 컴포넌트·관계만, 폴더는 유지). Show Extended Names는 부품 피처 전용이라 해당 없음(감사표). `qa_frontend_screens`의 메뉴 옵션 테스트에 두 항목 추가.

### Isolate / Undo Isolate

- 근거: Inventor 2026 도움말 Frame Generator browser, GUID-36AE9264-912D-4D3C-BB24-7EC16FA8777E ("Isolate turns off visibility of components … Undo Isolate restores visibility of components to the pre-isolation state"; Assembly browser 문서의 "Select All Visible … to restore an Isolate Components operation").
- 브라우저 컴포넌트 행·서브어셈블리 폴더 우클릭 "격리"(`Session::isolate`; 서브어셈블리는 안의 컴포넌트 전체로 펼쳐 넘김), 뷰 마킹 메뉴 오버플로의 선택된 컴포넌트 "격리". 격리 중에는 브라우저 "…" 메뉴와 뷰 메뉴에 "격리 취소"(`undo_isolate`).
- 테스트: `isolate_hides_the_others_and_undo_isolate_shows_them_again`.

### 브라우저 사용자 정의 폴더

- 근거: Inventor 2026 도움말 Assembly Browser Reference, GUID-CB6B1637-71AD-4753-8CC9-19559BFEE3A5 ("User-defined Folder groups assembly nodes for better browser organization").
- 같은 수준(부모가 같은) 행들 중 폴더에 든 것은 폴더 노드("이름 (개수)") 아래에, 나머지는 그 뒤에 그린다(`component_rows`가 `folders()`를 읽음). 폴더는 브라우저 배치일 뿐 트리 수준·부모는 그대로.
- 컴포넌트 행·서브어셈블리 폴더 우클릭: "새 폴더(이 항목으로)"(이름 입력 줄이 나와 `FolderOp::Add`), "폴더로 이동"(폴더 목록 → `FolderOp::Move`), 폴더에 든 행은 "폴더에서 빼기". 폴더 노드 우클릭: 이름 바꾸기(`Rename`)·폴더 삭제(`Delete`). 모두 되돌리기 한 단계, 거절(수준이 다른 행, 이름 중복)은 상태 줄.
- 알려진 차이: 끌어 넣기 없음(행이 클릭만 감지), 한 번에 한 행(다중 선택 없음), 폴더 안 폴더·관계·스케치는 백엔드 범위 밖.
- 테스트: `the_browser_makes_folders_and_moves_rows_into_them`.

### 브라우저 끌어 놓기·여러 행

- 근거: 기존 감사표(Assembly Browser 문서의 "Use drag and drop to move and rearrange", 위치 표현 문단은 표현 노드 한정 — 컴포넌트의 끌기 동작은 원문에서 확인하지 못함).
- 트리 행이 끌기 원천(`Sense::click_and_drag`, 짧은 클릭은 그대로). 컴포넌트 행을 끌어 폴더 머리글에 놓으면 `FolderOp::Move`, 서브어셈블리 머리글에 놓으면 `SubassemblyOp::Move`. Ctrl+클릭으로 행을 여러 개 표시(선택 색)하고 그중 하나를 끌거나 "새 폴더(이 항목으로)"·"폴더로 이동"을 고르면 표시한 행이 한꺼번에 간다. 동작 후 표시는 지워진다.
- 테스트: `dragging_a_browser_row_onto_a_folder_moves_it_in`. Ctrl+클릭 표시는 kittest가 수정 키를 못 넘겨 자동 테스트가 없다(코드로만).

### 닫힌 경로: 접선 방향·시작점, 닫힌 곡선 "경로로 지정"

- 우클릭 메뉴가 닫힌 곡선(원·전체 타원·닫힌 스플라인)도 `curve_outline`으로 집는다. 백엔드 `chain_at`이 [그 곡선]을 돌려주므로 "경로로 지정"이 뜬다.
- 경로 역할 속성: "접선 방향" 체크(`set_role_tangent`: 툴 X축이 진행 방향), 닫힌 경로(`role_closed`)면 "시작점" 선택(`set_role_start`, 스케치의 점 목록과 "곡선의 시작"). 점은 좌표(mm)로 표시한다.
- 테스트: `a_closed_curve_becomes_a_path_with_tangent_and_start_choices`. QA 브랜치의 `a_closed_curve_is_offered`는 병합되면 `#[ignore]`를 푼다.

### 작업 모델 UI (계획 9.4, GraphCTRL 고유, 인벤터 원문 없음)

- 브라우저 "작업" 최상위 노드(처음엔 접힘): 객체 유형 / 작업 객체 / 변수 / 작업 목록 네 그룹. 그룹 우클릭 "새로 만들기"(기본 값으로 만들고 이름은 `유형1`·`객체1`·`변수1`·`작업1`처럼 비는 번호), 행 클릭=선택(속성 패널에 편집기), 선택된 행을 한 번 더 클릭하거나 우클릭 이름 바꾸기=`Command::Rename`, 삭제=`WorkOp::Delete*`. 작업 행에는 준비 상태 점(●=시작 조건이 처음 상태에서 참, ○=거짓, 빨간 줄=끊긴 참조·조건 오류, `task_ready`/`task_problems`). 거절(InUse·NameTaken·Expr)은 상태 줄.
- 속성 패널 편집기(`gc_panels::work_properties`): 작업=위치(없음·컴포넌트·작업 좌표계·스케치·로봇), 시작 조건(입력 칸 아래에 `check_condition`의 메시지와 틀린 부분을 «» 안에 바로 표시, 올바르거나 비었을 때만 칸을 떠날 때 저장), 단계 목록(↑↓✕, 관절 이동·직선 이동은 웨이포인트 역할(위치가 로봇이면 그 로봇의 웨이포인트도) 선택, 경로 추종은 경로 역할, 출력 설정·입력 대기·툴 동작·대기), 단계 추가, 효과 목록(대상 객체/변수 + 값), 로봇(아무 로봇·고른 로봇만·조건에 맞는 로봇), 우선순위. 객체 유형=상태 목록(추가·이름·삭제), 작업 객체=유형·위치·처음 상태, 변수=종류(참/거짓·정수·실수·열거)·처음 값·선택지.
- 스케치 역할 우클릭: "이 역할로 작업 만들기"(웨이포인트=관절 이동 단계, 경로=경로 추종 단계, 위치=그 스케치).
- 알려진 차이: 작업의 뷰 표시(위치 표시)·실행 상태·스케줄러 UI는 없음. 단계의 입출력 이름·툴 동작은 글자 그대로(백엔드도 검사 안 함). 텍스트 편집 도중 Esc로 되돌리기 없음.
- 테스트: `the_browser_work_node_makes_and_deletes_tasks`, `selecting_a_task_shows_its_editor`(shell), `a_role_makes_a_task_from_its_browser_menu`(sketch_shape_tools).

### 툴 종류·후보 로봇·스케줄 UI (계획 9.4·9.5, GraphCTRL 고유)

- 브라우저 "작업" 아래 "툴 종류" 그룹(새로 만들기·이름 바꾸기·삭제), 편집기(`kit_properties`): 신호(이름·입력/출력·핀 번호, 추가·삭제), 툴 상태 목록, 동작 카드(이름, 설정 신호와 값, 확인 신호와 값, 툴 상태, 대상 객체 유형과 결과 상태, 조건식(`tool.state`·`target.state` 사용, 오류 인라인), 걸리는 시간·확인 대기). 거절(이름 중복·같은 핀·쓰는 동작 삭제)은 상태 줄.
- 로봇을 선택하면 속성 패널에 "툴 종류 장착"(없음 포함 목록)과 툴 상태 선택(`WorkOp::SetRobotTool`).
- 작업 편집기: 툴 종류가 하나라도 있으면 SetIo의 신호와 ToolAction의 동작을 그 목록에서 고른다(없으면 글자 입력 그대로). "후보 로봇 계산" 버튼 → `task_candidates` 결과 표(로봇, 예상 시간 또는 이유). 계획이 즉시 끝나지 않으므로 단추를 누를 때만 계산하고 다음 계산까지 표를 보인다(작업 id별로 임시 저장).
- 스케줄 패널(도구 > 스케줄, 아래쪽에 상태 표시줄 위): "스케줄 계산" → `Session::schedule`; 로봇마다 한 줄의 간트(작업 막대, 호버하면 시작·끝), 전체 시간(makespan), "하지 못하는 작업"과 이유. 결과는 다음 계산까지 보인다. 백엔드가 `request_schedule`/`schedule_result`(작업 스레드)를 내면 그걸로 바꾼다.
- 알려진 차이: 간트는 시간 눈금 없음, 작업 스레드 계산 아님(동기, 큰 셀에서는 눌렀을 때 멈출 수 있음), 신호·동작을 실제 로봇에 보내는 UI 없음.
- 테스트: `a_tool_kind_is_edited_and_fitted_to_a_robot`, `the_schedule_panel_works_out_the_schedule_and_lists_blocked_tasks`(shell).

### 스케줄 패널: 작업 스레드, 시간 눈금; 효과 추가 기본값 수정

- 스케줄 패널이 `request_schedule`/`schedule_result`/`schedule_pending`을 쓴다(결과는 세션이 들고 있음): 계산 중에는 단추를 끄고 스피너와 "계산 중…", 문서가 바뀐 뒤의 결과는 흐리게 그리고 "문서가 바뀌었습니다 — 다시 계산하십시오". 간트 위에 초 눈금(1-2-5 간격).
- QA 버그: 작업 편집기 "효과 추가"가 변수에 항상 `true`를 넣었다 → 변수 종류별 기본값(참/거짓=true, 정수=0, 실수=0.0, 열거=첫 선택지). QA `add_effect_works_for_an_integer_variable`의 `#[ignore]`를 풀었다.
- 테스트: shell `the_schedule_panel_…`에 "오래된 결과" 단언 추가, qa_work_screens 7개 통과.

### 스케줄 재생

- 스케줄 패널에 "스케줄 재생/일시정지", "스케줄 원위치", 시각 슬라이더(0..=makespan, s), 간트 위 현재 시각 세로선. 재생 중에만 repaint(`unstable_dt`로 시각을 진행, 끝에서 멈춤). 슬라이더·재생은 `set_schedule_playback(Some(t))`, 원위치·패널 닫기는 `None`(로봇이 원래 자리로, 문서는 바뀌지 않음). 버튼 이름은 셀 패널의 "재생"과 겹치지 않게 "스케줄 …"로 했다.
- 테스트: `the_schedule_panel_plays_the_schedule_and_resets`.

### 작업 반복·대기 자세·간트 대기/비키기·수치 효과 식

- 숫자 변수(정수·실수)의 효과 값 칸이 수치 식 입력(`count + 1`): `effect_number`로 입력 중에 검사해 칸 아래에 메시지와 «틀린 부분»을 보이고, 올바를 때만 칸을 떠날 때 저장. 그 밖의 변수는 전과 같음.
- 작업 편집기 "반복": 한 번 / 조건이 참일 때마다(`Some(0)`) / 최대 n번(`Some(n)`, 1..1000). 고르는 즉시 `SetTaskRepeat`.
- 로봇 선택 시 속성 패널 맨 위에 "대기 자세": "현재 자세로 설정"(`SetRobotPark`, 지금 보이는 관절값) / "지우기". 문서 자세와 다른 대기 자세가 있으면 "설정됨"으로 표시.
- 스케줄 간트: 비키기(`parks`)는 회색 막대(호버 시 시각), 대기(`waits`)는 작업 막대 앞의 빨간 빗금, `cut_short`면 "끝없이 반복하는 작업이 있어 …" 경고.
- 테스트: `a_task_repeats_and_a_robot_gets_a_park_pose`(shell). 수치 효과 식의 입력 중 검사와 간트 빗금·비키기 막대는 자동 테스트가 없다(코드로만).

## URScript 내보내기 UI (계획 9.6)

- 작업(Task) 속성의 로봇별 "URScript 내보내기" 버튼과 스케줄 패널의 "로봇별 URScript 내보내기…" 버튼이 `task_urscript`/`schedule_urscript` 결과를 읽기 전용 고정폭 창으로 보여 준다(복사·파일 저장·폴더에 모두 저장).
- 창 상단에 "URSim·실제 로봇으로 검증하지 않은 프로그램입니다 — 실행 전에 반드시 확인하십시오." 경고. 오프라인 텍스트만 만든다.
- `FileDialogs`에 `save_script`, `pick_folder` 추가(기본 `None`). 요청은 패널 클로저 밖(같은 프레임, 패널 뒤)에서 처리한다.
- 스케줄 패널의 버튼은 재생 줄에 둔다(따로 줄을 두면 패널 높이 때문에 재생 버튼이 잘려 클릭이 안 되는 회귀가 있었다).
- 이 기능은 Autodesk 도움말 대상이 아니다(GraphCTRL 고유 기능).

## 인벤터 환경 맞추기 2: 다중 선택·상태 표시줄·단축키

근거: Keyboard Shortcut Reference GUID-DEEB0723-DFC7-4B28-95F4-099CC1E1EBF8 (2026 원문, curl), Inventor Browser GUID-EDF6B951-C309-44A8-868C-B72D99F6D5A7.

- 브라우저 컴포넌트 행: Shift+클릭이 마지막 클릭한 행부터 이 행까지(그려진 순서) 표시, Ctrl+클릭은 토글(기존). 표시한 행은 끌기·폴더 명령에 함께 쓰인다. UI 테스트는 kittest가 `i.modifiers`를 못 채워 못 썼다(수동 확인 필요).
- 상태 표시줄: 스케치 밖에서도 대기 중인 조립 명령(조립·구속·조인트·패턴·복제)이 무엇을 고를지 프롬프트로 알려 준다.
- 단축키: `]` 작업 평면, `/` 작업 축, `.` 작업 점(스케치 밖), F2 = 선택한 브라우저 노드 이름 바꾸기(원문은 F2가 팬도 겸하므로 뷰의 F2 팬은 그대로).
- 아직 없는 원문 단축키: M 컴포넌트 이동·RO 회전·N 컴포넌트 만들기(우리 앱에 해당 명령이 없음), E 돌출 등 부품 환경 명령, 다문자 별칭.
- 이미 있었던 것: 브라우저 필터·찾기, 더블클릭 편집, 끌어 폴더 이동, 스케치 진입 시 스케치 탭 전환(감사표 3.1).

## 후보 로봇 백그라운드 계산·신호 대기 편집 (백엔드 4492e21·492b35a)

- "후보 로봇 계산" 버튼은 `request_task_candidates`로 요청하고 `task_candidates_result`/`task_candidates_pending`으로 표를 그린다(계산 중 표시, 문서가 바뀌면 "다시 계산하십시오").
- 입력 대기(WaitIo): 툴 종류의 입력 신호와 셀의 작업이 설정하는 신호(`driven_signals`) 중에서 신호 + 값(켜짐/꺼짐)을 고르고, 조건 문자열은 `이름 == true/false`(이름에 기호가 있으면 따옴표)로 저장한다. 목록이 비면 예전처럼 문자열 입력.
- Autodesk 도움말 대상 아님(GraphCTRL 고유).

## QA 수정: 스케줄 슬라이더 이름·효과 식 칸 밀림

- 스케줄 시간 슬라이더에 "시간" 라벨(접근성 이름)을 붙였다. QA의 ignore 해제.
- 효과 값 칸: 잘못된 식의 오류 문구가 칸과 같은 줄에 놓여 줄이 길어지면 칸이 옆으로 밀려, 다음 클릭이 빗나가 "한 번 저장 안 됨"처럼 보였다(`count count`에서 재현). 칸과 문구를 세로로 묶어 문구가 칸 아래에 나오게 했다. QA 테스트에 각 잘못된 식 뒤 올바른 식 저장 확인을 추가.

## 브라우저 우클릭: 복사·붙여넣기

- 컴포넌트·서브어셈블리 행 메뉴에 "복사"(표시한 행은 함께)와 "붙여넣기"(복사한 것이 있을 때만 활성; 위치를 클릭으로 지정)를 넣었다. 근거: Keyboard Shortcut Reference GUID-DEEB0723-DFC7-4B28-95F4-099CC1E1EBF8의 Copy/Paste 동작을 메뉴로 노출(우클릭 항목 표는 2026 원문에 없음 — 감사표 3.2의 "Copy / Paste" 행).
- 브라우저 끌어서 순서 바꾸기: 위치 표현 재정렬만 원문에 있고 우리 앱에 해당 개념(위치 표현)이 없어 보류.

## 간트: 신호 변화·단계 경계 (백엔드 4bd80ef)

- 로봇 행 아래에 `Schedule.signals`의 신호별 행: 켜진 구간을 채워 그린다(켜짐 → 꺼짐, 끝까지 켜져 있으면 스케줄 끝까지).
- 막대에 `TaskPlan.step_ends`(작업 시작 기준)로 단계 경계 눈금을 그리고, 호버 문구에 "단계 N: t s"를 나열한다.
- 시각 확인은 못 했다(화면 캡처 없음); 신호 행의 존재만 테스트.

## Ground and Root · Place at Component Origin 연결

근거(원문, curl): Ground and Root GUID-98FA88B2-AA32-4A27-946A-DCECB36A6696, Place at Component Origin GUID-E7A45820-E297-449D-AFBF-3C8D8EAE8A0E.

- 컴포넌트 행 우클릭: "원점에 고정 (Ground and Root)" → `session.ground_at_origin`(표시한 행은 함께; 실패 사유는 상태 줄 알림), "이 컴포넌트의 원점에 배치…" → 파일 열기 대화상자 → `add_mesh_body` + `place_at_component_origin(새 컴포넌트, 이 컴포넌트)`.
- 원문과 차이: 원문은 리본(Assemble > Productivity)에서 대화상자(Ground at origin / Create origin flush constraints / Reposition to the top of the browser)를 연다. 우리는 백엔드에 "Ground at origin"만 있어 대화상자 없이 우클릭 항목 하나로 했고 리본 단추는 아직 없다. 한국어 표기는 한국어 도움말을 못 읽어 임의("원점에 고정")이니 확인 필요. 여러 파일 동시 선택(Place at Component Origin)은 대화상자가 한 파일만 돌려줘 한 번에 하나.
- 테스트: `a_row_menu_grounds_the_component_at_the_origin`(원점 이동·고정·undo). 원점 배치는 파일 대화상자 의존이라 UI 테스트 없음.

## 감사표 잔여 점검 (Expand/Collapse All · 리본 탭 전환)

- Expand/Collapse All: Assembly Browser(GUID-5F975252…)·Inventor Browser(GUID-EDF6B951…) 2026 원문에 해당 항목이 없다("File nodes, both collapsed and expanded"만). 근거 없이 만들지 않고 보류.
- 리본 탭 전환: 스케치 진입 시 스케치 탭 활성화·종료 시 이전 탭 복귀는 이미 있다(`tab_before_sketch`). 그 밖의 문맥 전환은 원문 근거를 찾지 못했다.
- 백엔드의 구속 값 매개변수·식, Planar 조인트, Gap/정렬, Free Move/Rotate, 표현 재정의·Lock은 API가 오면 연결한다.

## 어셈블 탭 Productivity: Ground and Root 대화상자·Place at Component Origin

근거(원문): Ground and Root GUID-98FA88B2-AA32-4A27-946A-DCECB36A6696, Place at Component Origin GUID-E7A45820-E297-449D-AFBF-3C8D8EAE8A0E.

- 리본 조립 탭 "생산성 (Productivity)" 그룹(작은 단추 두 줄; 큰 단추 두 개는 1280px 너비 테스트를 넘겨 쌓았다).
- Ground and Root 대화상자(`productivity.rs`): 컴포넌트 체크 목록(브라우저에서 표시한 행이 미리 선택), 옵션 3개 — "Ground at origin"(켜짐, `ground_at_origin`), "Create origin flush constraints"·"Reposition to the top of the browser"(백엔드에 없어 비활성 + "아직 지원하지 않습니다" 툴팁). 거부되면 상태 줄 알림.
- Place at Component Origin: 단추 → 뷰에서 대상 컴포넌트 선택(상태 줄 프롬프트, Esc 취소) → 파일 대화상자 → `place_at_component_origin`. 원문 흐름과 같다(여러 파일 선택은 대화상자 한계로 한 개).
- 우클릭 항목(앞선 커밋)은 그대로 둔다. 한국어 표기는 한국어 도움말을 못 읽어 한국어 + 영어 원문 병기.
- 남은 지시: ② 마킹 메뉴 원문 조사·구현, ③ 리본 탭 배치를 Ribbon reference와 대조.

## 마킹 메뉴 원문 반영

근거(원문, curl): About Marking Menus GUID-4B081B74-45EA-451E-84BE-9FFE0E2ACAE3, To Work with Marking Menus GUID-756BC728-3366-497D-B480-4A06E932ED20, Reference for Marking Menu Tab of Customize Dialog GUID-1A2766FB-EB3E-476B-9C44-D500A5137478.

- 이미 있던 것: 우클릭 방사형 메뉴(중심은 커서), 누른 채 끌어 빨리 놓는 마크(제스처) 즉시 선택, 클릭하면 메뉴+오버플로 목록, 환경별 기본 명령(조립: Constrain·Place Component·Move·Rotate, 스케치: Center Point Circle·Line·Two Point Rectangle·Finish Sketch).
- 이번에 맞춘 것: ① 중앙 클릭으로 닫기는 있다. Esc로도 계속 닫는다(원문은 Esc를 쓰지 말라고 하나 QA 테스트 여러 개가 Esc로 닫는다고 가정하고, 메뉴가 Esc를 먹어도 해롭지 않아 유지). ② 위/아래 화살표로 명령을 옮기고 클릭으로 확정(방사형 순서, 북쪽부터 시계 방향). ③ View > User Interface의 "마킹 메뉴" 체크 해제 = "Use Classic Context Menu": 명령 전체를 목록으로(설정 `classic_context_menu`로 저장).
- 하지 않은 것(원문에 있으나 우리 범위 밖): Customize 대화상자의 명령 배치 편집·XML 가져오기/내보내기, Ctrl+우클릭 사용자 메뉴, 표시 형식(글자만/아이콘만), 오버플로 형식(전체/방사형만/짧게). 방향별 기본 항목 표는 2026 원문에 없다(위치는 우리가 북·동·남·서로 정함).

## 리본 배치 대조 (조사 결과)

- 2026 도움말에는 탭·패널을 한 장에 나열한 "Ribbon reference"가 없다. About the Ribbon(GUID-78BFC5E2-5323-48E3-A704-58DDD1836EFE)은 "탭 > 패널(드롭다운 화살표 = 추가 명령)" 구조만 설명하고, 패널 이름·순서는 명령 페이지마다 "Assemble tab > Component panel > Place" 식으로 흩어져 나온다.
- 우리 스케치 탭 패널 이름(Create·Modify·Pattern·Constrain·Exit)은 명령 페이지의 표기와 같다. 조립 탭은 Component·Pattern·Productivity·Relationships·Edit·Position 순이라 원문 표기(Component > Position > Relationships > Pattern > … > Productivity)와 순서가 다를 수 있으나, 순서를 확정할 원문 페이지를 못 찾아 바꾸지 않았다. 순서 근거(리본 스크린샷 또는 패널 목록)를 받으면 조정한다.

## ViewCube 우클릭 메뉴 (Home)

근거(원문, curl): To Reorient the View GUID-94F44C80-7313-4529-85D1-E64ABF732700, About Reorienting the View GUID-4A18A2AC-69F2-44DD-AD07-B8733321E8C9, To Work with the Navigation Bar GUID-FAAB453E-31EB-46B8-9A74-3BF7A4D88EC9.

- ViewCube 우클릭: "현재 뷰를 홈으로 설정"(Set Current View as Home), "홈으로 이동"(Go Home), "뷰를 바꿀 때 모델 맞추기"(Fit-to-View on view change). 홈 단추·마킹 메뉴의 Home도 저장한 홈 뷰로 간다(없으면 등각). 홈 뷰는 세션 안에서만 기억한다(문서에 저장 안 함).
- 큐브 위의 우클릭은 마킹 메뉴를 열지 않는다.
- 구현 메모: egui의 `context_menu`는 큐브 위젯이 우클릭을 못 받아 쓰지 못했고(뷰포트 응답과 겹침) 직접 그린 Area로 열었다. 메뉴가 열린 첫 프레임엔 크기가 없어 "밖을 눌렀다"로 닫히지 않게 나이를 센다.
- 하지 않은 것: Set Current View as Front / Reset Front(큐브 면 방향이 고정이라), 애니메이션 전환 옵션, 큐브 우클릭의 Orthographic/Perspective(2026 원문 페이지에 없음), 탐색 막대 Customize·도킹·불투명도.

## 구속 값 = 모델 매개변수 (백엔드 ac86218)

근거(원문, curl): About Parameters in Models GUID-8E5C63EA-0540-4EBE-9B17-B8159C7FE40C, To Work with Parameters in Models GUID-595F9D8E-9EEA-4446-91AB-830E5BAE548E, Edit assembly constraints GUID-36315745-5FE6-4277-9F00-E410B962D83F.

- ② 매개변수 표 "모델 매개변수"에 구속의 오프셋·각도 행(이름 dN·식·값·단위)이 스케치 치수 뒤에 나온다. 식 칸 Enter = `SetRelationValue`(실패는 대화상자에 표시), 이름 칸 Enter = `RenameRelationParam`, 우클릭 "사용자 매개변수로 복사".
- ① 브라우저의 구속 값 칸(Edit Values in Browser): 식을 입력할 수 있다(처음 값은 입력했던 식 그대로). 식이 틀리면 칸 아래에 "식을 읽을 수 없습니다"류 문구가 뜨고 저장하지 않는다. 구속 대화상자(Constrain)의 오프셋 칸도 같은 식 입력이다: 숫자는 mm, 식·단위·매개변수 이름 가능, 틀리면 칸 아래 문구 + 버튼 비활성, 식은 만든 뒤 `SetRelationValue`로 매개변수에 그대로 저장된다. Angle 구속 대화상자는 아직 없다(구속 대화상자가 Mate/Flush/Insert만).
- ③ 브라우저 노드 이름 옆 값 표시 형식("Mate:1 (d3 = 10 mm)")은 원문에 형식이 없어 넣지 않았다(Edit assembly constraints는 "브라우저 아래 편집 상자에 값을 입력"만 말한다).
- 테스트: `a_constraints_offset_is_a_model_parameter_edited_as_an_expression`.

## 남은 일
- 어셈블리 P1 FE 남은 것: #2 컴포넌트 원점 폴더·가시성(백엔드 원점 데이터 필요), #3 뷰포트에서 원점 평면·축·점 선택, #4 컴포넌트 평면 위 스케치, #8 단면도(Half/Quarter/Three-Quarter + 캡: 클립 평면은 F7 인프라 재사용, 캡 렌더 필요), #10 구속 표시·DOF 글리프, #13 Free Move/Rotate
- 브라우저: 삭제 시 `dependents_of` 목록을 보여 주는 확인 안내(지금은 거부 메시지만), 끌어서 순서 바꾸기(`MoveInTree`), 롤백 마커 표시(`rollback_index`), 설비(`assemblies`)·작업 피처·피처 그룹
- 인벤터 P1 FE 남은 것(우선순위 순): #2 마킹 메뉴, #3 F2 이름 바꾸기(백엔드 `Rename` 명령 필요), #5 전 노드 가시성(스케치·웨이포인트·경로·좌표계의 Look 문서 데이터 필요), #7 Navigation Bar, #8 Look At, #9 Select Other, #10 캔버스 치수 상자, #11 속성 패널 입력 규칙, #12 상태바 프롬프트(좌표·구속·단위 칸), #15 우클릭 메뉴 정비
- 사각형 3종(2점·3점·중심): 자리 버튼을 "생성" 그룹에 넣으면 "스케치 마무리"가 밀려 kittest가 실패해 "예정" 그룹을 맨 끝에 둠. 사각형은 드롭다운 한 버튼으로 묶을 것
- M2-1 모델 트리 나머지(그룹 확장, 소비된 스케치 접힘, 롤백 마커, 끌어서 순서 바꾸기, 오류 배지), 마킹 메뉴(우클릭 드래그는 내비게이션 프리셋과 겹치는지 먼저 확인), 값 입력 미니 툴바, 기본 도형 버튼 제거(필드 도입 시)
- 백엔드 요청(M2-1 트리용): 이름 바꾸기 명령(`Command::Rename{item,name}`, F2), 억제(suppress) 플래그와 명령, 삭제 시 종속 항목 목록(`dependents_of(item)`), 순서 바꾸기 명령과 종속 검사, 롤백 위치(`rollback_index`), 피처·필드·작업 그룹의 문서 데이터
- 경로 결과 카드·로봇 속성의 보증 문구(Guarantee)/ACM/접촉 허용 표시: **대기** — 백엔드 세션 API(`docs/handoff/backend.md`)가 아직 리드 브랜치에 없음
- 3D 스케치 점 드래그(아래 요청 필요)

## 백엔드에 보낸 요청 (2026-11-17, 시그니처는 제안, 회신 후 확정)
rename(item, name) · 스케치·웨이포인트·경로·좌표계 가시성(`Shown` 확장)과 set_suppressed/is_suppressed · dependents_of(item) · move_item/롤백(문서 모델 이후) · 3D 점 드래그(`begin_drag_3d`/`update_3d`, 세션 `spatial_drag_*`) · gc_session이 PairReason/LinkPair/Guarantee re-export.

## 요청 사항 (리드/백엔드)
- `gc_session`이 `gc_motion::{PairReason, LinkPair, Guarantee}`를 re-export해 주세요. gc_panels에 gc_motion 의존을 추가하면 Cargo.lock이 바뀌어 지금은 `PairReason`을 Debug 이름으로 구분하고 있습니다(`cell.rs` acm).
- 패닉 메시지 문구는 아직 영어 고정(언어 설정 접근 불가) — 필요하면 설정 파일의 언어를 읽도록.
- `gc_sketch::Sketch::begin_drag`가 공간 스케치를 `DragRefused::NothingMobile`로 거부하고 `SketchDrag::update`/`ToolOutput::DragTo`가 `[f64; 2]`만 받음. 3D 점 드래그에는 `begin_drag_3d(point, cursor: [f64; 3])`/`update_3d`(솔버 3D 드래그는 이미 있음)와 세션의 `spatial_drag_start/move/end`(광선과 화면 평면 또는 축 잠금으로 얻은 3D 위치를 받음)가 필요. 생기면 뷰포트 쪽 입력은 바로 연결한다.
