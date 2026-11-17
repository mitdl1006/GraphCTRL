//! Projects: a `.gctrl` file and, next to it, a folder with the mesh files it refers to.
//!
//! In memory every file is referred to by an absolute path. Saving copies the files that are outside the
//! project's folder into `<name>_files` and writes paths relative to the project; opening turns them
//! back into absolute paths.

use std::path::{Component, Path, PathBuf};

use gc_doc::{Body, CellOp, Command, Document, Solid, apply};
use gc_robot::Shape;

use crate::{PersistError, Session};

/// A path relative to `base` written with `/`, the same on every system.
fn relative(path: &Path, base: &Path) -> Option<String> {
    let inside = path.strip_prefix(base).ok()?;
    let parts: Vec<String> = inside
        .components()
        .map(|c| match c {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Option<_>>()?;
    Some(parts.join("/"))
}

/// A stored path made absolute against the project's folder.
fn absolute(stored: &str, base: &Path) -> PathBuf {
    let path = Path::new(stored);
    if path.is_absolute() { path.to_path_buf() } else { base.join(stored.split('/').collect::<PathBuf>()) }
}

/// A file or folder name that is safe on every system.
fn safe_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' })
        .collect();
    if cleaned.trim_matches('.').is_empty() { "item".to_owned() } else { cleaned }
}

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> PersistError + '_ {
    move |source| PersistError::Io { path: path.to_path_buf(), source }
}

/// Copies `from` to `to` unless they are the same file.
fn copy(from: &Path, to: &Path) -> Result<(), PersistError> {
    let same = match (from.canonicalize(), to.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    if !same {
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(io(parent))?;
        }
        std::fs::copy(from, to).map_err(io(from))?;
    }
    Ok(())
}

impl Session {
    /// The document as it is written to `path`: with its files inside the project and relative paths.
    pub(crate) fn packed_for(&self, path: &Path) -> Result<Document, PersistError> {
        let base = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
        let stem = path.file_stem().map_or_else(|| "project".to_owned(), |s| safe_name(&s.to_string_lossy()));
        let files = base.join(format!("{stem}_files"));
        let mut ops = Vec::new();
        let mut taken = std::collections::BTreeSet::new();

        for (id, body) in self.doc.bodies().iter() {
            let Solid::Mesh { file, scale } = &body.solid else { continue };
            let source = PathBuf::from(file);
            let stored = match relative(&source, base) {
                Some(inside) => inside,
                None => {
                    // Outside the project: copied in, under a name no other file has taken.
                    let name = source
                        .file_name()
                        .map_or_else(|| "mesh.stl".to_owned(), |n| safe_name(&n.to_string_lossy()));
                    let mut target = files.join(&name);
                    let mut n = 2;
                    while taken.contains(&target) {
                        target = files.join(format!("{n}_{name}"));
                        n += 1;
                    }
                    copy(&source, &target)?;
                    taken.insert(target.clone());
                    relative(&target, base).unwrap_or_default()
                }
            };
            let solid = Solid::Mesh { file: stored, scale: *scale };
            ops.push(CellOp::SetBody { id, body: Body { solid, ..body.clone() } });
        }

        for (id, robot) in self.doc.robots().iter() {
            if robot.resource_dir.is_empty() {
                continue; // a robot from the library brings its own files
            }
            let dir = PathBuf::from(&robot.resource_dir);
            let stored = match relative(&dir, base) {
                Some(inside) => inside,
                None => {
                    // The mesh files the description uses, copied where the same lookup finds them again.
                    let target = files.join(safe_name(&robot.name));
                    if let Some(loaded) = self.robot_model(id) {
                        let geometries =
                            loaded.model.links.iter().flat_map(|l| l.visuals.iter().chain(&l.collisions));
                        for geometry in geometries {
                            let Shape::Mesh { uri, .. } = &geometry.shape else { continue };
                            let inside = match uri.strip_prefix("package://") {
                                Some(rest) => rest.split_once('/').map(|(_, inside)| inside.to_owned()),
                                None => {
                                    let plain = uri.strip_prefix("file://").unwrap_or(uri);
                                    (!Path::new(plain).is_absolute()).then(|| plain.to_owned())
                                }
                            };
                            if let (Some(inside), Some(found)) =
                                (inside, crate::cell::find_near(&robot.resource_dir, uri))
                            {
                                copy(&found, &target.join(inside.split('/').collect::<PathBuf>()))?;
                            }
                        }
                    }
                    relative(&target, base).unwrap_or_default()
                }
            };
            ops.push(CellOp::SetRobotFiles { id, resource_dir: stored });
        }

        let mut packed = self.doc.clone();
        for op in ops {
            // These edits only rewrite paths; they cannot be refused.
            drop(apply(&mut packed, &Command::Cell(op)));
        }
        Ok(packed)
    }
}

/// A document read from `path` with the project's relative paths made absolute.
pub(crate) fn unpacked(mut doc: Document, path: &Path) -> Document {
    let base = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    let mut ops = Vec::new();
    for (id, body) in doc.bodies().iter() {
        if let Solid::Mesh { file, scale } = &body.solid
            && !Path::new(file).is_absolute()
        {
            let file = absolute(file, base).to_string_lossy().into_owned();
            ops.push(CellOp::SetBody {
                id,
                body: Body { solid: Solid::Mesh { file, scale: *scale }, ..body.clone() },
            });
        }
    }
    for (id, robot) in doc.robots().iter() {
        if !robot.resource_dir.is_empty() && !Path::new(&robot.resource_dir).is_absolute() {
            let resource_dir = absolute(&robot.resource_dir, base).to_string_lossy().into_owned();
            ops.push(CellOp::SetRobotFiles { id, resource_dir });
        }
    }
    for op in ops {
        drop(apply(&mut doc, &Command::Cell(op)));
    }
    doc
}
