use crate::agent::KvmAgent;
use crate::{helper::*, ocr};

// 종료 조건 정의
// pub enum DoneCondition {
//     Immediate,                                 // 전송 즉시 종료 (단순 키 입력 등)
//     LuminanceAbove(u8),                        // 평균 밝기가 기준치 이상일 때 (절전 해제)
//     ScreenDiffAbove(f32),                      // 실행 전 화면 대비 N% 이상 바뀌었을 때 (예: 0.08 = 8%)
//     RoiDiffAbove { x: u32, y: u32, w: u32, h: u32, threshold: f32 }, // 특정 구역만 바뀌었을 때 (Win+R 등)
//     ChangeThenSettle { diff_min: f32, settle_max: f32 },              // 화면이 바뀐 뒤 로딩이 멈췄을 때 (앱 실행/웹 이동)
// }

// 절전 화면 깨우기 및 잠금 화면 해제
pub fn unlock_screen(agent: &mut KvmAgent, pin: &str) -> Result<(), String> {
    println!("Trying to Wake PC Up...");
    // 1. 마우스 움직여서 모니터 깨워보기
    agent.hid.wake_mouse()?;

    // 2. 스페이스 바 눌러 잠금화면 시계 걷어내기
    agent.hid.send_key(MOD_NONE, KEY_SPACE, 0)?;
    let jpeg_bytes = agent.cam.capture_raw()?;

    // 스트림 데이터 처리
    ocr::run_ocr_from_memory(&jpeg_bytes)?;
    // agent.act_and_wait(5.0, 5, |a| a.hid.send_key(MOD_NONE, KEY_SPACE, 0))?;

    // PIN 타이핑 후 엔터
    agent.hid.type_text(pin)?;
    agent.hid.send_key(MOD_NONE, KEY_ENTER, 0)
}

pub fn open_web(agent: &mut KvmAgent) -> Result<(), String> {
    open_browser_with_hid(&mut agent.hid)
}

pub fn open_browser_with_hid(hid: &mut crate::HidController) -> Result<(), String> {
    hid.send_key(MOD_WIN, key_char('r').unwrap(), 0)?;
    hid.type_text("msedge")?;
    hid.send_key(MOD_NONE, KEY_ENTER, 0)
}

pub fn open_url(agent: &mut KvmAgent, url: &str) -> Result<(), String> {
    open_url_with_hid(&mut agent.hid, url)
}

/// Launch Edge with a URL using the existing Win+R HID path.
/// Caller must verify the resulting page using a fresh observation.
pub fn open_url_with_hid(hid: &mut crate::HidController, url: &str) -> Result<(), String> {
    validate_browser_url(url)?;
    hid.send_key(MOD_WIN, key_char('r').unwrap(), 0)?;
    hid.type_text(&format!("msedge {url}"))?;
    hid.send_key(MOD_NONE, KEY_ENTER, 0)
}

fn validate_browser_url(url: &str) -> Result<(), String> {
    if !(url.starts_with("http://")
        || url.starts_with("https://")
        || (url.starts_with("file:///C:/") && url.ends_with(".html")))
        || url
            .chars()
            .any(|c| c.is_whitespace() || c == '"' || char_to_hid(c).is_none())
    {
        return Err(
            "Expected HTTP(S) URL or local C: HTML file supported by the US keyboard".into(),
        );
    }
    Ok(())
}

/// Runs the embedded self-contained test app on the target browser, without a server.
/// The address bar must have been focused and verified on a fresh capture first.
pub fn type_local_test_app(hid: &mut crate::HidController) -> Result<(), String> {
    let mut url = String::from("data:text/html;charset=utf-8,");
    for line in include_str!("../test-app/index.html").lines() {
        for byte in line.trim().bytes() {
            if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
                url.push(byte as char);
            } else {
                use std::fmt::Write;
                write!(&mut url, "%{byte:02X}").map_err(|e| e.to_string())?;
            }
        }
    }
    hid.type_text_with_interval(&url, std::time::Duration::from_millis(10))?;
    hid.send_key(MOD_NONE, KEY_ENTER, 0)
}

/// Save the self-contained test page in the target PC and reopen that local HTML.
pub fn save_page_locally(_hid: &mut crate::HidController, path: &str) -> Result<(), String> {
    if !path.starts_with("C:\\")
        || !path.ends_with(".html")
        || path.contains('"')
        || path
            .chars()
            .any(|c| c.is_whitespace() || char_to_hid(c).is_none())
    {
        return Err("Expected an absolute C:\\ path ending in .html without spaces".into());
    }
    Err(
        "save-page disabled: save dialog transitions require separate recapture; use local-app"
            .into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn browser_accepts_local_html_but_rejects_other_schemes() {
        assert!(
            validate_browser_url(
                "file:///C:/Users/Public/Documents/windows-control-ev-20261113.html"
            )
            .is_ok()
        );
        assert!(validate_browser_url("javascript:alert(1)").is_err());
        assert!(validate_browser_url("file:///C:/test.exe").is_err());
    }
    #[test]
    fn local_save_rejects_invalid_path_before_input() {
        let mut hid = crate::HidController::new(1920, 1080, "missing-keyboard", "missing-mouse");
        assert!(
            save_page_locally(&mut hid, "relative.html")
                .unwrap_err()
                .contains("absolute")
        );
        assert!(hid.history.is_empty());
    }

    #[test]
    fn blind_save_sequence_is_disabled() {
        let mut hid = crate::HidController::new(1920, 1080, "missing-keyboard", "missing-mouse");
        assert!(
            save_page_locally(&mut hid, "C:\\Users\\Public\\Documents\\test.html")
                .unwrap_err()
                .contains("disabled")
        );
        assert!(hid.history.is_empty());
    }
}

pub fn get_monitoring_report(agent: &mut KvmAgent, url: &str) -> Result<(), String> {
    // println!("=== Start Task ===");
    agent.act_and_wait(40.0, 5, |a| open_url(a, url))?;
    agent.hid.click_at(900, 540, 1)?;
    agent.hid.type_text(
        &std::env::var("WINDOWS_CONTROL_USERNAME")
            .map_err(|_| "Missing WINDOWS_CONTROL_USERNAME")?,
    )?;
    agent.hid.click_at(900, 590, 1)?;
    agent.hid.type_text(
        &std::env::var("WINDOWS_CONTROL_PASSWORD")
            .map_err(|_| "Missing WINDOWS_CONTROL_PASSWORD")?,
    )?;
    agent.hid.send_key(MOD_NONE, KEY_ENTER, 0)?;

    Ok(())
}
