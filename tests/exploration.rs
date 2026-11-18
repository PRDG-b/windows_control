use std::time::Duration;
use windows_control::{execution::Observation, exploration::*};

struct World {
    state: &'static str,
    id: u64,
    time: Duration,
    clicks: Vec<String>,
    resets: usize,
    scenario: &'static str,
}
impl World {
    fn new(scenario: &'static str) -> Self {
        Self {
            state: "home",
            id: 0,
            time: Duration::ZERO,
            clicks: vec![],
            resets: 0,
            scenario,
        }
    }
}
impl Environment for World {
    fn now(&self) -> Duration {
        self.time
    }
    fn observe(&mut self) -> Result<Scene, String> {
        self.time += Duration::from_millis(1);
        self.id += 1;
        let keys = match self.state {
            "home" => vec![
                ("open", Permission::Allowed),
                ("delete", Permission::Denied),
                ("noop", Permission::Allowed),
            ],
            "details" => vec![("expand", Permission::Allowed)],
            _ => vec![],
        };
        Ok(Scene {
            observation: Observation {
                frame_id: self.id,
                captured_at: self.time,
                width: 800,
                height: 600,
                boxes: vec![],
            },
            state: if self.scenario == "initial-unknown"
                || (self.scenario == "unknown" && !self.clicks.is_empty())
            {
                None
            } else {
                Some(self.state.into())
            },
            candidates: keys
                .into_iter()
                .map(|(key, permission)| Candidate {
                    key: key.into(),
                    frame_id: if self.scenario == "stale" {
                        self.id - 1
                    } else {
                        self.id
                    },
                    bounds: [20 + self.id as u16, 30, 40, 20],
                    permission,
                })
                .collect(),
        })
    }
    fn act(&mut self, candidate: &Candidate) -> Result<(), String> {
        assert_eq!(candidate.frame_id, self.id, "old coordinates reused");
        self.clicks.push(candidate.key.clone());
        self.time += Duration::from_millis(1);
        if self.scenario == "slow" {
            self.time += Duration::from_secs(300);
        }
        if self.scenario == "input-error" {
            return Err("partially written".into());
        }
        self.state = match candidate.key.as_str() {
            "open" => "details",
            "expand" => "expanded",
            "noop" => "home",
            _ => panic!("forbidden input"),
        };
        Ok(())
    }
    fn restore(&mut self) -> Result<(), String> {
        self.resets += 1;
        self.time += Duration::from_millis(1);
        self.state = if self.scenario == "restore-error" {
            "wrong"
        } else {
            "home"
        };
        Ok(())
    }
}

#[test]
fn discovers_graph_reaches_deeper_states_and_restores_after_each_probe() {
    let mut world = World::new("normal");
    let report = explore(&mut world, &Limits::default());
    assert_eq!(report.stop, Stop::Complete);
    assert_eq!(report.graph.nodes.len(), 3);
    assert_eq!(report.graph.edges.len(), 3);
    assert!(
        report
            .graph
            .edges
            .iter()
            .any(|e| e.outcome == EdgeOutcome::SameObservedState)
    );
    assert_eq!(world.clicks, vec!["open", "noop", "open", "expand"]);
    assert_eq!(world.resets, 3);
    assert!(report.restored);
    assert_eq!(report.probes, 3);
    assert_eq!(report.input_attempts, 7);
    assert!(report.skipped.iter().any(|s| s.candidate == "delete"));
}

#[test]
fn unclear_result_never_reclicks_or_resets_uncertain_state() {
    let mut world = World::new("unknown");
    let report = explore(&mut world, &Limits::default());
    assert_eq!(report.stop, Stop::UnknownState);
    assert_eq!(world.clicks, vec!["open"]);
    assert_eq!(world.resets, 0);
    assert!(!report.restored);
    assert_eq!(report.graph.edges[0].outcome, EdgeOutcome::Unconfirmed);
}

#[test]
fn failed_input_and_restore_mismatch_stop_without_retry() {
    for (scenario, reason) in [
        ("input-error", Stop::InputError),
        ("restore-error", Stop::RestoreMismatch),
    ] {
        let mut world = World::new(scenario);
        let report = explore(&mut world, &Limits::default());
        assert_eq!(report.stop, reason);
        assert_eq!(world.clicks, vec!["open"]);
        assert!(!report.restored);
    }
}

#[test]
fn budgets_and_stale_targets_prevent_unbounded_or_invalid_input() {
    let mut world = World::new("normal");
    let report = explore(
        &mut world,
        &Limits {
            max_probes: 1,
            ..Limits::default()
        },
    );
    assert_eq!(report.stop, Stop::ProbeLimit);
    assert_eq!(report.probes, 1);
    assert!(report.restored);
    let mut world = World::new("normal");
    let report = explore(
        &mut world,
        &Limits {
            max_inputs: 1,
            ..Limits::default()
        },
    );
    assert_eq!(report.stop, Stop::InputLimit);
    assert_eq!(world.clicks.len(), 1);
    assert_eq!(world.resets, 0);
    let mut world = World::new("stale");
    let report = explore(&mut world, &Limits::default());
    assert_eq!(report.stop, Stop::InvalidCandidate);
    assert!(world.clicks.is_empty());
}

#[test]
fn reserves_post_input_observation_and_captures_even_after_input_overruns_deadline() {
    let mut world = World::new("normal");
    let report = explore(
        &mut world,
        &Limits {
            max_frames: 1,
            ..Limits::default()
        },
    );
    assert_eq!(report.stop, Stop::FrameLimit);
    assert!(world.clicks.is_empty());
    let mut world = World::new("slow");
    let report = explore(&mut world, &Limits::default());
    assert_eq!(report.stop, Stop::Timeout);
    assert_eq!(world.clicks, vec!["open"]);
    assert_eq!(report.observed_frames, 2);
    assert!(!report.restored);
}

#[test]
fn unknown_initial_screen_never_receives_input() {
    let mut world = World::new("initial-unknown");
    let report = explore(&mut world, &Limits::default());
    assert_eq!(report.stop, Stop::UnknownState);
    assert_eq!(report.input_attempts, 0);
    assert_eq!(report.observed_frames, 3);
    assert!(world.clicks.is_empty());
    assert_eq!(world.resets, 0);
}
