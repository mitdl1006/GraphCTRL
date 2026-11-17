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
