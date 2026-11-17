//! QA edge cases for Project Cut Edges (assembly parity P1-6): where a plane cuts a mesh. Inventor 2026 help,
//! To Project Geometry into a 2D Sketch (Project Cut Edges):
//! <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-B8D8B07B-86F2-4CB7-AB21-3131D94AD1CC.htm>

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use gc_math::{DPose3, DQuat, DVec3};
use gc_mesh::primitives::{cuboid, cylinder};
use gc_mesh::slice::{join_straight, plane_cut};

fn length(edges: &[[DVec3; 2]]) -> f64 {
    edges.iter().map(|[a, b]| a.distance(*b)).sum()
}

/// A plane through the middle of a box gives its outline, wherever the box stands and however it is turned;
/// the plane tilted 45 degrees gives a longer rectangle.
#[test]
fn a_box_cut_through_the_middle_gives_its_outline_in_any_pose() {
    let mesh = cuboid([0.4, 0.2, 0.1]).unwrap();
    for pose in [
        DPose3::IDENTITY,
        DPose3::from_translation(DVec3::new(3.0, -2.0, 1.0)),
        DPose3::from_parts(DVec3::new(0.0, 1.0, 0.0), DQuat::from_rotation_z(0.7)),
    ] {
        let at = pose.translation + pose.rotation * DVec3::new(0.0, 0.0, 0.01);
        let n = pose.rotation * DVec3::Z;
        let cut = join_straight(&plane_cut(&mesh, &pose, at, n), 1e-6);
        assert_eq!(cut.len(), 4, "{cut:?}");
        assert!((length(&cut) - 2.0 * (0.4 + 0.2)).abs() < 1e-6, "perimeter {}", length(&cut));
        for [a, b] in &cut {
            assert!(((*a - at).dot(n)).abs() < 1e-6 && ((*b - at).dot(n)).abs() < 1e-6, "on the plane");
        }
    }
}

/// A plane that misses, that only touches an edge or corner, or lies in a face gives nothing usable.
#[test]
fn missing_touching_and_coplanar_planes_give_no_area_outline() {
    let mesh = cuboid([0.2; 3]).unwrap();
    let id = DPose3::IDENTITY;
    assert!(plane_cut(&mesh, &id, DVec3::new(0.0, 0.0, 1.0), DVec3::Z).is_empty(), "misses");
    assert!(plane_cut(&mesh, &id, DVec3::new(0.0, 0.0, -1.0), DVec3::Z).is_empty(), "misses below");
    // Through a face: no stray segments, and not the face's outline twice either.
    let face = join_straight(&plane_cut(&mesh, &id, DVec3::new(0.0, 0.0, 0.1), DVec3::Z), 1e-6);
    assert!(face.is_empty() || length(&face) <= 0.8 + 1e-6, "{face:?}");
    // Through a corner only: nothing longer than a point.
    let corner = join_straight(
        &plane_cut(&mesh, &id, DVec3::new(0.1, 0.1, 0.1), DVec3::new(1.0, 1.0, 1.0).normalize()),
        1e-6,
    );
    assert!(length(&corner) < 1e-6, "{corner:?}");
}

/// Unusable planes and meshes: a zero or non-finite normal, a non-unit normal, an empty mesh.
#[test]
fn unusable_planes_do_not_panic_and_a_long_normal_still_works() {
    let mesh = cuboid([0.2; 3]).unwrap();
    let id = DPose3::IDENTITY;
    for normal in [DVec3::ZERO, DVec3::new(f64::NAN, 0.0, 1.0), DVec3::new(0.0, 0.0, f64::INFINITY)] {
        let cut = plane_cut(&mesh, &id, DVec3::ZERO, normal);
        assert!(cut.iter().all(|[a, b]| a.is_finite() && b.is_finite()), "{normal:?}");
    }
    for point in [DVec3::new(f64::NAN, 0.0, 0.0), DVec3::splat(f64::INFINITY)] {
        std::hint::black_box(plane_cut(&mesh, &id, point, DVec3::Z));
    }
    // A normal of length 5 is the same plane (the doc says unit; a caller's slip must not give a wrong cut).
    let unit = join_straight(&plane_cut(&mesh, &id, DVec3::new(0.0, 0.0, 0.02), DVec3::Z), 1e-6);
    let long =
        join_straight(&plane_cut(&mesh, &id, DVec3::new(0.0, 0.0, 0.02), DVec3::new(0.0, 0.0, 5.0)), 1e-6);
    assert_eq!(unit.len(), long.len(), "{unit:?} vs {long:?}");
    let empty = gc_mesh::MeshData::new(vec![], vec![]);
    if let Ok(empty) = empty {
        assert!(plane_cut(&empty, &id, DVec3::ZERO, DVec3::Z).is_empty());
    }
}

/// A cylinder cut across its axis is a circle (many short segments, none joined into a straight edge across
/// it); cut along its axis it is a rectangle of 2r by the length.
#[test]
fn a_cylinder_cut_across_and_along() {
    let mesh = cylinder(0.05, 0.4, 32).unwrap();
    let id = DPose3::IDENTITY;
    let across = join_straight(&plane_cut(&mesh, &id, DVec3::new(0.0, 0.0, 0.1), DVec3::Z), 1e-6);
    assert!(
        (length(&across) - 32.0 * 2.0 * 0.05 * gc_math::fm::sin(std::f64::consts::PI / 32.0)).abs() < 1e-4,
        "{}",
        length(&across)
    );
    assert!(across.len() >= 32, "a polygon's sides are not merged into longer lines: {}", across.len());
    let along = join_straight(&plane_cut(&mesh, &id, DVec3::ZERO, DVec3::Y), 1e-4);
    let long_sides = along.iter().filter(|[a, b]| a.distance(*b) > 0.39).count();
    assert_eq!(long_sides, 2, "two lines along the axis: {along:?}");
}

/// Joining: duplicates drop out, collinear neighbours merge, a bad angle tolerance does not panic.
#[test]
fn joining_segments_is_idempotent_and_safe() {
    let line = |x0: f64, x1: f64| [DVec3::new(x0, 0.0, 0.0), DVec3::new(x1, 0.0, 0.0)];
    let parts = [line(0.0, 1.0), line(1.0, 2.0), line(1.0, 2.0), line(2.0, 3.0), [DVec3::ONE, DVec3::ONE]];
    let joined = join_straight(&parts, 1e-6);
    assert_eq!(joined.len(), 1, "{joined:?}");
    assert!((length(&joined) - 3.0).abs() < 1e-9);
    assert_eq!(join_straight(&joined, 1e-6), joined, "joining again changes nothing");
    assert!(join_straight(&[], 1e-6).is_empty());
    for angle in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        std::hint::black_box(join_straight(&parts, angle));
    }
}
