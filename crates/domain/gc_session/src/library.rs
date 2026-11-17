//! The robot library: a folder of robot packages, kept outside the executable like add-ons.
//!
//! Each package is a folder with a `robot.json` that names its robot description (URDF) and says who
//! made it and under which licence; the meshes the description refers to sit beside it
//! (`package://<folder>/meshes/...`). Robots added from the library are stored in documents as
//! `library://<folder>`, so a project opens wherever the library is installed.

use std::path::{Path, PathBuf};

/// What a package's `robot.json` says.
#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize)]
#[serde(default)]
pub struct PackageInfo {
    /// The name shown in the library list.
    pub name: String,
    /// Who makes the robot.
    pub vendor: String,
    /// The robot description file, relative to the package folder.
    pub description: String,
    /// How far the arm reaches, in metres.
    pub reach_m: Option<f64>,
    /// What the arm carries, in kilograms.
    pub payload_kg: Option<f64>,
    /// The licence file, relative to the package folder.
    pub license: Option<String>,
    /// The licence's name, e.g. `BSD-3-Clause`.
    pub license_name: Option<String>,
    /// Whose work the package contains.
    pub attribution: Option<String>,
}

/// A robot package found in the library.
#[derive(Clone, Debug, PartialEq)]
pub struct Package {
    /// The package's folder name, used in `library://` paths.
    pub id: String,
    /// The package folder.
    pub dir: PathBuf,
    /// What its `robot.json` says.
    pub info: PackageInfo,
}

impl Package {
    /// The robot description text.
    pub fn urdf(&self) -> Result<String, String> {
        let path = self.dir.join(&self.info.description);
        std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The licence text, when the package has one.
    pub fn license_text(&self) -> Option<String> {
        std::fs::read_to_string(self.dir.join(self.info.license.as_deref()?)).ok()
    }

    /// How documents refer to the package's folder.
    pub fn resource_dir(&self) -> String {
        format!("{LIBRARY_SCHEME}{}", self.id)
    }
}

/// The prefix of a robot's resource folder that names a library package.
pub const LIBRARY_SCHEME: &str = "library://";

/// The packages in a library folder, by name. Folders without a readable `robot.json` naming an
/// existing description are left out.
pub fn scan(dir: &Path) -> Vec<Package> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut packages: Vec<Package> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let folder = entry.path();
            let text = std::fs::read_to_string(folder.join("robot.json")).ok()?;
            let info: PackageInfo = serde_json::from_str(&text).ok()?;
            // The description must lie inside the package: a relative path without `..`.
            let inside = std::path::Path::new(&info.description)
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_) | std::path::Component::CurDir));
            if info.name.is_empty() || !inside || !folder.join(&info.description).is_file() {
                return None;
            }
            let id = folder.file_name()?.to_string_lossy().into_owned();
            Some(Package { id, dir: folder, info })
        })
        .collect();
    packages.sort_by(|a, b| a.info.name.cmp(&b.info.name).then_with(|| a.id.cmp(&b.id)));
    packages
}

/// The library folder: the first of `starts` and the folders above each (up to four levels) that has
/// a `robots` folder with at least one package. Covers an installed program (beside the executable)
/// and a source checkout (above `target/debug`).
pub fn find(starts: &[PathBuf]) -> Option<PathBuf> {
    starts
        .iter()
        .flat_map(|start| start.ancestors().take(5).map(|dir| dir.join("robots")).collect::<Vec<_>>())
        .find(|dir| !scan(dir).is_empty())
}
