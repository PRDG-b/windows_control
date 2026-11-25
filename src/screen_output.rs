//! Private screenshot artifacts and metadata without OCR text or value hashes.
use crate::screen::{Analysis, ElementKind};
use image::{Rgb, RgbImage};
use std::{fmt::Write as _, io::Write as _, path::Path};

pub fn write_analysis(dir: &Path, image: &RgbImage, analysis: &Analysis) -> Result<(), String> {
    let id = analysis.scene.observation.frame_id;
    let mut overlay = image.clone();
    let mut elements = String::from("object\tkind\tx\ty\tw\th\tboundary_score\tpermission\n");
    for e in &analysis.elements {
        let permission = analysis
            .scene
            .candidates
            .iter()
            .find(|c| c.key == format!("{}_click", e.key))
            .map(|c| format!("{:?}", c.permission))
            .unwrap_or("not_actionable".into());
        writeln!(
            elements,
            "{}\t{:?}\t{}\t{}\t{}\t{}\t{:.3}\t{}",
            e.key, e.kind, e.bounds[0], e.bounds[1], e.bounds[2], e.bounds[3], e.score, permission
        )
        .unwrap();
        let color = Rgb(if e.kind == ElementKind::RectControl {
            [0, 180, 0]
        } else {
            [200, 140, 0]
        });
        let [x, y, w, h] = e.bounds.map(u32::from);
        for xx in x..x + w {
            overlay.put_pixel(xx, y, color);
            overlay.put_pixel(xx, y + h - 1, color);
        }
        for yy in y..y + h {
            overlay.put_pixel(x, yy, color);
            overlay.put_pixel(x + w - 1, yy, color);
        }
    }
    std::fs::write(dir.join(format!("ui-{id:04}.tsv")), elements)
        .map_err(|_| "Cannot write UI elements")?;
    overlay
        .save(dir.join(format!("ui-{id:04}.jpg")))
        .map_err(|_| "Cannot write UI overlay")?;
    let path = dir.join("observations.tsv");
    let exists = path.exists();
    let mut journal = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .map_err(|_| "Cannot write observation journal")?;
    if !exists {
        writeln!(
            journal,
            "frame\tcaptured_ms\tstate\tlayout\tdata_revision\tstable\tquality\telements"
        )
        .map_err(|_| "Cannot write journal header")?;
    }
    writeln!(
        journal,
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        id,
        analysis.scene.observation.captured_at.as_millis(),
        analysis.scene.state.as_deref().unwrap_or("unknown"),
        analysis.layout_id.as_deref().unwrap_or("unknown"),
        analysis
            .data_revision
            .map(|r| r.to_string())
            .unwrap_or_default(),
        analysis.stable,
        analysis.quality,
        analysis.elements.len()
    )
    .map_err(|_| "Cannot write observation journal")?;
    Ok(())
}
