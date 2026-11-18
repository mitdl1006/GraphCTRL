//! Grip Snap (Inventor 2026 help, Grip Snap move or rotate, GUID-E1CA109A-D5A1-4131-AC3A-AE217716DD8C):
//! an edge of a component is the reference; the component moves along it (Move along ray using reference
//! geometry) or turns about it (Rotate about line using reference geometry), by the offset or angle two
//! picked points give. The HUD reports the freedom left for the chosen line; Commit and Ground moves and
//! grounds what was manipulated.
//!
//! This module computes the poses and the freedom; picking and the drag belong to the viewport.

use gc_core::AnyId;
use gc_math::{DPose3, DQuat, DVec3};
use serde::{Deserialize, Serialize};

use crate::document::Document;

/// How the component moves, by the reference it was grabbed at (Inventor 2026 help, Grip Snap Move
/// Options Bar Reference, GUID-C030C32C-7E1E-4F37-BBDF-120412898D00).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GripMotion {
    /// A line: Reference Geometry Displacement Along Ray (the part of `to − from` along the line).
    MoveAlong,
    /// A line: Reference Geometry Rotate (about the line, by the angle from `from` to `to` seen along it).
    RotateAbout,
    /// A point: Reference Geometry Displacement (by `to − from`).
    Displace,
    /// A point: Reference Geometry Rotate (about the axis through the point that `from` and `to` give by
    /// their cross product, by the angle between them).
    RotateAboutPoint,
    /// A plane: Reference Geometry Displacement Within Plane (the part of `to − from` in the plane).
    MoveWithinPlane,
    /// A plane: Reference Geometry Displacement Along Normal (the part of `to − from` along the normal).
    MoveAlongNormal,
}

/// What the component was grabbed at: a point (vertex, work point), a line (edge, work axis) or a plane
/// (planar face, work plane), in the world.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum GripReference {
    /// A point.
    Point([f64; 3]),
    /// A line.
    Line(GripLine),
    /// A plane: a point on it and its normal (any length but zero).
    Plane {
        /// A point on the plane.
        point: [f64; 3],
        /// Its normal.
        normal: [f64; 3],
    },
}

/// The reference line (an edge): a point on it and its direction, in the world.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GripLine {
    /// A point on the line.
    pub point: [f64; 3],
    /// Its direction (any length but zero).
    pub direction: [f64; 3],
}

impl GripLine {
    /// The line through an edge's two ends; `None` for an edge of no length.
    pub fn through(a: [f64; 3], b: [f64; 3]) -> Option<GripLine> {
        let d = DVec3::from(b) - DVec3::from(a);
        (d.length() > 1e-12 && d.is_finite()).then_some(GripLine { point: a, direction: d.to_array() })
    }

    fn unit(&self) -> Option<(DVec3, DVec3)> {
        let p = DVec3::from(self.point);
        let d = DVec3::from(self.direction);
        let len = d.length();
        (len > 1e-12 && len.is_finite() && p.is_finite()).then(|| (p, d / len))
    }
}

/// Where a pose goes: moved along `line` by the part of `to − from` along it, or turned about `line` by
/// the angle from `from` to `to` seen along it. `None` when the line has no direction or (turning) a point
/// lies on the line.
pub fn grip_pose(
    pose: DPose3,
    motion: GripMotion,
    line: GripLine,
    from: [f64; 3],
    to: [f64; 3],
) -> Option<DPose3> {
    grip_pose_on(pose, motion, GripReference::Line(line), from, to)
}

/// Where a pose goes for a motion about a reference of any kind; `None` when the motion does not go with
/// the reference (a plane motion on a point, ...), the reference is degenerate, or the points give no
/// motion where one is needed (a point on the axis, no turn axis).
pub fn grip_pose_on(
    pose: DPose3,
    motion: GripMotion,
    reference: GripReference,
    from: [f64; 3],
    to: [f64; 3],
) -> Option<DPose3> {
    let (from, to) = (DVec3::from(from), DVec3::from(to));
    if !(from.is_finite() && to.is_finite()) {
        return None;
    }
    let shifted = |by: DVec3| Some(DPose3::from_parts(pose.translation + by, pose.rotation));
    let turned = |centre: DVec3, turn: DQuat| {
        Some(DPose3::from_parts(
            centre + turn * (pose.translation - centre),
            (turn * pose.rotation).normalize(),
        ))
    };
    match (motion, reference) {
        (GripMotion::Displace, GripReference::Point(p)) => {
            if !DVec3::from(p).is_finite() {
                return None;
            }
            shifted(to - from)
        }
        (GripMotion::RotateAboutPoint, GripReference::Point(p)) => {
            let p = DVec3::from(p);
            let (u, w) = (from - p, to - p);
            let axis = u.cross(w);
            if !p.is_finite() || axis.length() < 1e-12 {
                return None;
            }
            let angle = gc_math::fm::atan2(axis.length(), u.dot(w));
            turned(p, DQuat::from_axis_angle(axis.normalize(), angle))
        }
        (
            GripMotion::MoveWithinPlane | GripMotion::MoveAlongNormal,
            GripReference::Plane { point, normal },
        ) => {
            let n = DVec3::from(normal);
            if !(n.length() > 1e-12 && n.is_finite() && DVec3::from(point).is_finite()) {
                return None;
            }
            let n = n.normalize();
            let d = to - from;
            let along = n * d.dot(n);
            shifted(if motion == GripMotion::MoveAlongNormal { along } else { d - along })
        }
        (GripMotion::MoveAlong, GripReference::Line(line)) => {
            let (_, a) = line.unit()?;
            shifted(a * (to - from).dot(a))
        }
        (GripMotion::RotateAbout, GripReference::Line(line)) => {
            let (p, a) = line.unit()?;
            let flat = |v: DVec3| {
                let r = v - p;
                r - a * r.dot(a)
            };
            let (u, w) = (flat(from), flat(to));
            if u.length() < 1e-12 || w.length() < 1e-12 {
                return None;
            }
            let angle = gc_math::fm::atan2(a.dot(u.cross(w)), u.dot(w));
            turned(p, DQuat::from_axis_angle(a, angle))
        }
        (
            GripMotion::MoveAlong
            | GripMotion::RotateAbout
            | GripMotion::Displace
            | GripMotion::RotateAboutPoint
            | GripMotion::MoveWithinPlane
            | GripMotion::MoveAlongNormal,
            GripReference::Point(_) | GripReference::Line(_) | GripReference::Plane { .. },
        ) => None,
    }
}

impl Document {
    /// Where Grip Snap puts a component (a body, robot or imported assembly); see [`grip_pose`].
    pub fn grip_snap_pose(
        &self,
        item: AnyId,
        motion: GripMotion,
        line: GripLine,
        from: [f64; 3],
        to: [f64; 3],
    ) -> Option<DPose3> {
        grip_pose(self.owner_pose(item)?, motion, line, from, to)
    }

    /// True when the component's remaining freedom allows the motion about or along the line (the HUD's
    /// report): a grounded component allows none, one untouched by relationships every one.
    pub fn grip_allows(&self, item: AnyId, motion: GripMotion, line: GripLine) -> bool {
        let Some(pose) = self.owner_pose(item) else { return false };
        let Some((p, a)) = line.unit() else { return false };
        if self.component_flag(crate::design::Occurrence::Item(item), crate::design::ComponentFlag::Grounded)
        {
            return false;
        }
        // The motion as a rate: translation of the component's origin, then rotation (as the solver steps).
        let twist: [f64; 6] = match motion {
            GripMotion::MoveAlong => [a.x, a.y, a.z, 0.0, 0.0, 0.0],
            GripMotion::RotateAbout => {
                let v = a.cross(pose.translation - p);
                [v.x, v.y, v.z, a.x, a.y, a.z]
            }
            GripMotion::Displace
            | GripMotion::RotateAboutPoint
            | GripMotion::MoveWithinPlane
            | GripMotion::MoveAlongNormal => return false,
        };
        self.motion_allowed(item, twist)
    }

    /// Where Grip Snap puts a component for a motion about any reference; see [`grip_pose_on`].
    pub fn grip_snap_pose_on(
        &self,
        item: AnyId,
        motion: GripMotion,
        reference: GripReference,
        from: [f64; 3],
        to: [f64; 3],
    ) -> Option<DPose3> {
        grip_pose_on(self.owner_pose(item)?, motion, reference, from, to)
    }

    /// True when the relationships let the component make this snap (to first order: the direction of the
    /// change, translation and turn, is one its remaining freedom allows). False for a grounded
    /// component or a snap that does not work out; true for a snap that changes nothing.
    pub fn grip_snap_allowed(
        &self,
        item: AnyId,
        motion: GripMotion,
        reference: GripReference,
        from: [f64; 3],
        to: [f64; 3],
    ) -> bool {
        let Some(now) = self.owner_pose(item) else { return false };
        let Some(then) = grip_pose_on(now, motion, reference, from, to) else { return false };
        if self.component_flag(crate::design::Occurrence::Item(item), crate::design::ComponentFlag::Grounded)
        {
            return false;
        }
        let shift = then.translation - now.translation;
        let turn = gc_math::rotvec_from_quat(then.rotation * now.rotation.inverse());
        let twist = [shift.x, shift.y, shift.z, turn.x, turn.y, turn.z];
        twist.iter().all(|v| v.abs() < 1e-15) || self.motion_allowed(item, twist)
    }
}
