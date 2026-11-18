//! Geometric constraint solver for 2D and 3D sketches.
//!
//! The solver works on scalar parameters (point coordinates, radii) and [`Constraint`]s between them. It
//! knows nothing about sketches, documents or ids of the application; the sketch layer translates.
//!
//! How a solve proceeds:
//! 1. **Substitution.** Constraints of the form `a = b`, `a = b + c` and `a = c` (coincident, horizontal,
//!    fixed, ...) are removed by a weighted union-find, remembering which constraint caused each merge.
//!    Merged parameters end up bit-identical.
//! 2. **Solve.** Each connected component is solved by Levenberg-Marquardt on all remaining rows. A stall
//!    is retried once from a pseudo-random nudge inside the null space before it is called a conflict.
//! 3. **Diagnosis.** At the result, a pivoted QR of the transposed Jacobian gives the rank, the degrees of
//!    freedom, which rows are redundant or conflicting (and with which others) and which parameters can
//!    still move.
//!
//! A failed solve never changes parameter values. Solving a solved system changes nothing, bit for bit.
//! Results are deterministic: the same input gives the same output on the same platform.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::float_cmp, reason = "tests compare exactly representable values"))]

mod constraint;
mod drag;
mod problem;
mod rows;
mod scalar;
mod system;

pub use constraint::{Constraint, Line2, Line3, P2, P3, Round};
pub use drag::{DragRefused, Dragging, FrameOutcome, Grab};
pub use system::{AddOutcome, Blame, DepKind, Dependency, Flip, SolveReport, Status, System};

/// A scalar unknown of the system: one coordinate of a point, or a radius.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ParamId(pub u32);

/// Caller-chosen id of a constraint. Lower ids are older; when constraints depend on each other, the
/// newest is the one reported as redundant or conflicting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConstraintId(pub u64);

/// Numerical thresholds, relative to the sketch size where they are lengths.
pub(crate) mod tol {
    /// A residual below this (times the sketch size) is satisfied.
    pub const RESIDUAL: f64 = 1e-10;
    /// Relative gradient norm below which an unsatisfied system has stalled.
    pub const STATIONARY: f64 = 1e-9;
    /// A step shorter than this (times the sketch size) is no progress.
    pub const MIN_STEP: f64 = 1e-13;
    /// Pivots below this fraction of the first pivot are dependent rows.
    pub const RANK: f64 = 1e-8;
    /// Accepted pivots below this fraction are reported as ill-conditioned.
    pub const ILL_CONDITIONED: f64 = 1e-6;
    /// A dependent row whose residuals disagree by more than this (times the sketch size) conflicts.
    pub const COMBO: f64 = 1e-7;
    /// A row whose gradient is below this does not depend on any unknown.
    pub const ZERO_GRADIENT: f64 = 1e-12;
    /// A parameter whose null-space share is above this can still move.
    pub const MOBILITY: f64 = 1e-6;
    /// Length of the nudge and of the witness step, times the sketch size.
    pub const NUDGE: f64 = 1e-3;
    /// The sketch size never drops below this (metres).
    pub const MIN_SIZE: f64 = 1e-3;
}
