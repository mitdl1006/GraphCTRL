//! Mesh data and the operations that clean it up after import.

use std::collections::BTreeMap;

use gc_core::StableHasher;
use gc_math::{Aabb, DVec3};

/// Why vertex and index data do not form a mesh.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MeshError {
    /// The index list does not describe whole triangles.
    #[error("{0} indices do not form whole triangles")]
    IndexCount(usize),
    /// A triangle refers to a vertex that does not exist.
    #[error("triangle {triangle} refers to vertex {index}, but the mesh has {vertices} vertices")]
    IndexOutOfRange {
        /// Triangle number.
        triangle: usize,
        /// The offending index.
        index: u32,
        /// Number of vertices.
        vertices: usize,
    },
    /// A coordinate is NaN or infinite.
    #[error("vertex {0} has a coordinate that is not a finite number")]
    NonFinite(usize),
    /// There are normals, but not one per vertex.
    #[error("{normals} normals for {vertices} vertices")]
    NormalCount {
        /// Number of normals supplied.
        normals: usize,
        /// Number of vertices.
        vertices: usize,
    },
    /// More vertices than a 32-bit index can address.
    #[error("the mesh has more vertices than a 32-bit index can address")]
    TooManyVertices,
    /// Two different meshes have the same content hash, so the store cannot tell them apart.
    #[error("two different meshes share the content hash {0:#018x}")]
    HashCollision(u64),
}

/// An indexed triangle mesh. Always valid: every index is in range and every coordinate is finite.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshData {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    indices: Vec<u32>,
}

fn to_dvec3(p: [f32; 3]) -> DVec3 {
    DVec3::new(f64::from(p[0]), f64::from(p[1]), f64::from(p[2]))
}

fn index_u32(index: usize) -> Result<u32, MeshError> {
    u32::try_from(index).map_err(|_overflow| MeshError::TooManyVertices)
}

impl MeshData {
    /// Builds a mesh without vertex normals after validating it.
    pub fn new(positions: Vec<[f32; 3]>, indices: Vec<u32>) -> Result<Self, MeshError> {
        index_u32(positions.len())?;
        if !indices.len().is_multiple_of(3) {
            return Err(MeshError::IndexCount(indices.len()));
        }
        if let Some(vertex) = positions.iter().position(|p| p.iter().any(|c| !c.is_finite())) {
            return Err(MeshError::NonFinite(vertex));
        }
        if let Some(at) = indices.iter().position(|i| *i as usize >= positions.len()) {
            return Err(MeshError::IndexOutOfRange {
                triangle: at / 3,
                index: indices[at],
                vertices: positions.len(),
            });
        }
        Ok(Self { positions, normals: Vec::new(), indices })
    }

    /// Attaches one normal per vertex (as read from a file; they are not checked for unit length).
    pub fn with_normals(mut self, normals: Vec<[f32; 3]>) -> Result<Self, MeshError> {
        if normals.len() != self.positions.len() {
            return Err(MeshError::NormalCount { normals: normals.len(), vertices: self.positions.len() });
        }
        if let Some(vertex) = normals.iter().position(|n| n.iter().any(|c| !c.is_finite())) {
            return Err(MeshError::NonFinite(vertex));
        }
        self.normals = normals;
        Ok(self)
    }

    /// Vertex positions.
    pub fn positions(&self) -> &[[f32; 3]] {
        &self.positions
    }

    /// Vertex normals: one per vertex, or empty when the mesh has none.
    pub fn normals(&self) -> &[[f32; 3]] {
        &self.normals
    }

    /// Three vertex indices per triangle.
    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    /// Number of vertices.
    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    /// Number of triangles.
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// The corners of every triangle.
    pub fn triangles(&self) -> impl Iterator<Item = [DVec3; 3]> + '_ {
        self.indices
            .chunks_exact(3)
            .map(|t| [t[0], t[1], t[2]].map(|index| to_dvec3(self.positions[index as usize])))
    }

    /// The bounding box of all vertices.
    pub fn aabb(&self) -> Aabb {
        Aabb::from_points(self.positions.iter().map(|p| to_dvec3(*p)))
    }

    /// A hash of the mesh content that is the same on every platform and in every run.
    pub fn content_hash(&self) -> u64 {
        let mut hasher = StableHasher::new();
        let mut write_points = |points: &[[f32; 3]]| {
            hasher.write_u64(points.len() as u64);
            for coordinate in points.iter().flatten() {
                // `+ 0.0` turns -0.0 into 0.0 so that equal meshes hash equally.
                hasher.write_bytes(&(coordinate + 0.0).to_bits().to_le_bytes());
            }
        };
        write_points(&self.positions);
        write_points(&self.normals);
        hasher.write_u64(self.indices.len() as u64);
        for index in &self.indices {
            hasher.write_bytes(&index.to_le_bytes());
        }
        hasher.finish()
    }

    /// The mesh with every position multiplied by `factor` (e.g. 0.001 for a file in millimetres).
    /// Returns an error when a scaled coordinate is no longer finite.
    pub fn scaled(&self, factor: f32) -> Result<Self, MeshError> {
        let positions = self.positions.iter().map(|p| p.map(|c| c * factor)).collect();
        let mut scaled = Self::new(positions, self.indices.clone())?;
        scaled.normals.clone_from(&self.normals);
        Ok(scaled)
    }

    /// Merges vertices that fall into the same cell of a grid with spacing `tolerance`, then drops
    /// triangles that collapsed and vertices nothing uses. With `tolerance <= 0` only identical positions
    /// merge. Normals are dropped because merged vertices no longer have a single one.
    ///
    /// File formats such as STL store every triangle corner separately; welding restores the connectivity
    /// that [`MeshData::connected_components`] and collision shapes rely on. Two vertices closer than the
    /// tolerance can stay separate when a cell border runs between them.
    pub fn welded(&self, tolerance: f64) -> Self {
        let cell_of = |p: [f32; 3]| -> [i64; 3] {
            p.map(|c| {
                if tolerance > 0.0 {
                    // Saturating float-to-int conversion; coordinates are finite by construction.
                    #[allow(clippy::cast_possible_truncation, reason = "saturating conversion is intended")]
                    let cell = (f64::from(c) / tolerance).round() as i64;
                    cell
                } else {
                    i64::from((c + 0.0).to_bits())
                }
            })
        };
        let mut first_in_cell: BTreeMap<[i64; 3], u32> = BTreeMap::new();
        let mut positions: Vec<[f32; 3]> = Vec::new();
        let mut remap = Vec::with_capacity(self.positions.len());
        for position in &self.positions {
            let next = u32::try_from(positions.len()).unwrap_or(u32::MAX);
            let index = *first_in_cell.entry(cell_of(*position)).or_insert_with(|| {
                positions.push(*position);
                next
            });
            remap.push(index);
        }
        let mut indices = Vec::with_capacity(self.indices.len());
        for triangle in self.indices.chunks_exact(3) {
            let [a, b, c] = [triangle[0], triangle[1], triangle[2]].map(|i| remap[i as usize]);
            let corner = |i: u32| to_dvec3(positions[i as usize]);
            let area_vector = (corner(b) - corner(a)).cross(corner(c) - corner(a));
            if a != b && b != c && a != c && area_vector.length_squared() > 0.0 {
                indices.extend([a, b, c]);
            }
        }
        Self { positions, normals: Vec::new(), indices }.compacted()
    }

    /// The mesh without vertices that no triangle uses. The remaining vertices keep their order.
    fn compacted(&self) -> Self {
        let mut used = vec![false; self.positions.len()];
        for index in &self.indices {
            used[*index as usize] = true;
        }
        let mut new_index = vec![0_u32; self.positions.len()];
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        for (old, _) in used.iter().enumerate().filter(|(_, used)| **used) {
            new_index[old] = u32::try_from(positions.len()).unwrap_or(u32::MAX);
            positions.push(self.positions[old]);
            if let Some(normal) = self.normals.get(old) {
                normals.push(*normal);
            }
        }
        let indices = self.indices.iter().map(|old| new_index[*old as usize]).collect();
        Self { positions, normals, indices }
    }

    /// Splits the mesh into parts whose triangles are connected through shared vertices, ordered by the
    /// first triangle of each part. Weld the mesh first if its triangles do not share vertices.
    pub fn connected_components(&self) -> Vec<Self> {
        let mut parent: Vec<u32> = (0..u32::try_from(self.positions.len()).unwrap_or(u32::MAX)).collect();
        fn find(parent: &mut [u32], mut node: u32) -> u32 {
            while parent[node as usize] != node {
                let grandparent = parent[parent[node as usize] as usize];
                parent[node as usize] = grandparent;
                node = grandparent;
            }
            node
        }
        for triangle in self.indices.chunks_exact(3) {
            let root = find(&mut parent, triangle[0]);
            for other in &triangle[1..] {
                let other_root = find(&mut parent, *other);
                parent[other_root.max(root) as usize] = other_root.min(root);
            }
        }
        let mut part_of_root: BTreeMap<u32, usize> = BTreeMap::new();
        let mut parts: Vec<Vec<u32>> = Vec::new();
        for triangle in self.indices.chunks_exact(3) {
            let root = find(&mut parent, triangle[0]);
            let part = *part_of_root.entry(root).or_insert_with(|| {
                parts.push(Vec::new());
                parts.len() - 1
            });
            parts[part].extend_from_slice(triangle);
        }
        parts
            .into_iter()
            .map(|indices| {
                Self { positions: self.positions.clone(), normals: self.normals.clone(), indices }.compacted()
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{cube, merged, soup};

    #[test]
    fn validation_rejects_broken_input() {
        let p = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        assert_eq!(MeshData::new(p.clone(), vec![0, 1]), Err(MeshError::IndexCount(2)));
        assert_eq!(
            MeshData::new(p.clone(), vec![0, 1, 2, 0, 1, 3]),
            Err(MeshError::IndexOutOfRange { triangle: 1, index: 3, vertices: 3 })
        );
        let mut bad = p.clone();
        bad[1][2] = f32::NAN;
        assert_eq!(MeshData::new(bad, vec![0, 1, 2]), Err(MeshError::NonFinite(1)));
        let mesh = MeshData::new(p, vec![0, 1, 2]).unwrap();
        assert_eq!(
            mesh.clone().with_normals(vec![[0.0, 0.0, 1.0]]),
            Err(MeshError::NormalCount { normals: 1, vertices: 3 })
        );
        assert!(mesh.with_normals(vec![[0.0, 0.0, 1.0]; 3]).is_ok());
        assert_eq!(MeshData::new(Vec::new(), Vec::new()).unwrap().triangle_count(), 0);
    }

    #[test]
    fn counts_bounds_and_triangles() {
        let mesh = cube([1.0, 2.0, 3.0], 2.0);
        assert_eq!((mesh.vertex_count(), mesh.triangle_count()), (8, 12));
        let aabb = mesh.aabb();
        assert_eq!(aabb.min, DVec3::new(1.0, 2.0, 3.0));
        assert_eq!(aabb.max, DVec3::new(3.0, 4.0, 5.0));
        // Outward winding: the signed volume is positive and equals the cube's volume.
        let volume: f64 = mesh.triangles().map(|[a, b, c]| a.dot(b.cross(c)) / 6.0).sum();
        assert!((volume - 8.0).abs() < 1e-9);
    }

    #[test]
    fn content_hash_depends_on_content_only() {
        let a = cube([0.0; 3], 1.0);
        assert_eq!(a.content_hash(), cube([0.0; 3], 1.0).content_hash());
        assert_eq!(a.content_hash(), cube([-0.0; 3], 1.0).content_hash(), "-0.0 and 0.0 are the same point");
        assert_ne!(a.content_hash(), cube([0.0; 3], 1.000_001).content_hash());
        assert_ne!(a.content_hash(), soup(&a).content_hash());
        assert_ne!(
            a.content_hash(),
            a.clone().with_normals(vec![[0.0, 0.0, 1.0]; 8]).unwrap().content_hash()
        );
    }

    #[test]
    fn scaling_converts_units_and_rejects_overflow() {
        let mm = cube([0.0; 3], 1000.0);
        let m = mm.scaled(0.001).unwrap();
        assert!((m.aabb().max - DVec3::ONE).length() < 1e-6);
        assert!(matches!(mm.scaled(f32::MAX), Err(MeshError::NonFinite(_))));
    }

    #[test]
    fn welding_restores_shared_vertices() {
        let original = cube([0.0; 3], 1.0);
        let stl_like = soup(&original);
        assert_eq!(stl_like.vertex_count(), 36);
        let welded = stl_like.welded(0.0);
        assert_eq!((welded.vertex_count(), welded.triangle_count()), (8, 12));
        assert_eq!(welded.aabb(), original.aabb());
    }

    #[test]
    fn welding_with_a_tolerance_merges_near_duplicates_and_drops_collapsed_triangles() {
        let positions = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 0.000_01, 0.0], // within 0.1 mm of vertex 1
            [5.0, 5.0, 5.0],      // unused
        ];
        let mesh = MeshData::new(positions, vec![0, 1, 2, 1, 3, 2]).unwrap();
        let welded = mesh.welded(1e-4);
        assert_eq!((welded.vertex_count(), welded.triangle_count()), (3, 1));
        assert_eq!(mesh.welded(0.0).triangle_count(), 2, "without a tolerance nothing merges");
    }

    #[test]
    fn connected_components_split_separate_bodies() {
        let near = cube([0.0; 3], 1.0);
        let far = cube([5.0, 0.0, 0.0], 2.0);
        let both = merged(&near, &far);
        let parts = both.connected_components();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0], near);
        assert_eq!(parts[1], far);
        // An unwelded triangle soup has one component per triangle; welding fixes that.
        assert_eq!(soup(&both).connected_components().len(), 24);
        assert_eq!(soup(&both).welded(0.0).connected_components().len(), 2);
        assert!(MeshData::new(Vec::new(), Vec::new()).unwrap().connected_components().is_empty());
    }
}
