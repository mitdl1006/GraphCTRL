//! Project Cut Edges, as in Inventor (Sketch tab > Create panel > Project Cut Edges; Inventor 2026 help,
//! "To Project Geometry into a 2D Sketch",
//! https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-B8D8B07B-86F2-4CB7-AB21-3131D94AD1CC.htm):
//! "click a part that a section plane cuts. All edges that the plane cuts are projected onto the sketch
//! plane." Here the part is the selected component; with nothing selected, every component is cut. The
//! edges become fixed lines of the sketch, as a snapshot (Inventor's cut edges of parts do not follow
//! later changes either). When the plane cuts nothing, nothing is added and the caller says so.

use gc_doc::{Command, SketchOp};
use gc_math::DVec3;
use gc_session::Session;
use gc_sketch::Space;

/// Adds the cut edges to the active 2D sketch, as one undo step. Returns how many lines were added, or
/// why none were.
pub(crate) fn project_cut_edges(session: &mut Session) -> Result<usize, CutError> {
    let (sketch, frame) = {
        let (id, sketch) = session.active_sketch().ok_or(CutError::NoSketch)?;
        let Space::Planar(frame) = sketch.space else { return Err(CutError::NoSketch) };
        (id, frame)
    };
    let frame = session.doc().frame_world_pose(gc_core::Id::from_raw(frame.raw)).ok_or(CutError::NoSketch)?;
    let normal = frame.rotation * DVec3::Z;
    let scene = gc_scene::Scene::of(session);
    let mut segments = Vec::new();
    let chosen = session.selected_item();
    for solid in scene.solids.iter().filter(|s| s.owner.is_some() && (chosen.is_none() || s.owner == chosen))
    {
        let Ok(mesh) = solid.shape.mesh() else { continue };
        segments.extend(gc_mesh::slice::plane_cut(&mesh, &solid.pose, frame.translation, normal));
    }
    let to_sketch = frame.inverse();
    let lines: Vec<([f64; 3], [f64; 3])> = gc_mesh::slice::join_straight(&segments, 1e-6)
        .into_iter()
        .map(|[a, b]| {
            let flat = |p: DVec3| {
                let local = to_sketch * p;
                [local.x, local.y, 0.0]
            };
            (flat(a), flat(b))
        })
        .collect();
    if lines.is_empty() {
        return Err(CutError::NothingCut);
    }
    let count = lines.len();
    session
        .execute(&Command::Sketch { sketch, op: SketchOp::ProjectedLines { lines } })
        .map_err(|e| CutError::Refused(e.to_string()))?;
    Ok(count)
}

/// Why no cut edges were added.
#[derive(Debug)]
pub(crate) enum CutError {
    /// No 2D sketch is being edited.
    NoSketch,
    /// The sketch plane cuts nothing of the model.
    NothingCut,
    /// The sketch refused the lines.
    Refused(String),
}
