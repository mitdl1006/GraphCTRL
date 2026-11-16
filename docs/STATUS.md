# 진행 상태

계획: [PLAN.md](PLAN.md) · 근거 문서: [research/](research/) · 단계별 증거: [evidence/](evidence/)

| 단계 | 상태 | 비고 |
|---|---|---|
| 0. 골격·환경 | 완료(한글 입력 시험만 사용자 확인 대기) | 증거: [evidence/stage-0.md](evidence/stage-0.md). Windows에서 빌드·실행 확인됨(`-j`로 동시 작업 수 제한). `gen-ur`는 Stage 2b, `bench-quality`는 Stage 3a에서 추가 |
| 1. 기반 + 계약 동결 | 진행 중 | foundation 7개 완료. 남음: api 2개, 나머지 크레이트의 공개 타입 동결, 픽스처 |
| 2a / 2b 헤드리스 도메인 | 대기 | |
| 3a / 3b 통합 도메인 | 대기 | |
| 4. UI 키트·뷰포트 | 대기 | |
| 5a–5f 앱 조립 | 대기 | |
| 6. 하드닝 | 대기 | |

## 크레이트별 상태

| 크레이트 | 상태 | 테스트 |
|---|---|---|
| gc_core | 완료 | 10 |
| gc_linalg | 완료 | 19 (QR 랭크 == SVD 랭크, Kahan 행렬 포함) |
| gc_math | 완료 | 16 |
| gc_expr | 완료 | 16 |
| gc_i18n | 완료(메시지는 UI 작업과 함께 추가) | 5 |
| gc_jobs | 완료 | 8 |
| gc_mesh | 완료 | 14 |
| 그 외 | 스텁 | |
