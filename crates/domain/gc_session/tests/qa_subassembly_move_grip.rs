//! QA boundary tests for moving a subassembly as one unit (`subassembly_moved`, `move_subassembly`) and for
//! Grip Snap on points and planes (`grip_snap_pose_on`, `grip_snap_allowed`). Inventor 2026 help, under
//! <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/>: subassemblies
//! GUID-FCEE3F0E-AAE2-4619-8645-2588FEECD353 ("treated as a single unit"), Grip Snap move options
//! GUID-C030C32C-7E1E-4F37-BBDF-120412898D00, Grip Snap GUID-E1CA109A-D5A1-4131-AC3A-AE217716DD8C.
//! Every refused edit leaves the document and the history untouched; every accepted one is one undo step that
//! restores exactly.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::disallowed_methods,
    reason = "test code"
)]

use gc_core::Id;
use gc_doc::{
    AxisSense, Body, ComponentFlag, Geometry, GripLine, GripMotion, GripReference, Occurrence, Placement,
    RelationKind, Relationship, Selection, Solid, SubassemblyId, SubassemblyOp, TreeKey,
};
use gc_math::{DPose3, DQuat, DVec3};
use gc_session::{CellItem, Session};

fn body(s: &mut Session, x: f64) -> Id<Body> {
    s.add_body(Solid::Box { size: [0.1; 3] }, Placement { xyz: [x, 0.0, 0.0], rpy: [0.0; 3] }).unwrap()
}

fn key(b: Id<Body>) -> TreeKey {
    TreeKey::Component(Occurrence::Item(b.erase()))
}

fn occ(b: Id<Body>) -> Occurrence {
    Occurrence::Item(b.erase())
}

fn demote(s: &mut Session, members: Vec<TreeKey>, name: &str) -> SubassemblyId {
    match s.edit_subassemblies(SubassemblyOp::Demote { members, name: name.into() }).unwrap()[..] {
        [gc_doc::Created::Subassembly(id)] => id,
        ref other => panic!("{other:?}"),
    }
}

fn origin(s: &Session, b: Id<Body>) -> DPose3 {
    s.doc().component_origin(occ(b)).unwrap().pose
}

fn lift(z: f64) -> DPose3 {
    DPose3::from_parts(DVec3::new(0.0, 0.0, z), DQuat::IDENTITY)
}

fn refused(s: &mut Session, what: &str, id: SubassemblyId, delta: DPose3) {
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    let result = s.move_subassembly(id, delta);
    assert!(result.is_err(), "{what} was accepted");
    assert!(s.doc().same_content(&before), "{what} changed the document");
    assert_eq!(s.history().undo_depth(), depth, "{what}");
}

// ---- Moving a subassembly ----

#[test]
fn bad_deltas_and_missing_or_empty_subassemblies_are_refused() {
    let mut s = Session::new();
    let _ground = body(&mut s, 5.0);
    let (a, b) = (body(&mut s, 0.0), body(&mut s, 0.3));
    let one = demote(&mut s, vec![key(a), key(b)], "One");
    let nan = DPose3::from_parts(DVec3::new(f64::NAN, 0.0, 0.0), DQuat::IDENTITY);
    let inf = DPose3::from_parts(DVec3::new(0.0, f64::INFINITY, 0.0), DQuat::IDENTITY);
    let bad_turn = DPose3::from_parts(DVec3::ZERO, DQuat::from_xyzw(f64::NAN, 0.0, 0.0, 1.0));
    for (what, delta) in [("NaN move", nan), ("infinite move", inf), ("NaN turn", bad_turn)] {
        refused(&mut s, what, one, delta);
    }
    refused(&mut s, "ghost subassembly", SubassemblyId(99), lift(0.1));
    // Everything in it deleted: nothing to move.
    let (c, d) = (body(&mut s, 1.0), body(&mut s, 1.3));
    let two = demote(&mut s, vec![key(c), key(d)], "Two");
    s.delete_item(CellItem::Body(c)).unwrap();
    s.delete_item(CellItem::Body(d)).unwrap();
    refused(&mut s, "empty subassembly", two, lift(0.1));
    // A zero quaternion (not a rotation): refused or finite.
    let before = s.doc().clone();
    let zero = DPose3::from_parts(DVec3::ZERO, DQuat::from_xyzw(0.0, 0.0, 0.0, 0.0));
    match s.move_subassembly(one, zero) {
        Err(_) => assert!(s.doc().same_content(&before)),
        Ok(()) => {
            let p = origin(&s, a);
            assert!(
                p.translation.is_finite() && p.rotation.is_finite(),
                "a zero rotation made a non-finite pose"
            );
            assert!(s.undo() && s.doc().same_content(&before));
        }
    }
}

/// Nested levels move with the unit; the identity move leaves things as they are (accepted as a step or
/// refused, never a drift); one undo step restores everything exactly; redo repeats it; rotated moves keep
/// every distance in the unit.
#[test]
fn a_nested_subassembly_moves_rigidly_in_one_undo_step() {
    let mut s = Session::new();
    let _ground = body(&mut s, 5.0);
    let (a, b, c, d) = (body(&mut s, 0.0), body(&mut s, 0.3), body(&mut s, 0.6), body(&mut s, 0.9));
    let inner = demote(&mut s, vec![key(a), key(b)], "Inner");
    let other = demote(&mut s, vec![key(c)], "Sibling");
    let outer = demote(&mut s, vec![TreeKey::Subassembly(inner), TreeKey::Subassembly(other)], "Outer");
    let (before, depth) = (s.doc().clone(), s.history().undo_depth());
    let poses = [a, b, c, d].map(|x| origin(&s, x));
    let delta = DPose3::from_parts(
        DVec3::new(0.2, -0.1, 0.5),
        DQuat::from_axis_angle(DVec3::new(1.0, 2.0, 3.0).normalize(), 0.9),
    );
    s.move_subassembly(outer, delta).unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1, "one undo step for the whole unit");
    let moved = [a, b, c, d].map(|x| origin(&s, x));
    for i in 0..3 {
        let want = delta * poses[i];
        assert!((moved[i].translation - want.translation).length() < 1e-9, "member {i}");
        assert!(moved[i].rotation.dot(want.rotation).abs() > 1.0 - 1e-9);
    }
    assert!((moved[3].translation - poses[3].translation).length() < 1e-12, "a component outside moved");
    for (i, j) in [(0, 1), (0, 2), (1, 2)] {
        let was = (poses[i].translation - poses[j].translation).length();
        let is = (moved[i].translation - moved[j].translation).length();
        assert!((was - is).abs() < 1e-9, "the unit changed shape: {i}-{j}");
    }
    assert!(s.undo());
    assert!(s.doc().same_content(&before), "undo is not exact");
    assert!(s.redo());
    assert!((origin(&s, a).translation - moved[0].translation).length() < 1e-12, "redo differs");
    assert!(s.undo());
    // The identity: nothing drifts.
    match s.move_subassembly(outer, DPose3::IDENTITY) {
        Ok(()) => {
            for (x, p) in [a, b, c].into_iter().zip(poses) {
                assert!((origin(&s, x).translation - p.translation).length() < 1e-12);
            }
            assert!(s.undo());
        }
        Err(_) => assert!(s.doc().same_content(&before)),
    }
    assert!(s.doc().same_content(&before));
}

/// One grounded member (even deep inside) holds the whole unit; ground nothing and it moves; a member that was
/// deleted no longer holds it.
#[test]
fn a_grounded_member_anywhere_in_the_unit_refuses_the_move() {
    let mut s = Session::new();
    let _ground = body(&mut s, 5.0);
    let (a, b) = (body(&mut s, 0.0), body(&mut s, 0.3));
    let inner = demote(&mut s, vec![key(a)], "Inner");
    let outer = demote(&mut s, vec![TreeKey::Subassembly(inner), key(b)], "Outer");
    s.set_component(occ(a), ComponentFlag::Grounded, true).unwrap();
    assert!(s.subassembly_moved(outer, lift(0.1)).is_none());
    assert!(s.subassembly_moved(inner, lift(0.1)).is_none());
    refused(&mut s, "grounded deep member, outer", outer, lift(0.1));
    refused(&mut s, "grounded member, inner", inner, lift(0.1));
    // The unit that does not hold it moves.
    s.set_component(occ(a), ComponentFlag::Grounded, false).unwrap();
    s.move_subassembly(outer, lift(0.1)).unwrap();
    assert!((origin(&s, a).translation.z - 0.1).abs() < 1e-9);
    // Ground only B: the outer unit is held, but a unit that does not contain B is not.
    s.set_component(occ(b), ComponentFlag::Grounded, true).unwrap();
    refused(&mut s, "grounded sibling", outer, lift(0.1));
    s.move_subassembly(inner, lift(0.1)).unwrap();
    assert!((origin(&s, a).translation.z - 0.2).abs() < 1e-9);
}

/// What is tied to a member follows; a member tied to something that holds still (a grounded component outside
/// the unit) either keeps the relationship satisfied or the move is refused whole — never a torn cell.
#[test]
fn components_tied_to_the_unit_follow_and_relationships_stay_satisfied() {
    let mut s = Session::new();
    let floor = body(&mut s, 5.0);
    let (a, b, free) = (body(&mut s, 0.0), body(&mut s, 0.3), body(&mut s, 2.0));
    let one = demote(&mut s, vec![key(a), key(b)], "One");
    let frame = |item: Id<Body>, z: f64, nz: f64| Selection {
        occurrence: occ(item),
        geometry: Geometry::Frame { origin: [0.0, 0.0, z], z: [0.0, 0.0, nz], x: [1.0, 0.0, 0.0] },
    };
    // `free` (outside the unit) is rigidly joined to A: it follows when the unit moves.
    s.add_relationship(Relationship::new(
        "free to a",
        RelationKind::Rigid { flip: false },
        Selection {
            occurrence: occ(free),
            geometry: Geometry::Frame { origin: [0.0; 3], z: [0.0, 0.0, 1.0], x: [1.0, 0.0, 0.0] },
        },
        Selection {
            occurrence: occ(a),
            geometry: Geometry::Frame { origin: [0.0; 3], z: [0.0, 0.0, 1.0], x: [1.0, 0.0, 0.0] },
        },
    ))
    .unwrap();
    let before = s.doc().clone();
    let (fa, ff) = (origin(&s, a), origin(&s, free));
    s.move_subassembly(one, lift(0.4)).unwrap();
    assert!((origin(&s, a).translation.z - (fa.translation.z + 0.4)).abs() < 1e-6);
    assert!(
        (origin(&s, free).translation.z - (ff.translation.z + 0.4)).abs() < 1e-6,
        "the tied component did not follow"
    );
    assert!(s.doc().solve_relationships().is_ok());
    assert!(s.undo() && s.doc().same_content(&before));
    // B rigidly joined to the grounded floor: the unit cannot go anywhere the joint does not allow.
    s.add_relationship(Relationship::new(
        "b to floor",
        RelationKind::Rigid { flip: false },
        frame(b, 0.0, 1.0),
        frame(floor, 0.0, 1.0),
    ))
    .unwrap();
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    match s.move_subassembly(one, lift(0.4)) {
        Err(_) => {
            assert!(s.doc().same_content(&before), "a refused move changed the cell");
            assert_eq!(s.history().undo_depth(), depth);
        }
        Ok(()) => {
            assert!(s.doc().solve_relationships().is_ok(), "the move left the cell unsolvable");
            let pb = origin(&s, b).translation;
            let pf = origin(&s, floor).translation;
            assert!((pb - pf).length() < 1e-6, "the joint to the floor was torn: {pb:?} vs {pf:?}");
            assert!(s.undo() && s.doc().same_content(&before));
        }
    }
}

#[test]
fn a_big_subassembly_moves_in_one_undo_step() {
    let mut s = Session::new();
    let _ground = body(&mut s, 50.0);
    let members: Vec<TreeKey> = (0..200).map(|i| key(body(&mut s, f64::from(i) * 0.2))).collect();
    let big = demote(&mut s, members, "Big");
    // The history keeps 200 steps and the build made 201: count steps ever made (kept plus dropped).
    let made = |s: &Session| s.history().undo_depth() as u64 + s.history().trimmed();
    let steps = made(&s);
    let before = s.doc().clone();
    s.move_subassembly(big, lift(0.25)).unwrap();
    assert_eq!(made(&s), steps + 1, "one undo step for 200 components");
    assert!(s.undo() && s.doc().same_content(&before));
}

#[test]
fn a_moved_subassembly_survives_save_and_open() {
    let mut s = Session::new();
    let _ground = body(&mut s, 5.0);
    let (a, b) = (body(&mut s, 0.0), body(&mut s, 0.3));
    let one = demote(&mut s, vec![key(a), key(b)], "One");
    s.move_subassembly(one, lift(0.4)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("moved.gcp");
    s.save_as(&path).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert!(t.doc().same_content(s.doc()));
    assert!((origin(&t, a).translation.z - 0.4).abs() < 1e-9);
    assert_eq!(t.component_tree(), s.component_tree());
}

// ---- Grip Snap on points and planes ----

fn grip_body() -> (Session, Id<Body>) {
    let mut s = Session::new();
    let _floor = body(&mut s, 5.0);
    let b =
        s.add_body(Solid::Box { size: [0.2; 3] }, Placement { xyz: [0.1, 0.2, 0.3], rpy: [0.0; 3] }).unwrap();
    (s, b)
}

fn plane(point: [f64; 3], normal: [f64; 3]) -> GripReference {
    GripReference::Plane { point, normal }
}

/// Only the six pairings of the help work (point: Displace, Rotate; line: along, rotate about; plane: within,
/// along normal); every other pairing of a motion with a reference gives nothing.
#[test]
fn only_matching_motions_and_references_make_a_pose() {
    let (s, b) = grip_body();
    let line = GripLine { point: [0.0; 3], direction: [0.0, 0.0, 1.0] };
    let refs = [GripReference::Point([0.0; 3]), GripReference::Line(line), plane([0.0; 3], [0.0, 0.0, 1.0])];
    let motions = [
        GripMotion::MoveAlong,
        GripMotion::RotateAbout,
        GripMotion::Displace,
        GripMotion::RotateAboutPoint,
        GripMotion::MoveWithinPlane,
        GripMotion::MoveAlongNormal,
    ];
    let works = |m: GripMotion, r: &GripReference| {
        matches!(
            (m, r),
            (GripMotion::Displace | GripMotion::RotateAboutPoint, GripReference::Point(_))
                | (GripMotion::MoveAlong | GripMotion::RotateAbout, GripReference::Line(_))
                | (GripMotion::MoveWithinPlane | GripMotion::MoveAlongNormal, GripReference::Plane { .. })
        )
    };
    for m in motions {
        for r in &refs {
            let pose = s.grip_snap_pose_on(b.erase(), m, *r, [1.0, 0.0, 0.5], [0.0, 1.0, 0.9]);
            assert_eq!(pose.is_some(), works(m, r), "{m:?} on {r:?}: {pose:?}");
        }
    }
}

#[test]
fn plane_references_with_no_normal_or_non_finite_numbers_give_nothing() {
    let (s, b) = grip_body();
    for (what, reference) in [
        ("zero normal", plane([0.0; 3], [0.0; 3])),
        ("tiny normal", plane([0.0; 3], [1e-13, 0.0, 0.0])),
        ("NaN normal", plane([0.0; 3], [f64::NAN, 0.0, 1.0])),
        ("infinite normal", plane([0.0; 3], [0.0, f64::INFINITY, 1.0])),
        ("NaN point", plane([f64::NAN, 0.0, 0.0], [0.0, 0.0, 1.0])),
    ] {
        for motion in [GripMotion::MoveWithinPlane, GripMotion::MoveAlongNormal] {
            assert!(
                s.grip_snap_pose_on(b.erase(), motion, reference, [0.0; 3], [1.0, 1.0, 1.0]).is_none(),
                "{what} {motion:?} made a pose"
            );
            assert!(
                !s.grip_snap_allowed(b.erase(), motion, reference, [0.0; 3], [1.0, 1.0, 1.0]),
                "{what} {motion:?} is allowed"
            );
        }
    }
    // Non-finite points on a good plane.
    let good = plane([0.0; 3], [0.0, 0.0, 1.0]);
    for bad in [[f64::NAN, 0.0, 0.0], [0.0, f64::INFINITY, 0.0]] {
        assert!(s.grip_snap_pose_on(b.erase(), GripMotion::MoveWithinPlane, good, bad, [0.0; 3]).is_none());
        assert!(s.grip_snap_pose_on(b.erase(), GripMotion::MoveAlongNormal, good, [0.0; 3], bad).is_none());
    }
}

/// Within-plane and along-normal motions split the displacement: together they are the whole of it, neither
/// has any part of the other, the normal's length and sign do not matter, and the rotation is untouched.
#[test]
fn plane_motions_split_the_displacement() {
    let (s, b) = grip_body();
    let origin0 = origin(&s, b);
    let (from, to) = ([0.2, 0.1, 0.0], [0.5, 0.6, 0.7]);
    let d = DVec3::from(to) - DVec3::from(from);
    for normal in [[0.0, 0.0, 1.0], [0.0, 0.0, -3.0], [1.0, 1.0, 1.0], [0.0, 1e-3, 1e3]] {
        let n = DVec3::from(normal).normalize();
        let reference = plane([0.0; 3], normal);
        let within =
            s.grip_snap_pose_on(b.erase(), GripMotion::MoveWithinPlane, reference, from, to).unwrap();
        let along = s.grip_snap_pose_on(b.erase(), GripMotion::MoveAlongNormal, reference, from, to).unwrap();
        let (dw, da) = (within.translation - origin0.translation, along.translation - origin0.translation);
        assert!(dw.dot(n).abs() < 1e-9, "{normal:?}: the within-plane move has a normal part");
        assert!((da - n * da.dot(n)).length() < 1e-9, "{normal:?}: the along-normal move left the normal");
        assert!(((dw + da) - d).length() < 1e-9, "{normal:?}: the two do not add up to the displacement");
        assert!(
            within.rotation.dot(origin0.rotation).abs() > 1.0 - 1e-12
                && along.rotation.dot(origin0.rotation).abs() > 1.0 - 1e-12
        );
    }
}

#[test]
fn point_references_displace_and_turn_with_edge_cases() {
    let (s, b) = grip_body();
    let origin0 = origin(&s, b);
    let at = GripReference::Point([0.0; 3]);
    // Displace by the whole difference; the same two points change nothing; non-finite reference: nothing.
    let moved = s.grip_snap_pose_on(b.erase(), GripMotion::Displace, at, [0.0; 3], [0.1, 0.2, 0.3]).unwrap();
    assert!((moved.translation - (origin0.translation + DVec3::new(0.1, 0.2, 0.3))).length() < 1e-12);
    let same = s.grip_snap_pose_on(b.erase(), GripMotion::Displace, at, [0.4; 3], [0.4; 3]).unwrap();
    assert!((same.translation - origin0.translation).length() < 1e-12);
    assert!(
        s.grip_snap_pose_on(
            b.erase(),
            GripMotion::Displace,
            GripReference::Point([f64::NAN, 0.0, 0.0]),
            [0.0; 3],
            [1.0; 3]
        )
        .is_none()
    );
    // A quarter turn about the point: from +X to +Y turns about +Z by 90 degrees.
    let turned = s
        .grip_snap_pose_on(b.erase(), GripMotion::RotateAboutPoint, at, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0])
        .unwrap();
    assert!((turned.rotation * DVec3::X - DVec3::Y).length() < 1e-9, "a quarter turn");
    assert!((turned.rotation.length() - 1.0).abs() < 1e-9);
    // No turn axis: the two points are in line with the point (same, opposite, or on it): nothing.
    for (what, from, to) in [
        ("same point", [1.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
        ("opposite points", [1.0, 0.0, 0.0], [-1.0, 0.0, 0.0]),
        ("from on the point", [0.0; 3], [0.0, 1.0, 0.0]),
        ("to on the point", [1.0, 0.0, 0.0], [0.0; 3]),
        ("same direction, other length", [1.0, 0.0, 0.0], [5.0, 0.0, 0.0]),
    ] {
        assert!(
            s.grip_snap_pose_on(b.erase(), GripMotion::RotateAboutPoint, at, from, to).is_none(),
            "{what} made a turn"
        );
    }
    // A turn and its reverse (the two points swapped) is the identity.
    let there = s
        .grip_snap_pose_on(b.erase(), GripMotion::RotateAboutPoint, at, [1.0, 0.0, 0.0], [0.6, 0.0, 0.8])
        .unwrap();
    let mut t = Session::new();
    let _floor = body(&mut t, 5.0);
    let c = t
        .add_body(
            Solid::Box { size: [0.2; 3] },
            Placement { xyz: there.translation.to_array(), rpy: gc_math::rpy_from_quat(there.rotation) },
        )
        .unwrap();
    let back = t
        .grip_snap_pose_on(c.erase(), GripMotion::RotateAboutPoint, at, [0.6, 0.0, 0.8], [1.0, 0.0, 0.0])
        .unwrap();
    assert!((back.translation - origin0.translation).length() < 1e-9);
    assert!(back.rotation.dot(origin0.rotation).abs() > 1.0 - 1e-9);
}

/// `grip_snap_allowed` follows grounding, relationships and the "no change" case, for every kind of snap, and
/// is false for something that is not a component.
#[test]
fn the_snap_is_allowed_by_the_freedom_the_component_has() {
    let mut s = Session::new();
    let floor = body(&mut s, 5.0);
    let b =
        s.add_body(Solid::Box { size: [0.2; 3] }, Placement { xyz: [0.3, 0.0, 0.5], rpy: [0.0; 3] }).unwrap();
    let up = plane([0.0; 3], [0.0, 0.0, 1.0]);
    let at = GripReference::Point([0.0; 3]);
    let allowed = |s: &Session, item: Id<Body>, m, r, from: [f64; 3], to: [f64; 3]| {
        s.grip_snap_allowed(item.erase(), m, r, from, to)
    };
    // Free: every snap that works out is allowed; one that changes nothing is allowed too.
    assert!(allowed(&s, b, GripMotion::MoveWithinPlane, up, [0.0; 3], [0.4, 0.2, 0.0]));
    assert!(allowed(&s, b, GripMotion::Displace, at, [0.0; 3], [0.0; 3]), "a snap that changes nothing");
    // Grounded: never.
    assert!(!allowed(&s, floor, GripMotion::MoveWithinPlane, up, [0.0; 3], [0.4, 0.2, 0.0]));
    // A rigid joint to the floor: nothing.
    let frame = |item: Id<Body>, z: f64, nz: f64| Selection {
        occurrence: occ(item),
        geometry: Geometry::Frame { origin: [0.0, 0.0, z], z: [0.0, 0.0, nz], x: [1.0, 0.0, 0.0] },
    };
    s.add_relationship(Relationship::new(
        "rigid",
        RelationKind::Rigid { flip: false },
        frame(b, -0.1, -1.0),
        frame(floor, 0.05, 1.0),
    ))
    .unwrap();
    for (m, r) in [
        (GripMotion::MoveWithinPlane, up),
        (GripMotion::MoveAlongNormal, up),
        (GripMotion::Displace, at),
        (GripMotion::RotateAboutPoint, at),
    ] {
        assert!(!allowed(&s, b, m, r, [1.0, 0.0, 0.0], [0.0, 1.0, 0.4]), "a rigid component allowed {m:?}");
    }
    // A mate to the floor leaves sliding and spinning, not lifting.
    let mut t = Session::new();
    let tf = body(&mut t, 5.0);
    let c =
        t.add_body(Solid::Box { size: [0.2; 3] }, Placement { xyz: [0.3, 0.0, 0.5], rpy: [0.0; 3] }).unwrap();
    let on = |item: Id<Body>, z: f64, nz: f64| Selection {
        occurrence: occ(item),
        geometry: Geometry::Plane { point: [0.0, 0.0, z], normal: [0.0, 0.0, nz] },
    };
    t.add_relationship(Relationship::new(
        "mate",
        RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
        on(c, -0.1, -1.0),
        on(tf, 0.05, 1.0),
    ))
    .unwrap();
    assert!(
        allowed(&t, c, GripMotion::MoveWithinPlane, up, [0.0; 3], [0.4, 0.2, 0.0]),
        "sliding on the floor"
    );
    assert!(
        !allowed(&t, c, GripMotion::MoveAlongNormal, up, [0.0; 3], [0.0, 0.0, 0.4]),
        "lifting off the floor"
    );
    // Not a component, and a deleted one.
    let ghost = Id::<Body>::from_raw(std::num::NonZeroU64::new(9999).unwrap()).erase();
    assert!(!t.grip_snap_allowed(ghost, GripMotion::Displace, at, [0.0; 3], [1.0; 3]));
    assert!(t.grip_snap_pose_on(ghost, GripMotion::Displace, at, [0.0; 3], [1.0; 3]).is_none());
}
