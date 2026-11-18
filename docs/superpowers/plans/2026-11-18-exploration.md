# 화면 전이 탐색기 구현 계획

## 설계
사용자가 승인한 flow chart의 관측 → 후보 선택 → 단일 입력 → 새 관측 → 그래프 기록 → 복원 확인을 Rust로 구현한다.
상태의 의미와 허용 후보는 어댑터가 제공하고 공통 탐색기는 BFS frontier, 현재 프레임 검사, 예산, 이력과 복원을 관리한다. 영상 유사도만으로 상태를 합치지 않는다.
기존 Observation 및 수집/OCR/HID 경로와 std를 재사용한다. 실제 UI는 기존 테스트 웹앱 전용 어댑터로 한정한다. 임의 UI 검출 모델·무차별 격자 클릭·task 범용 의미 추론은 포함하지 않는다.

## 완료 조건
- 새 상태·행동 frontier와 관측된 전이 그래프. 동일 관측 상태 간선도 기능 없음으로 단정하지 않고 기록.
- 도달 경로와 복원 모두 매 입력 후 새 관측으로 확인. 이전 좌표는 재사용하지 않고 현재 후보 ID로 해석.
- 허용/금지/불명확 후보 구분, 단일 입력 후 결과 불명확 시 읽기 전용 추가 관측만 제한적으로 수행. 실패한 입력 재실행 금지.
- 시간/입력/탐색/상태/관측 예산. 부분 그래프와 미완료 이유, 복원 확인 여부 저장.
- `explore-demo <new-directory> [scenario]`: 하드웨어 없는 시뮬레이션 및 DOT/TSV/Mermaid 출력.
- `explore-test-web <new-directory> <max-probes>`: 명시적 실제 입력, 기존 웹앱 한정. 사전 success_ready 확인 후 시작하고 복원은 Ctrl+R 한 번과 재관측으로 한다. 기본 실행은 도움말.
- 오프라인 회귀 및 CLI 시험, 로컬/Pi 빌드, 실제 새 캡처와 상태 기록. 실제 UI 시험이 막히면 미수행으로 기록.

## 단계
- [x] 실패 테스트 작성·실패 확인: 그래프/경로/복원/불명확/입력 오류/예산/오래된 좌표.
- [x] 공통 프레임 검사 추출과 탐색 코어 구현, 테스트 통과.
- [x] 재생 데모·파일 출력·CLI 및 기존 테스트 웹 어댑터 구현.
- [x] 테스트/실행/재캡처, 문서 갱신과 독립 리뷰. 실제 전이/복원 성공 시험은 No Signal로 미수행이며 아래에서 구분한다.

## 실행 결정
현재 작업 트리의 기존 변경을 보존하며 직접 구현한다. 원격은 새 임시 배포를 사용한다. 커밋/푸시/모델 설치는 수행하지 않는다. complete는 허용된 후보의 탐색 종료이며 앱의 모든 상태를 발견했다는 뜻이 아니다.

## 검증 기록
- 로컬/Pi Rust 테스트 38개 통과. 기존 실행·검증 회귀 포함. 로컬 fmt/check/offline test/diff 검사 통과.
- 독립 리뷰에서 입력 후 관측 예산 소진/timeout으로 재캡처를 건너뛰는 문제를 발견했다. 입력 전 관측 1회분 확보 및 timeout 후 진단 재관측으로 수정하고 실패 재현→통과를 확인했다.
- 실제 복원에서 Win+R/입력 초점 미확인 체인 문제를 검토 후 제거했다. 이미 확인한 폐기 가능한 테스트 페이지의 Ctrl+R 한 번과 root 재관측으로 변경했다. 낮은 신뢰도의 CONFIRM이 밑의 RUN을 노출하지 않도록 회귀 수정했다. 최종 독립 리뷰에서 남은 중요 문제 없음.
- 로컬 `cargo run --offline -- explore-demo target/explore-flow-demo-v1`: simulation/complete, 노드3/간선3/probe3/입력7/관측8/restored=true/pending0, 가상15ms.
- Pi 임시 소스 `/tmp/windows-control-explore.SnsW8o`; CARGO_TARGET_DIR은 기존 실험 target을 캐시로 공유하되 매번 이 디렉터리 소스를 cargo run으로 빌드했다. 원본 저장소는 수정하지 않았다.
- Pi `cargo run --offline -- explore-demo demo-flow-01`도 동일 그래프 결과 확인. 실제 HID 입력은 없는 합성 실행이다.
- Pi `cargo run --offline -- explore-test-web live-flow-01 1`: 3회 새 캡처 후 unknown_state/예상 exit1, input_attempts=0/probes=0, elapsed=21157ms. 모든 캡처는 No Signal이어서 화면 전제 검사에서 입력을 차단했다. 실제 버튼 전이/복원 성공으로 기록하지 않는다.
- 2026-11-18 17:19 KST 직후 `cargo run --offline -- observe explore-final.jpg`: 1920×1080/OCR 3단어/7160ms. 최종 이미지는 검은 화면으로 앱 상태를 확인할 수 없었고, 직전 캡처에서는 No Signal을 시각 확인했다. 수신 JPEG/OCR 경로는 동작하지만 제어 PC의 실제 앱 화면은 관측하지 못했다. 물리 신호 복구 후 live 검증은 별도 필요하다.
