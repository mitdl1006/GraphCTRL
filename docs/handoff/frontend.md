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

## 남은 일
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
