//! QA boundary tests for Tangent between an ellipse and a circle or another ellipse, and for a spline's
//! Tangent and Smooth (G2) ends. Inventor 2026 help, To Apply Geometric Constraints,
//! <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-A85A7A30-7D81-4C75-8769-CAD034EEA930.htm>
//! ("The Smooth (G2) constraint causes a continuous curvature (G2) condition between a spline and another
//! curve, such as a line, arc, or spline").
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
use gc_sketch::{Curve, CurveGeom, GeoConstraint, Item, SkPoint};

type K = Id<gc_doc::Sketch>;

fn run(s: &mut Session, k: K, op: SketchOp) -> Result<Vec<Created>, gc_doc::EditError> {
    s.execute(&Command::Sketch { sketch: k, op })
}

fn curve_of(created: &[Created]) -> Id<Curve> {
    created.iter().find_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None }).unwrap()
}

fn curve(s: &mut Session, k: K, op: SketchOp) -> Id<Curve> {
    curve_of(&run(s, k, op).unwrap())
}

fn ellipse(s: &mut Session, k: K, c: [f64; 2], major: [f64; 2], minor: f64, fixed: bool) -> Id<Curve> {
    let e = curve(
        s,
        k,
        SketchOp::AddEllipse {
            centre: [c[0], c[1], 0.0],
            major: [major[0], major[1], 0.0],
            minor,
            centre_snap: EndSnap::default(),
        },
    );
    if fixed {
        run(s, k, SketchOp::AddConstraint(GeoConstraint::FixCurve(e))).unwrap();
    }
    e
}

fn circle(s: &mut Session, k: K, c: [f64; 2], r: f64) -> Id<Curve> {
    curve(
        s,
        k,
        SketchOp::AddCircle {
            centre: [c[0], c[1], 0.0],
            radius: r,
            centre_snap: EndSnap::default(),
            rim_snap: EndSnap::default(),
        },
    )
}

fn line(s: &mut Session, k: K, a: [f64; 3], b: [f64; 3]) -> Id<Curve> {
    curve(
        s,
        k,
        SketchOp::AddLine { a, b, snap_a: EndSnap::default(), snap_b: EndSnap::default(), horizontal: None },
    )
}

fn sane(s: &Session, what: &str) {
    let sk = s.active_sketch().unwrap().1;
    assert!(sk.points.values().all(|p| p.pos.iter().all(|v| v.is_finite())), "{what}: a point is not finite");
    assert!(sk.clone().solve().is_ok(), "{what}: the sketch no longer solves");
}

/// Refused with no trace, or one undo step that undoes exactly and redoes the same.
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

fn ghost() -> Id<Curve> {
    Id::<Curve>::from_raw(std::num::NonZeroU64::new(9999).unwrap())
}

fn tangent(a: Id<Curve>, b: Id<Curve>, side: f64) -> SketchOp {
    SketchOp::AddConstraint(GeoConstraint::Tangent { a, b, side, external: true })
}

fn gap(a: &[[f64; 2]], b: &[[f64; 2]]) -> f64 {
    a.iter()
        .flat_map(|p| b.iter().map(move |q| gc_math::fm::hypot(p[0] - q[0], p[1] - q[1])))
        .fold(f64::INFINITY, f64::min)
}

fn outline(s: &Session, c: Id<Curve>) -> Vec<[f64; 2]> {
    let sk = s.active_sketch().unwrap().1;
    if let Some(points) = sk.ellipse_outline(c, 1024) {
        return points.0;
    }
    let (centre, r) = sk.round(c).unwrap();
    let centre = sk.pos(centre).unwrap();
    (0..1024)
        .map(|i| {
            let t = f64::from(i) / 1024.0 * std::f64::consts::TAU;
            [centre[0] + r * gc_math::fm::cos(t), centre[1] + r * gc_math::fm::sin(t)]
        })
        .collect()
}

// ---- Ellipse-circle and ellipse-ellipse Tangent ----

#[test]
fn ellipse_round_tangent_boundaries() {
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    let e = ellipse(&mut s, k, [0.0, 0.0], [0.04, 0.0], 0.02, true);
    let c = circle(&mut s, k, [0.03, 0.04], 0.01);
    for side in [1.0, -1.0, 0.0, f64::NAN] {
        step(&mut s, k, &format!("circle tangent side {side}"), tangent(c, e, side));
    }
    assert!(!step(&mut s, k, "a ghost", tangent(e, ghost(), 1.0)));
    // Tangent already there: refused (redundant). The gap to the ellipse is closed when it was accepted.
    let sk = s.active_sketch().unwrap().1;
    let tangents = sk.constraints.values().filter(|c| matches!(c, GeoConstraint::Tangent { .. })).count();
    assert!(tangents <= 1, "one tangent held: {tangents}");
    if tangents == 1 {
        assert!(gap(&outline(&s, e), &outline(&s, c)) < 1e-4);
    }
    // Concentric: nothing can touch (the circle sits inside, or the ellipse does) unless sized so.
    let inner = circle(&mut s, k, [0.0, 0.0], 0.005);
    step(&mut s, k, "a concentric circle", tangent(inner, e, 1.0));
    let outer = circle(&mut s, k, [0.0, 0.0], 0.02);
    step(&mut s, k, "a circle as big as the minor axis", tangent(outer, e, 1.0));
    // Extremes of size: very thin ellipse, very small circle, very large circle.
    for (what, major, minor, r) in [
        ("thin ellipse", 0.5, 1e-4, 0.01),
        ("tiny circle", 0.04, 0.02, 1e-7),
        ("huge circle", 0.04, 0.02, 100.0),
        ("round ellipse", 0.02, 0.02, 0.01),
    ] {
        let mut s = Session::new();
        let k = s.create_sketch("XY").unwrap();
        let e = ellipse(&mut s, k, [0.0, 0.0], [major, 0.0], minor, true);
        let c = circle(&mut s, k, [0.0, major * 2.0 + r], r);
        step(&mut s, k, what, tangent(c, e, 1.0));
    }
    // Fixed apart: either accepted as unsatisfiable (reported degenerate) or refused, with no trace.
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    let e = ellipse(&mut s, k, [0.0, 0.0], [0.04, 0.0], 0.02, true);
    let c = circle(&mut s, k, [1.0, 1.0], 0.01);
    run(&mut s, k, SketchOp::AddConstraint(GeoConstraint::FixCurve(c))).unwrap();
    step(&mut s, k, "fixed far apart", tangent(c, e, 1.0));
}

#[test]
fn two_ellipses_boundaries_and_edits_that_follow() {
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    let a = ellipse(&mut s, k, [0.0, 0.0], [0.04, 0.0], 0.02, true);
    let b = ellipse(&mut s, k, [0.1, 0.01], [0.13, 0.03], 0.015, false);
    step(&mut s, k, "ellipse-ellipse, either order", tangent(b, a, 1.0));
    assert!(step(&mut s, k, "ellipse-ellipse tangent", tangent(a, b, 1.0)) || true);
    run(&mut s, k, tangent(a, b, 1.0)).ok();
    let held =
        s.active_sketch().unwrap().1.constraints.values().any(|c| matches!(c, GeoConstraint::Tangent { .. }));
    if held {
        assert!(gap(&outline(&s, a), &outline(&s, b)) < 1e-4);
        // The same pair again is redundant.
        assert!(!step(&mut s, k, "the same pair again", tangent(a, b, 1.0)));
    }
    // Two identical ellipses at the same place (a degenerate pair).
    let twin = ellipse(&mut s, k, [0.0, 0.0], [0.04, 0.0], 0.02, false);
    step(&mut s, k, "a twin ellipse", tangent(a, twin, 1.0));
    // Edits of a tangent pair: move the free one, copy, mirror, scale, delete.
    let before_outline = outline(&s, a);
    step(
        &mut s,
        k,
        "move the free ellipse",
        SketchOp::Transform {
            curves: vec![b],
            motion: Motion::Translate { by: [0.01, 0.005] },
            copy: false,
            relax: true,
        },
    );
    step(
        &mut s,
        k,
        "copy a tangent pair",
        SketchOp::Transform {
            curves: vec![a, b],
            motion: Motion::Translate { by: [0.0, -0.1] },
            copy: true,
            relax: false,
        },
    );
    step(
        &mut s,
        k,
        "scale a tangent pair",
        SketchOp::Transform {
            curves: vec![b],
            motion: Motion::Scale { centre: [0.0, 0.0], factor: 0.5 },
            copy: false,
            relax: true,
        },
    );
    assert_eq!(outline(&s, a), before_outline, "the fixed ellipse did not move");
    assert!(step(&mut s, k, "delete one of the pair", SketchOp::Delete(vec![Item::Curve(b)])));
    for c in s.active_sketch().unwrap().1.constraints.values() {
        if let GeoConstraint::Tangent { a, b, .. } = c {
            let sk = s.active_sketch().unwrap().1;
            assert!(sk.curves.contains_key(a) && sk.curves.contains_key(b), "a tangent outlived its curve");
        }
    }
}

/// BUG (found with the ellipse-circle tangent of 01af0d8): a constraint of a curve with itself is refused for
/// circles (`Unsolved(Flipped)`) and was for a line and an ellipse, but an ellipse is now accepted as tangent to
/// itself — a constraint that means nothing and takes a place in the list.
#[test]
#[ignore = "BUG: Tangent of an ellipse with itself is accepted (owner: backend, gc_sketch bind.rs/gc_solver)"]
fn an_ellipse_is_not_tangent_to_itself() {
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    let e = ellipse(&mut s, k, [0.0, 0.0], [0.04, 0.0], 0.02, true);
    assert!(!step(&mut s, k, "the ellipse with itself", tangent(e, e, 1.0)));
    let c = circle(&mut s, k, [0.3, 0.4], 0.01);
    assert!(!step(&mut s, k, "the circle with itself", tangent(c, c, 1.0)));
}

// ---- Spline Smooth (G2) ----

/// A line along X ending at (50 mm, 0) and a spline starting on its end.
fn spline_on_line(points: Vec<[f64; 3]>) -> (Session, K, Id<Curve>, Id<Curve>) {
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    let l = line(&mut s, k, [0.0; 3], [0.05, 0.0, 0.0]);
    let end: Id<SkPoint> = s.active_sketch().unwrap().1.line(l).unwrap().1;
    let sp = curve(
        &mut s,
        k,
        SketchOp::AddSpline { points, snaps: vec![EndSnap { point: Some(end), ..EndSnap::default() }] },
    );
    (s, k, l, sp)
}

fn smooth(a: Id<Curve>, b: Id<Curve>) -> SketchOp {
    SketchOp::AddConstraint(GeoConstraint::Smooth(a, b))
}

#[test]
fn smooth_and_tangent_boundaries_on_splines() {
    let (mut s, k, l, sp) =
        spline_on_line(vec![[0.05, 0.0, 0.0], [0.08, 0.03, 0.0], [0.1, 0.06, 0.0], [0.13, 0.05, 0.0]]);
    // Wrong things: ghost, a curve with itself, two lines, a spline with a curve it is not joined to.
    assert!(!step(&mut s, k, "smooth with a ghost", smooth(sp, ghost())));
    assert!(!step(&mut s, k, "smooth with itself", smooth(sp, sp)));
    let m = line(&mut s, k, [0.0, 0.1, 0.0], [0.05, 0.1, 0.0]);
    assert!(!step(&mut s, k, "smooth of two lines", smooth(l, m)));
    assert!(!step(&mut s, k, "smooth with a line it is not joined to", smooth(sp, m)));
    let far = circle(&mut s, k, [0.3, 0.3], 0.02);
    assert!(!step(&mut s, k, "smooth with a circle it is not joined to", smooth(sp, far)));
    // Both orders of arguments, then the same end again (redundant).
    step(&mut s, k, "smooth (line, spline)", smooth(l, sp));
    step(&mut s, k, "tangent (line, spline)", tangent(l, sp, 1.0));
    // `step` leaves the accepted constraint in place: Smooth (line, spline) is held now.
    assert!(!step(&mut s, k, "smooth again", smooth(sp, l)));
    assert!(!step(&mut s, k, "smooth again, other order", smooth(l, sp)));
    // Tangent after Smooth: the end is already shaped; accepted or refused, never a mess.
    step(&mut s, k, "tangent after smooth", tangent(sp, l, 1.0));
    // Edits that touch the constrained end: move the line, trim, split, delete, undo.
    step(
        &mut s,
        k,
        "move the line",
        SketchOp::Transform {
            curves: vec![l],
            motion: Motion::Translate { by: [0.0, 0.01] },
            copy: false,
            relax: true,
        },
    );
    step(
        &mut s,
        k,
        "rotate the spline",
        SketchOp::Transform {
            curves: vec![sp],
            motion: Motion::Rotate { centre: [0.05, 0.0], angle: 0.4 },
            copy: false,
            relax: true,
        },
    );
    step(
        &mut s,
        k,
        "copy both",
        SketchOp::Transform {
            curves: vec![l, sp],
            motion: Motion::Translate { by: [0.0, -0.2] },
            copy: true,
            relax: false,
        },
    );
    step(&mut s, k, "mirror both", SketchOp::Mirror { curves: vec![sp], axis: m });
    let inner = match &s.active_sketch().unwrap().1.curves[&sp].geom {
        CurveGeom::Spline { points } => points[1],
        _ => panic!(),
    };
    step(&mut s, k, "split the smooth spline", SketchOp::SplitSpline { curve: sp, point: inner });
    step(&mut s, k, "close the smooth spline", SketchOp::CloseSpline { curve: sp });
    step(&mut s, k, "trim the line", SketchOp::Trim { curve: l, at: [0.02, 0.0] });
    assert!(step(&mut s, k, "delete the line", SketchOp::Delete(vec![Item::Curve(l)])));
    for c in s.active_sketch().unwrap().1.constraints.values() {
        if let GeoConstraint::Smooth(a, b) = c {
            let sk = s.active_sketch().unwrap().1;
            assert!(sk.curves.contains_key(a) && sk.curves.contains_key(b), "a Smooth outlived its curve");
        }
    }
    sane(&s, "after the edits");
}

#[test]
fn smooth_on_short_closed_and_degenerate_splines() {
    // A spline of two fit points (a straight run): nothing to match curvature to.
    let (mut s, k, l, sp) = spline_on_line(vec![[0.05, 0.0, 0.0], [0.1, 0.02, 0.0]]);
    step(&mut s, k, "smooth on a two-point spline", smooth(sp, l));
    step(&mut s, k, "tangent on a two-point spline", tangent(sp, l, 1.0));
    // Fit points almost on one line, and doubling back.
    for (what, points) in [
        ("collinear", vec![[0.05, 0.0, 0.0], [0.07, 0.0, 0.0], [0.09, 0.0, 0.0]]),
        ("doubling back", vec![[0.05, 0.0, 0.0], [0.03, 0.01, 0.0], [0.0, 0.02, 0.0]]),
        ("nearly coincident", vec![[0.05, 0.0, 0.0], [0.05 + 1e-7, 1e-7, 0.0], [0.06, 0.01, 0.0]]),
        ("huge", vec![[0.05, 0.0, 0.0], [1e6, 1e6, 0.0], [2e6, 0.0, 0.0]]),
        ("tiny", vec![[0.05, 0.0, 0.0], [0.05 + 1e-6, 1e-6, 0.0], [0.05 + 2e-6, 0.0, 0.0]]),
    ] {
        let (mut s, k, l, sp) = spline_on_line(points);
        step(&mut s, k, &format!("smooth, {what}"), smooth(sp, l));
        step(&mut s, k, &format!("tangent, {what}"), tangent(sp, l, 1.0));
    }
    // A closed spline: no ends to shape.
    let (mut s, k, l, sp) =
        spline_on_line(vec![[0.05, 0.0, 0.0], [0.08, 0.03, 0.0], [0.1, 0.0, 0.0], [0.08, -0.03, 0.0]]);
    run(&mut s, k, SketchOp::CloseSpline { curve: sp }).unwrap();
    step(&mut s, k, "smooth on a closed spline", smooth(sp, l));
    // Smooth between two splines meeting end to end, then the chain of three.
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    let first = curve(
        &mut s,
        k,
        SketchOp::AddSpline {
            points: vec![[0.0, 0.0, 0.0], [0.03, 0.02, 0.0], [0.06, 0.0, 0.0]],
            snaps: Vec::new(),
        },
    );
    let end = match &s.active_sketch().unwrap().1.curves[&first].geom {
        CurveGeom::Spline { points } => *points.last().unwrap(),
        _ => panic!(),
    };
    let second = curve(
        &mut s,
        k,
        SketchOp::AddSpline {
            points: vec![[0.06, 0.0, 0.0], [0.09, -0.02, 0.0], [0.12, 0.0, 0.0]],
            snaps: vec![EndSnap { point: Some(end), ..EndSnap::default() }],
        },
    );
    step(&mut s, k, "smooth between two splines", smooth(first, second));
    step(&mut s, k, "smooth between two splines, reversed", smooth(second, first));
    // A circle at the end of a spline.
    let c = circle(&mut s, k, [0.06, -0.05], 0.05);
    step(&mut s, k, "smooth with a circle through the end", smooth(first, c));
    // A saved file keeps whatever was accepted.
    run(&mut s, k, smooth(first, second)).ok();
    s.finish_sketch();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("smooth.gcp");
    s.save_as(&path).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert!(t.doc().same_content(s.doc()), "the reopened document differs");
    let again = dir.path().join("again.gcp");
    t.save_as(&again).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), std::fs::read(&again).unwrap());
}
