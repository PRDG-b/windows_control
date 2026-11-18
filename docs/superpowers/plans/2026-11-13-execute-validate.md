# Execute and Validate Implementation Plan

최신 아키텍처는 [2026-11-18 에이전트 설계](../../2026-11-18-agent-design.md)를 따른다. 아래 Goal/Task는 초기 단일 행동 MVP 계획이며 현재 범용 에이전트 전체의 완료 조건이 아니다. 후속 목표 계약·세션·모델 연결은 설계 단계다. 과거 테스트 서버 배치 기록은 이력으로 보존하며 현재 정책은 작업 PC LAN 서버다.

> Agentic workers: executing-plans로 아래 작업을 순서대로 진행한다.

**Goal:** OCR 대상 클릭을 한 번 실행하고 새 관측의 기대 문구로 성공/실패를 판정한다.
**Architecture:** 기존 Rust HID/캡처를 유지한다. OCR TSV 파서와 하드웨어 독립 검증 루프를 추가하고 CLI에서 연결한다. 모델 없이 실행·검증의 정확성을 먼저 확보한다.
**Tech Stack:** Rust 2024, std, image, v4l2-ctl, Tesseract.
**Spec:** `docs/2026-11-13-feasibility.md`.

## Global Constraints
- 기본 실행은 HID에 접근하지 않는다. 실제 입력은 명시적 click 명령에서만 수행한다.
- 텍스트 대상은 confidence를 통과한 단일 단어의 정확 일치로 제한한다. 다단어 문구·부분 일치 클릭은 후속 범위다.
- timeout과 관측 오류를 성공으로 처리하지 않는다. 행동 자동 재실행은 없다.
- 예상 문구가 행동 전 이미 있으면 실행하지 않는다. 성공은 행동 이후 관측에서만 확인한다.
- timeout은 관측 간 deadline이며 개별 외부 프로세스는 별도 시간 제한을 둔다.

## Review Focus
- 중복 대상·잘못된 confidence·잘못된 TSV 좌표는 클릭하지 않는다.
- 기대 문구가 이미 있는 화면을 새로운 성공으로 오인하지 않는다.
- OCR/캡처 실패가 timeout 또는 성공으로 감춰지지 않는다.
- 지원하지 않는 문자가 입력 앞부분 전송 이후 발견되지 않는다.
- HID 종료 보고서 길이가 실제 장치와 일치한다.

## Task 1: 기반 복구
- [ ] 회귀 테스트 추가 후 실패 확인: 별표 매핑, 미지원 문자 사전 검사, 마우스 보고서 길이, 캡처 stdout 처리.
- [ ] `src/capture.rs`: 명시적 stdout 스트리밍 및 JPEG 디코딩 검사.
- [ ] `src/hid.rs`, `src/helper.rs`: 보고서·문자 입력 오류 수정, 민감 문자 로그 제거.
- [ ] `src/task.rs`: 컴파일 복구 및 하드코딩 인증 제거.

## Task 2: 관측·실행·검증
- [ ] `src/ocr.rs`: `parse_tsv(&str) -> Result<Vec<TextBox>, String>` 및 종료 상태 검사. 정상/손상/저신뢰 TSV 테스트.
- [ ] `src/validation.rs`: `execute_and_validate`는 observe/action closure를 받고 전제/사후조건 검사. 성공, timeout, 이미 완료, 관측 실패, 모호한 대상 테스트를 먼저 실패시킨다.
- [ ] `src/main.rs`: observe, click <단어> <기대단어> [timeout초], 도움말. 실제 HID 연결은 click에서만 한다.
- [ ] `tests/fixtures`, `test-app/index.html`: 재생 입력과 지연/실패/팝업 테스트 화면.

## Task 3: 검증 및 운영 안내
- [ ] `cargo fmt --check`, `cargo check --offline`, `cargo test --offline`, `git diff --check`.
- [ ] Pi 별도 임시 디렉터리에서 빌드/테스트 및 observe 실측. 원본 저장소 덮어쓰기 금지.
- [ ] 실제 HID 테스트는 테스트 화면이 준비된 경우에 수행하고 독립 채점 결과를 기록한다.
- [ ] 결과/명령/제한을 `docs/execute-validate.md`에 기록한다. 미수행 계층은 명시한다.

## 진행 기록
- 기존 `cargo check --offline`: task.rs:24 세미콜론 누락으로 실패 확인.
- 사용자의 계획 수립 및 진행 요청에 따라 현재 작업 공간에서 직접 수행한다. 커밋·push는 이 단계에 포함하지 않는다.
- 사용자 지정 개발 규칙: 기존 소스 수정 → Pi에서 `cargo run --offline -- ...` → 즉시 재캡처/OCR/이미지 확인 → 현재 상태 정의 → 다음 단계 결정. 상세 규칙은 `AGENTS.md`를 따른다.
- 실제 HID 검증: `open-url`로 Edge 및 테스트 페이지 도달 성공(17.820초). PSM 11의 RUN 누락을 동일 프레임의 PSM 3/6과 비교해 PSM 3으로 수정. RUN→COMPLETED 검증 성공(12.723초), 독립 채점 completed/clicks=1. FAILURE 시나리오 전환 성공(10.491초), ready/clicks=0.
- 사용자 지시에 따라 Pi 테스트 서버(PID 5921)와 현재 작업 Windows PC 서버(PID 23708)를 종료했다. Pi 8765 리스너 없음 확인. 테스트 웹앱은 제어 대상 PC에서 실행해야 한다.
- 로컬 HTML 저장 시도는 timeout으로 실패했다. 후속 캡처는 바탕화면이었다. 저장 여부·초점을 확인하지 않고 후속 입력을 이어간 접근을 중단한다. 제어 대상 PC에서 웹앱 실행 완료로 기록하지 않는다.
- 미구현 타이핑 최적화 실험은 보류하고 해당 실험용 실패 테스트만 제거했다. 기존 입력 동작은 유지한다. 다음 작업은 위 필수 개발 사이클로 재개한다.

### 2026-11-16: 제어 대상 PC 로컬 웹앱 실행
- 현재 상태 확인: `cargo run --offline -- observe current-state.jpg`. 바탕화면 확인 후 이전 로컬 파일을 열었으나 ERR_FILE_NOT_FOUND를 재캡처로 확인했다.
- `open-url`에 로컬 C: HTML URL 지원, `observe <snapshot.jpg>`에 동일 프레임 이미지 저장을 추가했다.
- `cargo run --offline -- focus-address` 실행 후 `address-focus.jpg`로 Edge 주소창 선택을 확인했다.
- `cargo run --offline -- local-app`: 내장 HTML을 percent-encoded data URL로 HID 입력. SUCCESSMODE 사후조건으로 132.431초에 검증 성공. 별도 웹 서버 없이 제어 대상 브라우저에서 실행한다. 디스크 설치는 아니다.
- 재캡처 `local-app-running.jpg`에서 Execute and Validate 페이지, SUCCESSMODE, READY 및 RUN 버튼을 시각 확인했다. Pi 8765 리스너 없음 확인.
- 후속 `cargo run --offline -- click RUN COMPLETED 30`은 대상 누락/저신뢰/모호성 검사에서 입력 전에 거부됐다. 재클릭하지 않았다. 최종 `local-app-final.jpg`(2026-11-16 08:35:03 +09:00)에서 실행 중인 페이지 상태를 다시 확인했다. RUN 반응 검증 완료로 기록하지 않는다.
- 문자열 전체 검증을 유지하면서 대량 입력 간격을 명시할 수 있게 수정했다. 문자/키코드 대신 문자 수만 이력에 기록한다. 기본 타이핑 간격은 유지한다.
- 고정 대기만으로 저장 대화상자를 연속 조작하던 save-page는 비활성화하고 회귀 테스트를 추가했다.
- 로컬/Pi: Rust 단위 16개 + 재생 통합 1개 통과. 로컬 fmt/check/diff 검사 통과. 다음 단계는 RUN 인식 거부 원인 분석이며 웹앱 실행 자체는 완료다.

### RUN 거부 원인 조사 (2026-11-16 08:37 이후)
- 클릭은 실행하지 않았다. 거부 조건은 원본 OCR의 정확 일치 RUN 중 confidence >= 70 후보 수가 1이 아닌 경우다. 기존 오류 문구는 누락/저신뢰/중복을 구분하지 않는다.
- `cargo run --offline -- observe run-diagnosis.jpg`: Rust 이미지 디코딩/재저장은 성공했으나, 원본 MJPEG를 받은 Tesseract는 premature end of data segment / scanline 1070 read error로 종료 코드 1을 반환했다.
- 같은 프레임의 재저장 JPEG에 동일한 Tesseract PSM 3을 적용하면 RUN 1개, confidence 96.414444, box (85,596,53,21)을 확인했다. 버튼 부재가 아니라 원본 JPEG 전달/해독 경로의 문제가 재현됐다.
- `observe <snapshot.jpg> <target-word>`에 민감한 전체 OCR 텍스트를 출력하지 않는 대상 후보 수/신뢰도 진단을 추가했다. `cargo run --offline -- observe run-candidates.jpg RUN`에서도 원본 JPEG read error가 반복되어 후보 파싱 전에 중단됐다.
- 최초 클릭 거부 시점의 원본 TSV는 남아 있지 않으므로 그 시점의 누락/저신뢰/중복 중 무엇이었는지는 확정하지 않는다. 현재 재현된 JPEG 손상과 최초 거부를 동일 원인으로 단정하지 않는다. 손상 이미지를 재저장해 인식된 사실만으로 캡처 전체의 무결성이 보장되지는 않는다.
- 사용자 요청으로 `compare-ocr` 실행: 각 프레임의 원본을 재인코딩 없이 한 번 저장하고 파일/stdin에 동일 바이트를 제공했다. 01·03은 양쪽 성공 및 출력 완전 일치, 02는 양쪽 모두 같은 JPEG scanline 1070 오류로 실패했다. stdin 전달 방식이 오류 원인이라는 가설은 이 재현에서 배제했다. 상세 바이트 수·해시는 `docs/execute-validate.md`의 동일 원본 비교에 기록했다. HID 입력 없음.
- 원본 JPEG/디코더 후속 검증: 보존된 02번에 EOI(FF D9)가 파일 전체에 없으며 FF E3에서 끝나는 것을 확인했다. 01/03은 정상 EOI로 끝난다. `cargo run --offline -- verify-jpeg`로 Rust RGB 디코딩→무손실 PPM 대조군을 만들면 3개 모두 OCR 성공했다. FFmpeg도 02에서 비정상 APP fields 메시지를 냈으나 rc=0이었다. Leptonica 1.84.1 공식 소스의 기본 JPEG 경고 중단 동작이 실제 오류와 일치한다. 원본 JPEG 이상과 디코더 허용 범위 차이는 확인됐으며 발생 계층은 아직 미확정이다. 현재 캡처 실행 경로를 자동 보정하도록 바꾸지는 않았다.
- 계층 조사: Rust 없는 v4l2-ctl 직접 파일에서도 EOI 누락 재현. 독립 ioctl/mmap 진단을 추가해 파일 쓰기 전의 ERROR 플래그/시작 마커 없는 버퍼도 포착했다. 이 사례와 원래의 무플래그 EOI 누락은 구분한다. 상세 통계·파일·제약은 검증 문서에 기록. 장치/USB/드라이버 사이 최종 분리에 필요한 usbmon은 현재 sudo 인증 때문에 접근 불가하여 사용자에게 관리자 진단 가능 여부를 요청했다.
- 사용자 관리자 수집 `/tmp/windows-control-usbtrace-kgcrnw6q` 분석 완료: USB 원본 첫 8프레임부터 EOI가 없었다. V4L2 60개는 USB 6~65번과 모두 바이트 단위 일치하여 이번 손상이 UVC 드라이버 이후 복사/저장에서 생긴 것이 아님을 확인했다. 데이터 URB status는 모두 0, -2는 종료 직후 0바이트 취소였다. 보드 기동/펌웨어 출력 문제를 가장 강하게 지지하되 물리 버스/HCD 전체 가능성 배제 및 펌웨어 코드 수준 확정은 별도임을 문서화했다. 재현 가능한 오프라인 분석기와 잘림/경계 합성 테스트 2개를 추가했다.
- 사용자 요청으로 capture_raw의 초기 폐기 수만 4→30 변경. 캡처 제한 3초 등 나머지 유지. 로컬/Pi Rust 테스트 17개 통과. cargo run으로 스트림을 재시작하는 10회 비교에서 원본 마커 10/10, 파일 및 stdin OCR 각각 10/10 성공, 출력 10/10 완전 일치. 최종 재캡처에서 READY 화면 확인. JPEG 오류는 이번 반복에서 재현되지 않았으나 RUN 정확 일치 후보 0개 문제는 별도 미해결이다. 상세 로그는 Pi skip30-20261116-01과 검증 문서 참조.

### 2026-11-18: 웹 재실행·캡처/OCR 재시험
- 이전 `/tmp/windows-control-ev.Z69aL5`가 없어 새 검증 사본 `/tmp/windows-control-ev.PJrHfR`을 생성했다. 원본 `~/windows_control`은 변경하지 않았다. 소스는 초기 폐기 30장 수정이 포함된 현재 로컬 사본이다.
- Pi의 단위 16개 + 재생 통합 1개 통과. 웹앱 JavaScript 문법 검사 통과.
- 최초 장치 60fps에서 `cargo run --offline -- observe initial-20261118.jpg`는 status 124, bytes 0. 30fps 및 640×480 재협상도 수신 0바이트였다. 최종 장치 설정은 1920×1080 MJPG 30fps다.
- `cargo run --offline --example wake`의 마우스 깨우기 입력은 전송했다. 다른 HID 입력이나 웹 열기는 현재 화면을 확인하지 못해 실행하지 않았다.
- `observe initial-30fps.jpg`, `observe initial-restored.jpg`, 사용자 확인 후 `observe after-reconnect.jpg`도 모두 수신 0바이트로 실패. 캡처할 때마다 uvcvideo -71 오류가 새로 발생했다. 이전의 EOI 누락 문제와 구분한다.
- 10:36 시점 커널 기록 확인: 부팅 시 USB device number 3으로 인식된 이력만 있고 그 뒤 분리/재연결 기록이 없었다. 물리 USB 재연결을 구체적으로 요청했다. 현재 웹 재실행·캡처/OCR 성공으로 기록하지 않는다.
- 사용자가 v4l2 정상 작동을 알려 캡처 보드 문제라는 단정을 철회하고 성공한 정확한 명령을 요청했다. 10:51 장치는 video0/MJPG1080p30fps, 점유 프로세스 없음. 포맷 재설정·프레임 폐기를 제외한 `timeout 10s v4l2-ctl -d /dev/video0 --stream-mmap=4 --stream-count=1 --stream-to=minimal-check.jpg`도 10.013초 후 status124/0바이트였다. 현재 코드의 3초 제한·30장 폐기만의 문제로는 설명되지 않지만 보드 고장이 입증된 것은 아니다. 사용자 성공 명령의 장치·환경·옵션 대조가 필요하다. 최근 dwc2 USB gadget 재인식과 캡처 보드 UVC 재인식은 구분한다.

### 2026-11-18: 캡처 재시도 및 LAN 서버 방식
- 새 배포 `/tmp/windows-control-ev.PJrHfR` 사용. 원격 원본 저장소는 보존했다.
- 재연결 후 직접 v4l2 캡처는 정상 수신. skip=30 조건의 캡처는 4032ms가 걸려 기존 timeout=3s가 부족했다. 기존 capture_raw의 제한을 10s로 수정한 소스로 cargo run 후 캡처·OCR 성공(1920×1080, 11단어, 11369ms). 이전 USB -71 오류와 이번 deadline 부족은 구분한다. 캡처 보드 고장으로 단정하지 않는다.
- 사용자 지시에 따라 작업 PC 192.168.0.20:8765에 기존 test-app/server.py 실행. Pi에서 HTTP 200 확인. Pi에는 웹 서버를 띄우지 않는다. AGENTS.md 기본 경로를 LAN HTTP 방식으로 변경했다.
- 긴 local-app data URL 입력은 이번 시도에서 사후조건 timeout으로 실패했다. 반복하지 않고 기존 Rust open-url 경로를 사용한다.
- `cargo run --offline -- open-url http://192.168.0.20:8765/ SUCCESSMODE 60`: 18521ms에 사후조건 확인. 별도 recapture는 바탕화면이었으므로 지속적인 전면 상태 확인으로 간주하지 않는다. 이후 open-browser 재캡처는 빈 새 탭이었다. 상태 변화 원인은 미확정이다.
- 로컬 fmt/check/offline Rust 테스트 17개/diff 공백 검사 통과.
- 최종 확인(2026-11-18 11:54 KST): 빈 새 탭 상태 확인 후 `open-url http://192.168.0.20:8765/ SUCCESSMODE 60` 실행, 18146ms 검증 성공. 즉시 `observe lan-final-confirmed.jpg SUCCESSMODE` 재캡처: 1920×1080, OCR 100단어, SUCCESSMODE 1개/confidence 92.15, 7702ms. 실제 이미지에서 HTTP 주소·테스트 페이지·SUCCESSMODE·READY·RUN 버튼 확인. RUN 클릭은 수행하지 않았다.

### 2026-11-18: 하드웨어 독립 실행·검증 기반
- 실행 구현은 src/execution.rs, 재생 CLI는 src/replay.rs. 기존 validation API와 실제 CLI가 같은 검증기를 사용한다.
- `cargo run --offline -- replay tests/fixtures/success.replay`: validated/actions=1, exit0. `stale.replay`: stale_frame/actions=1, 예상 exit1. 카메라·Tesseract·HID를 열지 않았다.
- 단일 입력 시도, 새 프레임/시각/좌표 검사, 복합 required/forbidden, 오류/불명확/timeout 구분, 실제 CLI 소유권 잠금 추가. 보고서는 OCR 원문 없이 출력한다.
- Rust 28개 테스트 통과. CLI 합성 사례 13개 결과와 입력 수 검증. fmt/check/offline test/diff 검사 통과. 독립 리뷰의 낮은 신뢰도 오류 상태 입력 문제를 수정하고 회귀 테스트 통과.
- 이번 수정 소스의 Pi 실행/실제 화면 재캡처/HID end-to-end는 미수행이다. 로컬 합성 재생 결과를 실제 장치 성공으로 기록하지 않는다.
- 사용법: docs/offline-validation.md. 세부 계획/기록: docs/superpowers/plans/2026-11-18-offline-validation.md.

### 2026-11-18: 대화 기반 에이전트 설계 갱신
- 최신 설계 docs/2026-11-18-agent-design.md 작성. task 공통 실행 절차와 목표별 의미 검증을 분리하고, 공통 판정 연산/선언적 계약을 우선하도록 정리했다.
- decision 모델 적재만으로 범용화하지 않는다. 관측/후보/목표/근거/평가와 함께 연결하며 모델 결정은 실행기 검사를 받는다.
- what to do differently를 act/wait/reobserve/revise_plan/finish/stop 결정과 세션 이력으로 공통화한다. 전체 deadline 및 의미상 중복 행동을 유지하고 불명확한 부작용은 자동 재실행하지 않는다.
- 현재 단어 기반 정책과 구현된 TSV 재생을 다단계 세션/범용 의미 검증의 완료로 표현하지 않는다. 후속 순서는 목표 계약 → 세션 → 규칙/모델 비교 → 미지 task/실제 장치 평가다.
- AGENTS.md, 초기 타당성 문서, 오프라인 사용 문서에 최신 설계 참조 및 역할 경계를 반영했다. 이번 변경은 문서만 수정했으며 모델 설치·소스 변경·실제 입력 시험은 수행하지 않았다.

### 2026-11-18: 금융 검색 수집 task 명세
- tasks/financial-search/README.md, suite.json, result-template.json 작성. Google/Bing/Naver/DuckDuckGo, 기본 6지표/24case 및 선택 암호화폐 포함 28case. smoke는 12case.
- 최신성은 case별 원문 관측 시점을 기준으로 판단한다. 종목·단위·세션·시각·원출처·근거를 검증하며 검색 스니펫을 최종 증거로 쓰지 않는다. 같은 발행자를 여러 엔진에서 발견한 것은 독립 출처로 세지 않는다.
- JSON/ID/profile수/계약 참조/검색 URL 84개 인코딩/문서 링크 검사 통과. 값은 미수집이며 실제 에이전트 실행 및 금융 task runner 구현은 미수행이다. 현재 replay CLI 입력과 구분한다.

### 2026-11-18: Rust 화면 전이 탐색기
- docs/exploration.md 및 구현 계획 2026-11-18-exploration.md 참조. 명시적 어댑터의 허용 후보에 대해 BFS 전이 탐색, 현재 프레임에서 경로 재해석, 복원 확인, 부분 그래프/미탐색 목록 저장 구현.
- explore-demo와 explore-test-web CLI 추가. 기본 실행은 도움말이며 실제 입력은 명시적 테스트 웹 명령으로만 수행한다. 테스트 웹 어댑터는 단일 Ctrl+R로 복원한 뒤 새 관측에서 root를 검증한다.
- 로컬/Pi Rust 38개 테스트 통과. fmt/check/offline test/diff 검사 통과. 독립 리뷰에서 관측 예산/timeout과 복원 초점 문제를 수정하고 최종 중요 문제 없음 확인.
- 로컬/Pi 합성 데모: 노드3/간선3/probe3/입력7/복원확인/pending0. 가상 UI 결과이며 실제 HID 성공은 아니다.
- Pi 최신 소스 /tmp/windows-control-explore.SnsW8o에서 explore-test-web live-flow-01 1 실행: No Signal 3프레임/unknown_state/입력0. 직후 explore-final.jpg 재캡처·OCR 및 실제 이미지 확인에서도 앱 화면은 없었음(검은 화면, 직전 No Signal). 실제 앱의 전이/복원 성공은 미수행이다.
