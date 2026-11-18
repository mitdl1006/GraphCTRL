//! Document Settings saved with the file (Inventor 2026 help, To Change Document Settings,
//! GUID-FB49837F-7832-4713-928E-1CEDBDB0C3BD; About Units of Measure, GUID-671A8679-AEC1-413A-A23B-428967648CE6:
//! "Unit value settings are saved with the file").

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "tests")]

use gc_doc::{AngleUnit, DimensionDisplay, DocSettings, LengthUnit};
use gc_session::Session;

#[test]
fn settings_are_one_undo_step_saved_and_loaded() {
    let mut s = Session::new();
    assert_eq!(s.doc_settings(), DocSettings::default());
    let changed = DocSettings {
        length_unit: LengthUnit::In,
        angle_unit: AngleUnit::Rad,
        linear_precision: 2,
        angular_precision: 4,
        dimension_display: DimensionDisplay::Expression,
        input_as_expression: false,
        auto_bend_radius: 0.005,
        snap_spacing: [0.002, 0.003],
        snaps_per_minor: 5,
        minor_per_major: 4,
        infer_constraints: false,
        persist_constraints: false,
    };
    let steps = s.history().undo_depth();
    s.set_doc_settings(changed).unwrap();
    assert_eq!(s.history().undo_depth(), steps + 1);
    assert_eq!(s.doc_settings(), changed);
    assert_eq!(s.doc_settings().format_length(0.0254), "1 in");
    // Not valid: refused, nothing changes.
    let before = s.doc().clone();
    for bad in [
        DocSettings { linear_precision: 12, ..changed },
        DocSettings { auto_bend_radius: -1.0, ..changed },
        DocSettings { snap_spacing: [f64::NAN, 0.001], ..changed },
        DocSettings { minor_per_major: 0, ..changed },
    ] {
        assert!(s.set_doc_settings(bad).is_err());
    }
    assert!(s.doc().same_content(&before));
    // Saved with the document.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.gcp");
    s.save_as(&path).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert_eq!(t.doc_settings(), changed);
    // Undo brings the defaults back; a document saved with them (as one from before settings existed)
    // writes no settings and reads the defaults.
    s.undo();
    assert_eq!(s.doc_settings(), DocSettings::default());
    let plain = dir.path().join("plain.gcp");
    s.save_as(&plain).unwrap();
    assert!(!std::fs::read_to_string(&plain).unwrap().contains("auto_bend_radius"));
    let mut u = Session::new();
    u.open(&plain).unwrap();
    assert_eq!(u.doc_settings(), DocSettings::default());
}

/// Auto-Bend bends with the document's radius.
#[test]
fn auto_bend_uses_the_documents_radius() {
    use gc_sketch::CurveGeom;
    use gc_tools::ToolKind;
    let mut s = Session::new();
    s.set_doc_settings(DocSettings { auto_bend_radius: 0.005, ..DocSettings::default() }).unwrap();
    let _k = s.create_sketch_3d().unwrap();
    s.set_auto_bend(true);
    s.set_tool(ToolKind::Line);
    for p in [[0.0, 0.0, 0.0], [0.1, 0.0, 0.0], [0.1, 0.1, 0.0]] {
        s.spatial_click(p, None, None);
    }
    let sk = s.active_sketch().unwrap().1;
    let (_, arc) = sk.curves.iter().find(|(_, c)| matches!(c.geom, CurveGeom::Arc { .. })).unwrap();
    let CurveGeom::Arc { c, s: start, .. } = arc.geom.clone() else { panic!() };
    let (c, p) = (sk.pos(c).unwrap(), sk.pos(start).unwrap());
    let r = ((c[0] - p[0]).powi(2) + (c[1] - p[1]).powi(2) + (c[2] - p[2]).powi(2)).sqrt();
    assert!((r - 0.005).abs() < 1e-9, "{r}");
}
