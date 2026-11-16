//! `cargo metadata` access shared by the checks.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

/// The part of `cargo metadata` output the checks read.
#[derive(Debug, Deserialize)]
pub struct Metadata {
    pub packages: Vec<Package>,
    pub workspace_members: Vec<String>,
    pub resolve: Option<Resolve>,
}

#[derive(Debug, Deserialize)]
pub struct Package {
    pub id: String,
    pub name: String,
    pub version: String,
    pub license: Option<String>,
    pub source: Option<String>,
    pub manifest_path: PathBuf,
    pub dependencies: Vec<Dependency>,
}

#[derive(Debug, Deserialize)]
pub struct Dependency {
    pub name: String,
    /// `None` for a normal dependency, otherwise `"dev"` or `"build"`.
    pub kind: Option<String>,
    /// Set for path dependencies, i.e. workspace crates.
    pub path: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
pub struct Resolve {
    pub nodes: Vec<Node>,
}

#[derive(Debug, Deserialize)]
pub struct Node {
    pub id: String,
}

impl Metadata {
    /// Ids of the packages that are part of the resolved graph (all of them unless a platform filter was given).
    pub fn resolved_ids(&self) -> BTreeSet<&str> {
        self.resolve.iter().flat_map(|r| r.nodes.iter().map(|n| n.id.as_str())).collect()
    }
}

/// The directory holding the workspace `Cargo.toml`.
pub fn workspace_root() -> Result<PathBuf> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().context("xtask has no parent directory")?;
    Ok(root.to_path_buf())
}

/// The cargo binary that started this process, or `cargo` from `PATH`.
pub fn cargo() -> Command {
    Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
}

/// Runs `cargo metadata`, optionally restricted to the dependency graph of one target triple.
pub fn load(root: &Path, filter_platform: Option<&str>) -> Result<Metadata> {
    let mut command = cargo();
    command.current_dir(root).args(["metadata", "--format-version", "1", "--locked"]);
    if let Some(triple) = filter_platform {
        command.args(["--filter-platform", triple]);
    }
    let output = command.output().context("could not run `cargo metadata`")?;
    if !output.status.success() {
        bail!("`cargo metadata` failed:\n{}", String::from_utf8_lossy(&output.stderr));
    }
    serde_json::from_slice(&output.stdout).context("could not parse `cargo metadata` output")
}
