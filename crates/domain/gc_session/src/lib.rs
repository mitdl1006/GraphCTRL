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
mod cell;
mod design;
pub mod library;
mod measure;
mod project;
mod spatial;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub use assembly::{AssemblyReader, Imported, Importer, ShownAssemblyPart};
pub use cell::{
    CellItem, CompareRow, Ghost, LoadedRobot, MeshRef, PADDING, PLANNERS, PlanStatus, PlannedPath, Reference,
    Resources, Shown, ShownPart,
};
pub use design::WorkShape;
pub use gc_collide::{Closest, Interference};
use gc_core::{Id, Revision};
use gc_doc::{Applied, Command, Created, Document, EditError, History, Sketch, SketchOp, apply};
pub use gc_motion::{Guarantee, LinkPair, PairReason, StateProblem};
pub use gc_persist::PersistError;
use gc_sketch::{Curve, DimKind, Dimension, FrameOutcome, Item, SkPoint, SketchDrag, SketchReport};
use gc_tools::{Preview, Prompt, Tool, ToolEvent, ToolKind, ToolOutput};
pub use measure::{
    Feature, FeaturePair, InterferenceRow, ItemMeasure, PairMeasure, angle_at, measure_features,
};

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
    drag: Option<SketchDrag>,
    report: Option<SketchReport>,
    request: Option<ValueRequest>,
    offer: Option<ReferenceOffer>,
    last_error: Option<EditError>,
    path: Option<PathBuf>,
    saved_revision: Revision,
    /// True for work recovered from an automatic save until it is saved.
    recovered: bool,
    cell: cell::CellState,
    spatial: spatial::SpatialState,
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
            construction: false,
            selection: BTreeSet::new(),
            picked: Vec::new(),
            drag: None,
            report: None,
            request: None,
            offer: None,
            last_error: None,
            path: None,
            saved_revision: Revision::default(),
            recovered: false,
            cell: cell::CellState::default(),
            spatial: spatial::SpatialState::default(),
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
        self.recovered || self.doc.revision() != self.saved_revision
    }

    fn replace_document(&mut self, doc: Document, path: Option<PathBuf>) {
        self.recovered = false;
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
        Ok(loaded.needs_repair)
    }

    /// Takes up work from an automatic save: the document as it was, belonging to `project` (saving goes
    /// there), and marked as not saved.
    pub fn open_recovered(&mut self, autosave: &Path, project: Option<&Path>) -> Result<(), PersistError> {
        let loaded = gc_persist::load(autosave)?;
        self.replace_document(loaded.doc, project.map(Path::to_path_buf));
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
        self.tool.prompt()
    }

    /// What the tool draws on top of the sketch.
    pub fn preview(&self) -> Preview {
        match self.active_sketch() {
            Some((_, sketch)) => self.tool.preview(sketch),
            None => Preview::default(),
        }
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
        self.tool = Tool::new(self.tool.kind());
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

    /// The freedom a component has left under the assembly relationships (the DOF symbol): total and pure
    /// translations. Grounded: none; unconstrained: six.
    pub fn component_dof(&self, item: gc_core::AnyId) -> Option<gc_doc::Dof> {
        self.doc.component_dof(item)
    }

    /// The constraint status dot of a component in the browser: full (black), under (hollow), unknown (dash).
    pub fn component_status(&self, item: gc_core::AnyId) -> Option<gc_doc::ComponentStatus> {
        self.doc.component_status(item)
    }

    /// The status of an assembly relationship: full, under-constrained or sick.
    pub fn relationship_status(&self, id: Id<gc_doc::Relationship>) -> Option<gc_doc::RelationStatus> {
        self.doc.relationship_status(id)
    }

    /// A component's origin folder in the world: centre point, axes and planes (assembly P1-2).
    pub fn component_origin(&self, occurrence: gc_doc::Occurrence) -> Option<gc_doc::ComponentOrigin> {
        self.doc.component_origin(occurrence)
    }

    /// Creates a 2D sketch on the plane of a triangle of an imported part (a picked face) and starts
    /// editing it: origin at the triangle's centre, normal out of its front side, X along its first
    /// edge. The sketch follows the assembly when it is moved.
    pub fn create_sketch_on_face(
        &mut self,
        assembly: Id<gc_doc::Assembly>,
        part: usize,
        mesh: usize,
        triangle: usize,
    ) -> Result<Id<Sketch>, EditError> {
        let source = self.face_source(assembly, part, mesh, triangle).ok_or(EditError::BadValue)?;
        self.create_sketch_on(source)
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
        let created = self.execute(&Command::CreateSketchOn { name, source })?;
        match created.first() {
            Some(Created::Sketch(id)) => {
                self.enter_sketch(*id);
                Ok(*id)
            }
            _ => Err(gc_sketch::SketchError::MissingItem.into()),
        }
    }

    /// Creates a 3D sketch in the world frame and starts editing it.
    pub fn create_sketch_3d(&mut self) -> Result<Id<Sketch>, EditError> {
        let name = format!("Sketch{}", self.doc.sketches().len() + 1);
        let frame = self.doc.world().ok_or(gc_sketch::SketchError::MissingItem)?;
        let created = self.execute(&Command::CreateSketch { name, frame, spatial: true })?;
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
        // Escape first drops a 3D line in progress.
        if matches!(event, ToolEvent::Key(gc_tools::Key::Escape)) && self.spatial.drop_line() {
            return;
        }
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
}
