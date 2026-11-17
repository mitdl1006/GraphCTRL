//! Assembly constraints between components (docs/ux/inventor-assembly-parity.md, P1-9): Mate, Flush and
//! Insert, solved by moving the components that are not grounded.
//!
//! Inventor 2026 help: To Place Mate or Flush Constraints in Assemblies
//! (GUID-0F500089-4F69-41C5-AFE8-B58AC9E8E15E), To Place Insert Constraints in Assemblies
//! (GUID-97DD5557-FAEE-46FA-845D-11123D725BA9), Place Constraint Reference - Assembly Tab
//! (GUID-EBD2C784-0E71-452A-B5F2-22738D22D45A).
//!
//! What moves is the component that owns the selected geometry: a body, a robot or an imported assembly
//! (a part moves with its assembly). Grounded components stay where they are. All relationships are solved
//! together; among the positions that satisfy them the solver keeps the components close to where they were.

use gc_core::{AnyId, Entity, Id, Kind};
use gc_math::{DPose3, DQuat, DVec3};
use serde::{Deserialize, Serialize};

use crate::design::{ComponentFlag, Occurrence, OriginElement, OriginPlane};
use crate::document::Document;

/// Geometry on a component, in the component's own frame (the frame of its origin folder).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Geometry {
    /// A plane (a planar face or an origin or work plane): a point on it and its normal (the side the
    /// face looks to).
    Plane {
        /// A point on the plane, metres.
        point: [f64; 3],
        /// The normal.
        normal: [f64; 3],
    },
    /// An axis (a straight edge, a cylinder's axis, an origin axis).
    Axis {
        /// A point on the axis.
        point: [f64; 3],
        /// Its direction.
        direction: [f64; 3],
    },
    /// A point (a vertex, a centre point).
    Point([f64; 3]),
    /// A circular edge: its centre and the normal of its plane (the side the face looks to). Insert uses
    /// both; Mate uses its axis.
    Circle {
        /// Centre.
        centre: [f64; 3],
        /// Normal of the circle's plane.
        normal: [f64; 3],
    },
    /// A cylindrical face: a point on its axis, the axis direction, the radius.
    Cylinder {
        /// A point on the axis.
        point: [f64; 3],
        /// The axis direction.
        direction: [f64; 3],
        /// Radius, metres.
        radius: f64,
    },
    /// A spherical face: centre and radius.
    Sphere {
        /// Centre.
        centre: [f64; 3],
        /// Radius, metres.
        radius: f64,
    },
    /// A joint origin: a point with a Z axis (out of the face it sits on) and an X axis.
    Frame {
        /// The point.
        origin: [f64; 3],
        /// Z axis.
        z: [f64; 3],
        /// X axis (made perpendicular to Z).
        x: [f64; 3],
    },
}

impl Geometry {
    /// An element of the component's origin folder as geometry. Planes face like the document's origin
    /// planes (XY +Z, XZ −Y, YZ +X).
    pub fn origin(element: OriginElement) -> Self {
        let zero = [0.0; 3];
        match element {
            OriginElement::Plane(OriginPlane::Xy) => Geometry::Plane { point: zero, normal: [0.0, 0.0, 1.0] },
            OriginElement::Plane(OriginPlane::Xz) => {
                Geometry::Plane { point: zero, normal: [0.0, -1.0, 0.0] }
            }
            OriginElement::Plane(OriginPlane::Yz) => Geometry::Plane { point: zero, normal: [1.0, 0.0, 0.0] },
            OriginElement::Axis(axis) => {
                let mut direction = [0.0; 3];
                direction[usize::from(axis.min(2))] = 1.0;
                Geometry::Axis { point: zero, direction }
            }
            OriginElement::Point => Geometry::Point(zero),
        }
    }

    /// Where a component must stand (its pose in a frame) so that this joint origin on it meets that
    /// frame's own origin by a rigid joint: Z axes opposed (`flip`: the same way), X axes alike. Used to
    /// mount a tool on a robot's flange.
    pub fn rigid_mount(&self, flip: bool) -> DPose3 {
        let (origin, z, x) = self.joint_frame();
        let y = z.cross(x);
        let local =
            DPose3::from_parts(origin, DQuat::from_mat3(&gc_math::DMat3::from_cols(x, y, z)).normalize());
        let target =
            if flip { DQuat::IDENTITY } else { DQuat::from_axis_angle(DVec3::X, std::f64::consts::PI) };
        DPose3::from_parts(DVec3::ZERO, target) * local.inverse()
    }

    /// The origin of the component itself as a joint origin: its centre point with its Z and X axes.
    pub fn origin_frame() -> Self {
        Geometry::Frame { origin: [0.0; 3], z: [0.0, 0.0, 1.0], x: [1.0, 0.0, 0.0] }
    }

    /// The geometry as a joint origin in the component's frame: point, unit Z, unit X. A plane or circle
    /// takes Z along its normal, an axis along its direction, a point the component's Z; X is then the
    /// component's X (or Y) made perpendicular.
    fn joint_frame(&self) -> (DVec3, DVec3, DVec3) {
        let v = DVec3::from_array;
        let (origin, z, x) = match *self {
            Geometry::Frame { origin, z, x } => (v(origin), v(z), Some(v(x))),
            Geometry::Plane { point, normal } => (v(point), v(normal), None),
            Geometry::Circle { centre, normal } => (v(centre), v(normal), None),
            Geometry::Axis { point, direction } => (v(point), v(direction), None),
            Geometry::Point(point) | Geometry::Sphere { centre: point, .. } => {
                (v(point), DVec3::Z, Some(DVec3::X))
            }
            Geometry::Cylinder { point, direction, .. } => (v(point), v(direction), None),
        };
        let z = z.normalize_or_zero();
        let helper = x.unwrap_or(if z.x.abs() < 0.9 { DVec3::X } else { DVec3::Y });
        let x = (helper - z * helper.dot(z)).normalize_or_zero();
        (origin, z, x)
    }

    fn is_finite(&self) -> bool {
        let (a, b) = match *self {
            Geometry::Plane { point, normal } => (point, normal),
            Geometry::Axis { point, direction } => (point, direction),
            Geometry::Point(p) => (p, [1.0, 0.0, 0.0]),
            Geometry::Circle { centre, normal } => (centre, normal),
            Geometry::Cylinder { point, direction, radius } => {
                if !(radius.is_finite() && radius > 0.0) {
                    return false;
                }
                (point, direction)
            }
            Geometry::Sphere { centre, radius } => {
                if !(radius.is_finite() && radius > 0.0) {
                    return false;
                }
                (centre, [1.0, 0.0, 0.0])
            }
            Geometry::Frame { origin, z, x } => {
                let cross = DVec3::from_array(z).cross(DVec3::from_array(x));
                if !x.iter().all(|v| v.is_finite()) || cross.length() < 1e-9 {
                    return false;
                }
                (origin, z)
            }
        };
        let direction = DVec3::from_array(b);
        a.iter().chain(&b).all(|v| v.is_finite()) && direction.length() > 1e-12
    }
}

/// Selected geometry on a component.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Selection {
    /// The component (a part counts as its assembly when it comes to moving).
    pub occurrence: Occurrence,
    /// The geometry, in the occurrence's frame.
    pub geometry: Geometry,
}

/// Which way two axes point after a mate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AxisSense {
    /// Opposite directions (Inventor's default).
    Opposed,
    /// The same direction.
    Aligned,
    /// Whichever is closer to how they lie when the constraint is placed.
    Undirected,
}

/// The kind of relationship.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum RelationKind {
    /// Faces face to face (normals opposed) at `offset` apart, or curves, axes and points coincident.
    Mate {
        /// Distance between the planes, metres (positive: a gap).
        offset: f64,
        /// For two axes: which way they point.
        sense: AxisSense,
    },
    /// Faces side by side, normals pointing the same way, `offset` apart.
    Flush {
        /// Distance, metres.
        offset: f64,
    },
    /// A rigid joint: the two joint origins coincide with their Z axes opposed (`flip`: the same way) and
    /// their X axes alike. Removes all freedom between the components.
    Rigid {
        /// Flip component: Z axes the same way.
        flip: bool,
    },
    /// A rotational joint: the origins coincide and the Z axes line up (opposed unless `flip`); turning
    /// about Z stays free.
    Rotational {
        /// Flip component: Z axes the same way.
        flip: bool,
    },
    /// A slider joint: Z axes on one line (opposed unless `flip`), X axes alike; sliding along Z stays free.
    Slider {
        /// Flip component.
        flip: bool,
    },
    /// A cylindrical joint: Z axes on one line (opposed unless `flip`); sliding along and turning about Z
    /// stay free.
    Cylindrical {
        /// Flip component.
        flip: bool,
    },
    /// A ball joint: the origins coincide; all turning stays free.
    Ball,
    /// An angle between two planes' normals or two edges' directions, radians.
    Angle {
        /// The angle.
        angle: f64,
        /// How its sense is read.
        solution: AngleSolution,
    },
    /// Faces touching: a plane, cylinder or sphere against another, `offset` apart, inside or outside.
    Tangent {
        /// Gap, metres.
        offset: f64,
        /// The first inside the second (for a plane: on the side its normal points away from).
        inside: bool,
    },
    /// The two selections mirror each other across a plane.
    Symmetry {
        /// The symmetry plane (a plane on any component).
        plane: Selection,
    },
    /// A circular edge into another: axes coincident and the circles' planes mated at `offset`. Rotation
    /// about the axis stays free.
    Insert {
        /// Distance between the circles' planes, metres.
        offset: f64,
        /// False (Opposed, the default): normals opposite, like a bolt head on the face around a hole.
        aligned: bool,
    },
}

/// How an angle constraint reads its sense (Inventor 2026 help, To Place Angle Constraints in Assemblies,
/// GUID-AD4679FD-EE9B-47C0-9E32-04B990BDD29E).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum AngleSolution {
    /// The right-hand rule always applies, about the axis inferred from how the selections lie.
    Directed,
    /// Either sense: only the size of the angle counts.
    Undirected,
    /// The axis of the right-hand rule is given by a third selection (a face, edge, work plane or axis).
    Explicit(Selection),
}

/// An assembly constraint between two components. The first selection is on the component that moves by
/// default; the second on the one that stays.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Relationship {
    /// Name shown in the Relationships folder.
    pub name: String,
    /// What it does.
    pub kind: RelationKind,
    /// First selection.
    pub first: Selection,
    /// Second selection.
    pub second: Selection,
    /// Limits on the offset or angle (Maximum, Minimum, Use Offset As Resting Position).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limits: Option<Limits>,
}

/// Limits of a constraint's offset (metres) or angle (radians): within them the components may move;
/// the constraint's own value is where they rest when `resting` is set (Inventor 2026 help, To Place Mate
/// or Flush Constraints: "Check Maximum … Minimum … Use Offset As Resting Position").
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Limits {
    /// The smallest value allowed.
    pub min: Option<f64>,
    /// The largest value allowed.
    pub max: Option<f64>,
    /// The constraint's value is its resting position: the solver holds it there, and Drive may move it
    /// within the limits. Without it the value may be anywhere within the limits.
    pub resting: bool,
}

impl Limits {
    /// The value kept within the limits.
    pub fn clamp(&self, value: f64) -> f64 {
        let low = self.min.map_or(value, |m| value.max(m));
        self.max.map_or(low, |m| low.min(m))
    }

    /// True when the limits are finite and in order.
    pub fn is_valid(&self) -> bool {
        self.min.is_none_or(f64::is_finite)
            && self.max.is_none_or(f64::is_finite)
            && match (self.min, self.max) {
                (Some(a), Some(b)) => a <= b,
                _ => true,
            }
    }
}

impl Entity for Relationship {
    const KIND: Kind = Kind::Relationship;
    fn refs(&self, out: &mut Vec<AnyId>) {
        out.push(self.first.occurrence.owner());
        out.push(self.second.occurrence.owner());
        if let Some(third) = self.third() {
            out.push(third.occurrence.owner());
        }
    }
}

impl Relationship {
    /// A relationship without limits.
    pub fn new(name: impl Into<String>, kind: RelationKind, first: Selection, second: Selection) -> Self {
        Self { name: name.into(), kind, first, second, limits: None }
    }

    /// A third selection some kinds carry: the symmetry plane, an angle's explicit reference.
    pub fn third(&self) -> Option<&Selection> {
        match &self.kind {
            RelationKind::Symmetry { plane } => Some(plane),
            RelationKind::Angle { solution: AngleSolution::Explicit(reference), .. } => Some(reference),
            _ => None,
        }
    }

    /// True when the kind fits the selected geometry and the values are finite.
    pub fn is_valid(&self) -> bool {
        let (g1, g2) = (self.first.geometry, self.second.geometry);
        let fits = match self.kind {
            RelationKind::Mate { offset, .. } => offset.is_finite(),
            RelationKind::Flush { offset } => {
                offset.is_finite() && matches!((g1, g2), (Geometry::Plane { .. }, Geometry::Plane { .. }))
            }
            RelationKind::Insert { offset, .. } => {
                offset.is_finite() && matches!((g1, g2), (Geometry::Circle { .. }, Geometry::Circle { .. }))
            }
            RelationKind::Rigid { .. }
            | RelationKind::Rotational { .. }
            | RelationKind::Slider { .. }
            | RelationKind::Cylindrical { .. }
            | RelationKind::Ball => true,
            RelationKind::Angle { angle, .. } => {
                angle.is_finite() && direction_of(&g1).is_some() && direction_of(&g2).is_some()
            }
            RelationKind::Tangent { offset, .. } => {
                offset.is_finite() && round_or_flat(&g1) && round_or_flat(&g2)
            }
            RelationKind::Symmetry { plane } => {
                matches!(plane.geometry, Geometry::Plane { .. }) && plane.geometry.is_finite()
            }
        };
        let joint = is_joint(&self.kind);
        let frames = matches!(g1, Geometry::Frame { .. }) || matches!(g2, Geometry::Frame { .. });
        let limits_fit = self.limits.is_none_or(|l| l.is_valid() && value_of(&self.kind).is_some());
        fits && limits_fit
            && (joint || !frames)
            && g1.is_finite()
            && g2.is_finite()
            && self.first.occurrence.owner() != self.second.occurrence.owner()
    }
}

/// Geometry in the world.
#[derive(Clone, Copy, Debug)]
enum World {
    Plane(DVec3, DVec3),
    Axis(DVec3, DVec3),
    Point(DVec3),
}

fn world(pose: &DPose3, geometry: &Geometry) -> (World, Option<(DVec3, DVec3)>) {
    let p = |v: [f64; 3]| pose.translation + pose.rotation * DVec3::from_array(v);
    let d = |v: [f64; 3]| (pose.rotation * DVec3::from_array(v)).normalize_or_zero();
    match *geometry {
        Geometry::Plane { point, normal } => (World::Plane(p(point), d(normal)), None),
        Geometry::Axis { point, direction } => (World::Axis(p(point), d(direction)), None),
        Geometry::Point(point) => (World::Point(p(point)), None),
        Geometry::Circle { centre, normal } => {
            (World::Axis(p(centre), d(normal)), Some((p(centre), d(normal))))
        }
        Geometry::Frame { origin, z, .. } => (World::Axis(p(origin), d(z)), None),
        // A cylinder mates by its axis, a sphere by its centre.
        Geometry::Cylinder { point, direction, .. } => (World::Axis(p(point), d(direction)), None),
        Geometry::Sphere { centre, .. } => (World::Point(p(centre)), None),
    }
}

/// True for the joint kinds (solved from joint origins).
fn is_joint(kind: &RelationKind) -> bool {
    matches!(
        kind,
        RelationKind::Rigid { .. }
            | RelationKind::Rotational { .. }
            | RelationKind::Slider { .. }
            | RelationKind::Cylindrical { .. }
            | RelationKind::Ball
    )
}

/// The direction an angle is measured on: a plane's normal, an edge's or axis's direction.
fn direction_of(geometry: &Geometry) -> Option<[f64; 3]> {
    match *geometry {
        Geometry::Plane { normal, .. } | Geometry::Circle { normal, .. } => Some(normal),
        Geometry::Axis { direction, .. } | Geometry::Cylinder { direction, .. } => Some(direction),
        Geometry::Frame { z, .. } => Some(z),
        Geometry::Point(_) | Geometry::Sphere { .. } => None,
    }
}

/// Geometry a tangent constraint takes: planes, cylinders, spheres.
fn round_or_flat(geometry: &Geometry) -> bool {
    matches!(geometry, Geometry::Plane { .. } | Geometry::Cylinder { .. } | Geometry::Sphere { .. })
}

/// The part of `v` across direction `d`.
fn across(v: DVec3, d: DVec3) -> DVec3 {
    v - d * v.dot(d)
}

/// The sign that the directions of a relationship should have relative to each other (−1 opposed, +1
/// the same), with Undirected settled from `now` (the dot product of the directions as they lie).
fn sense_sign(kind: &RelationKind, now: f64) -> f64 {
    match *kind {
        RelationKind::Rigid { flip }
        | RelationKind::Rotational { flip }
        | RelationKind::Slider { flip }
        | RelationKind::Cylindrical { flip } => {
            if flip {
                1.0
            } else {
                -1.0
            }
        }
        RelationKind::Mate { sense: AxisSense::Aligned, .. } | RelationKind::Flush { .. } => 1.0,
        RelationKind::Insert { aligned, .. } => {
            if aligned {
                1.0
            } else {
                -1.0
            }
        }
        RelationKind::Ball | RelationKind::Angle { .. } => 1.0,
        RelationKind::Mate { sense: AxisSense::Undirected, .. }
        | RelationKind::Tangent { .. }
        | RelationKind::Symmetry { .. } => {
            if now >= 0.0 {
                1.0
            } else {
                -1.0
            }
        }
        RelationKind::Mate { sense: AxisSense::Opposed, .. } => -1.0,
    }
}

/// How far a relationship is from being satisfied, as residuals (metres; directions as unit-vector
/// differences). `sign` is the settled direction sign.
fn residuals(
    kind: &RelationKind,
    first: (World, Option<(DVec3, DVec3)>),
    second: (World, Option<(DVec3, DVec3)>),
    sign: f64,
    out: &mut Vec<f64>,
) {
    let offset = match *kind {
        RelationKind::Mate { offset, .. }
        | RelationKind::Flush { offset }
        | RelationKind::Insert { offset, .. } => offset,
        RelationKind::Tangent { offset, .. } => offset,
        _ => 0.0,
    };
    let push3 = |out: &mut Vec<f64>, v: DVec3| out.extend([v.x, v.y, v.z]);
    if let (RelationKind::Insert { .. }, Some((c1, n1)), Some((c2, n2))) = (kind, first.1, second.1) {
        push3(out, n1 - n2 * sign);
        push3(out, across(c1 - c2, n2));
        out.push((c1 - c2).dot(n2) - offset);
        return;
    }
    match (first.0, second.0) {
        (World::Plane(p1, n1), World::Plane(p2, n2)) => {
            push3(out, n1 - n2 * sign);
            out.push((p1 - p2).dot(n2) - offset);
        }
        (World::Point(p1), World::Point(p2)) => push3(out, p1 - p2),
        (World::Point(p), World::Plane(q, n)) | (World::Plane(q, n), World::Point(p)) => {
            out.push((p - q).dot(n) - offset)
        }
        (World::Axis(p1, d1), World::Axis(p2, d2)) => {
            push3(out, d1 - d2 * sign);
            push3(out, across(p1 - p2, d2));
        }
        (World::Point(p), World::Axis(q, d)) | (World::Axis(q, d), World::Point(p)) => {
            push3(out, across(p - q, d))
        }
        (World::Axis(p, d), World::Plane(q, n)) | (World::Plane(q, n), World::Axis(p, d)) => {
            out.push(d.dot(n));
            out.push((p - q).dot(n) - offset);
        }
    }
}

/// The directions whose relative sense a relationship fixes, for the first rough turn.
fn directions(first: &World, second: &World) -> Option<(DVec3, DVec3)> {
    match (*first, *second) {
        (World::Plane(_, a), World::Plane(_, b)) | (World::Axis(_, a), World::Axis(_, b)) => Some((a, b)),
        _ => None,
    }
}

/// Why relationships could not be solved.
#[derive(Clone, Debug, PartialEq)]
pub struct Unsolved {
    /// The relationships that are not satisfied in the best position found.
    pub sick: Vec<Id<Relationship>>,
}

/// A component that moves when relationships are solved: the document item and its world pose.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Owner {
    item: AnyId,
    pose: DPose3,
    free: bool,
}

/// One relationship ready to evaluate: owner indices, geometry in the owners' frames, settled sign.
struct Term {
    id: Id<Relationship>,
    kind: RelationKind,
    owners: [usize; 2],
    geometry: [Geometry; 2],
    sign: f64,
    /// The third selection (symmetry plane, explicit angle reference): owner index and geometry.
    third: Option<(usize, Geometry)>,
    /// A directed angle's axis, read from how the selections lay when solving started.
    axis: DVec3,
    /// Limits of the value; without `resting` the value may be anywhere within them.
    limits: Option<Limits>,
}

impl Document {
    /// The world pose of a component that can move (a body, robot or imported assembly).
    pub(crate) fn owner_pose(&self, item: AnyId) -> Option<DPose3> {
        self.component_origin(Occurrence::Item(item)).map(|o| o.pose)
    }

    /// The geometry of a selection in its owner's frame (a part's geometry goes through the part's frame).
    fn in_owner(&self, selection: &Selection) -> Option<Geometry> {
        if let Occurrence::Item(_) = selection.occurrence {
            return Some(selection.geometry);
        }
        let owner = self.owner_pose(selection.occurrence.owner())?;
        let part = self.component_origin(selection.occurrence)?.pose;
        let local = owner.inverse() * part;
        let p = |v: [f64; 3]| (local.translation + local.rotation * DVec3::from_array(v)).to_array();
        let d = |v: [f64; 3]| (local.rotation * DVec3::from_array(v)).to_array();
        Some(match selection.geometry {
            Geometry::Plane { point, normal } => Geometry::Plane { point: p(point), normal: d(normal) },
            Geometry::Axis { point, direction } => {
                Geometry::Axis { point: p(point), direction: d(direction) }
            }
            Geometry::Point(point) => Geometry::Point(p(point)),
            Geometry::Circle { centre, normal } => Geometry::Circle { centre: p(centre), normal: d(normal) },
            Geometry::Frame { origin, z, x } => Geometry::Frame { origin: p(origin), z: d(z), x: d(x) },
            Geometry::Cylinder { point, direction, radius } => {
                Geometry::Cylinder { point: p(point), direction: d(direction), radius }
            }
            Geometry::Sphere { centre, radius } => Geometry::Sphere { centre: p(centre), radius },
        })
    }

    /// What Predict Offset and Orientation proposes for two selections as they lie now: Mate when two
    /// planes face each other (normals opposite), otherwise Flush, with their current distance as the
    /// offset; for other geometry a Mate at its current offset (axes: the closer sense). `None` when a
    /// component does not exist.
    pub fn predict_relationship(&self, first: &Selection, second: &Selection) -> Option<RelationKind> {
        let w1 = world(&self.owner_pose(first.occurrence.owner())?, &self.in_owner(first)?);
        let w2 = world(&self.owner_pose(second.occurrence.owner())?, &self.in_owner(second)?);
        if let (Some((c1, _)), Some((c2, n2))) = (w1.1, w2.1) {
            let aligned = w1.1.is_some_and(|(_, n1)| n1.dot(n2) > 0.0);
            return Some(RelationKind::Insert { offset: (c1 - c2).dot(n2), aligned });
        }
        Some(match (w1.0, w2.0) {
            (World::Plane(p1, n1), World::Plane(p2, n2)) => {
                let offset = (p1 - p2).dot(n2);
                if n1.dot(n2) < 0.0 {
                    RelationKind::Mate { offset, sense: AxisSense::Opposed }
                } else {
                    RelationKind::Flush { offset }
                }
            }
            (World::Point(p), World::Plane(q, n)) | (World::Plane(q, n), World::Point(p)) => {
                RelationKind::Mate { offset: (p - q).dot(n), sense: AxisSense::Opposed }
            }
            (World::Axis(_, a), World::Axis(_, b)) => RelationKind::Mate {
                offset: 0.0,
                sense: if a.dot(b) >= 0.0 { AxisSense::Aligned } else { AxisSense::Opposed },
            },
            _ => RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
        })
    }

    /// Solves all relationships: the new world poses of the components that move. Grounded components
    /// stay; so do components no relationship touches.
    pub fn solve_relationships(&self) -> Result<Vec<(AnyId, DPose3)>, Unsolved> {
        self.solve_relationships_holding(&[])
    }

    /// Like [`Document::solve_relationships`], with some components held where they are as if grounded
    /// (a robot whose joints were just set keeps its base; what is tied to its links follows).
    pub fn solve_relationships_holding(&self, held: &[AnyId]) -> Result<Vec<(AnyId, DPose3)>, Unsolved> {
        let (mut owners, terms) = self.gather()?;
        for owner in &mut owners {
            if held.contains(&owner.item) {
                owner.free = false;
            }
        }
        let start: Vec<DPose3> = owners.iter().map(|o| o.pose).collect();
        let mut poses = start.clone();
        rough_turns(&owners, &terms, &mut poses);
        least_squares(&owners, &terms, &mut poses);
        let sick: Vec<Id<Relationship>> = terms
            .iter()
            .filter(|t| term_residuals(t, &poses).iter().any(|r| r.abs() > 1e-7))
            .map(|t| t.id)
            .collect();
        if !sick.is_empty() {
            return Err(Unsolved { sick });
        }
        Ok(owners
            .iter()
            .zip(&poses)
            .zip(&start)
            .filter(|((o, now), before)| o.free && **now != **before)
            .map(|((o, now), _)| (o.item, *now))
            .collect())
    }

    /// Which active relationships stand in the way: for a new relationship `candidate` (or, without one,
    /// the relationships as they are), those whose suppression alone would let everything be satisfied.
    /// Empty when nothing conflicts, or when no single relationship is to blame. The constraint error
    /// dialog lists these and offers to suppress them (Inventor's Design Doctor).
    pub fn conflicting_relationships(&self, candidate: Option<&Relationship>) -> Vec<Id<Relationship>> {
        let solvable = |doc: &Document| doc.solve_relationships().is_ok();
        let mut trial = self.clone();
        if let Some(candidate) = candidate {
            // Without its own solve, so that an unsatisfiable candidate is kept for the trials.
            let mut txn = crate::txn::Txn::new(&mut trial);
            txn.insert(candidate.clone());
            if txn.commit().is_err() {
                return Vec::new();
            }
        }
        if solvable(&trial) {
            return Vec::new();
        }
        let active: Vec<Id<Relationship>> = self
            .relationships()
            .iter()
            .map(|(id, _)| id)
            .filter(|id| !self.is_inactive(id.erase()))
            .collect();
        active
            .into_iter()
            .filter(|id| {
                let mut without = trial.clone();
                let mut txn = crate::txn::Txn::new(&mut without);
                if txn.remove(*id).is_err() || txn.commit().is_err() {
                    return false;
                }
                solvable(&without)
            })
            .collect()
    }

    /// The components the active relationships touch and the relationships ready to evaluate.
    fn gather(&self) -> Result<(Vec<Owner>, Vec<Term>), Unsolved> {
        let mut owners: Vec<Owner> = Vec::new();
        let mut terms: Vec<Term> = Vec::new();
        let index_of = |doc: &Document, owners: &mut Vec<Owner>, item: AnyId| -> Option<usize> {
            if let Some(i) = owners.iter().position(|o| o.item == item) {
                return Some(i);
            }
            let pose = doc.owner_pose(item)?;
            let free = !doc.component_flag(Occurrence::Item(item), ComponentFlag::Grounded);
            owners.push(Owner { item, pose, free });
            Some(owners.len() - 1)
        };
        for (id, relationship) in self.relationships().iter() {
            if self.is_inactive(id.erase()) {
                continue;
            }
            let (Some(a), Some(b), Some(g1), Some(g2)) = (
                index_of(self, &mut owners, relationship.first.occurrence.owner()),
                index_of(self, &mut owners, relationship.second.occurrence.owner()),
                self.in_owner(&relationship.first),
                self.in_owner(&relationship.second),
            ) else {
                return Err(Unsolved { sick: vec![id] });
            };
            let third = match relationship.third() {
                Some(selection) => {
                    let (Some(c), Some(g3)) =
                        (index_of(self, &mut owners, selection.occurrence.owner()), self.in_owner(selection))
                    else {
                        return Err(Unsolved { sick: vec![id] });
                    };
                    Some((c, g3))
                }
                None => None,
            };
            let (w1, w2) = (world(&owners[a].pose, &g1), world(&owners[b].pose, &g2));
            let mut now = directions(&w1.0, &w2.0).map_or(1.0, |(x, y)| x.dot(y));
            if let (RelationKind::Symmetry { .. }, Some((c, g3))) = (relationship.kind, third) {
                // The mirror image of the first against the second.
                let (_, m) = plane_world(&owners[c].pose, &g3);
                if let (World::Axis(_, d1), World::Axis(_, d2)) = (w1.0, w2.0) {
                    now = mirror_direction(d1, m).dot(d2);
                }
            }
            if let (RelationKind::Tangent { .. }, Geometry::Cylinder { .. }, Geometry::Cylinder { .. }) =
                (relationship.kind, g1, g2)
                && let (World::Axis(_, d1), World::Axis(_, d2)) = (w1.0, w2.0)
            {
                now = d1.dot(d2);
            }
            let axis = match (direction_of(&g1), direction_of(&g2)) {
                (Some(x), Some(y)) => {
                    let (x, y) = (
                        owners[a].pose.rotation * DVec3::from_array(x),
                        owners[b].pose.rotation * DVec3::from_array(y),
                    );
                    let z = x.cross(y);
                    if z.length() > 1e-9 { z.normalize() } else { perpendicular(x) }
                }
                _ => DVec3::Z,
            };
            terms.push(Term {
                id,
                kind: relationship.kind,
                owners: [a, b],
                geometry: [g1, g2],
                sign: sense_sign(&relationship.kind, now),
                third,
                axis,
                limits: relationship.limits,
            });
        }
        Ok((owners, terms))
    }
}

/// A joint origin in the world: point, Z, X.
fn joint_world(pose: &DPose3, geometry: &Geometry) -> (DVec3, DVec3, DVec3) {
    let (p, z, x) = geometry.joint_frame();
    (pose.translation + pose.rotation * p, pose.rotation * z, pose.rotation * x)
}

fn term_residuals(term: &Term, poses: &[DPose3]) -> Vec<f64> {
    // Limits without a resting position: the value may be anywhere within them, so the constraint asks
    // only for the nearest value inside.
    if let Some(limits) = term.limits.filter(|l| !l.resting)
        && let Some(actual) = measured(term, poses)
    {
        let kind = with_value(&term.kind, limits.clamp(actual));
        return plain_residuals(&Term { kind, limits: None, ..*term }, poses);
    }
    plain_residuals(term, poses)
}

/// The value (offset or angle) a constraint has where the components stand now.
fn measured(term: &Term, poses: &[DPose3]) -> Option<f64> {
    if let RelationKind::Angle { solution, .. } = term.kind {
        let d = |pose: &DPose3, g: &Geometry| {
            (pose.rotation * DVec3::from_array(direction_of(g).unwrap_or([0.0, 0.0, 1.0])))
                .normalize_or_zero()
        };
        let (a1, a2) =
            (d(&poses[term.owners[0]], &term.geometry[0]), d(&poses[term.owners[1]], &term.geometry[1]));
        let cos = a1.dot(a2).clamp(-1.0, 1.0);
        let z = match (solution, term.third) {
            (AngleSolution::Undirected, _) => return Some(gc_math::fm::acos(cos)),
            (AngleSolution::Explicit(_), Some((c, g3))) => d(&poses[c], &g3),
            _ => term.axis,
        };
        return Some(gc_math::fm::atan2(a1.cross(a2).dot(z), cos));
    }
    // Offsets: the last residual is affine in the offset; solve it for zero.
    let at = |value: f64| {
        let probe = Term { kind: with_value(&term.kind, value), limits: None, ..*term };
        plain_residuals(&probe, poses).last().copied()
    };
    let (r0, r1) = (at(0.0)?, at(1.0)?);
    let slope = r1 - r0;
    (slope.abs() > 1e-12).then(|| -r0 / slope)
}

/// The kind's value: an offset or an angle; `None` for kinds without one.
fn value_of(kind: &RelationKind) -> Option<f64> {
    match *kind {
        RelationKind::Mate { offset, .. }
        | RelationKind::Flush { offset }
        | RelationKind::Insert { offset, .. }
        | RelationKind::Tangent { offset, .. } => Some(offset),
        RelationKind::Angle { angle, .. } => Some(angle),
        _ => None,
    }
}

/// The kind with another value (offset or angle).
fn with_value(kind: &RelationKind, value: f64) -> RelationKind {
    match *kind {
        RelationKind::Mate { sense, .. } => RelationKind::Mate { offset: value, sense },
        RelationKind::Flush { .. } => RelationKind::Flush { offset: value },
        RelationKind::Insert { aligned, .. } => RelationKind::Insert { offset: value, aligned },
        RelationKind::Tangent { inside, .. } => RelationKind::Tangent { offset: value, inside },
        RelationKind::Angle { solution, .. } => RelationKind::Angle { angle: value, solution },
        other => other,
    }
}

fn plain_residuals(term: &Term, poses: &[DPose3]) -> Vec<f64> {
    let mut out = Vec::with_capacity(9);
    let push3 = |out: &mut Vec<f64>, v: DVec3| out.extend([v.x, v.y, v.z]);
    let (pose1, pose2) = (&poses[term.owners[0]], &poses[term.owners[1]]);
    if is_joint(&term.kind) {
        let (p1, z1, x1) = joint_world(pose1, &term.geometry[0]);
        let (p2, z2, x2) = joint_world(pose2, &term.geometry[1]);
        match term.kind {
            RelationKind::Slider { .. } | RelationKind::Cylindrical { .. } => {
                push3(&mut out, across(p1 - p2, z2))
            }
            _ => push3(&mut out, p1 - p2),
        }
        if !matches!(term.kind, RelationKind::Ball) {
            push3(&mut out, z1 - z2 * term.sign);
        }
        if let RelationKind::Rigid { .. } | RelationKind::Slider { .. } = term.kind {
            push3(&mut out, x1 - x2);
        }
        return out;
    }
    match term.kind {
        RelationKind::Angle { angle, solution } => {
            let d = |pose: &DPose3, g: &Geometry| {
                (pose.rotation * DVec3::from_array(direction_of(g).unwrap_or([0.0, 0.0, 1.0])))
                    .normalize_or_zero()
            };
            let (a1, a2) = (d(pose1, &term.geometry[0]), d(pose2, &term.geometry[1]));
            let (sin, cos) = gc_math::fm::sin_cos(angle);
            out.push(a1.dot(a2) - cos);
            let z = match (solution, term.third) {
                (AngleSolution::Undirected, _) => return out,
                (AngleSolution::Explicit(_), Some((c, g3))) => d(&poses[c], &g3),
                _ => term.axis,
            };
            out.push(a1.cross(a2).dot(z) - sin);
            return out;
        }
        RelationKind::Tangent { offset, inside } => {
            tangent_residuals(
                pose1,
                &term.geometry[0],
                pose2,
                &term.geometry[1],
                offset,
                inside,
                term.sign,
                &mut out,
            );
            return out;
        }
        RelationKind::Symmetry { .. } => {
            if let Some((c, g3)) = term.third {
                let (q, m) = plane_world(&poses[c], &g3);
                let mirror_point = |p: DVec3| p - m * (2.0 * (p - q).dot(m));
                let (w1, w2) = (world(pose1, &term.geometry[0]).0, world(pose2, &term.geometry[1]).0);
                match (w1, w2) {
                    (World::Plane(p1, n1), World::Plane(p2, n2)) => {
                        push3(&mut out, mirror_direction(n1, m) - n2);
                        out.push((mirror_point(p1) - p2).dot(n2));
                    }
                    (World::Axis(p1, d1), World::Axis(p2, d2)) => {
                        push3(&mut out, mirror_direction(d1, m) - d2 * term.sign);
                        push3(&mut out, across(mirror_point(p1) - p2, d2));
                    }
                    (World::Point(p1), World::Point(p2)) => push3(&mut out, mirror_point(p1) - p2),
                    // Mixed kinds of geometry cannot mirror each other: always sick.
                    _ => out.push(1.0),
                }
            }
            return out;
        }
        _ => {}
    }
    let w1 = world(&poses[term.owners[0]], &term.geometry[0]);
    let w2 = world(&poses[term.owners[1]], &term.geometry[1]);
    residuals(&term.kind, w1, w2, term.sign, &mut out);
    out
}

fn all_residuals(terms: &[Term], poses: &[DPose3]) -> Vec<f64> {
    terms.iter().flat_map(|t| term_residuals(t, poses)).collect()
}

/// Turns a moving component half a turn where its direction points the wrong way, so that the least
/// squares do not start at the saddle where opposite directions have no gradient.
fn rough_turns(owners: &[Owner], terms: &[Term], poses: &mut [DPose3]) {
    for term in terms {
        if is_joint(&term.kind) {
            if !matches!(term.kind, RelationKind::Ball) {
                joint_turns(owners, term, poses);
            }
            continue;
        }
        if matches!(
            term.kind,
            RelationKind::Angle { .. } | RelationKind::Tangent { .. } | RelationKind::Symmetry { .. }
        ) {
            continue;
        }
        let w1 = world(&poses[term.owners[0]], &term.geometry[0]);
        let w2 = world(&poses[term.owners[1]], &term.geometry[1]);
        let pair = match (w1.1, w2.1) {
            (Some((_, a)), Some((_, b))) => Some((a, b)),
            _ => directions(&w1.0, &w2.0),
        };
        let Some((a, b)) = pair else { continue };
        if a.dot(b * term.sign) > -0.5 {
            continue;
        }
        let (mover, direction) = if owners[term.owners[0]].free {
            (term.owners[0], a)
        } else if owners[term.owners[1]].free {
            (term.owners[1], b)
        } else {
            continue;
        };
        let helper = if direction.x.abs() < 0.9 { DVec3::X } else { DVec3::Y };
        let axis = direction.cross(helper).normalize();
        let turn = DQuat::from_axis_angle(axis, std::f64::consts::PI);
        let pose = &mut poses[mover];
        pose.rotation = (turn * pose.rotation).normalize();
    }
}

/// Moves a pose by a parameter step: three translations, then a rotation vector about the pose's origin.
fn stepped(pose: &DPose3, step: &[f64]) -> DPose3 {
    let mut out = *pose;
    out.translation += DVec3::new(step[0], step[1], step[2]);
    let rotation = gc_math::quat_from_rotvec(DVec3::new(step[3], step[4], step[5]));
    out.rotation = (rotation * pose.rotation).normalize();
    out
}

/// Damped least squares over the free components' poses.
fn least_squares(owners: &[Owner], terms: &[Term], poses: &mut [DPose3]) {
    let free: Vec<usize> = (0..owners.len()).filter(|i| owners[*i].free).collect();
    let n = free.len() * 6;
    if n == 0 || terms.is_empty() {
        return;
    }
    let cost = |r: &[f64]| r.iter().map(|v| v * v).sum::<f64>();
    let mut r = all_residuals(terms, poses);
    let mut lambda = 1e-3;
    for _ in 0..200 {
        if cost(&r) < 1e-24 {
            break;
        }
        // The Jacobian by forward differences.
        let h = 1e-7;
        let m = r.len();
        let mut jac = vec![0.0; m * n];
        for (k, owner) in free.iter().enumerate() {
            for p in 0..6 {
                let mut step = [0.0; 6];
                step[p] = h;
                let saved = poses[*owner];
                poses[*owner] = stepped(&saved, &step);
                let moved = all_residuals(terms, poses);
                poses[*owner] = saved;
                for row in 0..m {
                    jac[row * n + k * 6 + p] = (moved[row] - r[row]) / h;
                }
            }
        }
        let mut normal = vec![0.0; n * n];
        let mut gradient = vec![0.0; n];
        for row in 0..m {
            for i in 0..n {
                let ji = jac[row * n + i];
                gradient[i] -= ji * r[row];
                for j in 0..n {
                    normal[i * n + j] += ji * jac[row * n + j];
                }
            }
        }
        let mut improved = false;
        for _ in 0..12 {
            let mut a = normal.clone();
            for i in 0..n {
                a[i * n + i] += lambda * (1.0 + normal[i * n + i]);
            }
            let Some(dx) = solve_linear(a, gradient.clone(), n) else {
                lambda *= 10.0;
                continue;
            };
            let candidate: Vec<DPose3> = poses
                .iter()
                .enumerate()
                .map(|(i, pose)| match free.iter().position(|f| *f == i) {
                    Some(k) => stepped(pose, &dx[k * 6..k * 6 + 6]),
                    None => *pose,
                })
                .collect();
            let r_new = all_residuals(terms, &candidate);
            if cost(&r_new) < cost(&r) {
                poses.copy_from_slice(&candidate);
                r = r_new;
                lambda = (lambda * 0.3).max(1e-12);
                improved = true;
                break;
            }
            lambda *= 10.0;
        }
        if !improved {
            break;
        }
    }
}

/// Solves `a x = b` (n × n, row-major) by Gaussian elimination with partial pivoting.
fn solve_linear(mut a: Vec<f64>, mut b: Vec<f64>, n: usize) -> Option<Vec<f64>> {
    for col in 0..n {
        let pivot = (col..n).max_by(|x, y| a[x * n + col].abs().total_cmp(&a[y * n + col].abs()))?;
        if a[pivot * n + col].abs() < 1e-300 {
            return None;
        }
        if pivot != col {
            for k in 0..n {
                a.swap(pivot * n + k, col * n + k);
            }
            b.swap(pivot, col);
        }
        for row in col + 1..n {
            let f = a[row * n + col] / a[col * n + col];
            for k in col..n {
                a[row * n + k] -= f * a[col * n + k];
            }
            b[row] -= f * b[col];
        }
    }
    let mut x = vec![0.0; n];
    for row in (0..n).rev() {
        let tail: f64 = (row + 1..n).map(|k| a[row * n + k] * x[k]).sum();
        x[row] = (b[row] - tail) / a[row * n + row];
    }
    x.iter().all(|v| v.is_finite()).then_some(x)
}

/// The rough turns of a joint: Z axes the right way, then (rigid) X axes alike, each by half a turn.
fn joint_turns(owners: &[Owner], term: &Term, poses: &mut [DPose3]) {
    let mover = if owners[term.owners[0]].free {
        0
    } else if owners[term.owners[1]].free {
        1
    } else {
        return;
    };
    let index = term.owners[mover];
    let (_, z1, _) = joint_world(&poses[term.owners[0]], &term.geometry[0]);
    let (_, z2, _) = joint_world(&poses[term.owners[1]], &term.geometry[1]);
    if z1.dot(z2 * term.sign) < -0.5 {
        let z = if mover == 0 { z1 } else { z2 };
        let helper = if z.x.abs() < 0.9 { DVec3::X } else { DVec3::Y };
        let turn = DQuat::from_axis_angle(z.cross(helper).normalize(), std::f64::consts::PI);
        poses[index].rotation = (turn * poses[index].rotation).normalize();
    }
    if let RelationKind::Rigid { .. } | RelationKind::Slider { .. } = term.kind {
        let (_, z1, x1) = joint_world(&poses[term.owners[0]], &term.geometry[0]);
        let (_, z2, x2) = joint_world(&poses[term.owners[1]], &term.geometry[1]);
        if x1.dot(x2) < -0.5 {
            let z = if mover == 0 { z1 } else { z2 };
            let turn = DQuat::from_axis_angle(z.normalize(), std::f64::consts::PI);
            poses[index].rotation = (turn * poses[index].rotation).normalize();
        }
    }
}

impl Document {
    /// The joint type Automatic picks for two joint origins: Rotational when both are circular, otherwise
    /// Rigid (Inventor 2026 help, Create Joints Reference, GUID-6AA68E8F-7C97-4806-8483-3941DE915E70;
    /// Cylindrical and Ball, for points on cylinders and spheres, are not modelled yet).
    pub fn automatic_joint(first: &Selection, second: &Selection) -> RelationKind {
        let circular = |s: &Selection| matches!(s.geometry, Geometry::Circle { .. });
        if circular(first) && circular(second) {
            RelationKind::Rotational { flip: false }
        } else {
            RelationKind::Rigid { flip: false }
        }
    }
}

/// The freedom a component has left: how many independent motions the relationships allow it, with the
/// others held where they are (docs/ux/inventor-assembly-parity.md, P1-10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dof {
    /// All remaining degrees of freedom (0 to 6).
    pub total: u8,
    /// Of those, pure translations (0 to 3); the rest involve turning.
    pub translation: u8,
}

impl Dof {
    /// Remaining degrees of freedom that involve turning.
    pub fn rotation(&self) -> u8 {
        self.total - self.translation
    }
}

/// A component's constraint status in the browser (Inventor 2026 help, To Manage the Display of Constraint
/// Status in the Browser, GUID-A0573060-3778-4D6F-9431-28587D837CC7): black dot, hollow dot, dash.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComponentStatus {
    /// Fully constrained (or grounded): a black dot.
    Full,
    /// Under constrained: a hollow dot.
    Under,
    /// Not known, because a relationship on it is not satisfied: a dash.
    Unknown,
}

/// The state of a single relationship.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelationStatus {
    /// Satisfied, and the components it ties are fully constrained (or grounded).
    Full,
    /// Satisfied, but a component it ties can still move.
    Under,
    /// Not satisfied where the components stand.
    Sick,
}

/// The rank of a column set (row-major `m × n`) by Gram–Schmidt with a relative tolerance.
fn rank(jac: &[f64], m: usize, n: usize, columns: &[usize]) -> usize {
    let mut basis: Vec<Vec<f64>> = Vec::new();
    for &c in columns {
        let mut v: Vec<f64> = (0..m).map(|r| jac[r * n + c]).collect();
        for b in &basis {
            let dot: f64 = v.iter().zip(b).map(|(x, y)| x * y).sum();
            v.iter_mut().zip(b).for_each(|(x, y)| *x -= dot * y);
        }
        let norm = v.iter().map(|x| x * x).sum::<f64>().sqrt();
        if norm > 1e-6 {
            basis.push(v.iter().map(|x| x / norm).collect());
        }
    }
    basis.len()
}

impl Document {
    /// The freedom a component has left under the relationships, the other components held still.
    /// Grounded: none; untouched by relationships: all six. `None` when it is not a component that moves.
    pub fn component_dof(&self, item: AnyId) -> Option<Dof> {
        self.owner_pose(item)?;
        if self.component_flag(Occurrence::Item(item), ComponentFlag::Grounded) {
            return Some(Dof { total: 0, translation: 0 });
        }
        let Ok((owners, terms)) = self.gather() else { return Some(Dof { total: 6, translation: 3 }) };
        let Some(index) = owners.iter().position(|o| o.item == item) else {
            return Some(Dof { total: 6, translation: 3 });
        };
        let terms: Vec<Term> = terms.into_iter().filter(|t| t.owners.contains(&index)).collect();
        let mut poses: Vec<DPose3> = owners.iter().map(|o| o.pose).collect();
        let base = all_residuals(&terms, &poses);
        let m = base.len();
        let h = 1e-7;
        let mut jac = vec![0.0; m * 6];
        for p in 0..6 {
            let mut step = [0.0; 6];
            step[p] = h;
            let saved = poses[index];
            poses[index] = stepped(&saved, &step);
            let moved = all_residuals(&terms, &poses);
            poses[index] = saved;
            for row in 0..m {
                jac[row * 6 + p] = (moved[row] - base[row]) / h;
            }
        }
        let fixed = rank(&jac, m, 6, &[0, 1, 2, 3, 4, 5]);
        let fixed_translation = rank(&jac, m, 6, &[0, 1, 2]);
        #[allow(clippy::cast_possible_truncation, reason = "ranks are at most 6")]
        Some(Dof { total: (6 - fixed) as u8, translation: (3 - fixed_translation) as u8 })
    }

    /// The constraint status dot of a component.
    pub fn component_status(&self, item: AnyId) -> Option<ComponentStatus> {
        let dof = self.component_dof(item)?;
        let sick = self.relationships().iter().any(|(id, r)| {
            (r.first.occurrence.owner() == item || r.second.occurrence.owner() == item)
                && self.relationship_status(id) == Some(RelationStatus::Sick)
        });
        Some(if sick {
            ComponentStatus::Unknown
        } else if dof.total == 0 {
            ComponentStatus::Full
        } else {
            ComponentStatus::Under
        })
    }

    /// The status of a relationship: sick when it is not satisfied, full when the components it
    /// ties have no freedom left, otherwise under-constrained. `None` when it does not exist.
    pub fn relationship_status(&self, id: Id<Relationship>) -> Option<RelationStatus> {
        let relationship = self.get(id)?;
        let (owners, terms) = self.gather().ok()?;
        let term = terms.iter().find(|t| t.id == id)?;
        let poses: Vec<DPose3> = owners.iter().map(|o| o.pose).collect();
        if term_residuals(term, &poses).iter().any(|r| r.abs() > 1e-7) {
            return Some(RelationStatus::Sick);
        }
        let held = [relationship.first.occurrence.owner(), relationship.second.occurrence.owner()]
            .iter()
            .all(|item| self.component_dof(*item).is_some_and(|d| d.total == 0));
        Some(if held { RelationStatus::Full } else { RelationStatus::Under })
    }
}

impl Document {
    /// Drive: where the components would stand with a relationship driven to `value`, without changing
    /// the document (Inventor's Drive dialog plays a constraint or joint through values). The value is the
    /// offset (metres) of a Mate, Flush or Insert, or the turn (radians, from where the moving component
    /// stands now) of a Rotational joint; a Rigid joint cannot be driven. Returns the world poses of the
    /// components that move.
    pub fn driven(&self, id: Id<Relationship>, value: f64) -> Result<Vec<(AnyId, DPose3)>, crate::EditError> {
        use crate::{CellOp, Command, EditError};
        let mut relationship = self.get(id).ok_or(EditError::BadValue)?.clone();
        if !value.is_finite() {
            return Err(EditError::BadValue);
        }
        // Drive stays within the limits, and holds the driven value there.
        let value = relationship.limits.map_or(value, |l| l.clamp(value));
        if let Some(limits) = &mut relationship.limits {
            limits.resting = true;
        }
        let mut preview = self.clone();
        match relationship.kind {
            RelationKind::Mate { sense, .. } => {
                let kind = RelationKind::Mate { offset: value, sense };
                let relationship = Relationship { kind, ..relationship };
                crate::apply(&mut preview, &Command::Cell(CellOp::SetRelationship { id, relationship }))?;
            }
            RelationKind::Flush { .. } => {
                let relationship =
                    Relationship { kind: RelationKind::Flush { offset: value }, ..relationship };
                crate::apply(&mut preview, &Command::Cell(CellOp::SetRelationship { id, relationship }))?;
            }
            RelationKind::Insert { aligned, .. } => {
                let kind = RelationKind::Insert { offset: value, aligned };
                let relationship = Relationship { kind, ..relationship };
                crate::apply(&mut preview, &Command::Cell(CellOp::SetRelationship { id, relationship }))?;
            }
            RelationKind::Rotational { .. } => {
                // Turn the moving component about the joint's axis, then let the others follow.
                let first = relationship.first.occurrence.owner();
                let second = relationship.second.occurrence.owner();
                let grounded =
                    |item: AnyId| self.component_flag(Occurrence::Item(item), ComponentFlag::Grounded);
                let (mover, sign) = if !grounded(first) {
                    (first, 1.0)
                } else if !grounded(second) {
                    (second, -1.0)
                } else {
                    return Err(EditError::BadValue);
                };
                let axis_on = self.in_owner(&relationship.second).ok_or(EditError::BadValue)?;
                let (point, z, _) =
                    joint_world(&self.owner_pose(second).ok_or(EditError::BadValue)?, &axis_on);
                let pose = self.owner_pose(mover).ok_or(EditError::BadValue)?;
                let turn = DQuat::from_axis_angle(z.normalize(), sign * value);
                let moved = DPose3::from_parts(
                    point + turn * (pose.translation - point),
                    (turn * pose.rotation).normalize(),
                );
                let placement = crate::Placement {
                    xyz: moved.translation.to_array(),
                    rpy: gc_math::rpy_from_quat(moved.rotation),
                };
                let op = match mover.kind {
                    Kind::Body => {
                        let mut body = self
                            .get(Id::<crate::Body>::from_raw(mover.raw))
                            .ok_or(EditError::BadValue)?
                            .clone();
                        body.placement = placement;
                        CellOp::SetBody { id: Id::from_raw(mover.raw), body }
                    }
                    Kind::RobotInstance => CellOp::PlaceRobot { id: Id::from_raw(mover.raw), placement },
                    Kind::Assembly => CellOp::PlaceAssembly { id: Id::from_raw(mover.raw), placement },
                    _ => return Err(EditError::BadValue),
                };
                crate::apply(&mut preview, &Command::Cell(op))?;
            }
            RelationKind::Angle { solution, .. } => {
                let relationship =
                    Relationship { kind: RelationKind::Angle { angle: value, solution }, ..relationship };
                crate::apply(&mut preview, &Command::Cell(CellOp::SetRelationship { id, relationship }))?;
            }
            RelationKind::Tangent { inside, .. } => {
                let relationship =
                    Relationship { kind: RelationKind::Tangent { offset: value, inside }, ..relationship };
                crate::apply(&mut preview, &Command::Cell(CellOp::SetRelationship { id, relationship }))?;
            }
            RelationKind::Rigid { .. }
            | RelationKind::Slider { .. }
            | RelationKind::Cylindrical { .. }
            | RelationKind::Ball
            | RelationKind::Symmetry { .. } => return Err(EditError::BadValue),
        }
        let items: Vec<AnyId> = self
            .bodies()
            .iter()
            .map(|(id, _)| id.erase())
            .chain(self.robots().iter().map(|(id, _)| id.erase()))
            .chain(self.assemblies().iter().map(|(id, _)| id.erase()))
            .collect();
        Ok(items
            .into_iter()
            .filter_map(|item| {
                let now = preview.owner_pose(item)?;
                (self.owner_pose(item)? != now).then_some((item, now))
            })
            .collect())
    }
}

/// A plane selection in the world: a point on it and its unit normal.
fn plane_world(pose: &DPose3, geometry: &Geometry) -> (DVec3, DVec3) {
    match world(pose, geometry).0 {
        World::Plane(q, m) => (q, m),
        World::Axis(q, d) => (q, d),
        World::Point(q) => (q, DVec3::Z),
    }
}

/// A direction mirrored across a plane with unit normal `m`.
fn mirror_direction(d: DVec3, m: DVec3) -> DVec3 {
    d - m * (2.0 * d.dot(m))
}

/// Some unit vector perpendicular to `v`.
fn perpendicular(v: DVec3) -> DVec3 {
    let helper = if v.x.abs() < 0.9 { DVec3::X } else { DVec3::Y };
    v.cross(helper).normalize_or_zero()
}

/// Tangency residuals between planes, cylinders and spheres (Inventor 2026 help, Place Tangent
/// constraints in assemblies, GUID-FF270566-CA9E-420A-A00A-F52DD0FDFD8E). `sign` is the settled sense of
/// two cylinders' axes.
#[allow(clippy::too_many_arguments, reason = "two posed selections and the constraint's values")]
fn tangent_residuals(
    pose1: &DPose3,
    g1: &Geometry,
    pose2: &DPose3,
    g2: &Geometry,
    offset: f64,
    inside: bool,
    sign: f64,
    out: &mut Vec<f64>,
) {
    let p = |pose: &DPose3, v: [f64; 3]| pose.translation + pose.rotation * DVec3::from_array(v);
    let d = |pose: &DPose3, v: [f64; 3]| (pose.rotation * DVec3::from_array(v)).normalize_or_zero();
    enum Shape {
        Plane(DVec3, DVec3),
        Cylinder(DVec3, DVec3, f64),
        Sphere(DVec3, f64),
    }
    let shape = |pose: &DPose3, g: &Geometry| match *g {
        Geometry::Plane { point, normal } => Some(Shape::Plane(p(pose, point), d(pose, normal))),
        Geometry::Cylinder { point, direction, radius } => {
            Some(Shape::Cylinder(p(pose, point), d(pose, direction), radius))
        }
        Geometry::Sphere { centre, radius } => Some(Shape::Sphere(p(pose, centre), radius)),
        _ => None,
    };
    let (Some(s1), Some(s2)) = (shape(pose1, g1), shape(pose2, g2)) else {
        out.push(1.0);
        return;
    };
    // Distance between centres (or from the plane) that touching asks for.
    let touching = |r1: f64, r2: f64| if inside { (r2 - r1).abs() - offset } else { r1 + r2 + offset };
    let side = if inside { -1.0 } else { 1.0 };
    match (s1, s2) {
        (Shape::Plane(p1, n1), Shape::Plane(q, n2)) => {
            // Two planes touching: face to face (like a mate), `offset` apart.
            let v = n1 + n2;
            out.extend([v.x, v.y, v.z]);
            out.push((p1 - q).dot(n2) - offset);
        }
        (Shape::Sphere(c, r), Shape::Plane(q, n)) | (Shape::Plane(q, n), Shape::Sphere(c, r)) => {
            out.push((c - q).dot(n) - side * (r + offset));
        }
        (Shape::Cylinder(a, u, r), Shape::Plane(q, n)) | (Shape::Plane(q, n), Shape::Cylinder(a, u, r)) => {
            out.push(u.dot(n));
            out.push((a - q).dot(n) - side * (r + offset));
        }
        (Shape::Sphere(c1, r1), Shape::Sphere(c2, r2)) => out.push((c1 - c2).length() - touching(r1, r2)),
        (Shape::Sphere(c, rs), Shape::Cylinder(a, u, rc))
        | (Shape::Cylinder(a, u, rc), Shape::Sphere(c, rs)) => {
            out.push(across(c - a, u).length() - touching(rs, rc));
        }
        (Shape::Cylinder(a1, u1, r1), Shape::Cylinder(a2, u2, r2)) => {
            let v = u1 - u2 * sign;
            out.extend([v.x, v.y, v.z]);
            out.push(across(a1 - a2, u2).length() - touching(r1, r2));
        }
    }
}
