//! Reads equipment files: glTF / GLB assemblies with their part tree, names, colours and instancing
//! (plan 9.1, CAD import).
//!
//! The result is plain data ([`Assembly`]): a tree of nodes with local transforms, and the triangle meshes
//! they show. A mesh used by several nodes is stored once (instancing). Coordinates are converted from
//! glTF's Y-up to the application's Z-up; units stay metres, as glTF defines them.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::path::Path;

/// An assembly read from a file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Assembly {
    /// The parts in depth-first order: a parent always comes before its children.
    pub nodes: Vec<AssemblyNode>,
    /// The triangle meshes the nodes show.
    pub meshes: Vec<AssemblyMesh>,
    /// What was skipped and why (points or lines, missing data, ...).
    pub warnings: Vec<String>,
}

/// One part of an assembly.
#[derive(Clone, Debug, PartialEq)]
pub struct AssemblyNode {
    /// Name from the file, or a made-up one ("Node 3") when it has none.
    pub name: String,
    /// The parent node; `None` for a root.
    pub parent: Option<usize>,
    /// The node's frame relative to its parent's (for a root: relative to the world, Z up), as a
    /// column-major 4 × 4 matrix. May contain scale.
    pub matrix: [[f64; 4]; 4],
    /// The meshes this node shows (indices into [`Assembly::meshes`]), in its own frame.
    pub meshes: Vec<usize>,
}

/// A triangle mesh with one colour.
#[derive(Clone, Debug, PartialEq)]
pub struct AssemblyMesh {
    /// Name of the mesh in the file (with the primitive number when it has several).
    pub name: String,
    /// Vertex positions in the mesh frame.
    pub positions: Vec<[f32; 3]>,
    /// Triangles: three vertex indices each.
    pub indices: Vec<u32>,
    /// Base colour, RGBA in 0..=1.
    pub color: [f32; 4],
}

impl Assembly {
    /// The world matrix of a node (the product of the matrices from its root down to it).
    pub fn world_matrix(&self, node: usize) -> [[f64; 4]; 4] {
        let mut chain = Vec::new();
        let mut at = Some(node);
        while let Some(index) = at.filter(|i| *i < self.nodes.len()) {
            chain.push(index);
            at = self.nodes[index].parent;
            if chain.len() > self.nodes.len() {
                break;
            }
        }
        chain.iter().rev().fold(IDENTITY, |acc, i| multiply(&acc, &self.nodes[*i].matrix))
    }

    /// Number of triangles over all node instances.
    pub fn instanced_triangles(&self) -> usize {
        self.nodes
            .iter()
            .flat_map(|n| &n.meshes)
            .map(|m| self.meshes.get(*m).map_or(0, |m| m.indices.len() / 3))
            .sum()
    }
}

/// Why a file could not be read.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ImportError {
    /// Not a glTF file, or a damaged one.
    #[error("the file is not a readable glTF/GLB file: {0}")]
    Unreadable(String),
    /// The file refers to data that is not there (an external buffer next to a .gltf file).
    #[error("the file refers to data that cannot be found: {0}")]
    MissingData(String),
    /// The file has no triangles at all.
    #[error("the file contains no triangles")]
    Empty,
}

const IDENTITY: [[f64; 4]; 4] =
    [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];

/// glTF's Y-up to Z-up: a quarter turn about X (Y becomes Z, Z becomes −Y). Column-major.
const Y_UP_TO_Z_UP: [[f64; 4]; 4] =
    [[1.0, 0.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, -1.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]];

/// Column-major product `a · b`.
fn multiply(a: &[[f64; 4]; 4], b: &[[f64; 4]; 4]) -> [[f64; 4]; 4] {
    let mut out = [[0.0; 4]; 4];
    for (column, out_column) in out.iter_mut().enumerate() {
        for (row, value) in out_column.iter_mut().enumerate() {
            *value = (0..4).map(|k| a[k][row] * b[column][k]).sum();
        }
    }
    out
}

/// Reads a GLB file, or a .gltf file whose buffers are embedded or lie in `base_dir`.
pub fn read_gltf(bytes: &[u8], base_dir: Option<&Path>) -> Result<Assembly, ImportError> {
    let file = gltf::Gltf::from_slice(bytes).map_err(|e| ImportError::Unreadable(e.to_string()))?;
    let buffers = gltf::import_buffers(&file.document, base_dir, file.blob.clone())
        .map_err(|e| ImportError::MissingData(e.to_string()))?;
    let document = &file.document;
    let mut assembly = Assembly::default();
    // (glTF mesh, primitive) → index in `assembly.meshes`: a mesh shown by several nodes is read once.
    let mut read: BTreeMap<(usize, usize), Option<usize>> = BTreeMap::new();
    let scene = document.default_scene().or_else(|| document.scenes().next());
    let roots: Vec<gltf::Node<'_>> = match &scene {
        Some(scene) => scene.nodes().collect(),
        // No scene: every node that is nobody's child is a root.
        None => {
            let children: std::collections::BTreeSet<usize> =
                document.nodes().flat_map(|n| n.children().map(|c| c.index()).collect::<Vec<_>>()).collect();
            document.nodes().filter(|n| !children.contains(&n.index())).collect()
        }
    };
    let mut stack: Vec<(gltf::Node<'_>, Option<usize>)> =
        roots.into_iter().rev().map(|n| (n, None)).collect();
    let mut visited = std::collections::BTreeSet::new();
    while let Some((node, parent)) = stack.pop() {
        if !visited.insert(node.index()) {
            assembly.warnings.push(format!("node {} appears twice in the tree; shown once", node.index()));
            continue;
        }
        let local = node.transform().matrix().map(|column| column.map(f64::from));
        let matrix = if parent.is_none() { multiply(&Y_UP_TO_Z_UP, &local) } else { local };
        let mut meshes = Vec::new();
        if let Some(mesh) = node.mesh() {
            let count = mesh.primitives().len();
            for primitive in mesh.primitives() {
                let key = (mesh.index(), primitive.index());
                let index = *read.entry(key).or_insert_with(|| {
                    read_primitive(&mesh, &primitive, count, &buffers, &mut assembly.warnings).map(|m| {
                        assembly.meshes.push(m);
                        assembly.meshes.len() - 1
                    })
                });
                meshes.extend(index);
            }
        }
        let name = node
            .name()
            .map(str::to_owned)
            .or_else(|| node.mesh().and_then(|m| m.name().map(str::to_owned)))
            .unwrap_or_else(|| format!("Node {}", node.index()));
        assembly.nodes.push(AssemblyNode { name, parent, matrix, meshes });
        let me = assembly.nodes.len() - 1;
        let children: Vec<gltf::Node<'_>> = node.children().collect();
        stack.extend(children.into_iter().rev().map(|c| (c, Some(me))));
    }
    if assembly.meshes.is_empty() {
        return Err(ImportError::Empty);
    }
    Ok(assembly)
}

fn read_primitive(
    mesh: &gltf::Mesh<'_>,
    primitive: &gltf::Primitive<'_>,
    count: usize,
    buffers: &[gltf::buffer::Data],
    warnings: &mut Vec<String>,
) -> Option<AssemblyMesh> {
    let base = mesh.name().map_or_else(|| format!("Mesh {}", mesh.index()), str::to_owned);
    let name = if count > 1 { format!("{base} #{}", primitive.index()) } else { base };
    if primitive.mode() != gltf::mesh::Mode::Triangles {
        warnings.push(format!("{name}: {:?} are not triangles; skipped", primitive.mode()));
        return None;
    }
    let reader = primitive.reader(|buffer| buffers.get(buffer.index()).map(|data| data.0.as_slice()));
    let Some(positions) = reader.read_positions() else {
        warnings.push(format!("{name}: no positions; skipped"));
        return None;
    };
    let positions: Vec<[f32; 3]> = positions.collect();
    let indices: Vec<u32> = match reader.read_indices() {
        Some(indices) => indices.into_u32().collect(),
        None => (0..u32::try_from(positions.len()).unwrap_or(u32::MAX)).collect(),
    };
    let usable = indices.len() / 3 * 3;
    let bad = indices[..usable].iter().any(|i| *i as usize >= positions.len());
    if usable == 0 || bad || positions.iter().flatten().any(|v| !v.is_finite()) {
        warnings.push(format!("{name}: damaged triangles; skipped"));
        return None;
    }
    let color = primitive.material().pbr_metallic_roughness().base_color_factor();
    Some(AssemblyMesh { name, positions, indices: indices[..usable].to_vec(), color })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic, clippy::float_cmp, reason = "test code")]

    use super::*;

    /// A GLB: one triangle mesh (red) shown by two nodes, the second a child of the first, moved 1 m
    /// along glTF's Y (up), plus a node with only lines.
    fn sample_glb() -> Vec<u8> {
        let mut bin: Vec<u8> = Vec::new();
        for v in [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]] {
            for c in v {
                bin.extend_from_slice(&c.to_le_bytes());
            }
        }
        for i in [0u16, 1, 2, 0] {
            bin.extend_from_slice(&i.to_le_bytes());
        }
        let json = format!(
            r#"{{"asset":{{"version":"2.0"}},"scene":0,"scenes":[{{"nodes":[0]}}],
            "nodes":[{{"name":"Frame","mesh":0,"children":[1,2]}},{{"name":"Copy","mesh":0,"translation":[0,1,0]}},{{"mesh":1}}],
            "meshes":[{{"name":"Plate","primitives":[{{"attributes":{{"POSITION":0}},"indices":1,"material":0}}]}},
                      {{"name":"Wire","primitives":[{{"attributes":{{"POSITION":0}},"mode":1}}]}}],
            "materials":[{{"pbrMetallicRoughness":{{"baseColorFactor":[1,0,0,1]}}}}],
            "accessors":[{{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,0,1]}},
                         {{"bufferView":1,"componentType":5123,"count":3,"type":"SCALAR"}}],
            "bufferViews":[{{"buffer":0,"byteOffset":0,"byteLength":36}},{{"buffer":0,"byteOffset":36,"byteLength":6}}],
            "buffers":[{{"byteLength":{}}}]}}"#,
            bin.len()
        );
        let mut json = json.into_bytes();
        while !json.len().is_multiple_of(4) {
            json.push(b' ');
        }
        while !bin.len().is_multiple_of(4) {
            bin.push(0);
        }
        let total = 12 + 8 + json.len() + 8 + bin.len();
        let mut glb = Vec::new();
        glb.extend_from_slice(b"glTF");
        glb.extend_from_slice(&2u32.to_le_bytes());
        glb.extend_from_slice(&u32::try_from(total).unwrap().to_le_bytes());
        glb.extend_from_slice(&u32::try_from(json.len()).unwrap().to_le_bytes());
        glb.extend_from_slice(&0x4E4F_534Au32.to_le_bytes());
        glb.extend_from_slice(&json);
        glb.extend_from_slice(&u32::try_from(bin.len()).unwrap().to_le_bytes());
        glb.extend_from_slice(&0x004E_4942u32.to_le_bytes());
        glb.extend_from_slice(&bin);
        glb
    }

    #[test]
    fn a_glb_keeps_its_tree_names_colours_and_instances() {
        let assembly = read_gltf(&sample_glb(), None).unwrap();
        let names: Vec<&str> = assembly.nodes.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, ["Frame", "Copy", "Wire"]);
        assert_eq!(assembly.nodes[1].parent, Some(0));
        assert_eq!(assembly.nodes[2].parent, Some(0));
        // One mesh, shown twice; the lines are skipped with a warning.
        assert_eq!(assembly.meshes.len(), 1);
        assert_eq!(assembly.nodes[0].meshes, vec![0]);
        assert_eq!(assembly.nodes[1].meshes, vec![0]);
        assert!(assembly.nodes[2].meshes.is_empty());
        assert_eq!(assembly.warnings.len(), 1, "{:?}", assembly.warnings);
        assert_eq!(assembly.meshes[0].color, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(assembly.meshes[0].name, "Plate");
        assert_eq!(assembly.meshes[0].indices, vec![0, 1, 2]);
        assert_eq!(assembly.instanced_triangles(), 2);
        // glTF's up (Y) is the world's Z: the copy sits 1 m above the frame.
        let world = assembly.world_matrix(1);
        assert_eq!([world[3][0], world[3][1], world[3][2]], [0.0, 0.0, 1.0]);
        // A point on glTF's +Z axis lands on the world's −Y axis.
        let root = assembly.world_matrix(0);
        assert_eq!([root[2][0], root[2][1], root[2][2]], [0.0, -1.0, 0.0]);
    }

    #[test]
    fn damaged_or_empty_files_are_refused() {
        assert!(matches!(read_gltf(b"not a gltf file", None), Err(ImportError::Unreadable(_))));
        let mut truncated = sample_glb();
        truncated.truncate(40);
        assert!(read_gltf(&truncated, None).is_err());
        let empty = br#"{"asset":{"version":"2.0"},"nodes":[{"name":"x"}]}"#;
        assert_eq!(read_gltf(empty, None), Err(ImportError::Empty));
    }
}
