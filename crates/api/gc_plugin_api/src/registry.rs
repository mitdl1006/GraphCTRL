//! Plugins, extension points and the registry that connects them.

use std::any::Any;
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use crate::GC_PLUGIN_API_VERSION;

/// Who a plugin is. A `const` in the plugin crate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PluginManifest {
    /// Unique id in reverse-domain style: `"gc.ur"`.
    pub id: &'static str,
    /// Name shown to the user.
    pub name: &'static str,
    /// The plugin's own version, for display.
    pub version: &'static str,
    /// The [`GC_PLUGIN_API_VERSION`] the plugin was written for.
    pub api: u32,
}

/// Why a plugin was not loaded.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PluginError {
    /// The plugin targets another version of the contract.
    #[error("plugin `{plugin}` targets plugin API {found}, this application provides {expected}")]
    ApiMismatch {
        /// Plugin id.
        plugin: String,
        /// Version the plugin targets.
        found: u32,
        /// Version of this application.
        expected: u32,
    },
    /// A plugin with the same id is already loaded.
    #[error("a plugin with id `{0}` is already loaded")]
    DuplicatePlugin(String),
    /// The plugin reported a failure of its own.
    #[error("plugin failed to register: {0}")]
    Failed(String),
    /// The plugin panicked while registering.
    #[error("plugin panicked while registering: {0}")]
    Panicked(String),
}

/// A unit of functionality that registers contributions with the application.
pub trait Plugin: Send + Sync {
    /// Who this plugin is.
    fn manifest(&self) -> &PluginManifest;

    /// Adds the plugin's contributions. If this returns an error, nothing the plugin added is kept.
    fn register(&self, reg: &mut Registrar<'_>) -> Result<(), PluginError>;
}

/// A typed slot plugins contribute to. Declared as a `const` next to the trait it carries, for example
/// `gc_robotics_api::MOTION_PLANNERS: ExtensionPoint<dyn MotionPlanner>`.
pub struct ExtensionPoint<T: ?Sized + 'static> {
    id: &'static str,
    carries: PhantomData<fn() -> Arc<T>>,
}

impl<T: ?Sized + 'static> ExtensionPoint<T> {
    /// Declares an extension point. Ids must be unique across the application.
    pub const fn new(id: &'static str) -> Self {
        Self { id, carries: PhantomData }
    }

    /// The id of the extension point.
    pub const fn id(&self) -> &'static str {
        self.id
    }
}

// Written by hand so that no bound on `T` is required.
impl<T: ?Sized + 'static> Clone for ExtensionPoint<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: ?Sized + 'static> Copy for ExtensionPoint<T> {}

/// A stored contribution: an `Arc<T>` behind `Any`.
type Contribution = Box<dyn Any + Send + Sync>;

/// Handed to [`Plugin::register`]; collects what the plugin contributes.
pub struct Registrar<'a> {
    staged: &'a mut Vec<(&'static str, Contribution)>,
}

impl Registrar<'_> {
    /// Contributes `item` to an extension point.
    pub fn add<T: ?Sized + Send + Sync + 'static>(&mut self, point: ExtensionPoint<T>, item: Arc<T>) {
        self.staged.push((point.id, Box::new(item)));
    }
}

/// A plugin the registry knows about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadedPlugin {
    /// The plugin's manifest.
    pub manifest: PluginManifest,
    /// `None` when the plugin is active; otherwise why its contributions were discarded.
    pub error: Option<PluginError>,
}

/// Everything the loaded plugins contributed, by extension point.
#[derive(Default)]
pub struct Registry {
    contributions: BTreeMap<&'static str, Vec<Contribution>>,
    plugins: Vec<LoadedPlugin>,
}

impl Registry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads a plugin. On failure the plugin is listed with its error and contributes nothing; other
    /// plugins are unaffected.
    pub fn load(&mut self, plugin: &dyn Plugin) -> Result<(), PluginError> {
        let manifest = *plugin.manifest();
        let result = self.try_load(plugin, &manifest);
        self.plugins.push(LoadedPlugin { manifest, error: result.clone().err() });
        result
    }

    fn try_load(&mut self, plugin: &dyn Plugin, manifest: &PluginManifest) -> Result<(), PluginError> {
        if manifest.api != GC_PLUGIN_API_VERSION {
            return Err(PluginError::ApiMismatch {
                plugin: manifest.id.to_owned(),
                found: manifest.api,
                expected: GC_PLUGIN_API_VERSION,
            });
        }
        if self.plugins.iter().any(|loaded| loaded.manifest.id == manifest.id) {
            return Err(PluginError::DuplicatePlugin(manifest.id.to_owned()));
        }
        let mut staged = Vec::new();
        // A panicking plugin must not take the application down. Its staged contributions are dropped.
        let outcome =
            catch_unwind(AssertUnwindSafe(|| plugin.register(&mut Registrar { staged: &mut staged })));
        match outcome {
            Ok(Ok(())) => {
                for (point, contribution) in staged {
                    self.contributions.entry(point).or_default().push(contribution);
                }
                Ok(())
            }
            Ok(Err(error)) => Err(error),
            Err(payload) => {
                let text = payload
                    .downcast_ref::<&str>()
                    .map(|text| (*text).to_owned())
                    .or_else(|| payload.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "no message".to_owned());
                Err(PluginError::Panicked(text))
            }
        }
    }

    /// Everything contributed to an extension point, in load order.
    pub fn get<T: ?Sized + 'static>(&self, point: ExtensionPoint<T>) -> Vec<Arc<T>> {
        self.contributions
            .get(point.id)
            .into_iter()
            .flatten()
            .filter_map(|contribution| contribution.downcast_ref::<Arc<T>>().cloned())
            .collect()
    }

    /// The plugins that were loaded or refused, in load order.
    pub fn plugins(&self) -> &[LoadedPlugin] {
        &self.plugins
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    trait Greeter: Send + Sync {
        fn greet(&self) -> String;
    }
    struct Hello(&'static str);
    impl Greeter for Hello {
        fn greet(&self) -> String {
            format!("hello from {}", self.0)
        }
    }
    const GREETERS: ExtensionPoint<dyn Greeter> = ExtensionPoint::new("test.greeters");
    const NUMBERS: ExtensionPoint<u32> = ExtensionPoint::new("test.numbers");

    enum Behaviour {
        Works,
        Fails,
        Panics,
    }
    struct TestPlugin {
        manifest: PluginManifest,
        behaviour: Behaviour,
    }
    impl TestPlugin {
        fn new(id: &'static str, behaviour: Behaviour) -> Self {
            Self {
                manifest: PluginManifest { id, name: id, version: "0.1.0", api: GC_PLUGIN_API_VERSION },
                behaviour,
            }
        }
    }
    impl Plugin for TestPlugin {
        fn manifest(&self) -> &PluginManifest {
            &self.manifest
        }
        fn register(&self, reg: &mut Registrar<'_>) -> Result<(), PluginError> {
            reg.add(GREETERS, Arc::new(Hello(self.manifest.id)));
            reg.add(NUMBERS, Arc::new(7));
            match self.behaviour {
                Behaviour::Works => Ok(()),
                Behaviour::Fails => Err(PluginError::Failed("no licence".to_owned())),
                Behaviour::Panics => panic!("plugin bug"),
            }
        }
    }

    #[test]
    fn contributions_are_found_by_extension_point_in_load_order() {
        let mut registry = Registry::new();
        registry.load(&TestPlugin::new("a", Behaviour::Works)).unwrap();
        registry.load(&TestPlugin::new("b", Behaviour::Works)).unwrap();
        let greetings: Vec<String> = registry.get(GREETERS).iter().map(|g| g.greet()).collect();
        assert_eq!(greetings, vec!["hello from a", "hello from b"]);
        assert_eq!(registry.get(NUMBERS).iter().map(|n| **n).sum::<u32>(), 14);
        assert!(registry.get(ExtensionPoint::<dyn Greeter>::new("test.unused")).is_empty());
    }

    #[test]
    fn a_failing_or_panicking_plugin_contributes_nothing_and_is_listed_with_its_error() {
        let mut registry = Registry::new();
        registry.load(&TestPlugin::new("good", Behaviour::Works)).unwrap();
        assert_eq!(
            registry.load(&TestPlugin::new("bad", Behaviour::Fails)),
            Err(PluginError::Failed("no licence".to_owned()))
        );
        assert_eq!(
            registry.load(&TestPlugin::new("ugly", Behaviour::Panics)),
            Err(PluginError::Panicked("plugin bug".to_owned()))
        );
        assert_eq!(registry.get(GREETERS).len(), 1);
        let errors: Vec<bool> = registry.plugins().iter().map(|p| p.error.is_some()).collect();
        assert_eq!(errors, vec![false, true, true]);
    }

    #[test]
    fn wrong_api_version_and_duplicate_ids_are_refused() {
        let mut registry = Registry::new();
        let mut old = TestPlugin::new("old", Behaviour::Works);
        old.manifest.api = GC_PLUGIN_API_VERSION + 1;
        assert!(matches!(registry.load(&old), Err(PluginError::ApiMismatch { .. })));
        registry.load(&TestPlugin::new("twin", Behaviour::Works)).unwrap();
        assert_eq!(
            registry.load(&TestPlugin::new("twin", Behaviour::Works)),
            Err(PluginError::DuplicatePlugin("twin".to_owned()))
        );
        assert_eq!(registry.get(GREETERS).len(), 1);
    }

    #[test]
    fn a_point_only_returns_contributions_of_its_own_type() {
        // Two points that (wrongly) share an id cannot hand out values of the other's type.
        const CLASH: ExtensionPoint<u64> = ExtensionPoint::new("test.numbers");
        let mut registry = Registry::new();
        registry.load(&TestPlugin::new("a", Behaviour::Works)).unwrap();
        assert!(registry.get(CLASH).is_empty());
        assert_eq!(registry.get(NUMBERS).len(), 1);
    }

    #[test]
    fn api_traits_are_object_safe() {
        const _: Option<&dyn Plugin> = None;
        const _: Option<&dyn crate::Importer> = None;
    }
}
