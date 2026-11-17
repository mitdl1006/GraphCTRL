# Autodesk Inventor 2026 어셈블리 환경 UI 명세와 GraphCTRL 비교

목적: GraphCTRL의 작업 환경을 Inventor와 최대한 비슷하게 만들기 위한 기준 문서. 각 항목에 Inventor 2026 도움말 원문 출처와
GraphCTRL의 현재 상태(있음 / 없음 / 다름)를 적었다. 아이콘과 그림은 넣지 않았다.

## 0. 수집 범위 (먼저 읽을 것)

- **원문으로 읽은 곳**: 아래 §1 출처 표의 S1–S8, 그리고 추가로 S11–S22(curl로 본문 확인).
- **검색 결과만 본 곳**: 리본 탭 중 Inspect, Manage의 패널 목록, Tools 탭의 전체 패널 목록. 검색 요약에만 있던 내용은 "검색 요약"으로 표시했다.
- **미수집**: 리본 탭 전체 패널·명령 목록, 상태바 항목 목록(위치와 역할만 확인), 브라우저 노드 전체.
- 도움말 목차는 자바스크립트 앱이라 직접 받지 못한다. 대신 상위 페이지(User Interface, Relationships 등)의 하위 링크를 따라갔다.

## 1. 출처 (GUID)

| 번호 | 페이지 | 링크 | 수집 |
|---|---|---|---|
| S1 | About Inventor Environments | [GUID-2D47FF53…](https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-2D47FF53-8016-4309-ABAC-C9F445F9F992.htm) | curl 원문 |
| S2 | About Marking Menus | [GUID-4B081B74…](https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-4B081B74-45EA-451E-84BE-9FFE0E2ACAE3.htm) | curl 원문 |
| S3 | Assembly Modeling Fundamentals | [GUID-5A8DD595…](https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-5A8DD595-7BD4-4B15-A48B-32AC0245FBB0.htm) | curl 원문 |
| S4 | Keyboard Shortcut Reference | [GUID-DEEB0723…](https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-DEEB0723-DFC7-4B28-95F4-099CC1E1EBF8.htm) | curl 원문(페이지가 "일부 표"라고 밝힘) |
| S5 | Place Constraint Reference – Assembly Tab | [GUID-EBD2C784…](https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-EBD2C784-0E71-452A-B5F2-22738D22D45A.htm) | curl, 앞부분만 읽음 |
| S6 | Defining Positions and Relationships with Constraints and Joints | [GUID-2DFEAF48…](https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-2DFEAF48-DCBF-4781-9AC7-76D91DC2D896.htm) | curl 원문 |
| S7 | Studio Browser and Context Menu Reference | [GUID-754DBBC0…](https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-754DBBC0-3461-418F-BE2B-29A0CA0ECA92.htm) | curl 원문(페이지 주제는 Studio지만 조립 컴포넌트 메뉴 항목이 들어 있음) |
| S8 | About the Autodesk Inventor View Interface | [GUID-E13A932A…](https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-E13A932A-6102-46E9-A132-05F205689777.htm) | curl 원문(옛 도구막대 설명) |
| S9 | Place Constraint Reference (검색 결과) | [GUID-198F05E4…](https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-198F05E4-2973-4818-8F25-A0B8B9796D07.htm) | 미수집 |
| S10 | About the Ribbon (2022 판, 검색 결과) | [GUID-78BFC5E2…](https://help.autodesk.com/cloudhelp/2022/ENU/Inventor-Help/files/GUID-78BFC5E2-5323-48E3-A704-58DDD1836EFE.htm) | 미수집, 2026 판 아님 |
| S11 | User Interface (상위 목차) | [GUID-8E3158B6…](https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-8E3158B6-56F5-4D74-9B6A-5D5B6049116A.htm) | curl 원문 — 하위 17개 링크 |
| S12 | About the Ribbon | [GUID-78BFC5E2…](https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-78BFC5E2-5323-48E3-A704-58DDD1836EFE.htm) | curl 원문 |
| S13 | To Work with the Navigation Bar | [GUID-FAAB453E…](https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-FAAB453E-31EB-46B8-9A74-3BF7A4D88EC9.htm) | curl 원문 |
| S14 | About Reorienting the View (ViewCube) | [GUID-4A18A2AC…](https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-4A18A2AC-69F2-44DD-AD07-B8733321E8C9.htm) | curl 원문 |
| S15 | To Reorient the View | [GUID-94F44C80…](https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-94F44C80-7313-4529-85D1-E64ABF732700.htm) | curl 원문 |
| S16 | Inventor Browser | [GUID-EDF6B951…](https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-EDF6B951-C309-44A8-868C-B72D99F6D5A7.htm) | curl 원문(앞부분) |
| S17 | About Graphics Windows | [GUID-7544AABF…](https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-7544AABF-95ED-4062-A27A-6F60AD9379A8.htm) | curl 원문(앞부분) — 상태바 위치 |
| S18 | Relationships: Constrain (Place Constraint Reference) | [GUID-198F05E4…](https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-198F05E4-2973-4818-8F25-A0B8B9796D07.htm) | curl 원문 — 접근 경로 |
| S19 | Create Joints Reference | [GUID-6AA68E8F…](https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-6AA68E8F-7C97-4806-8483-3941DE915E70.htm) | curl 원문(앞부분) |
| S20 | To Place Mate or Flush Constraints in Assemblies | [GUID-0F500089…](https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-0F500089-4F69-41C5-AFE8-B58AC9E8E15E.htm) | curl 원문 |
| S21 | Reference for Marking Menu Tab of Customize Dialog | [GUID-1A2766FB…](https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-1A2766FB-EB3E-476B-9C44-D500A5137478.htm) | curl 원문(접근 경로만) |
| S22 | Keyboard Shortcut Reference → Work Environment 하위 | [GUID-DEEB0723…](https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-DEEB0723-DFC7-4B28-95F4-099CC1E1EBF8.htm) | (S4와 같음) |

## 2. 리본 탭 (Ribbon)

리본은 작업별 탭이고, 각 탭은 패널로 나뉜다(S12). 탭 목록과 패널 이름은 아래에서 원문으로 확인된 것만 적었다.
정확한 패널 전체 목록은 목차 확보 후 채운다(§7).

| 탭 | 원문으로 확인된 패널·명령 | 출처 | GraphCTRL | 상태 |
|---|---|---|---|---|
| 조립 (Assemble) | 관계(Relationships) 패널: 구속(Constrain), 조인트(Joint), 조립(Assemble, 옛 방식). 구성(Component) 패널: **배치(Place)**, Content Center에서 배치(검색 요약). 생산성(Productivity) 패널: **구성요소 원점에 배치(Place at Component Origin)**. 관리(Manage) 패널의 부품 목록은 검색 요약 | S18, S19, S20, S6, GUID-2C55B507(원문 확인), GUID-E7A45820(원문 확인) | "로봇" 탭: 로봇 추가, 바디 추가, 좌표계, 경로, 계획, 재생. 구속·조인트 탭 없음 | 다름 |
| 설계 (Design) | 미수집 | — | 없음 | 없음 |
| 3D 모델 | 미수집 | — | "3D 모델" 탭: 3D 스케치, 3D 구속 | 다름 |
| 스케치 | 작성(Create) 패널: 문자(Text) 등. 접근 경로는 Format Text Reference([GUID-23757F14…](https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-23757F14-BE7A-479B-B967-C79A71FBFCF1.htm)). 2D 스케치 명령 전체는 미수집 | — | "스케치" 탭: 선·원·호·사각형, 구속, 치수, 스케치 마무리 | 있음(부분) |
| 검사 (Inspect) | 간섭(Interference) 패널: **간섭 분석(Analyze Interference)**, **접촉 솔버 활성화(Activate Contact Solver)** — GUID-80DED0A6, GUID-FA1A8282(원문 확인). 패널 전체 목록은 미수집 | GUID-80DED0A6, GUID-FA1A8282 | 도구 메뉴 > 간섭 분석. 접촉 솔버 없음 | 다름 |
| 도구 (Tools) | 측정(Measure) 패널: **Measure**(원문 확인, GUID-1805AB8F). 옵션(Options) 패널: 사용자 지정(Customize), 응용 프로그램 옵션(Application Options), 문서 설정(Document Settings), 애드인(Add-ins). 재질·외관(Material and Appearance) 패널: 외관(Appearance) 등([GUID-5F5AB893…](https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-5F5AB893-224F-4F62-BE55-7FD018F059E2.htm)) | S21, 위 링크 | 메뉴 "도구"(포즈 변환 계산기, 측정, 성능 오버레이, AI 에이전트 연결). 리본 탭 아님 | 다름 |
| 관리 (Manage) | 원문 미확인 | — | 없음 | 없음 |
| 뷰 (View) | 외형(Appearance), 창(Windows) 패널. 창 패널에 사용자 인터페이스(User Interface) 드롭다운: 탐색 막대, 상태바, 마킹 메뉴 체크 | S13, S17, S21 | "뷰" 탭: 홈, 정면, 평면, 우측, 전체 보기, 직교/원근 | 있음(부분) |
| 환경 (Environments) | 시작(Begin) 패널: Studio 등 특수 환경 진입. 종료/마침은 오른쪽 끝 | S1, S7 | 없음 | 없음 |
| 주석 (Annotate) | 표(Table) 패널: 개정 표([GUID-6DB7770F…](https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-6DB7770F-D1A0-44E2-8898-6C39AC57222A.htm)) | 원문 | 없음 | 없음 |
| 로봇 | (Inventor에 없음) | — | "로봇" 탭 | 추가 기능 |

## 3. 환경 (Environments)

| 항목 | Inventor 2026 | 출처 | GraphCTRL | 상태 |
|---|---|---|---|---|
| 기본 환경 | 부품, 드로잉, 시트 메탈, 부품, 어셈블리, 용접 어셈블리, 프레젠테이션. 파일 종류와 1:1 | S1 | 단일 셀 작업 공간. 파일 종류 구분 없음 | 다름 |
| 어셈블리 환경에 포함되는 것 | Studio, BIM 콘텐츠, 용접(변환 후), 동역학·응력 등(전문가판) | S1 | 없음 | 없음 |
| 스케치 환경 진입 | 2D·3D 스케치는 부품·어셈블리·드로잉 안에서 | S1 | 스케치 탭에서 평면 선택 후 시작, 마무리로 나감 | 있음 |
| 환경 종료 | 오른쪽 끝 종료/마침, 현재 편집 취소는 Return | S1 | 스케치 마무리 버튼 | 다름(동일 개념, 위치 다름) |

## 4. 마킹 메뉴 (Marking Menu)

| 항목 | Inventor 2026 | 출처 | GraphCTRL | 상태 |
|---|---|---|---|---|
| 방식 | 그래픽 창에서 우클릭하면 커서 중심의 방사형 메뉴. 항목(노드)은 부채꼴 영역 | S2 | 우클릭 시 일반 컨텍스트 메뉴 | 다름 |
| 마크 모드 | 메뉴를 열지 않고 항목 쪽으로 획을 긋고 바로 떼면 선택(제스처) | S2 | 없음 | 없음 |
| 보조 메뉴 | 방사형 메뉴 위나 아래에 짧은 컨텍스트 메뉴(오버플로) | S2 | 없음 | 없음 |
| 어셈블리 항목 | 구속(Constrain), 부품 배치(Place Component), 이동, 회전 | S2 | 이동·회전은 기즈모로만 가능 | 다름 |
| 스케치 항목 | 원(중심점), 선, 두 점 사각형, 스케치 마무리 | S2 | 스케치 도구는 리본에만 있음 | 다름 |
| 켜고 끄기 | 도구 탭 > 옵션 패널 > 사용자 지정 > 마킹 메뉴 탭(S21). 검색 요약에는 뷰 탭 > 사용자 인터페이스 경로도 있음(원문 미확인) | S21 | 없음 | 없음 |

## 5. 브라우저, 상태바, ViewCube, 탐색 막대

### 5.1 브라우저 (Browser)
| 항목 | Inventor 2026 | 출처 | GraphCTRL | 상태 |
|---|---|---|---|---|
| 구조 | 파일 계층(부품, 조립품, 프레젠테이션, 도면). 환경마다 다름. 특징(feature)은 만든 순서로 나열 | S16, S8 | 원점, 스케치, 파라미터, 바디, 로봇, 좌표계, 웨이포인트, 경로 | 다름(셀 전용 노드) |
| 패널 구성 | 패널을 끌어 옮기기·합치기·떼기, 여러 모니터 배치, 닫기, 숨은 패널 추가 | S16 | 고정 패널, 떼어내기 없음 | 없음 |
| 검색·필터 | 빠른 검색(파일 노드, 특징, 관계, iProperty), 필터로 노드 종류 제한 | S16 | 없음(항목은 전부 표시) | 없음 |
| 환경 전환 | 보기 탭 > 창 패널 > 전환(Switch) 목록으로 문서 전환. 브라우저가 안 보이면 왼쪽 위 아이콘 | S16, S17 | 없음(단일 셀) | 없음 |
| 우클릭 메뉴(컴포넌트) | 위치 찾기, 도움말, 상단 구속 선택, 모든 구속 선택, 구성요소 애니메이션, 열기, 구성요소 하위 메뉴, 표시 여부 | S7(Studio 페이지 안의 조립 항목) | 숨기기/보이기, 삭제, 계획, 스케치 편집 | 다름(항목 수 적음) |
| 조인트 메뉴 | 연결을 우클릭하면 잠금(Lock), 고정(Ground), 보호(Protect) | S6 | 조인트 개념 없음 | 없음 |
| 고정(Ground) | 구성요소를 공간에 고정. 원점에 고정 가능 | S6, S3 | 없음 | 없음 |

### 5.2 상태바
| 항목 | Inventor 2026 | 출처 | GraphCTRL | 상태 |
|---|---|---|---|---|
| 위치·역할 | 창 아래 끝. 명령의 상황별 메시지 표시. 보기 탭 > 창 패널 > 사용자 인터페이스 > 상태바 체크로 켜고 끔 | S17 | 아래에 안내 문구, "N개 치수 필요"/"완전 구속", GPU 사용, AI 에이전트 알림 | 있음(역할 비슷) |
| 항목 목록 | 미수집(스케치 완전 구속 표시 등은 검색 요약, 원문 미확인) | — | 위와 같음 | 부분 |

### 5.3 ViewCube
| 항목 | Inventor 2026 | 출처 | GraphCTRL | 상태 |
|---|---|---|---|---|
| 구성 | 클릭 가능한 26개 부분: 면 6개(위, 아래, 앞, 뒤, 왼쪽, 오른쪽 정투영), 모서리(12), 꼭짓점(8, 3/4 뷰) | S14 | 면 라벨 평면·정면·우측 등 클릭 가능 (정확한 개수 미확인) | 부분 |
| 면 클릭 | 해당 정투영 뷰 | S14 | 있음 | 있음 |
| 모서리·꼭짓점 클릭 | 모서리는 반 뷰, 꼭짓점은 3/4 뷰 | S14 | 미확인 | 미확인 |
| 드래그 | 자유 회전 | S14 | 있음(뷰 조작) | 있음 |
| 홈 뷰 | 정의·복원 가능 | S14 | 홈(F6) | 있음(이름 다름) |
| 인접 면 전환 | 면 뷰일 때 네 방향 삼각형으로 인접 면 뷰로 전환 | S14, S15 | 없음 | 없음 |

### 5.4 탐색 막대 (Navigation Bar)
| 항목 | Inventor 2026 | 출처 | GraphCTRL | 상태 |
|---|---|---|---|---|
| 켜고 끄기 | 보기 탭 > 창 패널 > 사용자 인터페이스 > 탐색 막대 | S13 | 없음 | 없음 |
| 명령 구성 | 탐색 막대 > 사용자 지정 메뉴에서 명령을 붙이거나 뗌(우클릭 > 탐색 막대에서 제거) | S13 | 없음 | 없음 |
| 위치 | 사용자 지정 > 도킹 위치. ViewCube와 연결하면 함께 움직임. 창 가장자리를 따라 이동 가능 | S13 | 없음 | 없음 |

## 6. 단축키 (S4의 표, 원문 기준)

S4 페이지는 "일부 표"라고 밝힌다. 전체 목록은 Inventor 키보드 단축키 PDF 또는 도구 > 옵션 > 사용자 지정 > 키보드 탭에서 확인해야 한다.
GraphCTRL 열은 현재 구현(README, 소스의 키 처리)과 비교한 것이다.

| 키 | Inventor 2026 | 범위 | GraphCTRL | 상태 |
|---|---|---|---|---|
| F1 | 도움말 | 전체 | 없음 | 없음 |
| F2 | 이동(팬). 브라우저에서는 이름 바꾸기 | 전체 | F2 할당 없음(이동은 가운데 버튼 드래그) | 다름 |
| F3 | 확대/축소 | 전체 | 휠 줌 | 다름(휠) |
| F4 | 회전 | 전체 | F4+왼쪽 드래그 = 회전 | 있음 |
| F5 | 이전 뷰 | 전체 | 없음 | 없음 |
| F6 | 등각 뷰 | 전체 | F6 = 홈 | 있음(이름 다름) |
| F7 | 그래픽 절단 | 스케치 | 없음 | 없음 |
| F8 / F9 | 구속 표시 / 숨김 | 스케치 | 없음 | 없음 |
| Esc | 명령 끝내기 | 전체 | 도구 취소, 선택 해제 | 있음 |
| Delete | 선택 삭제 | 전체 | 삭제 | 있음 |
| Alt + 드래그 | 어셈블리에서 맞대기 구속 적용 | 어셈블리 | 없음 | 없음 |
| Ctrl + A | 다른 선택 메뉴 (모델 뷰) | 전체 | 없음 | 없음 |
| Ctrl + D | 치수 HUD 켜기/끄기 | 3D 스케치 | 치수 도구는 D | 다름 |
| Ctrl + R / Ctrl + I / Ctrl + Shift + P | 직교, 구속 추론, 개체 스냅 | 3D 스케치 | 축 잠금(Tab으로 평면 전환) | 다름 |
| Ctrl + Z / Ctrl + Y | 실행 취소 / 다시 실행 | 전체 | 같음 | 있음 |
| Shift + 우클릭 | 선택 명령 메뉴 | 전체 | 없음 | 없음 |
| Shift + 회전 | 모델 자동 회전 | 전체 | 없음 | 없음 |
| Shift + 스페이스 | 스케치 평면 전환(선·스플라인) | 3D 스케치 | Tab(XY/YZ/ZX) | 다름(키 다름) |
| C | 구속 명령 | 어셈블리 | 구속 도구는 리본 | 없음 |
| P | 부품 배치 (Place Component) | 어셈블리 | 로봇·바디 추가는 리본 | 없음 |
| M / RO | 이동 / 회전 구성요소 | 어셈블리 | 기즈모로 이동·회전 | 다름 |
| N | 구성요소 만들기 (현 위치에서) | 어셈블리 | 없음 | 없음 |
| Q | iMate 만들기 | 부품·어셈블리 | 없음 | 없음 |
| T | 구성요소 조정 | 프레젠테이션 | 없음 | 없음 |
| L / C / A / R / D | (스케치) 선 / 원 / 호 / 사각형 / 일반 치수 | 스케치 | 같은 키 (L, C, A, R, D) | 있음 |
| S / S3 | 2D 스케치 / 3D 스케치 시작 | 전체 | 리본 버튼 | 다름 |
| Alt + V | 솔리드·작업 요소 표시 토글 | 부품·어셈블리 | 바디 표시 체크박스 | 다름 |

## 7. 남은 일 (다음 단계)

- **완료**: 조립·검사·도구 탭의 확인된 패널(§2), 마킹 메뉴, 탐색 막대, ViewCube 26개 부분, 브라우저 패널 구성, 상태바 위치와 역할.
- **미수집(이번 라운드에서도 못 찾음)**:
  1. 조립 탭 구성 패널 명령 전체, 3D 모델 탭 전체, 스케치 탭 2D 명령 전체, 뷰 탭 외형 패널(검색 결과에 없음), 관리 탭, 환경 탭의 나머지 패널.
  2. 상태바 항목 목록(검색 요약의 "스케치 완전 구속 표시" 등은 원문 미확인).
  3. 브라우저 노드 전체 구조(조립 브라우저 전용 페이지 GUID-5F975252 계열은 아직 열지 않음).
- 방법: 2026 페이지의 "Access" 줄과 "Parent page" 링크를 따라가는 방식이 가장 확실하다. 검색은 `allowed_domains: ["help.autodesk.com"]`를 쓰고, 버전이 2024 이하인 결과는 참고만 한다.
- §6 단축키는 S4의 "일부 표"다. 전체 목록은 PDF와 대조한다.

## 8. 요약: 가장 큰 차이

- 어셈블리 구속(맞대기·플러시·축·각도), 조인트, 고정이 없다. 로봇 셀에서는 관절 조그로 대신한다.
- 마킹 메뉴와 F2–F9 축 단축키 체계가 없다.
- 리본 탭은 셀 작업(로봇·경로)을 위해 따로 만들어졌다. Inventor의 조립·설계·검사·관리 탭은 없다.
- 브라우저는 셀 전용 노드 구조이고, 떼어내기와 조인트 메뉴가 없다.
