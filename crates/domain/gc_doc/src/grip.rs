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

/// Move along the reference line, or turn about it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GripMotion {
    /// Move along ray using reference geometry.
    MoveAlong,
    /// Rotate about line using reference geometry.
    RotateAbout,
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
    let (p, a) = line.unit()?;
    let (from, to) = (DVec3::from(from), DVec3::from(to));
    if !(from.is_finite() && to.is_finite()) {
        return None;
    }
    match motion {
        GripMotion::MoveAlong => {
            let t = (to - from).dot(a);
            Some(DPose3::from_parts(pose.translation + a * t, pose.rotation))
        }
        GripMotion::RotateAbout => {
            let flat = |v: DVec3| {
                let r = v - p;
                r - a * r.dot(a)
            };
            let (u, w) = (flat(from), flat(to));
            if u.length() < 1e-12 || w.length() < 1e-12 {
                return None;
            }
            let angle = gc_math::fm::atan2(a.dot(u.cross(w)), u.dot(w));
            let turn = DQuat::from_axis_angle(a, angle);
            Some(DPose3::from_parts(p + turn * (pose.translation - p), (turn * pose.rotation).normalize()))
        }
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
        };
        self.motion_allowed(item, twist)
    }
}
