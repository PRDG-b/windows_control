use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Debug, Clone)]
pub struct TextBox {
    pub text: String,
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
    pub confidence: f32,
}

impl TextBox {
    pub fn center(&self) -> (u16, u16) {
        (self.x + self.w / 2, self.y + self.h / 2)
    }
}

pub fn parse_tsv(tsv: &str) -> Result<Vec<TextBox>, String> {
    let mut lines = tsv.lines();
    if lines.next()
        != Some(
            "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext",
        )
    {
        return Err("Invalid OCR TSV header".into());
    }
    let mut boxes = Vec::new();
    for line in lines.filter(|l| !l.trim().is_empty()) {
        let fields: Vec<_> = line.splitn(12, '\t').collect();
        if fields.len() != 12 {
            return Err("Malformed OCR TSV row".into());
        }
        if fields[0] != "5" || fields[11].trim().is_empty() {
            continue;
        }
        let number = |i: usize| {
            fields[i]
                .parse::<u16>()
                .map_err(|_| "Invalid OCR coordinates".to_string())
        };
        let (x, y, w, h) = (number(6)?, number(7)?, number(8)?, number(9)?);
        let confidence = fields[10]
            .parse::<f32>()
            .map_err(|_| "Invalid OCR confidence")?;
        if w == 0
            || h == 0
            || x.checked_add(w).is_none()
            || y.checked_add(h).is_none()
            || !confidence.is_finite()
            || !(0.0..=100.0).contains(&confidence)
        {
            return Err("Invalid OCR box or confidence".into());
        }
        boxes.push(TextBox {
            text: fields[11].trim().into(),
            x,
            y,
            w,
            h,
            confidence,
        });
    }
    Ok(boxes)
}

pub fn run_ocr_from_memory(jpeg_bytes: &[u8]) -> Result<String, String> {
    // stdin, stdout 파이프
    let mut child = Command::new("timeout")
        .args([
            "15s",
            "tesseract",
            "stdin",
            "stdout",
            "-l",
            "eng+kor",
            "--psm",
            "3",
            "tsv",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("tesseract execution failed: {}", e))?;

    // 메모리의 이미지 바이트를 스트림으로 전송
    if let Some(mut stdin) = child.stdin.take() {
        if let Err(error) = stdin.write_all(jpeg_bytes) {
            drop(stdin);
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("tesseract stdin streaming failed: {error}"));
        }
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("tesseract without output: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "OCR process failed: {}; {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Diagnostic only: both routes consume the same saved, unmodified JPEG bytes.
pub fn compare_input_methods(path: &std::path::Path, captured: &[u8]) -> Result<(), String> {
    let saved = std::fs::read(path).map_err(|e| e.to_string())?;
    if saved != captured {
        return Err("Saved bytes differ from captured bytes".into());
    }
    println!("IDENTICAL_BYTES=true bytes={}", saved.len());
    let file = Command::new("timeout")
        .args(["15s", "tesseract"])
        .arg(path)
        .args(["stdout", "-l", "eng+kor", "--psm", "3", "tsv"])
        .output()
        .map_err(|e| e.to_string())?;
    println!("FILE status={}", file.status);
    if !file.status.success() {
        eprintln!("FILE_ERROR {}", String::from_utf8_lossy(&file.stderr));
    }
    let stdin = run_ocr_from_memory(&saved);
    match &stdin {
        Ok(_) => println!("STDIN status=success"),
        Err(error) => println!("STDIN_ERROR {error}"),
    }
    if file.status.success() {
        let text = String::from_utf8_lossy(&file.stdout);
        let boxes = parse_tsv(&text)?;
        println!(
            "FILE words={} run_candidates={}",
            boxes.len(),
            boxes
                .iter()
                .filter(|b| b.text == "RUN" && b.confidence >= 70.0)
                .count()
        );
    }
    if let Ok(text) = &stdin {
        let boxes = parse_tsv(text)?;
        println!(
            "STDIN words={} run_candidates={}",
            boxes.len(),
            boxes
                .iter()
                .filter(|b| b.text == "RUN" && b.confidence >= 70.0)
                .count()
        );
        if file.status.success() {
            println!("OCR_OUTPUT_IDENTICAL={}", text.as_bytes() == file.stdout);
        }
    }
    Ok(())
}

/// Decode preserved JPEG through image/zune-jpeg, then OCR lossless RGB pixels.
pub fn verify_jpeg_file(path: &std::path::Path) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let eoi = bytes.windows(2).rposition(|b| b == [0xff, 0xd9]);
    println!(
        "JPEG bytes={} soi={} eoi_offset={eoi:?} eoi_at_end={}",
        bytes.len(),
        bytes.starts_with(&[0xff, 0xd8]),
        bytes.ends_with(&[0xff, 0xd9])
    );
    let rgb = image::load_from_memory(&bytes)
        .map_err(|e| format!("RUST_DECODE_ERROR {e}"))?
        .to_rgb8();
    println!(
        "RUST_DECODE success width={} height={}",
        rgb.width(),
        rgb.height()
    );
    let ppm_path = path.with_extension("decoded.ppm");
    let mut ppm = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&ppm_path)
        .map_err(|e| e.to_string())?;
    write!(ppm, "P6\n{} {}\n255\n", rgb.width(), rgb.height()).map_err(|e| e.to_string())?;
    ppm.write_all(rgb.as_raw()).map_err(|e| e.to_string())?;
    drop(ppm);
    let result = Command::new("timeout")
        .args(["15s", "tesseract"])
        .arg(&ppm_path)
        .args(["stdout", "-l", "eng+kor", "--psm", "3", "tsv"])
        .output()
        .map_err(|e| e.to_string())?;
    println!("DECODED_PPM_OCR status={}", result.status);
    if result.status.success() {
        let boxes = parse_tsv(&String::from_utf8_lossy(&result.stdout))?;
        println!(
            "DECODED_PPM words={} run_candidates={}",
            boxes.len(),
            boxes
                .iter()
                .filter(|b| b.text == "RUN" && b.confidence >= 70.0)
                .count()
        );
    } else {
        eprintln!(
            "DECODED_PPM_ERROR {}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const HEADER: &str = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n";
    #[test]
    fn parses_words_and_ignores_layout_rows() {
        let tsv = format!(
            "{HEADER}1\t1\t0\t0\t0\t0\t0\t0\t1920\t1080\t-1\t\n5\t1\t1\t1\t1\t1\t10\t20\t50\t30\t95.5\tRUN\n"
        );
        let boxes = parse_tsv(&tsv).unwrap();
        assert_eq!(boxes.len(), 1);
        assert_eq!(boxes[0].center(), (35, 35));
        assert_eq!(boxes[0].text, "RUN");
    }
    #[test]
    fn rejects_malformed_and_overflowing_boxes() {
        assert!(parse_tsv("not tsv").is_err());
        for coords in [
            "x\t20\t50\t30\t95",
            "65530\t20\t50\t30\t95",
            "10\t20\t0\t30\t95",
            "10\t20\t50\t30\tNaN",
        ] {
            assert!(parse_tsv(&format!("{HEADER}5\t1\t1\t1\t1\t1\t{coords}\tRUN\n")).is_err());
        }
    }
}
