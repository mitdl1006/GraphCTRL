//! Numerical inverse kinematics for any chain.

use gc_linalg::{Cholesky, Matrix};
use gc_math::{DPose3, rotvec_from_quat};
use gc_robotics_api::{IkFailure, IkFailureReason};

use crate::chain::Chain;
use crate::model::JointKind;

/// Settings of the numerical solver.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IkOptions {
    /// Position accuracy in metres.
    pub pos_tol: f64,
    /// Orientation accuracy in radians.
    pub rot_tol: f64,
    /// Iterations per start.
    pub max_iterations: usize,
    /// Additional pseudo-random starts tried when the first one fails.
    pub restarts: usize,
}

impl Default for IkOptions {
    fn default() -> Self {
        Self { pos_tol: 1e-6, rot_tol: 1e-6, max_iterations: 200, restarts: 16 }
    }
}

/// Damped least-squares inverse kinematics with joint limits and deterministic restarts.
///
/// It finds one solution near a seed, or several by starting from many seeds. It cannot promise to find
/// every solution; closed-form solvers do that for the robot families they know.
#[derive(Clone, Copy, Debug)]
pub struct NumericIk<'a> {
    chain: &'a Chain,
    options: IkOptions,
}

/// SplitMix64 mapped to `[0, 1)`.
fn unit(seed: u64) -> f64 {
    let mut z = seed.wrapping_add(1).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    ((z ^ (z >> 31)) >> 11) as f64 / (1_u64 << 53) as f64
}

impl<'a> NumericIk<'a> {
    /// A solver for `chain`.
    pub fn new(chain: &'a Chain, options: IkOptions) -> Self {
        Self { chain, options }
    }

    /// Position error (metres) and orientation error (radians) of configuration `q`.
    pub fn errors(&self, target: &DPose3, q: &[f64]) -> (f64, f64) {
        let now = self.chain.fk(q);
        let turn = rotvec_from_quat(target.rotation * now.rotation.inverse());
        ((target.translation - now.translation).length(), turn.length())
    }

    fn clamp(&self, q: &mut [f64]) {
        for (value, (lower, upper)) in q.iter_mut().zip(self.chain.lower().iter().zip(self.chain.upper())) {
            *value = value.clamp(*lower, *upper);
        }
    }

    /// One descent from `start`. Returns the final configuration and whether it reaches the target.
    fn descend(&self, target: &DPose3, start: &[f64]) -> (Vec<f64>, bool) {
        let n = self.chain.dof();
        let reach = self.chain.reach();
        let mut q = start.to_vec();
        self.clamp(&mut q);
        // The task error: position in units of the reach, orientation as a rotation vector.
        let error = |q: &[f64]| -> [f64; 6] {
            let now = self.chain.fk(q);
            let dp = (target.translation - now.translation) / reach;
            let turn = rotvec_from_quat(target.rotation * now.rotation.inverse());
            [dp.x, dp.y, dp.z, turn.x, turn.y, turn.z]
        };
        let norm2 = |e: &[f64; 6]| e.iter().map(|v| v * v).sum::<f64>();
        let reached = |q: &[f64]| {
            let (position, rotation) = self.errors(target, q);
            position <= self.options.pos_tol && rotation <= self.options.rot_tol
        };
        let mut e = error(&q);
        for _ in 0..self.options.max_iterations {
            if reached(&q) {
                return (q, true);
            }
            let mut jacobian = self.chain.jacobian(&q);
            for k in 0..n {
                for row in 0..3 {
                    jacobian[(row, k)] /= reach;
                }
            }
            let gradient = jacobian.tr_mul_vec(&e);
            let normal = jacobian.transpose().mul(&jacobian);
            // Damping proportional to the error keeps steps short far away and near singularities.
            let mut damping = 0.5 * norm2(&e) + 1e-10;
            let mut improved = false;
            for _ in 0..8 {
                let mut system = normal.clone();
                for k in 0..n {
                    system[(k, k)] += damping;
                }
                let Some(factor) = Cholesky::new(&system) else {
                    damping *= 10.0;
                    continue;
                };
                let step = factor.solve(&gradient);
                let mut trial: Vec<f64> = q.iter().zip(&step).map(|(a, b)| a + b).collect();
                self.clamp(&mut trial);
                let trial_error = error(&trial);
                if norm2(&trial_error) < norm2(&e) {
                    q = trial;
                    e = trial_error;
                    improved = true;
                    break;
                }
                damping *= 10.0;
            }
            if !improved {
                break; // a local minimum: at a limit, at a singularity or out of reach
            }
        }
        let done = reached(&q);
        (q, done)
    }

    /// A start configuration for restart number `index`: uniform within the limits (one turn for
    /// continuous joints).
    fn start(&self, index: usize) -> Vec<f64> {
        (0..self.chain.dof())
            .map(|k| {
                let (lower, upper) = match self.chain.kinds()[k] {
                    JointKind::Continuous => (-std::f64::consts::PI, std::f64::consts::PI),
                    _ => (self.chain.lower()[k], self.chain.upper()[k]),
                };
                lower + (upper - lower) * unit((index as u64) << 16 | k as u64)
            })
            .collect()
    }

    /// A solution near `seed`, falling back to pseudo-random starts. The failure carries the closest
    /// configuration found.
    pub fn solve_near(&self, target: &DPose3, seed: &[f64]) -> Result<Vec<f64>, IkFailure> {
        if seed.len() != self.chain.dof() || !target.is_finite() || seed.iter().any(|v| !v.is_finite()) {
            return Err(IkFailure {
                reason: IkFailureReason::BadInput,
                best_q: None,
                pos_err: f64::NAN,
                rot_err: f64::NAN,
            });
        }
        let mut best: Option<(Vec<f64>, f64, f64)> = None;
        for attempt in 0..=self.options.restarts {
            let start = if attempt == 0 { seed.to_vec() } else { self.start(attempt) };
            let (q, reached) = self.descend(target, &start);
            if reached {
                return Ok(q);
            }
            let (position, rotation) = self.errors(target, &q);
            let score = position / self.chain.reach() + rotation;
            if best.as_ref().is_none_or(|(_, p, r)| score < p / self.chain.reach() + r) {
                best = Some((q, position, rotation));
            }
        }
        let (best_q, pos_err, rot_err) = best.unwrap_or((seed.to_vec(), f64::NAN, f64::NAN));
        let far = (target.translation.length() - self.chain.reach()).max(0.0) > 0.0;
        let at_limit = best_q
            .iter()
            .zip(self.chain.lower().iter().zip(self.chain.upper()))
            .any(|(v, (lo, hi))| v <= lo || v >= hi);
        let reason = if far {
            IkFailureReason::OutOfReach
        } else if at_limit {
            IkFailureReason::JointLimits
        } else {
            IkFailureReason::Budget
        };
        Err(IkFailure { reason, best_q: Some(best_q), pos_err, rot_err })
    }

    /// The distinct solutions found from `seed` (if any) and `starts` pseudo-random starts. Two solutions
    /// are the same when every joint agrees within a milliradian.
    pub fn solve_many(&self, target: &DPose3, seed: Option<&[f64]>, starts: usize) -> Vec<Vec<f64>> {
        let mut found: Vec<Vec<f64>> = Vec::new();
        let seeds = seed.map(<[f64]>::to_vec).into_iter().chain((1..=starts).map(|i| self.start(i)));
        for start in seeds {
            let (q, reached) = self.descend(target, &start);
            let known = found.iter().any(|other| other.iter().zip(&q).all(|(a, b)| (a - b).abs() < 1e-3));
            if reached && !known {
                found.push(q);
            }
        }
        found
    }

    /// The Jacobian's smallest singular value at `q`, with position rows in units of the reach: near zero
    /// at a singularity.
    pub fn conditioning(&self, q: &[f64]) -> f64 {
        let mut jacobian = self.chain.jacobian(q);
        let reach = self.chain.reach();
        for k in 0..self.chain.dof() {
            for row in 0..3 {
                jacobian[(row, k)] /= reach;
            }
        }
        let gram: Matrix = jacobian.mul(&jacobian.transpose());
        gc_linalg::sym_eigen(&gram).values.first().map_or(0.0, |v| v.max(0.0).sqrt())
    }
}
