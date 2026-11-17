//! Robot models from URDF files: reading, validation, kinematic chains, forward kinematics, Jacobians and
//! numerical inverse kinematics.
//!
//! The URDF text is the single source of kinematic truth. Packaged robots and files the user opens go
//! through the same reader. Problems are reported as data with line and column, never as panics.
//!
//! Conventions (URDF): `<origin>` places the child frame in the parent frame with rotation
//! `Rz(yaw) * Ry(pitch) * Rx(roll)`; a joint moves *after* its origin; the axis is given in the joint
//! frame. Units are metres and radians.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::float_cmp, reason = "tests compare exactly representable values"))]

mod chain;
mod ik;
mod model;
mod urdf;

pub use chain::{Chain, ChainError};
pub use ik::{IkOptions, NumericIk};
pub use model::{Geometry, Joint, JointKind, Link, RobotModel, Shape, import};
pub use urdf::{Diagnostic, Severity};
