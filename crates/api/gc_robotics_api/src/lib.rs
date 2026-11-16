//! The robotics part of the plugin contract: kinematics, motion planners, time parameterisation and robot
//! model packages.
//!
//! All signatures use plain `f64` data. Joint-space states are slices of length `dof`; lists of states are
//! flat slices of `count * dof` values. Lengths are metres, angles radians.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::float_cmp, reason = "tests compare exactly representable values"))]

mod chain;
mod kinematics;
mod model;
mod planning;
mod pose;

pub use chain::{ChainSpec, JointKind, JointSpec};
pub use kinematics::{
    FamilyMetadata, Fit, IkFailure, IkFailureReason, KINEMATICS_FACTORIES, KinError, KinematicsFactory,
    KinematicsSolver,
};
pub use model::{
    AcmEntryData, PlanningProfileData, ProviderError, ROBOT_MODEL_PROVIDERS, RobotModelInfo,
    RobotModelProvider,
};
pub use planning::{
    MOTION_PLANNERS, MotionPlanner, PlanBudget, PlanControl, PlanFailure, PlanOutput, PlanRequest, PlanStats,
    PlannerCaps, PlannerDescriptor, RetimerDescriptor, SpaceSpec, StateChecker, TIME_PARAMETERIZERS,
    TimeParameterizer, TimedPath, TimingFailure, TimingLimits, TimingRequest,
};
pub use pose::Pose;
