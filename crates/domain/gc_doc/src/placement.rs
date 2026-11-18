//! Where work features and sketch planes are in the world: planes, axes and points worked out from
//! their definitions.

use gc_core::Id;
use gc_math::{DPose3, DQuat, DVec3};
use gc_sketch::Space;

use crate::design::{LineRef, OriginAxis, OriginPlane, PlaneRef, PointRef, WorkFeature, WorkGeometry};
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
        WorkGeometry::AxisOnLine { line } => {
            let (a, b) = line_points(doc, *line)?;
            let d = b - a;
            (d.length() > 1e-12).then(|| WorkShape::Axis { point: a, direction: d.normalize() })
        }
        WorkGeometry::AxisParallelThrough { line, through } => {
            let (a, b) = line_points(doc, *line)?;
            let d = b - a;
            let at = point(doc, *through, depth)?;
            (d.length() > 1e-12).then(|| WorkShape::Axis { point: at, direction: d.normalize() })
        }
        WorkGeometry::AxisPlanesIntersection { planes } => {
            let (p, q) = (plane(doc, planes[0], depth)?, plane(doc, planes[1], depth)?);
            let (n1, n2) = (p.rotation * DVec3::Z, q.rotation * DVec3::Z);
            let direction = n1.cross(n2);
            if direction.length() < 1e-9 {
                return None;
            }
            // The point of the line nearest the first plane's origin: on both planes.
            let (d1, d2) = (n1.dot(p.translation), n2.dot(q.translation));
            let n12 = n1.dot(n2);
            let det = 1.0 - n12 * n12;
            let (c1, c2) = ((d1 - d2 * n12) / det, (d2 - d1 * n12) / det);
            Some(WorkShape::Axis { point: n1 * c1 + n2 * c2, direction: direction.normalize() })
        }
        WorkGeometry::AxisThroughCircle { curve } => {
            let sketch = doc.get(curve.sketch)?;
            let frame = sketch_frame(doc, sketch)?;
            let world = |p: [f64; 3]| frame.translation + frame.rotation * DVec3::from_array(p);
            match sketch.curves.get(&curve.curve)?.geom {
                gc_sketch::CurveGeom::Circle { c, .. } | gc_sketch::CurveGeom::Arc { c, .. }
                    if !sketch.is_spatial() =>
                {
                    Some(WorkShape::Axis {
                        point: world(sketch.pos(c)?),
                        direction: frame.rotation * DVec3::Z,
                    })
                }
                gc_sketch::CurveGeom::Arc { c, s, e } => {
                    let arc = gc_sketch::Arc3::new(sketch.pos(c)?, sketch.pos(s)?, sketch.pos(e)?)?;
                    let normal = DVec3::from_array(arc.u).cross(DVec3::from_array(arc.v));
                    Some(WorkShape::Axis { point: world(arc.centre), direction: frame.rotation * normal })
                }
                // Through Center of Circular or Elliptical Shape.
                gc_sketch::CurveGeom::Ellipse { c, .. } if !sketch.is_spatial() => Some(WorkShape::Axis {
                    point: world(sketch.pos(c)?),
                    direction: frame.rotation * DVec3::Z,
                }),
                _ => None,
            }
        }
        WorkGeometry::Point { at } => Some(WorkShape::Point(point(doc, *at, depth)?)),
        WorkGeometry::PlaneParallelThrough { base, through } => {
            let base = plane(doc, *base, depth)?;
            Some(WorkShape::Plane(DPose3::from_parts(point(doc, *through, depth)?, base.rotation)))
        }
        WorkGeometry::PlaneMidplane { planes } => {
            let (p, q) = (plane(doc, planes[0], depth)?, plane(doc, planes[1], depth)?);
            let (n1, n2) = (p.rotation * DVec3::Z, q.rotation * DVec3::Z);
            if n1.cross(n2).length() > 1e-9 {
                return None;
            }
            let gap = n1.dot(q.translation - p.translation);
            Some(WorkShape::Plane(DPose3::from_parts(p.translation + n1 * (gap / 2.0), p.rotation)))
        }
        WorkGeometry::PlaneAngleToPlane { base, line: about, angle } => {
            let base = plane(doc, *base, depth)?;
            let (a, d) = line(doc, *about, depth)?;
            let n = base.rotation * DVec3::Z;
            if d.dot(n).abs() > 1e-9 {
                return None;
            }
            let z = DQuat::from_axis_angle(d, *angle) * n;
            Some(WorkShape::Plane(frame_of(a, d, z)?))
        }
        WorkGeometry::PlaneLineAndPoint { line: on, through } => {
            let (a, d) = line(doc, *on, depth)?;
            let p = point(doc, *through, depth)?;
            let normal = d.cross(p - a);
            if normal.length() <= 1e-9 * (p - a).length().max(1e-9) {
                return None;
            }
            Some(WorkShape::Plane(frame_of(a, d, normal.normalize())?))
        }
        WorkGeometry::PlaneNormalToLine { line: along, through } => {
            let (a, d) = line(doc, *along, depth)?;
            let p = point(doc, *through, depth)?;
            let foot = a + d * d.dot(p - a);
            let off = p - foot;
            let x = if off.length() > 1e-12 { off.normalize() } else { d.any_orthonormal_vector() };
            Some(WorkShape::Plane(frame_of(foot, x, d)?))
        }
        WorkGeometry::PlaneNormalToCurve { curve, at } => {
            let p = point(doc, *at, depth)?;
            let (tangent, across) = curve_tangent(doc, *curve, p)?;
            Some(WorkShape::Plane(frame_of(p, across, tangent)?))
        }
        WorkGeometry::PointLinesIntersection { lines } => {
            let ((a, d), (b, e)) = (line(doc, lines[0], depth)?, line(doc, lines[1], depth)?);
            let n = d.cross(e);
            if n.length() < 1e-9 {
                return None;
            }
            // The nearest points of the two lines; they must meet.
            let w = a - b;
            let (dd, de, ee, dw, ew) = (d.dot(d), d.dot(e), e.dot(e), d.dot(w), e.dot(w));
            let den = dd * ee - de * de;
            let (s, t) = ((de * ew - ee * dw) / den, (dd * ew - de * dw) / den);
            let (p, q) = (a + d * s, b + e * t);
            ((p - q).length() <= 1e-6).then(|| WorkShape::Point((p + q) / 2.0))
        }
        WorkGeometry::PointPlaneLine { plane: on, line: through } => {
            let base = plane(doc, *on, depth)?;
            let (a, d) = line(doc, *through, depth)?;
            let n = base.rotation * DVec3::Z;
            let along = n.dot(d);
            if along.abs() < 1e-9 {
                return None;
            }
            Some(WorkShape::Point(a + d * (n.dot(base.translation - a) / along)))
        }
        WorkGeometry::PointThreePlanes { planes } => {
            let [p, q, r] =
                [plane(doc, planes[0], depth)?, plane(doc, planes[1], depth)?, plane(doc, planes[2], depth)?];
            let n = [p.rotation * DVec3::Z, q.rotation * DVec3::Z, r.rotation * DVec3::Z];
            let d = [n[0].dot(p.translation), n[1].dot(q.translation), n[2].dot(r.translation)];
            let det = n[0].dot(n[1].cross(n[2]));
            if det.abs() < 1e-9 {
                return None;
            }
            let x = (n[1].cross(n[2]) * d[0] + n[2].cross(n[0]) * d[1] + n[0].cross(n[1]) * d[2]) / det;
            x.is_finite().then_some(WorkShape::Point(x))
        }
        WorkGeometry::PointCurveCentre { curve } => {
            let sketch = doc.get(curve.sketch)?;
            let frame = sketch_frame(doc, sketch)?;
            let world = |p: [f64; 3]| frame.translation + frame.rotation * DVec3::from_array(p);
            match sketch.curves.get(&curve.curve)?.geom {
                gc_sketch::CurveGeom::Circle { c, .. } | gc_sketch::CurveGeom::Ellipse { c, .. } => {
                    Some(WorkShape::Point(world(sketch.pos(c)?)))
                }
                gc_sketch::CurveGeom::Arc { c, s, e } => {
                    if sketch.is_spatial() {
                        let arc = gc_sketch::Arc3::new(sketch.pos(c)?, sketch.pos(s)?, sketch.pos(e)?)?;
                        Some(WorkShape::Point(world(arc.centre)))
                    } else {
                        Some(WorkShape::Point(world(sketch.pos(c)?)))
                    }
                }
                gc_sketch::CurveGeom::Line { .. } | gc_sketch::CurveGeom::Spline { .. } => None,
            }
        }
    }
}

/// A plane's frame at `origin` with X along `x` (made square to `z`) and normal `z`; `None` when they are
/// parallel or zero.
fn frame_of(origin: DVec3, x: DVec3, z: DVec3) -> Option<DPose3> {
    let z = z.try_normalize()?;
    let x = (x - z * x.dot(z)).try_normalize()?;
    let rotation = DQuat::from_mat3(&gc_math::DMat3::from_cols(x, z.cross(x), z)).normalize();
    (origin.is_finite() && rotation.is_finite()).then(|| DPose3::from_parts(origin, rotation))
}

/// A line as a point on it and its unit direction.
fn line(doc: &Document, line: LineRef, depth: usize) -> Option<(DVec3, DVec3)> {
    let unit = |axis: OriginAxis| match axis {
        OriginAxis::X => DVec3::X,
        OriginAxis::Y => DVec3::Y,
        OriginAxis::Z => DVec3::Z,
    };
    match line {
        LineRef::Sketch(curve) => {
            let (a, b) = line_points(doc, curve)?;
            Some((a, (b - a).try_normalize()?))
        }
        LineRef::Work(id) => match shape(doc, id, depth + 1)? {
            WorkShape::Axis { point, direction } => Some((point, direction)),
            WorkShape::Plane(_) | WorkShape::Point(_) => None,
        },
        LineRef::Origin(axis) => Some((DVec3::ZERO, unit(axis))),
        LineRef::Component(occurrence, axis) => {
            let origin = doc.component_origin(occurrence)?;
            Some((origin.point(), origin.pose.rotation * unit(axis)))
        }
    }
}

/// Where a world point lies on a planar sketch's arc, circle, ellipse or spline: the curve's unit
/// tangent there and the sketch's normal (square to it), in the world. `None` when the point is not on
/// the curve (within a micrometre) or the curve is a line.
fn curve_tangent(doc: &Document, curve: crate::design::CurveRef, at: DVec3) -> Option<(DVec3, DVec3)> {
    const ON: f64 = 1e-6;
    let sketch = doc.get(curve.sketch)?;
    if sketch.is_spatial() {
        return None;
    }
    let frame = sketch_frame(doc, sketch)?;
    let local = frame.rotation.inverse() * (at - frame.translation);
    if local.z.abs() > ON {
        return None;
    }
    let p = [local.x, local.y];
    let xy = |q: gc_core::Id<gc_sketch::SkPoint>| sketch.pos(q).map(|v| [v[0], v[1]]);
    let dist = |a: [f64; 2], b: [f64; 2]| gc_math::fm::hypot(a[0] - b[0], a[1] - b[1]);
    let tangent: [f64; 2] = match sketch.curves.get(&curve.curve)?.geom {
        gc_sketch::CurveGeom::Circle { c, .. } | gc_sketch::CurveGeom::Arc { c, .. } => {
            let c = xy(c)?;
            let r = sketch.round(curve.curve)?.1;
            if (dist(c, p) - r).abs() > ON {
                return None;
            }
            [-(p[1] - c[1]), p[0] - c[0]]
        }
        gc_sketch::CurveGeom::Ellipse { c, major, minor, .. } => {
            let (c, m) = (xy(c)?, xy(major)?);
            let a = dist(c, m);
            if !(a > 1e-12 && minor > 1e-12) {
                return None;
            }
            let u = [(m[0] - c[0]) / a, (m[1] - c[1]) / a];
            let t = gc_sketch::ellipse_param(c, m, minor, p);
            let (sin, cos) = gc_math::fm::sin_cos(t);
            let on = [c[0] + a * cos * u[0] - minor * sin * u[1], c[1] + a * cos * u[1] + minor * sin * u[0]];
            if dist(on, p) > ON {
                return None;
            }
            [-a * sin * u[0] - minor * cos * u[1], -a * sin * u[1] + minor * cos * u[0]]
        }
        gc_sketch::CurveGeom::Spline { .. } => {
            let points = sketch.spline_outline(&sketch.curves.get(&curve.curve)?.geom, 64)?;
            let mut best: Option<(f64, [f64; 2])> = None;
            for w in points.windows(2) {
                let d = [w[1][0] - w[0][0], w[1][1] - w[0][1]];
                let l2 = d[0] * d[0] + d[1] * d[1];
                if l2 <= 0.0 {
                    continue;
                }
                let f = (((p[0] - w[0][0]) * d[0] + (p[1] - w[0][1]) * d[1]) / l2).clamp(0.0, 1.0);
                let gap = dist([w[0][0] + f * d[0], w[0][1] + f * d[1]], p);
                if best.is_none_or(|(g, _)| gap < g) {
                    best = Some((gap, d));
                }
            }
            let (gap, d) = best?;
            // The drawn polyline cuts corners by far less than this at its spacing.
            if gap > 1e-4 {
                return None;
            }
            d
        }
        gc_sketch::CurveGeom::Line { .. } => return None,
    };
    let t = frame.rotation * DVec3::new(tangent[0], tangent[1], 0.0);
    Some((t.try_normalize()?, frame.rotation * DVec3::Z))
}

fn sketch_frame(doc: &Document, sketch: &gc_sketch::Sketch) -> Option<DPose3> {
    let (Space::Planar(frame) | Space::Spatial(frame)) = sketch.space;
    doc.frame_world_pose(Id::from_raw(frame.raw))
}

/// A sketch line's ends in the world.
fn line_points(doc: &Document, line: crate::design::CurveRef) -> Option<(DVec3, DVec3)> {
    let sketch = doc.get(line.sketch)?;
    let frame = sketch_frame(doc, sketch)?;
    let (a, b) = sketch.line(line.curve)?;
    let world = |p: [f64; 3]| frame.translation + frame.rotation * DVec3::from_array(p);
    Some((world(sketch.pos(a)?), world(sketch.pos(b)?)))
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
