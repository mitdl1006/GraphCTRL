//! Assembly files (glTF/GLB, STEP AP242 with tessellation) read by the mesh plugin and handed to the
//! session in the document's form: as Inventor's Place command brings in a component from a file.

use std::path::Path;
use std::sync::Arc;

use gc_session::{AssemblyReader, Imported, Importer};

/// True for the file types read as assemblies.
pub(crate) fn is_assembly(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "glb" | "gltf" | "stp" | "step"))
}

/// Reads an assembly file with the plugin that knows its type.
fn read(file: &str) -> Result<gc_plugin_mesh::Assembly, String> {
    let path = Path::new(file);
    let bytes = std::fs::read(path).map_err(|e| format!("{file}: {e}"))?;
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match extension.as_str() {
        "stp" | "step" => gc_plugin_mesh::read_step(&bytes),
        _ => gc_plugin_mesh::read_gltf(&bytes, path.parent()),
    }
    .map_err(|e| format!("{file}: {e}"))
}

/// The plugin's assembly in the document's form, with its meshes.
fn convert(file: &str, read: gc_plugin_mesh::Assembly) -> Result<Imported, String> {
    let name = Path::new(file).file_stem().and_then(|s| s.to_str()).unwrap_or("Assembly").to_owned();
    #[allow(clippy::cast_possible_truncation, reason = "part and mesh counts fit u32")]
    let parts = read
        .nodes
        .iter()
        .map(|node| gc_doc::AssemblyPart {
            name: node.name.clone(),
            parent: node.parent.map(|p| p as u32),
            matrix: node.matrix,
            meshes: node.meshes.iter().map(|m| *m as u32).collect(),
        })
        .collect();
    let mut meshes = Vec::with_capacity(read.meshes.len());
    let mut described = Vec::with_capacity(read.meshes.len());
    for mesh in read.meshes {
        #[allow(clippy::cast_possible_truncation, reason = "triangle counts fit u32")]
        let triangles = (mesh.indices.len() / 3) as u32;
        described.push(gc_doc::AssemblyMesh { name: mesh.name.clone(), color: mesh.color, triangles });
        meshes
            .push(gc_mesh::MeshData::new(mesh.positions, mesh.indices).map_err(|e| format!("{file}: {e}"))?);
    }
    let assembly = gc_doc::Assembly {
        name,
        file: file.to_owned(),
        placement: gc_doc::Placement::default(),
        parts,
        meshes: described,
        look: gc_doc::Look::default(),
    };
    Ok((assembly, meshes))
}

/// Reads a new assembly for [`gc_session::Session::import_assembly`].
pub(crate) fn importer() -> Importer {
    Arc::new(|file: &str| convert(file, read(file)?))
}

/// Reads an assembly's meshes again when a project is opened.
pub(crate) fn reader() -> AssemblyReader {
    Arc::new(|file: &str| convert(file, read(file)?).map(|(_, meshes)| meshes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_step_fixture_reads_as_an_assembly() {
        let file = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../fixtures/step/two_blocks_ap242.stp");
        let (assembly, meshes) = importer()(file).expect("the fixture reads");
        assert!(assembly.is_valid());
        assert!(!meshes.is_empty());
        assert_eq!(assembly.meshes.len(), meshes.len());
        assert!(is_assembly(Path::new(file)));
        assert!(!is_assembly(Path::new("part.stl")));
    }
}
