//! Bounded exploration of explicitly permitted UI actions. No hardware dependency.
use crate::execution::{Observation, validate_frame};
use std::{
    collections::{HashSet, VecDeque},
    time::Duration,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    Allowed,
    Denied,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct Candidate {
    /// Stable action/locator identifier; never raw OCR text.
    pub key: String,
    pub frame_id: u64,
    pub bounds: [u16; 4],
    pub permission: Permission,
}
impl Candidate {
    pub fn point(&self) -> (u16, u16) {
        (
            self.bounds[0] + self.bounds[2] / 2,
            self.bounds[1] + self.bounds[3] / 2,
        )
    }
}

#[derive(Debug, Clone)]
pub struct Scene {
    pub observation: Observation,
    /// Adapter-verified state identity, including relevant context. None = unknown.
    pub state: Option<String>,
    pub candidates: Vec<Candidate>,
}

/// The adapter supplies state semantics, candidate generation and an explicitly
/// permitted reset. Restoration is verified by observation, never by return code.
pub trait Environment {
    fn now(&self) -> Duration;
    fn observe(&mut self) -> Result<Scene, String>;
    fn act(&mut self, candidate: &Candidate) -> Result<(), String>;
    fn restore(&mut self) -> Result<(), String>;
    fn pause(&mut self) {}
}

pub struct Limits {
    pub timeout: Duration,
    pub max_age: Duration,
    pub max_inputs: usize,
    pub max_probes: usize,
    pub max_states: usize,
    pub max_frames: usize,
    pub max_reobservations: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(180),
            max_age: Duration::from_secs(30),
            max_inputs: 100,
            max_probes: 30,
            max_states: 64,
            max_frames: 300,
            max_reobservations: 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    Complete,
    InvalidLimits,
    Timeout,
    InputLimit,
    ProbeLimit,
    StateLimit,
    FrameLimit,
    ObservationError,
    InvalidFrame,
    UnknownState,
    InvalidCandidate,
    InputError,
    RestoreMismatch,
    RouteMismatch,
}
impl Stop {
    pub fn code(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::InvalidLimits => "invalid_limits",
            Self::Timeout => "timeout",
            Self::InputLimit => "input_limit",
            Self::ProbeLimit => "probe_limit",
            Self::StateLimit => "state_limit",
            Self::FrameLimit => "frame_limit",
            Self::ObservationError => "observation_error",
            Self::InvalidFrame => "invalid_frame",
            Self::UnknownState => "unknown_state",
            Self::InvalidCandidate => "invalid_candidate",
            Self::InputError => "input_error",
            Self::RestoreMismatch => "restore_mismatch",
            Self::RouteMismatch => "route_mismatch",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Hop {
    pub action: String,
    pub expected_state: String,
}
#[derive(Debug)]
pub struct Node {
    pub state: String,
    pub path: Vec<Hop>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeOutcome {
    Transition,
    SameObservedState,
    Unconfirmed,
    InputError,
}
impl EdgeOutcome {
    pub fn code(self) -> &'static str {
        match self {
            Self::Transition => "transition",
            Self::SameObservedState => "same_observed_state",
            Self::Unconfirmed => "unconfirmed",
            Self::InputError => "input_error",
        }
    }
}
#[derive(Debug)]
pub struct Edge {
    pub from: usize,
    pub to: Option<usize>,
    pub action: String,
    pub point: (u16, u16),
    pub before_frame: u64,
    pub after_frame: Option<u64>,
    pub outcome: EdgeOutcome,
}
#[derive(Debug, Default)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}
#[derive(Debug)]
pub struct Skipped {
    pub node: usize,
    pub candidate: String,
    pub reason: &'static str,
}
#[derive(Debug)]
pub struct Attempt {
    pub kind: &'static str,
    pub action: String,
    pub frame: Option<u64>,
    pub point: Option<(u16, u16)>,
    pub acknowledged: bool,
}
#[derive(Debug)]
pub struct ExplorationReport {
    pub graph: Graph,
    pub stop: Stop,
    pub probes: usize,
    pub input_attempts: usize,
    pub observed_frames: usize,
    pub restored: bool,
    pub elapsed: Duration,
    pub skipped: Vec<Skipped>,
    pub attempts: Vec<Attempt>,
    pub pending: Vec<(usize, String)>,
}

pub fn explore(env: &mut impl Environment, limits: &Limits) -> ExplorationReport {
    let started = env.now();
    let mut runner = Runner {
        env,
        limits,
        started,
        last_now: started,
        previous: None,
        after: None,
        frontier: VecDeque::new(),
        report: ExplorationReport {
            graph: Graph::default(),
            stop: Stop::InvalidLimits,
            probes: 0,
            input_attempts: 0,
            observed_frames: 0,
            restored: false,
            elapsed: Duration::ZERO,
            skipped: vec![],
            attempts: vec![],
            pending: vec![],
        },
    };
    runner.report.stop = runner.run().err().unwrap_or(Stop::Complete);
    runner.report.pending = runner.frontier.into_iter().collect();
    runner.report.elapsed = runner.env.now().saturating_sub(started);
    runner.report
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-.:".contains(&b))
}

struct Runner<'a, E> {
    env: &'a mut E,
    limits: &'a Limits,
    started: Duration,
    last_now: Duration,
    previous: Option<(u64, Duration)>,
    after: Option<Duration>,
    frontier: VecDeque<(usize, String)>,
    report: ExplorationReport,
}
impl<E: Environment> Runner<'_, E> {
    fn time_check(&mut self) -> Result<Duration, Stop> {
        let now = self.env.now();
        if now < self.last_now {
            return Err(Stop::InvalidFrame);
        }
        self.last_now = now;
        if now.saturating_sub(self.started) >= self.limits.timeout {
            return Err(Stop::Timeout);
        }
        Ok(now)
    }

    fn read(&mut self) -> Result<Scene, Stop> {
        self.read_impl(false)
    }

    fn read_after_input(&mut self) -> Result<Scene, Stop> {
        self.read_impl(true)
    }

    fn read_impl(&mut self, force_first: bool) -> Result<Scene, Stop> {
        for retry in 0..=self.limits.max_reobservations {
            match self.time_check() {
                Err(Stop::Timeout) if force_first && retry == 0 => {}
                other => {
                    other?;
                }
            }
            if self.report.observed_frames >= self.limits.max_frames {
                return Err(Stop::FrameLimit);
            }
            self.report.observed_frames += 1;
            let result = self.env.observe();
            let now = self.time_check()?;
            let scene = result.map_err(|_| Stop::ObservationError)?;
            validate_frame(
                &scene.observation,
                now,
                self.previous,
                self.after,
                self.limits.max_age,
            )
            .map_err(|_| Stop::InvalidFrame)?;
            self.previous = Some((scene.observation.frame_id, scene.observation.captured_at));
            if let Some(state) = &scene.state {
                if !valid_id(state) {
                    return Err(Stop::InvalidFrame);
                }
                return Ok(scene);
            }
            if retry < self.limits.max_reobservations {
                self.env.pause();
            }
        }
        Err(Stop::UnknownState)
    }

    fn add_node(&mut self, scene: &Scene, path: Vec<Hop>) -> Result<usize, Stop> {
        let state = scene.state.as_ref().ok_or(Stop::UnknownState)?;
        if let Some(id) = self
            .report
            .graph
            .nodes
            .iter()
            .position(|n| n.state == *state)
        {
            return Ok(id);
        }
        if self.report.graph.nodes.len() >= self.limits.max_states {
            return Err(Stop::StateLimit);
        }
        let id = self.report.graph.nodes.len();
        let mut keys = HashSet::new();
        for candidate in &scene.candidates {
            if !valid_id(&candidate.key) || !keys.insert(candidate.key.clone()) {
                return Err(Stop::InvalidCandidate);
            }
        }
        self.report.graph.nodes.push(Node {
            state: state.clone(),
            path,
        });
        for c in &scene.candidates {
            if c.permission == Permission::Allowed {
                self.frontier.push_back((id, c.key.clone()));
            } else {
                self.report.skipped.push(Skipped {
                    node: id,
                    candidate: c.key.clone(),
                    reason: if c.permission == Permission::Denied {
                        "denied"
                    } else {
                        "unknown_permission"
                    },
                });
            }
        }
        Ok(id)
    }

    fn resolve(&mut self, scene: &Scene, key: &str) -> Result<Candidate, Stop> {
        let now = self.time_check()?;
        validate_frame(
            &scene.observation,
            now,
            None,
            self.after,
            self.limits.max_age,
        )
        .map_err(|_| Stop::InvalidFrame)?;
        let candidates: Vec<_> = scene.candidates.iter().filter(|c| c.key == key).collect();
        if candidates.len() != 1 {
            return Err(Stop::InvalidCandidate);
        }
        let c = candidates[0];
        let [x, y, w, h] = c.bounds;
        if c.permission != Permission::Allowed
            || c.frame_id != scene.observation.frame_id
            || w == 0
            || h == 0
            || u32::from(x) + u32::from(w) > u32::from(scene.observation.width)
            || u32::from(y) + u32::from(h) > u32::from(scene.observation.height)
        {
            return Err(Stop::InvalidCandidate);
        }
        Ok(c.clone())
    }

    fn input(&mut self, candidate: Option<&Candidate>, kind: &'static str) -> Result<bool, Stop> {
        self.time_check()?;
        // Admit no input unless a post-input observation can be attempted.
        if self.report.observed_frames >= self.limits.max_frames {
            return Err(Stop::FrameLimit);
        }
        if self.report.input_attempts >= self.limits.max_inputs {
            return Err(Stop::InputLimit);
        }
        self.report.input_attempts += 1;
        self.report.restored = false;
        let result = if let Some(c) = candidate {
            self.env.act(c)
        } else {
            self.env.restore()
        };
        let acknowledged = result.is_ok();
        self.report.attempts.push(Attempt {
            kind,
            action: candidate.map(|c| c.key.clone()).unwrap_or("restore".into()),
            frame: candidate.map(|c| c.frame_id),
            point: candidate.map(Candidate::point),
            acknowledged,
        });
        // A failed write may have partially executed. Never replay it.
        let ended = self.env.now();
        if ended < self.last_now {
            return Err(Stop::InvalidFrame);
        }
        self.last_now = ended;
        self.after = Some(ended);
        Ok(acknowledged)
    }

    fn restore(&mut self, root: &str) -> Result<Scene, Stop> {
        let acknowledged = self.input(None, "restore")?;
        let result = self.read_after_input();
        if !acknowledged {
            return Err(Stop::InputError);
        }
        let scene = result?;
        if scene.state.as_deref() != Some(root) {
            return Err(Stop::RestoreMismatch);
        }
        self.report.restored = true;
        Ok(scene)
    }

    fn run(&mut self) -> Result<(), Stop> {
        if self.limits.timeout.is_zero()
            || self.limits.max_age.is_zero()
            || self.limits.max_inputs == 0
            || self.limits.max_probes == 0
            || self.limits.max_states == 0
            || self.limits.max_frames == 0
        {
            return Err(Stop::InvalidLimits);
        }
        let mut current = self.read()?;
        let root = current.state.clone().ok_or(Stop::UnknownState)?;
        self.add_node(&current, vec![])?;
        self.report.restored = true;
        while let Some((from, key)) = self.frontier.front().cloned() {
            self.time_check()?;
            if self.report.probes >= self.limits.max_probes {
                return Err(Stop::ProbeLimit);
            }
            let path = self.report.graph.nodes[from].path.clone();
            for hop in &path {
                let candidate = self.resolve(&current, &hop.action)?;
                let acknowledged = self.input(Some(&candidate), "navigate")?;
                let result = self.read_after_input();
                if !acknowledged {
                    return Err(Stop::InputError);
                }
                current = result?;
                if current.state.as_deref() != Some(&hop.expected_state) {
                    return Err(Stop::RouteMismatch);
                }
            }
            if current.state.as_deref() != Some(&self.report.graph.nodes[from].state) {
                return Err(Stop::RouteMismatch);
            }
            let candidate = self.resolve(&current, &key)?;
            let acknowledged = self.input(Some(&candidate), "probe")?;
            self.report.probes += 1;
            self.frontier.pop_front();
            let observed = self.read_after_input();
            let mut edge = Edge {
                from,
                to: None,
                action: key.clone(),
                point: candidate.point(),
                before_frame: current.observation.frame_id,
                after_frame: self
                    .previous
                    .filter(|(id, _)| *id > current.observation.frame_id)
                    .map(|(id, _)| id),
                outcome: if acknowledged {
                    EdgeOutcome::Unconfirmed
                } else {
                    EdgeOutcome::InputError
                },
            };
            if !acknowledged {
                self.report.graph.edges.push(edge);
                return Err(Stop::InputError);
            }
            let next = match observed {
                Ok(scene) => scene,
                Err(reason) => {
                    self.report.graph.edges.push(edge);
                    return Err(reason);
                }
            };
            let next_state = next.state.clone().ok_or(Stop::UnknownState)?;
            let mut next_path = path;
            next_path.push(Hop {
                action: key,
                expected_state: next_state.clone(),
            });
            let to = match self.add_node(&next, next_path) {
                Ok(id) => id,
                Err(reason) => {
                    self.report.graph.edges.push(edge);
                    return Err(reason);
                }
            };
            edge.to = Some(to);
            edge.outcome = if from == to {
                EdgeOutcome::SameObservedState
            } else {
                EdgeOutcome::Transition
            };
            self.report.graph.edges.push(edge);
            current = self.restore(&root)?;
        }
        Ok(())
    }
}
