//! Grip Snap (Inventor 2026 help, Grip Snap move or rotate, GUID-E1CA109A-D5A1-4131-AC3A-AE217716DD8C):
//! pick an edge of a component as the reference line, choose to move along it or to turn about it, click
//! a start point and an end point; the component shows where the snap puts it, and Done (or Commit and
//! Ground) sets it down as one undo step. The HUD says whether the component's remaining freedom allows
//! the chosen snap.

use gc_core::AnyId;
use gc_doc::{GripLine, GripMotion};
use gc_draw::Camera;
use gc_math::{DPose3, DVec3};
use gc_session::{CellItem, Session};

use crate::gizmo;

/// The component being snapped and its reference line.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Target {
    pub(crate) item: CellItem,
    pub(crate) any: AnyId,
    pub(crate) line: GripLine,
    /// Where the component was when the edge was picked.
    pub(crate) start: DPose3,
}

/// A Grip Snap in progress.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Grip {
    pub(crate) motion: GripMotion,
    pub(crate) target: Option<Target>,
    /// The first point (where the motion is measured from).
    pub(crate) from: Option<DVec3>,
    /// Where the snap puts the component now.
    pub(crate) pose: Option<DPose3>,
    /// The end point was clicked: the pose stays until Done.
    pub(crate) fixed: bool,
}

impl Grip {
    pub(crate) fn new() -> Self {
        Self { motion: GripMotion::MoveAlong, target: None, from: None, pose: None, fixed: false }
    }
}

/// The component an edge pick belongs to (a body, a robot or an imported assembly), as an id.
pub(crate) fn component_of(item: CellItem) -> Option<AnyId> {
    match item {
        CellItem::Body(id) => Some(id.erase()),
        CellItem::Robot(id) => Some(id.erase()),
        _ => None,
    }
}

/// Where the ray through `at` meets the plane through `point` square to the view direction.
pub(crate) fn on_view_plane(camera: &Camera, size: [f64; 2], point: DVec3, at: [f64; 2]) -> Option<DVec3> {
    let ray = camera.ray(at, size);
    let forward = camera.forward();
    let denom = forward.dot(ray.dir);
    if denom.abs() < 1e-9 {
        return None;
    }
    let t = forward.dot(point - ray.origin) / denom;
    Some(ray.origin + ray.dir * t)
}

/// Shows the snapped pose (or, with `None`, puts the component back where it was) as a preview.
pub(crate) fn show(session: &mut Session, target: &Target, pose: Option<&DPose3>) {
    gizmo::preview(session, target.item, pose.unwrap_or(&target.start));
    if pose.is_none() {
        // Back at the start: nothing changed, so this ends the preview without an undo step.
        gizmo::commit(session, target.item);
    }
}

/// Ends the snap without a change.
pub(crate) fn cancel(session: &mut Session, grip: &Grip) {
    if let Some(target) = &grip.target {
        gizmo::preview(session, target.item, &target.start);
        gizmo::commit(session, target.item);
    }
}
