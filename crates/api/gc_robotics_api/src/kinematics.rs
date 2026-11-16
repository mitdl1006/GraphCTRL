//! Inverse-kinematics plugins.

use std::collections::BTreeMap;

use gc_plugin_api::ExtensionPoint;
use serde::{Deserialize, Serialize};

use crate::{ChainSpec, Pose};

/// What a robot package declares about the kinematic family of a model, e.g. family `"ur6r"` with its DH
/// parameters. Lets a factory skip guessing.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FamilyMetadata {
    /// Id of the kinematic family.
    pub family: String,
    /// Family-specific numbers by name.
    pub params: BTreeMap<String, f64>,
}

/// How well a chain matches the closed-form model of a factory.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fit {
    /// The chain equals the model (to about 1e-9): solutions are exact.
    Exact,
    /// The chain deviates slightly (for example a calibrated robot): solutions are good seeds that the
    /// host refines numerically on the real chain.
    NearNominal {
        /// Largest position deviation found, in metres.
        pos_err: f64,
        /// Largest orientation deviation found, in radians.
        rot_err: f64,
    },
}

/// Why a solver could not be built for a chain.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum KinError {
    /// The chain is not of the factory's kinematic family.
    #[error("the chain is not supported: {0}")]
    Unsupported(String),
}

/// Why no inverse-kinematics solution was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IkFailureReason {
    /// The target is too far away or too close.
    OutOfReach,
    /// Solutions exist but all violate joint limits.
    JointLimits,
    /// The target is at a singular configuration where the solution is not unique.
    Singular,
    /// A numerical solver ran out of iterations.
    Budget,
    /// The target or seed is not usable (NaN, wrong length).
    BadInput,
}

/// A failed inverse-kinematics query.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
#[error("no inverse-kinematics solution ({reason:?})")]
pub struct IkFailure {
    /// Why.
    pub reason: IkFailureReason,
    /// The closest configuration found, when there is one.
    pub best_q: Option<Vec<f64>>,
    /// Remaining position error at `best_q`, in metres.
    pub pos_err: f64,
    /// Remaining orientation error at `best_q`, in radians.
    pub rot_err: f64,
}

/// Recognises chains of one kinematic family and builds solvers for them.
pub trait KinematicsFactory: Send + Sync {
    /// Unique id of the family, e.g. `"ur6r"`.
    fn id(&self) -> &str;

    /// Whether this factory can solve the chain, and how exactly. `meta` is what the robot package
    /// declared, if anything; without it the factory inspects the chain itself.
    fn probe(&self, chain: &ChainSpec, meta: Option<&FamilyMetadata>) -> Option<Fit>;

    /// Builds a solver for a chain that [`KinematicsFactory::probe`] accepted.
    fn build(&self, chain: &ChainSpec) -> Result<Box<dyn KinematicsSolver>, KinError>;
}

/// Closed-form inverse kinematics of one chain.
pub trait KinematicsSolver: Send + Sync {
    /// Finds every solution for the tip pose `target` (in the chain's base frame).
    ///
    /// Solutions are appended to `out_q` (`dof` values each, angles in `(-pi, pi]`) and their branch
    /// numbers to `out_branch`; both are cleared first. Joint limits are not applied: the host filters and
    /// enumerates full turns. `seed` resolves joints that are free at a singularity. Returns the number of
    /// solutions.
    fn solve_all(
        &self,
        target: &Pose,
        seed: Option<&[f64]>,
        out_q: &mut Vec<f64>,
        out_branch: &mut Vec<u32>,
    ) -> Result<usize, IkFailure>;

    /// The branch a configuration belongs to, or `None` at a branch boundary.
    fn classify(&self, q: &[f64]) -> Option<u32>;

    /// A short label for a branch, for the user interface: `"shoulder left, elbow up, wrist flip"`.
    fn branch_label(&self, branch: u32) -> String;
}

/// Where kinematics factories register.
pub const KINEMATICS_FACTORIES: ExtensionPoint<dyn KinematicsFactory> =
    ExtensionPoint::new("gc.kinematics_factories");
