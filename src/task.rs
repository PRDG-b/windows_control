use crate::agent::{KvmAgent};
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
    ocr::run_ocr_from_memory(jpeg_bytes)
    // agent.act_and_wait(5.0, 5, |a| a.hid.send_key(MOD_NONE, KEY_SPACE, 0))?;

    // PIN 타이핑 후 엔터
    agent.hid.type_text(pin)?;
    agent.hid.send_key(MOD_NONE, KEY_ENTER, 0)
}

pub fn open_web(agent: &mut KvmAgent) -> Result<(), String> {
    agent.hid.send_key(MOD_WIN, key_char('r').unwrap(), 0)?;
    agent.hid.type_text("msedge")?;
    agent.hid.send_key(MOD_NONE, KEY_ENTER, 0)?;

    Ok(())
}

pub fn open_url(agent: &mut KvmAgent, url: &str) -> Result<(), String> {
    agent.act_and_wait(20.0, 5, open_web)?;

    if let Some((modifier, key_code)) = char_to_hid('t') {
        agent.hid.send_key(MOD_CTRL | modifier, key_code, 0)?;
    }
    agent.hid.type_text(url)?;
    agent.hid.send_key(MOD_NONE, KEY_ENTER, 0)?;

    Ok(())
}

pub fn get_monitoring_report(agent: &mut KvmAgent, url: &str) -> Result<(), String> {
    // println!("=== Start Task ===");
    agent.act_and_wait(40.0, 5, |a| open_url(a, url))?;
    agent.hid.click_at(900, 540, 1)?;
    agent.hid.type_text("dycni")?;
    agent.hid.click_at(900, 590, 1)?;
    agent.hid.type_text("dysolar#@!32")?;
    agent.hid.send_key(MOD_NONE, KEY_ENTER, 0)?;

    Ok(())
}