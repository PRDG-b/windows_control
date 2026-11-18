# 하드웨어 없는 Execute and Validate

최신 후속 구조는 [에이전트 설계](2026-11-18-agent-design.md)를 참조한다. 이 문서는 현재 구현 사용법이다. 단어 기반 Policy는 범용 task 성공 판정이 아니며, 목표 계약·다단계 세션·decision 모델은 아직 구현되지 않았다.

## 실행

작업 PC에서 Pi·제어 PC·카메라·Tesseract 없이 실행한다.

```powershell
cargo run --offline -- replay tests/fixtures/success.replay
cargo run --offline -- replay tests/fixtures/compound.replay
cargo run --offline -- replay tests/fixtures/stale.replay
cargo test --offline
```

성공은 종료 코드 0, 나머지는 1이다. 실패 시 JSON 보고서를 먼저 출력한다. `actions`는 재생에서 **가상 입력 시도 수**, live에서는 실제 입력 **시도 수**다. 전송 실패도 1회로 기록하며 성공적인 전달을 의미하지 않는다. `click`은 선택된 좌표다.

보고서의 `state`는 실행 검증 상태다. `completed`는 사후조건 확인, `failed`는 명시적 실패 관측, `blocked`는 입력 전 거부, `unconfirmed`는 입력 시도 후 결과 미확인이다. timeout도 실패 종료하며, 입력 결과를 추측해 재실행하지 않는다. 앱의 모든 내부 상태를 추론하는 기능은 아니다.

## 재생 파일

모든 제공 fixture는 합성 데이터다. 실제 캡처·OCR 엔진 정확도를 입증하지 않는다.

```text
# 버전 대상 필수단어(쉼표 구분) 금지단어(쉼표 구분 또는 -) deadline_ms max_age_ms
replay-v1 RUN COMPLETED,SAVED FAILED,ERROR 100 20
# 관측완료_ms frame_id 수집시작_ms 너비 높이 상대TSV경로
0 1 0 800 600 ready.tsv
10 2 10 800 600 completed.tsv
20 3 20 800 600 compound.tsv
```

- 관측 시각은 작업 시작 이후의 가상 시간이다. 실제 대기 없이 결정적으로 재생한다.
- frame_id와 수집 시각은 모두 증가해야 한다. 수집 시각은 관측 완료보다 미래일 수 없고 max_age 이내여야 한다.
- 입력 시도가 끝나기 전에 수집된 프레임은 결과 검증에 사용하지 않는다.
- 경로는 재생 파일이 있는 디렉터리 내부의 상대 경로다. 공백 포함 파일명, `..`, 절대 경로는 지원하지 않는다. 경로가 루트 밖으로 해석되는 심볼릭 링크도 거부한다.
- 프레임이 소진되면 `observation_error`다. 이를 자동 timeout이나 성공으로 바꾸지 않는다. deadline에 도달한 관측은 `timeout`이다.
- OCR 단어는 대소문자를 구분해 정확히 일치해야 한다. 클릭 대상은 전체 일치 박스가 정확히 하나이고 confidence >=70이어야 한다. 두 번째 저신뢰도 후보도 모호함으로 거부한다.
- 사후조건은 required 전부 confidence >=70, forbidden 전부 부재다. 낮은 신뢰도의 forbidden도 성공을 막는다. 입력 전 낮은 신뢰도 forbidden은 `uncertain_state`로 거부한다.
- 입력 전 required 중 하나라도 보이면 보수적으로 `already_present`로 거부한다. 낮은 신뢰도나 복합 조건의 일부만 보이는 경우도 재실행하지 않는다.
- JSON에는 OCR 원문·파일명·외부 프로세스 오류 원문을 출력하지 않는다. API의 Report.detail은 호출자가 보유할 수 있으므로 추적 로그에 그대로 저장하지 않는다.

## 제공 사례

| 파일 (`tests/fixtures/`) | 결과 | 입력 시도 |
|---|---|---:|
| success.replay | validated | 1 |
| compound.replay | validated, 복합 조건 전체 확인 | 1 |
| failure.replay | failure_observed | 1 |
| duplicate.replay | ambiguous_target | 0 |
| low-confidence.replay | low_confidence_target | 0 |
| missing-target.replay | missing_target | 0 |
| already-completed.replay | already_present | 0 |
| uncertain.replay | uncertain_state | 0 |
| stale.replay | stale_frame, 중복 ID | 1 |
| old-capture.replay | stale_frame, 오래된 수집 시각 | 1 |
| timeout.replay | timeout | 1 |
| ocr-error.replay | observation_error, 잘못된 TSV | 1 |
| exhausted.replay | observation_error, 프레임 소진 | 1 |

## 실제 실행과 공유하는 부분

`src/execution.rs::execute_observed`를 재생 및 실제 `click/open-url/local-app` CLI가 함께 사용한다. 실제 CLI는 매 관측마다 해상도, 증가하는 수집 호출 ID, 수집 시작 시각과 OCR 박스를 넘긴다. 기본 max_age는 캡처/OCR 처리 시간을 포함한 30초다. 이 시각과 ID는 USB 장치 자체의 촬영 시각/sequence가 아니므로 장치 내부의 캐시 프레임까지 증명하지는 않는다.

기존 `validation::execute_and_validate/perform_and_validate`는 같은 검증기의 호환 어댑터다. 메타데이터가 없는 호출자에게 합성 ID와 관측 시작 시각을 부여한다. 실제 해상도와 수집 메타데이터가 필요한 신규 코드는 `execute_observed`를 사용한다.

실제 CLI의 기본 금지 단어는 FAILED다. FAILED 자체를 기대하는 명시적 테스트에서는 기본 금지 목록에서 제외한다. 복합 조건·별도 금지 단어는 현재 Policy API와 재생 파일에서 설정하며 live CLI의 인자는 기존 단일 단어 형식을 유지한다.

HID 명령은 임시 디렉터리의 `windows-control-hid.lock`을 원자적으로 획득한다. 정상 종료 시 해제하며, 동시 입력 명령은 대기/재실행 없이 거부한다. 비정상 종료로 잠금 파일이 남으면 실행 중인 제어 프로세스와 입력 결과를 확인한 뒤 수동 해제한다. 이 잠금은 이 CLI끼리 협력하는 장치이며 이전 바이너리나 별도 HID 프로그램까지 막지는 않는다.

## 검증 경계

이번 개발은 로컬 합성 TSV 재생 및 가상 입력 검증이다. Pi 소스 배포, 실제 HID 재실행, 화면 재캡처는 이번 변경에서 수행하지 않았다. 실제 장치에서 시험할 때는 AGENTS.md의 소스 실행 → 재캡처 → 상태 정의 사이클을 그대로 적용한다. 실제 OCR 인식률, USB 안정성, Windows 초점과 좌표 정확도, Pi 처리 지연은 별도 실측이 필요하다.

저장 JPEG의 재OCR, 브라우저 가상 입력 통합, 자연어 계획기·Laya·이벤트 선점은 이번 구현에 포함하지 않는다.
