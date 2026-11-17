//! Where work features and sketch planes are in the world: planes, axes and points worked out from
//! their definitions.

use gc_core::Id;
use gc_math::{DPose3, DQuat, DVec3};
use gc_sketch::Space;

use crate::design::{OriginPlane, PlaneRef, PointRef, WorkFeature, WorkGeometry};
use crate::document::Document;

impl Document {
    /// The world pose of a sketch plane's source (its XY plane is the sketch plane); `None` when the
    /// source no longer works out.
    pub fn plane_source_pose(&self, source: &crate::design::PlaneSource) -> Option<DPose3> {
        match source {
            crate::design::PlaneSource::Work(id) => match self.work_shape(*id)? {
                WorkShape::Plane(pose) => Some(pose),
                _ => None,
            },
            crate::design::PlaneSource::Component { occurrence, plane } => {
                self.component_origin(*occurrence).map(|o| o.plane(*plane))
            }
            crate::design::PlaneSource::Face { assembly, part, xyz, quat_xyzw, .. } => {
                let assembly = self.get(*assembly)?;
                let m = assembly.part_matrix(*part as usize)?;
                let placement = gc_math::pose_from_xyz_rpy(assembly.placement.xyz, assembly.placement.rpy);
                let [qx, qy, qz, qw] = *quat_xyzw;
                let local =
                    DPose3::from_parts(DVec3::from_array(*xyz), DQuat::from_xyzw(qx, qy, qz, qw).normalize());
                // Through the part's matrix (which may scale): origin and two axes, made square again.
                let apply = |p: DVec3, w: f64| {
                    DVec3::new(
                        m[0][0] * p.x + m[1][0] * p.y + m[2][0] * p.z + m[3][0] * w,
                        m[0][1] * p.x + m[1][1] * p.y + m[2][1] * p.z + m[3][1] * w,
                        m[0][2] * p.x + m[1][2] * p.y + m[2][2] * p.z + m[3][2] * w,
                    )
                };
                let origin = apply(local.translation, 1.0);
                let x = apply(local.rotation * DVec3::X, 0.0);
                let y = apply(local.rotation * DVec3::Y, 0.0);
                let z = x.cross(y);
                if x.length() < 1e-12 || z.length() < 1e-12 {
                    return None;
                }
                let (x, z) = (x.normalize(), z.normalize());
                let rotation = DQuat::from_mat3(&gc_math::DMat3::from_cols(x, z.cross(x), z)).normalize();
                Some(placement * DPose3::from_parts(origin, rotation))
            }
        }
    }

    /// Where a work feature is in the world; `None` when its definition no longer works out (a
    /// reference is gone, three points on one line, two equal points).
    pub fn work_shape(&self, id: Id<WorkFeature>) -> Option<WorkShape> {
        shape(self, id, 0)
    }
}

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
        PlaneRef::Component(occurrence, which) => Some(doc.component_origin(occurrence)?.plane(which)),
    }
}

fn point(doc: &Document, point: PointRef, depth: usize) -> Option<DVec3> {
    match point {
        PointRef::Origin => Some(DVec3::ZERO),
        PointRef::Component(occurrence) => Some(doc.component_origin(occurrence)?.point()),
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

/// A component's origin folder in the world (docs/ux/inventor-assembly-parity.md, P1-2): three planes,
/// three axes and the centre point, all from the component's frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComponentOrigin {
    /// The component's frame in the world.
    pub pose: DPose3,
}

impl ComponentOrigin {
    /// The centre point.
    pub fn point(&self) -> DVec3 {
        self.pose.translation
    }

    /// An axis (0 = X, 1 = Y, 2 = Z) as a unit direction through [`ComponentOrigin::point`].
    pub fn axis(&self, axis: usize) -> DVec3 {
        let local = match axis {
            0 => DVec3::X,
            1 => DVec3::Y,
            _ => DVec3::Z,
        };
        self.pose.rotation * local
    }

    /// An origin plane as a frame whose XY plane it is, oriented like the document's origin plane of the
    /// same name (XY: normal +Z; XZ: normal −Y; YZ: normal +X).
    pub fn plane(&self, plane: OriginPlane) -> DPose3 {
        self.pose * DPose3::from_parts(DVec3::ZERO, origin_plane_rotation(plane))
    }
}

/// The rotation of a document origin plane's frame (as the document creates them).
fn origin_plane_rotation(plane: OriginPlane) -> DQuat {
    let (x, y) = match plane {
        OriginPlane::Xy => (DVec3::X, DVec3::Y),
        OriginPlane::Xz => (DVec3::X, DVec3::Z),
        OriginPlane::Yz => (DVec3::Y, DVec3::Z),
    };
    DQuat::from_mat3(&gc_math::DMat3::from_cols(x, y, x.cross(y))).normalize()
}

impl Document {
    /// The origin folder of a component: where its frame is in the world. A body's, robot's (its base) or
    /// imported assembly's frame is its placement; a part's is its frame in the file (made square when the
    /// file scales it) under the assembly's placement. `None` when the component does not exist.
    pub fn component_origin(&self, occurrence: crate::design::Occurrence) -> Option<ComponentOrigin> {
        use crate::design::Occurrence;
        use gc_core::Kind;
        let placed = |p: &crate::cell::Placement| gc_math::pose_from_xyz_rpy(p.xyz, p.rpy);
        let pose = match occurrence {
            Occurrence::Item(item) => match item.kind {
                Kind::Body => placed(&self.get(Id::<crate::cell::Body>::from_raw(item.raw))?.placement),
                Kind::RobotInstance => {
                    placed(&self.get(Id::<crate::cell::Robot>::from_raw(item.raw))?.placement)
                }
                Kind::Assembly => {
                    placed(&self.get(Id::<crate::design::Assembly>::from_raw(item.raw))?.placement)
                }
                _ => return None,
            },
            Occurrence::Part { assembly, index } => {
                let a = self.get(assembly)?;
                let m = a.part_matrix(index as usize)?;
                let column = |c: usize| DVec3::new(m[c][0], m[c][1], m[c][2]);
                let (x, y) = (column(0), column(1));
                let z = x.cross(y);
                if x.length() < 1e-12 || z.length() < 1e-12 {
                    return None;
                }
                let (x, z) = (x.normalize(), z.normalize());
                let rotation = DQuat::from_mat3(&gc_math::DMat3::from_cols(x, z.cross(x), z)).normalize();
                placed(&a.placement) * DPose3::from_parts(column(3), rotation)
            }
            Occurrence::Link { robot, index } => {
                let r = self.get(robot)?;
                placed(&r.placement) * *r.link_frames().get(index as usize)?
            }
            Occurrence::Tool { robot } => {
                // The flange (tip link) frame, then the tool's placement on it.
                let r = self.get(robot)?;
                let tool = r.tool.as_ref()?;
                let tip = r.links.iter().position(|l| l.name == r.tip_link)?;
                placed(&r.placement) * *r.link_frames().get(tip)? * placed(&tool.placement)
            }
        };
        Some(ComponentOrigin { pose })
    }
}
