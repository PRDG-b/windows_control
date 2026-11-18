# Execute and Validate 사용 및 검증 기록

## 제공 기능
- 기본 테스트 경로(2026-11-18 사용자 지시): 작업 PC `192.168.0.20`에서 `python test-app/server.py --host 192.168.0.20 --port 8765`로 제공하고 제어 대상 PC는 `http://192.168.0.20:8765/`를 연다. Pi는 캡처·OCR·HID만 수행한다. 긴 data URL은 기본 경로로 사용하지 않는다. 아래 `local-app` 설명은 보조 실험 경로다.
- `focus-address` → 재캡처로 Edge 주소창 초점 확인 → `local-app`: 내장 테스트 HTML을 data URL로 입력하여 **제어 대상 PC 브라우저에서만 실행**한다. Pi나 현재 작업 Windows PC에 서버를 실행하지 않는다. data URL은 브라우저 세션 안의 로컬 실행이며 디스크에 HTML을 설치하는 기능은 아니다.
- `observe <snapshot.jpg>`: OCR과 동일한 프레임을 JPEG로 정규화해 저장한다. 이미지를 확인해 상태를 정의하고, 민감한 화면은 추적 문서에 복사하지 않는다.
- 기본 실행과 `--help`: 도움말만 표시. HID 장치에 접근하지 않는다.
- `observe`: HDMI JPEG 수집 → 디코딩/해상도 검사 → Tesseract TSV → 박스 파싱. 텍스트 자체 대신 개수·해상도·소요 시간만 출력한다.
- `click <대상단어> <기대단어> [제한초]`: 현재 OCR에서 대상이 confidence 70 이상인 정확 일치 단일 박스일 때 한 번 클릭한다. 새 관측에 기대 단어가 confidence 70 이상으로 나타나면 성공한다.
- 기대 단어가 이전 화면에 이미 인식되면 confidence와 무관하게 실행을 거부한다. 중복·저신뢰·없는 대상, 관측 오류, 입력 오류, timeout은 실패다. 클릭 자동 재실행은 없다.
- `examples/wake.rs`: 명시적으로 실행하는 마우스 이동 기반 깨우기 시도. PC 복귀 성공을 의미하지 않는다.

## 실행
Pi에서:

```sh
cargo test --offline
cargo run --offline -- observe
cargo run --offline -- click RUN COMPLETED 20
cargo run --offline --example wake
```

캡처는 `/dev/video0`에서 탐지한 해상도를 유지하고 MJPEG로 수집한다. fps는 기존 장치 설정을 유지한다. 해상도 탐지 실패 시 임의 해상도로 클릭하지 않는다. 캡처 프로세스 10초, OCR 15초 제한이다. CLI 1..300초의 작업 deadline은 관측 전후에서 검사하므로 진행 중 프로세스 시간만큼 초과할 수 있다. HID 장치 write 자체에는 현재 hard deadline이 없으므로 운영 watchdog은 후속 과제다.

## 테스트 페이지
현재 운영 방식은 위 `local-app`의 서버 없는 실행이다. 아래 서버 방식은 이전 실험 기록이며 Pi에서 다시 실행하지 않는다.
제어 대상 Windows 브라우저에서 `test-app/index.html`을 열 수 있다. 독립 채점을 함께 사용하려면 `python3 test-app/server.py --host 192.168.0.215 --port 8765`로 서버를 실행하고 `http://192.168.0.215:8765/`를 연다. Python 표준 라이브러리만 사용하며 지정한 테스트 페이지만 제공한다.

| 시나리오 | 실행 및 기대 결과 |
|---|---|
| success | Reset 후 `click RUN COMPLETED 20`: 지연 후 성공 |
| failure | Reset 후 동일 명령: FAILED를 성공으로 처리하지 않고 timeout |
| duplicate | Reset 후 동일 명령: 실행 전 중복 대상으로 거부, clicks=0 |
| popup | `click RUN CONFIRM 20` 후 `click CONFIRM COMPLETED 20`: 단계별 성공 |
| already completed | 성공 후 `click RUN COMPLETED 20`: 재실행 거부 |

독립 채점은 브라우저 개발자 도구의 `window.testState` 또는 서버 `/oracle`의 `phase`, `clicks`, `scenario`를 확인한다. 페이지가 상태를 `/events`에 보고하며 이 값은 실행·검증 루프의 입력으로 사용하지 않는다. 서버 상태는 마지막에 보고한 페이지 하나에 해당하므로 테스트 탭을 하나만 연다. 실제 HID가 테스트 버튼에만 도달하는지 확인한다. 이전 상태의 지연 타이머는 Reset에서 취소된다.

대문자 SUCCESS/FAILURE/DUPLICATE/POPUP 버튼은 테스트 시나리오 선택용이다. 예: `click FAILURE FAILUREMODE 30`으로 전환 후 `click RUN COMPLETED 20`의 실패를 확인한다.

서버는 테스트 중에만 실행한다. 이번 임시 서버의 PID는 Pi `/tmp/windows-control-ev.Z69aL5/test-server.pid`, 로그는 같은 디렉터리 `test-server.log`에 있다. 테스트 종료 후 해당 PID의 Python 테스트 서버인지 확인하고 종료한다.

## 2026-11-13 검증
- 로컬 및 Pi에서 Rust 단위 테스트와 TSV 재생 통합 테스트 통과. 최신 개수는 계획 문서 진행 기록 참조.
- Pi 임시 사본: `/tmp/windows-control-ev.Z69aL5`. 원본 `~/windows_control`에는 덮어쓰지 않았다.
- 초기 캡처: status=124, bytes=0. 커널 로그 `uvcvideo ... Non-zero status (-71)` 반복.
- PC 전원 복귀만으로는 복구되지 않았다. 사용자가 USB를 재연결한 뒤 640×480 30fps, 1920×1080 30fps에서 프레임 수신 성공.
- 1080p 30fps의 `observe`: `words=48`, `elapsed_ms=5336`. 캡처·OCR 통합 시간이며 각 단계 지연이나 p95가 아니다.
- 첫 단일 프레임 진단 이미지에는 하단 녹색 손상이 관찰됐다. 이후 프로그램과 같은 skip=4 조건의 프레임은 전체 화면이 정상임을 시각 확인했다. 디코딩 성공만으로 전체 화면 품질을 보장할 수는 없다.
- 캡처 당시 Windows 잠금 화면이었다. 잠금 해제는 사용자에게 요청했다. 테스트 페이지의 실제 HID 성공·실패·중복 시나리오 검증은 준비 상태에 따라 아래에 추가 기록한다.
- 별도 코드 검토에서 사전 기대 문구의 저신뢰 인식이 거짓 성공을 만들 수 있는 점을 발견해 실패 테스트로 재현하고 보수적으로 실행 거부하도록 수정했다. 해상도 탐지 프로세스에도 3초 제한과 종료 상태 검사를 추가했다.
- 후속 교차 비교: 같은 USB 연결에서 60→30→60→30fps 모두 status=0, 약 331KB 수신. 60fps가 원인이라는 초기 추측은 철회한다. USB 재연결과 포맷 재협상 후 복구됐으나 정확한 원인은 미확정이다. fps 강제 코드는 제거하고 조사 전 설정 60fps로 복원한다.
- 탐색기 화면 OCR의 실제 시간은 5.828초였다. 임의로 둔 5초 제한이 관측 실패를 일으킨 회귀임을 확인하여 15초로 늘렸다. 이 값도 모든 화면의 처리 시간을 보장하지 않는다.
- 최신 Rust 테스트: 단위 11개 + TSV 재생 통합 1개, 로컬과 Pi 모두 통과. Python 독립 채점 서버 통합 테스트 1개도 양쪽에서 통과했다.

## 현재 제한
2026-11-16: `cargo run --offline -- local-app` 및 재캡처로 제어 대상 Edge에서 서버 없는 웹앱 실행을 확인했다. 화면은 SUCCESSMODE/READY다. 후속 RUN 클릭은 사전조건에서 거부되어 입력하지 않았으며 반응 검증은 별도 미완료다. 세부 명령·관측은 계획 문서 진행 기록에 있다. `save-page`는 화면 전환 검증을 갖추기 전까지 비활성화했다.
서버 없는 실행에서는 `/oracle` 채점 서버를 사용하지 않는다. 실행 여부는 새 프레임의 화면/OCR로 검증하며 독립 채점 서버 검증과 구분한다.
단일 단어 정확 일치 MVP다. 다단어 버튼, 의미적 목표, ROI·아이콘 검증, OCR 누락으로 인한 잘못된 부재 판단은 해결하지 않았다. capture 시각·프레임 freshness 메타데이터와 프로세스 전체 watchdog, 장치 간 입력 경쟁 방지는 후속 단계다. 현재 성공 조건은 새 관측에서 문구가 보인다는 뜻이며 행동과 결과의 인과관계 전체를 증명하지 않는다. Laya·자연어 계획기는 아직 연결하지 않았다.

## 동일 원본 바이트의 파일/stdin 비교 (2026-11-16)
`cargo run --offline -- compare-ocr <새 디렉터리>`를 추가했다. 기존 캡처 코드로 한 프레임을 받아 **재인코딩 없이** raw.jpg에 저장하고 메모리와 파일 바이트의 완전 일치를 검사한다. 파일 경로 입력과 기존 Rust `stdin.write_all()` 경로에 같은 바이트를 사용한다. 두 경로의 언어/PSM/TSV/15초 제한은 동일하다. HID 입력은 없다. 성공 결과의 전체 TSV는 로그에 남기지 않고 출력 일치 여부와 단어/테스트 RUN 후보 수만 출력한다.

Pi 검증 디렉터리: `/tmp/windows-control-ev.Z69aL5`.

| 원본 파일 | 바이트 수 | 파일 입력 | stdin 입력 | 비교 |
|---|---:|---|---|---|
| ocr-compare-20261116-01/raw.jpg | 286235 | 성공 | 성공 | OCR 출력 바이트 일치, 단어 111 |
| ocr-compare-20261116-02/raw.jpg | 286235 | 실패(1) | 실패(1) | 모두 premature end of data segment / scanline 1070 read error |
| ocr-compare-20261116-03/raw.jpg | 286270 | 성공 | 성공 | OCR 출력 바이트 일치, 단어 110 |

세 경우 모두 저장 전후 원본 바이트가 완전히 같았다. SHA-256:

```text
01 265c5937fa4d8cca860294a8702bf1fc26eb8b5c60ab3195239ac1dc1dcab961
02 543f4e4918bb10c630f41b05e71c7142a9a8178a82fa0d2536b644ab478e6158
03 4be0fca6c63d4df5dbfc69ffefb000fd76e0c214115a0f62801d001a2a38f076
```

결론: 재현된 오류는 파일/stdin 전달 방식 차이가 아니다. 원본 프레임별 JPEG 데이터와 Tesseract/Leptonica 해독 경로의 문제로 범위를 좁혔다. 캡처 보드·USB·드라이버 중 어느 계층에서 문제가 생기는지는 이 비교만으로 확정할 수 없다. 정상 해독된 두 프레임에서도 confidence >= 70의 정확 일치 RUN 후보는 0개였다. 따라서 JPEG 해독 실패와 RUN 대상 인식 실패는 별도로 조사해야 한다.

## 원본 JPEG와 디코더 검증 (2026-11-16)
보존된 01/02/03의 SHA-256이 이전 기록과 같음을 먼저 확인했다. 새 캡처·HID 입력 없이 원본을 검사했다. `cargo run --offline -- verify-jpeg <보존된 raw.jpg>`는 마커와 Rust 디코딩 결과를 출력하고, 디코딩된 RGB를 재압축 없는 P6 PPM으로 저장해 OCR 대조군으로 사용한다. 원본은 변경하지 않는다.

| 항목 | 01 | 02 (실패 원본) | 03 |
|---|---|---|---|
| SOI FF D8 | 있음 | 있음 | 있음 |
| EOI FF D9 | 마지막 2바이트 | **파일 전체에 없음** | 마지막 2바이트 |
| 마지막 2바이트 | FF D9 | **FF E3** | FF D9 |
| 원본 Tesseract 파일/stdin | 양쪽 성공 | 양쪽 JPEG 읽기 실패 | 양쪽 성공 |
| Rust image/zune-jpeg | 1920×1080 디코딩 | 1920×1080 디코딩 | 1920×1080 디코딩 |
| Pillow 11.1.0, LOAD_TRUNCATED_IMAGES=False | 디코딩 | 디코딩 | 디코딩 |
| FFmpeg -xerror -err_detect explode | 오류 메시지 없음, rc=0 | APP fields Invalid data 메시지, **rc=0** | 오류 메시지 없음, rc=0 |
| Rust RGB → PPM → Tesseract | 성공, 112단어 | 성공, 111단어 | 성공, 111단어 |

02번의 마지막 16바이트는 `96ff004ffd9aa443d0a851f20f97ffe3`이다. 완결된 JPEG 종료 마커가 없는 **불완전한 원본 JPEG**라는 점을 직접 확인했다. FFmpeg/Pillow/Rust가 이미지를 반환하거나 종료 코드 0을 주는 사실만으로 무결성을 판단할 수 없다.

Leptonica 1.84.1의 [jpegio.c](https://github.com/DanBloomberg/leptonica/blob/1.84.1/src/jpegio.c)는 기본적으로 JPEG 경고 발생 시 읽기를 중단한다. `abort_on_warning`이 참인 상태에서 경고 수가 증가하면 pix를 폐기하며 scanline 오류를 반환한다. 현재 `scanline 1070; nwarn=1`과 일치한다. FFmpeg의 [오류 검출 옵션](https://www.ffmpeg.org/ffmpeg-all.html)도 참고했지만, 이 파일에서는 엄격 옵션하에서도 메시지와 종료 코드가 다르므로 메시지까지 검사했다.

결론: 실패 원본의 JPEG 구조 이상과 디코더별 허용 차이가 검증됐다. stdin 전송의 문제가 아니며, Tesseract가 정상 JPEG를 이유 없이 거부했다는 증거도 없다. RGB 대조군의 OCR 성공은 JPEG 해독 경로를 우회하면 OCR이 가능함을 뜻할 뿐, 손상된 픽셀이 정확히 복구됐다는 보장은 아니다. 자동화에서는 이런 프레임을 정상으로 간주하지 말고 폐기 후 새 관측을 얻는 것이 적절하다. EOI 검사는 필요한 최소 조건이며 전체 JPEG 무결성을 충분히 증명하지 않는다. 손상 발생 위치(장치/USB/드라이버/v4l2 수집)는 후속 검증 대상이다.

## 손상 발생 계층 조사 (2026-11-16)
Pi 아티팩트: `/tmp/windows-control-ev.Z69aL5/capture-layer-20261116-01/`. HID 입력·웹 서버 실행은 없었다. 기본 캡처 설정 1920×1080 MJPG 30fps를 유지했다. 일부 실험에서 동일 포맷을 재설정했으며 다른 해상도나 fps로 바꾸지 않았다.

| 비교 경로 | 검사 | 결과 |
|---|---|---|
| Rust 없이 v4l2-ctl 직접 파일 | 단발 8회, SOI/EOI | 1개 EOI 없음 |
| Rust 없이 v4l2-ctl stdout 리다이렉트 | 단발 8회, SOI/EOI | 8개 모두 있음 |
| FFmpeg V4L2 `-c:v copy` | 16프레임, SOI/EOI | 16개 모두 있음. 재압축 아님 |
| v4l2-ctl `--stream-to-hdr` | skip 4 후 연속 90개, 경계 헤더로 분리 | 앞의 2개(seq 4,5) EOI 없음, 나머지 88개 있음 |
| 독립 DQBUF mmap 검사 | 90 + 8×30 + 동일 포맷 재설정 6×30 | 510개 모두 SOI/EOI 있음 |
| 독립 mmap 검사 + strace ioctl 추적 | 추가 30개 | 첫 버퍼 SOI 없음, 드라이버 ERROR 플래그 있음. 나머지 29개 SOI/EOI 있음 |

표의 정상 표시는 마커 검사 통과라는 뜻이다. 모든 프레임의 JPEG 전체 비트스트림 무결성이나 OCR 정확도를 증명한 것은 아니다.

### 파일 저장 이전의 직접 메모리 관측
`diagnostics/v4l2_buffer_probe.c`는 표준 커널 헤더와 ioctl/mmap만 사용한다. VIDIOC_DQBUF 직후, 파일 쓰기와 QBUF 전에 메모리의 시작/끝 바이트를 검사한다. 포맷 변경·HID·OCR은 하지 않는다. Pi에서 `cc -O2 -Wall -Wextra`로 경고 없이 빌드해 실행했다.

추적 실행의 첫 반환 버퍼:

```text
sequence=0 timestamp=241578.202562 bytesused=534478
flags=0x12041 error=1 soi=0 eoi=1
VIDIOC_DQBUF flags include V4L2_BUF_FLAG_ERROR
```

이 시점에 메모리는 이미 JPEG 시작 마커가 없었다. 보존 파일 길이는 bytesused와 일치하며 SHA-256은 `8d2b9bf83eb156e4c65c6cca08e41bb3adec7b1a5a193dd54d67ee88945a7945`다. 따라서 사용자 파일 쓰기 전부터 손상된 버퍼가 반환되는 사례가 확인됐다.

**이 사례와 오류 플래그 없는 EOI 누락 사례는 구분한다.** v4l2-ctl 직접 저장의 손상 파일은 seq=4, bytesused=286270, 실제 파일도 286270바이트였고 오류 플래그는 표시되지 않았다. 독립 mmap 검사에서 같은 종류의 무플래그 EOI 누락을 포착한 것은 아니다.

### 계층별 판단과 한계
- Rust Vec/Command::output/OCR 전달을 통하지 않아도 손상 재현: Rust 코드가 손상의 유일한 원인은 아니다.
- 직접 저장 파일 길이와 bytesused 일치, short-write 오류 없음: 단순 파일 쓰기 중 길이 잘림을 지지하지 않는다.
- [v4l-utils 1.30.1 소스](https://github.com/gjasny/v4l-utils/blob/v4l-utils-1.30.1/utils/v4l2-ctl/v4l2-ctl-streaming.cpp)의 기본 경로는 bytesused만큼 fwrite한 뒤 QBUF한다. 소스만으로 해당 도구의 모든 버그를 배제할 수는 없다.
- DQBUF 메모리에 이미 손상이 있고 드라이버가 ERROR를 보고한 사례: 장치/USB/UVC 드라이버 구간에서도 문제가 발생한다. 단, 무플래그 EOI 누락의 원인까지 동일하다고 확정하지 않는다.
- 손상은 관측된 연속 스트림의 시작 구간에 몰렸고 이후 안정 구간에는 재현되지 않았다. 초기 프레임 경계/기동 과정 관련 가설을 지지하지만 아직 원인 확정은 아니다.
- 실험 중 새 uvcvideo/USB 커널 로그는 없었다. 이전 -71 로그는 약 11,000초 시점이고 이번 실험은 약 241,000초 시점이므로 현재 손상의 직접 근거로 사용하지 않는다.
- 장치는 USB2 480M 경로의 bulk 영상 endpoint 0x85를 사용한다. 연결 속도만으로 손상 원인을 단정하지 않는다.
- 장치·USB·UVC 드라이버 사이의 최종 구분에는 영상 endpoint의 USB 원본과 반환 버퍼 대조가 필요하다. [usbmon 공식 문서](https://docs.kernel.org/usb/usbmon.html)상 이 도구도 HCD에 전달된 요청을 관측하므로 물리 버스 전체의 절대적인 정상 증명은 아니다.
- 현재 `/dev/usbmon*`가 없고 `sudo -n`은 비밀번호를 요구했다. 권한을 임의 변경하거나 비밀번호를 기록하지 않았다. 관리자 인증이 필요한 남은 관측은 사용자에게 요청했다.

### 관리자 USB 영상 추적 준비
`diagnostics/usb_video_trace.py`를 Pi 검증 디렉터리에 배포했다. 필터 단위 테스트와 Python 문법 검사를 통과했다. 실제 usbmon ioctl 실행은 아직 관리자 권한으로 검증하지 않았다. Linux 6.18 공식 `mon_bin.c`와 usbmon ABI의 GETX/통계/링 크기 정의를 대조했다.

사용자가 Pi에서 실행할 명령:

```sh
sudo modprobe usbmon
sudo python3 /tmp/windows-control-ev.Z69aL5/usb_video_trace.py
```

약 8초 동안 /dev/video0의 장치 번호를 자동 확인한 뒤 알려진 캡처 보드(1de1:f115)의 bulk IN 0x85 완료 이벤트만 저장한다. 같은 구간의 V4L2 60프레임을 재압축 없이 버퍼 경계와 함께 수집한다. HID 입력이나 웹 서버는 실행하지 않는다. 아티팩트는 `/tmp/windows-control-usbtrace-*`에 만들고 실행한 sudo 사용자의 소유로 반환한다. `monitor_dropped`, `clipped_packets`, `forced_stop` 값이 있으면 완전한 관측으로 간주하지 않는다.

## USB 원본과 V4L2의 최종 대조 (사용자 수집 결과)
아티팩트: `/tmp/windows-control-usbtrace-kgcrnw6q`.
수집: 영상 완료 URB 1193개, 18,892,868바이트. monitor_dropped=0, clipped_packets=0, monitor_queued=0, forced_stop=false, capture_exit=0.

`diagnostics/analyze_usb_video.py`를 추가해 오프라인으로 분석했다. 이 데이터의 bulk URB 최대 길이는 16384바이트이며 short transfer에서 UVC payload가 끝났다. 재구성된 payload 66개 모두 유효한 길이의 UVC 헤더와 EOF 비트가 있었고, 하나의 payload가 하나의 JPEG 프레임이었다. 분석기는 잘린 레코드, 실제/캡처 길이 불일치, 데이터가 있는 오류 URB, 미완성 payload를 거부한다. 합성 데이터 테스트 2개와 실제 데이터 분석을 통과했다.

| 증거 | 결과 |
|---|---|
| USB에서 재구성한 프레임 | 66개 |
| USB 원본의 EOI 없는 프레임 | 0~7번, 첫 8개 |
| 해당 프레임의 UVC 헤더 | EOF=1, ERR=0 |
| 첫 EOI 정상 USB 프레임 시작 | 추적 시작 후 1.018776초 |
| V4L2 저장 프레임 | 60개, 첫 2개 EOI 없음 |
| 전체 바이트 대조 | **V4L2 0~59번 = USB 6~65번, 60개 모두 완전 일치** |
| 손상된 V4L2 첫 2개의 SHA-256 | 둘 다 d1d3f4f742692aca68d4db52fc169524cd7e7990e4fe8dedb501401e0dfb51fa |
| 영상 데이터를 운반한 URB | 모두 status=0 |
| 마지막 영상 데이터 시각 | 시작 후 2.927102초 |
| status=-2 URB 5개 | 2.928018~2.929041초, 모두 데이터 0바이트 |

원본 USB payload에 이미 JPEG EOI가 없었고, 드라이버 이후 V4L2 파일과 동일했다. 따라서 **이번 손상 프레임은 UVC 조립·v4l2-ctl 파일 쓰기·Rust 복사·OCR 전달 과정에서 새로 손상된 것이 아니다.** 장치가 프레임 완료(EOF=1)·오류 없음(ERR=0)으로 표시해 전달한 영상 자체가 불완전했다.

영상 데이터의 URB는 모두 정상 완료였고 반복된 손상 JPEG의 해시도 같았다. 관측 결과는 **캡처 보드의 스트리밍 기동/펌웨어가 미완성 또는 오래된 JPEG를 완료 프레임으로 내보내는 문제**를 가장 강하게 지지한다. 이 부분은 원인에 대한 추론이다. usbmon은 호스트 컨트롤러 계층의 관측이므로 보드 펌웨어의 어느 코드가 잘못됐는지, 물리 USB/HCD의 모든 가능성까지 확정하려면 다른 호스트·장치 또는 물리 분석기 교차 검증이 필요하다.

`-2`는 데이터 전달 후 종료 구간의 빈 URB에서만 발생했다. [Linux USB 오류 코드 문서](https://docs.kernel.org/driver-api/usb/error-codes.html)의 완료 상태 -ENOENT(unlink/cancel)와 일치하며 이번 JPEG 손상의 전송 오류 근거로 보지 않는다.

해당 분석 시점의 `--stream-skip=4`는 충분한 준비 조건이 아니었다. 첫 프레임들이 약 7ms 간격으로 몰려 도착하고 약 1초 뒤 정상 스트림으로 전환됐다. 고정 프레임 수나 고정 대기만 믿지 말고 스트림을 유지하며 불완전 프레임을 폐기하고 유효 프레임을 새로 관측하는 방향이 적절하다. 해당 분석 단계에서는 실제 캡처 실행 로직이나 HID 동작을 변경하지 않았다. 이후 사용자가 요청한 폐기 수 증가 실험은 다음 절에 기록했다.

## 초기 폐기 프레임 4 → 30 수정 및 검증 (2026-11-16)
사용자 요청에 따라 `src/capture.rs`의 실제 `capture_raw()` 실행 인자를 `--stream-skip=4`에서 `--stream-skip=30`으로 변경했다. 다른 조건은 그대로 유지했다: mmap 버퍼 4개, 수집 1프레임, MJPEG, 탐지 해상도, 장치 fps 유지, 캡처 외부 프로세스 제한 3초. 이번 장치 설정은 1920×1080 / 30fps였다. 주석 처리된 이전 파일 저장 예제의 숫자는 실행 경로가 아니다.

로컬 및 Pi 검증 사본의 Rust 단위 16개 + 재생 통합 1개 통과. 로컬 fmt/check/diff 검사도 통과했다.

Pi `/tmp/windows-control-ev.Z69aL5`에서 매번 스트림을 새로 여는 명령을 10회 실행했다.

```sh
cargo run --offline -- compare-ocr skip30-20261116-01/case-01
# case-02 ... case-10 각각 독립 실행
```

| 검증 항목 | 결과 |
|---|---|
| 원본 SOI/EOI | 10/10 정상 |
| 원본 파일 입력 OCR | 10/10 성공 |
| 동일 원본 stdin 입력 OCR | 10/10 성공 |
| 파일/stdin OCR 출력 완전 일치 | 10/10 |
| JPEG 해독 오류·캡처 timeout | 0/10 |
| 전체 명령 시간 | 약 8.37~11.79초. 캡처+OCR 두 번이며 일부 빌드 시간 포함 |

원본·로그·집계: Pi `skip30-20261116-01/case-*/raw.jpg`, `case-*.log`, `summary.json`. 재인코딩한 파일이 아닌 원본으로 평가했다.

최종 `cargo run --offline -- observe skip30-final.jpg RUN`도 성공했다. 2026-11-16 09:26:22 +09:00 재캡처 이미지에서 웹앱 SUCCESSMODE/READY와 RUN 버튼을 확인했다. 관측은 약 8.412초, OCR 110단어, RUN exact=0 / qualified=0이었다. 입력은 실행하지 않았다.

결론: **폐기 수 30장 적용 후 이번 10회 반복과 최종 관측에서는 JPEG 해독 오류가 재현되지 않았다.** 모든 기동 조건의 영구 해결이나 보드 자체 수리를 입증한 것은 아니다. 별개의 RUN 텍스트 인식 누락은 남아 있으며 클릭 성공으로 기록하지 않는다.

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

### 2026-11-18: Rust 화면 전이 탐색기
- docs/exploration.md 및 구현 계획 2026-11-18-exploration.md 참조. 명시적 어댑터의 허용 후보에 대해 BFS 전이 탐색, 현재 프레임에서 경로 재해석, 복원 확인, 부분 그래프/미탐색 목록 저장 구현.
- explore-demo와 explore-test-web CLI 추가. 기본 실행은 도움말이며 실제 입력은 명시적 테스트 웹 명령으로만 수행한다. 테스트 웹 어댑터는 단일 Ctrl+R로 복원한 뒤 새 관측에서 root를 검증한다.
- 로컬/Pi Rust 38개 테스트 통과. fmt/check/offline test/diff 검사 통과. 독립 리뷰에서 관측 예산/timeout과 복원 초점 문제를 수정하고 최종 중요 문제 없음 확인.
- 로컬/Pi 합성 데모: 노드3/간선3/probe3/입력7/복원확인/pending0. 가상 UI 결과이며 실제 HID 성공은 아니다.
- Pi 최신 소스 /tmp/windows-control-explore.SnsW8o에서 explore-test-web live-flow-01 1 실행: No Signal 3프레임/unknown_state/입력0. 직후 explore-final.jpg 재캡처·OCR 및 실제 이미지 확인에서도 앱 화면은 없었음(검은 화면, 직전 No Signal). 실제 앱의 전이/복원 성공은 미수행이다.
