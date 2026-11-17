//! Universal Robots e-Series (UR3e, UR5e, UR10e, UR16e) as a built-in plugin: robot descriptions and
//! their meshes.
//!
//! The descriptions are plain URDF text. The host reads them with the same reader it uses for files the
//! user opens, so nothing here is specific to how the host represents robots.

#![forbid(unsafe_code)]

use std::sync::Arc;

pub mod ik;

use gc_plugin_api::{GC_PLUGIN_API_VERSION, Plugin, PluginError, PluginManifest, Registrar};
use gc_robotics_api::{
    KINEMATICS_FACTORIES, KinematicsFactory, PlanningProfileData, ProviderError, ROBOT_MODEL_PROVIDERS,
    RobotModelInfo, RobotModelProvider,
};

/// The models offered: id, name, reach (m), payload (kg) and description.
const MODELS: [(&str, &str, f64, f64, &str); 4] = [
    ("ur3e", "UR3e", 0.5, 3.0, include_str!("../../../../assets/ur/urdf/ur3e.urdf")),
    ("ur5e", "UR5e", 0.85, 5.0, include_str!("../../../../assets/ur/urdf/ur5e.urdf")),
    ("ur10e", "UR10e", 1.3, 12.5, include_str!("../../../../assets/ur/urdf/ur10e.urdf")),
    ("ur16e", "UR16e", 0.9, 16.0, include_str!("../../../../assets/ur/urdf/ur16e.urdf")),
];

macro_rules! mesh {
    ($model:literal, $kind:literal, $name:literal, $extension:literal) => {
        (
            concat!("package://ur_description/meshes/", $model, "/", $kind, "/", $name, ".", $extension),
            include_bytes!(concat!(
                "../../../../assets/ur/meshes/",
                $model,
                "/",
                $kind,
                "/",
                $name,
                ".",
                $extension
            ))
            .as_slice(),
        )
    };
}

/// The files the descriptions refer to, by URI: what is drawn and what is checked for collisions. Only
/// the UR5e has visual meshes; the others are drawn with their collision meshes. The UR16e shares five
/// of its parts with the UR10e.
const RESOURCES: [(&str, &[u8]); 30] = [
    mesh!("ur5e", "visual", "base", "dae"),
    mesh!("ur5e", "visual", "shoulder", "dae"),
    mesh!("ur5e", "visual", "upperarm", "dae"),
    mesh!("ur5e", "visual", "forearm", "dae"),
    mesh!("ur5e", "visual", "wrist1", "dae"),
    mesh!("ur5e", "visual", "wrist2", "dae"),
    mesh!("ur5e", "visual", "wrist3", "dae"),
    mesh!("ur5e", "collision", "base", "stl"),
    mesh!("ur5e", "collision", "shoulder", "stl"),
    mesh!("ur5e", "collision", "upperarm", "stl"),
    mesh!("ur5e", "collision", "forearm", "stl"),
    mesh!("ur5e", "collision", "wrist1", "stl"),
    mesh!("ur5e", "collision", "wrist2", "stl"),
    mesh!("ur5e", "collision", "wrist3", "stl"),
    mesh!("ur3e", "collision", "base", "stl"),
    mesh!("ur3e", "collision", "shoulder", "stl"),
    mesh!("ur3e", "collision", "upperarm", "stl"),
    mesh!("ur3e", "collision", "forearm", "stl"),
    mesh!("ur3e", "collision", "wrist1", "stl"),
    mesh!("ur3e", "collision", "wrist2", "stl"),
    mesh!("ur3e", "collision", "wrist3", "stl"),
    mesh!("ur10e", "collision", "base", "stl"),
    mesh!("ur10e", "collision", "shoulder", "stl"),
    mesh!("ur10e", "collision", "upperarm", "stl"),
    mesh!("ur10e", "collision", "forearm", "stl"),
    mesh!("ur10e", "collision", "wrist1", "stl"),
    mesh!("ur10e", "collision", "wrist2", "stl"),
    mesh!("ur10e", "collision", "wrist3", "stl"),
    mesh!("ur16e", "collision", "upperarm", "stl"),
    mesh!("ur16e", "collision", "forearm", "stl"),
];

/// Offers the UR models.
#[derive(Clone, Copy, Debug, Default)]
pub struct UrModels;

impl RobotModelProvider for UrModels {
    fn catalog(&self) -> Vec<RobotModelInfo> {
        MODELS
            .iter()
            .map(|(id, name, reach, payload, _)| RobotModelInfo {
                id: (*id).to_owned(),
                vendor: "Universal Robots".to_owned(),
                name: (*name).to_owned(),
                dof: 6,
                reach_m: Some(*reach),
                payload_kg: Some(*payload),
            })
            .collect()
    }

    fn urdf(&self, model: &str) -> Result<String, ProviderError> {
        MODELS
            .iter()
            .find(|(id, ..)| *id == model)
            .map(|(.., urdf)| (*urdf).to_owned())
            .ok_or_else(|| ProviderError::UnknownModel(model.to_owned()))
    }

    fn resource(&self, model: &str, uri: &str) -> Result<Vec<u8>, ProviderError> {
        let (.., urdf) = MODELS
            .iter()
            .find(|(id, ..)| *id == model)
            .ok_or_else(|| ProviderError::UnknownModel(model.to_owned()))?;
        // Only the files the model's own description names.
        RESOURCES
            .iter()
            .find(|(known, _)| *known == uri && urdf.contains(*known))
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
        reg.add(KINEMATICS_FACTORIES, Arc::new(ik::UrKinematics) as Arc<dyn KinematicsFactory>);
        Ok(())
    }
}
