//! Reactive synthetic application. Agent sees rendered pixels/OCR, not app state.
use image::{Rgb, RgbImage};
use std::time::Duration;
use windows_control::{
    execution::Observation,
    exploration::*,
    ocr::TextBox,
    screen::{ScreenAnalyzer, ScreenConfig},
};

struct VirtualDesktop {
    app: u8,
    id: u64,
    time: Duration,
    model: ScreenAnalyzer,
    clicks: usize,
    resets: usize,
}
fn draw_button(image: &mut RgbImage, x: u32, y: u32) {
    for yy in y..y + 50 {
        for xx in x..x + 160 {
            image.put_pixel(
                xx,
                yy,
                Rgb(if yy == y || yy == y + 49 || xx == x || xx == x + 159 {
                    [20; 3]
                } else {
                    [240; 3]
                }),
            );
        }
    }
}
impl Environment for VirtualDesktop {
    fn now(&self) -> Duration {
        self.time
    }
    fn observe(&mut self) -> Result<Scene, String> {
        self.id += 1;
        self.time += Duration::from_millis(1);
        let mut image = RgbImage::from_pixel(800, 600, Rgb([255; 3]));
        let labels = match self.app {
            0 => vec![
                ("Library", 40, 35),
                ("Browse", 115, 115),
                ("Preferences", 415, 115),
            ],
            1 => vec![("Details", 40, 35), ("Expand", 115, 115)],
            2 => vec![("Preferences", 40, 35), ("Display options", 40, 200)],
            _ => vec![("Details", 40, 35), ("Additional information", 40, 200)],
        };
        if self.app == 0 {
            draw_button(&mut image, 100, 100);
            draw_button(&mut image, 400, 100);
        } else if self.app == 1 {
            draw_button(&mut image, 100, 100);
        }
        let boxes = labels
            .into_iter()
            .map(|(text, x, y)| TextBox {
                text: text.into(),
                x,
                y,
                w: 100,
                h: 20,
                confidence: 95.,
            })
            .collect();
        let obs = Observation {
            frame_id: self.id,
            captured_at: self.time,
            width: 800,
            height: 600,
            boxes,
        };
        Ok(self.model.analyze(&image, obs, self.time)?.scene)
    }
    fn act(&mut self, c: &Candidate) -> Result<(), String> {
        assert_eq!(c.frame_id, self.id);
        let (x, y) = c.point();
        self.clicks += 1;
        self.time += Duration::from_millis(1);
        assert!((100..150).contains(&y));
        self.app = match (self.app, x) {
            (0, 100..=260) => 1,
            (0, 400..=560) => 2,
            (1, 100..=260) => 3,
            _ => panic!("clicked outside visible control"),
        };
        Ok(())
    }
    fn restore(&mut self) -> Result<(), String> {
        self.resets += 1;
        self.app = 0;
        self.time += Duration::from_millis(1);
        Ok(())
    }
}
#[test]
fn discovers_unknown_application_via_pixels_and_returns_to_initial_visual_state() {
    let mut env = VirtualDesktop {
        app: 0,
        id: 0,
        time: Duration::ZERO,
        model: ScreenAnalyzer::new(ScreenConfig {
            allow_rect_controls: true,
            ..ScreenConfig::default()
        })
        .unwrap(),
        clicks: 0,
        resets: 0,
    };
    let report = explore(&mut env, &Limits::default());
    assert_eq!(report.stop, Stop::Complete);
    assert_eq!(report.graph.nodes.len(), 4);
    assert_eq!(report.graph.edges.len(), 3);
    assert_eq!(report.input_attempts, 7);
    assert_eq!(env.clicks, 4);
    assert_eq!(env.resets, 3);
    assert!(report.restored);
    assert!(report.graph.nodes.iter().all(|n| n.state.starts_with('S')));
}
