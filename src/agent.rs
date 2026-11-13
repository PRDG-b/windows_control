use crate::HidController;
use crate::ScreenCapture;

pub struct KvmAgent {
    pub hid: HidController,   // 손 (키보드/마우스)
    pub cam: ScreenCapture,  // 눈 (캡처보드)
}

impl KvmAgent {
    pub fn new() -> Self {
        let dev = "/dev/video0";
        let (w, h) = ScreenCapture::detect_resolution(dev).unwrap_or((1920, 1080));
        Self {
            hid: HidController::new(w, h, "/dev/hidg0", "/dev/hidg1"),
            cam: ScreenCapture::new(dev, w as u32, h as u32),
        }
    }

    /// "동작 실행 후 화면 바뀔 때까지 대기"
    pub fn act_and_wait<F>(&mut self, change_pct: f32, timeout_sec: u64, action: F) -> Result<(), String>
    where F: FnOnce(&mut Self) -> Result<(), String>,
    {
        println!("[1/3]");
        let before = self.cam.capture_image()?;
        
        println!("[2/3]");
        action(self)?; // 전달받은 조작 실행

        println!("[3/3]");
        self.cam.wait_for_screen_change(&before, change_pct, timeout_sec)
    }
}