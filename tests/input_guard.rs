use windows_control::execution::InputGuard;

#[test]
fn competing_input_is_blocked_and_guard_releases_ownership() {
    let path =
        std::env::temp_dir().join(format!("windows-control-guard-{}.lock", std::process::id()));
    let first = InputGuard::acquire(&path).unwrap();
    assert!(InputGuard::acquire(&path).is_err());
    drop(first);
    let second = InputGuard::acquire(&path).unwrap();
    drop(second);
    assert!(!path.exists());
}
