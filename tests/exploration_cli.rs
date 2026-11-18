use std::{fs, process::Command};
#[test]
fn demo_exports_graph_and_preserves_partial_failure_without_devices() {
    let root = std::env::temp_dir().join(format!("windows-control-explore-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    for (scenario, reason) in [
        ("normal", "complete"),
        ("unknown", "unknown_state"),
        ("restore-error", "restore_mismatch"),
        ("input-error", "input_error"),
    ] {
        let output_dir = root.join(scenario);
        let result = Command::new(env!("CARGO_BIN_EXE_windows_control"))
            .arg("explore-demo")
            .arg(&output_dir)
            .arg(scenario)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert_eq!(result.status.success(), scenario == "normal", "{stdout}");
        let report = fs::read_to_string(output_dir.join("report.json"))
            .expect("partial report must persist");
        assert!(
            report.contains(&format!("\"stop\":\"{reason}\"")),
            "{report}"
        );
        assert!(output_dir.join("graph.dot").is_file());
        assert!(output_dir.join("graph.mmd").is_file());
        assert!(output_dir.join("attempts.tsv").is_file());
        assert!(output_dir.join("pending.tsv").is_file());
        let repeat = Command::new(env!("CARGO_BIN_EXE_windows_control"))
            .arg("explore-demo")
            .arg(&output_dir)
            .output()
            .unwrap();
        assert!(!repeat.status.success(), "must not overwrite evidence");
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_exploration_rejects_invalid_budget_before_opening_devices() {
    let output = Command::new(env!("CARGO_BIN_EXE_windows_control"))
        .args(["explore-test-web", "unused-output", "0"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("max-probes must be 1..30"), "{stderr}");
    assert!(!stderr.contains("video0"));
}
