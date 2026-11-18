use std::{fs, process::Command};

#[test]
fn bundled_replays_have_expected_outcomes_and_action_counts() {
    for (name, reason, actions) in [
        ("success", "validated", 1),
        ("compound", "validated", 1),
        ("failure", "failure_observed", 1),
        ("duplicate", "ambiguous_target", 0),
        ("low-confidence", "low_confidence_target", 0),
        ("missing-target", "missing_target", 0),
        ("already-completed", "already_present", 0),
        ("stale", "stale_frame", 1),
        ("old-capture", "stale_frame", 1),
        ("timeout", "timeout", 1),
        ("ocr-error", "observation_error", 1),
        ("exhausted", "observation_error", 1),
        ("uncertain", "uncertain_state", 0),
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(format!("{name}.replay"));
        let output = Command::new(env!("CARGO_BIN_EXE_windows_control"))
            .arg("replay")
            .arg(path)
            .output()
            .unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert_eq!(
            output.status.success(),
            reason == "validated",
            "{name}: {stdout}"
        );
        assert!(
            stdout.contains(&format!("\"reason\":\"{reason}\"")),
            "{name}: {stdout}"
        );
        assert!(
            stdout.contains(&format!("\"actions\":{actions}")),
            "{name}: {stdout}"
        );
        assert!(!stdout.contains("DO_NOT_LOG_FIXTURE_TEXT"));
    }
}

#[test]
fn replay_cli_runs_without_devices_and_reports_failures_without_ocr_text() {
    let root = std::env::temp_dir().join(format!("windows-control-replay-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("ready.tsv"), include_str!("fixtures/ready.tsv")).unwrap();
    fs::write(
        root.join("done.tsv"),
        include_str!("fixtures/completed.tsv"),
    )
    .unwrap();
    let cases = [
        (
            "success",
            "0 1 0 800 600 ready.tsv\n10 2 10 800 600 done.tsv",
            true,
            "validated",
            1,
        ),
        (
            "stale",
            "0 1 0 800 600 ready.tsv\n10 1 10 800 600 done.tsv",
            false,
            "stale_frame",
            1,
        ),
        (
            "timeout",
            "0 1 0 800 600 ready.tsv\n100 2 100 800 600 done.tsv",
            false,
            "timeout",
            1,
        ),
        (
            "missing",
            "0 1 0 800 600 absent.tsv",
            false,
            "observation_error",
            0,
        ),
        (
            "already",
            "0 1 0 800 600 done.tsv",
            false,
            "already_present",
            0,
        ),
    ];
    for (name, frames, success, reason, actions) in cases {
        let path = root.join(format!("{name}.replay"));
        fs::write(
            &path,
            format!("replay-v1 RUN COMPLETED FAILED 100 20\n{frames}\n"),
        )
        .unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_windows_control"))
            .arg("replay")
            .arg(path)
            .output()
            .unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert_eq!(output.status.success(), success, "{stdout}");
        assert!(
            stdout.contains(&format!("\"reason\":\"{reason}\"")),
            "{stdout}"
        );
        assert!(
            stdout.contains(&format!("\"actions\":{actions}")),
            "{stdout}"
        );
        assert!(
            !stdout.contains("RUN") && !stdout.contains("COMPLETED"),
            "{stdout}"
        );
        assert!(!String::from_utf8_lossy(&output.stderr).contains("video0"));
    }
    let path = root.join("escape.replay");
    fs::write(
        &path,
        "replay-v1 RUN COMPLETED FAILED 100 20\n0 1 0 800 600 ../outside.tsv",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_windows_control"))
        .arg("replay")
        .arg(path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Invalid replay"));
    fs::remove_dir_all(root).unwrap();
}
