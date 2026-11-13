use std::fs;
use std::io::{Write};
use std::io::stdout;
use std::thread;
use std::time::Duration;

use crate::helper;

#[derive(Debug, Clone)]
pub struct ActionLog {
    pub step: usize,
    pub description: String,        // 예: CLICK (960, 540) ABS(16383,16383)
    pub pixel_pos: Option<(u16, u16)>,
}

pub struct HidController {
    // 마우스/키보드 제어기 구조체 정의
    pub screen_width: u16,  // 모니터 가로 픽셀 (예: 1920.0)
    pub screen_height: u16, // 모니터 세로 픽셀 (예: 1080.0)
    kbd_path: String,
    mouse_path: String,
    last_pos: (u16, u16),    // 마지막으로 이동한 좌표 바이트
    pub history: Vec<ActionLog>,
}

impl HidController  {
    // 컨트롤러 구현
    // 마우스 제어 5바이트 패킷 송신
    // 키보드 제어 8바이트 패킷 송신

    // 생성자
    pub fn new(screen_width: u16, screen_height: u16, kbd_path: &str, mouse_path: &str) -> Self {
        Self {
            screen_width,
            screen_height,
            kbd_path: kbd_path.to_string(),
            mouse_path: mouse_path.to_string(),
            last_pos: (0, 0),
            history: Vec::new(),
        }
    }

    // ---------------------------------------------------------
    // 헬퍼 함수
    // ---------------------------------------------------------

    fn record(&mut self, desc: String, pos: Option<(u16, u16)>) {
        let step = self.history.len() + 1;
        self.history.push(
            ActionLog { step, description: desc, pixel_pos: pos }
        );

        // 후속 동작 위해 잠시 대기
        thread::sleep(Duration::from_millis(200));
    }

    // 핵심: 파일을 구조체에 계속 들고 있지 않고, 쓸 때만 잠깐 열었다가
    // 함수가 끝나는 순간 Drop 하여 KVM 점유 방지
    fn write_packet(path: &str, packet: &[u8]) -> Result<(), String> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .open(path)
            .map_err(|e| format!("디바이스 열기 실패({}): {}", path, e))?;
        
        match file.write_all(packet) {
            Ok(_) => {
                let _ = file.flush();
                return  Ok(());
            }
            Err(e) => return Err(format!("Failed to Send packet: {}", e)),
        }
    }

    // (x, y) 픽셀좌표를 HID 용 4바이트 변환하는 함수
    fn to_abs_bytes(&self, x: u16, y: u16) -> [u8;4] {
        let abs_x = ((x as u32 * 32767) / self.screen_width as u32) as u16;
        let abs_y = ((y as u32 * 32767) / self.screen_height as u32) as u16;
        let [xl, xh] = abs_x.to_le_bytes();
        let [yl, yh] = abs_y.to_le_bytes();
        [xl, xh, yl, yh]
    }

    // ---------------------------------------------------------
    // A. 마우스 제어 (/dev/hidg1)
    // ---------------------------------------------------------
    pub fn wake_mouse(&mut self) -> Result<(), String> {
        self.move_to(960, 540)?;
        self.move_to(970, 540)
    }
    
    // 좌표 이동
    pub fn move_to(&mut self, x:u16, y:u16) -> Result<(), String> {
        // 픽셀 좌표를 USB 절대 좌표로 비례 변환
        // as f32, as u16은 타입 캐스팅 문법임
        let abs_x = ((x as f32 / self.screen_width as f32) * 32767.0) as u16;
        let abs_y = ((y as f32 / self.screen_height as f32) * 32767.0) as u16;
        println!("Pixel casting: P({}, {}) -> USB({}, {})", x, y, abs_x, abs_y);

        // 16비트 숫자를 2바이트로 쪼개기
        let x_bytes: [u8; 2] = abs_x.to_le_bytes();      // [하위 1바이트, 상위 1바이트]
        let y_bytes: [u8; 2] = abs_y.to_le_bytes();
        
        // 마우스 이동 패킷 (버튼 = 0x00)
        let move_packet: [u8; 5] = [0x00, x_bytes[0], x_bytes[1], y_bytes[0], y_bytes[1]];
        
        // 패킷 전송
        // self.last_bytes = [x_bytes[0], x_bytes[1], y_bytes[0], y_bytes[1]];
        let _ = Self::write_packet(&self.mouse_path, &move_packet)?;
        self.last_pos = (x, y);
        self.record(
            format!("MOVE ({}, {}) ABS ({}, {})", x, y, abs_x, abs_y),
            Some((x, y))
        );
        Ok(())
    }

    // 제자리 클릭
    pub fn click(&mut self, button: u8, click_dur: u64) -> Result<(), String> {
        let (x, y) = self.last_pos;
        let b = self.to_abs_bytes(x, y);

        let _ = Self::write_packet(&self.mouse_path, &[button, b[0], b[1], b[2], b[3]])?;   // 누름
        thread::sleep(Duration::from_millis(click_dur));
        let _ = Self::write_packet(&self.mouse_path, &[0x00, b[0], b[1], b[2], b[3]]);      // 뗌

        let abs_x = u16::from_le_bytes([b[0], b[1]]);
        let abs_y = u16::from_le_bytes([b[2], b[3]]);
        let x = ((abs_x as u32 * self.screen_width as u32) / 32767) as u16;
        let y = ((abs_y as u32 * self.screen_height as u32) / 32767) as u16;

        let btn_name = match button {
            0x01 => "L-CLICK", // 좌클릭
            0x02 => "R-CLICK", // 우클릭
            0x04 => "M-CLICK", // 휠(중간)클릭
            _    => "CLICK",
        };
        // 5. 지속시간(click_dur)을 포함해서 history에 기록!
        // 예: "L-CLICK 2S (960,540) ABS(16383,16383)"
        self.record(
            format!("{} {}S ({},{}) ABS({},{})", btn_name, click_dur, x, y, abs_x, abs_y),
            Some((x, y)),
        );

        Ok(())
    }

    // 좌표 이동 후 좌클릭
    pub fn click_at(&mut self, x:u16, y:u16, button: u8) -> Result<(), String> {
        self.move_to(x, y)?;
        self.click(button, 0)
    }

    // ---------------------------------------------------------
    // B. 키보드 제어 (/dev/hidg0)
    // ---------------------------------------------------------
    // 조합키 + 일반키 1회 눌렀다 떼기
    pub fn send_key(&mut self, modifier: u8, key_code: u8, down_dur: u64) -> Result<(), String> {
        let down = [modifier, 0x00, key_code, 0, 0, 0, 0, 0];
        let up = [0x00u8; 8];

        let _ = Self::write_packet(&self.kbd_path, &down)?;
        thread::sleep(Duration::from_secs(down_dur));
        let _ = Self::write_packet(&self.kbd_path, &up)?;

        self.record(format!("KEY MOD:0x{:02X} CODE:0x{:02X}", modifier, key_code), None);
        Ok(())
    }

    // 문자열 타이핑
    pub fn type_text(&mut self, text:&str) -> Result<(), String> {
        println!("Typing: ");
        for ch in text.chars() {
            if let Some((modifier, key_code)) = helper::char_to_hid(ch) {
                self.send_key(modifier, key_code, 0)?;
                print!("{}", ch);
                stdout().flush().unwrap()
            }
        }
        println!("");
        Ok(())
    }
}

impl Drop for HidController {
    fn drop(&mut self) {
        eprintln!("[Drop] Release All control of Keyboard/Mouse...");

        // 1. 키보드 모든 키 떼기 (8바이트 모두 0)
        let _ = Self::write_packet(&self.kbd_path, &[0, 0, 0, 0, 0, 0, 0, 0]);

        // 2. 마우스 버튼 떼기 (마지막 좌표는 유지하되 첫 번째 바이트(버튼)를 0으로!)
        let (x, y) = self.last_pos;
        let [xl, xh, yl, yh] = self.to_abs_bytes(x, y);
        let _ = Self::write_packet(&self.mouse_path, &[0x00, xl, xh, yl, yh, 0x00]);
    }
}