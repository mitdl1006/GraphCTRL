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

mod assembly;
mod autoproject;
mod bom;
mod cell;
mod design;
mod editing;
mod grip;
pub mod library;
mod look;
mod measure;
mod physical;
mod project;
mod role;
mod schedule;
mod sketch_pick;
mod spatial;
mod urscript;
mod work;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub use assembly::{AssemblyReader, Imported, Importer, ShownAssemblyPart};
pub use bom::{BomRow, BomView, structure_name};
pub use cell::{
    CellItem, CompareRow, ContactStop, Ghost, LoadedRobot, MeshRef, PADDING, PLANNERS, PlanStatus,
    PlannedPath, ProgramStep, Reference, Resources, Shown, ShownPart, TaskCandidate, TaskPlan,
};
pub use design::WorkShape;
pub use gc_collide::{Closest, Interference};
use gc_core::{Id, Revision};
use gc_doc::{Applied, Command, Created, Document, EditError, History, Sketch, SketchOp, apply};
pub use gc_motion::{Guarantee, LinkPair, PairReason, StateProblem};
pub use gc_persist::PersistError;
use gc_sketch::{Curve, DimKind, Dimension, FrameOutcome, Item, SkPoint, SketchDrag, SketchReport};
use gc_tools::{Preview, Prompt, Tool, ToolEvent, ToolKind, ToolOutput};
pub use look::{LookItem, LookTarget};
pub use measure::{
    Feature, FeaturePair, InterferenceOptions, InterferenceRow, ItemMeasure, PairMeasure, angle_at,
    measure_features,
};
pub use physical::{InertialProperties, MassMoments, PhysicalProperties};
pub use schedule::{Assignment, Detour, MAX_ASSIGNMENTS, ParkMove, Schedule, Wait};
pub use sketch_pick::SketchPick;

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
    /// The Construction toggle: drawing tools make construction geometry.
    construction: bool,
    selection: BTreeSet<Item>,
    /// The selected items in the order they were picked (may hold items no longer selected).
    picked: Vec<Item>,
    /// The active selector of an open sketch dialog.
    pick_target: Option<SketchPick>,
    /// Where the history stood when the sketch being edited was entered (or before it was created), with
    /// the history's trim count then: what Cancel Sketch goes back to.
    sketch_mark: Option<(usize, u64)>,
    /// The Move and Rotate dialogs' Copy box, and their More options (relax: Always; otherwise Never).
    transform_copy: bool,
    transform_relax: bool,
    /// Autoproject edges for sketch creation (a sketch on a face takes the face's outline).
    autoproject_edges: bool,
    /// What the last Automatic Dimensions and Constraints added (for its Remove).
    auto_added: Vec<Item>,
    drag: Option<SketchDrag>,
    report: Option<SketchReport>,
    request: Option<ValueRequest>,
    offer: Option<ReferenceOffer>,
    last_error: Option<EditError>,
    path: Option<PathBuf>,
    /// Where files made for a document without a file are kept (`None`: the application's cache).
    asset_cache: Option<PathBuf>,
    saved_revision: Revision,
    /// True for work recovered from an automatic save until it is saved.
    recovered: bool,
    cell: cell::CellState,
    spatial: spatial::SpatialState,
    /// The component being edited in place and the ones before it.
    editing: editing::EditStack,
    /// What the last opened file had that could not be used and was dropped.
    load_warnings: Vec<String>,
    /// Snap to grid (an application option): free cursor positions go to the document's snap spacing.
    snap_to_grid: bool,
}

/// The browser's Representations folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepresentationFolder {
    /// View representations.
    pub views: Vec<RepresentationNode>,
    /// Positional representations, [Primary] first.
    pub positions: Vec<RepresentationNode>,
}

/// One representation in the folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepresentationNode {
    /// Its name.
    pub name: String,
    /// Shown checked: the active one.
    pub active: bool,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

/// A component's status in the Mirror Components dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MirrorState {
    /// A true mirror image (a reflected mesh for a mesh body).
    Mirror,
    /// Another instance of the same shape, turned into place.
    Reuse,
}

/// What [`Session::mirror_components_mixed`] did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MirrorOutcome {
    /// Names of the relationships made suppressed (their orientation was ambiguous or they could not be
    /// satisfied).
    pub suppressed: Vec<String>,
    /// Components placed as the same shape turned round instead of a mirror image.
    pub not_reflected: Vec<gc_core::AnyId>,
}

impl Session {
    /// A session with a new, empty document.
    pub fn new() -> Self {
        Self {
            doc: Document::new(),
            history: History::new(),
            active: None,
            tool: Tool::new(ToolKind::Select),
            construction: false,
            selection: BTreeSet::new(),
            picked: Vec::new(),
            pick_target: None,
            sketch_mark: None,
            transform_copy: false,
            transform_relax: false,
            autoproject_edges: true,
            auto_added: Vec::new(),
            drag: None,
            report: None,
            request: None,
            offer: None,
            last_error: None,
            path: None,
            asset_cache: None,
            saved_revision: Revision::default(),
            recovered: false,
            cell: cell::CellState::default(),
            spatial: spatial::SpatialState::default(),
            editing: editing::EditStack::default(),
            load_warnings: Vec::new(),
            snap_to_grid: false,
        }
    }

    /// What the last opened project had that could not be used and was dropped (a damaged joint range,
    /// ...); the rest opened. For the status bar.
    pub fn load_warnings(&self) -> &[String] {
        &self.load_warnings
    }

    /// Isolate (Inventor 2026 help, GUID-36AE9264-912D-4D3C-BB24-7EC16FA8777E): hides every visible
    /// component except `keep`, what they are inside and what is inside them. One undo step; saved.
    pub fn isolate(&mut self, keep: &[gc_doc::Occurrence]) -> Result<(), EditError> {
        self.execute(&Command::Cell(gc_doc::CellOp::Isolate { keep: keep.to_vec() })).map(|_| ())
    }

    /// Undo Isolate: shows again what [`Session::isolate`] hid, keeping what was done meanwhile.
    pub fn undo_isolate(&mut self) -> Result<(), EditError> {
        self.execute(&Command::Cell(gc_doc::CellOp::UndoIsolate)).map(|_| ())
    }

    /// True while components hidden by [`Session::isolate`] have not been shown again (Undo Isolate is
    /// offered then).
    pub fn is_isolated(&self) -> bool {
        self.doc.timeline().is_some_and(|(_, t)| !t.components.isolated.is_empty())
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
        self.recovered || self.doc.revision() != self.saved_revision
    }

    fn replace_document(&mut self, doc: Document, path: Option<PathBuf>) {
        self.recovered = false;
        // What an earlier open dropped belongs to that document.
        self.load_warnings.clear();
        self.saved_revision = doc.revision();
        self.doc = doc;
        self.path = path;
        self.history.clear();
        self.active = None;
        self.sketch_mark = None;
        self.selection.clear();
        self.last_error = None;
        self.set_tool(ToolKind::Select);
        self.refresh();
        self.fill_link_trees();
        // Another document: its revisions count from its own start.
        *self.cell.statuses.borrow_mut() = None;
    }

    /// Gives robots of an older project their link tree (from the loaded description), as part of opening
    /// it: not an edit to undo, and the project does not count as changed.
    fn fill_link_trees(&mut self) {
        let missing: Vec<(Id<gc_doc::Robot>, Vec<gc_doc::RobotLink>)> = self
            .doc
            .robots()
            .iter()
            .filter(|(_, r)| r.links.is_empty())
            .filter_map(|(id, _)| {
                let loaded = self.robot_model(id)?;
                Some((id, crate::cell::link_tree(&loaded.model, &loaded.chain)))
            })
            .collect();
        if missing.is_empty() {
            return;
        }
        let unchanged = self.saved_revision == self.doc.revision();
        for (id, links) in missing {
            // A tree from the loaded description is always well formed; if not, the robot simply has none.
            if gc_doc::apply(&mut self.doc, &Command::Cell(gc_doc::CellOp::SetRobotLinks { id, links }))
                .is_err()
            {
                log::warn!("could not record the link tree of robot {id:?}");
            }
        }
        if unchanged {
            self.saved_revision = self.doc.revision();
        }
    }

    /// Starts a new, empty document. Unsaved changes are discarded; ask the user first.
    pub fn new_document(&mut self) {
        self.replace_document(Document::new(), None);
    }

    /// Saves the document to `path` and remembers the path.
    ///
    /// The file is a project: mesh files the document refers to (equipment, robots read from URDF files)
    /// are copied into a folder next to it, `<name>_files`, and referred to relative to the project, so
    /// that the project can be moved or sent as that file and folder.
    pub fn save_as(&mut self, path: &Path) -> Result<(), PersistError> {
        let packed = self.packed_for(path)?;
        gc_persist::save(&packed, path)?;
        self.recovered = false;
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
        let doc = project::unpacked(loaded.doc, path);
        self.replace_document(doc, Some(path.to_path_buf()));
        self.load_warnings = loaded.warnings;
        Ok(loaded.needs_repair)
    }

    /// Takes up work from an automatic save: the document as it was, belonging to `project` (saving goes
    /// there), and marked as not saved.
    pub fn open_recovered(&mut self, autosave: &Path, project: Option<&Path>) -> Result<(), PersistError> {
        let loaded = gc_persist::load(autosave)?;
        self.replace_document(loaded.doc, project.map(Path::to_path_buf));
        self.load_warnings = loaded.warnings;
        self.recovered = true;
        Ok(())
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
        if self.tool.kind() == ToolKind::Select
            && let Some(prompt) = self.pick_prompt()
        {
            return prompt;
        }
        self.tool.prompt()
    }

    /// What the tool draws on top of the sketch.
    pub fn preview(&self) -> Preview {
        let Some((_, sketch)) = self.active_sketch() else { return Preview::default() };
        let mut preview = self.tool.preview(sketch);
        // Move, Copy, Rotate: the selection where it would go.
        if let Some(motion) = self.tool.transform_preview() {
            let at = |p| sketch.pos(p).map(|q| motion.place(q)).map(|q| [q[0], q[1]]);
            for curve in self.selected_curves() {
                match sketch.curves.get(&curve).map(|c| c.geom.clone()) {
                    Some(gc_sketch::CurveGeom::Line { a, b }) => {
                        if let (Some(a), Some(b)) = (at(a), at(b)) {
                            preview.lines.push((a, b));
                        }
                    }
                    Some(gc_sketch::CurveGeom::Circle { c, r }) => {
                        if let Some(c) = at(c) {
                            preview.circles.push((c, r));
                        }
                    }
                    Some(gc_sketch::CurveGeom::Spline { points }) => {
                        let fit: Option<Vec<[f64; 2]>> = points.iter().map(|p| at(*p)).collect();
                        if let Some(fit) = fit {
                            gc_tools::push_polyline(&mut preview, &gc_sketch::spline_points(&fit, 16), false);
                        }
                    }
                    Some(gc_sketch::CurveGeom::Ellipse { c, major, minor, arc }) => {
                        let ends = match arc {
                            Some((s, e)) => at(s).zip(at(e)),
                            None => None,
                        };
                        if let (Some(c), Some(m)) = (at(c), at(major))
                            && (arc.is_none() || ends.is_some())
                            && let Some((points, closed)) =
                                gc_sketch::ellipse_points(c, m, minor * motion.radius_factor(), ends, 64)
                        {
                            gc_tools::push_polyline(&mut preview, &points, closed);
                        }
                    }
                    Some(gc_sketch::CurveGeom::Arc { c, s, e }) => {
                        if let (Some(c), Some(s), Some(e)) = (at(c), at(s), at(e)) {
                            let angle = |p: [f64; 2]| gc_math::fm::atan2(p[1] - c[1], p[0] - c[0]);
                            let r = gc_math::fm::hypot(s[0] - c[0], s[1] - c[1]);
                            let sweep = (angle(e) - angle(s)).rem_euclid(std::f64::consts::TAU);
                            preview.arcs.push((c, r, angle(s), sweep));
                        }
                    }
                    None => {}
                }
            }
        }
        preview
    }

    /// The Move and Rotate dialogs: Copy (move a copy) and relax (the More options at Always: constraints
    /// to other geometry break, dimensions to it take their new values; otherwise Never).
    pub fn transform_options(&self) -> (bool, bool) {
        (self.transform_copy, self.transform_relax)
    }

    /// Sets the Move and Rotate dialogs' Copy and relax options.
    pub fn set_transform_options(&mut self, copy: bool, relax: bool) {
        self.transform_copy = copy;
        self.transform_relax = relax;
    }

    /// The input boxes the heads-up display should show for the tool now (empty: no display). Values
    /// typed go back with `ToolEvent::Hud`, Enter finishes with `ToolEvent::Key(Key::Enter)`.
    pub fn tool_hud(&self) -> Vec<gc_tools::HudSlot> {
        self.tool.hud()
    }

    /// True when the drawing tools make construction geometry (the Format panel's toggle).
    pub fn tool_construction(&self) -> bool {
        self.construction
    }

    /// Turns the Construction toggle on or off. It stays as set when the tool changes. With curves
    /// selected (and the select tool on), it also converts them, as one undo step.
    pub fn set_tool_construction(&mut self, construction: bool) -> Result<(), EditError> {
        self.construction = construction;
        self.tool.set_construction(construction);
        let curves: Vec<Id<Curve>> = self
            .selection
            .iter()
            .filter_map(|i| if let gc_sketch::Item::Curve(c) = i { Some(*c) } else { None })
            .collect();
        if self.tool.kind() == gc_tools::ToolKind::Select && !curves.is_empty() {
            self.sketch_op(SketchOp::SetConstructionOf { curves, construction })?;
        }
        Ok(())
    }

    /// The selected items of the active sketch.
    pub fn selection(&self) -> &BTreeSet<Item> {
        &self.selection
    }

    /// The selected items in the order they were picked.
    pub fn selection_order(&self) -> Vec<Item> {
        self.picked.iter().filter(|item| self.selection.contains(item)).copied().collect()
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
        self.spatial.reset();
        self.drag = None;
        self.request = None;
        self.offer = None;
        self.tool = self.tool.renewed(self.tool.kind());
        self.tool.set_construction(self.construction);
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
        // Editing a component in place: its own origin plane, so the sketch belongs to it and moves with it.
        if let Some(gc_doc::TreeKey::Component(occurrence)) = self.edit_target() {
            let plane = match plane {
                "XZ" => gc_doc::OriginPlane::Xz,
                "YZ" => gc_doc::OriginPlane::Yz,
                _ => gc_doc::OriginPlane::Xy,
            };
            return self.create_sketch_on(gc_doc::PlaneSource::Component { occurrence, plane });
        }
        let name = format!("Sketch{}", self.doc.sketches().len() + 1);
        let created = self.execute_owned(Command::CreateSketch { name, frame, spatial: false })?;
        match created.first() {
            Some(Created::Sketch(id)) => {
                self.enter_created_sketch(*id);
                Ok(*id)
            }
            _ => Err(gc_sketch::SketchError::MissingItem.into()),
        }
    }

    /// Renames a model-tree item (one undo step). Empty names and names another item of the same kind
    /// has are refused (`EditError::NameTaken`).
    pub fn rename(&mut self, item: gc_core::AnyId, name: &str) -> Result<(), EditError> {
        self.execute(&Command::Rename { item, name: name.to_owned() }).map(|_| ())
    }

    /// Shows or hides any model-tree item (one undo step). Hidden items are neither drawn nor picked.
    pub fn set_visible(&mut self, item: gc_core::AnyId, visible: bool) -> Result<(), EditError> {
        self.execute(&Command::SetVisible { item, visible }).map(|_| ())
    }

    /// True when the item is shown.
    pub fn is_visible(&self, item: gc_core::AnyId) -> bool {
        self.doc.is_visible(item)
    }

    /// The assembly tree: bodies, robots and imported assemblies, each followed by its parts, with their
    /// grounded, visible, enabled and suppressed states (assembly P1-1).
    pub fn components(&self) -> Vec<gc_doc::ComponentNode> {
        self.doc.components()
    }

    /// A robot's links as sub-components (each with an origin folder that follows the joint values), to
    /// show under the robot's node; not included in [`Session::components`].
    pub fn robot_links(&self, robot: Id<gc_doc::Robot>) -> Vec<gc_doc::ComponentNode> {
        self.doc.robot_links(robot)
    }

    /// Switches a state of a component (one undo step). Hidden parts are not drawn but stay obstacles;
    /// suppressed parts are neither. Grounding takes effect with assembly constraints; disabled
    /// components are for the display to draw in the background style and not pick.
    pub fn set_component(
        &mut self,
        occurrence: gc_doc::Occurrence,
        flag: gc_doc::ComponentFlag,
        on: bool,
    ) -> Result<(), EditError> {
        self.execute(&Command::SetComponent { occurrence, flag, on }).map(|_| ())
    }

    /// Suppresses a design item, waypoint or path, or brings it back (one undo step).
    pub fn set_suppressed(&mut self, item: gc_core::AnyId, suppressed: bool) -> Result<(), EditError> {
        self.execute(&Command::Cell(gc_doc::CellOp::SetSuppressed { item, suppressed })).map(|_| ())
    }

    /// True when the item is suppressed or stands after the rollback marker (kept, not computed).
    pub fn is_suppressed(&self, item: gc_core::AnyId) -> bool {
        self.doc.is_inactive(item)
    }

    /// What deleting the item would affect: everything that depends on it, directly first.
    pub fn dependents_of(&self, item: gc_core::AnyId) -> Vec<gc_core::AnyId> {
        self.doc.dependents_of(item)
    }

    /// The design items of the model tree in order (sketches, work features, features, assemblies).
    pub fn model_order(&self) -> Vec<gc_core::AnyId> {
        self.doc.model_order()
    }

    /// Moves a design item to just before another (`None`: to the end). Refused (`EditError::InUse`)
    /// when it would come before what it depends on or after what depends on it.
    pub fn move_item(
        &mut self,
        item: gc_core::AnyId,
        before: Option<gc_core::AnyId>,
    ) -> Result<(), EditError> {
        self.execute(&Command::Cell(gc_doc::CellOp::MoveInTree { item, before })).map(|_| ())
    }

    /// Where the rollback marker stands: `Some(i)` when the items after position `i` of
    /// [`Session::model_order`] are rolled back, `None` at the end.
    pub fn rollback_index(&self) -> Option<usize> {
        self.doc.rollback_index()
    }

    /// Puts the rollback marker after position `index` of [`Session::model_order`] (`None`: at the end).
    pub fn set_rollback(&mut self, index: Option<usize>) -> Result<(), EditError> {
        let after = match index {
            Some(i) => Some(*self.doc.model_order().get(i).ok_or(EditError::BadValue)?),
            None => None,
        };
        self.execute(&Command::Cell(gc_doc::CellOp::SetRollback { after })).map(|_| ())
    }

    /// Creates a 2D sketch on a work plane and starts editing it. The sketch follows the plane.
    pub fn create_sketch_on_work_plane(
        &mut self,
        plane: Id<gc_doc::WorkFeature>,
    ) -> Result<Id<Sketch>, EditError> {
        self.create_sketch_on(gc_doc::PlaneSource::Work(plane))
    }

    /// Creates a 2D sketch on an origin plane of a component (body, robot base, assembly or part) and
    /// starts editing it. The sketch moves with the component.
    pub fn create_sketch_on_component(
        &mut self,
        occurrence: gc_doc::Occurrence,
        plane: gc_doc::OriginPlane,
    ) -> Result<Id<Sketch>, EditError> {
        self.create_sketch_on(gc_doc::PlaneSource::Component { occurrence, plane })
    }

    /// Shows or hides elements of a component's origin folder (one undo step; saved). All are hidden by
    /// default. To switch the whole folder pass all seven elements.
    pub fn set_origin_visible(
        &mut self,
        occurrence: gc_doc::Occurrence,
        elements: &[gc_doc::OriginElement],
        visible: bool,
    ) -> Result<(), EditError> {
        self.execute(&Command::SetOriginVisible { occurrence, elements: elements.to_vec(), visible })
            .map(|_| ())
    }

    /// True when an element of a component's origin folder is shown.
    pub fn origin_visible(&self, occurrence: gc_doc::Occurrence, element: gc_doc::OriginElement) -> bool {
        self.doc.origin_visible(occurrence, element)
    }

    /// Adds an assembly constraint (Mate, Flush, Insert) and moves the components it ties that are not
    /// grounded (one undo step). Refused with `EditError::Unsolvable` when it cannot be met together with
    /// the others.
    pub fn add_relationship(
        &mut self,
        relationship: gc_doc::Relationship,
    ) -> Result<Id<gc_doc::Relationship>, EditError> {
        let created = self.execute(&Command::Cell(gc_doc::CellOp::AddRelationship(relationship)))?;
        created
            .iter()
            .find_map(|c| if let gc_doc::Created::Relationship(id) = c { Some(*id) } else { None })
            .ok_or(EditError::BadValue)
    }

    /// Changes an assembly constraint (its kind, offset or selections) and solves again (one undo step).
    pub fn set_relationship(
        &mut self,
        id: Id<gc_doc::Relationship>,
        relationship: gc_doc::Relationship,
    ) -> Result<(), EditError> {
        self.execute(&Command::Cell(gc_doc::CellOp::SetRelationship { id, relationship })).map(|_| ())
    }

    /// Deletes an assembly constraint; the components stay where they are.
    pub fn delete_relationship(&mut self, id: Id<gc_doc::Relationship>) -> Result<(), EditError> {
        self.execute(&Command::Cell(gc_doc::CellOp::DeleteRelationship(id))).map(|_| ())
    }

    /// Accept the relationship (the error dialog's third choice): adds it even when it cannot be
    /// satisfied; it is then kept with an error mark, left out of the solve, and loses the mark as soon
    /// as the assembly can satisfy it. One undo step.
    pub fn accept_relationship(
        &mut self,
        relationship: gc_doc::Relationship,
    ) -> Result<Id<gc_doc::Relationship>, EditError> {
        let created = self.execute(&Command::Cell(gc_doc::CellOp::AcceptRelationship(relationship)))?;
        created
            .iter()
            .find_map(|c| if let gc_doc::Created::Relationship(id) = c { Some(*id) } else { None })
            .ok_or(EditError::BadValue)
    }

    /// Accepts a change to a relationship although it cannot be solved. One undo step.
    pub fn accept_relationship_edit(
        &mut self,
        id: Id<gc_doc::Relationship>,
        relationship: gc_doc::Relationship,
    ) -> Result<(), EditError> {
        let op = gc_doc::CellOp::AcceptRelationshipEdit { id, relationship: Box::new(relationship) };
        self.execute(&Command::Cell(op)).map(|_| ())
    }

    /// The relationships kept with an error mark (what Recover lists).
    pub fn sick_relationships(&self) -> Vec<Id<gc_doc::Relationship>> {
        self.doc.sick_relationships()
    }

    /// Isolate Related Failed: the relationships that stand in the way of one with an error, and the
    /// components to keep visible (pass them to Isolate).
    pub fn related_failed(
        &self,
        id: Id<gc_doc::Relationship>,
    ) -> (Vec<Id<gc_doc::Relationship>>, Vec<gc_doc::Occurrence>) {
        self.doc.related_failed(id)
    }

    /// Check in Relationship Conflict Analysis: whether everything can be satisfied with the `broken`
    /// relationships left out and the grounding of the `toggled` components turned round. Changes nothing.
    pub fn conflict_check(
        &self,
        broken: &[Id<gc_doc::Relationship>],
        toggled: &[gc_doc::Occurrence],
    ) -> bool {
        self.doc.conflict_check(broken, toggled)
    }

    /// Free Move / Free Rotate: drops a component (body, robot, imported assembly) at `placement` without
    /// solving its relationships (Inventor 2026 help, Move assembly components,
    /// GUID-9796C22B-23C1-4D69-BC2E-54056E46F255). It stays there until [`Session::update_assembly`] or
    /// the next command that solves the relationships. One undo step.
    pub fn free_move(&mut self, item: gc_core::AnyId, placement: gc_doc::Placement) -> Result<(), EditError> {
        self.execute(&Command::Cell(gc_doc::CellOp::FreeMove { item, placement })).map(|_| ())
    }

    /// Update: components snap back to where their relationships put them; grounded ones stay and what
    /// is related to them follows. One undo step.
    pub fn update_assembly(&mut self) -> Result<(), EditError> {
        self.execute(&Command::Cell(gc_doc::CellOp::UpdateAssembly)).map(|_| ())
    }

    /// True when a component was dropped where its relationships do not hold (Update would move it).
    pub fn needs_update(&self) -> bool {
        self.doc.needs_update()
    }

    /// The relationships as elastic bands for Free Move: both ends in the world and whether each holds.
    pub fn relationship_bands(&self) -> Vec<gc_doc::RelationBand> {
        self.doc.relationship_bands()
    }

    /// The assembly constraints, for the Relationships folder.
    pub fn relationships(&self) -> Vec<(Id<gc_doc::Relationship>, &gc_doc::Relationship)> {
        self.doc.relationships().iter().collect()
    }

    /// Predict Offset and Orientation: the constraint two selections suggest as they lie now (Mate when
    /// planes face each other, otherwise Flush; the current distance as the offset).
    pub fn predict_relationship(
        &self,
        first: &gc_doc::Selection,
        second: &gc_doc::Selection,
    ) -> Option<gc_doc::RelationKind> {
        self.doc.predict_relationship(first, second)
    }

    /// The joint type Automatic picks for two joint origins (Rotational when both are circular, otherwise
    /// Rigid). Joints are added with [`Session::add_relationship`] like constraints.
    pub fn automatic_joint(
        &self,
        first: &gc_doc::Selection,
        second: &gc_doc::Selection,
    ) -> gc_doc::RelationKind {
        gc_doc::Document::automatic_joint(first, second)
    }

    /// Lock and Protect of a joint (none for a constraint).
    pub fn joint_condition(&self, id: Id<gc_doc::Relationship>) -> gc_doc::JointCondition {
        self.doc.joint_condition(id).unwrap_or_default()
    }

    /// Lock a joint where it stands, or free it (Inventor 2026 help, To Define and Manage Joint
    /// Relationships, GUID-21DC3336-5C51-42C1-90FB-4299CD66E0C6). One undo step; refused for constraints.
    pub fn set_joint_lock(&mut self, id: Id<gc_doc::Relationship>, on: bool) -> Result<(), EditError> {
        let protect = self.joint_condition(id).protected;
        let op = gc_doc::CellOp::SetJointCondition { id, lock: on, protect };
        self.execute(&Command::Cell(op)).map(|_| ())
    }

    /// Protect a joint's freedom, or not. One undo step; refused for constraints.
    pub fn set_joint_protect(&mut self, id: Id<gc_doc::Relationship>, on: bool) -> Result<(), EditError> {
        let lock = self.joint_condition(id).locked.is_some();
        let op = gc_doc::CellOp::SetJointCondition { id, lock, protect: on };
        self.execute(&Command::Cell(op)).map(|_| ())
    }

    /// The freedom a component has left under the assembly relationships (the DOF symbol): total and pure
    /// translations. Grounded: none; unconstrained: six.
    pub fn component_dof(&self, item: gc_core::AnyId) -> Option<gc_doc::Dof> {
        self.doc.component_dof(item)
    }

    /// The constraint status dot of a component in the browser: full (black), under (hollow), unknown (dash).
    pub fn component_status(&self, item: gc_core::AnyId) -> Option<gc_doc::ComponentStatus> {
        self.doc.component_status(item)
    }

    /// The constraint status dots of every body, robot and assembly at once — use this for the browser
    /// each frame rather than one `component_status` per row.
    pub fn component_statuses(&self) -> std::collections::BTreeMap<gc_core::AnyId, gc_doc::ComponentStatus> {
        let revision = self.doc.revision();
        if let Some((at, statuses)) = self.cell.statuses.borrow().as_ref()
            && *at == revision
        {
            return statuses.clone();
        }
        let statuses = self.doc.component_statuses();
        *self.cell.statuses.borrow_mut() = Some((revision, statuses.clone()));
        statuses
    }

    /// The status of an assembly relationship: full, under-constrained or sick.
    pub fn relationship_status(&self, id: Id<gc_doc::Relationship>) -> Option<gc_doc::RelationStatus> {
        self.doc.relationship_status(id)
    }

    /// Project Geometry into the sketch being edited: reference geometry that follows its source. It is
    /// construction geometry when the Construction toggle is on, the other way round with `shift` (Shift
    /// held while clicking). One undo step.
    pub fn project_geometry(
        &mut self,
        source: gc_doc::ProjectSource,
        shift: bool,
    ) -> Result<Vec<gc_doc::Created>, EditError> {
        let sketch = self.active.ok_or(EditError::BadValue)?;
        let construction = self.tool_construction() != shift;
        self.execute(&Command::Sketch { sketch, op: gc_doc::SketchOp::Project { source, construction } })
    }

    /// Ground and Root, "Ground at origin": moves components to the assembly's origin, aligned with it,
    /// and grounds them (one undo step).
    pub fn ground_at_origin(&mut self, items: &[gc_core::AnyId]) -> Result<(), EditError> {
        self.execute(&Command::Cell(gc_doc::CellOp::GroundAtOrigin(items.to_vec()))).map(|_| ())
    }

    /// Place at Component Origin: puts a component (just added, typically) on another's origin and
    /// constrains it there with three Flush constraints (one undo step).
    pub fn place_at_component_origin(
        &mut self,
        item: gc_core::AnyId,
        target: gc_doc::Occurrence,
    ) -> Result<(), EditError> {
        self.execute(&Command::Cell(gc_doc::CellOp::PlaceAtComponentOrigin { item, target })).map(|_| ())
    }

    /// The constraint error dialog: which relationships stand in the way of `candidate` (or of the
    /// relationships as they are), each of which alone, suppressed, would let everything be satisfied.
    pub fn conflicting_relationships(
        &self,
        candidate: Option<&gc_doc::Relationship>,
    ) -> Vec<Id<gc_doc::Relationship>> {
        self.doc.conflicting_relationships(candidate)
    }

    /// Suppresses an assembly relationship (or brings it back) and solves again: the recovery the
    /// constraint error dialog offers. One undo step.
    pub fn set_relationship_suppressed(
        &mut self,
        id: Id<gc_doc::Relationship>,
        suppressed: bool,
    ) -> Result<(), EditError> {
        self.set_suppressed(id.erase(), suppressed)
    }

    /// View and positional representations (the Representations folder). Change them with
    /// `CellOp::{AddViewRepresentation, SetViewRepresentation, ActivateViewRepresentation,
    /// AddPositionalRepresentation, SetPositionalRepresentation, ActivatePositionalRepresentation,
    /// DeleteRepresentation}` through [`Session::execute`].
    pub fn representations(&self) -> gc_doc::Representations {
        self.doc.representations()
    }

    /// What the browser's Representations folder lists: the view representations and the positional
    /// ones ([Primary] first), each with its name and whether it is the active one. Indices into the
    /// lists are what the activate/delete calls take ([Primary] is `None`).
    pub fn representation_folder(&self) -> RepresentationFolder {
        let reps = self.doc.representations();
        let views = reps
            .views
            .iter()
            .enumerate()
            .map(|(i, v)| RepresentationNode { name: v.name.clone(), active: reps.active_view == Some(i) })
            .collect();
        let positions = std::iter::once(RepresentationNode {
            name: "Primary".to_owned(),
            active: reps.active_position.is_none(),
        })
        .chain(reps.positions.iter().enumerate().map(|(i, p)| RepresentationNode {
            name: p.name.clone(),
            active: reps.active_position == Some(i),
        }))
        .collect();
        RepresentationFolder { views, positions }
    }

    /// Saves what is shown now as a new view representation. One undo step; a taken or empty name is
    /// refused.
    pub fn add_view_representation(&mut self, name: &str, camera: Option<String>) -> Result<(), EditError> {
        let op = gc_doc::CellOp::AddViewRepresentation { name: name.to_owned(), camera, section: None };
        self.execute(&Command::Cell(op)).map(|_| ())
    }

    /// Activates a view representation (visibility, enabled state, transparency). One undo step.
    pub fn activate_view_representation(&mut self, index: usize) -> Result<(), EditError> {
        self.execute(&Command::Cell(gc_doc::CellOp::ActivateViewRepresentation(index))).map(|_| ())
    }

    /// Lock or unlock a view representation: a locked one keeps its display characteristics, and
    /// components added while it is active are hidden. While a view is active and not locked, display
    /// edits (visibility, enabled, transparency) are saved in it. One undo step.
    pub fn lock_view_representation(&mut self, index: usize, locked: bool) -> Result<(), EditError> {
        let op = gc_doc::CellOp::LockViewRepresentation { index, locked };
        self.execute(&Command::Cell(op)).map(|_| ())
    }

    /// Copies a view representation under a numbered name; the copy becomes the active one.
    pub fn copy_view_representation(&mut self, index: usize) -> Result<(), EditError> {
        self.execute(&Command::Cell(gc_doc::CellOp::CopyViewRepresentation(index))).map(|_| ())
    }

    /// Copies a positional representation with its overrides under a numbered name.
    pub fn copy_positional_representation(&mut self, index: usize) -> Result<(), EditError> {
        self.execute(&Command::Cell(gc_doc::CellOp::CopyPositionalRepresentation(index))).map(|_| ())
    }

    /// Adds a positional representation (a copy of [Primary]). One undo step.
    pub fn add_positional_representation(&mut self, name: &str) -> Result<(), EditError> {
        self.execute(&Command::Cell(gc_doc::CellOp::AddPositionalRepresentation(name.to_owned()))).map(|_| ())
    }

    /// Activates a positional representation, or [Primary] with `None`. One undo step.
    pub fn activate_positional_representation(&mut self, index: Option<usize>) -> Result<(), EditError> {
        self.execute(&Command::Cell(gc_doc::CellOp::ActivatePositionalRepresentation(index))).map(|_| ())
    }

    /// Deletes a view and/or a positional representation (not [Primary]). One undo step.
    pub fn delete_representation(
        &mut self,
        view: Option<usize>,
        position: Option<usize>,
    ) -> Result<(), EditError> {
        self.execute(&Command::Cell(gc_doc::CellOp::DeleteRepresentation { view, position })).map(|_| ())
    }

    /// The component patterns (for the browser: each with its parents and occurrences).
    pub fn patterns(&self) -> Vec<gc_doc::ComponentPattern> {
        self.doc.patterns()
    }

    /// Patterns components (Pattern Component: rectangular or circular); the copies follow the parents.
    /// One undo step. Edit with `CellOp::SetPattern`, remove with `CellOp::DeletePattern`, take an occurrence
    /// out with `CellOp::MakePatternElementIndependent`.
    pub fn add_pattern(
        &mut self,
        name: &str,
        parents: &[gc_core::AnyId],
        layout: gc_doc::PatternLayout,
    ) -> Result<(), EditError> {
        self.execute(&Command::Cell(gc_doc::CellOp::AddPattern {
            name: name.to_owned(),
            parents: parents.to_vec(),
            layout,
        }))
        .map(|_| ())
    }

    /// Runs a mirror, copy or replace and returns the names of the relationships it left suppressed
    /// (made suppressed because their orientation was ambiguous, or because they could no longer be
    /// satisfied), for the message Inventor shows.
    fn replicate(&mut self, op: gc_doc::CellOp) -> Result<Vec<String>, EditError> {
        let was: Vec<gc_core::AnyId> = self
            .doc
            .relationships()
            .iter()
            .map(|(id, _)| id.erase())
            .filter(|id| self.doc.is_inactive(*id))
            .collect();
        self.execute(&Command::Cell(op))?;
        Ok(self
            .doc
            .relationships()
            .iter()
            .filter(|(id, _)| self.doc.is_inactive(id.erase()) && !was.contains(&id.erase()))
            .map(|(_, r)| r.name.clone())
            .collect())
    }

    /// Mirror Components across a plane (a planar face, work plane or origin plane): with
    /// `relationships`, those between the selected components are re-made for the copies. One undo step.
    /// Returns the names of the relationships made suppressed.
    pub fn mirror_components(
        &mut self,
        items: &[gc_core::AnyId],
        plane: gc_doc::Selection,
        relationships: bool,
        ground: bool,
    ) -> Result<Vec<String>, EditError> {
        self.replicate(gc_doc::CellOp::MirrorComponents {
            items: items.to_vec(),
            plane,
            relationships,
            ground,
        })
    }

    /// Mirror Components with true mirror images for every component (the dialog's Mirror status): see
    /// [`Session::mirror_components_mixed`].
    pub fn mirror_components_reflected(
        &mut self,
        items: &[gc_core::AnyId],
        plane: gc_doc::Selection,
        relationships: bool,
        ground: bool,
    ) -> Result<MirrorOutcome, EditError> {
        let all: Vec<(gc_core::AnyId, MirrorState)> =
            items.iter().map(|i| (*i, MirrorState::Mirror)).collect();
        self.mirror_components_mixed(&all, plane, relationships, ground)
    }

    /// Where mirror-image mesh files are written: the project's own files folder (`<name>_files` next
    /// to the project file) once the document has a file, before that the application's cache (or the
    /// folder given with [`Session::set_asset_cache`]). Saving copies what the document uses from the
    /// cache into the project and writes paths relative to it, like every other mesh file.
    fn mirror_folder(&self) -> Option<PathBuf> {
        match &self.path {
            Some(path) => Some(project::files_dir(path)),
            None => self
                .asset_cache
                .clone()
                .or_else(|| gc_persist::places::cache_dir().map(|dir| dir.join("mirrored"))),
        }
    }

    /// Sets the folder that files made for a document without a file of its own are kept in (the
    /// default is the application's cache folder).
    pub fn set_asset_cache(&mut self, dir: Option<PathBuf>) {
        self.asset_cache = dir;
    }

    /// Mirror Components with a status per component, as the dialog has it (Inventor 2026 help, Mirror
    /// Components: Reference, GUID-7B19CA69-8D14-40DA-9543-115A8AFE6336: Mirror "Creates a mirrored
    /// component and saves it in a new file", Reuse "Adds a component instance"; a component set to
    /// Exclude is simply left out). One undo step for all of them.
    ///
    /// Mirror: for a body whose shape is a mesh file, the reflected mesh is written as
    /// `<name>.mirror-<x|y|z>-<content hash>.stl` (once: the same mirror image is found again by its
    /// name) into the project's files folder, or the cache while the document has no file, and the copy
    /// points to it. Boxes, cylinders and spheres are their own mirror images. Robots, imported
    /// assemblies and bodies whose file cannot be read or written are placed as with Reuse (the same
    /// shape, turned) and are returned in `not_reflected`.
    pub fn mirror_components_mixed(
        &mut self,
        items: &[(gc_core::AnyId, MirrorState)],
        plane: gc_doc::Selection,
        relationships: bool,
        ground: bool,
    ) -> Result<MirrorOutcome, EditError> {
        let folder = self.mirror_folder();
        let mut mirrored = Vec::new();
        let mut not_reflected = Vec::new();
        for (item, state) in items {
            if *state == MirrorState::Reuse {
                continue;
            }
            if item.kind != gc_core::Kind::Body {
                not_reflected.push(*item);
                continue;
            }
            let id = Id::<gc_doc::Body>::from_raw(item.raw);
            let Some(gc_doc::Solid::Mesh { file, scale }) = self.doc.get(id).map(|b| b.solid.clone()) else {
                continue;
            };
            let reflected = self
                .doc
                .mirror_axis(*item, &plane)
                .zip(folder.as_deref())
                .and_then(|(axis, folder)| cell::mirrored_mesh_file(&file, axis, folder).ok());
            match reflected {
                Some(file) => mirrored.push((id, gc_doc::Solid::Mesh { file, scale })),
                None => not_reflected.push(*item),
            }
        }
        let suppressed = self.replicate(gc_doc::CellOp::MirrorComponentsWith {
            items: items.iter().map(|(item, _)| *item).collect(),
            plane,
            relationships,
            ground,
            mirrored,
        })?;
        Ok(MirrorOutcome { suppressed, not_reflected })
    }

    /// Copy Components, moved by `offset`. One undo step. Returns the names of the relationships made
    /// suppressed.
    pub fn copy_components(
        &mut self,
        items: &[gc_core::AnyId],
        offset: gc_doc::Placement,
        relationships: bool,
        ground: bool,
    ) -> Result<Vec<String>, EditError> {
        self.replicate(gc_doc::CellOp::CopyComponents {
            items: items.to_vec(),
            offset,
            relationships,
            ground,
        })
    }

    /// Replace (or, with `all`, Replace All: every component of the same source) a component with another
    /// shape or file. Returns the names of the relationships that did not survive (suppressed, to be
    /// applied again).
    pub fn replace_component(
        &mut self,
        item: gc_core::AnyId,
        with: gc_doc::Replacement,
        all: bool,
    ) -> Result<Vec<String>, EditError> {
        let items = if all { self.doc.same_source(item) } else { vec![item] };
        self.replicate(gc_doc::CellOp::ReplaceComponents { items, with })
    }

    /// Adds components to the contact set, or takes them out. One undo step.
    pub fn set_contact_set(&mut self, items: &[gc_core::AnyId], on: bool) -> Result<(), EditError> {
        self.execute(&Command::Cell(gc_doc::CellOp::SetContactSet { items: items.to_vec(), on })).map(|_| ())
    }

    /// Interactive Contact: Activate Contact Solver is `ContactSolver::ContactSet`; `All` analyses every
    /// component; `Off` turns it off. One undo step.
    pub fn set_contact_solver(&mut self, mode: gc_doc::ContactSolver) -> Result<(), EditError> {
        self.execute(&Command::Cell(gc_doc::CellOp::SetContactSolver(mode))).map(|_| ())
    }

    /// The degrees-of-freedom symbol of a component: directions it can slide along, axes it can turn
    /// about, and the under-constrained components it is constrained to (the cube).
    pub fn dof_symbol(&self, item: gc_core::AnyId) -> Option<gc_doc::DofSymbol> {
        self.doc.dof_symbol(item)
    }

    /// The symbols to draw this frame: every component whose symbol is shown.
    pub fn dof_symbols(&self) -> Vec<(gc_core::AnyId, gc_doc::DofSymbol)> {
        let doc = &self.doc;
        doc.bodies()
            .iter()
            .map(|(id, _)| id.erase())
            .chain(doc.robots().iter().map(|(id, _)| id.erase()))
            .chain(doc.assemblies().iter().map(|(id, _)| id.erase()))
            .filter(|item| doc.dof_shown(*item) && !doc.is_component_suppressed(*item))
            .filter_map(|item| Some((item, doc.dof_symbol(item)?)))
            .collect()
    }

    /// Shows or hides the degrees-of-freedom symbol of components. One undo step.
    pub fn set_dof_shown(&mut self, items: &[gc_core::AnyId], on: bool) -> Result<(), EditError> {
        self.execute(&Command::Cell(gc_doc::CellOp::SetDofShown { items: items.to_vec(), on })).map(|_| ())
    }

    /// What the Assemble command offers for two picked selections (geometry first, type after).
    pub fn possible_relations(
        &self,
        first: &gc_doc::Selection,
        second: &gc_doc::Selection,
    ) -> Vec<gc_doc::AssembleSolution> {
        gc_doc::possible_relations(first, second)
    }

    /// A work frame at the origin of a component's plane, in one undo step (a sketch on the plane holds
    /// its origin). Waypoints in it follow the component, wherever it is moved or constrained to.
    pub fn add_component_frame(
        &mut self,
        occurrence: gc_doc::Occurrence,
        plane: gc_doc::OriginPlane,
    ) -> Result<Id<gc_doc::WorkFrame>, EditError> {
        let name = format!("Frame{}", self.doc.work_frames().len() + 1);
        let sketch = format!("Sketch{}", self.doc.sketches().len() + 1);
        let created = self.execute(&Command::Cell(gc_doc::CellOp::AddComponentFrame {
            name,
            sketch,
            occurrence,
            plane,
        }))?;
        created
            .iter()
            .find_map(|c| if let Created::Frame(id) = c { Some(*id) } else { None })
            .ok_or(EditError::BadValue)
    }

    /// A component's origin folder in the world: centre point, axes and planes (assembly P1-2).
    pub fn component_origin(&self, occurrence: gc_doc::Occurrence) -> Option<gc_doc::ComponentOrigin> {
        self.doc.component_origin(occurrence)
    }

    /// Creates a 2D sketch on the plane of a triangle of an imported part (a picked face) and starts
    /// editing it: origin at the triangle's centre, normal out of its front side, X along its first
    /// edge. The sketch follows the assembly when it is moved. With Autoproject edges on (the default),
    /// the face's outline comes into the sketch as fixed lines (a step of its own, which Cancel Sketch
    /// takes back with the sketch).
    pub fn create_sketch_on_face(
        &mut self,
        assembly: Id<gc_doc::Assembly>,
        part: usize,
        mesh: usize,
        triangle: usize,
    ) -> Result<Id<Sketch>, EditError> {
        let source = self.face_source(assembly, part, mesh, triangle).ok_or(EditError::BadValue)?;
        let sketch = self.create_sketch_on(source)?;
        if self.autoproject_edges {
            self.project_face_outline(sketch, assembly, mesh, triangle);
        }
        Ok(sketch)
    }

    fn face_source(
        &self,
        assembly: Id<gc_doc::Assembly>,
        part: usize,
        mesh: usize,
        triangle: usize,
    ) -> Option<gc_doc::PlaneSource> {
        let stored = self.doc.get(assembly)?;
        if !stored.parts.get(part)?.meshes.contains(&u32::try_from(mesh).ok()?) {
            return None;
        }
        let data = &self.cell.assemblies.of(stored).ok()?.get(mesh)?.data;
        let corners = data.indices().get(triangle * 3..triangle * 3 + 3)?;
        let at = |i: u32| {
            data.positions()
                .get(i as usize)
                .map(|p| gc_math::DVec3::new(p[0].into(), p[1].into(), p[2].into()))
        };
        let (a, b, c) = (at(corners[0])?, at(corners[1])?, at(corners[2])?);
        let normal = (b - a).cross(c - a);
        if normal.length() < 1e-12 || (b - a).length() < 1e-12 {
            return None;
        }
        let x = (b - a).normalize();
        let z = normal.normalize();
        let rotation = gc_math::DQuat::from_mat3(&gc_math::DMat3::from_cols(x, z.cross(x), z)).normalize();
        Some(gc_doc::PlaneSource::Face {
            assembly,
            part: u32::try_from(part).ok()?,
            triangle: (u32::try_from(mesh).ok()?, u32::try_from(triangle).ok()?),
            xyz: ((a + b + c) / 3.0).to_array(),
            quat_xyzw: rotation.to_array(),
        })
    }

    fn create_sketch_on(&mut self, source: gc_doc::PlaneSource) -> Result<Id<Sketch>, EditError> {
        let name = format!("Sketch{}", self.doc.sketches().len() + 1);
        let created = self.execute_owned(Command::CreateSketchOn { name, source })?;
        match created.first() {
            Some(Created::Sketch(id)) => {
                self.enter_created_sketch(*id);
                Ok(*id)
            }
            _ => Err(gc_sketch::SketchError::MissingItem.into()),
        }
    }

    /// Creates a 3D sketch in the world frame and starts editing it.
    pub fn create_sketch_3d(&mut self) -> Result<Id<Sketch>, EditError> {
        let name = format!("Sketch{}", self.doc.sketches().len() + 1);
        let frame = self.doc.world().ok_or(gc_sketch::SketchError::MissingItem)?;
        let created = self.execute_owned(Command::CreateSketch { name, frame, spatial: true })?;
        match created.first() {
            Some(Created::Sketch(id)) => {
                self.enter_created_sketch(*id);
                Ok(*id)
            }
            _ => Err(gc_sketch::SketchError::MissingItem.into()),
        }
    }

    /// Starts editing an existing sketch.
    pub fn enter_sketch(&mut self, sketch: Id<Sketch>) {
        if self.doc.get(sketch).is_some() {
            self.pick_target = None;
            self.active = Some(sketch);
            self.sketch_mark = Some((self.history.undo_depth(), self.history.trimmed()));
            self.selection.clear();
            self.set_tool(ToolKind::Select);
            self.refresh();
        }
    }

    /// Enters a sketch just created: Cancel Sketch also takes back its creation.
    fn enter_created_sketch(&mut self, sketch: Id<Sketch>) {
        self.enter_sketch(sketch);
        self.sketch_mark = self.sketch_mark.map(|(depth, trimmed)| (depth.saturating_sub(1), trimmed));
    }

    /// Cancel Sketch (Inventor 2026 help, To Create and Edit Sketches, GUID-E96FB3C4-136B-4551-BF45-
    /// F54594543ECB, Cancel a Sketch): everything done since the sketch was entered is discarded — its
    /// creation too when it was new — and editing ends. The discarded steps cannot be redone. Returns false
    /// when no sketch is being edited, or when the history (200 steps) no longer reaches back to the start;
    /// then as much as it holds is taken back.
    pub fn cancel_sketch(&mut self) -> bool {
        let (Some(_), Some((depth, trimmed))) = (self.active, self.sketch_mark) else { return false };
        self.drag = None;
        let lost = self.history.trimmed().saturating_sub(trimmed);
        let mark = usize::try_from(lost).ok().and_then(|lost| depth.checked_sub(lost));
        let target = mark.unwrap_or(0);
        while self.history.undo_depth() > target {
            if self.history.undo(&mut self.doc).is_none() {
                break;
            }
        }
        self.history.drop_redo();
        self.after_history();
        self.finish_sketch();
        mark.is_some()
    }

    /// Stops editing the active sketch.
    pub fn finish_sketch(&mut self) {
        self.sketch_mark = None;
        self.pick_target = None;
        self.active = None;
        self.selection.clear();
        self.set_tool(ToolKind::Select);
        self.refresh();
    }

    /// Switches the tool.
    pub fn set_tool(&mut self, kind: ToolKind) {
        // The tools' settings (Center Point, the Polygon dialog, the Offset toggles) outlive the tool.
        self.tool = self.tool.renewed(kind);
        // Scale reads the factor off the selection's size.
        if kind == ToolKind::Scale
            && let Some((_, sketch)) = self.active_sketch()
        {
            let points: Vec<[f64; 2]> = self
                .selected_curves()
                .iter()
                .filter_map(|c| sketch.curves.get(c))
                .flat_map(gc_sketch::Curve::points)
                .filter_map(|p| sketch.pos(p).map(|q| [q[0], q[1]]))
                .collect();
            self.tool.set_selection_points(points);
        }
        // Another command ends picking for an open sketch dialog.
        if kind != ToolKind::Select {
            self.pick_target = None;
        }
        self.reset_interaction();
    }

    /// The Format panel's Center Point toggle: the Point tool places centre points (Inventor's default).
    pub fn center_point(&self) -> bool {
        self.tool.center_point()
    }

    /// Sets the Center Point toggle; with standalone points selected, switches their style too (one undo
    /// step), as Inventor's toggle does.
    pub fn set_center_point(&mut self, on: bool) -> Result<(), EditError> {
        self.tool.set_center_point(on);
        let Some((_, sketch)) = self.active_sketch() else { return Ok(()) };
        let owned: BTreeSet<Id<gc_sketch::SkPoint>> =
            sketch.curves.values().flat_map(gc_sketch::Curve::points).collect();
        let points: Vec<Id<gc_sketch::SkPoint>> = self
            .selection
            .iter()
            .filter_map(|i| if let gc_sketch::Item::Point(p) = i { Some(*p) } else { None })
            .filter(|p| !owned.contains(p))
            .collect();
        if points.is_empty() {
            return Ok(());
        }
        self.sketch_op(SketchOp::SetCenterPoint { points, center: on }).map(|_| ())
    }

    /// The Polygon dialog: number of sides and circumscribed.
    pub fn polygon_settings(&self) -> (u32, bool) {
        self.tool.polygon()
    }

    /// Sets the Polygon dialog's values (sides clamped to 3–120).
    pub fn set_polygon_settings(&mut self, sides: u32, circumscribed: bool) {
        self.tool.set_polygon(sides, circumscribed);
    }

    /// Offset: Loop Select and Constrain Offset.
    pub fn offset_options(&self) -> (bool, bool) {
        self.tool.offset_options()
    }

    /// Sets Loop Select and Constrain Offset.
    pub fn set_offset_options(&mut self, loop_select: bool, constrain: bool) {
        self.tool.set_offset_options(loop_select, constrain);
    }

    /// Automatic Dimensions and Constraints (Inventor 2026 help, To Apply 2D Sketch Dimensions
    /// Automatically, GUID-1DC71C6F-9F6D-4846-8E73-9CB7D3452882) on the selected curves, or the whole
    /// sketch when none is selected. Returns how many dimensions and constraints were added.
    pub fn auto_dimension(&mut self, dimensions: bool, constraints: bool) -> Result<usize, EditError> {
        let op = SketchOp::AutoDimension { curves: self.selected_curves(), dimensions, constraints };
        let created = self.edit_active_sketch(op)?;
        self.auto_added = created
            .iter()
            .filter_map(|c| match c {
                Created::Dimension(d) => Some(Item::Dimension(*d)),
                Created::Constraint(c) => Some(Item::Constraint(*c)),
                _ => None,
            })
            .collect();
        Ok(self.auto_added.len())
    }

    /// The Auto Dimension dialog's Remove: takes back what the last Apply added (the dimensions and/or the
    /// constraints, as the boxes say).
    pub fn remove_auto_dimensions(&mut self, dimensions: bool, constraints: bool) -> Result<(), EditError> {
        let (gone, kept): (Vec<Item>, Vec<Item>) =
            std::mem::take(&mut self.auto_added).into_iter().partition(|i| {
                (dimensions && matches!(i, Item::Dimension(_)))
                    || (constraints && matches!(i, Item::Constraint(_)))
            });
        self.auto_added = kept;
        if gone.is_empty() {
            return Ok(());
        }
        self.edit_active_sketch(SketchOp::Delete(gone)).map(|_| ())
    }

    /// The dimensions selected in the sketch being edited.
    pub fn selected_dimensions(&self) -> Vec<Id<gc_sketch::Dimension>> {
        self.selection
            .iter()
            .filter_map(|i| if let Item::Dimension(d) = i { Some(*d) } else { None })
            .collect()
    }

    /// Whether Flip Direction applies: dimensions are selected and every one of them has a direction
    /// (Inventor shows the menu entry only then).
    pub fn can_flip_selected(&self) -> bool {
        let Some((_, sketch)) = self.active_sketch() else { return false };
        let dims = self.selected_dimensions();
        !dims.is_empty()
            && dims
                .iter()
                .all(|d| sketch.dimensions.get(d).is_some_and(|d| gc_doc::flipped(&d.kind).is_some()))
    }

    /// Flip Direction on the selected dimensions (one undo step; refused, with the reason in the status bar,
    /// when the sketch cannot take it).
    pub fn flip_selected_dimensions(&mut self) -> Result<(), EditError> {
        let dims = self.selected_dimensions();
        self.edit_active_sketch(SketchOp::FlipDimensions(dims)).map(|_| ())
    }

    /// The Auto Dimension dialog's Dimensions Required: the freedom left in the sketch being edited.
    pub fn dimensions_required(&self) -> usize {
        self.report().map_or(0, |r| r.dof)
    }

    /// The 2D Chamfer dialog: the type with its sizes, and Create Dimensions.
    pub fn chamfer_options(&self) -> (gc_doc::ChamferKind, bool) {
        self.tool.chamfer_options()
    }

    /// Sets the 2D Chamfer dialog's values (kept for the next chamfers, as Inventor does).
    pub fn set_chamfer_options(&mut self, kind: gc_doc::ChamferKind, dimensions: bool) {
        self.tool.set_chamfer_options(kind, dimensions);
    }

    /// Applies an edit to the sketch being edited (the Mirror and Pattern dialogs). One undo step.
    pub fn edit_active_sketch(&mut self, op: SketchOp) -> Result<Vec<Created>, EditError> {
        if self.active.is_none() {
            return Err(gc_sketch::SketchError::MissingItem.into());
        }
        self.sketch_op(op)
    }

    /// The points selected in the sketch being edited, with the centres of selected circles and arcs (an
    /// axis for a circular pattern).
    pub fn selected_points(&self) -> Vec<Id<gc_sketch::SkPoint>> {
        let sketch = self.active_sketch().map(|(_, s)| s);
        self.selection
            .iter()
            .filter_map(|i| match i {
                gc_sketch::Item::Point(p) => Some(*p),
                gc_sketch::Item::Curve(c) => sketch.and_then(|s| s.round(*c)).map(|(centre, _)| centre),
                _ => None,
            })
            .collect()
    }

    /// The rectangular or circular pattern a selected curve belongs to (an original or a copy), for Edit
    /// Pattern, Delete Pattern and Suppress.
    pub fn selected_pattern(&self) -> Option<gc_sketch::SketchPattern> {
        let (_, sketch) = self.active_sketch()?;
        self.selected_curves()
            .into_iter()
            .find_map(|c| sketch.pattern_of(c))
            .and_then(|i| sketch.patterns.get(i).cloned())
    }

    /// The points selected on their own (not the centres of selected circles).
    pub fn selected_points_only(&self) -> Vec<Id<gc_sketch::SkPoint>> {
        self.selection.iter().filter_map(|i| if let Item::Point(p) = i { Some(*p) } else { None }).collect()
    }

    /// The curves selected in the sketch being edited.
    pub fn selected_curves(&self) -> Vec<Id<Curve>> {
        self.selection
            .iter()
            .filter_map(|i| if let gc_sketch::Item::Curve(c) = i { Some(*c) } else { None })
            .collect()
    }

    fn sketch_op(&mut self, op: SketchOp) -> Result<Vec<Created>, EditError> {
        let Some(sketch) = self.active else { return Ok(Vec::new()) };
        self.execute(&Command::Sketch { sketch, op })
    }

    /// Snap to grid on or off (Sketch tab of Application Options; the spacing is the document's Snap Spacing).
    pub fn set_snap_to_grid(&mut self, on: bool) {
        self.snap_to_grid = on;
    }

    /// True when Snap to grid is on.
    pub fn snap_to_grid(&self) -> bool {
        self.snap_to_grid
    }

    /// A tool event as the settings shape it: with constraint inference off (Document Settings), drawing
    /// tools neither snap nor infer, as with Ctrl held; with Snap to grid on, an unsnapped cursor position
    /// goes to the nearest grid point of the document's snap spacing.
    fn with_settings(&self, event: ToolEvent) -> ToolEvent {
        let settings = self.doc.settings();
        let drawing = self.tool.kind() != ToolKind::Select;
        let infer_off = drawing && !settings.infer_constraints;
        let grid = |pos: [f64; 2], free: bool| -> [f64; 2] {
            if !(self.snap_to_grid && drawing) || free {
                return pos;
            }
            let [sx, sy] = settings.snap_spacing;
            [(pos[0] / sx).round() * sx, (pos[1] / sy).round() * sy]
        };
        match event {
            ToolEvent::Move { pos, px, suppress_snap } => ToolEvent::Move {
                pos: grid(pos, suppress_snap),
                px,
                suppress_snap: suppress_snap || infer_off,
            },
            ToolEvent::Click { pos, px, modifier } if drawing => {
                ToolEvent::Click { pos: grid(pos, modifier), px, modifier: modifier || infer_off }
            }
            other => other,
        }
    }

    /// Feeds an input event to the current tool and carries out what the tool asks for.
    pub fn tool_event(&mut self, event: ToolEvent) {
        // Escape first drops a 3D line in progress.
        if matches!(event, ToolEvent::Key(gc_tools::Key::Escape)) && self.spatial.drop_line() {
            return;
        }
        let event = self.with_settings(event);
        let Some(sketch) = self.active.and_then(|id| self.doc.get(id)) else { return };
        // A typed box is evaluated here, with the document's parameters (Inventor: a value, a unit, an
        // expression or a parameter name).
        let typed = match &event {
            ToolEvent::Hud { field, value: Some(text) } => Some((*field, text.clone())),
            _ => None,
        };
        let outputs = self.tool.handle(sketch, event);
        if let Some((field, text)) = typed {
            let unit = match field {
                gc_tools::HudField::Angle => gc_doc::Unit::Deg,
                gc_tools::HudField::Scale => gc_doc::Unit::Scalar,
                gc_tools::HudField::Length
                | gc_tools::HudField::Radius
                | gc_tools::HudField::Diameter
                | gc_tools::HudField::Width
                | gc_tools::HudField::Height
                | gc_tools::HudField::Distance
                | gc_tools::HudField::X
                | gc_tools::HudField::Y => gc_doc::Unit::Mm,
            };
            self.tool.set_hud_value(field, gc_doc::evaluate_value(&self.doc, &text, unit));
        }
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
            ToolOutput::Select { item, .. } if self.pick_select(item) => {}
            ToolOutput::Select { item, extend } => match (item, extend) {
                (Some(item), true) => {
                    self.picked.retain(|p| *p != item);
                    if !self.selection.remove(&item) {
                        self.selection.insert(item);
                        self.picked.push(item);
                    }
                }
                (Some(item), false) => {
                    self.selection = BTreeSet::from([item]);
                    self.picked = vec![item];
                }
                (None, true) => {}
                (None, false) => self.selection.clear(),
            },
            // Picking for a sketch dialog neither drags nor deletes what it picks.
            ToolOutput::BeginDrag { .. } | ToolOutput::DeleteSelection if self.pick_target.is_some() => {}
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
            ToolOutput::Stretch(by) => {
                let (curves, points) = (self.selected_curves(), self.selected_points_only());
                if curves.is_empty() && points.is_empty() {
                    self.last_error = Some(gc_sketch::SketchError::InvalidValue.into());
                    return;
                }
                // A refusal shows in the status bar.
                drop(self.sketch_op(SketchOp::Stretch { curves, points, by, relax: self.transform_relax }));
            }
            ToolOutput::Transform(motion) => {
                let curves = self.selected_curves();
                if curves.is_empty() {
                    self.last_error = Some(gc_sketch::SketchError::InvalidValue.into());
                    return;
                }
                let copy = self.tool.kind() == ToolKind::Copy || self.transform_copy;
                let op = SketchOp::Transform { curves, motion, copy, relax: self.transform_relax };
                // A refusal (held by constraints) shows in the status bar. The selection stays, for more
                // copies of it.
                drop(self.sketch_op(op));
            }
        }
    }

    /// Commits the drag as one command, or as nothing at all when the geometry did not move.
    /// Starts dragging a point of the active 3D sketch with the cursor at `cursor` (sketch-space
    /// position: the viewport turns the mouse ray into a point on a screen-parallel plane or a locked
    /// axis). Returns false when the point cannot move (fully constrained, or not a 3D sketch point).
    pub fn spatial_drag_start(&mut self, point: Id<SkPoint>, cursor: [f64; 3]) -> bool {
        self.drag = self.active_sketch().and_then(|(_, sketch)| sketch.begin_drag_3d(point, cursor).ok());
        self.drag.is_some()
    }

    /// Moves the cursor of a 3D drag. The preview follows ([`Session::point_pos`]); a frame the
    /// constraints refuse keeps the previous one.
    pub fn spatial_drag_move(&mut self, cursor: [f64; 3]) -> Option<FrameOutcome> {
        self.drag.as_mut().map(|drag| drag.update_3d(cursor))
    }

    /// Ends a 3D drag: the result is one undo step.
    pub fn spatial_drag_end(&mut self) {
        self.end_drag();
    }

    /// Drops a 3D drag without changing the document.
    pub fn spatial_drag_cancel(&mut self) {
        self.drag = None;
    }

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

    /// The dimension edit box's `name = value`: adds the user parameter `name` with the expression `value`
    /// (in degrees for an angle dimension, millimetres otherwise) and makes the dimension use it, as one
    /// undo step. Refused whole (nothing added) when either part is refused.
    pub fn edit_dimension_with_param(
        &mut self,
        id: Id<Dimension>,
        name: &str,
        value: &str,
    ) -> Result<(), EditError> {
        let Some(sketch) = self.active else { return Err(EditError::BadValue) };
        let kind = self
            .doc
            .get(sketch)
            .and_then(|s| s.dimensions.get(&id))
            .map(|d| d.kind)
            .ok_or(EditError::BadValue)?;
        let angle = matches!(kind, gc_sketch::DimKind::Angle { .. });
        let name = name.trim().to_owned();
        let commands = vec![
            Command::AddParam { name: name.clone(), source: value.to_owned(), angle },
            Command::Sketch { sketch, op: SketchOp::SetDimension { id, source: name } },
        ];
        self.execute(&Command::Sequence(commands)).map(|_| ())
    }
}
