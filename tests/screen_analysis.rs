use image::{Rgb, RgbImage};
use std::time::Duration;
use windows_control::{
    execution::Observation,
    ocr::TextBox,
    screen::{ElementKind, ScreenAnalyzer, ScreenConfig},
};

fn rectangle(img: &mut RgbImage, x: u32, y: u32, w: u32, h: u32) {
    for yy in y..y + h {
        for xx in x..x + w {
            img.put_pixel(
                xx,
                yy,
                Rgb(
                    if xx == x || yy == y || xx == x + w - 1 || yy == y + h - 1 {
                        [20; 3]
                    } else {
                        [235; 3]
                    },
                ),
            );
        }
    }
}
fn sample(id: u64, price: &str, popup: bool) -> (RgbImage, Observation) {
    let mut img = RgbImage::from_pixel(800, 600, Rgb([250; 3]));
    rectangle(&mut img, 100, 100, 160, 50);
    rectangle(&mut img, 400, 100, 160, 50);
    let mut boxes = vec![
        text("Open", 115, 115, 70),
        text("Open", 415, 115, 70),
        text(price, 100, 230, 100),
    ];
    if popup {
        rectangle(&mut img, 250, 300, 280, 150);
        boxes.push(text("Continue", 290, 340, 140));
    }
    (
        img,
        Observation {
            frame_id: id,
            captured_at: Duration::from_millis(id),
            width: 800,
            height: 600,
            boxes,
        },
    )
}
fn text(s: &str, x: u16, y: u16, w: u16) -> TextBox {
    TextBox {
        text: s.into(),
        x,
        y,
        w,
        h: 20,
        confidence: 95.,
    }
}

#[test]
fn unlabeled_screens_get_stable_ids_and_current_frame_candidates() {
    let mut model = ScreenAnalyzer::new(ScreenConfig::default()).unwrap();
    let (img, obs) = sample(1, "123.45", false);
    let first = model.analyze(&img, obs, Duration::from_millis(1)).unwrap();
    assert!(first.scene.state.is_none());
    let (img, obs) = sample(2, "123.45", false);
    let second = model.analyze(&img, obs, Duration::from_millis(2)).unwrap();
    assert_eq!(second.scene.state.as_deref(), Some("S000001"));
    let controls: Vec<_> = second
        .elements
        .iter()
        .filter(|e| e.kind == ElementKind::RectControl)
        .collect();
    assert_eq!(controls.len(), 2);
    assert_ne!(
        controls[0].key, controls[1].key,
        "duplicate labels must have distinct objects"
    );
    assert!(
        second.scene.candidates.iter().all(|c| c.frame_id == 2
            && c.permission == windows_control::exploration::Permission::Unknown)
    );
    assert!(second.scene.candidates.iter().any(|c| {
        let (x, y) = c.point();
        x.abs_diff(180) < 3 && y.abs_diff(125) < 3
    }));
}

#[test]
fn numbers_update_data_but_popup_and_anchor_changes_create_states() {
    let mut model = ScreenAnalyzer::new(ScreenConfig::default()).unwrap();
    for id in 1..=2 {
        let (im, o) = sample(id, "123.45", false);
        model.analyze(&im, o, Duration::from_millis(id)).unwrap();
    }
    let (im, o) = sample(3, "124.00", false);
    let update = model.analyze(&im, o, Duration::from_millis(3)).unwrap();
    assert_eq!(update.scene.state.as_deref(), Some("S000001"));
    assert_eq!(update.data_revision, Some(1));
    for id in 4..=5 {
        let (im, o) = sample(id, "124.00", true);
        let a = model.analyze(&im, o, Duration::from_millis(id)).unwrap();
        if id == 4 {
            assert!(a.scene.state.is_none());
        } else {
            assert_eq!(a.scene.state.as_deref(), Some("S000002"));
        }
    }
    for id in 6..=7 {
        let (im, mut o) = sample(id, "124.00", false);
        o.boxes[0].text = "Settings".into();
        let a = model.analyze(&im, o, Duration::from_millis(id)).unwrap();
        if id == 7 {
            assert_eq!(a.scene.state.as_deref(), Some("S000003"));
            assert_eq!(a.layout_id.as_deref(), Some("L000001"));
        }
    }
}

#[test]
fn pinned_numeric_region_changes_identity_and_bad_frames_do_not_register() {
    let mut config = ScreenConfig::default();
    config.identity_regions.push([80, 210, 160, 60]);
    let mut model = ScreenAnalyzer::new(config).unwrap();
    for id in 1..=4 {
        let (im, o) = sample(id, if id <= 2 { "123.45" } else { "124.00" }, false);
        let a = model.analyze(&im, o, Duration::from_millis(id)).unwrap();
        if id == 4 {
            assert_eq!(a.scene.state.as_deref(), Some("S000002"));
        }
    }
    let (im, o) = sample(4, "124.00", false);
    assert!(model.analyze(&im, o, Duration::from_millis(5)).is_err());
    let (im, mut o) = sample(5, "124.00", false);
    o.width = 799;
    assert!(model.analyze(&im, o, Duration::from_millis(5)).is_err());
}

#[test]
fn low_confidence_and_blank_frames_do_not_authorize_input() {
    let mut model = ScreenAnalyzer::new(ScreenConfig::default()).unwrap();
    for id in 1..=3 {
        let (im, mut o) = sample(id, "123.45", false);
        o.boxes[0].confidence = 20.;
        let a = model.analyze(&im, o, Duration::from_millis(id)).unwrap();
        assert!(a.scene.state.is_none());
        assert!(
            a.scene
                .candidates
                .iter()
                .all(|c| c.permission != windows_control::exploration::Permission::Allowed)
        );
    }
}

#[test]
fn color_changes_are_distinct_and_outside_viewports_return_errors() {
    let mut model = ScreenAnalyzer::new(ScreenConfig::default()).unwrap();
    for id in 1..=4 {
        let (mut im, o) = sample(id, "123.45", false);
        for y in 101..149 {
            for x in 101..259 {
                im.put_pixel(x, y, Rgb(if id <= 2 { [255, 0, 0] } else { [0, 255, 0] }));
            }
        }
        let a = model.analyze(&im, o, Duration::from_millis(id)).unwrap();
        if id == 4 {
            assert_eq!(a.scene.state.as_deref(), Some("S000002"));
        }
    }
}

#[test]
fn outside_viewports_return_errors_instead_of_panicking() {
    for viewport in [[800, 0, 1, 10], [0, 600, 10, 1], [0, 0, 801, 600]] {
        let mut model = ScreenAnalyzer::new(ScreenConfig {
            viewport: Some(viewport),
            ..ScreenConfig::default()
        })
        .unwrap();
        let (im, o) = sample(1, "123.45", false);
        assert!(model.analyze(&im, o, Duration::from_millis(1)).is_err());
    }
}

#[test]
fn numeric_text_width_change_does_not_create_a_new_layout() {
    let mut model = ScreenAnalyzer::new(ScreenConfig::default()).unwrap();
    for id in 1..=4 {
        let (im, mut o) = sample(id, if id <= 2 { "9.9" } else { "1000.99" }, false);
        o.boxes[2].w = if id <= 2 { 30 } else { 120 };
        let a = model.analyze(&im, o, Duration::from_millis(id)).unwrap();
        if id == 4 {
            assert_eq!(a.scene.state.as_deref(), Some("S000001"));
            assert_eq!(a.data_revision, Some(1));
        }
    }
}

#[test]
fn analyzes_real_huggingface_trending_page() {
    let path = std::path::Path::new("diagnostics/huggingface-test/frame-0001.jpg");
    if !path.exists() {
        return;
    }
    let img = image::open(path)
        .expect("open huggingface image")
        .into_rgb8();
    let tsv =
        std::fs::read_to_string("diagnostics/huggingface-test/frame-0001.tsv").expect("read tsv");
    let boxes = windows_control::ocr::parse_tsv(&tsv).expect("parse tsv");
    let obs = Observation {
        frame_id: 1,
        captured_at: Duration::from_secs(1),
        width: img.width() as u16,
        height: img.height() as u16,
        boxes,
    };
    let mut model = ScreenAnalyzer::new(ScreenConfig {
        allow_rect_controls: true,
        ..ScreenConfig::default()
    })
    .unwrap();
    let analysis = model.analyze(&img, obs, Duration::from_secs(1)).unwrap();
    windows_control::screen_output::write_analysis(
        std::path::Path::new("diagnostics/huggingface-test"),
        &img,
        &analysis,
    )
    .unwrap();
    println!(
        "HuggingFace Real Page Analysis: elements={}, candidates={}, quality={}",
        analysis.elements.len(),
        analysis.scene.candidates.len(),
        analysis.quality
    );
    assert!(!analysis.elements.is_empty());
}
