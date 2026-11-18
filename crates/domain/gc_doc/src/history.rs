//! Undo and redo.

use crate::document::{ChangeSet, DocDelta, Document};

struct Step {
    label: String,
    delta: DocDelta,
}

/// The undo and redo stacks of a document.
pub struct History {
    undo: Vec<Step>,
    redo: Vec<Step>,
    limit: usize,
    /// How many of the oldest steps were dropped to keep within the limit, ever.
    trimmed: u64,
}

impl Default for History {
    fn default() -> Self {
        Self { undo: Vec::new(), redo: Vec::new(), limit: 200, trimmed: 0 }
    }
}

impl History {
    /// An empty history that keeps the last 200 steps.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a committed change. Empty deltas are ignored. Recording clears the redo stack.
    pub fn record(&mut self, label: &str, delta: DocDelta) {
        if delta.is_empty() {
            return;
        }
        self.redo.clear();
        self.undo.push(Step { label: label.to_owned(), delta });
        if self.undo.len() > self.limit {
            self.undo.remove(0);
            self.trimmed += 1;
        }
    }

    /// Undoes the latest step. Returns its label and what changed.
    pub fn undo(&mut self, doc: &mut Document) -> Option<(String, ChangeSet)> {
        Self::shift(&mut self.undo, &mut self.redo, doc)
    }

    /// Redoes the step undone last. Returns its label and what changed.
    pub fn redo(&mut self, doc: &mut Document) -> Option<(String, ChangeSet)> {
        Self::shift(&mut self.redo, &mut self.undo, doc)
    }

    fn shift(from: &mut Vec<Step>, to: &mut Vec<Step>, doc: &mut Document) -> Option<(String, ChangeSet)> {
        let mut step = from.pop()?;
        let changed = step.delta.swap_all(doc); // the step now holds the images for the opposite direction
        doc.revision.bump();
        let label = step.label.clone();
        to.push(step);
        Some((label, changed))
    }

    /// Number of steps that can be undone.
    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }

    /// Number of steps that can be redone.
    pub fn redo_depth(&self) -> usize {
        self.redo.len()
    }

    /// The label of the step [`History::undo`] would undo.
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|s| s.label.as_str())
    }

    /// How many of the oldest steps have been dropped to keep within the limit so far (a mark taken as an
    /// undo depth moves down by as many).
    pub fn trimmed(&self) -> u64 {
        self.trimmed
    }

    /// Forgets the steps that could be redone (they were discarded, as by Cancel Sketch).
    pub fn drop_redo(&mut self) {
        self.redo.clear();
    }

    /// Forgets everything (after opening another document).
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}
