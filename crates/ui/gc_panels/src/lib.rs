//! The panels around the viewport: model browser, properties and status bar.

mod cell;
mod pose;
mod role;
mod schedule;
mod work;

use egui::{RichText, Ui};
use gc_core::Id;
use gc_doc::{FrameKind, Sketch};
use gc_i18n::{Lang, Msg, tr};
use gc_session::Session;
use gc_sketch::{CurveGeom, Item, SketchError};
use gc_tools::Prompt;
use gc_ui::Icon;
use gc_ui::colors;
use gc_ui::panel::TreeRow;

pub use cell::{CellUi, cell, problem_text, status_text};
pub use pose::{PoseCalc, RotationInput, pose_calculator};
pub use role::role_properties;
pub use schedule::{ScheduleRequest, ScheduleUi, schedule_panel};
pub use work::{WorkItem, WorkRequest, WorkUi, robot_tool_kit, work_browser, work_properties};

/// What the user asked for in the browser.
#[derive(Clone, Debug, PartialEq)]
pub enum BrowserAction {
    /// Edit this sketch.
    EditSketch(Id<Sketch>),
    /// Apply this edit (delete a cell item, add or change a parameter).
    Edit(Box<gc_doc::Command>),
    /// Show or hide a body or a robot.
    Show(gc_session::Shown, bool),
    /// Select an item of the cell, or nothing.
    Select(Option<gc_session::CellItem>),
    /// Plan a path.
    Plan(gc_core::Id<gc_doc::MotionPath>),
    /// Show or hide elements of a component's origin folder.
    OriginVisible(gc_doc::Occurrence, Vec<gc_doc::OriginElement>, bool),
    /// Select an origin element in the browser (the view follows), or nothing.
    OriginSelect(Option<(gc_doc::Occurrence, gc_doc::OriginElement)>),
    /// Start a sketch on an origin plane of a component.
    SketchOnComponent(gc_doc::Occurrence, gc_doc::OriginPlane),
    /// Drive an assembly relationship (the Drive dialog).
    DriveRelationship(gc_core::Id<gc_doc::Relationship>),
    /// Make a view representation the active one; the app restores the camera and section it stored.
    ActivateView(usize),
    /// Create a view representation with this name from the current view (the app stores the camera
    /// and section).
    NewView(String),
    /// Lock a joint where it stands, or free it.
    JointLock(gc_core::Id<gc_doc::Relationship>, bool),
    /// Protect a joint's freedom, or not.
    JointProtect(gc_core::Id<gc_doc::Relationship>, bool),
    /// Open the joint's Limits dialog.
    JointLimits(gc_core::Id<gc_doc::Relationship>),
    /// Edit a work feature (double-click, or its menu).
    EditWorkFeature(Id<gc_doc::WorkFeature>),
    /// Edit a component or subassembly in place (double-click in the browser).
    EditComponent(gc_doc::TreeKey),
    /// Demote, move, dissolve or rename subassemblies.
    Subassembly(gc_doc::SubassemblyOp),
    /// Put a component into the contact set, or take it out.
    ContactSet(gc_core::AnyId, bool),
    /// Suppress a relationship (or bring it back) and solve again.
    SuppressRelationship(gc_core::Id<gc_doc::Relationship>, bool),
    /// Select a sketch role (a waypoint or a path of a sketch), or none; the view marks it.
    SelectRole(Option<Id<gc_doc::SketchRole>>),
    /// Change the work model (object types, objects, state variables, tasks).
    Work(gc_doc::WorkOp),
    /// Change the browser's user-defined folders.
    Folder(gc_doc::FolderOp),
    /// Isolate these components (Inventor: Isolate; a subassembly is given as its members).
    Isolate(Vec<gc_doc::Occurrence>),
    /// Undo Isolate: show again what Isolate hid.
    UndoIsolate,
    /// Open the iProperties dialog of a component.
    Properties(gc_doc::Occurrence),
    /// Zoom the graphics window to a component (Find in Window).
    FindInWindow(gc_core::AnyId),
}

/// What is typed into the parameter editor of the browser.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParamsUi {
    /// The name being typed.
    pub name: String,
    /// The expression being typed.
    pub source: String,
    /// The browser item being renamed, and the name typed so far.
    pub rename: Option<(gc_core::AnyId, String)>,
    /// The rename has just begun: the field takes the keyboard focus.
    pub rename_fresh: bool,
    /// The origin element selected in the view or here.
    pub origin_selected: Option<(gc_doc::Occurrence, gc_doc::OriginElement)>,
    /// The component statuses of this frame: one query for the whole tree, not one per row.
    pub statuses: std::collections::BTreeMap<gc_core::AnyId, gc_doc::ComponentStatus>,
    /// A name being asked for: the new subassembly of Demote, or the new name of a subassembly.
    pub group_name: Option<GroupName>,
    /// The Find box: only rows whose name contains this (case does not matter) are listed.
    pub find: String,
    /// Display Preferences of the browser menu (Inventor 2026 help, Working with the Assembly Browser,
    /// GUID-5F975252-BDB4-488C-A2C5-8B15C7D3C641). Hide Work Features also hides the Origin folders.
    pub hide_work_features: bool,
    /// Hide Fully Constrained: only components and relationships with issues are listed.
    pub hide_fully_constrained: bool,
    /// Show Constraint State is on unless this is set (so that the default state is the usual one).
    pub hide_constraint_state: bool,
    /// Show Children Only (Display Preferences): while a component is edited in place, only it, what is
    /// inside it and the rows it hangs under are listed.
    pub show_children_only: bool,
    /// Filter: only suppressed components and relationships (Find: "suppressed components or features").
    pub only_suppressed: bool,
    /// Hide Warnings: warnings attached to relationships (the sick marker) are not shown; failures are.
    pub hide_warnings: bool,
    /// Edit Values in Browser is on unless this is set.
    pub no_edit_values: bool,
    /// The component row the pointer is over this frame: the view shows its bounding box (Inventor 2026
    /// help, assembly parity notes: hovering a browser node shows its bounding box in the graphics).
    pub hovered: Option<gc_core::AnyId>,
    /// What the work node of the browser shows selected, and its rename.
    pub work: work::WorkUi,
    /// The rows marked with Ctrl+click (for moving several into a folder or subassembly at once).
    pub marked: Vec<gc_doc::TreeKey>,
    /// The component rows in the order they were drawn last frame, and the row a Shift+click extends from.
    pub row_order: Vec<gc_doc::TreeKey>,
    drawn_rows: Vec<gc_doc::TreeKey>,
    anchor: Option<gc_doc::TreeKey>,
    /// The sketch role selected in the browser.
    pub role: Option<Id<gc_doc::SketchRole>>,
    /// The relationship selected in the browser, the value typed for it and which one that was loaded for.
    pub relationship: Option<gc_core::Id<gc_doc::Relationship>>,
    /// The text of the browser edit box.
    pub relationship_value: String,
}

/// What a name typed in the browser is for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GroupName {
    /// Demote these rows into a new subassembly.
    Demote(Vec<gc_doc::TreeKey>, String),
    /// Rename a subassembly.
    Rename(gc_doc::SubassemblyId, String),
    /// Add to New Folder: a user-defined folder of the browser holding these rows.
    NewFolder(Vec<gc_doc::TreeKey>, String),
    /// Rename a user-defined folder.
    RenameFolder(u32, String),
}

/// The named parameters: a list, and one editor that adds a parameter or, when the name exists, changes
/// it. Clicking a parameter loads it into the editor.
fn params(ui: &mut Ui, session: &Session, lang: Lang, state: &mut ParamsUi) -> Option<gc_doc::Command> {
    let mut command = None;
    egui::CollapsingHeader::new(tr!(lang, BrowserParams)).default_open(true).show(ui, |ui| {
        let doc = session.doc();
        ui.spacing_mut().item_spacing.y = 1.0;
        for (id, param) in doc.params().iter() {
            let text = format!("{} = {}", param.name, param.source);
            let delete = tr!(lang, CmdDeleteItem, param.name);
            let row = TreeRow {
                icon: Icon::Text("="),
                text: &text,
                selected: state.name == param.name,
                visible: None,
                delete: Some(&delete),
                edit: None,
                focus: false,
                dim: false,
            };
            let result = gc_ui::panel::tree_row(ui, row);
            if result.row.clicked() {
                state.name.clone_from(&param.name);
                state.source.clone_from(&param.source);
            }
            if result.deleted {
                command = Some(gc_doc::Command::DeleteParam { id });
            }
        }
        let existing = doc.params().iter().find(|(_, p)| p.name == state.name).map(|(id, _)| id);
        ui.horizontal(|ui| {
            let name = egui::TextEdit::singleline(&mut state.name)
                .hint_text(tr!(lang, ParamName))
                .desired_width(52.0);
            ui.add(name)
                .widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "parameter name"));
            let source = egui::TextEdit::singleline(&mut state.source)
                .hint_text(tr!(lang, ParamValue))
                .desired_width(78.0);
            let field = ui.add(source);
            field.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "parameter value"));
            let entered = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let label = if existing.is_some() { tr!(lang, ParamApply) } else { tr!(lang, ParamAdd) };
            let ready = !state.name.is_empty() && !state.source.is_empty();
            if (ui.add_enabled(ready, egui::Button::new(label)).clicked() || entered) && ready {
                command = Some(match existing {
                    Some(id) => gc_doc::Command::SetParam { id, source: state.source.clone() },
                    None => {
                        // A bare number is millimetres unless the expression speaks of angles.
                        let angle = ["deg", "rad", "°"].iter().any(|unit| state.source.contains(unit));
                        gc_doc::Command::AddParam {
                            name: state.name.clone(),
                            source: state.source.clone(),
                            angle,
                        }
                    }
                });
            }
        });
    });
    command
}

/// The value of a constraint as the browser edit box shows it: millimetres, or degrees for an angle.
fn relation_value_text(kind: &gc_doc::RelationKind) -> Option<String> {
    use gc_doc::RelationKind;
    let show = |v: f64| format!("{}", (v * 1000.0).round() / 1000.0);
    match kind {
        RelationKind::Mate { offset, .. }
        | RelationKind::Flush { offset }
        | RelationKind::Insert { offset, .. }
        | RelationKind::Tangent { offset, .. } => Some(show(offset * 1000.0)),
        RelationKind::Angle { angle, .. } => Some(show(angle.to_degrees())),
        _ => None,
    }
}

/// The unit of that value, when the constraint has one.
fn relation_unit_name(kind: &gc_doc::RelationKind) -> Option<&'static str> {
    use gc_doc::RelationKind;
    match kind {
        RelationKind::Mate { .. }
        | RelationKind::Flush { .. }
        | RelationKind::Insert { .. }
        | RelationKind::Tangent { .. } => Some("mm"),
        RelationKind::Angle { .. } => Some("°"),
        _ => None,
    }
}

/// The constraint with the value typed in the browser edit box (millimetres or degrees), or `None` when the
/// text is not a number or the constraint has no value.
fn relation_with_value(relationship: &gc_doc::Relationship, text: &str) -> Option<gc_doc::Relationship> {
    use gc_doc::RelationKind;
    let typed = text.trim().parse::<f64>().ok().filter(|v| v.is_finite())?;
    let mut changed = relationship.clone();
    changed.kind = match changed.kind {
        RelationKind::Mate { sense, .. } => RelationKind::Mate { offset: typed / 1000.0, sense },
        RelationKind::Flush { .. } => RelationKind::Flush { offset: typed / 1000.0 },
        RelationKind::Insert { aligned, .. } => RelationKind::Insert { offset: typed / 1000.0, aligned },
        RelationKind::Tangent { inside, .. } => RelationKind::Tangent { offset: typed / 1000.0, inside },
        RelationKind::Angle { solution, .. } => RelationKind::Angle { angle: typed.to_radians(), solution },
        _ => return None,
    };
    Some(changed)
}

/// Whether `name` is listed with the Find text `find` (case does not matter; an empty text lists everything).
fn found(find: &str, name: &str) -> bool {
    find.trim().is_empty() || name.to_lowercase().contains(&find.trim().to_lowercase())
}

/// Whether a row of the component tree is listed: its name matches, or something below it does.
fn row_found(rows: &[gc_doc::TreeRow], row: &gc_doc::TreeRow, find: &str) -> bool {
    found(find, &row.name)
        || rows.iter().filter(|r| r.parent == Some(row.key)).any(|child| row_found(rows, child, find))
}

/// The model browser: origin geometry and sketches. Double-click a sketch to edit it.
pub fn browser(ui: &mut Ui, session: &Session, lang: Lang, state: &mut ParamsUi) -> Option<BrowserAction> {
    let mut action = None;
    state.statuses = session.component_statuses();
    state.hovered = None;
    gc_ui::panel::header(ui, tr!(lang, PanelBrowser));
    // Find (Inventor 2026 help, Working with the Assembly Browser, GUID-5F975252-BDB4-488C-A2C5-8B15C7D3C641:
    // "Find opens a search box enabling you to locate objects matching your search criteria"): only the
    // components, sketches and relationships whose name contains the text are listed.
    ui.horizontal(|ui| {
        let label = ui.label(tr!(lang, BrowserFind));
        ui.add(
            egui::TextEdit::singleline(&mut state.find)
                .hint_text(tr!(lang, BrowserFindHint))
                .desired_width(ui.available_width() - 34.0),
        )
        .labelled_by(label.id);
        // The browser menu: Display Preferences and Edit Values in Browser.
        let menu = ui.menu_button("…", |ui| {
            let mut show = !state.hide_constraint_state;
            let mut edit = !state.no_edit_values;
            ui.checkbox(&mut state.hide_work_features, tr!(lang, BrowserHideWork));
            ui.checkbox(&mut state.hide_fully_constrained, tr!(lang, BrowserHideConstrained));
            if session.is_isolated() && ui.button(tr!(lang, CmdUndoIsolate)).clicked() {
                action = Some(BrowserAction::UndoIsolate);
                ui.close();
            }
            ui.checkbox(&mut state.show_children_only, tr!(lang, BrowserChildrenOnly));
            ui.checkbox(&mut state.only_suppressed, tr!(lang, BrowserOnlySuppressed));
            ui.checkbox(&mut state.hide_warnings, tr!(lang, BrowserHideWarnings));
            ui.checkbox(&mut show, tr!(lang, BrowserShowState));
            ui.checkbox(&mut edit, tr!(lang, BrowserEditValues));
            state.hide_constraint_state = !show;
            state.no_edit_values = !edit;
        });
        menu.response
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tr!(lang, BrowserMenu)));
    });
    group_name_row(ui, lang, state, &mut action);
    // F2 renames the selected node when the mouse is over the browser (over the 3D view, F2 pans).
    let hovering = ui.rect_contains_pointer(ui.max_rect());
    if hovering && ui.input(|i| i.key_pressed(egui::Key::F2)) {
        let target = session
            .selected_item()
            .map(cell_any)
            .or_else(|| session.active_sketch().map(|(id, _)| id.erase()));
        if let Some(target) = target
            && let Some(name) = name_of(session, target)
        {
            state.rename = Some((target, name));
            state.rename_fresh = true;
        }
    }
    // The document's tree, as in an assembly browser: the document, its relationships and
    // representations, the origin, and the components (bodies and robots, "name:1" each) with an origin
    // folder of their own. What is specific to the robot cell follows in folders of its own.
    let doc = session.doc();
    let selected = session.selected_item();
    use gc_session::CellItem;

    let document = session.file_path().and_then(|p| p.file_stem().map(|n| n.to_string_lossy().into_owned()));
    let document = document.unwrap_or_else(|| tr!(lang, BrowserDocument).to_owned());
    egui::CollapsingHeader::new(RichText::new(document).strong())
        .id_salt("gc_browser_document")
        .default_open(true)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            egui::CollapsingHeader::new(tr!(lang, BrowserRelationships)).default_open(true).show(ui, |ui| {
                for (id, relationship) in session.relationships() {
                    if !found(&state.find, &relationship.name) {
                        continue;
                    }
                    if state.hide_fully_constrained
                        && session.relationship_status(id) == Some(gc_doc::RelationStatus::Full)
                    {
                        continue;
                    }
                    let suppressed = session.doc().is_inactive(id.erase());
                    if state.only_suppressed && !suppressed {
                        continue;
                    }
                    let is_joint = gc_doc::is_joint_kind(&relationship.kind);
                    let condition = session.joint_condition(id);
                    let locked = condition.locked.is_some();
                    let delete = tr!(lang, CmdDeleteItem, relationship.name);
                    // A joint with Limits is marked +/- (Inventor 2026 help, GUID-21DC3336-5C51-42C1-90FB-4299CD66E0C6).
                    let limited = is_joint && {
                        let (angular, linear) = session.joint_limits(id);
                        angular.is_some() || linear.is_some()
                    };
                    let label = if limited {
                        format!("{} +/-", relationship.name)
                    } else {
                        relationship.name.clone()
                    };
                    let row = TreeRow {
                        // A locked joint shows a padlock.
                        icon: if locked { Icon::Lock } else { Icon::Path },
                        text: &label,
                        selected: state.relationship == Some(id),
                        visible: None,
                        delete: Some(&delete),
                        edit: None,
                        focus: false,
                        dim: suppressed,
                    };
                    let result = gc_ui::panel::tree_row(ui, row);
                    // Clicking a constraint selects it; with Edit Values in Browser its value can be typed.
                    if result.row.clicked() && state.relationship != Some(id) {
                        state.relationship = Some(id);
                        state.relationship_value =
                            relation_value_text(&relationship.kind).unwrap_or_default();
                    }
                    if let Some(status) = session
                        .relationship_status(id)
                        .filter(|_| !suppressed && !state.hide_constraint_state)
                    {
                        use gc_doc::RelationStatus;
                        status_dot(
                            ui,
                            result.row.rect,
                            Some(status == RelationStatus::Full),
                            status == RelationStatus::Sick && !state.hide_warnings,
                        );
                    }
                    // A rigid joint has nothing to drive.
                    let drivable = !matches!(relationship.kind, gc_doc::RelationKind::Rigid { .. });
                    result.row.context_menu(|ui| {
                        if ui.add_enabled(drivable, egui::Button::new(tr!(lang, MenuDrive))).clicked() {
                            action = Some(BrowserAction::DriveRelationship(id));
                            ui.close();
                        }
                        // Lock and Protect are for joints only (Inventor 2026 help, To Define and Manage Joint
                        // Relationships, GUID-21DC3336-5C51-42C1-90FB-4299CD66E0C6).
                        if is_joint {
                            if ui.checkbox(&mut { locked }, tr!(lang, JointLockItem)).clicked() {
                                action = Some(BrowserAction::JointLock(id, !locked));
                                ui.close();
                            }
                            if ui.button(tr!(lang, JointLimitsItem)).clicked() {
                                action = Some(BrowserAction::JointLimits(id));
                                ui.close();
                            }
                            if ui
                                .checkbox(&mut { condition.protected }, tr!(lang, JointProtectItem))
                                .clicked()
                            {
                                action = Some(BrowserAction::JointProtect(id, !condition.protected));
                                ui.close();
                            }
                        }
                        let text = if suppressed { tr!(lang, CmdUnsuppress) } else { tr!(lang, CmdSuppress) };
                        if ui.button(text).clicked() {
                            action = Some(BrowserAction::SuppressRelationship(id, !suppressed));
                            ui.close();
                        }
                        if ui.button(delete.as_str()).clicked() {
                            action = Some(BrowserAction::Edit(Box::new(gc_doc::Command::Cell(
                                gc_doc::CellOp::DeleteRelationship(id),
                            ))));
                            ui.close();
                        }
                    });
                    if state.relationship == Some(id)
                        && !state.no_edit_values
                        && let Some(unit) = relation_unit_name(&relationship.kind)
                    {
                        ui.horizontal(|ui| {
                            ui.add_space(24.0);
                            let field = ui.add(
                                egui::TextEdit::singleline(&mut state.relationship_value).desired_width(80.0),
                            );
                            field.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::Role::TextInput,
                                    true,
                                    tr!(lang, BrowserRelationValue),
                                )
                            });
                            ui.label(unit);
                            if field.lost_focus()
                                && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                && let Some(changed) =
                                    relation_with_value(relationship, &state.relationship_value)
                            {
                                action = Some(BrowserAction::Edit(Box::new(gc_doc::Command::Cell(
                                    gc_doc::CellOp::SetRelationship { id, relationship: changed },
                                ))));
                            }
                        });
                    }
                    if result.deleted {
                        action = Some(BrowserAction::Edit(Box::new(gc_doc::Command::Cell(
                            gc_doc::CellOp::DeleteRelationship(id),
                        ))));
                    }
                }
            });
            let patterns = session.patterns();
            if !patterns.is_empty() {
                egui::CollapsingHeader::new(tr!(lang, BrowserPatterns)).default_open(true).show(ui, |ui| {
                    for (index, pattern) in patterns.iter().enumerate() {
                        let delete = tr!(lang, CmdDeleteItem, pattern.name);
                        let text = format!("{} ({})", pattern.name, pattern.layout.count());
                        let row = TreeRow {
                            icon: Icon::Frame,
                            text: &text,
                            selected: false,
                            visible: None,
                            delete: Some(&delete),
                            edit: None,
                            focus: false,
                            dim: false,
                        };
                        let result = gc_ui::panel::tree_row(ui, row);
                        let remove = gc_doc::CellOp::DeletePattern { index, keep_copies: false };
                        result.row.context_menu(|ui| {
                            if ui.button(delete.as_str()).clicked() {
                                action = Some(BrowserAction::Edit(Box::new(gc_doc::Command::Cell(
                                    remove.clone(),
                                ))));
                                ui.close();
                            }
                        });
                        if result.deleted {
                            action = Some(BrowserAction::Edit(Box::new(gc_doc::Command::Cell(remove))));
                        }
                    }
                });
            }
            representations(ui, session, lang, &mut action);
            if !state.hide_work_features {
                egui::CollapsingHeader::new(tr!(lang, BrowserOrigin)).default_open(false).show(ui, |ui| {
                    for (_, frame) in
                        session.doc().frames().iter().filter(|(_, f)| f.kind == FrameKind::OriginPlane)
                    {
                        let row = TreeRow {
                            icon: Icon::Frame,
                            text: &frame.name,
                            selected: false,
                            visible: None,
                            delete: None,
                            edit: None,
                            focus: false,
                            dim: false,
                        };
                        gc_ui::panel::tree_row(ui, row);
                    }
                });
            }
            let rows = session.component_tree();
            state.drawn_rows.clear();
            component_rows(ui, session, lang, state, selected, &mut action, &rows, None);
            state.row_order = std::mem::take(&mut state.drawn_rows);
        });
    egui::CollapsingHeader::new(tr!(lang, BrowserSketches)).default_open(true).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        // The sketches of the top level; those of a component or subassembly hang below its row.
        for id in session.sketches_of(None) {
            sketch_row(ui, session, lang, state, &mut action, id);
        }
    });
    if !state.hide_work_features {
        work_features(ui, session, lang, &mut action);
    }
    if let Some(command) = params(ui, session, lang, state) {
        action = Some(BrowserAction::Edit(Box::new(command)));
    }
    if let Some(work) = work::work_browser(ui, session, lang, &mut state.work) {
        action = Some(work);
    }
    let row = |name: &str, item| (name.to_owned(), name.to_owned(), item);
    browser_section(
        ui,
        session,
        lang,
        state,
        selected,
        &mut action,
        Some(tr!(lang, BrowserFrames)),
        doc.work_frames().iter().map(|(id, f)| row(&f.name, CellItem::Frame(id))).collect(),
    );
    browser_section(
        ui,
        session,
        lang,
        state,
        selected,
        &mut action,
        Some(tr!(lang, BrowserWaypoints)),
        doc.waypoints().iter().map(|(id, w)| row(&w.name, CellItem::Waypoint(id))).collect(),
    );
    let paths = doc
        .paths()
        .iter()
        .map(|(id, p)| {
            let shown = format!("{} ({})", p.name, status_text(lang, session.plan_status(id)));
            (p.name.clone(), shown, CellItem::Path(id))
        })
        .collect();
    browser_section(ui, session, lang, state, selected, &mut action, Some(tr!(lang, BrowserPaths)), paths);
    action
}

/// The Representations folder: view representations and positional ones ([Primary] first). A click
/// activates one; the menu creates, activates and deletes. Inventor 2026 help: About View Representations
/// (GUID-9A49CDC1-4DC7-4462-81D7-E18B8339DE0F), To Create, Edit, or Copy Positional Representations
/// (GUID-34C28C35-45CA-4476-9336-30146F7C9471).
fn representations(ui: &mut Ui, session: &Session, lang: Lang, action: &mut Option<BrowserAction>) {
    use gc_doc::{CellOp, Command};
    let folder = session.representation_folder();
    let edit = |op: CellOp| Some(BrowserAction::Edit(Box::new(Command::Cell(op))));
    egui::CollapsingHeader::new(tr!(lang, BrowserRepresentations)).default_open(false).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        for (is_view, title, nodes) in
            [(true, tr!(lang, RepViews), &folder.views), (false, tr!(lang, RepPositions), &folder.positions)]
        {
            egui::CollapsingHeader::new(title).id_salt(("gc_rep_folder", is_view)).default_open(true).show(
                ui,
                |ui| {
                    for (index, node) in nodes.iter().enumerate() {
                        // [Primary] is the first position and has no index of its own.
                        let position = index.checked_sub(1);
                        let delete = tr!(lang, CmdDeleteItem, node.name);
                        let primary = !is_view && index == 0;
                        let text = if node.active { format!("● {}", node.name) } else { node.name.clone() };
                        let row = TreeRow {
                            icon: Icon::View,
                            text: &text,
                            selected: node.active,
                            visible: None,
                            delete: (!primary).then_some(delete.as_str()),
                            edit: None,
                            focus: false,
                            dim: false,
                        };
                        let result = gc_ui::panel::tree_row(ui, row);
                        let activate = if is_view {
                            BrowserAction::ActivateView(index)
                        } else {
                            BrowserAction::Edit(Box::new(Command::Cell(
                                CellOp::ActivatePositionalRepresentation(position),
                            )))
                        };
                        let remove = if is_view {
                            CellOp::DeleteRepresentation { view: Some(index), position: None }
                        } else {
                            CellOp::DeleteRepresentation { view: None, position }
                        };
                        if result.row.clicked() && !node.active {
                            *action = Some(activate.clone());
                        }
                        result.row.context_menu(|ui| {
                            if ui
                                .add_enabled(!node.active, egui::Button::new(tr!(lang, CmdActivate)))
                                .clicked()
                            {
                                *action = Some(activate.clone());
                                ui.close();
                            }
                            if ui.add_enabled(!primary, egui::Button::new(delete.as_str())).clicked() {
                                *action = edit(remove.clone());
                                ui.close();
                            }
                        });
                        if result.deleted {
                            *action = edit(remove);
                        }
                    }
                    // New: a view representation keeps what is shown now; a positional one starts as a
                    // copy of [Primary].
                    // [Primary] is not counted among the positions that were made.
                    let (word, count) = if is_view {
                        ("View", nodes.len())
                    } else {
                        ("Position", nodes.len().saturating_sub(1))
                    };
                    let label = tr!(lang, RepNew, title);
                    if ui.small_button(label).clicked() {
                        let name = format!("{word}:{}", count + 1);
                        *action = if is_view {
                            Some(BrowserAction::NewView(name))
                        } else {
                            edit(CellOp::AddPositionalRepresentation(name))
                        };
                    }
                },
            );
        }
    });
}

/// The work features of the browser: each with a visibility check box and a delete button.
fn work_features(ui: &mut Ui, session: &Session, lang: Lang, action: &mut Option<BrowserAction>) {
    use gc_doc::{CellOp, Command};
    let doc = session.doc();
    if doc.work_features().iter().next().is_none() {
        return;
    }
    egui::CollapsingHeader::new(tr!(lang, BrowserWorkFeatures)).default_open(true).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        for (id, feature) in doc.work_features().iter() {
            let mut visible = feature.visible;
            let delete = tr!(lang, CmdDeleteItem, feature.name);
            let show = tr!(lang, ShowItem, feature.name);
            let row = TreeRow {
                icon: Icon::Frame,
                text: &feature.name,
                selected: false,
                visible: Some((&mut visible, show.as_str())),
                delete: Some(&delete),
                edit: None,
                focus: false,
                dim: false,
            };
            let result = gc_ui::panel::tree_row(ui, row);
            if result.row.double_clicked() {
                *action = Some(BrowserAction::EditWorkFeature(id));
            }
            result.row.context_menu(|ui| {
                ui.label(RichText::new(&feature.name).strong());
                ui.separator();
                if ui.button(tr!(lang, CmdEditWorkFeature)).clicked() {
                    *action = Some(BrowserAction::EditWorkFeature(id));
                    ui.close();
                }
                let text = if feature.visible { tr!(lang, CmdHide) } else { tr!(lang, CmdShow) };
                if ui.button(text).clicked() {
                    let mut changed = feature.clone();
                    changed.visible = !feature.visible;
                    *action = Some(BrowserAction::Edit(Box::new(Command::Cell(CellOp::SetWorkFeature {
                        id,
                        feature: changed,
                    }))));
                    ui.close();
                }
                if ui.button(delete.as_str()).clicked() {
                    *action =
                        Some(BrowserAction::Edit(Box::new(Command::Cell(CellOp::DeleteWorkFeature(id)))));
                    ui.close();
                }
            });
            if result.toggled {
                let mut changed = feature.clone();
                changed.visible = visible;
                *action = Some(BrowserAction::Edit(Box::new(Command::Cell(CellOp::SetWorkFeature {
                    id,
                    feature: changed,
                }))));
            }
            if result.deleted {
                *action = Some(BrowserAction::Edit(Box::new(Command::Cell(CellOp::DeleteWorkFeature(id)))));
            }
        }
    });
}

/// One sketch of the browser: select by clicking, double-click to edit.
fn sketch_row(
    ui: &mut Ui,
    session: &Session,
    lang: Lang,
    state: &mut ParamsUi,
    action: &mut Option<BrowserAction>,
    id: Id<Sketch>,
) {
    let Some(sketch) = session.doc().get(id) else { return };
    if !found(&state.find, &sketch.name) {
        return;
    }
    let active = session.active_sketch().map(|(active, _)| active);
    let node = Node {
        icon: Icon::Sketch,
        name: &sketch.name,
        text: &sketch.name,
        any: id.erase(),
        selected: active == Some(id),
        delete: gc_doc::Command::DeleteSketch { id },
        suppressible: true,
    };
    let result = node_row(ui, session, lang, state, &node, action, |ui, action| {
        if ui.button(tr!(lang, CmdEditSketch)).clicked() {
            *action = Some(BrowserAction::EditSketch(id));
            ui.close();
        }
    });
    if result.double_clicked() {
        *action = Some(BrowserAction::EditSketch(id));
    }
    // The roles of the sketch's elements (waypoints and paths of the robot work) hang below it.
    for role_id in session.sketch_roles(id) {
        role_row(ui, session, lang, state, action, role_id);
    }
}

/// A role of a sketch element: a waypoint (a point) or a path (a chain of curves). It is renamed, suppressed and
/// deleted like any other item; selecting it marks it in the view.
fn role_row(
    ui: &mut Ui,
    session: &Session,
    lang: Lang,
    state: &mut ParamsUi,
    action: &mut Option<BrowserAction>,
    id: Id<gc_doc::SketchRole>,
) {
    let Some(role) = session.doc().get(id) else { return };
    if !found(&state.find, &role.name) {
        return;
    }
    let waypoint = matches!(role.kind, gc_doc::RoleKind::Waypoint { .. });
    let node = Node {
        icon: if waypoint { Icon::Waypoint } else { Icon::Path },
        name: &role.name,
        text: &role.name,
        any: id.erase(),
        selected: state.role == Some(id),
        delete: gc_doc::Command::Cell(gc_doc::CellOp::DeleteRole(id)),
        suppressible: true,
    };
    ui.indent(("gc_role", id), |ui| {
        let result = node_row(ui, session, lang, state, &node, action, |ui, action| {
            // A task that moves to this waypoint or follows this path.
            if ui.button(tr!(lang, WorkTaskFromRole)).clicked() {
                let step = match role.kind {
                    gc_doc::RoleKind::Waypoint { .. } => gc_doc::Step::MoveJ(gc_doc::MoveTarget::Role(id)),
                    gc_doc::RoleKind::Path { .. } => gc_doc::Step::FollowPath(id),
                };
                let taken: Vec<String> = session.doc().tasks().iter().map(|(_, t)| t.name.clone()).collect();
                let name = (taken.len() + 1..)
                    .map(|n| format!("{}{n}", tr!(lang, WorkTaskBase)))
                    .find(|n| !taken.contains(n))
                    .unwrap_or_default();
                *action = Some(BrowserAction::Work(gc_doc::WorkOp::AddTask(gc_doc::Task {
                    name,
                    location: gc_doc::Location::Sketch(role.sketch),
                    precondition: String::new(),
                    steps: vec![step],
                    effects: Vec::new(),
                    robots: gc_doc::RobotChoice::Any,
                    priority: 0,
                })));
                ui.close();
            }
        });
        if result.clicked() {
            state.role = Some(id);
            *action = Some(BrowserAction::SelectRole(Some(id)));
        }
    });
}

/// The components inside a tree row, at any depth (the row itself when it is a component).
fn members_of(rows: &[gc_doc::TreeRow], key: gc_doc::TreeKey) -> Vec<gc_doc::Occurrence> {
    let mut found = Vec::new();
    let mut stack = vec![key];
    while let Some(next) = stack.pop() {
        if let gc_doc::TreeKey::Component(occurrence) = next {
            found.push(occurrence);
        }
        stack.extend(rows.iter().filter(|r| r.parent == Some(next)).map(|r| r.key));
    }
    found
}

/// The components of the browser as the assembly tree: subassemblies are folders (the one being edited in
/// place is marked), the components below them are rows of their own. Double-click edits in place; the menus
/// demote, move, dissolve and rename. Inventor 2026 help: Create Subassemblies in Place
/// (GUID-FCEE3F0E-AAE2-4619-8645-2588FEECD353), Edit components in place
/// (GUID-C79C36C9-A6DB-46C5-8864-41F66C750231).
#[allow(clippy::too_many_arguments, reason = "the browser's shared state")]
fn component_rows(
    ui: &mut Ui,
    session: &Session,
    lang: Lang,
    state: &mut ParamsUi,
    selected: Option<gc_session::CellItem>,
    action: &mut Option<BrowserAction>,
    rows: &[gc_doc::TreeRow],
    parent: Option<gc_doc::TreeKey>,
) {
    // User-defined folders of this level (Inventor 2026 help, Assembly Browser Reference,
    // GUID-CB6B1637-71AD-4753-8CC9-19559BFEE3A5: "User-defined Folder groups assembly nodes for better browser
    // organization"): each lists its rows below its name; the rows in no folder follow.
    let folders: Vec<gc_doc::BrowserFolder> = session
        .folders()
        .into_iter()
        .filter(|f| f.members.first().is_some_and(|m| session.doc().tree_parent(*m) == parent))
        .collect();
    let mut inside: Vec<gc_doc::TreeKey> = Vec::new();
    for folder in &folders {
        inside.extend(folder.members.iter().copied());
        let title = format!("{} ({})", folder.name, folder.members.len());
        let opened = egui::CollapsingHeader::new(title)
            .id_salt(("gc_folder", folder.id))
            .default_open(true)
            .show(ui, |ui| {
                component_rows_in(
                    ui,
                    session,
                    lang,
                    state,
                    selected,
                    action,
                    rows,
                    parent,
                    Scope::Only(&folder.members),
                );
            });
        // Rows dragged onto the folder move into it.
        if let Some(dropped) = opened.header_response.dnd_release_payload::<Vec<gc_doc::TreeKey>>() {
            *action = Some(BrowserAction::Folder(gc_doc::FolderOp::Move {
                members: (*dropped).clone(),
                into: Some(folder.id),
            }));
        }
        opened.header_response.context_menu(|ui| {
            if ui.button(tr!(lang, CmdRename)).clicked() {
                state.group_name = Some(GroupName::RenameFolder(folder.id, folder.name.clone()));
                ui.close();
            }
            if ui.button(tr!(lang, CmdDeleteFolder)).clicked() {
                *action = Some(BrowserAction::Folder(gc_doc::FolderOp::Delete(folder.id)));
                ui.close();
            }
        });
    }
    component_rows_in(ui, session, lang, state, selected, action, rows, parent, Scope::Except(&inside));
}

/// Which rows of a level a call lists.
#[derive(Clone, Copy)]
enum Scope<'a> {
    /// Only these (a user-defined folder's rows).
    Only(&'a [gc_doc::TreeKey]),
    /// All but these (the rows no folder holds).
    Except(&'a [gc_doc::TreeKey]),
}

/// The New Folder and Move to Folder entries of a row's menu: "새 폴더(이 항목으로)", a list of the folders to
/// move it to, and "폴더에서 빼기" when it is in one.
fn folder_menu(
    ui: &mut Ui,
    session: &Session,
    lang: Lang,
    state: &mut ParamsUi,
    action: &mut Option<BrowserAction>,
    key: gc_doc::TreeKey,
) {
    // The marked rows go together when this row is one of them.
    let targets: Vec<gc_doc::TreeKey> =
        if state.marked.contains(&key) { state.marked.clone() } else { vec![key] };
    if ui.button(tr!(lang, CmdNewFolder)).clicked() {
        state.group_name = Some(GroupName::NewFolder(targets.clone(), String::new()));
        ui.close();
    }
    let folders = session.folders();
    if !folders.is_empty() {
        ui.menu_button(tr!(lang, CmdMoveToFolder), |ui| {
            for folder in &folders {
                if ui.button(&folder.name).clicked() {
                    *action = Some(BrowserAction::Folder(gc_doc::FolderOp::Move {
                        members: targets.clone(),
                        into: Some(folder.id),
                    }));
                    ui.close();
                }
            }
        });
    }
    if session.folder_of(key).is_some() && ui.button(tr!(lang, CmdOutOfFolder)).clicked() {
        *action = Some(BrowserAction::Folder(gc_doc::FolderOp::Move { members: vec![key], into: None }));
        ui.close();
    }
}

#[allow(clippy::too_many_arguments, reason = "the browser's shared state")]
fn component_rows_in(
    ui: &mut Ui,
    session: &Session,
    lang: Lang,
    state: &mut ParamsUi,
    selected: Option<gc_session::CellItem>,
    action: &mut Option<BrowserAction>,
    rows: &[gc_doc::TreeRow],
    parent: Option<gc_doc::TreeKey>,
    scope: Scope<'_>,
) {
    use gc_doc::{Occurrence, SubassemblyOp, TreeKey};
    use gc_session::CellItem;
    let editing = session.edit_target();
    for row in rows.iter().filter(|row| row.parent == parent) {
        match scope {
            Scope::Only(keys) if !keys.contains(&row.key) => continue,
            Scope::Except(keys) if keys.contains(&row.key) => continue,
            _ => {}
        }
        if !row_found(rows, row, &state.find) {
            continue;
        }
        // Hide Fully Constrained: a component that is fully constrained is not listed.
        if state.hide_fully_constrained
            && let TreeKey::Component(Occurrence::Item(any)) = row.key
            && state.statuses.get(&any) == Some(&gc_doc::ComponentStatus::Full)
        {
            continue;
        }
        // Show Children Only: with a component edited in place, what is neither inside it nor above it is left out.
        if state.show_children_only
            && let Some(target) = editing
            && target != row.key
            && !session.doc().is_under(row.key, target)
            && !session.doc().is_under(target, row.key)
        {
            continue;
        }
        // The suppressed-only filter keeps folders (they may hold some) and suppressed components.
        if state.only_suppressed
            && let TreeKey::Component(Occurrence::Item(any)) = row.key
            && !session.doc().is_inactive(any)
        {
            continue;
        }
        match row.key {
            TreeKey::Subassembly(id) => {
                let mark = if editing == Some(row.key) { "● " } else { "" };
                let title = format!("{mark}{}", row.name);
                let folder = egui::CollapsingHeader::new(title)
                    .id_salt(("gc_subassembly", id))
                    .default_open(true)
                    .show(ui, |ui| {
                        // A folder that matches the Find text lists everything in it.
                        let saved = (!state.find.trim().is_empty() && found(&state.find, &row.name))
                            .then(|| std::mem::take(&mut state.find));
                        component_rows(ui, session, lang, state, selected, action, rows, Some(row.key));
                        for sketch in session.sketches_of(Some(row.key)) {
                            sketch_row(ui, session, lang, state, action, sketch);
                        }
                        if let Some(find) = saved {
                            state.find = find;
                        }
                    });
                let header = folder.header_response;
                // A flexible subassembly is marked with a spring icon.
                if session.doc().subassemblies().iter().any(|s| s.id == id && s.flexible) {
                    let at = egui::Rect::from_center_size(
                        egui::pos2(header.rect.right() - 14.0, header.rect.center().y),
                        egui::vec2(14.0, 14.0),
                    );
                    Icon::Flexible.paint(ui.painter(), at, gc_ui::colors::muted());
                }
                if header.double_clicked() {
                    *action = Some(BrowserAction::EditComponent(row.key));
                }
                // Rows dragged onto a subassembly move into it.
                if let Some(dropped) = header.dnd_release_payload::<Vec<gc_doc::TreeKey>>() {
                    let members: Vec<gc_doc::TreeKey> =
                        dropped.iter().copied().filter(|k| *k != row.key).collect();
                    if !members.is_empty() {
                        *action =
                            Some(BrowserAction::Subassembly(SubassemblyOp::Move { members, into: Some(id) }));
                    }
                }
                header.context_menu(|ui| {
                    // Isolate: the members of the subassembly stay, everything else is hidden.
                    if ui.button(tr!(lang, CmdIsolate)).clicked() {
                        *action = Some(BrowserAction::Isolate(members_of(rows, row.key)));
                        ui.close();
                    }
                    if ui.button(tr!(lang, CmdEditInPlace)).clicked() {
                        *action = Some(BrowserAction::EditComponent(row.key));
                        ui.close();
                    }
                    // Flexible (Inventor 2026 help, Create and edit flexible components,
                    // GUID-30817F08-A5C2-44B7-B237-4791ACA3F854): the freedom inside is available in the
                    // parent; the check mark is cleared to make it rigid again.
                    let flexible = session.doc().subassemblies().iter().any(|s| s.id == id && s.flexible);
                    if ui.checkbox(&mut { flexible }, tr!(lang, CmdFlexible)).clicked() {
                        *action = Some(BrowserAction::Subassembly(SubassemblyOp::SetFlexible {
                            id,
                            flexible: !flexible,
                        }));
                        ui.close();
                    }
                    if ui.button(tr!(lang, CmdRename)).clicked() {
                        state.group_name = Some(GroupName::Rename(id, row.name.clone()));
                        ui.close();
                    }
                    // Folders nest without dragging: demote this one into a new subassembly.
                    if ui.button(tr!(lang, CmdDemote)).clicked() {
                        state.group_name = Some(GroupName::Demote(vec![row.key], String::new()));
                        ui.close();
                    }
                    if row.parent.is_some() && ui.button(tr!(lang, CmdMoveToTop)).clicked() {
                        *action = Some(BrowserAction::Subassembly(SubassemblyOp::Move {
                            members: vec![row.key],
                            into: None,
                        }));
                        ui.close();
                    }
                    if ui.button(tr!(lang, CmdDissolve)).clicked() {
                        *action = Some(BrowserAction::Subassembly(SubassemblyOp::Dissolve(id)));
                        ui.close();
                    }
                    folder_menu(ui, session, lang, state, action, row.key);
                });
            }
            TreeKey::Component(Occurrence::Item(any)) => {
                let item = match any.kind {
                    gc_core::Kind::Body => any.downcast().map(CellItem::Body),
                    gc_core::Kind::RobotInstance => any.downcast().map(CellItem::Robot),
                    _ => None,
                };
                if let Some(item) = item {
                    let shown = format!("{}:1", row.name);
                    browser_section(
                        ui,
                        session,
                        lang,
                        state,
                        selected,
                        action,
                        None,
                        vec![(row.name.clone(), shown, item)],
                    );
                    // The sketches made in this component's context hang below it.
                    ui.indent(("gc_owned_sketches", row.key), |ui| {
                        for sketch in session.sketches_of(Some(row.key)) {
                            sketch_row(ui, session, lang, state, action, sketch);
                        }
                    });
                }
            }
            // Parts, links and tools hang below their components, which draw them.
            TreeKey::Component(_) => {}
        }
    }
}

/// The name being asked for (Demote's new subassembly, a subassembly's new name).
fn group_name_row(ui: &mut Ui, lang: Lang, state: &mut ParamsUi, action: &mut Option<BrowserAction>) {
    use gc_doc::SubassemblyOp;
    let Some(entry) = state.group_name.as_mut() else { return };
    let (label, text) = match entry {
        GroupName::Demote(_, text) => (tr!(lang, CmdDemote), text),
        GroupName::Rename(_, text) => (tr!(lang, CmdRename), text),
        GroupName::NewFolder(_, text) => (tr!(lang, CmdNewFolder), text),
        GroupName::RenameFolder(_, text) => (tr!(lang, CmdRename), text),
    };
    let (mut confirmed, mut cancelled) = (false, false);
    ui.horizontal(|ui| {
        ui.label(label);
        let field = ui.add(egui::TextEdit::singleline(text).desired_width(110.0));
        field.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, label));
        if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            confirmed = true;
        }
        if ui.small_button(tr!(lang, Ok)).clicked() {
            confirmed = true;
        }
        if ui.small_button(tr!(lang, Cancel)).clicked() {
            cancelled = true;
        }
    });
    if confirmed {
        *action = Some(match entry {
            GroupName::Demote(members, name) => BrowserAction::Subassembly(SubassemblyOp::Demote {
                members: members.clone(),
                name: name.trim().to_owned(),
            }),
            GroupName::Rename(id, name) => {
                BrowserAction::Subassembly(SubassemblyOp::Rename { id: *id, name: name.trim().to_owned() })
            }
            GroupName::NewFolder(members, name) => BrowserAction::Folder(gc_doc::FolderOp::Add {
                name: name.trim().to_owned(),
                members: members.clone(),
            }),
            GroupName::RenameFolder(id, name) => {
                BrowserAction::Folder(gc_doc::FolderOp::Rename { id: *id, name: name.trim().to_owned() })
            }
        });
    }
    if confirmed || cancelled {
        state.group_name = None;
    }
}

/// A group of rows of the browser tree (components, frames, waypoints, paths): a folder with `title`, or
/// the rows alone.
#[allow(clippy::too_many_arguments, reason = "the browser's shared state")]
fn browser_section(
    ui: &mut Ui,
    session: &Session,
    lang: Lang,
    state: &mut ParamsUi,
    selected: Option<gc_session::CellItem>,
    action: &mut Option<BrowserAction>,
    title: Option<&str>,
    items: Vec<(String, String, gc_session::CellItem)>,
) {
    use gc_session::CellItem;
    if items.is_empty() {
        return;
    }
    let rows = |ui: &mut Ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        for (name, shown, item) in items {
            let icon = match item {
                CellItem::Body(_) => Icon::Cuboid,
                CellItem::Robot(_) => Icon::Robot,
                CellItem::Frame(_) => Icon::Frame,
                CellItem::Waypoint(_) => Icon::Waypoint,
                CellItem::Path(_) => Icon::Path,
            };
            let node = Node {
                icon,
                name: &name,
                text: &shown,
                any: cell_any(item),
                selected: selected == Some(item),
                delete: gc_doc::Command::Cell(item.delete_op()),
                suppressible: matches!(item, CellItem::Waypoint(_) | CellItem::Path(_)),
            };
            let mut ask: Option<GroupName> = None;
            let result = node_row(ui, session, lang, state, &node, action, |ui, action| {
                if let CellItem::Body(_) | CellItem::Robot(_) = item {
                    // Inventor: right-click a component > Contact Set.
                    let any = cell_any(item);
                    let member = session.doc().in_contact_set(any);
                    if ui.checkbox(&mut { member }, tr!(lang, CmdContactSet)).clicked() {
                        *action = Some(BrowserAction::ContactSet(any, !member));
                        ui.close();
                    }
                    let key = gc_doc::TreeKey::Component(gc_doc::Occurrence::Item(any));
                    if ui.button(tr!(lang, CmdEditInPlace)).clicked() {
                        *action = Some(BrowserAction::EditComponent(key));
                        ui.close();
                    }
                    if ui.button(tr!(lang, CmdDemote)).clicked() {
                        ask = Some(GroupName::Demote(vec![key], String::new()));
                        ui.close();
                    }
                    // Move between levels: to the top, or into another subassembly.
                    let current = session.doc().subassembly_of(gc_doc::Occurrence::Item(any));
                    let move_to = |into| {
                        Some(BrowserAction::Subassembly(gc_doc::SubassemblyOp::Move {
                            members: vec![key],
                            into,
                        }))
                    };
                    if current.is_some() && ui.button(tr!(lang, CmdMoveToTop)).clicked() {
                        *action = move_to(None);
                        ui.close();
                    }
                    let others: Vec<_> =
                        session.doc().subassemblies().into_iter().filter(|s| Some(s.id) != current).collect();
                    if !others.is_empty() {
                        ui.menu_button(tr!(lang, CmdMoveInto), |ui| {
                            for other in others {
                                if ui.button(&other.name).clicked() {
                                    *action = move_to(Some(other.id));
                                    ui.close();
                                }
                            }
                        });
                    }
                }
                if let CellItem::Path(path) = item
                    && ui.button(tr!(lang, CmdPlan)).clicked()
                {
                    *action = Some(BrowserAction::Plan(path));
                    ui.close();
                }
            });
            if ask.is_some() {
                state.group_name = ask;
            }
            if result.clicked() || result.secondary_clicked() {
                *action = Some(BrowserAction::Select(Some(item)));
            }
            if result.double_clicked()
                && let CellItem::Body(_) | CellItem::Robot(_) = item
            {
                let key = gc_doc::TreeKey::Component(gc_doc::Occurrence::Item(cell_any(item)));
                *action = Some(BrowserAction::EditComponent(key));
            }
            // Components have an origin folder of their own.
            if let CellItem::Body(_) | CellItem::Robot(_) = item {
                if !state.hide_work_features {
                    origin_folder(ui, session, lang, state, gc_doc::Occurrence::Item(cell_any(item)), action);
                }
                if let CellItem::Robot(robot) = item {
                    robot_links(ui, session, lang, state, robot, action);
                }
            }
        }
    };
    match title {
        Some(title) => {
            egui::CollapsingHeader::new(title).default_open(true).show(ui, rows);
        }
        None => rows(ui),
    }
}

/// The links of a robot as components of their own, nested like the kinematic chain, each with an origin
/// folder (a joint origin can be picked on any of them).
fn robot_links(
    ui: &mut Ui,
    session: &Session,
    lang: Lang,
    state: &ParamsUi,
    robot: gc_core::Id<gc_doc::Robot>,
    action: &mut Option<BrowserAction>,
) {
    let links = session.robot_links(robot);
    if links.is_empty() {
        return;
    }
    let depth = |node: &gc_doc::ComponentNode| {
        let mut depth = 0;
        let mut parent = node.parent;
        while let Some(gc_doc::Occurrence::Link { index, .. }) = parent {
            depth += 1;
            parent = links
                .iter()
                .find(|l| l.occurrence == gc_doc::Occurrence::Link { robot, index })
                .and_then(|l| l.parent);
        }
        depth
    };
    ui.indent(("gc_robot_links", robot.raw()), |ui| {
        egui::CollapsingHeader::new(tr!(lang, BrowserLinks))
            .id_salt(("gc_robot_links_folder", robot.raw()))
            .default_open(false)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                for node in &links {
                    let text = format!("{}{}:1", "  ".repeat(depth(node)), node.name);
                    let row = TreeRow {
                        icon: Icon::Robot,
                        text: &text,
                        selected: false,
                        visible: None,
                        delete: None,
                        edit: None,
                        focus: false,
                        dim: false,
                    };
                    gc_ui::panel::tree_row(ui, row);
                    if !state.hide_work_features {
                        origin_folder(ui, session, lang, state, node.occurrence, action);
                    }
                }
            });
    });
}

/// A small dot at the right of a row: filled (fully constrained), hollow (can still move) or a dash
/// (not satisfied), as Inventor marks components and constraints.
pub(crate) fn status_dot(ui: &Ui, row: egui::Rect, full: Option<bool>, sick: bool) {
    let at = egui::pos2(row.right() - 34.0, row.center().y);
    let painter = ui.painter();
    if sick {
        painter.line_segment(
            [at - egui::vec2(4.0, 0.0), at + egui::vec2(4.0, 0.0)],
            egui::Stroke::new(2.0, colors::ERROR),
        );
    } else if let Some(full) = full {
        if full {
            painter.circle_filled(at, 4.0, colors::text());
        } else {
            painter.circle_stroke(at, 3.5, egui::Stroke::new(1.3, colors::text()));
        }
    }
}

/// The name of an origin element.
fn origin_name(lang: Lang, element: gc_doc::OriginElement) -> &'static str {
    use gc_doc::{OriginElement, OriginPlane};
    match element {
        OriginElement::Plane(OriginPlane::Yz) => tr!(lang, OriginYz),
        OriginElement::Plane(OriginPlane::Xz) => tr!(lang, OriginXz),
        OriginElement::Plane(OriginPlane::Xy) => tr!(lang, OriginXy),
        OriginElement::Axis(0) => tr!(lang, OriginAxisX),
        OriginElement::Axis(1) => tr!(lang, OriginAxisY),
        OriginElement::Axis(_) => tr!(lang, OriginAxisZ),
        OriginElement::Point => tr!(lang, OriginPoint),
    }
}

/// The "origin" folder of a component: three planes, three axes and the centre point, hidden until the
/// user shows them (a check box each; the folder's menu shows or hides all seven).
fn origin_folder(
    ui: &mut Ui,
    session: &Session,
    lang: Lang,
    state: &ParamsUi,
    occurrence: gc_doc::Occurrence,
    action: &mut Option<BrowserAction>,
) {
    use gc_doc::OriginElement;
    let elements = gc_viewport_elements();
    let all_shown = elements.iter().all(|e| session.origin_visible(occurrence, *e));
    ui.indent(("gc_origin_indent", occurrence), |ui| {
        let header = egui::CollapsingHeader::new(tr!(lang, BrowserOrigin))
            .id_salt(("gc_origin_folder", occurrence))
            .default_open(false)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                for element in elements {
                    let name = origin_name(lang, element);
                    let mut visible = session.origin_visible(occurrence, element);
                    let show_label = tr!(lang, ShowItem, name);
                    let icon = match element {
                        OriginElement::Plane(_) => Icon::Sketch,
                        OriginElement::Axis(_) => Icon::Line,
                        OriginElement::Point => Icon::Waypoint,
                    };
                    let row = TreeRow {
                        icon,
                        text: name,
                        selected: state.origin_selected == Some((occurrence, element)),
                        visible: Some((&mut visible, show_label.as_str())),
                        delete: None,
                        edit: None,
                        focus: false,
                        dim: false,
                    };
                    let result = gc_ui::panel::tree_row(ui, row);
                    if result.toggled {
                        *action = Some(BrowserAction::OriginVisible(occurrence, vec![element], visible));
                    }
                    if result.row.clicked() || result.row.secondary_clicked() {
                        *action = Some(BrowserAction::OriginSelect(Some((occurrence, element))));
                    }
                    result.row.context_menu(|ui| {
                        if let OriginElement::Plane(plane) = element
                            && ui.button(tr!(lang, CmdSketchHere)).clicked()
                        {
                            *action = Some(BrowserAction::SketchOnComponent(occurrence, plane));
                            ui.close();
                        }
                        let text = if visible { tr!(lang, CmdHide) } else { tr!(lang, CmdShow) };
                        if ui.button(text).clicked() {
                            *action = Some(BrowserAction::OriginVisible(occurrence, vec![element], !visible));
                            ui.close();
                        }
                    });
                }
            });
        header.header_response.context_menu(|ui| {
            let text = if all_shown { tr!(lang, CmdHideOrigin) } else { tr!(lang, CmdShowOrigin) };
            if ui.button(text).clicked() {
                *action = Some(BrowserAction::OriginVisible(occurrence, elements.to_vec(), !all_shown));
                ui.close();
            }
        });
    });
}

/// The seven origin elements in the order of the folder.
fn gc_viewport_elements() -> [gc_doc::OriginElement; 7] {
    use gc_doc::{OriginElement, OriginPlane};
    [
        OriginElement::Plane(OriginPlane::Yz),
        OriginElement::Plane(OriginPlane::Xz),
        OriginElement::Plane(OriginPlane::Xy),
        OriginElement::Axis(0),
        OriginElement::Axis(1),
        OriginElement::Axis(2),
        OriginElement::Point,
    ]
}

/// The id of a cell item, whatever its kind.
fn cell_any(item: gc_session::CellItem) -> gc_core::AnyId {
    use gc_session::CellItem;
    match item {
        CellItem::Body(id) => id.erase(),
        CellItem::Robot(id) => id.erase(),
        CellItem::Waypoint(id) => id.erase(),
        CellItem::Frame(id) => id.erase(),
        CellItem::Path(id) => id.erase(),
    }
}

/// The name of a renamable browser item.
fn name_of(session: &Session, item: gc_core::AnyId) -> Option<String> {
    use gc_core::Kind;
    let doc = session.doc();
    match item.kind {
        Kind::Sketch => doc.get(item.downcast()?).map(|s: &Sketch| s.name.clone()),
        Kind::Body => doc.get(item.downcast()?).map(|b: &gc_doc::Body| b.name.clone()),
        Kind::RobotInstance => doc.get(item.downcast()?).map(|r: &gc_doc::Robot| r.name.clone()),
        Kind::Waypoint => doc.get(item.downcast()?).map(|w: &gc_doc::Waypoint| w.name.clone()),
        Kind::Path => doc.get(item.downcast()?).map(|p: &gc_doc::MotionPath| p.name.clone()),
        Kind::WorkFrame => doc.get(item.downcast()?).map(|f: &gc_doc::WorkFrame| f.name.clone()),
        Kind::SketchRole => doc.get(item.downcast()?).map(|r: &gc_doc::SketchRole| r.name.clone()),
        _ => None,
    }
}

/// A row of the browser tree.
struct Node<'a> {
    icon: Icon,
    /// The name, for commands and accessible names.
    name: &'a str,
    /// The text shown (the name, and for paths the plan status).
    text: &'a str,
    any: gc_core::AnyId,
    selected: bool,
    /// The edit that deletes it.
    delete: gc_doc::Command,
    /// Whether it can be suppressed.
    suppressible: bool,
}

/// One row of the tree with everything the browser offers on a node: a visibility check box, rename in
/// place (F2, a second click, the menu), suppress, delete and the node's own menu entries.
fn node_row(
    ui: &mut Ui,
    session: &Session,
    lang: Lang,
    state: &mut ParamsUi,
    node: &Node<'_>,
    action: &mut Option<BrowserAction>,
    menu: impl FnOnce(&mut Ui, &mut Option<BrowserAction>),
) -> egui::Response {
    let doc = session.doc();
    let mut visible = doc.is_visible(node.any);
    // What lies outside the component being edited in place is faded.
    let out_of_scope = matches!(node.any.kind, gc_core::Kind::Body | gc_core::Kind::RobotInstance)
        && !session.in_edit_scope(gc_doc::Occurrence::Item(node.any));
    // Grounded, disabled and invisible components look different (Inventor 2026 help, Assembly Browser
    // Reference, GUID-CB6B1637-71AD-4753-8CC9-19559BFEE3A5): a pushpin on the icon, the name in the
    // background style.
    let groundable =
        matches!(node.any.kind, gc_core::Kind::Body | gc_core::Kind::RobotInstance | gc_core::Kind::Assembly);
    let occurrence = gc_doc::Occurrence::Item(node.any);
    let grounded = groundable && doc.component_flag(occurrence, gc_doc::ComponentFlag::Grounded);
    let enabled = !groundable || doc.component_flag(occurrence, gc_doc::ComponentFlag::Enabled);
    let suppressed_now = doc.is_inactive(node.any);
    let inactive = suppressed_now || out_of_scope || !enabled || (groundable && !visible);
    let show_label = tr!(lang, ShowItem, node.name);
    let delete_label = tr!(lang, CmdDeleteItem, node.name);
    let renaming = state.rename.as_ref().is_some_and(|(id, _)| *id == node.any);
    let fresh = renaming && std::mem::take(&mut state.rename_fresh);
    let row = TreeRow {
        icon: node.icon,
        text: node.text,
        selected: node.selected
            || (groundable && state.marked.contains(&gc_doc::TreeKey::Component(occurrence))),
        visible: Some((&mut visible, show_label.as_str())),
        delete: Some(&delete_label),
        edit: state.rename.as_mut().filter(|_| renaming).map(|(_, text)| text),
        focus: fresh,
        dim: inactive,
    };
    let result = gc_ui::panel::tree_row(ui, row);
    if grounded {
        // The pin sits on the corner of the kind icon (after the check box and its gap).
        let icon_centre = egui::pos2(result.row.rect.left() + 4.0 + 24.0 + 8.0, result.row.rect.center().y);
        let at = egui::Rect::from_center_size(icon_centre + egui::vec2(6.0, -5.0), egui::vec2(10.0, 10.0));
        Icon::Pin.paint(ui.painter(), at, gc_ui::colors::text());
    }
    if let Some(&status) = state.statuses.get(&node.any).filter(|_| !state.hide_constraint_state) {
        use gc_doc::ComponentStatus;
        status_dot(
            ui,
            result.row.rect,
            Some(status == ComponentStatus::Full),
            status == ComponentStatus::Unknown,
        );
    }
    match result.edited {
        Some(true) => {
            if let Some((item, name)) = state.rename.take() {
                *action = Some(BrowserAction::Edit(Box::new(gc_doc::Command::Rename { item, name })));
            }
        }
        Some(false) => state.rename = None,
        None => {}
    }
    if result.toggled {
        let command = gc_doc::Command::SetVisible { item: node.any, visible };
        *action = Some(BrowserAction::Edit(Box::new(command)));
    }
    if groundable {
        // Ctrl+click marks rows to move together; a drag takes the marked rows, or this one.
        let key = gc_doc::TreeKey::Component(occurrence);
        state.drawn_rows.push(key);
        if result.row.clicked() {
            let (ctrl, shift) = ui.input(|i| (i.modifiers.command || i.modifiers.ctrl, i.modifiers.shift));
            if shift {
                // Shift+click marks every row from the last clicked one to this one.
                let from = state.anchor.and_then(|a| state.row_order.iter().position(|k| *k == a));
                let to = state.row_order.iter().position(|k| *k == key);
                if let (Some(from), Some(to)) = (from, to) {
                    let (low, high) = (from.min(to), from.max(to));
                    state.marked = state.row_order[low..=high].to_vec();
                }
            } else if ctrl {
                if let Some(at) = state.marked.iter().position(|k| *k == key) {
                    state.marked.remove(at);
                } else {
                    state.marked.push(key);
                }
                state.anchor = Some(key);
            } else {
                state.anchor = Some(key);
            }
        }
        let payload: Vec<gc_doc::TreeKey> =
            if state.marked.contains(&key) { state.marked.clone() } else { vec![key] };
        result.row.dnd_set_drag_payload(payload);
    }
    if result.row.hovered() && matches!(node.any.kind, gc_core::Kind::Body | gc_core::Kind::RobotInstance) {
        state.hovered = Some(node.any);
    }
    // F2 renames the selected node (Keyboard Shortcut Reference, GUID-DEEB0723-DFC7-4B28-95F4-099CC1E1EBF8).
    let f2 = node.selected
        && !renaming
        && name_of(session, node.any).is_some()
        && ui.input(|i| i.key_pressed(egui::Key::F2));
    if f2 || (result.row.clicked() && node.selected && !result.row.double_clicked() && !renaming) {
        // A second click on the selected node edits its name.
        state.rename = Some((node.any, node.name.to_owned()));
        state.rename_fresh = true;
    }
    result.row.context_menu(|ui| {
        ui.label(RichText::new(node.name).strong());
        ui.separator();
        menu(ui, action);
        if name_of(session, node.any).is_some() && ui.button(tr!(lang, CmdRename)).clicked() {
            state.rename = Some((node.any, node.name.to_owned()));
            state.rename_fresh = true;
            ui.close();
        }
        let text = if visible { tr!(lang, CmdHide) } else { tr!(lang, CmdShow) };
        if ui.button(text).clicked() {
            let command = gc_doc::Command::SetVisible { item: node.any, visible: !visible };
            *action = Some(BrowserAction::Edit(Box::new(command)));
            ui.close();
        }
        // Enabled and Grounded, as check marks (a disabled component is drawn in the background style; a
        // grounded one has all its freedom removed).
        if groundable {
            if ui.checkbox(&mut { enabled }, tr!(lang, CmdEnabled)).clicked() {
                let command = gc_doc::Command::SetComponent {
                    occurrence,
                    flag: gc_doc::ComponentFlag::Enabled,
                    on: !enabled,
                };
                *action = Some(BrowserAction::Edit(Box::new(command)));
                ui.close();
            }
            if ui.checkbox(&mut { grounded }, tr!(lang, CmdGround)).clicked() {
                let command = gc_doc::Command::SetComponent {
                    occurrence,
                    flag: gc_doc::ComponentFlag::Grounded,
                    on: !grounded,
                };
                *action = Some(BrowserAction::Edit(Box::new(command)));
                ui.close();
            }
        }
        if matches!(node.any.kind, gc_core::Kind::Body | gc_core::Kind::RobotInstance)
            && ui.button(tr!(lang, CmdFindInWindow)).clicked()
        {
            *action = Some(BrowserAction::FindInWindow(node.any));
            ui.close();
        }
        if groundable && ui.button(tr!(lang, CmdIsolate)).clicked() {
            *action = Some(BrowserAction::Isolate(vec![occurrence]));
            ui.close();
        }
        if groundable {
            folder_menu(ui, session, lang, state, action, gc_doc::TreeKey::Component(occurrence));
        }
        if groundable && ui.button(tr!(lang, CmdIProperties)).clicked() {
            *action = Some(BrowserAction::Properties(occurrence));
            ui.close();
        }
        if node.suppressible {
            let text = if suppressed_now { tr!(lang, CmdUnsuppress) } else { tr!(lang, CmdSuppress) };
            if ui.button(text).clicked() {
                let op = gc_doc::CellOp::SetSuppressed { item: node.any, suppressed: !suppressed_now };
                *action = Some(BrowserAction::Edit(Box::new(gc_doc::Command::Cell(op))));
                ui.close();
            }
        }
        if ui.button(delete_label.as_str()).clicked() {
            *action = Some(BrowserAction::Edit(Box::new(node.delete.clone())));
            ui.close();
        }
    });
    if result.deleted {
        *action = Some(BrowserAction::Edit(Box::new(node.delete.clone())));
    }
    result.row
}

/// The properties of the selection. A selected dimension can be edited here.
pub fn properties(ui: &mut Ui, session: &mut Session, lang: Lang, expression: &mut String) {
    gc_ui::panel::header(ui, tr!(lang, PanelProperties));
    // Outside sketches the robot cell's panel below says what is selected.
    let Some((_, sketch)) = session.active_sketch() else { return };
    let selection: Vec<Item> = session.selection().iter().copied().collect();
    let length = |si: f64| format!("{} mm", trim(si * 1000.0));
    match selection.as_slice() {
        [] => {
            ui.weak(tr!(lang, PropNothing));
        }
        [Item::Point(p)] => {
            if let Some(pos) = session.point_pos(*p) {
                ui.label(tr!(lang, PropPoint, length(pos[0]), length(pos[1])));
            }
        }
        [Item::Curve(c)] => match sketch.curves.get(c).map(|c| c.geom.clone()) {
            Some(CurveGeom::Line { a, b }) => {
                if let Some(value) = sketch.measure(&gc_sketch::DimKind::Distance { p: a, q: b }) {
                    ui.label(tr!(lang, PropLine, length(value)));
                }
            }
            Some(CurveGeom::Circle { .. }) => {
                ui.label(tr!(lang, PropCircle, length(session.radius(*c).unwrap_or(0.0))));
            }
            Some(CurveGeom::Arc { .. }) => {
                ui.label(tr!(lang, PropArc, length(session.radius(*c).unwrap_or(0.0))));
            }
            // No property line for an ellipse yet (frontend).
            Some(CurveGeom::Ellipse { .. } | CurveGeom::Spline { .. }) | None => {}
        },
        [Item::Constraint(_)] => {
            ui.label(tr!(lang, PropConstraint));
        }
        [Item::Dimension(d)] => {
            let Some(dimension) = sketch.dimensions.get(d) else { return };
            ui.label(tr!(lang, PropDimension, dimension.source));
            if dimension.driving {
                let id = *d;
                let memory = egui::Id::unique(("gc_expression_of", d.raw()));
                let loaded: Option<u64> = ui.data(|data| data.get_temp(memory));
                if loaded != Some(d.raw().get()) {
                    expression.clone_from(&dimension.source);
                    ui.data_mut(|data| data.insert_temp(memory, d.raw().get()));
                }
                let label = ui.label(tr!(lang, PropExpression));
                let field = ui.text_edit_singleline(expression).labelled_by(label.id);
                if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    // A refused edit is shown in the status bar; the field keeps what was typed.
                    drop(session.edit_dimension(id, expression));
                }
            }
        }
        many => {
            ui.label(tr!(lang, PropSelected, many.len()));
        }
    }
}

fn trim(value: f64) -> String {
    let text = format!("{value:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

fn prompt_text(lang: Lang, prompt: Prompt) -> &'static str {
    let message = match prompt {
        Prompt::Select => Msg::PromptSelect,
        Prompt::LineStart => Msg::PromptLineStart,
        Prompt::LineEnd => Msg::PromptLineEnd,
        Prompt::CircleCentre => Msg::PromptCircleCentre,
        Prompt::CircleRim => Msg::PromptCircleRim,
        Prompt::ArcCentre => Msg::PromptArcCentre,
        Prompt::ArcStart => Msg::PromptArcStart,
        Prompt::ArcEnd => Msg::PromptArcEnd,
        Prompt::RectangleFirst => Msg::PromptRectFirst,
        Prompt::RectangleSecond => Msg::PromptRectSecond,
        Prompt::DimensionPick => Msg::PromptDimPick,
        Prompt::DimensionPlace => Msg::PromptDimPlace,
        Prompt::DimensionValue => Msg::PromptDimValue,
        Prompt::ConstraintPick => Msg::PromptConstraint,
        Prompt::PointPlace => Msg::PromptPointPlace,
        Prompt::PolygonCentre => Msg::PromptPolygonCentre,
        Prompt::PolygonCorner => Msg::PromptPolygonCorner,
        Prompt::SlotFirst => Msg::PromptSlotFirst,
        Prompt::SlotSecond => Msg::PromptSlotSecond,
        Prompt::SlotWidth => Msg::PromptSlotWidth,
        Prompt::OffsetPick => Msg::PromptOffsetPick,
        Prompt::OffsetPlace => Msg::PromptOffsetPlace,
        Prompt::ArcSlotCentre => Msg::PromptArcSlotCentre,
        Prompt::ArcSlotThrough => Msg::PromptArcSlotThrough,
        Prompt::ArcSlotEnd => Msg::PromptArcSlotEnd,
        Prompt::MidpointLineMid => Msg::PromptMidpointLineMid,
        Prompt::MidpointLineEnd => Msg::PromptMidpointLineEnd,
        Prompt::CenterRectCentre => Msg::PromptCenterRectCentre,
        Prompt::CenterRectCorner => Msg::PromptCenterRectCorner,
        Prompt::CenterRectSide => Msg::PromptCenterRectSide,
        Prompt::CenterRectWidth => Msg::PromptCenterRectWidth,
        Prompt::EllipseCentre => Msg::PromptEllipseCentre,
        Prompt::EllipseAxis => Msg::PromptEllipseAxis,
        Prompt::EllipseMinor => Msg::PromptEllipseMinor,
        Prompt::SplineFirst => Msg::PromptSplineFirst,
        Prompt::SplineNext => Msg::PromptSplineNext,
        Prompt::TangentCircleLine => Msg::PromptTangentCircleLine,
        Prompt::SplitPick => Msg::PromptSplitPick,
        Prompt::ChamferFirst => Msg::PromptChamferFirst,
        Prompt::ChamferSecond => Msg::PromptChamferSecond,
        Prompt::MoveBase => Msg::PromptMoveBase,
        Prompt::MoveEnd => Msg::PromptMoveEnd,
        Prompt::RotateCentre => Msg::PromptRotateCentre,
        Prompt::RotateEnd => Msg::PromptRotateEnd,
        Prompt::ScaleBase => Msg::PromptScaleBase,
        Prompt::ScaleEnd => Msg::PromptScaleEnd,
        Prompt::PickGeometry => Msg::PromptPickGeometry,
        Prompt::PickLine => Msg::PromptPickLine,
        Prompt::PickPoint => Msg::PromptPickPoint,
    };
    message.text(lang)
}

/// The status bar: what the tool expects next, why the last edit was refused, a notice from the
/// application (file saved, file error) and how constrained the sketch is.
pub fn status_bar(
    ui: &mut Ui,
    session: &Session,
    lang: Lang,
    gpu: bool,
    notice: Option<&str>,
    slice: Option<&mut bool>,
    command_prompt: Option<Msg>,
) {
    ui.horizontal(|ui| {
        if session.active_sketch().is_some() {
            ui.label(prompt_text(lang, session.prompt()));
        } else if let Some(prompt) = command_prompt {
            // A command of the assembly is waiting for a pick (the status bar tells what to pick next).
            ui.label(prompt.text(lang));
        } else {
            ui.label(tr!(lang, StatusReady));
        }
        // Slice Graphics (F7): offered while a 2D sketch is edited.
        if let Some(slice) = slice {
            ui.separator();
            let label = tr!(lang, StatusSlice);
            let button = ui.selectable_label(*slice, label);
            button.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, *slice, label));
            if button.clicked() {
                *slice = !*slice;
            }
        }
        if session.sketch_is_spatial() {
            let plane = ["YZ", "ZX", "XY"][session.spatial_plane() % 3];
            ui.separator();
            ui.label(tr!(lang, StatusPlane, plane));
        }
        if let Some(error) = session.last_error() {
            let message = match error {
                gc_doc::EditError::Redundant(_) => Msg::ErrRedundant,
                gc_doc::EditError::Conflicting(_) => Msg::ErrConflicting,
                gc_doc::EditError::Expr { .. } | gc_doc::EditError::Sketch(SketchError::InvalidValue) => {
                    Msg::ErrExpression
                }
                gc_doc::EditError::Sketch(SketchError::Unsolved(_)) => Msg::ErrUnsolved,
                gc_doc::EditError::InUse => Msg::ErrInUse,
                gc_doc::EditError::NameTaken => Msg::ErrNameTaken,
                gc_doc::EditError::Unsolvable { .. } => Msg::ErrUnsolvable,
                gc_doc::EditError::BadValue => Msg::ErrBadValue,
                gc_doc::EditError::Underconstrained => Msg::ErrUnderconstrained,
                gc_doc::EditError::DegenerateFrame => Msg::ErrDegenerateFrame,
                gc_doc::EditError::Protected(_) => Msg::ErrProtected,
                _ => Msg::ErrOther,
            };
            ui.separator();
            ui.label(RichText::new(message.text(lang)).color(colors::ERROR));
        }
        if let Some(notice) = notice {
            ui.separator();
            ui.label(notice);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // At the far right, as in Inventor: the number of occurrences, and of open documents.
            ui.weak(tr!(lang, StatusDocuments, 1));
            ui.separator();
            ui.weak(tr!(lang, StatusOccurrences, session.components().len()));
            ui.separator();
            ui.weak(if gpu { tr!(lang, GpuOn) } else { tr!(lang, GpuOff) });
            if let Some(report) = session.report() {
                ui.separator();
                if report.dof == 0 {
                    ui.label(RichText::new(tr!(lang, StatusFullyConstrained)).strong());
                } else {
                    ui.label(tr!(lang, StatusDimensionsNeeded, report.dof));
                }
            }
        });
    });
}
