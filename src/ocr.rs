use std::process::{Command, Stdio};
use std::io::Write;

use image::codecs::jpeg;


pub struct TextBox {
    pub text: String,
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
    pub confidence: f32,
}

impl TextBox {
    pub fn center(&self) -> (u16, u16) {
        (self.x + self.w / 2, self.y + self.h / 2)
    }
}

pub fn extract_boxes(psm: u8) -> Result<(), String> {
    let psm_arg = format!("--psm {}", psm);
    let output = Command::new("tesseract")
        .arg("path/to/jpg")
        .arg("stdout")
        .arg("-l eng+kor")
        .arg(psm_arg)
        .arg("tsv")
        .output()
        .map_err(|e| format!("Fail to detect text: {}", e))?;

    Ok(())
}

pub fn run_ocr_from_memory(jpeg_bytes: &[u8]) -> Result<String, String> {
    // stdin, stdout 파이프
    let mut child = Command::new("tesseract")
        .args(["stdin", "stdout", "-l", "eng+kor", "--psm", "11", "tsv"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("tesseract execution failed: {}", e))?;

    // 메모리의 이미지 바이트를 스트림으로 전송
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(jpeg_bytes)
            .map_err(|e| format!("tesseract stdin streaming failed: {}", e))?;
    }

    let output = child.wait_with_output()
        .map_err(|e| format!("tesseract without output: {}", e))?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}