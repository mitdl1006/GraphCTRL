//! Subassemblies made in the cell and the browser tree they give (Inventor 2026 help, Create Subassemblies
//! in Place, GUID-FCEE3F0E-AAE2-4619-8645-2588FEECD353): a subassembly is a named group of components,
//! which may hold other subassemblies. Components are put into one with Component Demote, moved between
//! levels by dragging in the browser, and a subassembly is treated as one unit in its parent.
//!
//! The groups and who belongs where are component states (in [`crate::design::Components`]), so they undo
//! and save with the rest. Imported assemblies and robots already have their own trees (parts, links);
//! those rows hang under their component as before.

use serde::{Deserialize, Serialize};

use crate::command::{Created, EditError};
use crate::design::Occurrence;
use crate::document::Document;
use crate::tree::ComponentNode;
use crate::txn::Txn;
use gc_core::Kind;

/// Identifies a subassembly made in the cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SubassemblyId(pub u32);

/// A subassembly made in the cell.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Subassembly {
    /// Its identity (never reused within a document).
    pub id: SubassemblyId,
    /// Name shown in the browser.
    pub name: String,
    /// The subassembly it sits in; `None` at the top level.
    pub parent: Option<SubassemblyId>,
}

/// A row of the browser's assembly tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TreeKey {
    /// A subassembly made in the cell.
    Subassembly(SubassemblyId),
    /// A component: placed in the cell, a part of an imported assembly, a robot's link or tool.
    Component(Occurrence),
}

/// One row of [`Document::component_tree`].
#[derive(Clone, Debug, PartialEq)]
pub struct TreeRow {
    /// What the row is.
    pub key: TreeKey,
    /// The row it hangs under; `None` at the top level.
    pub parent: Option<TreeKey>,
    /// Indentation: 0 at the top level.
    pub depth: u32,
    /// Name shown.
    pub name: String,
    /// The component's states (grounded, visible, ...); `None` for a subassembly.
    pub component: Option<ComponentNode>,
}

/// Changes to subassemblies.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SubassemblyOp {
    /// Component Demote: puts components (and subassemblies) of one level into a new subassembly at that
    /// level. Only components placed in the cell (bodies, robots, imported assemblies) and subassemblies
    /// can be demoted; an empty name takes the next free "Subassembly:N".
    Demote {
        /// What goes in.
        members: Vec<TreeKey>,
        /// Its name.
        name: String,
    },
    /// Moves components (and subassemblies) to another level (dragging in the browser); `into: None` is the
    /// top level. A subassembly cannot go into itself or one of its own.
    Move {
        /// What moves.
        members: Vec<TreeKey>,
        /// Where to.
        into: Option<SubassemblyId>,
    },
    /// Removes a subassembly; what it held goes up to its parent.
    Dissolve(SubassemblyId),
    /// Renames a subassembly.
    Rename {
        /// Which.
        id: SubassemblyId,
        /// The new name (not empty, not another subassembly's).
        name: String,
    },
}

impl Document {
    /// The subassemblies made in the cell.
    pub fn subassemblies(&self) -> Vec<Subassembly> {
        self.timeline().map(|(_, t)| t.components.subassemblies.clone()).unwrap_or_default()
    }

    /// The subassembly a component placed in the cell belongs to (`None`: the top level).
    pub fn subassembly_of(&self, occurrence: Occurrence) -> Option<SubassemblyId> {
        let (_, t) = self.timeline()?;
        t.components.placed_in.iter().find(|(o, _)| *o == occurrence).map(|(_, s)| *s)
    }

    /// The row a tree row hangs under; `None` at the top level or for a row that does not exist.
    pub fn tree_parent(&self, key: TreeKey) -> Option<TreeKey> {
        match key {
            TreeKey::Subassembly(id) => {
                self.subassemblies().iter().find(|s| s.id == id)?.parent.map(TreeKey::Subassembly)
            }
            TreeKey::Component(occurrence) => match occurrence {
                Occurrence::Item(_) => self.subassembly_of(occurrence).map(TreeKey::Subassembly),
                Occurrence::Part { assembly, index } => {
                    let parent = self.get(assembly)?.parts.get(index as usize)?.parent;
                    Some(TreeKey::Component(match parent {
                        Some(p) => Occurrence::Part { assembly, index: p },
                        None => Occurrence::Item(assembly.erase()),
                    }))
                }
                Occurrence::Link { robot, .. } | Occurrence::Tool { robot } => {
                    let rows = self.robot_links(robot);
                    let row = rows.iter().find(|n| n.occurrence == occurrence)?;
                    row.parent.map(TreeKey::Component)
                }
            },
        }
    }

    /// True when the row exists.
    pub fn has_tree_row(&self, key: TreeKey) -> bool {
        match key {
            TreeKey::Subassembly(id) => self.subassemblies().iter().any(|s| s.id == id),
            TreeKey::Component(occurrence) => self.has_occurrence(occurrence),
        }
    }

    /// True when `key` is `ancestor` or lies under it.
    pub fn is_under(&self, key: TreeKey, ancestor: TreeKey) -> bool {
        let mut at = Some(key);
        // A tree is never deeper than everything in it; the bound guards against a corrupt file.
        for _ in 0..=self.tree_size_bound() {
            match at {
                Some(k) if k == ancestor => return true,
                Some(k) => at = self.tree_parent(k),
                None => return false,
            }
        }
        false
    }

    fn tree_size_bound(&self) -> usize {
        let parts: usize = self.assemblies().iter().map(|(_, a)| a.parts.len()).sum();
        let links: usize = self.robots().iter().map(|(_, r)| r.links.len() + 1).sum();
        parts + links + self.subassemblies().len() + self.bodies().len() + self.robots().len() + 1
    }

    /// The browser's assembly tree, parents before children, each row with its indentation: per level the
    /// subassemblies (in the order made), then the components (in model order), each followed by its own
    /// rows (an assembly's parts, a robot's links and tool).
    pub fn component_tree(&self) -> Vec<TreeRow> {
        let subassemblies = self.subassemblies();
        let nodes = self.components();
        let mut rows = Vec::new();
        self.tree_level(None, 0, &subassemblies, &nodes, &mut rows);
        rows
    }

    fn tree_level(
        &self,
        level: Option<SubassemblyId>,
        depth: u32,
        subassemblies: &[Subassembly],
        nodes: &[ComponentNode],
        rows: &mut Vec<TreeRow>,
    ) {
        let parent = level.map(TreeKey::Subassembly);
        for s in subassemblies.iter().filter(|s| s.parent == level) {
            // A corrupt file could make a cycle; a subassembly already listed is not listed again.
            if rows.iter().any(|r| r.key == TreeKey::Subassembly(s.id)) {
                continue;
            }
            rows.push(TreeRow {
                key: TreeKey::Subassembly(s.id),
                parent,
                depth,
                name: s.name.clone(),
                component: None,
            });
            self.tree_level(Some(s.id), depth + 1, subassemblies, nodes, rows);
        }
        for node in nodes.iter().filter(|n| n.parent.is_none()) {
            let in_level = match self.subassembly_of(node.occurrence) {
                Some(s) => subassemblies.iter().any(|x| x.id == s).then_some(s) == level,
                None => level.is_none(),
            };
            if !in_level {
                continue;
            }
            rows.push(TreeRow {
                key: TreeKey::Component(node.occurrence),
                parent,
                depth,
                name: node.name.clone(),
                component: Some(node.clone()),
            });
            // Its own rows: an assembly's parts (in `nodes`), a robot's links and tool.
            let mut own: Vec<ComponentNode> = nodes
                .iter()
                .filter(|n| n.parent.is_some() && n.occurrence.owner() == node.occurrence.owner())
                .cloned()
                .collect();
            if let Occurrence::Item(item) = node.occurrence
                && item.kind == Kind::RobotInstance
            {
                own.extend(self.robot_links(gc_core::Id::from_raw(item.raw)));
            }
            let mut depths: Vec<(Occurrence, u32)> = vec![(node.occurrence, depth)];
            for n in own {
                let d = n
                    .parent
                    .and_then(|p| depths.iter().find(|(o, _)| *o == p).map(|(_, d)| d + 1))
                    .unwrap_or(depth + 1);
                depths.push((n.occurrence, d));
                rows.push(TreeRow {
                    key: TreeKey::Component(n.occurrence),
                    parent: n.parent.map(TreeKey::Component),
                    depth: d,
                    name: n.name.clone(),
                    component: Some(n),
                });
            }
        }
    }

    /// The level a tree row sits at, for the rows that can move between levels: a component placed in the
    /// cell or a subassembly.
    fn level_of(&self, key: TreeKey) -> Option<Option<SubassemblyId>> {
        match key {
            TreeKey::Subassembly(id) => self.subassemblies().iter().find(|s| s.id == id).map(|s| s.parent),
            TreeKey::Component(occurrence @ Occurrence::Item(item))
                if matches!(item.kind, Kind::Body | Kind::RobotInstance | Kind::Assembly)
                    && self.has_occurrence(occurrence) =>
            {
                Some(self.subassembly_of(occurrence))
            }
            TreeKey::Component(_) => None,
        }
    }
}

type Outcome = Result<(Vec<Created>, &'static str), EditError>;

pub(crate) fn run(txn: &mut Txn<'_>, op: &SubassemblyOp) -> Outcome {
    let doc = txn.doc();
    let all = doc.subassemblies();
    match op {
        SubassemblyOp::Demote { members, name } => {
            if members.is_empty() {
                return Err(EditError::BadValue);
            }
            let levels: Vec<Option<Option<SubassemblyId>>> =
                members.iter().map(|m| doc.level_of(*m)).collect();
            let Some(Some(level)) = levels.first().copied() else { return Err(EditError::BadValue) };
            if levels.iter().any(|l| *l != Some(level)) {
                return Err(EditError::BadValue);
            }
            let next = doc.timeline().map_or(0, |(_, t)| t.components.next_subassembly);
            let id = SubassemblyId(all.iter().map(|s| s.id.0 + 1).max().unwrap_or(1).max(next).max(1));
            let name = if name.trim().is_empty() {
                (1..)
                    .map(|n| format!("Subassembly:{n}"))
                    .find(|n| all.iter().all(|s| s.name != *n))
                    .unwrap_or_default()
            } else {
                name.trim().to_owned()
            };
            if all.iter().any(|s| s.name == name) {
                return Err(EditError::NameTaken);
            }
            let members = members.clone();
            crate::cell::set_timeline(txn, |t| {
                t.components.subassemblies.push(Subassembly { id, name, parent: level });
                t.components.next_subassembly = id.0 + 1;
                place(&mut t.components, &members, Some(id));
            })?;
            Ok((vec![Created::Subassembly(id)], "Demote"))
        }
        SubassemblyOp::Move { members, into } => {
            if members.is_empty() || into.is_some_and(|i| !all.iter().any(|s| s.id == i)) {
                return Err(EditError::BadValue);
            }
            for member in members {
                doc.level_of(*member).ok_or(EditError::BadValue)?;
                // Not into itself or one of its own.
                if let (TreeKey::Subassembly(_), Some(into)) = (member, into)
                    && doc.is_under(TreeKey::Subassembly(*into), *member)
                {
                    return Err(EditError::BadValue);
                }
            }
            let members = members.clone();
            let into = *into;
            crate::cell::set_timeline(txn, |t| place(&mut t.components, &members, into))?;
            Ok((Vec::new(), "Move to subassembly"))
        }
        SubassemblyOp::Dissolve(id) => {
            let gone = all.iter().find(|s| s.id == *id).ok_or(EditError::BadValue)?.clone();
            crate::cell::set_timeline(txn, |t| {
                let c = &mut t.components;
                c.subassemblies.retain(|s| s.id != *id);
                for s in &mut c.subassemblies {
                    if s.parent == Some(*id) {
                        s.parent = gone.parent;
                    }
                }
                match gone.parent {
                    Some(up) => {
                        for entry in &mut c.placed_in {
                            if entry.1 == *id {
                                entry.1 = up;
                            }
                        }
                    }
                    None => c.placed_in.retain(|(_, s)| s != id),
                }
            })?;
            Ok((Vec::new(), "Dissolve subassembly"))
        }
        SubassemblyOp::Rename { id, name } => {
            let name = name.trim().to_owned();
            if name.is_empty() || !all.iter().any(|s| s.id == *id) {
                return Err(EditError::BadValue);
            }
            if all.iter().any(|s| s.id != *id && s.name == name) {
                return Err(EditError::NameTaken);
            }
            crate::cell::set_timeline(txn, |t| {
                for s in &mut t.components.subassemblies {
                    if s.id == *id {
                        s.name.clone_from(&name);
                    }
                }
            })?;
            Ok((Vec::new(), "Rename"))
        }
    }
}

/// Puts members at a level (`None`: the top level).
fn place(c: &mut crate::design::Components, members: &[TreeKey], into: Option<SubassemblyId>) {
    for member in members {
        match member {
            TreeKey::Subassembly(id) => {
                for s in &mut c.subassemblies {
                    if s.id == *id {
                        s.parent = into;
                    }
                }
            }
            TreeKey::Component(occurrence) => {
                c.placed_in.retain(|(o, _)| o != occurrence);
                if let Some(into) = into {
                    c.placed_in.push((*occurrence, into));
                }
            }
        }
    }
}
