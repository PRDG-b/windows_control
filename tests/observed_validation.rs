use std::{cell::Cell, time::Duration};
use windows_control::{
    ocr::TextBox,
    validation::{Observation, Policy, Reason, execute_observed},
};

fn frame(id: u64, at: u64, words: &[(&str, f32)]) -> Observation {
    Observation {
        frame_id: id,
        captured_at: Duration::from_millis(at),
        width: 800,
        height: 600,
        boxes: words
            .iter()
            .map(|(text, confidence)| TextBox {
                text: (*text).into(),
                confidence: *confidence,
                x: 20,
                y: 30,
                w: 80,
                h: 20,
            })
            .collect(),
    }
}

fn run(
    frames: Vec<(u64, Result<Observation, String>)>,
    fail_action: bool,
) -> windows_control::validation::Report {
    let clock = Cell::new(Duration::ZERO);
    let mut frames = frames.into_iter();
    execute_observed(
        || {
            let (at, result) = frames.next().unwrap_or((100, Err("end".into())));
            clock.set(Duration::from_millis(at));
            result
        },
        |_| {
            if fail_action {
                Err("write failed".into())
            } else {
                Ok(())
            }
        },
        || clock.get(),
        |_| {},
        &Policy {
            timeout: Duration::from_millis(100),
            max_age: Duration::from_millis(20),
            ..Policy::click("RUN", "COMPLETED")
        },
    )
}

#[test]
fn success_requires_fresh_postcondition_and_only_one_action() {
    let report = run(
        vec![
            (0, Ok(frame(1, 0, &[("RUN", 95.)]))),
            (10, Ok(frame(2, 10, &[("WAITING", 95.)]))),
            (20, Ok(frame(3, 20, &[("COMPLETED", 95.)]))),
        ],
        false,
    );
    assert_eq!(report.reason, Reason::Validated);
    assert_eq!(report.actions, 1);
    assert_eq!(report.click, Some((60, 40)));
    assert_eq!(report.frames.len(), 3);
}

#[test]
fn invalid_before_frames_never_send_input() {
    let mut outside = frame(1, 0, &[("RUN", 95.)]);
    outside.width = 30;
    for (at, observation, reason) in [
        (30, frame(1, 0, &[("RUN", 95.)]), Reason::StaleFrame),
        (0, frame(1, 1, &[("RUN", 95.)]), Reason::InvalidFrame),
        (0, outside, Reason::InvalidFrame),
        (
            0,
            frame(1, 0, &[("RUN", 95.), ("RUN", 95.)]),
            Reason::AmbiguousTarget,
        ),
        (0, frame(1, 0, &[("RUN", 20.)]), Reason::LowConfidenceTarget),
        (0, frame(1, 0, &[("OTHER", 95.)]), Reason::MissingTarget),
        (
            0,
            frame(1, 0, &[("RUN", 95.), ("FAILED", 20.)]),
            Reason::UncertainState,
        ),
        (
            0,
            frame(1, 0, &[("RUN", 95.), ("COMPLETED", 20.)]),
            Reason::AlreadyPresent,
        ),
    ] {
        let report = run(vec![(at, Ok(observation))], false);
        assert_eq!(report.reason, reason);
        assert_eq!(report.actions, 0);
    }
}

#[test]
fn unsafe_after_frames_and_failures_are_not_success() {
    for (at, after, reason) in [
        (
            10,
            Ok(frame(1, 10, &[("COMPLETED", 95.)])),
            Reason::StaleFrame,
        ),
        (
            30,
            Ok(frame(2, 0, &[("COMPLETED", 95.)])),
            Reason::StaleFrame,
        ),
        (
            100,
            Ok(frame(2, 100, &[("COMPLETED", 95.)])),
            Reason::Timeout,
        ),
        (
            10,
            Ok(frame(2, 10, &[("FAILED", 95.), ("COMPLETED", 95.)])),
            Reason::FailureObserved,
        ),
        (10, Err("camera lost".into()), Reason::ObservationError),
    ] {
        let report = run(
            vec![(0, Ok(frame(1, 0, &[("RUN", 95.)]))), (at, after)],
            false,
        );
        assert_eq!(report.reason, reason);
        assert_eq!(report.actions, 1);
    }
    let report = run(vec![(0, Ok(frame(1, 0, &[("RUN", 95.)])))], true);
    assert_eq!(report.reason, Reason::ActionError);
    assert_eq!(report.actions, 1);
}

#[test]
fn compound_condition_requires_all_words_and_absence_of_forbidden_text() {
    let clock = Cell::new(Duration::ZERO);
    let mut frames = [
        frame(1, 0, &[("RUN", 95.)]),
        frame(2, 10, &[("COMPLETED", 95.)]),
        frame(3, 20, &[("COMPLETED", 95.), ("SAVED", 95.), ("ERROR", 20.)]),
        frame(4, 30, &[("COMPLETED", 95.), ("SAVED", 95.)]),
    ]
    .into_iter();
    let mut policy = Policy::click("RUN", "COMPLETED");
    policy.required.push("SAVED".into());
    policy.forbidden.push("ERROR".into());
    let report = execute_observed(
        || {
            let f = frames.next().unwrap();
            clock.set(f.captured_at);
            Ok(f)
        },
        |_| Ok(()),
        || clock.get(),
        |_| {},
        &policy,
    );
    assert_eq!(report.reason, Reason::Validated);
    assert_eq!(report.frames.len(), 4);
}

#[test]
fn postcondition_captured_during_action_is_rejected() {
    let clock = Cell::new(Duration::ZERO);
    let mut frames = [
        frame(1, 0, &[("RUN", 95.)]),
        frame(2, 5, &[("COMPLETED", 95.)]),
    ]
    .into_iter();
    let report = execute_observed(
        || {
            let f = frames.next().unwrap();
            if f.frame_id == 2 {
                clock.set(Duration::from_millis(15));
            }
            Ok(f)
        },
        |_| {
            clock.set(Duration::from_millis(10));
            Ok(())
        },
        || clock.get(),
        |_| {},
        &Policy::click("RUN", "COMPLETED"),
    );
    assert_eq!(report.reason, Reason::StaleFrame);
}

#[test]
fn reports_late_observation_and_time_spent_in_failed_action() {
    let report = run(
        vec![
            (0, Ok(frame(1, 0, &[("RUN", 95.)]))),
            (100, Ok(frame(2, 100, &[("COMPLETED", 95.)]))),
        ],
        false,
    );
    assert_eq!(report.reason, Reason::Timeout);
    assert_eq!(report.frames.len(), 2);
    let clock = Cell::new(Duration::ZERO);
    let report = execute_observed(
        || Ok(frame(1, 0, &[("RUN", 95.)])),
        |_| {
            clock.set(Duration::from_millis(50));
            Err("failure".into())
        },
        || clock.get(),
        |_| {},
        &Policy::click("RUN", "COMPLETED"),
    );
    assert_eq!(report.reason, Reason::ActionError);
    assert_eq!(report.elapsed, Duration::from_millis(50));
}

#[test]
fn invalid_policy_and_reversed_clock_never_validate() {
    for mutate in [0, 1, 2, 3] {
        let mut policy = Policy::click("RUN", "COMPLETED");
        match mutate {
            0 => policy.timeout = Duration::ZERO,
            1 => policy.max_age = Duration::ZERO,
            2 => policy.required.clear(),
            _ => policy.forbidden.push("COMPLETED".into()),
        }
        let report = execute_observed(
            || panic!("must not observe"),
            |_| panic!("must not act"),
            || Duration::ZERO,
            |_| {},
            &policy,
        );
        assert_eq!(report.reason, Reason::InvalidPolicy);
        assert_eq!(report.actions, 0);
    }
    let report = run(
        vec![
            (10, Ok(frame(1, 10, &[("RUN", 95.)]))),
            (5, Ok(frame(2, 5, &[("COMPLETED", 95.)]))),
        ],
        false,
    );
    assert_eq!(report.reason, Reason::InvalidFrame);
    assert_eq!(report.actions, 1);
}

#[test]
fn explicitly_expected_failure_word_can_be_verified() {
    let clock = Cell::new(Duration::ZERO);
    let mut frames = [
        frame(1, 0, &[("RUN", 95.)]),
        frame(2, 10, &[("FAILED", 95.)]),
    ]
    .into_iter();
    let report = execute_observed(
        || {
            let f = frames.next().unwrap();
            clock.set(f.captured_at);
            Ok(f)
        },
        |_| Ok(()),
        || clock.get(),
        |_| {},
        &Policy::click("RUN", "FAILED"),
    );
    assert_eq!(report.reason, Reason::Validated);
}
