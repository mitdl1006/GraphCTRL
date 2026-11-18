//! Joint Limits (Inventor 2026 help, To Define and Manage Joint Relationships,
//! GUID-21DC3336-5C51-42C1-90FB-4299CD66E0C6): "a Rotational joint supports Angular limits, but Linear
//! limits are not available. Limits are not available for Rigid joints. Set the necessary Start, Current,
//! and End values."

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "tests")]

use gc_core::{AnyId, Id};
use gc_doc::{
    Body, EditError, Geometry, JointRange, Occurrence, Placement, RelationKind, Relationship, Selection,
    Solid,
};
use gc_session::Session;

fn sel(item: AnyId, geometry: Geometry) -> Selection {
    Selection { occurrence: Occurrence::Item(item), geometry }
}

fn frame(z: f64, nz: f64) -> Geometry {
    Geometry::Frame { origin: [0.0, 0.0, z], z: [0.0, 0.0, nz], x: [1.0, 0.0, 0.0] }
}

/// A grounded table and a block on a joint of `kind` on the table's top.
fn jointed(kind: RelationKind) -> (Session, Id<Body>, Id<gc_doc::Relationship>) {
    let mut s = Session::new();
    let table = s
        .add_body(Solid::Box { size: [2.0, 1.0, 1.0] }, Placement { xyz: [0.0, 0.0, 0.5], rpy: [0.0; 3] })
        .unwrap();
    let block =
        s.add_body(Solid::Box { size: [0.2; 3] }, Placement { xyz: [0.3, 0.2, 1.6], rpy: [0.0; 3] }).unwrap();
    let id = s
        .add_relationship(Relationship::new(
            "J",
            kind,
            sel(block.erase(), frame(-0.1, -1.0)),
            sel(table.erase(), frame(0.5, 1.0)),
        ))
        .unwrap();
    (s, block, id)
}

fn range(start: f64, end: f64, current: Option<f64>) -> Option<JointRange> {
    Some(JointRange { start, end, current })
}

#[test]
fn an_angular_limit_holds_a_hinge_at_current_or_keeps_it_within_range() {
    let (mut s, block, hinge) = jointed(RelationKind::Rotational { flip: false });
    assert_eq!(s.component_dof(block.erase()).unwrap().total, 1, "a hinge turns");
    // Current holds it: no freedom left, standing at 0.3 rad.
    s.set_joint_limits(hinge, range(-0.5, 0.5, Some(0.3)), None).unwrap();
    let (angle, _) = s.joint_position(hinge).unwrap();
    assert!((angle - 0.3).abs() < 1e-6, "{angle}");
    assert_eq!(s.component_dof(block.erase()).unwrap().total, 0);
    assert_eq!(s.joint_limits(hinge), (range(-0.5, 0.5, Some(0.3)), None));
    // Without Current it turns freely within the range again (and stays where it is).
    s.set_joint_limits(hinge, range(-0.5, 0.5, None), None).unwrap();
    assert_eq!(s.component_dof(block.erase()).unwrap().total, 1);
    assert!((s.joint_position(hinge).unwrap().0 - 0.3).abs() < 1e-6);
    // Drive stays within the limits.
    s.drive_relationship(hinge, 2.0).unwrap();
    let yaw = s.driven_placement(block.erase()).unwrap().rpy[2];
    assert!((yaw.abs() - 0.5).abs() < 1e-6, "driven to the end of the range: {yaw}");
    s.drive_relationship(hinge, -0.2).unwrap();
    let yaw = s.driven_placement(block.erase()).unwrap().rpy[2];
    assert!((yaw.abs() - 0.2).abs() < 1e-6, "{yaw}");
    s.end_drive_relationship();
    // Clearing both is one undo step and leaves no state.
    s.set_joint_limits(hinge, None, None).unwrap();
    assert_eq!(s.joint_limits(hinge), (None, None));
    assert!(s.undo());
    assert_eq!(s.joint_limits(hinge), (range(-0.5, 0.5, None), None));
}

#[test]
fn a_linear_limit_positions_a_slider() {
    let (mut s, block, slider) = jointed(RelationKind::Slider { flip: false });
    s.set_joint_limits(slider, None, range(0.0, 0.4, Some(0.25))).unwrap();
    let (_, slide) = s.joint_position(slider).unwrap();
    assert!((slide - 0.25).abs() < 1e-6, "{slide}");
    assert_eq!(s.component_dof(block.erase()).unwrap().total, 0);
    // Drive moves a slider within its range.
    s.drive_relationship(slider, 0.1).unwrap();
    let z = s.driven_placement(block.erase()).unwrap().xyz[2];
    assert!((z - (1.0 + 0.1 + 0.1)).abs() < 1e-6, "top 1.0 + slide 0.1 + half block 0.1: {z}");
    s.end_drive_relationship();
}

#[test]
fn limits_are_refused_where_inventor_has_none_and_for_bad_ranges() {
    let (mut s, _, hinge) = jointed(RelationKind::Rotational { flip: false });
    let before = s.doc().clone();
    let refused = |s: &mut Session, angular, linear| {
        assert_eq!(
            s.set_joint_limits(hinge, angular, linear),
            Err(EditError::BadValue),
            "{angular:?} {linear:?}"
        );
        assert!(s.doc().same_content(&before));
    };
    // Rotational: no linear limits.
    refused(&mut s, None, range(0.0, 1.0, None));
    // Start past End, Current outside, not finite.
    refused(&mut s, range(1.0, 0.0, None), None);
    refused(&mut s, range(0.0, 1.0, Some(2.0)), None);
    refused(&mut s, range(f64::NAN, 1.0, None), None);
    refused(&mut s, range(0.0, f64::INFINITY, None), None);
    // Rigid: none at all.
    let (mut r, _, rigid) = jointed(RelationKind::Rigid { flip: false });
    assert_eq!(r.set_joint_limits(rigid, range(0.0, 1.0, None), None), Err(EditError::BadValue));
}

#[test]
fn limits_survive_lock_save_and_load_and_go_with_the_joint() {
    let (mut s, _, hinge) = jointed(RelationKind::Cylindrical { flip: false });
    s.set_joint_limits(hinge, range(-1.0, 1.0, None), range(-0.2, 0.2, Some(0.0))).unwrap();
    // Protect does not touch them.
    s.set_joint_protect(hinge, true).unwrap();
    assert_eq!(s.joint_limits(hinge).1, range(-0.2, 0.2, Some(0.0)));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cell.gcp");
    s.save_as(&path).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert_eq!(t.joint_limits(hinge), (range(-1.0, 1.0, None), range(-0.2, 0.2, Some(0.0))));
    s.delete_relationship(hinge).unwrap();
    assert_eq!(s.joint_limits(hinge), (None, None));
}
