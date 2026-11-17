//! Imported assemblies (plan 9.1): their meshes, how they are drawn, and their parts as obstacles.
//!
//! The session cannot read CAD files itself (readers live in plugins, which the domain layer does not
//! use). The application gives it a reader with [`crate::Session::set_assembly_reader`], and hands it
//! freshly imported assemblies with [`crate::Session::add_assembly`].

use std::collections::BTreeMap;
use std::sync::Arc;

use gc_collide::ShapeDesc;
use gc_core::Id;
use gc_doc::{Assembly, Document};
use gc_math::{DMat3, DPose3, DQuat, DVec3};
use gc_mesh::MeshData;
use gc_motion::Obstacle;

use crate::cell::MeshRef;

/// Reads an assembly file again (when a project is opened): the meshes in the order of
/// [`gc_doc::Assembly::meshes`]. The part tree comes from the document.
pub type AssemblyReader = Arc<dyn Fn(&str) -> Result<Vec<MeshData>, String> + Send + Sync>;

/// A freshly imported assembly: the document's part tree and the meshes, in the order of
/// [`gc_doc::Assembly::meshes`].
pub type Imported = (Assembly, Vec<MeshData>);

/// Reads an assembly file for [`crate::Session::import_assembly`] (a plugin's reader turned into the
/// document's form).
pub type Importer = Arc<dyn Fn(&str) -> Result<Imported, String> + Send + Sync>;

/// One mesh of one part of an assembly, placed in the world, ready to draw.
#[derive(Clone, Debug)]
pub struct ShownAssemblyPart {
    /// The part (index into [`gc_doc::Assembly::parts`]).
    pub part: usize,
    /// The part's frame in the world, column-major 4 × 4 (may contain scale).
    pub matrix: [[f64; 4]; 4],
    /// The mesh.
    pub mesh: MeshRef,
    /// Its colour from the file.
    pub color: [f32; 4],
}

/// The meshes of every imported file, by file path.
#[derive(Default)]
pub(crate) struct AssemblyMeshes {
    pub(crate) by_file: BTreeMap<String, Result<Arc<Vec<MeshRef>>, String>>,
    pub(crate) reader: Option<AssemblyReader>,
}

impl AssemblyMeshes {
    /// Stores the meshes of a file.
    pub(crate) fn insert(&mut self, file: &str, meshes: Vec<MeshData>) {
        let refs = meshes.into_iter().map(|m| mesh_ref(Arc::new(m))).collect();
        self.by_file.insert(file.to_owned(), Ok(Arc::new(refs)));
    }

    /// Reads the files of the document's assemblies that are not loaded yet (with the reader, if any).
    pub(crate) fn sync(&mut self, doc: &Document) {
        let Some(reader) = self.reader.clone() else { return };
        for (_, assembly) in doc.assemblies().iter() {
            if !self.by_file.contains_key(&assembly.file) {
                let read = reader(&assembly.file).and_then(|meshes| {
                    if meshes.len() == assembly.meshes.len() {
                        Ok(Arc::new(meshes.into_iter().map(|m| mesh_ref(Arc::new(m))).collect()))
                    } else {
                        Err(format!("{}: the file has changed since it was imported", assembly.file))
                    }
                });
                self.by_file.insert(assembly.file.clone(), read);
            }
        }
    }

    /// The loaded meshes of an assembly, or why there are none.
    pub(crate) fn of(&self, assembly: &Assembly) -> Result<&Arc<Vec<MeshRef>>, String> {
        match self.by_file.get(&assembly.file) {
            Some(Ok(meshes)) => Ok(meshes),
            Some(Err(reason)) => Err(reason.clone()),
            None => Err(format!("{}: not loaded", assembly.file)),
        }
    }
}

fn mesh_ref(data: Arc<MeshData>) -> MeshRef {
    let bounds = data.aabb();
    let radius = bounds.min.abs().max(bounds.max.abs()).length();
    MeshRef { key: data.content_hash(), radius, bounds, data }
}

/// Column-major 4 × 4 of a placement.
fn placement_matrix(pose: &DPose3) -> [[f64; 4]; 4] {
    let m = DMat3::from_quat(pose.rotation);
    let t = pose.translation;
    [
        [m.x_axis.x, m.x_axis.y, m.x_axis.z, 0.0],
        [m.y_axis.x, m.y_axis.y, m.y_axis.z, 0.0],
        [m.z_axis.x, m.z_axis.y, m.z_axis.z, 0.0],
        [t.x, t.y, t.z, 1.0],
    ]
}

fn multiply(a: &[[f64; 4]; 4], b: &[[f64; 4]; 4]) -> [[f64; 4]; 4] {
    let mut out = [[0.0; 4]; 4];
    for (column, out_column) in out.iter_mut().enumerate() {
        for (row, value) in out_column.iter_mut().enumerate() {
            *value = (0..4).map(|k| a[k][row] * b[column][k]).sum();
        }
    }
    out
}

/// The parts of an assembly placed at `pose`, one entry per mesh shown.
pub(crate) fn shown_parts(assembly: &Assembly, pose: &DPose3, meshes: &[MeshRef]) -> Vec<ShownAssemblyPart> {
    let origin = placement_matrix(pose);
    let mut shown = Vec::new();
    for (index, part) in assembly.parts.iter().enumerate() {
        let Some(local) = assembly.part_matrix(index) else { continue };
        let matrix = multiply(&origin, &local);
        for mesh in &part.meshes {
            let (Some(data), Some(info)) = (meshes.get(*mesh as usize), assembly.meshes.get(*mesh as usize))
            else {
                continue;
            };
            shown.push(ShownAssemblyPart { part: index, matrix, mesh: data.clone(), color: info.color });
        }
    }
    shown
}

/// The rigid pose of a matrix whose 3 × 3 part is a rotation (no scale, no shear); `None` otherwise.
fn rigid(matrix: &[[f64; 4]; 4]) -> Option<DPose3> {
    let axes = [0, 1, 2].map(|c| DVec3::new(matrix[c][0], matrix[c][1], matrix[c][2]));
    let unit = axes.iter().all(|a| (a.length() - 1.0).abs() < 1e-6);
    let square = axes[0].dot(axes[1]).abs() < 1e-6
        && axes[0].dot(axes[2]).abs() < 1e-6
        && axes[1].dot(axes[2]).abs() < 1e-6;
    let right_handed = axes[0].cross(axes[1]).dot(axes[2]) > 0.0;
    (unit && square && right_handed).then(|| {
        let rotation = DQuat::from_mat3(&DMat3::from_cols(axes[0], axes[1], axes[2])).normalize();
        DPose3::from_parts(DVec3::new(matrix[3][0], matrix[3][1], matrix[3][2]), rotation)
    })
}

/// The obstacles of the document's active assemblies (suppressed and rolled-back ones are left out).
/// Fails when an active assembly's meshes are not loaded: planning must not ignore equipment.
pub(crate) fn obstacles(
    doc: &Document,
    meshes: &AssemblyMeshes,
    pose_of: &dyn Fn(&gc_doc::Placement) -> DPose3,
) -> Result<Vec<Obstacle>, String> {
    let mut out = Vec::new();
    for (id, assembly) in active(doc) {
        let loaded = meshes.of(assembly)?;
        for shown in shown_parts(assembly, &pose_of(&assembly.placement), loaded) {
            let name = format!("{}/{}", assembly.name, assembly.parts[shown.part].name);
            let (pose, mesh) = match rigid(&shown.matrix) {
                Some(pose) => (pose, Arc::clone(&shown.mesh.data)),
                None => {
                    // Scaled or sheared: the matrix goes into the vertices.
                    let m = shown.matrix;
                    #[allow(clippy::cast_possible_truncation, reason = "mesh coordinates are f32")]
                    let positions = shown
                        .mesh
                        .data
                        .positions()
                        .iter()
                        .map(|p| {
                            let p = [f64::from(p[0]), f64::from(p[1]), f64::from(p[2])];
                            [0, 1, 2]
                                .map(|r| (m[0][r] * p[0] + m[1][r] * p[1] + m[2][r] * p[2] + m[3][r]) as f32)
                        })
                        .collect();
                    let baked = MeshData::new(positions, shown.mesh.data.indices().to_vec())
                        .map_err(|e| format!("{name} ({id:?}): {e}"))?;
                    (DPose3::IDENTITY, Arc::new(baked))
                }
            };
            out.push(Obstacle { name, pose, shape: ShapeDesc::Mesh(mesh) });
        }
    }
    Ok(out)
}

/// The assemblies that are computed: not suppressed, not after the rollback marker.
pub(crate) fn active(doc: &Document) -> impl Iterator<Item = (Id<Assembly>, &Assembly)> {
    doc.assemblies().iter().filter(|(id, _)| !doc.is_inactive(id.erase()))
}
