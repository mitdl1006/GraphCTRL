//! Project Cut Edges (docs/ux/inventor-assembly-parity.md, P1-6): where the sketch plane cuts a
//! component's meshes, as fixed segments in the sketch.
//!
//! Inventor 2026 help, To Project Geometry into a 2D Sketch (GUID-B8D8B07B-86F2-4CB7-AB21-3131D94AD1CC):
//! "click a part that a section plane cuts. All edges that the plane cuts are projected onto the sketch
//! plane"; About Projecting Sketch Geometry (GUID-B61431DA-FABC-41EE-945C-6E54B4806582): projected cut
//! edges are not associative, a snapshot of the geometry when projected.

use gc_core::Id;
use gc_doc::{Command, EditError, Occurrence, SketchOp, Solid};
use gc_math::{DPose3, DVec3};

use crate::Session;

/// The segments where a plane (point `origin`, unit `normal`) cuts triangles.
pub(crate) fn slice(
    triangles: impl Iterator<Item = [DVec3; 3]>,
    origin: DVec3,
    normal: DVec3,
) -> Vec<(DVec3, DVec3)> {
    let mut out = Vec::new();
    for tri in triangles {
        let d = tri.map(|v| (v - origin).dot(normal));
        let scale = (tri[1] - tri[0]).length().max((tri[2] - tri[0]).length()).max(1e-9);
        let eps = 1e-9 * scale;
        if d.iter().all(|x| x.abs() <= eps) {
            continue; // lying in the plane: no cut
        }
        let mut points: Vec<DVec3> = Vec::new();
        let add = |p: DVec3, points: &mut Vec<DVec3>| {
            if points.iter().all(|q| (*q - p).length() > eps) {
                points.push(p);
            }
        };
        for i in 0..3 {
            let (a, b) = (i, (i + 1) % 3);
            if d[a].abs() <= eps {
                add(tri[a], &mut points);
            }
            if (d[a] > eps && d[b] < -eps) || (d[a] < -eps && d[b] > eps) {
                let t = d[a] / (d[a] - d[b]);
                add(tri[a] + (tri[b] - tri[a]) * t, &mut points);
            }
        }
        if let [p, q] = points[..] {
            out.push((p, q));
        }
    }
    out
}

/// Joins segments that continue each other in a straight line and drops repeats.
pub(crate) fn merge(mut segments: Vec<([f64; 2], [f64; 2])>) -> Vec<([f64; 2], [f64; 2])> {
    let near = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9;
    let cross =
        |o: [f64; 2], a: [f64; 2], b: [f64; 2]| (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
    let length = |a: [f64; 2], b: [f64; 2]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
    segments.retain(|(a, b)| !near(*a, *b));
    let mut changed = true;
    while changed {
        changed = false;
        'outer: for i in 0..segments.len() {
            for j in i + 1..segments.len() {
                let (a, b) = segments[i];
                let (c, d) = segments[j];
                if (near(a, c) && near(b, d)) || (near(a, d) && near(b, c)) {
                    segments.remove(j);
                    changed = true;
                    break 'outer;
                }
                // A shared end, and the other ends in line with the first segment.
                let joined = [(a, b, c, d), (a, b, d, c), (b, a, c, d), (b, a, d, c)]
                    .into_iter()
                    .find(|(_, s, t, _)| near(*s, *t));
                if let Some((far1, shared, _, far2)) = joined {
                    let tolerance = 1e-9 * length(far1, far2).max(1e-9);
                    if cross(far1, shared, far2).abs() <= tolerance * length(far1, far2).max(1e-9) {
                        segments[i] = (far1, far2);
                        segments.remove(j);
                        changed = true;
                        break 'outer;
                    }
                }
            }
        }
    }
    segments
}

impl Session {
    /// The triangles of a component in the world.
    fn world_triangles(&self, occurrence: Occurrence) -> Result<Vec<[DVec3; 3]>, EditError> {
        let doc = self.doc();
        match occurrence {
            Occurrence::Item(item) if item.kind == gc_core::Kind::Body => {
                let body = self.shown_body(Id::from_raw(item.raw)).ok_or(EditError::BadValue)?;
                let mesh = match &body.solid {
                    Solid::Box { size } => {
                        gc_mesh::primitives::cuboid(*size).map_err(|_unavailable| EditError::BadValue)?
                    }
                    Solid::Cylinder { radius, length } => gc_mesh::primitives::cylinder(*radius, *length, 48)
                        .map_err(|_unavailable| EditError::BadValue)?,
                    Solid::Sphere { radius } => gc_mesh::primitives::sphere(*radius, 24)
                        .map_err(|_unavailable| EditError::BadValue)?,
                    Solid::Mesh { .. } => {
                        (*self.body_mesh(&body.solid).ok_or(EditError::BadValue)?.data).clone()
                    }
                };
                let pose: DPose3 = gc_math::pose_from_xyz_rpy(body.placement.xyz, body.placement.rpy);
                Ok(mesh.triangles().map(|t| t.map(|v| pose.translation + pose.rotation * v)).collect())
            }
            Occurrence::Item(item) if item.kind == gc_core::Kind::Assembly => {
                let parts = self
                    .assembly_parts(Id::from_raw(item.raw))
                    .map_err(|_unavailable| EditError::BadValue)?;
                Ok(parts.iter().flat_map(|p| transformed(&p.matrix, &p.mesh.data)).collect())
            }
            Occurrence::Part { assembly, index } => {
                if !doc.has_occurrence(occurrence) {
                    return Err(EditError::BadValue);
                }
                let parts = self.assembly_parts(assembly).map_err(|_unavailable| EditError::BadValue)?;
                // The part and the parts under it.
                let a = doc.get(assembly).ok_or(EditError::BadValue)?;
                let under = |mut at: usize| loop {
                    if at == index as usize {
                        return true;
                    }
                    match a.parts.get(at).and_then(|p| p.parent) {
                        Some(parent) => at = parent as usize,
                        None => return false,
                    }
                };
                Ok(parts
                    .iter()
                    .filter(|p| under(p.part))
                    .flat_map(|p| transformed(&p.matrix, &p.mesh.data))
                    .collect())
            }
            _ => Err(EditError::BadValue),
        }
    }

    /// Project Cut Edges: adds to the sketch being edited (a 2D one) the segments where its plane cuts a
    /// body, an imported assembly or a part (and the parts under it), as fixed geometry that does not
    /// follow the component later. Returns how many segments were added: 0 when the plane misses it (the
    /// status bar should say so). One undo step.
    pub fn project_cut_edges(&mut self, occurrence: Occurrence, shift: bool) -> Result<usize, EditError> {
        let sketch = self.active_sketch().map(|(id, _)| id).ok_or(EditError::BadValue)?;
        let s = self.doc().get(sketch).ok_or(EditError::BadValue)?;
        if s.is_spatial() {
            return Err(EditError::BadValue);
        }
        let gc_sketch::Space::Planar(frame) = s.space else { return Err(EditError::BadValue) };
        let frame = self.doc().frame_world_pose(Id::from_raw(frame.raw)).ok_or(EditError::BadValue)?;
        let normal = frame.rotation * DVec3::Z;
        let triangles = self.world_triangles(occurrence)?;
        let to_sketch = |p: DVec3| {
            let local = frame.rotation.inverse() * (p - frame.translation);
            [local.x, local.y]
        };
        let segments: Vec<([f64; 2], [f64; 2])> = slice(triangles.into_iter(), frame.translation, normal)
            .into_iter()
            .map(|(a, b)| (to_sketch(a), to_sketch(b)))
            .collect();
        let segments = merge(segments);
        if segments.is_empty() {
            return Ok(0);
        }
        let count = segments.len();
        let construction = self.tool_construction() != shift;
        self.execute(&Command::Sketch { sketch, op: SketchOp::AddFixedSegments { segments, construction } })?;
        Ok(count)
    }
}

/// A mesh's triangles through a column-major matrix (which may scale).
fn transformed<'a>(
    m: &'a [[f64; 4]; 4],
    mesh: &'a gc_mesh::MeshData,
) -> impl Iterator<Item = [DVec3; 3]> + 'a {
    mesh.triangles().map(move |t| {
        t.map(|p| {
            DVec3::new(
                m[0][0] * p.x + m[1][0] * p.y + m[2][0] * p.z + m[3][0],
                m[0][1] * p.x + m[1][1] * p.y + m[2][1] * p.z + m[3][1],
                m[0][2] * p.x + m[1][2] * p.y + m[2][2] * p.z + m[3][2],
            )
        })
    })
}
