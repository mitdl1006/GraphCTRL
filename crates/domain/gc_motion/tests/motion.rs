//! The motion pipeline on a six-axis arm that is not a UR: free moves, moves around an obstacle, pose
//! goals, and failures that name their cause.

#![allow(
    clippy::float_cmp,
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: exact values are compared and failures should panic"
)]

use std::sync::Arc;

use gc_collide::ShapeDesc;
use gc_math::{DPose3, DQuat, DVec3};
use gc_motion::{Goal, MotionError, MotionRequest, Obstacle, RobotScene, StateProblem, plan_motion};
use gc_plan::{Algorithm, Control, NeverCancel, Options};
use gc_robot::import;

const PADDING: f64 = 0.005;

fn scene(obstacles: &[Obstacle]) -> RobotScene {
    let path = format!("{}/../../../fixtures/urdf/six_axis.urdf", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let (model, _) = import(&text).unwrap_or_else(|findings| panic!("{findings:?}"));
    let chain = model.chain("base_link", "tool0").unwrap();
    let (scene, missing) =
        RobotScene::new(Arc::new(model), chain, "tool0", DPose3::IDENTITY, obstacles, PADDING, &|_| None)
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
