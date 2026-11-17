//! Where a plane cuts a mesh: the segments of the cut, joined where they continue straight. Used by
//! Project Cut Edges (the sketch plane through the model).

use std::collections::BTreeMap;

use gc_math::{DPose3, DVec3};

use crate::MeshData;

/// Corners closer to the plane than this count as on it (metres): a micrometre, since mesh coordinates
/// are single precision (a box face at 0.45 m lands about 10 nm off).
const ON_PLANE: f64 = 1e-6;
/// Points closer than this are one point when joining segments (metres).
const SAME_POINT: f64 = 1e-7;

/// The segments where the plane through `point` with unit `normal` cuts the mesh placed at `pose`, in the
/// world. Triangles lying in the plane give nothing.
pub fn plane_cut(mesh: &MeshData, pose: &DPose3, point: DVec3, normal: DVec3) -> Vec<[DVec3; 2]> {
    let mut segments = Vec::new();
    for triangle in mesh.triangles() {
        let corners = triangle.map(|c| *pose * c);
        let d = corners.map(|c| (c - point).dot(normal));
        if d.iter().all(|v| v.abs() <= ON_PLANE) {
            continue;
        }
        if d.iter().all(|v| *v > ON_PLANE) || d.iter().all(|v| *v < -ON_PLANE) {
            continue;
        }
        let mut found: Vec<DVec3> = Vec::with_capacity(3);
        let mut push = |p: DVec3| {
            if found.iter().all(|q| q.distance(p) > SAME_POINT) {
                found.push(p);
            }
        };
        for i in 0..3 {
            let j = (i + 1) % 3;
            if d[i].abs() <= ON_PLANE {
                push(corners[i]);
            } else if d[j].abs() > ON_PLANE && (d[i] > 0.0) != (d[j] > 0.0) {
                let t = d[i] / (d[i] - d[j]);
                push(corners[i] + (corners[j] - corners[i]) * t);
            }
        }
        if let [a, b] = found.as_slice() {
            segments.push([*a, *b]);
        }
    }
    segments
}

type Key = [i64; 3];

fn key(p: DVec3) -> Key {
    #[allow(clippy::cast_possible_truncation, reason = "0.1 µm cells of a robot cell fit i64")]
    (p / SAME_POINT).round().to_array().map(|c| c as i64)
}

/// Joins segments that continue one another in a straight line (within `angle` radians), and drops
/// duplicates (the same edge cut from both triangles that share it) and zero-length pieces.
pub fn join_straight(segments: &[[DVec3; 2]], angle: f64) -> Vec<[DVec3; 2]> {
    // Unique segments by their end keys.
    let mut unique: BTreeMap<(Key, Key), [DVec3; 2]> = BTreeMap::new();
    for [a, b] in segments {
        if a.distance(*b) <= SAME_POINT {
            continue;
        }
        let (ka, kb) = (key(*a), key(*b));
        unique.entry((ka.min(kb), ka.max(kb))).or_insert([*a, *b]);
    }
    let mut pieces: Vec<Option<[DVec3; 2]>> = unique.into_values().map(Some).collect();
    let sin_limit = gc_math::fm::sin(angle);
    loop {
        // Which pieces end at each point.
        let mut at: BTreeMap<Key, Vec<usize>> = BTreeMap::new();
        for (i, piece) in pieces.iter().enumerate() {
            if let Some([a, b]) = piece {
                at.entry(key(*a)).or_default().push(i);
                at.entry(key(*b)).or_default().push(i);
            }
        }
        let mut merged = false;
        for ends in at.values() {
            let [i, j] = ends.as_slice() else { continue };
            let (Some(Some(p)), Some(Some(q))) = (pieces.get(*i).copied(), pieces.get(*j).copied()) else {
                continue;
            };
            // Orient p as (x → shared) and q as (shared → y).
            let shared = if key(p[0]) == key(q[0]) || key(p[0]) == key(q[1]) { p[0] } else { p[1] };
            let x = if key(p[0]) == key(shared) { p[1] } else { p[0] };
            let y = if key(q[0]) == key(shared) { q[1] } else { q[0] };
            let (u, w) = (shared - x, y - shared);
            let (lu, lw) = (u.length(), w.length());
            if lu <= 0.0 || lw <= 0.0 || u.dot(w) <= 0.0 || u.cross(w).length() > sin_limit * lu * lw {
                continue;
            }
            if let Some(slot) = pieces.get_mut(*i) {
                *slot = Some([x, y]);
            }
            if let Some(slot) = pieces.get_mut(*j) {
                *slot = None;
            }
            merged = true;
            break;
        }
        if !merged {
            break;
        }
    }
    pieces.into_iter().flatten().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plane_through_a_cube_cuts_a_square() {
        let cube = crate::test_util::cube([0.0; 3], 2.0);
        let cut = plane_cut(&cube, &DPose3::IDENTITY, DVec3::new(0.0, 0.0, 0.5), DVec3::Z);
        let edges = join_straight(&cut, 1e-6);
        assert_eq!(edges.len(), 4, "{edges:?}");
        let length: f64 = edges.iter().map(|[a, b]| a.distance(*b)).sum();
        assert!((length - 8.0).abs() < 1e-9);
        assert!(edges.iter().all(|[a, b]| (a.z - 0.5).abs() < 1e-12 && (b.z - 0.5).abs() < 1e-12));
        // Placed elsewhere, the cut follows; a plane that misses gives nothing.
        let moved = DPose3::from_translation(DVec3::new(10.0, 0.0, 0.0));
        let far = plane_cut(&cube, &moved, DVec3::new(0.0, 0.0, 0.5), DVec3::Z);
        assert!(join_straight(&far, 1e-6).iter().all(|[a, _]| a.x >= 10.0 - 1e-12));
        assert!(plane_cut(&cube, &DPose3::IDENTITY, DVec3::new(0.0, 0.0, 5.0), DVec3::Z).is_empty());
        // A plane through a face of the cube (corners on it) gives no stray segments of the face itself.
        let on_face = join_straight(&plane_cut(&cube, &DPose3::IDENTITY, DVec3::ZERO, DVec3::Z), 1e-6);
        assert!(on_face.len() <= 4);
    }
}
