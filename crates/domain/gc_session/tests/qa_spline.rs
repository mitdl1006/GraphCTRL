//! QA boundary tests for the sketch Spline (`SketchOp::AddSpline`, `CurveGeom::Spline`): degenerate fit
//! points, Trim, Split, Split Spline, Offset, copy / mirror / scale, constraints, and save compatibility.
//! Inventor 2026 help, under <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/>: shapes
//! (Spline) GUID-D489CE6D-7299-4211-A43A-F3580A4BA357, create and edit splines (Split Spline)
//! GUID-DB15C479-66F2-456A-9E54-40A60F83DC92, split/trim GUID-DD00037F-6BBA-4B95-AA90-4F99DCEB667D, offset
//! GUID-962CEE71-8BE4-4DA6-B51E-341319C64FA4.
//! Every refused edit leaves the sketch and the history untouched; every accepted one is one undo step that
//! restores exactly, leaves only finite points and a sketch that solves.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::disallowed_methods,
    reason = "test code"
)]

use gc_core::Id;
use gc_doc::{Command, Created, EndSnap, Motion, SketchOp};
use gc_session::Session;
use gc_sketch::{Curve, CurveGeom, GeoConstraint, SkPoint};

type K = Id<gc_doc::Sketch>;

fn mm(x: f64, y: f64) -> [f64; 3] {
    [x * 1e-3, y * 1e-3, 0.0]
}

fn setup() -> (Session, K) {
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    (s, k)
}

fn run(s: &mut Session, k: K, op: SketchOp) -> Result<Vec<Created>, gc_doc::EditError> {
    s.execute(&Command::Sketch { sketch: k, op })
}

fn spline_op(points: Vec<[f64; 3]>) -> SketchOp {
    SketchOp::AddSpline { points, snaps: Vec::new() }
}

fn curve_of(created: &[Created]) -> Id<Curve> {
    created.iter().find_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None }).unwrap()
}

fn spline(s: &mut Session, k: K, points: Vec<[f64; 3]>) -> Id<Curve> {
    curve_of(&run(s, k, spline_op(points)).unwrap())
}

fn line(s: &mut Session, k: K, a: [f64; 3], b: [f64; 3]) -> Id<Curve> {
    curve_of(
        &run(
            s,
            k,
            SketchOp::AddLine {
                a,
                b,
                snap_a: EndSnap::default(),
                snap_b: EndSnap::default(),
                horizontal: None,
            },
        )
        .unwrap(),
    )
}

fn splines(s: &Session) -> Vec<(Id<Curve>, Vec<Id<SkPoint>>)> {
    let sk = s.active_sketch().unwrap().1;
    sk.curves
        .iter()
        .filter_map(|(id, c)| {
            if let CurveGeom::Spline { points } = &c.geom { Some((*id, points.clone())) } else { None }
        })
        .collect()
}

fn sane(s: &Session, what: &str) {
    let sk = s.active_sketch().unwrap().1;
    assert!(sk.points.values().all(|p| p.pos.iter().all(|v| v.is_finite())), "{what}: a point is not finite");
    for (id, points) in splines(s) {
        assert!(points.len() >= 2, "{what}: spline {id:?} has {} fit points", points.len());
        assert!(points.iter().all(|p| sk.points.contains_key(p)), "{what}: a fit point is missing");
    }
    assert!(sk.clone().solve().is_ok(), "{what}: the sketch no longer solves");
}

fn refused(s: &mut Session, k: K, what: &str, op: SketchOp) {
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    assert!(run(s, k, op).is_err(), "{what} was accepted");
    assert!(s.doc().same_content(&before), "{what} changed the sketch");
    assert_eq!(s.history().undo_depth(), depth, "{what}");
}

/// Refused with no trace, or one undo step that undoes exactly and leaves a sane sketch (and is then redone,
/// so the next step builds on it). Returns whether it was accepted.
fn step(s: &mut Session, k: K, what: &str, op: SketchOp) -> bool {
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
            let after = s.doc().clone();
            assert!(s.undo());
            assert!(s.doc().same_content(&before), "{what}: undo restores");
            assert!(s.redo());
            assert!(s.doc().same_content(&after), "{what}: redo repeats");
            true
        }
    }
}

fn ghost() -> Id<Curve> {
    Id::<Curve>::from_raw(std::num::NonZeroU64::new(9999).unwrap())
}

// ---- Creation ----

#[test]
fn a_spline_needs_two_distinct_finite_fit_points() {
    let (mut s, k) = setup();
    for (what, points) in [
        ("no points", vec![]),
        ("one point", vec![mm(0.0, 0.0)]),
        ("two equal points", vec![mm(5.0, 5.0), mm(5.0, 5.0)]),
        ("two points a hair apart", vec![mm(0.0, 0.0), [1e-12, 0.0, 0.0]]),
        ("a repeated point in the middle", vec![mm(0.0, 0.0), mm(10.0, 5.0), mm(10.0, 5.0), mm(20.0, 0.0)]),
        ("NaN", vec![mm(0.0, 0.0), [f64::NAN, 0.0, 0.0], mm(20.0, 0.0)]),
        ("infinite", vec![mm(0.0, 0.0), [0.0, f64::INFINITY, 0.0]]),
    ] {
        refused(&mut s, k, what, spline_op(points));
    }
    let spatial = s.create_sketch_3d().unwrap();
    refused(&mut s, spatial, "a spline in a 3D sketch", spline_op(vec![mm(0.0, 0.0), mm(10.0, 5.0)]));
}

/// Shapes that are legal but awkward: two points (a straight spline), collinear points, the first point
/// again at the end (a loop), a zig-zag, a thousand points, huge coordinates, a nonzero z.
#[test]
fn awkward_but_legal_splines_are_one_undo_step_and_stay_finite() {
    let (mut s, k) = setup();
    let wiggle: Vec<[f64; 3]> = (0..1000).map(|i| mm(f64::from(i) * 0.05, f64::from(i % 7) * 0.3)).collect();
    for (what, points) in [
        ("two points", vec![mm(0.0, 0.0), mm(30.0, 10.0)]),
        ("collinear", vec![mm(0.0, 0.0), mm(10.0, 10.0), mm(20.0, 20.0), mm(30.0, 30.0)]),
        ("closed (first again at the end)", vec![mm(0.0, 0.0), mm(10.0, 10.0), mm(20.0, 0.0), mm(0.0, 0.0)]),
        ("A, B, A", vec![mm(0.0, 0.0), mm(10.0, 10.0), mm(0.0, 0.0)]),
        ("zig-zag", vec![mm(0.0, 0.0), mm(10.0, 10.0), mm(0.1, 20.0), mm(10.0, 30.0)]),
        ("a thousand points", wiggle),
        ("huge", vec![mm(0.0, 0.0), mm(1e9, 1e9), mm(2e9, 0.0)]),
        ("z", vec![[0.0, 0.0, 5.0], mm(10.0, 10.0)]),
    ] {
        step(&mut s, k, what, spline_op(points));
    }
}

/// A spline needs as many snaps as it likes: fewer than points, more than points, snapped to a point and a
/// curve; extra snaps are ignored and none of it panics.
#[test]
fn snaps_may_be_missing_or_too_many() {
    let (mut s, k) = setup();
    let wall = line(&mut s, k, mm(0.0, 0.0), mm(0.0, 40.0));
    let anchor = {
        let created = run(&mut s, k, SketchOp::AddPoint { pos: mm(20.0, 20.0) }).unwrap();
        created.into_iter().find_map(|c| if let Created::Point(p) = c { Some(p) } else { None }).unwrap()
    };
    let pts = vec![mm(0.0, 10.0), mm(10.0, 15.0), mm(20.0, 20.0)];
    let on_wall = EndSnap { curve: Some(wall), ..EndSnap::default() };
    let on_point = EndSnap { point: Some(anchor), ..EndSnap::default() };
    for (what, snaps) in [
        ("none", vec![]),
        ("fewer", vec![on_wall]),
        ("exact", vec![on_wall, EndSnap::default(), on_point]),
        ("more", vec![on_wall, EndSnap::default(), on_point, on_wall, on_point]),
    ] {
        step(&mut s, k, what, SketchOp::AddSpline { points: pts.clone(), snaps });
    }
    // Snaps naming things that do not exist: refused or sane, never a dangling constraint.
    let ghost_snap = EndSnap { curve: Some(ghost()), ..EndSnap::default() };
    step(&mut s, k, "ghost curve snap", SketchOp::AddSpline { points: pts, snaps: vec![ghost_snap] });
}

// ---- Trim, Split, Split Spline ----

#[test]
fn trim_and_split_a_spline_at_crossings() {
    let (mut s, k) = setup();
    let sp =
        spline(&mut s, k, vec![mm(0.0, 0.0), mm(15.0, 12.0), mm(30.0, 0.0), mm(45.0, 12.0), mm(60.0, 0.0)]);
    let (a, b) =
        (line(&mut s, k, mm(20.0, -20.0), mm(20.0, 30.0)), line(&mut s, k, mm(40.0, -20.0), mm(40.0, 30.0)));
    let _ = (a, b);
    // The middle piece between the two cutters, the end pieces, and a click far from the curve.
    for (what, at) in [
        ("middle", [0.03, 0.0]),
        ("left end", [0.002, 0.001]),
        ("right end", [0.058, 0.0005]),
        ("far away", [0.0, 0.2]),
    ] {
        step(&mut s, k, &format!("trim {what}"), SketchOp::Trim { curve: sp, at });
    }
    // Trim twice in a row: the second trim works on what the first left.
    if run(&mut s, k, SketchOp::Trim { curve: sp, at: [0.03, 0.0] }).is_ok() {
        sane(&s, "after the first trim");
        for (id, points) in splines(&s) {
            let at = {
                let sk = s.active_sketch().unwrap().1;
                let p = sk.pos(points[0]).unwrap();
                [p[0], p[1]]
            };
            step(&mut s, k, &format!("second trim of spline {id:?}"), SketchOp::Trim { curve: id, at });
        }
    }
    // Non-finite and ghost inputs.
    let (mut t, kt) = setup();
    let sp2 = spline(&mut t, kt, vec![mm(0.0, 0.0), mm(15.0, 12.0), mm(30.0, 0.0)]);
    line(&mut t, kt, mm(10.0, -20.0), mm(10.0, 30.0));
    for at in [[f64::NAN, 0.0], [0.0, f64::INFINITY], [1e300, 1e300]] {
        step(&mut t, kt, &format!("trim at {at:?}"), SketchOp::Trim { curve: sp2, at });
    }
    refused(&mut t, kt, "trim a ghost", SketchOp::Trim { curve: ghost(), at: [0.0, 0.0] });
}

/// A cutter that touches a spline only at a fit point, or lies along it, or is a copy of it.
#[test]
fn cutters_at_fit_points_and_along_the_spline() {
    let (mut s, k) = setup();
    let sp = spline(&mut s, k, vec![mm(0.0, 0.0), mm(15.0, 12.0), mm(30.0, 0.0)]);
    let through_fit = line(&mut s, k, mm(15.0, -20.0), mm(15.0, 30.0));
    step(&mut s, k, "trim at a cutter through a fit point", SketchOp::Trim { curve: sp, at: [0.005, 0.005] });
    step(&mut s, k, "trim the cutter", SketchOp::Trim { curve: through_fit, at: [0.015, 0.02] });
    // A twin spline on top of it.
    let twin = spline(&mut s, k, vec![mm(0.0, 0.0), mm(15.0, 12.0), mm(30.0, 0.0)]);
    step(&mut s, k, "trim a spline against its twin", SketchOp::Trim { curve: sp, at: [0.005, 0.005] });
    let _ = twin;
}

#[test]
fn split_and_split_spline_checks() {
    let (mut s, k) = setup();
    let sp = spline(&mut s, k, vec![mm(0.0, 0.0), mm(10.0, 10.0), mm(20.0, 0.0), mm(30.0, 10.0)]);
    let (_, points) = splines(&s).into_iter().find(|(id, _)| *id == sp).unwrap();
    // Only an inner fit point can split a spline.
    refused(&mut s, k, "split at the first fit point", SketchOp::SplitSpline { curve: sp, point: points[0] });
    refused(&mut s, k, "split at the last fit point", SketchOp::SplitSpline { curve: sp, point: points[3] });
    let other = {
        let created = run(&mut s, k, SketchOp::AddPoint { pos: mm(5.0, 5.0) }).unwrap();
        created.into_iter().find_map(|c| if let Created::Point(p) = c { Some(p) } else { None }).unwrap()
    };
    refused(
        &mut s,
        k,
        "split at a point that is not a fit point",
        SketchOp::SplitSpline { curve: sp, point: other },
    );
    refused(&mut s, k, "split a ghost", SketchOp::SplitSpline { curve: ghost(), point: points[1] });
    let line_id = line(&mut s, k, mm(0.0, 20.0), mm(10.0, 20.0));
    refused(
        &mut s,
        k,
        "split a line as a spline",
        SketchOp::SplitSpline { curve: line_id, point: points[1] },
    );
    // Good splits: each inner point; the two halves share the point and keep all fit points.
    for inner in [points[1], points[2]] {
        let before = splines(&s).len();
        // (`step` leaves the split applied.)
        assert!(step(&mut s, k, "split spline", SketchOp::SplitSpline { curve: sp, point: inner }));
        let after = splines(&s);
        assert_eq!(after.len(), before + 1);
        let total: usize = after.iter().map(|(_, p)| p.len()).sum();
        assert_eq!(total, 4 + 1, "four fit points, one shared");
        assert!(s.undo());
    }
    // Splitting the two halves again (a spline of three points) at its inner point.
    run(&mut s, k, SketchOp::SplitSpline { curve: sp, point: points[1] }).unwrap();
    let tail = splines(&s).into_iter().find(|(id, _)| *id != sp).unwrap();
    assert_eq!(tail.1.len(), 3);
    step(&mut s, k, "split the second half", SketchOp::SplitSpline { curve: tail.0, point: tail.1[1] });
}

// ---- Offset, copy, mirror, scale ----

#[test]
fn offset_and_transforms_of_awkward_splines_are_refused_or_sane() {
    for (what, points) in [
        ("curved", vec![mm(0.0, 0.0), mm(10.0, 10.0), mm(20.0, 0.0), mm(30.0, 10.0)]),
        ("straight", vec![mm(0.0, 0.0), mm(30.0, 0.0)]),
        ("loop", vec![mm(0.0, 0.0), mm(10.0, 10.0), mm(20.0, 0.0), mm(0.0, 0.0)]),
        ("sharp zig-zag", vec![mm(0.0, 0.0), mm(10.0, 10.0), mm(0.1, 20.0)]),
    ] {
        let (mut s, k) = setup();
        let sp = spline(&mut s, k, points);
        for distance in [0.002, -0.002, 1e-9, 1.0, 1e9, 0.0, f64::NAN, f64::INFINITY] {
            for (loop_select, constrain) in [(false, false), (true, true)] {
                step(
                    &mut s,
                    k,
                    &format!("{what}: offset {distance}"),
                    SketchOp::Offset { curves: vec![sp], distance, loop_select, constrain },
                );
            }
        }
        for (name, motion) in [
            ("move", Motion::Translate { by: [0.01, 0.02] }),
            ("rotate", Motion::Rotate { centre: [0.0, 0.0], angle: 1.0 }),
            ("scale 2", Motion::Scale { centre: [0.0, 0.0], factor: 2.0 }),
            ("scale 0.01", Motion::Scale { centre: [0.0, 0.0], factor: 0.01 }),
        ] {
            for copy in [false, true] {
                step(
                    &mut s,
                    k,
                    &format!("{what}: {name} copy {copy}"),
                    SketchOp::Transform { curves: vec![sp], motion, copy, relax: true },
                );
            }
        }
        for bad in [0.0, -1.0, f64::NAN] {
            refused(
                &mut s,
                k,
                &format!("{what}: scale {bad}"),
                SketchOp::Transform {
                    curves: vec![sp],
                    motion: Motion::Scale { centre: [0.0; 2], factor: bad },
                    copy: false,
                    relax: true,
                },
            );
        }
    }
}

#[test]
fn mirror_and_constraints_on_a_spline() {
    let (mut s, k) = setup();
    let sp = spline(&mut s, k, vec![mm(5.0, 0.0), mm(15.0, 10.0), mm(25.0, 0.0)]);
    let axis = line(&mut s, k, mm(0.0, -20.0), mm(0.0, 40.0));
    let n = splines(&s).len();
    // (`step` leaves the mirror applied.)
    assert!(step(&mut s, k, "mirror", SketchOp::Mirror { curves: vec![sp], axis }));
    let after = splines(&s);
    assert_eq!(after.len(), n + 1);
    let sizes: Vec<usize> = after.iter().map(|(_, p)| p.len()).collect();
    assert!(sizes.iter().all(|c| *c == 3), "the mirror changed the fit points: {sizes:?}");
    // A point on a spline: refused or consistent (splines may not take it yet).
    let p = {
        let created = run(&mut s, k, SketchOp::AddPoint { pos: mm(15.0, 9.0) }).unwrap();
        created.into_iter().find_map(|c| if let Created::Point(p) = c { Some(p) } else { None }).unwrap()
    };
    step(&mut s, k, "point on spline", SketchOp::AddConstraint(GeoConstraint::PointOnCurve(p, sp)));
    // Deleting the spline removes its fit points; undo is exact.
    let before = s.doc().clone();
    run(&mut s, k, SketchOp::Delete(vec![gc_sketch::Item::Curve(sp)])).unwrap();
    assert!(splines(&s).iter().all(|(id, _)| *id != sp));
    assert!(s.undo() && s.doc().same_content(&before));
}

// ---- Save compatibility ----

#[test]
fn splines_round_trip_and_damaged_ones_do_not_crash() {
    let (mut s, k) = setup();
    let sp = spline(&mut s, k, vec![mm(0.0, 0.0), mm(10.0, 10.0), mm(20.0, 0.0), mm(30.0, 10.0)]);
    line(&mut s, k, mm(15.0, -10.0), mm(15.0, 20.0));
    run(&mut s, k, SketchOp::Trim { curve: sp, at: [0.005, 0.005] }).ok();
    s.finish_sketch();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("spline.gcp");
    s.save_as(&path).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert!(t.doc().same_content(s.doc()), "the reopened document differs");
    let again = dir.path().join("again.gcp");
    s.save_as(&again).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), std::fs::read(&again).unwrap(), "saving twice differs");

    fn damage(value: &mut serde_json::Value, how: &dyn Fn(&mut Vec<serde_json::Value>)) -> usize {
        let mut found = 0;
        match value {
            serde_json::Value::Object(map) => {
                if let Some(points) =
                    map.get_mut("Spline").and_then(|v| v.get_mut("points")).and_then(|v| v.as_array_mut())
                {
                    how(points);
                    found += 1;
                }
                for v in map.values_mut() {
                    found += damage(v, how);
                }
            }
            serde_json::Value::Array(items) => {
                for v in items {
                    found += damage(v, how);
                }
            }
            _ => {}
        }
        found
    }
    let original: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    type Case = (&'static str, Box<dyn Fn(&mut Vec<serde_json::Value>)>);
    let cases: Vec<Case> = vec![
        ("no fit points", Box::new(|p: &mut Vec<serde_json::Value>| p.clear())),
        ("one fit point", Box::new(|p: &mut Vec<serde_json::Value>| p.truncate(1))),
        (
            "a missing point id",
            Box::new(|p: &mut Vec<serde_json::Value>| p[0] = serde_json::json!(9_999_999)),
        ),
        (
            "the same point twice",
            Box::new(|p: &mut Vec<serde_json::Value>| {
                let first = p[0].clone();
                p[1] = first;
            }),
        ),
    ];
    for (what, how) in &cases {
        let mut damaged = original.clone();
        assert!(damage(&mut damaged, &**how) >= 1, "no spline found for {what}");
        let path = dir.path().join("damaged.gcp");
        std::fs::write(&path, serde_json::to_string(&damaged).unwrap()).unwrap();
        let mut w = Session::new();
        if w.open(&path).is_ok() {
            for (_, sk) in w.doc().sketches().iter() {
                let mut sk = sk.clone();
                drop(sk.solve());
                for c in sk.curves.keys().copied().collect::<Vec<_>>() {
                    std::hint::black_box(sk.ellipse_outline(c, 32));
                }
            }
        }
        let _ = what;
    }
}
