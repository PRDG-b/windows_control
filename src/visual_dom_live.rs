//! App-agnostic live HDMI/OCR + USB HID interactive visual DOM builder.
//! Captures screenshots from `/dev/video0`, runs Tesseract OCR + `ScreenAnalyzer`,
//! probes `Permission::Allowed` controls over `/dev/hidg0`/`/dev/hidg1`, and
//! exports HTML, ASCII tree, JSON summary, and interaction TSV artifacts.

use crate::{
    HidController, ScreenCapture,
    execution::{InputGuard, Observation},
    exploration::{Candidate, Environment, Permission, Scene},
    helper,
    ocr::{self, TextBox},
    screen::{ElementKind, ScreenAnalyzer, ScreenConfig},
    screen_output,
    visual_dom::{
        DomBuildConfig, DomBuildReport, DomStop, interact_and_build_dom_with_report,
        rewrap_with_containers,
    },
};
use std::{
    fmt::Write as _,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const MAX_INTERACTIONS_LIMIT: usize = 30;
const MAX_SETTLE_FRAMES: usize = 4;

/// How the live environment tries to restore to the root screen after probing a control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreMode {
    Esc,
    CtrlR,
    None,
}

impl RestoreMode {
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "esc" => Ok(Self::Esc),
            "ctrl-r" => Ok(Self::CtrlR),
            "none" => Ok(Self::None),
            _ => Err("Restore mode must be esc, ctrl-r, or none".into()),
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::Esc => "esc",
            Self::CtrlR => "ctrl-r",
            Self::None => "none",
        }
    }
}

/// Drops low-confidence or sub-glyph OCR boxes (e.g., 3x4px border noise on 1080p desktops/pages)
/// so they neither trigger false clicks nor poison `ScreenAnalyzer`'s `uncertain_ocr` gate.
pub fn filter_confident_boxes(boxes: Vec<TextBox>, min_confidence: f32) -> Vec<TextBox> {
    boxes
        .into_iter()
        .filter(|b| b.confidence >= min_confidence && b.w >= 4 && b.h >= 6)
        .collect()
}

/// Persists reconstructed DOM trees and controlled metadata artifacts (zero raw OCR text).
pub fn write_dom_artifacts(
    dir: &Path,
    report: &DomBuildReport,
    mode: &str,
    restore_mode: RestoreMode,
) -> Result<(), String> {
    let html = report.dom.to_html(0);
    let ascii = report.dom.to_ascii_tree(0);

    let root_state_json = report
        .root_state
        .as_ref()
        .map(|s| format!("{s:?}"))
        .unwrap_or_else(|| "null".into());

    let summary = format!(
        "{{\"mode\":\"{mode}\",\"restore\":\"{}\",\"stop\":\"{}\",\"root_state\":{root_state_json},\"total_nodes\":{},\"interactions_attempted\":{},\"interactions_completed\":{},\"observed_frames\":{},\"restored\":{},\"elapsed_ms\":{}}}",
        restore_mode.code(),
        report.stop.code(),
        report.dom.total_nodes(),
        report.interactions_attempted,
        report.interactions_completed,
        report.observed_frames,
        report.restored,
        report.elapsed.as_millis()
    );

    let mut events_tsv = String::from(
        "step\tkey\tx\ty\tw\th\tbefore_frame\tafter_frame\trestored_frame\teffect\tnote\n",
    );
    for ev in &report.events {
        let [x, y, w, h] = ev.bounds;
        let after = ev.after_frame.map(|f| f.to_string()).unwrap_or_default();
        let restored = ev.restored_frame.map(|f| f.to_string()).unwrap_or_default();
        writeln!(
            events_tsv,
            "{}\t{}\t{x}\t{y}\t{w}\t{h}\t{}\t{after}\t{restored}\t{}\t{}",
            ev.step, ev.candidate_key, ev.before_frame, ev.effect_code, ev.note
        )
        .map_err(|e| format!("Failed formatting dom_events.tsv: {e}"))?;
    }

    for (name, content) in [
        ("dom.html", html),
        ("dom.txt", ascii),
        ("dom_report.json", format!("{summary}\n")),
        ("dom_events.tsv", events_tsv),
    ] {
        std::fs::write(dir.join(name), content)
            .map_err(|e| format!("Cannot save DOM artifact {name}: {e}"))?;
    }

    println!("{summary}");
    Ok(())
}

/// Runs the app-agnostic HDMI/OCR + HID interactive DOM builder into `dir`.
pub fn run(dir: &Path, max_interactions: usize, restore_mode: RestoreMode) -> Result<(), String> {
    if max_interactions > MAX_INTERACTIONS_LIMIT {
        return Err(format!(
            "max-interactions must be 0..={MAX_INTERACTIONS_LIMIT}"
        ));
    }

    std::fs::create_dir(dir).map_err(|e| format!("Output directory must be new: {e}"))?;

    let _input_guard = if max_interactions > 0 {
        Some(InputGuard::acquire(
            &std::env::temp_dir().join("windows-control-hid.lock"),
        )?)
    } else {
        None
    };

    let (width, height) = ScreenCapture::detect_resolution("/dev/video0")?;
    let analyzer = ScreenAnalyzer::new(ScreenConfig {
        stable_frames: 2,
        max_elements: 2048,
        max_states: 128,
        max_age: Duration::from_secs(60),
        min_ocr_confidence: 70.0,
        allow_rect_controls: true,
        ..ScreenConfig::default()
    })?;

    let deadline = if max_interactions == 0 {
        Duration::from_secs(120)
    } else {
        Duration::from_secs((max_interactions as u64 * 120).clamp(180, 1800))
    };

    let hid = if max_interactions > 0 {
        Some(HidController::new(
            width,
            height,
            "/dev/hidg0",
            "/dev/hidg1",
        ))
    } else {
        None
    };

    let mut env = LiveScreenEnv {
        camera: ScreenCapture::new("/dev/video0", width.into(), height.into()),
        hid,
        analyzer,
        width,
        height,
        start: Instant::now(),
        deadline,
        frame_id: 0,
        dir: dir.to_path_buf(),
        restore_mode,
        max_interactions,
        base_panels: None,
    };

    let mut report = interact_and_build_dom_with_report(
        &mut env,
        &DomBuildConfig {
            max_interactions,
            timeout: deadline,
        },
    )?;

    if let Some(panels) = &env.base_panels {
        rewrap_with_containers(&mut report.dom, panels);
    }

    let mode = if max_interactions == 0 {
        "hdmi_observe_only"
    } else {
        "hdmi_hid"
    };
    write_dom_artifacts(dir, &report, mode, restore_mode)?;

    if report.stop == DomStop::Complete {
        Ok(())
    } else {
        Err(format!(
            "DOM build stopped: {} (partial DOM saved)",
            report.stop.code()
        ))
    }
}

struct LiveScreenEnv {
    camera: ScreenCapture,
    hid: Option<HidController>,
    analyzer: ScreenAnalyzer,
    width: u16,
    height: u16,
    start: Instant,
    deadline: Duration,
    frame_id: u64,
    dir: PathBuf,
    restore_mode: RestoreMode,
    max_interactions: usize,
    base_panels: Option<Vec<(String, [u16; 4])>>,
}

impl Environment for LiveScreenEnv {
    fn now(&self) -> Duration {
        self.start.elapsed()
    }

    fn observe(&mut self) -> Result<Scene, String> {
        let mut last_scene: Option<Scene> = None;

        for attempt in 0..MAX_SETTLE_FRAMES {
            if self.start.elapsed() >= self.deadline {
                return last_scene.ok_or_else(|| "DOM build timeout before observation".into());
            }

            let captured_at = self.start.elapsed();
            let raw = self.camera.capture_raw()?;
            let image = image::load_from_memory(&raw)
                .map_err(|e| format!("Capture decode failed: {e}"))?
                .into_rgb8();
            if image.width() != u32::from(self.width) || image.height() != u32::from(self.height) {
                return Err("Capture resolution changed; recalibrate before input".into());
            }

            self.frame_id += 1;
            image
                .save(self.dir.join(format!("frame-{:04}.jpg", self.frame_id)))
                .map_err(|e| format!("Cannot save capture: {e}"))?;

            let boxes =
                filter_confident_boxes(ocr::parse_tsv(&ocr::run_ocr_from_memory(&raw)?)?, 70.0);
            let obs = Observation {
                frame_id: self.frame_id,
                captured_at,
                width: self.width,
                height: self.height,
                boxes,
            };

            let analysis = self.analyzer.analyze(&image, obs, self.start.elapsed())?;
            screen_output::write_analysis(&self.dir, &image, &analysis)?;

            let allowed_count = analysis
                .scene
                .candidates
                .iter()
                .filter(|c| c.permission == Permission::Allowed)
                .count();
            println!(
                "DOM_CAPTURE frame={} state={} elements={} allowed={} stable={} quality={}",
                self.frame_id,
                analysis.scene.state.as_deref().unwrap_or("unknown"),
                analysis.elements.len(),
                allowed_count,
                analysis.stable,
                analysis.quality
            );

            let panels: Vec<(String, [u16; 4])> = analysis
                .elements
                .iter()
                .filter(|e| e.kind == ElementKind::Panel)
                .map(|e| (e.key.clone(), e.bounds))
                .collect();
            if (analysis.stable && self.base_panels.is_none()) || self.max_interactions == 0 {
                self.base_panels = Some(panels);
            }

            if analysis.stable {
                return Ok(analysis.scene);
            }

            last_scene = Some(analysis.scene);
            if attempt + 1 < MAX_SETTLE_FRAMES {
                std::thread::sleep(Duration::from_millis(250));
            }
        }

        last_scene.ok_or_else(|| "Observation failed without frames".into())
    }

    fn act(&mut self, c: &Candidate) -> Result<(), String> {
        let hid = self
            .hid
            .as_mut()
            .ok_or_else(|| "HID controller not initialized".to_string())?;
        let (x, y) = c.point();
        hid.click_at(x, y, 1)
    }

    fn restore(&mut self) -> Result<(), String> {
        match self.restore_mode {
            RestoreMode::Esc => {
                let hid = self
                    .hid
                    .as_mut()
                    .ok_or_else(|| "HID controller not initialized".to_string())?;
                hid.send_key(helper::MOD_NONE, helper::KEY_ESC, 0)
            }
            RestoreMode::CtrlR => {
                let hid = self
                    .hid
                    .as_mut()
                    .ok_or_else(|| "HID controller not initialized".to_string())?;
                let key_r =
                    helper::key_char('r').ok_or_else(|| "Missing HID key for 'r'".to_string())?;
                hid.send_key(helper::MOD_CTRL, key_r, 0)
            }
            RestoreMode::None => Ok(()),
        }
    }

    fn pause(&mut self) {
        std::thread::sleep(Duration::from_millis(350));
    }
}
