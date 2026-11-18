//! Symmetric on lines and curves (Inventor 2026 help, To Apply Geometric Constraints,
//! GUID-A85A7A30-7D81-4C75-8769-CAD034EEA930): "click the first line or curve. Click the second line or curve.
//! Click the line of symmetry. … Selected lines and arcs are constrained about a line to create a symmetrical
//! shape."

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "tests")]

use gc_core::Id;
use gc_doc::{Command, Created, EndSnap, SketchOp};
use gc_session::Session;
use gc_sketch::{Curve, CurveGeom, SkPoint};

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

fn xy(s: &Session, p: Id<SkPoint>) -> [f64; 2] {
    let q = s.active_sketch().unwrap().1.pos(p).unwrap();
    [q[0], q[1]]
}

/// Mirror in the Y axis (x = 0).
fn mirrored(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] + b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9
}

fn setup() -> (Session, K, Id<Curve>) {
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    let axis = line(&mut s, k, [0.0, -0.1], [0.0, 0.1]);
    // The line of symmetry held still, so that mirror images are about x = 0.
    run(&mut s, k, SketchOp::AddConstraint(gc_sketch::GeoConstraint::FixCurve(axis))).unwrap();
    (s, k, axis)
}

#[test]
fn two_lines_become_mirror_images() {
    let (mut s, k, axis) = setup();
    let left = line(&mut s, k, [-0.05, 0.0], [-0.02, 0.04]);
    // Drawn the other way round: the ends pair by where they would mirror to.
    let right = line(&mut s, k, [0.025, 0.035], [0.055, 0.005]);
    let steps = s.history().undo_depth();
    let made = run(&mut s, k, SketchOp::SymmetricCurves { first: left, second: right, axis }).unwrap();
    assert_eq!(s.history().undo_depth(), steps + 1);
    assert_eq!(made.len(), 2);
    let sk = s.active_sketch().unwrap().1;
    let ((a, b), (c, d)) = (sk.line(left).unwrap(), sk.line(right).unwrap());
    assert!(mirrored(xy(&s, a), xy(&s, d)) && mirrored(xy(&s, b), xy(&s, c)));
    // Not about a line, or the same curve twice: refused.
    assert!(run(&mut s, k, SketchOp::SymmetricCurves { first: left, second: left, axis }).is_err());
    assert!(run(&mut s, k, SketchOp::SymmetricCurves { first: left, second: right, axis: left }).is_err());
}

#[test]
fn circles_and_arcs_mirror_with_their_centres() {
    let (mut s, k, axis) = setup();
    let circle = |s: &mut Session, x: f64, r: f64| {
        curve(
            s,
            k,
            SketchOp::AddCircle {
                centre: [x, 0.02, 0.0],
                radius: r,
                centre_snap: EndSnap::default(),
                rim_snap: EndSnap::default(),
            },
        )
    };
    let (c1, c2) = (circle(&mut s, -0.04, 0.01), circle(&mut s, 0.05, 0.015));
    run(&mut s, k, SketchOp::SymmetricCurves { first: c1, second: c2, axis }).unwrap();
    let sk = s.active_sketch().unwrap().1;
    let ((p, r1), (q, r2)) = (sk.round(c1).unwrap(), sk.round(c2).unwrap());
    assert!(mirrored(xy(&s, p), xy(&s, q)) && (r1 - r2).abs() < 1e-9);
    // Arcs: the start of one mirrors the end of the other.
    let arc = |s: &mut Session, c: [f64; 2], start: [f64; 2], end: [f64; 2]| {
        curve(
            s,
            k,
            SketchOp::AddArc {
                centre: [c[0], c[1], 0.0],
                start: [start[0], start[1], 0.0],
                end: [end[0], end[1], 0.0],
            },
        )
    };
    let a1 = arc(&mut s, [-0.05, -0.05], [-0.03, -0.05], [-0.05, -0.03]);
    let a2 = arc(&mut s, [0.05, -0.05], [0.05, -0.03], [0.03, -0.05]);
    run(&mut s, k, SketchOp::SymmetricCurves { first: a1, second: a2, axis }).unwrap();
    let sk = s.active_sketch().unwrap().1;
    let (CurveGeom::Arc { c, s: st, e }, CurveGeom::Arc { c: c2, s: s2, e: e2 }) =
        (sk.curves[&a1].geom.clone(), sk.curves[&a2].geom.clone())
    else {
        panic!()
    };
    assert!(
        mirrored(xy(&s, c), xy(&s, c2))
            && mirrored(xy(&s, st), xy(&s, e2))
            && mirrored(xy(&s, e), xy(&s, s2))
    );
    // A line and a circle do not pair.
    assert!(run(&mut s, k, SketchOp::SymmetricCurves { first: c1, second: a1, axis }).is_err());
}

#[test]
fn the_symmetric_tool_takes_two_lines_and_the_axis() {
    use gc_tools::{ConstraintKind, ToolEvent, ToolKind};
    let (mut s, k, _axis) = setup();
    let left = line(&mut s, k, [-0.05, 0.0], [-0.03, 0.03]);
    let right = line(&mut s, k, [0.05, 0.01], [0.03, 0.04]);
    s.set_tool(ToolKind::Constraint(ConstraintKind::Symmetric));
    let px = 1e-4;
    for at in [[-0.04, 0.015], [0.04, 0.025], [0.0, 0.07]] {
        s.tool_event(ToolEvent::Move { pos: at, px, suppress_snap: true });
        s.tool_event(ToolEvent::Click { pos: at, px, modifier: false });
    }
    let sk = s.active_sketch().unwrap().1;
    let ((a, _), (c, d)) = (sk.line(left).unwrap(), sk.line(right).unwrap());
    assert!(mirrored(xy(&s, a), xy(&s, c)) || mirrored(xy(&s, a), xy(&s, d)));
}
