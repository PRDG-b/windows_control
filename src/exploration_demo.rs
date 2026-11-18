//! Deterministic synthetic environment; does not inspect or control a desktop.
use crate::{execution::Observation, exploration::*, exploration_output::write_report};
use std::{path::Path, time::Duration};

pub fn run(dir: &Path, scenario: &str) -> Result<(), String> {
    if !["normal", "unknown", "restore-error", "input-error"].contains(&scenario) {
        return Err("Unknown exploration demo scenario".into());
    }
    std::fs::create_dir(dir).map_err(|_| "Output directory must be new")?;
    let mut demo = Demo {
        state: "home",
        id: 0,
        time: Duration::ZERO,
        scenario,
        acted: false,
    };
    let report = explore(&mut demo, &Limits::default());
    write_report(&report, dir, false)?;
    if report.stop == Stop::Complete {
        Ok(())
    } else {
        Err(format!("Exploration stopped: {}", report.stop.code()))
    }
}

struct Demo<'a> {
    state: &'static str,
    id: u64,
    time: Duration,
    scenario: &'a str,
    acted: bool,
}
impl Environment for Demo<'_> {
    fn now(&self) -> Duration {
        self.time
    }
    fn observe(&mut self) -> Result<Scene, String> {
        self.id += 1;
        self.time += Duration::from_millis(1);
        let candidates = match self.state {
            "home" => vec![
                ("open", Permission::Allowed),
                ("delete", Permission::Denied),
                ("noop", Permission::Allowed),
            ],
            "details" => vec![("expand", Permission::Allowed)],
            _ => vec![],
        }
        .into_iter()
        .map(|(key, permission)| Candidate {
            key: key.into(),
            frame_id: self.id,
            bounds: [30 + self.id as u16, 40, 80, 30],
            permission,
        })
        .collect();
        Ok(Scene {
            observation: Observation {
                frame_id: self.id,
                captured_at: self.time,
                width: 800,
                height: 600,
                boxes: vec![],
            },
            state: if self.scenario == "unknown" && self.acted {
                None
            } else {
                Some(self.state.into())
            },
            candidates,
        })
    }
    fn act(&mut self, c: &Candidate) -> Result<(), String> {
        self.time += Duration::from_millis(1);
        self.acted = true;
        self.state = match c.key.as_str() {
            "open" => "details",
            "expand" => "expanded",
            "noop" => "home",
            _ => return Err("Unknown synthetic action".into()),
        };
        if self.scenario == "input-error" {
            Err("Synthetic partial input".into())
        } else {
            Ok(())
        }
    }
    fn restore(&mut self) -> Result<(), String> {
        self.time += Duration::from_millis(1);
        self.state = if self.scenario == "restore-error" {
            "wrong"
        } else {
            "home"
        };
        Ok(())
    }
}
