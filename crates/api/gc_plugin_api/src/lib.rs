//! The plugin contract: how features register themselves with the application.
//!
//! In this milestone plugins are compiled in: the application binary holds an explicit list of plugin
//! constructors and loads each into a [`Registry`]. Built-in features (robot packages, planners, importers)
//! register through this same API, so the extension points are exercised from day one.
//!
//! Rules for everything exposed to plugins, here and in `gc_robotics_api`:
//! - signatures use plain data (`&[f64]`, serde structs) and API trait objects only — never types of the
//!   maths, collision or user-interface libraries;
//! - nothing is `#[non_exhaustive]` while the version is 0.0.x; methods added to a trait later get default
//!   bodies and [`GC_PLUGIN_API_VERSION`] is bumped.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::float_cmp, reason = "tests compare exactly representable values"))]

mod import;
mod params;
mod registry;

pub use import::{IMPORTERS, ImportError, Imported, ImportedPart, Importer};
pub use params::{ParamError, ParamKind, ParamSchema, ParamSpec, ParamValue, ParamValues};
pub use registry::{ExtensionPoint, LoadedPlugin, Plugin, PluginError, PluginManifest, Registrar, Registry};

use serde::{Deserialize, Serialize};

/// Version of the plugin contract. A plugin built against another version is refused.
pub const GC_PLUGIN_API_VERSION: u32 = 1;

/// A short text in every language the application supports.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Text {
    /// Korean.
    pub ko: String,
    /// English.
    pub en: String,
}

impl Text {
    /// A text with both translations.
    pub fn new(ko: &str, en: &str) -> Self {
        Self { ko: ko.to_owned(), en: en.to_owned() }
    }
}

/// A command a plugin contributes, described as data so that menus, the ribbon and shortcuts can list it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandSpec {
    /// Unique id, prefixed with the plugin id: `"gc.mesh.import"`.
    pub id: String,
    /// Name shown to the user.
    pub title: Text,
    /// Ribbon tab or menu the command belongs to.
    pub category: String,
    /// Default shortcut such as `"Ctrl+I"`, if any.
    pub shortcut: Option<String>,
}
