mod hid;
mod task;
mod capture;
mod overlay;
mod helper;
mod agent;
mod ocr;

use hid::HidController;
use capture::ScreenCapture;
use agent::KvmAgent;

fn main() -> Result<(), String> {
    let mut agent = KvmAgent::new();

    if true {
        task::unlock_screen(&mut agent, "46904598")?;
        // agent.hid.move_to(500, 130)?;
        // agent.hid.click_at(500, 130, 1)?;
        // hid.click_at(120, 320, 1)?;
        // hid.click_at(120, 320, 1)?;
        
        // // screen capture for logging
        // let snapshot_path = "/home/dy/windows_control/result/current_screen.jpg";
        // if capturer.capture_to_file(snapshot_path)? > 0 {
            //     let res = ScreenCapture::get_jpeg_resolution(snapshot_path);
            //     let (width, height) = res?;
            //     println!("Screenshot Resolution: W-{}, H-{}", width, height);
            // }
            // overlay::draw_history_on_capture(snapshot_path, "/home/dy/windows_control/result/overlay_screen.jpg", &hid.history)?;
    }
    else {
        task::get_monitoring_report(
            &mut agent,
            "https://dy.dycni.com:2435/main.do"
        )?;
    }
    Ok(())
}