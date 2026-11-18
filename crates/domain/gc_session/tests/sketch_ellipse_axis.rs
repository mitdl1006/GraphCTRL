//! Ellipse axes in line constraints (Inventor 2026 help, To Apply Geometric Constraints,
//! GUID-A85A7A30-7D81-4C75-8769-CAD034EEA930): Collinear takes "an ellipse axis", Parallel and Perpendicular
//! "lines … or ellipse axes", Horizontal and Vertical "a line, an ellipse axis, or two points".

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

/// The major axis direction of an ellipse, as an angle.
fn axis_angle(s: &Session, e: Id<Curve>) -> f64 {
    let sk = s.active_sketch().unwrap().1;
    let (c, m, _) = sk.ellipse(e).unwrap();
    let (c, m) = (sk.pos(c).unwrap(), sk.pos(m).unwrap());
    gc_math::fm::atan2(m[1] - c[1], m[0] - c[0])
}

fn setup() -> (Session, K, Id<Curve>, Id<Curve>) {
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    let line = curve(
        &mut s,
        k,
        SketchOp::AddLine {
            a: [0.0, 0.1, 0.0],
            b: [0.1, 0.15, 0.0],
            snap_a: EndSnap::default(),
            snap_b: EndSnap::default(),
            horizontal: None,
        },
    );
    let e = curve(
        &mut s,
        k,
        SketchOp::AddEllipse {
            centre: [0.0, 0.0, 0.0],
            major: [0.03, 0.01, 0.0],
            minor: 0.01,
            centre_snap: EndSnap::default(),
        },
    );
    (s, k, line, e)
}

#[test]
fn an_ellipse_axis_is_parallel_perpendicular_or_collinear_to_a_line() {
    let line_angle = |s: &Session, line: Id<Curve>| {
        let sk = s.active_sketch().unwrap().1;
        let (a, b) = sk.line(line).unwrap();
        let (a, b) = (sk.pos(a).unwrap(), sk.pos(b).unwrap());
        gc_math::fm::atan2(b[1] - a[1], b[0] - a[0])
    };
    let quarter = std::f64::consts::FRAC_PI_2;
    let (mut s, k, line, e) = setup();
    run(&mut s, k, SketchOp::AddConstraint(GeoConstraint::Parallel(e, line))).unwrap();
    let d = (axis_angle(&s, e) - line_angle(&s, line)).rem_euclid(std::f64::consts::PI);
    assert!(d < 1e-9 || std::f64::consts::PI - d < 1e-9, "{d}");
    s.undo();
    run(&mut s, k, SketchOp::AddConstraint(GeoConstraint::Perpendicular(line, e))).unwrap();
    let d = (axis_angle(&s, e) - line_angle(&s, line)).rem_euclid(std::f64::consts::PI);
    assert!((d - quarter).abs() < 1e-9, "{d}");
    s.undo();
    // Collinear: the ellipse's centre comes onto the line too.
    run(&mut s, k, SketchOp::AddConstraint(GeoConstraint::Collinear(e, line))).unwrap();
    let sk = s.active_sketch().unwrap().1;
    let (c, _, _) = sk.ellipse(e).unwrap();
    let (a, b) = sk.line(line).unwrap();
    let (c, a, b) = (sk.pos(c).unwrap(), sk.pos(a).unwrap(), sk.pos(b).unwrap());
    let cross = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    // The ellipse does not shrink to nothing to get there.
    let m = sk.pos(sk.ellipse(e).unwrap().1).unwrap();
    assert!(gc_math::fm::hypot(m[0] - c[0], m[1] - c[1]) > 0.02);
    assert!(cross.abs() < 1e-12, "{cross}");
}

#[test]
fn the_horizontal_tool_takes_an_ellipse_axis() {
    use gc_tools::{ToolEvent, ToolKind};
    let (mut s, _k, _line, e) = setup();
    s.set_tool(ToolKind::Constraint(gc_tools::ConstraintKind::Horizontal));
    // Click on the ellipse, at the end of its minor axis (on the curve, away from the line).
    let sk = s.active_sketch().unwrap().1;
    let on = sk.ellipse_outline(e, 4).unwrap().0[1];
    let px = 1e-4;
    s.tool_event(ToolEvent::Move { pos: on, px, suppress_snap: true });
    s.tool_event(ToolEvent::Click { pos: on, px, modifier: false });
    assert!(axis_angle(&s, e).abs() < 1e-9, "{}", axis_angle(&s, e));
}
