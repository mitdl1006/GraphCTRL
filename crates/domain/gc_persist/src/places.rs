//! Where the application keeps its own files: the list of recent projects and the automatic save of
//! unsaved work. The folder is looked up here rather than with a crate (see `docs/PLAN.md`, appendix A).

use std::path::{Path, PathBuf};

use crate::PersistError;

/// How many recent projects are remembered.
pub const RECENT: usize = 8;

/// The application's own folder: `%APPDATA%\GraphCTRL` on Windows, `$XDG_CONFIG_HOME/graphctrl` or
/// `~/.config/graphctrl` elsewhere. `None` when the system does not say where that is.
pub fn settings_dir() -> Option<PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    if cfg!(windows) {
        var("APPDATA").map(|dir| dir.join("GraphCTRL"))
    } else {
        var("XDG_CONFIG_HOME")
            .or_else(|| var("HOME").map(|home| home.join(".config")))
            .map(|dir| dir.join("graphctrl"))
    }
}

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> PersistError + '_ {
    move |source| PersistError::Io { path: path.to_path_buf(), source }
}

fn recent_file(dir: &Path) -> PathBuf {
    dir.join("recent.txt")
}

/// The recent projects, newest first; files that no longer exist are left out.
pub fn load_recent(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_to_string(recent_file(dir))
        .unwrap_or_default()
        .lines()
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .take(RECENT)
        .collect()
}

/// Puts `project` first in the recent list and returns the list.
pub fn remember_recent(dir: &Path, project: &Path) -> Result<Vec<PathBuf>, PersistError> {
    let project = project.canonicalize().unwrap_or_else(|_| project.to_path_buf());
    let mut list = load_recent(dir);
    list.retain(|path| path.canonicalize().unwrap_or_else(|_| path.clone()) != project);
    list.insert(0, project);
    list.truncate(RECENT);
    std::fs::create_dir_all(dir).map_err(io(dir))?;
    let text: String = list.iter().map(|p| format!("{}\n", p.display())).collect();
    let file = recent_file(dir);
    std::fs::write(&file, text).map_err(io(&file))?;
    Ok(list)
}

/// The user's settings, kept between runs in `settings.json`. Missing fields take their defaults, so
/// a file from an older version still loads.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Format version of the file.
    pub version: u32,
    /// The interface language as a code (`ko`, `en`); empty for the default.
    pub language: String,
    /// Whether the AI agent server (MCP on localhost) starts with the application.
    pub agent_enabled: bool,
    /// Its port; 0 for the default.
    pub agent_port: u16,
    /// The access token agents must send; empty until one is made.
    pub agent_token: String,
    /// The side panels are opaque instead of letting the 3D scene show through (default: see-through).
    pub opaque_panels: bool,
    /// The light colour scheme instead of the dark one (default: dark, as in Inventor 2026).
    pub light_theme: bool,
    /// Sketches made on a face do not get the face's edges projected into them (Application Options, Sketch
    /// tab, "Autoproject edges for sketch creation and edit"; default: on, so this is false).
    pub autoproject_edges_off: bool,
}

/// The settings file in the settings folder.
pub fn settings_file(dir: &Path) -> PathBuf {
    dir.join("settings.json")
}

/// The saved settings; the defaults when there are none or the file cannot be read.
pub fn load_settings(dir: &Path) -> Settings {
    std::fs::read_to_string(settings_file(dir))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Saves the settings.
pub fn save_settings(dir: &Path, settings: &Settings) -> Result<(), PersistError> {
    std::fs::create_dir_all(dir).map_err(io(dir))?;
    let file = settings_file(dir);
    let text = serde_json::to_string_pretty(&Settings { version: 1, ..settings.clone() })
        .map_err(|e| io(&file)(std::io::Error::other(e)))?;
    std::fs::write(&file, text).map_err(io(&file))
}

/// The automatic save: the document, and a file naming the project it belongs to (empty when it has
/// none yet).
pub fn autosave_files(dir: &Path) -> (PathBuf, PathBuf) {
    let recovery = dir.join("recovery");
    (recovery.join("autosave.gctrl"), recovery.join("autosave-of.txt"))
}

/// Writes the automatic save.
pub fn write_autosave(
    dir: &Path,
    doc: &gc_doc::Document,
    project: Option<&Path>,
) -> Result<(), PersistError> {
    let (file, of) = autosave_files(dir);
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(io(parent))?;
    }
    crate::save(doc, &file)?;
    let named = project.map(|p| p.display().to_string()).unwrap_or_default();
    std::fs::write(&of, named).map_err(io(&of))
}

/// The automatic save left by a session that did not end normally, and the project it belongs to.
pub fn pending_autosave(dir: &Path) -> Option<(PathBuf, Option<PathBuf>)> {
    let (file, of) = autosave_files(dir);
    if !file.is_file() {
        return None;
    }
    let project = std::fs::read_to_string(of).ok().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
    Some((file, project.map(PathBuf::from)))
}

/// Removes the automatic save (the work was saved or deliberately discarded).
pub fn clear_autosave(dir: &Path) {
    let (file, of) = autosave_files(dir);
    for path in [file, of, dir.join("recovery").join("autosave.gctrl.bak")] {
        // Missing files are fine: there was nothing to clear.
        drop(std::fs::remove_file(path));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_projects_are_newest_first_without_duplicates_or_missing_files() {
        let dir = tempfile::tempdir().unwrap();
        let settings = dir.path().join("settings");
        let files: Vec<PathBuf> = (0..10).map(|k| dir.path().join(format!("p{k}.gctrl"))).collect();
        for file in &files {
            std::fs::write(file, "x").unwrap();
            remember_recent(&settings, file).unwrap();
        }
        let list = remember_recent(&settings, &files[3]).unwrap();
        assert_eq!(list.len(), RECENT);
        assert_eq!(list[0].file_name(), files[3].file_name());
        assert_eq!(list[1].file_name(), files[9].file_name());
        assert_eq!(list.iter().filter(|p| p.file_name() == files[3].file_name()).count(), 1);
        std::fs::remove_file(&files[9]).unwrap();
        assert!(load_recent(&settings).iter().all(|p| p.file_name() != files[9].file_name()));
    }

    #[test]
    fn an_autosave_names_its_project_and_can_be_cleared() {
        let dir = tempfile::tempdir().unwrap();
        assert!(pending_autosave(dir.path()).is_none());
        let doc = gc_doc::Document::new();
        write_autosave(dir.path(), &doc, Some(Path::new("/work/cell.gctrl"))).unwrap();
        let (file, project) = pending_autosave(dir.path()).unwrap();
        assert!(crate::load(&file).unwrap().doc.same_content(&doc));
        assert_eq!(project, Some(PathBuf::from("/work/cell.gctrl")));
        write_autosave(dir.path(), &doc, None).unwrap();
        assert_eq!(pending_autosave(dir.path()).unwrap().1, None);
        clear_autosave(dir.path());
        assert!(pending_autosave(dir.path()).is_none());
    }
}
