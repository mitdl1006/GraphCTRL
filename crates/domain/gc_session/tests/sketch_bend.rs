//! Bend in a 3D sketch (Inventor 2026 help, To Create Lines, Arcs, and Bends,
//! GUID-4F861674-F74C-474E-869A-4C190ED6EC4B): "The Bend tool lets you place tangent corner bends on lines in a
//! 3D sketch. … specify a Radius. … click any line".

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "tests")]

use gc_core::Id;
use gc_doc::{Command, Created, EndSnap, SketchOp};
use gc_session::Session;
use gc_sketch::{Curve, CurveGeom, DimKind};

type K = Id<gc_doc::Sketch>;

fn run(s: &mut Session, k: K, op: SketchOp) -> Result<Vec<Created>, gc_doc::EditError> {
    s.execute(&Command::Sketch { sketch: k, op })
}

fn line(s: &mut Session, k: K, a: [f64; 3], b: [f64; 3]) -> Id<Curve> {
    let op =
        SketchOp::AddLine { a, b, snap_a: EndSnap::default(), snap_b: EndSnap::default(), horizontal: None };
    run(s, k, op)
        .unwrap()
        .iter()
        .find_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None })
        .unwrap()
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn len(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

#[test]
fn a_bend_joins_two_lines_with_a_tangent_arc() {
    let mut s = Session::new();
    let k = s.create_sketch_3d().unwrap();
    // Two lines meeting at a corner, at an angle that is not square, out of every principal plane.
    let corner = [0.1, 0.1, 0.1];
    let l1 = line(&mut s, k, [0.0, 0.0, 0.0], corner);
    let l2 = line(&mut s, k, corner, [0.3, 0.1, 0.25]);
    let steps = s.history().undo_depth();
    let made = run(&mut s, k, SketchOp::Bend { lines: [l1, l2], radius: "20 mm".into() }).unwrap();
    assert_eq!(s.history().undo_depth(), steps + 1);
    let arc = made.iter().find_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None }).unwrap();
    let sk = s.active_sketch().unwrap().1;
    let CurveGeom::Arc { c, s: start, e } = sk.curves[&arc].geom.clone() else { panic!() };
    let (c, ps, pe) = (sk.pos(c).unwrap(), sk.pos(start).unwrap(), sk.pos(e).unwrap());
    assert!((len(sub(ps, c)) - 0.02).abs() < 1e-9 && (len(sub(pe, c)) - 0.02).abs() < 1e-9);
    // Each line now ends where the arc starts or ends, running along it there (square to the radius,
    // in its plane).
    let normal = {
        let (u, w) = (sub(ps, c), sub(pe, c));
        [u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]]
    };
    for (l, joint) in [(l1, ps), (l2, pe)] {
        let (a, b) = sk.line(l).unwrap();
        let (a, b) = (sk.pos(a).unwrap(), sk.pos(b).unwrap());
        let near = if len(sub(a, joint)) < len(sub(b, joint)) { a } else { b };
        assert!(len(sub(near, joint)) < 1e-9, "{near:?} vs {joint:?}");
        let dir = sub(b, a);
        assert!(dot(dir, sub(joint, c)).abs() / len(dir) < 1e-9);
        assert!(dot(dir, normal).abs() / (len(dir) * len(normal)) < 1e-9);
    }
    // A radius dimension drives it.
    assert!(sk.dimensions.values().any(|d| d.kind == DimKind::Radius(arc)));
    assert!(s.undo());
    assert_eq!(s.active_sketch().unwrap().1.curves.len(), 2);
}

#[test]
fn a_bend_is_refused_where_it_cannot_fit() {
    let mut s = Session::new();
    let k = s.create_sketch_3d().unwrap();
    let l1 = line(&mut s, k, [0.0, 0.0, 0.0], [0.1, 0.0, 0.0]);
    let l2 = line(&mut s, k, [0.1, 0.0, 0.0], [0.1, 0.01, 0.0]);
    // Too short for the radius; lines that do not meet; lines in line.
    assert!(run(&mut s, k, SketchOp::Bend { lines: [l1, l2], radius: "50 mm".into() }).is_err());
    let apart = line(&mut s, k, [0.0, 1.0, 0.0], [0.0, 1.0, 1.0]);
    assert!(run(&mut s, k, SketchOp::Bend { lines: [l1, apart], radius: "5 mm".into() }).is_err());
    let straight = line(&mut s, k, [0.1, 0.0, 0.0], [0.2, 0.0, 0.0]);
    assert!(run(&mut s, k, SketchOp::Bend { lines: [l1, straight], radius: "5 mm".into() }).is_err());
    // A 2D sketch has no Bend.
    let flat = s.create_sketch("XY").unwrap();
    let a = line(&mut s, flat, [0.0, 0.0, 0.0], [0.1, 0.0, 0.0]);
    let b = line(&mut s, flat, [0.1, 0.0, 0.0], [0.1, 0.1, 0.0]);
    assert!(run(&mut s, flat, SketchOp::Bend { lines: [a, b], radius: "5 mm".into() }).is_err());
}
