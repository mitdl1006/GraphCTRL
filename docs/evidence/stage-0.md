# Stage 0 증거 — 골격과 환경 증명

## 실행한 명령과 결과 (2026-11-16, 샌드박스: Rust 1.97.0, 4코어, GPU 없음)

| 항목 | 명령 | 결과 |
|---|---|---|
| 첫 전체 빌드 | `cargo build --workspace --locked` | 의존성 약 470개, 콜드 빌드 4분 48초, `target` 1.3 GB |
| 포맷 | `cargo fmt --all` | 통과 |
| 검사 | `cargo check --workspace --all-targets --locked` | 통과 |
| 린트 | `cargo clippy --workspace --all-targets --locked -- -D warnings` | 경고 0 |
| Windows 교차 검사 | `cargo check --workspace --all-targets --locked --target x86_64-pc-windows-msvc` | 통과 (1분 5초) |
| 테스트(GPU 포함) | `VK_ICD_FILENAMES=<SwiftShader> WGPU_BACKEND=vulkan GC_REQUIRE_GPU=1 cargo test --workspace --locked -- --test-threads=1` | 3개 통과, 실패 0 |

## 환경 증명

1. **렌더 → 픽셀 읽기** (`gc_render/tests/gpu.rs::clear_and_readback_returns_the_clear_colour`): SwiftShader 위에서
   wgpu 30이 어댑터·디바이스를 만들고, 4x MSAA + Depth32F 타깃을 지워 `Rgba8Unorm`으로 리졸브한 뒤 읽어 온 픽셀이
   기대 색과 일치. **통과.**
2. **앱 화면 스냅샷** (`gc_app/tests/shell.rs::shell_renders_with_gpu`): egui_kittest가 앱 전체를 오프스크린으로 렌더.
   `register_native_texture`로 올린 3D 타깃이 뷰포트 영역에 보임. **통과.** 이미지: [img/stage0-shell.png](img/stage0-shell.png)
   - 확인한 것: 메뉴 "파일", 한글 입력 시험 패널의 한글이 Jetendard로 렌더됨, 입력 칸 2개, "GPU: 사용 중",
     오른쪽 뷰포트가 지운 색(밝은 회색)으로 채워짐, 상태바 "준비".
3. **GPU 없이 실행** (`shell_runs_without_gpu`): 렌더 상태가 없어도 앱이 동작하고 위젯을 찾을 수 있음. **통과.**
4. **실제 창 스모크 실행** (`xvfb-run -a graphctrl --smoke-frames 5 --screenshot out.png`): **통과.**
   winit 창이 Xvfb에서 열리고 5프레임 뒤 스스로 닫히며 3D 타깃을 PNG로 저장(지운 색으로 채워진 1080×853 이미지 확인).
   - 샌드박스에 추가로 필요했던 것: `libxkbcommon-x11-0`(없으면 winit가 패닉), `mesa-vulkan-drivers`(lavapipe).
     apt 소스를 https로 바꾸면 설치된다.
   - SwiftShader는 `VK_KHR_xlib_surface`가 없어 **창 표면**을 만들지 못한다. 그래서 역할을 나눈다:
     오프스크린·kittest 테스트는 SwiftShader 또는 lavapipe, 실제 창 스모크는 lavapipe(`VK_ICD_FILENAMES` 지정 없이 실행).

## 의존성 점검

- `cargo tree -d`: wgpu, naga, glam, egui, winit 모두 단일 버전.
- 라이선스: 금지 계열 없음. `egui_kittest`의 `snapshot` 기능이 `dify → colored`(MPL-2.0)를 끌어와서 그 기능을 끄고
  이미지 비교를 `gc_test_support::differing_pixels`로 구현.

## 계획 대비 차이

- Jetendard 폰트는 파일당 약 4.6 MB(예상 2 MB). Regular + Bold 합계 9.3 MB가 실행 파일에 포함된다.
- `#[ignore]` 처리한 테스트: 없음.

## Windows 빌드 메모리 부족 (사용자 PC, 2026-11-16)

- 증상: `cargo run -p graphctrl`이 `rustc-LLVM ERROR: out of memory`로 실패. `-j`로 동시 작업 수를 줄이면 빌드·실행됨(사용자 확인).
- 측정(샌드박스, Windows 대상 라이브러리 빌드, rustc 프로세스당 최대 메모리): 의존성 `opt-level` 2 / 1 / 0에서
  `windows` 1611 / 1584 / 1568 MB, `naga` 903 / 889 / 740 MB, `wgpu_core` 845 / 827 / 682 MB.
  동시 16개 빌드 전체의 최대 사용량은 수준 2에서 3.2 GB, 수준 1에서도 3.2 GB.
- 결론: 최적화 수준은 원인이 아니므로 프로필은 그대로 둔다. 대응은 동시 작업 수 제한(`-j N` 또는 `CARGO_BUILD_JOBS`)이며
  PC마다 다르므로 저장소 설정에 고정하지 않고 README에 안내한다.
