//! Drag fixtures (F9, F10, F13, F16, F16b, F17 in docs/PLAN.md, appendix C).

#![allow(
    clippy::float_cmp,
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: exact values are compared and failures should panic"
)]

use std::f64::consts::{PI, TAU};

use gc_solver::{
    Constraint, ConstraintId, DragRefused, Flip, FrameOutcome, Grab, Line2, P2, Round, Status, System,
};

fn line(a: P2, b: P2) -> Line2 {
    Line2 { a, b }
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

fn solved(mut sys: System) -> System {
    assert_eq!(sys.solve().status, Status::Solved);
    sys
}

/// The F16 triangle: three points, three lengths, nothing fixed.
fn triangle() -> (System, [P2; 3]) {
    let mut sys = System::new();
    let p = [sys.add_point2(0.0, 0.0), sys.add_point2(10.0, 0.0), sys.add_point2(0.0, 10.0)];
    sys.add(ConstraintId(1), Constraint::Distance { p: p[0], q: p[1], value: 10.0 });
    sys.add(ConstraintId(2), Constraint::Distance { p: p[0], q: p[2], value: 10.0 });
    sys.add(ConstraintId(3), Constraint::Distance { p: p[1], q: p[2], value: 200.0_f64.sqrt() });
    (solved(sys), p)
}

/// Drags vertex A once round the circle of radius 3 about (0, 3) in `samples` cursor positions.
fn drag_round(samples: usize) -> (System, [P2; 3]) {
    let (sys, p) = triangle();
    let mut drag = sys.begin_drag(&Grab::Point(p[0]), [0.0, 0.0]).unwrap();
    for k in 1..=samples {
        let phi = TAU * k as f64 / samples as f64;
        // The last sample is the start point exactly, whatever sin and cos round to.
        let cursor =
            if k == samples { [0.0, 0.0] } else { [3.0 * libm::sin(phi), 3.0 - 3.0 * libm::cos(phi)] };
        assert_eq!(drag.update(cursor), FrameOutcome::Moved, "sample {k} of {samples}");
        let sys = drag.system();
        assert!(dist(sys.point2(p[0]), cursor) < 1e-7, "the grabbed point is at the cursor");
        let (a, b, c) = (sys.point2(p[0]), sys.point2(p[1]), sys.point2(p[2]));
        let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
        assert!(area > 0.0, "the triangle never mirrors");
        assert!(sys.violation() <= 1.0);
    }
    (drag.commit(), p)
}

#[test]
fn f16_a_drag_loop_returns_the_sketch_to_where_it_was() {
    for samples in [4, 64, 512] {
        let (sys, p) = drag_round(samples);
        let (a, b) = (sys.point2(p[0]), sys.point2(p[1]));
        let rotation = libm::atan2(b[1] - a[1], b[0] - a[0]);
        assert!(rotation.abs() < 1e-9, "{samples} samples: net rotation {rotation}");
        assert!(dist(a, [0.0, 0.0]) < 1e-9);
    }
}

#[test]
fn f16_the_same_cursor_samples_give_the_same_bits() {
    let (first, p) = drag_round(64);
    let (second, _) = drag_round(64);
    for point in p {
        assert_eq!(first.point2(point).map(f64::to_bits), second.point2(point).map(f64::to_bits));
    }
}

#[test]
fn f16b_a_line_with_one_fixed_end_rotates_when_grabbed_by_its_body() {
    let mut sys = System::new();
    let (a, b) = (sys.add_point2(0.0, 0.0), sys.add_point2(10.0, 0.0));
    sys.add(ConstraintId(1), Constraint::Fix(a.to_vec()));
    sys.add(ConstraintId(2), Constraint::Distance { p: a, q: b, value: 10.0 });
    let sys = solved(sys);
    let mut drag = sys.begin_drag(&Grab::Body(vec![a, b]), [5.0, 0.0]).unwrap();
    // The body is pulled upwards; the free end can only follow on its circle.
    assert_eq!(drag.update([5.0, 4.0]), FrameOutcome::Clamped);
    let sys = drag.commit();
    assert_eq!(sys.point2(a), [0.0, 0.0]);
    let end = sys.point2(b);
    assert!(end[1] > 1.0 && (dist(end, [0.0, 0.0]) - 10.0).abs() < 1e-9, "{end:?}");
}

#[test]
fn f17_fully_constrained_geometry_cannot_be_grabbed() {
    let mut sys = System::new();
    let (a, b, c) = (sys.add_point2(0.0, 0.0), sys.add_point2(10.0, 0.0), sys.add_point2(5.0, 5.0));
    sys.add(ConstraintId(1), Constraint::Fix(a.to_vec()));
    sys.add(ConstraintId(2), Constraint::Fix(b.to_vec()));
    sys.add(ConstraintId(3), Constraint::Distance { p: a, q: c, value: 50.0_f64.sqrt() });
    sys.add(ConstraintId(4), Constraint::Distance { p: b, q: c, value: 50.0_f64.sqrt() });
    let sys = solved(sys);
    let before = sys.point2(c).map(f64::to_bits);
    let (sys, refused) = sys.begin_drag(&Grab::Point(c), [5.0, 5.0]).unwrap_err();
    assert_eq!(refused, DragRefused::NothingMobile);
    assert_eq!(sys.point2(c).map(f64::to_bits), before);
}

#[test]
fn cancel_restores_every_bit_and_commit_keeps_the_last_frame() {
    let (sys, p) = triangle();
    let before: Vec<[u64; 2]> = p.iter().map(|q| sys.point2(*q).map(f64::to_bits)).collect();
    let mut drag = sys.clone().begin_drag(&Grab::Point(p[1]), [10.0, 0.0]).unwrap();
    assert_eq!(drag.update([12.0, 3.0]), FrameOutcome::Moved);
    assert_ne!(drag.system().point2(p[1]).map(f64::to_bits), before[1]);
    let cancelled = drag.clone().cancel();
    let after: Vec<[u64; 2]> = p.iter().map(|q| cancelled.point2(*q).map(f64::to_bits)).collect();
    assert_eq!(after, before);
    let committed = drag.commit();
    assert!(dist(committed.point2(p[1]), [12.0, 3.0]) < 1e-7);
}

#[test]
fn unconstrained_geometry_follows_freely_even_through_collinearity() {
    // A free polyline A-B-C: dragging B across the line through A and C must not be refused.
    let mut sys = System::new();
    let (a, b1) = (sys.add_point2(0.0, 0.0), sys.add_point2(5.0, 2.0));
    let (b2, c) = (sys.add_point2(5.0, 2.0), sys.add_point2(10.0, 0.0));
    sys.add(ConstraintId(1), Constraint::Coincident(b1, b2));
    let sys = solved(sys);
    let mut drag = sys.begin_drag(&Grab::Point(b1), [5.0, 2.0]).unwrap();
    assert_eq!(drag.update([5.0, -2.0]), FrameOutcome::Moved);
    let sys = drag.commit();
    assert_eq!(sys.point2(b1), [5.0, -2.0]);
    assert_eq!(sys.point2(b2), [5.0, -2.0]);
    assert_eq!((sys.point2(a), sys.point2(c)), ([0.0, 0.0], [10.0, 0.0]), "untouched points stay exactly");
}

/// F9: a fixed line continued by a tangent arc.
fn line_with_tangent_arc() -> (System, P2, P2, ParamId2) {
    let mut sys = System::new();
    let (a, b) = (sys.add_point2(0.0, 0.0), sys.add_point2(10.0, 0.0));
    let (c, s, e) = (sys.add_point2(10.0, 5.0), sys.add_point2(10.0, 0.0), sys.add_point2(15.0, 5.0));
    let r = sys.add_radius(5.0);
    let round = Round { c, r };
    sys.add(ConstraintId(1), Constraint::Fix([a, b].concat()));
    sys.add(ConstraintId(2), Constraint::Arc { round, start: s, end: e });
    sys.add(ConstraintId(3), Constraint::Coincident(b, s));
    sys.add(
        ConstraintId(4),
        Constraint::EndTangent { line: line(a, b), round, at: s, smooth: true, leaving: 1.0 },
    );
    (solved(sys), c, s, ParamId2(r))
}
struct ParamId2(gc_solver::ParamId);

#[test]
fn f9_a_tangent_arc_never_folds_back_over_its_line() {
    let (sys, c, s, r) = line_with_tangent_arc();
    let mut drag = sys.begin_drag(&Grab::Point(c), [10.0, 5.0]).unwrap();
    let mut rejected = 0;
    for k in 1..=40 {
        let cursor = [10.0, 5.0 - 0.25 * f64::from(k)];
        let outcome = drag.update(cursor);
        let sys = drag.system();
        let (centre, start) = (sys.point2(c), sys.point2(s));
        // The centre stays on the left of the line direction (+x): cross((1, 0), c - s) > 0.
        assert!(centre[1] - start[1] > 0.0, "frame {k}: centre {centre:?}");
        assert!(sys.value(r.0) > 0.0);
        assert!(sys.violation() <= 1.0);
        // Below the line the cursor cannot be followed: the frame is clamped or refused, never "moved".
        if cursor[1] < 0.0 {
            assert_ne!(outcome, FrameOutcome::Moved, "frame {k}");
            rejected += 1;
        }
    }
    assert!(rejected > 0);
}

#[test]
fn f10_an_arc_end_cannot_be_dragged_through_its_start() {
    let mut sys = System::new();
    let ten_degrees = PI / 18.0;
    let (c, s) = (sys.add_point2(0.0, 0.0), sys.add_point2(10.0, 0.0));
    let e = sys.add_point2(10.0 * libm::cos(ten_degrees), 10.0 * libm::sin(ten_degrees));
    let r = sys.add_radius(10.0);
    sys.add(ConstraintId(1), Constraint::Fix([c, s].concat()));
    sys.add(ConstraintId(2), Constraint::Arc { round: Round { c, r }, start: s, end: e });
    let sys = solved(sys);
    let mut drag = sys.begin_drag(&Grab::Point(e), sys_point(10.0, ten_degrees)).unwrap();
    let mut flipped = false;
    for k in 1..=20 {
        let angle = ten_degrees - f64::from(k) * (PI / 180.0);
        let outcome = drag.update(sys_point(10.0, angle));
        let end = drag.system().point2(e);
        assert!(end[1] > 0.0, "the arc never becomes a 350 degree arc: {end:?}");
        if let FrameOutcome::Flipped(flips) = outcome {
            flipped = true;
            assert!(flips.contains(&Flip::ArcSweep(ConstraintId(2))));
        }
    }
    assert!(flipped);
}

fn sys_point(radius: f64, angle: f64) -> [f64; 2] {
    [radius * libm::cos(angle), radius * libm::sin(angle)]
}

#[test]
fn an_arc_end_may_be_dragged_past_half_a_turn() {
    let mut sys = System::new();
    let start_angle = f64::to_radians(170.0);
    let (c, s) = (sys.add_point2(0.0, 0.0), sys.add_point2(10.0, 0.0));
    let e = sys.add_point2(10.0 * libm::cos(start_angle), 10.0 * libm::sin(start_angle));
    let r = sys.add_radius(10.0);
    sys.add(ConstraintId(1), Constraint::Fix([c, s].concat()));
    sys.add(ConstraintId(2), Constraint::Arc { round: Round { c, r }, start: s, end: e });
    let sys = solved(sys);
    let mut drag = sys.begin_drag(&Grab::Point(e), sys_point(10.0, start_angle)).unwrap();
    for degrees in [175.0_f64, 180.0, 185.0, 190.0] {
        assert_eq!(
            drag.update(sys_point(10.0, degrees.to_radians())),
            FrameOutcome::Moved,
            "{degrees} degrees"
        );
    }
    assert!(drag.system().point2(e)[1] < 0.0);
}

#[test]
fn f13_a_tangent_circle_keeps_its_side_and_a_positive_radius() {
    let mut sys = System::new();
    let (a, b) = (sys.add_point2(0.0, 0.0), sys.add_point2(10.0, 0.0));
    let c = sys.add_point2(3.0, 2.0);
    let r = sys.add_radius(2.0);
    sys.add(ConstraintId(1), Constraint::Fix([a, b].concat()));
    sys.add(
        ConstraintId(2),
        Constraint::TangentLineCircle { line: line(a, b), round: Round { c, r }, side: 1.0 },
    );
    let sys = solved(sys);
    let mut drag = sys.begin_drag(&Grab::Point(c), [3.0, 2.0]).unwrap();
    for k in 1..=16 {
        let outcome = drag.update([3.0, 2.0 - 0.25 * f64::from(k)]);
        let sys = drag.system();
        assert!(sys.value(r) > 0.0 && sys.point2(c)[1] > 0.0, "frame {k}: {outcome:?}");
        assert!((sys.point2(c)[1] - sys.value(r)).abs() < 1e-8, "still tangent on the same side");
    }
}

#[test]
fn dragging_a_rim_changes_the_radius() {
    let mut sys = System::new();
    let c = sys.add_point2(1.0, 1.0);
    let r = sys.add_radius(2.0);
    sys.add(ConstraintId(1), Constraint::Fix(c.to_vec()));
    let sys = solved(sys);
    let mut drag = sys.begin_drag(&Grab::Rim(Round { c, r }), [3.0, 1.0]).unwrap();
    assert_eq!(drag.update([1.0, 6.0]), FrameOutcome::Moved);
    let sys = drag.commit();
    assert!((sys.value(r) - 5.0).abs() < 1e-9);
    assert_eq!(sys.point2(c), [1.0, 1.0]);
}
