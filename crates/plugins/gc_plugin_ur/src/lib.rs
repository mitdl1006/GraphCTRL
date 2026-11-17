//! Universal Robots e-Series as a built-in plugin: robot descriptions and their collision meshes.
//!
//! The descriptions are plain URDF text. The host reads them with the same reader it uses for files the
//! user opens, so nothing here is specific to how the host represents robots.

#![forbid(unsafe_code)]

use std::sync::Arc;

use gc_plugin_api::{GC_PLUGIN_API_VERSION, Plugin, PluginError, PluginManifest, Registrar};
use gc_robotics_api::{
    PlanningProfileData, ProviderError, ROBOT_MODEL_PROVIDERS, RobotModelInfo, RobotModelProvider,
};

const UR5E_URDF: &str = include_str!("../../../../assets/ur/urdf/ur5e.urdf");

macro_rules! ur5e_mesh {
    ($kind:literal, $name:literal, $extension:literal) => {
        (
            concat!("package://ur_description/meshes/ur5e/", $kind, "/", $name, ".", $extension),
            include_bytes!(concat!("../../../../assets/ur/meshes/ur5e/", $kind, "/", $name, ".", $extension))
                .as_slice(),
        )
    };
}

/// The files the UR5e description refers to: what is drawn and what is checked for collisions.
const UR5E_RESOURCES: [(&str, &[u8]); 14] = [
    ur5e_mesh!("visual", "base", "dae"),
    ur5e_mesh!("visual", "shoulder", "dae"),
    ur5e_mesh!("visual", "upperarm", "dae"),
    ur5e_mesh!("visual", "forearm", "dae"),
    ur5e_mesh!("visual", "wrist1", "dae"),
    ur5e_mesh!("visual", "wrist2", "dae"),
    ur5e_mesh!("visual", "wrist3", "dae"),
    ur5e_mesh!("collision", "base", "stl"),
    ur5e_mesh!("collision", "shoulder", "stl"),
    ur5e_mesh!("collision", "upperarm", "stl"),
    ur5e_mesh!("collision", "forearm", "stl"),
    ur5e_mesh!("collision", "wrist1", "stl"),
    ur5e_mesh!("collision", "wrist2", "stl"),
    ur5e_mesh!("collision", "wrist3", "stl"),
];

/// Offers the UR models.
#[derive(Clone, Copy, Debug, Default)]
pub struct UrModels;

impl RobotModelProvider for UrModels {
    fn catalog(&self) -> Vec<RobotModelInfo> {
        vec![RobotModelInfo {
            id: "ur5e".to_owned(),
            vendor: "Universal Robots".to_owned(),
            name: "UR5e".to_owned(),
            dof: 6,
            reach_m: Some(0.85),
            payload_kg: Some(5.0),
        }]
    }

    fn urdf(&self, model: &str) -> Result<String, ProviderError> {
        match model {
            "ur5e" => Ok(UR5E_URDF.to_owned()),
            other => Err(ProviderError::UnknownModel(other.to_owned())),
        }
    }

    fn resource(&self, model: &str, uri: &str) -> Result<Vec<u8>, ProviderError> {
        if model != "ur5e" {
            return Err(ProviderError::UnknownModel(model.to_owned()));
        }
        UR5E_RESOURCES
            .iter()
            .find(|(known, _)| *known == uri)
            .map(|(_, bytes)| bytes.to_vec())
            .ok_or_else(|| ProviderError::UnknownResource { model: model.to_owned(), uri: uri.to_owned() })
    }

    fn profile(&self, _model: &str) -> Option<PlanningProfileData> {
        None
    }
}

/// The plugin.
#[derive(Clone, Copy, Debug, Default)]
pub struct UrPlugin;

impl Plugin for UrPlugin {
    fn manifest(&self) -> &PluginManifest {
        const MANIFEST: PluginManifest = PluginManifest {
            id: "gc.ur",
            name: "Universal Robots",
            version: "0.1.0",
            api: GC_PLUGIN_API_VERSION,
        };
        &MANIFEST
    }

    fn register(&self, reg: &mut Registrar<'_>) -> Result<(), PluginError> {
        reg.add(ROBOT_MODEL_PROVIDERS, Arc::new(UrModels) as Arc<dyn RobotModelProvider>);
        Ok(())
    }
}
