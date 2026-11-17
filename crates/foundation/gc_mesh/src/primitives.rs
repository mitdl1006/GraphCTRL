//! Meshes of the basic solids, centred on their own origin.

use std::f32::consts::{PI, TAU};

use crate::{MeshData, MeshError};

#[allow(clippy::cast_possible_truncation, reason = "mesh coordinates are f32")]
fn narrow(v: f64) -> f32 {
    v as f32
}

fn index(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX)
}

/// A box with edge lengths `size`.
pub fn cuboid(size: [f64; 3]) -> Result<MeshData, MeshError> {
    let h = size.map(|s| narrow(s * 0.5));
    let p = |x: f32, y: f32, z: f32| [x * h[0], y * h[1], z * h[2]];
    let positions = vec![
        p(-1.0, -1.0, -1.0),
        p(1.0, -1.0, -1.0),
        p(1.0, 1.0, -1.0),
        p(-1.0, 1.0, -1.0),
        p(-1.0, -1.0, 1.0),
        p(1.0, -1.0, 1.0),
        p(1.0, 1.0, 1.0),
        p(-1.0, 1.0, 1.0),
    ];
    #[rustfmt::skip]
    let indices = vec![
        0, 2, 1, 0, 3, 2,
        4, 5, 6, 4, 6, 7,
        0, 1, 5, 0, 5, 4,
        2, 3, 7, 2, 7, 6,
        1, 2, 6, 1, 6, 5,
        3, 0, 4, 3, 4, 7,
    ];
    MeshData::new(positions, indices)
}

/// A cylinder along Z with `segments` sides (at least 3).
pub fn cylinder(radius: f64, length: f64, segments: usize) -> Result<MeshData, MeshError> {
    let n = segments.max(3);
    let (r, h) = (narrow(radius), narrow(length * 0.5));
    let mut positions = Vec::with_capacity(2 * n + 2);
    for k in 0..n {
        let angle = TAU * k as f32 / n as f32;
        let (x, y) = (r * angle.cos(), r * angle.sin());
        positions.push([x, y, -h]);
        positions.push([x, y, h]);
    }
    let (bottom, top) = (index(2 * n), index(2 * n + 1));
    positions.push([0.0, 0.0, -h]);
    positions.push([0.0, 0.0, h]);
    let mut indices = Vec::with_capacity(12 * n);
    for k in 0..n {
        let (a, b) = (index(2 * k), index(2 * ((k + 1) % n)));
        indices.extend([a, b, b + 1, a, b + 1, a + 1]); // side
        indices.extend([bottom, b, a]);
        indices.extend([top, a + 1, b + 1]);
    }
    MeshData::new(positions, indices)
}

/// A sphere with `segments` steps around and half as many from pole to pole (at least 6).
pub fn sphere(radius: f64, segments: usize) -> Result<MeshData, MeshError> {
    let around = segments.max(6);
    let rings = around / 2;
    let r = narrow(radius);
    let mut positions = vec![[0.0, 0.0, -r]];
    for ring in 1..rings {
        let polar = PI * ring as f32 / rings as f32;
        let (z, ring_radius) = (-r * polar.cos(), r * polar.sin());
        for k in 0..around {
            let angle = TAU * k as f32 / around as f32;
            positions.push([ring_radius * angle.cos(), ring_radius * angle.sin(), z]);
        }
    }
    positions.push([0.0, 0.0, r]);
    let at = |ring: usize, k: usize| index(1 + (ring - 1) * around + k % around);
    let north = index(positions.len() - 1);
    let mut indices = Vec::new();
    for k in 0..around {
        indices.extend([0, at(1, k + 1), at(1, k)]);
        indices.extend([north, at(rings - 1, k), at(rings - 1, k + 1)]);
        for ring in 1..rings - 1 {
            let (a, b, c, d) = (at(ring, k), at(ring, k + 1), at(ring + 1, k + 1), at(ring + 1, k));
            indices.extend([a, b, c, a, c, d]);
        }
    }
    MeshData::new(positions, indices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gc_math::DVec3;

    /// Signed volume by the divergence theorem: positive when the triangles face outwards.
    fn volume(mesh: &MeshData) -> f64 {
        mesh.triangles().map(|[a, b, c]| a.dot(b.cross(c)) / 6.0).sum()
    }

    #[test]
    fn solids_are_closed_outward_facing_and_have_the_right_size() {
        let b = cuboid([0.2, 0.4, 0.6]).unwrap();
        assert!((volume(&b) - 0.048).abs() < 1e-7);
        assert_eq!(b.aabb().max, DVec3::new(f64::from(0.1_f32), f64::from(0.2_f32), f64::from(0.3_f32)));

        let c = cylinder(0.5, 2.0, 64).unwrap();
        let exact = std::f64::consts::PI * 0.25 * 2.0;
        assert!(volume(&c) > 0.99 * exact && volume(&c) < exact);
        assert!((c.aabb().max.z - 1.0).abs() < 1e-6 && (c.aabb().min.z + 1.0).abs() < 1e-6);

        let s = sphere(0.3, 48).unwrap();
        let exact = 4.0 / 3.0 * std::f64::consts::PI * 0.027;
        assert!(volume(&s) > 0.98 * exact && volume(&s) < exact);
        // Closed: every edge is used by exactly two triangles, once in each direction.
        for mesh in [&b, &c, &s] {
            let mut edges = std::collections::BTreeMap::new();
            for t in mesh.indices().chunks_exact(3) {
                for (i, j) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                    *edges.entry((i, j)).or_insert(0) += 1;
                }
            }
            assert!(edges.iter().all(|((i, j), n)| *n == 1 && edges.get(&(*j, *i)) == Some(&1)));
        }
    }
}
