//! Helical Curve in a 3D sketch (Inventor 2026 help, To Draw 3D Curves, GUID-37BA1DCE-6BC4-41CD-A36D-577F31B304FC):
//! a constant helix by Pitch and Revolution, Revolution and Height, Pitch and Height or Spiral, with taper
//! and hand.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "tests")]

use gc_doc::{Command, Created, HelixShape, SketchOp};
use gc_session::Session;
use gc_sketch::CurveGeom;

fn helix(
    s: &mut Session,
    k: gc_core::Id<gc_doc::Sketch>,
    shape: HelixShape,
    taper: f64,
    right_hand: bool,
) -> Vec<[f64; 3]> {
    let op = SketchOp::AddHelix {
        start: [0.0, 0.0, 0.0],
        end: [0.0, 0.0, 1.0],
        diameter: 0.04,
        shape,
        taper,
        right_hand,
    };
    let created = s.execute(&Command::Sketch { sketch: k, op }).unwrap();
    let curve =
        created.iter().find_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None }).unwrap();
    let sk = s.active_sketch().unwrap().1;
    let CurveGeom::Spline { points } = &sk.curves[&curve].geom else { panic!() };
    points.iter().map(|p| sk.pos(*p).unwrap()).collect()
}

#[test]
fn a_helix_climbs_round_its_axis() {
    let mut s = Session::new();
    let k = s.create_sketch_3d().unwrap();
    let steps = s.history().undo_depth();
    let fit = helix(&mut s, k, HelixShape::PitchRevolution { pitch: 0.01, revolutions: 3.0 }, 0.0, true);
    assert_eq!(s.history().undo_depth(), steps + 1);
    assert_eq!(fit.len(), 25, "eight fit points a turn");
    for p in &fit {
        assert!((gc_math::fm::hypot(p[0], p[1]) - 0.02).abs() < 1e-12, "on the diameter: {p:?}");
    }
    assert!(fit[0][2].abs() < 1e-12 && (fit[24][2] - 0.03).abs() < 1e-12, "three pitches high");
    // Right hand: counter-clockwise seen from the top, rising.
    let turn = |a: [f64; 3], b: [f64; 3]| a[0] * b[1] - a[1] * b[0];
    assert!(fit.windows(2).all(|w| turn(w[0], w[1]) > 0.0 && w[1][2] > w[0][2]));
    // Left hand turns the other way.
    let left = helix(&mut s, k, HelixShape::PitchRevolution { pitch: 0.01, revolutions: 1.0 }, 0.0, false);
    assert!(left.windows(2).all(|w| turn(w[0], w[1]) < 0.0));
    // The other definitions make the same helix.
    let a = helix(&mut s, k, HelixShape::RevolutionHeight { revolutions: 2.0, height: 0.02 }, 0.0, true);
    let b = helix(&mut s, k, HelixShape::PitchHeight { pitch: 0.01, height: 0.02 }, 0.0, true);
    assert_eq!(a.len(), b.len());
    assert!(a.iter().zip(&b).all(|(p, q)| (0..3).all(|i| (p[i] - q[i]).abs() < 1e-12)));
}

#[test]
fn spirals_tapers_and_bad_values() {
    let mut s = Session::new();
    let k = s.create_sketch_3d().unwrap();
    // A spiral is flat and its radius grows by the pitch each turn.
    let spiral = helix(&mut s, k, HelixShape::Spiral { pitch: 0.005, revolutions: 2.0 }, 0.0, true);
    assert!(spiral.iter().all(|p| p[2].abs() < 1e-12));
    let last = spiral[spiral.len() - 1];
    assert!((gc_math::fm::hypot(last[0], last[1]) - 0.03).abs() < 1e-12);
    // A taper widens it with height.
    let taper = 10f64.to_radians();
    let cone = helix(&mut s, k, HelixShape::PitchRevolution { pitch: 0.01, revolutions: 2.0 }, taper, true);
    let top = cone[cone.len() - 1];
    assert!((gc_math::fm::hypot(top[0], top[1]) - (0.02 + 0.02 * gc_math::fm::tan(taper))).abs() < 1e-12);
    // Values that make no helix, and a 2D sketch: refused with no trace.
    let before = s.doc().clone();
    for shape in [
        HelixShape::PitchRevolution { pitch: 0.0, revolutions: 2.0 },
        HelixShape::PitchRevolution { pitch: 0.01, revolutions: -1.0 },
        HelixShape::RevolutionHeight { revolutions: 2.0, height: f64::NAN },
        HelixShape::PitchRevolution { pitch: 0.01, revolutions: 1e6 },
    ] {
        let op = SketchOp::AddHelix {
            start: [0.0; 3],
            end: [0.0, 0.0, 1.0],
            diameter: 0.04,
            shape,
            taper: 0.0,
            right_hand: true,
        };
        assert!(s.execute(&Command::Sketch { sketch: k, op }).is_err(), "{shape:?}");
    }
    // A taper that shrinks the radius to nothing.
    let op = SketchOp::AddHelix {
        start: [0.0; 3],
        end: [0.0, 0.0, 1.0],
        diameter: 0.04,
        shape: HelixShape::PitchRevolution { pitch: 0.1, revolutions: 5.0 },
        taper: -1.0,
        right_hand: true,
    };
    assert!(s.execute(&Command::Sketch { sketch: k, op }).is_err());
    assert!(s.doc().same_content(&before));
    let flat = s.create_sketch("XY").unwrap();
    let op = SketchOp::AddHelix {
        start: [0.0; 3],
        end: [0.0, 0.0, 1.0],
        diameter: 0.04,
        shape: HelixShape::PitchRevolution { pitch: 0.01, revolutions: 1.0 },
        taper: 0.0,
        right_hand: true,
    };
    assert!(s.execute(&Command::Sketch { sketch: flat, op }).is_err());
}
