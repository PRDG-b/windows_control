//! Explicit mouse-only wake attempt. This does not claim that the PC woke up.
use windows_control::{HidController, ScreenCapture};

fn main() -> Result<(), String> {
    let (width, height) = ScreenCapture::detect_resolution("/dev/video0")?;
    if width < 2 || height == 0 {
        return Err("Invalid resolution".into());
    }
    let mut hid = HidController::new(width, height, "/dev/hidg0", "/dev/hidg1");
    hid.move_to(width / 2, height / 2)?;
    hid.move_to(width / 2 + 1, height / 2)?;
    println!("WAKE_INPUT_SENT; verify with observe");
    Ok(())
}
