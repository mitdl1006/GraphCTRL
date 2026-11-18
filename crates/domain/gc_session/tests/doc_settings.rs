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

fn line_dimension_setup() -> (Session, gc_core::Id<gc_doc::Sketch>, gc_core::Id<gc_sketch::Curve>) {
    use gc_doc::{Command, Created, EndSnap, SketchOp};
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    let op = SketchOp::AddLine {
        a: [0.0, 0.0, 0.0],
        b: [0.03, 0.0, 0.0],
        snap_a: EndSnap::default(),
        snap_b: EndSnap::default(),
        horizontal: Some(true),
    };
    let created = s.execute(&Command::Sketch { sketch: k, op }).unwrap();
    let line =
        created.iter().find_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None }).unwrap();
    (s, k, line)
}

/// "You can also override the default units in any value box" (About Units of Measure): a bare number means
/// the document's unit; one with a unit means that unit.
#[test]
fn a_bare_number_means_the_documents_unit() {
    use gc_doc::{Command, SketchOp};
    use gc_sketch::DimKind;
    let (mut s, k, line) = line_dimension_setup();
    s.set_doc_settings(DocSettings { length_unit: LengthUnit::In, ..DocSettings::default() }).unwrap();
    let (a, b) = s.active_sketch().unwrap().1.line(line).unwrap();
    let kind = DimKind::Distance { p: a, q: b };
    s.execute(&Command::Sketch {
        sketch: k,
        op: SketchOp::AddDimension { kind, source: "1".into(), text_pos: [0.0; 2] },
    })
    .unwrap();
    assert!((s.active_sketch().unwrap().1.measure(&kind).unwrap() - 0.0254).abs() < 1e-12);
    // A unit typed overrides it.
    let id = *s.active_sketch().unwrap().1.dimensions.keys().next().unwrap();
    s.execute(&Command::Sketch { sketch: k, op: SketchOp::SetDimension { id, source: "12 mm".into() } })
        .unwrap();
    assert!((s.active_sketch().unwrap().1.measure(&kind).unwrap() - 0.012).abs() < 1e-12);
    // The common value box too.
    assert!((gc_doc::evaluate_value(s.doc(), "2", gc_doc::Unit::Mm).unwrap() - 0.0508).abs() < 1e-12);
    // Angles: radians.
    s.set_doc_settings(DocSettings {
        length_unit: LengthUnit::In,
        angle_unit: AngleUnit::Rad,
        ..DocSettings::default()
    })
    .unwrap();
    assert!((gc_doc::evaluate_value(s.doc(), "1", gc_doc::Unit::Deg).unwrap() - 1.0).abs() < 1e-12);
}

/// "When you change the units setting, all existing values in the file display as the new units": sizes do
/// not change; a bare number is written out in the old unit.
#[test]
fn changing_units_keeps_every_size() {
    use gc_doc::{Command, SketchOp};
    use gc_sketch::DimKind;
    let (mut s, k, line) = line_dimension_setup();
    let (a, b) = s.active_sketch().unwrap().1.line(line).unwrap();
    let kind = DimKind::Distance { p: a, q: b };
    s.execute(&Command::Sketch {
        sketch: k,
        op: SketchOp::AddDimension { kind, source: "25".into(), text_pos: [0.0; 2] },
    })
    .unwrap();
    s.set_doc_settings(DocSettings { length_unit: LengthUnit::In, ..DocSettings::default() }).unwrap();
    let sk = s.active_sketch().unwrap().1;
    let d = sk.dimensions.values().next().unwrap();
    assert_eq!(d.source, "25 mm");
    assert!((sk.measure(&kind).unwrap() - 0.025).abs() < 1e-12);
    // One undo step brings back the source and the unit.
    s.undo();
    assert_eq!(s.active_sketch().unwrap().1.dimensions.values().next().unwrap().source, "25");
}

/// Constraint inference off: drawing does not snap or infer (as with Ctrl held). Snap to grid: an unsnapped
/// point goes to the document's snap spacing.
#[test]
fn inference_and_grid_shape_what_the_tools_place() {
    use gc_sketch::{CurveGeom, GeoConstraint};
    use gc_tools::{ToolEvent, ToolKind};
    let px = 1e-4;
    let click = |s: &mut Session, at: [f64; 2]| {
        s.tool_event(ToolEvent::Move { pos: at, px, suppress_snap: false });
        s.tool_event(ToolEvent::Click { pos: at, px, modifier: false });
    };
    // With inference: a line drawn from the end of another joins it.
    let (mut s, _k, _line) = line_dimension_setup();
    s.set_doc_settings(DocSettings { infer_constraints: false, ..DocSettings::default() }).unwrap();
    s.set_tool(ToolKind::Line);
    click(&mut s, [0.03, 0.00001]);
    click(&mut s, [0.05, 0.02]);
    let sk = s.active_sketch().unwrap().1;
    assert!(
        !sk.constraints.values().any(|c| matches!(c, GeoConstraint::Coincident(..))),
        "no join inferred: {:?}",
        sk.constraints
    );
    // Snap to grid with a 5 mm spacing.
    let mut s = Session::new();
    s.set_doc_settings(DocSettings { snap_spacing: [0.005, 0.005], ..DocSettings::default() }).unwrap();
    let _k = s.create_sketch("XY").unwrap();
    s.set_snap_to_grid(true);
    s.set_tool(ToolKind::Line);
    click(&mut s, [0.0123, 0.0071]);
    click(&mut s, [0.0412, 0.0188]);
    let sk = s.active_sketch().unwrap().1;
    let (_, line) = sk.curves.iter().find(|(_, c)| matches!(c.geom, CurveGeom::Line { .. })).unwrap();
    let CurveGeom::Line { a, b } = line.geom else { panic!() };
    let (pa, pb) = (sk.pos(a).unwrap(), sk.pos(b).unwrap());
    for (got, want) in [(pa, [0.01, 0.005]), (pb, [0.04, 0.02])] {
        assert!((got[0] - want[0]).abs() < 1e-12 && (got[1] - want[1]).abs() < 1e-12, "{got:?} vs {want:?}");
    }
}
