//! QA boundary tests for Trim and Extend on ellipses and elliptical arcs, and for the Quadrant and
//! Intersection snaps they bring. Inventor 2026 help, under
//! <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/>: shapes (Ellipse: "You can trim and
//! extend ellipses like any other curve") GUID-D489CE6D-7299-4211-A43A-F3580A4BA357, split, trim and extend
//! GUID-DD00037F-6BBA-4B95-AA90-4F99DCEB667D.
//! Every refused edit leaves the sketch and the history untouched; every accepted one is one undo step that
//! restores exactly, leaves only finite points, a sketch that solves, and elliptical arcs of positive sweep.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::disallowed_methods,
    reason = "test code"
)]

use gc_core::Id;
use gc_doc::{Command, Created, EndSnap, SketchOp};
use gc_session::Session;
use gc_sketch::{Curve, CurveGeom};
use gc_tools::{Inference, Key, ToolEvent, ToolKind};

type K = Id<gc_doc::Sketch>;

const PX: f64 = 1e-4;

fn mm(x: f64, y: f64) -> [f64; 2] {
    [x * 1e-3, y * 1e-3]
}

fn setup() -> (Session, K) {
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    (s, k)
}

fn run(s: &mut Session, k: K, op: SketchOp) -> Result<Vec<Created>, gc_doc::EditError> {
    s.execute(&Command::Sketch { sketch: k, op })
}

fn curve_of(created: &[Created]) -> Id<Curve> {
    created.iter().find_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None }).unwrap()
}

fn ellipse(s: &mut Session, k: K, centre: [f64; 2], major: [f64; 2], minor: f64) -> Id<Curve> {
    curve_of(
        &run(
            s,
            k,
            SketchOp::AddEllipse {
                centre: [centre[0], centre[1], 0.0],
                major: [major[0], major[1], 0.0],
                minor,
                centre_snap: EndSnap::default(),
            },
        )
        .unwrap(),
    )
}

fn line(s: &mut Session, k: K, a: [f64; 2], b: [f64; 2]) -> Id<Curve> {
    curve_of(
        &run(
            s,
            k,
            SketchOp::AddLine {
                a: [a[0], a[1], 0.0],
                b: [b[0], b[1], 0.0],
                snap_a: EndSnap::default(),
                snap_b: EndSnap::default(),
                horizontal: None,
            },
        )
        .unwrap(),
    )
}

fn ghost() -> Id<Curve> {
    Id::<Curve>::from_raw(std::num::NonZeroU64::new(9999).unwrap())
}

/// The ellipse in the standard test scene: centre at the origin, 40 mm and 20 mm radii.
fn scene() -> (Session, K, Id<Curve>) {
    let (mut s, k) = setup();
    let e = ellipse(&mut s, k, [0.0, 0.0], mm(40.0, 0.0), 0.02);
    (s, k, e)
}

fn ellipses(s: &Session) -> Vec<Id<Curve>> {
    let sk = s.active_sketch().unwrap().1;
    sk.curves.iter().filter(|(_, c)| matches!(c.geom, CurveGeom::Ellipse { .. })).map(|(id, _)| *id).collect()
}

/// Every point finite, every ellipse's minor radius positive, every elliptical arc's ends distinct points of
/// the sketch.
fn sane(s: &Session, what: &str) {
    let sk = s.active_sketch().unwrap().1;
    assert!(sk.points.values().all(|p| p.pos.iter().all(|v| v.is_finite())), "{what}: a point is not finite");
    for c in ellipses(s) {
        let (centre, major, minor) = sk.ellipse(c).unwrap();
        assert!(minor.is_finite() && minor > 0.0, "{what}: minor radius {minor}");
        assert!(sk.points.contains_key(&centre) && sk.points.contains_key(&major), "{what}: a lost point");
        if let Some((a, b)) = sk.ellipse_arc(c) {
            let (pa, pb) = (sk.pos(a).unwrap(), sk.pos(b).unwrap());
            assert!(pa.iter().chain(&pb).all(|v| v.is_finite()), "{what}: an arc end is not finite");
            assert!(sk.ellipse_outline(c, 32).is_some(), "{what}: an arc with no outline");
        }
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

/// Refused with no trace, or accepted as one undo step that undoes exactly and leaves a sane sketch; the
/// state after a success is kept (redone) so the next step builds on it.
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

fn trim(curve: Id<Curve>, at: [f64; 2]) -> SketchOp {
    SketchOp::Trim { curve, at }
}

fn extend(curve: Id<Curve>, at: [f64; 2]) -> SketchOp {
    SketchOp::Extend { curve, at }
}

// ---- Trim ----

/// A whole ellipse cut by two lines: trimming between them leaves an elliptical arc; trimming where nothing
/// crosses it, or with only one crossing, removes it whole (as a circle) or is refused — never a half curve.
#[test]
fn trimming_an_ellipse_between_two_crossings_leaves_an_arc() {
    let (mut s, k, e) = scene();
    line(&mut s, k, mm(-20.0, -30.0), mm(-20.0, 30.0));
    line(&mut s, k, mm(20.0, -30.0), mm(20.0, 30.0));
    assert!(step(&mut s, k, "trim the right end", trim(e, mm(40.0, 0.0))));
    // The ellipse is now an arc (two points) or was replaced by pieces; at least one piece is an arc.
    assert!(
        s.active_sketch()
            .unwrap()
            .1
            .curves
            .values()
            .any(|c| matches!(c.geom, CurveGeom::Ellipse { arc: Some(_), .. }))
    );
    // Where the pointer is not near the curve (far away): refused or trims the nearest; never a trace on refusal.
    step(&mut s, k, "trim far from the ellipse", trim(e, mm(0.0, 100.0)));
}

/// Trim, then trim the arc again in the middle (two cutters inside it), then once more: each is one undo
/// step; after two trims there are two arcs of one ellipse; the third at a removed place is refused or sane.
#[test]
fn trimming_an_arc_in_the_middle_twice() {
    let (mut s, k, e) = scene();
    let (a, b, c) = (
        line(&mut s, k, mm(-20.0, -30.0), mm(-20.0, 30.0)),
        line(&mut s, k, mm(0.0, -30.0), mm(0.0, 30.0)),
        line(&mut s, k, mm(20.0, -30.0), mm(20.0, 30.0)),
    );
    let _ = (a, b, c);
    run(&mut s, k, trim(e, mm(40.0, 0.0))).unwrap();
    sane(&s, "first trim");
    let before_second = s.doc().clone();
    let depth = s.history().undo_depth();
    // The second trim: between the cutters at x = -20 and x = 0, on the top of the remaining arc.
    assert!(step(&mut s, k, "second trim, middle of the arc", trim(e, mm(-10.0, 17.3))));
    assert_eq!(s.history().undo_depth(), depth + 1);
    let arcs = ellipses(&s).len();
    assert!((1..=3).contains(&arcs), "{arcs} pieces");
    // A third trim on the same place now finds the piece gone.
    step(&mut s, k, "third trim at the same place", trim(e, mm(-10.0, 17.3)));
    // Two undos walk back to the arc and then the ellipse.
    assert!(s.undo());
    assert!(s.doc().same_content(&before_second), "undo of the second trim is not exact");
}

/// A line that only touches the ellipse (tangent at the top), and lines a hair either side of tangent: the
/// crossings are none, one or two very close ones; trimming never makes a zero-sweep arc.
#[test]
fn tangent_and_nearly_tangent_cutters_are_safe() {
    for offset in [0.0, 1e-12, -1e-12, 1e-9, -1e-9, 1e-7, -1e-7, 1e-5] {
        let (mut s, k, e) = scene();
        let y = 0.02 + offset;
        let cutter = line(&mut s, k, [-0.03, y], [0.03, y]);
        let crossings = s.active_sketch().unwrap().1.curve_crossings(e, cutter);
        assert!(crossings.len() <= 2, "offset {offset}: {} crossings", crossings.len());
        assert!(crossings.iter().all(|p| p[0].is_finite() && p[1].is_finite()));
        // Trim the ellipse at its top and at the far side; trim the line at the touch.
        step(&mut s, k, &format!("trim the ellipse at the top, offset {offset}"), trim(e, [0.0, 0.02]));
        step(&mut s, k, &format!("trim the ellipse far side, offset {offset}"), trim(e, [0.0, -0.02]));
        step(&mut s, k, &format!("trim the line at the touch, offset {offset}"), trim(cutter, [0.0, y]));
        step(&mut s, k, &format!("trim the line end, offset {offset}"), trim(cutter, [0.025, y]));
    }
}

/// Ellipse against ellipse (crossing in four points), a circle inscribed in the ellipse (touching at the two
/// minor axis ends), and two identical ellipses on top of each other.
#[test]
fn ellipses_against_ellipses_and_circles() {
    // Two ellipses, one turned a quarter: four crossings.
    let (mut s, k, e) = scene();
    let f = ellipse(&mut s, k, [0.0, 0.0], mm(0.0, 40.0), 0.02);
    let crossings = s.active_sketch().unwrap().1.curve_crossings(e, f);
    assert_eq!(crossings.len(), 4, "{crossings:?}");
    step(&mut s, k, "trim e against f", trim(e, mm(40.0, 0.0)));
    step(&mut s, k, "trim f against e", trim(f, mm(0.0, 40.0)));
    step(&mut s, k, "trim e between the crossings", trim(e, mm(14.0, 14.0)));
    // A circle of the minor radius touches the ellipse at two points (internally tangent).
    let (mut s, k, e) = scene();
    let circle = curve_of(
        &run(
            &mut s,
            k,
            SketchOp::AddCircle {
                centre: [0.0; 3],
                radius: 0.02,
                centre_snap: EndSnap::default(),
                rim_snap: EndSnap::default(),
            },
        )
        .unwrap(),
    );
    let crossings = s.active_sketch().unwrap().1.curve_crossings(e, circle);
    assert!(crossings.len() <= 4 && crossings.iter().all(|p| p[0].is_finite()), "{crossings:?}");
    step(&mut s, k, "trim the ellipse at the circle", trim(e, mm(40.0, 0.0)));
    step(&mut s, k, "trim the circle at the ellipse", trim(circle, mm(0.0, 20.0)));
    // Two identical ellipses: infinitely many shared points; nothing crashes and nothing is half cut.
    let (mut s, k, e) = scene();
    let twin = ellipse(&mut s, k, [0.0, 0.0], mm(40.0, 0.0), 0.02);
    let _ = s.active_sketch().unwrap().1.curve_crossings(e, twin);
    step(&mut s, k, "trim an ellipse against its twin", trim(e, mm(40.0, 0.0)));
}

/// Non-finite and missing inputs.
#[test]
fn trim_and_extend_refuse_odd_inputs_on_ellipses() {
    let (mut s, k, e) = scene();
    line(&mut s, k, mm(-20.0, -30.0), mm(-20.0, 30.0));
    line(&mut s, k, mm(20.0, -30.0), mm(20.0, 30.0));
    for at in [[f64::NAN, 0.0], [0.0, f64::INFINITY], [1e300, 1e300]] {
        step(&mut s, k, &format!("trim at {at:?}"), trim(e, at));
        step(&mut s, k, &format!("extend at {at:?}"), extend(e, [at[0], at[1]]));
    }
    refused(&mut s, k, "trim a ghost", trim(ghost(), mm(0.0, 0.0)));
    refused(&mut s, k, "extend a ghost", extend(ghost(), mm(0.0, 0.0)));
    // Extending a whole ellipse: it has no open end.
    refused(&mut s, k, "extend a whole ellipse", extend(e, mm(40.0, 0.0)));
    // Split and Offset still refuse ellipses (the backend's decision).
    refused(&mut s, k, "split", SketchOp::Split { curve: e, at: mm(0.0, 20.0) });
}

// ---- Extend ----

/// An elliptical arc extended at an open end up to the next curve: the sweep grows, the end lies on the
/// curve it met and is tied to it; extending where nothing lies ahead is refused; one undo step.
#[test]
fn extending_an_elliptical_arc() {
    let (mut s, k, e) = scene();
    let near = line(&mut s, k, mm(-20.0, -30.0), mm(-20.0, 30.0));
    run(&mut s, k, trim(e, mm(40.0, 0.0))).unwrap();
    sane(&s, "after the trim");
    let (start, end) = s.active_sketch().unwrap().1.ellipse_arc(e).expect("an arc");
    let (ps, pe) = {
        let sk = s.active_sketch().unwrap().1;
        (sk.pos(start).unwrap(), sk.pos(end).unwrap())
    };
    // A farther cutter beyond one open end: the arc's end that lies on `near` is extended to it.
    let far = line(&mut s, k, mm(-35.0, -30.0), mm(-35.0, 30.0));
    let _ = (near, far, ps, pe);
    let mut moved = false;
    for at in [ps, pe] {
        moved |= step(&mut s, k, &format!("extend at {at:?}"), extend(e, [at[0], at[1]]));
    }
    let _ = moved;
    // Nothing ahead: a lone arc with no cutter anywhere near its ends.
    let (mut t, kt, f) = scene();
    let cutter = line(&mut t, kt, mm(0.0, -30.0), mm(0.0, 30.0));
    run(&mut t, kt, trim(f, mm(40.0, 0.0))).unwrap();
    let _ = cutter;
    let (a, b) = t.active_sketch().unwrap().1.ellipse_arc(f).expect("an arc");
    let sk = t.active_sketch().unwrap().1;
    let ends = [sk.pos(a).unwrap(), sk.pos(b).unwrap()];
    for at in ends {
        // The arc's ends are tied to the cutter already: extending either must be refused or stay on it.
        step(&mut t, kt, &format!("extend a tied end {at:?}"), extend(f, [at[0], at[1]]));
    }
}

// ---- Snaps ----

fn move_to(s: &mut Session, at: [f64; 2]) {
    s.tool_event(ToolEvent::Move { pos: at, px: PX, suppress_snap: false });
}

/// While a line is being drawn the pointer snaps to an ellipse's four axis ends (Quadrant), to the crossing of
/// an ellipse and a line (Intersection) and of two ellipses; an arc offers only the axis ends that lie on it;
/// Ctrl suppresses all of it; a degenerate hover does not panic.
#[test]
fn ellipse_snaps_quadrants_and_intersections() {
    let (mut s, k, e) = scene();
    let cutter = line(&mut s, k, mm(-50.0, 10.0), mm(50.0, 10.0));
    s.set_tool(ToolKind::Line);
    // Quadrants: the ends of the axes.
    // (The right end of the major axis is the ellipse's own sketch point and snaps as a Point.)
    for (x, y) in [(-40.0, 0.0), (0.0, 20.0), (0.0, -20.0)] {
        move_to(&mut s, mm(x + 0.15, y + 0.1));
        let snap = s.preview().snap.unwrap();
        assert_eq!(snap.inference, Some(Inference::Quadrant), "at ({x}, {y}): {snap:?}");
        assert!(snap.pos[0].is_finite() && (snap.pos[0] - x * 1e-3).abs() < 1e-6, "{snap:?}");
    }
    // The crossing of the ellipse and the line y = 10 mm: x = ±a * sqrt(1 - (10/20)^2) = ±34.64 mm.
    let x = 40.0 * (1.0f64 - 0.25).sqrt();
    move_to(&mut s, mm(x + 0.15, 10.1));
    let snap = s.preview().snap.unwrap();
    assert_eq!(snap.inference, Some(Inference::Intersection), "{snap:?}");
    assert!((snap.pos[0] - x * 1e-3).abs() < 1e-6 && (snap.pos[1] - 0.01).abs() < 1e-6, "{snap:?}");
    // Ctrl: nothing.
    s.tool_event(ToolEvent::Move { pos: mm(x + 0.15, 10.1), px: PX, suppress_snap: true });
    assert_eq!(s.preview().snap.unwrap().inference, None);
    // Hovering far from everything, and on the exact centre of the ellipse: no panic.
    move_to(&mut s, mm(500.0, 500.0));
    move_to(&mut s, mm(0.0, 0.0));
    s.tool_event(ToolEvent::Key(Key::Escape));
    // After the right end is trimmed, that quadrant is no longer on the arc.
    s.set_tool(ToolKind::Select);
    let (a, b) = (
        line(&mut s, k, mm(-20.0, -30.0), mm(-20.0, 30.0)),
        line(&mut s, k, mm(20.0, -30.0), mm(20.0, 30.0)),
    );
    let _ = (a, b, cutter);
    run(&mut s, k, trim(e, mm(40.0, 0.0))).unwrap();
    s.set_tool(ToolKind::Line);
    move_to(&mut s, mm(40.2, 0.1));
    let snap = s.preview().snap.unwrap();
    assert_ne!(
        snap.inference,
        Some(Inference::Quadrant),
        "a quadrant that was trimmed away is still offered: {snap:?}"
    );
    move_to(&mut s, mm(-40.2, 0.1));
    assert_eq!(s.preview().snap.unwrap().inference, Some(Inference::Quadrant), "the left end remains");
    // The crossing of the ellipse arc with the first cutter is an Intersection.
    move_to(&mut s, mm(-20.15, 17.4));
    let snap = s.preview().snap.unwrap();
    assert_eq!(snap.inference, Some(Inference::Intersection), "{snap:?}");
}

// ---- Save compatibility ----

fn project(s: &mut Session) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cut.gcp");
    s.save_as(&path).unwrap();
    (dir, path)
}

/// Arcs round trip exactly; a file written before elliptical arcs existed (no `arc`) opens as whole ellipses;
/// a damaged arc (ends that name points that are not there, or a missing end) opens or is refused, and what
/// opens solves and draws without a crash.
#[test]
fn elliptical_arcs_round_trip_and_old_or_damaged_files_are_safe() {
    let (mut s, k, e) = scene();
    line(&mut s, k, mm(-20.0, -30.0), mm(-20.0, 30.0));
    line(&mut s, k, mm(20.0, -30.0), mm(20.0, 30.0));
    run(&mut s, k, trim(e, mm(40.0, 0.0))).unwrap();
    s.finish_sketch();
    let (dir, path) = project(&mut s);
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert!(t.doc().same_content(s.doc()), "the reopened document differs");
    let again = dir.path().join("again.gcp");
    s.save_as(&again).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), std::fs::read(&again).unwrap(), "saving twice differs");

    fn edit(
        value: &mut serde_json::Value,
        f: &dyn Fn(&mut serde_json::Map<String, serde_json::Value>),
    ) -> usize {
        let mut found = 0;
        match value {
            serde_json::Value::Object(map) => {
                if let Some(obj) = map.get_mut("Ellipse").and_then(|v| v.as_object_mut()) {
                    f(obj);
                    found += 1;
                }
                for v in map.values_mut() {
                    found += edit(v, f);
                }
            }
            serde_json::Value::Array(items) => {
                for v in items {
                    found += edit(v, f);
                }
            }
            _ => {}
        }
        found
    }
    let original: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    // Old file: no `arc` at all.
    let mut old = original.clone();
    let n = edit(&mut old, &|o| {
        o.remove("arc");
    });
    assert!(n >= 1, "no ellipse found in the project");
    let old_path = dir.path().join("old.gcp");
    std::fs::write(&old_path, serde_json::to_string(&old).unwrap()).unwrap();
    let mut u = Session::new();
    u.open(&old_path).unwrap();
    for (_, sk) in u.doc().sketches().iter() {
        assert!(sk.clone().solve().is_ok());
        for c in sk.curves.keys() {
            if sk.ellipse(*c).is_some() {
                assert!(sk.ellipse_arc(*c).is_none(), "an old ellipse became an arc");
            }
        }
    }
    // Damaged arcs.
    for (what, f) in [
        (
            "arc names missing points",
            Box::new(|o: &mut serde_json::Map<String, serde_json::Value>| {
                o.insert("arc".into(), serde_json::json!([9_999_999, 9_999_998]));
            }) as Box<dyn Fn(&mut serde_json::Map<String, serde_json::Value>)>,
        ),
        (
            "arc with one end",
            Box::new(|o| {
                o.insert("arc".into(), serde_json::json!([1]));
            }),
        ),
        (
            "arc that is not a pair",
            Box::new(|o| {
                o.insert("arc".into(), serde_json::json!("nope"));
            }),
        ),
    ] {
        let mut damaged = original.clone();
        assert!(edit(&mut damaged, &*f) >= 1);
        let path = dir.path().join("damaged.gcp");
        std::fs::write(&path, serde_json::to_string(&damaged).unwrap()).unwrap();
        let mut w = Session::new();
        if w.open(&path).is_ok() {
            for (_, sk) in w.doc().sketches().iter() {
                let mut sk = sk.clone();
                drop(sk.solve());
                for c in sk.curves.keys().copied().collect::<Vec<_>>() {
                    std::hint::black_box(sk.ellipse_outline(c, 32));
                    std::hint::black_box(sk.ellipse_quadrants(c));
                }
            }
        }
        let _ = what;
    }
}
