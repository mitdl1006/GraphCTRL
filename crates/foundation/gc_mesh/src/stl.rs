//! Reading STL files, binary and text.

use crate::{MeshData, MeshError};

/// Reads an STL file. Corners that share a position become one vertex, so that the mesh is connected
/// the way the solid is.
pub fn parse(bytes: &[u8]) -> Result<MeshData, MeshError> {
    let positions = match binary(bytes) {
        Some(positions) => positions,
        None => text(bytes).ok_or(MeshError::Unreadable("not a binary or text STL file"))?,
    };
    if positions.is_empty() {
        return Err(MeshError::Unreadable("the STL file has no triangles"));
    }
    let count = u32::try_from(positions.len()).map_err(|_overflow| MeshError::TooManyVertices)?;
    Ok(MeshData::new(positions, (0..count).collect())?.welded(0.0))
}

/// The mesh as a binary STL file.
pub fn to_binary(mesh: &MeshData) -> Vec<u8> {
    let mut bytes = vec![0_u8; 80];
    bytes.extend(u32::try_from(mesh.triangle_count()).unwrap_or(u32::MAX).to_le_bytes());
    for triangle in mesh.indices().chunks_exact(3) {
        bytes.extend([0_u8; 12]); // the normal: readers recompute it
        for index in triangle {
            let position = mesh.positions().get(*index as usize).copied().unwrap_or_default();
            for value in position {
                bytes.extend(value.to_le_bytes());
            }
        }
        bytes.extend([0_u8; 2]);
    }
    bytes
}

/// A binary file: 80 bytes of header, the triangle count, then 50 bytes per triangle. The count must
/// match the length exactly, which also tells binary files that start with `solid` from text files.
fn binary(bytes: &[u8]) -> Option<Vec<[f32; 3]>> {
    let count = u32::from_le_bytes(bytes.get(80..84)?.try_into().ok()?) as usize;
    let body = bytes.get(84..)?;
    if body.len() != count.checked_mul(50)? {
        return None;
    }
    let mut positions = Vec::with_capacity(count * 3);
    for triangle in body.chunks_exact(50) {
        // 12 bytes of normal (ignored: it is recomputed), three corners, two attribute bytes.
        for corner in triangle.get(12..48)?.chunks_exact(12) {
            let mut position = [0.0_f32; 3];
            for (value, raw) in position.iter_mut().zip(corner.chunks_exact(4)) {
                *value = f32::from_le_bytes(raw.try_into().ok()?);
            }
            positions.push(position);
        }
    }
    Some(positions)
}

/// A text file: every `vertex x y z` line is a corner; three in a row make a triangle.
fn text(bytes: &[u8]) -> Option<Vec<[f32; 3]>> {
    let text = std::str::from_utf8(bytes).ok()?;
    if !text.trim_start().starts_with("solid") {
        return None;
    }
    let mut positions = Vec::new();
    let mut words = text.split_ascii_whitespace();
    while let Some(word) = words.next() {
        if word == "vertex" {
            let mut position = [0.0_f32; 3];
            for value in &mut position {
                *value = words.next()?.parse().ok()?;
            }
            positions.push(position);
        }
    }
    (positions.len() % 3 == 0).then_some(positions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    /// A binary file that starts with `solid`, as some writers produce.
    fn binary_of(mesh: &MeshData) -> Vec<u8> {
        let mut bytes = to_binary(mesh);
        bytes[..5].copy_from_slice(b"solid");
        bytes
    }

    #[test]
    fn binary_and_text_files_give_the_same_connected_mesh() {
        let cube = primitives::cuboid([0.2, 0.4, 0.6]).unwrap();
        let from_binary = parse(&binary_of(&cube)).unwrap();
        assert_eq!((from_binary.vertex_count(), from_binary.triangle_count()), (8, 12));
        assert_eq!(from_binary.aabb(), cube.aabb());
        assert_eq!(from_binary.connected_components().len(), 1);

        let mut ascii = String::from("solid cube\n");
        for [a, b, c] in cube.triangles() {
            ascii.push_str("facet normal 0 0 0\n outer loop\n");
            for p in [a, b, c] {
                ascii.push_str(&format!("  vertex {} {} {}\n", p.x, p.y, p.z));
            }
            ascii.push_str(" endloop\nendfacet\n");
        }
        ascii.push_str("endsolid cube\n");
        let from_text = parse(ascii.as_bytes()).unwrap();
        assert_eq!((from_text.vertex_count(), from_text.triangle_count()), (8, 12));
        assert_eq!(from_text.aabb(), cube.aabb());
    }

    #[test]
    fn damaged_files_are_refused() {
        let cube = primitives::cuboid([1.0; 3]).unwrap();
        let mut bytes = binary_of(&cube);
        bytes.truncate(bytes.len() - 7);
        assert!(parse(&bytes).is_err());
        assert!(parse(b"").is_err());
        assert!(parse(b"solid empty\nendsolid empty\n").is_err());
        assert!(parse(b"solid x\nvertex 1 2\nendsolid").is_err());
        assert!(parse(b"<html>not a mesh</html>").is_err());
    }
}
