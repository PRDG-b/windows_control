//! Replay OCR observations and record hypothetical inputs. Never opens hardware.
use crate::{
    execution::{Observation, Policy, Report, execute_observed},
    ocr::parse_tsv,
};
use std::{
    cell::Cell,
    path::{Component, Path, PathBuf},
    time::Duration,
};

struct Frame {
    at: Duration,
    id: u64,
    captured: Duration,
    width: u16,
    height: u16,
    path: PathBuf,
}

/// Header: replay-v1 TARGET REQUIRED[,WORD] FORBIDDEN[,WORD]|- TIMEOUT_MS MAX_AGE_MS
/// Frames: OBSERVED_MS FRAME_ID CAPTURED_MS WIDTH HEIGHT RELATIVE_TSV
pub fn run(path: &Path) -> Result<Report, String> {
    let data = std::fs::read_to_string(path).map_err(|_| "Cannot read replay file")?;
    let mut lines = data
        .lines()
        .filter(|s| !s.trim().is_empty() && !s.trim_start().starts_with('#'));
    let header: Vec<_> = lines
        .next()
        .ok_or("Invalid replay header")?
        .split_whitespace()
        .collect();
    if header.len() != 6 || header[0] != "replay-v1" {
        return Err("Invalid replay header".into());
    }
    let millis = |s: &str| {
        s.parse::<u64>()
            .map(Duration::from_millis)
            .map_err(|_| "Invalid replay time".to_string())
    };
    let mut policy = Policy::click(header[1], header[2]);
    policy.required = header[2].split(',').map(str::to_string).collect();
    policy.forbidden = if header[3] == "-" {
        vec![]
    } else {
        header[3].split(',').map(str::to_string).collect()
    };
    policy.timeout = millis(header[4])?;
    policy.max_age = millis(header[5])?;
    policy.poll_interval = Duration::ZERO;
    let root = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut frames = Vec::new();
    for line in lines {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() != 6 {
            return Err("Invalid replay frame".into());
        }
        let relative = Path::new(fields[5]);
        if relative.components().next().is_none()
            || relative
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err("Invalid replay relative path".into());
        }
        frames.push(Frame {
            at: millis(fields[0])?,
            id: fields[1].parse().map_err(|_| "Invalid replay frame id")?,
            captured: millis(fields[2])?,
            width: fields[3].parse().map_err(|_| "Invalid replay width")?,
            height: fields[4].parse().map_err(|_| "Invalid replay height")?,
            path: root.join(relative),
        });
    }
    if frames.is_empty() {
        return Err("Invalid replay: no frames".into());
    }
    let root = root
        .canonicalize()
        .map_err(|_| "Invalid replay directory")?;
    let clock = Cell::new(Duration::ZERO);
    let mut frames = frames.into_iter();
    Ok(execute_observed(
        || {
            let frame = frames
                .next()
                .ok_or("Replay exhausted before postcondition")?;
            clock.set(frame.at);
            let resolved = frame
                .path
                .canonicalize()
                .map_err(|_| "Cannot read replay observation")?;
            if !resolved.starts_with(&root) {
                return Err("Replay observation outside directory".into());
            }
            let tsv =
                std::fs::read_to_string(resolved).map_err(|_| "Cannot read replay observation")?;
            Ok(Observation {
                frame_id: frame.id,
                captured_at: frame.captured,
                width: frame.width,
                height: frame.height,
                boxes: parse_tsv(&tsv)?,
            })
        },
        |_| Ok(()),
        || clock.get(),
        |_| {},
        &policy,
    ))
}

/// Stable JSON report with no OCR text, filenames or raw error detail.
pub fn print_report(report: &Report, mode: &str) {
    let mode = if mode == "live" { "live" } else { "replay" };
    let state = match report.reason {
        crate::execution::Reason::Validated => "completed",
        crate::execution::Reason::FailureObserved => "failed",
        _ if report.actions == 0 => "blocked",
        _ => "unconfirmed",
    };
    let click = report
        .click
        .map(|(x, y)| format!("[{x},{y}]"))
        .unwrap_or("null".into());
    let frames = report
        .frames
        .iter()
        .map(|f| {
            format!(
                "{{\"frame_id\":{},\"captured_ms\":{},\"observed_ms\":{},\"words\":{}}}",
                f.frame_id,
                f.captured_at.as_millis(),
                f.observed_at.as_millis(),
                f.words
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "{{\"mode\":\"{mode}\",\"state\":\"{state}\",\"reason\":\"{}\",\"actions\":{},\"click\":{click},\"elapsed_ms\":{},\"frames\":[{frames}]}}",
        report.reason.code(),
        report.actions,
        report.elapsed.as_millis()
    );
}
