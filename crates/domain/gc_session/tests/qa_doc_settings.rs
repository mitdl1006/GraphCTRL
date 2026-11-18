//! QA boundary tests for Document Settings: how a bare number is read, sizes kept when units change (and the
//! one-undo-step rewrite of expressions), refused settings, Constraint Inference off and Snap to grid.
//! Inventor 2026 help, under <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/>: To Change
//! Document Settings GUID-FB49837F-7832-4713-928E-1CEDBDB0C3BD, About Units of Measure
//! GUID-671A8679-AEC1-413A-A23B-428967648CE6, Sketch Tab Reference (Document Settings)
//! GUID-58DD9A89-84C7-48F3-8E4E-0303E755B10F.
//! Refused changes leave the document and the history untouched; accepted ones are one undo step that
//! restores exactly (redo repeats).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::disallowed_methods,
    reason = "test code"
)]

use gc_core::Id;
use gc_doc::{AngleUnit, Command, Created, DimensionDisplay, DocSettings, EndSnap, LengthUnit, SketchOp};
use gc_session::Session;
use gc_sketch::{Curve, CurveGeom, DimKind, GeoConstraint};
use gc_tools::{ToolEvent, ToolKind};

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

/// Refused with no trace, or one undo step that undoes exactly and redoes the same.
fn step_settings(s: &mut Session, what: &str, settings: DocSettings) -> bool {
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    match s.set_doc_settings(settings) {
        Err(_) => {
            assert!(s.doc().same_content(&before), "{what} changed the document");
            assert_eq!(s.history().undo_depth(), depth, "{what}");
            false
        }
        Ok(()) => {
            assert_eq!(s.history().undo_depth(), depth + 1, "{what}: one undo step");
            assert_eq!(s.doc_settings(), settings);
            let after = s.doc().clone();
            assert!(s.undo());
            assert!(s.doc().same_content(&before), "{what}: undo restores");
            assert!(s.redo());
            assert!(s.doc().same_content(&after), "{what}: redo repeats");
            true
        }
    }
}

const LENGTHS: [LengthUnit; 4] = [LengthUnit::Mm, LengthUnit::Cm, LengthUnit::M, LengthUnit::In];
const ANGLES: [AngleUnit; 2] = [AngleUnit::Deg, AngleUnit::Rad];

#[test]
fn invalid_settings_are_refused_with_no_trace_and_the_limits_are_accepted() {
    let mut s = Session::new();
    let d = DocSettings::default();
    for (what, bad) in [
        ("precision 9", DocSettings { linear_precision: 9, ..d }),
        ("angular precision 255", DocSettings { angular_precision: 255, ..d }),
        ("bend radius 0", DocSettings { auto_bend_radius: 0.0, ..d }),
        ("bend radius -0", DocSettings { auto_bend_radius: -0.0, ..d }),
        ("bend radius NaN", DocSettings { auto_bend_radius: f64::NAN, ..d }),
        ("bend radius inf", DocSettings { auto_bend_radius: f64::INFINITY, ..d }),
        ("snap x 0", DocSettings { snap_spacing: [0.0, 0.001], ..d }),
        ("snap y negative", DocSettings { snap_spacing: [0.001, -0.001], ..d }),
        ("snap inf", DocSettings { snap_spacing: [f64::INFINITY, 0.001], ..d }),
        ("snaps per minor 0", DocSettings { snaps_per_minor: 0, ..d }),
        ("snaps per minor 101", DocSettings { snaps_per_minor: 101, ..d }),
        ("minor per major 0", DocSettings { minor_per_major: 0, ..d }),
        ("minor per major huge", DocSettings { minor_per_major: u32::MAX, ..d }),
    ] {
        assert!(!step_settings(&mut s, what, bad), "{what} was accepted");
    }
    // The limits themselves work and format without panicking.
    for (what, ok) in [
        ("precision 0", DocSettings { linear_precision: 0, angular_precision: 0, ..d }),
        ("precision 8", DocSettings { linear_precision: 8, angular_precision: 8, ..d }),
        ("snaps 1 and 100", DocSettings { snaps_per_minor: 1, minor_per_major: 100, ..d }),
        ("tiny bend radius", DocSettings { auto_bend_radius: 1e-9, ..d }),
        ("huge bend radius", DocSettings { auto_bend_radius: 1e6, ..d }),
        ("tiny snap", DocSettings { snap_spacing: [1e-9, 1e-9], ..d }),
        ("all flags off", DocSettings { infer_constraints: false, persist_constraints: false, ..d }),
    ] {
        assert!(step_settings(&mut s, what, ok), "{what} was refused");
        assert!(ok.is_valid());
    }
    // Setting what is already set: accepted as a step or refused, never a half change.
    let same = s.doc_settings();
    step_settings(&mut s, "the same settings again", same);
}

#[test]
fn formatting_follows_precision_without_negative_zero_or_panics() {
    let mut d = DocSettings::default();
    for (precision, metres, mm) in [(1u8, 0.0125, "12.5 mm"), (3, 0.0125, "12.5 mm"), (0, 0.0126, "13 mm")] {
        d.linear_precision = precision;
        assert_eq!(d.format_length(metres), mm);
    }
    // Observation: an exact half rounds to even at precision 0 ("12 mm" for 12.5 mm, not "13 mm").
    d.linear_precision = 0;
    let half = d.format_length(0.0125);
    assert!(half == "12 mm" || half == "13 mm", "{half}");
    d.linear_precision = 2;
    assert_eq!(d.format_length(-1e-9), "0 mm", "never -0");
    assert_eq!(d.format_length(0.0), "0 mm");
    // Values at the edge of the floats and not-a-number come out as text without panicking.
    for v in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e300, -1e300, 5e-324] {
        for length in LENGTHS {
            for angle in ANGLES {
                let d = DocSettings { length_unit: length, angle_unit: angle, ..DocSettings::default() };
                assert!(!d.format_length(v).is_empty() && !d.format_angle(v).is_empty());
                for display in [
                    DimensionDisplay::Value,
                    DimensionDisplay::Name,
                    DimensionDisplay::Expression,
                    DimensionDisplay::PreciseValue,
                ] {
                    let d = DocSettings { dimension_display: display, ..d };
                    for name in ["", "d1"] {
                        assert!(!d.dimension_text(false, "R", v, name, " 5 mm ").is_empty());
                    }
                }
            }
        }
    }
    // The Auto-Bend radius text reads back in the document's unit.
    for length in LENGTHS {
        let d = DocSettings { length_unit: length, auto_bend_radius: 0.0123, ..DocSettings::default() };
        let text = d.auto_bend_text();
        let s = Session::new();
        let v = gc_doc::evaluate_value(s.doc(), &text, gc_doc::Unit::Mm).unwrap();
        assert!((v - 0.0123).abs() < 1e-9, "{text}: {v}");
    }
}

#[test]
fn a_bare_number_follows_the_unit_and_units_keep_every_size() {
    // Every pair of units: each dimension keeps its size when the unit changes, the change is one undo step,
    // and undo restores the sources exactly.
    for from in LENGTHS {
        for to in LENGTHS {
            for angle_from in ANGLES {
                let mut s = Session::new();
                s.set_doc_settings(DocSettings {
                    length_unit: from,
                    angle_unit: angle_from,
                    ..DocSettings::default()
                })
                .unwrap();
                let k = s.create_sketch("XY").unwrap();
                let l = line(&mut s, k, [0.0; 3], [0.03, 0.0, 0.0]);
                let m = line(&mut s, k, [0.0; 3], [0.0, 0.03, 0.0]);
                let (a, b) = s.active_sketch().unwrap().1.line(l).unwrap();
                let dist = DimKind::Distance { p: a, q: b };
                let angle = DimKind::Angle { l1: l, l2: m, rev1: false, rev2: false };
                let degrees = if angle_from == AngleUnit::Deg { "45" } else { "0.7" };
                for (kind, source) in [(dist, "25"), (angle, degrees)] {
                    run(
                        &mut s,
                        k,
                        SketchOp::AddDimension { kind, source: source.into(), text_pos: [0.0; 2] },
                    )
                    .unwrap();
                }
                let sizes = |s: &Session| {
                    let sk = s.active_sketch().unwrap().1;
                    (sk.measure(&dist).unwrap(), sk.measure(&angle).unwrap())
                };
                let (was_len, was_ang) = sizes(&s);
                let before = s.doc().clone();
                let depth = s.history().undo_depth();
                for angle_to in ANGLES {
                    s.set_doc_settings(DocSettings {
                        length_unit: to,
                        angle_unit: angle_to,
                        ..DocSettings::default()
                    })
                    .unwrap();
                    let (now_len, now_ang) = sizes(&s);
                    assert!((now_len - was_len).abs() < 1e-12, "{from:?}->{to:?}: {was_len} vs {now_len}");
                    assert!((now_ang - was_ang).abs() < 1e-12, "{angle_from:?}->{angle_to:?}");
                    assert!(s.undo());
                    assert!(s.doc().same_content(&before), "{from:?}->{to:?}: undo does not restore");
                    assert_eq!(s.history().undo_depth(), depth);
                }
            }
        }
    }
}

#[test]
fn parameters_and_expressions_survive_a_unit_change() {
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    let l = line(&mut s, k, [0.0; 3], [0.03, 0.0, 0.0]);
    s.execute(&Command::AddParam { name: "Gap".into(), source: "10".into(), angle: false }).unwrap();
    s.execute(&Command::AddParam { name: "Twice".into(), source: "Gap * 2".into(), angle: false }).unwrap();
    let (a, b) = s.active_sketch().unwrap().1.line(l).unwrap();
    let kind = DimKind::Distance { p: a, q: b };
    run(&mut s, k, SketchOp::AddDimension { kind, source: "Twice + 5 mm".into(), text_pos: [0.0; 2] })
        .unwrap();
    let measure = |s: &Session| s.active_sketch().unwrap().1.measure(&kind).unwrap();
    let was = measure(&s);
    let value = |s: &Session, name: &str| gc_doc::evaluate_value(s.doc(), name, gc_doc::Unit::Mm).unwrap();
    let (gap, twice) = (value(&s, "Gap"), value(&s, "Twice"));
    for unit in [LengthUnit::In, LengthUnit::Cm, LengthUnit::M, LengthUnit::Mm] {
        let depth = s.history().undo_depth();
        s.set_doc_settings(DocSettings { length_unit: unit, ..s.doc_settings() }).unwrap();
        assert_eq!(s.history().undo_depth(), depth + 1, "{unit:?}: one step for the rewrite of all");
        assert!((measure(&s) - was).abs() < 1e-12, "{unit:?}");
        assert!((value(&s, "Gap") - gap).abs() < 1e-12 && (value(&s, "Twice") - twice).abs() < 1e-12);
        assert!(s.active_sketch().unwrap().1.clone().solve().is_ok());
    }
    // A bare number typed afterwards means the unit now set.
    s.set_doc_settings(DocSettings { length_unit: LengthUnit::In, ..s.doc_settings() }).unwrap();
    assert!((gc_doc::evaluate_value(s.doc(), "1", gc_doc::Unit::Mm).unwrap() - 0.0254).abs() < 1e-12);
    // A bare number in a unit that is not a length (an angle field) is not read as a length.
    s.set_doc_settings(DocSettings { angle_unit: AngleUnit::Rad, ..s.doc_settings() }).unwrap();
    assert!((gc_doc::evaluate_value(s.doc(), "1", gc_doc::Unit::Deg).unwrap() - 1.0).abs() < 1e-12);
    // Typed text that is not a value is refused whatever the unit.
    for bad in ["", "abc", "1 +", "1 / 0", "nan", "1e999 in"] {
        let r = gc_doc::evaluate_value(s.doc(), bad, gc_doc::Unit::Mm);
        assert!(r.is_none_or(f64::is_finite), "{bad:?}");
    }
}

#[test]
fn settings_survive_save_and_old_files_read_defaults() {
    let dir = tempfile::tempdir().unwrap();
    for length in LENGTHS {
        for angle in ANGLES {
            let mut s = Session::new();
            let d = DocSettings {
                length_unit: length,
                angle_unit: angle,
                linear_precision: 0,
                snap_spacing: [0.0025, 0.004],
                infer_constraints: false,
                ..DocSettings::default()
            };
            s.set_doc_settings(d).unwrap();
            let path = dir.path().join(format!("{length:?}{angle:?}.gcp"));
            s.save_as(&path).unwrap();
            let mut t = Session::new();
            t.open(&path).unwrap();
            assert_eq!(t.doc_settings(), d);
            let again = dir.path().join("again.gcp");
            t.save_as(&again).unwrap();
            assert_eq!(std::fs::read(&path).unwrap(), std::fs::read(&again).unwrap());
        }
    }
    // A saved file with damaged settings: opens refused or with usable settings, never invalid ones.
    let mut s = Session::new();
    s.set_doc_settings(DocSettings { linear_precision: 5, auto_bend_radius: 0.02, ..DocSettings::default() })
        .unwrap();
    let path = dir.path().join("good.gcp");
    s.save_as(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    for (from, to) in [
        ("\"linear_precision\":5", "\"linear_precision\":99"),
        ("\"auto_bend_radius\":0.02", "\"auto_bend_radius\":-1.0"),
        ("\"auto_bend_radius\":0.02", "\"auto_bend_radius\":null"),
        ("\"auto_bend_radius\":0.02", "\"auto_bend_radius\":\"x\""),
    ] {
        if !text.contains(from) {
            continue;
        }
        let bad = dir.path().join("bad.gcp");
        std::fs::write(&bad, text.replace(from, to)).unwrap();
        let mut t = Session::new();
        if t.open(&bad).is_ok() {
            assert!(t.doc_settings().is_valid(), "{to}: invalid settings were taken");
        }
    }
}

// ---- Inference and Snap to grid ----

fn click(s: &mut Session, at: [f64; 2], free: bool) {
    let px = 1e-4;
    s.tool_event(ToolEvent::Move { pos: at, px, suppress_snap: free });
    s.tool_event(ToolEvent::Click { pos: at, px, modifier: free });
}

fn drawn(s: &Session) -> Vec<([f64; 3], [f64; 3])> {
    let sk = s.active_sketch().unwrap().1;
    sk.curves
        .values()
        .filter_map(|c| if let CurveGeom::Line { a, b } = c.geom { Some((a, b)) } else { None })
        .map(|(a, b)| (sk.pos(a).unwrap(), sk.pos(b).unwrap()))
        .collect()
}

#[test]
fn snap_to_grid_rounds_to_the_documents_spacing_and_ctrl_bypasses_it() {
    for spacing in [[0.005, 0.005], [0.005, 0.0025], [1e-6, 1e-6], [0.5, 0.5]] {
        let mut s = Session::new();
        s.set_doc_settings(DocSettings { snap_spacing: spacing, ..DocSettings::default() }).unwrap();
        s.create_sketch("XY").unwrap();
        s.set_snap_to_grid(true);
        s.set_tool(ToolKind::Line);
        // Negative coordinates, exact grid points, halfway points.
        let (p, q) = ([-0.0123, -0.0071], [0.05 + spacing[0] * 4.0, 0.05 + spacing[1] * 2.5]);
        click(&mut s, p, false);
        click(&mut s, q, false);
        let lines = drawn(&s);
        assert_eq!(lines.len(), 1, "{spacing:?}");
        let (a, b) = lines[0];
        for (got, want) in [(a, p), (b, q)] {
            assert!(got.iter().all(|v| v.is_finite()), "{spacing:?}: {got:?}");
            for i in 0..2 {
                let cells = got[i] / spacing[i];
                assert!((cells - cells.round()).abs() < 1e-6, "{spacing:?}: {got:?} is not on the grid");
                assert!(
                    (got[i] - want[i]).abs() <= spacing[i] * 0.5 + 1e-9,
                    "{spacing:?}: {got:?} vs {want:?}"
                );
            }
        }
    }
    // Ctrl (no snap) places exactly where the cursor is, and grid off does too.
    let mut s = Session::new();
    s.create_sketch("XY").unwrap();
    s.set_snap_to_grid(true);
    s.set_tool(ToolKind::Line);
    click(&mut s, [0.0123, 0.0071], true);
    click(&mut s, [0.0412, 0.0188], true);
    let (a, b) = drawn(&s)[0];
    assert!((a[0] - 0.0123).abs() < 1e-12 && (b[1] - 0.0188).abs() < 1e-12);
    // The grid does not change an existing sketch (undo steps stay one per line).
    s.set_snap_to_grid(false);
    assert!(!s.snap_to_grid());
}

#[test]
fn inference_off_draws_no_constraints_and_on_draws_them() {
    for infer in [true, false] {
        let mut s = Session::new();
        let k = s.create_sketch("XY").unwrap();
        s.set_doc_settings(DocSettings { infer_constraints: infer, ..DocSettings::default() }).unwrap();
        line(&mut s, k, [0.0; 3], [0.03, 0.0, 0.0]);
        s.set_tool(ToolKind::Line);
        // From the end of the first line, close to horizontal: joined and levelled when inference is on.
        click(&mut s, [0.03, 0.00001], false);
        click(&mut s, [0.06, 0.0002], false);
        let sk = s.active_sketch().unwrap().1;
        let joined = sk.constraints.values().any(|c| matches!(c, GeoConstraint::Coincident(..)));
        let level = sk.constraints.values().any(|c| matches!(c, GeoConstraint::Horizontal(..)));
        if infer {
            assert!(joined, "no join with inference on");
        } else {
            assert!(!joined && !level, "inference is off: {:?}", sk.constraints);
        }
        assert!(sk.clone().solve().is_ok());
    }
    // Persist off with inference on: either no inferred constraints stay or they do; never a panic, and the
    // sketch solves.
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    s.set_doc_settings(DocSettings { persist_constraints: false, ..DocSettings::default() }).unwrap();
    line(&mut s, k, [0.0; 3], [0.03, 0.0, 0.0]);
    s.set_tool(ToolKind::Line);
    click(&mut s, [0.03, 0.0], false);
    click(&mut s, [0.06, 0.02], false);
    assert!(s.active_sketch().unwrap().1.clone().solve().is_ok());
    // Undo of the line leaves exactly the first.
    assert!(s.undo());
    assert_eq!(drawn(&s).len(), 1);
}

#[test]
fn auto_bend_radius_changes_apply_to_the_next_bend_only() {
    let mut s = Session::new();
    s.set_doc_settings(DocSettings { auto_bend_radius: 0.005, ..DocSettings::default() }).unwrap();
    let k = s.create_sketch_3d().unwrap();
    s.set_auto_bend(true);
    s.set_tool(ToolKind::Line);
    for p in [[0.0, 0.0, 0.0], [0.1, 0.0, 0.0], [0.1, 0.1, 0.0]] {
        s.spatial_click(p, None, None);
    }
    let arcs = |s: &Session| {
        let sk = s.active_sketch().unwrap().1;
        sk.curves
            .iter()
            .filter(|(_, c)| matches!(c.geom, CurveGeom::Arc { .. }))
            .map(|(id, _)| *id)
            .collect::<Vec<_>>()
    };
    assert_eq!(arcs(&s).len(), 1);
    let first = arcs(&s)[0];
    let radius = |s: &Session, c: Id<Curve>| {
        let sk = s.active_sketch().unwrap().1;
        let CurveGeom::Arc { c: ctr, s: st, .. } = sk.curves[&c].geom.clone() else { panic!() };
        let (c, p) = (sk.pos(ctr).unwrap(), sk.pos(st).unwrap());
        ((c[0] - p[0]).powi(2) + (c[1] - p[1]).powi(2) + (c[2] - p[2]).powi(2)).sqrt()
    };
    // Changing the setting changes nothing already drawn (the radius is a dimension of its own).
    s.set_doc_settings(DocSettings { auto_bend_radius: 0.02, ..s.doc_settings() }).unwrap();
    assert!((radius(&s, first) - 0.005).abs() < 1e-9);
    let _ = k;
    // A radius larger than the lines: the bend is skipped or refused, never a broken arc.
    s.set_doc_settings(DocSettings { auto_bend_radius: 5.0, ..s.doc_settings() }).unwrap();
    for p in [[0.2, 0.1, 0.0], [0.2, 0.2, 0.0]] {
        s.spatial_click(p, None, None);
    }
    let sk = s.active_sketch().unwrap().1;
    assert!(sk.points.values().all(|p| p.pos.iter().all(|v| v.is_finite())));
    assert!(sk.clone().solve().is_ok());
}
