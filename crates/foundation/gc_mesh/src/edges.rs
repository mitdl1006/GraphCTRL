//! The edges a CAD program draws as lines: where the surface folds by more than an angle, and where it
//! ends.

use std::collections::BTreeMap;

use crate::MeshData;

/// A position rounded to micrometres.
type Key = [i64; 3];
/// An edge's vertex indices and the normals of the faces that share it.
type Seen = ([u32; 2], Vec<gc_math::DVec3>);

/// The feature edges of a mesh as pairs of vertex indices: edges between two triangles whose normals
/// differ by more than `angle` (radians), and edges with only one triangle. Positions closer than a
/// micrometre count as one, so meshes that repeat corners per triangle give the same edges.
pub fn feature_edges(mesh: &MeshData, angle: f64) -> Vec<[u32; 2]> {
    let positions = mesh.positions();
    // Vertices that share a position share a key.
    let key = |index: u32| {
        let p = positions.get(index as usize).copied().unwrap_or_default();
        #[allow(clippy::cast_possible_truncation, reason = "micrometre cells of a robot cell fit i64")]
        p.map(|c| (f64::from(c) * 1e6).round() as i64)
    };
    let normal = |t: &[u32]| {
        let corner = |i: u32| {
            let p = positions.get(i as usize).copied().unwrap_or_default();
            gc_math::DVec3::new(f64::from(p[0]), f64::from(p[1]), f64::from(p[2]))
        };
        let (a, b, c) = (corner(t[0]), corner(t[1]), corner(t[2]));
        (b - a).cross(c - a).try_normalize()
    };
    // Each undirected edge: one of its vertex pairs, and the normals of the triangles on it.
    let mut edges: BTreeMap<(Key, Key), Seen> = BTreeMap::new();
    for triangle in mesh.indices().chunks_exact(3) {
        let Some(n) = normal(triangle) else { continue };
        for (a, b) in [(triangle[0], triangle[1]), (triangle[1], triangle[2]), (triangle[2], triangle[0])] {
            let (ka, kb) = (key(a), key(b));
            if ka == kb {
                continue;
            }
            let id = if ka < kb { (ka, kb) } else { (kb, ka) };
            edges.entry(id).or_insert_with(|| ([a, b], Vec::new())).1.push(n);
        }
    }
    let cosine = gc_math::fm::cos(angle);
    edges
        .into_values()
        .filter(|(_, normals)| match normals.as_slice() {
            [a, b] => a.dot(*b) < cosine,
            // The surface ends here, or more than two faces meet: both are edges to show.
            _ => true,
        })
        .map(|(pair, _)| pair)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    #[test]
    fn a_box_has_twelve_edges_a_sphere_none_and_a_cylinder_its_rims() {
        let angle = 30.0_f64.to_radians();
        assert_eq!(feature_edges(&primitives::cuboid([1.0, 2.0, 3.0]).unwrap(), angle).len(), 12);
        assert_eq!(feature_edges(&primitives::sphere(1.0, 32).unwrap(), angle).len(), 0);
        // 24 sides: the two rims of 24 edges each; the seams between the sides are smooth.
        assert_eq!(feature_edges(&primitives::cylinder(1.0, 2.0, 24).unwrap(), angle).len(), 48);
        // Corners repeated per triangle, as in STL files, give the same edges.
        let cube = primitives::cuboid([1.0; 3]).unwrap();
        let soup: Vec<[f32; 3]> = cube.indices().iter().map(|i| cube.positions()[*i as usize]).collect();
        let count = u32::try_from(soup.len()).unwrap();
        let soup = MeshData::new(soup, (0..count).collect()).unwrap();
        assert_eq!(feature_edges(&soup, angle).len(), 12);
        // A single triangle: all three sides are open edges.
        let one = MeshData::new(vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], vec![0, 1, 2]).unwrap();
        assert_eq!(feature_edges(&one, angle).len(), 3);
    }
}
