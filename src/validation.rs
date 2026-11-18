#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn frame(text: &str) -> Vec<crate::ocr::TextBox> {
        vec![crate::ocr::TextBox {
            text: text.into(),
            x: 10,
            y: 10,
            w: 40,
            h: 20,
            confidence: 95.0,
        }]
    }

    #[test]
    fn action_runs_once_and_requires_new_observation() {
        let mut frames = vec![frame("RUN"), frame("WAIT"), frame("DONE")].into_iter();
        let mut clicks = 0;
        let result = execute_and_validate(
            || Ok(frames.next().unwrap()),
            |_| {
                clicks += 1;
                Ok(())
            },
            "RUN",
            "DONE",
            Duration::from_secs(1),
            Duration::ZERO,
        );
        assert!(result.is_ok());
        assert_eq!(clicks, 1);
    }

    #[test]
    fn already_done_and_ambiguous_targets_do_not_execute() {
        for boxes in [
            frame("DONE"),
            vec![frame("RUN").remove(0), frame("RUN").remove(0)],
        ] {
            let mut clicks = 0;
            assert!(
                execute_and_validate(
                    || Ok(boxes.clone()),
                    |_| {
                        clicks += 1;
                        Ok(())
                    },
                    "RUN",
                    "DONE",
                    Duration::from_secs(1),
                    Duration::ZERO
                )
                .is_err()
            );
            assert_eq!(clicks, 0);
        }
    }

    #[test]
    fn timeout_and_observation_failure_are_not_success() {
        assert!(
            execute_and_validate(
                || Ok(frame("RUN")),
                |_| Ok(()),
                "RUN",
                "DONE",
                Duration::from_millis(1),
                Duration::ZERO
            )
            .unwrap_err()
            .contains("Timeout")
        );
        let mut count = 0;
        let error = execute_and_validate(
            || {
                count += 1;
                if count == 1 {
                    Ok(frame("RUN"))
                } else {
                    Err("camera disconnected".into())
                }
            },
            |_| Ok(()),
            "RUN",
            "DONE",
            Duration::from_secs(1),
            Duration::ZERO,
        )
        .unwrap_err();
        assert!(error.contains("camera disconnected"));
    }

    #[test]
    fn low_confidence_and_invalid_deadline_do_not_execute() {
        let mut boxes = frame("RUN");
        boxes[0].confidence = 20.0;
        assert!(
            execute_and_validate(
                || Ok(boxes.clone()),
                |_| panic!("must not click"),
                "RUN",
                "DONE",
                Duration::from_secs(1),
                Duration::ZERO
            )
            .is_err()
        );
        assert!(
            execute_and_validate(
                || panic!("must not observe"),
                |_| panic!("must not click"),
                "RUN",
                "DONE",
                Duration::ZERO,
                Duration::ZERO
            )
            .is_err()
        );
    }

    #[test]
    fn action_error_does_not_retry_or_observe_again() {
        let mut observations = 0;
        let mut actions = 0;
        let error = execute_and_validate(
            || {
                observations += 1;
                Ok(frame("RUN"))
            },
            |_| {
                actions += 1;
                Err("HID write failed".into())
            },
            "RUN",
            "DONE",
            Duration::from_secs(1),
            Duration::ZERO,
        )
        .unwrap_err();
        assert_eq!(error, "HID write failed");
        assert_eq!((observations, actions), (1, 1));
    }

    #[test]
    fn low_confidence_existing_postcondition_is_unknown_not_absent() {
        let mut boxes = frame("RUN");
        let mut done = frame("DONE").remove(0);
        done.confidence = 60.0;
        boxes.push(done);
        assert!(
            execute_and_validate(
                || Ok(boxes.clone()),
                |_| panic!("must not execute on ambiguous prior state"),
                "RUN",
                "DONE",
                Duration::from_secs(1),
                Duration::ZERO
            )
            .is_err()
        );
    }

    #[test]
    fn launch_action_is_verified_on_new_observation() {
        let mut frames = [frame("DESKTOP"), frame("READY")].into_iter();
        let mut launched = 0;
        perform_and_validate(
            || Ok(frames.next().unwrap()),
            |_| {
                launched += 1;
                Ok(())
            },
            "READY",
            Duration::from_secs(1),
            Duration::ZERO,
        )
        .unwrap();
        assert_eq!(launched, 1);
    }
}
pub use crate::execution::{Observation, Policy, Reason, Report, execute_observed};
use crate::ocr::TextBox;
use std::time::{Duration, Instant};

/// Compatibility API for callers without capture metadata. Live CLI uses
/// execute_observed directly; this adapter timestamps acquisition conservatively.
pub fn execute_and_validate(
    observe: impl FnMut() -> Result<Vec<TextBox>, String>,
    mut action: impl FnMut(&TextBox) -> Result<(), String>,
    target: &str,
    expected: &str,
    timeout: Duration,
    poll_interval: Duration,
) -> Result<(), String> {
    let mut policy = Policy::click(target, expected);
    policy.timeout = timeout;
    policy.poll_interval = poll_interval;
    legacy_run(
        observe,
        |target| action(target.expect("click policy has target")),
        policy,
    )
}

pub fn perform_and_validate(
    observe: impl FnMut() -> Result<Vec<TextBox>, String>,
    mut action: impl FnMut(&[TextBox]) -> Result<(), String>,
    expected: &str,
    timeout: Duration,
    poll_interval: Duration,
) -> Result<(), String> {
    // Preserve the launch callback's pre-observation without exposing another engine.
    let before = std::cell::RefCell::new(Vec::new());
    let mut observe = observe;
    let mut policy = Policy::click("", expected);
    policy.target = None;
    policy.timeout = timeout;
    policy.poll_interval = poll_interval;
    legacy_run(
        || {
            let boxes = observe()?;
            *before.borrow_mut() = boxes.clone();
            Ok(boxes)
        },
        |_| action(&before.borrow()),
        policy,
    )
}

fn legacy_run(
    mut observe: impl FnMut() -> Result<Vec<TextBox>, String>,
    action: impl FnMut(Option<&TextBox>) -> Result<(), String>,
    policy: Policy,
) -> Result<(), String> {
    let start = Instant::now();
    let mut frame_id = 0;
    execute_observed(
        || {
            let captured_at = start.elapsed();
            let boxes = observe()?;
            frame_id += 1;
            Ok(Observation {
                frame_id,
                captured_at,
                width: u16::MAX,
                height: u16::MAX,
                boxes,
            })
        },
        action,
        || start.elapsed(),
        std::thread::sleep,
        &policy,
    )
    .result()
}
