//! Visual DOM: Reconstructs a DOM-like hierarchical structure by combining
//! geometric containment and live interactive probing with the environment.

use crate::exploration::{Environment, Permission, Scene};
use std::fmt::Write as _;
use std::time::Duration;

/// Semantic role of a visual DOM node, mirroring HTML/Accessibility roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomRole {
    Root,
    Container,
    Button,
    TextInput,
    Text,
    ModalDialog,
}

impl DomRole {
    pub fn tag_name(&self) -> &'static str {
        match self {
            Self::Root => "main",
            Self::Container => "div",
            Self::Button => "button",
            Self::TextInput => "input",
            Self::Text => "span",
            Self::ModalDialog => "dialog",
        }
    }
}

/// Observed behavior when interacting with a node (proven by live action).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InteractionEffect {
    Static,
    Toggle,
    ContentMutation { detail: String },
    ChildExpansion { added_nodes: usize },
    ModalOpened { title: String },
    StateTransition { next_state: String },
}

impl InteractionEffect {
    /// Returns a controlled symbolic code for reports/logs without raw OCR text.
    pub fn code(&self) -> String {
        match self {
            Self::Static => "static".into(),
            Self::Toggle => "toggle".into(),
            Self::ContentMutation { .. } => "content_mutation".into(),
            Self::ChildExpansion { added_nodes } => format!("child_expansion:{added_nodes}"),
            Self::ModalOpened { .. } => "modal_opened".into(),
            Self::StateTransition { next_state } => format!("state_transition:{next_state}"),
        }
    }
}

/// Reason why interactive DOM building stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomStop {
    Complete,
    UnknownInitialState,
    Timeout,
    InputError,
    ObservationError,
    RestoreError,
    RestoreMismatch,
    StaleCandidate,
}

impl DomStop {
    pub fn code(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::UnknownInitialState => "unknown_state",
            Self::Timeout => "timeout",
            Self::InputError => "input_error",
            Self::ObservationError => "observation_error",
            Self::RestoreError => "restore_error",
            Self::RestoreMismatch => "restore_mismatch",
            Self::StaleCandidate => "stale_candidate",
        }
    }
}

/// Configuration for bounded interactive DOM probing.
#[derive(Debug, Clone)]
pub struct DomBuildConfig {
    pub max_interactions: usize,
    pub timeout: Duration,
}

impl Default for DomBuildConfig {
    fn default() -> Self {
        Self {
            max_interactions: 10,
            timeout: Duration::from_secs(300),
        }
    }
}

/// Controlled interaction audit event without raw OCR text.
#[derive(Debug, Clone)]
pub struct DomEvent {
    pub step: usize,
    pub candidate_key: String,
    pub bounds: [u16; 4],
    pub before_frame: u64,
    pub after_frame: Option<u64>,
    pub restored_frame: Option<u64>,
    pub effect_code: String,
    pub note: &'static str,
}

/// Complete report from interactive visual DOM construction.
#[derive(Debug, Clone)]
pub struct DomBuildReport {
    pub dom: DomNode,
    pub stop: DomStop,
    pub root_state: Option<String>,
    pub interactions_attempted: usize,
    pub interactions_completed: usize,
    pub observed_frames: usize,
    pub restored: bool,
    pub elapsed: Duration,
    pub events: Vec<DomEvent>,
}

/// A node in the visual DOM tree.
#[derive(Debug, Clone)]
pub struct DomNode {
    pub id: String,
    pub role: DomRole,
    pub bounds: [u16; 4],
    pub text: String,
    pub effect: InteractionEffect,
    pub interactive: bool,
    pub children: Vec<DomNode>,
}

impl DomNode {
    pub fn new(
        id: impl Into<String>,
        role: DomRole,
        bounds: [u16; 4],
        text: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            role,
            bounds,
            text: text.into(),
            effect: InteractionEffect::Static,
            interactive: false,
            children: vec![],
        }
    }

    /// Recursively counts all descendant nodes including self.
    pub fn total_nodes(&self) -> usize {
        1 + self.children.iter().map(Self::total_nodes).sum::<usize>()
    }

    /// Renders the visual DOM tree as an HTML string.
    pub fn to_html(&self, indent: usize) -> String {
        let pad = "  ".repeat(indent);
        let tag = self.role.tag_name();
        let [x, y, w, h] = self.bounds;
        let mut html = format!(
            "{pad}<{tag} id=\"{}\" class=\"{:?}\" style=\"left:{}px; top:{}px; width:{}px; height:{}px;\"",
            self.id, self.role, x, y, w, h
        );

        if self.interactive {
            html.push_str(" data-interactive=\"true\"");
        }
        if self.effect != InteractionEffect::Static {
            let escaped_effect = html_escape(&format!("{:?}", self.effect));
            let _ = write!(html, " data-effect=\"{escaped_effect}\"");
        }

        if self.children.is_empty() && self.text.is_empty() {
            html.push_str(" />\n");
            return html;
        }

        html.push('>');
        if !self.text.is_empty() {
            html.push_str(&html_escape(&self.text));
        }

        if self.children.is_empty() {
            html.push_str(&format!("</{tag}>\n"));
        } else {
            html.push('\n');
            for child in &self.children {
                html.push_str(&child.to_html(indent + 1));
            }
            html.push_str(&format!("{pad}</{tag}>\n"));
        }
        html
    }

    /// Renders an ASCII tree representation for terminal output.
    pub fn to_ascii_tree(&self, indent: usize) -> String {
        let pad = "  ".repeat(indent);
        let [x, y, w, h] = self.bounds;
        let mut out = format!(
            "{pad}├── <{} id=\"{}\" [{},{},{}x{}]",
            self.role.tag_name(),
            self.id,
            x,
            y,
            w,
            h
        );
        if !self.text.is_empty() {
            let _ = write!(out, " text=\"{}\"", self.text);
        }
        if self.interactive {
            out.push_str(" (interactive)");
        }
        if self.effect != InteractionEffect::Static {
            let _ = write!(out, " effect={:?}", self.effect);
        }
        out.push_str(">\n");
        for child in &self.children {
            out.push_str(&child.to_ascii_tree(indent + 1));
        }
        out
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn contains(parent: [u16; 4], child: [u16; 4]) -> bool {
    u32::from(parent[0]) <= u32::from(child[0]) + 2
        && u32::from(parent[1]) <= u32::from(child[1]) + 2
        && u32::from(parent[0]) + u32::from(parent[2]) + 2
            >= u32::from(child[0]) + u32::from(child[2])
        && u32::from(parent[1]) + u32::from(parent[3]) + 2
            >= u32::from(child[1]) + u32::from(child[3])
}

/// Builds a static visual DOM tree based strictly on geometric containment.
pub fn build_geometric_tree(scene: &Scene) -> DomNode {
    let width = scene.observation.width;
    let height = scene.observation.height;

    let mut root = DomNode::new("root", DomRole::Root, [0, 0, width, height], "");

    // 1. Convert candidates and OCR boxes into raw node items
    let mut items: Vec<DomNode> = Vec::new();

    // Add candidates (buttons / clickable regions)
    for (i, c) in scene.candidates.iter().enumerate() {
        let [x, y, w, h] = c.bounds;
        let role = if c.permission == Permission::Allowed {
            DomRole::Button
        } else {
            DomRole::Container
        };
        let mut node = DomNode::new(format!("elem_{i}"), role, [x, y, w, h], "");
        node.interactive = c.permission == Permission::Allowed;
        items.push(node);
    }

    // Attach OCR words to containing elements or add as loose Text nodes
    for b in &scene.observation.boxes {
        let b_box = [b.x, b.y, b.w, b.h];
        // Find the smallest candidate containing this text box
        let parent = items
            .iter_mut()
            .filter(|item| contains(item.bounds, b_box))
            .min_by_key(|item| u32::from(item.bounds[2]) * u32::from(item.bounds[3]));

        if let Some(p) = parent {
            if !p.text.is_empty() {
                p.text.push(' ');
            }
            p.text.push_str(&b.text);
        } else {
            items.push(DomNode::new(
                format!("text_{}_{}", b.x, b.y),
                DomRole::Text,
                b_box,
                &b.text,
            ));
        }
    }

    // 2. Sort by bounding box area in descending order so outer containers come first
    items.sort_by_key(|item| {
        std::cmp::Reverse(u32::from(item.bounds[2]) * u32::from(item.bounds[3]))
    });

    // 3. Assemble hierarchy using nested tree insertion
    for item in items {
        insert_into_deepest_container(&mut root, item);
    }

    root
}

fn insert_into_deepest_container(parent: &mut DomNode, child: DomNode) {
    // Try to find an existing child container that can hold this element
    for existing_child in &mut parent.children {
        if contains(existing_child.bounds, child.bounds) && existing_child.bounds != child.bounds {
            insert_into_deepest_container(existing_child, child);
            return;
        }
    }
    // If no existing child contains it, add as direct child of current parent
    parent.children.push(child);
}

fn rects_close(a: [u16; 4], b: [u16; 4], tol: u16) -> bool {
    a[0].abs_diff(b[0]) <= tol
        && a[1].abs_diff(b[1]) <= tol
        && a[2].abs_diff(b[2]) <= tol
        && a[3].abs_diff(b[3]) <= tol
}

fn boxes_near(a: [u16; 4], b: [u16; 4], gap: u32) -> bool {
    let [ax, ay, aw, ah] = a.map(u32::from);
    let [bx, by, bw, bh] = b.map(u32::from);
    bx <= ax + aw + gap && ax <= bx + bw + gap && by <= ay + ah + gap && ay <= by + bh + gap
}

/// Inserts detected structural containers (`Panel`) into the DOM hierarchy,
/// wrapping matching child siblings without dropping their dynamic effects or children.
pub fn rewrap_with_containers(root: &mut DomNode, containers: &[(String, [u16; 4])]) {
    let mut sorted = containers.to_vec();
    sorted.sort_by_key(|(_, b)| std::cmp::Reverse(u32::from(b[2]) * u32::from(b[3])));
    for (id, bounds) in sorted {
        if bounds[2] == 0 || bounds[3] == 0 {
            continue;
        }
        insert_container_rewrapping(root, id, bounds);
    }
}

fn insert_container_rewrapping(parent: &mut DomNode, id: String, bounds: [u16; 4]) {
    for child in &mut parent.children {
        if contains(child.bounds, bounds)
            && !(contains(bounds, child.bounds) && contains(child.bounds, bounds))
        {
            insert_container_rewrapping(child, id, bounds);
            return;
        }
    }

    if parent
        .children
        .iter()
        .any(|c| rects_close(c.bounds, bounds, 2))
    {
        return;
    }

    let mut container = DomNode::new(id, DomRole::Container, bounds, "");
    let existing = std::mem::take(&mut parent.children);
    for child in existing {
        if contains(bounds, child.bounds) && child.bounds != bounds {
            container.children.push(child);
        } else {
            parent.children.push(child);
        }
    }
    parent.children.push(container);
}

/// Probes the screen interactively to discover dynamic child relationships and behaviors,
/// producing a verified, live DOM-like structure.
pub fn interact_and_build_dom(
    env: &mut impl Environment,
    max_interactions: usize,
) -> Result<DomNode, String> {
    let config = DomBuildConfig {
        max_interactions,
        ..DomBuildConfig::default()
    };
    let report = interact_and_build_dom_with_report(env, &config)?;
    if report.stop == DomStop::UnknownInitialState {
        return Err("Initial screen state is unknown; refusing input".into());
    }
    Ok(report.dom)
}

/// Probes the screen interactively with safety checks, re-localization, and audit reporting.
pub fn interact_and_build_dom_with_report(
    env: &mut impl Environment,
    config: &DomBuildConfig,
) -> Result<DomBuildReport, String> {
    let start_time = env.now();
    let base_scene = env
        .observe()
        .map_err(|e| format!("Base observation failed: {e}"))?;
    let mut dom = build_geometric_tree(&base_scene);
    let mut observed_frames = 1usize;

    if config.max_interactions == 0 {
        return Ok(DomBuildReport {
            dom,
            stop: DomStop::Complete,
            root_state: base_scene.state.clone(),
            interactions_attempted: 0,
            interactions_completed: 0,
            observed_frames,
            restored: true,
            elapsed: env.now().saturating_sub(start_time),
            events: vec![],
        });
    }

    let Some(root_state) = base_scene.state.clone() else {
        return Ok(DomBuildReport {
            dom,
            stop: DomStop::UnknownInitialState,
            root_state: None,
            interactions_attempted: 0,
            interactions_completed: 0,
            observed_frames,
            restored: false,
            elapsed: env.now().saturating_sub(start_time),
            events: vec![],
        });
    };

    let targets: Vec<(String, [u16; 4])> = base_scene
        .candidates
        .iter()
        .filter(|c| c.permission == Permission::Allowed)
        .take(config.max_interactions)
        .map(|c| (c.key.clone(), c.bounds))
        .collect();

    let mut current_scene = base_scene.clone();
    let mut interactions_attempted = 0usize;
    let mut interactions_completed = 0usize;
    let mut restored = true;
    let mut stop = DomStop::Complete;
    let mut events = Vec::new();

    for (step_idx, (target_key, base_bounds)) in targets.into_iter().enumerate() {
        if env.now().saturating_sub(start_time) >= config.timeout {
            stop = DomStop::Timeout;
            break;
        }

        let fresh_candidate = current_scene
            .candidates
            .iter()
            .find(|c| {
                c.permission == Permission::Allowed
                    && c.frame_id == current_scene.observation.frame_id
                    && c.key == target_key
            })
            .or_else(|| {
                current_scene.candidates.iter().find(|c| {
                    c.permission == Permission::Allowed
                        && c.frame_id == current_scene.observation.frame_id
                        && rects_close(c.bounds, base_bounds, 8)
                })
            })
            .cloned();

        let before_frame = current_scene.observation.frame_id;
        let Some(candidate) = fresh_candidate else {
            events.push(DomEvent {
                step: step_idx + 1,
                candidate_key: target_key,
                bounds: base_bounds,
                before_frame,
                after_frame: None,
                restored_frame: None,
                effect_code: "none".into(),
                note: "stale_candidate",
            });
            stop = DomStop::StaleCandidate;
            break;
        };

        interactions_attempted += 1;
        if env.act(&candidate).is_err() {
            events.push(DomEvent {
                step: step_idx + 1,
                candidate_key: target_key,
                bounds: candidate.bounds,
                before_frame,
                after_frame: None,
                restored_frame: None,
                effect_code: "none".into(),
                note: "input_error",
            });
            stop = DomStop::InputError;
            restored = false;
            break;
        }

        env.pause();

        let after_scene = match env.observe() {
            Ok(s) if s.observation.frame_id > before_frame => s,
            _ => {
                events.push(DomEvent {
                    step: step_idx + 1,
                    candidate_key: target_key,
                    bounds: candidate.bounds,
                    before_frame,
                    after_frame: None,
                    restored_frame: None,
                    effect_code: "none".into(),
                    note: "observation_error",
                });
                stop = DomStop::ObservationError;
                restored = false;
                break;
            }
        };
        observed_frames += 1;
        let after_frame = after_scene.observation.frame_id;

        let (effect, dynamic_children) = analyze_delta(&base_scene, &after_scene, base_bounds);
        let effect_code = effect.code();
        let is_static = effect == InteractionEffect::Static;
        apply_node_delta(&mut dom, base_bounds, effect, &mut Some(dynamic_children));
        interactions_completed += 1;

        if after_scene.state.as_deref() == Some(&root_state) && is_static {
            events.push(DomEvent {
                step: step_idx + 1,
                candidate_key: target_key,
                bounds: base_bounds,
                before_frame,
                after_frame: Some(after_frame),
                restored_frame: Some(after_frame),
                effect_code,
                note: "unchanged_no_restore",
            });
            current_scene = after_scene;
            restored = true;
            continue;
        }

        if env.restore().is_err() {
            events.push(DomEvent {
                step: step_idx + 1,
                candidate_key: target_key,
                bounds: base_bounds,
                before_frame,
                after_frame: Some(after_frame),
                restored_frame: None,
                effect_code,
                note: "restore_error",
            });
            stop = DomStop::RestoreError;
            restored = false;
            break;
        }

        env.pause();

        let restored_scene = match env.observe() {
            Ok(s) if s.observation.frame_id > after_frame => s,
            _ => {
                events.push(DomEvent {
                    step: step_idx + 1,
                    candidate_key: target_key,
                    bounds: base_bounds,
                    before_frame,
                    after_frame: Some(after_frame),
                    restored_frame: None,
                    effect_code,
                    note: "restore_observation_error",
                });
                stop = DomStop::ObservationError;
                restored = false;
                break;
            }
        };
        observed_frames += 1;
        let restored_frame = restored_scene.observation.frame_id;

        if restored_scene.state.as_deref() != Some(&root_state) {
            events.push(DomEvent {
                step: step_idx + 1,
                candidate_key: target_key,
                bounds: base_bounds,
                before_frame,
                after_frame: Some(after_frame),
                restored_frame: Some(restored_frame),
                effect_code,
                note: "restore_mismatch",
            });
            stop = DomStop::RestoreMismatch;
            restored = false;
            break;
        }

        events.push(DomEvent {
            step: step_idx + 1,
            candidate_key: target_key,
            bounds: base_bounds,
            before_frame,
            after_frame: Some(after_frame),
            restored_frame: Some(restored_frame),
            effect_code,
            note: "restored",
        });
        current_scene = restored_scene;
        restored = true;
    }

    Ok(DomBuildReport {
        dom,
        stop,
        root_state: Some(root_state),
        interactions_attempted,
        interactions_completed,
        observed_frames,
        restored,
        elapsed: env.now().saturating_sub(start_time),
        events,
    })
}

fn analyze_delta(
    base: &Scene,
    after: &Scene,
    target_bounds: [u16; 4],
) -> (InteractionEffect, Vec<DomNode>) {
    let new_boxes: Vec<_> = after
        .observation
        .boxes
        .iter()
        .filter(|a| {
            !base.observation.boxes.iter().any(|b| {
                b.text == a.text && rects_close([b.x, b.y, b.w, b.h], [a.x, a.y, a.w, a.h], 8)
            })
        })
        .collect();

    let new_candidates: Vec<_> = after
        .candidates
        .iter()
        .filter(|a| {
            !base
                .candidates
                .iter()
                .any(|b| rects_close(b.bounds, a.bounds, 8))
        })
        .collect();

    let retained_ratio = if !base.observation.boxes.is_empty() {
        let matched = base
            .observation
            .boxes
            .iter()
            .filter(|b| {
                after.observation.boxes.iter().any(|a| {
                    a.text == b.text && rects_close([b.x, b.y, b.w, b.h], [a.x, a.y, a.w, a.h], 8)
                })
            })
            .count();
        matched as f32 / base.observation.boxes.len() as f32
    } else if !base.candidates.is_empty() {
        let matched = base
            .candidates
            .iter()
            .filter(|b| {
                after
                    .candidates
                    .iter()
                    .any(|a| rects_close(b.bounds, a.bounds, 8))
            })
            .count();
        matched as f32 / base.candidates.len() as f32
    } else {
        1.0
    };

    let mut new_nodes: Vec<DomNode> = new_candidates
        .iter()
        .enumerate()
        .map(|(idx, c)| {
            let role = if c.permission == Permission::Allowed {
                DomRole::Button
            } else {
                DomRole::Container
            };
            let mut node = DomNode::new(
                format!(
                    "dyn_{}_{}_{}_{}",
                    after.observation.frame_id, idx, c.bounds[0], c.bounds[1]
                ),
                role,
                c.bounds,
                "",
            );
            node.interactive = c.permission == Permission::Allowed;
            node
        })
        .collect();
    let candidate_count = new_nodes.len();

    for b in &new_boxes {
        let b_bounds = [b.x, b.y, b.w, b.h];
        let host = new_nodes[..candidate_count]
            .iter_mut()
            .filter(|node| contains(node.bounds, b_bounds))
            .min_by_key(|node| u32::from(node.bounds[2]) * u32::from(node.bounds[3]));
        if let Some(parent) = host {
            if !parent.text.is_empty() {
                parent.text.push(' ');
            }
            parent.text.push_str(&b.text);
        } else {
            new_nodes.push(DomNode::new(
                format!("dynamic_{}_{}", b.x, b.y),
                DomRole::Text,
                b_bounds,
                &b.text,
            ));
        }
    }

    let mut adjacent_nodes = Vec::new();
    let mut remaining = Vec::new();
    for node in new_nodes {
        if contains(target_bounds, node.bounds) || is_adjacent(target_bounds, node.bounds) {
            adjacent_nodes.push(node);
        } else {
            remaining.push(node);
        }
    }

    loop {
        let mut progressed = false;
        let mut next_remaining = Vec::new();
        for node in remaining {
            if adjacent_nodes
                .iter()
                .any(|seed| boxes_near(seed.bounds, node.bounds, 48))
            {
                adjacent_nodes.push(node);
                progressed = true;
            } else {
                next_remaining.push(node);
            }
        }
        remaining = next_remaining;
        if !progressed {
            break;
        }
    }

    if !adjacent_nodes.is_empty() && retained_ratio >= 0.45 {
        return (
            InteractionEffect::ChildExpansion {
                added_nodes: adjacent_nodes.len(),
            },
            adjacent_nodes,
        );
    }

    let state_changed = match (&base.state, &after.state) {
        (Some(b), Some(a)) if b != a => Some(a.clone()),
        _ => None,
    };

    if let Some(next_state) = &state_changed {
        if retained_ratio < 0.45 || (new_boxes.is_empty() && new_candidates.is_empty()) {
            return (
                InteractionEffect::StateTransition {
                    next_state: next_state.clone(),
                },
                vec![],
            );
        }
    }

    if (!new_boxes.is_empty() || !new_candidates.is_empty()) && retained_ratio >= 0.45 {
        if (remaining.len() >= 2 || !new_candidates.is_empty()) && !remaining.is_empty() {
            let mut min_x = u32::MAX;
            let mut min_y = u32::MAX;
            let mut max_r = 0u32;
            let mut max_b = 0u32;
            for node in &remaining {
                let [x, y, w, h] = node.bounds.map(u32::from);
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_r = max_r.max(x + w);
                max_b = max_b.max(y + h);
            }
            let union_bounds = [
                min_x as u16,
                min_y as u16,
                (max_r.saturating_sub(min_x)) as u16,
                (max_b.saturating_sub(min_y)) as u16,
            ];
            let title = remaining
                .iter()
                .find(|n| !n.text.trim().is_empty())
                .map(|n| n.text.clone())
                .unwrap_or_else(|| "dialog".into());
            let mut modal = DomNode::new(
                format!("modal_{}", after.observation.frame_id),
                DomRole::ModalDialog,
                union_bounds,
                "",
            );
            modal.children = remaining;
            return (InteractionEffect::ModalOpened { title }, vec![modal]);
        }

        let detail = remaining
            .iter()
            .map(|n| n.text.as_str())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        return (InteractionEffect::ContentMutation { detail }, vec![]);
    }

    if let Some(next_state) = state_changed {
        return (InteractionEffect::StateTransition { next_state }, vec![]);
    }

    if retained_ratio < 1.0 {
        (InteractionEffect::Toggle, vec![])
    } else {
        (InteractionEffect::Static, vec![])
    }
}

fn is_adjacent(a: [u16; 4], b: [u16; 4]) -> bool {
    let [ax, ay, aw, ah] = a.map(u32::from);
    let [bx, by, bw, bh] = b.map(u32::from);

    let horiz_overlap = bx <= ax + aw + 120 && ax <= bx + bw + 120;
    let below = by >= (ay + ah).saturating_sub(12) && by <= ay + ah + 300;
    let above = by + bh <= ay + 12 && ay <= by + bh + 300;
    let vertical_exp = horiz_overlap && (below || above);

    let vert_overlap = by <= ay + ah + 100 && ay <= by + bh + 100;
    let right = bx >= (ax + aw).saturating_sub(12) && bx <= ax + aw + 260;
    let left = bx + bw <= ax + 12 && ax <= bx + bw + 260;
    let horizontal_exp = vert_overlap && (right || left);

    vertical_exp || horizontal_exp
}

fn apply_node_delta(
    parent: &mut DomNode,
    target_bounds: [u16; 4],
    effect: InteractionEffect,
    dynamic_children: &mut Option<Vec<DomNode>>,
) -> bool {
    if parent.bounds == target_bounds {
        parent.effect = effect;
        if let Some(children) = dynamic_children.take() {
            for child in children {
                if !parent
                    .children
                    .iter()
                    .any(|c| c.bounds == child.bounds && c.text == child.text)
                {
                    parent.children.push(child);
                }
            }
        }
        return true;
    }

    for child in &mut parent.children {
        if (contains(child.bounds, target_bounds) || child.bounds == target_bounds)
            && apply_node_delta(child, target_bounds, effect.clone(), dynamic_children)
        {
            return true;
        }
    }
    for child in &mut parent.children {
        if apply_node_delta(child, target_bounds, effect.clone(), dynamic_children) {
            return true;
        }
    }
    false
}
