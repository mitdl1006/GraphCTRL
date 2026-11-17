//! From "move the robot there" to a validated trajectory.
//!
//! The pipeline: resolve the goal into joint configurations (inverse kinematics for a pose goal), try the
//! straight joint-space move, otherwise run the chosen planner from several seeds and keep the shortest
//! result, shortcut it, re-validate it independently, time it and validate the timed trajectory by dense
//! sampling. What comes out is a [`gc_traj::Trajectory<Validated>`] or an error that names the cause.
//!
//! The same seed gives the same result, bit for bit.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::float_cmp, reason = "tests compare exactly representable values"))]

mod scene;

use std::sync::Arc;

use gc_math::DPose3;
use gc_plan::{
    Algorithm, Control, Failure, Options, Request, Rng, Space, Stats, Validity, path_cost, plan, shortcut,
    validate,
};
use gc_robot::{IkOptions, NumericIk};
use gc_robotics_api::IkFailureReason;
use gc_traj::{Limits, TimingError, Trajectory, Validated, ValidationError};

pub use scene::{Checker, Obstacle, RobotScene, StateProblem};

/// How far a blend may leave the corner it rounds, as a distance in joint space (radians for an arm).
pub const BLEND_DEVIATION: f64 = 0.1;

/// Where the motion should end.
#[derive(Clone, Debug, PartialEq)]
pub enum Goal {
    /// A joint configuration.
    Joints(Vec<f64>),
    /// A pose of the chain's tip link in the world.
    Pose(DPose3),
}

/// A motion to plan.
#[derive(Clone, Debug, PartialEq)]
pub struct MotionRequest {
    /// The configuration the robot starts in.
    pub start: Vec<f64>,
    /// Where to go.
    pub goal: Goal,
    /// The planner used when the straight move is blocked.
    pub algorithm: Algorithm,
    /// Planner settings.
    pub options: Options,
    /// Seed of the first planner run; further runs use the following seeds.
    pub seed: u64,
    /// How many seeds to run; the shortest result wins.
    pub runs: usize,
    /// Acceleration limit per joint (the robot description has none).
    pub max_acc: Vec<f64>,
}

/// A planned and validated motion.
#[derive(Clone, Debug)]
pub struct MotionPlan {
    /// The joint-space waypoints.
    pub path: Vec<Vec<f64>>,
    /// The timed motion through them.
    pub trajectory: Trajectory<Validated>,
    /// World positions of the tip along the trajectory, for display.
    pub tip_path: Vec<[f64; 3]>,
    /// Weighted joint-space length of the path.
    pub cost: f64,
    /// True when the straight move was possible and no planner ran.
    pub straight: bool,
    /// Planner counters, summed over all runs.
    pub stats: Stats,
    /// The smallest distance to an obstacle or between links found along the trajectory.
    pub min_clearance: f64,
    /// The padding added on top of the required one while planning (smaller in tight cells).
    pub planning_margin: f64,
}

/// Why no motion was produced.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum MotionError {
    /// The request does not fit the robot.
    #[error("invalid motion request: {0}")]
    InvalidRequest(String),
    /// The start configuration is not allowed.
    #[error("the start is not valid: {0}")]
    StartInvalid(StateProblem),
    /// No joint configuration reaches the goal pose.
    #[error("the goal pose cannot be reached ({0:?})")]
    GoalUnreachable(IkFailureReason),
    /// The goal can be reached, but only in configurations that are not allowed.
    #[error("the goal is not valid: {0}")]
    GoalInvalid(StateProblem),
    /// A joint on the chain has no speed limit, so the motion cannot be timed.
    #[error("joint `{0}` has no speed limit")]
    NoSpeedLimit(String),
    /// The planner found no path within its budget.
    #[error("no path found within the budget (closest approach to the goal: {nearest_goal_distance:.3})")]
    NoPath {
        /// Planner counters.
        stats: Stats,
        /// How close the search came to a goal, in weighted joint-space distance.
        nearest_goal_distance: f64,
    },
    /// The caller cancelled.
    #[error("planning was cancelled")]
    Cancelled,
    /// The path could not be timed.
    #[error(transparent)]
    Timing(#[from] TimingError),
    /// The timed trajectory failed its final check.
    #[error(transparent)]
    Validation(#[from] ValidationError),
}

/// Plans a motion in a scene.
pub fn plan_motion(
    scene: &RobotScene,
    request: &MotionRequest,
    control: &dyn Control,
) -> Result<MotionPlan, MotionError> {
    let chain = scene.chain();
    let dof = chain.dof();
    if request.start.len() != dof || request.max_acc.len() != dof || request.runs == 0 {
        return Err(MotionError::InvalidRequest(format!("the chain has {dof} joints")));
    }
    if let Some(problem) = scene.state_problem(&request.start) {
        return Err(MotionError::StartInvalid(problem));
    }
    let mut max_vel = Vec::with_capacity(dof);
    for (name, speed) in chain.joint_names().iter().zip(chain.max_vel()) {
        max_vel.push(speed.ok_or_else(|| MotionError::NoSpeedLimit(name.clone()))?);
    }
    // Joints that are slow relative to the others cost more: path length then approximates time.
    let slowest = max_vel.iter().fold(0.0_f64, |acc, v| acc.max(1.0 / v));
    let space = Space {
        lo: chain.lower().to_vec(),
        hi: chain.upper().to_vec(),
        weights: max_vel.iter().map(|v| (1.0 / v) / slowest).collect(),
    };

    let goals = resolve_goals(scene, request, &space)?;
    let limits = Limits { max_vel, max_acc: request.max_acc.clone() };
    // Plan with more padding than required, so that the timed motion keeps the required padding between
    // the states the planner checked. Where the cell is too tight for that, check in finer steps, which
    // needs less extra room and costs more checks.
    let mut last = MotionError::NoPath { stats: Stats::default(), nearest_goal_distance: f64::INFINITY };
    for fineness in [1.0, 0.25, 0.0625] {
        match attempt(scene, request, &space, &goals, &limits, fineness, control) {
            Ok(plan) => return Ok(plan),
            Err(MotionError::Cancelled) => return Err(MotionError::Cancelled),
            Err(error) => last = error,
        }
    }
    Err(last)
}

/// One pass of the pipeline at a given checking fineness.
fn attempt(
    scene: &RobotScene,
    request: &MotionRequest,
    space: &Space,
    goals: &[Vec<f64>],
    limits: &Limits,
    fineness: f64,
    control: &dyn Control,
) -> Result<MotionPlan, MotionError> {
    let mut stats = Stats::default();
    let no_path =
        |stats: Stats, nearest_goal_distance: f64| MotionError::NoPath { stats, nearest_goal_distance };
    let checker = scene.checker(fineness);
    let goals: Vec<Vec<f64>> = goals.iter().filter(|goal| checker.state_valid(goal)).cloned().collect();
    if goals.is_empty() || !checker.state_valid(&request.start) {
        return Err(no_path(stats, f64::INFINITY));
    }
    let mut straight = false;
    let mut best: Option<Vec<Vec<f64>>> = None;
    if let Some(goal) = goals.iter().find(|goal| checker.motion_valid(&request.start, goal)) {
        straight = true;
        best = Some(vec![request.start.clone(), goal.clone()]);
    }
    let mut nearest = f64::INFINITY;
    if best.is_none() {
        for run in 0..request.runs {
            let seed = request.seed.wrapping_add(run as u64);
            let planned = Request {
                space,
                start: &request.start,
                goals: &goals,
                algorithm: request.algorithm,
                options: request.options,
                seed,
            };
            match plan(&planned, &checker, control) {
                Ok(solution) => {
                    add(&mut stats, &solution.stats);
                    let mut path = solution.path;
                    shortcut(space, &mut path, &checker, &mut Rng::new(seed), 150);
                    // Nothing a planner or a simplifier returns is trusted without this check.
                    let shorter = best.as_ref().is_none_or(|b| path_cost(space, &path) < path_cost(space, b));
                    if shorter && validate(&path, &checker) {
                        best = Some(path);
                    }
                }
                Err(Failure::Cancelled) => return Err(MotionError::Cancelled),
                Err(Failure::NoPath { stats: run_stats, nearest_goal_distance }) => {
                    add(&mut stats, &run_stats);
                    nearest = nearest.min(nearest_goal_distance);
                }
                Err(other) => return Err(MotionError::InvalidRequest(other.to_string())),
            }
        }
    }
    let Some(path) = best else {
        return Err(no_path(stats, nearest));
    };

    // The timed motion is checked against the required padding, not the planning margin.
    let strict = scene.strict_checker();
    // A path with corners is driven through them on blends when that passes the final check: the
    // blends are tried with the planning margin and the whole timed motion against the required
    // padding. Otherwise, and for a single straight move, the motion stops at every waypoint.
    let blended = (path.len() > 2)
        .then(|| Trajectory::blended(&path, limits, BLEND_DEVIATION, &|q| checker.state_valid(q)).ok())
        .flatten()
        .and_then(|timed| timed.validate(limits, 0.004, &|q| strict.state_valid(q)).ok());
    let trajectory = match blended {
        Some(trajectory) => trajectory,
        None => Trajectory::stopping_at_waypoints(&path, limits)?
            .validate(limits, 0.004, &|q| strict.state_valid(q))?,
    };
    let mut tip_path = Vec::new();
    let mut min_clearance = f64::INFINITY;
    let samples = ((trajectory.duration() / 0.02).ceil() as usize).clamp(1, 20_000);
    for k in 0..=samples {
        let q = trajectory.sample(trajectory.duration() * k as f64 / samples as f64).q;
        let tip = scene.tip_in_world(&q).translation;
        tip_path.push([tip.x, tip.y, tip.z]);
        if k % 5 == 0 {
            min_clearance = min_clearance.min(scene.clearance(&q));
        }
    }
    Ok(MotionPlan {
        cost: path_cost(space, &path),
        path,
        trajectory,
        tip_path,
        straight,
        stats,
        min_clearance,
        planning_margin: checker.margin(),
    })
}

fn add(total: &mut Stats, run: &Stats) {
    total.iterations += run.iterations;
    total.state_checks += run.state_checks;
    total.motion_checks += run.motion_checks;
    total.nodes += run.nodes;
}

/// The valid goal configurations, nearest to the start first.
fn resolve_goals(
    scene: &RobotScene,
    request: &MotionRequest,
    space: &Space,
) -> Result<Vec<Vec<f64>>, MotionError> {
    let chain = scene.chain();
    let candidates = match &request.goal {
        Goal::Joints(q) => {
            if q.len() != chain.dof() {
                return Err(MotionError::InvalidRequest(format!("the chain has {} joints", chain.dof())));
            }
            vec![q.clone()]
        }
        Goal::Pose(world) => {
            let target = scene.base().inverse() * *world;
            let ik = NumericIk::new(chain, IkOptions::default());
            let found = ik.solve_many(&target, Some(&request.start), 32);
            if found.is_empty() {
                let reason = ik
                    .solve_near(&target, &request.start)
                    .err()
                    .map_or(IkFailureReason::Budget, |f| f.reason);
                return Err(MotionError::GoalUnreachable(reason));
            }
            found
        }
    };
    let mut problem = None;
    let mut valid: Vec<Vec<f64>> = Vec::new();
    for candidate in candidates {
        match scene.state_problem(&candidate) {
            None => valid.push(candidate),
            Some(found) => {
                problem.get_or_insert(found);
            }
        }
    }
    if valid.is_empty() {
        return Err(MotionError::GoalInvalid(problem.unwrap_or(StateProblem::Unknown)));
    }
    valid.sort_by(|a, b| space.distance(&request.start, a).total_cmp(&space.distance(&request.start, b)));
    valid.truncate(8);
    Ok(valid)
}

/// Shared handle to a robot model.
pub type SharedModel = Arc<gc_robot::RobotModel>;
