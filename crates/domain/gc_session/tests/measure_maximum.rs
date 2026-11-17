//! Measure's Maximum Distance between two components (Inventor 2026 help, To Measure Distance ...,
//! GUID-1805AB8F-F782-4B24-A332-B4F2E10A3D00: "Toggle between Minimum Distance, Center to Center, and
//! Maximum Distance").

#![allow(clippy::unwrap_used, reason = "test code")]

use gc_doc::{Placement, Solid};
use gc_session::{CellItem, Session};

#[test]
fn the_pair_measure_gives_minimum_centre_and_maximum_distances() {
    let mut session = Session::new();
    let a = session
        .add_body(Solid::Box { size: [0.2, 0.2, 0.2] }, Placement { xyz: [0.0, 0.0, 0.0], rpy: [0.0; 3] })
        .unwrap();
    let b = session
        .add_body(Solid::Sphere { radius: 0.1 }, Placement { xyz: [1.0, 0.0, 0.0], rpy: [0.0; 3] })
        .unwrap();
    let pair = session.measure_pair(CellItem::Body(a), CellItem::Body(b)).unwrap();
    assert!((pair.minimum.unwrap().distance - 0.8).abs() < 1e-9);
    assert!((pair.centre_to_centre - 1.0).abs() < 1e-12);
    // From the far corner of the box (-0.1, ±0.1, ±0.1) to the far side of the sphere.
    let corner = gc_math::DVec3::new(-0.1, 0.1, 0.1);
    let expected = (gc_math::DVec3::new(1.0, 0.0, 0.0) - corner).length() + 0.1;
    let maximum = pair.maximum.unwrap();
    assert!((maximum.distance - expected).abs() < 1e-9, "{maximum:?} vs {expected}");
    assert!(
        maximum.distance >= pair.centre_to_centre && pair.centre_to_centre >= pair.minimum.unwrap().distance
    );
}
