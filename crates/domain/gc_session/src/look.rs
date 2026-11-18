//! Look At (Inventor 2026 help, Navigation Bar, GUID-3B926737-92BA-46CF-B722-97F68651772B): "zooms and
//! rotates the model to display the selected element planar to the screen or a selected edge or line
//! horizontal to the screen". The view itself belongs to the viewport; this works out where it should look.

use gc_core::Id;
use gc_doc::{Geometry, Occurrence, OriginPlane, Selection, Sketch, WorkFeature, WorkShape};
use gc_math::{DPose3, DVec3};
use gc_sketch::Space;

use crate::Session;

/// What Look At was used on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LookItem {
    /// A work plane or axis.
    WorkFeature(Id<WorkFeature>),
    /// A sketch: its plane.
    Sketch(Id<Sketch>),
    /// A line or other curve of a sketch: a line lies level on the screen, a curve's sketch plane faces it.
    SketchCurve {
        /// The sketch.
        sketch: Id<Sketch>,
        /// The curve.
        curve: Id<gc_sketch::Curve>,
    },
    /// An origin plane of a component (or of the assembly's own origin, with the document's origin
    /// planes).
    ComponentPlane(Occurrence, OriginPlane),
    /// Picked geometry on a component: a planar face faces the screen, a straight edge lies level, a
    /// circular edge's plane faces the screen.
    Face(Selection),
}

/// Where the view should turn to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LookTarget {
    /// The direction from the target towards the eye (unit): the view looks along its opposite.
    pub toward_eye: DVec3,
    /// The direction that is up on the screen (unit, square to `toward_eye`).
    pub up: DVec3,
    /// The point to centre the view on.
    pub centre: DVec3,
    /// About how far the element reaches from `centre`, metres, for the zoom (`None`: keep the zoom).
    pub reach: Option<f64>,
}

impl Session {
    /// Where Look At on `item` turns the view, from a view looking along `forward` now: a plane faces the
    /// screen from the side the eye is on, with its own up kept; a line lies level on the screen, seen from
    /// as close to the present direction as can be. `None` for an item that does not exist or has no plane
    /// or line to look at (a point).
    pub fn look_target(&self, item: LookItem, forward: DVec3) -> Option<LookTarget> {
        let doc = self.doc();
        let eye = -forward.try_normalize()?;
        match item {
            LookItem::WorkFeature(id) => match doc.work_shape(id)? {
                WorkShape::Plane(pose) => plane(&pose, eye, None),
                WorkShape::Axis { point, direction } => line(point, direction, eye, None),
                WorkShape::Point(_) => None,
            },
            LookItem::Sketch(id) => {
                let sketch = doc.get(id)?;
                let pose = sketch_pose(self, sketch)?;
                if sketch.is_spatial() {
                    return None;
                }
                let (centre, reach) = sketch_extent(sketch, &pose).unzip();
                let mut target = plane(&pose, eye, reach)?;
                if let Some(centre) = centre {
                    target.centre = centre;
                }
                Some(target)
            }
            LookItem::SketchCurve { sketch, curve } => {
                let sketch = doc.get(sketch)?;
                let pose = sketch_pose(self, sketch)?;
                let world = |p: [f64; 3]| pose.translation + pose.rotation * DVec3::from_array(p);
                if let Some((a, b)) = sketch.line(curve) {
                    let (a, b) = (world(sketch.pos(a)?), world(sketch.pos(b)?));
                    let half = (b - a).length() / 2.0;
                    // A line of a 2D sketch is seen from straight above the sketch, lying level.
                    let mut target = line((a + b) / 2.0, b - a, eye, Some(half))?;
                    if !sketch.is_spatial() {
                        let normal = pose.rotation * DVec3::Z;
                        let toward_eye = if normal.dot(eye) >= 0.0 { normal } else { -normal };
                        let along = (b - a).try_normalize()?;
                        target.toward_eye = toward_eye;
                        target.up = toward_eye.cross(along).try_normalize()?;
                    }
                    return Some(target);
                }
                if sketch.is_spatial() || !sketch.curves.contains_key(&curve) {
                    return None;
                }
                plane(&pose, eye, None)
            }
            LookItem::ComponentPlane(occurrence, which) => {
                let origin = doc.component_origin(occurrence)?;
                plane(&origin.plane(which), eye, None)
            }
            LookItem::Face(selection) => {
                let pose = doc.component_origin(selection.occurrence)?.pose;
                let at = |p: [f64; 3]| pose.translation + pose.rotation * DVec3::from_array(p);
                let turn = |d: [f64; 3]| pose.rotation * DVec3::from_array(d);
                match selection.geometry {
                    Geometry::Plane { point, normal } => face(at(point), turn(normal), eye, None),
                    Geometry::Circle { centre, normal } => face(at(centre), turn(normal), eye, None),
                    Geometry::Axis { point, direction } => line(at(point), turn(direction), eye, None),
                    Geometry::Cylinder { point, direction, radius } => {
                        line(at(point), turn(direction), eye, Some(radius))
                    }
                    Geometry::Frame { origin, z, .. } => face(at(origin), turn(z), eye, None),
                    Geometry::Point(_) | Geometry::Sphere { .. } => None,
                }
            }
        }
    }
}

fn sketch_pose(session: &Session, sketch: &Sketch) -> Option<DPose3> {
    let (Space::Planar(frame) | Space::Spatial(frame)) = sketch.space;
    session.doc().frame_world_pose(Id::from_raw(frame.raw))
}

/// The middle and half the diagonal of a sketch's points, in the world; `None` without points.
fn sketch_extent(sketch: &Sketch, pose: &DPose3) -> Option<(DVec3, f64)> {
    let mut points = sketch.points.values().map(|p| DVec3::from_array(p.pos));
    let first = points.next()?;
    let (lo, hi) = points.fold((first, first), |(lo, hi), p| (lo.min(p), hi.max(p)));
    let middle = (lo + hi) / 2.0;
    Some((pose.translation + pose.rotation * middle, ((hi - lo).length() / 2.0).max(1e-3)))
}

/// A plane given as a frame (its XY plane, normal Z) seen face on from the eye's side, its Y up.
fn plane(pose: &DPose3, eye: DVec3, reach: Option<f64>) -> Option<LookTarget> {
    let normal = pose.rotation * DVec3::Z;
    let toward_eye = if normal.dot(eye) >= 0.0 { normal } else { -normal };
    let up = (pose.rotation * DVec3::Y).try_normalize()?;
    Some(LookTarget { toward_eye, up, centre: pose.translation, reach })
}

/// A plane given by a point and a normal, seen face on from the eye's side, with up as near the present
/// eye's up (world Z, else Y) as the plane allows.
fn face(point: DVec3, normal: DVec3, eye: DVec3, reach: Option<f64>) -> Option<LookTarget> {
    let normal = normal.try_normalize()?;
    let toward_eye = if normal.dot(eye) >= 0.0 { normal } else { -normal };
    let up = [DVec3::Z, DVec3::Y]
        .into_iter()
        .find_map(|u| (u - toward_eye * u.dot(toward_eye)).try_normalize())?;
    Some(LookTarget { toward_eye, up, centre: point, reach })
}

/// A line lying level on the screen, seen from as close to the eye's present direction as can be (square
/// to the line).
fn line(point: DVec3, direction: DVec3, eye: DVec3, reach: Option<f64>) -> Option<LookTarget> {
    let along = direction.try_normalize()?;
    let toward_eye =
        (eye - along * eye.dot(along)).try_normalize().unwrap_or_else(|| along.any_orthonormal_vector());
    let up = toward_eye.cross(along).try_normalize()?;
    Some(LookTarget { toward_eye, up, centre: point, reach })
}
