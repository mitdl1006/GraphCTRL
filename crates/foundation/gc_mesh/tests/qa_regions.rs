//! QA edge cases for picking "faces" of a triangle mesh: flat regions and cylindrical regions (the Edges and
//! Faces priority of Measure: Inventor 2026 help, To Measure Distance, Length, Angle, Loop, or Area:
//! <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-1805AB8F-F782-4B24-A332-B4F2E10A3D00.htm>).

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use gc_mesh::MeshData;
use gc_mesh::primitives::{cuboid, cylinder, sphere};
use gc_mesh::region::{cylindrical_region, planar_region};

const BEND: f64 = 0.3;
const TOL: f64 = 0.02;

fn triangles(mesh: &MeshData) -> u32 {
    u32::try_from(mesh.indices().len() / 3).unwrap()
}

/// A box has six flat faces of two triangles each; every triangle finds its whole face and nothing else.
#[test]
fn every_triangle_of_a_box_finds_exactly_its_face() {
    let mesh = cuboid([0.2, 0.1, 0.05]).unwrap();
    let mut areas = Vec::new();
    for t in 0..triangles(&mesh) {
        let r = planar_region(&mesh, t, 0.05, 1e-6).unwrap();
        assert_eq!(r.triangles.len(), 2, "triangle {t}");
        assert!(r.triangles.contains(&t));
        assert_eq!(r.outline.len(), 4, "a rectangle has four outline edges");
        assert!((r.normal.length() - 1.0).abs() < 1e-9);
        areas.push(r.area);
    }
    areas.sort_by(f64::total_cmp);
    // Two faces of each size, each found from both of its triangles (single-precision corners: 1e-7 is what the mesh holds).
    for (pair, want) in areas.chunks(4).zip([0.1 * 0.05, 0.2 * 0.05, 0.2 * 0.1]) {
        assert!(pair.iter().all(|a| (a - want).abs() < 1e-7), "{pair:?} vs {want}");
    }
}

/// Triangles that do not exist, an empty mesh, a degenerate triangle: nothing, not a panic.
#[test]
fn missing_and_degenerate_triangles_give_no_region() {
    let mesh = cuboid([0.2; 3]).unwrap();
    assert!(planar_region(&mesh, triangles(&mesh), 0.05, 1e-6).is_none());
    assert!(planar_region(&mesh, u32::MAX, 0.05, 1e-6).is_none());
    assert!(cylindrical_region(&mesh, u32::MAX, BEND, TOL).is_none());
    // Three equal corners: no area, no normal.
    let flat = MeshData::new(vec![[0.0; 3], [0.0; 3], [0.0; 3]], vec![0, 1, 2]).unwrap();
    assert!(planar_region(&flat, 0, 0.05, 1e-6).is_none());
    assert!(cylindrical_region(&flat, 0, BEND, TOL).is_none());
    // Three points on a line.
    let line = MeshData::new(vec![[0.0; 3], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]], vec![0, 1, 2]).unwrap();
    assert!(planar_region(&line, 0, 0.05, 1e-6).is_none());
}

/// Silly tolerances (zero, negative, NaN, huge) never panic; zero tolerance still returns the triangle's own
/// region at least, and a huge one stays within the mesh.
#[test]
fn silly_tolerances_do_not_panic() {
    let mesh = cuboid([0.2; 3]).unwrap();
    let all = triangles(&mesh) as usize;
    for angle in [0.0, -1.0, f64::NAN, f64::INFINITY, 10.0] {
        for distance in [0.0, -1.0, f64::NAN, f64::INFINITY, 1e300] {
            if let Some(r) = planar_region(&mesh, 0, angle, distance) {
                assert!(!r.triangles.is_empty() && r.triangles.len() <= all, "{angle} {distance}");
                assert!(r.triangles.contains(&0) || angle.is_nan() || distance.is_nan());
            }
        }
    }
    for bend in [0.0, -1.0, f64::NAN, f64::INFINITY, 10.0] {
        for tol in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            drop(cylindrical_region(&mesh, 0, bend, tol));
            let c = cylinder(0.05, 0.4, 24).unwrap();
            drop(cylindrical_region(&c, 0, bend, tol));
        }
    }
}

/// A triangle soup that repeats its corners per triangle (many STL files) still joins into faces.
#[test]
fn a_soup_with_repeated_corners_still_forms_faces() {
    let mesh = cuboid([0.2; 3]).unwrap();
    let positions = mesh.positions();
    let mut soup_positions = Vec::new();
    let mut soup_indices = Vec::new();
    for tri in mesh.indices().chunks(3) {
        for &i in tri {
            soup_indices.push(u32::try_from(soup_positions.len()).unwrap());
            soup_positions.push(positions[i as usize]);
        }
    }
    let soup = MeshData::new(soup_positions, soup_indices).unwrap();
    let r = planar_region(&soup, 0, 0.05, 1e-6).unwrap();
    assert_eq!(r.triangles.len(), 2);
    assert_eq!(r.outline.len(), 4);
}

/// The side of a cylinder along Z: radius, axis and length come back (polygonal sides: radius a little
/// under the true one, within 1 %); the end caps are flat, not cylindrical; a box has no cylinder.
#[test]
fn a_cylinder_side_is_fitted_and_caps_and_boxes_are_not() {
    let (radius, length) = (0.05, 0.4);
    let mesh = cylinder(radius, length, 48).unwrap();
    let mut found = 0;
    let mut flat_only = 0;
    for t in 0..triangles(&mesh) {
        match cylindrical_region(&mesh, t, BEND, TOL) {
            Some(c) => {
                found += 1;
                assert!(c.axis.z.abs() > 0.999, "axis {:?}", c.axis);
                assert!((c.radius - radius).abs() < 0.01 * radius, "radius {}", c.radius);
                assert!((c.length - length).abs() < 1e-6, "length {}", c.length);
                assert!(c.centre.x.abs() < 1e-3 && c.centre.y.abs() < 1e-3 && c.centre.z.abs() < 1e-3);
            }
            None => flat_only += 1,
        }
    }
    assert_eq!(found, 2 * 48, "all side triangles find the same cylinder");
    assert_eq!(flat_only, 2 * 48, "all cap triangles are flat, not a cylinder");
    let cap = (0..triangles(&mesh)).find(|t| cylindrical_region(&mesh, *t, BEND, TOL).is_none()).unwrap();
    assert!(planar_region(&mesh, cap, 0.05, 1e-6).is_some());
    let cube = cuboid([0.2; 3]).unwrap();
    assert!((0..triangles(&cube)).all(|t| cylindrical_region(&cube, t, BEND, TOL).is_none()));
}

/// Coarse cylinders (few sides) and a sphere: no panic; a sphere has no axis (no cylinder).
#[test]
fn coarse_cylinders_and_spheres_do_not_crash_the_fit() {
    for sides in [3, 4, 6, 8] {
        let mesh = cylinder(0.05, 0.4, sides).unwrap();
        for t in 0..triangles(&mesh) {
            if let Some(c) = cylindrical_region(&mesh, t, BEND, TOL) {
                assert!(c.radius.is_finite() && c.radius > 0.0 && c.length.is_finite(), "{sides} sides");
            }
        }
    }
    let ball = sphere(0.1, 24).unwrap();
    let cylinders =
        (0..triangles(&ball)).filter(|t| cylindrical_region(&ball, *t, BEND, TOL).is_some()).count();
    assert_eq!(cylinders, 0, "a sphere is not a cylinder");
}

/// Non-finite and huge coordinates do not crash the region search.
#[test]
fn non_finite_coordinates_do_not_crash() {
    let bad = MeshData::new(
        vec![[0.0, 0.0, 0.0], [f32::NAN, 0.0, 0.0], [0.0, f32::INFINITY, 0.0], [1e30, 1e30, 1e30]],
        vec![0, 1, 2, 0, 2, 3, 0, 1, 3],
    );
    if let Ok(mesh) = bad {
        for t in 0..3 {
            drop(planar_region(&mesh, t, 0.05, 1e-6));
            drop(cylindrical_region(&mesh, t, BEND, TOL));
        }
    }
}
