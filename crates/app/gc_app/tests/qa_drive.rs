//! QA edge cases for the Drive sequence (Inventor 2026 help, Drive Relationship Reference:
//! <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-267E6CE6-7B8D-4972-A20A-350477A447DF.htm>
//! and To Drive Relationships in Assemblies:
//! <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-B78E59C9-F9FB-4ED5-B60E-C9198770B1AF.htm>).
//!
//! From the help: Start defaults to the current value and End to Start + 10; the increment is an amount (default
//! 1.0) or a total number of equal steps; Start/End "drives from the start value to the end value and resets at
//! the start value"; Start/End/Start goes there and back, each movement one cycle ("to move out, in and out
//! again requires three cycles"). The sequence function is private to the application crate, so this file
//! compiles the module itself.

#![allow(
    dead_code,
    unused_imports,
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    reason = "test code; the module is included whole"
)]

#[path = "../src/drive.rs"]
mod drive;

use drive::{Increment, Repeat, sequence};

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

/// Every value stays between Start and End (a drive must never overshoot the range the user gave).
fn within(values: &[f64], start: f64, end: f64) -> bool {
    let (low, high) = (start.min(end) - 1e-9, start.max(end) + 1e-9);
    values.iter().all(|v| v.is_finite() && *v >= low && *v <= high)
}

#[test]
fn one_total_step_goes_from_start_to_end_in_one_move() {
    assert_eq!(sequence(0.0, 10.0, Increment::Steps, 1.0, Repeat::StartEnd, 1), vec![0.0, 10.0]);
    assert_eq!(sequence(10.0, 0.0, Increment::Steps, 1.0, Repeat::StartEnd, 1), vec![10.0, 0.0]);
    // Fractions of a step are rounded, never zero steps (no divide by zero, no empty sequence).
    for steps in [0.0, 0.4, -3.0, f64::NAN] {
        let v = sequence(0.0, 10.0, Increment::Steps, steps, Repeat::StartEnd, 1);
        assert!(v.len() >= 2 && within(&v, 0.0, 10.0), "{steps}: {v:?}");
        assert_eq!((v[0], *v.last().unwrap()), (0.0, 10.0), "{steps}");
    }
}

#[test]
fn end_equal_to_start_stays_put() {
    for increment in [Increment::Amount, Increment::Steps] {
        for repeat in [Repeat::StartEnd, Repeat::StartEndStart] {
            for step in [1.0, 0.0, 5.0] {
                let v = sequence(3.0, 3.0, increment, step, repeat, 2);
                assert!(
                    !v.is_empty() && v.iter().all(|x| *x == 3.0),
                    "{increment:?} {repeat:?} {step}: {v:?}"
                );
            }
        }
    }
}

#[test]
fn a_reverse_range_counts_down_to_exactly_end() {
    let v = sequence(10.0, 0.0, Increment::Amount, 3.0, Repeat::StartEnd, 1);
    assert_eq!(v, vec![10.0, 7.0, 4.0, 1.0, 0.0], "the last step is cut short at End");
    // A negative amount means the same as a positive one: the direction comes from Start and End.
    assert_eq!(sequence(10.0, 0.0, Increment::Amount, -3.0, Repeat::StartEnd, 1), v);
    assert_eq!(sequence(0.0, 10.0, Increment::Amount, -4.0, Repeat::StartEnd, 1), vec![0.0, 4.0, 8.0, 10.0]);
}

#[test]
fn an_increment_larger_than_the_range_or_unusable_still_ends_at_end() {
    assert_eq!(sequence(0.0, 10.0, Increment::Amount, 50.0, Repeat::StartEnd, 1), vec![0.0, 10.0]);
    // The smallest increment that still fits under the step cap.
    let v = sequence(0.0, 10.0, Increment::Amount, 1e-3, Repeat::StartEnd, 1);
    assert!(within(&v, 0.0, 10.0));
    assert!(v.len() <= 100_001 + 1, "{} values", v.len());
    assert_eq!((v[0], *v.last().unwrap()), (0.0, 10.0));
}

#[test]
fn start_end_resets_at_start_and_start_end_start_goes_there_and_back() {
    let once = sequence(0.0, 4.0, Increment::Amount, 1.0, Repeat::StartEnd, 1);
    assert_eq!(once, vec![0.0, 1.0, 2.0, 3.0, 4.0]);
    // Start/End twice: the second repetition begins again at Start.
    let twice = sequence(0.0, 4.0, Increment::Amount, 1.0, Repeat::StartEnd, 2);
    assert_eq!(twice, vec![0.0, 1.0, 2.0, 3.0, 4.0, 0.0, 1.0, 2.0, 3.0, 4.0]);
    // Start/End/Start with 3 cycles: out, in, out. Each turn happens once (no repeated value).
    let cycles = sequence(0.0, 4.0, Increment::Amount, 1.0, Repeat::StartEndStart, 3);
    assert_eq!(cycles, vec![0.0, 1.0, 2.0, 3.0, 4.0, 3.0, 2.0, 1.0, 0.0, 1.0, 2.0, 3.0, 4.0]);
    // One cycle is just the way out; two cycles end back at Start.
    assert_eq!(*sequence(0.0, 4.0, Increment::Amount, 1.0, Repeat::StartEndStart, 1).last().unwrap(), 4.0);
    assert_eq!(*sequence(0.0, 4.0, Increment::Amount, 1.0, Repeat::StartEndStart, 2).last().unwrap(), 0.0);
    // With one step per movement the turn points are Start and End alternately.
    assert_eq!(
        sequence(0.0, 4.0, Increment::Steps, 1.0, Repeat::StartEndStart, 4),
        vec![0.0, 4.0, 0.0, 4.0, 0.0]
    );
}

#[test]
fn absurd_counts_and_values_stay_bounded_and_finite() {
    for count in [0, 1, 1000, 1_000_000, u32::MAX] {
        for repeat in [Repeat::StartEnd, Repeat::StartEndStart] {
            let v = sequence(0.0, 10.0, Increment::Steps, 10.0, repeat, count);
            assert!(!v.is_empty() && v.len() <= 20_000, "count {count}: {} values", v.len());
            assert!(within(&v, 0.0, 10.0));
        }
    }
    // Huge and tiny ranges: no panic, no unbounded allocation.
    for (start, end) in [(0.0, 1e300), (-1e300, 1e300), (0.0, 1e-300)] {
        let v = sequence(start, end, Increment::Amount, 1.0, Repeat::StartEnd, 1);
        assert!(v.len() <= 100_001 + 1, "{start} {end}: {} values", v.len());
        let w = sequence(start, end, Increment::Steps, 5.0, Repeat::StartEndStart, 2);
        assert!(w.len() <= 100_001 * 2, "{start} {end}");
    }
}

/// An increment of 0, "nan", "inf" or one so small that the 100 000-step cap is hit before End (1e-6 over a span of 10
/// stops at 0.1) is accepted and the drive silently stays at Start (or produces NaN joint
/// values): a sequence from Start to End must always reach End, whatever the increment box holds.
#[test]
fn an_unusable_increment_still_reaches_end() {
    for step in [0.0, -0.0, f64::NAN, f64::INFINITY, 1e-300, 1e-6] {
        let v = sequence(0.0, 10.0, Increment::Amount, step, Repeat::StartEnd, 1);
        assert!(within(&v, 0.0, 10.0), "{step}: {v:?}");
        assert_eq!(*v.last().unwrap(), 10.0, "{step}: {v:?}");
    }
}

/// A NaN or infinite Start or End (typed as "nan" or "inf" into a number box) panics inside `f64::clamp`.
#[test]
fn a_nan_start_or_end_does_not_panic() {
    for (start, end) in [
        (f64::NAN, 1.0),
        (0.0, f64::NAN),
        (f64::NAN, f64::NAN),
        (0.0, f64::INFINITY),
        (f64::NEG_INFINITY, f64::INFINITY),
    ] {
        drop(sequence(start, end, Increment::Amount, 1.0, Repeat::StartEnd, 1));
        drop(sequence(start, end, Increment::Steps, 3.0, Repeat::StartEndStart, 2));
    }
}
