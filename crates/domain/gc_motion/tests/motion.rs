//! The motion pipeline on a six-axis arm that is not a UR: free moves, moves around an obstacle, pose
//! goals, and failures that name their cause.

#![allow(
    clippy::float_cmp,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout,
    reason = "test code: exact values are compared and failures should panic"
)]

use std::sync::Arc;

use gc_collide::ShapeDesc;
use gc_math::{DPose3, DQuat, DVec3};
use gc_motion::{
    ContactAllowance, Goal, MotionError, MotionRequest, Obstacle, RobotScene, StateProblem, plan_motion,
    replan,
};
use gc_plan::{Algorithm, Control, NeverCancel, Options};
use gc_robot::import;

const PADDING: f64 = 0.005;

fn scene(obstacles: &[Obstacle]) -> RobotScene {
    let path = format!("{}/../../../fixtures/urdf/six_axis.urdf", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let (model, _) = import(&text).unwrap_or_else(|findings| panic!("{findings:?}"));
    let chain = model.chain("base_link", "tool0").unwrap();
    let (scene, missing) = RobotScene::new(
        Arc::new(model),
        chain,
        "tool0",
        DPose3::IDENTITY,
        obstacles,
        PADDING,
        &|_| None,
        &[],
    )
    .unwrap();
    assert!(missing.is_empty());
    scene
}

/// A pillar standing where the arm sweeps through when it turns about its base.
fn pillar() -> Obstacle {
    Obstacle {
        name: "pillar".to_owned(),
        pose: DPose3::from_parts(DVec3::new(0.55, 0.0, 0.7), DQuat::IDENTITY),
        shape: ShapeDesc::Box([0.1, 0.1, 1.4]),
    }
}

fn request(start: [f64; 6], goal: Goal, algorithm: Algorithm) -> MotionRequest {
    MotionRequest {
        start: start.to_vec(),
        goal,
        algorithm,
        options: Options::default(),
        seed: 7,
        runs: 4,
        max_acc: vec![5.0; 6],
        linear: false,
        max_tcp_speed: None,
        goal_seeds: Vec::new(),
        contacts: Vec::new(),
    }
}

const LEFT: [f64; 6] = [-1.0, 0.2, 0.3, 0.0, 0.5, 0.0];
const RIGHT: [f64; 6] = [1.0, 0.2, 0.3, 0.0, 0.5, 0.0];

#[test]
fn a_free_move_goes_straight_and_takes_the_trapezoid_time() {
    let scene = scene(&[]);
    let plan = plan_motion(
        &scene,
        &request(LEFT, Goal::Joints(RIGHT.to_vec()), Algorithm::RrtConnect),
        &NeverCancel,
    )
    .unwrap();
    assert!(plan.straight);
    assert_eq!(plan.path, vec![LEFT.to_vec(), RIGHT.to_vec()]);
    assert_eq!(plan.stats.iterations, 0);
    // Joint 1 moves 2 rad at 2 rad/s and 5 rad/s²: 2/2 + 2/5 seconds.
    assert!((plan.trajectory.duration() - 1.4).abs() < 1e-12, "{}", plan.trajectory.duration());
    assert!(plan.min_clearance > 0.0);
    assert!(plan.tip_path.len() > 10);
}

#[test]
fn every_planner_gets_around_the_pillar_with_a_valid_trajectory() {
    let scene = scene(&[pillar()]);
    for algorithm in [
        Algorithm::RrtConnect,
        Algorithm::Rrt,
        Algorithm::RrtStar,
        Algorithm::InformedRrtStar,
        Algorithm::Prm,
    ] {
        let mut request = request(LEFT, Goal::Joints(RIGHT.to_vec()), algorithm);
        if matches!(algorithm, Algorithm::RrtStar | Algorithm::InformedRrtStar) {
            // These spend most of their checks on rewiring and always use their whole budget: one run
            // with a larger budget instead of four.
            request.options.max_state_checks = 600_000;
            request.runs = 1;
        }
        let plan =
            plan_motion(&scene, &request, &NeverCancel).unwrap_or_else(|e| panic!("{algorithm:?}: {e}"));
        assert!(!plan.straight, "{algorithm:?}");
        assert!(plan.path.len() > 2, "{algorithm:?}");
        assert_eq!(plan.path.first().unwrap(), &LEFT.to_vec());
        assert_eq!(plan.path.last().unwrap(), &RIGHT.to_vec());
        // Checked here independently of the pipeline: no sampled state is in collision or inside the padding.
        let steps = (plan.trajectory.duration() / 0.002).ceil() as usize;
        for k in 0..=steps {
            let q = plan.trajectory.sample(plan.trajectory.duration() * k as f64 / steps as f64).q;
            assert_eq!(scene.state_problem(&q), None, "{algorithm:?} at sample {k}");
        }
        assert!(plan.min_clearance >= PADDING, "{algorithm:?}: {}", plan.min_clearance);
        // Through its corners the motion does not stop: it is quicker than stopping at each of them.
        assert!(plan.trajectory.is_blended(), "{algorithm:?}");
        let limits = gc_traj::Limits { max_vel: vec![2.0, 2.0, 2.5, 3.5, 3.5, 5.0], max_acc: vec![5.0; 6] };
        let stopping = gc_traj::Trajectory::stopping_at_waypoints(&plan.path, &limits).unwrap().duration();
        assert!(
            plan.trajectory.duration() < stopping,
            "{algorithm:?}: {} vs {stopping}",
            plan.trajectory.duration()
        );
        println!(
            "{algorithm:?}: {:.2} s blended, {stopping:.2} s stopping, {} waypoints",
            plan.trajectory.duration(),
            plan.path.len()
        );
    }
}

#[test]
fn the_same_seed_gives_the_same_plan() {
    let scene = scene(&[pillar()]);
    let request = request(LEFT, Goal::Joints(RIGHT.to_vec()), Algorithm::RrtConnect);
    let first = plan_motion(&scene, &request, &NeverCancel).unwrap();
    let second = plan_motion(&scene, &request, &NeverCancel).unwrap();
    assert_eq!(first.path, second.path);
    assert_eq!(first.cost.to_bits(), second.cost.to_bits());
    assert_eq!(first.trajectory.duration().to_bits(), second.trajectory.duration().to_bits());
    assert_eq!(first.stats, second.stats);
}

#[test]
fn a_pose_goal_is_reached_by_the_tip() {
    let scene = scene(&[pillar()]);
    let target = scene.tip_in_world(&RIGHT);
    let plan =
        plan_motion(&scene, &request(LEFT, Goal::Pose(target), Algorithm::RrtConnect), &NeverCancel).unwrap();
    let reached = scene.tip_in_world(plan.path.last().unwrap());
    assert!((reached.translation - target.translation).length() < 1e-5);
    assert!(reached.rotation.angle_between(target.rotation) < 1e-5);
}

#[test]
fn failures_name_their_cause() {
    let blocked = scene(&[pillar()]);
    // The arm stretched towards +X stands in the pillar.
    let inside = [0.0, 0.2, 0.3, 0.0, 0.5, 0.0];
    match plan_motion(
        &blocked,
        &request(inside, Goal::Joints(RIGHT.to_vec()), Algorithm::RrtConnect),
        &NeverCancel,
    ) {
        Err(MotionError::StartInvalid(StateProblem::Collision { a, b })) => {
            assert!(a == "pillar" || b == "pillar", "{a} / {b}");
        }
        other => panic!("{other:?}"),
    }
    match plan_motion(
        &blocked,
        &request(LEFT, Goal::Joints(inside.to_vec()), Algorithm::RrtConnect),
        &NeverCancel,
    ) {
        Err(MotionError::GoalInvalid(StateProblem::Collision { .. })) => {}
        other => panic!("{other:?}"),
    }
    let free = scene(&[]);
    let beyond = [1.0, 2.5, 0.3, 0.0, 0.5, 0.0];
    assert_eq!(
        plan_motion(
            &free,
            &request(LEFT, Goal::Joints(beyond.to_vec()), Algorithm::RrtConnect),
            &NeverCancel
        )
        .err(),
        Some(MotionError::GoalInvalid(StateProblem::JointLimit { joint: "joint_2".to_owned() }))
    );
    let far = DPose3::from_parts(DVec3::new(5.0, 0.0, 0.5), DQuat::IDENTITY);
    assert!(matches!(
        plan_motion(&free, &request(LEFT, Goal::Pose(far), Algorithm::RrtConnect), &NeverCancel),
        Err(MotionError::GoalUnreachable(_))
    ));
    assert!(matches!(
        plan_motion(&free, &request(LEFT, Goal::Joints(vec![0.0; 3]), Algorithm::RrtConnect), &NeverCancel),
        Err(MotionError::InvalidRequest(_))
    ));
}

#[test]
fn cancelling_stops_the_planner() {
    struct Cancelled;
    impl Control for Cancelled {
        fn cancelled(&self) -> bool {
            true
        }
    }
    let scene = scene(&[pillar()]);
    let request = request(LEFT, Goal::Joints(RIGHT.to_vec()), Algorithm::RrtConnect);
    assert_eq!(plan_motion(&scene, &request, &Cancelled).err(), Some(MotionError::Cancelled));
}

#[test]
fn a_linear_move_keeps_the_tip_on_the_line() {
    let scene = scene(&[]);
    // From a bent pose, 0.25 m sideways and a little up, keeping the tool's direction.
    let start = [0.0, 0.2, 0.9, 0.0, 0.5, 0.0];
    let from = scene.tip_in_world(&start);
    let to = DPose3::from_parts(from.translation + DVec3::new(-0.05, 0.25, 0.05), from.rotation);
    let mut request = request(start, Goal::Pose(to), Algorithm::RrtConnect);
    request.linear = true;
    let plan = plan_motion(&scene, &request, &NeverCancel).unwrap();
    let along = (to.translation - from.translation).normalize();
    for tip in &plan.tip_path {
        let offset = DVec3::from_array(*tip) - from.translation;
        let off_line = (offset - along * offset.dot(along)).length();
        assert!(off_line < 0.5e-3, "{off_line} m off the line");
    }
    let steps = (plan.trajectory.duration() / 0.01).ceil() as usize;
    for k in 0..=steps {
        let q = plan.trajectory.sample(plan.trajectory.duration() * k as f64 / steps as f64).q;
        assert!(
            scene.tip_in_world(&q).rotation.angle_between(from.rotation) < 2e-3,
            "the tool keeps its direction"
        );
    }
    let end = scene.tip_in_world(plan.path.last().unwrap());
    assert!((end.translation - to.translation).length() < 1e-5);
    assert!(plan.trajectory.is_blended(), "driven through its steps without stopping");
    // The same request again gives the same motion.
    let again = plan_motion(&scene, &request, &NeverCancel).unwrap();
    assert_eq!(again.path, plan.path);

    // A line that leaves the arm's reach is followed part of the way and refused with how far.
    let far = DPose3::from_parts(from.translation + DVec3::new(2.0, 0.0, 0.0), from.rotation);
    request.goal = Goal::Pose(far);
    match plan_motion(&scene, &request, &NeverCancel) {
        Err(MotionError::Linear { percent, .. }) => assert!(percent > 0.5 && percent < 60.0, "{percent}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_linear_move_through_an_obstacle_names_it() {
    let start = [-0.6, 0.2, 0.3, 0.0, 0.5, 0.0];
    let goal = [0.6, 0.2, 0.3, 0.0, 0.5, 0.0];
    let free = scene(&[]);
    let mut request = request(start, Goal::Joints(goal.to_vec()), Algorithm::RrtConnect);
    request.linear = true;
    let plan = plan_motion(&free, &request, &NeverCancel).unwrap();
    assert_eq!(plan.path.last().unwrap(), &goal.to_vec(), "a joint goal on the line is met exactly");
    // The chord between the two tip positions passes through the pillar.
    let blocked = scene(&[pillar()]);
    match plan_motion(&blocked, &request, &NeverCancel) {
        Err(MotionError::Linear { percent, cause }) => {
            assert!(percent > 10.0 && percent < 60.0, "{percent}");
            assert!(cause.contains("pillar"), "{cause}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn goals_are_about_the_tool_centre_point() {
    // A tool 120 mm long, bent 30 degrees.
    let tool = DPose3::from_parts(DVec3::new(0.02, 0.0, 0.12), DQuat::from_rotation_y(0.5));
    let bare = scene(&[]);
    let with_tool = scene(&[]).with_tool(tool);
    let q = [0.0, 0.2, 0.9, 0.0, 0.5, 0.0];
    let flange = bare.tip_in_world(&q);
    let tcp = with_tool.tip_in_world(&q);
    assert!(((flange * tool).translation - tcp.translation).length() < 1e-12);

    // A pose goal is reached by the tool centre point, not by the flange.
    let target = DPose3::from_parts(tcp.translation + DVec3::new(0.0, 0.15, 0.05), tcp.rotation);
    for linear in [false, true] {
        let mut request = request(q, Goal::Pose(target), Algorithm::RrtConnect);
        request.linear = linear;
        let plan = plan_motion(&with_tool, &request, &NeverCancel).unwrap();
        let end = plan.path.last().unwrap();
        assert!(
            (with_tool.tip_in_world(end).translation - target.translation).length() < 1e-5,
            "linear {linear}"
        );
        assert!((bare.tip_in_world(end).translation - target.translation).length() > 0.1, "linear {linear}");
        // The trace shown is the tool centre point's.
        let last = DVec3::from_array(*plan.tip_path.last().unwrap());
        assert!((last - target.translation).length() < 1e-5);
    }
}

#[test]
fn replanning_keeps_a_path_that_is_still_valid() {
    let scene = scene(&[pillar()]);
    let mut request = request(LEFT, Goal::Joints(RIGHT.to_vec()), Algorithm::RrtConnect);
    let first = plan_motion(&scene, &request, &NeverCancel).unwrap();
    assert!(!first.reused);
    // Nothing that matters changed (a different seed would plan differently): the path is kept.
    request.seed = 99;
    let again = replan(&scene, &request, &first, &NeverCancel).unwrap();
    assert!(again.reused);
    assert_eq!(again.path, first.path);
    assert_eq!(again.stats.state_checks, 0, "no planner ran");
    // A slower robot: the same path, timed again.
    request.max_acc = vec![1.0; 6];
    let slower = replan(&scene, &request, &first, &NeverCancel).unwrap();
    assert!(slower.reused && slower.trajectory.duration() > first.trajectory.duration());

    // An obstacle in the way of the old path: planned again, and the new path is valid.
    let blocker = Obstacle {
        name: "blocker".to_owned(),
        pose: scene.tip_in_world(&first.path[first.path.len() / 2]),
        shape: ShapeDesc::Ball(0.08),
    };
    let moved = self::scene(&[pillar(), blocker]);
    let fresh = replan(&moved, &request, &first, &NeverCancel).unwrap();
    assert!(!fresh.reused);
    assert_ne!(fresh.path, first.path);
    for q in &fresh.path {
        assert_eq!(moved.state_problem(q), None);
    }

    // A path that does not start at the start or end at the goal is never kept.
    let mut shifted = request.clone();
    shifted.goal = Goal::Joints(vec![1.0, 0.2, 0.3, 0.0, 0.6, 0.0]);
    assert!(!replan(&scene, &shifted, &first, &NeverCancel).unwrap().reused);
}

#[test]
fn replanning_a_moved_pose_goal_stays_in_the_same_configuration() {
    let scene = scene(&[]);
    // A wrist that can also be reached flipped (joint 4 and 6 half a turn round, joint 5 negated).
    let target = scene.tip_in_world(&[1.0, 0.2, 0.3, 1.0, 0.5, 1.0]);
    // The previous plan ended in an IK branch that is not the one nearest to the start.
    let ik = gc_robot::NumericIk::new(scene.chain(), gc_robot::IkOptions::default());
    let mut branches: Vec<Vec<f64>> = ik
        .solve_many(&scene.flange_target(&target), Some(&LEFT), 32)
        .into_iter()
        .filter(|q| scene.state_problem(q).is_none())
        .collect();
    let distance = |q: &[f64]| q.iter().zip(&LEFT).map(|(a, b)| (a - b).powi(2)).sum::<f64>();
    branches.sort_by(|a, b| distance(a).total_cmp(&distance(b)));
    let apart = |a: &[f64], b: &[f64]| a.iter().zip(b).fold(0.0_f64, |acc, (x, y)| acc.max((x - y).abs()));
    let other = branches.iter().skip(1).find(|q| apart(q, &branches[0]) > 0.5).unwrap().clone();
    let mut request = request(LEFT, Goal::Joints(other.clone()), Algorithm::RrtConnect);
    let first = plan_motion(&scene, &request, &NeverCancel).unwrap();
    // The waypoint moves by a millimetre: the old end no longer reaches it, the new end stays in that
    // branch although another one is nearer to the start.
    let moved = DPose3::from_parts(target.translation + DVec3::new(0.0, 0.001, 0.0), target.rotation);
    request.goal = Goal::Pose(moved);
    let fresh = plan_motion(&scene, &request, &NeverCancel).unwrap();
    assert!(apart(fresh.path.last().unwrap(), &other) > 0.5, "without the previous plan: the nearest branch");
    let plan = replan(&scene, &request, &first, &NeverCancel).unwrap();
    assert!(!plan.reused);
    let end = plan.path.last().unwrap();
    assert!(apart(end, &other) < 0.05, "the arm switched configurations ({} rad)", apart(end, &other));
    assert!((scene.tip_in_world(end).translation - moved.translation).length() < 1e-5);
}

#[test]
fn a_plan_carries_its_guarantee() {
    let scene = scene(&[pillar()]);
    let plan = plan_motion(
        &scene,
        &request(LEFT, Goal::Joints(RIGHT.to_vec()), Algorithm::RrtConnect),
        &NeverCancel,
    )
    .unwrap();
    let g = &plan.guarantee;
    assert_eq!(g.padding_nominal, PADDING);
    assert_eq!(g.padding_min, PADDING);
    // No robot point moves further than the padding between two checked states, so half the padding
    // is kept between them.
    assert!(g.step_mm > 0.0 && g.step_mm <= PADDING * 1e3 + 1e-9, "{}", g.step_mm);
    assert!(g.holds_between_states() && g.clearance >= PADDING / 2.0 - 1e-12);
    assert!(g.step_deg > 0.0 && g.states > (plan.trajectory.duration() / 0.004) as usize);
    assert_eq!(g.acm_hash, scene.excluded_pairs_hash());
    assert!(!scene.excluded_pairs().is_empty(), "neighbouring links are excluded");

    // The sweep bound really bounds the motion of the tip.
    let (a, b) = (&plan.path[0], &plan.path[1]);
    let moved = (scene.tip_in_world(a).translation - scene.tip_in_world(b).translation).length();
    assert!(scene.sweep_bound(a, b) >= moved, "{} < {moved}", scene.sweep_bound(a, b));
}

#[test]
fn a_contact_allowance_lets_a_goal_inside_the_padding_be_reached() {
    // A ball lowered towards the tip until it is inside the padding at the goal, but not touching.
    let tip = scene(&[]).tip_in_world(&RIGHT).translation;
    let ball = |height: f64| Obstacle {
        name: "fixture".to_owned(),
        pose: DPose3::from_translation(tip + DVec3::new(0.0, 0.0, height)),
        shape: ShapeDesc::Ball(0.02),
    };
    let (height, link) = (0..300)
        .map(|mm| 0.3 - f64::from(mm) * 0.001)
        .find_map(|h| match scene(&[ball(h)]).state_problem(&RIGHT) {
            Some(StateProblem::WithinPadding { a, b, .. }) => Some((h, if a == "fixture" { b } else { a })),
            _ => None,
        })
        .unwrap();
    let scene = scene(&[ball(height)]);
    let mut request = request(LEFT, Goal::Joints(RIGHT.to_vec()), Algorithm::RrtConnect);
    assert!(matches!(
        plan_motion(&scene, &request, &NeverCancel),
        Err(MotionError::GoalInvalid(StateProblem::WithinPadding { .. }))
    ));
    request.contacts = vec![ContactAllowance::near_waypoint(&link, "fixture")];
    let plan = plan_motion(&scene, &request, &NeverCancel).unwrap();
    assert_eq!(plan.path.last().unwrap(), &RIGHT.to_vec());
    // Away from the goal the padding still holds: at the start the tip is far from the allowance.
    assert_eq!(scene.state_problem(&LEFT), None);

    // Touching is refused unless the allowance says so.
    let touching = self::scene(&[ball(height - 0.02)]);
    assert!(matches!(touching.state_problem(&RIGHT), Some(StateProblem::Collision { .. })));
    assert!(plan_motion(&touching, &request, &NeverCancel).is_err());
    request.contacts[0].allow_touch = true;
    assert!(plan_motion(&touching, &request, &NeverCancel).is_ok());
}

#[test]
fn a_sphere_is_planned_around_a_wall_without_a_robot() {
    use gc_motion::{SphereRequest, plan_sphere};
    // A wall between start and goal, with the sphere 40 mm across.
    let wall = Obstacle {
        name: "wall".to_owned(),
        pose: DPose3::from_translation(DVec3::new(0.0, 0.0, 0.5)),
        shape: ShapeDesc::Box([0.02, 1.0, 1.0]),
    };
    let mut request = SphereRequest {
        radius: 0.02,
        padding: PADDING,
        start: DVec3::new(-0.3, 0.0, 0.5),
        goal: DVec3::new(0.3, 0.0, 0.5),
        algorithm: Algorithm::RrtConnect,
        options: Options { range: 0.2, ..Options::default() },
        seed: 3,
        runs: 4,
        max_vel: 0.25,
        max_acc: 1.0,
    };
    let walls = [wall];
    let plan = plan_sphere(&walls, &request, &NeverCancel).unwrap();
    assert!(!plan.straight && plan.path.len() > 2);
    assert_eq!(plan.path.first(), Some(&request.start));
    assert_eq!(plan.path.last(), Some(&request.goal));
    // Around the wall, not through it: longer than the straight 0.6 m, and every sample keeps the padding.
    assert!(plan.length > 0.6);
    let steps = (plan.trajectory.duration() / 0.002).ceil() as usize;
    for k in 0..=steps {
        let q = plan.trajectory.sample(plan.trajectory.duration() * k as f64 / steps as f64).q;
        // Distance from the centre to the wall box, minus the radius.
        let outside =
            DVec3::new(q[0].abs() - 0.01, q[1].abs() - 0.5, (q[2] - 0.5).abs() - 0.5).max(DVec3::ZERO);
        assert!(outside.length() - 0.02 >= PADDING - 1e-9, "sample {k} at {q:?} is inside the padding");
    }
    // Same seed, same plan.
    let again = plan_sphere(&walls, &request, &NeverCancel).unwrap();
    assert_eq!(again.path, plan.path);
    // A free line is taken straight; a goal inside the wall is refused with the cause.
    assert!(plan_sphere(&[], &request, &NeverCancel).unwrap().straight);
    request.goal = DVec3::new(0.0, 0.0, 0.5);
    assert!(matches!(plan_sphere(&walls, &request, &NeverCancel), Err(MotionError::GoalInvalid(_))));
}

#[test]
fn the_planning_margin_applies_to_the_tool_too() {
    use gc_plan::Validity;
    // A 0.3 m tool along the flange's Z; a ball just beyond the required padding from its tip.
    let tool = [(
        DPose3::from_translation(DVec3::new(0.0, 0.0, 0.15)),
        ShapeDesc::Cylinder { radius: 0.02, length: 0.3 },
    )];
    let path = format!("{}/../../../fixtures/urdf/six_axis.urdf", env!("CARGO_MANIFEST_DIR"));
    let (model, _) = import(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let chain = model.chain("base_link", "tool0").unwrap();
    let q = [0.0, 0.2, 0.3, 0.0, 0.5, 0.0];
    let probe = RobotScene::new(
        Arc::new(model.clone()),
        chain.clone(),
        "tool0",
        DPose3::IDENTITY,
        &[],
        PADDING,
        &|_| None,
        &tool,
    )
    .unwrap()
    .0;
    let flange = probe.tip_in_world(&q);
    let tip = flange * DVec3::new(0.0, 0.0, 0.3);
    let outward = flange.rotation * DVec3::Z;
    let ball = Obstacle {
        name: "ball".to_owned(),
        pose: DPose3::from_translation(tip + outward * (0.05 + PADDING + 0.002)),
        shape: ShapeDesc::Ball(0.05),
    };
    let (scene, _) = RobotScene::new(
        Arc::new(model),
        chain,
        "tool0",
        DPose3::IDENTITY,
        &[ball],
        PADDING,
        &|_| None,
        &tool,
    )
    .unwrap();
    assert!(scene.strict_checker().state_valid(&q), "2 mm beyond the padding");
    assert!(!scene.checker(1.0).state_valid(&q), "inside the planning margin, which covers the tool");
    assert!(scene.checker(1.0).margin() > 0.002);
}
