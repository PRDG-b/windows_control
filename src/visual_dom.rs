//! Visual DOM: Reconstructs a DOM-like hierarchical structure by combining
//! geometric containment and live interactive probing with the environment.

use crate::exploration::{Candidate, Environment, Permission, Scene};
use std::fmt::Write as _;

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
            let _ = write!(html, " data-effect=\"{:?}\"", self.effect);
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

/// Probes the screen interactively to discover dynamic child relationships and behaviors,
/// producing a verified, live DOM-like structure.
pub fn interact_and_build_dom(
    env: &mut impl Environment,
    max_interactions: usize,
) -> Result<DomNode, String> {
    // Step 1: Initial observation & baseline geometric DOM
    let base_scene = env
        .observe()
        .map_err(|e| format!("Base observation failed: {e}"))?;
    let mut dom = build_geometric_tree(&base_scene);
    let root_state = base_scene.state.clone().unwrap_or_else(|| "root".into());

    // Step 2: Collect probeable interactive candidates
    let allowed_candidates: Vec<Candidate> = base_scene
        .candidates
        .iter()
        .filter(|c| c.permission == Permission::Allowed)
        .take(max_interactions)
        .cloned()
        .collect();

    // Step 3: Act on each candidate, observe mutations/children, and restore
    for candidate in allowed_candidates {
        let (_cx, _cy) = candidate.point();

        // Perform the click action
        if env.act(&candidate).is_err() {
            continue;
        }

        // Observe the immediate reaction frame
        let after_scene = match env.observe() {
            Ok(s) => s,
            Err(_) => {
                let _ = env.restore();
                continue;
            }
        };

        // Determine effect by comparing baseline and post-action scenes
        let effect = detect_interaction_effect(&base_scene, &after_scene, candidate.bounds);

        // Update the corresponding node in the DOM tree
        update_node_effect_and_children(
            &mut dom,
            candidate.bounds,
            effect,
            &base_scene,
            &after_scene,
        );

        // Safely restore back to the root baseline for the next probe
        let _ = env.restore();
        // Wait/pause for stability if supported
        env.pause();

        // Verify restoration before next action
        if let Ok(restored_scene) = env.observe() {
            if restored_scene.state.as_deref() != Some(&root_state) {
                // If restore deviated, stop further probing to avoid side-effects
                break;
            }
        } else {
            break;
        }
    }

    Ok(dom)
}

fn detect_interaction_effect(
    base: &Scene,
    after: &Scene,
    target_bounds: [u16; 4],
) -> InteractionEffect {
    // 1. Check if the screen transitioned to an entirely new state
    if let (Some(b_state), Some(a_state)) = (&base.state, &after.state) {
        if b_state != a_state {
            return InteractionEffect::StateTransition {
                next_state: a_state.clone(),
            };
        }
    }

    // 2. Check for newly appeared OCR text or elements
    let base_words: Vec<&str> = base
        .observation
        .boxes
        .iter()
        .map(|b| b.text.as_str())
        .collect();
    let new_words: Vec<&str> = after
        .observation
        .boxes
        .iter()
        .map(|b| b.text.as_str())
        .filter(|txt| !base_words.contains(txt))
        .collect();

    if !new_words.is_empty() {
        // If new words are nearby or within target, treat as ChildExpansion
        let nearby_count = after
            .observation
            .boxes
            .iter()
            .filter(|b| {
                !base_words.contains(&b.text.as_str())
                    && (contains(target_bounds, [b.x, b.y, b.w, b.h])
                        || is_adjacent(target_bounds, [b.x, b.y, b.w, b.h]))
            })
            .count();

        if nearby_count > 0 {
            return InteractionEffect::ChildExpansion {
                added_nodes: nearby_count,
            };
        }

        return InteractionEffect::ContentMutation {
            detail: new_words.join(" "),
        };
    }

    // 3. Fallback: check if candidates count changed
    if after.candidates.len() > base.candidates.len() {
        InteractionEffect::ChildExpansion {
            added_nodes: after.candidates.len() - base.candidates.len(),
        }
    } else {
        InteractionEffect::Toggle
    }
}

fn is_adjacent(a: [u16; 4], b: [u16; 4]) -> bool {
    // Checks if b is directly below or adjacent to a within 150px
    let a_bottom = u32::from(a[1]) + u32::from(a[3]);
    let b_top = u32::from(b[1]);
    b_top >= a_bottom.saturating_sub(10) && b_top <= a_bottom + 150
}

fn update_node_effect_and_children(
    parent: &mut DomNode,
    target_bounds: [u16; 4],
    effect: InteractionEffect,
    base_scene: &Scene,
    after_scene: &Scene,
) {
    if parent.bounds == target_bounds {
        parent.effect = effect.clone();
        if let InteractionEffect::ChildExpansion { added_nodes: _ } = effect {
            let base_boxes: Vec<[u16; 4]> = base_scene
                .observation
                .boxes
                .iter()
                .map(|b| [b.x, b.y, b.w, b.h])
                .collect();
            // Find newly appeared elements in after_scene that belong under this node
            for b in &after_scene.observation.boxes {
                let box_bounds = [b.x, b.y, b.w, b.h];
                if (contains(parent.bounds, box_bounds) || is_adjacent(parent.bounds, box_bounds))
                    && !base_boxes.contains(&box_bounds)
                    && !parent.children.iter().any(|c| c.bounds == box_bounds)
                {
                    parent.children.push(DomNode::new(
                        format!("dynamic_{}_{}", b.x, b.y),
                        DomRole::Text,
                        box_bounds,
                        &b.text,
                    ));
                }
            }
        }
        return;
    }

    for child in &mut parent.children {
        update_node_effect_and_children(
            child,
            target_bounds,
            effect.clone(),
            base_scene,
            after_scene,
        );
    }
}
