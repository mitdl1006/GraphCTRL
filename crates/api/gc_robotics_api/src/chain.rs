//! A serial kinematic chain as plain data.

use serde::{Deserialize, Serialize};

use crate::Pose;

/// How a joint moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum JointKind {
    /// Rotation about the axis within limits.
    Revolute,
    /// Translation along the axis within limits.
    Prismatic,
    /// Rotation about the axis without limits.
    Continuous,
}

/// One movable joint of a chain.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JointSpec {
    /// Joint name from the robot description.
    pub name: String,
    /// How the joint moves.
    pub kind: JointKind,
    /// Unit axis of motion in the joint frame.
    pub axis: [f64; 3],
    /// Lower position limit (rad or m). `-inf` for continuous joints.
    pub lower: f64,
    /// Upper position limit (rad or m). `+inf` for continuous joints.
    pub upper: f64,
    /// Speed limit (rad/s or m/s), when the description has one.
    pub max_vel: Option<f64>,
    /// Fixed transform from this joint's frame, after its motion, to the next joint's frame. For the last
    /// joint: to the frame [`ChainSpec::tip`] is expressed in.
    pub to_next: Pose,
}

/// A serial chain from a base frame to a tip frame. Fixed joints are already folded into the transforms.
///
/// `T(base -> tip)(q) = base_to_first * M_0(q_0) * to_next_0 * M_1(q_1) * ... * to_next_(n-1) * tip`,
/// where `M_i` rotates about (or translates along) the joint axis.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChainSpec {
    /// Frame of the first joint in the base frame.
    pub base_to_first: Pose,
    /// The movable joints, base to tip.
    pub joints: Vec<JointSpec>,
    /// The tip frame, relative to the frame after the last joint's `to_next`.
    pub tip: Pose,
}

impl ChainSpec {
    /// Number of movable joints.
    pub fn dof(&self) -> usize {
        self.joints.len()
    }

    /// Reference forward kinematics: the pose of the tip in the base frame.
    ///
    /// This definition is the contract. The host's optimised kinematics is tested against it, and plugins
    /// may rely on it. Returns `None` when `q` does not have one value per joint.
    pub fn fk(&self, q: &[f64]) -> Option<Pose> {
        if q.len() != self.joints.len() {
            return None;
        }
        let mut pose = self.base_to_first;
        for (joint, value) in self.joints.iter().zip(q) {
            let motion = match joint.kind {
                JointKind::Revolute | JointKind::Continuous => Pose::from_axis_angle(joint.axis, *value),
                JointKind::Prismatic => Pose::from_translation(joint.axis.map(|a| a * value)),
            };
            pose = pose.mul(&motion).mul(&joint.to_next);
        }
        Some(pose.mul(&self.tip))
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::FRAC_PI_2;

    use super::*;

    fn joint(kind: JointKind, axis: [f64; 3], to_next: Pose) -> JointSpec {
        JointSpec { name: String::new(), kind, axis, lower: -3.0, upper: 3.0, max_vel: None, to_next }
    }

    #[test]
    fn planar_two_link_arm() {
        let (l1, l2) = (0.5, 0.3);
        let chain = ChainSpec {
            base_to_first: Pose::IDENTITY,
            joints: vec![
                joint(JointKind::Revolute, [0.0, 0.0, 1.0], Pose::from_translation([l1, 0.0, 0.0])),
                joint(JointKind::Revolute, [0.0, 0.0, 1.0], Pose::from_translation([l2, 0.0, 0.0])),
            ],
            tip: Pose::IDENTITY,
        };
        let straight = chain.fk(&[0.0, 0.0]).unwrap();
        assert!((straight.xyz[0] - 0.8).abs() < 1e-12 && straight.xyz[1].abs() < 1e-12);
        let bent = chain.fk(&[FRAC_PI_2, -FRAC_PI_2]).unwrap();
        assert!((bent.xyz[0] - l2).abs() < 1e-12 && (bent.xyz[1] - l1).abs() < 1e-12);
        assert!(bent.error_to(&Pose::from_translation([l2, l1, 0.0])).1 < 1e-12, "the rotations cancel");
        assert_eq!(chain.fk(&[0.0]), None);
        assert_eq!(chain.dof(), 2);
    }

    #[test]
    fn prismatic_joints_translate_and_fixed_offsets_apply() {
        let chain = ChainSpec {
            base_to_first: Pose::from_translation([0.0, 0.0, 1.0]),
            joints: vec![
                joint(JointKind::Prismatic, [0.0, 0.0, 1.0], Pose::IDENTITY),
                joint(JointKind::Continuous, [0.0, 0.0, 1.0], Pose::IDENTITY),
            ],
            tip: Pose::from_translation([0.2, 0.0, 0.0]),
        };
        let pose = chain.fk(&[0.25, FRAC_PI_2]).unwrap();
        assert!(pose.xyz[0].abs() < 1e-12);
        assert!((pose.xyz[1] - 0.2).abs() < 1e-12);
        assert!((pose.xyz[2] - 1.25).abs() < 1e-12);
    }
}
