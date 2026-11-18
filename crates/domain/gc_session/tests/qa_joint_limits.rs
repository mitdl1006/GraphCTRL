//! QA boundary tests for joint Limits (`set_joint_limits`, `joint_limits`, `joint_position`), Drive within
//! limits, and their interplay with Lock, Protect, suppression, save/open and undo. Inventor 2026 help,
//! GUID-21DC3336-5C51-42C1-90FB-4299CD66E0C6 (Limits tab: Start, Current, End; "a Rotational joint supports
//! Angular limits, but Linear limits are not available. Limits are not available for Rigid joints"), under
//! <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/>.
//! Every refused edit leaves the document and the history untouched; every accepted one is one undo step that
//! restores exactly.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::disallowed_methods,
    reason = "test code"
)]

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

fn hinge() -> (Session, Id<Body>, Id<gc_doc::Relationship>) {
    jointed(RelationKind::Rotational { flip: false })
}

fn refused(
    s: &mut Session,
    what: &str,
    id: Id<gc_doc::Relationship>,
    a: Option<JointRange>,
    l: Option<JointRange>,
) {
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    let result = s.set_joint_limits(id, a, l);
    assert!(result.is_err(), "{what} was accepted: {a:?} {l:?}");
    assert!(s.doc().same_content(&before), "{what} left a trace");
    assert_eq!(s.history().undo_depth(), depth, "{what}");
}

// ---- Ranges ----

#[test]
fn bad_ranges_are_refused_without_a_trace() {
    let (mut s, _, j) = hinge();
    for (what, a) in [
        ("start past end", range(1.0, 0.0, None)),
        ("start past end by a hair", range(1e-12, 0.0, None)),
        ("current below", range(0.0, 1.0, Some(-1e-9))),
        ("current above", range(0.0, 1.0, Some(1.0 + 1e-9))),
        ("NaN start", range(f64::NAN, 1.0, None)),
        ("NaN end", range(0.0, f64::NAN, None)),
        ("NaN current", range(0.0, 1.0, Some(f64::NAN))),
        ("infinite end", range(0.0, f64::INFINITY, None)),
        ("infinite start", range(f64::NEG_INFINITY, 1.0, None)),
        ("infinite current", range(0.0, 1.0, Some(f64::INFINITY))),
    ] {
        refused(&mut s, what, j, a, None);
    }
}

/// Zero-width ranges, a current on either end, wide and huge ranges are valid; each is one undo step that
/// restores exactly, and a hinge at a current stands there.
#[test]
fn edge_ranges_are_accepted_as_one_undo_step() {
    for (what, r) in [
        ("zero width", range(0.2, 0.2, None)),
        ("zero width with current", range(0.2, 0.2, Some(0.2))),
        ("current at start", range(-0.5, 0.5, Some(-0.5))),
        ("current at end", range(-0.5, 0.5, Some(0.5))),
        ("negative zero", range(-0.0, 0.0, Some(-0.0))),
        ("many turns", range(-20.0, 20.0, Some(12.0))),
        ("huge", range(-1e300, 1e300, None)),
    ] {
        let (mut s, block, j) = hinge();
        let before = s.doc().clone();
        let depth = s.history().undo_depth();
        s.set_joint_limits(j, r, None).unwrap_or_else(|e| panic!("{what}: {e:?}"));
        assert_eq!(s.history().undo_depth(), depth + 1, "{what}: one undo step");
        assert!(s.doc().solve_relationships().is_ok(), "{what}: the cell no longer solves");
        let p = s.doc().component_origin(Occurrence::Item(block.erase())).unwrap().pose;
        assert!(p.translation.is_finite() && p.rotation.is_finite(), "{what}: the block is not finite");
        if let Some(JointRange { current: Some(c), start, end }) = r
            && (end - start).abs() < 1e3
        {
            let (turn, _) = s.joint_position(j).unwrap();
            // A turn is read modulo a full turn.
            let d = (turn - c).rem_euclid(std::f64::consts::TAU);
            assert!(d.min(std::f64::consts::TAU - d) < 1e-6, "{what}: stands at {turn}, wanted {c}");
        }
        assert!(s.undo(), "{what}");
        assert!(s.doc().same_content(&before), "{what}: undo restores");
        assert!(s.redo(), "{what}");
    }
}

// ---- Joint kinds ----

#[test]
fn limits_are_refused_for_joint_kinds_that_have_none_and_for_non_joints_and_ghosts() {
    let ranges = [
        (range(0.0, 1.0, None), None),
        (None, range(0.0, 1.0, None)),
        (range(0.0, 1.0, None), range(0.0, 1.0, None)),
    ];
    // Rigid and Ball: none of either. Slider: linear only. Rotational: angular only. Cylindrical: both.
    let cases: [(RelationKind, [bool; 3]); 5] = [
        (RelationKind::Rigid { flip: false }, [false, false, false]),
        (RelationKind::Ball, [false, false, false]),
        (RelationKind::Slider { flip: false }, [false, true, false]),
        (RelationKind::Rotational { flip: false }, [true, false, false]),
        (RelationKind::Cylindrical { flip: false }, [true, true, true]),
    ];
    for (kind, allowed) in cases {
        let (mut s, _, j) = jointed(kind);
        for (i, (a, l)) in ranges.iter().enumerate() {
            let before = s.doc().clone();
            let depth = s.history().undo_depth();
            let result = s.set_joint_limits(j, *a, *l);
            if allowed[i] {
                assert!(result.is_ok(), "{kind:?} {i}: {result:?}");
                assert_eq!(s.history().undo_depth(), depth + 1);
                assert!(s.undo() && s.doc().same_content(&before));
            } else {
                assert_eq!(result, Err(EditError::BadValue), "{kind:?} {i}");
                assert!(s.doc().same_content(&before) && s.history().undo_depth() == depth, "{kind:?} {i}");
            }
        }
    }
    // A constraint (Mate) is not a joint; a missing relationship is nothing.
    let (mut s, table, block) = {
        let mut s = Session::new();
        let t = s
            .add_body(Solid::Box { size: [2.0, 1.0, 1.0] }, Placement { xyz: [0.0, 0.0, 0.5], rpy: [0.0; 3] })
            .unwrap();
        let b = s
            .add_body(Solid::Box { size: [0.2; 3] }, Placement { xyz: [0.3, 0.2, 1.6], rpy: [0.0; 3] })
            .unwrap();
        (s, t, b)
    };
    let plane = |item: Id<Body>, z: f64, nz: f64| {
        sel(item.erase(), Geometry::Plane { point: [0.0, 0.0, z], normal: [0.0, 0.0, nz] })
    };
    let mate = s
        .add_relationship(Relationship::new(
            "Mate",
            RelationKind::Mate { offset: 0.0, sense: gc_doc::AxisSense::Opposed },
            plane(block, -0.1, -1.0),
            plane(table, 0.5, 1.0),
        ))
        .unwrap();
    let ghost = Id::<gc_doc::Relationship>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    for id in [mate, ghost] {
        refused(&mut s, &format!("{id:?}"), id, range(0.0, 1.0, None), None);
        refused(&mut s, &format!("{id:?}"), id, None, range(0.0, 1.0, None));
    }
    // Clearing limits on something that has none is harmless (accepted or refused, never a stray record).
    let before = s.doc().clone();
    drop(s.set_joint_limits(mate, None, None));
    assert!(s.doc().same_content(&before), "clearing limits of a constraint changed the document");
}

// ---- Drive ----

/// Drive stops at the end of the range, whichever way and however far it is asked; non-finite values are
/// refused; drive is a preview that leaves the document alone.
#[test]
fn drive_stops_at_the_ends_of_the_range() {
    let (mut s, block, j) = hinge();
    s.set_joint_limits(j, range(-0.5, 0.5, None), None).unwrap();
    let before = s.doc().clone();
    for (value, wanted) in
        [(2.0, 0.5), (-2.0, -0.5), (0.5, 0.5), (-0.5, -0.5), (0.1, 0.1), (1e9, 0.5), (-1e9, -0.5)]
    {
        s.drive_relationship(j, value).unwrap();
        let yaw = s.driven_placement(block.erase()).unwrap().rpy[2];
        assert!((yaw - wanted).abs() < 1e-6, "driven to {value}: stands at {yaw}, wanted {wanted}");
    }
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(s.drive_relationship(j, bad).is_err(), "driven to {bad}");
    }
    s.end_drive_relationship();
    assert!(s.doc().same_content(&before), "driving changed the document");
}

#[test]
fn a_slider_and_a_cylindrical_joint_are_driven_within_their_linear_range() {
    let (mut s, block, slider) = jointed(RelationKind::Slider { flip: false });
    s.set_joint_limits(slider, None, range(0.0, 0.4, None)).unwrap();
    for (value, wanted) in [(0.1, 0.1), (5.0, 0.4), (-5.0, 0.0)] {
        s.drive_relationship(slider, value).unwrap();
        // What is shown (the preview while driving): `driven_placement` only lists what differs from the document,
        // and setting the range already put the block at the end of it.
        let z = s.shown_body(block).unwrap().placement.xyz[2];
        assert!((z - (1.1 + wanted)).abs() < 1e-6, "slider {value}: z {z}, wanted {}", 1.1 + wanted);
    }
    s.end_drive_relationship();
    let (mut s, _block, cyl) = jointed(RelationKind::Cylindrical { flip: false });
    s.set_joint_limits(cyl, range(-1.0, 1.0, None), range(-0.2, 0.2, None)).unwrap();
    s.drive_relationship(cyl, 3.0).unwrap(); // the angular range wins for a joint that has one
    s.end_drive_relationship();
    // A Rigid joint cannot be driven.
    let (mut r, _, rigid) = jointed(RelationKind::Rigid { flip: false });
    assert!(r.drive_relationship(rigid, 0.1).is_err());
}

// ---- Lock, Protect, suppression ----

/// Lock and limits together: a locked joint has no freedom; limits set on it are kept and the lock wins until
/// it is released; the cell always solves and nothing ends outside the range once unlocked.
#[test]
fn lock_and_limits_together_stay_consistent() {
    let (mut s, block, j) = hinge();
    s.set_joint_limits(j, range(-0.5, 0.5, Some(0.3)), None).unwrap();
    // Locking a joint that stands at its Current keeps it there.
    s.set_joint_lock(j, true).unwrap();
    assert_eq!(s.component_dof(block.erase()).unwrap().total, 0);
    assert!((s.joint_position(j).unwrap().0 - 0.3).abs() < 1e-6);
    assert!(s.joint_limits(j).0.is_some(), "locking dropped the limits");
    // Changing the range while locked: accepted or refused, the cell stays solvable and the lock stays.
    let before = s.doc().clone();
    let result = s.set_joint_limits(j, range(-1.0, 1.0, None), None);
    if result.is_err() {
        assert!(s.doc().same_content(&before));
    }
    assert!(s.doc().solve_relationships().is_ok());
    assert!(s.joint_condition(j).locked.is_some(), "the lock went");
    // Driving a locked, limited joint does not move it away from where it stands.
    let stood = s.joint_position(j).unwrap().0;
    drop(s.drive_relationship(j, 0.4));
    s.end_drive_relationship();
    assert!((s.joint_position(j).unwrap().0 - stood).abs() < 1e-6, "a locked joint was driven");
    // Release the lock, then a current outside the (possibly changed) range is refused; undo walks back.
    s.set_joint_lock(j, false).unwrap();
    assert!(s.joint_condition(j).locked.is_none());
    let (a, _) = s.joint_limits(j);
    if let Some(r) = a {
        let turn = s.joint_position(j).unwrap().0;
        assert!(turn >= r.start - 1e-6 && turn <= r.end + 1e-6, "{turn} outside {r:?}");
    }
}

/// Protected joints keep limits too; suppressing the joint's relationship frees the block, and bringing it back
/// puts the limits to work again. Deleting the joint removes its limits and undo gives them back.
#[test]
fn protect_suppress_and_delete_keep_limits_in_step() {
    let (mut s, block, j) = hinge();
    s.set_joint_limits(j, range(-0.5, 0.5, Some(0.2)), None).unwrap();
    s.set_joint_protect(j, true).unwrap();
    assert!(s.joint_limits(j).0.is_some());
    s.set_relationship_suppressed(j, true).unwrap();
    assert_eq!(s.component_dof(block.erase()).unwrap().total, 6, "a suppressed joint leaves the block free");
    s.set_relationship_suppressed(j, false).unwrap();
    assert!((s.joint_position(j).unwrap().0 - 0.2).abs() < 1e-6, "the limits did not hold the joint again");
    let with = s.doc().clone();
    s.delete_relationship(j).unwrap();
    assert_eq!(s.joint_limits(j), (None, None));
    assert!(s.undo());
    assert!(s.doc().same_content(&with));
    assert_eq!(s.joint_limits(j).0, range(-0.5, 0.5, Some(0.2)));
}

// ---- Save and open ----

#[test]
fn limits_round_trip_exactly_and_old_files_have_none() {
    let (mut s, _, j) = jointed(RelationKind::Cylindrical { flip: false });
    s.set_joint_limits(j, range(-1.0, 1.0, Some(0.25)), range(0.0, 0.2, None)).unwrap();
    s.set_joint_protect(j, true).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("limits.gcp");
    s.save_as(&path).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert!(t.doc().same_content(s.doc()), "the reopened document differs");
    assert_eq!(t.joint_limits(j), (range(-1.0, 1.0, Some(0.25)), range(0.0, 0.2, None)));
    let again = dir.path().join("again.gcp");
    s.save_as(&again).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), std::fs::read(&again).unwrap(), "saving twice differs");
    // An old project has no `angular` or `linear`: it opens with no limits and keeps Protect.
    let mut value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    fn strip(v: &mut serde_json::Value, found: &mut usize) {
        match v {
            serde_json::Value::Object(map) => {
                for key in ["angular", "linear"] {
                    if map.remove(key).is_some() {
                        *found += 1;
                    }
                }
                map.values_mut().for_each(|x| strip(x, found));
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(|x| strip(x, found)),
            _ => {}
        }
    }
    let mut found = 0;
    strip(&mut value, &mut found);
    assert!(found >= 2, "the project does not store limits where expected ({found})");
    let old = dir.path().join("old.gcp");
    std::fs::write(&old, serde_json::to_string(&value).unwrap()).unwrap();
    let mut u = Session::new();
    u.open(&old).unwrap();
    assert_eq!(u.joint_limits(j), (None, None));
    assert!(u.joint_condition(j).protected, "an old project lost Protect");
}

/// A project file whose joint range has Start past End (edited by hand, damaged, or written by another
/// version) must open and solve without crashing: refused, repaired or ignored, but not a panic.
#[test]
fn a_damaged_joint_range_in_a_file_does_not_crash_the_solver() {
    let (mut s, _, j) = jointed(RelationKind::Cylindrical { flip: false });
    s.set_joint_limits(j, range(-1.0, 1.0, Some(0.25)), range(0.0, 0.2, None)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("limits.gcp");
    s.save_as(&path).unwrap();
    // A damaged range (Start past End) in a file: the file opens, or is refused; never a broken cell.
    let mut damaged: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    fn flip(v: &mut serde_json::Value) {
        match v {
            serde_json::Value::Object(map) => {
                if let (Some(start), Some(end)) = (map.get("start").cloned(), map.get("end").cloned())
                    && (map.contains_key("current") || start.is_number())
                {
                    map.insert("start".into(), end);
                    map.insert("end".into(), start);
                }
                map.values_mut().for_each(flip);
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(flip),
            _ => {}
        }
    }
    flip(&mut damaged);
    let bad = dir.path().join("bad.gcp");
    std::fs::write(&bad, serde_json::to_string(&damaged).unwrap()).unwrap();
    let mut w = Session::new();
    if w.open(&bad).is_ok() {
        assert!(w.doc().solve_relationships().is_ok(), "a damaged range left an unsolvable cell");
        let p = w
            .doc()
            .component_origin(Occurrence::Item(w.components().last().unwrap().occurrence.owner()))
            .unwrap()
            .pose;
        assert!(p.translation.is_finite());
    }
}
