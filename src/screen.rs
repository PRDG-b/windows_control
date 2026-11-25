//! Observable-state identity and candidates, with no application marker strings.
pub use crate::screen_elements::ElementKind;
use crate::{
    execution::{Observation, validate_frame},
    exploration::{Candidate, Permission, Scene},
    screen_elements::{self, Element, contains, signature},
};
use image::RgbImage;
use std::time::Duration;

#[derive(Clone)]
pub struct ScreenConfig {
    pub stable_frames: usize,
    pub max_elements: usize,
    pub max_states: usize,
    pub max_age: Duration,
    pub min_ocr_confidence: f32,
    pub geometry_tolerance: f32,
    pub visual_tolerance: f32,
    pub viewport: Option<[u16; 4]>,
    pub identity_regions: Vec<[u16; 4]>,
    /// Explicit disposable-environment policy. Detection alone never sets this.
    pub allow_rect_controls: bool,
}
impl Default for ScreenConfig {
    fn default() -> Self {
        Self {
            stable_frames: 2,
            max_elements: 256,
            max_states: 128,
            max_age: Duration::from_secs(30),
            min_ocr_confidence: 70.,
            geometry_tolerance: 0.06,
            visual_tolerance: 0.08,
            viewport: None,
            identity_regions: vec![],
            allow_rect_controls: false,
        }
    }
}

pub struct Analysis {
    pub scene: Scene,
    pub elements: Vec<Element>,
    pub layout_id: Option<String>,
    pub data_revision: Option<u64>,
    pub stable: bool,
    pub quality: &'static str,
}
#[derive(Clone)]
struct Feature {
    kind: ElementKind,
    rect: [f32; 4],
    anchor: String,
    visual: Vec<u8>,
}
#[derive(Clone)]
struct Descriptor {
    features: Vec<Feature>,
    content: Vec<String>,
    global: Vec<u8>,
}
struct Layout {
    id: String,
    descriptor: Descriptor,
}
struct State {
    id: String,
    layout: usize,
    descriptor: Descriptor,
    data: Vec<String>,
    revision: u64,
}
pub struct ScreenAnalyzer {
    config: ScreenConfig,
    previous: Option<(u64, Duration)>,
    last_now: Duration,
    pending: Option<Descriptor>,
    stable_count: usize,
    layouts: Vec<Layout>,
    states: Vec<State>,
}
impl ScreenAnalyzer {
    pub fn new(config: ScreenConfig) -> Result<Self, String> {
        if config.stable_frames < 2
            || config.max_elements == 0
            || config.max_states == 0
            || config.max_age.is_zero()
            || !config.min_ocr_confidence.is_finite()
            || !(0. ..=100.).contains(&config.min_ocr_confidence)
            || !config.geometry_tolerance.is_finite()
            || !(0. ..=0.2).contains(&config.geometry_tolerance)
            || config.geometry_tolerance == 0.
            || !config.visual_tolerance.is_finite()
            || !(0. ..=0.3).contains(&config.visual_tolerance)
            || config.viewport.is_some_and(|r| r[2] == 0 || r[3] == 0)
            || config
                .identity_regions
                .iter()
                .any(|r| r[2] == 0 || r[3] == 0)
        {
            return Err("Invalid screen analysis configuration".into());
        }
        Ok(Self {
            config,
            previous: None,
            last_now: Duration::ZERO,
            pending: None,
            stable_count: 0,
            layouts: vec![],
            states: vec![],
        })
    }

    pub fn analyze(
        &mut self,
        image: &RgbImage,
        observation: Observation,
        now: Duration,
    ) -> Result<Analysis, String> {
        if now < self.last_now {
            return Err("Non-monotonic screen observation".into());
        }
        validate_frame(&observation, now, self.previous, None, self.config.max_age)
            .map_err(|r| r.code().to_string())?;
        if image.width() != u32::from(observation.width)
            || image.height() != u32::from(observation.height)
            || image.width() < 3
            || image.height() < 3
            || u64::from(image.width()) * u64::from(image.height()) > 4_000_000
        {
            return Err("Invalid image dimensions for observation".into());
        }
        let viewport =
            self.config
                .viewport
                .unwrap_or([0, 0, observation.width, observation.height]);
        if u32::from(viewport[0]) + u32::from(viewport[2]) > u32::from(observation.width)
            || u32::from(viewport[1]) + u32::from(viewport[3]) > u32::from(observation.height)
        {
            return Err("Viewport outside image".into());
        }
        let mut elements =
            screen_elements::detect(image, &observation.boxes, self.config.max_elements)?;
        elements.retain(|e| contains(viewport, e.bounds));
        let masks: Vec<_> = observation
            .boxes
            .iter()
            .map(|b| [b.x, b.y, b.w, b.h])
            .collect();
        let normalize = |s: &str| {
            s.split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_lowercase()
        };
        let content: Vec<_> = elements.iter().map(|e| normalize(&e.text)).collect();
        let features = elements
            .iter()
            .zip(&content)
            .map(|(e, text)| {
                let numeric = text.chars().any(|c| c.is_numeric())
                    && text
                        .chars()
                        .all(|c| c.is_numeric() || c.is_whitespace() || ".,:/%+-_$()".contains(c));
                let pinned = self
                    .config
                    .identity_regions
                    .iter()
                    .any(|r| contains(*r, e.bounds));
                Feature {
                    kind: e.kind,
                    rect: [
                        (f32::from(e.bounds[0]) - f32::from(viewport[0])) / f32::from(viewport[2]),
                        (f32::from(e.bounds[1]) - f32::from(viewport[1])) / f32::from(viewport[3]),
                        f32::from(e.bounds[2]) / f32::from(viewport[2]),
                        f32::from(e.bounds[3]) / f32::from(viewport[3]),
                    ],
                    anchor: if numeric && !pinned {
                        String::new()
                    } else {
                        text.clone()
                    },
                    visual: signature(image, e.bounds, &masks, 4, 4),
                }
            })
            .collect();
        let descriptor = Descriptor {
            features,
            content,
            global: signature(image, viewport, &masks, 12, 8),
        };
        let uncertain = observation.boxes.iter().any(|b| {
            contains(viewport, [b.x, b.y, b.w, b.h])
                && b.confidence < self.config.min_ocr_confidence
        });
        self.previous = Some((observation.frame_id, observation.captured_at));
        self.last_now = now;
        let quality = if elements.is_empty() {
            "no_elements"
        } else if uncertain {
            "uncertain_ocr"
        } else {
            "observed"
        };
        if quality != "observed" {
            self.pending = None;
            self.stable_count = 0;
        } else {
            let matched = self
                .pending
                .as_ref()
                .is_some_and(|p| self.semantic_match(p, &descriptor).is_some());
            self.stable_count = if matched { self.stable_count + 1 } else { 1 };
            self.pending = Some(descriptor.clone());
        }
        let stable = quality == "observed" && self.stable_count >= self.config.stable_frames;
        let (mut state_id, mut layout_id, mut data_revision) = (None, None, None);
        let mut object_indices: Vec<_> = (0..elements.len()).collect();
        if stable {
            let matched_layout = self.layouts.iter().enumerate().find_map(|(i, l)| {
                self.layout_match(&l.descriptor, &descriptor)
                    .map(|map| (i, map))
            });
            let (layout, map) = if let Some(value) = matched_layout {
                value
            } else {
                if self.layouts.len() >= self.config.max_states {
                    return Err("Screen layout limit reached".into());
                }
                let i = self.layouts.len();
                self.layouts.push(Layout {
                    id: format!("L{:06}", i + 1),
                    descriptor: descriptor.clone(),
                });
                (i, (0..elements.len()).collect())
            };
            object_indices = map;
            layout_id = Some(self.layouts[layout].id.clone());
            let matched_state = self
                .states
                .iter()
                .enumerate()
                .filter(|(_, s)| s.layout == layout)
                .find_map(|(i, s)| {
                    self.semantic_match(&s.descriptor, &descriptor)
                        .map(|map| (i, map))
                });
            let state = if let Some((i, map)) = matched_state {
                let mut data = vec![String::new(); map.len()];
                for (new, old) in map.into_iter().enumerate() {
                    data[old] = descriptor.content[new].clone();
                }
                if self.states[i].data != data {
                    self.states[i].revision += 1;
                    self.states[i].data = data;
                }
                i
            } else {
                if self.states.len() >= self.config.max_states {
                    return Err("Screen state limit reached".into());
                }
                let i = self.states.len();
                self.states.push(State {
                    id: format!("S{:06}", i + 1),
                    layout,
                    descriptor: descriptor.clone(),
                    data: descriptor.content.clone(),
                    revision: 0,
                });
                i
            };
            state_id = Some(self.states[state].id.clone());
            data_revision = Some(self.states[state].revision);
        }
        let mut candidates = vec![];
        for (i, element) in elements.iter_mut().enumerate() {
            element.key = format!(
                "{}_O{:04}",
                layout_id.as_deref().unwrap_or("pending"),
                object_indices[i] + 1
            );
            if element.kind == ElementKind::Panel {
                continue;
            }
            let permission = if stable
                && self.config.allow_rect_controls
                && element.kind == ElementKind::RectControl
                && element.score >= 0.8
            {
                Permission::Allowed
            } else {
                Permission::Unknown
            };
            candidates.push(Candidate {
                key: format!("{}_click", element.key),
                frame_id: observation.frame_id,
                bounds: element.bounds,
                permission,
            });
        }
        Ok(Analysis {
            scene: Scene {
                observation,
                state: state_id,
                candidates,
            },
            elements,
            layout_id,
            data_revision,
            stable,
            quality,
        })
    }

    fn layout_match(&self, old: &Descriptor, new: &Descriptor) -> Option<Vec<usize>> {
        if old.features.len() != new.features.len() {
            return None;
        }
        let mut used = vec![false; old.features.len()];
        let mut map = vec![];
        for feature in &new.features {
            let mut choices: Vec<_> = old
                .features
                .iter()
                .enumerate()
                .filter(|(i, f)| !used[*i] && f.kind == feature.kind)
                .filter_map(|(i, f)| {
                    let deltas: Vec<_> = f
                        .rect
                        .iter()
                        .zip(feature.rect)
                        .map(|(a, b)| (a - b).abs())
                        .collect();
                    if deltas.iter().enumerate().any(|(axis, d)| {
                        let tolerance = if axis == 2
                            && f.kind == ElementKind::Text
                            && f.anchor.is_empty()
                            && feature.anchor.is_empty()
                        {
                            0.3
                        } else {
                            self.config.geometry_tolerance
                        };
                        *d > tolerance
                    }) {
                        None
                    } else {
                        Some((i, deltas.iter().sum::<f32>()))
                    }
                })
                .collect();
            choices.sort_by(|a, b| a.1.total_cmp(&b.1));
            if choices.is_empty()
                || (choices.len() > 1 && (choices[1].1 - choices[0].1).abs() < 0.001)
            {
                return None;
            }
            let best = choices[0].0;
            used[best] = true;
            map.push(best);
        }
        Some(map)
    }

    fn semantic_match(&self, old: &Descriptor, new: &Descriptor) -> Option<Vec<usize>> {
        let map = self.layout_match(old, new)?;
        let distance = |a: &[u8], b: &[u8]| {
            a.iter()
                .zip(b)
                .map(|(a, b)| a.abs_diff(*b) as f32 / 255.)
                .sum::<f32>()
                / a.len().max(1) as f32
        };
        if distance(&old.global, &new.global) > self.config.visual_tolerance {
            return None;
        }
        for (new_index, &old_index) in map.iter().enumerate() {
            let (a, b) = (&old.features[old_index], &new.features[new_index]);
            if a.anchor != b.anchor || distance(&a.visual, &b.visual) > self.config.visual_tolerance
            {
                return None;
            }
        }
        Some(map)
    }
}
