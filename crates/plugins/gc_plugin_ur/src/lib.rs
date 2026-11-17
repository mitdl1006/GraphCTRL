//! Universal Robots-type arms as a built-in plugin: their closed-form inverse kinematics.
//!
//! The robots themselves (descriptions, meshes, licences) are not part of the program: they are packages
//! in the robot library folder (`robots/`), read at run time like any robot file the user opens.

#![forbid(unsafe_code)]

use std::sync::Arc;

pub mod ik;

use gc_plugin_api::{GC_PLUGIN_API_VERSION, Plugin, PluginError, PluginManifest, Registrar};
use gc_robotics_api::{KINEMATICS_FACTORIES, KinematicsFactory};

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
        reg.add(KINEMATICS_FACTORIES, Arc::new(ik::UrKinematics) as Arc<dyn KinematicsFactory>);
        Ok(())
    }
}
