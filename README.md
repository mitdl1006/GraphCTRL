# GraphCTRL

설비·라인에 장착되는 로봇(첫 대상: Universal Robots e-Series)을 3D 작업공간에서 시뮬레이션하고 프로그래밍하는 데스크톱
프로그램입니다. Rust로 작성하며 화면은 egui + wgpu를 씁니다. 작업 환경은 Autodesk Inventor의 어셈블리 환경을 최대한
닮게 만드는 것이 목표이며, 명령과 단축키는 Inventor 2026 도움말을 근거로 합니다.

아직 개발 초기입니다. 현재 상태는 [진행 상태](docs/STATUS.md), 계획은 [계획서](docs/PLAN.md), 알려진 문제는
[알려진 문제](docs/KNOWN_ISSUES.md)에 있습니다.

## 사용자 가이드

처음 쓴다면 **[사용자 가이드](docs/guide/README.md)**부터 보세요. 설치와 첫 실행(파일 > 예제 열기), 화면 구성, 스케치와
치수, 로봇 추가, 웨이포인트, 경로 계획과 재생, 측정·간섭 분석·구동, 구속, 조인트, 패턴, 파일 가져오기, AI 에이전트
연결까지 순서대로 다룹니다. 아직 화면이 없는 기능은 각 장에서 **예정**으로 표시합니다.

## 주요 기능

| 영역 | 할 수 있는 것 | 화면 |
|---|---|---|
| 스케치 | 2D 선·원·호·사각형, 구속 12종, 치수(수식과 파라미터), 3D 스케치 | [3D 스케치](docs/evidence/img/review-10-3d-sketch.png) |
| 로봇 셀 | UR3e·UR5e·UR10e·UR16e와 URDF 로봇, 조그, 툴, 충돌 검사 | [예제 셀](docs/evidence/img/review-19-example.png) |
| 로봇 추가 | 라이브러리의 `robots/` 패키지 목록 | [로봇 추가 목록](docs/evidence/img/review-21-library.png) |
| 웨이포인트 | 기준 좌표계(월드·로봇 베이스·작업 좌표계), 기즈모, 고스트 로봇 | [고스트 로봇](docs/evidence/img/review-17-ghost.png) |
| 경로 | RRT-Connect, RRT, RRT*, Informed RRT*, PRM, 직선 이동(LIN), TCP 속도 제한, 재생 | [경로 계획](docs/evidence/img/review-20-path-selected.png) |
| 검사 | 측정(최소·중심 간·최대 거리), 간섭 분석, 구동(관절 시연, 충돌 시 정지) | [측정](docs/evidence/img/review-26-measure.png), [구동](docs/evidence/img/review-29-drive.png) |
| 어셈블리 | 구속(맞춤·플러시·삽입), 조인트, 패턴, 파일 가져오기, 매개변수 | [패턴](docs/evidence/img/review-40-assemble-pattern.png) |
| 도구 | 포즈 변환 계산기, 저장·자동 저장 복구, AI 에이전트 연결(MCP) | [포즈 계산기](docs/evidence/img/review-24-pose-calculator.png) |

조작은 Inventor와 같습니다: 휠 = 확대/축소, 가운데 버튼 드래그 = 이동, Shift + 가운데 버튼 드래그 또는 F4 + 왼쪽 드래그 =
회전, F6 = 홈 뷰. 이동·회전 기즈모와 우클릭 마킹 메뉴도 Inventor 방식입니다.

로봇 모델은 실행 파일 밖의 `robots` 폴더에 패키지로 들어 있습니다. 배포할 때는 실행 파일 옆에 `robots` 폴더를 함께 두세요.

## 빌드와 실행

필요한 것: [rustup](https://rustup.rs). Rust 버전은 `rust-toolchain.toml`에 고정되어 있어 처음 실행할 때 자동으로
설치됩니다. Windows는 Visual Studio Build Tools(“C++를 사용한 데스크톱 개발”)가 필요합니다.

Windows에서는 다음 명령을 씁니다(릴리스 빌드, 동시 컴파일 4개).

```bash
cargo run -p graphctrl --release -j 4
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
