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
            Geometry::Point(point) => (v(point), DVec3::Z, Some(DVec3::X)),
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
    /// A circular edge into another: axes coincident and the circles' planes mated at `offset`. Rotation
    /// about the axis stays free.
    Insert {
        /// Distance between the circles' planes, metres.
        offset: f64,
        /// False (Opposed, the default): normals opposite, like a bolt head on the face around a hole.
        aligned: bool,
    },
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
}

impl Entity for Relationship {
    const KIND: Kind = Kind::Relationship;
    fn refs(&self, out: &mut Vec<AnyId>) {
        out.push(self.first.occurrence.owner());
        out.push(self.second.occurrence.owner());
    }
}

impl Relationship {
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
            RelationKind::Rigid { .. } | RelationKind::Rotational { .. } => true,
        };
        let joint = matches!(self.kind, RelationKind::Rigid { .. } | RelationKind::Rotational { .. });
        let frames = matches!(g1, Geometry::Frame { .. }) || matches!(g2, Geometry::Frame { .. });
        fits && (joint || !frames)
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
    }
}

/// The part of `v` across direction `d`.
fn across(v: DVec3, d: DVec3) -> DVec3 {
    v - d * v.dot(d)
}

/// The sign that the directions of a relationship should have relative to each other (−1 opposed, +1
/// the same), with Undirected settled from `now` (the dot product of the directions as they lie).
fn sense_sign(kind: &RelationKind, now: f64) -> f64 {
    match *kind {
        RelationKind::Rigid { flip } | RelationKind::Rotational { flip } => {
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
        RelationKind::Mate { sense: AxisSense::Undirected, .. } => {
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
        RelationKind::Rigid { .. } | RelationKind::Rotational { .. } => 0.0,
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
}

impl Document {
    /// The world pose of a component that can move (a body, robot or imported assembly).
    pub(crate) fn owner_pose(&self, item: AnyId) -> Option<DPose3> {
        self.component_origin(Occurrence::Item(item)).map(|o| o.pose)
    }

    /// The geometry of a selection in its owner's frame (a part's geometry goes through the part's frame).
    fn in_owner(&self, selection: &Selection) -> Option<Geometry> {
        let Occurrence::Part { .. } = selection.occurrence else { return Some(selection.geometry) };
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
            let (w1, w2) = (world(&owners[a].pose, &g1), world(&owners[b].pose, &g2));
            let now = directions(&w1.0, &w2.0).map_or(1.0, |(x, y)| x.dot(y));
            terms.push(Term {
                id,
                kind: relationship.kind,
                owners: [a, b],
                geometry: [g1, g2],
                sign: sense_sign(&relationship.kind, now),
            });
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
}

/// A joint origin in the world: point, Z, X.
fn joint_world(pose: &DPose3, geometry: &Geometry) -> (DVec3, DVec3, DVec3) {
    let (p, z, x) = geometry.joint_frame();
    (pose.translation + pose.rotation * p, pose.rotation * z, pose.rotation * x)
}

fn term_residuals(term: &Term, poses: &[DPose3]) -> Vec<f64> {
    let mut out = Vec::with_capacity(9);
    if let RelationKind::Rigid { .. } | RelationKind::Rotational { .. } = term.kind {
        let (p1, z1, x1) = joint_world(&poses[term.owners[0]], &term.geometry[0]);
        let (p2, z2, x2) = joint_world(&poses[term.owners[1]], &term.geometry[1]);
        let push3 = |out: &mut Vec<f64>, v: DVec3| out.extend([v.x, v.y, v.z]);
        push3(&mut out, p1 - p2);
        push3(&mut out, z1 - z2 * term.sign);
        if let RelationKind::Rigid { .. } = term.kind {
            push3(&mut out, x1 - x2);
        }
        return out;
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
        if let RelationKind::Rigid { .. } | RelationKind::Rotational { .. } = term.kind {
            joint_turns(owners, term, poses);
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
    if let RelationKind::Rigid { .. } = term.kind {
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
