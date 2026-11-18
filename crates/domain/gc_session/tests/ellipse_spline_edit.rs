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
            SketchOp::Offset {
                curves: vec![e],
                distance: 0.005,
                loop_select: true,
                constrain: true,
                picked: None,
            },
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
            SketchOp::Offset {
                curves: vec![e],
                distance: -0.03,
                loop_select: false,
                constrain: false,
                picked: None
            }
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
                picked: None,
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
            SketchOp::Offset {
                curves: vec![sp],
                distance: 0.002,
                loop_select: true,
                constrain: true,
                picked: None,
            },
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

fn vertical(s: &mut Session, k: K, x: f64) -> Id<Curve> {
    let op = SketchOp::AddLine {
        a: [x, -0.02, 0.0],
        b: [x, 0.02, 0.0],
        snap_a: EndSnap::default(),
        snap_b: EndSnap::default(),
        horizontal: Some(false),
    };
    curves(&run(s, k, op).unwrap())[0]
}

fn splines(s: &Session) -> Vec<Vec<[f64; 2]>> {
    let sk = s.active_sketch().unwrap().1;
    sk.curves
        .values()
        .filter_map(|c| if let CurveGeom::Spline { points } = &c.geom { Some(points.clone()) } else { None })
        .map(|points| points.iter().map(|p| xy(s, *p)).collect())
        .collect()
}

/// Splines cross other curves (To Split, Trim, or Extend Curves, GUID-DD00037F-6BBA-4B95-AA90-4F99DCEB667D:
/// trim "to the nearest intersecting curve"): a line trims at a spline, a spline trims and splits at
/// lines, and the crossings are snap points.
#[test]
fn splines_cut_and_are_cut_where_they_cross() {
    let (mut s, k) = setup();
    let sp = spline(&mut s, k);
    let line = vertical(&mut s, k, 0.015);
    let sk = s.active_sketch().unwrap().1;
    let crossings = sk.curve_crossings(line, sp);
    assert_eq!(crossings.len(), 1, "{crossings:?}");
    let x = crossings[0];
    assert!((x[0] - 0.015).abs() < 1e-9 && x[1] > 0.0);
    assert_eq!(sk.curve_crossings(sp, line).len(), 1);

    // The line's lower part goes, up to the spline.
    run(&mut s, k, SketchOp::Trim { curve: line, at: [0.015, -0.015] }).unwrap();
    let CurveGeom::Line { a, b } = geom(&s, line) else { panic!() };
    let low = if xy(&s, a)[1] < xy(&s, b)[1] { xy(&s, a) } else { xy(&s, b) };
    assert!((low[1] - x[1]).abs() < 1e-6, "{low:?} vs {x:?}");
    s.undo();

    // Split the spline at the line: two splines meeting there.
    let steps = s.history().undo_depth();
    run(&mut s, k, SketchOp::Split { curve: sp, at: [0.012, 0.009] }).unwrap();
    assert_eq!(s.history().undo_depth(), steps + 1);
    let parts = splines(&s);
    assert_eq!(parts.len(), 2, "{parts:?}");
    for part in &parts {
        assert!(part.iter().any(|p| (p[0] - x[0]).abs() < 1e-9 && (p[1] - x[1]).abs() < 1e-9), "{part:?}");
    }
    s.undo();

    // Trim the middle between two lines: two splines are left, ending at the lines.
    let _left = vertical(&mut s, k, 0.008);
    let _right = vertical(&mut s, k, 0.022);
    run(&mut s, k, SketchOp::Delete(vec![gc_sketch::Item::Curve(line)])).unwrap();
    run(&mut s, k, SketchOp::Trim { curve: sp, at: [0.015, 0.007] }).unwrap();
    let parts = splines(&s);
    assert_eq!(parts.len(), 2, "{parts:?}");
    let ends: Vec<f64> = parts.iter().flat_map(|p| [p[0][0], p[p.len() - 1][0]]).collect();
    assert!(
        ends.iter().any(|x| (x - 0.008).abs() < 1e-9) && ends.iter().any(|x| (x - 0.022).abs() < 1e-9),
        "{ends:?}"
    );
    assert!(
        parts.iter().flatten().all(|p| p[0] < 0.008 + 1e-9 || p[0] > 0.022 - 1e-9),
        "the middle fit point went"
    );
    s.undo();
    s.undo();

    // Trim the end past the right line: the spline now ends there.
    run(&mut s, k, SketchOp::Trim { curve: sp, at: [0.028, -0.008] }).unwrap();
    let parts = splines(&s);
    assert_eq!(parts.len(), 1);
    let last = parts[0][parts[0].len() - 1];
    assert!((last[0] - 0.022).abs() < 1e-9, "{last:?}");
    assert!(s.active_sketch().unwrap().1.points.values().all(|p| p.pos.iter().all(|v| v.is_finite())));
}

/// Two splines cross each other; a spline with nothing crossing it trims away whole; a split with no
/// crossing is refused.
#[test]
fn spline_against_spline_and_lone_splines() {
    let (mut s, k) = setup();
    let a = spline(&mut s, k);
    let points = vec![[0.0, 0.02, 0.0], [0.015, -0.01, 0.0], [0.03, 0.02, 0.0]];
    let b = curves(&run(&mut s, k, SketchOp::AddSpline { points, snaps: Vec::new() }).unwrap())[0];
    let crossings = s.active_sketch().unwrap().1.curve_crossings(a, b);
    assert!(!crossings.is_empty(), "the two splines cross");
    run(&mut s, k, SketchOp::Delete(vec![gc_sketch::Item::Curve(b)])).unwrap();
    let before = s.doc().clone();
    assert!(run(&mut s, k, SketchOp::Split { curve: a, at: [0.01, 0.01] }).is_err());
    assert!(s.doc().same_content(&before));
    run(&mut s, k, SketchOp::Trim { curve: a, at: [0.01, 0.01] }).unwrap();
    assert!(splines(&s).is_empty());
}

/// Close Curve and Insert Point (To Create, Lock Spline Length, and Edit Splines,
/// GUID-DB15C479-66F2-456A-9E54-40A60F83DC92): a closed spline runs smoothly back to its first point and is
/// not trimmed or split open; a point inserted on a spline becomes a fit point there.
#[test]
fn close_curve_and_insert_point() {
    let (mut s, k) = setup();
    let sp = spline(&mut s, k);
    let CurveGeom::Spline { points } = geom(&s, sp) else { panic!() };
    // Insert a point on the second span.
    let steps = s.history().undo_depth();
    let created = run(&mut s, k, SketchOp::InsertSplinePoint { curve: sp, at: [0.015, 0.006] }).unwrap();
    assert_eq!(s.history().undo_depth(), steps + 1);
    let [Created::Point(new)] = created[..] else { panic!("{created:?}") };
    let CurveGeom::Spline { points: more } = geom(&s, sp) else { panic!() };
    assert_eq!(more.len(), 5);
    assert_eq!(more[2], new);
    assert_eq!([more[0], more[1], more[3], more[4]], [points[0], points[1], points[2], points[3]]);
    let p = xy(&s, new);
    assert!(p[0] > 0.01 && p[0] < 0.02, "{p:?}");
    // On top of a fit point: refused.
    let on_point = xy(&s, points[1]);
    assert!(run(&mut s, k, SketchOp::InsertSplinePoint { curve: sp, at: on_point }).is_err());
    s.undo();

    // Close Curve: the first point again at the end; the outline ends where it began, smoothly.
    run(&mut s, k, SketchOp::CloseSpline { curve: sp }).unwrap();
    let CurveGeom::Spline { points: closed } = geom(&s, sp) else { panic!() };
    assert_eq!(closed.len(), 5);
    assert_eq!(closed[0], closed[4]);
    let sk = s.active_sketch().unwrap().1;
    let outline = sk.spline_outline(&sk.curves[&sp].geom, 16).unwrap();
    assert_eq!(outline.len(), 4 * 16 + 1);
    let (first, last) = (outline[0], outline[outline.len() - 1]);
    assert!(gc_math::fm::hypot(first[0] - last[0], first[1] - last[1]) < 1e-12);
    // Smooth through the first point: the steps either side of it point the same way.
    let (a, b, c) = (outline[outline.len() - 2], outline[0], outline[1]);
    let (u, v) = ([b[0] - a[0], b[1] - a[1]], [c[0] - b[0], c[1] - b[1]]);
    let cos = (u[0] * v[0] + u[1] * v[1]) / (gc_math::fm::hypot(u[0], u[1]) * gc_math::fm::hypot(v[0], v[1]));
    assert!(cos > 0.9, "{cos}");
    // Closing again, splitting at a point, and trimming or splitting at a crossing are refused.
    let before = s.doc().clone();
    assert!(run(&mut s, k, SketchOp::CloseSpline { curve: sp }).is_err());
    assert!(run(&mut s, k, SketchOp::SplitSpline { curve: sp, point: closed[2] }).is_err());
    let _line = vertical(&mut s, k, 0.015);
    let with_line = s.doc().clone();
    assert!(run(&mut s, k, SketchOp::Split { curve: sp, at: [0.012, 0.009] }).is_err());
    assert!(run(&mut s, k, SketchOp::Trim { curve: sp, at: [0.012, 0.009] }).is_err());
    assert!(s.doc().same_content(&with_line));
    // A point inserted on the closing span lands between the last fit point and the first.
    run(&mut s, k, SketchOp::Delete(vec![gc_sketch::Item::Curve(_line)])).unwrap();
    let sk = s.active_sketch().unwrap().1;
    let outline = sk.spline_outline(&sk.curves[&sp].geom, 16).unwrap();
    let on_closing = outline[3 * 16 + 8];
    let created = run(&mut s, k, SketchOp::InsertSplinePoint { curve: sp, at: on_closing }).unwrap();
    let [Created::Point(new)] = created[..] else { panic!() };
    let CurveGeom::Spline { points: now } = geom(&s, sp) else { panic!() };
    assert_eq!(now.len(), 6);
    assert_eq!(now[4], new);
    assert_eq!(now[0], now[5]);
    // A closed spline offsets to a closed spline.
    let made = curves(
        &run(
            &mut s,
            k,
            SketchOp::Offset {
                curves: vec![sp],
                distance: 0.001,
                loop_select: false,
                constrain: false,
                picked: None,
            },
        )
        .unwrap(),
    );
    let CurveGeom::Spline { points: offset } = geom(&s, made[0]) else { panic!() };
    assert!(gc_sketch::spline_closed(&offset));
    // Two fit points cannot close.
    let two = curves(
        &run(
            &mut s,
            k,
            SketchOp::AddSpline { points: vec![[0.1, 0.0, 0.0], [0.2, 0.0, 0.0]], snaps: Vec::new() },
        )
        .unwrap(),
    )[0];
    assert!(run(&mut s, k, SketchOp::CloseSpline { curve: two }).is_err());
    let _ = before;
}

/// Extend (To Split, Trim, or Extend Curves, GUID-DD00037F-6BBA-4B95-AA90-4F99DCEB667D: "extends a curve to
/// the nearest intersecting curve"): a spline's end goes straight on to the nearest curve, through a new
/// end fit point tied there.
#[test]
fn a_spline_extends_to_the_nearest_curve() {
    let (mut s, k) = setup();
    let sp = spline(&mut s, k);
    let tall = SketchOp::AddLine {
        a: [0.05, -0.2, 0.0],
        b: [0.05, 0.2, 0.0],
        snap_a: EndSnap::default(),
        snap_b: EndSnap::default(),
        horizontal: Some(false),
    };
    let wall = curves(&run(&mut s, k, tall).unwrap())[0];
    let steps = s.history().undo_depth();
    run(&mut s, k, SketchOp::Extend { curve: sp, at: [0.029, -0.009] }).unwrap();
    assert_eq!(s.history().undo_depth(), steps + 1);
    let CurveGeom::Spline { points } = geom(&s, sp) else { panic!() };
    assert_eq!(points.len(), 5);
    let end = xy(&s, points[4]);
    assert!((end[0] - 0.05).abs() < 1e-9, "{end:?}");
    // Tied to the wall: the end is held on it.
    let sk = s.active_sketch().unwrap().1;
    assert!(
        sk.constraints.values().any(
            |c| matches!(*c, gc_sketch::GeoConstraint::PointOnCurve(p, w) if p == points[4] && w == wall)
        )
    );
    // Nothing ahead of the other end: refused, nothing changed.
    let before = s.doc().clone();
    assert!(run(&mut s, k, SketchOp::Extend { curve: sp, at: [0.001, 0.001] }).is_err());
    assert!(s.doc().same_content(&before));
}

/// "If you select an ellipse near a quadrant and an axis is visible, the offset geometry is an ellipse. If
/// you select an ellipse between quadrants … the offset geometry is a spline" (GUID-D489CE6D): picked
/// between quadrants, the offset is a closed spline on the true offset curve; near a quadrant, an ellipse.
#[test]
fn an_ellipse_picked_between_quadrants_offsets_to_a_spline() {
    let (mut s, k) = setup();
    let e = ellipse(&mut s, k); // a = 0.04, b = 0.02
    let offset = |picked: [f64; 2]| SketchOp::Offset {
        curves: vec![e],
        distance: 0.005,
        loop_select: false,
        constrain: false,
        picked: Some(picked),
    };
    // Near the end of the major axis: an ellipse.
    let near = curves(&run(&mut s, k, offset([0.0399, 0.001])).unwrap());
    assert!(matches!(geom(&s, near[0]), CurveGeom::Ellipse { .. }));
    s.undo();
    // Between: a closed spline whose fit points lie 5 mm out from the ellipse.
    let between = [0.04 * std::f64::consts::FRAC_1_SQRT_2, 0.02 * std::f64::consts::FRAC_1_SQRT_2];
    let made = curves(&run(&mut s, k, offset(between)).unwrap());
    let CurveGeom::Spline { points } = geom(&s, made[0]) else { panic!("{:?}", geom(&s, made[0])) };
    assert!(gc_sketch::spline_closed(&points));
    assert_eq!(points.len(), 17);
    let sk = s.active_sketch().unwrap().1;
    let outline = sk.ellipse_outline(e, 2048).unwrap().0;
    for p in &points[..16] {
        let q = xy(&s, *p);
        let gap = outline
            .iter()
            .map(|o| gc_math::fm::hypot(o[0] - q[0], o[1] - q[1]))
            .fold(f64::INFINITY, f64::min);
        assert!((gap - 0.005).abs() < 1e-4, "{gap}");
    }
    s.undo();
    // Inward past the tightest curvature (b²/a = 10 mm): refused.
    let deep = SketchOp::Offset {
        curves: vec![e],
        distance: -0.011,
        loop_select: false,
        constrain: false,
        picked: Some(between),
    };
    assert!(run(&mut s, k, deep).is_err());
}
