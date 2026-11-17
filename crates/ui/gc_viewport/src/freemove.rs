//! Free Move and Free Rotate: with the tool on, dragging a component moves it in the plane of the view or
//! turns it about its own origin; the drag is one undo step. Esc ends the tool.
//! Reference: Inventor 2026 help, assembly Free Move / Free Rotate (docs/ux/inventor-assembly-parity.md
//! A12/A13); the origin-based rotation here is simpler than Inventor's rotation symbol.

use gc_draw::Camera;
use gc_math::{DPose3, DQuat, DVec3};
use gc_session::{CellItem, Session};

use crate::gizmo;

/// Which free tool is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FreeTool {
    /// Move in the plane of the view.
    Move,
    /// Turn about the component's origin.
    Rotate,
}

/// A drag of a component in progress.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FreeDrag {
    item: CellItem,
    start: DPose3,
    /// Where the press hit the plane of the view through the component's origin.
    anchor: DVec3,
    /// The pointer when the drag began, in view pixels.
    from: [f64; 2],
}

/// Starts a drag of `item` pressed at `at` (view pixels).
pub(crate) fn begin(
    camera: &Camera,
    size: [f64; 2],
    session: &Session,
    item: CellItem,
    at: [f64; 2],
) -> Option<FreeDrag> {
    if !matches!(item, CellItem::Body(_) | CellItem::Robot(_)) {
        return None;
    }
    let (start, _) = gizmo::pose_of(session, item)?;
    let anchor = on_view_plane(camera, size, start.translation, at)?;
    Some(FreeDrag { item, start, anchor, from: at })
}

/// Where the ray through `at` meets the plane through `point` square to the view direction.
fn on_view_plane(camera: &Camera, size: [f64; 2], point: DVec3, at: [f64; 2]) -> Option<DVec3> {
    let ray = camera.ray(at, size);
    let forward = camera.forward();
    let denom = forward.dot(ray.dir);
    if denom.abs() < 1e-9 {
        return None;
    }
    let t = forward.dot(point - ray.origin) / denom;
    Some(ray.origin + ray.dir * t)
}

/// Carries the drag to the pointer at `to`, as a preview.
pub(crate) fn update(
    drag: &FreeDrag,
    tool: FreeTool,
    camera: &Camera,
    size: [f64; 2],
    session: &mut Session,
    to: [f64; 2],
) {
    let pose = match tool {
        FreeTool::Move => {
            let Some(now) = on_view_plane(camera, size, drag.start.translation, to) else { return };
            DPose3::from_parts(drag.start.translation + (now - drag.anchor), drag.start.rotation)
        }
        FreeTool::Rotate => {
            const PER_PIXEL: f64 = 0.01;
            let (dx, dy) = (to[0] - drag.from[0], to[1] - drag.from[1]);
            let yaw = DQuat::from_axis_angle(camera.up(), dx * PER_PIXEL);
            let pitch = DQuat::from_axis_angle(camera.right(), dy * PER_PIXEL);
            DPose3::from_parts(drag.start.translation, (yaw * pitch * drag.start.rotation).normalize())
        }
    };
    gizmo::preview(session, drag.item, &pose);
}

/// Ends the drag: one undo step.
pub(crate) fn end(drag: &FreeDrag, session: &mut Session) {
    gizmo::commit(session, drag.item);
}
