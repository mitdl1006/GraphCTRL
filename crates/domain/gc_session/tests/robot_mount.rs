//! A robot standing on equipment through assembly relationships (lead's robot-cell order ①): when the
//! equipment moves, the robot's base follows at once, and with it everything that is "in the base frame"
//! — base-frame waypoints, IK for world waypoints, the TCP — while plans made before become stale.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use gc_core::Id;
use gc_doc::{
    AxisSense, Body, CellOp, Command, Geometry, Occurrence, Placement, RelationKind, Relationship, Selection,
    Solid, WaypointTarget,
};
use gc_math::DPose3;
use gc_session::{PlanStatus, Session};

fn six_axis() -> String {
    let path = format!("{}/../../../fixtures/urdf/six_axis.urdf", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn close(a: &DPose3, b: &DPose3) -> bool {
    (a.translation - b.translation).length() < 1e-6 && a.rotation.angle_between(b.rotation) < 1e-6
}

fn placement_of(pose: &DPose3) -> Placement {
    Placement { xyz: pose.translation.to_array(), rpy: gc_math::rpy_from_quat(pose.rotation) }
}

const JOINTS: [f64; 6] = [0.3, -0.9, 1.2, -0.6, -0.4, 0.2];

/// A grounded pedestal 0.5 m high (its top at z = 0.5) and a robot tied onto it by `tie`.
fn mounted(
    tie: impl Fn(Id<gc_doc::Robot>, Id<Body>) -> Relationship,
) -> (Session, Id<Body>, Id<gc_doc::Robot>) {
    let mut session = Session::new();
    let pedestal = session
        .add_body(Solid::Box { size: [0.4, 0.4, 0.5] }, Placement { xyz: [0.0, 0.0, 0.25], rpy: [0.0; 3] })
        .unwrap();
    let robot = session.add_robot("Robot1", &six_axis()).unwrap();
    session.add_relationship(tie(robot, pedestal)).unwrap();
    (session, pedestal, robot)
}

fn mate(robot: Id<gc_doc::Robot>, pedestal: Id<Body>) -> Relationship {
    Relationship::new(
        "Mate:1",
        RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
        Selection {
            occurrence: Occurrence::Item(robot.erase()),
            geometry: Geometry::Plane { point: [0.0; 3], normal: [0.0, 0.0, -1.0] },
        },
        Selection {
            occurrence: Occurrence::Item(pedestal.erase()),
            geometry: Geometry::Plane { point: [0.0, 0.0, 0.25], normal: [0.0, 0.0, 1.0] },
        },
    )
}

fn rigid(robot: Id<gc_doc::Robot>, pedestal: Id<Body>) -> Relationship {
    Relationship::new(
        "Rigid:1",
        RelationKind::Rigid { flip: true },
        Selection { occurrence: Occurrence::Item(robot.erase()), geometry: Geometry::origin_frame() },
        Selection {
            occurrence: Occurrence::Item(pedestal.erase()),
            geometry: Geometry::Frame { origin: [0.0, 0.0, 0.25], z: [0.0, 0.0, 1.0], x: [1.0, 0.0, 0.0] },
        },
    )
}

/// Everything that hangs on the base follows the equipment.
fn follows_the_pedestal(mut session: Session, pedestal: Id<Body>, robot: Id<gc_doc::Robot>) {
    assert!((session.doc().get(robot).unwrap().placement.xyz[2] - 0.5).abs() < 1e-7, "on the top");
    session.execute(&Command::Cell(CellOp::SetJoints { id: robot, joints: JOINTS.to_vec() })).unwrap();
    let tcp = session.tcp_pose(robot).unwrap();
    let base = session.robot_base_frame(robot).unwrap();
    let world = session.add_waypoint(robot, WaypointTarget::InWorld { offset: placement_of(&tcp) }).unwrap();
    let in_base = session
        .add_waypoint(robot, WaypointTarget::InBase { offset: placement_of(&(base.inverse() * tcp)) })
        .unwrap();
    assert!(close(&session.waypoint_pose(in_base).unwrap(), &tcp));
    let ghost = session.waypoint_ghost(world).unwrap();
    let joints = ghost.joints.unwrap();
    assert!(joints.iter().zip(JOINTS).all(|(a, b)| (a - b).abs() < 1e-5), "{joints:?}");
    let path = session.add_path(robot, vec![world]).unwrap();
    assert_eq!(session.plan(path), PlanStatus::Ready, "{:?}", session.plan_error(path));

    // The pedestal moves 0.3 m along X and grows 0.1 m: the robot goes with it at once.
    let moved = Body {
        placement: Placement { xyz: [0.3, 0.0, 0.3], rpy: [0.0, 0.0, 0.4] },
        solid: Solid::Box { size: [0.4, 0.4, 0.6] },
        ..session.doc().get(pedestal).unwrap().clone()
    };
    session.execute(&Command::Cell(CellOp::SetBody { id: pedestal, body: moved })).unwrap();
    let placed = session.doc().get(robot).unwrap().placement;
    // The tie was to the old top (0.25 above the centre): 0.55 now; the turn about Z goes with it for the
    // joint, and the mate leaves it free to turn.
    assert!((placed.xyz[2] - 0.55).abs() < 1e-7, "{placed:?}");
    let base_now = session.robot_base_frame(robot).unwrap();
    assert!(!close(&base_now, &base), "the base frame moved");

    // The base-frame waypoint moves with the base; the world one stays.
    let expected = base_now * base.inverse() * tcp;
    assert!(close(&session.waypoint_pose(in_base).unwrap(), &expected));
    assert!(close(&session.waypoint_pose(world).unwrap(), &tcp));
    // IK for the world waypoint is solved from the new base: the arm reaches the same world pose.
    let ghost = session.waypoint_ghost(world).unwrap();
    let reach = ghost.joints.unwrap(); // still reachable
    session.jog(robot, &reach);
    assert!(close(&session.tcp_pose(robot).unwrap(), &tcp), "the TCP is on the world waypoint");
    session.end_jog();
    // A plan made before the move is no longer current.
    assert_eq!(session.plan_status(path), PlanStatus::Stale);
    // Undo puts the pedestal and the robot back together.
    session.undo(); // the jog
    session.undo(); // the move
    assert!(close(&session.robot_base_frame(robot).unwrap(), &base));
    assert_eq!(session.plan_status(path), PlanStatus::Ready, "back as planned");
}

#[test]
fn a_robot_mated_on_a_pedestal_follows_it_with_its_base_frame_waypoints_and_ik() {
    let (session, pedestal, robot) = mounted(mate);
    follows_the_pedestal(session, pedestal, robot);
}

#[test]
fn a_robot_jointed_on_a_pedestal_follows_it_and_turns_with_it() {
    let (session, pedestal, robot) = mounted(rigid);
    let start = session.doc().get(robot).unwrap().placement;
    assert!((start.xyz[2] - 0.5).abs() < 1e-7);
    follows_the_pedestal(session, pedestal, robot);
}
