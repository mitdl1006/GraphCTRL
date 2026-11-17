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

/// A cylindrical region of a mesh, in the mesh's frame.
#[derive(Clone, Debug, PartialEq)]
pub struct CylinderRegion {
    /// The triangles of the region.
    pub triangles: Vec<u32>,
    /// A point of the axis: the middle of the region along it.
    pub centre: DVec3,
    /// The unit axis direction.
    pub axis: DVec3,
    /// The radius.
    pub radius: f64,
    /// The length of the region along the axis.
    pub length: f64,
}

/// The cylindrical surface around `triangle`, when there is one: triangles reached across edges where
/// the surface bends by less than `bend` (radians) are fitted with an axis (the direction all their
/// normals are square to) and a circle; the fit must hold within `tolerance` (relative to the radius).
/// `None` for flat or irregular surfaces.
pub fn cylindrical_region(
    mesh: &MeshData,
    triangle: u32,
    bend: f64,
    tolerance: f64,
) -> Option<CylinderRegion> {
    let positions = mesh.positions();
    let indices = mesh.indices();
    let count = indices.len() / 3;
    let corners = |t: usize| -> Option<[[f32; 3]; 3]> {
        let at = |k: usize| indices.get(3 * t + k).and_then(|i| positions.get(*i as usize)).copied();
        Some([at(0)?, at(1)?, at(2)?])
    };
    let normal_of = |t: usize| {
        corners(t).and_then(|c| {
            let (a, b, c) = (corner(c[0]), corner(c[1]), corner(c[2]));
            (b - a).cross(c - a).try_normalize()
        })
    };
    let first = normal_of(triangle as usize)?;
    let mut by_edge: BTreeMap<(Key, Key), Vec<usize>> = BTreeMap::new();
    for t in 0..count {
        let Some(c) = corners(t) else { continue };
        for (a, b) in [(c[0], c[1]), (c[1], c[2]), (c[2], c[0])] {
            let (ka, kb) = (key(a), key(b));
            by_edge.entry((ka.min(kb), ka.max(kb))).or_default().push(t);
        }
    }
    let cos_bend = gc_math::fm::cos(bend);
    let mut region = BTreeSet::from([triangle as usize]);
    let mut todo = vec![triangle as usize];
    while let Some(t) = todo.pop() {
        let (Some(c), Some(n)) = (corners(t), normal_of(t)) else { continue };
        for (a, b) in [(c[0], c[1]), (c[1], c[2]), (c[2], c[0])] {
            let (ka, kb) = (key(a), key(b));
            for &next in by_edge.get(&(ka.min(kb), ka.max(kb))).map_or(&[][..], Vec::as_slice) {
                if !region.contains(&next) && normal_of(next).is_some_and(|m| m.dot(n) >= cos_bend) {
                    region.insert(next);
                    todo.push(next);
                }
            }
        }
    }
    let normals: Vec<DVec3> = region.iter().filter_map(|t| normal_of(*t)).collect();
    // The axis is square to every normal: sum the cross products with the first normal, each turned to
    // agree with the sum so far.
    let mut sum = DVec3::ZERO;
    for n in &normals {
        let c = first.cross(*n);
        sum += if c.dot(sum) < 0.0 { -c } else { c };
    }
    let axis = sum.try_normalize()?;
    if normals.iter().any(|n| n.dot(axis).abs() > tolerance.max(0.02)) {
        return None;
    }
    // Fit a circle to the corners projected on the plane square to the axis (Kasa's algebraic fit).
    let u = axis.any_orthonormal_vector();
    let v = axis.cross(u);
    let points: Vec<DVec3> = region.iter().filter_map(|t| corners(*t)).flat_map(|c| c.map(corner)).collect();
    let flat: Vec<(f64, f64)> = points.iter().map(|p| (p.dot(u), p.dot(v))).collect();
    let (mut sxx, mut sxy, mut syy, mut sx, mut sy, mut sz, mut sxz, mut syz) =
        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    #[allow(clippy::cast_precision_loss, reason = "corner counts are small")]
    let n = flat.len() as f64;
    for (x, y) in &flat {
        let z = x * x + y * y;
        sxx += x * x;
        sxy += x * y;
        syy += y * y;
        sx += x;
        sy += y;
        sz += z;
        sxz += x * z;
        syz += y * z;
    }
    // Solve [sxx sxy sx; sxy syy sy; sx sy n] [a b c]ᵀ = [sxz syz sz]ᵀ; centre (a/2, b/2).
    let det3 = |m: [[f64; 3]; 3]| {
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    };
    let m = [[sxx, sxy, sx], [sxy, syy, sy], [sx, sy, n]];
    let d = det3(m);
    if d.abs() < 1e-30 {
        return None;
    }
    let rhs = [sxz, syz, sz];
    let with = |col: usize| {
        let mut k = m;
        for row in 0..3 {
            k[row][col] = rhs[row];
        }
        det3(k) / d
    };
    let (a, b, c) = (with(0), with(1), with(2));
    let (cx, cy) = (a / 2.0, b / 2.0);
    let radius = (c + cx * cx + cy * cy).sqrt();
    if !(radius.is_finite() && radius > 0.0) {
        return None;
    }
    if flat
        .iter()
        .any(|(x, y)| (((x - cx).powi(2) + (y - cy).powi(2)).sqrt() - radius).abs() > tolerance * radius)
    {
        return None;
    }
    let along: Vec<f64> = points.iter().map(|p| p.dot(axis)).collect();
    let low = along.iter().copied().fold(f64::INFINITY, f64::min);
    let high = along.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let centre = u * cx + v * cy + axis * ((low + high) / 2.0);
    #[allow(clippy::cast_possible_truncation, reason = "triangle indices fit u32")]
    let triangles = region.into_iter().map(|t| t as u32).collect();
    Some(CylinderRegion { triangles, centre, axis, radius, length: high - low })
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

    #[test]
    fn a_cylinder_side_is_recognised_with_its_axis_and_radius() {
        let cylinder = crate::primitives::cylinder(0.05, 0.3, 48).unwrap();
        // Find a side triangle: its normal is square to Z.
        let side = (0..cylinder.triangle_count())
            .find(|t| {
                let c: Vec<DVec3> = cylinder.triangles().nth(*t).unwrap().to_vec();
                (c[1] - c[0]).cross(c[2] - c[0]).normalize().z.abs() < 1e-6
            })
            .unwrap();
        #[allow(clippy::cast_possible_truncation, reason = "test mesh")]
        let found = cylindrical_region(&cylinder, side as u32, 20f64.to_radians(), 0.01).unwrap();
        assert!(found.axis.z.abs() > 1.0 - 1e-9, "{:?}", found.axis);
        assert!((found.radius - 0.05).abs() < 1e-4, "{}", found.radius);
        assert!((found.length - 0.3).abs() < 1e-6);
        assert!(found.centre.truncate().length() < 1e-6);
        // A box face is not a cylinder.
        let cube = crate::test_util::cube([0.0; 3], 1.0);
        assert!(cylindrical_region(&cube, 0, 20f64.to_radians(), 0.01).is_none());
    }
}
