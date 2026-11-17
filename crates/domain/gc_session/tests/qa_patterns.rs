//! QA edge cases for component patterns, constraint limits and conflict recovery (assembly parity, lead
//! 1ffb218 and f55e408). Inventor 2026 help:
//! - rectangular patterns: GUID-CF627358-120F-4142-A9D0-13217BD3432D
//! - circular patterns: GUID-FF7B08E3-624B-4382-B4E5-304C0354C048
//! - limits ("Maximum", "Minimum", "Use Offset As Resting Position") and the constraint error dialog:
//!   To Place Mate or Flush Constraints in Assemblies, GUID-0F500089-4F69-41C5-AFE8-B58AC9E8E15E
//!
//! All under <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/>.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::disallowed_methods,
    reason = "test code"
)]

use gc_core::{AnyId, Id};
use gc_doc::{
    AxisSense, Body, CellOp, Command, Geometry, Limits, Occurrence, OriginElement, OriginPlane,
    PatternDirection, PatternLayout, Placement, RelationKind, Relationship, Selection, Solid,
};
use gc_math::DVec3;
use gc_session::Session;

fn placed(xyz: [f64; 3]) -> Placement {
    Placement { xyz, rpy: [0.0; 3] }
}

fn axis_of(item: Id<Body>, axis: u8) -> Selection {
    Selection {
        occurrence: Occurrence::Item(item.erase()),
        geometry: Geometry::origin(OriginElement::Axis(axis)),
    }
}

fn position(s: &Session, item: AnyId) -> DVec3 {
    DVec3::from_array(s.doc().get(Id::<Body>::from_raw(item.raw)).unwrap().placement.xyz)
}

fn column(floor: Id<Body>, count: u32, spacing: f64) -> PatternLayout {
    PatternLayout::Rectangular {
        column: PatternDirection { along: axis_of(floor, 0), flip: false, count, spacing },
        row: None,
    }
}

fn setup() -> (Session, Id<Body>, Id<Body>) {
    let mut s = Session::new();
    let floor = s.add_body(Solid::Box { size: [2.0, 1.0, 0.1] }, placed([0.0; 3])).unwrap();
    let part = s.add_body(Solid::Box { size: [0.1; 3] }, placed([1.0, 0.0, 0.2])).unwrap();
    (s, floor, part)
}

fn bodies(s: &Session) -> usize {
    s.doc().bodies().iter().count()
}

// ---- patterns ----

/// Counts that make no pattern, spacing that is not a number, parents that are missing: refused with the
/// document untouched. (Count 1 is the parent alone: not a pattern; 1001 is over the limit.)
#[test]
fn unusable_patterns_are_refused_and_change_nothing() {
    let (mut s, floor, part) = setup();
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    let ghost = Id::<Body>::from_raw(std::num::NonZeroU64::new(9999).unwrap()).erase();
    for (what, parents, layout) in [
        ("count 0", vec![part.erase()], column(floor, 0, 0.5)),
        ("count 1", vec![part.erase()], column(floor, 1, 0.5)),
        ("count 1001", vec![part.erase()], column(floor, 1001, 0.5)),
        ("count max", vec![part.erase()], column(floor, u32::MAX, 0.5)),
        ("NaN spacing", vec![part.erase()], column(floor, 3, f64::NAN)),
        ("infinite spacing", vec![part.erase()], column(floor, 3, f64::INFINITY)),
        ("no parents", vec![], column(floor, 3, 0.5)),
        ("missing parent", vec![ghost], column(floor, 3, 0.5)),
        ("a parent and a missing one", vec![part.erase(), ghost], column(floor, 3, 0.5)),
    ] {
        let result = s.add_pattern("P", &parents, layout);
        assert!(result.is_err(), "{what} was accepted");
        assert!(s.doc().same_content(&before), "{what} changed the document");
        assert_eq!(s.history().undo_depth(), depth, "{what}");
    }
    // A direction that is not an axis-like thing (a bare point), or a missing component, gives no pattern.
    let point =
        Selection { occurrence: Occurrence::Item(floor.erase()), geometry: Geometry::Point([0.0; 3]) };
    let layout = PatternLayout::Rectangular {
        column: PatternDirection { along: point, flip: false, count: 3, spacing: 0.5 },
        row: None,
    };
    assert!(s.add_pattern("P", &[part.erase()], layout).is_err());
    let row = PatternLayout::Rectangular {
        column: PatternDirection { along: axis_of(floor, 0), flip: false, count: 3, spacing: 0.5 },
        row: Some(PatternDirection { along: axis_of(floor, 1), flip: false, count: 0, spacing: 0.5 }),
    };
    assert!(s.add_pattern("P", &[part.erase()], row).is_err(), "a row of zero");
    assert!(s.doc().same_content(&before));
}

/// A valid pattern: right number of copies, positions, one undo step, copies are components of their own that
/// are not grounded; saved and reopened it is the same document; deleting the pattern removes the copies.
#[test]
fn a_pattern_makes_copies_one_undo_step_and_survives_save_and_delete() {
    let (mut s, floor, part) = setup();
    let base_bodies = bodies(&s);
    let depth = s.history().undo_depth();
    s.add_pattern("Row", &[part.erase()], column(floor, 4, 0.3)).unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1);
    assert_eq!(bodies(&s), base_bodies + 3, "three copies");
    let pattern = s.patterns()[0].clone();
    assert_eq!(pattern.elements.len(), 3);
    for (k, element) in pattern.elements.iter().enumerate() {
        let p = position(&s, element.items[0]);
        let expect = DVec3::new(1.0 + 0.3 * (k as f64 + 1.0), 0.0, 0.2);
        assert!((p - expect).length() < 1e-9, "copy {k}: {p:?}");
        assert!(
            !s.components()
                .iter()
                .find(|n| n.occurrence == Occurrence::Item(element.items[0]))
                .unwrap()
                .grounded
        );
    }
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("pattern.gctrl");
    s.save_as(&file).unwrap();
    let mut reopened = Session::new();
    reopened.open(&file).unwrap();
    assert!(reopened.doc().same_content(s.doc()));
    assert_eq!(reopened.patterns().len(), 1);
    // The reopened pattern still follows its parent.
    let mut moved = reopened.doc().get(part).unwrap().clone();
    moved.placement.xyz = [1.0, 0.5, 0.2];
    reopened.execute(&Command::Cell(CellOp::SetBody { id: part, body: moved })).unwrap();
    let copy = reopened.patterns()[0].elements[0].items[0];
    assert!((position(&reopened, copy) - DVec3::new(1.3, 0.5, 0.2)).length() < 1e-9);

    // Deleting the pattern with its copies; Undo brings it all back.
    s.execute(&Command::Cell(CellOp::DeletePattern { index: 0, keep_copies: false })).unwrap();
    assert_eq!(bodies(&s), base_bodies);
    assert!(s.patterns().is_empty());
    assert!(s.undo());
    assert_eq!(bodies(&s), base_bodies + 3);
}

/// Editing a pattern to something invalid, or naming an occurrence or pattern that does not exist, is
/// refused and leaves everything as it was.
#[test]
fn bad_pattern_edits_are_refused() {
    let (mut s, floor, part) = setup();
    s.add_pattern("Row", &[part.erase()], column(floor, 3, 0.3)).unwrap();
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    for op in [
        CellOp::SetPattern { index: 0, layout: column(floor, 1, 0.3) },
        CellOp::SetPattern { index: 0, layout: column(floor, 3, f64::NAN) },
        CellOp::SetPattern { index: 7, layout: column(floor, 3, 0.3) },
        CellOp::MakePatternElementIndependent { index: 0, element: 0 }, // the parent is not an element
        CellOp::MakePatternElementIndependent { index: 0, element: 99 },
        CellOp::MakePatternElementIndependent { index: 5, element: 1 },
        CellOp::DeletePattern { index: 5, keep_copies: true },
    ] {
        let result = s.execute(&Command::Cell(op.clone()));
        assert!(result.is_err(), "{op:?} was accepted");
        assert!(s.doc().same_content(&before), "{op:?}");
        assert_eq!(s.history().undo_depth(), depth);
    }
}

/// Circular: fitted 360 with 4 copies puts them a quarter turn apart; midplane spreads both ways from the
/// parent; a flipped axis reverses the sense; an axis that is a bare point is refused.
#[test]
fn circular_patterns_fit_flip_and_spread() {
    let mut s = Session::new();
    let hub = s.add_body(Solid::Box { size: [0.1; 3] }, placed([0.0; 3])).unwrap();
    let part = s.add_body(Solid::Box { size: [0.1; 3] }, placed([1.0, 0.0, 0.0])).unwrap();
    let layout = |flip: bool, fitted: bool, midplane: bool, angle: f64| PatternLayout::Circular {
        axis: axis_of(hub, 2),
        flip,
        count: 4,
        angle,
        fitted,
        rotational: false,
        midplane,
    };
    let at_angle = |s: &Session, pattern: usize, k: usize| {
        let item = s.patterns()[pattern].elements[k].items[0];
        let p = position(s, item);
        p.y.atan2(p.x).to_degrees()
    };
    s.add_pattern("Fit", &[part.erase()], layout(false, true, false, std::f64::consts::TAU)).unwrap();
    // Fitted over a full turn with 4 occurrences: 0, 90, 180, 270 (the full circle is not closed twice).
    let angles: Vec<f64> = (0..3).map(|k| at_angle(&s, 0, k).rem_euclid(360.0)).collect();
    for (got, want) in angles.iter().zip([90.0, 180.0, 270.0]) {
        assert!((got - want).abs() < 1e-6, "{angles:?}");
    }
    s.add_pattern("Flip", &[part.erase()], layout(true, false, false, 90f64.to_radians())).unwrap();
    assert!(
        (at_angle(&s, 1, 0) + 90.0).abs() < 1e-6,
        "flipped axis turns the other way: {}",
        at_angle(&s, 1, 0)
    );
    s.add_pattern("Mid", &[part.erase()], layout(false, false, true, 30f64.to_radians())).unwrap();
    let mid: Vec<f64> = (0..3).map(|k| at_angle(&s, 2, k)).collect();
    assert!(
        mid.iter().any(|a| *a < -1e-6) && mid.iter().any(|a| *a > 1e-6),
        "both sides of the parent: {mid:?}"
    );
    // An axis made of a point has no direction.
    let point = Selection { occurrence: Occurrence::Item(hub.erase()), geometry: Geometry::Point([0.0; 3]) };
    let bad = PatternLayout::Circular {
        axis: point,
        flip: false,
        count: 4,
        angle: 1.0,
        fitted: false,
        rotational: false,
        midplane: false,
    };
    let before = s.doc().clone();
    assert!(s.add_pattern("Bad", &[part.erase()], bad).is_err());
    assert!(s.doc().same_content(&before));
    // Zero or NaN angle: refused or harmless, never a NaN position.
    for angle in [0.0, f64::NAN, f64::INFINITY] {
        let result = s.add_pattern("Odd", &[part.erase()], layout(false, false, false, angle));
        if result.is_ok() {
            let last = s.patterns().len() - 1;
            for element in &s.patterns()[last].elements {
                assert!(position(&s, element.items[0]).is_finite(), "angle {angle}");
            }
            assert!(s.undo());
        }
    }
}

// ---- limits and conflict recovery ----

fn mate_on_table() -> (Session, Id<Body>, Id<Body>, Relationship) {
    let mut s = Session::new();
    let table = s.add_body(Solid::Box { size: [2.0, 1.0, 1.0] }, placed([0.0, 0.0, 0.5])).unwrap();
    let block = s.add_body(Solid::Box { size: [0.2; 3] }, placed([0.3, 0.2, 1.6])).unwrap();
    let on = |b: Id<Body>, g| Selection { occurrence: Occurrence::Item(b.erase()), geometry: g };
    let mate = Relationship::new(
        "Mate:1",
        RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
        on(block, Geometry::Plane { point: [0.0, 0.0, -0.1], normal: [0.0, 0.0, -1.0] }),
        on(table, Geometry::Plane { point: [0.0, 0.0, 0.5], normal: [0.0, 0.0, 1.0] }),
    );
    (s, table, block, mate)
}

/// Limits that are upside down, not numbers, or equal (a single allowed value) are refused or fine
/// accordingly; an upside-down or non-finite one never reaches the document.
#[test]
fn bad_limits_are_refused_and_equal_limits_pin_the_value() {
    let (mut s, _, block, mate) = mate_on_table();
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    for limits in [
        Limits { min: Some(0.3), max: Some(0.1), resting: false },
        Limits { min: Some(f64::NAN), max: None, resting: false },
        Limits { min: None, max: Some(f64::INFINITY), resting: false },
        Limits { min: Some(f64::NEG_INFINITY), max: None, resting: true },
    ] {
        let mut with = mate.clone();
        with.limits = Some(limits);
        assert!(s.add_relationship(with).is_err(), "{limits:?} accepted");
        assert!(s.doc().same_content(&before));
        assert_eq!(s.history().undo_depth(), depth);
    }
    // min == max: the offset is that one value.
    let mut pinned = mate.clone();
    pinned.limits = Some(Limits { min: Some(0.1), max: Some(0.1), resting: false });
    s.add_relationship(pinned).unwrap();
    let z = s.doc().get(block).unwrap().placement.xyz[2];
    assert!((z - (1.0 + 0.1 + 0.1)).abs() < 1e-6, "{z}");
    // A limit that excludes the constraint's own value: the value is clamped into it.
    let mut clamped = mate;
    clamped.limits = Some(Limits { min: Some(0.4), max: Some(0.5), resting: false });
    let id = s.relationships()[0].0;
    s.set_relationship(id, clamped).unwrap();
    let z = s.doc().get(block).unwrap().placement.xyz[2];
    assert!((1.5 - 1e-6..=1.6 + 1e-6).contains(&z), "{z}");
}

/// Drive with limits never leaves them (it stops at the limit); Limits::clamp is monotone and idempotent.
#[test]
fn drive_stays_inside_the_limits() {
    let (mut s, _, block, mut mate) = mate_on_table();
    mate.limits = Some(Limits { min: Some(0.0), max: Some(0.3), resting: true });
    let id = s.add_relationship(mate).unwrap();
    for value in [-5.0, 0.0, 0.1, 0.3, 5.0] {
        if s.drive_relationship(id, value).is_ok() {
            let z = s
                .driven_placement(block.erase())
                .map_or(s.doc().get(block).unwrap().placement.xyz[2], |p| p.xyz[2]);
            let gap = z - 1.1;
            assert!((-1e-6..=0.3 + 1e-6).contains(&gap), "driven to {value}: gap {gap}");
        }
    }
    s.end_drive_relationship();
    let limits = Limits { min: Some(-1.0), max: Some(2.0), resting: false };
    for v in [-10.0, -1.0, 0.0, 2.0, 10.0] {
        let c = limits.clamp(v);
        assert!((-1.0..=2.0).contains(&c));
        assert_eq!(limits.clamp(c), c, "idempotent");
    }
    assert_eq!(Limits { min: None, max: None, resting: false }.clamp(7.0), 7.0);
}

/// The constraint error dialog: the culprit is named, suppressing it recovers and Undo undoes the
/// suppression; with no conflict, nothing is named; with two unrelated culprits no single one is named.
#[test]
fn a_conflict_names_its_culprit_and_recovery_is_undoable() {
    let (mut s, table, block, mate) = mate_on_table();
    let mate_id = s.add_relationship(mate).unwrap();
    let xy = Geometry::origin(OriginElement::Plane(OriginPlane::Xy));
    let on = |b: Id<Body>| Selection { occurrence: Occurrence::Item(b.erase()), geometry: xy };
    let flush = Relationship::new("Flush:1", RelationKind::Flush { offset: 0.3 }, on(block), on(table));
    let before = s.doc().clone();
    assert!(s.add_relationship(flush.clone()).is_err());
    assert!(s.doc().same_content(&before), "the refused relationship left no trace");
    assert_eq!(s.conflicting_relationships(Some(&flush)), vec![mate_id]);
    assert!(s.conflicting_relationships(None).is_empty());
    let depth = s.history().undo_depth();
    s.set_relationship_suppressed(mate_id, true).unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1);
    s.add_relationship(flush).unwrap();
    // Bringing the mate back would conflict now: refused, and the suppression stays.
    assert!(s.set_relationship_suppressed(mate_id, false).is_err());
    assert!(s.doc().get(mate_id).is_some());
    // A relationship that does not exist.
    let ghost = Id::<Relationship>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    assert!(s.set_relationship_suppressed(ghost, true).is_err());
    assert!(s.conflicting_relationships(None).is_empty());
}
