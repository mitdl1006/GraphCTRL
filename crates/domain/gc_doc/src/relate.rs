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

    /// Offset Origin (Inventor 2026 help, To Define and Manage Joint Relationships,
    /// GUID-21DC3336-5C51-42C1-90FB-4299CD66E0C6: "To create a joint origin and position it an offset
    /// distance from the origin, select Offset Origin"): this joint origin moved by `offset` along its own
    /// X, Y and Z axes, as a frame.
    pub fn offset_origin(&self, offset: [f64; 3]) -> Self {
        let (origin, z, x) = self.joint_frame();
        let moved = origin + x * offset[0] + z.cross(x) * offset[1] + z * offset[2];
        Geometry::Frame { origin: moved.to_array(), z: z.to_array(), x: x.to_array() }
    }

    /// Between Two Faces (same help page: "specify a virtual midpoint between two faces, by selecting two
    /// faces and one point"): a joint origin half way between two planes of one component, at the foot
    /// of `point` on that middle plane, its Z along the first face's normal. `None` unless both are
    /// planes that are parallel.
    pub fn between_faces(first: &Geometry, second: &Geometry, point: [f64; 3]) -> Option<Self> {
        let (Geometry::Plane { point: p1, normal: n1 }, Geometry::Plane { point: p2, normal: n2 }) =
            (*first, *second)
        else {
            return None;
        };
        let v = DVec3::from_array;
        let (n1, n2) = (v(n1).normalize_or_zero(), v(n2).normalize_or_zero());
        if n1.cross(n2).length() > 1e-9 || n1.length() < 0.5 || n2.length() < 0.5 {
            return None;
        }
        let middle = (v(p1) + v(p2)) * 0.5;
        let foot = v(point) - n1 * (v(point) - middle).dot(n1);
        let (_, z, x) = Geometry::Plane { point: foot.to_array(), normal: n1.to_array() }.joint_frame();
        foot.is_finite().then(|| Geometry::Frame {
            origin: foot.to_array(),
            z: z.to_array(),
            x: x.to_array(),
        })
    }

    /// Align (same help page: "select Align 1 and pick … to specify the alignment direction", "To change
    /// the alignment direction, select Invert alignment"): this joint origin with its X axis along
    /// `direction` (made perpendicular to Z), the other way with `invert`. `None` when the direction runs
    /// along Z or has no length.
    pub fn aligned_to(&self, direction: [f64; 3], invert: bool) -> Option<Self> {
        let (origin, z, _) = self.joint_frame();
        let direction = DVec3::from_array(direction);
        let across = direction - z * direction.dot(z);
        if !across.is_finite() || across.length() <= 1e-9 * direction.length().max(1e-300) {
            return None;
        }
        let x = across.normalize() * if invert { -1.0 } else { 1.0 };
        Some(Geometry::Frame { origin: origin.to_array(), z: z.to_array(), x: x.to_array() })
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

    pub(crate) fn is_finite(&self) -> bool {
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

/// A relationship as Free Move draws it: an elastic band between its two selections.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RelationBand {
    /// The relationship.
    pub id: Id<Relationship>,
    /// Its first and second selection in the world.
    pub ends: [DVec3; 2],
    /// False when it does not hold where the components stand (until Update).
    pub satisfied: bool,
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

/// Lock and Protect on a joint (Inventor 2026 help, To Define and Manage Joint Relationships,
/// GUID-21DC3336-5C51-42C1-90FB-4299CD66E0C6): "Lock eliminates all motion, but allows the component to
/// change position when related components move"; "Protect a relationship to warn if added relationships
/// violate the necessary degrees of freedom". Not for constraints.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct JointCondition {
    /// Locked where it stood: the first joint origin as seen from the second.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locked: Option<LockedPose>,
    /// Protected: relationships that take its freedom away are refused.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub protected: bool,
    /// Angular limits (Rotational, Cylindrical): the turn about the joint's Z axis, radians.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angular: Option<JointRange>,
    /// Linear limits (Slider, Cylindrical): the slide along the joint's Z axis, metres.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linear: Option<JointRange>,
}

/// A joint's range of motion (Inventor 2026 help, To Define and Manage Joint Relationships,
/// GUID-21DC3336-5C51-42C1-90FB-4299CD66E0C6, Limits tab: Start, Current, End). The joint stays within
/// Start..End; with `current` it is held there (as Inventor's Current value positions the joint).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct JointRange {
    /// The smallest value.
    pub start: f64,
    /// The largest value.
    pub end: f64,
    /// Where the joint is held; `None` leaves it free within the range.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<f64>,
}

impl JointRange {
    /// True when the values are finite, Start is not past End, and Current lies between them.
    pub fn is_valid(&self) -> bool {
        self.start.is_finite()
            && self.end.is_finite()
            && self.start <= self.end
            && self.current.is_none_or(|c| c.is_finite() && (self.start..=self.end).contains(&c))
    }

    /// What the joint is asked to be, given where it is: Current when set, otherwise the value kept within
    /// the range.
    fn target(&self, value: f64) -> f64 {
        // Never panics, whatever a damaged file holds (Start past End, NaN).
        self.current.unwrap_or_else(|| gc_math::clamp_between(value, self.start, self.end))
    }
}

/// The joint kinds that take angular (`true`) or linear (`false`) limits (Rigid takes none).
pub fn takes_limits(kind: &RelationKind, angular: bool) -> bool {
    match kind {
        RelationKind::Rotational { .. } => angular,
        RelationKind::Slider { .. } => !angular,
        RelationKind::Cylindrical { .. } => true,
        RelationKind::Mate { .. }
        | RelationKind::Flush { .. }
        | RelationKind::Insert { .. }
        | RelationKind::Tangent { .. }
        | RelationKind::Angle { .. }
        | RelationKind::Symmetry { .. }
        | RelationKind::Rigid { .. }
        | RelationKind::Ball => false,
    }
}

/// A joint's first origin in the second's frame, when it was locked.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LockedPose {
    /// Origin.
    pub origin: [f64; 3],
    /// Z axis.
    pub z: [f64; 3],
    /// X axis.
    pub x: [f64; 3],
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
    /// True when one of its selections belongs to `item`.
    pub fn references(&self, item: AnyId) -> bool {
        let mut refs = Vec::new();
        self.refs(&mut refs);
        refs.contains(&item)
    }

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

/// A solution the Assemble command offers once the geometry is picked (Inventor 2026 help, Create assembly
/// constraints using the Assemble command, GUID-C1E8D955-784D-47DE-9DD4-C9C695051320: "The geometry you
/// select determines the available constraint type"). Joints, Symmetry, Angle-Undirected and Explicit
/// Reference Vector are not offered by Assemble.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AssembleSolution {
    /// Mate, faces facing (normals or axes opposed).
    MateMate,
    /// Flush: faces side by side.
    MateFlush,
    /// Mate of two axes pointing the same way.
    MateAligned,
    /// Angle, directed (right-hand rule).
    AngleDirected,
    /// Tangent, inside.
    TangentInside,
    /// Tangent, outside.
    TangentOutside,
    /// Insert, opposed.
    InsertOpposed,
    /// Insert, aligned.
    InsertAligned,
}

impl AssembleSolution {
    /// The relationship kind for this solution with an offset (metres) or angle (radians).
    pub fn kind(self, value: f64) -> RelationKind {
        match self {
            AssembleSolution::MateMate => RelationKind::Mate { offset: value, sense: AxisSense::Opposed },
            AssembleSolution::MateFlush => RelationKind::Flush { offset: value },
            AssembleSolution::MateAligned => RelationKind::Mate { offset: value, sense: AxisSense::Aligned },
            AssembleSolution::AngleDirected => {
                RelationKind::Angle { angle: value, solution: AngleSolution::Directed }
            }
            AssembleSolution::TangentInside => RelationKind::Tangent { offset: value, inside: true },
            AssembleSolution::TangentOutside => RelationKind::Tangent { offset: value, inside: false },
            AssembleSolution::InsertOpposed => RelationKind::Insert { offset: value, aligned: false },
            AssembleSolution::InsertAligned => RelationKind::Insert { offset: value, aligned: true },
        }
    }
}

/// The solutions Assemble offers for two picked selections, in Inventor's order; empty when nothing fits
/// (both on one component, a joint origin picked).
pub fn possible_relations(first: &Selection, second: &Selection) -> Vec<AssembleSolution> {
    let round = |g: &Geometry| matches!(g, Geometry::Cylinder { .. } | Geometry::Sphere { .. });
    let linear = |g: &Geometry| {
        matches!(g, Geometry::Axis { .. } | Geometry::Cylinder { .. } | Geometry::Circle { .. })
    };
    let (g1, g2) = (&first.geometry, &second.geometry);
    let offered = |solution: &AssembleSolution| match solution {
        AssembleSolution::MateAligned => linear(g1) && linear(g2),
        AssembleSolution::TangentInside | AssembleSolution::TangentOutside => round(g1) || round(g2),
        _ => true,
    };
    [
        AssembleSolution::MateMate,
        AssembleSolution::MateFlush,
        AssembleSolution::MateAligned,
        AssembleSolution::AngleDirected,
        AssembleSolution::TangentInside,
        AssembleSolution::TangentOutside,
        AssembleSolution::InsertOpposed,
        AssembleSolution::InsertAligned,
    ]
    .into_iter()
    .filter(|solution| {
        offered(solution) && Relationship::new("", solution.kind(0.0), *first, *second).is_valid()
    })
    .collect()
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
#[derive(Clone)]
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
    /// A locked joint: where its first origin stays relative to the second.
    locked: Option<LockedPose>,
    /// A joint's angular and linear limits.
    ranges: [Option<JointRange>; 2],
}

impl Document {
    /// The world pose of a component that can move (a body, robot or imported assembly).
    pub(crate) fn owner_pose(&self, item: AnyId) -> Option<DPose3> {
        // A work feature of the assembly holds still in the world; its geometry is given in the world.
        if item.kind == Kind::WorkFeature {
            return self.work_shape(gc_core::Id::from_raw(item.raw)).map(|_| DPose3::IDENTITY);
        }
        self.component_origin(Occurrence::Item(item)).map(|o| o.pose)
    }

    /// A selection of a work plane, axis or point of the assembly, for a relationship (Inventor 2026 help, To
    /// Place Mate or Flush Constraints in Assemblies, GUID-0F500089-4F69-41C5-AFE8-B58AC9E8E15E: "Select the
    /// first point, curve, face, or work feature"). Its geometry is read from
    /// the work feature whenever the relationships are solved, so the relationship follows it when it
    /// changes. `None` for a work feature that does not exist or does not work out.
    pub fn work_feature_selection(&self, id: gc_core::Id<crate::design::WorkFeature>) -> Option<Selection> {
        Some(Selection { occurrence: Occurrence::Item(id.erase()), geometry: self.work_geometry(id)? })
    }

    /// The geometry of a work feature in the world: a plane, an axis or a point.
    fn work_geometry(&self, id: gc_core::Id<crate::design::WorkFeature>) -> Option<Geometry> {
        Some(match self.work_shape(id)? {
            crate::placement::WorkShape::Plane(pose) => Geometry::Plane {
                point: pose.translation.to_array(),
                normal: (pose.rotation * DVec3::Z).to_array(),
            },
            crate::placement::WorkShape::Axis { point, direction } => {
                Geometry::Axis { point: point.to_array(), direction: direction.to_array() }
            }
            crate::placement::WorkShape::Point(point) => Geometry::Point(point.to_array()),
        })
    }

    /// True when a selection's owner can take part in a relationship: a component, or a work feature of the
    /// assembly that works out.
    pub(crate) fn relatable(&self, occurrence: Occurrence) -> bool {
        match occurrence {
            Occurrence::Item(item) if item.kind == Kind::WorkFeature => {
                self.work_shape(gc_core::Id::from_raw(item.raw)).is_some()
            }
            _ => self.has_occurrence(occurrence),
        }
    }

    /// The geometry of a selection in its owner's frame (a part's geometry goes through the part's frame).
    fn in_owner(&self, selection: &Selection) -> Option<Geometry> {
        if let Occurrence::Item(item) = selection.occurrence
            && item.kind == Kind::WorkFeature
        {
            return self.work_geometry(gc_core::Id::from_raw(item.raw));
        }
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

    /// A joint origin with its alignment direction taken from another selection (Align 1 on the moving
    /// component, Align 2 on the stationary one): a planar face gives its normal, an edge or axis its
    /// direction, a point the direction from the origin to it; a work plane, axis or point of the
    /// assembly may be picked as well. `None` when the direction runs along the origin's Z axis or a
    /// component is missing. Use the result as the relationship's selection.
    pub fn joint_origin_aligned(
        &self,
        origin: &Selection,
        align: &Selection,
        invert: bool,
    ) -> Option<Selection> {
        let frame = self.component_origin(origin.occurrence)?.pose;
        let pose = self.owner_pose(align.occurrence.owner())?;
        let geometry = self.in_owner(align)?;
        let world = match geometry {
            Geometry::Point(point) | Geometry::Sphere { centre: point, .. } => {
                let (at, _, _) = origin.geometry.joint_frame();
                pose.translation + pose.rotation * DVec3::from_array(point)
                    - (frame.translation + frame.rotation * at)
            }
            other => pose.rotation * DVec3::from_array(direction_of(&other)?),
        };
        let local = frame.rotation.inverse() * world;
        let geometry = origin.geometry.aligned_to(local.to_array(), invert)?;
        Some(Selection { occurrence: origin.occurrence, geometry })
    }

    /// Where a selection is in the world: a plane's or axis's point, a circle's or sphere's centre, a
    /// point, a joint origin. `None` for a missing component.
    pub fn selection_point(&self, selection: &Selection) -> Option<DVec3> {
        let pose = self.owner_pose(selection.occurrence.owner())?;
        let (point, _, _) = self.in_owner(selection)?.joint_frame();
        Some(pose.translation + pose.rotation * point)
    }

    /// The relationships as Free Move shows them (Inventor 2026 help, Move assembly components,
    /// GUID-9796C22B-23C1-4D69-BC2E-54056E46F255: "The Free Move command displays component relationships
    /// as an elastic band"): for each relationship that is not suppressed, where its two selections are
    /// in the world and whether it holds where the components stand now.
    pub fn relationship_bands(&self) -> Vec<RelationBand> {
        let Ok((owners, terms)) = self.gather() else { return Vec::new() };
        let poses: Vec<DPose3> = owners.iter().map(|o| o.pose).collect();
        terms
            .iter()
            .filter_map(|term| {
                let relationship = self.get(term.id)?;
                let ends =
                    [self.selection_point(&relationship.first)?, self.selection_point(&relationship.second)?];
                let satisfied = term_residuals(term, &poses).iter().all(|r| r.abs() <= 1e-7);
                Some(RelationBand { id: term.id, ends, satisfied })
            })
            .collect()
    }

    /// True when a component stands where its relationships do not hold (dropped there by Free Move or
    /// Free Rotate): Update would move something.
    pub fn needs_update(&self) -> bool {
        self.relationship_bands().iter().any(|band| !band.satisfied)
    }

    /// A selection as a line in the world: a point and a unit direction (a plane's normal, an axis's or
    /// cylinder's direction, a circle's normal, a frame's Z). `None` for a point or sphere, or a missing
    /// component.
    pub fn selection_axis(&self, selection: &Selection) -> Option<(DVec3, DVec3)> {
        let geometry = self.in_owner(selection)?;
        let pose = self.owner_pose(selection.occurrence.owner())?;
        let direction = (pose.rotation * DVec3::from_array(direction_of(&geometry)?)).normalize_or_zero();
        let point = match geometry {
            Geometry::Plane { point, .. }
            | Geometry::Axis { point, .. }
            | Geometry::Cylinder { point, .. } => point,
            Geometry::Circle { centre, .. } => centre,
            Geometry::Frame { origin, .. } => origin,
            Geometry::Point(_) | Geometry::Sphere { .. } => return None,
        };
        (direction.length() > 0.5)
            .then(|| (pose.translation + pose.rotation * DVec3::from_array(point), direction))
    }

    /// What Predict Offset and Orientation proposes for two selections as they lie now: Mate when two
    /// planes face each other (normals opposite), otherwise Flush, with their current distance as the
    /// offset; for other geometry a Mate at its current offset (axes: the closer sense). `None` when a
    /// component does not exist.
    pub fn predict_relationship(&self, first: &Selection, second: &Selection) -> Option<RelationKind> {
        // Nothing to offer from geometry that is not a number (the offset would be NaN).
        if !first.geometry.is_finite() || !second.geometry.is_finite() {
            return None;
        }
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
        let held: Vec<AnyId> = held.iter().map(|h| self.solver_owner(*h)).collect();
        for owner in &mut owners {
            if held.contains(&owner.item) {
                owner.free = false;
            }
        }
        let start: Vec<DPose3> = owners.iter().map(|o| o.pose).collect();
        let mut poses = start.clone();
        // Free components tied together by relationships form islands that do not affect one another:
        // each is solved on its own, and an island already satisfied is left alone (adding a relationship
        // re-solves only its island).
        for (free, island) in islands(&owners, &terms) {
            let subset: Vec<&Term> = island.iter().map(|i| &terms[*i]).collect();
            let satisfied = subset.iter().all(|t| term_residuals(t, &poses).iter().all(|r| r.abs() <= 1e-9));
            if satisfied {
                continue;
            }
            let subset: Vec<Term> = subset.into_iter().cloned().collect();
            // Fast path: one component at a time out from what holds still (a chain is a row of six-unknown
            // problems instead of one big one). Anything left unsatisfied (a loop, a component that needs a
            // later one to settle, a conflict) is solved as a whole from where it started, as before.
            let before: Vec<DPose3> = poses.clone();
            if staged(&owners, &free, &subset, &mut poses) {
                continue;
            }
            poses.copy_from_slice(&before);
            rough_turns(&owners, &subset, &mut poses);
            least_squares_on(&free, &subset, &mut poses);
        }
        let sick: Vec<Id<Relationship>> = terms
            .iter()
            .filter(|t| term_residuals(t, &poses).iter().any(|r| r.abs() > 1e-7))
            .map(|t| t.id)
            .collect();
        if !sick.is_empty() {
            return Err(Unsolved { sick });
        }
        // A rigid unit's components all move as the one that stands for it.
        let mut moved = Vec::new();
        for ((o, now), before) in owners.iter().zip(&poses).zip(&start) {
            if !(o.free && *now != *before) {
                continue;
            }
            let shift = *now * before.inverse();
            for member in self.rigid_unit_members(o.item) {
                if member == o.item {
                    moved.push((member, *now));
                } else if let Some(pose) = self.owner_pose(member) {
                    moved.push((member, shift * pose));
                }
            }
        }
        Ok(moved)
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
        let active = self.active_position();
        let mut owners: Vec<Owner> = Vec::new();
        let mut terms: Vec<Term> = Vec::new();
        // A rigid subassembly is one owner, stood for by its first component; it is held when any of its
        // components is grounded.
        let index_of = |doc: &Document, owners: &mut Vec<Owner>, item: AnyId| -> Option<usize> {
            let item = doc.solver_owner(item);
            if let Some(i) = owners.iter().position(|o| o.item == item) {
                return Some(i);
            }
            let pose = doc.owner_pose(item)?;
            let free = item.kind != Kind::WorkFeature
                && !doc
                    .rigid_unit_members(item)
                    .iter()
                    .any(|m| doc.component_flag(Occurrence::Item(*m), ComponentFlag::Grounded));
            owners.push(Owner { item, pose, free });
            Some(owners.len() - 1)
        };
        // A selection's geometry in the frame of the owner that stands for it.
        let in_solver_owner = |doc: &Document, selection: &Selection| -> Option<Geometry> {
            let geometry = doc.in_owner(selection)?;
            let item = selection.occurrence.owner();
            let owner = doc.solver_owner(item);
            if owner == item {
                return Some(geometry);
            }
            let local = doc.owner_pose(owner)?.inverse() * doc.owner_pose(item)?;
            Some(transformed(&geometry, &local))
        };
        for (id, relationship) in self.relationships().iter() {
            // The active positional representation may suppress, bring back or change relationships.
            let position = active.as_ref();
            let suppressed = position
                .and_then(|p| p.suppression.iter().find(|(r, _)| *r == id).map(|(_, s)| *s))
                .unwrap_or_else(|| self.is_inactive(id.erase()));
            if suppressed {
                continue;
            }
            let value = position.and_then(|p| p.values.iter().find(|(r, _)| *r == id).map(|(_, v)| *v));
            let relationship = &match value {
                Some(v) => Relationship { kind: with_value(&relationship.kind, v), ..relationship.clone() },
                None => relationship.clone(),
            };
            let (Some(a), Some(b), Some(g1), Some(g2)) = (
                index_of(self, &mut owners, relationship.first.occurrence.owner()),
                index_of(self, &mut owners, relationship.second.occurrence.owner()),
                in_solver_owner(self, &relationship.first),
                in_solver_owner(self, &relationship.second),
            ) else {
                return Err(Unsolved { sick: vec![id] });
            };
            // Within one rigid unit: kept as it is, nothing to solve.
            if a == b {
                continue;
            }
            let third = match relationship.third() {
                Some(selection) => {
                    let (Some(c), Some(g3)) = (
                        index_of(self, &mut owners, selection.occurrence.owner()),
                        in_solver_owner(self, selection),
                    ) else {
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
                locked: self.joint_condition(id).and_then(|c| c.locked),
                ranges: self.joint_condition(id).map_or([None, None], |c| [c.angular, c.linear]),
            });
        }
        Ok((owners, terms))
    }
}

impl Document {
    /// Lock and Protect of a joint.
    /// Where a joint stands now: its turn about its Z axis (radians) and slide along it (metres), the values
    /// its Limits are measured in. `None` for a constraint or a missing component.
    pub fn joint_position(&self, id: Id<Relationship>) -> Option<(f64, f64)> {
        let r = self.get(id)?;
        if !is_joint_kind(&r.kind) {
            return None;
        }
        let (g1, g2) = (self.in_owner(&r.first)?, self.in_owner(&r.second)?);
        let (p1, p2) =
            (self.owner_pose(r.first.occurrence.owner())?, self.owner_pose(r.second.occurrence.owner())?);
        Some(joint_values(&p1, &g1, &p2, &g2))
    }

    pub fn joint_condition(&self, id: Id<Relationship>) -> Option<JointCondition> {
        self.timeline().and_then(|(_, t)| t.components.joint_conditions.get(&id).copied())
    }

    /// Where a joint's first origin stands in its second's frame now (for Lock). `None` for a constraint or
    /// a missing component.
    pub(crate) fn lock_pose(&self, relationship: &Relationship) -> Option<LockedPose> {
        if !is_joint(&relationship.kind) {
            return None;
        }
        let g1 = self.in_owner(&relationship.first)?;
        let g2 = self.in_owner(&relationship.second)?;
        let pose1 = self.owner_pose(relationship.first.occurrence.owner())?;
        let pose2 = self.owner_pose(relationship.second.occurrence.owner())?;
        let (p1, z1, x1) = joint_world(&pose1, &g1);
        let (p2, z2, x2) = joint_world(&pose2, &g2);
        let y2 = z2.cross(x2);
        let to = |v: DVec3| [v.dot(x2), v.dot(y2), v.dot(z2)];
        Some(LockedPose { origin: to(p1 - p2), z: to(z1), x: to(x1) })
    }

    /// The freedom of each protected joint's moving component (Protect's check).
    pub(crate) fn protected_freedom(&self) -> Vec<(Id<Relationship>, u8)> {
        let Some((_, timeline)) = self.timeline() else { return Vec::new() };
        timeline
            .components
            .joint_conditions
            .iter()
            .filter(|(_, c)| c.protected)
            .filter_map(|(id, _)| {
                let r = self.get(*id)?;
                Some((*id, self.component_dof(r.first.occurrence.owner())?.total))
            })
            .collect()
    }
}

/// True for the joint kinds (Lock and Protect apply to them only).
pub fn is_joint_kind(kind: &RelationKind) -> bool {
    is_joint(kind)
}

/// A joint origin in the world: point, Z, X.
fn joint_world(pose: &DPose3, geometry: &Geometry) -> (DVec3, DVec3, DVec3) {
    let (p, z, x) = geometry.joint_frame();
    (pose.translation + pose.rotation * p, pose.rotation * z, pose.rotation * x)
}

/// A joint's turn about its Z axis (radians) and slide along it (metres), as the first origin stands in the
/// second's frame.
pub(crate) fn joint_values(pose1: &DPose3, g1: &Geometry, pose2: &DPose3, g2: &Geometry) -> (f64, f64) {
    let (p1, _, x1) = joint_world(pose1, g1);
    let (p2, z2, x2) = joint_world(pose2, g2);
    let angle = gc_math::fm::atan2(z2.dot(x2.cross(x1)), x2.dot(x1));
    (angle, (p1 - p2).dot(z2))
}

/// The residuals of a joint's limits: how far its turn and slide are from what the ranges ask (zero
/// within a free range). One row per range, whatever the pose, so the rows stay the same.
fn range_residuals(term: &Term, poses: &[DPose3]) -> Vec<f64> {
    if term.ranges.iter().all(Option::is_none) {
        return Vec::new();
    }
    let (angle, slide) =
        joint_values(&poses[term.owners[0]], &term.geometry[0], &poses[term.owners[1]], &term.geometry[1]);
    let wrap = |a: f64| gc_math::fm::atan2(gc_math::fm::sin(a), gc_math::fm::cos(a));
    let mut out = Vec::with_capacity(2);
    if let Some(range) = term.ranges[0] {
        out.push(wrap(angle - range.target(angle)));
    }
    if let Some(range) = term.ranges[1] {
        out.push(slide - range.target(slide));
    }
    out
}

fn term_residuals(term: &Term, poses: &[DPose3]) -> Vec<f64> {
    if is_joint(&term.kind) {
        let mut out = plain_residuals(term, poses);
        out.extend(range_residuals(term, poses));
        return out;
    }
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
pub(crate) fn value_of(kind: &RelationKind) -> Option<f64> {
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
pub(crate) fn with_value(kind: &RelationKind, value: f64) -> RelationKind {
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
        if let Some(lock) = term.locked {
            // Locked: the first origin stays where it was in the second's frame.
            let y2 = z2.cross(x2);
            let from = |v: [f64; 3]| x2 * v[0] + y2 * v[1] + z2 * v[2];
            push3(&mut out, p1 - (p2 + from(lock.origin)));
            push3(&mut out, z1 - from(lock.z));
            push3(&mut out, x1 - from(lock.x));
            return out;
        }
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
/// Islands of free owners tied by relationships: per island, its free owners and the indices of the terms
/// that touch it. Terms between fixed owners only belong to no island (they are only checked).
fn islands(owners: &[Owner], terms: &[Term]) -> Vec<(Vec<usize>, Vec<usize>)> {
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let mut parent: Vec<usize> = (0..owners.len()).collect();
    let touched = |t: &Term| -> Vec<usize> {
        t.owners
            .iter()
            .copied()
            .chain(t.third.as_ref().map(|(i, _)| *i))
            .filter(|i| owners[*i].free)
            .collect()
    };
    for term in terms {
        let free = touched(term);
        for pair in free.windows(2) {
            let (a, b) = (root(&mut parent, pair[0]), root(&mut parent, pair[1]));
            if a != b {
                parent[a] = b;
            }
        }
    }
    let mut by_root: std::collections::BTreeMap<usize, (Vec<usize>, Vec<usize>)> = Default::default();
    for (i, owner) in owners.iter().enumerate() {
        if owner.free {
            let r = root(&mut parent, i);
            by_root.entry(r).or_default().0.push(i);
        }
    }
    for (index, term) in terms.iter().enumerate() {
        if let Some(first) = touched(term).first() {
            let r = root(&mut parent, *first);
            by_root.entry(r).or_default().1.push(index);
        }
    }
    by_root.into_values().filter(|(_, terms)| !terms.is_empty()).collect()
}

/// Solves an island one component at a time, breadth first from the fixed components it touches (from its
/// first component when it touches none, which then stays put): each component meets the relationships
/// to the components already placed. True when every relationship of the island is satisfied after.
fn staged(owners: &[Owner], free: &[usize], terms: &[Term], poses: &mut [DPose3]) -> bool {
    let members = |t: &Term| -> Vec<usize> {
        t.owners.iter().copied().chain(t.third.as_ref().map(|(i, _)| *i)).collect()
    };
    let mut placed: Vec<bool> = owners.iter().map(|o| !o.free).collect();
    let mut queue: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
    let mut queued = vec![false; owners.len()];
    let seed = |queue: &mut std::collections::VecDeque<usize>, queued: &mut Vec<bool>, placed: &[bool]| {
        for term in terms {
            let m = members(term);
            if m.iter().any(|i| placed[*i]) {
                for i in m {
                    if !placed[i] && !queued[i] {
                        queued[i] = true;
                        queue.push_back(i);
                    }
                }
            }
        }
    };
    seed(&mut queue, &mut queued, &placed);
    if queue.is_empty() {
        // Floating: the first component holds still.
        let Some(first) = free.first() else { return true };
        placed[*first] = true;
        seed(&mut queue, &mut queued, &placed);
    }
    // Terms by owner, to find a component's relationships quickly.
    let mut by_owner: Vec<Vec<usize>> = vec![Vec::new(); owners.len()];
    for (index, term) in terms.iter().enumerate() {
        for i in members(term) {
            by_owner[i].push(index);
        }
    }
    while let Some(node) = queue.pop_front() {
        let ready: Vec<Term> = by_owner[node]
            .iter()
            .map(|t| &terms[*t])
            .filter(|t| members(t).iter().all(|i| *i == node || placed[*i]))
            .cloned()
            .collect();
        placed[node] = true;
        for t in &by_owner[node] {
            for i in members(&terms[*t]) {
                if !placed[i] && !queued[i] {
                    queued[i] = true;
                    queue.push_back(i);
                }
            }
        }
        if ready.iter().all(|t| term_residuals(t, poses).iter().all(|r| r.abs() <= 1e-9)) {
            continue;
        }
        // Only this component may move (the rough turns too).
        let local: Vec<Owner> =
            owners.iter().enumerate().map(|(i, o)| Owner { free: i == node, ..*o }).collect();
        rough_turns(&local, &ready, poses);
        least_squares_on(&[node], &ready, poses);
    }
    free.iter().all(|i| placed[*i])
        && terms.iter().all(|t| term_residuals(t, poses).iter().all(|r| r.abs() <= 1e-9))
}

/// Damped least squares over the poses of the owners `free`. The normal matrix is stored by its envelope
/// with the owners in breadth-first order, so a chain (or any long, thin assembly) costs about as much per
/// component as one component on its own.
fn least_squares_on(free: &[usize], terms: &[Term], poses: &mut [DPose3]) {
    if free.is_empty() || terms.is_empty() {
        return;
    }
    // Owners each term touches (free ones only), and terms by owner.
    let mut touched: Vec<Vec<usize>> = Vec::with_capacity(terms.len());
    let mut by_owner: Vec<Vec<usize>> = vec![Vec::new(); poses.len()];
    for (index, term) in terms.iter().enumerate() {
        let mut m: Vec<usize> = term
            .owners
            .iter()
            .copied()
            .chain(term.third.as_ref().map(|(i, _)| *i))
            .filter(|i| free.contains(i))
            .collect();
        m.sort_unstable();
        m.dedup();
        for i in &m {
            by_owner[*i].push(index);
        }
        touched.push(m);
    }
    // Breadth-first order keeps the envelope narrow.
    let mut order: Vec<usize> = Vec::with_capacity(free.len());
    let mut seen = vec![false; poses.len()];
    for root in free {
        if seen[*root] {
            continue;
        }
        seen[*root] = true;
        let mut queue = std::collections::VecDeque::from([*root]);
        while let Some(o) = queue.pop_front() {
            order.push(o);
            for t in &by_owner[o] {
                for i in &touched[*t] {
                    if !seen[*i] {
                        seen[*i] = true;
                        queue.push_back(*i);
                    }
                }
            }
        }
    }
    let mut block = vec![usize::MAX; poses.len()];
    for (k, o) in order.iter().enumerate() {
        block[*o] = k;
    }
    let n = order.len() * 6;
    // The envelope: the first column of each row of the (symmetric) normal matrix.
    let mut first: Vec<usize> = (0..n).collect();
    for m in &touched {
        let Some(lo) = m.iter().map(|o| block[*o] * 6).min() else { continue };
        for o in m {
            for p in 0..6 {
                let row = block[*o] * 6 + p;
                first[row] = first[row].min(lo);
            }
        }
    }
    let residuals =
        |poses: &[DPose3]| -> Vec<Vec<f64>> { terms.iter().map(|t| term_residuals(t, poses)).collect() };
    let cost = |r: &[Vec<f64>]| r.iter().flatten().map(|v| v * v).sum::<f64>();
    let mut r = residuals(poses);
    let mut lambda = 1e-3;
    for _ in 0..200 {
        let current = cost(&r);
        if current < 1e-24 {
            break;
        }
        // The Jacobian by forward differences, term by term: jac[t][k] holds the columns of the k-th owner
        // the term touches (rows × 6, row-major).
        let h = 1e-7;
        let mut jac: Vec<Vec<Vec<f64>>> =
            touched.iter().zip(&r).map(|(m, rows)| vec![vec![0.0; rows.len() * 6]; m.len()]).collect();
        for o in &order {
            for p in 0..6 {
                let mut step = [0.0; 6];
                step[p] = h;
                let saved = poses[*o];
                poses[*o] = stepped(&saved, &step);
                for t in &by_owner[*o] {
                    let moved = term_residuals(&terms[*t], poses);
                    let Some(k) = touched[*t].iter().position(|i| i == o) else { continue };
                    let cols = &mut jac[*t][k];
                    for (row, (after, before)) in moved.iter().zip(&r[*t]).enumerate() {
                        cols[row * 6 + p] = (after - before) / h;
                    }
                }
                poses[*o] = saved;
            }
        }
        // Normal matrix (lower envelope) and gradient.
        let mut normal: Vec<Vec<f64>> = (0..n).map(|i| vec![0.0; i - first[i] + 1]).collect();
        let mut gradient = vec![0.0; n];
        for (t, m) in touched.iter().enumerate() {
            let rows = r[t].len();
            for (ka, a) in m.iter().enumerate() {
                let ja = &jac[t][ka];
                for row in 0..rows {
                    for p in 0..6 {
                        gradient[block[*a] * 6 + p] -= ja[row * 6 + p] * r[t][row];
                    }
                }
                for (kb, b) in m.iter().enumerate() {
                    if block[*b] > block[*a] {
                        continue;
                    }
                    let jb = &jac[t][kb];
                    for p in 0..6 {
                        let i = block[*a] * 6 + p;
                        for q in 0..6 {
                            let j = block[*b] * 6 + q;
                            if j > i {
                                continue;
                            }
                            let sum: f64 = (0..rows).map(|row| ja[row * 6 + p] * jb[row * 6 + q]).sum();
                            normal[i][j - first[i]] += sum;
                        }
                    }
                }
            }
        }
        let mut improved = false;
        for _ in 0..12 {
            let mut a = normal.clone();
            for (i, row) in a.iter_mut().enumerate() {
                let d = i - first[i];
                row[d] += lambda * (1.0 + normal[i][d]);
            }
            let Some(dx) = envelope_solve(a, &first, gradient.clone()) else {
                lambda *= 10.0;
                continue;
            };
            let mut candidate = poses.to_vec();
            for o in &order {
                let k = block[*o] * 6;
                candidate[*o] = stepped(&poses[*o], &dx[k..k + 6]);
            }
            let r_new = residuals(&candidate);
            let new_cost = cost(&r_new);
            if new_cost < current {
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

/// Solves `a x = b` for a symmetric positive definite `a` given by the lower envelope of its rows (row
/// `i` holds columns `first[i]..=i`), by Cholesky factorisation within the envelope.
fn envelope_solve(mut a: Vec<Vec<f64>>, first: &[usize], mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for i in 0..n {
        for j in first[i]..=i {
            let start = first[i].max(first[j]);
            let mut sum = a[i][j - first[i]];
            for k in start..j {
                sum -= a[i][k - first[i]] * a[j][k - first[j]];
            }
            if i == j {
                if sum.is_nan() || sum <= 1e-300 {
                    return None;
                }
                a[i][i - first[i]] = sum.sqrt();
            } else {
                a[i][j - first[i]] = sum / a[j][j - first[j]];
            }
        }
    }
    // L y = b, then Lᵀ x = y.
    for i in 0..n {
        let mut sum = b[i];
        for k in first[i]..i {
            sum -= a[i][k - first[i]] * b[k];
        }
        b[i] = sum / a[i][i - first[i]];
    }
    for i in (0..n).rev() {
        b[i] /= a[i][i - first[i]];
        let x = b[i];
        for k in first[i]..i {
            b[k] -= a[i][k - first[i]] * x;
        }
    }
    b.iter().all(|v| v.is_finite()).then_some(b)
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

/// The Jacobian (row-major, `m × 6`) of the residuals of the relationships on a component with respect
/// to a small slide (world X, Y, Z) and turn (about world axes through its origin); `None` when no
/// relationship touches it.
fn dof_jacobian(gathered: Option<&(Vec<Owner>, Vec<Term>)>, item: AnyId) -> Option<(Vec<f64>, usize)> {
    let (owners, terms) = gathered?;
    let index = owners.iter().position(|o| o.item == item)?;
    let terms: Vec<&Term> = terms.iter().filter(|t| t.owners.contains(&index)).collect();
    let mut poses: Vec<DPose3> = owners.iter().map(|o| o.pose).collect();
    let residuals =
        |poses: &[DPose3]| -> Vec<f64> { terms.iter().flat_map(|t| term_residuals(t, poses)).collect() };
    let base = residuals(&poses);
    let m = base.len();
    let h = 1e-7;
    let mut jac = vec![0.0; m * 6];
    for p in 0..6 {
        let mut step = [0.0; 6];
        step[p] = h;
        let saved = poses[index];
        poses[index] = stepped(&saved, &step);
        let moved = residuals(&poses);
        poses[index] = saved;
        for row in 0..m {
            jac[row * 6 + p] = (moved[row] - base[row]) / h;
        }
    }
    Some((jac, m))
}

/// An orthonormal basis of the vectors (of length `n`) that every row leaves at zero, by Gram–Schmidt with
/// the same tolerance as [`rank`]; the basis comes from the unit vectors in order, so it is stable.
fn null_space(rows: &[Vec<f64>], n: usize) -> Vec<Vec<f64>> {
    let mut basis: Vec<Vec<f64>> = Vec::new();
    let orthogonalise = |v: &mut Vec<f64>, basis: &[Vec<f64>]| {
        for b in basis {
            let dot: f64 = v.iter().zip(b).map(|(x, y)| x * y).sum();
            v.iter_mut().zip(b).for_each(|(x, y)| *x -= dot * y);
        }
        v.iter().map(|x| x * x).sum::<f64>().sqrt()
    };
    for row in rows {
        let scale = row.iter().map(|x| x * x).sum::<f64>().sqrt();
        if scale < 1e-6 {
            continue;
        }
        let mut v: Vec<f64> = row.iter().map(|x| x / scale).collect();
        let norm = orthogonalise(&mut v, &basis);
        if norm > 1e-6 {
            basis.push(v.iter().map(|x| x / norm).collect());
        }
    }
    let fixed = basis.len();
    for k in 0..n {
        let mut v = vec![0.0; n];
        v[k] = 1.0;
        let norm = orthogonalise(&mut v, &basis);
        if norm > 1e-6 {
            basis.push(v.iter().map(|x| x / norm).collect());
        }
    }
    basis.split_off(fixed)
}

/// An axis a component can still turn about: a point on it and its direction (world).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DofAxis {
    /// A point on the axis.
    pub point: [f64; 3],
    /// Its unit direction.
    pub direction: [f64; 3],
}

/// What the degrees-of-freedom symbol of a component shows.
#[derive(Clone, Debug, PartialEq)]
pub struct DofSymbol {
    /// Where the symbol stands: the component's origin (world).
    pub origin: [f64; 3],
    /// Unit directions it can still slide along (0 to 3).
    pub translations: Vec<[f64; 3]>,
    /// Axes it can still turn about (0 to 3).
    pub rotations: Vec<DofAxis>,
    /// Components it is constrained to that are not fully constrained themselves (the symbol's cube).
    pub relative_to: Vec<AnyId>,
}

impl Document {
    /// The freedom a component has left under the relationships, the other components held still.
    /// Grounded: none; untouched by relationships: all six. `None` when it is not a component that moves.
    pub fn component_dof(&self, item: AnyId) -> Option<Dof> {
        self.owner_pose(item)?;
        let gathered = self.gather().ok();
        Some(self.dof_in(gathered.as_ref(), item))
    }

    /// True when the relationships let the component move at the rate `twist` (its origin's velocity, then
    /// its rotation vector) to first order, the other components held still.
    pub(crate) fn motion_allowed(&self, item: AnyId, twist: [f64; 6]) -> bool {
        let norm = twist.iter().map(|v| v * v).sum::<f64>().sqrt();
        if !(norm > 1e-12 && norm.is_finite()) {
            return false;
        }
        let gathered = self.gather().ok();
        let Some((jac, m)) = dof_jacobian(gathered.as_ref(), self.solver_owner(item)) else { return true };
        (0..m).all(|row| {
            let rate: f64 = (0..6).map(|c| jac[row * 6 + c] * twist[c] / norm).sum();
            rate.abs() <= 1e-6
        })
    }

    /// [`Document::component_dof`] from relationships gathered once.
    fn dof_in(&self, gathered: Option<&(Vec<Owner>, Vec<Term>)>, item: AnyId) -> Dof {
        // A rigid unit's components share its freedom, and a grounded one holds them all.
        if self
            .rigid_unit_members(item)
            .iter()
            .any(|m| self.component_flag(Occurrence::Item(*m), ComponentFlag::Grounded))
        {
            return Dof { total: 0, translation: 0 };
        }
        let Some((jac, m)) = dof_jacobian(gathered, self.solver_owner(item)) else {
            return Dof { total: 6, translation: 3 };
        };
        let fixed = rank(&jac, m, 6, &[0, 1, 2, 3, 4, 5]);
        let fixed_translation = rank(&jac, m, 6, &[0, 1, 2]);
        #[allow(clippy::cast_possible_truncation, reason = "ranks are at most 6")]
        Dof { total: (6 - fixed) as u8, translation: (3 - fixed_translation) as u8 }
    }

    /// The degrees-of-freedom symbol of a component (Inventor 2026 help, View degrees of freedom in
    /// assemblies, GUID-3BCBF614-CF92-4980-A965-80AEAFEF768E; Degrees of freedom in assemblies,
    /// GUID-6E131E5A-438B-4008-94F3-D3A3E92EF44C): the directions it can still slide along, the axes it can
    /// still turn about, and the under-constrained components it is constrained to (the symbol's cube).
    /// The other components are held still. `None` when it is not a component that moves.
    pub fn dof_symbol(&self, item: AnyId) -> Option<DofSymbol> {
        let pose = self.owner_pose(item)?;
        let origin = pose.translation;
        let mut symbol = DofSymbol {
            origin: origin.to_array(),
            translations: Vec::new(),
            rotations: Vec::new(),
            relative_to: Vec::new(),
        };
        if self.component_flag(Occurrence::Item(item), ComponentFlag::Grounded) {
            return Some(symbol);
        }
        let gathered = self.gather().ok();
        let (jac, m) = dof_jacobian(gathered.as_ref(), self.solver_owner(item)).unwrap_or_default();
        let rows: Vec<[f64; 6]> = (0..m)
            .map(|r| {
                let mut row = [0.0; 6];
                row.copy_from_slice(&jac[r * 6..r * 6 + 6]);
                row
            })
            .collect();
        // Pure translations: what the rows' translation parts leave free.
        let translation_rows: Vec<Vec<f64>> = rows.iter().map(|r| r[..3].to_vec()).collect();
        let slides = null_space(&translation_rows, 3);
        for slide in &slides {
            symbol.translations.push(DVec3::new(slide[0], slide[1], slide[2]).normalize().to_array());
        }
        // Turns: the free motions left once the slides are taken out, as screw axes.
        let full_rows: Vec<Vec<f64>> = rows
            .iter()
            .map(|r| r.to_vec())
            .chain(slides.iter().map(|t| vec![t[0], t[1], t[2], 0.0, 0.0, 0.0]))
            .collect();
        for twist in null_space(&full_rows, 6) {
            let v = DVec3::new(twist[0], twist[1], twist[2]);
            let w = DVec3::new(twist[3], twist[4], twist[5]);
            if w.length() < 1e-9 {
                continue;
            }
            let point = origin + w.cross(v) / w.length_squared();
            symbol.rotations.push(DofAxis { point: point.to_array(), direction: w.normalize().to_array() });
        }
        // The cube: components it is constrained to that can still move themselves.
        for (id, relationship) in self.relationships().iter() {
            if self.is_inactive(id.erase()) {
                continue;
            }
            let (a, b) = (relationship.first.occurrence.owner(), relationship.second.occurrence.owner());
            let other = if a == item {
                b
            } else if b == item {
                a
            } else {
                continue;
            };
            if other != item
                && !symbol.relative_to.contains(&other)
                && self.dof_in(gathered.as_ref(), other).total > 0
            {
                symbol.relative_to.push(other);
            }
        }
        Some(symbol)
    }

    /// Components whose degrees-of-freedom symbol is shown (iProperties, Occurrence tab, Degrees of
    /// Freedom), or all of them when the assembly's symbol is on (View tab, Degrees of Freedom).
    pub fn dof_shown(&self, item: AnyId) -> bool {
        let state = self.timeline().map(|(_, t)| t.components.clone()).unwrap_or_default();
        state.dof_all || state.dof_shown.contains(&Occurrence::Item(item))
    }

    /// The constraint status dot of a component.
    pub fn component_status(&self, item: AnyId) -> Option<ComponentStatus> {
        self.owner_pose(item)?;
        let gathered = self.gather().ok();
        Some(self.status_in(gathered.as_ref(), item))
    }

    /// The constraint status dots of all bodies, robots and assemblies at once (what the browser shows
    /// every frame): the relationships are gathered once.
    pub fn component_statuses(&self) -> std::collections::BTreeMap<AnyId, ComponentStatus> {
        let gathered = self.gather().ok();
        self.bodies()
            .iter()
            .map(|(id, _)| id.erase())
            .chain(self.robots().iter().map(|(id, _)| id.erase()))
            .chain(self.assemblies().iter().map(|(id, _)| id.erase()))
            .map(|item| (item, self.status_in(gathered.as_ref(), item)))
            .collect()
    }

    fn status_in(&self, gathered: Option<&(Vec<Owner>, Vec<Term>)>, item: AnyId) -> ComponentStatus {
        let Some(found) = gathered else {
            // A relationship refers to something missing: nothing can be said.
            return if self
                .relationships()
                .iter()
                .any(|(_, r)| r.first.occurrence.owner() == item || r.second.occurrence.owner() == item)
            {
                ComponentStatus::Unknown
            } else {
                ComponentStatus::Under
            };
        };
        let (owners, terms) = found;
        let poses: Vec<DPose3> = owners.iter().map(|o| o.pose).collect();
        let owner = self.solver_owner(item);
        let index = owners.iter().position(|o| o.item == owner);
        let sick = index.is_some_and(|index| {
            terms
                .iter()
                .filter(|t| t.owners.contains(&index))
                .any(|t| term_residuals(t, &poses).iter().any(|r| r.abs() > 1e-7))
        });
        if sick {
            ComponentStatus::Unknown
        } else if self.dof_in(gathered, item).total == 0 {
            ComponentStatus::Full
        } else {
            ComponentStatus::Under
        }
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
    /// The value Drive starts from for a relationship: its offset or angle as defined, and 0 for a
    /// Rotational joint (whose drive value is a turn from where it stands).
    pub fn drive_start(&self, id: Id<Relationship>) -> Option<f64> {
        match self.get(id)?.kind {
            RelationKind::Rotational { .. } => Some(0.0),
            kind => value_of(&kind),
        }
    }

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
        // A joint with Limits is driven to a position within them (its angular range when it has one,
        // otherwise its linear range), held there like the Limits tab's Current.
        let ranged = self.joint_condition(id).filter(|c| c.angular.is_some() || c.linear.is_some());
        if let (true, Some(condition)) = (is_joint_kind(&relationship.kind), ranged) {
            let (mut angular, mut linear) = (condition.angular, condition.linear);
            match (&mut angular, &mut linear) {
                (Some(r), _) | (None, Some(r)) => {
                    r.current = Some(gc_math::clamp_between(value, r.start, r.end))
                }
                (None, None) => {}
            }
            crate::apply(&mut preview, &Command::Cell(CellOp::SetJointLimits { id, angular, linear }))?;
            return Ok(self.moved_in(&preview));
        }
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
        Ok(self.moved_in(&preview))
    }

    /// The components that stand elsewhere in `preview` than here, with where they stand there.
    fn moved_in(&self, preview: &Document) -> Vec<(AnyId, DPose3)> {
        let items: Vec<AnyId> = self
            .bodies()
            .iter()
            .map(|(id, _)| id.erase())
            .chain(self.robots().iter().map(|(id, _)| id.erase()))
            .chain(self.assemblies().iter().map(|(id, _)| id.erase()))
            .collect();
        items
            .into_iter()
            .filter_map(|item| {
                let now = preview.owner_pose(item)?;
                (self.owner_pose(item)? != now).then_some((item, now))
            })
            .collect()
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

/// A geometry carried by a pose: from the frame `local` describes into the frame it is given in.
fn transformed(geometry: &Geometry, local: &DPose3) -> Geometry {
    let p = |v: [f64; 3]| (local.translation + local.rotation * DVec3::from_array(v)).to_array();
    let d = |v: [f64; 3]| (local.rotation * DVec3::from_array(v)).to_array();
    match *geometry {
        Geometry::Plane { point, normal } => Geometry::Plane { point: p(point), normal: d(normal) },
        Geometry::Axis { point, direction } => Geometry::Axis { point: p(point), direction: d(direction) },
        Geometry::Point(point) => Geometry::Point(p(point)),
        Geometry::Circle { centre, normal } => Geometry::Circle { centre: p(centre), normal: d(normal) },
        Geometry::Frame { origin, z, x } => Geometry::Frame { origin: p(origin), z: d(z), x: d(x) },
        Geometry::Cylinder { point, direction, radius } => {
            Geometry::Cylinder { point: p(point), direction: d(direction), radius }
        }
        Geometry::Sphere { centre, radius } => Geometry::Sphere { centre: p(centre), radius },
    }
}
