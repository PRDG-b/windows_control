# 재사용 가능한 UI 관측과 가상 탐색 구현

## 목표와 결정
고정 앱 표식 없이 화면 요소/배치/내용으로 관측 상태를 식별하고 프레임별 조작 후보를 만든다. 기존 fixture 전용 어댑터는 회귀용으로 남기고 신규 generic 경로와 명확히 구분한다.
SDK/모델을 설치하지 않고 Rust std와 기존 image를 사용한다. 탐색 코어는 재사용한다. Rerun 코드는 사용자가 직접 작성하기로 했으므로 수정하지 않는다.

## 설계
- 픽셀 edge/연결 성분/사각 경계 검사 + OCR 요소 연결. RectControl/Text/VisualRegion/Panel 후보이며 실제 위젯 역할을 확정하지 않는다.
- 관측은 구조·앵커 텍스트·동적 값·마스킹한 시각 특징을 분리한다. 순수 숫자/날짜 변화는 데이터 revision으로 보존하고, 의미를 지정한 영역은 상태 대응에도 반영한다.
- 두 번 연속 대응한 관측을 안정 상태로 등록. S/L ID는 순차 생성하고 원문을 ID/로그에 넣지 않는다. 후보는 상태 내 객체 ID와 현재 frame_id/좌표로 표현한다.
- 기본 후보 권한은 Unknown. 컨테이너의 폐기 가능한 fixture에서만 명시적 sandbox 정책으로 사각 컨트롤 클릭을 허용한다. 복원은 어댑터 계약이며 범용 Ctrl+R 가정은 하지 않는다.
- Windows/Pi 독립 Rust 시뮬레이션과 컨테이너 브라우저 어댑터를 제공한다. 컨테이너에서 screenshot→Tesseract→동일 분석기→좌표 클릭→재캡처. DOM은 탐색 입력에 사용하지 않는다.
- Docker/Playwright/Tesseract는 개발 컨테이너 안에만 설치. 실제 Windows 앱 검증은 향후 VM, HDMI/HID 경로는 별도 실측으로 남긴다.

## 완료 조건과 작업
- [x] 인식/상태 대응 실패 테스트 → 구현: 사각 제어, 중복 문구 구별, 프레임 신선함, 숫자 변경, 문구/팝업/배율 변경, 일시적 OCR 실패 (`tests/screen_analysis.rs`).
- [x] 앱 표식 없는 synthetic reactive UI와 탐색 그래프 검증, 후보 좌표 overlay/메타데이터 출력 (`tests/visual_exploration.rs`, `src/screen_output.rs`).
- [x] 컨테이너 브라우저 어댑터/개발 환경/실행 안내 및 실제 컨테이너 시험 (`windows-control-browser` 컨테이너 빌드 및 `diagnostics/browser-run-1` 실측 완료: 4개 상태, 3개 엣지, 완전 탐색).
- [x] 로컬 fmt/check/offline test/diff 및 코드 리뷰 반영 (RGB 색상 채널 보존, 뷰포트 경계 검사 등 단위/통합 테스트 47건 및 브리지 테스트 6건 통과).

## 검증 결과 요약 (2026-11-19)
- **컨테이너 브라우저 실측:** `windows_control explore-browser diagnostics/browser-run-1 10`
  - 결과: `{"mode":"simulation","stop":"complete","nodes":4,"edges":3,"probes":3,"input_attempts":7,"observed_frames":16,"restored":true,"pending":0,"elapsed_ms":11041}`
  - 상태 전이 그래프: `S000001` (Workspace/Root) → `S000002` (Details) → `S000004` (Details Expanded) 및 `S000001` → `S000003` (Preferences)
  - DOM 오라클 검증: `window.testOracle.history` = `["Browse", "Preferences", "Browse", "Expand"]` 일치 확인.
  - 관측 메타데이터: `frame-0001`~`frame-0016` (JPG/TSV), `ui-0001`~`ui-0016` (오버레이), `graph.dot`, `graph.mmd`, `report.json` 정상 생성.

## 검증 경계
시각적 상태 대응은 앱 내부 상태나 task 성공의 증명이 아니다. 사각형 검출 점수는 모델 확률이나 클릭 안전성을 의미하지 않는다. 기본 공간/유사도 한도는 조정 가능한 휴리스틱이며 실제 화면으로 평가해야 한다. 합성 OCR 재생·브라우저 OCR·실제 HDMI/HID를 구분한다.
