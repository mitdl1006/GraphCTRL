//! Interpolation splines in a 3D sketch (Inventor 2026 help, To Create, Lock Spline Length, and Edit Splines,
//! GUID-DB15C479-66F2-456A-9E54-40A60F83DC92: "Interpolation splines can be created in both 2D and 3D
//! sketches").

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "tests")]

use gc_doc::{Command, Created, SketchOp};
use gc_session::Session;
use gc_sketch::CurveGeom;
use gc_tools::ToolKind;

#[test]
fn a_spline_runs_through_fit_points_in_space() {
    let mut s = Session::new();
    let k = s.create_sketch_3d().unwrap();
    let points = vec![[0.0, 0.0, 0.0], [0.1, 0.05, 0.1], [0.2, 0.0, 0.3], [0.3, 0.1, 0.35]];
    let created = s
        .execute(&Command::Sketch {
            sketch: k,
            op: SketchOp::AddSpline { points: points.clone(), snaps: Vec::new() },
        })
        .unwrap();
    let curve =
        created.iter().find_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None }).unwrap();
    let sk = s.active_sketch().unwrap().1;
    let outline = sk.spline_outline3(&sk.curves[&curve].geom, 16).unwrap();
    assert_eq!(outline.len(), 3 * 16 + 1);
    // Through every fit point, in space.
    for (k, p) in points.iter().enumerate() {
        let q = outline[k * 16];
        assert!((0..3).all(|i| (q[i] - p[i]).abs() < 1e-12), "{q:?} vs {p:?}");
    }
    // The sketch solves with it (nothing to solve: it follows its fit points).
    assert_eq!(sk.clone().solve().unwrap().dof, 12);
    // Closing works in space too; Insert Point (in a plane) does not.
    s.execute(&Command::Sketch { sketch: k, op: SketchOp::CloseSpline { curve } }).unwrap();
    let sk = s.active_sketch().unwrap().1;
    assert!(gc_sketch::spline_closed(match &sk.curves[&curve].geom {
        CurveGeom::Spline { points } => points,
        other => panic!("{other:?}"),
    }));
    let insert = SketchOp::InsertSplinePoint { curve, at: [0.1, 0.05] };
    assert!(s.execute(&Command::Sketch { sketch: k, op: insert }).is_err());
}

#[test]
fn the_spline_tool_places_fit_points_in_space() {
    let mut s = Session::new();
    let _k = s.create_sketch_3d().unwrap();
    s.set_tool(ToolKind::Spline);
    s.spatial_click([0.0, 0.0, 0.0], None, None);
    s.spatial_click([0.1, 0.0, 0.1], None, None);
    s.spatial_hover(Some(([0.2, 0.1, 0.2], None)));
    assert!(s.spatial_spline_preview().unwrap().len() > 16, "through the points and the cursor");
    s.spatial_click([0.2, 0.1, 0.2], None, None);
    // Enter finishes; one undo step removes it.
    let steps = s.history().undo_depth();
    s.spatial_finish();
    assert_eq!(s.history().undo_depth(), steps + 1);
    let count = |s: &Session| {
        s.active_sketch()
            .unwrap()
            .1
            .curves
            .values()
            .filter(|c| matches!(c.geom, CurveGeom::Spline { .. }))
            .count()
    };
    assert_eq!(count(&s), 1);
    // A second click on the last point finishes too.
    s.spatial_click([0.0, 0.5, 0.0], None, None);
    s.spatial_click([0.1, 0.5, 0.2], None, None);
    s.spatial_click([0.1, 0.5, 0.2], None, None);
    assert_eq!(count(&s), 2);
    // One point and Enter: nothing.
    s.spatial_click([0.0, 1.0, 0.0], None, None);
    s.spatial_finish();
    assert_eq!(count(&s), 2);
}
