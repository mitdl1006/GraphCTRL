//! Timing report for the solver. Not a pass/fail test: run it with
//! `cargo test -p gc_solver --release --test perf -- --ignored --nocapture`.

#![allow(clippy::unwrap_used, clippy::print_stdout, reason = "a timing report printed by a test")]

use std::time::Instant;

use gc_solver::{Constraint, ConstraintId, Grab, Line2, Status, System};

/// A staircase of `points` points (every corner a right angle, every step 12 long), drawn a little off
/// so that the first solve has work to do. Constrained: all lengths, every second corner perpendicular.
fn chain(points: usize) -> (System, Vec<gc_solver::P2>) {
    let mut sys = System::new();
    let mut state = 7_u64;
    let mut wobble = || {
        state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        ((state >> 40) as f64 / (1_u64 << 24) as f64 - 0.5) * 0.6
    };
    let p: Vec<_> = (0..points)
        .map(|i| sys.add_point2(12.0 * i.div_ceil(2) as f64 + wobble(), 12.0 * (i / 2) as f64 + wobble()))
        .collect();
    let mut id = 0;
    let mut next = || {
        id += 1;
        ConstraintId(id)
    };
    sys.add(next(), Constraint::Fix(p[0].to_vec()));
    for i in 0..points - 1 {
        sys.add(next(), Constraint::Distance { p: p[i], q: p[i + 1], value: 12.0 });
    }
    for i in (0..points - 2).step_by(2) {
        let (a, b) = (Line2 { a: p[i], b: p[i + 1] }, Line2 { a: p[i + 1], b: p[i + 2] });
        sys.add(next(), Constraint::Perpendicular(a, b));
    }
    (sys, p)
}

#[test]
#[ignore = "timing report, run on demand in release"]
fn report_solve_and_drag_times() {
    for unknowns in [50, 100, 300] {
        let (mut sys, p) = chain(unknowns / 2);
        let started = Instant::now();
        let report = sys.solve();
        let cold = started.elapsed();
        assert_eq!(report.status, Status::Solved);
        let started = Instant::now();
        let again = sys.solve();
        let warm = started.elapsed();
        assert!(!again.changed);

        let last = p[p.len() - 1];
        let from = sys.point2(last);
        let mut drag = sys.begin_drag(&Grab::Point(last), from).unwrap();
        let frames = 30;
        let started = Instant::now();
        for k in 1..=frames {
            drag.update([from[0] + 0.2 * f64::from(k), from[1] + 0.1 * f64::from(k)]);
        }
        let per_frame = started.elapsed() / frames as u32;
        println!(
            "{unknowns:>4} unknowns, dof {:>3}: first solve {cold:>10.2?}, re-solve {warm:>10.2?}, drag frame {per_frame:>10.2?}",
            report.dof
        );
    }
}
