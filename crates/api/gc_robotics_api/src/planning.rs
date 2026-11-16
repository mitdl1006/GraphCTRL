//! Motion-planner and time-parameterisation plugins.

use gc_plugin_api::{ExtensionPoint, ParamSchema, ParamValues, Text};
use serde::{Deserialize, Serialize};

/// The space a planner searches: a box in joint space (or in R^3 for the tool-sphere mode).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpaceSpec {
    /// Lower bound per dimension.
    pub lo: Vec<f64>,
    /// Upper bound per dimension.
    pub hi: Vec<f64>,
    /// Cost weight per dimension; the distance between states is the weighted Euclidean norm.
    pub weights: Vec<f64>,
    /// True for dimensions that wrap around at their bounds (unlimited rotation).
    pub wrapping: Vec<bool>,
}

impl SpaceSpec {
    /// Number of dimensions.
    pub fn dof(&self) -> usize {
        self.lo.len()
    }

    /// True when all four lists have the same length, bounds are ordered and weights are positive.
    pub fn is_consistent(&self) -> bool {
        let n = self.lo.len();
        self.hi.len() == n
            && self.weights.len() == n
            && self.wrapping.len() == n
            && self.lo.iter().zip(&self.hi).all(|(lo, hi)| lo <= hi)
            && self.weights.iter().all(|w| *w > 0.0 && w.is_finite())
    }
}

/// Tells a planner which states and motions are allowed. Implemented by the host over the robot model and
/// the collision world; planners never see either.
pub trait StateChecker: Sync {
    /// True when the state is within limits and collision free.
    fn state_valid(&self, q: &[f64]) -> bool;

    /// True when the straight segment from `a` to `b` is valid. Both ends are assumed valid.
    fn motion_valid(&self, a: &[f64], b: &[f64]) -> bool;

    /// Distance to the nearest obstacle at `q` in metres, when the host can compute it.
    fn clearance(&self, q: &[f64]) -> Option<f64>;
}

/// Lets a running planner see cancellation and report progress.
pub trait PlanControl: Sync {
    /// True once the user or the application asked the planner to stop.
    fn is_cancelled(&self) -> bool;

    /// Reports the completed fraction, 0 to 1.
    fn progress(&self, _fraction: f32) {}
}

/// What a planner can do.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannerCaps {
    /// Keeps improving the path until the budget is used up (RRT*, Informed RRT*).
    pub optimizing: bool,
    /// Accepts several goal states and connects to whichever it reaches first.
    pub multi_goal: bool,
}

/// Describes a planner to the application.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlannerDescriptor {
    /// Unique id, e.g. `"gc.rrt_connect"`. Stored in documents.
    pub id: String,
    /// Name shown to the user.
    pub name: Text,
    /// Bumped whenever the planner's output for a given request changes; invalidates cached paths.
    pub version: u32,
    /// The parameters the planner accepts.
    pub params: ParamSchema,
    /// What the planner can do.
    pub caps: PlannerCaps,
}

/// Limits on the work a planner may do. Counted in deterministic units, never in time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanBudget {
    /// Maximum number of planner iterations.
    pub max_iterations: u64,
    /// Maximum number of single-state validity checks (motion checks count the states they sample).
    pub max_state_checks: u64,
}

/// A planning query.
#[derive(Clone, Copy, Debug)]
pub struct PlanRequest<'a> {
    /// The space to search.
    pub space: &'a SpaceSpec,
    /// Start state (`dof` values).
    pub start: &'a [f64],
    /// Goal states, flat (`count * dof` values). Reaching any one of them is success.
    pub goals: &'a [f64],
    /// Planner parameters, already completed against the descriptor's schema.
    pub params: &'a ParamValues,
    /// Seed of the planner's random numbers. The same request and seed give the same result.
    pub seed: u64,
    /// Work limits.
    pub budget: PlanBudget,
}

/// Counters describing a planning run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanStats {
    /// Planner iterations executed.
    pub iterations: u64,
    /// Single-state validity checks made.
    pub state_checks: u64,
    /// Motion (edge) validity checks made.
    pub motion_checks: u64,
    /// Nodes in the search structure at the end.
    pub nodes: u64,
}

/// A path found by a planner.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanOutput {
    /// Waypoints from start to goal, flat (`count * dof` values), wrapping dimensions unwrapped so that
    /// consecutive waypoints are joined by straight segments.
    pub path: Vec<f64>,
    /// Index of the goal that was reached.
    pub goal_index: usize,
    /// Weighted length of the path.
    pub cost: f64,
    /// Counters of the run.
    pub stats: PlanStats,
}

/// Why a planner returned no path.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum PlanFailure {
    /// The request is malformed (dimension mismatch, no goals, bad parameters).
    #[error("invalid planning request: {0}")]
    InvalidRequest(String),
    /// The start state is not valid.
    #[error("the start state is not valid")]
    StartInvalid,
    /// None of the goal states is valid.
    #[error("no goal state is valid")]
    NoValidGoal,
    /// The budget ran out before a path was found.
    #[error("no path found within the budget")]
    NoPathWithinBudget {
        /// Counters of the run.
        stats: PlanStats,
        /// Weighted distance from the closest tree node to the nearest goal.
        nearest_goal_distance: f64,
    },
    /// The planner was cancelled.
    #[error("planning was cancelled")]
    Cancelled,
}

/// A sampling-based (or other) path planner.
pub trait MotionPlanner: Send + Sync {
    /// Who the planner is and which parameters it takes.
    fn descriptor(&self) -> &PlannerDescriptor;

    /// Searches for a valid path from the start to any goal.
    fn plan(
        &self,
        req: &PlanRequest<'_>,
        checker: &dyn StateChecker,
        control: &dyn PlanControl,
    ) -> Result<PlanOutput, PlanFailure>;
}

/// Where motion planners register.
pub const MOTION_PLANNERS: ExtensionPoint<dyn MotionPlanner> = ExtensionPoint::new("gc.motion_planners");

/// Describes a time parameteriser to the application.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RetimerDescriptor {
    /// Unique id, e.g. `"gc.toppra"`. Stored in documents.
    pub id: String,
    /// Name shown to the user.
    pub name: Text,
    /// Bumped whenever the output for a given request changes.
    pub version: u32,
    /// The parameters it accepts.
    pub params: ParamSchema,
}

/// Per-joint limits for timing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimingLimits {
    /// Maximum speed per joint.
    pub max_vel: Vec<f64>,
    /// Maximum acceleration per joint.
    pub max_acc: Vec<f64>,
}

/// A timing query: put a time law on a geometric path.
#[derive(Clone, Copy, Debug)]
pub struct TimingRequest<'a> {
    /// Number of joints.
    pub dof: usize,
    /// The path, flat (`count * dof` values). The motion starts and ends at rest.
    pub path: &'a [f64],
    /// Limits to respect.
    pub limits: &'a TimingLimits,
    /// Parameters, already completed against the descriptor's schema.
    pub params: &'a ParamValues,
}

/// A path with timing, sampled densely enough to be interpolated linearly.
#[derive(Clone, Debug, PartialEq)]
pub struct TimedPath {
    /// Sample times in seconds, starting at 0 and increasing.
    pub times: Vec<f64>,
    /// Joint positions per sample, flat (`times.len() * dof` values).
    pub positions: Vec<f64>,
    /// Joint velocities per sample, flat.
    pub velocities: Vec<f64>,
}

/// Why a path could not be timed.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum TimingFailure {
    /// The request is malformed.
    #[error("invalid timing request: {0}")]
    InvalidRequest(String),
    /// No time law satisfies the limits.
    #[error("the limits of joint {joint} cannot be met at path position {s}")]
    Infeasible {
        /// Index of the limiting joint.
        joint: usize,
        /// Normalised path position (0 to 1) where it fails.
        s: f64,
    },
    /// The computation was cancelled.
    #[error("timing was cancelled")]
    Cancelled,
}

/// Computes how fast a path can be traversed.
pub trait TimeParameterizer: Send + Sync {
    /// Who the parameteriser is and which parameters it takes.
    fn descriptor(&self) -> &RetimerDescriptor;

    /// Puts a time law on the path.
    fn retime(&self, req: &TimingRequest<'_>, control: &dyn PlanControl) -> Result<TimedPath, TimingFailure>;
}

/// Where time parameterisers register.
pub const TIME_PARAMETERIZERS: ExtensionPoint<dyn TimeParameterizer> =
    ExtensionPoint::new("gc.time_parameterizers");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn space_consistency() {
        let good = SpaceSpec {
            lo: vec![-1.0, 0.0],
            hi: vec![1.0, 0.0],
            weights: vec![1.0, 0.5],
            wrapping: vec![false; 2],
        };
        assert!(good.is_consistent());
        assert_eq!(good.dof(), 2);
        assert!(!SpaceSpec { hi: vec![1.0], ..good.clone() }.is_consistent());
        assert!(!SpaceSpec { lo: vec![2.0, 0.0], ..good.clone() }.is_consistent());
        assert!(!SpaceSpec { weights: vec![1.0, 0.0], ..good.clone() }.is_consistent());
        assert!(!SpaceSpec { weights: vec![1.0, f64::NAN], ..good }.is_consistent());
    }
}
