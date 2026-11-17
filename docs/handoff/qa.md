# QA 인계

## 발견한 버그

1. **UR 해석 IK가 완전히 뻗은 팔(q3 = 0)에서 `OutOfReach`** — 담당: 백엔드(`gc_plugin_ur/src/ik.rs`).
   - 재현: `cargo test -p gc_plugin_ur --test qa_ik_edges -- --ignored the_fully_stretched_arm_is_reachable`
   - 실제 FK로 얻은 자세인데 반올림 때문에 팔꿈치 코사인이 1을 살짝 넘어 도달 불가로 판정. 1 근처로 클램프(허용오차 ~1e-9)하면 될 것.
2. **(확인 필요) 팔 전체를 감싸는 거대한 툴이 충돌로 보고되지 않음** — 담당: 백엔드(`gc_session`/`gc_collide`).
   - 재현: `cargo test -p gc_session --test qa_cell_edges -- --ignored a_huge_tool`
   - 툴이 환경과는 검사되지만 팔 자기 링크와는 검사되지 않는 듯. 설계라면 테스트 삭제.
3. **`robot.json`의 `description`이 패키지 폴더 밖(`../../x.urdf`)을 가리켜도 패키지로 인식** — 담당: 백엔드(`gc_session/src/library.rs::scan`). 낮은 심각도.
   - 재현: `cargo test -p gc_session --test qa_library -- --ignored a_description_path`
   - `description`을 절대 경로·`..` 포함 시 거부하면 된다.
4. **5.11 미구현**(테스트 불가): 패닉 훅·복구 저장, 로그(stderr·파일·패널), 창 아이콘, `logs/` 폴더.

## 결과

- 기준: `cargo xtask gate`(fmt·check·clippy·Windows 교차 clippy·test·GPU·레이어·라이선스) 전체 통과, 추가 테스트 포함. 추가 테스트 목록은 `docs/qa/coverage.md`.
- CI는 Windows 빌드·테스트를 이미 포함(수정 불필요).

## M2-1 (리드 요청)

- 리드 브랜치를 이 QA 브랜치에 merge함(`f76d087`).
- 최우선 수용 기준: `docs/qa/m2-scenarios.md`의 "스케치 동등성" 절(`docs/ux/inventor-sketch-parity.md` 4절 P1-1~14, 명령별 클릭 순서·구속·Undo 단위·Ctrl 억제).
- 수용 기준: `docs/qa/m2-scenarios.md`(아래 "인벤터 동등성" 절에 `docs/ux/inventor-parity.md` 3절 U1–U25 병합, 상태 열로 추적; T1 이름 바꾸기·T2 억제·T3 순서·T4 스케치 편집·T5 삭제 종속 안내·T6 선택). 모델 트리 기능은 아직 병합된 브랜치에 없어 테스트는 기능이 올라온 뒤 추가.
- 로봇 라이브러리 경계 테스트 추가: `gc_session/tests/qa_library.rs`(8개, 1개는 위 버그 3로 `#[ignore]`).
- 리드 답: 프로젝트의 `Robot.urdf` 사본이 기구학 기준, 메시만 라이브러리에서 찾는다. `qa_library.rs::the_projects_urdf_copy_decides_the_kinematics_not_the_library`로 고정.
- 대기: 백엔드·프론트엔드 브랜치에 버그 1–4 수정과 M2-1 기능이 올라오면 `#[ignore]`를 풀고 `m2-scenarios.md` 기준 테스트 추가(리드 브랜치에는 아직 없음, 2026-11-17 확인).

## 2026-11-17 백엔드 브랜치 merge 후

- 백엔드 브랜치(`7826efd`)와 리드 브랜치를 QA 브랜치에 merge. 버그 1–3의 `#[ignore]`는 백엔드가 풀었고(수정 커밋 `241e015`) 해당 재현 테스트는 모두 통과. `--include-ignored` 전체 통과, `cargo xtask gate` 전체 통과.
- 남은 버그: 5.11 배관은 프론트엔드 브랜치(`fe3e2af` 로그·패닉 훅·창 아이콘)에서 처리 중 — 프론트엔드 브랜치는 아직 merge하지 않음(리드가 병합하면 확인).
- 다음: 백엔드의 트리 API·작업 평면 스케치(U13)·GLB 불러오기(U14) 테스트를 `m2-scenarios.md` 기준으로 추가.

- 상시 규칙(리드): 인벤터 동작 테스트의 기대값은 help.autodesk.com 원문(curl)을 근거로 하고, 테스트 주석에 참고 링크를 남긴다. 아직 인벤터 동작 테스트는 기능 대기 중이라 추가한 것 없음.

## 2026-11-17 리드 병합(93a1a73) 이후

- 리드 브랜치 merge. 남은 `#[ignore]`는 `gc_solver/tests/perf.rs`(성능 보고, 버그 아님)뿐 — 버그 1–3은 백엔드가 고쳤고 재현 테스트는 상시 통과 중.
- **5.11 배관(프론트엔드) 확인**: 로그(stderr·파일·링 버퍼 500줄)·패닉 훅·창 아이콘 코드 있음. `gc_app/tests/qa_diagnostics.rs` 추가: 레벨 필터(info 이상), 링 500줄 상한·순서, 여러 스레드 동시 기록, 설정 폴더 자리에 파일이 있을 때도 시작은 계속됨, 로그 파일 기록. **패닉 훅은 네이티브 메시지 상자를 띄워 자동 테스트 불가** → 사용자 PC 수동 확인 항목(패닉 → 메시지 → `crash-<시각>.gctrl` 보존 → 다음 실행 복구 제안).
- **gc_agent 엣지 케이스**(`gc_agent/tests/qa_edges.rs`, 10개 통과): 모르는 이름·NaN·무한·빈 이름·빈 경로는 오류 메시지 + 문서 불변(리비전·히스토리 불변), 잘못된 인자 JSON(타입·길이·누락·null·없는 도구)은 앱에 닿기 전에 거부(호출 수 0 단언), 잘못된 JSON-RPC 봉투, 연속 25회·Undo/Redo 50회·읽기 호출 반복, 토큰 형식 변형(대소문자·접두 공백·쿼리·다른 헤더)과 GET/DELETE·다른 경로 거부, **재시작+토큰 변경 후 옛 토큰·옛 MCP 세션 거부**, 포트 충돌 시 주소가 든 오류, 앱 쪽이 오류를 답하거나 닫힌 경우의 오류 전달.
- **새 버그 (낮은 심각도)**: `place_robot`이 `1e300` mm 같은 터무니없는 유한 위치를 받아들임(범위 검사 없음) — 담당: 백엔드(`gc_agent/src/exec.rs`·`gc_doc` 배치 검증). 재현: `cargo test -p gc_agent --test qa_edges -- --ignored an_absurd_placement_is_refused`. 뒤따르는 호출이 패닉하지는 않음(별도 통과 테스트).
- 확인했으나 버그 아님: 헤더 끝 공백(`Bearer secret `)은 HTTP 계층이 잘라 내므로 통과 — 토큰 일치에는 영향 없음.
