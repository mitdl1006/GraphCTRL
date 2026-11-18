//! Tangent between a line and an ellipse (Inventor 2026 help, To Apply Geometric Constraints,
//! GUID-A85A7A30-7D81-4C75-8769-CAD034EEA930: "click the first curve … Click the second curve").

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

fn line(s: &mut Session, k: K, a: [f64; 2], b: [f64; 2]) -> Id<Curve> {
    curve(
        s,
        k,
        SketchOp::AddLine {
            a: [a[0], a[1], 0.0],
            b: [b[0], b[1], 0.0],
            snap_a: EndSnap::default(),
            snap_b: EndSnap::default(),
            horizontal: None,
        },
    )
}

/// A fixed ellipse: centre at the origin, major radius 40 mm along `dir`, minor radius 20 mm.
fn ellipse(s: &mut Session, k: K, dir: [f64; 2]) -> Id<Curve> {
    let e = curve(
        s,
        k,
        SketchOp::AddEllipse {
            centre: [0.0, 0.0, 0.0],
            major: [0.04 * dir[0], 0.04 * dir[1], 0.0],
            minor: 0.02,
            centre_snap: EndSnap::default(),
        },
    );
    run(s, k, SketchOp::AddConstraint(GeoConstraint::FixCurve(e))).unwrap();
    e
}

/// The signed distance of the origin from a line and the line's unit direction.
fn line_place(s: &Session, l: Id<Curve>) -> (f64, [f64; 2]) {
    let sk = s.active_sketch().unwrap().1;
    let (a, b) = sk.line(l).unwrap();
    let (a, b) = (sk.pos(a).unwrap(), sk.pos(b).unwrap());
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = gc_math::fm::hypot(dx, dy);
    ((dx * (0.0 - a[1]) - dy * (0.0 - a[0])) / len, [dx / len, dy / len])
}

#[test]
fn a_line_touches_an_ellipse() {
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    let e = ellipse(&mut s, k, [1.0, 0.0]);
    let l = line(&mut s, k, [-0.05, 0.05], [0.05, 0.055]);
    run(
        &mut s,
        k,
        SketchOp::AddConstraint(GeoConstraint::Tangent { a: l, b: e, side: -1.0, external: true }),
    )
    .unwrap();
    let (d, dir) = line_place(&s, l);
    // The reach of the ellipse across the line's direction.
    let n = [-dir[1], dir[0]];
    let reach = gc_math::fm::hypot(0.04 * n[0], 0.02 * n[1]);
    assert!((d.abs() - reach).abs() < 1e-9, "{d} vs {reach}");
    // The tangent tool works out the side by itself.
    s.undo();
    use gc_tools::{ConstraintKind, ToolEvent, ToolKind};
    s.set_tool(ToolKind::Constraint(ConstraintKind::Tangent));
    let px = 1e-4;
    for at in [[0.0, 0.0525], [0.0, 0.02]] {
        s.tool_event(ToolEvent::Move { pos: at, px, suppress_snap: true });
        s.tool_event(ToolEvent::Click { pos: at, px, modifier: false });
    }
    let (d, dir) = line_place(&s, l);
    let n = [-dir[1], dir[0]];
    assert!((d.abs() - gc_math::fm::hypot(0.04 * n[0], 0.02 * n[1])).abs() < 1e-9);
}

#[test]
fn a_line_ending_on_an_ellipse_runs_along_it_there() {
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    // A turned ellipse, so that the axes are not the sketch's.
    let e = ellipse(&mut s, k, [0.8, 0.6]);
    let sk = s.active_sketch().unwrap().1;
    let on = sk.ellipse_outline(e, 8).unwrap().0[1];
    let l = line(&mut s, k, on, [on[0] + 0.03, on[1] + 0.05]);
    let (start, _) = s.active_sketch().unwrap().1.line(l).unwrap();
    run(&mut s, k, SketchOp::AddConstraint(GeoConstraint::PointOnCurve(start, e))).unwrap();
    run(&mut s, k, SketchOp::AddConstraint(GeoConstraint::Tangent { a: e, b: l, side: 1.0, external: true }))
        .unwrap();
    // The ellipse's normal at the shared point is square to the line.
    let sk = s.active_sketch().unwrap().1;
    let p = sk.pos(start).unwrap();
    let (u, v) = ([0.8, 0.6], [-0.6, 0.8]);
    let (x, y) = (p[0] * u[0] + p[1] * u[1], p[0] * v[0] + p[1] * v[1]);
    let on_curve = (x / 0.04).powi(2) + (y / 0.02).powi(2);
    assert!((on_curve - 1.0).abs() < 1e-9, "{on_curve}");
    let (nl_x, nl_y) = (x / 0.04f64.powi(2), y / 0.02f64.powi(2));
    let normal = [u[0] * nl_x + v[0] * nl_y, u[1] * nl_x + v[1] * nl_y];
    let (_, dir) = line_place(&s, l);
    let cos = (dir[0] * normal[0] + dir[1] * normal[1]) / gc_math::fm::hypot(normal[0], normal[1]);
    assert!(cos.abs() < 1e-9, "{cos}");
}
