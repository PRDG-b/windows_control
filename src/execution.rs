//! Hardware-independent execution and observation validation.
use crate::ocr::TextBox;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Observation {
    pub frame_id: u64,
    pub captured_at: Duration,
    pub width: u16,
    pub height: u16,
    pub boxes: Vec<TextBox>,
}

#[derive(Debug, Clone)]
pub struct Policy {
    pub target: Option<String>,
    pub required: Vec<String>,
    pub forbidden: Vec<String>,
    pub timeout: Duration,
    pub max_age: Duration,
    pub poll_interval: Duration,
}

impl Policy {
    pub fn click(target: &str, expected: &str) -> Self {
        Self {
            target: Some(target.into()),
            required: vec![expected.into()],
            forbidden: if expected == "FAILED" {
                vec![]
            } else {
                vec!["FAILED".into()]
            },
            timeout: Duration::from_secs(15),
            max_age: Duration::from_secs(30),
            poll_interval: Duration::from_millis(200),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    Validated,
    InvalidPolicy,
    InvalidFrame,
    StaleFrame,
    AlreadyPresent,
    MissingTarget,
    LowConfidenceTarget,
    AmbiguousTarget,
    FailureObserved,
    UncertainState,
    Timeout,
    ObservationError,
    ActionError,
}

impl Reason {
    pub fn code(self) -> &'static str {
        match self {
            Self::Validated => "validated",
            Self::InvalidPolicy => "invalid_policy",
            Self::InvalidFrame => "invalid_frame",
            Self::StaleFrame => "stale_frame",
            Self::AlreadyPresent => "already_present",
            Self::MissingTarget => "missing_target",
            Self::LowConfidenceTarget => "low_confidence_target",
            Self::AmbiguousTarget => "ambiguous_target",
            Self::FailureObserved => "failure_observed",
            Self::UncertainState => "uncertain_state",
            Self::Timeout => "timeout",
            Self::ObservationError => "observation_error",
            Self::ActionError => "action_error",
        }
    }
}

/// Evidence deliberately excludes OCR text.
#[derive(Debug)]
pub struct FrameEvidence {
    pub frame_id: u64,
    pub captured_at: Duration,
    pub observed_at: Duration,
    pub words: usize,
}

#[derive(Debug)]
pub struct Report {
    pub reason: Reason,
    /// Attempted actions, including a failed/partially transmitted input.
    pub actions: usize,
    pub click: Option<(u16, u16)>,
    pub frames: Vec<FrameEvidence>,
    pub elapsed: Duration,
    pub detail: Option<String>,
}

impl Report {
    pub fn result(&self) -> Result<(), String> {
        if self.reason == Reason::Validated {
            return Ok(());
        }
        if let Some(detail) = &self.detail {
            return Err(detail.clone());
        }
        Err(match self.reason {
            Reason::AlreadyPresent => "Postcondition already present; action not executed",
            Reason::Timeout => "Timeout: postcondition not observed; action was not retried",
            Reason::MissingTarget | Reason::LowConfidenceTarget | Reason::AmbiguousTarget => {
                "Target missing, low confidence or ambiguous"
            }
            _ => self.reason.code(),
        }
        .into())
    }
}

/// `now` is monotonic elapsed time, shared with capture timestamps. Replay supplies
/// a virtual clock; live execution supplies Instant::elapsed. No action is retried.
pub fn execute_observed(
    mut observe: impl FnMut() -> Result<Observation, String>,
    mut action: impl FnMut(Option<&TextBox>) -> Result<(), String>,
    now: impl Fn() -> Duration,
    mut pause: impl FnMut(Duration),
    policy: &Policy,
) -> Report {
    let mut report = Report {
        reason: Reason::InvalidPolicy,
        actions: 0,
        click: None,
        frames: vec![],
        elapsed: Duration::ZERO,
        detail: None,
    };
    let valid_word =
        |s: &String| !s.is_empty() && s.split_whitespace().count() == 1 && s.trim() == s;
    if policy.timeout.is_zero()
        || policy.max_age.is_zero()
        || policy.required.is_empty()
        || !policy
            .required
            .iter()
            .chain(&policy.forbidden)
            .all(valid_word)
        || policy.required.iter().any(|s| policy.forbidden.contains(s))
        || policy
            .target
            .as_ref()
            .is_some_and(|s| !valid_word(s) || policy.required.contains(s))
    {
        return report;
    }
    let qualified = |frame: &Observation, word: &str| {
        frame
            .boxes
            .iter()
            .any(|b| b.text == word && b.confidence >= 70.0)
    };
    let mut previous: Option<(u64, Duration)> = None;
    let mut last_now = Duration::ZERO;
    let mut action_completed_at = None;
    loop {
        let began = now();
        report.elapsed = began;
        if began < last_now {
            report.reason = Reason::InvalidFrame;
            break;
        }
        if began >= policy.timeout {
            report.reason = Reason::Timeout;
            break;
        }
        let observation = observe();
        let observed_at = now();
        report.elapsed = observed_at;
        if observed_at < began {
            report.reason = Reason::InvalidFrame;
            break;
        }
        last_now = observed_at;
        if let Ok(frame) = &observation {
            report.frames.push(FrameEvidence {
                frame_id: frame.frame_id,
                captured_at: frame.captured_at,
                observed_at,
                words: frame.boxes.len(),
            });
        }
        if observed_at >= policy.timeout {
            report.reason = Reason::Timeout;
            break;
        }
        let frame = match observation {
            Ok(f) => f,
            Err(e) => {
                report.reason = Reason::ObservationError;
                report.detail = Some(e);
                break;
            }
        };
        if let Err(reason) = validate_frame(
            &frame,
            observed_at,
            previous,
            action_completed_at,
            policy.max_age,
        ) {
            report.reason = reason;
            break;
        }
        previous = Some((frame.frame_id, frame.captured_at));
        if policy.forbidden.iter().any(|s| qualified(&frame, s)) {
            report.reason = Reason::FailureObserved;
            break;
        }
        if report.actions == 0 {
            if policy
                .forbidden
                .iter()
                .any(|s| frame.boxes.iter().any(|b| b.text == *s))
            {
                report.reason = Reason::UncertainState;
                break;
            }
            if policy
                .required
                .iter()
                .any(|s| frame.boxes.iter().any(|b| b.text == *s))
            {
                report.reason = Reason::AlreadyPresent;
                break;
            }
            let target = if let Some(word) = &policy.target {
                let matches: Vec<_> = frame.boxes.iter().filter(|b| b.text == *word).collect();
                // A second low-confidence box is still an ambiguous target.
                if matches.len() > 1 {
                    report.reason = Reason::AmbiguousTarget;
                    break;
                }
                let Some(target) = matches.first().copied() else {
                    report.reason = Reason::MissingTarget;
                    break;
                };
                if target.confidence < 70.0 {
                    report.reason = Reason::LowConfidenceTarget;
                    break;
                }
                report.click = Some(target.center());
                Some(target)
            } else {
                None
            };
            report.actions = 1;
            let action_result = action(target);
            report.elapsed = now();
            if let Err(e) = action_result {
                report.reason = Reason::ActionError;
                report.detail = Some(e);
                break;
            }
            if report.elapsed < observed_at {
                report.reason = Reason::InvalidFrame;
                break;
            }
            last_now = report.elapsed;
            action_completed_at = Some(report.elapsed);
        } else if policy.required.iter().all(|s| qualified(&frame, s))
            && !policy
                .forbidden
                .iter()
                .any(|s| frame.boxes.iter().any(|b| b.text == *s))
        {
            report.reason = Reason::Validated;
            break;
        } else {
            pause(
                policy
                    .poll_interval
                    .min(policy.timeout.saturating_sub(observed_at)),
            );
        }
    }
    report
}

/// One CLI process owns the HID input stream. Never auto-removes a stale lock:
/// a crashed owner's uncertain input must be investigated before resuming.
pub struct InputGuard {
    path: std::path::PathBuf,
    file: Option<std::fs::File>,
}

impl InputGuard {
    pub fn acquire(path: &std::path::Path) -> Result<Self, String> {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|_| {
                "HID input lock unavailable; another command or stale lock exists".to_string()
            })?;
        Ok(Self {
            path: path.into(),
            file: Some(file),
        })
    }
}

impl Drop for InputGuard {
    fn drop(&mut self) {
        drop(self.file.take());
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Shared frame validity/freshness checks for execution and exploration.
pub fn validate_frame(
    frame: &Observation,
    observed_at: Duration,
    previous: Option<(u64, Duration)>,
    after: Option<Duration>,
    max_age: Duration,
) -> Result<(), Reason> {
    if frame.frame_id == 0
        || frame.width == 0
        || frame.height == 0
        || frame.captured_at > observed_at
        || frame.boxes.iter().any(|b| {
            b.w == 0
                || b.h == 0
                || !b.confidence.is_finite()
                || !(0.0..=100.0).contains(&b.confidence)
                || u32::from(b.x) + u32::from(b.w) > u32::from(frame.width)
                || u32::from(b.y) + u32::from(b.h) > u32::from(frame.height)
        })
    {
        return Err(Reason::InvalidFrame);
    }
    if observed_at.saturating_sub(frame.captured_at) > max_age
        || after.is_some_and(|at| frame.captured_at < at)
        || previous.is_some_and(|(id, at)| frame.frame_id <= id || frame.captured_at <= at)
    {
        return Err(Reason::StaleFrame);
    }
    Ok(())
}
