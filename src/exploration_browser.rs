//! Headless disposable browser adapter. Pixels/OCR and coordinate input only.
use crate::{
    execution::Observation,
    exploration::*,
    exploration_output::write_report,
    ocr,
    screen::{ScreenAnalyzer, ScreenConfig},
    screen_output::write_analysis,
};
use std::{
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{Receiver, sync_channel},
    time::{Duration, Instant},
};

struct Bridge {
    child: Child,
    input: ChildStdin,
    output: Receiver<Result<String, String>>,
    closed: bool,
}
impl Bridge {
    fn start(dir: &Path) -> Result<Self, String> {
        let fixture = Path::new("test-env/fixture.html")
            .canonicalize()
            .map_err(|_| "Missing isolated browser fixture")?;
        let mut cmd = Command::new("python3");
        cmd.args(["-u", "test-env/browser_bridge.py", "--output-dir"])
            .arg(dir)
            .arg("--fixture")
            .arg(fixture);
        if let Ok(addr) = std::env::var("RERUN_ADDR") {
            if !addr.trim().is_empty() {
                cmd.arg("--rerun").arg(addr.trim());
            }
        }
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|_| "Cannot start browser bridge (use test container)")?;
        let input = child.stdin.take().ok_or("Missing bridge input")?;
        let reader = BufReader::new(child.stdout.take().ok_or("Missing bridge output")?);
        let (tx, output) = sync_channel(1);
        std::thread::spawn(move || {
            for line in reader.lines() {
                if tx
                    .send(line.map_err(|_| "Bridge output failed".into()))
                    .is_err()
                {
                    break;
                }
            }
        });
        let mut bridge = Self {
            child,
            input,
            output,
            closed: false,
        };
        if bridge.receive()? != "READY" {
            return Err("Browser bridge did not initialize".into());
        }
        Ok(bridge)
    }
    fn receive(&mut self) -> Result<String, String> {
        self.output
            .recv_timeout(Duration::from_secs(30))
            .map_err(|_| "Browser bridge timeout or disconnect".to_string())?
    }
    fn request(&mut self, command: &str) -> Result<(), String> {
        if self.closed {
            return Err("Browser bridge already closed".into());
        }
        let result = (|| {
            writeln!(self.input, "{command}")
                .and_then(|_| self.input.flush())
                .map_err(|_| "Browser bridge write failed")?;
            if self.receive()? != "OK" {
                return Err("Browser operation failed; action not retried".into());
            }
            Ok(())
        })();
        if result.is_err() {
            self.closed = true;
            let _ = self.child.kill();
        }
        result
    }
    fn close(&mut self) -> Result<(), String> {
        if self.closed {
            return Ok(());
        }
        let result = self.request("QUIT");
        self.closed = true;
        result
    }
}
impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = self.close();
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(2) {
            if self.child.try_wait().ok().flatten().is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn run(dir: &Path, max_probes: usize) -> Result<(), String> {
    if !(1..=30).contains(&max_probes) {
        return Err("max-probes must be 1..30".into());
    }
    std::fs::create_dir(dir).map_err(|_| "Output directory must be new")?;
    let dir = dir
        .canonicalize()
        .map_err(|_| "Cannot resolve output directory")?;
    let bridge = Bridge::start(&dir)?;
    let mut env = Browser {
        bridge,
        dir,
        model: ScreenAnalyzer::new(ScreenConfig {
            allow_rect_controls: true,
            ..ScreenConfig::default()
        })?,
        start: Instant::now(),
        id: 0,
    };
    let report = explore(
        &mut env,
        &Limits {
            max_probes,
            timeout: Duration::from_secs(300),
            max_reobservations: 3,
            ..Limits::default()
        },
    );
    write_report(&report, &env.dir, false)?;
    // Only the independent evaluator reads oracle.json after this point.
    env.bridge.close()?;
    if report.stop == Stop::Complete {
        Ok(())
    } else {
        Err(format!(
            "Browser exploration stopped: {}",
            report.stop.code()
        ))
    }
}
struct Browser {
    bridge: Bridge,
    dir: PathBuf,
    model: ScreenAnalyzer,
    start: Instant,
    id: u64,
}
impl Environment for Browser {
    fn now(&self) -> Duration {
        self.start.elapsed()
    }
    fn observe(&mut self) -> Result<Scene, String> {
        let captured_at = self.start.elapsed();
        self.id += 1;
        self.bridge.request(&format!("OBSERVE\t{}", self.id))?;
        let image = image::open(self.dir.join(format!("frame-{:04}.jpg", self.id)))
            .map_err(|_| "Cannot decode browser screenshot")?
            .into_rgb8();
        let tsv = std::fs::read_to_string(self.dir.join(format!("frame-{:04}.tsv", self.id)))
            .map_err(|_| "Cannot read browser OCR")?;
        let obs = Observation {
            frame_id: self.id,
            captured_at,
            width: u16::try_from(image.width()).map_err(|_| "Browser screenshot too wide")?,
            height: u16::try_from(image.height()).map_err(|_| "Browser screenshot too tall")?,
            boxes: ocr::parse_tsv(&tsv)?,
        };
        let analysis = self.model.analyze(&image, obs, self.start.elapsed())?;
        write_analysis(&self.dir, &image, &analysis)?;
        println!(
            "BROWSER frame={} state={} elements={} quality={}",
            self.id,
            analysis.scene.state.as_deref().unwrap_or("unknown"),
            analysis.elements.len(),
            analysis.quality
        );
        Ok(analysis.scene)
    }
    fn act(&mut self, c: &Candidate) -> Result<(), String> {
        let (x, y) = c.point();
        self.bridge.request(&format!("CLICK\t{x}\t{y}"))
    }
    fn restore(&mut self) -> Result<(), String> {
        self.bridge.request("RESET")
    }
    fn pause(&mut self) {
        std::thread::sleep(Duration::from_millis(100));
    }
}
