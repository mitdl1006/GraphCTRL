//! Shared, de-duplicated mesh storage.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

use gc_math::Aabb;

use crate::{MeshBvh, MeshData, MeshError};

/// Identifies a mesh by its content hash. The same file imported twice yields the same key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MeshKey(pub u64);

/// A stored mesh with data derived from it.
#[derive(Debug)]
pub struct MeshAsset {
    /// The triangles.
    pub data: MeshData,
    /// Bounding box in the mesh frame.
    pub aabb: Aabb,
    bvh: OnceLock<MeshBvh>,
}

impl MeshAsset {
    /// The ray-casting hierarchy, built on first use and then shared.
    pub fn bvh(&self) -> &MeshBvh {
        self.bvh.get_or_init(|| MeshBvh::build(&self.data))
    }
}

/// All meshes of a session. Meshes are immutable and handed out as `Arc`.
#[derive(Clone, Debug, Default)]
pub struct AssetStore {
    meshes: BTreeMap<MeshKey, Arc<MeshAsset>>,
}

impl AssetStore {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a mesh and returns its key. Adding a mesh that is already stored returns the existing key.
    pub fn insert(&mut self, data: MeshData) -> Result<MeshKey, MeshError> {
        let key = MeshKey(data.content_hash());
        match self.meshes.get(&key) {
            Some(existing) if existing.data == data => Ok(key),
            Some(_) => Err(MeshError::HashCollision(key.0)),
            None => {
                let aabb = data.aabb();
                self.meshes.insert(key, Arc::new(MeshAsset { data, aabb, bvh: OnceLock::new() }));
                Ok(key)
            }
        }
    }

    /// The mesh stored under `key`.
    pub fn get(&self, key: MeshKey) -> Option<&Arc<MeshAsset>> {
        self.meshes.get(&key)
    }

    /// Number of stored meshes.
    pub fn len(&self) -> usize {
        self.meshes.len()
    }

    /// True when the store holds no mesh.
    pub fn is_empty(&self) -> bool {
        self.meshes.is_empty()
    }

    /// The keys of all stored meshes, ascending.
    pub fn keys(&self) -> impl Iterator<Item = MeshKey> + '_ {
        self.meshes.keys().copied()
    }

    /// Drops every mesh whose key is not in `used` and returns how many were dropped.
    pub fn retain(&mut self, used: &BTreeSet<MeshKey>) -> usize {
        let before = self.meshes.len();
        self.meshes.retain(|key, _| used.contains(key));
        before - self.meshes.len()
    }
}

#[cfg(test)]
mod tests {
    use gc_math::{DVec3, Ray};

    use super::*;
    use crate::test_util::cube;

    #[test]
    fn identical_meshes_are_stored_once() {
        let mut store = AssetStore::new();
        let a = store.insert(cube([0.0; 3], 1.0)).unwrap();
        let b = store.insert(cube([0.0; 3], 1.0)).unwrap();
        let c = store.insert(cube([0.0; 3], 2.0)).unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(store.len(), 2);
        assert_eq!(store.get(a).unwrap().aabb.max, DVec3::ONE);
        assert!(store.get(MeshKey(0)).is_none());
    }

    #[test]
    fn the_bvh_is_built_once_and_shared() {
        let mut store = AssetStore::new();
        let key = store.insert(cube([0.0; 3], 1.0)).unwrap();
        let asset = Arc::clone(store.get(key).unwrap());
        assert!(std::ptr::eq(asset.bvh(), store.get(key).unwrap().bvh()));
        let ray = Ray::new(DVec3::new(0.5, 0.5, 3.0), -DVec3::Z).unwrap();
        assert!(asset.bvh().raycast(&ray, f64::INFINITY).is_some());
    }

    #[test]
    fn retain_drops_unused_meshes() {
        let mut store = AssetStore::new();
        let keep = store.insert(cube([0.0; 3], 1.0)).unwrap();
        store.insert(cube([0.0; 3], 2.0)).unwrap();
        assert_eq!(store.retain(&BTreeSet::from([keep])), 1);
        assert_eq!(store.keys().collect::<Vec<_>>(), vec![keep]);
        assert!(!store.is_empty());
    }
}
