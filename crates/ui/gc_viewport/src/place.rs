//! Placing a component (Inventor's Place Component): the component hangs on the cursor over the ground
//! plane; each click sets it down and attaches another copy, Esc or a right click ends and drops the copy
//! that was still on the cursor.
//! Reference: Inventor 2026 help, Place Components (GUID-2E0B9110-BBAE-482B-A9AF-953BC40E8F2A): the first
//! component placed is grounded at the origin (the backend does that); every other one is placed
//! unconstrained and ungrounded.

use gc_draw::Camera;
use gc_math::{DPose3, DVec3};
use gc_session::{CellItem, Session};

use crate::gizmo;

/// The component on the cursor.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Placing {
    pub(crate) item: CellItem,
    /// A copy made while placing: it is dropped when placing ends without a click.
    pub(crate) copy: bool,
}

/// Where the ray through `at` meets the ground plane (z = 0); when the view looks along the ground, the
/// plane through `fallback` square to the view direction.
pub(crate) fn ground_hit(camera: &Camera, size: [f64; 2], at: [f64; 2], fallback: DVec3) -> Option<DVec3> {
    let ray = camera.ray(at, size);
    if ray.dir.z.abs() > 1e-3 {
        let t = -ray.origin.z / ray.dir.z;
        if t > 0.0 {
            return Some(ray.origin + ray.dir * t);
        }
    }
    let forward = camera.forward();
    let denom = forward.dot(ray.dir);
    if denom.abs() < 1e-9 {
        return None;
    }
    let t = forward.dot(fallback - ray.origin) / denom;
    Some(ray.origin + ray.dir * t)
}

/// Hangs the component on the cursor at `at` (view pixels): its origin goes to the ground point.
pub(crate) fn follow(
    placing: &Placing,
    camera: &Camera,
    size: [f64; 2],
    session: &mut Session,
    at: [f64; 2],
) {
    let Some((pose, _)) = gizmo::pose_of(session, placing.item) else { return };
    let Some(point) = ground_hit(camera, size, at, pose.translation) else { return };
    gizmo::preview(session, placing.item, &DPose3::from_parts(point, pose.rotation));
}

/// Sets the component down where it hangs: one undo step.
pub(crate) fn set_down(placing: &Placing, session: &mut Session) {
    gizmo::commit(session, placing.item);
}
