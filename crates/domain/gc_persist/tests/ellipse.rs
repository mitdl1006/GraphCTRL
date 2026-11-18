//! Ellipses (Inventor 2026 help, To Create 2D Shape Geometry, Ellipse): drawn from the centre, an axis end
//! and the minor radius; constrained by their centre, points on them, a horizontal or vertical major axis
//! and the major and minor radius dimensions; saved and loaded, while older files without ellipses load as
//! before.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "tests")]

use gc_core::Id;
use gc_doc::{Command, Created, Document, EditError, SketchOp, apply};
use gc_sketch::{CurveGeom, DimKind, ExternalRef, GeoConstraint, PointKind, SkPoint, Sketch};

fn new_sketch() -> (Document, Id<Sketch>) {
    let mut doc = Document::new();
    let frame = doc.origin_plane("XY").unwrap();
    let applied =
        apply(&mut doc, &Command::CreateSketch { name: "S".into(), frame, spatial: false }).unwrap();
    let Created::Sketch(sketch) = applied.created[0] else { panic!() };
    (doc, sketch)
}

fn op(doc: &mut Document, sketch: Id<Sketch>, op: SketchOp) -> Result<Vec<Created>, EditError> {
    apply(doc, &Command::Sketch { sketch, op }).map(|a| a.created)
}

fn origin(sk: &Sketch) -> Id<SkPoint> {
    *sk.points
        .iter()
        .find(|(_, p)| matches!(p.kind, PointKind::External(ExternalRef::FrameOrigin(_))))
        .unwrap()
        .0
}

fn dimension(
    doc: &mut Document,
    sketch: Id<Sketch>,
    kind: DimKind,
    source: &str,
) -> Id<gc_sketch::Dimension> {
    let created =
        op(doc, sketch, SketchOp::AddDimension { kind, source: source.into(), text_pos: [0.0; 2] }).unwrap();
    created.iter().find_map(|c| if let Created::Dimension(d) = c { Some(*d) } else { None }).unwrap()
}

/// On the ellipse: `(x / a)² + (y / b)² = 1` in its own axes.
fn on_ellipse(sk: &Sketch, curve: Id<gc_sketch::Curve>, p: [f64; 3]) -> f64 {
    let (c, m, b) = sk.ellipse(curve).unwrap();
    let (c, m) = (sk.pos(c).unwrap(), sk.pos(m).unwrap());
    let (dx, dy) = (m[0] - c[0], m[1] - c[1]);
    let a = (dx * dx + dy * dy).sqrt();
    let (u, v) = (p[0] - c[0], p[1] - c[1]);
    let (x, y) = ((u * dx + v * dy) / a, (dx * v - dy * u) / a);
    (x / a).powi(2) + (y / b).powi(2) - 1.0
}

#[test]
fn an_ellipse_is_constrained_fully_by_its_centre_axis_and_two_radii() {
    let (mut doc, k) = new_sketch();
    let created = op(
        &mut doc,
        k,
        SketchOp::AddEllipse {
            centre: [0.01, 0.005, 0.0],
            major: [0.04, 0.012, 0.0],
            minor: 0.012,
            centre_snap: Default::default(),
        },
    )
    .unwrap();
    let curve = created.iter().find_map(|c| if let Created::Curve(c) = c { Some(*c) } else { None }).unwrap();
    let sk = doc.get(k).unwrap();
    let (c, major, _) = sk.ellipse(curve).unwrap();
    assert_eq!(sk.clone().solve().unwrap().dof, 5, "centre 2, axis end 2, minor radius 1");
    let o = origin(sk);
    for constraint in [GeoConstraint::Coincident(c, o), GeoConstraint::Horizontal(c, major)] {
        op(&mut doc, k, SketchOp::AddConstraint(constraint)).unwrap();
    }
    dimension(&mut doc, k, DimKind::Distance { p: c, q: major }, "30 mm");
    let minor = dimension(&mut doc, k, DimKind::Radius(curve), "10 mm");
    let sk = doc.get(k).unwrap();
    assert_eq!(sk.clone().solve().unwrap().dof, 0, "fully constrained");
    let m = sk.pos(major).unwrap();
    assert!((m[0] - 0.03).abs() < 1e-9 && m[1].abs() < 1e-9, "major axis end on +X at 30 mm: {m:?}");
    assert!((sk.ellipse(curve).unwrap().2 - 0.01).abs() < 1e-9);
    assert!((sk.measure(&DimKind::Radius(curve)).unwrap() - 0.01).abs() < 1e-12);

    // A point on the ellipse stays on it when the minor radius changes.
    let p = op(&mut doc, k, SketchOp::AddPoint { pos: [0.01, 0.012, 0.0] }).unwrap();
    let Created::Point(p) = p[0] else { panic!() };
    op(&mut doc, k, SketchOp::AddConstraint(GeoConstraint::PointOnCurve(p, curve))).unwrap();
    op(&mut doc, k, SketchOp::SetDimension { id: minor, source: "15 mm".into() }).unwrap();
    let sk = doc.get(k).unwrap();
    assert!(on_ellipse(sk, curve, sk.pos(p).unwrap()).abs() < 1e-6);
    assert!((sk.ellipse(curve).unwrap().2 - 0.015).abs() < 1e-9);
    // A vertical axis instead: the axis end goes to +Y.
    let horizontal = sk
        .constraints
        .iter()
        .find(|(_, c)| matches!(c, GeoConstraint::Horizontal(..)))
        .map(|(id, _)| *id)
        .unwrap();
    op(&mut doc, k, SketchOp::Delete(vec![gc_sketch::Item::Constraint(horizontal)])).unwrap();
    op(&mut doc, k, SketchOp::AddConstraint(GeoConstraint::Vertical(c, major))).unwrap();
    let m = doc.get(k).unwrap().pos(major).unwrap();
    assert!(m[0].abs() < 1e-9 && (m[1].abs() - 0.03).abs() < 1e-9, "{m:?}");
}

#[test]
fn bad_ellipses_are_refused_and_ellipses_round_trip() {
    let (mut doc, k) = new_sketch();
    let before = doc.clone();
    for (major, minor) in [
        ([0.0, 0.0, 0.0], 0.01),
        ([0.03, 0.0, 0.0], 0.0),
        ([0.03, 0.0, 0.0], f64::NAN),
        ([f64::INFINITY, 0.0, 0.0], 0.01),
    ] {
        let result = op(
            &mut doc,
            k,
            SketchOp::AddEllipse { centre: [0.0; 3], major, minor, centre_snap: Default::default() },
        );
        assert!(result.is_err(), "{major:?} {minor}");
        assert!(doc.same_content(&before));
    }
    op(
        &mut doc,
        k,
        SketchOp::AddEllipse {
            centre: [0.0; 3],
            major: [0.03, 0.0, 0.0],
            minor: 0.01,
            centre_snap: Default::default(),
        },
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ellipse.gcp");
    gc_persist::save(&doc, &path).unwrap();
    let loaded = gc_persist::load(&path).unwrap();
    assert!(loaded.needs_repair.is_empty());
    let sk = loaded.doc.get(k).unwrap();
    assert!(
        sk.curves
            .values()
            .any(|c| matches!(c.geom, CurveGeom::Ellipse { minor, .. } if (minor - 0.01).abs() < 1e-12))
    );
    assert!(loaded.doc.same_content(&doc));
}

fn line(doc: &mut Document, k: Id<Sketch>, a: [f64; 2], b: [f64; 2]) -> Id<gc_sketch::Curve> {
    let created = op(
        doc,
        k,
        SketchOp::AddLine {
            a: [a[0], a[1], 0.0],
            b: [b[0], b[1], 0.0],
            snap_a: Default::default(),
            snap_b: Default::default(),
            horizontal: None,
        },
    )
    .unwrap();
    created.iter().find_map(|c| if let Created::Curve(c) = c { Some(*c) } else { None }).unwrap()
}

fn ellipse(doc: &mut Document, k: Id<Sketch>) -> Id<gc_sketch::Curve> {
    let created = op(
        doc,
        k,
        SketchOp::AddEllipse {
            centre: [0.0; 3],
            major: [0.03, 0.0, 0.0],
            minor: 0.01,
            centre_snap: Default::default(),
        },
    )
    .unwrap();
    created.iter().find_map(|c| if let Created::Curve(c) = c { Some(*c) } else { None }).unwrap()
}

/// Help (To Create 2D Shape Geometry, Ellipse): "You can trim and extend ellipses like any other curve."
#[test]
fn ellipses_trim_split_and_extend_like_other_curves() {
    let (mut doc, k) = new_sketch();
    let e = ellipse(&mut doc, k);
    let cutter = line(&mut doc, k, [0.0, -0.02], [0.0, 0.02]);
    // Where they cross: the ends of the minor axis; the quadrants are the four axis ends.
    let sk = doc.get(k).unwrap();
    let mut crossings = sk.curve_crossings(e, cutter);
    crossings.sort_by(|p, q| p[1].total_cmp(&q[1]));
    assert_eq!(crossings.len(), 2);
    assert!((crossings[0][1] + 0.01).abs() < 1e-9 && (crossings[1][1] - 0.01).abs() < 1e-9, "{crossings:?}");
    assert_eq!(sk.ellipse_quadrants(e).len(), 4);
    // Trimming the right half leaves the left half as an elliptical arc ending on the line.
    let preview = sk.trim_preview(e, [0.03, 0.0]).unwrap();
    assert!(
        matches!(preview, gc_sketch::Piece::EllipseArc { sweep, .. } if (sweep - std::f64::consts::PI).abs() < 1e-6)
    );
    op(&mut doc, k, SketchOp::Trim { curve: e, at: [0.03, 0.0] }).unwrap();
    let sk = doc.get(k).unwrap();
    let (s, en) = sk.ellipse_arc(e).expect("an elliptical arc now");
    let (ps, pe) = (sk.pos(s).unwrap(), sk.pos(en).unwrap());
    assert!(ps[0].abs() < 1e-9 && (ps[1] - 0.01).abs() < 1e-9, "starts at the top: {ps:?}");
    assert!(pe[0].abs() < 1e-9 && (pe[1] + 0.01).abs() < 1e-9, "ends at the bottom: {pe:?}");
    assert!(sk.clone().solve().is_ok());
    assert_eq!(sk.ellipse_quadrants(e).len(), 3, "the top, left and bottom axis ends remain on it");

    // A cut through the middle of the arc splits it into two arcs of one ellipse.
    let middle = line(&mut doc, k, [-0.015, -0.02], [-0.015, 0.02]);
    op(&mut doc, k, SketchOp::Trim { curve: e, at: [-0.03, 0.0] }).unwrap();
    let sk = doc.get(k).unwrap();
    let arcs: Vec<_> = sk
        .curves
        .iter()
        .filter(|(_, c)| matches!(c.geom, CurveGeom::Ellipse { arc: Some(_), .. }))
        .map(|(id, _)| *id)
        .collect();
    assert_eq!(arcs.len(), 2, "split in two");
    assert!(
        sk.constraints
            .values()
            .any(|c| matches!(c, GeoConstraint::Equal(a, b) if arcs.contains(a) && arcs.contains(b)))
    );
    // Both stay on one ellipse when its minor radius is driven.
    let minor = dimension(&mut doc, k, DimKind::Radius(arcs[0]), "10 mm");
    op(&mut doc, k, SketchOp::SetDimension { id: minor, source: "12 mm".into() }).unwrap();
    let sk = doc.get(k).unwrap();
    for arc in &arcs {
        assert!((sk.ellipse(*arc).unwrap().2 - 0.012).abs() < 1e-9);
    }
    let _ = middle;

    // Extend: an elliptical arc lengthens to the next curve beyond its end.
    let (mut doc, k) = new_sketch();
    let e = ellipse(&mut doc, k);
    let cutter = line(&mut doc, k, [0.0, -0.02], [0.0, 0.02]);
    op(&mut doc, k, SketchOp::Trim { curve: e, at: [0.03, 0.0] }).unwrap();
    op(&mut doc, k, SketchOp::Delete(vec![gc_sketch::Item::Curve(cutter)])).unwrap();
    let wall = line(&mut doc, k, [0.02, -0.02], [0.02, 0.02]);
    let sk = doc.get(k).unwrap();
    assert!(sk.extend_preview(e, [0.001, 0.0099]).is_some());
    op(&mut doc, k, SketchOp::Extend { curve: e, at: [0.001, 0.0099] }).unwrap();
    let sk = doc.get(k).unwrap();
    let (s, _) = sk.ellipse_arc(e).unwrap();
    let ps = sk.pos(s).unwrap();
    let y = 0.01 * (1.0f64 - (0.02f64 / 0.03).powi(2)).sqrt();
    assert!((ps[0] - 0.02).abs() < 1e-9 && (ps[1] - y).abs() < 1e-9, "extended to the wall: {ps:?}");
    assert!(sk.clone().solve().is_ok());
    let _ = wall;
    // An elliptical arc saves and loads.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("arc.gcp");
    gc_persist::save(&doc, &path).unwrap();
    assert!(gc_persist::load(&path).unwrap().doc.same_content(&doc));
}

/// A circle crossing an ellipse: found numerically, on both curves.
#[test]
fn circles_and_ellipses_cross_where_both_hold() {
    let (mut doc, k) = new_sketch();
    let e = ellipse(&mut doc, k);
    let circle = match op(
        &mut doc,
        k,
        SketchOp::AddCircle {
            centre: [0.03, 0.0, 0.0],
            radius: 0.01,
            centre_snap: Default::default(),
            rim_snap: Default::default(),
        },
    )
    .unwrap()[..]
    {
        [Created::Curve(c), ..] => c,
        ref other => panic!("{other:?}"),
    };
    let sk = doc.get(k).unwrap();
    let crossings = sk.curve_crossings(e, circle);
    assert_eq!(crossings.len(), 2, "{crossings:?}");
    for p in crossings {
        assert!(on_ellipse(sk, e, [p[0], p[1], 0.0]).abs() < 1e-9);
        assert!((((p[0] - 0.03).powi(2) + p[1].powi(2)).sqrt() - 0.01).abs() < 1e-9);
    }
}
