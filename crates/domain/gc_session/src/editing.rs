//! Editing a component in place and the subassembly levels of the browser (Inventor 2026 help, Edit
//! components in place, GUID-C79C36C9-A6DB-46C5-8864-41F66C750231; Create Subassemblies in Place,
//! GUID-FCEE3F0E-AAE2-4619-8645-2588FEECD353).
//!
//! Double-clicking a component (or subassembly) in the browser or the graphics window makes it the edit
//! target; everything outside it is shown faded. Return goes back to the previous target, Return to
//! Parent to the target's parent, Return to Top to the top-level assembly. While a subassembly (or the
//! top level) is active, its subassemblies are picked as one unit.
//!
//! The edit target is a view state of the session: it is not saved and not undone. A target that stops
//! existing (deleted, undone) counts as none.

use gc_doc::{EditError, Occurrence, SubassemblyOp, TreeKey, TreeRow};

use crate::Session;

/// The edit target and the targets before it, and the component clipboard.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct EditStack {
    target: Option<TreeKey>,
    back: Vec<Option<TreeKey>>,
    clipboard: Vec<TreeKey>,
}

impl Session {
    /// The browser's assembly tree with indentation (subassemblies, components, their parts, links and
    /// tools); see [`gc_doc::Document::component_tree`].
    pub fn component_tree(&self) -> Vec<TreeRow> {
        self.doc().component_tree()
    }

    /// Changes subassemblies: Demote, move between levels, dissolve, rename. Each is one undo step.
    pub fn edit_subassemblies(&mut self, op: SubassemblyOp) -> Result<Vec<gc_doc::Created>, EditError> {
        self.execute(&gc_doc::Command::Cell(gc_doc::CellOp::Subassembly(op)))
    }

    /// Runs a command that creates sketches; while a component or subassembly is edited in place the new
    /// sketches belong to it (Sketches in assemblies, GUID-2C180C3A-2064-4671-B1C2-F74B383B5D39).
    pub(crate) fn execute_owned(
        &mut self,
        command: gc_doc::Command,
    ) -> Result<Vec<gc_doc::Created>, EditError> {
        match self.edit_target() {
            Some(owner) => {
                self.execute(&gc_doc::Command::Owned { owner: Some(owner), command: Box::new(command) })
            }
            None => self.execute(&command),
        }
    }

    /// The owner of a sketch: the component or subassembly it was made in, `None` for the top-level
    /// assembly (whose sketches Inventor lists under the top-level Origin folder).
    pub fn sketch_owner(&self, sketch: gc_core::Id<gc_doc::Sketch>) -> Option<TreeKey> {
        self.doc().sketch_owner(sketch)
    }

    /// The sketches an owner holds (`None`: the top-level assembly's).
    pub fn sketches_of(&self, owner: Option<TreeKey>) -> Vec<gc_core::Id<gc_doc::Sketch>> {
        self.doc().sketches_of(owner)
    }

    /// Copy (Ctrl+C): remembers components placed in the cell and subassemblies to paste. Rows that are not
    /// components or subassemblies (parts, links, missing rows) are left out; refused when none is left.
    pub fn copy_to_clipboard(&mut self, keys: &[TreeKey]) -> Result<(), EditError> {
        let doc = self.doc();
        let usable: Vec<TreeKey> = keys
            .iter()
            .copied()
            .filter(|k| match k {
                TreeKey::Subassembly(_) => doc.has_tree_row(*k),
                TreeKey::Component(Occurrence::Item(item)) => {
                    matches!(
                        item.kind,
                        gc_core::Kind::Body | gc_core::Kind::RobotInstance | gc_core::Kind::Assembly
                    ) && doc.has_tree_row(*k)
                }
                TreeKey::Component(_) => false,
            })
            .collect();
        if usable.is_empty() {
            return Err(EditError::BadValue);
        }
        self.editing.clipboard = usable;
        Ok(())
    }

    /// What Copy remembered (some rows may since have been deleted; Paste skips them).
    pub fn clipboard(&self) -> &[TreeKey] {
        &self.editing.clipboard
    }

    /// Paste (Ctrl+V): new instances of what was copied, same orientation, moved by `offset`, without
    /// relationships; whole subassemblies keep their structure. They go into the subassembly being edited
    /// in place (the top level otherwise). One undo step. Refused when nothing copied is left.
    pub fn paste(&mut self, offset: gc_doc::Placement) -> Result<Vec<gc_doc::Created>, EditError> {
        let into = match self.edit_target() {
            Some(TreeKey::Subassembly(id)) => Some(id),
            Some(TreeKey::Component(_)) | None => None,
        };
        let keys = self.editing.clipboard.clone();
        self.edit_subassemblies(SubassemblyOp::Paste { keys, offset, into })
    }

    /// The component or subassembly being edited in place; `None` at the top level.
    pub fn edit_target(&self) -> Option<TreeKey> {
        self.editing.target.filter(|t| self.doc().has_tree_row(*t))
    }

    /// Double-click: edits `key` in place. Refused for a row that does not exist.
    pub fn edit_component(&mut self, key: TreeKey) -> Result<(), EditError> {
        if !self.doc().has_tree_row(key) {
            return Err(EditError::BadValue);
        }
        let now = self.edit_target();
        if now != Some(key) {
            self.editing.back.push(now);
            self.editing.target = Some(key);
        }
        Ok(())
    }

    /// Return: back to the previous edit target (skipping any that no longer exist).
    pub fn return_edit(&mut self) {
        while let Some(previous) = self.editing.back.pop() {
            if previous.is_none_or(|p| self.doc().has_tree_row(p)) {
                self.editing.target = previous;
                return;
            }
        }
        self.editing.target = None;
    }

    /// Return to Parent: edits the target's parent in the browser (the top level above a top-level row).
    pub fn return_to_parent(&mut self) {
        let Some(target) = self.edit_target() else { return };
        let parent = self.doc().tree_parent(target);
        self.editing.back.push(Some(target));
        self.editing.target = parent;
    }

    /// Return to Top: back to the top-level assembly.
    pub fn return_to_top(&mut self) {
        if let Some(target) = self.edit_target() {
            self.editing.back.push(Some(target));
        }
        self.editing.target = None;
    }

    /// True when a component is inside the edit target (shown normally); false when it is outside (shown
    /// faded, as Inventor shows the other components transparent). Everything is inside at the top level.
    pub fn in_edit_scope(&self, occurrence: Occurrence) -> bool {
        match self.edit_target() {
            None => true,
            Some(target) => self.doc().is_under(TreeKey::Component(occurrence), target),
        }
    }

    /// Where a subassembly's components go when it is moved as one unit by `delta` (a move in the world,
    /// applied after where each stands now): a subassembly is treated as a single unit in its parent
    /// (Create Subassemblies in Place, GUID-FCEE3F0E-AAE2-4619-8645-2588FEECD353). `None` for a missing or
    /// empty subassembly, or when one of its components is grounded (the unit cannot move).
    pub fn subassembly_moved(
        &self,
        id: gc_doc::SubassemblyId,
        delta: gc_math::DPose3,
    ) -> Option<Vec<(gc_core::AnyId, gc_math::DPose3)>> {
        let doc = self.doc();
        let members = doc.subassembly_components(id);
        if members.is_empty() || !(delta.translation.is_finite() && delta.rotation.is_finite()) {
            return None;
        }
        members
            .into_iter()
            .map(|item| {
                if doc.component_flag(Occurrence::Item(item), gc_doc::ComponentFlag::Grounded) {
                    return None;
                }
                let now = doc.component_origin(Occurrence::Item(item))?.pose;
                Some((item, delta * now))
            })
            .collect()
    }

    /// Moves a subassembly as one unit by `delta` (see [`Session::subassembly_moved`]); what is tied to its
    /// components follows. One undo step. Refused for a missing or empty subassembly or a grounded member.
    pub fn move_subassembly(
        &mut self,
        id: gc_doc::SubassemblyId,
        delta: gc_math::DPose3,
    ) -> Result<(), EditError> {
        let moves = self.subassembly_moved(id, delta).ok_or(EditError::BadValue)?;
        self.grip_snap_commit(&moves, false)
    }

    /// What a click on a component picks: the row directly inside the edit target (or at the top level)
    /// that holds it, so that a subassembly is picked as one unit. A component of an imported assembly or
    /// a robot link is picked itself. `None` for a component outside the edit target.
    pub fn pick_unit(&self, occurrence: Occurrence) -> Option<TreeKey> {
        let target = self.edit_target();
        let doc = self.doc();
        let mut at = TreeKey::Component(occurrence);
        if !doc.has_tree_row(at) {
            return None;
        }
        if Some(at) == target {
            return Some(at);
        }
        // Up through the subassemblies made in the cell, stopping just under the edit target.
        let mut unit = at;
        for _ in 0..1024 {
            let parent = doc.tree_parent(at);
            if parent == target {
                return Some(unit);
            }
            let parent = parent?;
            if matches!(parent, TreeKey::Subassembly(_)) {
                unit = parent;
            }
            at = parent;
        }
        None
    }
}
