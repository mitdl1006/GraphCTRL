//! QA edge cases for the newer sketch tools: Polygon, Slot, Mirror, Rectangular and Circular patterns and
//! Offset (docs/ux/inventor-sketch-parity.md; Inventor 2026 help, under
//! <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/>: shapes GUID-D489CE6D-7299-4211-A43A-F3580A4BA357,
//! sketch patterns and mirror GUID-AAD3A699-FED8-4614-A31D-0A2316328CEE, offset GUID-962CEE71-8BE4-4DA6-B51E-341319C64FA4).
//! Every refused edit must leave the sketch and the history untouched; every accepted one is one undo step.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::disallowed_methods,
    reason = "test code"
)]

use gc_core::Id;
use gc_doc::{Command, Created, EndSnap, SketchOp, SketchPatternDirection, SlotKind};
use gc_session::Session;
use gc_sketch::{Curve, SkPoint};

fn setup() -> (Session, Id<gc_doc::Sketch>) {
    let mut s = Session::new();
    let sketch = s.create_sketch("XY").unwrap();
    (s, sketch)
}

fn run(s: &mut Session, sketch: Id<gc_doc::Sketch>, op: SketchOp) -> Result<Vec<Created>, gc_doc::EditError> {
    s.execute(&Command::Sketch { sketch, op })
}

fn line(s: &mut Session, sketch: Id<gc_doc::Sketch>, a: [f64; 2], b: [f64; 2]) -> Id<Curve> {
    let op = SketchOp::AddLine {
        a: [a[0], a[1], 0.0],
        b: [b[0], b[1], 0.0],
        snap_a: EndSnap::default(),
        snap_b: EndSnap::default(),
        horizontal: None,
    };
    match run(s, sketch, op)
        .unwrap()
        .iter()
        .find_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None })
    {
        Some(id) => id,
        None => panic!("no curve"),
    }
}

/// Drawn (not construction) curves: tools add construction helpers (a polygon's circle, a pattern's spokes).
fn curves(s: &Session) -> usize {
    s.active_sketch().unwrap().1.curves.values().filter(|c| !c.construction).count()
}

/// Runs `op` expecting a refusal: nothing changed.
fn refused(s: &mut Session, sketch: Id<gc_doc::Sketch>, what: &str, op: SketchOp) {
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    let result = run(s, sketch, op);
    assert!(result.is_err(), "{what} was accepted");
    assert!(s.doc().same_content(&before), "{what} changed the sketch");
    assert_eq!(s.history().undo_depth(), depth, "{what}");
}

/// Runs `op` expecting success as one undo step that restores exactly.
fn accepted(s: &mut Session, sketch: Id<gc_doc::Sketch>, what: &str, op: SketchOp) -> usize {
    let before = s.doc().clone();
    let (depth, n) = (s.history().undo_depth(), curves(s));
    run(s, sketch, op).unwrap_or_else(|e| panic!("{what} refused: {e:?}"));
    assert_eq!(s.history().undo_depth(), depth + 1, "{what}: one undo step");
    let made = curves(s) - n;
    // Everything made is finite.
    let sk = s.active_sketch().unwrap().1;
    assert!(sk.points.values().all(|p| p.pos.iter().all(|v| v.is_finite())), "{what}: a point is not finite");
    assert!(s.undo());
    assert!(s.doc().same_content(&before), "{what}: undo restores");
    made
}

// ---- polygon ----

#[test]
fn polygon_sides_and_reach_are_checked() {
    let (mut s, k) = setup();
    for sides in [0, 1, 2, 121, 1000, u32::MAX] {
        refused(
            &mut s,
            k,
            &format!("{sides} sides"),
            SketchOp::AddPolygon { centre: [0.0; 3], at: [0.01, 0.0, 0.0], sides, circumscribed: false },
        );
    }
    refused(
        &mut s,
        k,
        "zero reach",
        SketchOp::AddPolygon { centre: [0.0; 3], at: [0.0; 3], sides: 5, circumscribed: false },
    );
    refused(
        &mut s,
        k,
        "NaN centre",
        SketchOp::AddPolygon {
            centre: [f64::NAN, 0.0, 0.0],
            at: [0.01, 0.0, 0.0],
            sides: 5,
            circumscribed: false,
        },
    );
    refused(
        &mut s,
        k,
        "infinite corner",
        SketchOp::AddPolygon {
            centre: [0.0; 3],
            at: [f64::INFINITY, 0.0, 0.0],
            sides: 5,
            circumscribed: true,
        },
    );
    for sides in [3, 4, 6, 120] {
        for circumscribed in [false, true] {
            let made = accepted(
                &mut s,
                k,
                &format!("{sides} sides {circumscribed}"),
                SketchOp::AddPolygon { centre: [0.0; 3], at: [0.01, 0.0, 0.0], sides, circumscribed },
            );
            assert_eq!(made, sides as usize, "{sides} lines");
        }
    }
}

/// Inscribed: the clicked point is a corner on the circle; circumscribed: the clicked point is the middle of a
/// side (help: Inscribed / Circumscribed), so the corners lie farther out. All sides are equal.
#[test]
fn polygon_corners_lie_where_the_help_says() {
    for circumscribed in [false, true] {
        let (mut s, k) = setup();
        run(
            &mut s,
            k,
            SketchOp::AddPolygon { centre: [0.0; 3], at: [0.02, 0.0, 0.0], sides: 6, circumscribed },
        )
        .unwrap();
        let sk = s.active_sketch().unwrap().1;
        let mut lengths = Vec::new();
        let mut radii = Vec::new();
        for (c, curve) in &sk.curves {
            if curve.construction {
                continue;
            }
            let (a, b) = sk.line(*c).unwrap();
            let (pa, pb) = (sk.pos(a).unwrap(), sk.pos(b).unwrap());
            lengths.push(gc_math::fm::hypot(pb[0] - pa[0], pb[1] - pa[1]));
            radii.push(gc_math::fm::hypot(pa[0], pa[1]));
        }
        assert!(lengths.iter().all(|l| (l - lengths[0]).abs() < 1e-9), "regular: {lengths:?}");
        let expected = if circumscribed { 0.02 / gc_math::fm::cos(std::f64::consts::PI / 6.0) } else { 0.02 };
        assert!(
            radii.iter().all(|r| (r - expected).abs() < 1e-9),
            "{circumscribed}: {radii:?} vs {expected}"
        );
    }
}

// ---- slot ----

#[test]
fn slot_widths_and_spans_are_checked() {
    let (mut s, k) = setup();
    for width in [0.0, -0.01, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for kind in [SlotKind::CenterToCenter, SlotKind::Overall, SlotKind::CenterPoint] {
            refused(
                &mut s,
                k,
                &format!("{kind:?} width {width}"),
                SketchOp::AddSlot { a: [0.0; 3], b: [0.04, 0.0, 0.0], width, kind },
            );
        }
    }
    refused(
        &mut s,
        k,
        "NaN end",
        SketchOp::AddSlot {
            a: [0.0; 3],
            b: [f64::NAN, 0.0, 0.0],
            width: 0.01,
            kind: SlotKind::CenterToCenter,
        },
    );
    // Overall: the ends are part of the length; a slot shorter than its width has no straight part.
    refused(
        &mut s,
        k,
        "overall shorter than wide",
        SketchOp::AddSlot { a: [0.0; 3], b: [0.01, 0.0, 0.0], width: 0.02, kind: SlotKind::Overall },
    );
    refused(
        &mut s,
        k,
        "overall exactly as long as wide",
        SketchOp::AddSlot { a: [0.0; 3], b: [0.02, 0.0, 0.0], width: 0.02, kind: SlotKind::Overall },
    );
    for kind in [SlotKind::CenterToCenter, SlotKind::Overall, SlotKind::CenterPoint] {
        let made = accepted(
            &mut s,
            k,
            &format!("{kind:?}"),
            SketchOp::AddSlot { a: [0.0; 3], b: [0.04, 0.0, 0.0], width: 0.01, kind },
        );
        assert_eq!(made, 4, "{kind:?}: two lines and two arcs");
    }
}

/// A centre-to-centre slot with both centres on one point is a degenerate (a circle): refused or made with
/// finite geometry; never NaN, and Undo restores either way.
#[test]
fn a_slot_with_equal_centres_is_refused_or_finite() {
    let (mut s, k) = setup();
    let before = s.doc().clone();
    if run(
        &mut s,
        k,
        SketchOp::AddSlot { a: [0.0; 3], b: [0.0; 3], width: 0.01, kind: SlotKind::CenterToCenter },
    )
    .is_ok()
    {
        let sk = s.active_sketch().unwrap().1;
        assert!(sk.points.values().all(|p| p.pos.iter().all(|v| v.is_finite())));
        assert!(s.report().is_some());
        assert!(s.undo());
    }
    assert!(s.doc().same_content(&before));
}

// ---- mirror ----

#[test]
fn mirror_needs_a_line_to_mirror_across_and_something_to_mirror() {
    let (mut s, k) = setup();
    let axis = line(&mut s, k, [0.0, -0.05], [0.0, 0.05]);
    let a = line(&mut s, k, [0.01, 0.0], [0.03, 0.02]);
    let circle = match run(
        &mut s,
        k,
        SketchOp::AddCircle {
            centre: [0.05, 0.0, 0.0],
            radius: 0.005,
            centre_snap: EndSnap::default(),
            rim_snap: EndSnap::default(),
        },
    ) {
        Ok(created) => created.iter().find_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None }),
        Err(_) => None,
    };
    let ghost = Id::<Curve>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    refused(&mut s, k, "no axis (a ghost curve)", SketchOp::Mirror { curves: vec![a], axis: ghost });
    refused(&mut s, k, "nothing to mirror", SketchOp::Mirror { curves: vec![], axis });
    refused(&mut s, k, "a ghost to mirror", SketchOp::Mirror { curves: vec![ghost], axis });
    refused(&mut s, k, "the axis in the list", SketchOp::Mirror { curves: vec![a, axis], axis });
    if let Some(circle) = circle {
        refused(&mut s, k, "an axis that is a circle", SketchOp::Mirror { curves: vec![a], axis: circle });
    }
    // A zero-length axis line.
    let dot = line(&mut s, k, [0.07, 0.07], [0.07, 0.07 + 1e-15]);
    refused(&mut s, k, "a zero-length axis", SketchOp::Mirror { curves: vec![a], axis: dot });
    let made = accepted(&mut s, k, "mirror a line", SketchOp::Mirror { curves: vec![a], axis });
    assert_eq!(made, 1);
    // Mirroring twice across the same line puts the copy on the original again: harmless, finite.
    let two = accepted(
        &mut s,
        k,
        "mirror a and the circle",
        SketchOp::Mirror { curves: [Some(a), circle].into_iter().flatten().collect(), axis },
    );
    assert!(two >= 1);
}

// ---- rectangular pattern ----

fn direction(line: Id<Curve>, count: u32, spacing: &str) -> SketchPatternDirection {
    SketchPatternDirection { line, count, spacing: spacing.into(), flip: false }
}

#[test]
fn rectangular_pattern_counts_spacings_and_directions_are_checked() {
    let (mut s, k) = setup();
    let along = line(&mut s, k, [0.0, -0.05], [0.1, -0.05]);
    let up = line(&mut s, k, [-0.05, 0.0], [-0.05, 0.1]);
    let tiny = line(&mut s, k, [0.0, 0.2], [1e-15, 0.2]);
    let a = line(&mut s, k, [0.0, 0.0], [0.01, 0.0]);
    let ghost = Id::<Curve>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    let op = |first: SketchPatternDirection, second: Option<SketchPatternDirection>, fitted: bool| {
        SketchOp::RectangularPattern { curves: vec![a], first, second, fitted }
    };
    for count in [0, 1, 501, u32::MAX] {
        refused(&mut s, k, &format!("count {count}"), op(direction(along, count, "10"), None, false));
    }
    for spacing in ["0", "-5", "abc", "", "1/0", "0 mm", "10 +"] {
        refused(&mut s, k, &format!("spacing {spacing:?}"), op(direction(along, 3, spacing), None, false));
    }
    refused(&mut s, k, "a ghost direction line", op(direction(ghost, 3, "10"), None, false));
    refused(&mut s, k, "a zero-length direction line", op(direction(tiny, 3, "10"), None, false));
    refused(
        &mut s,
        k,
        "the direction line is being patterned",
        SketchOp::RectangularPattern {
            curves: vec![a, along],
            first: direction(along, 3, "10"),
            second: None,
            fitted: false,
        },
    );
    refused(
        &mut s,
        k,
        "nothing to pattern",
        SketchOp::RectangularPattern {
            curves: vec![],
            first: direction(along, 3, "10"),
            second: None,
            fitted: false,
        },
    );
    refused(
        &mut s,
        k,
        "a bad second direction",
        op(direction(along, 3, "10"), Some(direction(up, 1, "10")), false),
    );
    // Good ones: 4 columns → 3 copies; 4 × 3 → 11 copies; fitted spreads the spacing over the extent.
    assert_eq!(accepted(&mut s, k, "4 columns", op(direction(along, 4, "10"), None, false)), 3);
    assert_eq!(
        accepted(&mut s, k, "4 x 3", op(direction(along, 4, "10"), Some(direction(up, 3, "8")), false)),
        11
    );
    assert_eq!(accepted(&mut s, k, "fitted", op(direction(along, 5, "40"), None, true)), 4);
    let mut flipped = direction(along, 3, "10");
    flipped.flip = true;
    assert_eq!(accepted(&mut s, k, "flipped", op(flipped, None, false)), 2);
}

/// Times patterns of N x N elements (report only; wall-clock is never asserted). A big pattern must finish
/// (docs/handoff/qa.md: 60 x 60 = 3600 elements did not finish in 10 minutes).
/// `cargo test -p gc_session --test qa_sketch_shapes -- --ignored --nocapture pattern_times`
#[test]
#[ignore = "timing report, run on demand"]
#[allow(clippy::print_stdout, reason = "a report")]
fn pattern_times() {
    for n in [3u32, 5, 8, 10, 12] {
        let (mut s, k) = setup();
        let along = line(&mut s, k, [0.0, -0.05], [0.1, -0.05]);
        let up = line(&mut s, k, [-0.05, 0.0], [-0.05, 0.1]);
        let a = line(&mut s, k, [0.0, 0.0], [0.001, 0.0]);
        let op = SketchOp::RectangularPattern {
            curves: vec![a],
            first: direction(along, n, "1"),
            second: Some(direction(up, n, "1")),
            fitted: false,
        };
        let started = std::time::Instant::now();
        let result = run(&mut s, k, op);
        println!(
            "{n} x {n} = {} elements: {:.1} ms ({})",
            n * n,
            started.elapsed().as_secs_f64() * 1e3,
            if result.is_ok() { "made" } else { "refused" }
        );
    }
}

// ---- circular pattern ----

#[test]
fn circular_pattern_counts_angles_and_axes_are_checked() {
    let (mut s, k) = setup();
    let a = line(&mut s, k, [0.02, 0.0], [0.03, 0.0]);
    let centre: Id<SkPoint> = s
        .active_sketch()
        .unwrap()
        .1
        .points
        .iter()
        .find(|(_, p)| p.pos == [0.0; 3])
        .map(|(id, _)| *id)
        .unwrap();
    let ghost = Id::<SkPoint>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    let op = |count: u32, angle: &str, fitted: bool, axis: Id<SkPoint>| SketchOp::CircularPattern {
        curves: vec![a],
        axis,
        count,
        angle: angle.into(),
        flip: false,
        fitted,
    };
    for count in [0, 1, 501, u32::MAX] {
        refused(&mut s, k, &format!("count {count}"), op(count, "60", false, centre));
    }
    for angle in ["0", "abc", "", "0 deg", "1/0", "60 +"] {
        refused(&mut s, k, &format!("angle {angle:?}"), op(4, angle, false, centre));
    }
    refused(&mut s, k, "a ghost axis point", op(4, "60", false, ghost));
    refused(
        &mut s,
        k,
        "nothing to pattern",
        SketchOp::CircularPattern {
            curves: vec![],
            axis: centre,
            count: 4,
            angle: "60".into(),
            flip: false,
            fitted: false,
        },
    );
    assert_eq!(accepted(&mut s, k, "4 at 60", op(4, "60", false, centre)), 3);
    assert_eq!(accepted(&mut s, k, "fitted 360", op(6, "360", true, centre)), 5);
    assert_eq!(accepted(&mut s, k, "fitted 180", op(3, "180", true, centre)), 2);
    // A negative step turns the other way; a step of more than a full turn still gives finite geometry.
    assert_eq!(accepted(&mut s, k, "negative", op(3, "-45", false, centre)), 2);
    // More than a full turn per step: refused or finite (the help gives no limit).
    let before = s.doc().clone();
    if run(&mut s, k, op(4, "400", false, centre)).is_ok() {
        assert!(s.undo());
    }
    assert!(s.doc().same_content(&before));
}

// ---- offset ----

#[test]
fn offset_distances_and_inputs_are_checked() {
    let (mut s, k) = setup();
    let a = line(&mut s, k, [0.0, 0.0], [0.04, 0.0]);
    let ghost = Id::<Curve>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    let op = |curves: Vec<Id<Curve>>, distance: f64, loop_select: bool, constrain: bool| SketchOp::Offset {
        curves,
        distance,
        loop_select,
        constrain,
    };
    refused(&mut s, k, "nothing", op(vec![], 0.005, true, true));
    refused(&mut s, k, "a ghost", op(vec![ghost], 0.005, true, true));
    for distance in [0.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        refused(&mut s, k, &format!("distance {distance}"), op(vec![a], distance, true, true));
    }
    for (loop_select, constrain) in [(true, true), (true, false), (false, true), (false, false)] {
        assert_eq!(
            accepted(
                &mut s,
                k,
                &format!("a line ({loop_select}, {constrain})"),
                op(vec![a], 0.005, loop_select, constrain)
            ),
            1
        );
        assert_eq!(
            accepted(
                &mut s,
                k,
                &format!("a line down ({loop_select}, {constrain})"),
                op(vec![a], -0.005, loop_select, constrain)
            ),
            1
        );
    }
    // A huge distance: finite geometry (or refused), never NaN.
    let before = s.doc().clone();
    if run(&mut s, k, op(vec![a], 1e9, true, false)).is_ok() {
        assert!(s.active_sketch().unwrap().1.points.values().all(|p| p.pos.iter().all(|v| v.is_finite())));
        assert!(s.undo());
    }
    assert!(s.doc().same_content(&before));
}

/// A circle offset grows or shrinks; one that would shrink past its centre is refused or leaves nothing broken.
#[test]
fn offsetting_a_circle_inward_past_its_centre_is_safe() {
    let (mut s, k) = setup();
    let circle = match run(
        &mut s,
        k,
        SketchOp::AddCircle {
            centre: [0.0; 3],
            radius: 0.01,
            centre_snap: EndSnap::default(),
            rim_snap: EndSnap::default(),
        },
    ) {
        Ok(created) => {
            created.iter().find_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None }).unwrap()
        }
        Err(e) => panic!("{e:?}"),
    };
    let before = s.doc().clone();
    for distance in [0.005, -0.005, -0.01, -0.02, -1.0] {
        let result = run(
            &mut s,
            k,
            SketchOp::Offset { curves: vec![circle], distance, loop_select: true, constrain: false },
        );
        if result.is_ok() {
            let sk = s.active_sketch().unwrap().1;
            for c in sk.curves.keys() {
                if let Some((_, r)) = sk.round(*c) {
                    assert!(r.is_finite() && r > 0.0, "distance {distance}: radius {r}");
                }
            }
            assert!(s.undo());
        } else {
            assert!(
                s.doc().same_content(&before),
                "distance {distance}: a refused offset changed the sketch"
            );
        }
    }
}
