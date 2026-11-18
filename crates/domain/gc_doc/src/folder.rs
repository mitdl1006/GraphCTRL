//! User-defined browser folders (Inventor 2026 help, Assembly Browser Reference,
//! GUID-CB6B1637-71AD-4753-8CC9-19559BFEE3A5: "User-defined Folder groups assembly nodes for better browser
//! organization"; Working with the Assembly Browser, GUID-5F975252-BDB4-488C-A2C5-8B15C7D3C641: "Create
//! folders to organize the browser nodes into groups or to communicate design intent").
//!
//! A folder only arranges the browser: what is in it stays where it is in the assembly (its level, its
//! relationships, the bill of materials). A folder holds rows of one level of the tree; a row is in at
//! most one folder.

use serde::{Deserialize, Serialize};

use crate::command::{Created, EditError};
use crate::document::Document;
use crate::subassembly::TreeKey;
use crate::txn::Txn;

/// A user-defined folder of the browser.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BrowserFolder {
    /// Its identity (never reused within a document).
    pub id: u32,
    /// Name shown in the browser.
    pub name: String,
    /// The rows in it, in the order they were put in.
    pub members: Vec<TreeKey>,
}

/// Changes to browser folders.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum FolderOp {
    /// Add to New Folder: a folder holding rows of one level (possibly none yet). An empty name takes the
    /// next free "Folder:N".
    Add {
        /// Its name.
        name: String,
        /// What goes in.
        members: Vec<TreeKey>,
    },
    /// Drags rows into a folder, or out of their folders (`into: None`). They must be at the level of
    /// what the folder holds already.
    Move {
        /// What moves.
        members: Vec<TreeKey>,
        /// The folder, or `None` for no folder.
        into: Option<u32>,
    },
    /// Renames a folder (not empty, not another folder's name).
    Rename {
        /// Which.
        id: u32,
        /// The new name.
        name: String,
    },
    /// Removes a folder; what it held stays in the browser at its level.
    Delete(u32),
}

impl Document {
    /// The browser's user-defined folders, each with the rows it holds that still exist and still are at
    /// one level of the tree (the level of its first such row).
    pub fn folders(&self) -> Vec<BrowserFolder> {
        let stored = self.timeline().map(|(_, t)| t.components.folders.clone()).unwrap_or_default();
        stored
            .into_iter()
            .map(|mut folder| {
                folder.members.retain(|m| self.has_tree_row(*m));
                let level = folder.members.first().map(|m| self.tree_parent(*m));
                folder.members.retain(|m| Some(self.tree_parent(*m)) == level);
                folder
            })
            .collect()
    }

    /// The folder a row is in, if any.
    pub fn folder_of(&self, key: TreeKey) -> Option<u32> {
        self.folders().into_iter().find(|f| f.members.contains(&key)).map(|f| f.id)
    }
}

/// The rows, each once, when they all exist and are at one level; refused otherwise.
fn one_level(doc: &Document, members: &[TreeKey]) -> Result<Vec<TreeKey>, EditError> {
    let mut rows: Vec<TreeKey> = Vec::with_capacity(members.len());
    for member in members {
        if !doc.has_tree_row(*member) {
            return Err(EditError::BadValue);
        }
        if !rows.contains(member) {
            rows.push(*member);
        }
    }
    let level = rows.first().map(|m| doc.tree_parent(*m));
    if rows.iter().any(|m| Some(doc.tree_parent(*m)) != level) {
        return Err(EditError::BadValue);
    }
    Ok(rows)
}

pub(crate) fn run(txn: &mut Txn<'_>, op: &FolderOp) -> Result<(Vec<Created>, &'static str), EditError> {
    // What is stored, cleaned of rows that are gone or have left the level.
    let mut folders = txn.doc().folders();
    let label = match op {
        FolderOp::Add { name, members } => {
            let rows = one_level(txn.doc(), members)?;
            let id = txn.doc().timeline().map_or(0, |(_, t)| t.components.next_folder);
            let name = match name.trim() {
                "" => (1..)
                    .map(|n| format!("Folder:{n}"))
                    .find(|candidate| !folders.iter().any(|f| &f.name == candidate))
                    .unwrap_or_default(),
                given if folders.iter().any(|f| f.name == given) => return Err(EditError::NameTaken),
                given => given.to_owned(),
            };
            for folder in &mut folders {
                folder.members.retain(|m| !rows.contains(m));
            }
            folders.push(BrowserFolder { id, name, members: rows });
            crate::cell::set_timeline(txn, |t| {
                t.components.folders = folders;
                t.components.next_folder = id + 1;
            })?;
            return Ok((Vec::new(), "Add to New Folder"));
        }
        FolderOp::Move { members, into } => {
            let rows = one_level(txn.doc(), members)?;
            if rows.is_empty() {
                return Err(EditError::BadValue);
            }
            for folder in &mut folders {
                folder.members.retain(|m| !rows.contains(m));
            }
            if let Some(id) = into {
                let doc = txn.doc();
                let folder = folders.iter_mut().find(|f| f.id == *id).ok_or(EditError::BadValue)?;
                // A folder is at one level of the tree.
                let level = folder.members.first().map(|m| doc.tree_parent(*m));
                if level.is_some_and(|level| rows.iter().any(|m| doc.tree_parent(*m) != level)) {
                    return Err(EditError::BadValue);
                }
                folder.members.extend(rows);
            }
            "Move to folder"
        }
        FolderOp::Rename { id, name } => {
            let name = name.trim();
            if name.is_empty() {
                return Err(EditError::BadValue);
            }
            if folders.iter().any(|f| f.id != *id && f.name == name) {
                return Err(EditError::NameTaken);
            }
            let folder = folders.iter_mut().find(|f| f.id == *id).ok_or(EditError::BadValue)?;
            name.clone_into(&mut folder.name);
            "Rename folder"
        }
        FolderOp::Delete(id) => {
            let before = folders.len();
            folders.retain(|f| f.id != *id);
            if folders.len() == before {
                return Err(EditError::BadValue);
            }
            "Delete folder"
        }
    };
    crate::cell::set_timeline(txn, |t| t.components.folders = folders)?;
    Ok((Vec::new(), label))
}
