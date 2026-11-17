//! Waypoints associated with equipment (lead's robot-cell order ②): a work frame made on a component's
//! face or origin plane follows the component as Project Geometry does, and the waypoints in it follow too.

#![allow(clippy::unwrap_used, reason = "test code")]

use gc_doc::{CellOp, Command, Occurrence, OriginPlane, Placement, Solid, WaypointTarget};
use gc_math::DVec3;
use gc_session::Session;

fn six_axis() -> String {
    let path = format!("{}/../../../fixtures/urdf/six_axis.urdf", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn a_work_frame_on_a_fixture_plane_carries_its_waypoints_when_the_fixture_moves() {
    let mut session = Session::new();
    let robot = session.add_robot("Robot1", &six_axis()).unwrap();
    let fixture = session
        .add_body(Solid::Box { size: [0.4, 0.4, 0.2] }, Placement { xyz: [0.5, 0.0, 0.1], rpy: [0.0; 3] })
        .unwrap();
    // A sketch on the fixture's XY plane; its fixed origin point is the origin of a work frame (a point
    // held by dimensions would do as well).
    let sketch =
        session.create_sketch_on_component(Occurrence::Item(fixture.erase()), OriginPlane::Xy).unwrap();
    let point = session.active_sketch().and_then(|(_, s)| s.points.keys().next().copied()).unwrap();
    let frame = session.add_work_frame(sketch, point).unwrap();
    session.finish_sketch();
    let offset = Placement { xyz: [0.0, 0.0, 0.2], rpy: [std::f64::consts::PI, 0.0, 0.0] };
    let waypoint = session.add_waypoint(robot, WaypointTarget::InFrame { frame, offset }).unwrap();
    let before = session.waypoint_pose(waypoint).unwrap();
    assert!((before.translation - DVec3::new(0.5, 0.0, 0.3)).length() < 1e-9, "{before:?}");

    // The fixture moves and turns: the frame and the waypoint follow, as projected geometry does.
    let mut moved = session.doc().get(fixture).unwrap().clone();
    moved.placement = Placement { xyz: [0.3, 0.4, 0.1], rpy: [0.0, 0.0, std::f64::consts::FRAC_PI_2] };
    session.execute(&Command::Cell(CellOp::SetBody { id: fixture, body: moved })).unwrap();
    let after = session.waypoint_pose(waypoint).unwrap();
    assert!((after.translation - DVec3::new(0.3, 0.4, 0.3)).length() < 1e-9, "{after:?}");
    // Turned with it about Z.
    let x = after.rotation * DVec3::X;
    assert!((x - DVec3::Y).length() < 1e-9, "{x}");
    // And the robot's IK aims at the new place.
    assert!(session.waypoint_ghost(waypoint).is_some());
}

#[test]
fn a_component_frame_is_one_step_and_its_waypoints_follow_a_constrained_fixture() {
    use gc_doc::{AxisSense, Geometry, RelationKind, Relationship, Selection};
    let mut session = Session::new();
    let table = session
        .add_body(Solid::Box { size: [1.0, 1.0, 0.8] }, Placement { xyz: [0.6, 0.0, 0.4], rpy: [0.0; 3] })
        .unwrap();
    let robot = session.add_robot("Robot1", &six_axis()).unwrap();
    let fixture = session
        .add_body(Solid::Box { size: [0.2, 0.2, 0.1] }, Placement { xyz: [0.5, 0.2, 1.5], rpy: [0.0; 3] })
        .unwrap();
    let depth = session.history().undo_depth();
    let frame = session.add_component_frame(Occurrence::Item(fixture.erase()), OriginPlane::Xy).unwrap();
    assert_eq!(session.history().undo_depth(), depth + 1, "one undo step");
    let offset = Placement { xyz: [0.0, 0.0, 0.15], rpy: [std::f64::consts::PI, 0.0, 0.0] };
    let waypoint = session.add_waypoint(robot, WaypointTarget::InFrame { frame, offset }).unwrap();
    assert!(
        (session.waypoint_pose(waypoint).unwrap().translation - DVec3::new(0.5, 0.2, 1.65)).length() < 1e-9
    );

    // The fixture is mated onto the table: the waypoint comes down with it.
    let face = |id: gc_core::Id<gc_doc::Body>, z: f64, n: f64| Selection {
        occurrence: Occurrence::Item(id.erase()),
        geometry: Geometry::Plane { point: [0.0, 0.0, z], normal: [0.0, 0.0, n] },
    };
    session
        .add_relationship(Relationship::new(
            "Mate:1",
            RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
            face(fixture, -0.05, -1.0),
            face(table, 0.4, 1.0),
        ))
        .unwrap();
    let at = session.waypoint_pose(waypoint).unwrap().translation;
    assert!((at - DVec3::new(0.5, 0.2, 0.85 + 0.15)).length() < 1e-7, "{at}");
    // A component that is gone is refused.
    let gone = session.add_body(Solid::Box { size: [0.1; 3] }, Placement::default()).unwrap();
    session.delete_item(gc_session::CellItem::Body(gone)).unwrap();
    assert!(session.add_component_frame(Occurrence::Item(gone.erase()), OriginPlane::Xy).is_err());
}
