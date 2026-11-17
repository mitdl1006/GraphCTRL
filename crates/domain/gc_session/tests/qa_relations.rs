//! QA acceptance and edge cases for assembly constraints (parity P1-9: Mate, Flush, Insert) and the 3-point
//! angle of Measure. Inventor 2026 help:
//! - Mate/Flush: <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-0F500089-4F69-41C5-AFE8-B58AC9E8E15E.htm>
//! - Insert: <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-97DD5557-FAEE-46FA-845D-11123D725BA9.htm>
//! - Constrain reference: <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-EBD2C784-0E71-452A-B5F2-22738D22D45A.htm>
//! - Measure: <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-1805AB8F-F782-4B24-A332-B4F2E10A3D00.htm>

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::disallowed_methods,
    reason = "test code"
)]

use std::path::Path;

use gc_core::Id;
use gc_doc::{
    AxisSense, Body, Geometry, Occurrence, Placement, RelationKind, Relationship, Robot, Selection, Solid,
};
use gc_math::DVec3;
use gc_session::{Session, angle_at, library};

fn placed(xyz: [f64; 3]) -> Placement {
    Placement { xyz, rpy: [0.0; 3] }
}

fn on_body(body: Id<Body>, geometry: Geometry) -> Selection {
    Selection { occurrence: Occurrence::Item(body.erase()), geometry }
}

fn on_robot(robot: Id<Robot>, geometry: Geometry) -> Selection {
    Selection { occurrence: Occurrence::Item(robot.erase()), geometry }
}

fn top(h: f64) -> Geometry {
    Geometry::Plane { point: [0.0, 0.0, h / 2.0], normal: [0.0, 0.0, 1.0] }
}

fn bottom(h: f64) -> Geometry {
    Geometry::Plane { point: [0.0, 0.0, -h / 2.0], normal: [0.0, 0.0, -1.0] }
}

/// The robot base's underside in the robot's own frame: the plane z = 0 looking down.
fn base_bottom() -> Geometry {
    Geometry::Plane { point: [0.0; 3], normal: [0.0, 0.0, -1.0] }
}

fn mate(first: Selection, second: Selection, offset: f64) -> Relationship {
    Relationship {
        limits: None,
        name: "Mate".into(),
        kind: RelationKind::Mate { offset, sense: AxisSense::Opposed },
        first,
        second,
    }
}

fn robot_cell() -> (Session, Id<Body>, Id<Robot>) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../robots");
    let packages = library::scan(&dir);
    let mut s = Session::new();
    s.set_robot_library(Some(dir));
    // The box is placed first, so it is the grounded component; the robot stands somewhere else.
    let pedestal = s.add_body(Solid::Box { size: [0.4, 0.4, 0.5] }, placed([1.0, 0.5, 0.25])).unwrap();
    let ur5e = packages.iter().find(|p| p.info.name == "UR5e").unwrap();
    let robot = s.add_library_robot(ur5e).unwrap();
    (s, pedestal, robot)
}

fn base_of(s: &Session, robot: Id<Robot>) -> gc_math::DPose3 {
    let p = s.doc().get(robot).unwrap().placement;
    gc_math::pose_from_xyz_rpy(p.xyz, p.rpy)
}

/// The acceptance scenario of P1-9: the top of a box and the bottom of a robot base are mated; the robot
/// moves onto the box (the grounded box stays), upright; a gap is an offset; Undo puts everything back.
#[test]
fn mating_a_robot_base_to_a_box_top_moves_the_robot() {
    let (mut s, pedestal, robot) = robot_cell();
    let box_before = s.doc().get(pedestal).unwrap().placement;
    let robot_before = s.doc().get(robot).unwrap().placement;
    let depth = s.history().undo_depth();
    let id =
        s.add_relationship(mate(on_robot(robot, base_bottom()), on_body(pedestal, top(0.5)), 0.0)).unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1, "one undo step");
    assert_eq!(s.doc().get(pedestal).unwrap().placement, box_before, "the grounded box does not move");
    let base = base_of(&s, robot);
    assert!((base.translation.z - 0.5).abs() < 1e-7, "base bottom on the box top: {:?}", base.translation);
    assert!((base.rotation * DVec3::Z - DVec3::Z).length() < 1e-7, "upright");
    assert!(s.robot_model(robot).is_some());
    assert_eq!(s.relationships().len(), 1);

    // A gap of 20 mm: the robot rises by exactly that much.
    s.set_relationship(id, mate(on_robot(robot, base_bottom()), on_body(pedestal, top(0.5)), 0.02)).unwrap();
    assert!((base_of(&s, robot).translation.z - 0.52).abs() < 1e-7);
    // The box moves: the robot follows (Inventor: the relationship holds).
    let mut moved = s.doc().get(pedestal).unwrap().clone();
    moved.placement.xyz = [2.0, 0.0, 0.25];
    s.execute(&gc_doc::Command::Cell(gc_doc::CellOp::SetBody { id: pedestal, body: moved })).unwrap();
    let base = base_of(&s, robot);
    assert!((base.translation.z - 0.52).abs() < 1e-7);
    // Undo everything: both back where they started and no relationship is left.
    while s.history().undo_depth() > depth {
        assert!(s.undo());
    }
    assert!(s.relationships().is_empty());
    assert_eq!(s.doc().get(robot).unwrap().placement, robot_before);
    assert_eq!(s.doc().get(pedestal).unwrap().placement, box_before);
}

/// Deleting a relationship leaves the components where they are (no snap back); Undo brings it back.
#[test]
fn deleting_a_relationship_leaves_the_components_in_place() {
    let (mut s, pedestal, robot) = robot_cell();
    let id =
        s.add_relationship(mate(on_robot(robot, base_bottom()), on_body(pedestal, top(0.5)), 0.0)).unwrap();
    let placed_robot = s.doc().get(robot).unwrap().placement;
    s.delete_relationship(id).unwrap();
    assert!(s.relationships().is_empty());
    assert_eq!(s.doc().get(robot).unwrap().placement, placed_robot, "the robot stays where the mate put it");
    // Now the robot can be moved freely.
    s.execute(&gc_doc::Command::Cell(gc_doc::CellOp::PlaceRobot {
        id: robot,
        placement: placed([3.0, 3.0, 0.0]),
    }))
    .unwrap();
    assert_eq!(s.doc().get(robot).unwrap().placement.xyz, [3.0, 3.0, 0.0]);
    assert!(s.undo() && s.undo());
    assert_eq!(s.relationships().len(), 1, "undo brought the relationship back");
}

/// A constraint that contradicts the others, or cannot be built from what was picked, is refused with the
/// document untouched: conflicting offsets, the same pair twice, wrong geometry kinds, a component with
/// itself, numbers that are not numbers.
#[test]
fn contradicting_and_malformed_constraints_are_refused_and_change_nothing() {
    let (mut s, pedestal, robot) = robot_cell();
    s.add_relationship(mate(on_robot(robot, base_bottom()), on_body(pedestal, top(0.5)), 0.0)).unwrap();
    let before = s.doc().clone();
    let depth = s.history().undo_depth();

    let first = || on_robot(robot, base_bottom());
    let second = || on_body(pedestal, top(0.5));
    let nan_plane = Geometry::Plane { point: [f64::NAN, 0.0, 0.0], normal: [0.0, 0.0, 1.0] };
    let flat_normal = Geometry::Plane { point: [0.0; 3], normal: [0.0; 3] };
    let inf_axis = Geometry::Axis { point: [0.0; 3], direction: [f64::INFINITY, 0.0, 0.0] };
    let cases: Vec<(&str, Relationship)> = vec![
        ("conflicting offset", mate(first(), second(), 0.1)),
        ("NaN offset", mate(first(), second(), f64::NAN)),
        ("infinite offset", mate(first(), second(), f64::INFINITY)),
        ("NaN plane", mate(on_robot(robot, nan_plane), second(), 0.0)),
        ("zero normal", mate(on_robot(robot, flat_normal), second(), 0.0)),
        ("infinite axis", mate(on_robot(robot, inf_axis), second(), 0.0)),
        ("a component with itself", mate(on_body(pedestal, top(0.5)), on_body(pedestal, bottom(0.5)), 0.0)),
        (
            "Flush with a point",
            Relationship {
                kind: RelationKind::Flush { offset: 0.0 },
                first: on_robot(robot, Geometry::Point([0.0; 3])),
                ..mate(first(), second(), 0.0)
            },
        ),
        (
            "Insert with planes",
            Relationship {
                kind: RelationKind::Insert { offset: 0.0, aligned: false },
                ..mate(first(), second(), 0.0)
            },
        ),
    ];
    for (what, relationship) in cases {
        let result = s.add_relationship(relationship);
        assert!(result.is_err(), "{what} was accepted");
        assert!(s.doc().same_content(&before), "{what} changed the document");
        assert_eq!(s.history().undo_depth(), depth, "{what}");
    }
    // Editing an existing relationship into nonsense is refused too, and the original stands.
    let id = s.relationships()[0].0;
    assert!(s.set_relationship(id, mate(first(), second(), f64::NAN)).is_err());
    assert!(s.doc().same_content(&before));
}

/// A repeated constraint (the same pair, the same numbers) is either refused as redundant or accepted
/// harmlessly; either way nothing moves and Undo takes it back exactly.
#[test]
fn a_repeated_constraint_is_harmless() {
    let (mut s, pedestal, robot) = robot_cell();
    s.add_relationship(mate(on_robot(robot, base_bottom()), on_body(pedestal, top(0.5)), 0.0)).unwrap();
    let placed_robot = s.doc().get(robot).unwrap().placement;
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    if s.add_relationship(mate(on_robot(robot, base_bottom()), on_body(pedestal, top(0.5)), 0.0)).is_ok() {
        assert_eq!(s.doc().get(robot).unwrap().placement.xyz, placed_robot.xyz, "nothing moved");
        assert!(s.undo());
    }
    assert!(s.doc().same_content(&before));
    assert_eq!(s.history().undo_depth(), depth);
}

/// A chain: C grounded, B mated on C, A mated on B. Moving C moves both; a consistent triangle is accepted,
/// a triangle that cannot close is refused.
#[test]
fn chains_follow_the_grounded_component_and_impossible_loops_are_refused() {
    let mut s = Session::new();
    let cube = || Solid::Box { size: [0.2; 3] };
    let c = s.add_body(Solid::Box { size: [1.0, 1.0, 0.2] }, placed([0.0, 0.0, 0.1])).unwrap(); // grounded
    let b = s.add_body(cube(), placed([0.0, 0.0, 1.0])).unwrap();
    let a = s.add_body(cube(), placed([0.0, 0.0, 2.0])).unwrap();
    s.add_relationship(mate(on_body(b, bottom(0.2)), on_body(c, top(0.2)), 0.0)).unwrap();
    s.add_relationship(mate(on_body(a, bottom(0.2)), on_body(b, top(0.2)), 0.0)).unwrap();
    let z = |id: Id<Body>| s.doc().get(id).unwrap().placement.xyz[2];
    assert!((z(b) - 0.3).abs() < 1e-7 && (z(a) - 0.5).abs() < 1e-7, "{} {}", z(b), z(a));
    let mut up = s.doc().get(c).unwrap().clone();
    up.placement.xyz = [0.0, 0.0, 1.1];
    s.execute(&gc_doc::Command::Cell(gc_doc::CellOp::SetBody { id: c, body: up })).unwrap();
    let z = |id: Id<Body>| s.doc().get(id).unwrap().placement.xyz[2];
    assert!(
        (z(b) - 1.3).abs() < 1e-7 && (z(a) - 1.5).abs() < 1e-7,
        "the whole stack followed: {} {}",
        z(b),
        z(a)
    );
    // A third mate that asks A to sit on C directly (0.2 m lower than the stack) cannot be met.
    let before = s.doc().clone();
    let result = s.add_relationship(mate(on_body(a, bottom(0.2)), on_body(c, top(0.2)), 0.0));
    assert!(result.is_err(), "an impossible loop was accepted");
    assert!(s.doc().same_content(&before));
}

/// Negative offsets (interpenetration) are allowed by Inventor; huge ones must not break the solver.
#[test]
fn negative_and_huge_offsets_are_solved_or_refused_cleanly() {
    let (mut s, pedestal, robot) = robot_cell();
    let id =
        s.add_relationship(mate(on_robot(robot, base_bottom()), on_body(pedestal, top(0.5)), -0.1)).unwrap();
    assert!((base_of(&s, robot).translation.z - 0.4).abs() < 1e-7, "sunk 100 mm into the box");
    for offset in [1e6, -1e6, 1e12] {
        let relationship = mate(on_robot(robot, base_bottom()), on_body(pedestal, top(0.5)), offset);
        if s.set_relationship(id, relationship).is_ok() {
            let z = base_of(&s, robot).translation.z;
            assert!(
                z.is_finite() && (z - (0.5 + offset)).abs() < 1e-3 * offset.abs().max(1.0),
                "{offset}: {z}"
            );
        }
    }
}

/// Predict Offset: planes that already face each other, are level, are tilted, or are not planes.
#[test]
fn predicting_a_relationship_reads_the_current_pose() {
    let (mut s, pedestal, robot) = robot_cell();
    // The robot at the origin, the box top at z = 0.5 facing up: the base bottom (z = 0, facing down) is
    // 0.5 m *behind* the box top, so the faces face away from each other: a Mate at offset -0.5.
    let predict =
        |s: &Session| s.predict_relationship(&on_robot(robot, base_bottom()), &on_body(pedestal, top(0.5)));
    let Some(RelationKind::Mate { offset, .. }) = predict(&s) else { panic!("{:?}", predict(&s)) };
    assert!((offset + 0.5).abs() < 1e-9, "{offset}");
    // Lifted 0.3 m above the box top the faces look at each other across a gap: offset +0.3.
    s.execute(&gc_doc::Command::Cell(gc_doc::CellOp::PlaceRobot {
        id: robot,
        placement: placed([0.0, 0.0, 0.8]),
    }))
    .unwrap();
    let Some(RelationKind::Mate { offset, .. }) = predict(&s) else { panic!("{:?}", predict(&s)) };
    assert!((offset - 0.3).abs() < 1e-9, "{offset}");
    // Two points have no orientation to predict: a Mate of coincident points or nothing, never a panic.
    std::hint::black_box(s.predict_relationship(
        &on_robot(robot, Geometry::Point([0.0; 3])),
        &on_body(pedestal, Geometry::Point([0.0; 3])),
    ));
}

/// A selection that is not a number predicts nothing (the offer would carry a NaN offset into the dialog).
#[test]
#[ignore = "BUG: Session::predict_relationship returns Some(..) for a non-finite selection; see docs/handoff/qa.md"]
fn predicting_from_a_non_finite_selection_offers_nothing() {
    let (s, pedestal, robot) = robot_cell();
    let nan = Geometry::Plane { point: [f64::NAN; 3], normal: [0.0, 0.0, 1.0] };
    let predicted = s.predict_relationship(&on_robot(robot, nan), &on_body(pedestal, top(0.5)));
    assert!(predicted.is_none(), "{predicted:?}");
}

/// Relationships survive saving and reopening, and the constrained pose is the one in the file.
#[test]
fn relationships_are_saved_and_reopened() {
    let (mut s, pedestal, robot) = robot_cell();
    s.add_relationship(mate(on_robot(robot, base_bottom()), on_body(pedestal, top(0.5)), 0.0)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("rel.gctrl");
    s.save_as(&file).unwrap();
    let mut reopened = Session::new();
    reopened.set_robot_library(s.robot_library().map(Path::to_path_buf));
    reopened.open(&file).unwrap();
    assert_eq!(reopened.relationships().len(), 1);
    assert!(reopened.doc().same_content(s.doc()));
    // The reopened relationship still holds when the box moves.
    let mut moved = reopened.doc().get(pedestal).unwrap().clone();
    moved.placement.xyz = [0.0, 0.0, 0.25];
    reopened.execute(&gc_doc::Command::Cell(gc_doc::CellOp::SetBody { id: pedestal, body: moved })).unwrap();
    assert!((base_of(&reopened, robot).translation.z - 0.5).abs() < 1e-7);
}

// ---- Measure: the angle at a vertex (Shift adds a third point) ----

/// Right angle, straight line (180 degrees), a fold (0 degrees), and the symmetry of the two arms.
#[test]
fn three_point_angles_are_right_at_the_edges() {
    let v = [1.0, 2.0, 3.0];
    let angle = |a: [f64; 3], c: [f64; 3]| angle_at(a, v, c).unwrap();
    let pi = std::f64::consts::PI;
    assert!((angle([2.0, 2.0, 3.0], [1.0, 3.0, 3.0]) - pi / 2.0).abs() < 1e-12);
    assert!((angle([2.0, 2.0, 3.0], [0.0, 2.0, 3.0]) - pi).abs() < 1e-12, "a straight line is 180 degrees");
    assert!(angle([2.0, 2.0, 3.0], [5.0, 2.0, 3.0]).abs() < 1e-12, "folded back on itself is 0");
    assert!((angle([2.0, 2.0, 3.0], [1.5, 2.0 + 0.5 * 3.0_f64.sqrt(), 3.0]) - pi / 3.0).abs() < 1e-12);
    // The arms can be swapped, and the lengths of the arms do not matter.
    let (a, c) = ([4.0, 0.0, 3.0], [1.0, 9.0, 5.0]);
    assert_eq!(angle_at(a, v, c), angle_at(c, v, a));
    assert!((angle_at([1.0 + 1e-9, 2.0, 3.0], v, [1.0, 2.0 + 1e6, 3.0]).unwrap() - pi / 2.0).abs() < 1e-6);
    // Always within 0..=pi.
    for k in 0..50 {
        let t = f64::from(k) * 0.37;
        let r = angle_at([t.cos() + 1.0, t.sin() + 2.0, 3.0 + t], v, [2.0, 2.0 - t, 3.5]).unwrap();
        assert!((0.0..=pi).contains(&r), "{r}");
    }
}

/// A point on the vertex (or nearly), or a value that is not a number: no angle, never NaN or a panic.
#[test]
fn three_point_angles_without_a_direction_are_none() {
    let v = [0.0, 0.0, 0.0];
    let p = [1.0, 0.0, 0.0];
    assert_eq!(angle_at(v, v, p), None, "first point on the vertex");
    assert_eq!(angle_at(p, v, v), None, "third point on the vertex");
    assert_eq!(angle_at(v, v, v), None, "all three the same point");
    assert_eq!(angle_at([1e-13, 0.0, 0.0], v, p), None, "within a picometre of the vertex");
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(angle_at([bad, 0.0, 0.0], v, p), None);
        assert_eq!(angle_at(p, [0.0, bad, 0.0], p), None);
        assert_eq!(angle_at(p, v, [0.0, 0.0, bad]), None);
    }
    // A hair above the limit is still an angle.
    assert!(angle_at([2e-12, 0.0, 0.0], v, [0.0, 1.0, 0.0]).is_some());
}
