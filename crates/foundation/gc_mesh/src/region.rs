//! The flat face of a mesh around a triangle: what a CAD program selects as a "face" when the model is a
//! triangle mesh. Neighbouring triangles join while they lie in the same plane; the outline is the
//! region's edges that no other region triangle shares.

use std::collections::{BTreeMap, BTreeSet};

use gc_math::DVec3;

use crate::MeshData;

/// A flat region of a mesh, in the mesh's frame.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanarRegion {
    /// The triangles of the region.
    pub triangles: Vec<u32>,
    /// The unit normal (by the winding of the first triangle).
    pub normal: DVec3,
    /// The centroid of the region (a point of its plane).
    pub point: DVec3,
    /// The area.
    pub area: f64,
    /// The outline: edges with only one region triangle, as pairs of corner positions.
    pub outline: Vec<[DVec3; 2]>,
}

type Key = [i64; 3];
/// An edge's two corners and how many region triangles use it.
type Use = ([f32; 3], [f32; 3], u32);

fn key(p: [f32; 3]) -> Key {
    #[allow(clippy::cast_possible_truncation, reason = "micrometre cells of a robot cell fit i64")]
    p.map(|c| (f64::from(c) * 1e6).round() as i64)
}

fn corner(p: [f32; 3]) -> DVec3 {
    DVec3::new(f64::from(p[0]), f64::from(p[1]), f64::from(p[2]))
}

/// The flat region around `triangle`: triangles reached across shared edges whose normals are within
/// `angle` (radians) of the first one and whose corners lie within `distance` of its plane. Corners
/// closer than a micrometre count as shared, so meshes that repeat corners per triangle work too.
/// `None` when the triangle does not exist or is degenerate.
pub fn planar_region(mesh: &MeshData, triangle: u32, angle: f64, distance: f64) -> Option<PlanarRegion> {
    let positions = mesh.positions();
    let indices = mesh.indices();
    let count = indices.len() / 3;
    let corners = |t: usize| -> Option<[[f32; 3]; 3]> {
        let at = |k: usize| indices.get(3 * t + k).and_then(|i| positions.get(*i as usize)).copied();
        Some([at(0)?, at(1)?, at(2)?])
    };
    let normal_of = |c: &[[f32; 3]; 3]| {
        let (a, b, c) = (corner(c[0]), corner(c[1]), corner(c[2]));
        (b - a).cross(c - a)
    };
    let first = corners(triangle as usize)?;
    let normal = normal_of(&first).try_normalize()?;
    let point = corner(first[0]);
    let cos_limit = gc_math::fm::cos(angle);

    // Which triangles share each edge (by corner keys, the smaller key first).
    let mut by_edge: BTreeMap<(Key, Key), Vec<usize>> = BTreeMap::new();
    for t in 0..count {
        let Some(c) = corners(t) else { continue };
        for (a, b) in [(c[0], c[1]), (c[1], c[2]), (c[2], c[0])] {
            let (ka, kb) = (key(a), key(b));
            by_edge.entry((ka.min(kb), ka.max(kb))).or_default().push(t);
        }
    }
    let joins = |t: usize| -> bool {
        corners(t).is_some_and(|c| {
            normal_of(&c).try_normalize().is_some_and(|n| n.dot(normal) >= cos_limit)
                && c.iter().all(|p| (corner(*p) - point).dot(normal).abs() <= distance)
        })
    };
    let mut region = BTreeSet::from([triangle as usize]);
    let mut todo = vec![triangle as usize];
    while let Some(t) = todo.pop() {
        let Some(c) = corners(t) else { continue };
        for (a, b) in [(c[0], c[1]), (c[1], c[2]), (c[2], c[0])] {
            let (ka, kb) = (key(a), key(b));
            for &next in by_edge.get(&(ka.min(kb), ka.max(kb))).map_or(&[][..], Vec::as_slice) {
                if !region.contains(&next) && joins(next) {
                    region.insert(next);
                    todo.push(next);
                }
            }
        }
    }
    let mut area = 0.0;
    let mut weighted = DVec3::ZERO;
    let mut uses: BTreeMap<(Key, Key), Use> = BTreeMap::new();
    for &t in &region {
        let Some(c) = corners(t) else { continue };
        let piece = normal_of(&c).length() / 2.0;
        area += piece;
        weighted += (corner(c[0]) + corner(c[1]) + corner(c[2])) * (piece / 3.0);
        for (a, b) in [(c[0], c[1]), (c[1], c[2]), (c[2], c[0])] {
            let (ka, kb) = (key(a), key(b));
            uses.entry((ka.min(kb), ka.max(kb))).or_insert((a, b, 0)).2 += 1;
        }
    }
    let outline =
        uses.values().filter(|(_, _, n)| *n == 1).map(|(a, b, _)| [corner(*a), corner(*b)]).collect();
    #[allow(clippy::cast_possible_truncation, reason = "triangle indices fit u32")]
    let triangles = region.into_iter().map(|t| t as u32).collect();
    let centroid = if area > 0.0 { weighted / area } else { point };
    Some(PlanarRegion { triangles, normal, point: centroid, area, outline })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cube_face_is_two_triangles_with_four_outline_edges() {
        let cube = crate::test_util::cube([0.0; 3], 2.0);
        let region = planar_region(&cube, 0, 0.01, 1e-6).unwrap();
        assert_eq!(region.triangles.len(), 2);
        assert!((region.area - 4.0).abs() < 1e-9);
        assert_eq!(region.outline.len(), 4);
        // The centroid is the middle of the face.
        let middle = region.point;
        assert!(region.normal.dot(middle - DVec3::new(1.0, 1.0, 1.0)).abs() > 0.99, "{middle:?}");
        let soup = crate::test_util::soup(&cube);
        let again = planar_region(&soup, 0, 0.01, 1e-6).unwrap();
        assert_eq!(again.triangles.len(), 2);
        assert!(planar_region(&cube, 999, 0.01, 1e-6).is_none());
    }
}
