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
