//! QA edge cases for the robot cell: speed limits, degenerate moves, damaged tools and placements.

#![allow(clippy::float_cmp, clippy::unwrap_used, clippy::panic, reason = "test code")]

use gc_doc::{CellOp, Command, Placement, Solid, Tool};
use gc_session::{PlanStatus, Session};

fn six_axis() -> String {
    let path = format!("{}/../../../fixtures/urdf/six_axis.urdf", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn linear_path(
    session: &mut Session,
    targets: &[[f64; 6]],
) -> (gc_core::Id<gc_doc::Robot>, gc_core::Id<gc_doc::MotionPath>) {
    let robot = session.add_robot("Robot1", &six_axis()).unwrap();
    let mut waypoints = Vec::new();
    for q in targets {
        session.jog(robot, q);
        session.end_jog();
        waypoints.push(session.add_waypoint_here(robot).unwrap());
    }
    let path = session.add_path(robot, waypoints).unwrap();
    for index in 0..targets.len() {
        session.execute(&Command::Cell(CellOp::SetLinear { id: path, index, linear: true })).unwrap();
    }
    (robot, path)
}

#[test]
fn a_non_finite_tcp_speed_limit_is_refused() {
    let mut session = Session::new();
    let (_, path) = linear_path(&mut session, &[[0.6, 0.2, 0.3, 0.0, 0.5, 0.0]]);
    let before = session.doc().clone();
    for speed in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.0, 0.0] {
        let result = session.execute(&Command::Cell(CellOp::SetTcpSpeed { id: path, speed: Some(speed) }));
        assert!(result.is_err(), "{speed}");
        assert!(session.doc().same_content(&before), "{speed}");
    }
}

#[test]
fn a_very_small_speed_limit_plans_a_long_but_finite_move_or_fails_cleanly() {
    let mut session = Session::new();
    let (_, path) =
        linear_path(&mut session, &[[0.6, 0.2, 0.3, 0.0, 0.5, 0.0], [-0.6, 0.2, 0.3, 0.0, 0.5, 0.0]]);
    session.execute(&Command::Cell(CellOp::SetTcpSpeed { id: path, speed: Some(1e-6) })).unwrap();
    match session.plan(path) {
        PlanStatus::Ready => {
            let d = session.planned(path).unwrap().duration();
            assert!(d.is_finite() && d > 100.0, "{d}");
        }
        PlanStatus::Failed => assert!(session.plan_error(path).is_some()),
        other => panic!("{other:?}"),
    }
}

/// Two identical consecutive waypoints on a straight move: a zero-length line must not divide by zero.
#[test]
fn a_zero_length_straight_move_is_harmless() {
    let mut session = Session::new();
    let q = [0.6, 0.2, 0.3, 0.0, 0.5, 0.0];
    let (_, path) = linear_path(&mut session, &[q, q]);
    match session.plan(path) {
        PlanStatus::Ready => {
            let planned = session.planned(path).unwrap();
            assert!(planned.duration().is_finite() && planned.peak_tcp_speed().is_finite());
        }
        PlanStatus::Failed => assert!(session.plan_error(path).is_some()),
        other => panic!("{other:?}"),
    }
}

#[test]
fn damaged_tools_and_placements_are_refused_and_change_nothing() {
    let mut session = Session::new();
    let robot = session.add_robot("Robot1", &six_axis()).unwrap();
    let before = session.doc().clone();
    let placement = |xyz: [f64; 3]| Placement { xyz, rpy: [0.0; 3] };
    let solids = [
        Solid::Cylinder { radius: 0.0, length: 0.1 },
        Solid::Cylinder { radius: f64::NAN, length: 0.1 },
        Solid::Box { size: [0.1, f64::INFINITY, 0.1] },
        Solid::Box { size: [0.0, 0.1, 0.1] },
        Solid::Sphere { radius: -0.1 },
    ];
    for solid in solids {
        let tool = Tool { solid: solid.clone(), placement: placement([0.0; 3]) };
        assert!(
            session.execute(&Command::Cell(CellOp::SetTool { id: robot, tool: Some(tool) })).is_err(),
            "{solid:?}"
        );
    }
    let tool = Tool { solid: Solid::Sphere { radius: 0.05 }, placement: placement([f64::NAN, 0.0, 0.0]) };
    assert!(session.execute(&Command::Cell(CellOp::SetTool { id: robot, tool: Some(tool) })).is_err());
    for xyz in [[f64::NAN, 0.0, 0.0], [f64::INFINITY, 0.0, 0.0]] {
        let op = CellOp::PlaceRobot { id: robot, placement: placement(xyz) };
        assert!(session.execute(&Command::Cell(op)).is_err(), "{xyz:?}");
    }
    assert!(session.doc().same_content(&before));
}

/// A 100 m box tool engulfing the whole arm is a collision with the arm's own links: the tool is checked
/// against every link except the tip link and the link it is mounted on.
#[test]
fn a_huge_tool_is_a_collision_problem_not_a_crash() {
    let mut session = Session::new();
    let robot = session.add_robot("Robot1", &six_axis()).unwrap();
    session.jog(robot, &[0.6, 0.2, 0.3, 0.0, 0.5, 0.0]);
    session.end_jog();
    let tool = Tool {
        solid: Solid::Box { size: [100.0; 3] },
        placement: Placement { xyz: [0.0; 3], rpy: [0.0; 3] },
    };
    if session.execute(&Command::Cell(CellOp::SetTool { id: robot, tool: Some(tool) })).is_ok() {
        assert!(session.robot_problem(robot).is_some());
    }
}
