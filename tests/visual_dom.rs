use std::{process::Command, time::Duration};
use windows_control::{
    execution::Observation,
    exploration::{Candidate, Environment, Permission, Scene},
    ocr::TextBox,
    visual_dom::{
        DomBuildConfig, DomNode, DomRole, DomStop, InteractionEffect, build_geometric_tree,
        interact_and_build_dom, interact_and_build_dom_with_report, rewrap_with_containers,
    },
    visual_dom_live::filter_confident_boxes,
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

#[derive(Clone)]
struct ScriptedStep {
    state: Option<String>,
    boxes: Vec<TextBox>,
    candidates: Vec<Candidate>,
}

struct ConfigurableEnv {
    step: usize,
    restore_calls: usize,
    act_calls: Vec<String>,
    fail_on_act_call: Option<usize>,
    restore_override: Option<ScriptedStep>,
    base: ScriptedStep,
    current: ScriptedStep,
    on_act: Box<dyn FnMut(&Candidate, usize) -> Option<ScriptedStep>>,
}

impl ConfigurableEnv {
    fn new(
        base: ScriptedStep,
        on_act: impl FnMut(&Candidate, usize) -> Option<ScriptedStep> + 'static,
    ) -> Self {
        Self {
            step: 1,
            restore_calls: 0,
            act_calls: vec![],
            fail_on_act_call: None,
            restore_override: None,
            current: base.clone(),
            base,
            on_act: Box::new(on_act),
        }
    }
}

impl Environment for ConfigurableEnv {
    fn now(&self) -> Duration {
        Duration::from_millis(self.step as u64 * 100)
    }

    fn observe(&mut self) -> Result<Scene, String> {
        let frame_id = self.step as u64;
        self.step += 1;
        let candidates = self
            .current
            .candidates
            .iter()
            .cloned()
            .map(|mut c| {
                c.frame_id = frame_id;
                c
            })
            .collect();
        Ok(Scene {
            observation: Observation {
                frame_id,
                captured_at: self.now(),
                width: 1920,
                height: 1080,
                boxes: self.current.boxes.clone(),
            },
            state: self.current.state.clone(),
            candidates,
        })
    }

    fn act(&mut self, candidate: &Candidate) -> Result<(), String> {
        let call_index = self.act_calls.len();
        if self.fail_on_act_call == Some(call_index) {
            return Err("Simulated HID write error".into());
        }
        self.act_calls.push(candidate.key.clone());
        if let Some(next) = (self.on_act)(candidate, call_index) {
            self.current = next;
        }
        Ok(())
    }

    fn restore(&mut self) -> Result<(), String> {
        self.restore_calls += 1;
        self.current = self
            .restore_override
            .clone()
            .unwrap_or_else(|| self.base.clone());
        Ok(())
    }
}

fn tb(text: &str, x: u16, y: u16, w: u16, h: u16, confidence: f32) -> TextBox {
    TextBox {
        text: text.into(),
        x,
        y,
        w,
        h,
        confidence,
    }
}

fn cand(key: &str, bounds: [u16; 4]) -> Candidate {
    Candidate {
        key: key.into(),
        frame_id: 0,
        bounds,
        permission: Permission::Allowed,
    }
}

#[test]
fn test_unknown_initial_state_refuses_hid_input() {
    let base = ScriptedStep {
        state: None,
        boxes: vec![tb("Start", 20, 1040, 60, 24, 95.0)],
        candidates: vec![cand("L000001_O0001_click", [10, 1030, 80, 40])],
    };
    let mut env = ConfigurableEnv::new(base, |_, _| None);

    let report = interact_and_build_dom_with_report(
        &mut env,
        &DomBuildConfig {
            max_interactions: 3,
            timeout: Duration::from_secs(30),
        },
    )
    .unwrap();

    assert_eq!(report.stop, DomStop::UnknownInitialState);
    assert_eq!(report.interactions_attempted, 0);
    assert!(env.act_calls.is_empty());
}

#[test]
fn test_no_change_click_is_static_with_jitter_and_skips_restore() {
    let base = ScriptedStep {
        state: Some("S000001".into()),
        boxes: vec![
            tb("Desktop", 100, 100, 90, 22, 95.0),
            tb("Files", 220, 100, 60, 22, 96.0),
        ],
        candidates: vec![cand("L000001_O0001_click", [90, 90, 110, 42])],
    };
    let jittered = ScriptedStep {
        state: Some("S000001".into()),
        boxes: vec![
            tb("Desktop", 102, 99, 89, 23, 95.0),
            tb("Files", 218, 101, 61, 21, 96.0),
        ],
        candidates: vec![cand("L000001_O0001_click", [92, 89, 109, 43])],
    };
    let mut env = ConfigurableEnv::new(base, move |_, _| Some(jittered.clone()));

    let report = interact_and_build_dom_with_report(&mut env, &DomBuildConfig::default()).unwrap();

    assert_eq!(report.stop, DomStop::Complete);
    assert_eq!(report.interactions_completed, 1);
    assert_eq!(env.restore_calls, 0);
    assert_eq!(report.dom.children[0].effect, InteractionEffect::Static);
}

#[test]
fn test_screen_analyzer_dropdown_creates_child_expansion_even_with_new_state_id() {
    let base = ScriptedStep {
        state: Some("S000001".into()),
        boxes: vec![
            tb("Start", 20, 1040, 60, 24, 95.0),
            tb("Browser", 200, 1040, 80, 24, 94.0),
            tb("Notes", 500, 120, 70, 24, 93.0),
        ],
        candidates: vec![cand("L000001_O0001_click", [10, 1030, 80, 40])],
    };
    let mut after = base.clone();
    after.state = Some("S000002".into());
    after.boxes.push(tb("Settings", 20, 880, 120, 28, 96.0));
    after.boxes.push(tb("Power", 20, 930, 90, 28, 96.0));
    after
        .candidates
        .push(cand("L000002_O0010_click", [15, 875, 130, 38]));

    let mut env = ConfigurableEnv::new(base, move |_, _| Some(after.clone()));
    let report = interact_and_build_dom_with_report(&mut env, &DomBuildConfig::default()).unwrap();

    assert_eq!(report.stop, DomStop::Complete);
    let start_btn = report
        .dom
        .children
        .iter()
        .find(|n| n.text == "Start")
        .unwrap();
    match &start_btn.effect {
        InteractionEffect::ChildExpansion { added_nodes } => assert_eq!(*added_nodes, 2),
        other => panic!("Expected ChildExpansion even on new S000002, got {other:?}"),
    }
    assert_eq!(start_btn.children.len(), 2);
}

#[test]
fn test_restore_mismatch_stops_immediately_and_saves_partial_dom() {
    let base = ScriptedStep {
        state: Some("S000001".into()),
        boxes: vec![
            tb("Open", 100, 100, 60, 24, 95.0),
            tb("Save", 300, 100, 60, 24, 95.0),
        ],
        candidates: vec![
            cand("L000001_O0001_click", [90, 90, 80, 44]),
            cand("L000001_O0002_click", [290, 90, 80, 44]),
        ],
    };
    let mut env = ConfigurableEnv::new(base, |_, _| {
        Some(ScriptedStep {
            state: Some("S000002".into()),
            boxes: vec![tb("NextScreen", 100, 100, 120, 24, 95.0)],
            candidates: vec![],
        })
    });
    env.restore_override = Some(ScriptedStep {
        state: Some("S000999".into()),
        boxes: vec![tb("Unexpected", 100, 100, 120, 24, 95.0)],
        candidates: vec![],
    });

    let report = interact_and_build_dom_with_report(
        &mut env,
        &DomBuildConfig {
            max_interactions: 2,
            timeout: Duration::from_secs(30),
        },
    )
    .unwrap();

    assert_eq!(report.stop, DomStop::RestoreMismatch);
    assert!(!report.restored);
    assert_eq!(env.act_calls, vec!["L000001_O0001_click"]);
    assert_eq!(
        report.dom.children[0].effect,
        InteractionEffect::StateTransition {
            next_state: "S000002".into()
        }
    );
}

#[test]
fn test_input_error_stops_without_retry() {
    let base = ScriptedStep {
        state: Some("S000001".into()),
        boxes: vec![tb("Open", 100, 100, 60, 24, 95.0)],
        candidates: vec![cand("L000001_O0001_click", [90, 90, 80, 44])],
    };
    let mut env = ConfigurableEnv::new(base, |_, _| None);
    env.fail_on_act_call = Some(0);

    let report = interact_and_build_dom_with_report(&mut env, &DomBuildConfig::default()).unwrap();

    assert_eq!(report.stop, DomStop::InputError);
    assert!(!report.restored);
    assert_eq!(env.restore_calls, 0);
    assert!(env.act_calls.is_empty());
}

#[test]
fn test_rewrap_with_containers_nests_buttons_inside_panels() {
    let mut root = DomNode::new("root", DomRole::Root, [0, 0, 1920, 1080], "");
    let mut inner_btn = DomNode::new("elem_0", DomRole::Button, [120, 120, 80, 30], "Menu");
    inner_btn.interactive = true;
    inner_btn.effect = InteractionEffect::ChildExpansion { added_nodes: 1 };
    inner_btn.children.push(DomNode::new(
        "dynamic_120_160",
        DomRole::Text,
        [120, 160, 70, 20],
        "Item",
    ));
    let outside_btn = DomNode::new("elem_1", DomRole::Button, [900, 120, 80, 30], "Help");

    root.children.push(inner_btn);
    root.children.push(outside_btn);

    rewrap_with_containers(&mut root, &[("L000001_O0005".into(), [100, 100, 300, 250])]);

    let panel = root
        .children
        .iter()
        .find(|n| n.id == "L000001_O0005")
        .expect("panel container inserted");
    assert_eq!(panel.role, DomRole::Container);
    assert_eq!(panel.children.len(), 1);
    assert_eq!(panel.children[0].id, "elem_0");
    assert_eq!(
        panel.children[0].effect,
        InteractionEffect::ChildExpansion { added_nodes: 1 }
    );
    assert_eq!(panel.children[0].children.len(), 1);
    assert!(root.children.iter().any(|n| n.id == "elem_1"));
}

#[test]
fn test_filter_confident_boxes_drops_low_confidence_icon_noise() {
    let input = vec![
        tb("ValidWord", 10, 10, 80, 20, 85.0),
        tb("LowConf", 100, 10, 80, 20, 69.9),
        tb(".", 200, 10, 3, 20, 99.0),
        tb("-", 220, 10, 20, 4, 99.0),
    ];

    let kept = filter_confident_boxes(input, 70.0);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].text, "ValidWord");
}

#[test]
fn test_build_dom_cli_rejects_invalid_args_before_opening_devices() {
    let over_limit = Command::new(env!("CARGO_BIN_EXE_windows_control"))
        .args(["build-dom", "unused-dir", "31"])
        .output()
        .unwrap();
    assert!(!over_limit.status.success());
    let stderr_limit = String::from_utf8_lossy(&over_limit.stderr);
    assert!(
        stderr_limit.contains("max-interactions must be 0..=30"),
        "{stderr_limit}"
    );
    assert!(!stderr_limit.contains("video0"));

    let bad_restore = Command::new(env!("CARGO_BIN_EXE_windows_control"))
        .args(["build-dom", "unused-dir", "2", "invalid"])
        .output()
        .unwrap();
    assert!(!bad_restore.status.success());
    let stderr_restore = String::from_utf8_lossy(&bad_restore.stderr);
    assert!(
        stderr_restore.contains("Restore mode must be esc, ctrl-r, or none"),
        "{stderr_restore}"
    );
    assert!(!stderr_restore.contains("video0"));
}
