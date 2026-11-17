# GraphCTRL 작업 규칙

계획은 `docs/PLAN.md`, 진행 상태는 `docs/STATUS.md`, 근거 문서는 `docs/research/`에 있다.

## Rust 코드를 작성한 뒤에는 반드시 (커밋 전)

```bash
cargo fmt --all
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

- 순서는 fmt → check → clippy → test. 하나라도 실패하면 고친 뒤 처음부터 다시 돌린다.
- GPU가 필요한 테스트는 어댑터가 없으면 건너뛴다. 이 샌드박스에서 실제로 돌리려면:

```bash
VK_ICD_FILENAMES=$(ls /opt/pw-browsers/chromium-*/chrome-linux*/vk_swiftshader_icd.json | head -1) WGPU_BACKEND=vulkan GC_REQUIRE_GPU=1 cargo test --workspace --locked -- --test-threads=1
```

- 이 환경 변수를 `.cargo/config.toml`에 넣지 않는다(사용자 PC의 GPU 드라이버를 가린다).

## 그 밖의 규칙

- 의존성은 crates.io만 쓴다(git 의존성 금지). GPL/LGPL/AGPL/MPL/EUPL 크레이트 금지.
- 레이어 순서 `foundation < api < domain < render < ui < app`를 지킨다(`layers.toml`).
- 비테스트 코드에서 `unwrap`/`expect`/`panic!`/`todo!` 금지(clippy가 거부).
- 선택 기준의 우선순위: 큰 장면 3D 성능 → 유지보수 지속성 → 주류 크레이트.
- 커밋 전 전체 검사는 `cargo xtask gate` 한 번으로 돌릴 수 있다(위 네 단계 + Windows 교차 clippy + GPU 테스트 + 레이어·라이선스 검사).
- 인벤터와 같은 경험이 목표다. 인벤터 동작을 구현·변경·테스트할 때는 먼저 해당 기능의 Autodesk 공식 도움말 원문
  (`https://help.autodesk.com/view/INVNTOR/2026/ENU/`, `cloudhelp/2026/ENU/Inventor-Help/files/GUID-*.htm`)을 읽고 근거로 삼는다.
  이 환경에서는 `curl`로 열린다(WebFetch는 안 열림). 커밋 메시지나 `docs/handoff/<역할>.md`에 참고한 링크를 남긴다.
  `docs/ux/inventor-*.md` 요약과 원문이 다르면 원문을 따르고 차이를 handoff에 적는다. Autodesk 아이콘·그림·상표는 쓰지 않는다.
