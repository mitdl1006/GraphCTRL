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

/// The edit target and the targets before it.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct EditStack {
    target: Option<TreeKey>,
    back: Vec<Option<TreeKey>>,
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
