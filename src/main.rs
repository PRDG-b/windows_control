use std::time::{Duration, Instant};
use windows_control::{HidController, ScreenCapture, ocr, task, validation};

fn main() {
    if let Err(error) = run() {
        eprintln!("ERROR: {error}");
        std::process::exit(1);
    }
}

fn connect_rerun(url: impl Into<String>) -> Option<rerun::RecordingStream> {
    let rec = rerun::RecordingStreamBuilder::new("windows_control")
        .connect_grpc_opts(url)
        .inspect_err(|err| {
            eprintln!("Rerun Connection Error: {err}");
        })
        .ok();
    rec
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = args.first().map(String::as_str) else {
        usage();
        return Ok(());
    };
    if command == "--help" || command == "help" {
        usage();
        return Ok(());
    }
    if command == "explore-browser" {
        if args.len() != 3 {
            return Err(
                "Usage: explore-browser <new-directory> <max-probes> (isolated test container)"
                    .into(),
            );
        }
        let probes = args[2].parse().map_err(|_| "Invalid max-probes")?;
        return windows_control::exploration_browser::run(std::path::Path::new(&args[1]), probes);
    }
    if command == "explore-test-web" {
        if args.len() != 3 {
            return Err("Usage: explore-test-web <new-directory> <max-probes>".into());
        }
        let max_probes = args[2].parse::<usize>().map_err(|_| "Invalid max-probes")?;
        let rec = connect_rerun("rerun+http://192.168.0.20:9876/proxy");
        let path_buf = std::path::Path::new(".rerun").join(&args[1]);
        let path = path_buf.as_path();

        return windows_control::exploration_web::run(path, max_probes, rec);
    }
    if command == "explore-demo" {
        if !(2..=3).contains(&args.len()) {
            return Err(
                "Usage: explore-demo <new-directory> [normal|unknown|restore-error|input-error]"
                    .into(),
            );
        }
        return windows_control::exploration_demo::run(
            std::path::Path::new(&args[1]),
            args.get(2).map(String::as_str).unwrap_or("normal"),
        );
    }
    if command == "replay" {
        if args.len() != 2 {
            return Err("Usage: replay <scenario.replay>".into());
        }
        let report = windows_control::replay::run(std::path::Path::new(&args[1]))?;
        windows_control::replay::print_report(&report, "replay");
        return if report.reason == validation::Reason::Validated {
            Ok(())
        } else {
            Err("Replay did not validate; see report".into())
        };
    }
    if command == "verify-jpeg" && args.len() == 2 {
        return ocr::verify_jpeg_file(std::path::Path::new(&args[1]));
    }
    if !matches!(
        command,
        "observe"
            | "click"
            | "open-url"
            | "save-page"
            | "focus-address"
            | "local-app"
            | "compare-ocr"
            | "open-browser"
            | "click-point"
    ) {
        return Err("Unknown command; use --help".into());
    }
    let timeout = match command {
        "observe" if (1..=3).contains(&args.len()) => Duration::from_secs(10),
        "compare-ocr" if args.len() == 2 => Duration::from_secs(40),
        "focus-address" if args.len() == 1 => Duration::from_secs(10),
        "open-browser" if args.len() == 1 => Duration::from_secs(10),
        "click-point" if args.len() == 3 => Duration::from_secs(10),
        "local-app" if args.len() == 1 => Duration::from_secs(240),
        "click" | "open-url" | "save-page" if args.len() == 3 || args.len() == 4 => {
            let seconds = args
                .get(3)
                .map(|s| s.parse::<u64>())
                .transpose()
                .map_err(|_| "Invalid timeout")?
                .unwrap_or(15);
            if seconds == 0 || seconds > 300 {
                return Err("Timeout must be 1..300 seconds".into());
            }
            if args[1].trim().is_empty()
                || args[2].trim().is_empty()
                || args[1] == args[2]
                || args[1].split_whitespace().count() != 1
                || args[2].split_whitespace().count() != 1
            {
                return Err(
                    "Target and expected must be different nonempty single OCR words".into(),
                );
            }
            Duration::from_secs(seconds)
        }
        _ => return Err("Invalid arguments; use --help".into()),
    };
    let _input_guard = if matches!(command, "observe" | "compare-ocr") {
        None
    } else {
        Some(windows_control::execution::InputGuard::acquire(
            &std::env::temp_dir().join("windows-control-hid.lock"),
        )?)
    };
    let (width, height) = ScreenCapture::detect_resolution("/dev/video0")?;
    let camera = ScreenCapture::new("/dev/video0", width as u32, height as u32);
    if command == "compare-ocr" {
        let directory = std::path::Path::new(&args[1]);
        std::fs::create_dir(directory).map_err(|e| e.to_string())?;
        let raw = camera.capture_raw()?;
        let path = directory.join("raw.jpg");
        std::fs::write(&path, &raw).map_err(|e| e.to_string())?;
        return ocr::compare_input_methods(&path, &raw);
    }
    let start = Instant::now();
    let frame_id = std::cell::Cell::new(0u64);
    let observe = || {
        let captured_at = start.elapsed();
        let raw = camera.capture_raw()?;
        if command == "observe" && args.len() >= 2 {
            image::load_from_memory(&raw)
                .map_err(|e| e.to_string())?
                .save(&args[1])
                .map_err(|e| e.to_string())?;
        }
        let boxes = ocr::parse_tsv(&ocr::run_ocr_from_memory(&raw)?)?;
        if boxes.iter().any(|b| {
            u32::from(b.x) + u32::from(b.w) > u32::from(width)
                || u32::from(b.y) + u32::from(b.h) > u32::from(height)
        }) {
            return Err("OCR coordinates outside capture".into());
        }
        frame_id.set(frame_id.get() + 1);
        Ok(validation::Observation {
            frame_id: frame_id.get(),
            captured_at,
            width,
            height,
            boxes,
        })
    };
    if command == "observe" {
        let boxes = observe()?.boxes;
        if let Some(target) = args.get(2) {
            let candidates: Vec<_> = boxes.iter().filter(|b| b.text == *target).collect();
            println!(
                "TARGET_DIAG exact={} qualified={}",
                candidates.len(),
                candidates.iter().filter(|b| b.confidence >= 70.0).count()
            );
            for b in candidates {
                println!(
                    "TARGET_BOX confidence={:.2} x={} y={} w={} h={}",
                    b.confidence, b.x, b.y, b.w, b.h
                );
            }
        }
        println!(
            "OBSERVED words={} resolution={}x{} elapsed_ms={}",
            boxes.len(),
            width,
            height,
            start.elapsed().as_millis()
        );
    } else {
        let mut hid = HidController::new(width, height, "/dev/hidg0", "/dev/hidg1");
        if command == "click-point" {
            let x = args[1].parse::<u16>().map_err(|_| "Invalid x coordinate")?;
            let y = args[2].parse::<u16>().map_err(|_| "Invalid y coordinate")?;
            hid.click_at(x, y, 1)?;
            println!("INPUT_SENT; recapture and verify the result before another action");
            return Ok(());
        } else if command == "open-browser" {
            task::open_browser_with_hid(&mut hid)?;
            println!("INPUT_SENT; recapture and verify browser before focus-address");
            return Ok(());
        } else if command == "focus-address" {
            hid.send_key(
                windows_control::helper::MOD_CTRL,
                windows_control::helper::key_char('l').unwrap(),
                0,
            )?;
            println!("INPUT_SENT; recapture and verify address focus before local-app");
            return Ok(());
        }
        let mut policy = validation::Policy::click(
            if command == "click" { &args[1] } else { "" },
            if command == "local-app" {
                "SUCCESSMODE"
            } else {
                &args[2]
            },
        );
        if command != "click" {
            policy.target = None;
        }
        policy.timeout = timeout;
        let report = validation::execute_observed(
            observe,
            |target| match command {
                "local-app" => task::type_local_test_app(&mut hid),
                "save-page" => task::save_page_locally(&mut hid, &args[1]),
                "open-url" => task::open_url_with_hid(&mut hid, &args[1]),
                "click" => {
                    let (x, y) = target.expect("click policy has target").center();
                    hid.click_at(x, y, 1)
                }
                _ => unreachable!(),
            },
            || start.elapsed(),
            std::thread::sleep,
            &policy,
        );
        windows_control::replay::print_report(&report, "live");
        if report.reason != validation::Reason::Validated {
            return Err(format!(
                "Execution did not validate: {}",
                report.reason.code()
            ));
        }
        println!("VALIDATED elapsed_ms={}", start.elapsed().as_millis());
    }
    Ok(())
}

fn usage() {
    println!(
        "windows_control explore-browser <new-directory> <max-probes>\n  Disposable headless browser: screenshots/OCR only; no Pi or HID. Run in test container.\n"
    );
    println!(
        "windows_control explore-test-web <new-directory> <max-probes>\n  REAL HID exploration of the repository test page only. Open/verify success READY first.\n"
    );
    println!(
        "windows_control explore-demo <new-directory> [scenario]\n  Synthetic UI exploration, graph and partial-failure artifacts. No hardware.\n"
    );
    println!(
        "windows_control replay <scenario.replay>\n  Offline OCR replay and hypothetical input report. No camera, OCR process or HID.\n"
    );
    println!(
        "windows_control click-point <x> <y>\n  Explicit visual target on the latest capture; recapture after this single click.\n"
    );
    println!(
        "windows_control open-browser\n  Launch Edge using the existing HID path; recapture before the next action.\n"
    );
    println!(
        "windows_control verify-jpeg <preserved-raw.jpg>\n  Offline JPEG marker/decode check and lossless PPM OCR control. No HID.\n"
    );
    println!(
        "windows_control compare-ocr <new-output-directory>\n  Captures once; compares unmodified JPEG file input and stdin input. No HID.\n"
    );
    println!(
        "windows_control focus-address\nwindows_control local-app\n  Open Edge first; focus-address, recapture/verify focus, then local-app.\n  Runs the embedded HTML locally as a data URL; no web server is used.\n"
    );
    println!(
        "windows_control observe\nwindows_control click <target-word> <expected-new-word> [timeout-seconds]\nwindows_control open-url <url> <expected-new-word> [timeout-seconds]\nwindows_control save-page <new-absolute-html-path> <expected-new-word> [timeout-seconds]\n\nobserve captures/OCRs without HID. Actions execute once, then validate a new screen.\nMatching is exact, case-sensitive, confidence >= 70. Never retries the action."
    );
}
