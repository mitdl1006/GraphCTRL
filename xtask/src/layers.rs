//! Layer check: crates may only depend downwards (see `layers.toml`).

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::meta;

/// Contents of `layers.toml`.
#[derive(Debug, Deserialize)]
pub struct Layers {
    /// Layer names from lowest to highest.
    pub order: Vec<String>,
    pub plugins: Plugins,
}

#[derive(Debug, Deserialize)]
pub struct Plugins {
    /// Layers a plugin crate may depend on.
    pub allowed_layers: Vec<String>,
    /// Extra workspace crates a named plugin may depend on.
    #[serde(default)]
    pub exceptions: BTreeMap<String, Vec<String>>,
}

/// Name of the directory under `crates/` that holds plugin crates.
const PLUGINS: &str = "plugins";
/// Layer whose crates assemble the application and may therefore depend on plugins.
const APP: &str = "app";

/// Why `from` (in `from_layer`) must not depend on `to` (in `to_layer`), or `None` when it may.
pub fn violation(layers: &Layers, from: &str, from_layer: &str, to: &str, to_layer: &str) -> Option<String> {
    let index = |layer: &str| layers.order.iter().position(|l| l == layer);
    if from_layer == PLUGINS {
        let excepted = layers.plugins.exceptions.get(from).is_some_and(|list| list.iter().any(|c| c == to));
        let allowed = layers.plugins.allowed_layers.iter().any(|l| l == to_layer);
        return (!allowed && !excepted).then(|| {
            format!("plugin `{from}` depends on `{to}` ({to_layer}); plugins may use only {:?}", {
                &layers.plugins.allowed_layers
            })
        });
    }
    if to_layer == PLUGINS {
        return (from_layer != APP)
            .then(|| format!("`{from}` ({from_layer}) depends on plugin `{to}`; only {APP} crates may"));
    }
    match (index(from_layer), index(to_layer)) {
        (Some(a), Some(b)) if b <= a => None,
        (Some(_), Some(_)) => {
            Some(format!("`{from}` ({from_layer}) depends on `{to}` ({to_layer}), which is a higher layer"))
        }
        _ => Some(format!(
            "`{from}` ({from_layer}) or `{to}` ({to_layer}) is in a layer layers.toml does not list"
        )),
    }
}

/// The layer of a workspace crate: the directory name under `crates/`.
fn layer_of(root: &Path, manifest_path: &Path) -> Option<String> {
    let relative = manifest_path.strip_prefix(root).ok()?;
    let mut parts = relative.components().map(|c| c.as_os_str().to_string_lossy().into_owned());
    (parts.next()? == "crates").then(|| parts.next()).flatten()
}

/// Checks every workspace crate under `crates/` against `layers.toml`. Dev-dependencies are exempt.
pub fn check(root: &Path) -> Result<()> {
    let text = std::fs::read_to_string(root.join("layers.toml")).context("could not read layers.toml")?;
    let layers: Layers = toml::from_str(&text).context("could not parse layers.toml")?;
    let metadata = meta::load(root, None)?;
    let layer_by_crate: BTreeMap<&str, String> = metadata
        .packages
        .iter()
        .filter(|p| metadata.workspace_members.contains(&p.id))
        .filter_map(|p| Some((p.name.as_str(), layer_of(root, &p.manifest_path)?)))
        .collect();

    let mut violations = Vec::new();
    let mut edges = 0_usize;
    for package in &metadata.packages {
        let Some(from_layer) = layer_by_crate.get(package.name.as_str()) else { continue };
        for dependency in &package.dependencies {
            if dependency.kind.as_deref() == Some("dev") || dependency.path.is_none() {
                continue;
            }
            edges += 1;
            let found = match layer_by_crate.get(dependency.name.as_str()) {
                Some(to_layer) => violation(&layers, &package.name, from_layer, &dependency.name, to_layer),
                None => Some(format!(
                    "`{}` depends on `{}`, which is not under crates/",
                    package.name, dependency.name
                )),
            };
            violations.extend(found);
        }
    }
    if !violations.is_empty() {
        bail!("layer violations:\n  {}", violations.join("\n  "));
    }
    println!("check-layers: {} crates, {edges} workspace dependencies, no violations", layer_by_crate.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layers() -> Layers {
        toml::from_str(
            r#"
            order = ["foundation", "api", "domain", "render", "ui", "app"]
            [plugins]
            allowed_layers = ["foundation", "api"]
            exceptions = { gc_plugin_planners = ["gc_plan"] }
            "#,
        )
        .unwrap()
    }

    #[test]
    fn lower_and_same_layer_dependencies_are_allowed() {
        let l = layers();
        assert_eq!(violation(&l, "gc_sketch", "domain", "gc_core", "foundation"), None);
        assert_eq!(violation(&l, "gc_sketch", "domain", "gc_solver", "domain"), None);
    }

    #[test]
    fn upward_dependencies_are_rejected() {
        let l = layers();
        assert!(violation(&l, "gc_core", "foundation", "gc_doc", "domain").is_some());
        assert!(violation(&l, "gc_render", "render", "gc_ui", "ui").is_some());
    }

    #[test]
    fn plugins_reach_only_allowed_layers_and_their_exceptions() {
        let l = layers();
        assert_eq!(violation(&l, "gc_plugin_ur", "plugins", "gc_math", "foundation"), None);
        assert_eq!(violation(&l, "gc_plugin_planners", "plugins", "gc_plan", "domain"), None);
        assert!(violation(&l, "gc_plugin_ur", "plugins", "gc_plan", "domain").is_some());
        assert!(violation(&l, "gc_plugin_ur", "plugins", "gc_plugin_mesh", "plugins").is_some());
    }

    #[test]
    fn only_app_crates_depend_on_plugins() {
        let l = layers();
        assert_eq!(violation(&l, "graphctrl", "app", "gc_plugin_ur", "plugins"), None);
        assert!(violation(&l, "gc_session", "domain", "gc_plugin_ur", "plugins").is_some());
    }

    #[test]
    fn unknown_layers_are_rejected() {
        assert!(violation(&layers(), "gc_x", "misc", "gc_core", "foundation").is_some());
    }
}
