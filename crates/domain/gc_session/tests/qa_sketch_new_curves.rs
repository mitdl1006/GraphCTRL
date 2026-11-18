//! QA boundary tests for the newest sketch features: Fix on curves, Symmetric on curve pairs, line–arc and
//! arc–arc distance dimensions, line–ellipse Tangent, 3D-sketch splines and 3D Bend.
//! Inventor 2026 help, under <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/>:
//! constraints (Fix, Symmetric, Tangent) GUID-A85A7A30-7D81-4C75-8769-CAD034EEA930, dimensions
//! GUID-026FE8D3-AFBB-4834-BC18-73D9CFF78185, lines/arcs/Bend GUID-4F861674-F74C-474E-869A-4C190ED6EC4B,
//! 3D sketch GUID-37BA1DCE-6BC4-41CD-A36D-577F31B304FC, splines GUID-DB15C479-66F2-456A-9E54-40A60F83DC92.
//! Every refused edit leaves the document and the history untouched; every accepted one is one undo step
//! that restores exactly (redo repeats), with finite points and a sketch that still solves.

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
use gc_sketch::{Curve, DimKind, GeoConstraint, Item};

type K = Id<gc_doc::Sketch>;

fn run(s: &mut Session, k: K, op: SketchOp) -> Result<Vec<Created>, gc_doc::EditError> {
    s.execute(&Command::Sketch { sketch: k, op })
}

fn curve_of(created: &[Created]) -> Id<Curve> {
    created.iter().find_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None }).unwrap()
}

fn line(s: &mut Session, k: K, a: [f64; 3], b: [f64; 3]) -> Id<Curve> {
    let op =
        SketchOp::AddLine { a, b, snap_a: EndSnap::default(), snap_b: EndSnap::default(), horizontal: None };
    curve_of(&run(s, k, op).unwrap())
}

fn circle(s: &mut Session, k: K, c: [f64; 2], r: f64) -> Id<Curve> {
    let op = SketchOp::AddCircle {
        centre: [c[0], c[1], 0.0],
        radius: r,
        centre_snap: EndSnap::default(),
        rim_snap: EndSnap::default(),
    };
    curve_of(&run(s, k, op).unwrap())
}

fn ellipse(s: &mut Session, k: K, c: [f64; 2], major: f64, minor: f64) -> Id<Curve> {
    let op = SketchOp::AddEllipse {
        centre: [c[0], c[1], 0.0],
        major: [c[0] + major, c[1], 0.0],
        minor,
        centre_snap: EndSnap::default(),
    };
    curve_of(&run(s, k, op).unwrap())
}

fn sane(s: &Session, what: &str) {
    let sk = s.active_sketch().unwrap().1;
    assert!(sk.points.values().all(|p| p.pos.iter().all(|v| v.is_finite())), "{what}: a point is not finite");
    assert!(sk.clone().solve().is_ok(), "{what}: the sketch no longer solves");
}

/// Refused with no trace, or one undo step that undoes exactly and leaves a sane sketch (then redone).
fn step(s: &mut Session, k: K, what: &str, op: SketchOp) -> bool {
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    match run(s, k, op) {
        Err(_) => {
            assert!(s.doc().same_content(&before), "{what} changed the document");
            assert_eq!(s.history().undo_depth(), depth, "{what}");
            false
        }
        Ok(_) => {
            assert_eq!(s.history().undo_depth(), depth + 1, "{what}: one undo step");
            sane(s, what);
            let after = s.doc().clone();
            assert!(s.undo());
            assert!(s.doc().same_content(&before), "{what}: undo restores");
            assert!(s.redo());
            assert!(s.doc().same_content(&after), "{what}: redo repeats");
            true
        }
    }
}

fn refused(s: &mut Session, k: K, what: &str, op: SketchOp) {
    assert!(!step(s, k, what, op), "{what} was accepted");
}

fn ghost() -> Id<Curve> {
    Id::<Curve>::from_raw(std::num::NonZeroU64::new(9999).unwrap())
}

fn fix(c: Id<Curve>) -> SketchOp {
    SketchOp::AddConstraint(GeoConstraint::FixCurve(c))
}

fn dim(kind: DimKind, v: &str) -> SketchOp {
    SketchOp::AddDimension { kind, source: v.into(), text_pos: [0.0, 0.0] }
}

fn m(x: f64, y: f64) -> [f64; 3] {
    [x * 1e-3, y * 1e-3, 0.0]
}

// ---- Fix on curves ----

#[test]
fn fix_curve_boundaries() {
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    let l = line(&mut s, k, m(0.0, 0.0), m(100.0, 20.0));
    refused(&mut s, k, "fix a missing curve", fix(ghost()));
    assert!(step(&mut s, k, "fix a line", fix(l)));
    refused(&mut s, k, "fix the same line twice", fix(l));
    // Moving or deleting the fixed curve.
    step(
        &mut s,
        k,
        "translate a fixed line (no relax)",
        SketchOp::Transform {
            curves: vec![l],
            motion: Motion::Translate { by: [0.01, 0.0] },
            copy: false,
            relax: false,
        },
    );
    step(
        &mut s,
        k,
        "translate a fixed line (relax)",
        SketchOp::Transform {
            curves: vec![l],
            motion: Motion::Translate { by: [0.01, 0.0] },
            copy: false,
            relax: true,
        },
    );
    step(
        &mut s,
        k,
        "copy a fixed line",
        SketchOp::Transform {
            curves: vec![l],
            motion: Motion::Translate { by: [0.0, 0.03] },
            copy: true,
            relax: false,
        },
    );
    // A zero-length line and a zero-radius circle cannot be fixed, or are fixed without a trace of NaN.
    let tiny = line(&mut s, k, m(50.0, 50.0), [0.05 + 1e-12, 0.05, 0.0]);
    step(&mut s, k, "fix a nearly zero-length line", fix(tiny));
    // Fix of every kind, then delete each: the constraint goes with the curve.
    let c = circle(&mut s, k, [0.0, -0.05], 0.01);
    let e = ellipse(&mut s, k, [0.1, -0.05], 0.03, 0.01);
    for (what, id) in [("circle", c), ("ellipse", e)] {
        assert!(step(&mut s, k, &format!("fix {what}"), fix(id)));
    }
    for (what, id) in [("circle", c), ("ellipse", e), ("line", l)] {
        run(&mut s, k, fix(id)).ok();
        assert!(step(&mut s, k, &format!("delete fixed {what}"), SketchOp::Delete(vec![Item::Curve(id)])));
    }
    let sk = s.active_sketch().unwrap().1;
    for c in sk.constraints.values() {
        if let GeoConstraint::FixCurve(id) = c {
            assert!(sk.curves.contains_key(id), "a Fix constraint outlived its curve");
        }
    }
    // Offset, mirror, and trim a fixed curve.
    let a = line(&mut s, k, m(-50.0, 40.0), m(50.0, 40.0));
    run(&mut s, k, fix(a)).unwrap();
    step(
        &mut s,
        k,
        "offset a fixed line",
        SketchOp::Offset {
            curves: vec![a],
            distance: 0.01,
            loop_select: false,
            constrain: true,
            picked: None,
        },
    );
    let cut = line(&mut s, k, m(0.0, 30.0), m(0.0, 60.0));
    step(&mut s, k, "trim a fixed line", SketchOp::Trim { curve: a, at: [0.03, 0.04] });
    step(&mut s, k, "mirror about a fixed line", SketchOp::Mirror { curves: vec![cut], axis: a });
}

// ---- Symmetric on curve pairs ----

#[test]
fn symmetric_curve_boundaries() {
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    let axis = line(&mut s, k, m(0.0, -100.0), m(0.0, 100.0));
    run(&mut s, k, fix(axis)).unwrap();
    let a = line(&mut s, k, m(-50.0, 0.0), m(-20.0, 40.0));
    let b = line(&mut s, k, m(25.0, 35.0), m(55.0, 5.0));
    let sym = |first, second, axis| SketchOp::SymmetricCurves { first, second, axis };
    refused(&mut s, k, "same curve twice", sym(a, a, axis));
    refused(&mut s, k, "axis is a pair member", sym(a, b, a));
    refused(&mut s, k, "ghost first", sym(ghost(), b, axis));
    refused(&mut s, k, "ghost second", sym(a, ghost(), axis));
    refused(&mut s, k, "ghost axis", sym(a, b, ghost()));
    let c = circle(&mut s, k, [-0.05, -0.05], 0.01);
    refused(&mut s, k, "line and circle", sym(a, c, axis));
    let circ_axis = circle(&mut s, k, [0.0, 0.0], 0.02);
    refused(&mut s, k, "circle as the axis", sym(a, b, circ_axis));
    assert!(step(&mut s, k, "two lines", sym(a, b, axis)));
    // The same pair again is already symmetric: refused with no trace (not a duplicate constraint).
    refused(&mut s, k, "the same pair again", sym(a, b, axis));
    refused(&mut s, k, "the reversed pair", sym(b, a, axis));
    // Circles of different fixed radii cannot be symmetric: conflict is refused with no trace.
    let c1 = circle(&mut s, k, [-0.06, 0.06], 0.01);
    let c2 = circle(&mut s, k, [0.06, 0.06], 0.02);
    run(&mut s, k, fix(c1)).unwrap();
    run(&mut s, k, fix(c2)).unwrap();
    refused(&mut s, k, "circles of different fixed size", sym(c1, c2, axis));
    // A curve lying on the axis itself.
    let on_axis = line(&mut s, k, m(0.0, -10.0), m(0.0, 10.0));
    step(&mut s, k, "a curve on the axis against itself's twin", sym(on_axis, b, axis));
    // A zero-length axis.
    let dot = line(&mut s, k, m(10.0, 10.0), [0.01 + 1e-13, 0.01, 0.0]);
    step(&mut s, k, "a nearly zero-length axis", sym(a, b, dot));
    // Delete a member: the constraints go with it, nothing dangles.
    assert!(step(&mut s, k, "delete a symmetric member", SketchOp::Delete(vec![Item::Curve(a)])));
    sane(&s, "after delete");
}

// ---- Distances to arcs ----

#[test]
fn line_arc_and_arc_arc_dimension_boundaries() {
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    let l = line(&mut s, k, m(-100.0, 0.0), m(100.0, 0.0));
    run(&mut s, k, fix(l)).unwrap();
    let c1 = circle(&mut s, k, [0.0, 0.05], 0.02);
    let c2 = circle(&mut s, k, [0.1, 0.05], 0.01);
    let la = |line, arc, side| DimKind::LineArc { line, arc, side };
    let aa = |a, b, external| DimKind::ArcArc { a, b, external };
    refused(&mut s, k, "ghost arc", dim(la(l, ghost(), 1.0), "10"));
    refused(&mut s, k, "ghost line", dim(la(ghost(), c1, 1.0), "10"));
    refused(&mut s, k, "arc as the line", dim(la(c1, c2, 1.0), "10"));
    refused(&mut s, k, "line as the arc", dim(la(l, l, 1.0), "10"));
    refused(&mut s, k, "same circle twice", dim(aa(c1, c1, true), "10"));
    refused(&mut s, k, "ghost circle pair", dim(aa(c1, ghost(), true), "10"));
    refused(&mut s, k, "line in an arc-arc", dim(aa(c1, l, true), "10"));
    for v in ["0", "-5", "nan", "inf", "1e308", "1e-300", "abc", ""] {
        step(&mut s, k, &format!("line-arc value {v:?}"), dim(la(l, c1, 1.0), v));
        step(&mut s, k, &format!("arc-arc value {v:?}"), dim(aa(c1, c2, true), v));
    }
    // The wrong side: a circle left of the line asked for on the right (a signed side the solver can hold).
    step(&mut s, k, "wrong side", dim(la(l, c1, -1.0), "10"));
    // Overlapping circles measured as apart / inside.
    let c3 = circle(&mut s, k, [0.01, 0.05], 0.03);
    step(&mut s, k, "overlapping, external", dim(aa(c1, c3, true), "5"));
    step(&mut s, k, "overlapping, inside", dim(aa(c1, c3, false), "5"));
    // Concentric circles.
    let c4 = circle(&mut s, k, [0.0, 0.05], 0.04);
    step(&mut s, k, "concentric, inside", dim(aa(c1, c4, false), "10"));
    step(&mut s, k, "concentric, apart", dim(aa(c1, c4, true), "10"));
    // A circle crossing the line.
    let c5 = circle(&mut s, k, [0.0, 0.01], 0.02);
    step(&mut s, k, "circle crossing the line", dim(la(l, c5, 1.0), "5"));
    // Arcs, not just circles.
    let arc = curve_of(
        &run(&mut s, k, SketchOp::AddArc { centre: m(0.0, 80.0), start: m(20.0, 80.0), end: m(0.0, 100.0) })
            .unwrap(),
    );
    step(&mut s, k, "line to an arc", dim(la(l, arc, 1.0), "40"));
    step(&mut s, k, "circle to an arc", dim(aa(c1, arc, true), "30"));
    // Deleting a dimensioned circle takes the dimension along.
    assert!(step(&mut s, k, "delete a dimensioned circle", SketchOp::Delete(vec![Item::Curve(c1)])));
    let sk = s.active_sketch().unwrap().1;
    for d in sk.dimensions.values() {
        if let DimKind::LineArc { arc, .. } = d.kind {
            assert!(sk.curves.contains_key(&arc), "a dimension outlived its curve");
        }
    }
}

// ---- Line–ellipse tangent ----

#[test]
fn line_ellipse_tangent_boundaries() {
    use gc_sketch::GeoConstraint::Tangent;
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    let e = ellipse(&mut s, k, [0.0, 0.0], 0.04, 0.02);
    run(&mut s, k, fix(e)).unwrap();
    let tan = |a, b, side| SketchOp::AddConstraint(Tangent { a, b, side, external: true });
    let l = line(&mut s, k, m(-50.0, 50.0), m(50.0, 55.0));
    refused(&mut s, k, "ghost", tan(l, ghost(), 1.0));
    refused(&mut s, k, "ellipse with itself", tan(e, e, 1.0));
    for side in [1.0, -1.0, 0.0, f64::NAN] {
        step(&mut s, k, &format!("tangent side {side}"), tan(l, e, side));
    }
    // A second, equal constraint is redundant; a line through the centre cannot also touch.
    refused(&mut s, k, "tangent again", tan(l, e, -1.0));
    let through = line(&mut s, k, m(-60.0, 0.0), m(60.0, 0.0));
    let start = s.active_sketch().unwrap().1.line(through).unwrap().0;
    run(&mut s, k, SketchOp::AddConstraint(GeoConstraint::PointOnCurve(start, e))).ok();
    step(&mut s, k, "tangent for a line through the ellipse", tan(through, e, 1.0));
    // A line fixed across the ellipse: conflict is refused.
    let cross = line(&mut s, k, m(-60.0, 5.0), m(60.0, 5.0));
    run(&mut s, k, fix(cross)).unwrap();
    // Observation (circles behave the same): two fixed curves that cannot touch are accepted, the solver
    // reports the constraints as degenerate; either way no trace on refusal, one undo step on acceptance.
    step(&mut s, k, "fixed crossing line made tangent", tan(cross, e, 1.0));
    // A zero-length line.
    let dot = line(&mut s, k, m(10.0, 30.0), [0.01 + 1e-13, 0.03, 0.0]);
    step(&mut s, k, "tangent for a zero-length line", tan(dot, e, 1.0));
    // A very thin, very long, and a turned ellipse.
    for (what, major, minor) in [("thin", 0.5, 1e-4), ("round", 0.02, 0.02), ("tiny", 1e-4, 5e-5)] {
        let f = ellipse(&mut s, k, [0.5, 0.5], major, minor);
        let t = line(&mut s, k, m(400.0, 700.0), m(600.0, 710.0));
        step(&mut s, k, &format!("{what} ellipse tangent"), tan(t, f, 1.0));
    }
    // Delete the ellipse: the tangent goes with it.
    assert!(step(&mut s, k, "delete the ellipse", SketchOp::Delete(vec![Item::Curve(e)])));
    sane(&s, "after delete");
}

// ---- 3D splines ----

#[test]
fn spline_3d_boundaries() {
    let mut s = Session::new();
    let k = s.create_sketch_3d().unwrap();
    let sp = |points: Vec<[f64; 3]>| SketchOp::AddSpline { points, snaps: Vec::new() };
    refused(&mut s, k, "no points", sp(vec![]));
    refused(&mut s, k, "one point", sp(vec![[0.0; 3]]));
    refused(&mut s, k, "two equal points", sp(vec![[0.1, 0.1, 0.1], [0.1, 0.1, 0.1]]));
    refused(&mut s, k, "NaN z", sp(vec![[0.0; 3], [0.1, 0.0, f64::NAN]]));
    refused(&mut s, k, "inf", sp(vec![[0.0; 3], [f64::INFINITY, 0.0, 0.0]]));
    // Only z differs (a 2D implementation would drop it).
    assert!(step(&mut s, k, "points differing only in z", sp(vec![[0.0; 3], [0.0, 0.0, 0.2]])));
    step(
        &mut s,
        k,
        "repeated middle point",
        sp(vec![[0.0; 3], [0.1, 0.1, 0.1], [0.1, 0.1, 0.1], [0.2, 0.0, 0.3]]),
    );
    step(&mut s, k, "huge", sp(vec![[0.0; 3], [1e9, 1e9, 1e9], [2e9, 0.0, 1e9]]));
    step(&mut s, k, "tiny", sp(vec![[0.0; 3], [1e-9, 1e-9, 1e-9], [2e-9, 0.0, 1e-9]]));
    let made =
        run(&mut s, k, sp(vec![[0.0; 3], [0.1, 0.05, 0.1], [0.2, 0.0, 0.3], [0.3, 0.1, 0.35]])).unwrap();
    let spline = curve_of(&made);
    // Ops that act on a planar spline, on a spatial one.
    let (l, _) = {
        let l = line(&mut s, k, [0.1, -0.1, 0.1], [0.1, 0.2, 0.1]);
        (l, ())
    };
    step(&mut s, k, "trim a 3D spline", SketchOp::Trim { curve: spline, at: [0.1, 0.05] });
    step(&mut s, k, "split a 3D spline", SketchOp::Split { curve: spline, at: [0.1, 0.05] });
    step(&mut s, k, "extend a line to a 3D spline", SketchOp::Extend { curve: l, at: [0.1, 0.2] });
    step(
        &mut s,
        k,
        "offset a 3D spline",
        SketchOp::Offset {
            curves: vec![spline],
            distance: 0.01,
            loop_select: false,
            constrain: false,
            picked: None,
        },
    );
    step(&mut s, k, "mirror a 3D spline about a line", SketchOp::Mirror { curves: vec![spline], axis: l });
    step(&mut s, k, "close a 3D spline", SketchOp::CloseSpline { curve: spline });
    step(
        &mut s,
        k,
        "insert point in a 3D spline",
        SketchOp::InsertSplinePoint { curve: spline, at: [0.1, 0.05] },
    );
    step(&mut s, k, "fix a 3D spline", fix(spline));
    step(&mut s, k, "delete a 3D spline", SketchOp::Delete(vec![Item::Curve(spline)]));
    // Save, reopen, save again.
    s.finish_sketch();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("spline3d.gcp");
    s.save_as(&path).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert!(t.doc().same_content(s.doc()), "the reopened document differs");
    let again = dir.path().join("again.gcp");
    t.save_as(&again).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), std::fs::read(&again).unwrap(), "saving twice differs");
}

// ---- 3D Bend ----

#[test]
fn bend_3d_boundaries() {
    let mut s = Session::new();
    let k = s.create_sketch_3d().unwrap();
    let corner = [0.1, 0.1, 0.1];
    let l1 = line(&mut s, k, [0.0; 3], corner);
    let l2 = line(&mut s, k, corner, [0.3, 0.1, 0.25]);
    let bend = |a, b, r: &str| SketchOp::Bend { lines: [a, b], radius: r.into() };
    refused(&mut s, k, "same line twice", bend(l1, l1, "10 mm"));
    refused(&mut s, k, "ghost line", bend(l1, ghost(), "10 mm"));
    for r in ["0", "-5 mm", "nan", "inf", "", "abc", "1e308 mm", "1e-300 mm"] {
        refused(&mut s, k, &format!("radius {r:?}"), bend(l1, l2, r));
    }
    refused(&mut s, k, "radius too large", bend(l1, l2, "5 m"));
    assert!(step(&mut s, k, "a good bend", bend(l1, l2, "20 mm")));
    // A bend of lines already bent: the corner is gone, so the second one has no corner to round.
    let before = s.doc().clone();
    let again = run(&mut s, k, bend(l1, l2, "20 mm"));
    if again.is_err() {
        assert!(s.doc().same_content(&before), "a refused second bend left a trace");
    } else {
        sane(&s, "second bend accepted");
        assert!(s.undo());
        assert!(s.doc().same_content(&before));
    }
    // Lines meeting end to end in line, and head on (reversed).
    let a = line(&mut s, k, [0.5, 0.0, 0.0], [0.6, 0.0, 0.0]);
    let b = line(&mut s, k, [0.6, 0.0, 0.0], [0.7, 0.0, 0.0]);
    refused(&mut s, k, "collinear", bend(a, b, "5 mm"));
    let back = line(&mut s, k, [0.6, 0.0, 0.0], [0.5, 0.0, 0.0]);
    refused(&mut s, k, "doubled back", bend(a, back, "5 mm"));
    // Lines meeting end to end where the direction is the other way round (head to head).
    let p = line(&mut s, k, [0.0, 0.8, 0.0], [0.1, 0.8, 0.0]);
    let q = line(&mut s, k, [0.1, 0.9, 0.0], [0.1, 0.8, 0.0]);
    step(&mut s, k, "head to head", bend(p, q, "5 mm"));
    // A near-straight corner.
    let n1 = line(&mut s, k, [0.0, 1.0, 0.0], [0.1, 1.0, 0.0]);
    let n2 = line(&mut s, k, [0.1, 1.0, 0.0], [0.2, 1.0 + 1e-9, 0.0]);
    step(&mut s, k, "near straight", bend(n1, n2, "5 mm"));
    // A bend, then delete one line: no dangling dimension.
    assert!(step(&mut s, k, "delete a bent line", SketchOp::Delete(vec![Item::Curve(l1)])));
    let sk = s.active_sketch().unwrap().1;
    for d in sk.dimensions.values() {
        if let DimKind::Radius(c) = d.kind {
            assert!(sk.curves.contains_key(&c), "a radius dimension outlived its arc");
        }
    }
    // Save and reopen.
    s.finish_sketch();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bend.gcp");
    s.save_as(&path).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert!(t.doc().same_content(s.doc()), "the reopened document differs");
}
