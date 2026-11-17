# GraphCTRL

설비·라인에 장착되는 로봇(첫 대상: Universal Robots e-Series)의 동작을 3D 작업공간에서 스케치하고, 치수·구속조건을
넣고, 경로를 자동으로 계획해 보는 데스크톱 프로그램입니다. 전부 Rust로 작성하며 화면은 egui + wgpu를 씁니다.

아직 개발 초기입니다. 지금 되는 것:

- **스케치**: 원점 평면에 2D 스케치(선·원·호·사각형), 구속조건 12종, 치수(`25 mm`, `폭 * 2` 같은 수식, 더블클릭으로 수정),
  브라우저의 파라미터 표(이름 = 값, 치수 수식에서 사용), 드래그,
  과구속 시 참조 치수 전환, 실행 취소/다시 실행, 저장/열기(`.gctrl`).
- **로봇 셀**: 리본의 "로봇" 탭에서 UR5e(또는 샘플 6축 로봇) 추가 → 오른쪽 패널의 슬라이더로 조그 → "현재 자세"로
  웨이포인트 추가(스케치에서 점 하나를 선택하고 "스케치 점"을 누르면 그 점을 따라다니는 웨이포인트) → 상자·원기둥·구
  장애물 추가와 배치, "메시 불러오기"로 설비 STL 추가(mm 단위 파일은 자동으로 m로 환산) → "경로 만들기" → 플래너 선택(RRT-Connect, RRT, RRT*, Informed RRT*, PRM) → "계획" → "재생".
  장애물이나 스케치 치수를 바꾸면 경로가 "오래됨"으로 표시됩니다.
- **뷰 조작(Inventor 방식)**: 휠 = 줌, 가운데 드래그 = 팬, Shift + 가운데 드래그 또는 F4 + 왼쪽 드래그 = 오빗, F6 = 홈.

계획은 [docs/PLAN.md](docs/PLAN.md), 진행 상태는 [docs/STATUS.md](docs/STATUS.md), 화면 캡처는
[docs/evidence/stage-4.md](docs/evidence/stage-4.md)에 있습니다.

## 빌드와 실행

필요한 것: [rustup](https://rustup.rs). Rust 버전은 `rust-toolchain.toml`에 고정되어 있어 처음 실행할 때 자동으로
설치됩니다. Windows는 Visual Studio Build Tools(“C++를 사용한 데스크톱 개발”)가 필요합니다.

```bash
cargo run -p graphctrl
```

첫 빌드는 의존성 약 470개를 컴파일하므로 몇 분 걸립니다. 두 번째부터는 바뀐 부분만 다시 컴파일합니다.

### 빌드 중 메모리 부족 오류가 날 때

`rustc-LLVM ERROR: out of memory`나 `memory allocation of ... bytes failed`가 나오면 코드 문제가 아니라 컴파일러가
쓸 메모리가 모자란 것입니다. cargo는 논리 프로세서 수만큼 크레이트를 동시에 컴파일하는데, 동시 개수를 줄이면 됩니다.
이미 끝난 크레이트는 다시 컴파일하지 않습니다.

```bash
cargo run -p graphctrl -j 2
```

매번 붙이기 번거로우면 환경 변수로 정해 둘 수 있습니다(PowerShell).

```powershell
$env:CARGO_BUILD_JOBS = "2"
```

숫자가 작을수록 메모리를 덜 쓰고 빌드는 느려집니다. 최적화 수준을 낮추는 것으로는 해결되지 않습니다(크레이트당 컴파일러
메모리가 거의 같음, 측정값은 [docs/evidence/stage-0.md](docs/evidence/stage-0.md)).

## 개발

커밋 전에 다음 한 줄이 통과해야 합니다. 포맷, `cargo check`, clippy(Linux와 Windows 대상), 테스트, GPU 테스트,
레이어 검사, 라이선스 검사를 순서대로 돌립니다.

```bash
cargo xtask gate
```

| 명령 | 하는 일 |
|---|---|
| `cargo xtask check-layers` | 크레이트 의존 방향이 `layers.toml`의 레이어 순서를 지키는지 검사 |
| `cargo xtask check-licences` | 모든 의존성의 라이선스와 출처가 `licences.toml` 정책에 맞는지 검사 |
| `cargo xtask test-gpu` | GPU가 반드시 있어야 하는 조건으로 테스트 실행(소프트웨어 렌더러가 있으면 그것을 사용) |

작업 규칙은 [CLAUDE.md](CLAUDE.md)에 있습니다.

## 라이선스

MIT. 함께 들어 있는 서드파티 자산(Jetendard 폰트, Universal Robots 로봇 기술 파일)의 출처와 라이선스는
[assets/SOURCES.toml](assets/SOURCES.toml)에 적혀 있습니다.
