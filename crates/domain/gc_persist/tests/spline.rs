//! Interpolation splines (Inventor 2026 help, Spline (Interpolation)): an open smooth curve through fit
//! points that are sketch points; saved and loaded, older files without splines load as before.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "tests")]

use gc_core::Id;
use gc_doc::{Command, Created, Document, SketchOp, apply};
use gc_sketch::{CurveGeom, GeoConstraint, Sketch};

fn new_sketch() -> (Document, Id<Sketch>) {
    let mut doc = Document::new();
    let frame = doc.origin_plane("XY").unwrap();
    let applied =
        apply(&mut doc, &Command::CreateSketch { name: "S".into(), frame, spatial: false }).unwrap();
    let Created::Sketch(sketch) = applied.created[0] else { panic!() };
    (doc, sketch)
}

fn spline(
    doc: &mut Document,
    k: Id<Sketch>,
    points: &[[f64; 2]],
) -> Result<Id<gc_sketch::Curve>, gc_doc::EditError> {
    let op =
        SketchOp::AddSpline { points: points.iter().map(|p| [p[0], p[1], 0.0]).collect(), snaps: Vec::new() };
    let created = apply(doc, &Command::Sketch { sketch: k, op })?.created;
    Ok(created.iter().find_map(|c| if let Created::Curve(c) = c { Some(*c) } else { None }).unwrap())
}

#[test]
fn a_spline_passes_through_its_fit_points_and_follows_them() {
    let (mut doc, k) = new_sketch();
    let fit = [[0.0, 0.0], [0.01, 0.01], [0.02, -0.005], [0.03, 0.0]];
    let s = spline(&mut doc, k, &fit).unwrap();
    let sk = doc.get(k).unwrap();
    let CurveGeom::Spline { points } = &sk.curves[&s].geom else { panic!() };
    assert_eq!(points.len(), 4);
    assert_eq!(sk.clone().solve().unwrap().dof, 8, "two per fit point");
    // Through every fit point (every 16th sample is one), and smooth between them.
    let line = sk.spline_outline(&sk.curves[&s].geom, 16).unwrap();
    assert_eq!(line.len(), 3 * 16 + 1);
    for (k, p) in fit.iter().enumerate() {
        let q = line[k * 16];
        assert!((q[0] - p[0]).abs() < 1e-12 && (q[1] - p[1]).abs() < 1e-12, "{k}: {q:?}");
    }
    // A fit point tied to the sketch origin moves the curve with it.
    let origin =
        *sk.points.iter().find(|(_, p)| matches!(p.kind, gc_sketch::PointKind::External(_))).unwrap().0;
    let first = points[0];
    let second = points[1];
    apply(
        &mut doc,
        &Command::Sketch { sketch: k, op: SketchOp::AddConstraint(GeoConstraint::Coincident(first, origin)) },
    )
    .unwrap();
    apply(
        &mut doc,
        &Command::Sketch { sketch: k, op: SketchOp::AddConstraint(GeoConstraint::Horizontal(first, second)) },
    )
    .unwrap();
    let sk = doc.get(k).unwrap();
    assert!(sk.pos(second).unwrap()[1].abs() < 1e-9);
    assert_eq!(sk.clone().solve().unwrap().dof, 5);
    // Copied with its fit points; deleted with them.
    let copied = apply(
        &mut doc,
        &Command::Sketch {
            sketch: k,
            op: SketchOp::Transform {
                curves: vec![s],
                motion: gc_doc::Motion::Translate { by: [0.0, 0.05] },
                copy: true,
                relax: true,
            },
        },
    )
    .unwrap();
    let copy =
        copied.created.iter().find_map(|c| if let Created::Curve(c) = c { Some(*c) } else { None }).unwrap();
    assert!(
        matches!(doc.get(k).unwrap().curves[&copy].geom, CurveGeom::Spline { ref points } if points.len() == 4)
    );
    let before = doc.get(k).unwrap().points.len();
    apply(&mut doc, &Command::Sketch { sketch: k, op: SketchOp::Delete(vec![gc_sketch::Item::Curve(copy)]) })
        .unwrap();
    assert_eq!(doc.get(k).unwrap().points.len(), before - 4);
}

#[test]
fn bad_splines_are_refused_and_splines_round_trip() {
    let (mut doc, k) = new_sketch();
    let before = doc.clone();
    for bad in [vec![[0.0, 0.0]], vec![], vec![[0.0, 0.0], [0.0, 0.0]], vec![[0.0, 0.0], [f64::NAN, 1.0]]] {
        assert!(spline(&mut doc, k, &bad).is_err(), "{bad:?}");
        assert!(doc.same_content(&before));
    }
    spline(&mut doc, k, &[[0.0, 0.0], [0.02, 0.01], [0.04, 0.0]]).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("spline.gcp");
    gc_persist::save(&doc, &path).unwrap();
    let loaded = gc_persist::load(&path).unwrap();
    assert!(loaded.doc.same_content(&doc));
    assert!(loaded.needs_repair.is_empty());
}
