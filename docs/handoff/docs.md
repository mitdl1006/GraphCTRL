# 문서·UX 인계 (2026-11-17)

## Inventor 2026 어셈블리 UI 명세 (`docs/ux/inventor-assembly-ui.md`)

**출처 원문(curl로 읽음, cloudhelp/2026/ENU/Inventor-Help/files/)**: S1 환경(GUID-2D47FF53), S2 마킹 메뉴(GUID-4B081B74), S3 조립 기초(GUID-5A8DD595), S4 단축키 표(GUID-DEEB0723), S5 구속 Assembly 탭(GUID-EBD2C784, 앞부분), S6 구속·조인트(GUID-2DFEAF48), S7 Studio 브라우저·컨텍스트 메뉴(GUID-754DBBC0), S8 뷰 인터페이스(GUID-E13A932A), S11 사용자 인터페이스 상위(GUID-8E3158B6), S12 리본(GUID-78BFC5E2), S13 탐색 막대(GUID-FAAB453E), S14 ViewCube 재정위(GUID-4A18A2AC), S15 뷰 재정위(GUID-94F44C80), S16 브라우저(GUID-EDF6B951), S17 그래픽 창(GUID-7544AABF), S18 구속 접근 경로(GUID-198F05E4), S19 조인트 생성(GUID-6AA68E8F), S20 맞대기·플러시(GUID-0F500089), S21 마킹 메뉴 탭(GUID-1A2766FB), 그 밖의 접근 경로용 페이지(GUID-23757F14, GUID-6DB7770F, GUID-5F5AB893).

**원문과 요약의 차이·주의점**
- 리본 탭 목록(조립·설계·3D 모델·스케치·검사·도구·관리·뷰·환경)은 검색 결과와 2026 "About the Ribbon"의 "탭은 작업별" 설명에 근거한다. 탭별 전체 패널 목록은 원문으로 확인하지 못했다(목차 미확보).
- 단축키 표(S4)는 페이지 스스로 "일부"라고 밝힌다. 전체 목록은 PDF로 대조해야 한다.
- S7은 Studio 페이지지만 그 안에 조립 컴포넌트 우클릭 항목이 있어 근거로 썼다. 조립 브라우저 전용 페이지(GUID-5F975252 계열)는 아직 열지 않았다.
- ViewCube의 모서리·꼭짓점 개수와 동작은 S14의 "26개 부분" 설명을 따랐고, 검색 요약에 있던 "Go Home" 메뉴는 원문에서 확인하지 못해 적지 않았다.
- 상태바 항목 목록은 원문에 없었다. 위치와 역할만 S17로 확인했다.
- 아이콘과 그림은 쓰지 않았다.

## 2026-11-17 오후 추가 (측정·간섭·구동, 탭 패널 확인)

- 원문 재확인(curl): GUID-1805AB8F(측정, Tools > Measure 패널), GUID-80DED0A6(간섭 분석, Inspect > Interference), GUID-B78E59C9(구동, 관계 우클릭), GUID-FA1A8282(접촉 솔버, Inspect > Interference), GUID-2C55B507·GUID-E7A45820(Assemble > Component/Productivity 패널), GUID-DAAB6834(간섭 확인 2026판).
- `docs/ux/inventor-assembly-parity.md` §3.6.1에 측정·간섭·구동의 원문 차이 정리. 최대 거리 전환, 값 하나 복사, UCS, 간섭 필터, 구동 적응성, 기록은 원문에 있고 구현에 없음. 간섭 결과를 솔리드로 보여 준다는 점은 원문 문장을 확인하지 못해 "확인 못 함"으로 남김.
- 구동 후 자세 유지 여부는 원문 페이지에서 확인하지 못했으므로 현행(닫으면 복귀)을 유지하고 미확인으로 표시.
- 탭 패널 목록은 검색으로 일부만 확인됨(검색 결과에 2024 이하 판이 섞임). 미확인 항목은 `docs/ux/inventor-assembly-ui.md` §7.

## 2026-11-18 밤 — 인벤터 어셈블리 동등성 "현재" 열 갱신, 작업 모델 장

- `docs/ux/inventor-assembly-parity.md`의 "GraphCTRL 현재" 열을 handoff(backend·frontend)와 i18n 문자열, 대화상자 모듈 기준으로 고쳤다. 원문 대조는 이전 항목의 GUID를 그대로 쓴다. 확인하지 못한 것은 "확인 중"으로 표시(단면 참여, 조인트 편집 대화상자, 컴포넌트 단면 제외, 교차 부품 투영).
- 가이드 39장(작업 모델)과 40장(스케줄과 URScript)을 추가했다. URScript는 "URSim·실제 로봇으로 검증하지 않은 프로그램"이라는 안내를 넣었다(i18n `UrScriptNotice`).

## 2026-11-18 밤 2 — 구속 값·조인트 원점·자유 이동·표현·구속 오류 보강 (가이드·parity)

- 가이드: 16장(모션 구속, 구속 값 = 모델 매개변수), 17장(조인트 원점 정렬·뒤집기·오프셋, Planar·Gap·Ball 한계 보류), 18장(Accept·구속 복구·충돌 분석), 19장(패턴 요소 억제·독립·복원), 20장(생산성 그룹의 Ground and Root·Place at Component Origin, 화면 연결됨), 21장(고무줄·업데이트·마킹 메뉴 닫기), 26장(뷰 표현 잠금·복사, 위치 표현 복사, 오버라이드 보류), 3장(ViewCube 우클릭), 34장(예정 목록 갱신).
- parity 문서 "현재" 열: 위 항목을 handoff의 GUID 근거와 i18n 문자열 기준으로 갱신. 보류 항목(Transitional, Planar 조인트, Gap, Ball 한계, 위치 표현 오버라이드, 마킹 메뉴 명령 배치)은 "보류"로 표시.
