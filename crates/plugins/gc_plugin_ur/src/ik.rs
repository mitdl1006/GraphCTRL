//! Closed-form inverse kinematics of the UR e-Series.
//!
//! The arm is described by Universal Robots' standard DH parameters: `A_i = Rz(θ_i) Tz(d_i) Tx(a_i)
//! Rx(α_i)` with `a = (0, a2, a3, 0, 0, 0)`, `d = (d1, 0, 0, d4, d5, d6)` and `α = (π/2, 0, 0, π/2, -π/2,
//! 0)`; `base_link` is the DH base turned half a turn about Z, and `tool0` is DH frame 6.
//!
//! For a tool pose `T` there are up to eight solutions:
//! 1. The wrist point `p05 = p - d6 z6` lies `d4` off the plane of the shoulder: `r sin(θ1 - ψ) = d4`
//!    gives two shoulder angles.
//! 2. The rotation of `A1⁻¹ T` is `Rz(θ2+θ3+θ4) Ry(-θ5) Rz(θ6)`; read as Z-Y-Z angles it gives two
//!    wrist angles θ5 and, for each, θ6 and the sum θ2+θ3+θ4. When `sin θ5` vanishes (wrist singular)
//!    θ6 is free and taken from the seed.
//! 3. What remains is a planar two-link arm for θ2 and θ3: elbow up or down. θ4 completes the sum.
//!
//! Every solution is checked with the forward kinematics before it is returned.

use gc_math::{DPose3, DQuat, DVec3, fm};
use gc_robotics_api::{
    ChainSpec, Fit, IkFailure, IkFailureReason, KinError, KinematicsFactory, KinematicsSolver, Pose,
};

/// The DH parameters of one model: d1, a2, a3, d4, d5, d6 (metres).
pub type Dh = [f64; 6];

/// The models whose kinematics are recognised, with their published DH parameters.
pub const MODELS: [(&str, Dh); 4] = [
    ("UR3e", [0.15185, -0.24355, -0.2132, 0.13105, 0.08535, 0.0921]),
    ("UR5e", [0.1625, -0.425, -0.3922, 0.1333, 0.0997, 0.0996]),
    ("UR10e", [0.1807, -0.6127, -0.57155, 0.17415, 0.11985, 0.11655]),
    ("UR16e", [0.1807, -0.4784, -0.36, 0.17415, 0.11985, 0.11655]),
];

/// Largest deviation of a chain from a model that still counts as that model, calibrated (5 mm, 0.5°).
const NEAR: (f64, f64) = (0.005, 0.0087);
/// Deviation below which a chain is the model itself.
const EXACT: f64 = 1e-9;
/// Largest error of a solution's forward kinematics that is accepted.
const ACCEPT: f64 = 1e-6;

fn to_pose(pose: &DPose3) -> Pose {
    Pose { xyz: pose.translation.to_array(), quat_xyzw: pose.rotation.to_array() }
}

fn from_pose(pose: &Pose) -> DPose3 {
    DPose3::from_parts(DVec3::from_array(pose.xyz), DQuat::from_array(pose.quat_xyzw).normalize())
}

/// One DH transform.
fn link(theta: f64, d: f64, a: f64, alpha: f64) -> DPose3 {
    DPose3::from_parts(DVec3::ZERO, DQuat::from_rotation_z(theta))
        * DPose3::from_parts(DVec3::new(a, 0.0, d), DQuat::from_rotation_x(alpha))
}

const ALPHA: [f64; 6] =
    [std::f64::consts::FRAC_PI_2, 0.0, 0.0, std::f64::consts::FRAC_PI_2, -std::f64::consts::FRAC_PI_2, 0.0];

fn dh_a(dh: &Dh) -> [f64; 6] {
    [0.0, dh[1], dh[2], 0.0, 0.0, 0.0]
}

fn dh_d(dh: &Dh) -> [f64; 6] {
    [dh[0], 0.0, 0.0, dh[3], dh[4], dh[5]]
}

/// `base_link` in the DH base frame's terms: half a turn about Z.
fn pre() -> DPose3 {
    DPose3::from_parts(DVec3::ZERO, DQuat::from_rotation_z(std::f64::consts::PI))
}

/// The pose of `tool0` in `base_link` for joint values `q`.
pub fn forward(dh: &Dh, q: &[f64]) -> DPose3 {
    let (a, d) = (dh_a(dh), dh_d(dh));
    let mut pose = pre();
    for j in 0..6 {
        pose *= link(q.get(j).copied().unwrap_or(0.0), d[j], a[j], ALPHA[j]);
    }
    pose
}

/// Position and angle between two poses.
fn deviation(a: &DPose3, b: &DPose3) -> (f64, f64) {
    let position = (a.translation - b.translation).length();
    // Through the axes: the angle of a nearly identical rotation is imprecise.
    let turn = [DVec3::X, DVec3::Y, DVec3::Z]
        .iter()
        .map(|v| (a.rotation * *v - b.rotation * *v).length())
        .fold(0.0, f64::max);
    (position, turn)
}

/// Joint values spread over the workspace, the same every time.
fn samples() -> impl Iterator<Item = [f64; 6]> {
    (0..48_u32).map(|k| {
        let mut q = [0.0; 6];
        for (j, v) in q.iter_mut().enumerate() {
            let step = f64::from((k * 7 + u32::try_from(j).unwrap_or(0) * 13) % 29);
            *v = step / 29.0 * 6.0 - 3.0;
        }
        q
    })
}

/// The model a chain is, and how closely: the best of the known models within [`NEAR`].
pub fn recognise(chain: &ChainSpec) -> Option<(Dh, Fit)> {
    if chain.dof() != 6 {
        return None;
    }
    MODELS
        .iter()
        .filter_map(|(_, dh)| {
            let (mut position, mut turn) = (0.0_f64, 0.0_f64);
            for q in samples() {
                let tip = from_pose(&chain.fk(&q)?);
                let (p, r) = deviation(&tip, &forward(dh, &q));
                position = position.max(p);
                turn = turn.max(r);
            }
            (position <= NEAR.0 && turn <= NEAR.1).then_some((*dh, position, turn))
        })
        .min_by(|a, b| (a.1 + a.2).total_cmp(&(b.1 + b.2)))
        .map(|(dh, position, turn)| {
            let fit = if position <= EXACT && turn <= EXACT {
                Fit::Exact
            } else {
                Fit::NearNominal { pos_err: position, rot_err: turn }
            };
            (dh, fit)
        })
}

/// The element in row `i`, column `j` of a rotation.
fn at(rotation: &gc_math::DMat3, i: usize, j: usize) -> f64 {
    rotation.col(j)[i]
}

/// All solutions of the closed-form inverse kinematics for `tool0` at `target` (in `base_link`), each
/// with its branch number. Angles are in (-π, π]; joint limits are not applied.
pub fn solve(dh: &Dh, target: &DPose3, seed: Option<&[f64]>) -> Vec<([f64; 6], u32)> {
    let [d1, a2, a3, d4, d5, d6] = *dh;
    let goal = pre().inverse() * *target;
    let z6 = goal.rotation * DVec3::Z;
    let wrist = goal.translation - z6 * d6;
    let reach = fm::hypot(wrist.x, wrist.y);
    let mut found = Vec::new();
    if reach < d4.abs() || reach < 1e-12 {
        return found;
    }
    let psi = fm::atan2(wrist.y, wrist.x);
    let offset = fm::asin_clamped(d4 / reach);
    for (shoulder, theta1) in [psi + offset, psi + std::f64::consts::PI - offset].into_iter().enumerate() {
        let t16 = link(theta1, d1, 0.0, ALPHA[0]).inverse() * goal;
        let r = gc_math::DMat3::from_quat(t16.rotation);
        let c5 = at(&r, 2, 2).clamp(-1.0, 1.0);
        for (flip, theta5) in [fm::acos(c5), -fm::acos(c5)].into_iter().enumerate() {
            let s5 = fm::sin(theta5);
            let (theta6, sum) = if s5.abs() < 1e-9 {
                // Wrist singular: θ6 and θ2+θ3+θ4 only matter as a sum (or difference).
                let theta6 = seed.and_then(|q| q.get(5).copied()).unwrap_or(0.0);
                if c5 > 0.0 {
                    (theta6, fm::atan2(at(&r, 1, 0), at(&r, 0, 0)) - theta6)
                } else {
                    (theta6, fm::atan2(-at(&r, 1, 0), -at(&r, 0, 0)) + theta6)
                }
            } else {
                (
                    fm::atan2(-at(&r, 2, 1) / s5, at(&r, 2, 0) / s5),
                    fm::atan2(-at(&r, 1, 2) / s5, -at(&r, 0, 2) / s5),
                )
            };
            let wrist_part = link(theta5, d5, 0.0, ALPHA[4]) * link(theta6, d6, 0.0, ALPHA[5]);
            let t14 = t16 * wrist_part.inverse();
            let (x, y) = (t14.translation.x, t14.translation.y);
            let c3 = (x * x + y * y - a2 * a2 - a3 * a3) / (2.0 * a2 * a3);
            if c3.abs() > 1.0 + 1e-9 {
                continue;
            }
            let c3 = c3.clamp(-1.0, 1.0);
            for (elbow, theta3) in [fm::acos(c3), -fm::acos(c3)].into_iter().enumerate() {
                let (s3, c3) = fm::sin_cos(theta3);
                let theta2 = fm::atan2(y, x) - fm::atan2(a3 * s3, a2 + a3 * c3);
                let theta4 = sum - theta2 - theta3;
                let q = [theta1, theta2, theta3, theta4, theta5, theta6].map(fm::wrap_angle);
                let (position, turn) = deviation(&forward(dh, &q), target);
                let branch = u32::try_from(shoulder + 2 * flip + 4 * elbow).unwrap_or(0);
                let known = found.iter().any(|(other, _): &([f64; 6], u32)| {
                    other.iter().zip(&q).all(|(u, v)| fm::wrap_angle(u - v).abs() < 1e-9)
                });
                if position < ACCEPT && turn < ACCEPT && !known {
                    found.push((q, branch));
                }
            }
        }
    }
    found
}

/// The UR family as a kinematics plugin.
#[derive(Clone, Copy, Debug, Default)]
pub struct UrKinematics;

impl KinematicsFactory for UrKinematics {
    fn id(&self) -> &str {
        "ur6r"
    }

    fn probe(&self, chain: &ChainSpec, _meta: Option<&gc_robotics_api::FamilyMetadata>) -> Option<Fit> {
        recognise(chain).map(|(_, fit)| fit)
    }

    fn build(&self, chain: &ChainSpec) -> Result<Box<dyn KinematicsSolver>, KinError> {
        let (dh, _) =
            recognise(chain).ok_or_else(|| KinError::Unsupported("not a UR e-Series arm".to_owned()))?;
        // The chain's base and tip may differ from base_link and tool0 by fixed transforms.
        let zero = [0.0; 6];
        let tip = from_pose(&chain.fk(&zero).ok_or_else(|| KinError::Unsupported("bad chain".to_owned()))?);
        let correction = forward(&dh, &zero).inverse() * tip;
        Ok(Box::new(UrSolver { dh, correction }))
    }
}

/// The closed-form solver of one UR arm.
#[derive(Clone, Copy, Debug)]
pub struct UrSolver {
    dh: Dh,
    /// The chain's tip in `tool0`'s frame (identity for the packaged descriptions).
    correction: DPose3,
}

impl KinematicsSolver for UrSolver {
    fn solve_all(
        &self,
        target: &Pose,
        seed: Option<&[f64]>,
        out_q: &mut Vec<f64>,
        out_branch: &mut Vec<u32>,
    ) -> Result<usize, IkFailure> {
        out_q.clear();
        out_branch.clear();
        let target = from_pose(target);
        if !target.translation.is_finite() || !target.rotation.is_finite() {
            return Err(IkFailure {
                reason: IkFailureReason::BadInput,
                best_q: None,
                pos_err: 0.0,
                rot_err: 0.0,
            });
        }
        let found = solve(&self.dh, &(target * self.correction.inverse()), seed);
        if found.is_empty() {
            return Err(IkFailure {
                reason: IkFailureReason::OutOfReach,
                best_q: None,
                pos_err: 0.0,
                rot_err: 0.0,
            });
        }
        for (q, branch) in &found {
            out_q.extend_from_slice(q);
            out_branch.push(*branch);
        }
        Ok(found.len())
    }

    fn classify(&self, q: &[f64]) -> Option<u32> {
        let target = forward(&self.dh, q);
        solve(&self.dh, &target, Some(q))
            .into_iter()
            .find(|(other, _)| other.iter().zip(q).all(|(u, v)| fm::wrap_angle(u - v).abs() < 1e-6))
            .map(|(_, branch)| branch)
    }

    fn branch_label(&self, branch: u32) -> String {
        let shoulder = if branch & 1 == 0 { "shoulder A" } else { "shoulder B" };
        let wrist = if branch & 2 == 0 { "wrist up" } else { "wrist down" };
        let elbow = if branch & 4 == 0 { "elbow +" } else { "elbow -" };
        format!("{shoulder}, {elbow}, {wrist}")
    }
}

/// The API pose of a host pose, for callers that hold glam poses.
pub fn pose_of(pose: &DPose3) -> Pose {
    to_pose(pose)
}
