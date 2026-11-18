//! Tangent between an ellipse and a circle, or two ellipses (To Apply Geometric Constraints,
//! GUID-A85A7A30-7D81-4C75-8769-CAD034EEA930: Tangent takes two curves).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "tests")]

use gc_core::Id;
use gc_doc::{Command, Created, EndSnap, SketchOp};
use gc_session::Session;
use gc_sketch::{Curve, GeoConstraint};

type K = Id<gc_doc::Sketch>;

fn run(s: &mut Session, k: K, op: SketchOp) -> Result<Vec<Created>, gc_doc::EditError> {
    s.execute(&Command::Sketch { sketch: k, op })
}

fn curve(s: &mut Session, k: K, op: SketchOp) -> Id<Curve> {
    run(s, k, op)
        .unwrap()
        .iter()
        .find_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None })
        .unwrap()
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

/// The least distance between a point of the first outline and the second outline.
fn gap(a: &[[f64; 2]], b: &[[f64; 2]]) -> f64 {
    a.iter()
        .flat_map(|p| b.iter().map(move |q| gc_math::fm::hypot(p[0] - q[0], p[1] - q[1])))
        .fold(f64::INFINITY, f64::min)
}

fn outline(s: &Session, c: Id<Curve>) -> Vec<[f64; 2]> {
    let sk = s.active_sketch().unwrap().1;
    if let Some(points) = sk.ellipse_outline(c, 2048) {
        return points.0;
    }
    let (centre, r) = sk.round(c).unwrap();
    let centre = sk.pos(centre).unwrap();
    (0..2048)
        .map(|i| {
            let t = f64::from(i) / 2048.0 * std::f64::consts::TAU;
            [centre[0] + r * gc_math::fm::cos(t), centre[1] + r * gc_math::fm::sin(t)]
        })
        .collect()
}

#[test]
fn a_circle_touches_a_fixed_ellipse_from_outside_or_inside() {
    // Outside.
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    let e = ellipse(&mut s, k, [0.0, 0.0], [0.04, 0.0], 0.02, true);
    let c = circle(&mut s, k, [0.03, 0.04], 0.01);
    let steps = s.history().undo_depth();
    run(&mut s, k, SketchOp::AddConstraint(GeoConstraint::Tangent { a: e, b: c, side: 1.0, external: true }))
        .unwrap();
    assert_eq!(s.history().undo_depth(), steps + 1);
    let g = gap(&outline(&s, e), &outline(&s, c));
    assert!(g < 1e-4, "they touch: {g}");
    // Touching, not crossing: the circle's centre is outside the ellipse by its radius.
    let sk = s.active_sketch().unwrap().1;
    let (centre, r) = sk.round(c).unwrap();
    let p = sk.pos(centre).unwrap();
    let inside = (p[0] / 0.04).powi(2) + (p[1] / 0.02).powi(2) < 1.0;
    assert!(!inside && r > 0.0);
    // Inside.
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    let e = ellipse(&mut s, k, [0.0, 0.0], [0.04, 0.0], 0.02, true);
    let c = circle(&mut s, k, [0.02, 0.0], 0.008);
    run(&mut s, k, SketchOp::AddConstraint(GeoConstraint::Tangent { a: c, b: e, side: 1.0, external: true }))
        .unwrap();
    assert!(gap(&outline(&s, e), &outline(&s, c)) < 1e-4);
    let sk = s.active_sketch().unwrap().1;
    let p = sk.pos(sk.round(c).unwrap().0).unwrap();
    assert!((p[0] / 0.04).powi(2) + (p[1] / 0.02).powi(2) < 1.0, "still inside");
}

#[test]
fn two_ellipses_touch_and_the_tool_takes_them() {
    use gc_tools::{ConstraintKind, ToolEvent, ToolKind};
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    let a = ellipse(&mut s, k, [0.0, 0.0], [0.04, 0.0], 0.02, true);
    let b = ellipse(&mut s, k, [0.1, 0.01], [0.13, 0.03], 0.015, false);
    s.set_tool(ToolKind::Constraint(ConstraintKind::Tangent));
    let px = 1e-4;
    for at in [outline(&s, a)[300], outline(&s, b)[1300]] {
        s.tool_event(ToolEvent::Move { pos: at, px, suppress_snap: true });
        s.tool_event(ToolEvent::Click { pos: at, px, modifier: false });
    }
    let sk = s.active_sketch().unwrap().1;
    assert!(sk.constraints.values().any(|c| matches!(c, GeoConstraint::Tangent { .. })));
    let g = gap(&outline(&s, a), &outline(&s, b));
    assert!(g < 1e-4, "{g}");
    // Saved and loaded, it still holds.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.gcp");
    s.save_as(&path).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    let sketch = t.doc().sketches().iter().next().unwrap().0;
    t.enter_sketch(sketch);
    assert!(gap(&outline(&t, a), &outline(&t, b)) < 1e-4);
}
