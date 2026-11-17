//! Sampling-based path planning in a box-shaped configuration space.
//!
//! This crate knows nothing about robots: a state is a slice of numbers, and whether a state or a
//! straight motion between two states is allowed is answered by a [`Validity`] the caller supplies.
//! The same planners therefore serve a six-axis arm and a sphere moving in space.
//!
//! Everything is deterministic: the same request with the same seed gives the same path, bit for bit.
//! Work is bounded by counts (iterations, validity checks), never by time.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::float_cmp, reason = "tests compare exactly representable values"))]

mod planners;
mod rng;
mod simplify;
mod space;

pub use planners::{Algorithm, Failure, Options, Request, Solution, Stats, plan};
pub use rng::Rng;
pub use simplify::{densify, path_cost, shortcut, smooth_corners, validate};
pub use space::Space;

/// Decides which states and motions are allowed.
pub trait Validity {
    /// True when the state is allowed.
    fn state_valid(&self, q: &[f64]) -> bool;

    /// True when every state on the straight segment from `a` to `b` is allowed. Both ends are known to
    /// be valid.
    fn motion_valid(&self, a: &[f64], b: &[f64]) -> bool;

    /// How many single states a call to [`Validity::motion_valid`] examines, for budgeting.
    fn motion_cost(&self, _a: &[f64], _b: &[f64]) -> u64 {
        1
    }
}

/// Lets the caller stop a running planner.
pub trait Control {
    /// True once planning should stop.
    fn cancelled(&self) -> bool;
}

/// A control that never cancels.
#[derive(Clone, Copy, Debug, Default)]
pub struct NeverCancel;

impl Control for NeverCancel {
    fn cancelled(&self) -> bool {
        false
    }
}
