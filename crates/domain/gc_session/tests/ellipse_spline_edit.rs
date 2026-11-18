//! Offset of ellipses and splines, and Split Spline. Inventor 2026 help: To Create 2D Shape Geometry
//! (GUID-D489CE6D-7299-4211-A43A-F3580A4BA357) — "When you Offset an ellipse, the result is either a
//! mathematical ellipse or an associative spline … An offset ellipse is a nonassociative mathematical
//! ellipse"; To Create, Lock Spline Length, and Edit Splines (GUID-DB15C479-66F2-456A-9E54-40A60F83DC92) —
//! "To split a spline, right-click a point and choose Split Spline".

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "tests")]

use gc_core::Id;
use gc_doc::{Command, Created, EditError, EndSnap, SketchOp};
use gc_session::Session;
use gc_sketch::{Curve, CurveGeom, SkPoint};

type K = Id<gc_doc::Sketch>;

fn setup() -> (Session, K) {
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    (s, k)
}

fn run(s: &mut Session, k: K, op: SketchOp) -> Result<Vec<Created>, EditError> {
    s.execute(&Command::Sketch { sketch: k, op })
}

fn curves(created: &[Created]) -> Vec<Id<Curve>> {
    created.iter().filter_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None }).collect()
}

fn xy(s: &Session, p: Id<SkPoint>) -> [f64; 2] {
    let q = s.active_sketch().unwrap().1.pos(p).unwrap();
    [q[0], q[1]]
}

fn geom(s: &Session, c: Id<Curve>) -> CurveGeom {
    s.active_sketch().unwrap().1.curves[&c].geom.clone()
}

fn ellipse(s: &mut Session, k: K) -> Id<Curve> {
    let op = SketchOp::AddEllipse {
        centre: [0.0, 0.0, 0.0],
        major: [0.04, 0.0, 0.0],
        minor: 0.02,
        centre_snap: EndSnap::default(),
    };
    curves(&run(s, k, op).unwrap())[0]
}

fn spline(s: &mut Session, k: K) -> Id<Curve> {
    let points = vec![[0.0, 0.0, 0.0], [0.01, 0.01, 0.0], [0.02, 0.0, 0.0], [0.03, -0.01, 0.0]];
    curves(&run(s, k, SketchOp::AddSpline { points, snaps: Vec::new() }).unwrap())[0]
}

#[test]
fn an_offset_ellipse_is_a_new_ellipse_with_longer_radii() {
    let (mut s, k) = setup();
    let e = ellipse(&mut s, k);
    let steps = s.history().undo_depth();
    let made = curves(
        &run(
            &mut s,
            k,
            SketchOp::Offset { curves: vec![e], distance: 0.005, loop_select: true, constrain: true },
        )
        .unwrap(),
    );
    assert_eq!(s.history().undo_depth(), steps + 1);
    let [o] = made[..] else { panic!("{made:?}") };
    let CurveGeom::Ellipse { c, major, minor, arc: None } = geom(&s, o) else { panic!() };
    let (cc, m) = (xy(&s, c), xy(&s, major));
    assert!(cc[0].abs() < 1e-9 && cc[1].abs() < 1e-9);
    assert!((m[0] - 0.045).abs() < 1e-9 && m[1].abs() < 1e-9, "{m:?}");
    assert!((minor - 0.025).abs() < 1e-9);
    // Inward past the minor radius: refused, nothing left behind.
    let before = s.doc().clone();
    assert!(
        run(
            &mut s,
            k,
            SketchOp::Offset { curves: vec![e], distance: -0.03, loop_select: false, constrain: false }
        )
        .is_err()
    );
    assert!(s.doc().same_content(&before));
}

#[test]
fn an_offset_elliptical_arc_keeps_its_end_angles() {
    let (mut s, k) = setup();
    let e = ellipse(&mut s, k);
    // Trim the ellipse with a vertical line to get an arc.
    let line = SketchOp::AddLine {
        a: [0.0, -0.05, 0.0],
        b: [0.0, 0.05, 0.0],
        snap_a: EndSnap::default(),
        snap_b: EndSnap::default(),
        horizontal: None,
    };
    run(&mut s, k, line).unwrap();
    run(&mut s, k, SketchOp::Trim { curve: e, at: [-0.04, 0.0] }).unwrap();
    let arc_curve = s
        .active_sketch()
        .unwrap()
        .1
        .curves
        .iter()
        .find_map(|(id, c)| matches!(c.geom, CurveGeom::Ellipse { arc: Some(_), .. }).then_some(*id))
        .unwrap();
    let made = curves(
        &run(
            &mut s,
            k,
            SketchOp::Offset {
                curves: vec![arc_curve],
                distance: 0.01,
                loop_select: false,
                constrain: false,
            },
        )
        .unwrap(),
    );
    let CurveGeom::Ellipse { arc: Some((a, b)), minor, .. } = geom(&s, made[0]) else { panic!() };
    assert!((minor - 0.03).abs() < 1e-9);
    // The ends sit where the trimmed arc ended (on the Y axis), at the new minor radius.
    for p in [xy(&s, a), xy(&s, b)] {
        assert!(p[0].abs() < 1e-6 && (p[1].abs() - 0.03).abs() < 1e-6, "{p:?}");
    }
}

#[test]
fn an_offset_spline_runs_beside_the_original() {
    let (mut s, k) = setup();
    let sp = spline(&mut s, k);
    let made = curves(
        &run(
            &mut s,
            k,
            SketchOp::Offset { curves: vec![sp], distance: 0.002, loop_select: true, constrain: true },
        )
        .unwrap(),
    );
    let [o] = made[..] else { panic!("{made:?}") };
    let (CurveGeom::Spline { points: old }, CurveGeom::Spline { points: new }) = (geom(&s, sp), geom(&s, o))
    else {
        panic!()
    };
    assert_eq!(old.len(), new.len());
    for (p, q) in old.iter().zip(&new) {
        let (p, q) = (xy(&s, *p), xy(&s, *q));
        assert!((gc_math::fm::hypot(q[0] - p[0], q[1] - p[1]) - 0.002).abs() < 1e-9);
    }
    // Left of the direction: the first fit point (heading up-right) moves up-left.
    let (p, q) = (xy(&s, old[0]), xy(&s, new[0]));
    assert!(q[1] > p[1] && q[0] < p[0]);
}

#[test]
fn split_spline_at_an_inner_fit_point() {
    let (mut s, k) = setup();
    let sp = spline(&mut s, k);
    let CurveGeom::Spline { points } = geom(&s, sp) else { panic!() };
    // An end point, or a point not on the spline: refused with no trace.
    let before = s.doc().clone();
    assert!(run(&mut s, k, SketchOp::SplitSpline { curve: sp, point: points[0] }).is_err());
    assert!(run(&mut s, k, SketchOp::SplitSpline { curve: sp, point: points[3] }).is_err());
    assert!(s.doc().same_content(&before));
    let steps = s.history().undo_depth();
    let made = curves(&run(&mut s, k, SketchOp::SplitSpline { curve: sp, point: points[1] }).unwrap());
    assert_eq!(s.history().undo_depth(), steps + 1);
    let (CurveGeom::Spline { points: head }, CurveGeom::Spline { points: tail }) =
        (geom(&s, sp), geom(&s, made[0]))
    else {
        panic!()
    };
    assert_eq!(head, points[..=1].to_vec());
    assert_eq!(tail, points[1..].to_vec());
    // A curve that is not a spline is refused.
    let e = ellipse(&mut s, k);
    assert!(run(&mut s, k, SketchOp::SplitSpline { curve: e, point: points[2] }).is_err());
    s.undo();
    s.undo();
    let CurveGeom::Spline { points: back } = geom(&s, sp) else { panic!() };
    assert_eq!(back, points);
}

/// The Offset tool takes an ellipse like any other curve: pick it, the cursor sets the distance, click.
#[test]
fn the_offset_tool_offsets_an_ellipse_through_the_cursor() {
    use gc_tools::{ToolEvent, ToolKind};
    let (mut s, k) = setup();
    let _e = ellipse(&mut s, k);
    s.set_tool(ToolKind::Offset);
    let px = 1e-4;
    for at in [[0.04, 0.0], [0.05, 0.0]] {
        s.tool_event(ToolEvent::Move { pos: at, px, suppress_snap: true });
        s.tool_event(ToolEvent::Click { pos: at, px, modifier: false });
    }
    let sk = s.active_sketch().unwrap().1;
    let minors: Vec<f64> = sk
        .curves
        .values()
        .filter_map(|c| if let CurveGeom::Ellipse { minor, .. } = c.geom { Some(minor) } else { None })
        .collect();
    assert_eq!(minors.len(), 2, "{minors:?}");
    assert!(minors.iter().any(|m| (m - 0.03).abs() < 1e-6), "{minors:?}");
}
