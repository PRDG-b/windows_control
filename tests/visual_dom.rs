use std::time::Duration;
use windows_control::{
    execution::Observation,
    exploration::{Candidate, Environment, Permission, Scene},
    ocr::TextBox,
    visual_dom::{DomRole, InteractionEffect, build_geometric_tree, interact_and_build_dom},
};

struct MockInteractiveApp {
    step: usize,
    state: String,
    clicked_keys: Vec<String>,
}

impl Environment for MockInteractiveApp {
    fn now(&self) -> Duration {
        Duration::from_millis(self.step as u64 * 100)
    }

    fn observe(&mut self) -> Result<Scene, String> {
        let mut boxes = vec![
            TextBox {
                text: "Menu".into(),
                x: 100,
                y: 50,
                w: 80,
                h: 30,
                confidence: 99.0,
            },
            TextBox {
                text: "Submit".into(),
                x: 300,
                y: 50,
                w: 90,
                h: 30,
                confidence: 98.0,
            },
        ];

        let candidates = vec![
            Candidate {
                key: "btn_menu".into(),
                frame_id: self.step as u64,
                bounds: [90, 40, 100, 50],
                permission: Permission::Allowed,
            },
            Candidate {
                key: "btn_submit".into(),
                frame_id: self.step as u64,
                bounds: [290, 40, 110, 50],
                permission: Permission::Allowed,
            },
        ];

        // If Menu was clicked, dynamic dropdown child items appear directly below it
        if self.clicked_keys.contains(&"btn_menu".to_string()) {
            boxes.push(TextBox {
                text: "Item 1".into(),
                x: 100,
                y: 100,
                w: 70,
                h: 25,
                confidence: 95.0,
            });
            boxes.push(TextBox {
                text: "Item 2".into(),
                x: 100,
                y: 130,
                w: 70,
                h: 25,
                confidence: 95.0,
            });
        }

        // If Submit was clicked, state transitions to 'submitted'
        if self.clicked_keys.contains(&"btn_submit".to_string()) {
            self.state = "submitted".into();
        }

        Ok(Scene {
            observation: Observation {
                frame_id: self.step as u64,
                captured_at: Duration::from_millis(self.step as u64 * 100),
                width: 800,
                height: 600,
                boxes,
            },
            state: Some(self.state.clone()),
            candidates,
        })
    }

    fn act(&mut self, candidate: &Candidate) -> Result<(), String> {
        self.step += 1;
        self.clicked_keys.push(candidate.key.clone());
        Ok(())
    }

    fn restore(&mut self) -> Result<(), String> {
        self.step += 1;
        self.clicked_keys.clear();
        self.state = "main".into();
        Ok(())
    }
}

#[test]
fn test_static_geometric_dom_construction() {
    let mut app = MockInteractiveApp {
        step: 1,
        state: "main".into(),
        clicked_keys: vec![],
    };

    let scene = app.observe().unwrap();
    let dom = build_geometric_tree(&scene);

    assert_eq!(dom.role, DomRole::Root);
    assert_eq!(dom.bounds, [0, 0, 800, 600]);

    // Should have 2 button children corresponding to the 2 candidates
    assert_eq!(dom.children.len(), 2);
    assert!(
        dom.children
            .iter()
            .any(|c| c.role == DomRole::Button && c.text == "Menu")
    );
    assert!(
        dom.children
            .iter()
            .any(|c| c.role == DomRole::Button && c.text == "Submit")
    );

    let html = dom.to_html(0);
    assert!(html.contains("<main id=\"root\""));
    assert!(html.contains("<button id=\"elem_0\""));
    assert!(html.contains("Menu</button>"));
}

#[test]
fn test_live_interactive_dom_construction() {
    let mut app = MockInteractiveApp {
        step: 1,
        state: "main".into(),
        clicked_keys: vec![],
    };

    // Interact with up to 2 candidates
    let dom = interact_and_build_dom(&mut app, 2).unwrap();

    // Verify ASCII output format
    let ascii = dom.to_ascii_tree(0);
    println!("Visual DOM Tree:\n{}", ascii);

    // Menu button should have expanded dynamic child nodes!
    let menu_node = dom.children.iter().find(|c| c.text == "Menu").unwrap();
    match &menu_node.effect {
        InteractionEffect::ChildExpansion { added_nodes } => {
            assert_eq!(*added_nodes, 2);
        }
        other => panic!("Expected ChildExpansion, got {:?}", other),
    }

    // Children of Menu should contain Item 1 and Item 2
    assert_eq!(menu_node.children.len(), 2);
    assert_eq!(menu_node.children[0].text, "Item 1");
    assert_eq!(menu_node.children[1].text, "Item 2");

    // Submit button should have triggered StateTransition
    let submit_node = dom.children.iter().find(|c| c.text == "Submit").unwrap();
    match &submit_node.effect {
        InteractionEffect::StateTransition { next_state } => {
            assert_eq!(next_state, "submitted");
        }
        other => panic!("Expected StateTransition, got {:?}", other),
    }

    // Verify HTML output contains interactive effects
    let html = dom.to_html(0);
    println!("Visual DOM HTML:\n{}", html);
    assert!(html.contains("data-effect=\"ChildExpansion"));
    assert!(html.contains("data-effect=\"StateTransition"));
}
