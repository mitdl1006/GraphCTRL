//! Blended motion: timing against closed forms, limits, and what happens at corners.

#![allow(clippy::float_cmp, clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::f64::consts::PI;

use gc_traj::{Limits, Trajectory, Trapezoid};

fn limits(v: &[f64], a: f64) -> Limits {
    Limits { max_vel: v.to_vec(), max_acc: vec![a; v.len()] }
}

fn speed(trajectory: &Trajectory<gc_traj::Validated>, t: f64) -> f64 {
    trajectory.sample(t).qd.iter().map(|v| v * v).sum::<f64>().sqrt()
}

#[test]
fn a_straight_move_takes_the_trapezoid_time() {
    // The closed-form durations of a synchronised trapezoid (see the unit tests of the crate).
    let cases: [(&[f64], Limits, f64); 4] = [
        (&[1.0, 0.0, 0.0, 0.0, 0.0, 0.0], limits(&[PI; 6], 5.0), 0.894_427),
        (&[PI, 0.0, 0.0, 0.0, 0.0, 0.0], limits(&[PI; 6], 5.0), 1.628_319),
        (&[1.0, 0.0, 2.0, 0.0, 0.0, 0.0], limits(&[2.094_395, 2.094_395, PI, PI, PI, PI], 5.0), 1.264_938),
        (&[0.5], limits(&[1.05], 1.4), 1.195_229),
    ];
    for (delta, limits, expected) in cases {
        let path = [vec![0.0; delta.len()], delta.to_vec()];
        let timed = Trajectory::blended(&path, &limits, 0.05, &|_| true).unwrap();
        let exact = Trapezoid::for_move(delta, &limits).unwrap().duration;
        assert!((exact - expected).abs() < 1e-5);
        assert!((timed.duration() - exact).abs() < 0.005 * exact, "{} vs {exact}", timed.duration());
        assert!(timed.duration() >= exact - 1e-9, "never faster than the limits allow");
        let timed = timed.validate(&limits, 0.001, &|_| true).unwrap();
        assert_eq!(timed.sample(0.0).q, path[0]);
        let end = timed.sample(timed.duration()).q;
        assert!(end.iter().zip(&path[1]).all(|(a, b)| (a - b).abs() < 1e-9), "{end:?}");
    }
}

#[test]
fn a_corner_is_rounded_and_passed_without_stopping() {
    let path = [vec![0.0, 0.0], vec![1.0, 0.0], vec![1.0, 1.0]];
    let limits = limits(&[2.0, 2.0], 5.0);
    let stopping = Trajectory::stopping_at_waypoints(&path, &limits).unwrap().duration();
    let blended = Trajectory::blended(&path, &limits, 0.05, &|_| true).unwrap();
    assert!(blended.is_blended());
    let blended = blended.validate(&limits, 0.0005, &|_| true).unwrap();
    assert!(blended.duration() < 0.9 * stopping, "{} vs {stopping}", blended.duration());

    // At the time of the middle waypoint the motion is on the blend: close to the corner, inside it,
    // and moving.
    let at_corner = blended.waypoint_times()[1];
    let q = blended.sample(at_corner).q;
    let away = ((q[0] - 1.0).powi(2) + q[1].powi(2)).sqrt();
    assert!(away > 1e-3 && away <= 0.05 + 1e-9, "{away}");
    assert!(q[0] < 1.0 && q[1] > 0.0, "{q:?}");
    assert!(speed(&blended, at_corner) > 0.3);
    // It starts and ends at rest, exactly on the end points, and never leaves the corner's square.
    assert_eq!(blended.sample(0.0).q, path[0]);
    let end = blended.sample(blended.duration());
    assert!((end.q[0] - 1.0).abs() < 1e-9 && (end.q[1] - 1.0).abs() < 1e-9 && speed(&blended, 1e9) < 1e-6);
    for k in 0..=1000 {
        let q = blended.sample(blended.duration() * f64::from(k) / 1000.0).q;
        assert!((-1e-9..=1.0 + 1e-9).contains(&q[0]) && (-1e-9..=1.0 + 1e-9).contains(&q[1]), "{q:?}");
    }
}

#[test]
fn a_corner_that_cannot_be_rounded_is_a_stop() {
    let path = [vec![0.0, 0.0], vec![1.0, 0.0], vec![1.0, 1.0]];
    let limits = limits(&[2.0, 2.0], 5.0);
    // Only the two segments themselves are allowed: any blend cuts the corner and is rejected.
    let on_segments = |q: &[f64]| q[1].abs() < 1e-9 || (q[0] - 1.0).abs() < 1e-9;
    let timed = Trajectory::blended(&path, &limits, 0.05, &on_segments).unwrap();
    let timed = timed.validate(&limits, 0.0005, &on_segments).unwrap();
    let at_corner = timed.waypoint_times()[1];
    let q = timed.sample(at_corner).q;
    assert!((q[0] - 1.0).abs() < 1e-9 && q[1].abs() < 1e-9, "{q:?}");
    assert!(speed(&timed, at_corner) < 1e-6);
    // Two rest-to-rest moves of one radian each.
    let one = Trapezoid::for_move(&[1.0, 0.0], &limits).unwrap().duration;
    assert!((timed.duration() - 2.0 * one).abs() < 0.01 * one, "{} vs {}", timed.duration(), 2.0 * one);
}

#[test]
fn zigzags_and_reversals_stay_within_the_limits() {
    // A deterministic zigzag in six joints, with a reversal and a repeated point in it.
    let mut state = 7_u64;
    let mut unit = || {
        state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (state >> 11) as f64 / (1_u64 << 53) as f64
    };
    let mut path: Vec<Vec<f64>> = (0..9).map(|_| (0..6).map(|_| 2.0 * unit() - 1.0).collect()).collect();
    let back = path[3].clone();
    path.insert(5, back); // go to path[4] and straight back
    let twice = path[7].clone();
    path.insert(7, twice);
    let limits = limits(&[PI, PI, PI, 2.0 * PI, 2.0 * PI, 2.0 * PI], 5.0);
    let stopping = Trajectory::stopping_at_waypoints(&path, &limits).unwrap().duration();
    let timed = Trajectory::blended(&path, &limits, 0.1, &|_| true).unwrap();
    let timed = timed.validate(&limits, 0.0005, &|_| true).unwrap();
    assert!(timed.duration() < stopping, "{} vs {stopping}", timed.duration());
    assert_eq!(timed.waypoint_times().len(), path.len());
    assert!(timed.waypoint_times().windows(2).all(|w| w[0] <= w[1]));
    let end = timed.sample(timed.duration()).q;
    assert!(end.iter().zip(path.last().unwrap()).all(|(a, b)| (a - b).abs() < 1e-9));
    // The same input gives the same motion, bit for bit.
    let again = Trajectory::blended(&path, &limits, 0.1, &|_| true).unwrap();
    assert_eq!(again.duration().to_bits(), timed.duration().to_bits());
}
