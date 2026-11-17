//! A serial chain through the robot's link tree: forward kinematics and Jacobian.

use gc_linalg::Matrix;
use gc_math::{DPose3, DVec3};
use gc_robotics_api::{ChainSpec, JointKind as ApiKind, JointSpec, Pose};

use crate::model::{JointKind, RobotModel};

/// Why a chain could not be built.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ChainError {
    /// The model has no link with this name.
    #[error("the robot has no link `{0}`")]
    UnknownLink(String),
    /// The tip is not below the base in the link tree.
    #[error("link `{tip}` is not reached from link `{base}`")]
    NotConnected {
        /// Requested base link.
        base: String,
        /// Requested tip link.
        tip: String,
    },
    /// A joint on the chain copies a joint that is not on it.
    #[error("joint `{0}` mimics a joint outside the chain")]
    MimicOutsideChain(String),
    /// There is nothing to move between base and tip.
    #[error("there is no movable joint between the two links")]
    NoMovableJoint,
    /// The planning chain cannot be chosen automatically.
    #[error("the planning chain is ambiguous; candidate tip links: {0:?}")]
    Ambiguous(Vec<String>),
}

/// What sets the value of a joint on the chain.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Drive {
    Fixed,
    /// Coordinate `k` of the chain's configuration.
    Active(usize),
    /// `multiplier * q[k] + offset`.
    Mimic(usize, f64, f64),
}

#[derive(Clone, Debug, PartialEq)]
struct Step {
    origin: DPose3,
    axis: DVec3,
    kind: JointKind,
    drive: Drive,
}

/// A serial chain from a base link to a tip link. Its configuration `q` has one value per independent
/// movable joint on the way, base to tip.
#[derive(Clone, Debug, PartialEq)]
pub struct Chain {
    steps: Vec<Step>,
    /// Model joint index of each coordinate of `q`.
    active: Vec<usize>,
    names: Vec<String>,
    lower: Vec<f64>,
    upper: Vec<f64>,
    max_vel: Vec<Option<f64>>,
    kinds: Vec<JointKind>,
    model_joints: usize,
    has_mimic: bool,
}

impl RobotModel {
    /// The chain from link `base` down to link `tip`.
    pub fn chain(&self, base: &str, tip: &str) -> Result<Chain, ChainError> {
        let find = |name: &str| self.link(name).ok_or_else(|| ChainError::UnknownLink(name.to_owned()));
        let (base_link, mut at) = (find(base)?, find(tip)?);
        let mut path = Vec::new();
        while at != base_link {
            let Some(joint) = self.links[at].parent_joint else {
                return Err(ChainError::NotConnected { base: base.to_owned(), tip: tip.to_owned() });
            };
            path.push(joint);
            at = self.joints[joint].parent;
        }
        path.reverse();
        let active: Vec<usize> = path
            .iter()
            .copied()
            .filter(|j| self.joints[*j].kind.is_movable() && self.joints[*j].mimic.is_none())
            .collect();
        if active.is_empty() {
            return Err(ChainError::NoMovableJoint);
        }
        let coordinate = |joint: usize| active.iter().position(|a| *a == joint);
        let mut steps = Vec::with_capacity(path.len());
        for joint_index in &path {
            let joint = &self.joints[*joint_index];
            let drive = if !joint.kind.is_movable() {
                Drive::Fixed
            } else if joint.mimic.is_some() {
                // Follow the chain of masters down to an independent joint.
                let (mut master, mut scale, mut shift) = (*joint_index, 1.0, 0.0);
                while let Some((next, multiplier, offset)) = self.joints[master].mimic {
                    shift += scale * offset;
                    scale *= multiplier;
                    master = next;
                }
                let k =
                    coordinate(master).ok_or_else(|| ChainError::MimicOutsideChain(joint.name.clone()))?;
                Drive::Mimic(k, scale, shift)
            } else {
                Drive::Active(coordinate(*joint_index).unwrap_or(0))
            };
            steps.push(Step { origin: joint.origin, axis: joint.axis, kind: joint.kind, drive });
        }
        let pick =
            |f: &dyn Fn(&crate::model::Joint) -> f64| active.iter().map(|j| f(&self.joints[*j])).collect();
        Ok(Chain {
            has_mimic: steps.iter().any(|s| matches!(s.drive, Drive::Mimic(..))),
            steps,
            names: active.iter().map(|j| self.joints[*j].name.clone()).collect(),
            lower: pick(&|j| j.lower),
            upper: pick(&|j| j.upper),
            max_vel: active.iter().map(|j| self.joints[*j].max_vel).collect(),
            kinds: active.iter().map(|j| self.joints[*j].kind).collect(),
            model_joints: self.joints.len(),
            active,
        })
    }

    /// The chain to plan for when the user has not chosen one: from the root to `tool0` if the model has
    /// such a link, otherwise to the one leaf link with the most movable joints on its way.
    pub fn default_chain(&self) -> Result<Chain, ChainError> {
        let root = &self.links[self.root].name;
        if self.link("tool0").is_some() {
            return self.chain(root, "tool0");
        }
        let is_leaf = |link: usize| !self.joints.iter().any(|j| j.parent == link);
        let movable_above = |mut link: usize| {
            let mut count = 0;
            while let Some(joint) = self.links[link].parent_joint {
                count +=
                    usize::from(self.joints[joint].kind.is_movable() && self.joints[joint].mimic.is_none());
                link = self.joints[joint].parent;
            }
            count
        };
        let leaves: Vec<(usize, usize)> =
            (0..self.links.len()).filter(|l| is_leaf(*l)).map(|l| (movable_above(l), l)).collect();
        let most = leaves.iter().map(|(count, _)| *count).max().unwrap_or(0);
        let best: Vec<&str> = leaves
            .iter()
            .filter(|(count, _)| *count == most)
            .map(|(_, l)| self.links[*l].name.as_str())
            .collect();
        match best.as_slice() {
            [only] if most > 0 => self.chain(root, only),
            [] | [_] => Err(ChainError::NoMovableJoint),
            several => Err(ChainError::Ambiguous(several.iter().map(|s| (*s).to_owned()).collect())),
        }
    }
}

fn to_pose(pose: &DPose3) -> Pose {
    let (t, r) = (pose.translation, pose.rotation);
    Pose { xyz: [t.x, t.y, t.z], quat_xyzw: [r.x, r.y, r.z, r.w] }
}

impl Chain {
    /// Number of independent joints.
    pub fn dof(&self) -> usize {
        self.active.len()
    }

    /// Names of the independent joints, base to tip.
    pub fn joint_names(&self) -> &[String] {
        &self.names
    }

    /// Lower position limits (`-inf` for continuous joints).
    pub fn lower(&self) -> &[f64] {
        &self.lower
    }

    /// Upper position limits (`+inf` for continuous joints).
    pub fn upper(&self) -> &[f64] {
        &self.upper
    }

    /// Speed limits, where the description has them.
    pub fn max_vel(&self) -> &[Option<f64>] {
        &self.max_vel
    }

    /// How each independent joint moves.
    pub fn kinds(&self) -> &[JointKind] {
        &self.kinds
    }

    /// True when every value of `q` is within its limits (with a small tolerance).
    pub fn within_limits(&self, q: &[f64]) -> bool {
        q.len() == self.dof()
            && q.iter()
                .zip(self.lower.iter().zip(&self.upper))
                .all(|(v, (lo, hi))| *v >= lo - 1e-9 && *v <= hi + 1e-9)
    }

    fn value(drive: Drive, q: &[f64]) -> f64 {
        match drive {
            Drive::Fixed => 0.0,
            Drive::Active(k) => q.get(k).copied().unwrap_or(0.0),
            Drive::Mimic(k, scale, shift) => scale * q.get(k).copied().unwrap_or(0.0) + shift,
        }
    }

    fn motion(step: &Step, value: f64) -> DPose3 {
        match step.kind {
            JointKind::Fixed => DPose3::IDENTITY,
            JointKind::Revolute | JointKind::Continuous => DPose3::new(DVec3::ZERO, step.axis * value),
            JointKind::Prismatic => DPose3::from_translation(step.axis * value),
        }
    }

    /// The pose of the tip link in the base link's frame.
    pub fn fk(&self, q: &[f64]) -> DPose3 {
        self.steps.iter().fold(DPose3::IDENTITY, |pose, step| {
            pose * step.origin * Self::motion(step, Self::value(step.drive, q))
        })
    }

    /// The geometric Jacobian in the base frame with the tip origin as reference point: rows 0..3 are the
    /// linear velocity, rows 3..6 the angular velocity, one column per independent joint.
    pub fn jacobian(&self, q: &[f64]) -> Matrix {
        let tip = self.fk(q).translation;
        let mut jacobian = Matrix::zeros(6, self.dof());
        let mut pose = DPose3::IDENTITY;
        for step in &self.steps {
            let frame = pose * step.origin;
            let (column, scale) = match step.drive {
                Drive::Fixed => (None, 0.0),
                Drive::Active(k) => (Some(k), 1.0),
                Drive::Mimic(k, scale, _) => (Some(k), scale),
            };
            if let Some(k) = column {
                let axis = frame.rotation * step.axis;
                let (linear, angular) = match step.kind {
                    JointKind::Prismatic => (axis, DVec3::ZERO),
                    _ => (axis.cross(tip - frame.translation), axis),
                };
                for row in 0..3 {
                    jacobian[(row, k)] += scale * linear[row];
                    jacobian[(row + 3, k)] += scale * angular[row];
                }
            }
            pose = frame * Self::motion(step, Self::value(step.drive, q));
        }
        jacobian
    }

    /// The sum of the fixed offsets along the chain: an upper bound of how far the tip can be from the base.
    pub fn reach(&self) -> f64 {
        self.steps.iter().map(|s| s.origin.translation.length()).sum::<f64>().max(0.1)
    }

    /// Values for every joint of the model (as [`RobotModel::link_poses`] takes them): the chain's joints
    /// from `q`, all others zero.
    pub fn expand(&self, q: &[f64]) -> Vec<f64> {
        let mut values = vec![0.0; self.model_joints];
        for (k, joint) in self.active.iter().enumerate() {
            values[*joint] = q.get(k).copied().unwrap_or(0.0);
        }
        values
    }

    /// The chain as plain data for plugins. `None` when the chain contains mimic joints, which the plain
    /// form cannot express.
    pub fn to_spec(&self) -> Option<ChainSpec> {
        if self.has_mimic {
            return None;
        }
        let mut base_to_first = DPose3::IDENTITY;
        let mut joints: Vec<JointSpec> = Vec::with_capacity(self.dof());
        let mut pending = DPose3::IDENTITY; // fixed transforms since the last movable joint
        for step in &self.steps {
            pending *= step.origin;
            let Drive::Active(k) = step.drive else { continue };
            match joints.last_mut() {
                Some(previous) => previous.to_next = to_pose(&pending),
                None => base_to_first = pending,
            }
            pending = DPose3::IDENTITY;
            joints.push(JointSpec {
                name: self.names[k].clone(),
                kind: match step.kind {
                    JointKind::Prismatic => ApiKind::Prismatic,
                    JointKind::Continuous => ApiKind::Continuous,
                    _ => ApiKind::Revolute,
                },
                axis: [step.axis.x, step.axis.y, step.axis.z],
                lower: self.lower[k],
                upper: self.upper[k],
                max_vel: self.max_vel[k],
                to_next: Pose::IDENTITY,
            });
        }
        if let Some(last) = joints.last_mut() {
            last.to_next = to_pose(&pending);
        }
        Some(ChainSpec { base_to_first: to_pose(&base_to_first), joints, tip: Pose::IDENTITY })
    }
}
