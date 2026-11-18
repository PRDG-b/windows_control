//! Adapter for this repository's disposable Execute and Validate test page.
//! Not a general website detector. Never uses DOM or the test oracle as input.
use crate::{
    HidController, ScreenCapture,
    execution::{InputGuard, Observation},
    exploration::*,
    exploration_output::write_report,
    helper, ocr,
};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub fn scene_from_ocr(observation: Observation) -> Scene {
    let has = |s: &str| {
        observation
            .boxes
            .iter()
            .any(|b| b.text == s && b.confidence >= 70.)
    };
    let unknown = || Scene {
        observation: observation.clone(),
        state: None,
        candidates: vec![],
    };
    if !has("Execute") || !has("Validate") || observation.boxes.iter().any(|b| b.text == "WAITING")
    {
        return unknown();
    }
    let modes = [
        ("SUCCESSMODE", "success"),
        ("FAILUREMODE", "failure"),
        ("DUPLICATEMODE", "duplicate"),
        ("POPUPMODE", "popup"),
    ];
    let matches: Vec<_> = observation
        .boxes
        .iter()
        .filter_map(|b| {
            modes
                .iter()
                .find(|(label, _)| b.text == *label)
                .map(|(_, id)| (*id, b.confidence))
        })
        .collect();
    if matches.len() != 1 || matches[0].1 < 70. {
        return unknown();
    }
    let mode = matches[0].0;
    let confirms: Vec<_> = observation
        .boxes
        .iter()
        .filter(|b| b.text == "CONFIRM")
        .collect();
    let phase = if !confirms.is_empty() {
        if mode != "popup" || confirms.len() != 1 || confirms[0].confidence < 70. {
            return unknown();
        }
        "confirm"
    } else {
        let phases: Vec<_> = observation
            .boxes
            .iter()
            .filter(|b| ["READY", "COMPLETED", "FAILED"].contains(&b.text.as_str()))
            .collect();
        if phases.len() != 1 || phases[0].confidence < 70. {
            return unknown();
        }
        match phases[0].text.as_str() {
            "READY" => "ready",
            "COMPLETED" => "completed",
            "FAILED" => "failed",
            _ => unreachable!(),
        }
    };
    let candidate = |key: &str, text: &str| {
        let boxes: Vec<_> = observation
            .boxes
            .iter()
            .filter(|b| b.text == text)
            .collect();
        let allowed = boxes.len() == 1 && boxes[0].confidence >= 70.;
        Candidate {
            key: key.into(),
            frame_id: observation.frame_id,
            bounds: boxes
                .first()
                .map(|b| [b.x, b.y, b.w, b.h])
                .unwrap_or([0; 4]),
            permission: if allowed {
                Permission::Allowed
            } else {
                Permission::Unknown
            },
        }
    };
    let mut candidates = vec![];
    if phase == "ready" {
        candidates.push(candidate("run", "RUN"));
        for (id, label) in [
            ("failure", "FAILURE"),
            ("duplicate", "DUPLICATE"),
            ("popup", "POPUP"),
            ("success", "SUCCESS"),
        ] {
            if mode != id {
                candidates.push(candidate(&format!("mode_{id}"), label));
            }
        }
    } else if phase == "confirm" {
        candidates.push(candidate("confirm", "CONFIRM"));
    }
    Scene {
        state: Some(format!("{mode}_{phase}")),
        observation,
        candidates,
    }
}

pub fn run(dir: &Path, max_probes: usize) -> Result<(), String> {
    if !(1..=30).contains(&max_probes) {
        return Err("max-probes must be 1..30".into());
    }
    std::fs::create_dir(dir).map_err(|_| "Output directory must be new")?;
    let _guard = InputGuard::acquire(&std::env::temp_dir().join("windows-control-hid.lock"))?;
    let (width, height) = ScreenCapture::detect_resolution("/dev/video0")?;
    let mut env = Web {
        camera: ScreenCapture::new("/dev/video0", width.into(), height.into()),
        hid: HidController::new(width, height, "/dev/hidg0", "/dev/hidg1"),
        width,
        height,
        start: Instant::now(),
        id: 0,
        dir: dir.into(),
        require_root: true,
    };
    let limits = Limits {
        max_probes,
        timeout: Duration::from_secs((max_probes as u64 * 180).min(1800)),
        ..Limits::default()
    };
    // Initial observation must show success/ready. The caller opens and verifies
    // this test page first; an arbitrary foreground application is never probed.
    let report = explore(&mut env, &limits);
    write_report(&report, dir, true)?;
    if report.stop == Stop::Complete {
        Ok(())
    } else {
        Err(format!(
            "Exploration stopped: {} (partial graph saved)",
            report.stop.code()
        ))
    }
}

struct Web {
    camera: ScreenCapture,
    hid: HidController,
    width: u16,
    height: u16,
    start: Instant,
    id: u64,
    dir: PathBuf,
    require_root: bool,
}
impl Environment for Web {
    fn now(&self) -> Duration {
        self.start.elapsed()
    }
    fn observe(&mut self) -> Result<Scene, String> {
        let captured_at = self.start.elapsed();
        self.id += 1;
        let raw = self.camera.capture_raw()?;
        image::load_from_memory(&raw)
            .map_err(|_| "Capture decode failed")?
            .save(self.dir.join(format!("frame-{:04}.jpg", self.id)))
            .map_err(|_| "Cannot save capture")?;
        let boxes = ocr::parse_tsv(&ocr::run_ocr_from_memory(&raw)?)?;
        let mut scene = scene_from_ocr(Observation {
            frame_id: self.id,
            captured_at,
            width: self.width,
            height: self.height,
            boxes,
        });
        if self.require_root {
            if scene.state.as_deref() == Some("success_ready") {
                self.require_root = false;
            } else {
                scene.state = None;
                scene.candidates.clear();
            }
        }
        println!(
            "CAPTURE frame={} state={}",
            self.id,
            scene.state.as_deref().unwrap_or("unknown")
        );
        Ok(scene)
    }
    fn act(&mut self, c: &Candidate) -> Result<(), String> {
        let (x, y) = c.point();
        self.hid.click_at(x, y, 1)
    }
    fn restore(&mut self) -> Result<(), String> {
        // Refresh the already observed disposable page once; core recaptures.
        self.hid
            .send_key(helper::MOD_CTRL, helper::key_char('r').unwrap(), 0)
    }
    fn pause(&mut self) {
        std::thread::sleep(Duration::from_millis(200));
    }
}
