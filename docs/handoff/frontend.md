# 프론트엔드 인수인계

## 한 일
- 브라우저: `gc_ui::panel::tree_row`(종류 아이콘·선택/호버 배경·가시성 체크·삭제 ×). × 는 호버·선택 행에서만 보이고, 접근성 이름("X 삭제")은 항상 존재. 원점·스케치·파라미터·셀 항목 모두 이 행을 사용. 스크린샷: `docs/evidence/img/review-25-browser-rows.png`
- 속성 창: 콤보박스(좌표계 축·티치 좌표계·툴 형상·경로 선택·플래너·웨이포인트 추가)와 시드 행을 `panel::combo_row`/`panel::label` 왼쪽 라벨 열로 정렬(라벨이 접근성 이름). `review-26-properties-combo.png`

- 3D 스케치 구속 글리프: 축 방향("ax X/Y/Z")·일치·고정 등을 3D 스케치에도 표시(`paint::glyph_spots`로 배치 계산을 그리기·피킹이 공유). 선택 도구로 글리프를 클릭하면 구속이 선택되고 Delete로 삭제(테스트 `a_3d_sketch_is_drawn_in_the_viewport`). `review-27-3d-sketch-glyphs.png`

- 도구 > 성능 오버레이: 뷰포트 왼쪽 아래에 프레임 시간(ms, 지수 평균)·fps·드로 호출·삼각형. 켜면 계속 다시 그림. `review-28-perf-overlay.png`(스크린샷의 250 ms는 테스트 하네스의 고정 dt)

- M2-1 첫 조각: 스케치 행에 우클릭 메뉴(스케치 편집·삭제)·삭제 ×·더블클릭 편집. 스케치 리본 끝의 "예정" 그룹에 점·다각형·슬롯 자리(비활성).

## 남은 일
- 사각형 3종(2점·3점·중심): 자리 버튼을 "생성" 그룹에 넣으면 "스케치 마무리"가 밀려 kittest가 실패해 "예정" 그룹을 맨 끝에 둠. 사각형은 드롭다운 한 버튼으로 묶을 것
- M2-1 모델 트리 나머지(그룹 확장, 소비된 스케치 접힘, 롤백 마커, 끌어서 순서 바꾸기, 오류 배지), 마킹 메뉴(우클릭 드래그는 내비게이션 프리셋과 겹치는지 먼저 확인), 값 입력 미니 툴바, 기본 도형 버튼 제거(필드 도입 시)
- 백엔드 요청(M2-1 트리용): 이름 바꾸기 명령(`Command::Rename{item,name}`, F2), 억제(suppress) 플래그와 명령, 삭제 시 종속 항목 목록(`dependents_of(item)`), 순서 바꾸기 명령과 종속 검사, 롤백 위치(`rollback_index`), 피처·필드·작업 그룹의 문서 데이터
- 경로 결과 카드·로봇 속성의 보증 문구(Guarantee)/ACM/접촉 허용 표시: **대기** — 백엔드 세션 API(`docs/handoff/backend.md`)가 아직 리드 브랜치에 없음
- 3D 스케치 점 드래그(아래 요청 필요)

## 요청 사항 (리드/백엔드)
- `gc_sketch::Sketch::begin_drag`가 공간 스케치를 `DragRefused::NothingMobile`로 거부하고 `SketchDrag::update`/`ToolOutput::DragTo`가 `[f64; 2]`만 받음. 3D 점 드래그에는 `begin_drag_3d(point, cursor: [f64; 3])`/`update_3d`(솔버 3D 드래그는 이미 있음)와 세션의 `spatial_drag_start/move/end`(광선과 화면 평면 또는 축 잠금으로 얻은 3D 위치를 받음)가 필요. 생기면 뷰포트 쪽 입력은 바로 연결한다.
