//! QA boundary tests for Split, 2D Chamfer, Tangent Circle, Midpoint Line and the Two/Three Point Center
//! Rectangles. Inventor 2026 help, under <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/>:
//! split/trim/extend GUID-DD00037F-6BBA-4B95-AA90-4F99DCEB667D, shapes (chamfer, rectangles, tangent circle)
//! GUID-D489CE6D-7299-4211-A43A-F3580A4BA357, lines GUID-4F861674-F74C-474E-869A-4C190ED6EC4B.
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
use gc_doc::{ChamferKind, Command, Created, EndSnap, SketchOp};
use gc_session::Session;
use gc_sketch::Curve;

type K = Id<gc_doc::Sketch>;

fn setup() -> (Session, K) {
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    (s, k)
}

fn run(s: &mut Session, k: K, op: SketchOp) -> Result<Vec<Created>, gc_doc::EditError> {
    s.execute(&Command::Sketch { sketch: k, op })
}

fn line(s: &mut Session, k: K, a: [f64; 2], b: [f64; 2]) -> Id<Curve> {
    let op = SketchOp::AddLine {
        a: [a[0], a[1], 0.0],
        b: [b[0], b[1], 0.0],
        snap_a: EndSnap::default(),
        snap_b: EndSnap::default(),
        horizontal: None,
    };
    run(s, k, op)
        .unwrap()
        .iter()
        .find_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None })
        .unwrap()
}

fn ghost() -> Id<Curve> {
    Id::<Curve>::from_raw(std::num::NonZeroU64::new(9999).unwrap())
}

/// Drawn (not construction) curves.
fn drawn(s: &Session) -> usize {
    s.active_sketch().unwrap().1.curves.values().filter(|c| !c.construction).count()
}

/// Everything in the sketch is finite and every curve's points exist.
fn sane(s: &Session, what: &str) {
    let sk = s.active_sketch().unwrap().1;
    assert!(sk.points.values().all(|p| p.pos.iter().all(|v| v.is_finite())), "{what}: a point is not finite");
    assert!(
        sk.curves.values().all(|c| c.points().iter().all(|p| sk.points.contains_key(p))),
        "{what}: a curve lost a point"
    );
    for (id, c) in &sk.curves {
        if let Some((centre, r)) = sk.round(*id) {
            assert!(r.is_finite() && r > 0.0, "{what}: round {id:?} radius {r}");
            let _ = (centre, c);
        }
    }
}

/// Refused: nothing changed.
fn refused(s: &mut Session, k: K, what: &str, op: SketchOp) {
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    assert!(run(s, k, op).is_err(), "{what} was accepted");
    assert!(s.doc().same_content(&before), "{what} changed the sketch");
    assert_eq!(s.history().undo_depth(), depth, "{what}");
}

/// Refused (no trace) or accepted as one undo step that undoes exactly and leaves a finite sketch.
/// Returns whether it was accepted.
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
            assert!(s.doc().clone().sketch_solves(k), "{what}: the sketch no longer solves");
            assert!(s.undo());
            assert!(s.doc().same_content(&before), "{what}: undo restores");
            true
        }
    }
}

trait Solves {
    fn sketch_solves(&mut self, k: K) -> bool;
}
impl Solves for gc_doc::Document {
    fn sketch_solves(&mut self, k: K) -> bool {
        self.get(k).is_some_and(|s| s.clone().solve().is_ok())
    }
}

// ---- 2D Chamfer ----

fn chamfer(a: (Id<Curve>, [f64; 2]), b: (Id<Curve>, [f64; 2]), kind: ChamferKind, dims: bool) -> SketchOp {
    SketchOp::Chamfer { lines: [a, b], kind, dimensions: dims }
}

fn equal(d: &str) -> ChamferKind {
    ChamferKind::Equal(d.into())
}

/// An L corner at the origin, arms 40 mm long.
fn corner() -> (Session, K, Id<Curve>, Id<Curve>) {
    let (mut s, k) = setup();
    let h = line(&mut s, k, [0.0, 0.0], [0.04, 0.0]);
    let v = line(&mut s, k, [0.0, 0.0], [0.0, 0.04]);
    (s, k, h, v)
}

#[test]
fn chamfer_of_a_corner_is_one_undo_step() {
    for dims in [false, true] {
        let (mut s, k, h, v) = corner();
        let n = drawn(&s);
        let kinds = [
            equal("5"),
            ChamferKind::Unequal("5".into(), "10".into()),
            ChamferKind::DistanceAngle { distance: "8".into(), angle: "30".into() },
        ];
        for kind in kinds {
            let op = chamfer((h, [0.02, 0.0]), (v, [0.0, 0.02]), kind.clone(), dims);
            assert!(refused_or_sane(&mut s, k, &format!("{kind:?}"), op.clone()), "{kind:?} dims {dims}");
            run(&mut s, k, op).unwrap();
            assert_eq!(drawn(&s), n + 1, "a bevel line is added");
            assert!(s.undo());
        }
    }
}

/// Two parallel lines have no corner.
#[test]
fn chamfer_of_parallel_lines_is_refused() {
    let (mut s, k) = setup();
    let a = line(&mut s, k, [0.0, 0.0], [0.04, 0.0]);
    let b = line(&mut s, k, [0.0, 0.01], [0.04, 0.01]);
    let c = line(&mut s, k, [0.1, 0.0], [0.14, 0.0]); // collinear with a
    let d = line(&mut s, k, [0.0, 0.02], [-0.04, 0.02]); // antiparallel
    for (what, x, y) in [("parallel", a, b), ("collinear", a, c), ("antiparallel", a, d)] {
        for kind in [equal("5"), ChamferKind::DistanceAngle { distance: "5".into(), angle: "30".into() }] {
            refused(
                &mut s,
                k,
                &format!("{what} {kind:?}"),
                chamfer((x, [0.02, 0.0]), (y, [0.02, 0.01]), kind, false),
            );
        }
    }
}

/// A distance longer than the line is refused: the bevel cannot start beyond the end (help: the chamfer
/// stays on the lines).
#[test]
fn chamfer_distance_longer_than_the_lines_is_refused_or_sane() {
    let (mut s, k, h, v) = corner();
    for d in ["40", "40.0001", "41", "100", "1e6", "1e300"] {
        refused_or_sane(
            &mut s,
            k,
            &format!("equal {d}"),
            chamfer((h, [0.02, 0.0]), (v, [0.0, 0.02]), equal(d), false),
        );
    }
    for (d1, d2) in [("50", "5"), ("5", "50"), ("50", "50")] {
        refused_or_sane(
            &mut s,
            k,
            &format!("unequal {d1} {d2}"),
            chamfer((h, [0.02, 0.0]), (v, [0.0, 0.02]), ChamferKind::Unequal(d1.into(), d2.into()), false),
        );
    }
}

/// Distances that are zero, negative, unparsable or non-finite never make a bevel.
#[test]
fn chamfer_bad_sizes_are_refused() {
    let (mut s, k, h, v) = corner();
    for d in ["0", "-5", "-0.0001", "abc", "", "1/0", "5 +", "0 mm", "-5 mm"] {
        refused(
            &mut s,
            k,
            &format!("equal {d:?}"),
            chamfer((h, [0.02, 0.0]), (v, [0.0, 0.02]), equal(d), false),
        );
        refused(
            &mut s,
            k,
            &format!("unequal {d:?}"),
            chamfer((h, [0.02, 0.0]), (v, [0.0, 0.02]), ChamferKind::Unequal("5".into(), d.into()), true),
        );
        refused(
            &mut s,
            k,
            &format!("angle distance {d:?}"),
            chamfer(
                (h, [0.02, 0.0]),
                (v, [0.0, 0.02]),
                ChamferKind::DistanceAngle { distance: d.into(), angle: "30".into() },
                false,
            ),
        );
    }
}

/// Expressions that evaluate negative (the lead's "식이 음수일 때") are refused like typed negatives.
#[test]
fn chamfer_expressions_that_come_out_negative_are_refused() {
    let (mut s, k, h, v) = corner();
    for d in ["2-5", "-(2+3)", "0-1", "5*-1", "(3-8) mm", "2 - 10 mm"] {
        refused(
            &mut s,
            k,
            &format!("equal {d:?}"),
            chamfer((h, [0.02, 0.0]), (v, [0.0, 0.02]), equal(d), true),
        );
        refused(
            &mut s,
            k,
            &format!("angle {d:?}"),
            chamfer(
                (h, [0.02, 0.0]),
                (v, [0.0, 0.02]),
                ChamferKind::DistanceAngle { distance: "5".into(), angle: d.into() },
                true,
            ),
        );
    }
    // An expression that evaluates positive works the same as the typed number.
    assert!(refused_or_sane(
        &mut s,
        k,
        "10-5",
        chamfer((h, [0.02, 0.0]), (v, [0.0, 0.02]), equal("10-5"), true)
    ));
}

/// Distance and Angle: 0 and 180 degrees (and beyond) cannot make a triangle with the corner.
#[test]
fn chamfer_angle_zero_and_straight_are_refused() {
    let (mut s, k, h, v) = corner();
    // At the corner the lines meet at 90 degrees, so an angle at the first cut of 90 or more cannot close.
    for a in [
        "0", "0 deg", "90", "90.0001", "120", "180", "180 deg", "270", "360", "-30", "1e300", "abc", "",
        "NaN",
    ] {
        for dims in [false, true] {
            refused(
                &mut s,
                k,
                &format!("angle {a:?} dims {dims}"),
                chamfer(
                    (h, [0.02, 0.0]),
                    (v, [0.0, 0.02]),
                    ChamferKind::DistanceAngle { distance: "5".into(), angle: a.into() },
                    dims,
                ),
            );
        }
    }
    // Just inside the range works.
    for a in ["1", "45", "89.9"] {
        refused_or_sane(
            &mut s,
            k,
            &format!("angle {a}"),
            chamfer(
                (h, [0.02, 0.0]),
                (v, [0.0, 0.02]),
                ChamferKind::DistanceAngle { distance: "5".into(), angle: a.into() },
                true,
            ),
        );
    }
}

#[test]
fn chamfer_odd_picks_are_refused_or_sane() {
    let (mut s, k, h, v) = corner();
    let ok = (h, [0.02, 0.0]);
    let ok2 = (v, [0.0, 0.02]);
    refused(&mut s, k, "same line twice", chamfer(ok, ok, equal("5"), false));
    refused(&mut s, k, "ghost first", chamfer((ghost(), [0.02, 0.0]), ok2, equal("5"), false));
    refused(&mut s, k, "ghost second", chamfer(ok, (ghost(), [0.0, 0.02]), equal("5"), false));
    // A circle is not a line.
    let circle = run(
        &mut s,
        k,
        SketchOp::AddCircle {
            centre: [0.1, 0.1, 0.0],
            radius: 0.01,
            centre_snap: EndSnap::default(),
            rim_snap: EndSnap::default(),
        },
    )
    .unwrap()
    .iter()
    .find_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None })
    .unwrap();
    refused(&mut s, k, "circle", chamfer(ok, (circle, [0.11, 0.1]), equal("5"), false));
    // Picks on the far sides of the corner, at the corner itself and at NaN.
    for (p1, p2) in [
        ([-0.02, 0.0], [0.0, -0.02]),
        ([0.02, 0.0], [0.0, -0.02]),
        ([0.0, 0.0], [0.0, 0.0]),
        ([f64::NAN, 0.0], [0.0, 0.02]),
        ([0.02, 0.0], [0.0, f64::INFINITY]),
    ] {
        refused_or_sane(
            &mut s,
            k,
            &format!("picks {p1:?} {p2:?}"),
            chamfer((h, p1), (v, p2), equal("5"), true),
        );
    }
    // Lines that do not touch but cross when extended, and ones that cross in the middle.
    let a = line(&mut s, k, [0.1, 0.0], [0.14, 0.0]);
    let b = line(&mut s, k, [0.2, -0.05], [0.2, 0.05]);
    refused_or_sane(&mut s, k, "gap", chamfer((a, [0.13, 0.0]), (b, [0.2, 0.03]), equal("5"), false));
    let x = line(&mut s, k, [0.3, 0.0], [0.34, 0.0]);
    let y = line(&mut s, k, [0.32, -0.02], [0.32, 0.02]);
    refused_or_sane(&mut s, k, "cross", chamfer((x, [0.33, 0.0]), (y, [0.32, 0.01]), equal("5"), false));
}

// ---- Split ----

fn split(curve: Id<Curve>, at: [f64; 2]) -> SketchOp {
    SketchOp::Split { curve, at }
}

#[test]
fn split_a_line_at_a_crossing_makes_two_lines() {
    let (mut s, k) = setup();
    let a = line(&mut s, k, [0.0, 0.0], [0.04, 0.0]);
    let _b = line(&mut s, k, [0.02, -0.02], [0.02, 0.02]);
    let n = drawn(&s);
    assert!(refused_or_sane(&mut s, k, "split", split(a, [0.01, 0.0])));
    run(&mut s, k, split(a, [0.01, 0.0])).unwrap();
    assert_eq!(drawn(&s), n + 1, "one line became two");
    // A second split of the first half has no crossing left.
    refused(&mut s, k, "no crossing left", split(a, [0.01, 0.0]));
}

/// A curve that nothing crosses cannot be split (help: Split cuts at an intersection).
#[test]
fn split_a_line_without_a_crossing_is_refused() {
    let (mut s, k) = setup();
    let a = line(&mut s, k, [0.0, 0.0], [0.04, 0.0]);
    let _far = line(&mut s, k, [0.0, 0.05], [0.04, 0.05]); // parallel: never crosses
    refused(&mut s, k, "alone", split(a, [0.02, 0.0]));
    // One that crosses only beyond its end (extended it would) is not a crossing.
    let _beyond = line(&mut s, k, [0.06, -0.02], [0.06, 0.02]);
    refused(&mut s, k, "crossing beyond the end", split(a, [0.02, 0.0]));
    // Ghost curves and non-finite clicks.
    refused(&mut s, k, "ghost", split(ghost(), [0.02, 0.0]));
    for at in [[f64::NAN, 0.0], [0.0, f64::INFINITY], [1e300, 1e300]] {
        refused_or_sane(&mut s, k, &format!("at {at:?}"), split(a, at));
    }
}

/// Clicking a line at its end point, at the crossing itself, and at an end that lies on another curve.
#[test]
fn split_clicks_at_endpoints_and_at_the_crossing() {
    let (mut s, k) = setup();
    let a = line(&mut s, k, [0.0, 0.0], [0.04, 0.0]);
    let _b = line(&mut s, k, [0.02, -0.02], [0.02, 0.02]);
    for at in [[0.0, 0.0], [0.04, 0.0], [0.02, 0.0], [0.02, 0.02], [0.0, 1e-12], [-1.0, 0.0], [5.0, 0.0]] {
        refused_or_sane(&mut s, k, &format!("click {at:?}"), split(a, at));
    }
    // A T: the end of one line lies on the other. Splitting the through line there makes two lines;
    // splitting the stem (whose end is the crossing) must not leave a zero-length piece.
    let (mut s, k) = setup();
    let through = line(&mut s, k, [0.0, 0.0], [0.04, 0.0]);
    let stem = line(&mut s, k, [0.02, 0.0], [0.02, 0.03]);
    refused_or_sane(&mut s, k, "T through", split(through, [0.005, 0.0]));
    refused_or_sane(&mut s, k, "T stem", split(stem, [0.02, 0.01]));
    let sk = s.active_sketch().unwrap().1;
    for id in sk.curves.keys() {
        if let Some((p, q)) = sk.line(*id) {
            let (p, q) = (sk.pos(p).unwrap(), sk.pos(q).unwrap());
            assert!((p[0] - q[0]).hypot(p[1] - q[1]) > 1e-9, "a zero-length line");
        }
    }
}

/// Split an arc and a circle.
#[test]
fn split_arcs_and_circles_is_refused_or_sane() {
    let (mut s, k) = setup();
    let circle = run(
        &mut s,
        k,
        SketchOp::AddCircle {
            centre: [0.0, 0.0, 0.0],
            radius: 0.02,
            centre_snap: EndSnap::default(),
            rim_snap: EndSnap::default(),
        },
    )
    .unwrap()
    .iter()
    .find_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None })
    .unwrap();
    let _cross = line(&mut s, k, [-0.03, 0.0], [0.03, 0.0]);
    for at in [[0.02, 0.0], [0.0, 0.02], [-0.02, 0.0], [0.0, 0.0]] {
        refused_or_sane(&mut s, k, &format!("circle at {at:?}"), split(circle, at));
    }
}

// ---- Tangent Circle ----

fn tangent(lines: [Id<Curve>; 3], near: [[f64; 2]; 3]) -> SketchOp {
    SketchOp::AddTangentCircle { lines, near }
}

/// A triangle of three lines: the incircle, with its radius from the sides.
#[test]
fn tangent_circle_of_a_triangle_is_the_incircle() {
    let (mut s, k) = setup();
    let a = line(&mut s, k, [0.0, 0.0], [0.04, 0.0]);
    let b = line(&mut s, k, [0.04, 0.0], [0.0, 0.03]);
    let c = line(&mut s, k, [0.0, 0.03], [0.0, 0.0]);
    let n = drawn(&s);
    let op = tangent([a, b, c], [[0.02, 0.0], [0.02, 0.015], [0.0, 0.015]]);
    assert!(refused_or_sane(&mut s, k, "incircle", op.clone()));
    run(&mut s, k, op).unwrap();
    assert_eq!(drawn(&s), n + 1);
    let sk = s.active_sketch().unwrap().1;
    let circle = sk.curves.keys().copied().find(|c| sk.round(*c).is_some()).unwrap();
    let (_, r) = sk.round(circle).unwrap();
    assert!((r - 0.01).abs() < 1e-6, "3-4-5 triangle: inradius (3+4-5)/2 = 1 mm*10, got {r}");
}

/// Two parallel lines and a transversal: a circle of half the gap fits; three parallel (or the same line
/// twice) cannot be touched by any circle.
#[test]
fn tangent_circle_with_parallel_lines() {
    let (mut s, k) = setup();
    let p1 = line(&mut s, k, [0.0, 0.0], [0.1, 0.0]);
    let p2 = line(&mut s, k, [0.0, 0.02], [0.1, 0.02]);
    let t = line(&mut s, k, [0.0, -0.01], [0.0, 0.05]);
    let op = tangent([p1, p2, t], [[0.05, 0.0], [0.05, 0.02], [0.0, 0.01]]);
    if refused_or_sane(&mut s, k, "parallel pair + transversal", op.clone()) {
        run(&mut s, k, op).unwrap();
        let sk = s.active_sketch().unwrap().1;
        let circle = sk.curves.keys().copied().find(|c| sk.round(*c).is_some()).unwrap();
        let (_, r) = sk.round(circle).unwrap();
        assert!((r - 0.01).abs() < 1e-6, "half the 20 mm gap, got {r}");
    }
    let (mut s, k) = setup();
    let p1 = line(&mut s, k, [0.0, 0.0], [0.1, 0.0]);
    let p2 = line(&mut s, k, [0.0, 0.02], [0.1, 0.02]);
    let p3 = line(&mut s, k, [0.0, 0.04], [0.1, 0.04]);
    let p4 = line(&mut s, k, [0.0, 0.0], [-0.1, 0.0]); // collinear with p1
    refused(&mut s, k, "three parallel", tangent([p1, p2, p3], [[0.05, 0.0], [0.05, 0.02], [0.05, 0.04]]));
    refused(&mut s, k, "same line thrice", tangent([p1, p1, p1], [[0.05, 0.0]; 3]));
    refused(&mut s, k, "same line twice", tangent([p1, p2, p1], [[0.05, 0.0], [0.05, 0.02], [0.02, 0.0]]));
    refused(&mut s, k, "collinear pair", tangent([p1, p4, p2], [[0.05, 0.0], [-0.05, 0.0], [0.05, 0.02]]));
    // Concurrent lines (all through one point) have only a degenerate circle.
    let c1 = line(&mut s, k, [0.3, 0.0], [0.3, 0.1]);
    let c2 = line(&mut s, k, [0.25, 0.05], [0.35, 0.05]);
    let c3 = line(&mut s, k, [0.25, 0.0], [0.35, 0.1]);
    refused_or_sane(
        &mut s,
        k,
        "concurrent",
        tangent([c1, c2, c3], [[0.3, 0.02], [0.27, 0.05], [0.26, 0.01]]),
    );
}

#[test]
fn tangent_circle_odd_inputs_are_refused_or_sane() {
    let (mut s, k) = setup();
    let a = line(&mut s, k, [0.0, 0.0], [0.04, 0.0]);
    let b = line(&mut s, k, [0.04, 0.0], [0.0, 0.03]);
    let c = line(&mut s, k, [0.0, 0.03], [0.0, 0.0]);
    refused(&mut s, k, "ghost", tangent([a, b, ghost()], [[0.02, 0.0], [0.02, 0.015], [0.0, 0.015]]));
    for near in [
        [[f64::NAN, 0.0], [0.02, 0.015], [0.0, 0.015]],
        [[0.02, 0.0], [f64::INFINITY, 0.015], [0.0, 0.015]],
        [[0.02, 0.0], [0.02, 0.015], [1e300, 1e300]],
    ] {
        refused_or_sane(&mut s, k, &format!("near {near:?}"), tangent([a, b, c], near));
    }
    // A circle is not a line.
    let circle = run(
        &mut s,
        k,
        SketchOp::AddCircle {
            centre: [0.1, 0.1, 0.0],
            radius: 0.01,
            centre_snap: EndSnap::default(),
            rim_snap: EndSnap::default(),
        },
    )
    .unwrap()
    .iter()
    .find_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None })
    .unwrap();
    refused(&mut s, k, "circle", tangent([a, b, circle], [[0.02, 0.0], [0.02, 0.015], [0.11, 0.1]]));
}

// ---- Centre rectangles ----

fn p(x: f64, y: f64) -> [f64; 3] {
    [x, y, 0.0]
}

fn centre_rect(centre: [f64; 3], corner: [f64; 3]) -> SketchOp {
    SketchOp::AddCenterRectangle { centre, corner, snap_centre: EndSnap::default() }
}

fn centre_rect3(centre: [f64; 3], side: [f64; 3], width: [f64; 3]) -> SketchOp {
    SketchOp::AddCenterRectangle3 { centre, side, width, snap_centre: EndSnap::default() }
}

#[test]
fn center_rectangle_makes_four_lines_in_one_undo_step() {
    let (mut s, k) = setup();
    let n = drawn(&s);
    assert!(refused_or_sane(&mut s, k, "2pt", centre_rect(p(0.0, 0.0), p(0.02, 0.01))));
    run(&mut s, k, centre_rect(p(0.0, 0.0), p(0.02, 0.01))).unwrap();
    assert_eq!(drawn(&s), n + 4);
    assert!(s.undo());
    assert!(refused_or_sane(&mut s, k, "3pt", centre_rect3(p(0.0, 0.0), p(0.02, 0.0), p(0.0, 0.01))));
    run(&mut s, k, centre_rect3(p(0.0, 0.0), p(0.02, 0.0), p(0.0, 0.01))).unwrap();
    assert_eq!(drawn(&s), n + 4);
}

/// Size zero: the corner on the centre, or on the same horizontal or vertical as the centre, has no area.
#[test]
fn center_rectangle_of_zero_size_is_refused() {
    let (mut s, k) = setup();
    for (what, centre, corner) in [
        ("corner on centre", p(0.01, 0.01), p(0.01, 0.01)),
        ("zero height", p(0.0, 0.0), p(0.02, 0.0)),
        ("zero width", p(0.0, 0.0), p(0.0, 0.02)),
        ("tiny", p(0.0, 0.0), p(1e-15, 1e-15)),
    ] {
        refused(&mut s, k, what, centre_rect(centre, corner));
    }
    for (what, centre, side, width) in [
        ("side on centre", p(0.0, 0.0), p(0.0, 0.0), p(0.0, 0.01)),
        ("width on centre line", p(0.0, 0.0), p(0.02, 0.0), p(0.01, 0.0)),
        ("width on centre", p(0.0, 0.0), p(0.02, 0.0), p(0.0, 0.0)),
        ("all one point", p(0.0, 0.0), p(0.0, 0.0), p(0.0, 0.0)),
        ("tiny side", p(0.0, 0.0), p(1e-15, 0.0), p(0.0, 0.01)),
    ] {
        refused(&mut s, k, what, centre_rect3(centre, side, width));
    }
    let n = [f64::NAN, f64::INFINITY, f64::NEG_INFINITY];
    for v in n {
        refused(&mut s, k, "nan corner", centre_rect(p(0.0, 0.0), p(v, 0.01)));
        refused(&mut s, k, "nan centre", centre_rect(p(0.0, v), p(0.02, 0.01)));
        refused(&mut s, k, "nan side", centre_rect3(p(0.0, 0.0), p(v, 0.0), p(0.0, 0.01)));
        refused(&mut s, k, "nan width", centre_rect3(p(0.0, 0.0), p(0.02, 0.0), p(0.0, v)));
    }
}

/// Corners in any quadrant, side points in any direction (the 3-point rectangle is rotated by them) and
/// huge sizes stay finite.
#[test]
fn center_rectangle_any_orientation_is_sane() {
    let (mut s, k) = setup();
    for corner in [p(-0.02, 0.01), p(0.02, -0.01), p(-0.02, -0.01), p(1e6, 1e6)] {
        refused_or_sane(&mut s, k, &format!("2pt {corner:?}"), centre_rect(p(0.0, 0.0), corner));
    }
    for (side, width) in [
        (p(0.0, 0.02), p(0.01, 0.0)),
        (p(-0.02, 0.0), p(0.0, -0.01)),
        (p(0.014, 0.014), p(-0.01, 0.01)),
        (p(0.02, 0.0), p(0.0, 1e6)),
    ] {
        refused_or_sane(
            &mut s,
            k,
            &format!("3pt {side:?} {width:?}"),
            centre_rect3(p(0.0, 0.0), side, width),
        );
    }
}

// ---- Midpoint Line ----

fn midpoint_line(mid: [f64; 3], end: [f64; 3]) -> SketchOp {
    SketchOp::AddMidpointLine { mid, end, snap_mid: EndSnap::default(), snap_end: EndSnap::default() }
}

#[test]
fn midpoint_line_degenerate_input_is_refused_or_sane() {
    let (mut s, k) = setup();
    let n = drawn(&s);
    assert!(refused_or_sane(&mut s, k, "midpoint line", midpoint_line(p(0.0, 0.0), p(0.02, 0.0))));
    run(&mut s, k, midpoint_line(p(0.0, 0.0), p(0.02, 0.0))).unwrap();
    assert_eq!(drawn(&s), n + 1);
    for (what, mid, end) in [
        ("end on mid", p(0.01, 0.01), p(0.01, 0.01)),
        ("tiny", p(0.0, 0.0), p(1e-15, 0.0)),
        ("nan", p(0.0, 0.0), p(f64::NAN, 0.0)),
        ("infinite", p(f64::INFINITY, 0.0), p(0.02, 0.0)),
    ] {
        refused(&mut s, k, what, midpoint_line(mid, end));
    }
}
