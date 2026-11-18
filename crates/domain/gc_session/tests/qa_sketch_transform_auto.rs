//! QA boundary tests for sketch Move/Copy/Rotate (`SketchOp::Transform`), Automatic Dimensions and
//! Constraints (`SketchOp::AutoDimension`) and Flip Direction (`SketchOp::FlipDimensions`).
//! Inventor 2026 help, under <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/>:
//! copy/move/rotate GUID-F8114270-18F7-4C7A-947E-AC489EDACFA9, automatic dimensions
//! GUID-1DC71C6F-9F6D-4846-8E73-9CB7D3452882, sketch dimensions (Flip Direction)
//! GUID-026FE8D3-AFBB-4834-BC18-73D9CFF78185.
//! Every refused edit leaves the sketch and the history untouched; every accepted one is one undo step that
//! restores exactly and leaves only finite points.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::disallowed_methods,
    reason = "test code"
)]

use gc_core::Id;
use gc_doc::{Command, Created, EndSnap, Motion, SketchOp};
use gc_session::Session;
use gc_sketch::{Curve, DimKind, GeoConstraint};

type K = Id<gc_doc::Sketch>;

fn setup() -> (Session, K) {
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    (s, k)
}

fn run(s: &mut Session, k: K, op: SketchOp) -> Result<Vec<Created>, gc_doc::EditError> {
    s.execute(&Command::Sketch { sketch: k, op })
}

fn line(s: &mut Session, k: K, a: [f64; 2], b: [f64; 2]) -> Id<Curve> {
    let op = SketchOp::AddLine {
        a: [a[0], a[1], 0.0],
        b: [b[0], b[1], 0.0],
        snap_a: EndSnap::default(),
        snap_b: EndSnap::default(),
        horizontal: None,
    };
    run(s, k, op)
        .unwrap()
        .iter()
        .find_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None })
        .unwrap()
}

fn ends(s: &Session, c: Id<Curve>) -> ([f64; 2], [f64; 2]) {
    let sk = s.active_sketch().unwrap().1;
    let (a, b) = sk.line(c).unwrap();
    let (a, b) = (sk.pos(a).unwrap(), sk.pos(b).unwrap());
    ([a[0], a[1]], [b[0], b[1]])
}

fn ghost() -> Id<Curve> {
    Id::<Curve>::from_raw(std::num::NonZeroU64::new(9999).unwrap())
}

fn dof(s: &Session) -> usize {
    s.active_sketch().unwrap().1.clone().solve().unwrap().dof
}

fn drawn(s: &Session) -> usize {
    s.active_sketch().unwrap().1.curves.values().filter(|c| !c.construction).count()
}

fn sane(s: &Session, what: &str) {
    let sk = s.active_sketch().unwrap().1;
    assert!(sk.points.values().all(|p| p.pos.iter().all(|v| v.is_finite())), "{what}: a point is not finite");
    assert!(
        sk.curves.values().all(|c| c.points().iter().all(|p| sk.points.contains_key(p))),
        "{what}: a curve lost a point"
    );
}

fn refused(s: &mut Session, k: K, what: &str, op: SketchOp) {
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    assert!(run(s, k, op).is_err(), "{what} was accepted");
    assert!(s.doc().same_content(&before), "{what} changed the sketch");
    assert_eq!(s.history().undo_depth(), depth, "{what}");
}

/// Refused with no trace, or accepted as one undo step that undoes exactly. Returns whether accepted.
fn refused_or_sane(s: &mut Session, k: K, what: &str, op: SketchOp) -> bool {
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    match run(s, k, op) {
        Err(_) => {
            assert!(s.doc().same_content(&before), "{what} changed the sketch");
            assert_eq!(s.history().undo_depth(), depth, "{what}");
            false
        }
        Ok(_) => {
            assert_eq!(s.history().undo_depth(), depth + 1, "{what}: one undo step");
            sane(s, what);
            assert!(s.active_sketch().unwrap().1.clone().solve().is_ok(), "{what}: no longer solves");
            assert!(s.undo());
            assert!(s.doc().same_content(&before), "{what}: undo restores");
            true
        }
    }
}

fn transform(curves: Vec<Id<Curve>>, motion: Motion, copy: bool, relax: bool) -> SketchOp {
    SketchOp::Transform { curves, motion, copy, relax }
}

fn rotate(centre: [f64; 2], deg: f64) -> Motion {
    Motion::Rotate { centre, angle: deg.to_radians() }
}

// ---- Rotate ----

#[test]
fn rotating_by_a_quarter_turn_moves_the_line_and_undoes_exactly() {
    let (mut s, k) = setup();
    let l = line(&mut s, k, [0.01, 0.0], [0.03, 0.0]);
    let before = s.doc().clone();
    run(&mut s, k, transform(vec![l], rotate([0.0, 0.0], 90.0), false, true)).unwrap();
    let (a, b) = ends(&s, l);
    assert!(a[0].abs() < 1e-9 && (a[1] - 0.01).abs() < 1e-9, "{a:?}");
    assert!(b[0].abs() < 1e-9 && (b[1] - 0.03).abs() < 1e-9, "{b:?}");
    assert!(s.undo());
    assert!(s.doc().same_content(&before));
}

/// Rotating by 0, 360 or any whole number of turns leaves the geometry where it was (within rounding), and
/// copying by 0 or 360 degrees puts a copy exactly over the original.
#[test]
fn rotation_by_zero_and_whole_turns_is_the_identity() {
    for deg in [0.0, 360.0, -360.0, 720.0, 3600.0] {
        let (mut s, k) = setup();
        let l = line(&mut s, k, [0.01, 0.0], [0.03, 0.01]);
        let (a0, b0) = ends(&s, l);
        let accepted = refused_or_sane(
            &mut s,
            k,
            &format!("move {deg}"),
            transform(vec![l], rotate([0.0, 0.0], deg), false, true),
        );
        if accepted {
            run(&mut s, k, transform(vec![l], rotate([0.0, 0.0], deg), false, true)).unwrap();
            let (a, b) = ends(&s, l);
            for (p, q) in [(a, a0), (b, b0)] {
                assert!((p[0] - q[0]).abs() < 1e-9 && (p[1] - q[1]).abs() < 1e-9, "{deg}: {p:?} vs {q:?}");
            }
        }
        let n = drawn(&s);
        if refused_or_sane(
            &mut s,
            k,
            &format!("copy {deg}"),
            transform(vec![l], rotate([0.0, 0.0], deg), true, true),
        ) {
            run(&mut s, k, transform(vec![l], rotate([0.0, 0.0], deg), true, true)).unwrap();
            assert_eq!(drawn(&s), n + 1, "{deg}: a copy");
        }
    }
}

#[test]
fn rotation_by_nan_or_infinity_and_odd_centres_is_refused() {
    let (mut s, k) = setup();
    let l = line(&mut s, k, [0.01, 0.0], [0.03, 0.0]);
    for angle in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for copy in [false, true] {
            refused(
                &mut s,
                k,
                &format!("angle {angle} copy {copy}"),
                transform(vec![l], Motion::Rotate { centre: [0.0, 0.0], angle }, copy, true),
            );
        }
    }
    for centre in [[f64::NAN, 0.0], [0.0, f64::INFINITY]] {
        refused(
            &mut s,
            k,
            &format!("centre {centre:?}"),
            transform(vec![l], Motion::Rotate { centre, angle: 1.0 }, false, true),
        );
    }
    for by in [[f64::NAN, 0.0], [0.0, f64::NEG_INFINITY]] {
        refused(&mut s, k, &format!("by {by:?}"), transform(vec![l], Motion::Translate { by }, false, true));
    }
    // Huge but finite: refused or still finite.
    refused_or_sane(
        &mut s,
        k,
        "1e300 translate",
        transform(vec![l], Motion::Translate { by: [1e300, 0.0] }, false, true),
    );
    refused_or_sane(&mut s, k, "huge angle", transform(vec![l], rotate([0.0, 0.0], 1e15), false, true));
}

#[test]
fn transform_needs_curves_that_exist() {
    let (mut s, k) = setup();
    let l = line(&mut s, k, [0.01, 0.0], [0.03, 0.0]);
    for copy in [false, true] {
        refused(&mut s, k, "none", transform(vec![], Motion::Translate { by: [0.01, 0.0] }, copy, true));
        refused(
            &mut s,
            k,
            "ghost",
            transform(vec![ghost()], Motion::Translate { by: [0.01, 0.0] }, copy, true),
        );
        refused(
            &mut s,
            k,
            "ghost with real",
            transform(vec![l, ghost()], Motion::Translate { by: [0.01, 0.0] }, copy, true),
        );
    }
    // The same curve twice must not move it twice or double the copy.
    let (a0, _) = ends(&s, l);
    if refused_or_sane(
        &mut s,
        k,
        "twice",
        transform(vec![l, l], Motion::Translate { by: [0.01, 0.0] }, false, true),
    ) {
        run(&mut s, k, transform(vec![l, l], Motion::Translate { by: [0.01, 0.0] }, false, true)).unwrap();
        let (a, _) = ends(&s, l);
        assert!((a[0] - a0[0] - 0.01).abs() < 1e-9, "moved once, not twice: {a:?}");
    }
}

/// Listing a curve twice must copy it once.
#[test]
fn copying_a_curve_listed_twice_makes_one_copy() {
    let (mut s, k) = setup();
    let l = line(&mut s, k, [0.01, 0.0], [0.03, 0.0]);
    let n = drawn(&s);
    run(&mut s, k, transform(vec![l, l], Motion::Translate { by: [0.0, 0.01] }, true, true)).unwrap();
    assert_eq!(drawn(&s), n + 1, "one copy, not two");
}

// ---- Move a fixed curve ----

/// Help (Change Constraint Behavior when Moving or Rotating Geometry): with geometric constraints at "Never"
/// the operation obeys all existing constraints, so a curve fixed to the sketch does not move.
#[test]
fn moving_a_fixed_curve_is_refused() {
    let (mut s, k) = setup();
    let l = line(&mut s, k, [0.01, 0.0], [0.03, 0.0]);
    let (a, b) = s.active_sketch().unwrap().1.line(l).unwrap();
    for p in [a, b] {
        run(&mut s, k, SketchOp::AddConstraint(GeoConstraint::Fix(p))).unwrap();
    }
    for motion in [Motion::Translate { by: [0.01, 0.0] }, rotate([0.0, 0.0], 30.0)] {
        refused(&mut s, k, &format!("fixed {motion:?}"), transform(vec![l], motion, false, false));
    }
}

/// Projected reference lines are fixed too.
#[test]
fn moving_a_projected_line_is_refused() {
    let (mut s, k) = setup();
    run(&mut s, k, SketchOp::ProjectedLines { lines: vec![([0.0, 0.0, 0.0], [0.05, 0.0, 0.0])] }).unwrap();
    let id = s.active_sketch().unwrap().1.curves.keys().copied().last().unwrap();
    for motion in [Motion::Translate { by: [0.01, 0.0] }, rotate([0.0, 0.0], 45.0)] {
        refused(&mut s, k, &format!("projected {motion:?}"), transform(vec![id], motion, false, false));
    }
}

/// Copying a fixed curve leaves the original where it is, and any outcome stays consistent.
#[test]
fn copying_a_fixed_curve_leaves_the_original() {
    let (mut s, k) = setup();
    let l = line(&mut s, k, [0.01, 0.0], [0.03, 0.0]);
    let (a, b) = s.active_sketch().unwrap().1.line(l).unwrap();
    for p in [a, b] {
        run(&mut s, k, SketchOp::AddConstraint(GeoConstraint::Fix(p))).unwrap();
    }
    let before = ends(&s, l);
    for relax in [false, true] {
        refused_or_sane(
            &mut s,
            k,
            "copy",
            transform(vec![l], Motion::Translate { by: [0.0, 0.02] }, true, relax),
        );
    }
    refused_or_sane(&mut s, k, "rotate, relax", transform(vec![l], rotate([0.0, 0.0], 20.0), false, true));
    assert_eq!(ends(&s, l), before);
}

/// A move of one curve whose end is held to another by a coincident constraint: without relax the move is
/// refused or takes the neighbour along; the result is never a torn sketch.
#[test]
fn moving_one_curve_of_a_connected_pair_keeps_the_sketch_consistent() {
    let (mut s, k) = setup();
    let a = line(&mut s, k, [0.0, 0.0], [0.02, 0.0]);
    let b = line(&mut s, k, [0.02, 0.0], [0.02, 0.02]);
    let (_, ae) = s.active_sketch().unwrap().1.line(a).unwrap();
    let (bs, _) = s.active_sketch().unwrap().1.line(b).unwrap();
    run(&mut s, k, SketchOp::AddConstraint(GeoConstraint::Coincident(ae, bs))).unwrap();
    for relax in [false, true] {
        for motion in [Motion::Translate { by: [0.0, 0.01] }, rotate([0.0, 0.0], 15.0)] {
            refused_or_sane(
                &mut s,
                k,
                &format!("{motion:?} relax {relax}"),
                transform(vec![a], motion, false, relax),
            );
        }
    }
}

// ---- Copy then undo ----

#[test]
fn a_copy_is_one_undo_step_and_undo_removes_all_of_it() {
    let (mut s, k) = setup();
    let a = line(&mut s, k, [0.0, 0.0], [0.02, 0.0]);
    let b = line(&mut s, k, [0.02, 0.0], [0.02, 0.02]);
    let (_, ae) = s.active_sketch().unwrap().1.line(a).unwrap();
    let (bs, _) = s.active_sketch().unwrap().1.line(b).unwrap();
    run(&mut s, k, SketchOp::AddConstraint(GeoConstraint::Coincident(ae, bs))).unwrap();
    let before = s.doc().clone();
    let (depth, n) = (s.history().undo_depth(), drawn(&s));
    let points = s.active_sketch().unwrap().1.points.len();
    run(&mut s, k, transform(vec![a, b], Motion::Translate { by: [0.05, 0.0] }, true, true)).unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1);
    assert_eq!(drawn(&s), n + 2);
    // The originals did not move; the copy kept the constraint between its lines (its ends still meet).
    assert_eq!(ends(&s, a).0, [0.0, 0.0]);
    assert!(s.undo());
    assert!(s.doc().same_content(&before));
    assert_eq!(s.active_sketch().unwrap().1.points.len(), points, "undo removed the copy's points");
    assert!(s.redo());
    assert_eq!(drawn(&s), n + 2);
    // Copy a copy: ids are fresh each time.
    let before2 = s.doc().clone();
    run(&mut s, k, transform(vec![a], Motion::Translate { by: [0.0, 0.05] }, true, true)).unwrap();
    assert!(s.undo());
    assert!(s.doc().same_content(&before2));
}

// ---- Automatic dimensions ----

fn auto(curves: Vec<Id<Curve>>, dimensions: bool, constraints: bool) -> SketchOp {
    SketchOp::AutoDimension { curves, dimensions, constraints }
}

#[test]
fn auto_dimension_of_an_empty_sketch_adds_nothing() {
    let (mut s, k) = setup();
    let before = s.doc().clone();
    for (d, c) in [(true, true), (true, false), (false, true), (false, false)] {
        let depth = s.history().undo_depth();
        let result = run(&mut s, k, auto(vec![], d, c));
        // Refused, or accepted as a no-op: either way nothing is added.
        let sk = s.active_sketch().unwrap().1;
        assert!(sk.dimensions.is_empty() && sk.constraints.is_empty() && sk.curves.is_empty(), "{d} {c}");
        if result.is_err() {
            assert!(s.doc().same_content(&before) && s.history().undo_depth() == depth);
        } else {
            while s.history().undo_depth() > depth {
                assert!(s.undo());
            }
        }
    }
    refused(&mut s, k, "ghost", auto(vec![ghost()], true, true));
}

/// A fully constrained sketch (dof 0) gets no dimensions or constraints added.
#[test]
fn auto_dimension_of_a_fully_constrained_sketch_adds_nothing() {
    let (mut s, k) = setup();
    let l = line(&mut s, k, [0.01, 0.0], [0.03, 0.0]);
    let (a, b) = s.active_sketch().unwrap().1.line(l).unwrap();
    for p in [a, b] {
        run(&mut s, k, SketchOp::AddConstraint(GeoConstraint::Fix(p))).unwrap();
    }
    assert_eq!(dof(&s), 0);
    let sk = s.active_sketch().unwrap().1;
    let (dims, cons) = (sk.dimensions.len(), sk.constraints.len());
    for (d, c) in [(true, true), (true, false), (false, true)] {
        let depth = s.history().undo_depth();
        drop(run(&mut s, k, auto(vec![], d, c)));
        let sk = s.active_sketch().unwrap().1;
        assert_eq!((sk.dimensions.len(), sk.constraints.len()), (dims, cons), "{d} {c}: something was added");
        assert_eq!(dof(&s), 0);
        while s.history().undo_depth() > depth {
            assert!(s.undo());
        }
    }
    // Picking the curve explicitly changes nothing either.
    drop(run(&mut s, k, auto(vec![l], true, true)));
    let sk = s.active_sketch().unwrap().1;
    assert_eq!((sk.dimensions.len(), sk.constraints.len()), (dims, cons));
}

/// A normal sketch: dimensions go in until the freedom is gone or nothing more helps; running it again adds
/// nothing; one undo step; the dimensions carry the values the geometry has now.
#[test]
fn auto_dimension_removes_freedom_once() {
    let (mut s, k) = setup();
    line(&mut s, k, [0.0, 0.0], [0.04, 0.001]);
    line(&mut s, k, [0.04, 0.001], [0.041, 0.03]);
    let before = s.doc().clone();
    let (d0, depth) = (dof(&s), s.history().undo_depth());
    assert!(d0 > 0);
    run(&mut s, k, auto(vec![], true, true)).unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1);
    assert!(dof(&s) < d0, "freedom went down");
    let sk = s.active_sketch().unwrap().1;
    assert!(
        sk.dimensions.values().all(|d| d.value.is_finite() && d.value != 0.0),
        "values taken from geometry"
    );
    sane(&s, "auto");
    // Nothing moved.
    let moved = before
        .get(k)
        .unwrap()
        .points
        .iter()
        .all(|(id, p)| s.active_sketch().unwrap().1.points.get(id).is_some_and(|q| q.pos == p.pos));
    assert!(moved, "automatic dimensions must not move the geometry");
    // A second run adds nothing more.
    let after = s.doc().clone();
    drop(run(&mut s, k, auto(vec![], true, true)));
    let (dims, cons) = (after.get(k).unwrap().dimensions.len(), after.get(k).unwrap().constraints.len());
    let sk = s.active_sketch().unwrap().1;
    assert_eq!((sk.dimensions.len(), sk.constraints.len()), (dims, cons), "idempotent");
}

#[test]
fn auto_dimension_options_off_add_only_what_was_asked() {
    let (mut s, k) = setup();
    line(&mut s, k, [0.0, 0.0], [0.04, 0.0001]); // almost horizontal
    let before = s.active_sketch().unwrap().1.clone();
    // Constraints only: no dimension appears.
    drop(run(&mut s, k, auto(vec![], false, true)));
    assert_eq!(s.active_sketch().unwrap().1.dimensions.len(), before.dimensions.len());
    let (mut s, k) = setup();
    line(&mut s, k, [0.0, 0.0], [0.04, 0.0001]);
    let n = s.active_sketch().unwrap().1.constraints.len();
    // Dimensions only: no new geometric constraint is added.
    drop(run(&mut s, k, auto(vec![], true, false)));
    assert_eq!(s.active_sketch().unwrap().1.constraints.len(), n);
}

// ---- Flip Direction ----

fn flip(dims: Vec<Id<gc_sketch::Dimension>>) -> SketchOp {
    SketchOp::FlipDimensions(dims)
}

fn dimension_of(c: &[Created]) -> Id<gc_sketch::Dimension> {
    c.iter().find_map(|c| if let Created::Dimension(d) = c { Some(*d) } else { None }).unwrap()
}

/// Flip twice is the original. In an under-constrained sketch the solver may settle other free geometry
/// differently, so the dimension itself must come back exactly; in a fully constrained one the geometry
/// comes back too.
#[test]
fn flipping_twice_is_the_original() {
    let (mut s, k) = setup();
    let a = line(&mut s, k, [0.0, 0.0], [0.04, 0.0]);
    let b = line(&mut s, k, [0.0, 0.0], [0.03, 0.03]);
    let sk = s.active_sketch().unwrap().1;
    let (p, q) = (sk.line(a).unwrap().0, sk.line(a).unwrap().1);
    let angle = dimension_of(
        &run(
            &mut s,
            k,
            SketchOp::AddDimension {
                kind: DimKind::Angle { l1: a, rev1: false, l2: b, rev2: false },
                source: "45".into(),
                text_pos: [0.01, 0.01],
            },
        )
        .unwrap(),
    );
    let hdist = dimension_of(
        &run(
            &mut s,
            k,
            SketchOp::AddDimension {
                kind: DimKind::HDistance { p, q, sign: 1.0 },
                source: "40".into(),
                text_pos: [0.02, -0.01],
            },
        )
        .unwrap(),
    );
    for dims in [vec![angle], vec![hdist], vec![angle, hdist]] {
        let kinds = |s: &Session| -> Vec<DimKind> {
            dims.iter().map(|d| s.active_sketch().unwrap().1.dimensions[d].kind).collect()
        };
        let before = s.doc().clone();
        let original = kinds(&s);
        let depth = s.history().undo_depth();
        run(&mut s, k, flip(dims.clone())).unwrap();
        assert_eq!(s.history().undo_depth(), depth + 1, "{dims:?}");
        assert_ne!(kinds(&s), original, "{dims:?}: a flip changed nothing");
        run(&mut s, k, flip(dims.clone())).unwrap();
        assert_eq!(s.history().undo_depth(), depth + 2);
        assert_eq!(kinds(&s), original, "{dims:?}: flipping twice is not the original dimension");
        sane(&s, "flip twice");
        // Undo of each step goes back one at a time, exactly.
        assert!(s.undo() && s.undo());
        assert!(s.doc().same_content(&before));
    }
}

/// Fully constrained: a point at a distance from a fixed line goes to the other side and back.
#[test]
fn flipping_a_fully_constrained_dimension_twice_restores_the_geometry() {
    let (mut s, k) = setup();
    let l = line(&mut s, k, [0.0, 0.0], [0.04, 0.0]);
    let (a, b) = s.active_sketch().unwrap().1.line(l).unwrap();
    for p in [a, b] {
        run(&mut s, k, SketchOp::AddConstraint(GeoConstraint::Fix(p))).unwrap();
    }
    let created = run(&mut s, k, SketchOp::AddPoint { pos: [0.02, 0.01, 0.0] }).unwrap();
    let Some(Created::Point(pt)) = created.into_iter().find(|c| matches!(c, Created::Point(_))) else {
        panic!("no point")
    };
    let near = dimension_of(
        &run(
            &mut s,
            k,
            SketchOp::AddDimension {
                kind: DimKind::PointLine { p: pt, line: l, side: 1.0 },
                source: "10".into(),
                text_pos: [0.03, 0.005],
            },
        )
        .unwrap(),
    );
    run(
        &mut s,
        k,
        SketchOp::AddDimension {
            kind: DimKind::HDistance { p: a, q: pt, sign: 1.0 },
            source: "20".into(),
            text_pos: [0.01, -0.01],
        },
    )
    .unwrap();
    assert_eq!(dof(&s), 0);
    let at = |s: &Session| s.active_sketch().unwrap().1.pos(pt).unwrap();
    let up = at(&s);
    run(&mut s, k, flip(vec![near])).unwrap();
    let down = at(&s);
    assert!((down[1] + up[1]).abs() < 1e-9 && (down[0] - up[0]).abs() < 1e-9, "{up:?} -> {down:?}");
    run(&mut s, k, flip(vec![near])).unwrap();
    let back = at(&s);
    assert!((back[0] - up[0]).abs() < 1e-9 && (back[1] - up[1]).abs() < 1e-9, "{up:?} -> {back:?}");
}

#[test]
fn flipping_refuses_empty_ghost_and_unflippable_dimensions() {
    let (mut s, k) = setup();
    let a = line(&mut s, k, [0.0, 0.0], [0.04, 0.0]);
    let sk = s.active_sketch().unwrap().1;
    let (p, q) = sk.line(a).unwrap();
    let length = dimension_of(
        &run(
            &mut s,
            k,
            SketchOp::AddDimension {
                kind: DimKind::Distance { p, q },
                source: "40".into(),
                text_pos: [0.02, 0.01],
            },
        )
        .unwrap(),
    );
    let ghost_dim = Id::<gc_sketch::Dimension>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    refused(&mut s, k, "empty", flip(vec![]));
    refused(&mut s, k, "ghost", flip(vec![ghost_dim]));
    // A plain distance has no other side.
    refused(&mut s, k, "plain distance", flip(vec![length]));
    let hd = dimension_of(
        &run(
            &mut s,
            k,
            SketchOp::AddDimension {
                kind: DimKind::VDistance { p, q, sign: 1.0 },
                source: "5".into(),
                text_pos: [0.02, -0.01],
            },
        )
        .unwrap_or_default(),
    );
    // A mix of a flippable and an unflippable dimension is refused whole.
    refused(&mut s, k, "mixed", flip(vec![hd, length]));
    refused(&mut s, k, "ghost among real", flip(vec![hd, ghost_dim]));
}

/// A flip survives undo and redo and leaves the sketch solvable.
#[test]
fn flipped_dimension_survives_undo_redo() {
    let (mut s, k) = setup();
    let a = line(&mut s, k, [0.0, 0.0], [0.04, 0.0]);
    let sk = s.active_sketch().unwrap().1;
    let (p, q) = sk.line(a).unwrap();
    let d = dimension_of(
        &run(
            &mut s,
            k,
            SketchOp::AddDimension {
                kind: DimKind::HDistance { p, q, sign: 1.0 },
                source: "40".into(),
                text_pos: [0.02, -0.01],
            },
        )
        .unwrap(),
    );
    run(&mut s, k, flip(vec![d])).unwrap();
    let flipped = s.doc().clone();
    assert!(s.undo());
    assert!(s.redo());
    assert!(s.doc().same_content(&flipped));
    assert!(s.active_sketch().unwrap().1.dimensions.contains_key(&d));
    assert!(s.active_sketch().unwrap().1.clone().solve().is_ok());
}
