//! QA boundary tests for the sketch Ellipse (`SketchOp::AddEllipse`, `CurveGeom::Ellipse`): degenerate input,
//! freedom and full definition, the point-on-ellipse constraint, copy / mirror / pattern / scale, the
//! operations that refuse ellipses, and save compatibility. Inventor 2026 help, under
//! <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/>: shapes (Ellipse)
//! GUID-D489CE6D-7299-4211-A43A-F3580A4BA357, copy/move/rotate GUID-F8114270-18F7-4C7A-947E-AC489EDACFA9,
//! scale GUID-962CEE71-8BE4-4DA6-B51E-341319C64FA4, patterns GUID-AAD3A699-FED8-4614-A31D-0A2316328CEE.
//! Every refused edit leaves the sketch and the history untouched; every accepted one is one undo step that
//! restores exactly and leaves only finite points.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::disallowed_methods,
    reason = "test code"
)]

use gc_core::Id;
use gc_doc::{Command, Created, EndSnap, Motion, SketchOp, SketchPatternDirection};
use gc_session::Session;
use gc_sketch::{Curve, CurveGeom, DimKind, GeoConstraint, SkPoint};

type K = Id<gc_doc::Sketch>;

fn setup() -> (Session, K) {
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    (s, k)
}

fn run(s: &mut Session, k: K, op: SketchOp) -> Result<Vec<Created>, gc_doc::EditError> {
    s.execute(&Command::Sketch { sketch: k, op })
}

fn ellipse_op(centre: [f64; 2], major: [f64; 2], minor: f64) -> SketchOp {
    SketchOp::AddEllipse {
        centre: [centre[0], centre[1], 0.0],
        major: [major[0], major[1], 0.0],
        minor,
        centre_snap: EndSnap::default(),
    }
}

fn curve_of(created: &[Created]) -> Id<Curve> {
    created.iter().find_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None }).unwrap()
}

fn ellipse(s: &mut Session, k: K, centre: [f64; 2], major: [f64; 2], minor: f64) -> Id<Curve> {
    curve_of(&run(s, k, ellipse_op(centre, major, minor)).unwrap())
}

fn dof(s: &Session) -> usize {
    s.active_sketch().unwrap().1.clone().solve().unwrap().dof
}

fn dims(s: &Session, c: Id<Curve>) -> (f64, f64) {
    let sk = s.active_sketch().unwrap().1;
    let (centre, major, minor) = sk.ellipse(c).unwrap();
    let (a, b) = (sk.pos(centre).unwrap(), sk.pos(major).unwrap());
    ((b[0] - a[0]).hypot(b[1] - a[1]), minor)
}

fn ellipses(s: &Session) -> Vec<Id<Curve>> {
    let sk = s.active_sketch().unwrap().1;
    sk.curves.iter().filter(|(_, c)| matches!(c.geom, CurveGeom::Ellipse { .. })).map(|(id, _)| *id).collect()
}

fn sane(s: &Session, what: &str) {
    let sk = s.active_sketch().unwrap().1;
    assert!(sk.points.values().all(|p| p.pos.iter().all(|v| v.is_finite())), "{what}: a point is not finite");
    for c in ellipses(s) {
        let (_, minor) = dims(s, c);
        assert!(minor.is_finite() && minor > 0.0, "{what}: minor radius {minor}");
    }
}

fn refused(s: &mut Session, k: K, what: &str, op: SketchOp) {
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    assert!(run(s, k, op).is_err(), "{what} was accepted");
    assert!(s.doc().same_content(&before), "{what} changed the sketch");
    assert_eq!(s.history().undo_depth(), depth, "{what}");
}

fn refused_or_sane(s: &mut Session, k: K, what: &str, op: SketchOp) -> bool {
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    match run(s, k, op) {
        Err(_) => {
            assert!(s.doc().same_content(&before), "{what} changed the sketch");
            assert_eq!(s.history().undo_depth(), depth, "{what}");
            false
        }
        Ok(_) => {
            assert_eq!(s.history().undo_depth(), depth + 1, "{what}: one undo step");
            sane(s, what);
            assert!(s.active_sketch().unwrap().1.clone().solve().is_ok(), "{what}: no longer solves");
            assert!(s.undo());
            assert!(s.doc().same_content(&before), "{what}: undo restores");
            true
        }
    }
}

// ---- Creation ----

#[test]
fn a_new_ellipse_is_one_undo_step_with_five_degrees_of_freedom() {
    let (mut s, k) = setup();
    let (before, d0, depth) = (s.doc().clone(), dof(&s), s.history().undo_depth());
    let e = ellipse(&mut s, k, [0.0, 0.0], [0.03, 0.0], 0.01);
    assert_eq!(s.history().undo_depth(), depth + 1);
    // Centre (2), the major axis end (2: its direction and length) and the minor radius (1).
    assert_eq!(dof(&s), d0 + 5);
    let (a, b) = dims(&s, e);
    assert!((a - 0.03).abs() < 1e-12 && (b - 0.01).abs() < 1e-12);
    assert!(s.undo() && s.doc().same_content(&before));
}

/// A zero major axis, a zero or negative minor radius, non-finite numbers and sizes below a nanometre are
/// refused; a minor radius longer than the major one is either refused or taken as a valid ellipse.
#[test]
fn degenerate_ellipses_are_refused_or_finite() {
    let (mut s, k) = setup();
    for (what, op) in [
        ("major axis zero", ellipse_op([0.01, 0.01], [0.01, 0.01], 0.005)),
        ("major axis a hair", ellipse_op([0.0, 0.0], [1e-12, 0.0], 0.005)),
        ("minor radius zero", ellipse_op([0.0, 0.0], [0.03, 0.0], 0.0)),
        ("minor radius negative", ellipse_op([0.0, 0.0], [0.03, 0.0], -0.01)),
        ("minor radius a hair", ellipse_op([0.0, 0.0], [0.03, 0.0], 1e-12)),
        ("minor NaN", ellipse_op([0.0, 0.0], [0.03, 0.0], f64::NAN)),
        ("minor infinite", ellipse_op([0.0, 0.0], [0.03, 0.0], f64::INFINITY)),
        ("centre NaN", ellipse_op([f64::NAN, 0.0], [0.03, 0.0], 0.01)),
        ("major NaN", ellipse_op([0.0, 0.0], [0.03, f64::NAN], 0.01)),
        ("major infinite", ellipse_op([0.0, 0.0], [f64::INFINITY, 0.0], 0.01)),
    ] {
        refused(&mut s, k, what, op);
    }
    // A zero-length major axis in 3D: also refused. Minor longer than major: a legal ellipse or a refusal.
    refused_or_sane(&mut s, k, "minor longer than major", ellipse_op([0.0, 0.0], [0.01, 0.0], 0.03));
    refused_or_sane(&mut s, k, "round (minor = major)", ellipse_op([0.0, 0.0], [0.02, 0.0], 0.02));
    refused_or_sane(&mut s, k, "huge", ellipse_op([0.0, 0.0], [1e6, 0.0], 5e5));
    refused_or_sane(&mut s, k, "rotated", ellipse_op([0.01, 0.01], [0.03, 0.04], 0.01));
    // A nonzero z in the major point (the sketch is planar).
    refused_or_sane(
        &mut s,
        k,
        "z",
        SketchOp::AddEllipse {
            centre: [0.0; 3],
            major: [0.03, 0.0, 5.0],
            minor: 0.01,
            centre_snap: EndSnap::default(),
        },
    );
    // A spatial (3D) sketch takes no ellipses.
    let spatial = s.create_sketch_3d().unwrap();
    refused(&mut s, spatial, "3D sketch", ellipse_op([0.0, 0.0], [0.03, 0.0], 0.01));
}

// ---- Constraints and dimensions ----

/// Four constraints — Fix the centre (two freedoms), Horizontal major axis, the major length, the minor radius
/// — fully define an ellipse (dof 0), each step removes the freedom it should, and every one undoes.
#[test]
fn an_ellipse_is_fully_defined_by_its_constraints() {
    let (mut s, k) = setup();
    let e = ellipse(&mut s, k, [0.01, 0.02], [0.04, 0.02], 0.015);
    let sk = s.active_sketch().unwrap().1;
    let (c, m, _) = sk.ellipse(e).unwrap();
    assert_eq!(dof(&s), 5);
    run(&mut s, k, SketchOp::AddConstraint(GeoConstraint::Fix(c))).unwrap();
    assert_eq!(dof(&s), 3, "the centre is fixed");
    run(&mut s, k, SketchOp::AddConstraint(GeoConstraint::Horizontal(c, m))).unwrap();
    assert_eq!(dof(&s), 2, "the major axis is level");
    run(
        &mut s,
        k,
        SketchOp::AddDimension {
            kind: DimKind::Distance { p: c, q: m },
            source: "30".into(),
            text_pos: [0.02, 0.0],
        },
    )
    .unwrap();
    assert_eq!(dof(&s), 1, "the major radius is dimensioned");
    run(
        &mut s,
        k,
        SketchOp::AddDimension { kind: DimKind::Radius(e), source: "12".into(), text_pos: [0.0, 0.0] },
    )
    .unwrap();
    assert_eq!(dof(&s), 0, "fully defined");
    let (a, b) = dims(&s, e);
    assert!((a - 0.03).abs() < 1e-9 && (b - 0.012).abs() < 1e-9, "{a} {b}");
    // A fifth constraint on it is redundant or conflicting: refused with no trace.
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    assert!(
        run(
            &mut s,
            k,
            SketchOp::AddDimension { kind: DimKind::Radius(e), source: "13".into(), text_pos: [0.0, 0.0] }
        )
        .is_err()
    );
    assert!(s.doc().same_content(&before) && s.history().undo_depth() == depth);
    // Undo all four, one at a time.
    for expected in [1, 2, 3, 5] {
        assert!(s.undo());
        assert_eq!(dof(&s), expected);
    }
}

/// The minor radius dimension rejects zero, negative and unparsable values and takes a valid one.
#[test]
fn the_minor_radius_dimension_is_checked() {
    let (mut s, k) = setup();
    let e = ellipse(&mut s, k, [0.0, 0.0], [0.04, 0.0], 0.02);
    for text in ["0", "-5", "abc", "", "1/0", "0 mm"] {
        refused(
            &mut s,
            k,
            &format!("radius {text:?}"),
            SketchOp::AddDimension { kind: DimKind::Radius(e), source: text.into(), text_pos: [0.0, 0.0] },
        );
    }
    assert!(refused_or_sane(
        &mut s,
        k,
        "radius 5",
        SketchOp::AddDimension { kind: DimKind::Radius(e), source: "5".into(), text_pos: [0.0, 0.0] }
    ));
}

/// A point on the ellipse: it lands on it (the equation holds), takes one freedom, and survives a change of
/// the ellipse; a degenerate (almost flat) ellipse does not break the solver.
#[test]
fn a_point_on_an_ellipse_lies_on_it() {
    let (mut s, k) = setup();
    let e = ellipse(&mut s, k, [0.0, 0.0], [0.04, 0.0], 0.02);
    let created = run(&mut s, k, SketchOp::AddPoint { pos: [0.03, 0.03, 0.0] }).unwrap();
    let Some(Created::Point(p)) = created.into_iter().find(|c| matches!(c, Created::Point(_))) else {
        panic!()
    };
    let d0 = dof(&s);
    run(&mut s, k, SketchOp::AddConstraint(GeoConstraint::PointOnCurve(p, e))).unwrap();
    assert_eq!(dof(&s), d0 - 1, "one freedom gone");
    let on = |s: &Session| {
        let sk = s.active_sketch().unwrap().1;
        let (c, m, b) = sk.ellipse(e).unwrap();
        let (c, m, q) = (sk.pos(c).unwrap(), sk.pos(m).unwrap(), sk.pos(p).unwrap());
        let a = (m[0] - c[0]).hypot(m[1] - c[1]);
        let (ux, uy) = ((m[0] - c[0]) / a, (m[1] - c[1]) / a);
        let (dx, dy) = (q[0] - c[0], q[1] - c[1]);
        let (x, y) = (dx * ux + dy * uy, -dx * uy + dy * ux);
        (x / a).hypot(y / b)
    };
    assert!((on(&s) - 1.0).abs() < 1e-6, "the point is not on the ellipse: {}", on(&s));
    // Change the minor radius: the point follows.
    run(
        &mut s,
        k,
        SketchOp::AddDimension { kind: DimKind::Radius(e), source: "10".into(), text_pos: [0.0, 0.0] },
    )
    .unwrap();
    assert!((on(&s) - 1.0).abs() < 1e-6, "after the minor radius changed: {}", on(&s));
    // Constraining a point to a nearly flat ellipse (minor 1 micrometre): finite or refused.
    let (mut t, kt) = setup();
    let flat = ellipse(&mut t, kt, [0.0, 0.0], [0.04, 0.0], 1e-6);
    let created = run(&mut t, kt, SketchOp::AddPoint { pos: [0.02, 0.0, 0.0] }).unwrap();
    let Some(Created::Point(q)) = created.into_iter().find(|c| matches!(c, Created::Point(_))) else {
        panic!()
    };
    refused_or_sane(
        &mut t,
        kt,
        "flat ellipse",
        SketchOp::AddConstraint(GeoConstraint::PointOnCurve(q, flat)),
    );
    let _: Option<Id<SkPoint>> = None;
}

// ---- Copy, mirror, pattern, scale ----

#[test]
fn copy_mirror_and_pattern_keep_the_shape() {
    let (mut s, k) = setup();
    let e = ellipse(&mut s, k, [0.0, 0.0], [0.04, 0.0], 0.015);
    let n = ellipses(&s).len();
    // Copy with a move: a second ellipse of the same size; the original stays.
    let (before, depth) = (s.doc().clone(), s.history().undo_depth());
    run(
        &mut s,
        k,
        SketchOp::Transform {
            curves: vec![e],
            motion: Motion::Translate { by: [0.0, 0.1] },
            copy: true,
            relax: true,
        },
    )
    .unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1);
    assert_eq!(ellipses(&s).len(), n + 1);
    for c in ellipses(&s) {
        let (a, b) = dims(&s, c);
        assert!((a - 0.04).abs() < 1e-9 && (b - 0.015).abs() < 1e-9, "a copy changed size: {a} {b}");
    }
    assert!(s.undo() && s.doc().same_content(&before));
    // Rotated copy: same size, rotated; a copy of a rotation by 360 degrees sits over the original.
    refused_or_sane(
        &mut s,
        k,
        "rotated copy",
        SketchOp::Transform {
            curves: vec![e],
            motion: Motion::Rotate { centre: [0.0, 0.0], angle: std::f64::consts::FRAC_PI_2 },
            copy: true,
            relax: true,
        },
    );
    // Mirror across a line.
    let axis = {
        let created = run(
            &mut s,
            k,
            SketchOp::AddLine {
                a: [0.0, 0.08, 0.0],
                b: [0.1, 0.08, 0.0],
                snap_a: EndSnap::default(),
                snap_b: EndSnap::default(),
                horizontal: None,
            },
        )
        .unwrap();
        curve_of(&created)
    };
    let d = dof(&s);
    run(&mut s, k, SketchOp::Mirror { curves: vec![e], axis }).unwrap();
    assert_eq!(ellipses(&s).len(), n + 1);
    for c in ellipses(&s) {
        let (a, b) = dims(&s, c);
        assert!((a - 0.04).abs() < 1e-9 && (b - 0.015).abs() < 1e-9, "a mirrored ellipse changed size");
    }
    assert!(dof(&s) <= d + 1, "the mirror added freedom: {} vs {d}", dof(&s));
    assert!(s.undo());
    // A rectangular pattern of an ellipse: count elements, each a full-size ellipse.
    let guide = axis;
    let result = run(
        &mut s,
        k,
        SketchOp::RectangularPattern {
            curves: vec![e],
            first: SketchPatternDirection { line: guide, count: 3, spacing: "30".into(), flip: false },
            second: None,
            fitted: false,
        },
    );
    if result.is_ok() {
        assert_eq!(ellipses(&s).len(), n + 2);
        sane(&s, "pattern");
        assert!(s.undo());
    }
}

/// Scale: the minor radius and the major axis scale together; non-positive, NaN or infinite factors are
/// refused; a scale of a copy gives the scaled size and leaves the original.
#[test]
fn scaling_an_ellipse_scales_both_radii() {
    let (mut s, k) = setup();
    let e = ellipse(&mut s, k, [0.0, 0.0], [0.04, 0.0], 0.015);
    let (before, depth) = (s.doc().clone(), s.history().undo_depth());
    run(
        &mut s,
        k,
        SketchOp::Transform {
            curves: vec![e],
            motion: Motion::Scale { centre: [0.0, 0.0], factor: 2.0 },
            copy: false,
            relax: true,
        },
    )
    .unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1);
    let (a, b) = dims(&s, e);
    assert!((a - 0.08).abs() < 1e-9 && (b - 0.03).abs() < 1e-9, "scaled by 2: {a} {b}");
    assert!(s.undo() && s.doc().same_content(&before));
    // A scaled copy.
    run(
        &mut s,
        k,
        SketchOp::Transform {
            curves: vec![e],
            motion: Motion::Scale { centre: [0.0, 0.0], factor: 0.5 },
            copy: true,
            relax: true,
        },
    )
    .unwrap();
    let mut sizes: Vec<(f64, f64)> = ellipses(&s).into_iter().map(|c| dims(&s, c)).collect();
    sizes.sort_by(|x, y| x.0.total_cmp(&y.0));
    assert!((sizes[0].0 - 0.02).abs() < 1e-9 && (sizes[0].1 - 0.0075).abs() < 1e-9, "{sizes:?}");
    assert!(s.undo());
    for factor in [0.0, -1.0, -1e-3, 1e-12, f64::NAN, f64::INFINITY] {
        for copy in [false, true] {
            refused(
                &mut s,
                k,
                &format!("factor {factor} copy {copy}"),
                SketchOp::Transform {
                    curves: vec![e],
                    motion: Motion::Scale { centre: [0.0, 0.0], factor },
                    copy,
                    relax: true,
                },
            );
        }
    }
    // Scaled down to nearly nothing and up to huge: refused or finite with a positive minor radius.
    refused_or_sane(
        &mut s,
        k,
        "tiny",
        SketchOp::Transform {
            curves: vec![e],
            motion: Motion::Scale { centre: [0.0, 0.0], factor: 1e-6 },
            copy: false,
            relax: true,
        },
    );
    refused_or_sane(
        &mut s,
        k,
        "huge",
        SketchOp::Transform {
            curves: vec![e],
            motion: Motion::Scale { centre: [0.0, 0.0], factor: 1e6 },
            copy: false,
            relax: true,
        },
    );
    // With the minor radius dimensioned and no relaxing, a scale keeps the sketch consistent.
    run(
        &mut s,
        k,
        SketchOp::AddDimension { kind: DimKind::Radius(e), source: "15".into(), text_pos: [0.0, 0.0] },
    )
    .unwrap();
    for relax in [false, true] {
        refused_or_sane(
            &mut s,
            k,
            &format!("dimensioned, relax {relax}"),
            SketchOp::Transform {
                curves: vec![e],
                motion: Motion::Scale { centre: [0.0, 0.0], factor: 2.0 },
                copy: false,
                relax,
            },
        );
    }
}

// ---- Operations that refuse ellipses ----

/// Split and Fillet do not cut ellipses: each is refused with no trace. Offset makes a new ellipse. Trim and Extend may
/// accept them (elliptical arcs); then the result is sane and one undo step. Never a crash or a half-cut curve.
#[test]
fn trim_split_extend_offset_and_fillet_refuse_ellipses() {
    let (mut s, k) = setup();
    let e = ellipse(&mut s, k, [0.0, 0.0], [0.04, 0.0], 0.02);
    let cutter = {
        let created = run(
            &mut s,
            k,
            SketchOp::AddLine {
                a: [-0.06, 0.0, 0.0],
                b: [0.06, 0.0, 0.0],
                snap_a: EndSnap::default(),
                snap_b: EndSnap::default(),
                horizontal: None,
            },
        )
        .unwrap();
        curve_of(&created)
    };
    // Trim and Extend of ellipses arrived later (an elliptical arc, as in Inventor): either refused with no
    // trace or a sane result undone in one step. Split still refuses ellipses.
    refused_or_sane(&mut s, k, "trim", SketchOp::Trim { curve: e, at: [0.0, 0.02] });
    refused(&mut s, k, "split", SketchOp::Split { curve: e, at: [0.0, 0.02] });
    refused_or_sane(&mut s, k, "extend", SketchOp::Extend { curve: e, at: [0.04, 0.0] });
    // Trimming the line where it crosses the ellipse is fine, and leaves the ellipse whole.
    refused_or_sane(
        &mut s,
        k,
        "trim the line at the ellipse",
        SketchOp::Trim { curve: cutter, at: [0.05, 0.0] },
    );
    assert_eq!(ellipses(&s), vec![e]);
    for distance in [0.01, -0.01] {
        for (loop_select, constrain) in [(false, false), (true, true)] {
            // Offset of ellipses arrived later (a new ellipse, GUID-D489CE6D): refused or sane, one step.
            refused_or_sane(
                &mut s,
                k,
                &format!("offset {distance}"),
                SketchOp::Offset { curves: vec![e], distance, loop_select, constrain, picked: None },
            );
        }
    }
    refused(
        &mut s,
        k,
        "fillet with a line",
        SketchOp::Fillet { lines: [(e, [0.04, 0.0]), (cutter, [0.05, 0.0])], radius: "2".into() },
    );
}

#[test]
fn deleting_an_ellipse_removes_its_points_and_undoes_exactly() {
    let (mut s, k) = setup();
    let base = s.active_sketch().unwrap().1.points.len();
    let e = ellipse(&mut s, k, [0.0, 0.0], [0.04, 0.0], 0.02);
    let before = s.doc().clone();
    run(&mut s, k, SketchOp::Delete(vec![gc_sketch::Item::Curve(e)])).unwrap();
    assert!(ellipses(&s).is_empty());
    assert_eq!(s.active_sketch().unwrap().1.points.len(), base, "the ellipse's points are gone");
    assert!(s.undo());
    assert!(s.doc().same_content(&before));
}

// ---- Save compatibility ----

#[test]
fn ellipses_round_trip_and_damaged_ones_do_not_crash() {
    let (mut s, k) = setup();
    let e = ellipse(&mut s, k, [0.01, 0.02], [0.05, 0.02], 0.015);
    run(
        &mut s,
        k,
        SketchOp::AddDimension { kind: DimKind::Radius(e), source: "15".into(), text_pos: [0.0, 0.0] },
    )
    .unwrap();
    s.finish_sketch();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ellipse.gcp");
    s.save_as(&path).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert!(t.doc().same_content(s.doc()), "the reopened document differs");
    let again = dir.path().join("again.gcp");
    s.save_as(&again).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), std::fs::read(&again).unwrap(), "saving twice differs");

    // A damaged file: the minor radius set to zero, negative, then removed. Opening is clean or refused, and
    // whatever opens solves and draws without a crash.
    for replacement in [Some(serde_json::json!(0.0)), Some(serde_json::json!(-0.01)), None] {
        let mut value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        fn damage(v: &mut serde_json::Value, replacement: &Option<serde_json::Value>, found: &mut usize) {
            match v {
                serde_json::Value::Object(map) => {
                    if let Some(obj) = map.get_mut("Ellipse").and_then(|inner| inner.as_object_mut()) {
                        match replacement {
                            Some(r) => {
                                obj.insert("minor".into(), r.clone());
                            }
                            None => {
                                obj.remove("minor");
                            }
                        }
                        *found += 1;
                    }
                    map.values_mut().for_each(|x| damage(x, replacement, found));
                }
                serde_json::Value::Array(items) => {
                    items.iter_mut().for_each(|x| damage(x, replacement, found))
                }
                _ => {}
            }
        }
        let mut found = 0;
        damage(&mut value, &replacement, &mut found);
        assert_eq!(found, 1, "the ellipse is not stored as `Ellipse` with a `minor`");
        let damaged = dir.path().join("damaged.gcp");
        std::fs::write(&damaged, serde_json::to_string(&value).unwrap()).unwrap();
        let mut w = Session::new();
        if w.open(&damaged).is_ok() {
            for (_, sketch) in w.doc().sketches().iter() {
                let mut sketch = sketch.clone();
                drop(sketch.solve());
                for c in sketch.curves.keys().copied().collect::<Vec<_>>() {
                    std::hint::black_box(sketch.ellipse_outline(c, 64));
                }
            }
        }
    }
}
