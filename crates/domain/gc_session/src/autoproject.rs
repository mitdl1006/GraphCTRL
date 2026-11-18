//! Autoproject edges for sketch creation (Inventor 2026 help, Sketch Tab Reference (Application Options),
//! GUID-1898C432-CFD2-46F1-8BAE-631392DFBF1D: "When you create or edit a sketch, automatically projects the
//! edges of the selected face onto the sketch plane as reference geometry"; About Projecting Sketch
//! Geometry, GUID-B61431DA-FABC-41EE-945C-6E54B4806582: "When you create a sketch on the planar face of a
//! model, all edges of the face are automatically added to the sketch as reference geometry").
//!
//! An imported part is a triangle mesh, so its face is the flat region of triangles joined to the picked
//! one in its plane; the region's outline (edges used by one triangle of it) becomes fixed lines of the
//! new sketch, as Project Cut Edges adds them.

use std::collections::{BTreeMap, BTreeSet};

use gc_math::DVec3;

use crate::Session;

/// An edge by its rounded ends, smaller first.
type EdgeKey = ([i64; 3], [i64; 3]);

/// A position rounded for matching corners of neighbouring triangles (meshes often repeat vertices).
fn key(p: DVec3) -> [i64; 3] {
    let k = |v: f64| {
        #[allow(clippy::cast_possible_truncation, reason = "rounded to nanometres, well inside i64")]
        let n = (v * 1e9).round() as i64;
        n
    };
    [k(p.x), k(p.y), k(p.z)]
}

/// The outline of the flat face of `positions`/`indices` that holds `triangle`: segments in the mesh's
/// coordinates.
pub(crate) fn face_outline(positions: &[[f32; 3]], indices: &[u32], triangle: usize) -> Vec<[DVec3; 2]> {
    let at = |i: u32| positions.get(i as usize).map(|p| DVec3::new(p[0].into(), p[1].into(), p[2].into()));
    let corners = |t: usize| -> Option<[DVec3; 3]> {
        let c = indices.get(t * 3..t * 3 + 3)?;
        Some([at(c[0])?, at(c[1])?, at(c[2])?])
    };
    let Some(first) = corners(triangle) else { return Vec::new() };
    let normal = (first[1] - first[0]).cross(first[2] - first[0]);
    if normal.length() < 1e-18 {
        return Vec::new();
    }
    let normal = normal.normalize();
    let size = (first[1] - first[0]).length().max((first[2] - first[0]).length());
    let flat = |c: &[DVec3; 3]| {
        let n = (c[1] - c[0]).cross(c[2] - c[0]);
        n.length() > 1e-18
            && n.normalize().dot(normal) > 1.0 - 1e-9
            && c.iter().all(|p| (*p - first[0]).dot(normal).abs() <= 1e-7 * size.max(1e-3))
    };
    // Triangles by their edges, for walking from one to its neighbours in the plane.
    let count = indices.len() / 3;
    let mut by_edge: BTreeMap<EdgeKey, Vec<usize>> = BTreeMap::new();
    let edge = |a: DVec3, b: DVec3| {
        let (ka, kb) = (key(a), key(b));
        if ka <= kb { (ka, kb) } else { (kb, ka) }
    };
    let mut flat_triangles = BTreeMap::new();
    for t in 0..count {
        if let Some(c) = corners(t).filter(flat) {
            for (a, b) in [(c[0], c[1]), (c[1], c[2]), (c[2], c[0])] {
                by_edge.entry(edge(a, b)).or_default().push(t);
            }
            flat_triangles.insert(t, c);
        }
    }
    let mut region = BTreeSet::from([triangle]);
    let mut todo = vec![triangle];
    while let Some(t) = todo.pop() {
        let Some(c) = flat_triangles.get(&t) else { continue };
        for (a, b) in [(c[0], c[1]), (c[1], c[2]), (c[2], c[0])] {
            for next in by_edge.get(&edge(a, b)).into_iter().flatten() {
                if region.insert(*next) {
                    todo.push(*next);
                }
            }
        }
    }
    // The outline: edges used by one triangle of the region.
    let mut uses: BTreeMap<EdgeKey, (usize, [DVec3; 2])> = BTreeMap::new();
    for t in &region {
        let Some(c) = flat_triangles.get(t) else { continue };
        for (a, b) in [(c[0], c[1]), (c[1], c[2]), (c[2], c[0])] {
            uses.entry(edge(a, b)).or_insert((0, [a, b])).0 += 1;
        }
    }
    let outline: Vec<[DVec3; 2]> = uses.into_values().filter(|(n, _)| *n == 1).map(|(_, s)| s).collect();
    gc_mesh::slice::join_straight(&outline, 1e-6)
}

impl Session {
    /// Autoproject edges for sketch creation and edit (Application Options, Sketch tab): on by default,
    /// as in Inventor.
    pub fn autoproject_face_edges(&self) -> bool {
        self.autoproject_edges
    }

    /// Turns Autoproject edges for sketch creation on or off.
    pub fn set_autoproject_face_edges(&mut self, on: bool) {
        self.autoproject_edges = on;
    }

    /// Adds the outline of the picked face to the sketch just created on it (its plane's origin is the
    /// triangle's centre, X along its first edge, as `face_source` makes it).
    pub(crate) fn project_face_outline(
        &mut self,
        sketch: gc_core::Id<gc_sketch::Sketch>,
        assembly: gc_core::Id<gc_doc::Assembly>,
        mesh: usize,
        triangle: usize,
    ) {
        let Some(stored) = self.doc.get(assembly) else { return };
        let Some(data) = self.cell.assemblies.of(stored).ok().and_then(|m| m.get(mesh)).map(|m| &m.data)
        else {
            return;
        };
        let (positions, indices) = (data.positions(), data.indices());
        let Some(c) = indices.get(triangle * 3..triangle * 3 + 3) else { return };
        let at =
            |i: u32| positions.get(i as usize).map(|p| DVec3::new(p[0].into(), p[1].into(), p[2].into()));
        let (Some(a), Some(b), Some(cc)) = (at(c[0]), at(c[1]), at(c[2])) else { return };
        let x = (b - a).normalize();
        let z = (b - a).cross(cc - a).normalize();
        let y = z.cross(x);
        let centre = (a + b + cc) / 3.0;
        let flat = |p: DVec3| {
            let d = p - centre;
            [d.dot(x), d.dot(y), 0.0]
        };
        let lines: Vec<([f64; 3], [f64; 3])> =
            face_outline(positions, indices, triangle).into_iter().map(|[p, q]| (flat(p), flat(q))).collect();
        if lines.is_empty() {
            return;
        }
        let op = gc_doc::SketchOp::ProjectedLines { lines };
        // A sketch that cannot take the outline is still made; the outline is only a help.
        if self.execute(&gc_doc::Command::Sketch { sketch, op }).is_err() {
            self.last_error = None;
        }
    }
}
