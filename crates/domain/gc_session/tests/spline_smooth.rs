//! A spline's Tangent and Smooth (G2) ends (Inventor 2026 help, To Apply Geometric Constraints,
//! GUID-A85A7A30-7D81-4C75-8769-CAD034EEA930: "The Smooth (G2) constraint causes a continuous curvature (G2)
//! condition between a spline and another curve, such as a line, arc, or spline. … click the spline and the
//! curve attached to the spline endpoint").

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "tests")]

use gc_core::Id;
use gc_doc::{Command, Created, EndSnap, SketchOp};
use gc_session::Session;
use gc_sketch::{Curve, GeoConstraint, SkPoint};

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

/// A line along X ending at (50 mm, 0) and a spline starting on its end, heading up and away.
fn setup() -> (Session, K, Id<Curve>, Id<Curve>) {
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    let line = curve(
        &mut s,
        k,
        SketchOp::AddLine {
            a: [0.0, 0.0, 0.0],
            b: [0.05, 0.0, 0.0],
            snap_a: EndSnap::default(),
            snap_b: EndSnap::default(),
            horizontal: None,
        },
    );
    let end: Id<SkPoint> = s.active_sketch().unwrap().1.line(line).unwrap().1;
    let spline = curve(
        &mut s,
        k,
        SketchOp::AddSpline {
            points: vec![[0.05, 0.0, 0.0], [0.08, 0.03, 0.0], [0.1, 0.06, 0.0]],
            snaps: vec![EndSnap { point: Some(end), ..EndSnap::default() }],
        },
    );
    (s, k, line, spline)
}

fn outline(s: &Session, spline: Id<Curve>) -> Vec<[f64; 2]> {
    let sk = s.active_sketch().unwrap().1;
    sk.spline_outline(&sk.curves[&spline].geom, 64).unwrap()
}

/// A fine outline, for curvature right at the joint.
fn fine(s: &Session, spline: Id<Curve>) -> Vec<[f64; 2]> {
    let sk = s.active_sketch().unwrap().1;
    sk.spline_outline(&sk.curves[&spline].geom, 4096).unwrap()
}

/// The direction (unit) and curvature at the start of a polyline, by differences.
fn start_shape(line: &[[f64; 2]]) -> ([f64; 2], f64) {
    let (p0, p1, p2) = (line[0], line[1], line[2]);
    let (d1, d2) = ([p1[0] - p0[0], p1[1] - p0[1]], [p2[0] - p1[0], p2[1] - p1[1]]);
    let l1 = gc_math::fm::hypot(d1[0], d1[1]);
    let l2 = gc_math::fm::hypot(d2[0], d2[1]);
    let turn = gc_math::fm::atan2(d1[0] * d2[1] - d1[1] * d2[0], d1[0] * d2[0] + d1[1] * d2[1]);
    ([d1[0] / l1, d1[1] / l1], turn / ((l1 + l2) / 2.0))
}

#[test]
fn a_tangent_spline_leaves_the_line_along_it() {
    let (mut s, k, line, spline) = setup();
    let (free, _) = start_shape(&outline(&s, spline));
    assert!(free[1] > 0.3, "without it the spline heads up: {free:?}");
    let dof = s.active_sketch().unwrap().1.clone().solve().unwrap().dof;
    let steps = s.history().undo_depth();
    run(
        &mut s,
        k,
        SketchOp::AddConstraint(GeoConstraint::Tangent { a: spline, b: line, side: 1.0, external: true }),
    )
    .unwrap();
    assert_eq!(s.history().undo_depth(), steps + 1);
    let (dir, _) = start_shape(&outline(&s, spline));
    assert!(dir[0] > 0.999 && dir[1].abs() < 0.03, "{dir:?}");
    // Nothing to solve: the degrees of freedom are as they were.
    assert_eq!(s.active_sketch().unwrap().1.clone().solve().unwrap().dof, dof);
    // The same end again: redundant, refused.
    let again = SketchOp::AddConstraint(GeoConstraint::Smooth(spline, line));
    assert!(run(&mut s, k, again).is_err());
    // Saved and loaded, it keeps the shape.
    let dir_path = tempfile::tempdir().unwrap();
    let path = dir_path.path().join("smooth.gcp");
    let before = outline(&s, spline);
    s.save_as(&path).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    let sketch = t.doc().sketches().iter().next().unwrap().0;
    t.enter_sketch(sketch);
    let after = outline(&t, spline);
    assert!(
        before.iter().zip(&after).all(|(p, q)| (p[0] - q[0]).abs() < 1e-12 && (p[1] - q[1]).abs() < 1e-12)
    );
}

#[test]
fn a_smooth_spline_matches_direction_and_curvature() {
    // To a line: along it, with no curvature at the joint.
    // Tangent only, for comparison: the curvature at the joint is whatever the fit points make it.
    let (mut s, k, line, spline) = setup();
    let tangent = run(
        &mut s,
        k,
        SketchOp::AddConstraint(GeoConstraint::Tangent { a: spline, b: line, side: 1.0, external: true }),
    )
    .unwrap();
    let (_, kappa_tangent) = start_shape(&outline(&s, spline));
    let id =
        tangent.iter().find_map(|c| if let Created::Constraint(id) = c { Some(*id) } else { None }).unwrap();
    run(&mut s, k, SketchOp::Delete(vec![gc_sketch::Item::Constraint(id)])).unwrap();
    run(&mut s, k, SketchOp::AddConstraint(GeoConstraint::Smooth(spline, line))).unwrap();
    let (dir, kappa) = start_shape(&outline(&s, spline));
    assert!(dir[0] > 0.999, "{dir:?}");
    // At the joint itself it is zero; the first steps (measured here) are a little way along.
    assert!(
        kappa.abs() < 0.1 * kappa_tangent.abs(),
        "smooth {kappa} /m against tangent-only {kappa_tangent} /m"
    );
    // Not joined to the spline: refused.
    let apart = curve(
        &mut s,
        k,
        SketchOp::AddLine {
            a: [0.0, 1.0, 0.0],
            b: [0.1, 1.0, 0.0],
            snap_a: EndSnap::default(),
            snap_b: EndSnap::default(),
            horizontal: None,
        },
    );
    assert!(run(&mut s, k, SketchOp::AddConstraint(GeoConstraint::Smooth(spline, apart))).is_err());

    // To an arc: the arc's curvature carries on.
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    // A quarter circle of radius 50 mm about the origin, from (50, 0) to (0, 50) mm.
    let arc = curve(
        &mut s,
        k,
        SketchOp::AddArc { centre: [0.0; 3], start: [0.05, 0.0, 0.0], end: [0.0, 0.05, 0.0] },
    );
    let end = match s.active_sketch().unwrap().1.curves[&arc].geom {
        gc_sketch::CurveGeom::Arc { e, .. } => e,
        _ => panic!(),
    };
    let at = s.active_sketch().unwrap().1.pos(end).unwrap();
    let spline = curve(
        &mut s,
        k,
        SketchOp::AddSpline {
            points: vec![at, [-0.03, 0.06, 0.0], [-0.06, 0.05, 0.0]],
            snaps: vec![EndSnap { point: Some(end), ..EndSnap::default() }],
        },
    );
    run(&mut s, k, SketchOp::AddConstraint(GeoConstraint::Smooth(spline, arc))).unwrap();
    let (dir, kappa) = start_shape(&fine(&s, spline));
    // Leaving the arc's end (0, 50) going on counter-clockwise: towards -X, turning left at 1/0.05.
    assert!(dir[0] < -0.999, "{dir:?}");
    assert!((kappa - 20.0).abs() < 1.0, "curvature {kappa} /m, the arc's is 20");
}

#[test]
fn the_smooth_tool_takes_a_spline_and_the_curve_at_its_end() {
    use gc_tools::{ConstraintKind, ToolEvent, ToolKind};
    let (mut s, _k, _line, spline) = setup();
    s.set_tool(ToolKind::Constraint(ConstraintKind::Smooth));
    let px = 1e-4;
    let on_spline = outline(&s, spline)[64 + 10];
    for at in [on_spline, [0.02, 0.0]] {
        s.tool_event(ToolEvent::Move { pos: at, px, suppress_snap: true });
        s.tool_event(ToolEvent::Click { pos: at, px, modifier: false });
    }
    let sk = s.active_sketch().unwrap().1;
    assert!(sk.constraints.values().any(|c| matches!(c, GeoConstraint::Smooth(a, _) if *a == spline)));
}
