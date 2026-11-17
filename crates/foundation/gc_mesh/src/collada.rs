//! Reading COLLADA (`.dae`) files: the triangles of every geometry the scene places, grouped by material.
//!
//! This reads what robot descriptions use: `triangles` and `polylist` primitives, node transforms, the
//! file's unit and up axis, and each material's diffuse colour. Textures, animations and skinning are
//! ignored.

use std::collections::BTreeMap;

use gc_math::{DMat4, DVec3};
use roxmltree::{Document, Node};

use crate::{MeshData, MeshError};

/// The triangles of one material.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    /// The material's name in the file; empty for a file format without materials.
    pub material: String,
    /// The material's diffuse colour as RGBA in 0..=1, when the file gives one.
    pub color: Option<[f32; 4]>,
    /// The triangles, in metres, Z up.
    pub mesh: MeshData,
}

const BAD: MeshError = MeshError::Unreadable("the COLLADA file is damaged");

fn child<'a>(node: Node<'a, 'a>, name: &str) -> Option<Node<'a, 'a>> {
    node.children().find(|c| c.has_tag_name(name))
}

fn children<'a>(node: Node<'a, 'a>, name: &'a str) -> impl Iterator<Item = Node<'a, 'a>> {
    node.children().filter(move |c| c.has_tag_name(name))
}

fn numbers<T: std::str::FromStr>(node: Node<'_, '_>) -> Result<Vec<T>, MeshError> {
    node.text().unwrap_or("").split_ascii_whitespace().map(|word| word.parse().map_err(|_bad| BAD)).collect()
}

/// The element an `url="#id"` or `source="#id"` attribute points at.
fn target<'a>(by_id: &BTreeMap<&'a str, Node<'a, 'a>>, reference: Option<&str>) -> Option<Node<'a, 'a>> {
    by_id.get(reference?.strip_prefix('#')?).copied()
}

/// The transform of one node: its `matrix`, `translate`, `rotate` and `scale` children in order.
fn local_transform(node: Node<'_, '_>) -> Result<DMat4, MeshError> {
    let mut transform = DMat4::IDENTITY;
    for step in node.children().filter(Node::is_element) {
        let values = || numbers::<f64>(step);
        let next = match step.tag_name().name() {
            "matrix" => {
                // Written row by row.
                let m: [f64; 16] = values()?.try_into().map_err(|_bad| BAD)?;
                DMat4::from_cols_array(&m).transpose()
            }
            "translate" => match values()?.as_slice() {
                [x, y, z] => DMat4::from_translation(DVec3::new(*x, *y, *z)),
                _ => return Err(BAD),
            },
            "scale" => match values()?.as_slice() {
                [x, y, z] => DMat4::from_scale(DVec3::new(*x, *y, *z)),
                _ => return Err(BAD),
            },
            "rotate" => match values()?.as_slice() {
                [x, y, z, degrees] => match DVec3::new(*x, *y, *z).try_normalize() {
                    Some(axis) => DMat4::from_axis_angle(axis, degrees.to_radians()),
                    None => DMat4::IDENTITY,
                },
                _ => return Err(BAD),
            },
            _ => continue,
        };
        transform *= next;
    }
    Ok(transform)
}

struct Reader<'a> {
    by_id: BTreeMap<&'a str, Node<'a, 'a>>,
    /// From file coordinates to metres with Z up.
    to_world: DMat4,
    /// Triangles collected so far, by material id.
    parts: BTreeMap<String, (Vec<[f32; 3]>, Vec<u32>)>,
}

impl<'a> Reader<'a> {
    /// Adds the triangles of every geometry placed under `node`.
    fn walk(&mut self, node: Node<'a, 'a>, parent: DMat4, depth: usize) -> Result<(), MeshError> {
        if depth > 64 {
            return Err(MeshError::Unreadable("the COLLADA scene is nested too deeply"));
        }
        let transform = parent * local_transform(node)?;
        for instance in children(node, "instance_geometry") {
            let Some(geometry) = target(&self.by_id, instance.attribute("url")) else { continue };
            // Which material each of the geometry's material symbols stands for here.
            let bound: BTreeMap<&str, &str> = instance
                .descendants()
                .filter(|n| n.has_tag_name("instance_material"))
                .filter_map(|n| Some((n.attribute("symbol")?, n.attribute("target")?.strip_prefix('#')?)))
                .collect();
            self.geometry(geometry, transform, &bound)?;
        }
        for below in children(node, "node") {
            self.walk(below, transform, depth + 1)?;
        }
        Ok(())
    }

    fn geometry(
        &mut self,
        geometry: Node<'a, 'a>,
        transform: DMat4,
        bound: &BTreeMap<&str, &str>,
    ) -> Result<(), MeshError> {
        let Some(mesh) = child(geometry, "mesh") else { return Ok(()) };
        for primitive in mesh.children().filter(|n| n.has_tag_name("triangles") || n.has_tag_name("polylist"))
        {
            let inputs: Vec<Node<'_, '_>> = children(primitive, "input").collect();
            let stride = inputs
                .iter()
                .filter_map(|i| i.attribute("offset")?.parse::<usize>().ok())
                .max()
                .map_or(1, |highest| highest + 1);
            let vertex = inputs.iter().find(|i| i.attribute("semantic") == Some("VERTEX")).ok_or(BAD)?;
            let offset: usize = vertex.attribute("offset").and_then(|o| o.parse().ok()).unwrap_or(0);
            let vertices = target(&self.by_id, vertex.attribute("source")).ok_or(BAD)?;
            let position_input = children(vertices, "input")
                .find(|i| i.attribute("semantic") == Some("POSITION"))
                .ok_or(BAD)?;
            let source = target(&self.by_id, position_input.attribute("source")).ok_or(BAD)?;
            let coordinates: Vec<f64> = numbers(child(source, "float_array").ok_or(BAD)?)?;
            let indices: Vec<usize> = match child(primitive, "p") {
                Some(p) => numbers(p)?,
                None => continue,
            };
            let corners: Vec<usize> = match child(primitive, "vcount") {
                Some(vcount) => numbers(vcount)?,
                None => vec![3; indices.len() / stride / 3],
            };

            let symbol = primitive.attribute("material").unwrap_or("");
            let material = bound.get(symbol).copied().unwrap_or(symbol).to_owned();
            let (positions, triangles) = self.parts.entry(material).or_default();
            // Each position of the source is copied the first time a corner uses it.
            let mut copied = vec![u32::MAX; coordinates.len() / 3];
            let place = self.to_world * transform;
            let mut corner_of = |k: usize| -> Result<u32, MeshError> {
                let index = *indices.get(k * stride + offset).ok_or(BAD)?;
                let slot = copied.get_mut(index).ok_or(BAD)?;
                if *slot == u32::MAX {
                    let p = DVec3::new(
                        coordinates[3 * index],
                        coordinates[3 * index + 1],
                        coordinates[3 * index + 2],
                    );
                    let p = place.transform_point3(p);
                    *slot = u32::try_from(positions.len()).map_err(|_overflow| MeshError::TooManyVertices)?;
                    #[allow(clippy::cast_possible_truncation, reason = "mesh coordinates are f32")]
                    positions.push([p.x as f32, p.y as f32, p.z as f32]);
                }
                Ok(*slot)
            };
            let mut first = 0;
            for count in corners {
                // A polygon with more than three corners becomes a fan of triangles.
                for k in 1..count.saturating_sub(1) {
                    triangles.extend([corner_of(first)?, corner_of(first + k)?, corner_of(first + k + 1)?]);
                }
                first += count;
            }
        }
        Ok(())
    }

    /// The diffuse colour of a material.
    fn color(&self, material: &str) -> Option<[f32; 4]> {
        let material = self.by_id.get(material)?;
        let effect = target(&self.by_id, child(*material, "instance_effect")?.attribute("url"))?;
        let diffuse = effect.descendants().find(|n| n.has_tag_name("diffuse"))?;
        let rgba: Vec<f32> = numbers(child(diffuse, "color")?).ok()?;
        match rgba.as_slice() {
            [r, g, b, a] => Some([*r, *g, *b, *a]),
            [r, g, b] => Some([*r, *g, *b, 1.0]),
            _ => None,
        }
    }
}

/// True when the bytes look like a COLLADA file.
pub fn is_collada(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(2048)];
    head.windows(8).any(|window| window == b"<COLLADA")
}

/// Reads a COLLADA file into one part per material.
pub fn parse(bytes: &[u8]) -> Result<Vec<Part>, MeshError> {
    let text = std::str::from_utf8(bytes).map_err(|_not_text| BAD)?;
    let document = Document::parse(text).map_err(|_not_xml| BAD)?;
    let root = document.root_element();
    if !root.has_tag_name("COLLADA") {
        return Err(MeshError::Unreadable("not a COLLADA file"));
    }
    let asset = child(root, "asset");
    let metres = asset
        .and_then(|a| child(a, "unit"))
        .and_then(|u| u.attribute("meter")?.parse::<f64>().ok())
        .filter(|m| m.is_finite() && *m > 0.0)
        .unwrap_or(1.0);
    // The file's up axis becomes Z; the default of the format is Y up.
    let up = asset.and_then(|a| child(a, "up_axis")).and_then(|u| u.text()).map_or("Y_UP", str::trim);
    let (x, y, z) = (DVec3::X, DVec3::Y, DVec3::Z);
    let axes = match up {
        "Z_UP" => [x, y, z],
        "X_UP" => [-y, x, z],
        _ => [x, z, -y],
    };
    // `axes[i]` is where the file's i-th axis points in the Z-up frame.
    let to_world = DMat4::from_cols(
        (axes[0] * metres).extend(0.0),
        (axes[1] * metres).extend(0.0),
        (axes[2] * metres).extend(0.0),
        DVec3::ZERO.extend(1.0),
    );
    let by_id = root.descendants().filter_map(|n| Some((n.attribute("id")?, n))).collect();
    let mut reader = Reader { by_id, to_world, parts: BTreeMap::new() };

    let scenes: Vec<Node<'_, '_>> = root.descendants().filter(|n| n.has_tag_name("visual_scene")).collect();
    for scene in &scenes {
        for node in children(*scene, "node") {
            reader.walk(node, DMat4::IDENTITY, 0)?;
        }
    }
    if reader.parts.is_empty() {
        // A file without a scene: take every geometry as it is.
        let geometries: Vec<Node<'_, '_>> =
            root.descendants().filter(|n| n.has_tag_name("geometry")).collect();
        for geometry in geometries {
            reader.geometry(geometry, DMat4::IDENTITY, &BTreeMap::new())?;
        }
    }
    let mut parts = Vec::new();
    for (material, (positions, indices)) in std::mem::take(&mut reader.parts) {
        if indices.is_empty() {
            continue;
        }
        let color = reader.color(&material);
        parts.push(Part { material, color, mesh: MeshData::new(positions, indices)? });
    }
    if parts.is_empty() {
        return Err(MeshError::Unreadable("the COLLADA file has no triangles"));
    }
    Ok(parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file in millimetres with Y up: a red triangle and a blue quad, under a node moved 1000 along X.
    const FILE: &str = r##"<?xml version="1.0"?>
<COLLADA xmlns="http://www.collada.org/2005/11/COLLADASchema" version="1.4.1">
  <asset><unit name="millimeter" meter="0.001"/><up_axis>Y_UP</up_axis></asset>
  <library_effects>
    <effect id="red-fx"><profile_COMMON><technique sid="c"><phong>
      <diffuse><color sid="diffuse">1 0 0 1</color></diffuse></phong></technique></profile_COMMON></effect>
    <effect id="blue-fx"><profile_COMMON><technique sid="c"><lambert>
      <diffuse><color>0 0 1 0.5</color></diffuse></lambert></technique></profile_COMMON></effect>
  </library_effects>
  <library_materials>
    <material id="red"><instance_effect url="#red-fx"/></material>
    <material id="blue"><instance_effect url="#blue-fx"/></material>
  </library_materials>
  <library_geometries>
    <geometry id="g"><mesh>
      <source id="g-pos"><float_array id="g-pos-a" count="15">0 0 0  100 0 0  100 200 0  0 200 0  0 0 300</float_array></source>
      <source id="g-nor"><float_array id="g-nor-a" count="3">0 0 1</float_array></source>
      <vertices id="g-vtx"><input semantic="POSITION" source="#g-pos"/></vertices>
      <triangles material="m0" count="1">
        <input semantic="VERTEX" source="#g-vtx" offset="0"/><input semantic="NORMAL" source="#g-nor" offset="1"/>
        <p>0 0 1 0 4 0</p>
      </triangles>
      <polylist material="m1" count="1">
        <input semantic="VERTEX" source="#g-vtx" offset="0"/><input semantic="NORMAL" source="#g-nor" offset="1"/>
        <vcount>4</vcount>
        <p>0 0 1 0 2 0 3 0</p>
      </polylist>
    </mesh></geometry>
  </library_geometries>
  <library_visual_scenes><visual_scene id="s">
    <node id="outer"><translate>1000 0 0</translate>
      <node id="inner"><matrix>1 0 0 0  0 1 0 0  0 0 1 0  0 0 0 1</matrix>
        <instance_geometry url="#g"><bind_material><technique_common>
          <instance_material symbol="m0" target="#red"/><instance_material symbol="m1" target="#blue"/>
        </technique_common></bind_material></instance_geometry>
      </node>
    </node>
  </visual_scene></library_visual_scenes>
</COLLADA>"##;

    #[test]
    fn parts_follow_materials_units_axes_and_node_transforms() {
        assert!(is_collada(FILE.as_bytes()) && !is_collada(b"solid x"));
        let parts = parse(FILE.as_bytes()).unwrap();
        assert_eq!(parts.len(), 2);
        let (blue, red) = (&parts[0], &parts[1]);
        assert_eq!((blue.material.as_str(), blue.color), ("blue", Some([0.0, 0.0, 1.0, 0.5])));
        assert_eq!((red.material.as_str(), red.color), ("red", Some([1.0, 0.0, 0.0, 1.0])));
        // The quad became two triangles on four vertices; the triangle has three.
        assert_eq!((blue.mesh.vertex_count(), blue.mesh.triangle_count()), (4, 2));
        assert_eq!((red.mesh.vertex_count(), red.mesh.triangle_count()), (3, 1));
        // Millimetres became metres, the node moved everything 1 m along X, and the file's Y (up) is Z:
        // the quad spans 0.1 m in X and 0.2 m in Z; the triangle's third corner, 300 along the file's Z,
        // ends up at -0.3 in Y.
        let near = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).all(|(p, q)| (p - q).abs() < 1e-6);
        assert!(near(blue.mesh.positions()[2], [1.1, 0.0, 0.2]), "{:?}", blue.mesh.positions());
        assert!(near(red.mesh.positions()[2], [1.0, -0.3, 0.0]), "{:?}", red.mesh.positions());
    }

    #[test]
    fn damaged_files_are_refused() {
        assert!(parse(b"<COLLADA><asset/></COLLADA>").is_err(), "no triangles");
        assert!(parse(b"<html/>").is_err());
        assert!(parse(FILE.replace("0 0 1 0 4 0", "0 0 1 0 9 0").as_bytes()).is_err(), "index out of range");
        assert!(parse(FILE.replace("100 200 0", "100 abc 0").as_bytes()).is_err());
        assert!(parse(&FILE.as_bytes()[..FILE.len() / 2]).is_err());
    }
}
