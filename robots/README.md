# 로봇 라이브러리

GraphCTRL이 "로봇 > 로봇 추가" 목록에 보여 주는 로봇들입니다. 실행 파일과 분리된 애드온 폴더로, 폴더를 넣거나 빼면 목록이
바뀝니다. 프로그램은 이 `robots` 폴더를 실행 파일 옆이나 작업 폴더(또는 그 위 네 단계)에서 찾습니다. 배포할 때는 실행 파일 옆에
`robots` 폴더를 함께 두면 됩니다.

## 패키지 구조

```
robots/<패키지>/
  robot.json            이름·제조사·설명 파일·도달 거리·가반 하중·라이선스
  <설명>.urdf            메시는 package://<패키지>/meshes/... 로 참조
  meshes/visual/        화면에 그리는 메시(DAE·STL·OBJ)
  meshes/collision/     충돌 검사용 메시
  LICENSE               패키지 내용의 라이선스(도움말 > 라이선스에 표시)
```

`robot.json` 예:

```json
{
  "name": "UR5e",
  "vendor": "Universal Robots",
  "description": "ur5e.urdf",
  "reach_m": 0.85,
  "payload_kg": 5.0,
  "license": "LICENSE",
  "license_name": "BSD-3-Clause",
  "attribution": "Universal Robots A/S and contributors, Universal_Robots_ROS2_Description 4.4.1"
}
```

프로젝트 파일은 라이브러리 로봇을 `library://<패키지>`로 기억하므로, 다른 PC에서 라이브러리 위치가 달라도 열립니다.
UR형 6축 팔은 기종 정보 없이도 체인 모양으로 인식되어 해석적 IK가 붙습니다.

## 포함된 패키지

| 패키지 | 내용 | 출처·라이선스 |
|---|---|---|
| `ur3e`, `ur5e`, `ur10e`, `ur16e` | Universal Robots e-Series (시각·충돌 메시) | Universal_Robots_ROS2_Description 4.4.1, BSD-3-Clause. 파일별 해시는 `SOURCES.toml` |
| `sample-6r` | 기본 도형으로 만든 6축 샘플 팔 | GraphCTRL, MIT |

UR URDF는 `assets/ur/config`의 공식 YAML에서 `tools/gen_ur_urdf.py`로 생성했습니다.
