//! Fix on curves (Inventor 2026 help, To Apply Geometric Constraints, GUID-A85A7A30-7D81-4C75-8769-CAD034EEA930):
//! "Lines are fixed in position and angle but endpoints can move to shorten or lengthen the line. Circles
//! and arcs have fixed center point and radius. Arc and line endpoints are free to move along the extents
//! of the radius or length. … Position, size, and orientation of a circle or ellipse are fixed."

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "tests")]

use gc_core::Id;
use gc_doc::{Command, Created, EndSnap, SketchOp};
use gc_session::Session;
use gc_sketch::{Curve, CurveGeom, DimKind, GeoConstraint};

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

fn dof(s: &Session) -> usize {
    s.active_sketch().unwrap().1.clone().solve().unwrap().dof
}

fn fix(s: &mut Session, k: K, c: Id<Curve>) -> Result<Vec<Created>, gc_doc::EditError> {
    run(s, k, SketchOp::AddConstraint(GeoConstraint::FixCurve(c)))
}

#[test]
fn a_fixed_line_keeps_its_place_and_angle_but_its_ends_slide() {
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    let line = curve(
        &mut s,
        k,
        SketchOp::AddLine {
            a: [0.0, 0.01, 0.0],
            b: [0.1, 0.06, 0.0],
            snap_a: EndSnap::default(),
            snap_b: EndSnap::default(),
            horizontal: None,
        },
    );
    assert_eq!(dof(&s), 4);
    fix(&mut s, k, line).unwrap();
    assert_eq!(dof(&s), 2, "each end slides along the line");
    // Fixing it again adds nothing: refused.
    assert!(fix(&mut s, k, line).is_err());
    // A length dimension lengthens it along itself.
    let (a, b) = s.active_sketch().unwrap().1.line(line).unwrap();
    run(
        &mut s,
        k,
        SketchOp::AddDimension {
            kind: DimKind::Distance { p: a, q: b },
            source: "200".into(),
            text_pos: [0.0, 0.0],
        },
    )
    .unwrap();
    let sk = s.active_sketch().unwrap().1;
    let (pa, pb) = (sk.pos(a).unwrap(), sk.pos(b).unwrap());
    assert!((gc_math::fm::hypot(pb[0] - pa[0], pb[1] - pa[1]) - 0.2).abs() < 1e-9);
    for p in [pa, pb] {
        // Still on the line y = 0.01 + x / 2.
        assert!((p[1] - (0.01 + p[0] / 2.0)).abs() < 1e-9, "{p:?}");
    }
    assert_eq!(dof(&s), 1);
}

#[test]
fn fixed_circles_arcs_ellipses_and_splines() {
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    let circle = curve(
        &mut s,
        k,
        SketchOp::AddCircle {
            centre: [0.0, 0.0, 0.0],
            radius: 0.02,
            centre_snap: EndSnap::default(),
            rim_snap: EndSnap::default(),
        },
    );
    let start = dof(&s);
    fix(&mut s, k, circle).unwrap();
    assert_eq!(dof(&s), start - 3, "centre and radius");
    let ellipse = curve(
        &mut s,
        k,
        SketchOp::AddEllipse {
            centre: [0.1, 0.0, 0.0],
            major: [0.14, 0.0, 0.0],
            minor: 0.02,
            centre_snap: EndSnap::default(),
        },
    );
    let start = dof(&s);
    fix(&mut s, k, ellipse).unwrap();
    assert_eq!(dof(&s), start - 5, "position, size and orientation");
    let spline = curve(
        &mut s,
        k,
        SketchOp::AddSpline {
            points: vec![[0.0, 0.1, 0.0], [0.05, 0.12, 0.0], [0.1, 0.1, 0.0]],
            snaps: Vec::new(),
        },
    );
    let start = dof(&s);
    fix(&mut s, k, spline).unwrap();
    assert_eq!(dof(&s), start - 6);
    // An arc keeps its centre and radius; its ends slide round.
    let arc = curve(
        &mut s,
        k,
        SketchOp::AddArc { centre: [0.0, -0.1, 0.0], start: [0.02, -0.1, 0.0], end: [0.0, -0.08, 0.0] },
    );
    let start = dof(&s);
    fix(&mut s, k, arc).unwrap();
    assert_eq!(dof(&s), start - 3);
    // The Fix tool takes a curve too.
    let sk = s.active_sketch().unwrap().1;
    assert!(matches!(sk.curves[&arc].geom, CurveGeom::Arc { .. }));
    assert_eq!(sk.constraints.values().filter(|c| matches!(c, GeoConstraint::FixCurve(_))).count(), 4);
}
