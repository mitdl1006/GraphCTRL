# 메뉴·브라우저 조작 대조표 (프론트엔드, 2026-11-18)

목적: Inventor 2026 원문 기준으로 파일(애플리케이션) 메뉴·빠른 실행 도구 모음·브라우저·마킹 메뉴의 항목과 조작을 우리 앱과 맞춘다.
상태: ✓ 같음 · △ 일부(차이를 적음) · ✗ 없음(채울 것) · ⛔ 대상 없음(우리 앱에 해당 개념이 없음) · ? 원문을 읽지 못해 확정 못 함.
근거 등급: **원문** = 2026 도움말을 curl로 읽음, **구판** = 2026 쪽이 404여서 구판 원문(연도 표기), **기록** = docs/ux 기존 조사.

읽은 원문(2026 쪽 기준):
- Inventor Browser, GUID-EDF6B951-C309-44A8-868C-B72D99F6D5A7 (개요·검색·필터·노드 이름 바꾸기)
- Working with the Assembly Browser, GUID-5F975252-BDB4-488C-A2C5-8B15C7D3C641 (브라우저 메뉴·표시 설정·폴더·우클릭)
- Assembly Browser Reference, GUID-CB6B1637-71AD-4753-8CC9-19559BFEE3A5 (아이콘 목록)
- To Customize the Quick Access Toolbar, GUID-3BF60B85-4BE9-4619-9FD3-0A179FA680E0 (QAT; 2024 쪽 문서)
- Application menu, GUID-39DE5C99-A7AD-41CD-AB63-DEA72BFCFC28 (**2014 쪽만 열림**; 2026 쪽은 404)
- Frame Generator browser, GUID-36AE9264-912D-4D3C-BB24-7EC16FA8777E (Find in Window, Isolate의 뜻)
- To Work with Graphics Windows, GUID-84B053B0-E83B-4D9C-8436-1BE9AD5E9EBE (Clean Screen)

**읽지 못한 것(리드 확인 요청)**: 브라우저 노드 종류별 우클릭 메뉴의 완전한 항목·순서 표와 마킹 메뉴 방향별 항목 표는 2026 원문 쪽에서
찾지 못했다(Assembly Browser 페이지는 "Depending on the browser configuration and the selected item, some or all options are available"로
끝나고 항목을 나열하지 않는다). 아래 우클릭 항목은 원문에 낱개로 언급된 것(Open, Find in Window, Isolate, Demote, Copy, Activate, Override,
Suppress, Visibility, Grounded, iProperties)을 기준으로 했고, 완전한 표는 사용자가 실제 Inventor 화면에서 확인해 주지 않으면 만들 수 없다.

## 1. 파일(애플리케이션) 메뉴

| 원문 항목 | 우리 앱 | 상태 |
|---|---|---|
| 명령 검색 (구판) | 없음 | ✗ (작음: 메뉴 항목 이름 검색 상자) |
| New (템플릿 선택) | 파일 > 새로 만들기 (Ctrl+N), 템플릿 없음 | △ 템플릿 개념 없음 |
| Open (Ctrl+O) | 파일 > 열기… (Ctrl+O) | ✓ |
| Save / Save As / Export (구판 "Save or Save As and Export") | 저장 (Ctrl+S) · 다른 이름으로 저장 (Ctrl+Shift+S) | △ Save Copy As·Export(다른 형식) 없음 — 내보내기는 백엔드 형식 필요 |
| Manage > iProperties (구판: Save 탭 접근) | 도구 > iProperties… (문서) | △ 위치가 다름(파일 > 관리 아래가 원문) |
| Print / 3D 인쇄 서비스 | 없음 | ⛔ 인쇄 개념 없음 |
| Recent 문서 / 열려 있는 문서 | 파일 > 최근 파일 | △ 열려 있는 문서 목록은 단일 문서라 해당 없음 |
| Options (응용 프로그램 옵션) | 파일 > 응용 프로그램 옵션 | ✓ (스케치 탭만) |
| 닫기 / 종료 | 파일 > 종료 | ✓ |
| 예제 열기(우리 고유) | 파일 > 예제 열기 | 고유 |

## 2. 빠른 실행 도구 모음 (QAT)

| 원문 항목 | 우리 앱 | 상태 |
|---|---|---|
| Undo / Redo | 메뉴 줄 오른쪽 ← → | ✓ (이번에 추가) |
| New / Open / Save | + · O · S 글자 단추 | △ 아이콘이 아닌 글자(Autodesk 아이콘 금지라 자체 아이콘이 필요) |
| Return ("QAT에서만 가능, 제자리 편집을 끝내고 이전 편집 상태로") | ↑ 단추(제자리 편집 중에만 활성) | ✓ 이번에 추가 |
| 리본 명령 우클릭 > Add to the QAT | 없음 | ✗ |
| QAT 위치: 리본 위/아래 | 없음 | ✗ |
| Customize 드롭다운(체크로 켜고 끔) | 없음 | ✗ |

## 3. 브라우저

### 3.1 브라우저 자체 (원문: Inventor Browser / Working with the Assembly Browser)

| 원문 항목 | 우리 앱 | 상태 |
|---|---|---|
| 빠른 검색(Find)으로 파일·피처·관계·iProperties 검색, 공백으로 키워드 구분 | 찾기 상자(이름 부분 일치) | △ iProperties·인스턴스 속성 일치 아이콘 없음 |
| 필터: 미해결 파일만 / 오래된 파일 제외 / 필터 지우기 | 없음 | ✗ (조건 검색: 문제 있는 관계·억제된 것 등은 백엔드 상태가 있어 가능) |
| 브라우저 메뉴 > Find | 있음 | ✓ |
| Edit Values in Browser 켜기/끄기 | "…" 메뉴 | ✓ |
| Display Preferences: Hide UCS | — | ⛔ UCS 없음 |
| Hide Work Features(원점 폴더 포함) | "…" 메뉴 | ✓ |
| Show Children Only | 없음 | ✗ (서브어셈블리 편집 중 부모 숨김) |
| Hide Fully Constrained | "…" 메뉴 | ✓ |
| Hide Notes / Hide Documents | — | ⛔ |
| Hide Warnings | 없음 | ✗ (관계 경고 표시 끄기) |
| Show Extended Names | 없음 | ✗ |
| Show Constraint State(●/○/—) | "…" 메뉴, 관계 상태 점 | ✓ |
| 폴더: Model States / Relationships / Representations | 관계·표현 폴더 있음, Model States 없음 | △ |
| 사용자 정의 폴더(User-defined Folder) | 없음 (서브어셈블리 "묶기"만) | ✗ |
| 노드 이름 바꾸기: 선택 후 한 번 더 클릭(더블클릭 아님) | 있음 | ✓ |
| 더블클릭 = 편집 상태 진입 | 스케치·작업 피처·컴포넌트 | ✓ |
| 패널 도킹·끌어 옮기기·+ 로 패널 추가 | 좌/우 패널 접기만 | △ |
| 브라우저 노드에 커서를 올리면 뷰에 경계 상자 (기록) | 없음 | ✗ |
| 브라우저 선택 → 뷰 강조 | 있음 | ✓ |
| 뷰에서 선택 → 브라우저 강조 (원문 View 문서는 "highlight 안 함"이라 했으나 Inventor 본체는 함) | 있음 | ✓ |
| 아이콘: 고정(압정)·비활성(배경 스타일)·비가시·다중 바디·표면·가상·표준 부품 | 압정·비활성·비가시 | △ 부품 종류 아이콘은 우리 개념과 다름 |
| 관계 아이콘 종류(Mate/Flush/Angle/Tangent/Insert/Rotation …) | 관계 종류별 글리프 | △ 확인 필요 |

### 3.2 노드 종류별 우클릭 (원문에 낱개로 나온 항목 기준)

| 항목 | 어셈블리 루트 | 컴포넌트 | 서브어셈블리 | 원점 폴더·요소 | 관계 | 스케치 | 작업 피처 | 로봇·링크·툴 |
|---|---|---|---|---|---|---|---|---|
| Open (별도 세션) | ⛔ | ⛔ | ⛔ | — | — | — | — | ⛔ |
| Edit (제자리 편집) | — | ✓ 제자리 편집 | ✓ | — | ✗ 관계 편집(구속 대화상자 열기) | ✓ 스케치 편집 | ✓ | △ |
| Find in Window | ✗ | ✓ 이번에 추가(바디·로봇) | ✗ | — | — | ✗ | ✗ | ✓ 로봇 |
| Isolate / Undo Isolate | ✗ | ✗ | ✗ | — | — | — | — | ✗ |
| Visibility | — | ✓ 보이기/숨기기 | ✓ | ✓ 원점 표시/숨기기 | — | ✓ | ✓ | ✓ |
| Enabled | — | ✓ | ✗ | — | — | — | — | ✓ |
| Grounded | — | ✓ | ✗ | — | — | — | ✓(작업 점은 항목 없음) | ✓ |
| Flexible | — | — | ✓ | — | — | — | — | — |
| Adaptive | — | ⛔ | ⛔ | — | — | ⛔ | — | — |
| Suppress | — | ✓(억제 가능한 것) | ✗ | — | ✓ | — | — | — |
| Copy / Paste | — | △ Ctrl+C/V만, 우클릭 항목 없음 | ✗ | — | — | — | — | — |
| Delete | — | ✓ | ✗ | — | ✓ | ✓ | ✓ | ✓ |
| Rename | — | ✓ | ✓ | — | ✓ | ✓ | ✓ | ✓ |
| Demote / Dissolve | — | ✓ 묶기 | ✓ 해체 | — | — | — | — | — |
| iProperties | — | ✓ 이번에 추가 | ✗ | — | — | — | — | — |
| Drive / Lock / Protect / Limits | — | — | — | — | ✓ | — | — | — |
| New Sketch(원점 평면) | — | — | — | ✓ 스케치 만들기 | — | — | — | — |
| Activate(표현) | — | — | — | — | — | — | — | — (표현 폴더 ✓) |
| Override(위치 표현) | ✗ | ✗ | — | — | ✗ | — | — | — |
| Expand/Collapse All | ✗ | ✗ | ✗ | — | — | — | — | — |

### 3.3 끌어 놓기·선택

| 원문 | 우리 앱 | 상태 |
|---|---|---|
| 위치 표현 끌어 이동·재정렬, Ctrl/Shift 선택 | 표현 재정렬 없음 | ✗ |
| 컴포넌트를 끌어 다른 폴더(서브어셈블리)로 옮기기 | 우클릭 "이동"(MoveInto) | △ 끌어 놓기 없음 |
| 여러 컴포넌트 선택 후 우클릭 (Open 등) | 다중 선택 Ctrl/Shift 일부 | △ 확인 필요 |

## 4. 마킹 메뉴(뷰 우클릭)

원문 방향별 항목 표를 읽지 못했다(위 안내). 현재 구현: 조립 환경 N/E/S/W에 Constrain·Move·Rotate·Place, 오버플로에
스케치 만들기·표시/숨기기·삭제·Flexible·Select Other·Look At·Home·Fit All; 스케치 환경 별도 세트. 방향별 표는 확인 후 채운다.

## 5. 이번에 채운 것 / 다음에 채울 것

채움(이번 커밋): Find in Window(바디·로봇), 빠른 실행 도구 모음(New·Open·Save·Undo·Redo·Return).

다음(원문 근거가 있는 것부터):
1. Isolate / Undo Isolate (브라우저·뷰 우클릭; 원문: "Select All Visible … to restore an Isolate Components operation")
2. 브라우저 Display Preferences: Show Children Only, Hide Warnings, Show Extended Names
3. 브라우저 필터(문제 있는 관계·억제된 것) · 사용자 정의 폴더
4. 노드에 커서를 올리면 뷰에 경계 상자
5. 서브어셈블리의 Enabled·Grounded·Suppress, 컴포넌트 끌어 놓기로 폴더 이동

리드에게 요청(백엔드 데이터 필요): 사용자 정의 폴더 저장, Isolate 상태(실행 취소 가능한 가시성 묶음), 브라우저 필터용 상태 질의,
Model States.
