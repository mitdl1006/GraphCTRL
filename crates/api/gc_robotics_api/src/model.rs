//! Robot model packages.

use std::collections::BTreeMap;

use gc_plugin_api::ExtensionPoint;
use serde::{Deserialize, Serialize};

use crate::FamilyMetadata;

/// Catalogue entry of a robot model. Cheap: no geometry is loaded to produce it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RobotModelInfo {
    /// Unique id, e.g. `"ur5e"`. Stored in documents.
    pub id: String,
    /// Manufacturer.
    pub vendor: String,
    /// Model name shown to the user.
    pub name: String,
    /// Number of movable joints.
    pub dof: usize,
    /// Reach in metres, when known.
    pub reach_m: Option<f64>,
    /// Payload in kilograms, when known.
    pub payload_kg: Option<f64>,
}

/// Why a provider could not deliver.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ProviderError {
    /// The provider has no model with this id.
    #[error("unknown robot model `{0}`")]
    UnknownModel(String),
    /// The model does not have this resource.
    #[error("robot model `{model}` has no resource `{uri}`")]
    UnknownResource {
        /// Model id.
        model: String,
        /// The requested URI.
        uri: String,
    },
}

/// A pair of links whose collisions are not checked, with the reason.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcmEntryData {
    /// First link.
    pub link_a: String,
    /// Second link.
    pub link_b: String,
    /// Why the pair is skipped: `"adjacent"`, `"default"`, `"always"`, `"never"` or a user note.
    pub reason: String,
}

/// Planning information a robot description file does not carry. Every field is optional; the host derives
/// what is missing and marks derived values as assumptions.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PlanningProfileData {
    /// First link of the planning chain (default: the root link).
    pub chain_base: Option<String>,
    /// Last link of the planning chain (default: `tool0` if present).
    pub chain_tip: Option<String>,
    /// Acceleration limit per joint name (rad/s² or m/s²).
    pub max_acc: BTreeMap<String, f64>,
    /// Where the acceleration limits come from, shown to the user.
    pub max_acc_source: Option<String>,
    /// Named joint configurations such as `"home"`, in chain order.
    pub named_poses: BTreeMap<String, Vec<f64>>,
    /// Link pairs excluded from collision checking, precomputed for the model.
    pub acm: Vec<AcmEntryData>,
    /// Default clearance kept between the robot and obstacles, in metres.
    pub padding: Option<f64>,
    /// The kinematic family of the model, for closed-form inverse kinematics.
    pub family: Option<FamilyMetadata>,
}

/// Supplies robot models as description-file text plus the resources (meshes) it refers to.
///
/// The host parses the text with the same reader it uses for files the user opens, so packaged robots and
/// imported robots take one code path.
pub trait RobotModelProvider: Send + Sync {
    /// The models this provider offers.
    fn catalog(&self) -> Vec<RobotModelInfo>;

    /// The URDF text of a model.
    fn urdf(&self, model: &str) -> Result<String, ProviderError>;

    /// The bytes of a resource the URDF refers to (e.g. `package://.../forearm.stl`).
    fn resource(&self, model: &str, uri: &str) -> Result<Vec<u8>, ProviderError>;

    /// Planning information for a model, if the provider has any.
    fn profile(&self, model: &str) -> Option<PlanningProfileData>;
}

/// Where robot model providers register.
pub const ROBOT_MODEL_PROVIDERS: ExtensionPoint<dyn RobotModelProvider> =
    ExtensionPoint::new("gc.robot_model_providers");
