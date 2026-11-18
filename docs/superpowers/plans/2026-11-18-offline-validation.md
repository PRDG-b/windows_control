# Offline Execute and Validate Implementation Plan

## 설계와 범위
사용자가 승인한 재생 실행 → 실패 사례 → 상태/사후조건 확장 순서로 진행한다.
Rust 표준 라이브러리만 사용한다. 기존 클로저 기반 실행기를 확장하고 실제 CLI와 재생 CLI가 같은 검증기를 호출한다.
대안인 별도 시뮬레이터나 브라우저 자동화 프레임워크는 현재 관측 검증 문제를 해결하는 데 필요하지 않다.
실제 장치/USB, OCR 엔진 정확도, 모델 계획기, 이벤트 선점은 이번 완료 조건에 포함하지 않는다.

## 완료 조건
- 하드웨어를 열기 전에 분기하는 `replay` CLI, TSV 관측 재생과 가상 클릭 기록.
- 관측마다 frame_id, capture 시각, 해상도를 검사. 오래된/역순/중복 프레임, 잘못된 좌표는 실행/성공 근거로 사용하지 않는다.
- 입력 1회와 검증 완료를 구분. 복합 required/forbidden 문구, 명시적 실패 문구, timeout, 관측 오류를 구분한다.
- 보고서는 상태/이유/관측 수/입력 수/좌표/시각만 출력하고 OCR 원문을 출력하지 않는다.
- 성공, 실패, 중복/누락/낮은 신뢰도, 이미 완료, stale frame, timeout, 입력 실패, 관측 실패 테스트.
- 실제 웹 실행/HDMI/HID 결과와 재생 결과를 구분한다.

## 작업
- [x] 관측 메타데이터와 공통 검증기: 실패 테스트 → 구현 → 로컬 테스트.
- [x] 재생 파일 로더/CLI/예제: CLI 실패 테스트 → 구현 → 예제 실행.
- [x] 실제 CLI 관측 메타데이터 연결, 문서와 검증 결과 기록.
- [x] fmt/check/test/diff 검사 및 독립 코드 리뷰.

## 실행 결정
현재 작업 트리의 기존 미커밋 구현 위에서 직접 진행한다. 원격 배포/하드웨어 조작 없이 로컬에서 검증한다. 별도 checkout으로 기존 변경을 누락시키지 않는다. 커밋/푸시는 요청되지 않아 수행하지 않는다.

## 검증 기록 (2026-11-18)
- 새 API 미구현/CLI 미구현의 실패를 확인한 뒤 구현했다. 입력 중 수집된 완료 프레임, timeout 프레임 증거 누락, 명시적 FAILED 기대 조건의 회귀도 실패 재현 후 수정했다.
- 독립 리뷰에서 입력 전 낮은 신뢰도 forbidden이 입력을 허용하는 문제를 발견했다. uncertain_state/actions=0 회귀로 수정했다. 입력 오류 시 elapsed 누락 및 늦은 관측 증거 누락도 수정하고 테스트했다.
- 실제 CLI 입력 소유권 잠금 추가. 경쟁 획득 거부와 정상 Drop 후 재획득 테스트 통과. 강제 종료 시 stale lock은 자동 삭제하지 않는다.
- `cargo fmt --check`, `cargo check --offline`, `cargo test --offline`, `git diff --check` 통과. Rust 28개 테스트, 이 중 CLI 테스트는 합성 재생 13개 사례의 결과와 입력 수를 확인한다. 환경 경로 canonicalize 경고와 Git CRLF 안내는 있으나 검사 실패는 없다.
- `cargo run --offline -- replay tests/fixtures/success.replay`: exit 0, mode=replay, completed/validated, actions=1, click=[140,220], 가상 20ms/3프레임.
- `cargo run --offline -- replay tests/fixtures/stale.replay`: 예상 exit 1, unconfirmed/stale_frame, actions=1, 가상 10ms/2프레임. 중복 ID 완료 프레임은 성공 처리하지 않았다.
- 재생 시간은 가상 시간이며 Pi 처리 지연 실측이 아니다. 실제 장치 재캡처/입력 시험은 이번 변경에서 미수행이다.
- 사용법/제약: `docs/offline-validation.md`. 이번 완료는 하드웨어 독립 실행·검증 기반이며 이미지 재OCR·브라우저 통합·실제 HID 검증은 별도 단계다.
