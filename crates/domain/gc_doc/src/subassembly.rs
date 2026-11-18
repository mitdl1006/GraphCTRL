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
    /// Flexible (Inventor 2026 help, Create and edit flexible components,
    /// GUID-30817F08-A5C2-44B7-B237-4791ACA3F854): the freedom its components have among themselves is
    /// available in the parent. Otherwise (the default) it moves in the parent as one rigid unit.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub flexible: bool,
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
    /// Paste (Ctrl+V): new instances of components and of whole subassemblies (their structure kept, new
    /// subassemblies with free names), in the same orientation, moved by `offset`, without relationships
    /// (Inventor 2026 help, About Placing Components, GUID-249C50F9-5A8C-48BD-8341-D1BDB7A16B19: "Use Copy
    /// and Paste to place new instances using the same orientation"). They go into `into` (`None`: the top
    /// level). Rows that no longer exist are skipped; nothing left to paste is refused.
    Paste {
        /// What was copied: components placed in the cell and subassemblies.
        keys: Vec<TreeKey>,
        /// How far the new instances are moved.
        offset: crate::cell::Placement,
        /// Where they go.
        into: Option<SubassemblyId>,
    },
    /// Flexible on or off.
    SetFlexible {
        /// Which.
        id: SubassemblyId,
        /// On.
        flexible: bool,
    },
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

    /// The owner of a sketch: the component or subassembly it was made in; `None` for the top-level
    /// assembly (or an owner that no longer exists).
    pub fn sketch_owner(&self, sketch: gc_core::Id<gc_sketch::Sketch>) -> Option<TreeKey> {
        let (_, t) = self.timeline()?;
        t.components
            .sketch_owners
            .iter()
            .find(|(s, _)| *s == sketch)
            .map(|(_, o)| *o)
            .filter(|o| self.has_tree_row(*o))
    }

    /// The sketches an owner holds (`None`: the top-level assembly's), in model order.
    pub fn sketches_of(&self, owner: Option<TreeKey>) -> Vec<gc_core::Id<gc_sketch::Sketch>> {
        self.sketches().iter().map(|(id, _)| id).filter(|id| self.sketch_owner(*id) == owner).collect()
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

    /// The components placed in the cell (bodies, robots, imported assemblies) that sit in a subassembly,
    /// at any depth below it, in model order.
    pub fn subassembly_components(&self, id: SubassemblyId) -> Vec<gc_core::AnyId> {
        self.components()
            .into_iter()
            .filter(|n| n.parent.is_none())
            .filter_map(|n| match n.occurrence {
                Occurrence::Item(item) => Some(item),
                Occurrence::Part { .. } | Occurrence::Link { .. } | Occurrence::Tool { .. } => None,
            })
            .filter(|item| {
                self.subassembly_of(Occurrence::Item(*item))
                    .is_some_and(|s| self.is_under(TreeKey::Subassembly(s), TreeKey::Subassembly(id)))
            })
            .collect()
    }

    /// The rigid unit a component placed in the cell moves with: the outermost subassembly around it that
    /// is not flexible (a rigid subassembly keeps everything in it rigid in its parent). `None` when every
    /// subassembly around it is flexible, or it is at the top level.
    pub fn rigid_unit(&self, item: gc_core::AnyId) -> Option<SubassemblyId> {
        let all = self.subassemblies();
        let mut at = self.subassembly_of(Occurrence::Item(item));
        let mut unit = None;
        for _ in 0..=all.len() {
            let Some(id) = at else { break };
            let Some(s) = all.iter().find(|s| s.id == id) else { break };
            if !s.flexible {
                unit = Some(id);
            }
            at = s.parent;
        }
        unit
    }

    /// The component that stands for a component in the relationship solver: the first component of its
    /// rigid unit, or itself.
    pub(crate) fn solver_owner(&self, item: gc_core::AnyId) -> gc_core::AnyId {
        self.rigid_unit(item).and_then(|u| self.subassembly_components(u).first().copied()).unwrap_or(item)
    }

    /// The components that move with `item` as one rigid unit (itself alone when it is in none).
    pub fn rigid_unit_members(&self, item: gc_core::AnyId) -> Vec<gc_core::AnyId> {
        match self.rigid_unit(item) {
            Some(u) => self.subassembly_components(u),
            None => vec![item],
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

/// Deletes the sketches a component placed in the cell owns (made while editing it or one of its parts in
/// place), in the transaction that deletes the component: Inventor deletes a part's sketches with the part.
/// Refused (`InUse`) when anything else uses one of them (a feature, a work feature, a path).
pub(crate) fn delete_owned_sketches(txn: &mut Txn<'_>, item: gc_core::AnyId) -> Result<(), EditError> {
    let doc = txn.doc();
    let owned: Vec<gc_core::Id<gc_sketch::Sketch>> = doc
        .timeline()
        .map(|(_, t)| {
            t.components
                .sketch_owners
                .iter()
                .filter(|(s, o)| {
                    doc.get(*s).is_some()
                        && matches!(o, TreeKey::Component(occurrence) if occurrence.owner() == item)
                })
                .map(|(s, _)| *s)
                .collect()
        })
        .unwrap_or_default();
    for sketch in &owned {
        if doc.dependents_of(sketch.erase()).iter().any(|d| !owned.iter().any(|s| s.erase() == *d)) {
            return Err(EditError::InUse);
        }
    }
    for sketch in owned {
        crate::command::run(txn, &crate::Command::DeleteSketch { id: sketch })?;
    }
    Ok(())
}

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
                t.components.subassemblies.push(Subassembly { id, name, parent: level, flexible: false });
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
        SubassemblyOp::Paste { keys, offset, into } => paste(txn, keys, offset, *into),
        SubassemblyOp::SetFlexible { id, flexible } => {
            if !all.iter().any(|s| s.id == *id) {
                return Err(EditError::BadValue);
            }
            crate::cell::set_timeline(txn, |t| {
                for s in &mut t.components.subassemblies {
                    if s.id == *id {
                        s.flexible = *flexible;
                    }
                }
            })?;
            crate::cell::settle(txn)?;
            Ok((Vec::new(), if *flexible { "Flexible" } else { "Rigid" }))
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

impl Document {
    /// The components a Paste of `keys` copies: each copied component, and every component in each
    /// copied subassembly (nested levels too). Rows that no longer exist are skipped. For the paste
    /// preview.
    pub fn pasted_components(&self, keys: &[TreeKey]) -> Vec<gc_core::AnyId> {
        self.pasted(keys).0
    }

    fn pasted(&self, keys: &[TreeKey]) -> (Vec<gc_core::AnyId>, Vec<Subassembly>) {
        let doc = self;
        let all = doc.subassemblies();
        // What is pasted: the components, and each copied subassembly with everything below it.
        let mut items: Vec<gc_core::AnyId> = Vec::new();
        let mut groups: Vec<Subassembly> = Vec::new();
        for key in keys {
            match *key {
                TreeKey::Component(Occurrence::Item(item)) if doc.level_of(*key).is_some() => {
                    if !items.contains(&item) {
                        items.push(item);
                    }
                }
                TreeKey::Subassembly(id) if all.iter().any(|s| s.id == id) => {
                    for s in all
                        .iter()
                        .filter(|s| doc.is_under(TreeKey::Subassembly(s.id), TreeKey::Subassembly(id)))
                    {
                        if !groups.iter().any(|g| g.id == s.id) {
                            groups.push(s.clone());
                        }
                    }
                    for item in doc.subassembly_components(id) {
                        if !items.contains(&item) {
                            items.push(item);
                        }
                    }
                }
                TreeKey::Component(_) | TreeKey::Subassembly(_) => {}
            }
        }
        (items, groups)
    }
}

fn paste(
    txn: &mut Txn<'_>,
    keys: &[TreeKey],
    offset: &crate::cell::Placement,
    into: Option<SubassemblyId>,
) -> Outcome {
    let doc = txn.doc();
    let all = doc.subassemblies();
    if into.is_some_and(|i| !all.iter().any(|s| s.id == i)) {
        return Err(EditError::BadValue);
    }
    let (items, groups) = doc.pasted(keys);
    if items.is_empty() && groups.is_empty() {
        return Err(EditError::BadValue);
    }
    // Where each original sat, to rebuild the structure.
    let level: Vec<(gc_core::AnyId, Option<SubassemblyId>)> =
        items.iter().map(|i| (*i, doc.subassembly_of(Occurrence::Item(*i)))).collect();
    let (mut created, pairs) = crate::replicate::copy_mapped(txn, &items, offset)?;
    // New ids and free names for the copied subassemblies.
    let mut next = txn.doc().timeline().map_or(0, |(_, t)| t.components.next_subassembly);
    let mut taken: Vec<String> = all.iter().map(|s| s.name.clone()).collect();
    let mut renumbered: Vec<(SubassemblyId, SubassemblyId)> = Vec::new();
    let mut made: Vec<Subassembly> = Vec::new();
    for g in &groups {
        let id =
            SubassemblyId(all.iter().chain(&made).map(|s| s.id.0 + 1).max().unwrap_or(1).max(next).max(1));
        next = id.0 + 1;
        let stem = g.name.rsplit_once(':').map_or(g.name.as_str(), |(stem, n)| {
            if n.parse::<u32>().is_ok() { stem } else { g.name.as_str() }
        });
        let name = (1..).map(|n| format!("{stem}:{n}")).find(|n| !taken.contains(n)).unwrap_or_default();
        taken.push(name.clone());
        renumbered.push((g.id, id));
        made.push(Subassembly { id, name, parent: g.parent, flexible: g.flexible });
    }
    let new_id = |old: Option<SubassemblyId>| {
        old.and_then(|o| renumbered.iter().find(|(a, _)| *a == o).map(|(_, b)| *b))
    };
    for s in &mut made {
        // A copied subassembly under another copied one stays under that copy; the topmost go into `into`.
        s.parent = new_id(s.parent).or(into);
    }
    let placed: Vec<(Occurrence, SubassemblyId)> = pairs
        .iter()
        .filter_map(|(source, copy)| {
            let was = level.iter().find(|(i, _)| i == source).and_then(|(_, l)| *l);
            new_id(was).or(into).map(|s| (Occurrence::Item(*copy), s))
        })
        .collect();
    crate::cell::set_timeline(txn, |t| {
        t.components.next_subassembly = next;
        t.components.subassemblies.extend(made.iter().cloned());
        t.components.placed_in.extend(placed.iter().copied());
    })?;
    created.extend(made.iter().map(|s| Created::Subassembly(s.id)));
    Ok((created, "Paste"))
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
