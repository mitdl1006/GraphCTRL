# Inventor 동등성(parity) 명세 — GraphCTRL vs Autodesk Inventor 2026

작성 2026-11-17. 근거: Autodesk Inventor 2026(없으면 2025·이전) 공식 도움말·What's New. 현재 상태: `docs/STATUS.md`, `docs/PLAN.md` 0·5.2·5.3·9·부록 B, 스크린샷 `docs/evidence/img/review-19/20/23/25`.

**검증 상태(2026-11-17 갱신)**: `help.autodesk.com` 공식 도움말 페이지 전문을 직접 열어 대조했다(2026판, 없으면 2025·이전판). 전문에서 확인한 항목에는 출처를 달았고, 여전히 도움말에서 근거를 못 찾은 것만 **(미확인)** 으로 남겼다. 도움말 텍스트는 확인했지만 실제 화면·타이밍 동작은 보지 못했다는 점에 유의(이 문서는 도움말 기준이며 실기 검증이 아님). 출처 연도가 2026이 아니면 연도를 적었다. 이전 초안의 S9(2026 What's New) 인용 중 일부는 오귀속이어서 바로잡았다(S9는 실제로 2020 UI 개선 페이지이고, 속성 패널 규칙의 출처는 S27).

## 1. 범위

- **동등성이란**: (a) 룩(밝은/어두운 테마, 리본·브라우저·속성 패널의 모양), (b) 레이아웃(리본 + 좌 브라우저 + 중앙 그래픽 + ViewCube/내비게이션 바 + 상태바), (c) 상호작용(F2 이름 바꾸기, 마킹 메뉴, 선택 규칙, 속성 패널의 입력 흐름, 스케치 시작·마무리), (d) 단축키(F2/F3/F4/F5/F6/Home, F8/F9 등)가 인벤터 사용자에게 같은 손맛으로 동작하는 것.
- **대상 객체는 다르다**: 사용자가 만드는 것은 솔리드 부품이 아니라 로봇 셀(스케치, 불러온 설비, 필드·구역, 웨이포인트, 경로, 작업)이다. 인벤터의 "피처"는 우리의 "작업 피처·필드·웨이포인트·경로"로 대응시킨다.
- **복제하지 않는 것**: 솔리드 모델링 전반(돌출 외 피처, 시트메탈, 어셈블리 구속, 도면, iLogic, 시뮬레이션 등), Vault, Autodesk 계정·클라우드. **Autodesk 상표·로고·아이콘·그림·색 팔레트 자산은 쓰지 않는다**. 아이콘은 코드로 그린 자체 벡터(PLAN 5.3), 용어는 일반 명칭만 쓰고 제품명 "Inventor"는 호환성 설명에만 쓴다. 내비게이션 프리셋 이름 "Inventor"는 설정 항목의 설명 문구로만 허용(법무 확인 필요).
- 담당 표기: 프론트엔드 = `crates/ui`·`render`·`gc_app`·`gc_panels`, 백엔드 = `crates/domain`·`plugins`(문서 모델·명령·솔버).
- 우선순위: P1 = M2-1 게이트 전(인벤터 사용자가 첫 10분에 막히는 것), P2 = M2 중, P3 = 이후/선택.

## 2. 영역별 비교

### 2.1 Home 화면 · 파일/프로젝트

| 항목 | 인벤터 2026 동작 (출처) | GraphCTRL 현재 | 차이 | 우선순위 | 담당 |
|---|---|---|---|---|---|
| Home 화면 | 2026판 도움말: 왼쪽에 활성 프로젝트·Open·New(템플릿 드롭다운, "Browse templates")·Settings, 오른쪽 Recent Documents(목록/썸네일 전환, 정렬 기본 "Last Opened", 검색창·필터·고정 pin, 선택 후 Open Selected/Remove Selected/Remove Unpinned), 하단 Learning·Support·Community. 검색은 Esc로 비움 [S10]. 시작 시 표시는 응용 옵션 "Show My Home on startup" [S25] | 파일 메뉴의 최근 항목·예제 열기(STATUS). 전용 Home 화면은 스크린샷에 없음 | Home 화면·썸네일·고정 없음 | P2 | 프론트엔드 |
| 최근 문서 수 | 응용 옵션 일반 탭 Home 항목에서 설정: Home 페이지는 최대 199 [S10], 옵션 페이지는 기본 50·최대 200 [S25](두 쪽이 불일치). 어느 쪽이든 상한 약 200 | 최근 항목 있음, 개수 설정은 (미확인) | 설정 없음 | P3 | 프론트엔드 |
| 프로젝트(Project) | 활성 프로젝트 파일이 경로·라이브러리를 정함. Home 또는 File > Manage Projects에서 활성 프로젝트 변경, 문서를 모두 닫아야 바꿀 수 있음 [S10][S36](2026) | `.gctrl` 단일 파일 + `robots/` 라이브러리 | 프로젝트 개념 없음 | P3 | 백엔드 |
| 새로 만들기 템플릿 | Home의 New 드롭다운에서 표준 템플릿 선택 또는 Browse templates, 기본 템플릿 `standard.ipt` [S10][S25] | 파일 > 새로 만들기(PLAN 부록 B 단축키 Ctrl+N) | 템플릿(예: "빈 셀", "UR 단일 셀") 없음 | P2 | 둘 다 |
| 저장·복구 | Ctrl+S 저장 [S6] | 원자적 저장, `.bak`, 자동 저장 복구(STATUS) | 인벤터보다 앞섬(유지) | - | - |
| 외부 파일 열기 | 다양한 CAD 형식 열기 [S20] | STL/DAE/URDF 있음, GLB·STEP은 M2-1 계획 | 어셈블리 트리 불러오기 미구현 | P1 | 백엔드 |

### 2.2 Application menu · Quick Access Toolbar · 문서 탭

| 항목 | 인벤터 2026 동작 (출처) | GraphCTRL 현재 | 차이 | 우선순위 | 담당 |
|---|---|---|---|---|---|
| 응용 프로그램 메뉴 | "File 메뉴"(Application menu): 최상단 검색 상자로 QAT·메뉴·리본의 명령 위치를 찾고(와일드카드 `*` `?`, 결과 클릭 시 리본 위치가 깜박이며 실행, 현재 환경에서 못 쓰는 명령은 비활성 표시) [S34], Manage Projects·Save/Save As/Export·Options(응용 옵션) 항목 확인 [S36][S37 검색 요약]. 전체 항목 배치는 페이지 전문 확인 못 함 (미확인) | 상단 텍스트 메뉴줄 "파일 편집 도움말 도구"(스크린샷) | 형태 다름, 기능은 대응. 인벤터식 큰 버튼 메뉴는 필수 아님 | P3 | 프론트엔드 |
| Quick Access Toolbar | 드롭다운 화살표의 Customize 메뉴에서 체크로 명령 추가/제거, "Show Above/Below the Ribbon"으로 위치 선택, 넘치면 드롭다운. 리본 명령 우클릭 "Add to the QAT" 확인. QAT 전용 Return 명령(제자리 편집 종료)이 있음 [S19](2026). Select 풀다운도 QAT에 있음 [S8] | 없음(스크린샷) | 실행 취소·저장·열기 고정 도구 모음 없음 | P2 | 프론트엔드 |
| 문서 탭 | 열린 문서는 그래픽 창 **하단** 탭(파일명 위에 커서를 두면 썸네일·문서 정보 툴팁). 시작 기본 탭은 "My Home". 탭 끌어 탭 그룹·새 창·다른 모니터로 분리, 가운데 클릭으로 비활성 탭 닫기, 탭 메뉴에서 정렬(타일)·문서 목록·Close All, 탭 줄 더블클릭 최대화, Reset UI Layout [S33](2026) | 단일 문서 | 다중 문서·탭 없음. 로봇 셀 1개가 기본이라 낮음 | P3 | 둘 다 |
| 도움말 | 브라우저 메뉴에서 표시 옵션·도움말 접근 [S7]. F1 = 활성 명령·대화상자의 도움말 [S5]. 온라인/로컬 도움말 선택(응용 옵션) [S25] | 도움말 > 정보·라이선스 | 문맥 도움말(F1) 없음 추정(현 코드 미확인) | P3 | P3 | 프론트엔드 |

### 2.3 Ribbon

| 항목 | 인벤터 2026 동작 (출처) | GraphCTRL 현재 | 차이 | 우선순위 | 담당 |
|---|---|---|---|---|---|
| 탭 구성 | 작업 중심 탭, 각 탭에 User Commands 패널 1개 허용, Get Started 탭에 What's New·동영상·튜토리얼, 새·갱신 명령에 색 점 표시 옵션 [S1] | 3D 모델·스케치·로봇·뷰 탭, 밑줄 탭, 패널 라벨("추가","경로")(스크린샷) | 구성은 근접. 도구 탭(설정·계산기)은 메뉴에 있음. 사용자 패널·Get Started 없음 | P3 | 프론트엔드 |
| 문맥 탭(Sketch) | 스케치 같은 특수 환경에 들어가면 **초록색 문맥 탭**이 나타나고 종료하면 닫힘 [S1]. 스케치 시작은 3D 모델 탭 Start 2D Sketch, 브라우저 우클릭 New Sketch, 평면·작업 평면 클릭 후 미니 툴바 Create Sketch(2024.1) [S15]. 탭 안 패널 이름 세부는 (미확인) | 스케치 탭은 수동 선택. 진입 시 자동 전환 여부는 (미확인) | 진입·종료 시 탭 자동 전환, 마무리 시 이전 탭 복귀 보장 필요 | P1 | 프론트엔드 |
| 패널·드롭다운 | 패널 제목 옆 화살표로 확장(핀으로 유지), 드롭다운 화살표 명령은 변형 묶음이며 우클릭 Ungroup Commands로 풀어 표시, 명령을 확장 패널↔주 패널로 이동 가능 [S1][S35] | 평면 버튼 나열. 드롭다운 변형 없음 | 변형 명령 묶음 없음(원 2종, 사각형 3종, 호 3종) | P2 | 프론트엔드 |
| 리본 최소화 | 탭 줄 오른쪽 화살표로 "탭만/패널 타일" 최소화, 아이콘 클릭 시 순환, 우클릭 Ribbon Appearance > Reset Ribbon으로 복원, 좌·우 도킹 가능 [S35] | 없음(스크린샷) | 큰 장면 화면 확보용 | P3 | 프론트엔드 |
| 명령 검색 | 있음: 탭 줄 위 검색 상자에 명령 이름 입력 후 결과 클릭 [S35], 검색 상자는 File 메뉴 상단에도 있음 [S34] | 없음 | 자체 도입 시 "명령 레지스트리"(PLAN 5.3) 위에 쉽게 가능 | P3 | 프론트엔드 |
| 툴팁 | 대부분의 명령에 단계형(progressive) 툴팁: 먼저 이름+짧은 설명, 계속 멈추면 확장 정보 [S1]. 지연 초는 응용 옵션 General(1·2단계 각각) [S25] | 접근성 이름 필수(PLAN 5.3), 툴팁 표시는 (미확인) | 확장 툴팁(설명·단축키) | P2 | 프론트엔드 |
| 아이콘 | 리본·브라우저 아이콘은 Light Theme 정의가 기본이고 "Amber"로 레거시 색 전환 [S1][S8]. (아이콘 새로 그림은 2020판 변경 [S9]) | 기능별 벡터 아이콘(STATUS) | 우리 것 사용(복제 금지) | - | - |

### 2.4 Browser (모델 트리)

| 항목 | 인벤터 2026 동작 (출처) | GraphCTRL 현재 | 차이 | 우선순위 | 담당 |
|---|---|---|---|---|---|
| 노드 구성 | 패널(탭)을 끌어 도킹·분리(float)·창 밖(다른 모니터) 배치, 환경별 브라우저 전환, 브라우저가 없으면 그래픽 좌상단 아이콘으로 표시. 노드: 파일·피처·관계 [S7] | 단일 "모델" 패널: 원점/스케치/파라미터/바디/로봇/좌표계/웨이포인트/경로. 아이콘 행 일부(review-25) | 작업 피처·필드·설비 트리·상태·작업 그룹 없음(M2-1·2·4 계획) | P1 | 둘 다 |
| End of Part(EOP) 마커 | 피처를 EOP 마커 아래로 끌어 임시 제외(예: 큰 패턴 생성 중 모깎기를 아래로), 다시 끌어 복원. 마커 아래 피처는 일괄 억제·억제 해제·삭제. 종속 피처는 부모를 내리면 함께 억제. 이동 중 Esc로 취소 [S4]. 우클릭 "Move EOP Marker"/"Move EOP to End" 메뉴명은 이전 판 도움말·검색 요약으로만 확인 [S4 외] (미확인: 2026 전문) | 없음 (PLAN 9.1에 "롤백 마커(파트 끝)" 계획) | 전체 없음. 우리 의미: 마커 아래 웨이포인트·경로·필드는 평가 제외 | P2 | 둘 다 |
| 이름 바꾸기 F2 | F2 = 브라우저 노드 이름 바꾸기(같은 키 F2는 팬, 둘 다 "Global" 분류) [S5]. 또 **선택된 노드를 한 번 더 클릭**하면 편집(전체 이름이 선택됨, 글자 단위 수정 가능). 더블클릭은 피처 편집이라 이름 바꾸기가 아님 [S7] | 이름 입력은 속성 패널. F2 이름 바꾸기 없음(PLAN 9.1 계획) | 트리 안 인라인 편집 없음 | P1 | 프론트엔드 |
| 가시성 | 2026 도움말에 전구/눈 아이콘 서술 없음. 확인된 것: 브라우저 우클릭 Visibility(작업 피처·스케치, 구 판 근거), 스케치 우클릭 Dimension Visibility, View > Visibility > Object Visibility로 전체 종류별 표시, Alt+V = 선택한 솔리드·작업 피처 가시성 토글 [S5][S31]. 아이콘 형태는 (미확인) | 바디·로봇에 체크박스(스크린샷) | 스케치·웨이포인트·경로·좌표계에도 필요 | P1 | 프론트엔드 |
| 억제(Suppress) | EOP 마커 아래로 끌어 억제하는 방식이 기본(위 EOP 행), 마커 아래 피처를 개별 억제/해제/삭제, 조건부 억제는 iPart·구성에 사용. 억제된 구속은 미해결 상태로 유지 [S4]. 우클릭 메뉴 문구는 (미확인) | 없음(PLAN 9.1에 억제 계획) | 억제 상태·평가 제외 규칙 없음 | P2 | 둘 다 |
| 끌어서 순서 변경 | 피처를 EOP 마커 기준으로 끌어 이동하는 것은 확인 [S4]. 일반 재정렬(트리 순서 변경)의 2026 근거는 (미확인) | 경로 안 웨이포인트는 ↑↓ 버튼만 | 트리 드래그 재정렬(종속 검사) 없음 | P2 | 둘 다 |
| 검색·필터 | 돋보기로 검색/필터 영역 토글. Quick Search는 전체 계층에서 파일 노드(접힘·펼침)·피처·관계·iProperty를 찾고, 공백 한 칸으로 키워드 구분, 결과에서 바로 편집, 닫기/취소 버튼. 필터는 "미해결 파일만"·"오래된 파일 제외"·초기화. 환경 전환·부품 편집 등 일부 작업이 검색을 자동 해제. Find in Browser 명령 별도 [S7] | 없음 | 검색창·필터 없음 | P2 | 프론트엔드 |
| 패널 도킹 | 브라우저 탭을 다른 패널로 끌어 도킹·분리·창 밖 이동 가능 [S7] | 고정 레이아웃(PLAN 5.3 "레이아웃 고정") | 의도적 차이(P3 이후) | P3 | 프론트엔드 |
| 선택 동기화·우클릭 | 트리·그래픽 선택 연동, 우클릭 문맥 메뉴 [S4] | 선택·컨텍스트 메뉴 있음(STATUS) | 메뉴 항목 인벤터식 정비(편집·가시성·억제·삭제·속성) | P1 | 프론트엔드 |
| 삭제 종속 안내 | 삭제 전에 지울 종속 피처가 그래픽·브라우저에 강조되고 확인 대화상자에서 처리 선택: 종속 피처·사용된 스케치 모두 기본 "삭제", 체크하면 유지(미해결 아이콘 표시). 종속 없는 피처는 바로 삭제 [S4] | 항목 삭제(×) 버튼, 종속 안내는 (미확인) | 종속 안내 필요(웨이포인트→경로) | P2 | 백엔드 |

### 2.5 Property Panel

| 항목 | 인벤터 2026 동작 (출처) | GraphCTRL 현재 | 차이 | 우선순위 | 담당 |
|---|---|---|---|---|---|
| 형태 | 그래픽 위에 **떠 있는(floating)** 패널, 처음 그래픽 좌상단에 나타남. 크기 조절, 모델 브라우저와 결합, 다른 위치·보조 모니터로 이동, 그래픽 창·브라우저 영역에 **도킹** 가능(다음 실행 때 마지막 위치 기억). 탭 대신 아코디언 그룹, 위→아래 워크플로, 도구 팔레트(패널 옆 분리·도킹), 마지막 표시 상태 기억 [S27](2026). 2026에서 패턴 명령이 속성 패널로 전환 [S22] | 오른쪽 고정 속성 창, 선택 항목별 섹션(스크린샷) | 도킹은 근접. 명령 중 모달 속성 패널(명령 UI)은 없음 | P1 | 프론트엔드 |
| 입력 흐름 | 패널·캔버스 편집 필드 공통. 값을 입력 중이면 Enter = 그 값만 적용(패널은 유지). 패널에 포커스가 있거나 값만 선택된 상태의 Enter = OK와 동일(완료 후 패널 닫힘). Ctrl+Enter 또는 Enter 두 번 = 피처 정의 완료 후 닫기. 미충족 선택 필드가 있으면 Enter는 Continue처럼 그 필드로 포커스 이동. Tab/Shift+Tab 다음/이전 필드(패널·캔버스 사이에서 교차 강조), 목록에서 첫 글자·위/아래 키로 선택, 잘못된 값은 빨강·유효는 검정, 필수 미입력은 빨강+경고 아이콘. 고급 메뉴 "Single ENTER to finish command"로 구형 Enter 동작 복원. Undo: 패널 표시 중 실행 취소는 명령 취소 [S27] | 즉시 반영형 필드, Tab 순서·Enter 규칙 (미확인) | 키 규칙 통일 필요 | P1 | 프론트엔드 |
| 빵부스러기(breadcrumb) | 피처↔스케치 편집 전환(전환 시 현재 변경 확정), 브레드크럼에서 피처·스케치 이름 바꾸기(클릭 두 번 후 Enter), 옆에 Preview 토글 [S27] | 없음 | 경로 만들기·필드 만들기 단계 표시용 | P3 | 프론트엔드 |
| OK/Apply/Cancel | 패널에 **OK**와 "**Apply and create new**"(현재 설정으로 만들고 같은 설정으로 다음 피처 계속) 확인. 닫기·취소 쪽 버튼의 정확한 라벨(Cancel 여부)은 도움말 문장에 없음 (미확인). 마킹 메뉴 3시 방향에 OK(필수 입력 미충족 시 비활성 OK 또는 Continue)가 표시됨 [S27]. 취소는 Esc(명령 종료) [S5] | 해당 버튼 없음, 취소는 Esc | 명령형 작업(필드·경로 만들기)에 확인/취소 필요 | P1 | 프론트엔드 |
| 프리셋 | 프리셋으로 재사용 가능한 피처 정의 저장(Presets 섹션) [S27] | 없음 | 로봇 툴·속도 프리셋은 P3 | P3 | 둘 다 |
| 수식 입력 | 패널·캔버스 숫자 필드에서 수식·파라미터 입력 가능. 피처 치수를 고르면 커서가 필드 끝에 남아 수식 작성이 쉬움, 파라미터 이름·수식 표시 전환(Value/Name/Expression) [S27][S31] | `gc_expr` 있음, 파라미터 표는 있음. 속성 필드의 수식 입력 지원은 (미확인) | 모든 수치 필드가 수식·파라미터 허용 | P2 | 둘 다 |
| 투명도 | floating·도킹은 확인(위 형태 행). 패널 **투명도 설정은 2026 속성 패널 페이지에 서술 없음** (미확인, 내비게이션 바에는 Active/Inactive opacity 있음 [S29]). 핀(pin) 개념도 속성 패널에는 없음(리본 패널 확장 핀만 [S35]) | 속성 창 조그의 투명도 슬라이더 | 투명도는 우리 고유(가시성·충돌 확인용) | - | - |

### 2.6 Mini-toolbar · 캔버스 안 치수 입력

| 항목 | 인벤터 2026 동작 (출처) | GraphCTRL 현재 | 차이 | 우선순위 | 담당 |
|---|---|---|---|---|---|
| 미니 툴바 | Shell·Face Draft·Chamfer·Joint 대화상자 옆의 미니 툴바를 View > User Interface > Mini-toolbar로 켜고 끔(기본 꺼짐, 옵션은 대화상자와 동일) [S24](2026 본문 확인). 평면·작업 평면 클릭 시 미니 툴바에 Create Sketch가 뜸 [S15]. 스케치 중 값 입력은 미니 툴바가 아니라 Heads-Up Display(Dynamic Input) [S32] | 없음. 계획만 있음(PLAN 9.1 "값 입력 미니 툴바") | 전체 없음 | P2 | 프론트엔드 |
| 스케치 치수 입력 | Dimension 명령으로 형상 선택 후 위치 클릭 → "Edit Dimension" 상자(수정: 치수 더블클릭, Dimension 명령 후 치수 클릭, 우클릭). 값 입력·화살표 메뉴(Measure·Show Dimensions·목록 값)·`HGHT = 5mm` 형태의 식 입력, **체크 표시 클릭으로 확정**(Enter 확정 문구는 도움말에 없음). 치수 텍스트는 커서를 올려 끌어 이동, 완전 구속 후 추가 시 Accept로 참조(구동 아님) 치수 [S31](2026) | 스마트 치수 있음(STATUS). 편집 상자 방식은 (미확인) | 캔버스 위 인라인 편집 상자(수식 허용) | P1 | 프론트엔드 |
| 선 그리기 중 길이·각도 입력 | Heads-Up Display(Dynamic Input): 응용 옵션 Sketch 탭에서 켬. 선·원·호·사각형 등에서 첫 점 후 입력 상자에 길이·각도·반지름 입력, Tab으로 다음 상자(입력 값은 잠금 아이콘), Enter로 완성(입력 값으로 치수 자동 생성), Esc 종료. 3D 스케치는 Ctrl+D로 Dynamic Dimension 토글 [S32][S31] | 없음 | 로봇 셀 스케치에는 중요(정확한 위치) | P2 | 프론트엔드 |

### 2.7 Marking menu

| 항목 | 인벤터 2026 동작 (출처) | GraphCTRL 현재 | 차이 | 우선순위 | 담당 |
|---|---|---|---|---|---|
| 호출 | 우클릭 시 커서를 중심으로 방사형 메뉴(노드마다 이름+아이콘). **우클릭을 누른 채 노드 쪽으로 끌고(경로 흔적 표시) 메뉴가 다 뜨기 전에 놓으면** 그 노드 명령 실행. 닫기는 **중앙 클릭**(Esc는 진행 중 명령을 취소하므로 쓰지 말 것). 위/아래 키로 항목 이동 후 클릭 확정, 클릭만 해도 선택. 비활성은 View > User Interface > Marking Menu 체크 해제 또는 "Use Classic Context Menu" [S28][S2][S3] | 우클릭 컨텍스트 메뉴(목록)(STATUS) | 방사형 메뉴·마크 모드 없음(PLAN 9.1 계획) | P1 | 프론트엔드 |
| 노드 수·방향 | 노드 위치를 North/South/East/West "and so on"으로 지정(Customize 대화상자) [S3]. 도움말에 **숫자는 없음** (미확인; 8은 Fusion 자료 [S23] 유추). 속성 패널 명령에서는 3시(East) 위치에 OK/Continue [S27] | - | 8방향 채택, 인벤터 값으로 보증하지 않음 | P1 | 프론트엔드 |
| 기본 명령 | 스케치: Center Point Circle, Line, Two Point Rectangle, Finish Sketch. 파트: Hole, Extrude, Fillet, Work Plane. 어셈블리: Constrain, Place Component, Move Component, Rotate Component(예시 목록) [S2]. 노드별 전체 기본 배치 표는 도움말에 없음 (미확인). 노드 삭제 우클릭 Remove, 복원 Reset Node [S28] | - | 우리 기본: 스케치 = 선·원·사각형·치수·마무리, 3D = 홈 뷰·이전 뷰·이동 기즈모·삭제 등 | P1 | 프론트엔드 |
| 오버플로 메뉴 | 방사형 위/아래(커서 위치에 따라)의 축약 컨텍스트 목록: Full Menu(기본), Radial Menu Only, Short Menu(자주 쓰는 명령) [S3][S2]. 표시 스타일 Icon+Text(기본)/Text Only/Icon Only [S3] | - | 방사형 + 나머지 목록 구조 채택 | P2 | 프론트엔드 |
| 사용자 정의 | Customize 대화상자 Marking Menu 탭: 환경·하위 환경별 노드 지정·명령 검색, Restore Default(현재/전체), XML Import/Export. **Ctrl+우클릭 전용 사용자 메뉴**를 하위 환경으로 정의(없으면 기본 메뉴) [S3][S28] | - | 명령 레지스트리 기반으로 데이터화 | P3 | 둘 다 |
| 환경별 메뉴 | 환경(스케치·파트·어셈블리·도면·용접 등)과 하위 환경마다 다른 메뉴, 애드인 제외 모든 환경에서 사용 [S2][S3]. Shift+우클릭은 Select 명령 메뉴 [S5] | - | 컨텍스트(스케치/3D/로봇/경로)마다 | P2 | 프론트엔드 |

### 2.8 ViewCube · Navigation Bar

| 항목 | 인벤터 2026 동작 (출처) | GraphCTRL 현재 | 차이 | 우선순위 | 담당 |
|---|---|---|---|---|---|
| ViewCube | 큐브 면·모서리·꼭짓점 클릭으로 프리셋 뷰, 면 뷰에서 가장자리 삼각형으로 인접 면, 롤 화살표로 90도 회전, 누른 채 끌어 오빗, 선택 객체가 있으면 그 중심 기준, 애니메이션 전환·Fit-to-View 옵션, 우클릭으로 Set Current View as Home/Front·Go Home·Reset Front [S14] | 26방향 피킹, 클릭, 드래그 오빗, 홈 버튼(PLAN 5.2). 면 라벨 "평면·정면·우측"(스크린샷) | 근접. 컴퍼스·UCS 메뉴·현재 뷰 아이콘은 (미확인) | P3 | 프론트엔드 |
| Navigation Bar | 공통 명령(ViewCube·SteeringWheels) + 제품별 명령: Zoom All, Zoom Window, Zoom, Pan, Zoom Selected, Rotate, Look At, View Previous/Next(기본 비표시, Customize로 추가) [S12](2026 본문 동일). 표시는 View > Windows > User Interface > Navigation Bar, Customize 메뉴에서 명령 추가·제거(우클릭 Remove), 도킹 위치(ViewCube와 연결 Link to ViewCube, 가장자리 따라 끌어 가로/세로 배치), Active/Inactive opacity [S29]. 기본 표시 명령 집합·정확한 순서는 문장으로 없음 (미확인) | 우측 상단 ViewCube뿐. 내비게이션 바 없음(스크린샷) | 바 없음: Zoom All·Zoom Window·Pan·Orbit·Look At 버튼 | P1 | 프론트엔드 |
| Orbit | F4 = 회전, 도구 모음에 Free Orbit [S5][S12] | Shift+가운데 드래그, F4+드래그(부록 B) | 구현됨(부록 B) | - | - |
| Pan / Zoom | F2 = 팬, F3 = 줌 [S5]. Shift+F3 = 영역 줌은 Inventor View 단축키 표에서 확인 [S40](Zoom Window 도구는 내비게이션 바 [S12]) | F2/F3 있음(부록 B), 영역 줌은 (미확인) | Zoom Window | P2 | 프론트엔드 |
| Look At | 모델에서 선택 요소(면)를 화면과 평행하게 확대·회전, 선택 모서리·선은 화면에서 수평으로, 도면에서는 쓰지 않음 [S12](2026 본문 동일) | 없음. 스케치 평면 정렬은 있음(미확인) | 면·평면 선택 후 Look At | P1 | 프론트엔드 |
| Full Navigation Wheel | SteeringWheels 중 크게/작게 표시되는 풀 휠(미니 버전 있음) [S13](2023) | 없음 | 휠 | P3 | 프론트엔드 |
| 이전/다음 뷰 | F5 이전(Global), Shift+F5 다음은 Inventor View 표 [S5][S40] | 구현(부록 B) | - | - | - |
| 홈 뷰·등각 | F6 = 등각 뷰 [S5], Home = 전체 보기(Zoom All)는 Inventor View 표 [S40] | Home/F6 있음(부록 B) | - | - | - |

### 2.9 선택 동작

| 항목 | 인벤터 2026 동작 (출처) | GraphCTRL 현재 | 차이 | 우선순위 | 담당 |
|---|---|---|---|---|---|
| 프리하이라이트 | 커서가 올라가면 강조. "Enable Prehighlight" 기본 켬(Select 풀다운에서도 켜고 끔), 끄면 어셈블리·용접 우선순위 선택 시 미표시지만 Select Other는 항상 강조. Enhanced Highlighting(다른 부품 너머 강조)·Highlight Hidden Faces 옵션 [S8] | 호버 강조(STATUS) | 옵션 없음(P3) | P3 | 프론트엔드 |
| Select Other | 선택 후 우클릭 > Select Other, 화살표로 후보 전환 [S11]. Ctrl+A = Select Other 드롭다운(모델 뷰에서 부품·서브어셈블리 선택 시) [S5]. 커서를 지오메트리 위에 멈추면 자동 표시되며 지연은 응용 옵션 General "Select Other delay (sec)"(숫자 또는 Off) [S25]. **기본 지연 값은 도움말에 없음** (미확인). 큰 어셈블리용 Optimized Selection(가까운 객체부터 순위, 클릭해 넘기면 다음 그룹)과 Locate tolerance(1~10픽셀) [S25] | 없음(스크린샷·STATUS에 없음) | 겹친 스케치·메시·웨이포인트 선택에 필요 | P1 | 프론트엔드 |
| 윈도/크로싱 | 좌상→우하 창은 완전히 안에 든 것만, 우하→좌상은 걸친 것 포함. **Shift 드래그 = 미선택만 추가(나머지 유지), Ctrl 드래그 = 창 안 모든 객체의 선택 상태 반전** [S11]. 빈 곳 클릭(좌클릭 또는 Shift+클릭)으로 선택 해제, Shift/Ctrl+클릭으로 개별 해제 | 좌→우 윈도(실선)·우→좌 크로싱(점선), Ctrl 토글, Shift 추가(PLAN 5.3) | 방향 규칙 일치. Ctrl 드래그는 반전(토글)이고 Shift 드래그는 추가임을 확인: 우리 Ctrl 토글·Shift 추가와 일치 여부만 점검 | P2 | 프론트엔드 |
| 선택 우선순위 필터 | Select 명령의 모드(Edge/Feature/Part Priority 등) [S11](세부 (미확인)) | 없음 | 우리: 로봇·설비·스케치·웨이포인트 필터 | P2 | 둘 다 |
| Esc / Delete | Esc = 명령 종료(Quit), Delete = 선택 삭제 [S5]. 선택 해제는 빈 곳 클릭 [S11] | 구현(PLAN 5.3) | - | - | - |

### 2.10 스케치 환경

| 항목 | 인벤터 2026 동작 (출처) | GraphCTRL 현재 | 차이 | 우선순위 | 담당 |
|---|---|---|---|---|---|
| 스케치 시작 | 3D 모델 탭 Start 2D Sketch(또는 3D Sketch), 브라우저 우클릭 New Sketch, 평면·작업 평면 클릭 후 미니 툴바 Create Sketch. 편집은 브라우저 스케치 더블클릭·우클릭 Edit Sketch·그래픽 평면/치수 우클릭 [S15] | 원점 평면·오프셋 평면(PLAN 9.1) | 임의 면·작업 평면·불러온 부품 면 위 스케치는 M2-1 계획 | P1 | 백엔드 |
| 스케치 환경 | 스케치 평면 + 스케치 명령, 활성일 때만 작성·구속·치수 [S15] | 스케치 환경 있음(STATUS) | - | - | - |
| 마무리/취소 | Sketch 탭 Finish Sketch 또는 그래픽 우클릭 "Finish 2D Sketch"(마킹 메뉴에도 Finish Sketch). 별도 Cancel Sketch 명령은 이번 편집의 모든 작업을 폐기(새 스케치면 스케치 생성까지) [S15][S2] | 마무리 있음(S 키) | 우클릭·마킹 메뉴에서 마무리 | P1 | 프론트엔드 |
| 구속 글리프 | 구속마다 글리프 아이콘, 글리프에 커서를 올리면 연관 형상·짝 글리프 강조, 우클릭 Hide [S17] | 구속 표시(F8)/숨김(F9)(PLAN 5.3). 글리프 호버 강조는 (미확인) | 호버 연동, 글리프별 숨김 | P2 | 프론트엔드 |
| F8/F9 | F8 모든 구속 표시, F9 숨김(둘 다 Sketch 분류, F7 Slice Graphics도 Sketch) [S5][S17] | 동일(PLAN 5.3) | - | - | - |
| 추론·지속 | Infer/Persist Constraints 설정, Ctrl 누르면 일시 해제 [S18](2014) | 스냅·추론은 gc_tools (STATUS). Ctrl 해제는 (미확인) | 설정·Ctrl 규칙 | P2 | 둘 다 |
| 치수 표시 | 구동 치수와 참조 치수 구분 | 참조 치수 색 `#808080`(부록 B) | 편집 상자 방식 (2.6) | P1 | 프론트엔드 |
| 스케치 도구 | 점·선·원·호·사각형·다각형·슬롯·모깎기·트림·연장·간격띄우기·투영(인벤터 기본, 2026 목록 (미확인)) | 선·원·호·사각형·치수·구속, 트림 계획(STATUS) | 다각형·슬롯·모깎기·트림·연장·간격띄우기·투영 (PLAN 9.1) | P2 | 백엔드 |
| 색 | 인벤터 스케치 색 (미확인) | 미구속 파랑·완전구속 검정·과구속 빨강(부록 B) | 사용자 설정 가능하게(P3) | P3 | 프론트엔드 |

### 2.11 상태바와 프롬프트

| 항목 | 인벤터 2026 동작 (출처) | GraphCTRL 현재 | 차이 | 우선순위 | 담당 |
|---|---|---|---|---|---|
| 상태바 | 명령 프롬프트(응용 옵션 "Show command prompting (Dynamic Prompts)"로 커서 근처 툴팁 표시 가능) [S25], 스케치 구속 표시 버튼(F8/F9), 스케치 치수 표시 아이콘(Value/Name/Expression/Tolerance/Precise Value), 3D 스케치 Dynamic Dimension 토글, F7 = Slice Graphics(스케치 평면이 드러나게 모델을 일부 절단) [S5][S31][S17] | 하단 "준비" + 오른쪽 "GPU 사용 중"(스크린샷). 명령 프롬프트는 계획만(PLAN 9.1) | 명령별 프롬프트·좌표·구속 상태·단위 | P1 | 프론트엔드 |
| 단면(Slice) F7 | 스케치 평면 기준 슬라이스 보기 | 없음 | 필드·스케치 편집 시 유용 | P3 | 프론트엔드 |

### 2.12 테마와 색

| 항목 | 인벤터 2026 동작 (출처) | GraphCTRL 현재 | 차이 | 우선순위 | 담당 |
|---|---|---|---|---|---|
| UI 테마 | 응용 옵션 Colors 탭 UI Theme: **Dark(Default)** / Light(+Amber 레거시 아이콘, Light 전용·재시작 필요). 아이콘 색·창 프레임·배경·속성 패널·대화상자에 적용. 도움말 주석에 Dark 테마가 "pre-release"로 표기 [S8](2026) | 밝은 테마 하나(스크린샷), 테마 토큰은 이미 있음(STATUS "테마 토큰") | Dark 테마 없음 | P1 | 프론트엔드 |
| 인캔버스 색 구성표 | 그래픽 창 색·배경(1 Color/Gradient/Background Image)·반사 환경·단면 질감은 UI 테마와 별개(목록에서 구성표 선택, Customize Schemes로 편집). "Light Theme" 구성표 + 연한 파란 배경 이미지는 2020판 추가 [S9], Colors 탭 "What's New" 연도 표기 2021·2021.1·2025·2026 [S8] | 그라데이션 배경 `#E6EAEE→#C3CAD3` 고정(부록 B) | 구성표 선택 없음 | P2 | 프론트엔드 |
| 조명 스타일 | Light Theme, Light Theme IBL(기본) 조명 스타일은 2020판 추가 [S9]. Graphic Presets(High Quality·Balanced·Performance) 3종도 같은 판 | 반구 조명(STATUS) | 조명 프리셋 없음(낮음) | P3 | 프론트엔드 |
| 사용자 정의 | Customize Schemes 편집기, 적용 후 재시작 [S8] | 없음 | 편집기는 P3 (재시작 불필요하게) | P3 | 프론트엔드 |

### 2.13 단축키와 사용자 정의

| 항목 | 인벤터 2026 동작 (출처) | GraphCTRL 현재 | 차이 | 우선순위 | 담당 |
|---|---|---|---|---|---|
| 기본 키 | 2026 표(일부)에서 확인: F1 도움말, F2 팬/이름 바꾸기, F3 줌, F4 회전, F5 이전 뷰, F6 등각, F7 Slice(Sketch), F8/F9 구속 표시·숨김(Sketch), Esc 종료, Delete 삭제, Ctrl+A Select Other, Ctrl+Z/Y, Shift+우클릭 Select 메뉴, Alt+V 가시성 [S5]. Shift+F3(영역 줌)·Shift+F5(다음 뷰)·Home(전체 보기)는 이 표에 없고 Inventor View 표에서 확인 [S40]. End 키는 확인 불가 (미확인) | 부록 B에 대부분 정의, End = 선택 줌은 우리 정의(인벤터 값 아님) | End 키 표기는 인벤터 근거 없음 | P3 | 프론트엔드 |
| 명령 별칭 | 기본 다문자 별칭 로드 시 AutoCAD식 별칭(C 원, L 선, D 치수, TR 트림, E 돌출, S 2D 스케치 등), 입력 대화상자·자동완성 옵션(응용 옵션 General), 가져오기 시 일부 단축키 무효 [S5][S25][S6] | 단일 키·Ctrl 조합만 | 별칭 없음 | P3 | 프론트엔드 |
| 사용자 정의 | Tools > Options > Customize > Keyboard 탭에서 확인·변경. 기능키는 예약돼 사용자 정의 불가(문자·구두점·Home/End/PageUp/Down/방향키와 Shift/Ctrl/Alt 조합만) [S5] | 없음(키맵은 데이터지만 UI 없음) | 키 사용자 정의 화면, 충돌 검출 | P2 | 프론트엔드 |
| 한글 자판 | 해당 없음/(미확인) | `physical_key` 매칭(PLAN 5.3) | 우리가 더 낫다(유지) | - | - |

## 3. 우선순위 구현 목록 (상위 25)

수용 기준은 한 줄. 담당: FE = 프론트엔드, BE = 백엔드.

### P1 (M2-1 게이트 전)
1. **Dark 테마 + 테마 선택(FE)**: 옵션에서 Dark/Light 전환 즉시 반영(인벤터 2026 기본은 **Dark**이므로 기본값 선택 검토), 모든 패널·리본·뷰포트 배경이 토큰으로 바뀌고 재시작 불필요(인벤터의 Light 전용 Amber는 복제 안 함), 스크린샷 테스트 2종.
2. **마킹 메뉴 1차(FE)**: 우클릭 누른 채 방향 드래그(경로 흔적 표시) 후 메뉴가 다 뜨기 전에 놓으면 해당 노드 실행, 천천히 놓으면 메뉴 유지 후 클릭 선택·위/아래 키 이동, **중앙 클릭으로 닫기(Esc 아님)**, 문맥(스케치/3D)별 노드 세트와 Ctrl+우클릭 보조 메뉴 자리, 방사형 아래 축약 목록(Full/Radial Only/Short), 클래식 컨텍스트 메뉴 복귀 설정.
3. **브라우저 F2 이름 바꾸기 + 인라인 편집(FE)**: 노드 선택 후 F2 **또는 선택된 노드 재클릭(더블클릭 아님: 더블클릭=편집 진입)** 으로 텍스트 편집(전체 선택 상태 시작), Enter 확정·Esc 취소, 중복 이름 거부, 실행 취소 1단계.
4. **브라우저 노드 확장(FE+BE)**: 작업 피처·필드·설비·상태·작업 그룹이 모델에 생기면 트리에 나타나고 선택이 뷰포트와 양방향 동기화.
5. **가시성 토글 전 노드(FE)**: 스케치·웨이포인트·경로·좌표계·필드에 눈/체크 토글(아이콘 형태는 인벤터 2026 도움말에 규정 없어 자체 설계), 선택 노드 가시성 토글 단축키(Alt+V에 대응), 저장·복원, 숨김은 피킹에서도 제외.
6. **문맥 탭 자동 전환(FE)**: 스케치 진입 시 별도 색(인벤터는 초록)의 문맥 스케치 탭이 나타나 활성, Finish Sketch 시 탭이 닫히고 이전 탭으로 복귀, kittest로 검증.
7. **Navigation Bar(FE)**: ViewCube와 연동 도킹(Link to ViewCube)되는 바에 Zoom All/Zoom Window/Zoom/Pan/Zoom Selected/Rotate/Look At 버튼(View Previous/Next는 Customize로 추가), 활성 도구 표시, Esc 종료, F2/F3/F4와 동일 동작, 바 Customize 메뉴(명령 추가·제거, 도킹 위치, 투명도).
8. **Look At(FE)**: 평면·스케치·좌표계를 고른 뒤 클릭하면 그 면 법선 방향으로 애니메이션 정렬(스케치 진입 시 자동 옵션).
9. **Select Other(FE)**: 우클릭 메뉴 "Select Other"(및 Ctrl+A)가 커서 아래 겹친 후보 목록을 보여주고 화살표/클릭으로 선택, 후보 호버 강조(프리하이라이트를 꺼도 항상 강조), 커서를 멈추면 자동 표시되는 지연 설정(숫자/끄기, 기본값은 인벤터 근거 없으니 자체 결정).
10. **캔버스 치수 편집 상자(FE)**: 치수 더블클릭(또는 치수 명령으로 배치 직후) 시 편집 상자, 수식·파라미터 이름·`이름 = 값` 식 허용, 화살표 메뉴로 Measure/치수 참조, 체크 버튼 + Enter 확정·Esc 취소, 잘못된 값은 빨강. 인벤터는 체크 클릭 확정만 문서화돼 Enter는 우리 확장.
11. **속성 패널 입력 규칙 + 확인/취소(FE)**: 값 입력 중 Enter는 그 값만 적용, 패널 포커스/값 선택 상태 Enter 또는 Ctrl+Enter(Enter 두 번)는 확정 후 닫기, 미충족 필드가 있으면 Enter는 그 필드로 이동(Continue), Tab/Shift+Tab 이동(패널↔캔버스 교차 강조), 명령형 패널에 OK + "적용 후 새로 만들기"(Apply and create new) + 취소(Esc, 라벨은 인벤터 근거 없음), 잘못된 값 빨강/유효 검정, "Enter 한 번에 완료" 설정, 패널 floating·도킹 및 위치 기억.
12. **상태바 프롬프트(FE)**: 활성 명령마다 다음 동작 안내문 표시(예: "첫 점을 선택"), 커서 좌표·구속 상태·단위 칸, 명령 종료 시 "준비".
13. **임의 면·작업 평면 위 스케치(BE)**: 불러온 부품 면(GLB 메시)·작업 평면 선택으로 스케치 생성, 저장·재열기 후 평면 참조 유지.
14. **GLB 어셈블리 불러오기(BE)**: GLB의 노드 계층·이름·색·인스턴스를 설비 트리로 불러오고 큰 파일은 작업 스레드(UI 프레임 무정지).
15. **브라우저 우클릭 메뉴 정비(FE)**: 노드 종류별 항목(편집·가시성·삭제·이름 바꾸기), 종속 항목이 있는 삭제는 확인 안내.

### P2
16. **End of Part(롤백) 마커(FE+BE)**: 항목을 마커 아래로 끌거나 마커를 끌어(우클릭 "여기로 이동"은 우리 확장) 아래 항목을 평가·그리기에서 제외, 종속 항목은 함께 억제, 이동 중 Esc 취소, 끌어 올리거나 "끝으로 이동"으로 복원, 마커 아래 일괄 억제/해제/삭제, 저장됨.
17. **억제(Suppress)(BE+FE)**: 웨이포인트·경로·필드를 억제하면 계획·표시에서 빠지고 해제 시 복원, 종속 경로에 경고.
18. **브라우저 검색·필터(FE)**: 돋보기 토글, 한 칸 공백 구분 다중 키워드, 종류별 필터(미해결·오래됨 등)와 초기화, 결과에서 바로 편집·선택·뷰포트 줌, 환경 전환 같은 작업 시 검색 자동 해제.
19. **트리 끌어서 순서 변경(FE+BE)**: 같은 그룹 안 드래그 재정렬, 종속 위반 시 거부와 이유 표시, 실행 취소 1단계.
20. **리본 드롭다운 변형(FE)**: 원(중심/3점)·사각형(2점/3점/중심)·호를 한 버튼+드롭다운으로 묶고 마지막 선택을 기억.
21. **Quick Access Toolbar(FE)**: 저장·실행 취소·다시 실행·열기 고정, 드롭다운 Customize 메뉴 체크 + 명령 우클릭 "QAT에 추가"로 추가·제거, 리본 위/아래 배치 선택, 넘치면 드롭다운, 설정 파일에 저장.
22. **값 입력 HUD(Dynamic Input)(FE)**: 선·원·사각형 등 명령 중 커서 근처 입력 상자에 길이·각도·반지름, Tab 이동(잠금 표시), Enter 완성(입력값으로 치수 생성), Esc 종료, 옵션에서 켜고 끔. (인벤터의 미니 툴바는 별개이며 기본 꺼짐.)
23. **인캔버스 색 구성표(FE)**: 밝은/어두운/중립 배경 구성표와 그리드·스케치 색 토큰 선택, 즉시 반영.
24. **단축키 사용자 정의 화면(FE)**: 명령 목록에서 키 재지정, 같은 컨텍스트 충돌 시 거부(기존 테스트 유지), 기본값 복원.
25. **스케치 도구 확장(BE)**: 다각형·슬롯·모깎기·트림·연장·간격띄우기·형상 투영, 각 도구에 솔버 시나리오 테스트.

### P3
- Home 화면(썸네일·고정·검색), 문서 탭, 프로젝트 개념, 풀 내비게이션 휠, 리본 최소화·명령 검색·사용자 패널, 패널 도킹, 슬라이스(F7), 사용자 색 구성표 편집기, 명령 별칭, 마킹 메뉴 사용자 정의.

## 4. 다음 확인 사항 (미해소만)

- 마킹 메뉴: 노드 개수(도움말에 숫자 없음)와 환경별 노드 배치 표 → 실제 인벤터 화면 확인.
- 속성 패널: 취소/닫기 버튼 라벨(Cancel 여부), 투명도 설정 존재 여부 → 실제 화면 확인.
- Select Other 지연 기본값(옵션 UI 스크린샷 필요), Navigation Bar 기본 표시 명령 집합.
- File(Application) 메뉴의 항목 전체 배치, 브라우저 가시성 아이콘 형태, Move EOP Marker 우클릭 메뉴의 2026판 문구, 2D 스케치 치수 편집 상자의 Enter 확정 여부.
- 사용자(인벤터 숙련자)에게 실제 화면을 확인받아 위 (미확인)을 채운다.
- 법무: 설정 항목 이름에 "Inventor" 사용 여부(2.8·1절).

## 출처

[S1]: https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-78BFC5E2-5323-48E3-A704-58DDD1836EFE.htm
[S2]: https://help.autodesk.com/cloudhelp/2023/ENU/Inventor-Help/files/GUID-4B081B74-45EA-451E-84BE-9FFE0E2ACAE3.htm
[S3]: https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-1A2766FB-EB3E-476B-9C44-D500A5137478.htm
[S4]: https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-C6A19638-C241-4E35-92D5-6C8D3D78B1C6.htm
[S5]: https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-DEEB0723-DFC7-4B28-95F4-099CC1E1EBF8.htm (2026 본문 확인)
[S6]: https://help.autodesk.com/cloudhelp/2024/ENU/Inventor-Help/files/GUID-428B9A8E-E513-44C8-888A-F6048AE89F8E.htm
[S7]: https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-EDF6B951-C309-44A8-868C-B72D99F6D5A7.htm
[S8]: https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-62188BB2-1880-4CDA-91DB-10237FC3ED99.htm
[S9]: https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-WhatsNew/files/GUID-333B7827-5CD7-4E79-810A-5BD1274254E6.htm (2026 사이트에 있으나 내용은 2020판 UI 개선)
[S10]: https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-4DFDDCD8-FF43-4B90-8EF6-82E8A76ED6BC.htm
[S11]: https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-B8F6E805-2E4B-4366-8FF6-33F0ED1C1C4A.htm
[S12]: https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-3B926737-92BA-46CF-B722-97F68651772B.htm
[S13]: https://help.autodesk.com/cloudhelp/2023/ENU/Inventor-Help/files/GUID-5D371AC3-EBC4-4B8D-9A9B-4B9B2B290D8B.htm
[S14]: https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-94F44C80-7313-4529-85D1-E64ABF732700.htm
[S15]: https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-E96FB3C4-136B-4551-BF45-F54594543ECB.htm
[S17]: https://help.autodesk.com/cloudhelp/2015/ENU/Inventor-Help/files/GUID-FAE12787-140F-49BE-8739-65B6A6F34043.htm
[S18]: https://help.autodesk.com/cloudhelp/2014/ENU/Inventor/files/GUID-1E504789-DFC6-4B59-81AA-1EFC1B656FB1.htm
[S19]: https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-3BF60B85-4BE9-4619-9FD3-0A179FA680E0.htm
[S20]: https://www.autodesk.com/blogs/design-and-manufacturing/autodesk-inventor-2026-whats-new/
[S22]: https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-WhatsNew/files/INV2026-PATTERNS.htm
[S23]: https://help.autodesk.com/cloudhelp/ENU/Fusion-GetStarted/files/GUID-6514ABC1-CB75-4F0B-AB0E-316FAD36BA93.htm (Fusion 문서: 인벤터가 아님, 대조용)
[S24]: https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-1FFC26DB-1410-422F-8629-9E11D46868DA.htm (2026 본문 확인)
[S25]: https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-A8E50133-E4FF-4459-B2AE-9568D2C9F565.htm
[S26]: https://help.autodesk.com/cloudhelp/2015/ENU/Inventor-Fundamentals/files/GUID-360EDBEB-2D3C-4562-AE91-94CBD0F580FF.htm
[S27]: https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-3E5505B9-9F60-43A0-BF32-97D183B91A70.htm (About Property Panels)
[S28]: https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-756BC728-3366-497D-B480-4A06E932ED20.htm (To Work with Marking Menus)
[S29]: https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-FAAB453E-31EB-46B8-9A74-3BF7A4D88EC9.htm (To Work with the Navigation Bar)
[S31]: https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-026FE8D3-AFBB-4834-BC18-73D9CFF78185.htm (To Work with Sketch Dimensions)
[S32]: https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-33A8B11D-A04C-4809-BC82-C0419215915F.htm (Dynamic Input, HUD)
[S33]: https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-84B053B0-E83B-4D9C-8436-1BE9AD5E9EBE.htm (To Work with Graphics Windows, 문서 탭)
[S34]: https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-DBFC94B3-D287-4C07-9790-B76A2DCE9F69.htm (To Search for Commands)
[S35]: https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-887E98E8-57B5-424F-83F6-D91274F9F268.htm (To Work with the Ribbon)
[S36]: https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-34126F60-3093-4144-8AA5-809B4D35DCA1.htm (To Work with Projects, File > Manage Projects)
[S37]: https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-1898C432-CFD2-46F1-8BAE-631392DFBF1D.htm (Sketch Tab Reference; File 메뉴 항목은 08D6DD13 Saving and Exporting 검색 요약)
[S38]: https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-3E3537A9-565E-4F2E-B69D-069EDCDF1AE1.htm (Select Command Reference)
[S39]: https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-8E3158B6-56F5-4D74-9B6A-5D5B6049116A.htm (User Interface 목차)
[S40]: https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-E13A932A-6102-46E9-A132-05F205689777.htm (Inventor View 단축키 표: Shift+F3, Shift+F5, Home)
