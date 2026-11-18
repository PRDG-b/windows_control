use std::time::Duration;
use windows_control::{ocr::parse_tsv, validation::execute_and_validate};

#[test]
fn replay_click_and_validate_from_tsv() {
    let mut frames = [
        include_str!("fixtures/ready.tsv"),
        include_str!("fixtures/completed.tsv"),
    ]
    .into_iter();
    let mut clicks = Vec::new();
    execute_and_validate(
        || parse_tsv(frames.next().unwrap()),
        |target| {
            clicks.push(target.center());
            Ok(())
        },
        "RUN",
        "COMPLETED",
        Duration::from_secs(1),
        Duration::ZERO,
    )
    .unwrap();
    assert_eq!(clicks, vec![(140, 220)]);
}
