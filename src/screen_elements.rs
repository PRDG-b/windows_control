//! Screenshot geometry and OCR association. Shapes are candidates, not UI roles.
use crate::ocr::TextBox;
use image::RgbImage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementKind {
    RectControl,
    Panel,
    Text,
    VisualRegion,
}
#[derive(Debug, Clone)]
pub struct Element {
    pub key: String,
    pub kind: ElementKind,
    pub bounds: [u16; 4],
    /// Heuristic boundary support, not a calibrated probability.
    pub score: f32,
    /// Observation data, kept in memory. Artifact writers omit OCR text.
    pub text: String,
}

pub(crate) fn contains(a: [u16; 4], b: [u16; 4]) -> bool {
    u32::from(a[0]) <= u32::from(b[0]) + 2
        && u32::from(a[1]) <= u32::from(b[1]) + 2
        && u32::from(a[0]) + u32::from(a[2]) + 2 >= u32::from(b[0]) + u32::from(b[2])
        && u32::from(a[1]) + u32::from(a[3]) + 2 >= u32::from(b[1]) + u32::from(b[3])
}
fn overlaps(a: [u16; 4], b: [u16; 4]) -> bool {
    u32::from(a[0]) < u32::from(b[0]) + u32::from(b[2]) + 2
        && u32::from(b[0]) < u32::from(a[0]) + u32::from(a[2]) + 2
        && u32::from(a[1]) < u32::from(b[1]) + u32::from(b[3]) + 2
        && u32::from(b[1]) < u32::from(a[1]) + u32::from(a[3]) + 2
}
fn bbox(b: &TextBox) -> [u16; 4] {
    [b.x, b.y, b.w, b.h]
}

pub(crate) fn detect(
    image: &RgbImage,
    words: &[TextBox],
    limit: usize,
) -> Result<Vec<Element>, String> {
    let (w, h) = image.dimensions();
    let width = w as usize;
    let height = h as usize;
    let pixels = image.as_raw();
    let mut edges = vec![0u8; width * height];
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let i = y * width + x;
            if (0..3).any(|c| {
                pixels[(i + 1) * 3 + c]
                    .abs_diff(pixels[(i - 1) * 3 + c])
                    .max(pixels[(i + width) * 3 + c].abs_diff(pixels[(i - width) * 3 + c]))
                    >= 32
            }) {
                edges[i] = 1;
            }
        }
    }
    let mut components = Vec::new();
    let mut queue = Vec::new();
    for start in 0..edges.len() {
        if edges[start] != 1 {
            continue;
        }
        edges[start] = 2;
        queue.clear();
        queue.push(start);
        let (mut x0, mut y0, mut x1, mut y1) =
            (start % width, start / width, start % width, start / width);
        let mut next = 0;
        while next < queue.len() {
            let i = queue[next];
            next += 1;
            let x = i % width;
            let y = i / width;
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
            for yy in y.saturating_sub(1)..=(y + 1).min(height - 1) {
                for xx in x.saturating_sub(1)..=(x + 1).min(width - 1) {
                    let n = yy * width + xx;
                    if edges[n] == 1 {
                        edges[n] = 2;
                        queue.push(n);
                    }
                }
            }
        }
        if x1 - x0 >= 7 && y1 - y0 >= 7 && queue.len() >= 12 {
            components.push([
                x0 as u16,
                y0 as u16,
                (x1 - x0 + 1) as u16,
                (y1 - y0 + 1) as u16,
            ]);
        }
    }
    let nearby = |x: usize, y: usize| {
        (y.saturating_sub(2)..=(y + 2).min(height - 1)).any(|yy| {
            (x.saturating_sub(2)..=(x + 2).min(width - 1)).any(|xx| edges[yy * width + xx] != 0)
        })
    };
    let support = |r: [u16; 4]| {
        let [x, y, rw, rh] = r.map(usize::from);
        let xs: Vec<_> = (x + 2..x + rw - 2).step_by(2).collect();
        let ys: Vec<_> = (y + 2..y + rh - 2).step_by(2).collect();
        [
            xs.iter().filter(|&&xx| nearby(xx, y)).count() as f32 / xs.len().max(1) as f32,
            xs.iter().filter(|&&xx| nearby(xx, y + rh - 1)).count() as f32 / xs.len().max(1) as f32,
            ys.iter().filter(|&&yy| nearby(x, yy)).count() as f32 / ys.len().max(1) as f32,
            ys.iter().filter(|&&yy| nearby(x + rw - 1, yy)).count() as f32 / ys.len().max(1) as f32,
        ]
        .into_iter()
        .fold(1f32, f32::min)
    };
    let mut elements: Vec<Element> = vec![];
    for r in &components {
        let area = u32::from(r[2]) * u32::from(r[3]);
        if r[2] < 24
            || r[3] < 18
            || words
                .iter()
                .any(|b| contains(bbox(b), *r) && !contains(*r, bbox(b)))
        {
            continue;
        }
        let score = support(*r);
        if score < 0.75 {
            continue;
        }
        if elements
            .iter()
            .any(|e| contains(e.bounds, *r) && contains(*r, e.bounds))
        {
            continue;
        }
        elements.push(Element {
            key: String::new(),
            kind: if area > w * h / 5 {
                ElementKind::Panel
            } else {
                ElementKind::RectControl
            },
            bounds: *r,
            score,
            text: String::new(),
        });
        if elements.len() > limit {
            return Err("Too many screen elements".into());
        }
    }
    let mut words = words.to_vec();
    words.sort_by_key(|b| (b.y, b.x));
    let mut loose: Vec<TextBox> = vec![];
    for word in words {
        let parent = elements
            .iter()
            .enumerate()
            .filter(|(_, e)| contains(e.bounds, bbox(&word)))
            .min_by_key(|(_, e)| u32::from(e.bounds[2]) * u32::from(e.bounds[3]))
            .map(|(i, _)| i);
        if let Some(i) = parent {
            if !elements[i].text.is_empty() {
                elements[i].text.push(' ');
            }
            elements[i].text.push_str(&word.text);
        } else {
            // Join neighbouring OCR words, without classifying text as a button.
            let line = loose.iter_mut().find(|b| {
                b.y.abs_diff(word.y) <= b.h.min(word.h) / 2
                    && u32::from(word.x) >= u32::from(b.x) + u32::from(b.w)
                    && u32::from(word.x) <= u32::from(b.x) + u32::from(b.w) + u32::from(b.h) * 2
            });
            if let Some(line) = line {
                line.w = word.x + word.w - line.x;
                line.h = line.h.max(word.y + word.h - line.y);
                line.text.push(' ');
                line.text.push_str(&word.text);
            } else {
                loose.push(word);
            }
        }
    }
    elements.extend(loose.iter().map(|b| Element {
        key: String::new(),
        kind: ElementKind::Text,
        bounds: bbox(b),
        score: b.confidence / 100.,
        text: b.text.clone(),
    }));
    for r in components {
        if r[2] > 96 || r[3] > 96 || elements.iter().any(|e| overlaps(e.bounds, r)) {
            continue;
        }
        elements.push(Element {
            key: String::new(),
            kind: ElementKind::VisualRegion,
            bounds: r,
            score: 0.25,
            text: String::new(),
        });
        if elements.len() > limit {
            return Err("Too many screen elements".into());
        }
    }
    if elements.len() > limit {
        return Err("Too many screen elements".into());
    }
    elements.sort_by_key(|e| (e.kind as u8, e.bounds[1], e.bounds[0]));
    Ok(elements)
}

pub(crate) fn signature(
    image: &RgbImage,
    r: [u16; 4],
    masks: &[[u16; 4]],
    columns: usize,
    rows: usize,
) -> Vec<u8> {
    let [x, y, w, h] = r.map(u32::from);
    let mut values = Vec::with_capacity(columns * rows * 3);
    for gy in 0..rows {
        for gx in 0..columns {
            let mut total = [0u32; 3];
            let mut count = 0;
            for sy in 0..3 {
                for sx in 0..3 {
                    let px = x + ((gx * 3 + sx) as u32 * w / (columns as u32 * 3)).min(w - 1);
                    let py = y + ((gy * 3 + sy) as u32 * h / (rows as u32 * 3)).min(h - 1);
                    if masks
                        .iter()
                        .any(|r| contains(*r, [px as u16, py as u16, 1, 1]))
                    {
                        continue;
                    }
                    let p = image.get_pixel(px, py);
                    for c in 0..3 {
                        total[c] += u32::from(p[c]);
                    }
                    count += 1;
                }
            }
            for value in total {
                values.push(if count == 0 {
                    128
                } else {
                    (value / count) as u8
                });
            }
        }
    }
    values
}
