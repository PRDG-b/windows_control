use crate::hid::ActionLog;
use image::{Rgb, RgbImage};

pub fn draw_history_on_capture(
    input_jpg: &str,
    output_jpg: &str,
    logs: &[ActionLog],
) -> Result<(), String> {
    let mut img: RgbImage = image::open(input_jpg)
        .map_err(|e| format!("Failed to Open image: {}", e))?
        .to_rgb8();

    let (w, h) = (img.width() as i32, img.height() as i32);

    let box_w = 520.min(w - 20);
    let box_h = (36 + logs.len() as i32 * 22).min(h - 20);
    for py in 10..(10 + box_h) {
        for px in 10..(10 + box_w) {
            let p = img.get_pixel_mut(px as u32, py as u32);
            *p = Rgb([p[0] / 3, p[1] / 3, p[2] / 3]);
        }
    }

    // 제목
    draw_text(&mut img, 20, 18, "=== ACTION HISTORY OVERLAY ===", Rgb([0, 230, 255]), 2);

    // 각 조작 내역을 순회하며 HUD 텍스트 & 마우스 위치 그리기
    let mut prev_pos: Option<(i32, i32)> = None;

    for (idx, log) in logs.iter().enumerate() {
        // (A) 좌측 상단 HUD에 조작 내역 한 줄씩 출력 (흰색)
        let line_y = 44 + (idx as i32 * 20);
        let text = format!("#{} {}", log.step, log.description);
        if line_y + 16 < h {
            draw_text(&mut img, 20, line_y, &text, Rgb([255, 255, 255]), 2);
        }

        // (B) 마우스 좌표가 있는 조작이면 화면 해당 위치에 과녁과 경로 그리기
        if let Some((mx, my)) = log.pixel_pos {
            let (cx, cy) = (mx as i32, my as i32);

            // 이전 마우스 위치가 있으면 노란색 이동 경로 선 연결
            if let Some((px, py)) = prev_pos {
                draw_line(&mut img, px, py, cx, cy, Rgb([255, 220, 0]));
            }
            prev_pos = Some((cx, cy));

            // 현재 마우스 위치에 빨간색 과녁(십자선 + 사각형) 그리기
            draw_crosshair(&mut img, cx, cy, Rgb([255, 40, 40]));

            // 과녁 옆에 "#단계 (x,y)" 라벨 찍기 (노란색)
            let label = format!("#{} ({},{})", log.step, cx, cy);
            let label_x = (cx + 18).min(w - 180).max(5);
            let label_y = (cy - 18).min(h - 20).max(5);
            draw_text(&mut img, label_x, label_y, &label, Rgb([255, 255, 0]), 2);
        }
    }

    img.save(output_jpg).map_err(|e| format!("Failed to create Overlay image: {}", e))?;
    println!("Overlay Image saved at {}", output_jpg);
    Ok(())
}

/// 빨간색 과녁 (십자선 + 테두리 박스) 그리기
fn draw_crosshair(img: &mut RgbImage, cx: i32, cy: i32, color: Rgb<u8>) {
    let (w, h) = (img.width() as i32, img.height() as i32);
    // 십자선 (+ 모양, 길이 16px, 두께 3px)
    for d in -16..=16 {
        for t in -1..=1 {
            if cx + d >= 0 && cx + d < w && cy + t >= 0 && cy + t < h {
                img.put_pixel((cx + d) as u32, (cy + t) as u32, color);
            }
            if cx + t >= 0 && cx + t < w && cy + d >= 0 && cy + d < h {
                img.put_pixel((cx + t) as u32, (cy + d) as u32, color);
            }
        }
    }
    // 바깥 사각형 테두리 (20x20 크기)
    let r = 10;
    for d in -r..=r {
        for &(dx, dy) in &[(d, -r), (d, r), (-r, d), (r, d)] {
            if cx + dx >= 0 && cx + dx < w && cy + dy >= 0 && cy + dy < h {
                img.put_pixel((cx + dx) as u32, (cy + dy) as u32, color);
            }
        }
    }
}

/// 두 점 사이를 잇는 직선 그리기 (Bresenham 알고리즘)
fn draw_line(img: &mut RgbImage, x0: i32, y0: i32, x1: i32, y1: i32, color: Rgb<u8>) {
    let (w, h) = (img.width() as i32, img.height() as i32);
    let (mut x, mut y) = (x0, y0);
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        if x >= 0 && x < w && y >= 0 && y < h {
            img.put_pixel(x as u32, y as u32, color);
        }
        if x == x1 && y == y1 { break; }
        let e2 = 2 * err;
        if e2 >= dy { err += dy; x += sx; }
        if e2 <= dx { err += dx; y += sy; }
    }
}

/// 외부 폰트 파일 없이 영문/숫자/기호를 선명하게 찍는 5x7 비트맵 폰트 렌더러
fn draw_text(img: &mut RgbImage, x: i32, y: i32, text: &str, color: Rgb<u8>, scale: i32) {
    let (w, h) = (img.width() as i32, img.height() as i32);
    for (i, ch) in text.chars().enumerate() {
        let glyph = glyph_5x7(ch.to_ascii_uppercase());
        let char_x = x + (i as i32) * (6 * scale);
        for (row, &bits) in glyph.iter().enumerate() {
            for col in 0..5 {
                if (bits >> (4 - col)) & 1 == 1 {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            let px = char_x + col * scale + sx;
                            let py = y + (row as i32) * scale + sy;
                            if px >= 0 && px < w && py >= 0 && py < h {
                                img.put_pixel(px as u32, py as u32, color);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn glyph_5x7(c: char) -> [u8; 7] {
    match c {
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
        'D' => [0b11100, 0b10010, 0b10001, 0b10001, 0b10001, 0b10010, 0b11100],
        'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
        'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110],
        'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100],
        'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'Q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001],
        'X' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
        'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111],
        '3' => [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110],
        '#' => [0b01010, 0b11111, 0b01010, 0b01010, 0b11111, 0b01010, 0b00000],
        '(' => [0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010],
        ')' => [0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000],
        ',' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00100, 0b00100, 0b01000],
        ':' => [0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000],
        '=' => [0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000],
        '-' => [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000],
        _   => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000],
    }
}