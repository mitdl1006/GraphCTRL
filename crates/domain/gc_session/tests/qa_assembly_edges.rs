//! QA edge cases for the assembly features: component states (first component grounded: lead decision
//! 2026-11-17), Measure, Analyze Interference. Inventor 2026 help:
//! - components/browser: <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-5F975252-BDB4-488C-A2C5-8B15C7D3C641.htm>
//! - measure: <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-1805AB8F-F782-4B24-A332-B4F2E10A3D00.htm>
//! - interference: <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-80DED0A6-AC65-4C7C-AD85-0144EE6DAB5B.htm>

#![allow(clippy::unwrap_used, clippy::panic, clippy::float_cmp, reason = "test code")]

use gc_doc::{ComponentFlag, Occurrence, Placement, Solid};
use gc_math::DVec3;
use gc_session::{CellItem, Feature, Session, measure_features};

fn cube() -> Solid {
    Solid::Box { size: [0.2, 0.2, 0.2] }
}

fn at(x: f64) -> Placement {
    Placement { xyz: [x, 0.0, 0.0], rpy: [0.0; 3] }
}

fn body(session: &mut Session, x: f64) -> gc_core::Id<gc_doc::Body> {
    session.add_body(cube(), at(x)).unwrap()
}

fn item(id: gc_core::Id<gc_doc::Body>) -> Occurrence {
    Occurrence::Item(id.erase())
}

// ---- component states ----

/// Lead decision 2: the first component is grounded, later ones are not; undoing the first removes it.
#[test]
fn only_the_first_component_is_grounded() {
    let mut s = Session::new();
    let first = body(&mut s, 0.0);
    let second = body(&mut s, 0.5);
    let third = body(&mut s, 1.0);
    let nodes = s.components();
    let grounded: Vec<bool> = [first, second, third]
        .iter()
        .map(|b| nodes.iter().find(|n| n.occurrence == item(*b)).unwrap().grounded)
        .collect();
    assert_eq!(grounded, [true, false, false]);
    // Deleting the grounded one does not ground another behind the user's back.
    s.delete_item(CellItem::Body(first)).unwrap();
    assert!(s.components().iter().all(|n| !n.grounded), "{:?}", s.components());
    // Undo brings it back grounded.
    assert!(s.undo());
    assert!(s.components().iter().find(|n| n.occurrence == item(first)).unwrap().grounded);
}

/// A state of something that does not exist is refused and changes nothing; setting a state to what it
/// already is costs no history; every state switch is one undo step and restores the document exactly.
#[test]
fn component_states_switch_cleanly_and_refuse_nonsense() {
    let mut s = Session::new();
    let a = body(&mut s, 0.0);
    let b = body(&mut s, 0.5);
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    let ghost = Occurrence::Item(
        gc_core::Id::<gc_doc::Body>::from_raw(std::num::NonZeroU64::new(9999).unwrap()).erase(),
    );
    for flag in
        [ComponentFlag::Grounded, ComponentFlag::Visible, ComponentFlag::Enabled, ComponentFlag::Suppressed]
    {
        assert!(s.set_component(ghost, flag, true).is_err(), "{flag:?}");
        let part = Occurrence::Part {
            assembly: gc_core::Id::<gc_doc::Assembly>::from_raw(std::num::NonZeroU64::new(9999).unwrap()),
            index: 0,
        };
        assert!(s.set_component(part, flag, false).is_err(), "{flag:?}");
    }
    assert!(s.doc().same_content(&before));
    assert_eq!(s.history().undo_depth(), depth);

    for flag in [ComponentFlag::Grounded, ComponentFlag::Visible, ComponentFlag::Enabled] {
        let snapshot = s.doc().clone();
        let steps = s.history().undo_depth();
        let current = s.components().iter().find(|n| n.occurrence == item(b)).map(|n| match flag {
            ComponentFlag::Grounded => n.grounded,
            ComponentFlag::Visible => n.visible,
            ComponentFlag::Enabled => n.enabled,
            ComponentFlag::Suppressed => n.suppressed,
        });
        s.set_component(item(b), flag, !current.unwrap()).unwrap();
        assert_eq!(s.history().undo_depth(), steps + 1, "{flag:?} is one step");
        assert!(s.undo());
        assert!(s.doc().same_content(&snapshot), "{flag:?}");
    }
    // Setting the same value twice never leaves the document in a different state than setting it once.
    s.set_component(item(a), ComponentFlag::Enabled, false).unwrap();
    let once = s.doc().clone();
    drop(s.set_component(item(a), ComponentFlag::Enabled, false));
    assert!(s.doc().same_content(&once));
}

// ---- measure ----

/// Measuring something that has been deleted since it was picked gives nothing, not a panic.
#[test]
fn measuring_a_deleted_item_gives_nothing() {
    let mut s = Session::new();
    let a = body(&mut s, 0.0);
    let b = body(&mut s, 0.5);
    assert!(s.measure_pair(CellItem::Body(a), CellItem::Body(b)).is_some());
    s.delete_item(CellItem::Body(b)).unwrap();
    assert!(s.measure_pair(CellItem::Body(a), CellItem::Body(b)).is_none());
    assert!(s.measure_pair(CellItem::Body(b), CellItem::Body(a)).is_none());
    assert!(s.measure_item(CellItem::Body(b)).is_none());
    assert!(s.item_geometry(CellItem::Body(b)).is_none());
    // Undo brings it back and it measures again.
    assert!(s.undo());
    assert!(s.measure_pair(CellItem::Body(a), CellItem::Body(b)).is_some());
}

/// A component measured against itself: distance 0, angle 0, no centre offset.
#[test]
fn an_item_measured_against_itself_is_zero() {
    let mut s = Session::new();
    let a = body(&mut s, 0.3);
    let pair = s.measure_pair(CellItem::Body(a), CellItem::Body(a)).unwrap();
    assert_eq!(pair.minimum.unwrap().distance, 0.0);
    assert_eq!(pair.centre_to_centre, 0.0);
    assert_eq!(pair.delta, [0.0; 3]);
    assert!(pair.angle.unwrap().abs() < 1e-12);
}

/// Hidden or suppressed bodies can still be measured (the user may pick them in the browser): the numbers
/// do not depend on whether the body is drawn.
#[test]
fn measure_does_not_depend_on_visibility() {
    let mut s = Session::new();
    let a = body(&mut s, 0.0);
    let b = body(&mut s, 0.5);
    let shown = s.measure_pair(CellItem::Body(a), CellItem::Body(b)).unwrap();
    s.set_component(item(b), ComponentFlag::Visible, false).unwrap();
    let hidden = s.measure_pair(CellItem::Body(a), CellItem::Body(b)).unwrap();
    assert_eq!(shown.minimum.unwrap().distance, hidden.minimum.unwrap().distance);
}

/// Edges-and-faces measuring with degenerate input: a zero-length edge, equal points, a face with no
/// area or a zero normal, huge and non-finite coordinates. No panic; finite input gives finite output.
#[test]
fn feature_measuring_survives_degenerate_geometry() {
    let p = Feature::Point([1.0, 2.0, 3.0]);
    let dot_edge = Feature::Edge([1.0, 2.0, 3.0], [1.0, 2.0, 3.0]);
    let edge = Feature::Edge([0.0; 3], [1.0, 0.0, 0.0]);
    let flat = Feature::Face { point: [0.0; 3], normal: [0.0; 3], area: 0.0 };
    let face = Feature::Face { point: [0.0; 3], normal: [0.0, 0.0, 1.0], area: 1.0 };
    let all = [p.clone(), dot_edge.clone(), edge.clone(), flat.clone(), face.clone()];
    for a in &all {
        for b in &all {
            let r = measure_features(a, b);
            for value in [r.distance, r.angle].into_iter().flatten() {
                assert!(value.is_finite(), "{a:?} vs {b:?}: {r:?}");
            }
            if let Some((x, y)) = r.closest {
                assert!(x.iter().chain(&y).all(|v| v.is_finite()), "{a:?} vs {b:?}: {r:?}");
            }
        }
    }
    // The same point twice: distance 0 and delta 0.
    let same = measure_features(&p, &p);
    assert_eq!(same.distance, Some(0.0));
    assert_eq!(same.delta, Some([0.0; 3]));
    // A point on the edge and on the face: distance 0.
    assert!(measure_features(&Feature::Point([0.5, 0.0, 0.0]), &edge).distance.unwrap().abs() < 1e-12);
    assert!(measure_features(&Feature::Point([4.0, 5.0, 0.0]), &face).distance.unwrap().abs() < 1e-12);
    // A point beyond the end of an edge measures to the end, not to the infinite line.
    let beyond = measure_features(&Feature::Point([3.0, 4.0, 0.0]), &edge);
    assert!((beyond.distance.unwrap() - (2.0_f64.powi(2) + 16.0).sqrt()).abs() < 1e-12, "{beyond:?}");
    // Non-finite input: no panic (the answer may be anything).
    let nan = Feature::Point([f64::NAN, 0.0, 0.0]);
    let inf = Feature::Edge([0.0; 3], [f64::INFINITY, 0.0, 0.0]);
    for a in [&nan, &inf] {
        for b in &all {
            std::hint::black_box(measure_features(a, b));
            std::hint::black_box(measure_features(b, a));
        }
    }
}

// ---- interference ----

/// Faces that touch exactly have no volume to share: whatever is reported, it is not a real overlap.
#[test]
fn boxes_that_only_touch_have_no_interference_volume() {
    let mut s = Session::new();
    let a = CellItem::Body(body(&mut s, 0.0));
    let b = CellItem::Body(body(&mut s, 0.2)); // face to face at x = 0.1
    let c = CellItem::Body(body(&mut s, 0.2 + 1e-9)); // a hair apart
    for rows in [s.interference(&[a, b], &[]), s.interference(&[a], &[c])] {
        for row in rows {
            assert!(row.overlap.volume < 1e-9, "touching bodies report a volume of {}", row.overlap.volume);
        }
    }
}

/// Identical bodies at the same place overlap completely; one inside another overlaps by the inner volume.
#[test]
fn full_and_nested_overlap_have_the_right_volume() {
    let mut s = Session::new();
    let a = CellItem::Body(body(&mut s, 0.0));
    let twin = CellItem::Body(body(&mut s, 0.0));
    let rows = s.interference(&[a, twin], &[]);
    assert_eq!(rows.len(), 1);
    assert!((rows[0].overlap.volume - 0.008).abs() < 1e-6, "{}", rows[0].overlap.volume);
    let inner = CellItem::Body(s.add_body(Solid::Box { size: [0.05; 3] }, at(0.01)).unwrap());
    let rows = s.interference(&[a], &[inner]);
    assert_eq!(rows.len(), 1);
    assert!((rows[0].overlap.volume - 0.05_f64.powi(3)).abs() < 1e-6, "{}", rows[0].overlap.volume);
}

/// Empty sets, deleted items and the same item named twice never produce a row of an item with itself.
#[test]
fn interference_ignores_empty_sets_gone_items_and_self_pairs() {
    let mut s = Session::new();
    let a = CellItem::Body(body(&mut s, 0.0));
    let b = CellItem::Body(body(&mut s, 0.1));
    assert!(s.interference(&[], &[]).is_empty());
    assert!(s.interference(&[], &[a]).is_empty());
    assert!(s.interference(&[a], &[a]).is_empty(), "a body does not interfere with itself");
    assert!(s.interference(&[a], &[a, a]).is_empty());
    let CellItem::Body(id) = b else { panic!() };
    s.delete_item(CellItem::Body(id)).unwrap();
    assert!(s.interference(&[a, b], &[]).is_empty(), "a deleted component is left out");
    assert!(s.interference(&[a], &[b]).is_empty());
}

/// The same component listed twice in one set (an agent or a script can do this; the picker cannot) is
/// paired with itself, and a body "interferes" with itself over its whole volume.
#[test]
fn a_component_listed_twice_does_not_interfere_with_itself() {
    let mut s = Session::new();
    let a = CellItem::Body(body(&mut s, 0.0));
    let b = CellItem::Body(body(&mut s, 0.5));
    for row in s.interference(&[a, a, b], &[]) {
        assert_ne!(row.first, row.second, "an item paired with itself");
    }
}

// ---- measure: cylindrical faces (Inventor: diameter, radius, centre to centre) ----

fn shaft(centre: [f64; 3], axis: [f64; 3], radius: f64) -> Feature {
    Feature::Cylinder { centre, axis, radius, length: 1.0 }
}

/// Cylinder against a point, an edge, a flat face and another cylinder: the surface gap is the centre
/// distance less the radii, never negative; the centre-to-centre value is the distance to the axis.
#[test]
fn cylinder_measures_give_the_surface_gap_and_the_axis_distance() {
    let up = [0.0, 0.0, 1.0];
    let a = shaft([0.0; 3], up, 0.05);
    // Another shaft 0.3 m away, radius 0.02: gap 0.3 - 0.05 - 0.02, axis distance 0.3, parallel.
    let b = shaft([0.3, 0.0, 0.0], up, 0.02);
    let pair = measure_features(&a, &b);
    assert!((pair.distance.unwrap() - 0.23).abs() < 1e-12, "{pair:?}");
    assert!((pair.centre_distance.unwrap() - 0.3).abs() < 1e-12);
    assert!(pair.angle.unwrap().abs() < 1e-9, "parallel axes");
    // Swapped: the same numbers, the nearest points swapped.
    let swapped = measure_features(&b, &a);
    assert!((swapped.distance.unwrap() - pair.distance.unwrap()).abs() < 1e-12);
    let (p, q) = pair.closest.unwrap();
    let (sq, sp) = swapped.closest.unwrap();
    assert!((v3(p) - v3(sp)).length() < 1e-12 && (v3(q) - v3(sq)).length() < 1e-12, "nearest points swap");
    // Touching and overlapping shafts: the gap is 0, never negative.
    for x in [0.07, 0.05, 0.0] {
        let near = measure_features(&a, &shaft([x, 0.0, 0.0], up, 0.02));
        assert!(near.distance.unwrap().abs() < 1e-12, "x = {x}");
    }
    // Crossing axes at right angles: angle 90 degrees.
    let cross = measure_features(&a, &shaft([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], 0.01));
    assert!((cross.angle.unwrap() - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    // A point on the axis: radius away from the surface; a point on the surface: 0.
    let on_axis = measure_features(&a, &Feature::Point([0.0, 0.0, 0.2]));
    assert!(
        (on_axis.distance.unwrap() - 0.05).abs() < 1e-12 && on_axis.centre_distance.unwrap().abs() < 1e-12
    );
    let on_surface = measure_features(&a, &Feature::Point([0.05, 0.0, 0.2]));
    assert!(on_surface.distance.unwrap().abs() < 1e-12);
    // Beyond the end of the shaft the nearest point is on the end of the axis.
    let beyond = measure_features(&a, &Feature::Point([0.0, 0.0, 2.0]));
    assert!((beyond.centre_distance.unwrap() - 1.5).abs() < 1e-12, "{beyond:?}");
}

/// The nearest points reported must be as far apart as the distance (the overlay draws a line between them).
#[test]
fn cylinder_closest_points_agree_with_the_distance() {
    let up = [0.0, 0.0, 1.0];
    let a = shaft([0.0; 3], up, 0.05);
    let others = [
        shaft([0.3, 0.0, 0.0], up, 0.02),
        Feature::Edge([0.3, -1.0, 0.0], [0.3, 1.0, 0.0]),
        Feature::Face { point: [0.3, 0.0, 0.0], normal: [1.0, 0.0, 0.0], area: 1.0 },
        Feature::Point([0.3, 0.0, 0.0]),
    ];
    for other in &others {
        let pair = measure_features(&a, other);
        let (p, q) = pair.closest.unwrap();
        let apart = (v3(p) - v3(q)).length();
        assert!(
            (apart - pair.distance.unwrap()).abs() < 1e-9,
            "{other:?}: points {apart} vs distance {pair:?}"
        );
    }
}

fn v3(p: [f64; 3]) -> DVec3 {
    DVec3::from_array(p)
}

/// Zero radius, zero length, no axis direction and NaN: no panic, and finite numbers for finite input.
#[test]
fn degenerate_cylinders_do_not_break_measuring() {
    let others = [
        Feature::Point([0.1, 0.2, 0.3]),
        Feature::Edge([0.0; 3], [1.0, 0.0, 0.0]),
        Feature::Face { point: [0.0; 3], normal: [0.0, 0.0, 1.0], area: 1.0 },
        shaft([0.5; 3], [0.0, 0.0, 1.0], 0.1),
    ];
    let cylinders = [
        Feature::Cylinder { centre: [0.0; 3], axis: [0.0, 0.0, 1.0], radius: 0.0, length: 1.0 },
        Feature::Cylinder { centre: [0.0; 3], axis: [0.0, 0.0, 1.0], radius: 0.05, length: 0.0 },
        Feature::Cylinder { centre: [0.0; 3], axis: [0.0; 3], radius: 0.05, length: 1.0 },
        Feature::Cylinder { centre: [0.0; 3], axis: [0.0, 0.0, 1.0], radius: f64::NAN, length: 1.0 },
        Feature::Cylinder {
            centre: [f64::NAN; 3],
            axis: [0.0, 0.0, 1.0],
            radius: 0.05,
            length: f64::INFINITY,
        },
    ];
    for (k, c) in cylinders.iter().enumerate() {
        for o in &others {
            let (a, b) = (measure_features(c, o), measure_features(o, c));
            if k < 3 {
                for value in [a.distance, a.centre_distance, a.angle, b.distance].into_iter().flatten() {
                    assert!(value.is_finite(), "{c:?} vs {o:?}: {a:?}");
                }
            }
        }
    }
}
