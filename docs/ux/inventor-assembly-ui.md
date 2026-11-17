# Autodesk Inventor 2026 어셈블리 환경 UI 명세와 GraphCTRL 비교

목적: GraphCTRL의 작업 환경을 Inventor와 최대한 비슷하게 만들기 위한 기준 문서. 각 항목에 Inventor 2026 도움말 원문 출처와
GraphCTRL의 현재 상태(있음 / 없음 / 다름)를 적었다. 아이콘과 그림은 넣지 않았다.

## 0. 수집 범위 (먼저 읽을 것)

- **원문으로 읽은 곳**(curl로 `help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/…`를 받아 본문을 확인): 아래 출처 표의 S1–S8.
- **검색 결과만 본 곳**: 리본 탭별 패널과 명령 전체 목록, ViewCube·내비게이션 막대·상태바 항목, 브라우저 노드 구조 전체는
  도움말 목차가 자바스크립트 앱이라 목차 데이터를 얻지 못해 **미수집**이다. 아래 표에서 "미수집"으로 표시했다.
- 따라서 이 문서는 **완성본이 아니다.** 특히 §2(리본 탭별 명령)와 §5(ViewCube·상태바)는 골격만 있다. 다음 단계로 목차 페이지의
  GUID 목록을 얻어 탭마다 같은 방식으로 채워야 한다(§7).

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

## 2. 리본 탭 (Ribbon)

Inventor 2026의 탭 목록(조립·설계·3D 모델·스케치·검사·도구·관리·뷰·환경)은 검색 결과로만 확인했다. 탭별 패널과 명령은 **미수집**.
S1에 따르면 특수 환경(Studio, BIM 콘텐츠 등)은 **환경(Environments) 탭**에 있고, 들어가면 초록색 문맥 탭이 생기며, 오른쪽 끝의
종료/마침(Exit/Finish) 명령으로 나온다. 그 밖의 탭별 명령은 목차 확보 후 채운다.

| 탭 | Inventor 2026 | 출처 | GraphCTRL | 상태 |
|---|---|---|---|---|
| 조립 (Assemble) | 구성요소 배치, 구속, 조인트, 패턴 등. 패널명은 미확인 | S3, S5, S6 (명령 일부만) | 탭 없음. 바디 추가·로봇 추가는 "로봇" 탭 | 다름 |
| 설계 (Design) | 미수집 | — | 없음 | 없음 |
| 3D 모델 | 미수집(3D 스케치 정도) | — | "3D 모델" 탭: 3D 스케치, 3D 구속 | 다름 |
| 스케치 | 미수집 | — | "스케치" 탭: 선·원·호·사각형·구속·치수·스케치 마무리 | 있음(부분) |
| 검사 (Inspect) | 미수집 | — | 없음 | 없음 |
| 도구 (Tools) | 옵션, 단축키 설정 등. 단축키 설정은 S4 | S4 | 메뉴 "도구" (포즈 변환 계산기, 성능 오버레이, AI 에이전트 연결 등). 리본 탭 아님 | 다름 |
| 관리 (Manage) | 미수집 | — | 없음 | 없음 |
| 뷰 (View) | 표준 뷰, 표시 설정 | S8 (옛 도구막대 설명) | "뷰" 탭: 홈, 정면, 평면, 우측, 전체 보기, 직교/원근 | 있음(부분) |
| 환경 (Environments) | 특수 환경 진입, 종료/마침 | S1 | 없음 | 없음 |
| 로봇 | (Inventor에 없음) | — | "로봇" 탭: 추가, 좌표계, 경로, 계획, 재생 | 추가 기능 |

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
| 어셈블리 항목 | 구속(Constrain), 부품 배치(Place Component), 이동, 회전 | S2 | 이동·회전은 기즈모로만 가능, 메뉴 항목 없음 | 다름 |
| 스케치 항목 | 원(중심점), 선, 두 점 사각형, 스케치 마무리 | S2 | 스케치 도구는 리본에만 있음 | 다름 |
| 끄기 | 도구 > 옵션 설정 (구체 경로는 미확인) | S2 | 없음 | — |

## 5. 브라우저, 상태바, 뷰큐브, 탐색 막대

### 5.1 브라우저 (Browser)
| 항목 | Inventor 2026 | 출처 | GraphCTRL | 상태 |
|---|---|---|---|---|
| 구조 | 파일의 계층(부품, 조립품, 프레젠테이션, 도면). 환경마다 다름. 각 노드는 단계 순서로 나열 | S8 | 원점, 스케치, 파라미터, 바디, 로봇, 좌표계, 웨이포인트, 경로 | 다름(셀 전용 노드) |
| 떼어내기 | 브라우저를 창 밖으로 빼서 화면 가장자리에 붙이기, 항상 위 옵션 | S8 | 떼어내기 없음 | 없음 |
| 컴포넌트 우클릭 메뉴 | 위치 찾기, 도움말, 상단 구속 선택, 모든 구속 선택, 구성요소 애니메이션, 열기, 구성요소 하위 메뉴, 표시 여부 | S7 | 숨기기/보이기, 삭제, 계획(경로), 스케치 편집 | 다름(항목 수 적음) |
| 조인트 우클릭 메뉴 | 잠금(Lock), 고정(Ground), 보호(Protect) | S6 | 조인트 개념 없음 | 없음 |
| 구속 종류 | 맞대기(Mate), 플러시(Flush), 축 2개 (반대/정렬/무방향), 각도(방향/무방향) 등. 한계·정지 위치 설정 가능 | S5 | 2D·3D 스케치 구속 12종 + 3D 구속. 조립 구속 없음 | 없음(부품 간) |
| 조인트 | 구성요소를 위치시키고 운동을 정의하는 권장 방식. 구속은 옛 방식 | S6 | 로봇 관절은 조그 슬라이더 | 다름 |
| 고정(Ground) | 구성요소를 공간에 고정. 원점에 고정 가능 | S6, S3 | 없음 | 없음 |

### 5.2 상태바, ViewCube, 탐색 막대
| 항목 | Inventor 2026 | 출처 | GraphCTRL | 상태 |
|---|---|---|---|---|
| 상태바 | 미수집 (검색 요약: 스케치 완전 구속 여부, 다음 단계 안내 등. 원문 미확인) | — | 안내 문구, "N개 치수 필요"/"완전 구속", GPU 사용, AI 에이전트 알림 | 부분 있음 |
| ViewCube | 미수집 | — | 있음(면 라벨 평면·정면·우측, 클릭 가능) | 있음 |
| 탐색 막대 | 미수집 | — | 없음 | 없음 |

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

1. 도움말 목차(탭별 GUID 목록)를 얻는다. 확인된 방법: `help.autodesk.com/view/INVNTOR/2026/ENU/`는 자바스크립트 앱이고
   `config/config.json`과 `common.json`만 공개 JSON으로 확인됨. 목차 데이터 위치는 아직 찾지 못했다. 찾지 못하면
   리본 탭마다 "Ribbon tab reference" 계열 검색으로 GUID를 모은다.
2. §2의 탭별 패널·명령, §5.2의 ViewCube·탐색 막대·상태바, 브라우저 노드 전체를 같은 형식으로 채운다.
3. 마킹 메뉴의 실제 항목 배치(방사형 8방향 등)는 S2 원문에 없다. 이미지 없이 쓸 수 있는 범위만 적었다.
4. §6은 "일부 표"다. 전체 목록은 PDF를 받아 대조한다.

## 8. 요약: 가장 큰 차이

- 어셈블리 구속(맞대기·플러시·축·각도), 조인트, 고정이 없다. 로봇 셀에서는 관절 조그로 대신한다.
- 마킹 메뉴와 F2–F9 축 단축키 체계가 없다.
- 리본 탭은 셀 작업(로봇·경로)을 위해 따로 만들어졌다. Inventor의 조립·설계·검사·관리 탭은 없다.
- 브라우저는 셀 전용 노드 구조이고, 떼어내기와 조인트 메뉴가 없다.
