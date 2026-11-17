# AI 에이전트 연결 (MCP)

Claude Code나 Claude Desktop 같은 AI 에이전트가 GraphCTRL에서 열어 둔 로봇 셀을 읽고 편집하게 하는 방법입니다.
에이전트는 MCP(Model Context Protocol)의 도구를 호출하고, 각 호출은 화면에 바로 나타나며 Ctrl+Z로 되돌릴 수 있습니다.
설계 배경과 단계별 계획은 [계획서 9.8절](../PLAN.md)에 있습니다.

## 1. 켜기

1. **도구 > AI 에이전트 연결**을 엽니다.
2. **에이전트 연결 허용 (이 PC에서만)** 을 켭니다. 기본은 꺼져 있습니다.
3. 창에 **주소**(기본 `http://127.0.0.1:47615/mcp`), **포트**, **접근 토큰**이 나타납니다.

서버는 `127.0.0.1`(이 PC)에만 열립니다. 다른 PC에서는 연결되지 않습니다. 포트와 토큰은 설정 파일 `settings.json`에 저장되며,
설정 파일 위치는 **도움말 > 정보**에서 확인할 수 있습니다.

- **포트**를 바꾸면 서버가 그 포트로 다시 시작합니다. 등록 명령의 주소도 바뀌므로 다시 등록하세요.
- **새 토큰**은 기존 토큰을 무효로 만듭니다. 등록한 에이전트 설정도 새 토큰으로 바꿔야 합니다.
- 토큰은 비밀번호와 같습니다. 화면을 공유하거나 저장소에 올리지 마세요.

## 2. Claude Code에 등록

창의 **Claude Code에 등록하는 명령**을 **복사** 버튼으로 복사해 터미널에서 실행합니다. 형식은 다음과 같습니다(토큰은 실제 값).

```bash
claude mcp add --transport http graphctrl http://127.0.0.1:47615/mcp --header "Authorization: Bearer <토큰>"
```

등록한 뒤 Claude Code에서 `/mcp`로 `graphctrl`이 연결되었는지 확인합니다. GraphCTRL이 꺼져 있거나 토큰이 틀리면
연결되지 않습니다. 이 경우 창에서 허용 상태와 토큰을 다시 확인하세요.

## 3. Claude Desktop에 연결

Claude Desktop은 로컬 프로세스(stdio) 방식의 MCP 설정 파일을 읽습니다. 이 PC의 HTTP 서버에 붙이려면 중간에 `mcp-remote`
패키지를 쓰는 방법이 일반적입니다. 아래 설정은 `mcp-remote` 예시를 따른 것이며, Claude Desktop의 공식 문서로는 확인하지
못했습니다. 설정을 바꾸기 전에 사용 중인 버전의 안내를 먼저 확인하세요.

설정 파일 위치: macOS `~/Library/Application Support/Claude/claude_desktop_config.json`, Windows `%APPDATA%\Claude\claude_desktop_config.json`

```json
{
  "mcpServers": {
    "graphctrl": {
      "command": "npx",
      "args": ["-y", "mcp-remote", "http://127.0.0.1:47615/mcp", "--header", "Authorization:${AUTH_HEADER}"],
      "env": { "AUTH_HEADER": "Bearer <토큰>" }
    }
  }
}
```

- `Authorization:${AUTH_HEADER}`처럼 콜론 뒤에 공백을 두지 않고, 값은 `env`에 넣습니다. Windows에서 인자의 공백이 깨지는 문제를 피하기 위한 방법입니다.
- Node.js 18 이상이 필요합니다.
- 설정을 바꾼 뒤에는 Claude Desktop을 완전히 종료했다가 다시 실행하세요.
- 연결 로그는 Claude Desktop 로그 폴더의 `mcp*.log`에서 볼 수 있습니다.
- Claude Desktop의 "맞춤 커넥터"(원격 주소 입력)는 쓰지 마세요. 그 연결은 Anthropic 서버에서 이루어지므로 이 PC의 `127.0.0.1` 주소에 닿지 않습니다.

## 4. 도구 목록

모든 길이는 **mm**, 각도는 **deg**(도)입니다. 에이전트는 먼저 `overview`를 호출하는 것이 좋습니다.

| 도구 | 하는 일 | 편집 |
|---|---|---|
| `overview` | 열린 셀 전체: 로봇(베이스 자세, 관절값), 바디, 작업 좌표계, 웨이포인트(월드 자세), 경로와 계획 상태 | 아니오 |
| `library_robots` | `add_robot`에 쓸 수 있는 로봇 목록(이름, 제조사, 도달 거리, 가반 하중) | 아니오 |
| `add_robot` | 라이브러리 로봇을 월드 원점에 추가. 셀에서의 이름을 돌려줌 | 예 |
| `place_robot` | 로봇 베이스를 월드에 배치: `xyz_mm`, `rpy_deg` | 예 |
| `add_waypoint` | 로봇의 웨이포인트 추가. 기준은 `world`, `base`(UR 티치펜던트의 Base), 또는 작업 좌표계 이름. TCP 위치 `xyz_mm`, 자세 `rpy_deg` (`[180, 0, 0]`이면 월드에서 툴이 아래를 향함). 도달 여부 반환 | 예 |
| `waypoint_info` | 웨이포인트를 월드·베이스·모든 작업 좌표계 기준으로 본 값, 도달 여부, 관절값, 충돌·한계 문제 | 아니오 |
| `create_path` | 한 로봇의 웨이포인트를 순서대로 잇는 경로 만들기. `linear`는 웨이포인트마다 직선 이동(LIN) 여부 | 예 |
| `plan_path` | 경로 계획. 상태, 예상 시간, 최대 TCP 속도, 실패 이유 반환. 오래 걸리면 `planning` 상태가 오므로 잠시 후 `overview`로 결과 확인 | 예 |
| `undo` / `redo` | 마지막 편집(에이전트 것이든 사용자 것이든) 되돌리기 / 다시 하기 | 예 |

### 자주 쓰는 순서

1. `library_robots`로 이름 확인 → `add_robot`(예: `UR5e`) → 셀에서의 이름(예: `UR5e_1`)을 받음
2. `place_robot`으로 베이스 위치 지정
3. `add_waypoint`를 필요한 만큼 호출. 응답의 `reachable`이 거짓이면 다른 자세로 다시 시도
4. `create_path`로 웨이포인트 이름을 순서대로 넘김 → `plan_path`

## 5. 안전

- **꺼져 있는 것이 기본입니다.** 사용하지 않을 때는 연결을 끄세요.
- **이 PC에서만 연결**됩니다(`127.0.0.1`). 외부 네트워크에서 접근할 수 없습니다.
- **토큰이 없으면 거부됩니다.** 모든 요청에 `Authorization: Bearer <토큰>`이 필요합니다.
- **모든 편집은 Ctrl+Z로 되돌릴 수 있습니다.** 에이전트가 한 편집도 사용자가 한 편집과 같은 실행 취소 기록에 들어갑니다.
  `undo` 도구로도 되돌릴 수 있습니다.
- **상태바**에 "AI 에이전트: …"가 나타나 마지막 편집을 알려 줍니다. **에이전트 작업 기록**에서 호출 목록과 실패 이유를 볼 수 있습니다.
- 로봇 동작은 아직 실제 로봇에 보내지 않습니다. 이 도구들은 시뮬레이션 셀만 편집합니다.

## 6. 예시 대화

사용자: "UR5e를 원점에 놓고 바닥에서 300 mm 위에 툴이 아래를 향하는 점 두 개를 만든 뒤 경로를 계획해 줘."

에이전트가 하는 일(요약):

1. `library_robots`로 `UR5e`가 있음을 확인
2. `add_robot` `UR5e` → `UR5e_1`
3. `place_robot` `UR5e_1`, 위치 `[0, 0, 0]`, 자세 `[0, 0, 0]`
4. `add_waypoint` `UR5e_1`, 기준 `world`, 위치 `[400, 0, 300]`, 자세 `[180, 0, 0]` → `P1`
5. `add_waypoint` `UR5e_1`, 기준 `world`, 위치 `[400, 200, 300]`, 자세 `[180, 0, 0]` → `P2`
6. `create_path` `["P1", "P2"]`, `linear: [false, true]` → `Path1`
7. `plan_path` `Path1` → 상태가 `planning`이면 잠시 후 `overview`로 확인

사용자가 "P2 자세가 마음에 안 들어"라고 하면 에이전트는 `undo`로 편집을 되돌리고 다른 자세로 `add_waypoint`를 다시 호출합니다.
화면에서는 각 단계가 바로 보이고, 사용자는 Ctrl+Z로 같은 편집을 되돌릴 수 있습니다.

> 위 좌표와 자세는 설명을 위한 예입니다. 실제 결과(도달 여부, 예상 시간)는 셀과 로봇 설정에 따라 다릅니다.
