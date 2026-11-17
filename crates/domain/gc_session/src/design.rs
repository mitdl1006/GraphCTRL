//! Where work features are in the world (plan 9.1): planes, axes and points worked out from their
//! definitions, for drawing and for building on them.

use gc_core::Id;
use gc_doc::{Document, OriginPlane, PlaneRef, PointRef, WorkFeature, WorkGeometry};
use gc_math::{DPose3, DQuat, DVec3};
use gc_sketch::Space;

/// A work feature placed in the world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WorkShape {
    /// A plane: its frame's XY plane, normal along the frame's Z axis.
    Plane(DPose3),
    /// An axis: a point on it and its unit direction.
    Axis {
        /// A point on the axis.
        point: DVec3,
        /// Unit direction.
        direction: DVec3,
    },
    /// A point.
    Point(DVec3),
}

/// Work features may build on work features; a chain longer than this is treated as broken.
const MAX_DEPTH: usize = 64;

/// The world shape of a work feature; `None` when its definition no longer works out (a reference is
/// gone, three points on one line, two equal points).
pub(crate) fn work_shape(doc: &Document, id: Id<WorkFeature>) -> Option<WorkShape> {
    shape(doc, id, 0)
}

fn shape(doc: &Document, id: Id<WorkFeature>, depth: usize) -> Option<WorkShape> {
    if depth > MAX_DEPTH {
        return None;
    }
    let feature = doc.get(id)?;
    match &feature.geometry {
        WorkGeometry::PlaneOffset { base, distance } => {
            let base = plane(doc, *base, depth)?;
            let normal = base.rotation * DVec3::Z;
            Some(WorkShape::Plane(DPose3::from_parts(base.translation + normal * *distance, base.rotation)))
        }
        WorkGeometry::PlaneThroughPoints { points } => {
            let [a, b, c] =
                [point(doc, points[0], depth)?, point(doc, points[1], depth)?, point(doc, points[2], depth)?];
            let (u, v) = (b - a, c - a);
            let normal = u.cross(v);
            if normal.length() <= 1e-9 * u.length().max(v.length()).max(1e-9) {
                return None;
            }
            // X along the first edge, Z along the normal.
            let x = u.normalize();
            let z = normal.normalize();
            let rotation = DQuat::from_mat3(&gc_math::DMat3::from_cols(x, z.cross(x), z)).normalize();
            Some(WorkShape::Plane(DPose3::from_parts(a, rotation)))
        }
        WorkGeometry::AxisThroughPoints { points } => {
            let (a, b) = (point(doc, points[0], depth)?, point(doc, points[1], depth)?);
            let d = b - a;
            (d.length() > 1e-12).then(|| WorkShape::Axis { point: a, direction: d.normalize() })
        }
        WorkGeometry::AxisNormalToPlane { plane: base, through } => {
            let base = plane(doc, *base, depth)?;
            Some(WorkShape::Axis { point: point(doc, *through, depth)?, direction: base.rotation * DVec3::Z })
        }
        WorkGeometry::Point { at } => Some(WorkShape::Point(point(doc, *at, depth)?)),
    }
}

fn plane(doc: &Document, plane: PlaneRef, depth: usize) -> Option<DPose3> {
    match plane {
        PlaneRef::Origin(which) => {
            let name = match which {
                OriginPlane::Xy => "XY",
                OriginPlane::Xz => "XZ",
                OriginPlane::Yz => "YZ",
            };
            doc.frame_world_pose(doc.origin_plane(name)?)
        }
        PlaneRef::Work(id) => match shape(doc, id, depth + 1)? {
            WorkShape::Plane(pose) => Some(pose),
            _ => None,
        },
    }
}

fn point(doc: &Document, point: PointRef, depth: usize) -> Option<DVec3> {
    match point {
        PointRef::Origin => Some(DVec3::ZERO),
        PointRef::Fixed(xyz) => Some(DVec3::from_array(xyz)),
        PointRef::SketchPoint { sketch, point } => {
            let sketch = doc.get(sketch)?;
            let (Space::Planar(frame) | Space::Spatial(frame)) = sketch.space;
            let frame = doc.frame_world_pose(Id::from_raw(frame.raw))?;
            Some(frame.translation + frame.rotation * DVec3::from_array(sketch.points.get(&point)?.pos))
        }
        PointRef::Work(id) => match shape(doc, id, depth + 1)? {
            WorkShape::Point(p) => Some(p),
            _ => None,
        },
    }
}
