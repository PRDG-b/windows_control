use std::fs;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};
use image::RgbImage;

pub struct ScreenCapture {
    device_path: String,
    width: u32,
    height: u32,
}

impl ScreenCapture {

    // 캡처하고 있는 화면의 해상도 감지
    pub fn detect_resolution(device_path: &str) -> Result<(u16, u16), String>{
        let output = Command::new("v4l2-ctl")
            .arg(format!("--device={}", device_path))
            .arg("--get-fmt-video")
            .output()
            .map_err(|e| format!("Fail to execute v412-ctl: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        // 출력에서 Width/Height 줄을 찾아서 파싱
        for line in stdout.lines() {
            if line.contains("Width/Height") {
                if let Some( dims ) = line.split(":").nth(1) {
                    let parts: Vec<&str> = dims.trim().split('/').collect();
                    if parts.len() == 2 {
                        let w = parts[0].parse::<u16>().map_err(|_| "Failed to parse resoltuion")?;
                        let h = parts[1].parse::<u16>().map_err(|_| "Failed to parse resoltuion")?;
                        return Ok((w, h));
                    }
                }
            }
        }
        Err("Cant' find resolution info".to_string())
    }

    pub fn new(device_path: &str, width: u32, height: u32) -> Self {
        Self {
            device_path: device_path.to_string(),
            width,
            height,
        }
    }

    /// 캡처된 JPG 파일의 바이트 헤더를 읽어 실제 (가로, 세로) 픽셀 해상도를 반환
    pub fn get_jpeg_resolution(jpg_path: &str) -> Result<(u16, u16), String> {
        let bytes = fs::read(jpg_path)
            .map_err(|e| format!("Failed to read JPG File: {}", e))?;

        let mut i = 0;
        while i + 9 < bytes.len() {
            // JPEG에서 프레임 시작 마커(SOF0: 0xFF 0xC0 또는 SOF2: 0xFF 0xC2)를 찾습니다.
            if bytes[i] == 0xFF && (bytes[i + 1] == 0xC0 || bytes[i + 1] == 0xC2) {
                // 마커 뒤 5~6번째 바이트가 세로(Height), 7~8번째 바이트가 가로(Width)입니다!
                let height = u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]);
                let width  = u16::from_be_bytes([bytes[i + 7], bytes[i + 8]]);
                return Ok((width, height));
            }
            i += 1;
        }

        Err("No valid JPEG Header.".to_string())
    }

    /// 화면 캡처후 메모리의 byte로
    pub fn capture_raw(&self) -> Result<Vec<u8>, String> {
        let fmt_arg = format!("--set-fmt-video=width={},height={},pixelformat=MJPG", self.width, self.height);
        let device_arg = format!("--device={}", self.device_path);

        // v4l2-ctl 실행
        // --stream-mmap = 4: 스트림 받아 저장할 프레임 개수(Memory Map)
        // --stream-skip = 4: 저장된 과거 프레임 4장 버림
        // --stream-count = 1: 딱 1장만 촬영
        let output = Command::new("timeout")
            .arg("3s")
            .arg("v4l2-ctl")
            .arg("--silent")
            .arg(&device_arg)
            .arg(&fmt_arg)
            .arg("--stream-mmap=4")
            .arg("--stream-skip=4")
            .arg("--stream-count=1")
            .output()
            .map_err(|e| format!("Fail to execute v412-ctl: {}", e))?;

        if !output.status.success() || output.stdout.is_empty() {
            let err_msg = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Display Capture Error: {}", err_msg));
        }

        Ok(output.stderr)

    }


    // // 현재 화면을 캡처하여 지정된 경로에 jpg로 저장하고 파일 크기를 반환
    // pub fn capture_to_file(&self, output_path: &str) -> Result<usize, String> {
    //     let fmt_arg = format!("--set-fmt-video=width={},height={},pixelformat=MJPG", self.width, self.height);
    //     let device_arg = format!("--device={}", self.device_path);
    //     let stream_to_arg = format!("--stream-to={}", output_path);

    //     // v4l2-ctl 실행
    //     // --stream-skip = 4: 저장된 과거 프레임 4장 버림
    //     // --stream-count = 1: 딱 1장만 촬영
    //     let output = Command::new("timeout")
    //         .arg("3s")
    //         .arg("v4l2-ctl")
    //         .arg(&device_arg)
    //         .arg(&fmt_arg)
    //         .arg("--stream-mmap=4")
    //         .arg("--stream-skip=4")
    //         .arg("--stream-count=1")
    //         .arg(&stream_to_arg)
    //         .output()
    //         .map_err(|e| format!("Fail to execute v412-ctl: {}", e))?;

    //     if !output.status.success() {
    //         let err_msg = String::from_utf8_lossy(&output.stderr);
    //         return Err(format!("Display Capture Error: {}", err_msg));
    //     }
    //     else {
    //         println!("Status: OK")
    //     }

    //     let metadata = fs::metadata(output_path)
    //         .map_err(|e| format!("Cant' find image: {}", e))?;
    //     let file_size = metadata.len() as usize;
    //     if file_size == 0{
    //         return  Err("No response from Capture Board (Zero byte size error)".to_string());
    //     }

    //     println!("Capture Complete: {} ({} Bytes)", output_path, file_size);
    //     Ok(file_size)

    // }

    pub fn capture_image(&self) -> Result<RgbImage, String> {
        let raw = self.capture_raw()?;

        // 메모리에 담긴 JPEG 바이트 배열(output.stdout)을 바로 이미지로 변환!
        let img = image::load_from_memory(&raw)
            .map_err(|e| format!("Failed to Decode JPEG Memory: {}", e))?
            .to_rgb8();

        Ok(img)
    }

    /// 조건함수가 true를 반환할 때 까지 500ms 마다 캡처하며 대기
    pub fn wait_until<F>(&self, timeout_secs: u64, mut check_fn: F) -> Result<(), String>
    where
        F: FnMut(&RgbImage) -> bool,
    {
        let start = Instant::now();
        let timeout = Duration::from_secs(timeout_secs);

        while start.elapsed() < timeout {
            if let Ok(img) = self.capture_image() {
                if check_fn(&img) {
                    println!("Screen Check Success ({:.1} sec)", start.elapsed().as_secs_f32());

                    return Ok(());
                }
            }
            println!("Wait for Screen Capture Image... ({:1}s / {}s)", start.elapsed().as_secs_f32(), timeout_secs);
            thread::sleep(Duration::from_millis(500));
        }
        Err(format!("Timeout Error: No Screen Image."))
    }

    /// 이전 화면과 비교해 화면이 Threshold 이상 바뀔 때 까지 대기
    /// 화면 변화 여부만 알려주고 validate는 안됨
    pub fn wait_for_screen_change(
        &self,
        before_img: &RgbImage,
        thershold_percent: f32,
        timeout_secs: u64,
    ) -> Result<(), String> {
        self.wait_until(timeout_secs, |current_img| {
            let diff_ratio = Self::calc_diff_percent(before_img, current_img);
            println!("  -> Screen Change Ratio: {:.1}% / {:.1}%", diff_ratio, thershold_percent);
            diff_ratio >= thershold_percent
        })
    }

    /// 두 이미지 픽셀 차이를 비교해 변화율 계산
    fn calc_diff_percent(img1: &RgbImage, img2: &RgbImage) -> f32 {
        let (w, h) = (img1.width().min(img2.width()), img1.height().min(img2.height()));
        let mut changed = 0u32;
        let mut total = 0u32;

        for y in (0..h).step_by(10) {
            for x in (0..w).step_by(10) {
                let p1 = img1.get_pixel(x, y).0;
                let p2 = img2.get_pixel(x, y).0;

                // R, G, B 차이 합이 45 이상이면 바뀐 픽셀로 판정
                let diff = (p1[0] as i32 - p2[0] as i32).abs()
                         + (p1[1] as i32 - p2[1] as i32).abs()
                         + (p1[2] as i32 - p2[2] as i32).abs();
                if diff > 45 {
                    changed += 1;
                }
                total += 1;
            }
        }
        (changed as f32 / total as f32) * 100.0
    }
}