//! QA edge cases of the closed-form UR solver: singular poses, the edge of the reach, damaged input.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use gc_plugin_api::Registry;
use gc_plugin_ur::UrPlugin;
use gc_robot::import;
use gc_robotics_api::{KINEMATICS_FACTORIES, Pose, ROBOT_MODEL_PROVIDERS};

fn max_error(a: &Pose, b: &Pose) -> f64 {
    a.xyz.iter().zip(&b.xyz).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max)
}

/// Poses a user can easily reach by jogging: wrist, elbow and shoulder singularities. Whatever the
/// solver answers must be finite and must reach the pose; "no solution" is not acceptable for a pose
/// that came from a real configuration.
#[test]
fn singular_poses_still_give_valid_finite_solutions() {
    let mut registry = Registry::default();
    registry.load(&UrPlugin).unwrap();
    let provider = &registry.get(ROBOT_MODEL_PROVIDERS)[0];
    let factory = &registry.get(KINEMATICS_FACTORIES)[0];
    let pi = std::f64::consts::PI;
    let cases: [[f64; 6]; 6] = [
        [0.0; 6],                                    // stretched out sideways: elbow singular
        [0.3, -pi / 2.0, 1e-3, -pi / 2.0, 0.4, 0.0], // elbow almost straight
        [0.0, -pi / 2.0, pi / 2.0, 0.0, 0.0, 0.7],   // wrist singular (q5 = 0)
        [0.0, -pi / 2.0, pi / 2.0, 0.0, pi, 0.7],    // wrist singular (q5 = pi)
        [1.0, -0.6, 1.2, -0.6, pi / 2.0 - 1e-7, 0.0],
        [-pi + 1e-7, -1.0, 1.0, -1.0, -1.0, pi - 1e-7], // at the angle wrap
    ];
    for model in ["ur3e", "ur5e", "ur10e", "ur16e"] {
        let (robot, _) = import(&provider.urdf(model).unwrap()).unwrap();
        let spec = robot.default_chain().unwrap().to_spec().unwrap();
        let solver = factory.build(&spec).unwrap();
        let (mut q_out, mut branches) = (Vec::new(), Vec::new());
        for q in cases {
            let target = spec.fk(&q).unwrap();
            let count = solver
                .solve_all(&target, Some(&q), &mut q_out, &mut branches)
                .unwrap_or_else(|e| panic!("{model} {q:?}: {e:?}"));
            assert!(count >= 1 && q_out.len() == count * 6 && branches.len() == count, "{model} {q:?}");
            assert!(q_out.iter().all(|v| v.is_finite()), "{model} {q:?}: {q_out:?}");
            for s in q_out.chunks(6) {
                let err = max_error(&spec.fk(s).unwrap(), &target);
                assert!(err < 1e-6, "{model} {q:?}: {s:?} misses by {err}");
            }
        }
    }
}

/// A damaged target (NaN, infinite, zero quaternion) is an error or an empty answer, never a panic and
/// never non-finite joint values.
#[test]
fn damaged_targets_are_refused_without_panic() {
    let mut registry = Registry::default();
    registry.load(&UrPlugin).unwrap();
    let provider = &registry.get(ROBOT_MODEL_PROVIDERS)[0];
    let factory = &registry.get(KINEMATICS_FACTORIES)[0];
    let (robot, _) = import(&provider.urdf("ur5e").unwrap()).unwrap();
    let spec = robot.default_chain().unwrap().to_spec().unwrap();
    let solver = factory.build(&spec).unwrap();
    let (mut q_out, mut branches) = (Vec::new(), Vec::new());
    let good = spec.fk(&[0.1, -1.0, 1.0, -1.0, -1.0, 0.2]).unwrap();
    let mut bad = Vec::new();
    for v in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e300] {
        bad.push(Pose { xyz: [v, 0.0, 0.5], ..good });
        bad.push(Pose { quat_xyzw: [v, 0.0, 0.0, 1.0], ..good });
    }
    bad.push(Pose { quat_xyzw: [0.0; 4], ..good });
    for target in bad {
        let result = solver.solve_all(&target, None, &mut q_out, &mut branches);
        if matches!(result, Ok(n) if n > 0) {
            assert!(q_out.iter().all(|v| v.is_finite()), "{target:?}: {q_out:?}");
        }
    }
    // The tool exactly at the base axis and at the edge of the reach.
    for xyz in [[0.0, 0.0, 0.3], [0.0, 0.0, 0.0], [0.8499, 0.0, 0.1625], [0.9, 0.0, 0.1625]] {
        let target = Pose { xyz, ..good };
        if let Ok(n) = solver.solve_all(&target, None, &mut q_out, &mut branches) {
            assert!(q_out.len() == n * 6 && q_out.iter().all(|v| v.is_finite()), "{xyz:?}");
        }
    }
}

/// The arm fully stretched (elbow exactly straight, the farthest the tool can be) is reachable because
/// it came from forward kinematics, but the solver reports `OutOfReach` (rounding pushes the elbow
/// cosine just past 1).
#[test]
#[ignore = "BUG: UR closed-form IK says OutOfReach for the exactly stretched arm (q3 = 0); see docs/handoff/qa.md"]
fn the_fully_stretched_arm_is_reachable() {
    let mut registry = Registry::default();
    registry.load(&UrPlugin).unwrap();
    let provider = &registry.get(ROBOT_MODEL_PROVIDERS)[0];
    let factory = &registry.get(KINEMATICS_FACTORIES)[0];
    let pi = std::f64::consts::PI;
    for model in ["ur3e", "ur5e", "ur10e", "ur16e"] {
        let (robot, _) = import(&provider.urdf(model).unwrap()).unwrap();
        let spec = robot.default_chain().unwrap().to_spec().unwrap();
        let solver = factory.build(&spec).unwrap();
        let (mut q_out, mut branches) = (Vec::new(), Vec::new());
        for q3 in [0.0, 1e-9, -1e-9, 1e-6] {
            let q = [0.3, -pi / 2.0, q3, -pi / 2.0, 0.4, 0.0];
            let target = spec.fk(&q).unwrap();
            assert!(
                solver.solve_all(&target, Some(&q), &mut q_out, &mut branches).is_ok(),
                "{model} q3={q3}"
            );
        }
    }
}
