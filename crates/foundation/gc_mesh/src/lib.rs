//! Triangle meshes: data, validation, content hashing, a shared asset store and ray casting.
//!
//! Meshes are immutable once built and shared by `Arc`, so the renderer, the collision world and picking
//! all look at the same memory. Positions are metres in the mesh's own frame.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::float_cmp, reason = "tests compare exactly representable values"))]

mod bvh;
mod data;
pub mod primitives;
pub mod stl;
mod store;

pub use bvh::{MeshBvh, RayHit};
pub use data::{MeshData, MeshError};
pub use store::{AssetStore, MeshAsset, MeshKey};

#[cfg(test)]
pub(crate) mod test_util {
    use crate::MeshData;

    /// An axis-aligned cube with corners `min` and `min + size`, 8 vertices, 12 outward-facing triangles.
    pub fn cube(min: [f32; 3], size: f32) -> MeshData {
        let p = |x: f32, y: f32, z: f32| [min[0] + x * size, min[1] + y * size, min[2] + z * size];
        let positions = vec![
            p(0.0, 0.0, 0.0),
            p(1.0, 0.0, 0.0),
            p(1.0, 1.0, 0.0),
            p(0.0, 1.0, 0.0),
            p(0.0, 0.0, 1.0),
            p(1.0, 0.0, 1.0),
            p(1.0, 1.0, 1.0),
            p(0.0, 1.0, 1.0),
        ];
        #[rustfmt::skip]
        let indices = vec![
            0, 2, 1, 0, 3, 2, // bottom (-Z)
            4, 5, 6, 4, 6, 7, // top (+Z)
            0, 1, 5, 0, 5, 4, // front (-Y)
            2, 3, 7, 2, 7, 6, // back (+Y)
            1, 2, 6, 1, 6, 5, // right (+X)
            3, 0, 4, 3, 4, 7, // left (-X)
        ];
        MeshData::new(positions, indices).unwrap()
    }

    /// The same triangles with every corner as its own vertex, as an STL file stores them.
    pub fn soup(mesh: &MeshData) -> MeshData {
        let positions: Vec<[f32; 3]> = mesh.indices().iter().map(|i| mesh.positions()[*i as usize]).collect();
        let indices = (0..u32::try_from(positions.len()).unwrap()).collect();
        MeshData::new(positions, indices).unwrap()
    }

    /// Two meshes in one.
    pub fn merged(a: &MeshData, b: &MeshData) -> MeshData {
        let offset = u32::try_from(a.vertex_count()).unwrap();
        let positions = a.positions().iter().chain(b.positions()).copied().collect();
        let indices = a.indices().iter().copied().chain(b.indices().iter().map(|i| i + offset)).collect();
        MeshData::new(positions, indices).unwrap()
    }
}
