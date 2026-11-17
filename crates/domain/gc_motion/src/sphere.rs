//! TCP sphere mode (plan 5.8): a ball of radius `r` moving in space among the cell's obstacles, planned
//! with the same planners and path processing as a robot, without a robot. Useful to see whether a way
//! exists at all before a robot is chosen, and to compare planners on a simple problem.

use std::cell::RefCell;

use gc_collide::{BodyId, Proximity, ShapeDesc, World};
use gc_math::{DPose3, DVec3};
use gc_plan::{
    Algorithm, Control, Failure, Options, Request, Rng, Space, Stats, Validity, path_cost, plan, shortcut,
    validate,
};
use gc_traj::{Limits, Trajectory, Validated};

use crate::{BLEND_DEVIATION, MotionError, Obstacle, StateProblem};

/// A sphere motion to plan.
#[derive(Clone, Debug, PartialEq)]
pub struct SphereRequest {
    /// Radius of the sphere, metres.
    pub radius: f64,
    /// The distance it keeps from obstacles, metres.
    pub padding: f64,
    /// Where its centre starts.
    pub start: DVec3,
    /// Where its centre should end.
    pub goal: DVec3,
    /// The planner used when the straight line is blocked.
    pub algorithm: Algorithm,
    /// Planner settings (`range` is a distance in metres here).
    pub options: Options,
    /// Seed of the first planner run; further runs use the following seeds.
    pub seed: u64,
    /// How many seeds to run; the shortest result wins.
    pub runs: usize,
    /// Speed and acceleration limits of the centre per axis, m/s and m/s².
    pub max_vel: f64,
    /// See `max_vel`.
    pub max_acc: f64,
}

/// A planned sphere motion.
#[derive(Clone, Debug)]
pub struct SpherePlan {
    /// The centre's waypoints.
    pub path: Vec<DVec3>,
    /// The timed motion (three coordinates: x, y, z).
    pub trajectory: Trajectory<Validated>,
    /// Length of the path, metres.
    pub length: f64,
    /// True when the straight line was free and no planner ran.
    pub straight: bool,
    /// Planner counters, summed over all runs.
    pub stats: Stats,
}

/// The sphere among the obstacles.
struct SphereWorld {
    world: RefCell<World>,
    ball: BodyId,
    /// Largest step between checked positions of a motion, metres.
    step: f64,
}

impl SphereWorld {
    fn problem(&self, q: &[f64]) -> Option<StateProblem> {
        let mut world = self.world.borrow_mut();
        let [x, y, z] = [q.first(), q.get(1), q.get(2)].map(|v| v.copied().unwrap_or(f64::NAN));
        world.set_pose(self.ball, DPose3::from_translation(DVec3::new(x, y, z)));
        match world.first_hit() {
            Ok(None) => None,
            Ok(Some(hit)) => {
                let (a, b) = (world.name(hit.a).to_owned(), world.name(hit.b).to_owned());
                Some(match hit.proximity {
                    Proximity::WithinPadding(gap) => StateProblem::WithinPadding { a, b, gap },
                    _ => StateProblem::Collision { a, b },
                })
            }
            Err(_) => Some(StateProblem::Unknown),
        }
    }
}

impl Validity for SphereWorld {
    fn state_valid(&self, q: &[f64]) -> bool {
        self.problem(q).is_none()
    }

    fn motion_valid(&self, a: &[f64], b: &[f64]) -> bool {
        let pieces = self.motion_cost(a, b);
        (1..pieces).all(|k| {
            let t = k as f64 / pieces as f64;
            let q: Vec<f64> = a.iter().zip(b).map(|(x, y)| x + (y - x) * t).collect();
            self.state_valid(&q)
        })
    }

    fn motion_cost(&self, a: &[f64], b: &[f64]) -> u64 {
        let length = a.iter().zip(b).map(|(x, y)| (y - x).powi(2)).sum::<f64>().sqrt();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "a small positive count")]
        let pieces = (length / self.step).ceil().clamp(1.0, 1e6) as u64;
        pieces
    }
}

/// Plans the sphere's motion: the straight line if it is free, else the planner from several seeds
/// (shortest result), shortcut, re-validated, timed and validated, as for a robot.
///
/// The sphere is checked against the padding plus the distance it travels in one checking step, so the
/// padding holds between checked positions too.
pub fn plan_sphere(
    obstacles: &[Obstacle],
    request: &SphereRequest,
    control: &dyn Control,
) -> Result<SpherePlan, MotionError> {
    let positive = |v: f64| v.is_finite() && v > 0.0;
    if !(positive(request.radius) && request.padding.is_finite() && request.padding >= 0.0)
        || !(positive(request.max_vel) && positive(request.max_acc))
        || !(request.start.is_finite() && request.goal.is_finite())
        || request.runs == 0
    {
        return Err(MotionError::InvalidRequest(
            "radius, padding, limits and positions must be valid".to_owned(),
        ));
    }
    // Checking step: a fifth of the radius, at most 10 mm.
    let step = (request.radius / 5.0).min(0.01);
    let build = |margin: f64| -> Result<SphereWorld, MotionError> {
        let mut world = World::new();
        let ball = world
            .add_body(
                "sphere",
                true,
                request.padding + margin,
                &[(DPose3::IDENTITY, ShapeDesc::Ball(request.radius))],
            )
            .map_err(|e| MotionError::InvalidRequest(e.to_string()))?;
        for obstacle in obstacles {
            let body = world
                .add_body(&obstacle.name, false, 0.0, &[(DPose3::IDENTITY, obstacle.shape.clone())])
                .map_err(|e| MotionError::InvalidRequest(e.to_string()))?;
            world.set_pose(body, obstacle.pose);
        }
        Ok(SphereWorld { world: RefCell::new(world), ball, step })
    };
    let strict = build(0.0)?;
    let checker = build(step / 2.0)?;
    let (start, goal) = (request.start.to_array().to_vec(), request.goal.to_array().to_vec());
    if let Some(problem) = strict.problem(&start) {
        return Err(MotionError::StartInvalid(problem));
    }
    if let Some(problem) = strict.problem(&goal) {
        return Err(MotionError::GoalInvalid(problem));
    }
    // The space: the box around start, goal and obstacles, grown by a metre.
    let mut lo = request.start.min(request.goal);
    let mut hi = request.start.max(request.goal);
    for obstacle in obstacles {
        let reach = obstacle.shape.radius();
        lo = lo.min(obstacle.pose.translation - DVec3::splat(reach));
        hi = hi.max(obstacle.pose.translation + DVec3::splat(reach));
    }
    let space = Space {
        lo: (lo - DVec3::ONE).to_array().to_vec(),
        hi: (hi + DVec3::ONE).to_array().to_vec(),
        weights: vec![1.0; 3],
    };
    let mut stats = Stats::default();
    let mut straight = false;
    let mut best: Option<Vec<Vec<f64>>> = None;
    let mut nearest = f64::INFINITY;
    if checker.state_valid(&start) && checker.state_valid(&goal) && checker.motion_valid(&start, &goal) {
        straight = true;
        best = Some(vec![start.clone(), goal.clone()]);
    } else {
        let goals = [goal.clone()];
        for run in 0..request.runs {
            let seed = request.seed.wrapping_add(run as u64);
            let planned = Request {
                space: &space,
                start: &start,
                goals: &goals,
                algorithm: request.algorithm,
                options: request.options,
                seed,
            };
            match plan(&planned, &checker, control) {
                Ok(solution) => {
                    add(&mut stats, &solution.stats);
                    let mut path = solution.path;
                    shortcut(&space, &mut path, &checker, &mut Rng::new(seed), 150);
                    let shorter =
                        best.as_ref().is_none_or(|b| path_cost(&space, &path) < path_cost(&space, b));
                    if shorter && validate(&path, &checker) {
                        best = Some(path);
                    }
                }
                Err(Failure::Cancelled) => return Err(MotionError::Cancelled),
                Err(Failure::NoPath { stats: run_stats, nearest_goal_distance }) => {
                    add(&mut stats, &run_stats);
                    nearest = nearest.min(nearest_goal_distance);
                }
                Err(Failure::StartInvalid | Failure::NoValidGoal) => {
                    // Valid with the padding but not with the checking margin: too tight to plan in.
                    return Err(MotionError::NoPath { stats, nearest_goal_distance: f64::INFINITY });
                }
                Err(other) => return Err(MotionError::InvalidRequest(other.to_string())),
            }
        }
    }
    let Some(path) = best else {
        return Err(MotionError::NoPath { stats, nearest_goal_distance: nearest });
    };
    let limits = Limits { max_vel: vec![request.max_vel; 3], max_acc: vec![request.max_acc; 3] };
    let blended = (path.len() > 2)
        .then(|| {
            Trajectory::blended(&path, &limits, BLEND_DEVIATION.min(request.radius), &|q| {
                checker.state_valid(q)
            })
            .ok()
        })
        .flatten()
        .and_then(|timed| timed.validate(&limits, 0.004, &|q| strict.state_valid(q)).ok());
    let trajectory = match blended {
        Some(trajectory) => trajectory,
        None => Trajectory::stopping_at_waypoints(&path, &limits)?
            .validate(&limits, 0.004, &|q| strict.state_valid(q))?,
    };
    Ok(SpherePlan {
        length: path_cost(&space, &path),
        path: path.iter().map(|q| DVec3::new(q[0], q[1], q[2])).collect(),
        trajectory,
        straight,
        stats,
    })
}

fn add(total: &mut Stats, run: &Stats) {
    total.iterations += run.iterations;
    total.state_checks += run.state_checks;
    total.motion_checks += run.motion_checks;
    total.nodes += run.nodes;
}
