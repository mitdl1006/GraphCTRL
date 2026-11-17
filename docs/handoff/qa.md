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
- 수용 기준: `docs/qa/m2-scenarios.md`(T1 이름 바꾸기·T2 억제·T3 순서·T4 스케치 편집·T5 삭제 종속 안내·T6 선택). 모델 트리 기능은 아직 병합된 브랜치에 없어 테스트는 기능이 올라온 뒤 추가.
- 로봇 라이브러리 경계 테스트 추가: `gc_session/tests/qa_library.rs`(8개, 1개는 위 버그 3로 `#[ignore]`).
- 리드 확인 요청: 프로젝트에 저장된 로봇 `urdf` 사본과 라이브러리 패키지가 다를 때 우선순위 규칙.
