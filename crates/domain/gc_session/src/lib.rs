//! The application core without a user interface.
//!
//! A [`Session`] owns the document, its history, the active sketch, the current tool, the selection and
//! any drag in progress. The user interface feeds it events and draws what it reports; tests drive it the
//! same way, so whole editing scenarios run headless.
//!
//! Continuous interactions (dragging) never touch the document while they run: the preview lives in the
//! session, and one command is committed when the drag ends. That gives one undo step per drag and keeps
//! everything derived from the document from being invalidated sixty times a second.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::float_cmp, reason = "tests compare exactly representable values"))]

mod cell;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub use cell::{LoadedRobot, PADDING, PLANNERS, PlanStatus, PlannedPath};
use gc_core::{Id, Revision};
use gc_doc::{Applied, Command, Created, Document, EditError, History, Sketch, SketchOp, apply};
pub use gc_persist::PersistError;
use gc_sketch::{Curve, DimKind, Dimension, FrameOutcome, Item, SkPoint, SketchDrag, SketchReport};
use gc_tools::{Preview, Prompt, Tool, ToolEvent, ToolKind, ToolOutput};

/// A dimension that is about to be placed and needs its value from the user.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ValueRequest {
    /// What will be measured.
    pub kind: DimKind,
    /// The current value in SI units: the default shown in the box.
    pub measured: f64,
    /// Where the dimension text goes (the value box opens there).
    pub text_pos: [f64; 2],
}

/// A dimension that was refused as over-constraining and can be added as a reference dimension instead.
#[derive(Clone, Debug, PartialEq)]
pub struct ReferenceOffer {
    /// What it would measure.
    pub kind: DimKind,
    /// Where its text would go.
    pub text_pos: [f64; 2],
    /// The items that already determine or contradict it.
    pub with: Vec<Item>,
}

/// Everything the application does, minus the pixels.
pub struct Session {
    doc: Document,
    history: History,
    active: Option<Id<Sketch>>,
    tool: Tool,
    selection: BTreeSet<Item>,
    drag: Option<SketchDrag>,
    report: Option<SketchReport>,
    request: Option<ValueRequest>,
    offer: Option<ReferenceOffer>,
    last_error: Option<EditError>,
    path: Option<PathBuf>,
    saved_revision: Revision,
    cell: cell::CellState,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    /// A session with a new, empty document.
    pub fn new() -> Self {
        Self {
            doc: Document::new(),
            history: History::new(),
            active: None,
            tool: Tool::new(ToolKind::Select),
            selection: BTreeSet::new(),
            drag: None,
            report: None,
            request: None,
            offer: None,
            last_error: None,
            path: None,
            saved_revision: Revision::default(),
            cell: cell::CellState::default(),
        }
    }

    /// The document.
    pub fn doc(&self) -> &Document {
        &self.doc
    }

    /// The file the document was loaded from or last saved to.
    pub fn file_path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// True when the document has changes that are not in its file.
    pub fn is_dirty(&self) -> bool {
        self.doc.revision() != self.saved_revision
    }

    fn replace_document(&mut self, doc: Document, path: Option<PathBuf>) {
        self.saved_revision = doc.revision();
        self.doc = doc;
        self.path = path;
        self.history.clear();
        self.active = None;
        self.selection.clear();
        self.last_error = None;
        self.set_tool(ToolKind::Select);
        self.refresh();
    }

    /// Starts a new, empty document. Unsaved changes are discarded; ask the user first.
    pub fn new_document(&mut self) {
        self.replace_document(Document::new(), None);
    }

    /// Saves the document to `path` and remembers the path.
    pub fn save_as(&mut self, path: &Path) -> Result<(), PersistError> {
        gc_persist::save(&self.doc, path)?;
        self.path = Some(path.to_path_buf());
        self.saved_revision = self.doc.revision();
        Ok(())
    }

    /// Saves the document to its file. Returns `false` when it has none yet (use [`Session::save_as`]).
    pub fn save(&mut self) -> Result<bool, PersistError> {
        match self.path.clone() {
            Some(path) => self.save_as(&path).map(|()| true),
            None => Ok(false),
        }
    }

    /// Opens a document, replacing the current one. Returns the sketches whose stored coordinates violate
    /// their constraints; they are shown as stored.
    pub fn open(&mut self, path: &Path) -> Result<Vec<Id<Sketch>>, PersistError> {
        let loaded = gc_persist::load(path)?;
        self.replace_document(loaded.doc, Some(path.to_path_buf()));
        Ok(loaded.needs_repair)
    }

    /// The history, for the undo and redo menu entries.
    pub fn history(&self) -> &History {
        &self.history
    }

    /// The sketch being edited.
    pub fn active_sketch(&self) -> Option<(Id<Sketch>, &Sketch)> {
        self.active.and_then(|id| self.doc.get(id).map(|sketch| (id, sketch)))
    }

    /// The current tool.
    pub fn tool(&self) -> ToolKind {
        self.tool.kind()
    }

    /// What the status bar should say about the tool.
    pub fn prompt(&self) -> Prompt {
        self.tool.prompt()
    }

    /// What the tool draws on top of the sketch.
    pub fn preview(&self) -> Preview {
        self.tool.preview()
    }

    /// The selected items of the active sketch.
    pub fn selection(&self) -> &BTreeSet<Item> {
        &self.selection
    }

    /// The solve state of the active sketch: degrees of freedom, what is free, what is redundant.
    pub fn report(&self) -> Option<&SketchReport> {
        self.report.as_ref()
    }

    /// The value box that should be open, if any.
    pub fn value_request(&self) -> Option<&ValueRequest> {
        self.request.as_ref()
    }

    /// The "add as reference dimension instead?" question that should be shown, if any.
    pub fn reference_offer(&self) -> Option<&ReferenceOffer> {
        self.offer.as_ref()
    }

    /// Why the latest edit was refused; cleared by the next successful edit.
    pub fn last_error(&self) -> Option<&EditError> {
        self.last_error.as_ref()
    }

    /// True while a drag is in progress.
    pub fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// Where a point of the active sketch is right now: the drag preview if one is running, the document
    /// otherwise. Everything that draws the sketch reads positions through this.
    pub fn point_pos(&self, point: Id<SkPoint>) -> Option<[f64; 3]> {
        self.drag.as_ref().and_then(|d| d.pos(point)).or_else(|| self.active_sketch()?.1.pos(point))
    }

    /// The radius of a circle or arc right now (see [`Session::point_pos`]).
    pub fn radius(&self, curve: Id<Curve>) -> Option<f64> {
        self.drag
            .as_ref()
            .and_then(|d| d.radius(curve))
            .or_else(|| Some(self.active_sketch()?.1.round(curve)?.1))
    }

    fn refresh(&mut self) {
        self.cell.sync(&self.doc);
        // Solving a consistent sketch changes nothing; it is run on a copy only to obtain the report.
        self.report = self.active_sketch().and_then(|(_, sketch)| sketch.clone().solve().ok());
        if let Some((_, sketch)) = self.active.and_then(|id| self.doc.get(id).map(|s| (id, s))) {
            let exists = |item: &Item| match item {
                Item::Point(p) => sketch.points.contains_key(p),
                Item::Curve(c) => sketch.curves.contains_key(c),
                Item::Constraint(c) => sketch.constraints.contains_key(c),
                Item::Dimension(d) => sketch.dimensions.contains_key(d),
            };
            self.selection.retain(exists);
        } else {
            self.selection.clear();
        }
    }

    /// Applies a command and records it for undo. On failure the document is unchanged and the error is
    /// also kept for the status bar.
    pub fn execute(&mut self, command: &Command) -> Result<Vec<Created>, EditError> {
        match apply(&mut self.doc, command) {
            Ok(Applied { delta, created, label, .. }) => {
                self.history.record(label, delta);
                self.last_error = None;
                self.refresh();
                Ok(created)
            }
            Err(error) => {
                self.last_error = Some(error.clone());
                Err(error)
            }
        }
    }

    fn reset_interaction(&mut self) {
        self.drag = None;
        self.request = None;
        self.offer = None;
        self.tool = Tool::new(self.tool.kind());
    }

    /// Undoes the latest step. Ignored while dragging.
    pub fn undo(&mut self) -> bool {
        if self.drag.is_some() {
            return false;
        }
        let done = self.history.undo(&mut self.doc).is_some();
        self.after_history();
        done
    }

    /// Redoes the step undone last. Ignored while dragging.
    pub fn redo(&mut self) -> bool {
        if self.drag.is_some() {
            return false;
        }
        let done = self.history.redo(&mut self.doc).is_some();
        self.after_history();
        done
    }

    fn after_history(&mut self) {
        if self.active.is_some_and(|id| self.doc.get(id).is_none()) {
            self.active = None; // the sketch itself was undone
        }
        self.reset_interaction();
        self.refresh();
    }

    /// Creates a sketch on an origin plane (`"XY"`, `"XZ"` or `"YZ"`) and starts editing it.
    pub fn create_sketch(&mut self, plane: &str) -> Result<Id<Sketch>, EditError> {
        let frame = self.doc.origin_plane(plane).ok_or(gc_sketch::SketchError::MissingItem)?;
        let name = format!("Sketch{}", self.doc.sketches().len() + 1);
        let created = self.execute(&Command::CreateSketch { name, frame, spatial: false })?;
        match created.first() {
            Some(Created::Sketch(id)) => {
                self.enter_sketch(*id);
                Ok(*id)
            }
            _ => Err(gc_sketch::SketchError::MissingItem.into()),
        }
    }

    /// Starts editing an existing sketch.
    pub fn enter_sketch(&mut self, sketch: Id<Sketch>) {
        if self.doc.get(sketch).is_some() {
            self.active = Some(sketch);
            self.selection.clear();
            self.set_tool(ToolKind::Select);
            self.refresh();
        }
    }

    /// Stops editing the active sketch.
    pub fn finish_sketch(&mut self) {
        self.active = None;
        self.selection.clear();
        self.set_tool(ToolKind::Select);
        self.refresh();
    }

    /// Switches the tool.
    pub fn set_tool(&mut self, kind: ToolKind) {
        self.tool = Tool::new(kind);
        self.reset_interaction();
    }

    fn sketch_op(&mut self, op: SketchOp) -> Result<Vec<Created>, EditError> {
        let Some(sketch) = self.active else { return Ok(Vec::new()) };
        self.execute(&Command::Sketch { sketch, op })
    }

    /// Feeds an input event to the current tool and carries out what the tool asks for.
    pub fn tool_event(&mut self, event: ToolEvent) {
        let Some(sketch) = self.active.and_then(|id| self.doc.get(id)) else { return };
        let outputs = self.tool.handle(sketch, event);
        for output in outputs {
            self.carry_out(output);
        }
    }

    fn carry_out(&mut self, output: ToolOutput) {
        match output {
            ToolOutput::Edit(op) => {
                let placing = match &op {
                    SketchOp::AddDimension { kind, text_pos, .. } => Some((*kind, *text_pos)),
                    _ => None,
                };
                self.request = None;
                let result = self.sketch_op(op);
                // An over-constraining dimension is not simply refused: it can become a reference dimension.
                if let (
                    Some((kind, text_pos)),
                    Err(EditError::Redundant(with) | EditError::Conflicting(with)),
                ) = (placing, result)
                {
                    self.offer = Some(ReferenceOffer { kind, text_pos, with });
                }
            }
            ToolOutput::Select { item, extend } => match (item, extend) {
                (Some(item), true) => {
                    if !self.selection.remove(&item) {
                        self.selection.insert(item);
                    }
                }
                (Some(item), false) => self.selection = BTreeSet::from([item]),
                (None, true) => {}
                (None, false) => self.selection.clear(),
            },
            ToolOutput::BeginDrag { grab, cursor } => {
                self.drag = self.active_sketch().and_then(|(_, sketch)| sketch.begin_drag(grab, cursor).ok());
            }
            ToolOutput::DragTo(cursor) => {
                if let Some(drag) = &mut self.drag {
                    // A refused frame keeps the previous preview; nothing else to do.
                    let _outcome: FrameOutcome = drag.update(cursor);
                }
            }
            ToolOutput::EndDrag => self.end_drag(),
            ToolOutput::CancelDrag => self.drag = None,
            ToolOutput::DeleteSelection => {
                if !self.selection.is_empty() {
                    let items = self.selection.iter().copied().collect();
                    // A failed delete is reported through `last_error`.
                    drop(self.sketch_op(SketchOp::Delete(items)));
                }
            }
            ToolOutput::RequestValue { kind, measured, text_pos } => {
                self.request = Some(ValueRequest { kind, measured, text_pos });
            }
            ToolOutput::Finish => self.set_tool(ToolKind::Select),
        }
    }

    /// Commits the drag as one command, or as nothing at all when the geometry did not move.
    fn end_drag(&mut self) {
        let (Some(drag), Some((_, sketch))) = (self.drag.take(), self.active_sketch()) else { return };
        let mut moved = sketch.clone();
        if !drag.apply(&mut moved) {
            return;
        }
        let points = moved
            .points
            .iter()
            .filter(|(id, p)| {
                sketch.pos(**id).is_some_and(|old| old.map(f64::to_bits) != p.pos.map(f64::to_bits))
            })
            .map(|(id, p)| (*id, p.pos))
            .collect();
        let radii = moved
            .curves
            .keys()
            .filter_map(|id| match (moved.round(*id), sketch.round(*id)) {
                (Some((_, new)), Some((_, old))) if new.to_bits() != old.to_bits() => Some((*id, new)),
                _ => None,
            })
            .collect();
        // A failed commit is reported through `last_error`; the sketch stays as it was.
        drop(self.sketch_op(SketchOp::SetCoords { points, radii }));
    }

    /// Confirms the open value box with the text the user typed.
    pub fn submit_value(&mut self, source: &str) {
        self.tool_event(ToolEvent::Value(source.to_owned()));
    }

    /// Dismisses the open value box.
    pub fn cancel_value(&mut self) {
        self.request = None;
        self.tool_event(ToolEvent::CancelValue);
    }

    /// Answers the reference-dimension question. Accepting adds the reference dimension.
    pub fn answer_reference_offer(&mut self, accept: bool) {
        if let (Some(offer), true) = (self.offer.take(), accept) {
            drop(
                self.sketch_op(SketchOp::AddReferenceDimension {
                    kind: offer.kind,
                    text_pos: offer.text_pos,
                }),
            );
        }
    }

    /// Changes the expression of an existing dimension.
    pub fn edit_dimension(&mut self, id: Id<Dimension>, source: &str) -> Result<(), EditError> {
        self.sketch_op(SketchOp::SetDimension { id, source: source.to_owned() }).map(|_| ())
    }
}
