//! Grip Snap (Inventor 2026 help, Grip Snap move or rotate, GUID-E1CA109A-D5A1-4131-AC3A-AE217716DD8C):
//! pick an edge of a component as the reference line, choose to move along it or to turn about it, click
//! a start point and an end point; the component shows where the snap puts it, and Done (or Commit and
//! Ground) sets it down as one undo step. The HUD says whether the component's remaining freedom allows
//! the chosen snap.

use gc_core::AnyId;
use gc_doc::{GripMotion, GripReference};
use gc_draw::Camera;
use gc_math::{DPose3, DVec3};
use gc_session::{CellItem, Session};

use crate::gizmo;

/// The component being snapped and its reference line.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Target {
    pub(crate) item: CellItem,
    pub(crate) any: AnyId,
    /// What was grabbed: a point, an edge's line or a face's plane.
    pub(crate) reference: GripReference,
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
    /// The end point the cursor gives.
    pub(crate) to: Option<DVec3>,
    /// Where the snap puts the component now.
    pub(crate) pose: Option<DPose3>,
    /// The end point was clicked: the pose stays until Done.
    pub(crate) fixed: bool,
}

impl Grip {
    pub(crate) fn new() -> Self {
        Self { motion: GripMotion::MoveAlong, target: None, from: None, to: None, pose: None, fixed: false }
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

/// The ways to move about a reference (the move options of Inventor's Grip Snap Move Options Bar, GUID-C030C32C-7E1E-4F37-BBDF-120412898D00).
pub(crate) fn motions(reference: &GripReference) -> [GripMotion; 2] {
    match reference {
        GripReference::Point(_) => [GripMotion::Displace, GripMotion::RotateAboutPoint],
        GripReference::Line(_) => [GripMotion::MoveAlong, GripMotion::RotateAbout],
        GripReference::Plane { .. } => [GripMotion::MoveWithinPlane, GripMotion::MoveAlongNormal],
    }
}

/// The point the cursor gives as the end of the motion: along the reference's line or normal the
/// point of a parallel line through the start nearest the cursor's ray (so the component follows it), else
/// the point of the view plane through the start.
pub(crate) fn end_point(
    camera: &Camera,
    size: [f64; 2],
    reference: &GripReference,
    motion: GripMotion,
    from: DVec3,
    at: [f64; 2],
) -> Option<DVec3> {
    let along = |direction: DVec3| {
        let direction = direction.normalize_or_zero();
        gizmo::along_axis(camera, at, size, from, direction).map(|t| from + direction * t)
    };
    match (motion, reference) {
        (GripMotion::MoveAlong, GripReference::Line(line)) => along(DVec3::from(line.direction)),
        (GripMotion::MoveAlongNormal, GripReference::Plane { normal, .. }) => along(DVec3::from(*normal)),
        _ => on_view_plane(camera, size, from, at),
    }
}

/// What an edge, a corner or a flat face picked in the view grabs: a line, a point or a plane.
pub(crate) fn reference_of(feature: &gc_session::Feature) -> Option<GripReference> {
    use gc_session::Feature;
    match feature {
        Feature::Edge(a, b) => gc_doc::GripLine::through(*a, *b).map(GripReference::Line),
        Feature::Point(p) => Some(GripReference::Point(*p)),
        Feature::Face { point, normal, .. } => Some(GripReference::Plane { point: *point, normal: *normal }),
        Feature::Cylinder { .. } | Feature::Cone { .. } => None,
    }
}

/// A point of the reference (where a first click that hits nothing is taken to be).
pub(crate) fn anchor(reference: &GripReference) -> DVec3 {
    match reference {
        GripReference::Point(p) => DVec3::from(*p),
        GripReference::Line(line) => DVec3::from(line.point),
        GripReference::Plane { point, .. } => DVec3::from(*point),
    }
}

/// The name of a move option.
pub(crate) fn motion_name(motion: GripMotion, lang: gc_i18n::Lang) -> &'static str {
    use gc_i18n::tr;
    match motion {
        GripMotion::MoveAlong => tr!(lang, GripMove),
        GripMotion::RotateAbout => tr!(lang, GripRotate),
        GripMotion::Displace => tr!(lang, GripMovePoint),
        GripMotion::RotateAboutPoint => tr!(lang, GripRotatePoint),
        GripMotion::MoveWithinPlane => tr!(lang, GripMovePlane),
        GripMotion::MoveAlongNormal => tr!(lang, GripMoveNormal),
    }
}
