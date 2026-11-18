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
mod sphere;

use std::sync::Arc;

use gc_math::{DPose3, DVec3};
use gc_plan::{
    Algorithm, Control, Failure, Options, Request, Rng, Space, Stats, Validity, path_cost, plan, shortcut,
    validate,
};
use gc_robot::{IkOptions, NumericIk};
use gc_robotics_api::{
    IkFailureReason, MotionPlanner, PlanBudget, PlanControl, PlanFailure, PlanRequest, PlanStats, SpaceSpec,
    StateChecker,
};
use gc_traj::{Limits, TimingError, Trajectory, Validated, ValidationError};

pub use scene::{
    Checker, ContactAllowance, ContactScope, LinkPair, Obstacle, PairReason, RobotScene, StateProblem,
};
pub use sphere::{SpherePlan, SphereRequest, plan_sphere};

/// How far a blend may leave the corner it rounds, as a distance in joint space (radians for an arm).
pub const BLEND_DEVIATION: f64 = 0.1;

/// Where the motion should end.
#[derive(Clone, Debug, PartialEq)]
pub enum Goal {
    /// A joint configuration.
    Joints(Vec<f64>),
    /// A pose of the tool centre point in the world (of the chain's tip link when there is no tool).
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
    /// Move the tool tip to the goal in a straight line, turning evenly, instead of planning freely.
    pub linear: bool,
    /// For a straight move: the fastest the tool tip may go, m/s. The motion is slowed down evenly
    /// until it keeps to it.
    pub max_tcp_speed: Option<f64>,
    /// Goal configurations already known for a pose goal (from a closed-form solver), nearest first.
    /// When given, they replace the numerical search for goal configurations.
    pub goal_seeds: Vec<Vec<f64>>,
    /// Robot parts allowed closer to obstacles than the padding (near the start and the goal, or along
    /// the whole motion). Allowances naming a body the scene does not have are ignored.
    pub contacts: Vec<ContactAllowance>,
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
    /// The highest speed of the tool tip along the trajectory, m/s.
    pub peak_tcp_speed: f64,
    /// True when the previous path was still valid for the new input and was kept as it was ("valid,
    /// not re-optimised"; see [`replan`]).
    pub reused: bool,
    /// True for a straight-line move of the tool tip ([`MotionRequest::linear`]).
    pub linear: bool,
    /// What the final check of the timed motion establishes.
    pub guarantee: Guarantee,
}

/// What the final check of a motion establishes (plan 5.7, appendix E Q2).
///
/// The timed motion was checked at states close enough together that no point of a robot body moves
/// more than [`Guarantee::step_mm`] between two of them. Every checked state keeps
/// [`Guarantee::padding_nominal`] from every obstacle, so between them the convex collision model keeps
/// at least [`Guarantee::clearance`] (padding minus half a step, in metres). Not covered: pairs excluded from
/// checking ([`RobotScene::excluded_pairs`]), bodies that start inside a closed mesh, and how closely the
/// controller follows the motion. With no padding the check is sampling only.
#[derive(Clone, Debug, PartialEq)]
pub struct Guarantee {
    /// The padding every checked state keeps from obstacles, metres.
    pub padding_nominal: f64,
    /// The smallest padding used anywhere along the motion, metres: zero where contact allowances are in
    /// force for some pairs, else the nominal padding.
    pub padding_min: f64,
    /// The largest change of a joint between two checked states, degrees.
    pub step_deg: f64,
    /// An upper bound of how far any point of a robot body moves between two checked states, millimetres.
    pub step_mm: f64,
    /// The clearance the convex collision model keeps between checked states, metres: padding minus half
    /// a step. Zero or less means sampling only.
    pub clearance: f64,
    /// Number of states checked.
    pub states: usize,
    /// The collision model that was checked.
    pub model: &'static str,
    /// Fingerprint of the body pairs that are not checked ([`RobotScene::excluded_pairs_hash`]).
    pub acm_hash: u64,
}

impl Guarantee {
    /// True when the clearance is positive: the guarantee holds between the checked states, not only at
    /// them.
    pub fn holds_between_states(&self) -> bool {
        self.clearance > 0.0
    }
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
    /// The straight line to the goal cannot be followed all the way.
    #[error("the straight line can be followed for {percent:.0} % only: {cause}")]
    Linear {
        /// How much of the line can be followed, in percent.
        percent: f64,
        /// What stops it there.
        cause: String,
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
    plan_motion_using(scene, request, None, control)
}

/// [`plan_motion`] with a planner from the plugin registry (`MOTION_PLANNERS`) in place of
/// [`MotionRequest::algorithm`], when one is given. It runs with its default parameters and the request's
/// budget.
pub fn plan_motion_using(
    scene: &RobotScene,
    request: &MotionRequest,
    planner: Option<&dyn MotionPlanner>,
    control: &dyn Control,
) -> Result<MotionPlan, MotionError> {
    let scoped = with_contacts(scene, request);
    plan_preferring(scoped.as_ref().unwrap_or(scene), request, None, planner, control)
}

/// The scene with the request's contact allowances in force, when it has any: they apply near the start
/// and the goal positions of the tool centre point.
fn with_contacts(scene: &RobotScene, request: &MotionRequest) -> Option<RobotScene> {
    if request.contacts.is_empty() {
        return None;
    }
    let dof = scene.chain().dof();
    let mut waypoints = Vec::new();
    if request.start.len() == dof {
        waypoints.push(scene.tip_in_world(&request.start).translation);
    }
    match &request.goal {
        Goal::Pose(pose) => waypoints.push(pose.translation),
        Goal::Joints(q) if q.len() == dof => waypoints.push(scene.tip_in_world(q).translation),
        Goal::Joints(_) => {}
    }
    Some(scene.clone().with_contacts(&request.contacts, &waypoints).0)
}

/// Plans a motion again after its input changed, re-using what can be re-used (plan 5.8, re-planning
/// ladder):
///
/// 1. When `previous` (the last plan) is the same kind of motion, still starts at the start, still ends at
///    the goal and is still valid in the scene, it is kept as it is and only timed again: the result has
///    [`MotionPlan::reused`] set ("valid, not re-optimised"). A detour is not kept when the straight move
///    to its end has become free.
/// 2. Otherwise the motion is planned in full, with the goal configuration nearest to the previous end
///    (the same IK branch and turn) tried first, so that a small change of the goal does not make the
///    arm switch configurations.
pub fn replan(
    scene: &RobotScene,
    request: &MotionRequest,
    previous: &MotionPlan,
    control: &dyn Control,
) -> Result<MotionPlan, MotionError> {
    replan_using(scene, request, previous, None, control)
}

/// [`replan`] with a planner from the plugin registry, as for [`plan_motion_using`].
pub fn replan_using(
    scene: &RobotScene,
    request: &MotionRequest,
    previous: &MotionPlan,
    planner: Option<&dyn MotionPlanner>,
    control: &dyn Control,
) -> Result<MotionPlan, MotionError> {
    let scoped = with_contacts(scene, request);
    let scene = scoped.as_ref().unwrap_or(scene);
    let (space, limits) = setup(scene, request)?;
    if previous.linear == request.linear
        && let Some(plan) = keep(scene, request, &space, &limits, &previous.path)
    {
        return Ok(plan);
    }
    plan_preferring(scene, request, previous.path.last().map(Vec::as_slice), planner, control)
}

/// The planning space and the limits of a request, after checking that it fits the robot.
fn setup(scene: &RobotScene, request: &MotionRequest) -> Result<(Space, Limits), MotionError> {
    let chain = scene.chain();
    let dof = chain.dof();
    if request.start.len() != dof || request.max_acc.len() != dof || request.runs == 0 {
        return Err(MotionError::InvalidRequest(format!("the chain has {dof} joints")));
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
    Ok((space, Limits { max_vel, max_acc: request.max_acc.clone() }))
}

/// Rung 1 of the re-planning ladder: the previous path, timed again, when it still fits the request.
fn keep(
    scene: &RobotScene,
    request: &MotionRequest,
    space: &Space,
    limits: &Limits,
    previous: &[Vec<f64>],
) -> Option<MotionPlan> {
    let dof = scene.chain().dof();
    let (first, last) = (previous.first()?, previous.last()?);
    if previous.iter().any(|q| q.len() != dof || q.iter().any(|v| !v.is_finite()))
        || space.distance(first, &request.start) > 1e-9
    {
        return None;
    }
    let reaches = match &request.goal {
        Goal::Joints(goal) => goal.len() == dof && space.distance(goal, last) <= 1e-9,
        Goal::Pose(goal) => {
            let at = scene.tip_in_world(last);
            (at.translation - goal.translation).length() <= 1e-6
                && at.rotation.angle_between(goal.rotation) <= 1e-6
        }
    };
    if !reaches || scene.state_problem(first).is_some() {
        return None;
    }
    let path = previous.to_vec();
    let mut plan = if request.linear {
        // A straight move was checked against the required padding alone.
        let checker = scene.strict_checker();
        if !validate(&path, &checker) {
            return None;
        }
        let mut plan = finish(scene, space, limits, path, &checker, 0.002, false, Stats::default()).ok()?;
        plan.linear = true;
        cap_tcp_speed(plan, request.max_tcp_speed)
    } else {
        // A free move keeps the planning margin of the finest checking step the pipeline uses.
        let checker = [1.0, 0.25, 0.0625]
            .into_iter()
            .map(|fineness| scene.checker(fineness))
            .find(|checker| validate(&path, checker))?;
        let straight = path.len() == 2;
        // A detour gives way once the straight move has become free: that is shorter, and cheap to see.
        if !straight && checker.motion_valid(first, last) {
            return None;
        }
        finish(scene, space, limits, path, &checker, BLEND_DEVIATION, straight, Stats::default()).ok()?
    };
    plan.reused = true;
    Some(plan)
}

/// The pipeline, with the goal configuration nearest to `prefer` tried first.
fn plan_preferring(
    scene: &RobotScene,
    request: &MotionRequest,
    prefer: Option<&[f64]>,
    planner: Option<&dyn MotionPlanner>,
    control: &dyn Control,
) -> Result<MotionPlan, MotionError> {
    let (space, limits) = setup(scene, request)?;
    if let Some(problem) = scene.state_problem(&request.start) {
        return Err(MotionError::StartInvalid(problem));
    }
    if request.linear {
        return linear(scene, request, &space, &limits, control);
    }
    let goals = resolve_goals(scene, request, &space, prefer)?;
    // Plan with more padding than required, so that the timed motion keeps the required padding between
    // the states the planner checked. Where the cell is too tight for that, check in finer steps, which
    // needs less extra room and costs more checks.
    let mut last = MotionError::NoPath { stats: Stats::default(), nearest_goal_distance: f64::INFINITY };
    for fineness in [1.0, 0.25, 0.0625] {
        match attempt(scene, request, &space, &goals, &limits, fineness, planner, control) {
            Ok(plan) => return Ok(plan),
            Err(MotionError::Cancelled) => return Err(MotionError::Cancelled),
            Err(error) => last = error,
        }
    }
    Err(last)
}

/// One pass of the pipeline at a given checking fineness.
#[allow(clippy::too_many_arguments, reason = "the pipeline's state; a struct would only rename it")]
fn attempt(
    scene: &RobotScene,
    request: &MotionRequest,
    space: &Space,
    goals: &[Vec<f64>],
    limits: &Limits,
    fineness: f64,
    planner: Option<&dyn MotionPlanner>,
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
    // The straight move to the nearest goal is the shortest motion there is: taken at once. A straight
    // move to a goal further away only competes with the planned paths on length (it may swing the arm
    // round the back where going over an obstacle to the nearest goal is much shorter).
    let mut best: Option<Vec<Vec<f64>>> = goals
        .iter()
        .find(|goal| checker.motion_valid(&request.start, goal))
        .map(|goal| vec![request.start.clone(), goal.clone()]);
    let nearest_is_straight = best.as_ref().is_some_and(|b| b[1] == goals[0]);
    let mut nearest = f64::INFINITY;
    if !nearest_is_straight {
        for run in 0..request.runs {
            let seed = request.seed.wrapping_add(run as u64);
            // The goals about as near as the nearest first; all of them only when none of those is
            // reached (a multi-goal planner otherwise ends at whichever goal it meets first, often a
            // far configuration).
            let near = near_goals(space, &request.start, &goals);
            let mut outcome =
                Err(Failure::NoPath { stats: Stats::default(), nearest_goal_distance: f64::INFINITY });
            let tiers: Vec<&[Vec<f64>]> =
                if near.len() < goals.len() { vec![&near, &goals] } else { vec![&goals] };
            // With two tiers each gets half of the budget, so a run never exceeds it.
            let options = if tiers.len() > 1 {
                Options {
                    max_iterations: request.options.max_iterations / 2,
                    max_state_checks: request.options.max_state_checks / 2,
                    ..request.options
                }
            } else {
                request.options
            };
            for tier in tiers {
                let planned = Request {
                    space,
                    start: &request.start,
                    goals: tier,
                    algorithm: request.algorithm,
                    options,
                    seed,
                };
                let previous = match &outcome {
                    Err(Failure::NoPath { stats, .. }) => *stats,
                    _ => Stats::default(),
                };
                outcome = run_planner(&planned, planner, &checker, control);
                match &mut outcome {
                    Ok(solution) => {
                        add(&mut solution.stats, &previous);
                        break;
                    }
                    Err(Failure::NoPath { stats, .. }) => add(stats, &previous),
                    Err(_) => break,
                }
            }
            match outcome {
                Ok(solution) => {
                    add(&mut stats, &solution.stats);
                    let mut path = solution.path;
                    simplify_path(space, &mut path, &checker, seed);
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
    let straight = path.len() == 2;
    finish(scene, space, limits, path, &checker, BLEND_DEVIATION, straight, stats)
}

/// Times a checked path, validates the timed motion against the required padding and measures it.
#[allow(clippy::too_many_arguments, reason = "the pipeline's state; a struct would only rename it")]
fn finish(
    scene: &RobotScene,
    space: &Space,
    limits: &Limits,
    path: Vec<Vec<f64>>,
    checker: &scene::Checker<'_>,
    deviation: f64,
    straight: bool,
    stats: Stats,
) -> Result<MotionPlan, MotionError> {
    // The timed motion is checked against the required padding, not the planning margin.
    let strict = scene.strict_checker();
    // A path with corners is driven through them on blends when that passes the final check: the
    // blends are tried with the planning margin and the whole timed motion against the required
    // padding. Otherwise, and for a single straight move, the motion stops at every waypoint.
    let blended = (path.len() > 2)
        .then(|| Trajectory::blended(&path, limits, deviation, &|q| checker.state_valid(q)).ok())
        .flatten()
        .and_then(|timed| timed.validate(limits, 0.004, &|q| strict.state_valid(q)).ok())
        .and_then(|timed| certify(scene, &timed, &strict).ok().map(|guarantee| (timed, guarantee)));
    let (trajectory, guarantee) = match blended {
        Some(found) => found,
        None => {
            let timed = Trajectory::stopping_at_waypoints(&path, limits)?
                .validate(limits, 0.004, &|q| strict.state_valid(q))?;
            let guarantee = certify(scene, &timed, &strict)?;
            (timed, guarantee)
        }
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
    // The tip's positions are evenly spaced in time.
    let dt = trajectory.duration() / samples as f64;
    let peak_tcp_speed = if dt > 0.0 {
        tip_path
            .windows(2)
            .map(|pair| DVec3::from_array(pair[0]).distance(DVec3::from_array(pair[1])) / dt)
            .fold(0.0, f64::max)
    } else {
        0.0
    };
    Ok(MotionPlan {
        peak_tcp_speed,
        cost: path_cost(space, &path),
        path,
        trajectory,
        tip_path,
        straight,
        stats,
        min_clearance,
        planning_margin: checker.margin(),
        reused: false,
        linear: false,
        guarantee,
    })
}

/// Checks a timed motion densely enough for a [`Guarantee`]: the states 4 ms apart, and between two of
/// them as many more as it takes for no robot point to move further than the padding from one checked
/// state to the next (so half a step, the most a point can be from its nearest checked position, stays
/// below the padding).
fn certify(
    scene: &RobotScene,
    trajectory: &Trajectory<Validated>,
    strict: &scene::Checker<'_>,
) -> Result<Guarantee, ValidationError> {
    const STEP: f64 = 0.004;
    let padding = scene.padding();
    let duration = trajectory.duration();
    let intervals = ((duration / STEP).ceil() as usize).clamp(1, 1_000_000);
    let (mut step_deg, mut step_m, mut states) = (0.0_f64, 0.0_f64, 1_usize);
    let mut previous = trajectory.sample(0.0).q;
    for k in 1..=intervals {
        let (t0, t1) = (duration * (k - 1) as f64 / intervals as f64, duration * k as f64 / intervals as f64);
        let end = trajectory.sample(t1).q;
        // With padding, subdivide until no step moves a point further than the padding (the path is
        // curved, so a sub-step can move more than its share of the whole interval: double until it holds).
        let mut pieces = if padding > 0.0 {
            ((scene.sweep_bound(&previous, &end) / padding).ceil() as usize).clamp(1, 4096)
        } else {
            1
        };
        let steps = loop {
            let mut qs: Vec<(f64, Vec<f64>)> = (1..pieces)
                .map(|piece| {
                    let t = t0 + (t1 - t0) * piece as f64 / pieces as f64;
                    (t, trajectory.sample(t).q)
                })
                .collect();
            qs.push((t1, end.clone()));
            let worst = std::iter::once(&previous)
                .chain(qs.iter().map(|(_, q)| q))
                .zip(qs.iter().map(|(_, q)| q))
                .map(|(a, b)| scene.sweep_bound(a, b))
                .fold(0.0, f64::max);
            if padding <= 0.0 || worst <= padding || pieces >= 4096 {
                break qs;
            }
            pieces *= 2;
        };
        let mut at = previous;
        let last = steps.len() - 1;
        for (index, (t, q)) in steps.into_iter().enumerate() {
            if index < last && !strict.state_valid(&q) {
                return Err(ValidationError::State { time: t });
            }
            step_m = step_m.max(scene.sweep_bound(&at, &q));
            step_deg =
                step_deg.max(at.iter().zip(&q).fold(0.0_f64, |m, (a, b)| m.max((a - b).abs())).to_degrees());
            states += 1;
            at = q;
        }
        previous = at;
    }
    Ok(Guarantee {
        padding_nominal: padding,
        padding_min: if scene.has_contacts() { 0.0 } else { padding },
        step_deg,
        step_mm: step_m * 1e3,
        clearance: padding - step_m / 2.0,
        states,
        model: "convex hulls of the collision geometry",
        acm_hash: scene.excluded_pairs_hash(),
    })
}

/// A straight line of the tool tip from where it is to the goal, turning evenly on the way.
///
/// The line is followed in small steps; at each one the joint values nearest to the previous step are
/// taken. The motion is refused when a step cannot be reached, when the nearest joint values are a jump
/// away (the arm would have to reconfigure, typically near a singularity), or when a step is not
/// allowed.
fn linear(
    scene: &RobotScene,
    request: &MotionRequest,
    space: &Space,
    limits: &Limits,
    control: &dyn Control,
) -> Result<MotionPlan, MotionError> {
    follow(scene, request, &[], space, limits, control)
}

/// Plans a motion of the tool tip along a curve given as poses close together: from where it is in a
/// straight line to the first of `via`, on through each of them and to the goal, turning evenly between
/// them. It is one motion without a stop on the way, refused like a straight move
/// ([`MotionRequest::linear`], which this is with no `via`) with how far along the curve it got.
/// [`MotionRequest::max_tcp_speed`] applies; the planner is not used.
pub fn plan_follow(
    scene: &RobotScene,
    request: &MotionRequest,
    via: &[DPose3],
    control: &dyn Control,
) -> Result<MotionPlan, MotionError> {
    let scoped = with_contacts(scene, request);
    let scene = scoped.as_ref().unwrap_or(scene);
    let (space, limits) = setup(scene, request)?;
    if let Some(problem) = scene.state_problem(&request.start) {
        return Err(MotionError::StartInvalid(problem));
    }
    follow(scene, request, via, &space, &limits, control)
}

/// The tool tip in straight lines through `via` to the goal (see [`linear`] and [`plan_follow`]).
fn follow(
    scene: &RobotScene,
    request: &MotionRequest,
    via: &[DPose3],
    space: &Space,
    limits: &Limits,
    control: &dyn Control,
) -> Result<MotionPlan, MotionError> {
    /// Steps along the line: at most this far and this much turning each.
    const STEP: (f64, f64) = (0.005, 0.02);
    /// The largest change of a joint between two steps that still counts as following.
    const JUMP: f64 = 0.25;
    let chain = scene.chain();
    let to = match &request.goal {
        Goal::Pose(pose) => *pose,
        Goal::Joints(q) if q.len() == chain.dof() => scene.tip_in_world(q),
        Goal::Joints(_) => {
            return Err(MotionError::InvalidRequest(format!("the chain has {} joints", chain.dof())));
        }
    };
    // Each leg: where it starts, where it ends and in how many steps.
    let mut legs = Vec::with_capacity(via.len() + 1);
    let mut from = scene.tip_in_world(&request.start);
    for target in via.iter().chain(std::iter::once(&to)) {
        let distance = (target.translation - from.translation).length();
        let turn = from.rotation.angle_between(target.rotation);
        let steps = ((distance / STEP.0).max(turn / STEP.1).ceil() as usize).clamp(1, 20_000);
        legs.push((from, *target, steps));
        from = *target;
    }
    let total: usize = legs.iter().map(|(_, _, steps)| steps).sum();
    let ik = NumericIk::new(chain, IkOptions::default());
    let stopped = |done: usize, cause: String| MotionError::Linear {
        percent: 100.0 * done as f64 / total as f64,
        cause,
    };
    let mut path = vec![request.start.clone()];
    for (from, to, steps) in legs {
        for k in 1..=steps {
            if control.cancelled() {
                return Err(MotionError::Cancelled);
            }
            let t = k as f64 / steps as f64;
            let pose = DPose3::from_parts(
                from.translation.lerp(to.translation, t),
                from.rotation.slerp(to.rotation, t).normalize(),
            );
            let done = path.len() - 1;
            let previous = &path[done];
            let q = ik.solve_near(&scene.flange_target(&pose), previous).map_err(|failure| {
                stopped(done, format!("the arm cannot reach further ({:?})", failure.reason))
            })?;
            if q.iter().zip(previous).any(|(a, b)| (a - b).abs() > JUMP) {
                return Err(stopped(
                    done,
                    "the arm would have to change its configuration (near a singularity)".to_owned(),
                ));
            }
            if let Some(problem) = scene.state_problem(&q) {
                return Err(stopped(done, problem.to_string()));
            }
            path.push(q);
        }
    }
    // A joint goal is met exactly when following the line ends there.
    if let (Goal::Joints(goal), Some(last)) = (&request.goal, path.last_mut())
        && space.distance(goal, last) < 1e-4
    {
        last.clone_from(goal);
    }
    let checker = scene.strict_checker();
    // Small blends: the tip stays on the line within a fraction of a millimetre.
    let mut plan = finish(scene, space, limits, path, &checker, 0.002, false, Stats::default())?;
    plan.linear = true;
    Ok(cap_tcp_speed(plan, request.max_tcp_speed))
}

/// Slows a motion down evenly until its tool tip keeps to `cap` (m/s).
fn cap_tcp_speed(mut plan: MotionPlan, cap: Option<f64>) -> MotionPlan {
    if let Some(cap) = cap.filter(|v| v.is_finite() && *v > 0.0)
        && plan.peak_tcp_speed > cap
    {
        let factor = plan.peak_tcp_speed / cap;
        plan.trajectory = plan.trajectory.slowed(factor);
        plan.peak_tcp_speed = cap;
    }
    plan
}

/// Runs the built-in planner of the request, or the registered `planner` when one is given.
fn run_planner(
    request: &Request<'_>,
    planner: Option<&dyn MotionPlanner>,
    checker: &scene::Checker<'_>,
    control: &dyn Control,
) -> Result<gc_plan::Solution, Failure> {
    let Some(planner) = planner else { return plan(request, checker, control) };
    let dof = request.space.dof();
    let space = SpaceSpec {
        lo: request.space.lo.clone(),
        hi: request.space.hi.clone(),
        weights: request.space.weights.clone(),
        wrapping: vec![false; dof],
    };
    let goals: Vec<f64> = request.goals.concat();
    let params = planner.descriptor().params.defaults();
    let api_request = PlanRequest {
        space: &space,
        start: request.start,
        goals: &goals,
        params: &params,
        seed: request.seed,
        budget: PlanBudget {
            max_iterations: request.options.max_iterations,
            max_state_checks: request.options.max_state_checks,
        },
    };
    let stats = |s: PlanStats| Stats {
        iterations: s.iterations,
        state_checks: s.state_checks,
        motion_checks: s.motion_checks,
        nodes: s.nodes,
    };
    let shared = SharedChecker(checker);
    match planner.plan(&api_request, &shared, &ControlAdapter(control)) {
        Ok(output) if dof > 0 && output.path.len() % dof == 0 && !output.path.is_empty() => {
            let path: Vec<Vec<f64>> = output.path.chunks(dof).map(<[f64]>::to_vec).collect();
            // Whatever a plugin returns is checked again by the pipeline; the cost is recomputed here.
            Ok(gc_plan::Solution {
                cost: path_cost(request.space, &path),
                path,
                goal_index: output.goal_index,
                stats: stats(output.stats),
            })
        }
        Ok(_) => Err(Failure::InvalidRequest(format!(
            "planner `{}` returned a malformed path",
            planner.descriptor().id
        ))),
        Err(PlanFailure::InvalidRequest(reason)) => Err(Failure::InvalidRequest(reason)),
        Err(PlanFailure::StartInvalid) => Err(Failure::StartInvalid),
        Err(PlanFailure::NoValidGoal) => Err(Failure::NoValidGoal),
        Err(PlanFailure::NoPathWithinBudget { stats: s, nearest_goal_distance }) => {
            Err(Failure::NoPath { stats: stats(s), nearest_goal_distance })
        }
        Err(PlanFailure::Cancelled) => Err(Failure::Cancelled),
    }
}

/// The pipeline's checker for a plugin planner, which may call it from any thread.
struct SharedChecker<'a, 'b>(&'a scene::Checker<'b>);

impl StateChecker for SharedChecker<'_, '_> {
    fn state_valid(&self, q: &[f64]) -> bool {
        self.0.state_valid(q)
    }

    fn motion_valid(&self, a: &[f64], b: &[f64]) -> bool {
        self.0.motion_valid(a, b)
    }

    fn motion_cost(&self, a: &[f64], b: &[f64]) -> u64 {
        self.0.motion_cost(a, b)
    }

    fn clearance(&self, _q: &[f64]) -> Option<f64> {
        None
    }
}

struct ControlAdapter<'a>(&'a dyn Control);

impl PlanControl for ControlAdapter<'_> {
    fn is_cancelled(&self) -> bool {
        self.0.cancelled()
    }
}

fn add(total: &mut Stats, run: &Stats) {
    total.iterations += run.iterations;
    total.state_checks += run.state_checks;
    total.motion_checks += run.motion_checks;
    total.nodes += run.nodes;
}

/// Shortens a planned path as the pipeline does (plan 5.8): whole-state shortcuts, then per-joint
/// shortcuts, then whole-state shortcuts again. Deterministic for a seed; the path stays valid for
/// `validity` and never gets longer. Used by the motion pipeline and the quality bench.
pub fn simplify_path(space: &Space, path: &mut Vec<Vec<f64>>, validity: &dyn Validity, seed: u64) {
    let mut rng = Rng::new(seed);
    shortcut(space, path, validity, &mut rng, 150);
    gc_plan::partial_shortcut(space, path, validity, &mut rng, 150);
    shortcut(space, path, validity, &mut rng, 50);
}

/// Orders goal configurations as plan 5.8 asks: per IK branch (configurations that differ only by whole
/// turns of joints) the variant nearest to the start, by distance plus a small penalty for joints beyond
/// half a turn (so that goals do not drift towards ±2π); then the other turn variants in the same order;
/// at most eight. Ties keep the given order. Used by the motion pipeline and the quality bench.
pub fn rank_goals(space: &Space, start: &[f64], candidates: Vec<Vec<f64>>) -> Vec<Vec<f64>> {
    let score = |q: &[f64]| {
        space.distance(start, q)
            + 0.1 * q.iter().map(|v| (v.abs() - std::f64::consts::PI).max(0.0)).sum::<f64>()
    };
    let mut ordered: Vec<(f64, Vec<f64>)> = candidates.into_iter().map(|q| (score(&q), q)).collect();
    ordered.sort_by(|a, b| a.0.total_cmp(&b.0));
    // The branch of a configuration: every joint brought into (-π, π], to the milliradian.
    let branch = |q: &[f64]| -> Vec<i64> {
        q.iter()
            .map(|v| {
                let wrapped = v - std::f64::consts::TAU * (v / std::f64::consts::TAU).round();
                #[allow(clippy::cast_possible_truncation, reason = "a joint angle in milliradians")]
                let key = (wrapped * 1e3).round() as i64;
                // -π and π are the same angle.
                if key == -3142 { 3142 } else { key }
            })
            .collect()
    };
    let mut seen = std::collections::BTreeSet::new();
    let (mut first, mut rest) = (Vec::new(), Vec::new());
    for (_, q) in ordered {
        if seen.insert(branch(&q)) { first.push(q) } else { rest.push(q) }
    }
    first.extend(rest);
    first.truncate(8);
    first
}

/// The goals worth planning to first: those at most twice as far from the start as the nearest one (in
/// the given order). Used by the motion pipeline and the quality bench.
pub fn near_goals(space: &Space, start: &[f64], goals: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let nearest = goals.iter().map(|g| space.distance(start, g)).fold(f64::INFINITY, f64::min);
    goals.iter().filter(|g| space.distance(start, g) <= 2.0 * nearest + 1e-9).cloned().collect()
}

/// The valid goal configurations, ranked by [`rank_goals`]. With `prefer`, the configuration in the
/// same IK branch and turn as `prefer` (nearest to it, every joint within half a radian) comes first.
fn resolve_goals(
    scene: &RobotScene,
    request: &MotionRequest,
    space: &Space,
    prefer: Option<&[f64]>,
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
            let target = scene.flange_target(world);
            let ik = NumericIk::new(chain, IkOptions::default());
            let seeded =
                request.goal_seeds.iter().filter(|q| q.len() == chain.dof()).cloned().collect::<Vec<_>>();
            let found =
                if seeded.is_empty() { ik.solve_many(&target, Some(&request.start), 32) } else { seeded };
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
    let mut valid = rank_goals(space, &request.start, valid);
    if let Some(prefer) = prefer.filter(|p| p.len() == chain.dof()) {
        let apart = |q: &[f64]| q.iter().zip(prefer).fold(0.0_f64, |acc, (a, b)| acc.max((a - b).abs()));
        let same_branch = (0..valid.len())
            .filter(|i| apart(&valid[*i]) < 0.5)
            .min_by(|a, b| apart(&valid[*a]).total_cmp(&apart(&valid[*b])).then(a.cmp(b)));
        if let Some(index) = same_branch {
            let preferred = valid.remove(index);
            valid.insert(0, preferred);
        }
    }
    valid.truncate(8);
    Ok(valid)
}

/// Shared handle to a robot model.
pub type SharedModel = Arc<gc_robot::RobotModel>;
