//! Grip Snap (Inventor 2026 help, Grip Snap move or rotate, GUID-E1CA109A-D5A1-4131-AC3A-AE217716DD8C).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "tests")]

use gc_core::Id;
use gc_doc::{
    AxisSense, Body, ComponentFlag, Geometry, GripLine, GripMotion, Occurrence, Placement, RelationKind,
    Relationship, Selection, Solid,
};
use gc_math::DVec3;
use gc_session::Session;

fn body(s: &mut Session, xyz: [f64; 3]) -> Id<Body> {
    s.add_body(Solid::Box { size: [0.2; 3] }, Placement { xyz, rpy: [0.0; 3] }).unwrap()
}

fn origin(s: &Session, b: Id<Body>) -> gc_math::DPose3 {
    s.doc().component_origin(Occurrence::Item(b.erase())).unwrap().pose
}

fn x_line() -> GripLine {
    GripLine::through([0.0; 3], [1.0, 0.0, 0.0]).unwrap()
}

#[test]
fn moving_along_a_ray_takes_the_offset_along_it() {
    let mut s = Session::new();
    let b = body(&mut s, [0.0, 0.0, 0.5]);
    // The part of (0.3, 0.2, 0.1) along X.
    let pose =
        s.grip_snap_pose(b.erase(), GripMotion::MoveAlong, x_line(), [0.0; 3], [0.3, 0.2, 0.1]).unwrap();
    assert!((pose.translation - DVec3::new(0.3, 0.0, 0.5)).length() < 1e-12);
    assert!((pose.rotation.dot(origin(&s, b).rotation)).abs() > 1.0 - 1e-12, "no turn");
}

#[test]
fn rotating_about_a_line_takes_the_angle_seen_along_it() {
    let mut s = Session::new();
    let b = body(&mut s, [0.0; 3]);
    // About the vertical line through (1, 0, 0), from (2, 0, 0) to (1, 1, 5): a quarter turn.
    let z = GripLine { point: [1.0, 0.0, 0.0], direction: [0.0, 0.0, 2.0] };
    let pose =
        s.grip_snap_pose(b.erase(), GripMotion::RotateAbout, z, [2.0, 0.0, 0.0], [1.0, 1.0, 5.0]).unwrap();
    assert!((pose.translation - DVec3::new(1.0, -1.0, 0.0)).length() < 1e-12, "{:?}", pose.translation);
    let x = pose.rotation * DVec3::X;
    assert!((x - DVec3::Y).length() < 1e-12);
    // A point on the line gives no angle; an edge of no length gives no line.
    assert!(
        s.grip_snap_pose(b.erase(), GripMotion::RotateAbout, z, [1.0, 0.0, 3.0], [2.0, 0.0, 0.0]).is_none()
    );
    assert!(GripLine::through([1.0; 3], [1.0; 3]).is_none());
    assert!(s.grip_snap_pose(b.erase(), GripMotion::MoveAlong, x_line(), [f64::NAN; 3], [0.0; 3]).is_none());
}

#[test]
fn the_freedom_left_decides_which_snaps_are_allowed() {
    let mut s = Session::new();
    let floor = body(&mut s, [0.0, 0.0, -0.1]);
    s.set_component(Occurrence::Item(floor.erase()), ComponentFlag::Grounded, true).unwrap();
    let b = body(&mut s, [0.3, 0.0, 0.5]);
    let free_x = s.grip_allows(b.erase(), GripMotion::MoveAlong, x_line());
    assert!(free_x, "a free component moves any way");
    // Mated face to face on the floor: it slides and spins on the floor only.
    let on = |item: Id<Body>, z: f64, nz: f64| Selection {
        occurrence: Occurrence::Item(item.erase()),
        geometry: Geometry::Plane { point: [0.0, 0.0, z], normal: [0.0, 0.0, nz] },
    };
    s.add_relationship(Relationship::new(
        "Mate",
        RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
        on(b, -0.1, -1.0),
        on(floor, 0.1, 1.0),
    ))
    .unwrap();
    let z_line = GripLine { point: [0.3, 0.0, 0.0], direction: [0.0, 0.0, 1.0] };
    assert!(s.grip_allows(b.erase(), GripMotion::MoveAlong, x_line()));
    assert!(!s.grip_allows(b.erase(), GripMotion::MoveAlong, z_line));
    assert!(s.grip_allows(b.erase(), GripMotion::RotateAbout, z_line));
    assert!(!s.grip_allows(b.erase(), GripMotion::RotateAbout, x_line()));
    // Grounded: nothing.
    assert!(!s.grip_allows(floor.erase(), GripMotion::MoveAlong, x_line()));
}

#[test]
fn commit_and_ground_is_one_undo_step() {
    let mut s = Session::new();
    // The first component placed is grounded (as in Inventor); the second is free.
    let _first = body(&mut s, [0.0, 1.0, 0.0]);
    let b = body(&mut s, [0.0; 3]);
    assert!(!s.doc().component_flag(Occurrence::Item(b.erase()), ComponentFlag::Grounded));
    let pose =
        s.grip_snap_pose(b.erase(), GripMotion::MoveAlong, x_line(), [0.0; 3], [0.4, 0.0, 0.0]).unwrap();
    let steps = s.history().undo_depth();
    s.grip_snap_commit(&[(b.erase(), pose)], true).unwrap();
    assert_eq!(s.history().undo_depth(), steps + 1);
    assert!((origin(&s, b).translation.x - 0.4).abs() < 1e-9);
    assert!(s.doc().component_flag(Occurrence::Item(b.erase()), ComponentFlag::Grounded));
    s.undo();
    assert!(origin(&s, b).translation.x.abs() < 1e-9);
    assert!(!s.doc().component_flag(Occurrence::Item(b.erase()), ComponentFlag::Grounded));
    // Done (without grounding) only moves.
    s.grip_snap_commit(&[(b.erase(), pose)], false).unwrap();
    assert!(!s.doc().component_flag(Occurrence::Item(b.erase()), ComponentFlag::Grounded));
    assert!(s.grip_snap_commit(&[], false).is_err());
}
