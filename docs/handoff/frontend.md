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
