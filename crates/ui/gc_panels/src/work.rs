//! The work model in the browser (plan 9.4, GraphCTRL's own): a "작업" node with object types, work objects,
//! state variables and tasks. Rows rename, delete and show whether a task is ready or has problems; the
//! editors are in [`crate::work_properties`].

use egui::Ui;
use gc_core::{AnyId, Id};
use gc_doc::{ObjectType, StateValue, StateVar, Task, WorkObject, WorkOp};
use gc_i18n::{Lang, tr};
use gc_session::Session;
use gc_ui::Icon;
use gc_ui::panel::{TreeRow, tree_row};

use crate::BrowserAction;

/// What is selected in the work node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkItem {
    /// An object type.
    Type(Id<ObjectType>),
    /// A work object.
    Object(Id<WorkObject>),
    /// A state variable.
    Var(Id<StateVar>),
    /// A task.
    Task(Id<Task>),
    /// A kind of tool (a device with signals, states and actions).
    Kit(Id<gc_doc::ToolKit>),
}

impl WorkItem {
    fn any(self) -> AnyId {
        match self {
            Self::Type(id) => id.erase(),
            Self::Object(id) => id.erase(),
            Self::Var(id) => id.erase(),
            Self::Task(id) => id.erase(),
            Self::Kit(id) => id.erase(),
        }
    }

    fn delete(self) -> WorkOp {
        match self {
            Self::Type(id) => WorkOp::DeleteObjectType(id),
            Self::Object(id) => WorkOp::DeleteObject(id),
            Self::Var(id) => WorkOp::DeleteStateVar(id),
            Self::Task(id) => WorkOp::DeleteTask(id),
            Self::Kit(id) => WorkOp::DeleteToolKit(id),
        }
    }
}

/// The state of the work node that lives between frames.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkUi {
    /// What the properties panel shows.
    pub selected: Option<WorkItem>,
    /// The item being renamed and the name typed so far.
    rename: Option<(AnyId, String)>,
    /// The rename has just begun: the field takes the keyboard.
    fresh: bool,
}

/// A name no item of the list has: `base` and the next number.
fn free_name(base: &str, taken: &[String]) -> String {
    (taken.len() + 1..)
        .map(|n| format!("{base}{n}"))
        .find(|name| !taken.contains(name))
        .unwrap_or_else(|| base.to_owned())
}

/// The work node of the browser. The action it asks for (a change of the model, a rename) goes back to the
/// application, which reports a refusal in the status bar.
pub fn work_browser(ui: &mut Ui, session: &Session, lang: Lang, state: &mut WorkUi) -> Option<BrowserAction> {
    let doc = session.doc();
    let mut action = None;
    egui::CollapsingHeader::new(tr!(lang, WorkTitle)).default_open(false).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        // Object types.
        let header =
            egui::CollapsingHeader::new(tr!(lang, WorkObjectTypes)).default_open(true).show(ui, |ui| {
                for (id, ty) in doc.object_types().iter() {
                    item_row(ui, session, lang, state, &mut action, WorkItem::Type(id), &ty.name, None);
                }
            });
        header.header_response.context_menu(|ui| {
            if ui.button(tr!(lang, WorkNew)).clicked() {
                let taken: Vec<String> = doc.object_types().iter().map(|(_, t)| t.name.clone()).collect();
                action = Some(BrowserAction::Work(WorkOp::AddObjectType(ObjectType {
                    name: free_name(tr!(lang, WorkTypeBase), &taken),
                    states: vec![tr!(lang, WorkStateBase1).to_owned(), tr!(lang, WorkStateBase2).to_owned()],
                })));
                ui.close();
            }
        });
        // Work objects.
        let header = egui::CollapsingHeader::new(tr!(lang, WorkObjects)).default_open(true).show(ui, |ui| {
            for (id, object) in doc.objects().iter() {
                item_row(ui, session, lang, state, &mut action, WorkItem::Object(id), &object.name, None);
            }
        });
        header.header_response.context_menu(|ui| {
            let first = doc.object_types().iter().next().map(|(id, t)| (id, t.states.first().cloned()));
            let enabled = first.is_some();
            if ui.add_enabled(enabled, egui::Button::new(tr!(lang, WorkNew))).clicked()
                && let Some((ty, Some(state_name))) = first
            {
                let taken: Vec<String> = doc.objects().iter().map(|(_, o)| o.name.clone()).collect();
                action = Some(BrowserAction::Work(WorkOp::AddObject(WorkObject {
                    name: free_name(tr!(lang, WorkObjectBase), &taken),
                    ty,
                    location: gc_doc::Location::None,
                    state: state_name,
                })));
                ui.close();
            }
        });
        // State variables.
        let header = egui::CollapsingHeader::new(tr!(lang, WorkVars)).default_open(true).show(ui, |ui| {
            for (id, var) in doc.state_vars().iter() {
                item_row(ui, session, lang, state, &mut action, WorkItem::Var(id), &var.name, None);
            }
        });
        header.header_response.context_menu(|ui| {
            if ui.button(tr!(lang, WorkNew)).clicked() {
                let taken: Vec<String> = doc.state_vars().iter().map(|(_, v)| v.name.clone()).collect();
                action = Some(BrowserAction::Work(WorkOp::AddStateVar(StateVar {
                    name: free_name(tr!(lang, WorkVarBase), &taken),
                    value: StateValue::Bool(false),
                })));
                ui.close();
            }
        });
        // Kinds of tool.
        let header = egui::CollapsingHeader::new(tr!(lang, WorkToolKits)).default_open(true).show(ui, |ui| {
            for (id, kit) in doc.tool_kits().iter() {
                item_row(ui, session, lang, state, &mut action, WorkItem::Kit(id), &kit.name, None);
            }
        });
        header.header_response.context_menu(|ui| {
            if ui.button(tr!(lang, WorkNew)).clicked() {
                let taken: Vec<String> = doc.tool_kits().iter().map(|(_, k)| k.name.clone()).collect();
                action = Some(BrowserAction::Work(WorkOp::AddToolKit(gc_doc::ToolKit {
                    name: free_name(tr!(lang, WorkKitBase), &taken),
                    signals: Vec::new(),
                    states: Vec::new(),
                    actions: Vec::new(),
                })));
                ui.close();
            }
        });
        // Tasks.
        let header = egui::CollapsingHeader::new(tr!(lang, WorkTasks)).default_open(true).show(ui, |ui| {
            for (id, task) in doc.tasks().iter() {
                // Ready (the precondition holds in the starting states), not ready, or with problems.
                let problems = !session.task_problems(id).is_empty();
                let ready = session.task_ready(id);
                let status = if problems { None } else { ready.as_ref().ok().copied() };
                let sick = problems || ready.is_err();
                item_row(
                    ui,
                    session,
                    lang,
                    state,
                    &mut action,
                    WorkItem::Task(id),
                    &task.name,
                    Some((status, sick)),
                );
            }
        });
        header.header_response.context_menu(|ui| {
            if ui.button(tr!(lang, WorkNew)).clicked() {
                let taken: Vec<String> = doc.tasks().iter().map(|(_, t)| t.name.clone()).collect();
                action = Some(BrowserAction::Work(WorkOp::AddTask(Task {
                    name: free_name(tr!(lang, WorkTaskBase), &taken),
                    location: gc_doc::Location::None,
                    precondition: String::new(),
                    steps: Vec::new(),
                    effects: Vec::new(),
                    robots: gc_doc::RobotChoice::Any,
                    priority: 0,
                })));
                ui.close();
            }
        });
    });
    action
}

/// One row of the work node: a click selects it, a second click renames it, the menu renames and deletes.
/// `status` is a task's readiness dot and whether it has problems.
#[allow(clippy::too_many_arguments, reason = "the browser's shared state")]
fn item_row(
    ui: &mut Ui,
    session: &Session,
    lang: Lang,
    state: &mut WorkUi,
    action: &mut Option<BrowserAction>,
    item: WorkItem,
    name: &str,
    status: Option<(Option<bool>, bool)>,
) {
    let any = item.any();
    let renaming = state.rename.as_ref().is_some_and(|(id, _)| *id == any);
    let fresh = renaming && std::mem::take(&mut state.fresh);
    let delete_label = tr!(lang, CmdDeleteItem, name);
    let icon = match item {
        WorkItem::Type(_) => Icon::Frame,
        WorkItem::Object(_) => Icon::Waypoint,
        WorkItem::Var(_) => Icon::Text("x"),
        WorkItem::Task(_) => Icon::Path,
        WorkItem::Kit(_) => Icon::Text("T"),
    };
    let was_selected = state.selected == Some(item);
    let row = TreeRow {
        icon,
        text: name,
        selected: was_selected,
        visible: None,
        delete: Some(&delete_label),
        edit: state.rename.as_mut().filter(|_| renaming).map(|(_, text)| text),
        focus: fresh,
        dim: false,
    };
    let result = tree_row(ui, row);
    match result.edited {
        Some(true) => {
            if let Some((any, name)) = state.rename.take() {
                *action = Some(BrowserAction::Edit(Box::new(gc_doc::Command::Rename { item: any, name })));
            }
        }
        Some(false) => state.rename = None,
        None => {}
    }
    if let Some((full, sick)) = status {
        crate::status_dot(ui, result.row.rect, full, sick);
    }
    if result.row.clicked() {
        if was_selected && !renaming {
            // A second click on the selected row edits its name.
            state.rename = Some((any, name.to_owned()));
            state.fresh = true;
        }
        state.selected = Some(item);
    }
    if result.deleted {
        *action = Some(BrowserAction::Work(item.delete()));
    }
    result.row.context_menu(|ui| {
        ui.label(egui::RichText::new(name).strong());
        ui.separator();
        if ui.button(tr!(lang, CmdRename)).clicked() {
            state.rename = Some((any, name.to_owned()));
            state.fresh = true;
            ui.close();
        }
        if ui.button(delete_label.as_str()).clicked() {
            *action = Some(BrowserAction::Work(item.delete()));
            ui.close();
        }
    });
    let _ = session;
}

// ---------------------------------------------------------------------------------------------------------
// The editors of the work node's items (properties panel).

/// A text box that keeps what is typed until it is left: `Some(new)` when it was left with a different text.
fn committed_text(ui: &mut Ui, key: egui::Id, current: &str, width: f32) -> (egui::Response, Option<String>) {
    let mut text = ui
        .data(|d| d.get_temp::<(String, String)>(key))
        .filter(|(from, _)| from == current)
        .map_or_else(|| current.to_owned(), |(_, typed)| typed);
    let response = ui.add(egui::TextEdit::singleline(&mut text).desired_width(width));
    let done = response.lost_focus() && text != current;
    ui.data_mut(|d| d.insert_temp(key, (current.to_owned(), text.clone())));
    (response, done.then_some(text))
}

/// A list of choices as a combo box; true when another was chosen.
fn choice<T: Clone + PartialEq>(
    ui: &mut Ui,
    salt: impl std::hash::Hash + std::fmt::Debug,
    current: &mut T,
    options: &[(T, String)],
) -> bool {
    let before = current.clone();
    let shown = options.iter().find(|(v, _)| *v == *current).map_or_else(String::new, |(_, n)| n.clone());
    let name = shown.clone();
    let popup = egui::ComboBox::from_id_salt(salt).selected_text(shown).show_ui(ui, |ui| {
        for (value, name) in options {
            ui.selectable_value(current, value.clone(), name);
        }
    });
    popup.response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, name.as_str()));
    *current != before
}

/// Where an item is: nowhere, a component, a work frame, a sketch or a robot.
fn location_options(session: &Session, lang: Lang) -> Vec<(gc_doc::Location, String)> {
    use gc_doc::Location;
    let doc = session.doc();
    let mut out = vec![(Location::None, tr!(lang, WorkNowhere).to_owned())];
    for node in session.components() {
        out.push((Location::Component(node.occurrence), tr!(lang, WorkAtComponent, node.name)));
    }
    for (id, frame) in doc.work_frames().iter() {
        out.push((Location::Frame(id), tr!(lang, WorkAtFrame, frame.name)));
    }
    for (id, sketch) in doc.sketches().iter() {
        out.push((Location::Sketch(id), tr!(lang, WorkAtSketch, sketch.name)));
    }
    for (id, robot) in doc.robots().iter() {
        out.push((Location::Robot(id), tr!(lang, WorkAtRobot, robot.name)));
    }
    out
}

/// The properties of what is selected in the work node: a task (location, condition, steps, effects, robots,
/// priority), an object type (its states), an object (type, place, state) or a variable (its kind and value).
pub fn work_properties(ui: &mut Ui, session: &mut Session, lang: Lang, state: &WorkUi) {
    match state.selected {
        Some(WorkItem::Task(id)) => task_properties(ui, session, lang, id),
        Some(WorkItem::Type(id)) => type_properties(ui, session, lang, id),
        Some(WorkItem::Object(id)) => object_properties(ui, session, lang, id),
        Some(WorkItem::Var(id)) => var_properties(ui, session, lang, id),
        Some(WorkItem::Kit(id)) => kit_properties(ui, session, lang, id),
        None => {}
    }
}

fn type_properties(ui: &mut Ui, session: &mut Session, lang: Lang, id: Id<ObjectType>) {
    let Some(ty) = session.doc().get(id).cloned() else { return };
    gc_ui::panel::header(ui, &ty.name);
    ui.label(tr!(lang, WorkStates));
    let mut next = ty.clone();
    let mut changed = false;
    let mut remove = None;
    for (index, state_name) in ty.states.iter().enumerate() {
        ui.horizontal(|ui| {
            let key = egui::Id::unique("gc_work_state").with((id, index));
            let (_, done) = committed_text(ui, key, state_name, 120.0);
            if let Some(text) = done {
                next.states[index] = text;
                changed = true;
            }
            if ui.add_enabled(ty.states.len() > 1, egui::Button::new("✕").small()).clicked() {
                remove = Some(index);
            }
        });
    }
    if let Some(index) = remove {
        next.states.remove(index);
        changed = true;
    }
    if ui.button(tr!(lang, WorkAddState)).clicked() {
        let taken = next.states.clone();
        next.states.push(free_name(tr!(lang, WorkStateBase), &taken));
        changed = true;
    }
    if changed {
        // A refusal (a name twice, a state still used) shows in the status bar.
        drop(session.edit_work(WorkOp::SetObjectType { id, ty: next }));
    }
}

fn object_properties(ui: &mut Ui, session: &mut Session, lang: Lang, id: Id<WorkObject>) {
    let Some(object) = session.doc().get(id).cloned() else { return };
    gc_ui::panel::header(ui, &object.name);
    let mut next = object.clone();
    let mut changed = false;
    let types: Vec<(Id<ObjectType>, String)> =
        session.doc().object_types().iter().map(|(t, ty)| (t, ty.name.clone())).collect();
    ui.horizontal(|ui| {
        ui.label(tr!(lang, WorkType));
        if choice(ui, ("gc_work_obj_type", id), &mut next.ty, &types) {
            // A new type: the object starts in its first state.
            if let Some(first) = session.doc().get(next.ty).and_then(|t| t.states.first().cloned()) {
                next.state = first;
            }
            changed = true;
        }
    });
    ui.horizontal(|ui| {
        ui.label(tr!(lang, WorkPlace));
        changed |= choice(ui, ("gc_work_obj_loc", id), &mut next.location, &location_options(session, lang));
    });
    let states: Vec<(usize, String)> = session
        .doc()
        .get(next.ty)
        .map(|t| t.states.iter().cloned().enumerate().collect())
        .unwrap_or_default();
    let mut index = states.iter().position(|(_, s)| *s == next.state).unwrap_or(0);
    ui.horizontal(|ui| {
        ui.label(tr!(lang, WorkInitialState));
        if choice(ui, ("gc_work_obj_state", id), &mut index, &states)
            && let Some((_, name)) = states.get(index)
        {
            next.state = name.clone();
            changed = true;
        }
    });
    if changed {
        drop(session.edit_work(WorkOp::SetObject { id, object: next }));
    }
}

fn var_properties(ui: &mut Ui, session: &mut Session, lang: Lang, id: Id<StateVar>) {
    let Some(var) = session.doc().get(id).cloned() else { return };
    gc_ui::panel::header(ui, &var.name);
    let mut next = var.clone();
    let mut changed = false;
    // The kind: the initial value decides it.
    let kind = match var.value {
        StateValue::Bool(_) => 0,
        StateValue::Int(_) => 1,
        StateValue::Real(_) => 2,
        StateValue::Enum { .. } => 3,
    };
    let mut chosen = kind;
    let kinds = vec![
        (0, tr!(lang, WorkKindBool).to_owned()),
        (1, tr!(lang, WorkKindInt).to_owned()),
        (2, tr!(lang, WorkKindReal).to_owned()),
        (3, tr!(lang, WorkKindEnum).to_owned()),
    ];
    ui.horizontal(|ui| {
        ui.label(tr!(lang, WorkKind));
        if choice(ui, ("gc_work_var_kind", id), &mut chosen, &kinds) {
            next.value = match chosen {
                0 => StateValue::Bool(false),
                1 => StateValue::Int(0),
                2 => StateValue::Real(0.0),
                _ => StateValue::Enum {
                    choices: vec![tr!(lang, WorkStateBase1).to_owned(), tr!(lang, WorkStateBase2).to_owned()],
                    value: tr!(lang, WorkStateBase1).to_owned(),
                },
            };
            changed = true;
        }
    });
    ui.horizontal(|ui| {
        ui.label(tr!(lang, WorkInitialValue));
        match &mut next.value {
            StateValue::Bool(b) => changed |= ui.checkbox(b, "").changed(),
            StateValue::Int(i) => {
                let r = ui.add(egui::DragValue::new(i));
                changed |= r.drag_stopped() || (r.lost_focus() && r.changed());
            }
            StateValue::Real(x) => {
                let r = ui.add(egui::DragValue::new(x).speed(0.01));
                changed |= r.drag_stopped() || (r.lost_focus() && r.changed());
            }
            StateValue::Enum { choices, value } => {
                let options: Vec<(usize, String)> = choices.iter().cloned().enumerate().collect();
                let mut index = choices.iter().position(|c| c == value).unwrap_or(0);
                if choice(ui, ("gc_work_var_enum", id), &mut index, &options) {
                    value.clone_from(&choices[index]);
                    changed = true;
                }
            }
        }
    });
    if let StateValue::Enum { choices, .. } = &mut next.value {
        ui.label(tr!(lang, WorkChoices));
        let mut remove = None;
        for index in 0..choices.len() {
            ui.horizontal(|ui| {
                let key = egui::Id::unique("gc_work_choice").with((id, index));
                let current = choices[index].clone();
                let (_, done) = committed_text(ui, key, &current, 120.0);
                if let Some(text) = done {
                    // The value follows a renamed choice.
                    choices[index] = text;
                    changed = true;
                }
                if ui.add_enabled(choices.len() > 1, egui::Button::new("✕").small()).clicked() {
                    remove = Some(index);
                }
            });
        }
        if let Some(index) = remove {
            choices.remove(index);
            changed = true;
        }
        if ui.button(tr!(lang, WorkAddChoice)).clicked() {
            let taken = choices.clone();
            choices.push(free_name(tr!(lang, WorkStateBase), &taken));
            changed = true;
        }
        if let StateValue::Enum { choices, value } = &mut next.value
            && !choices.contains(value)
        {
            value.clone_from(&choices[0]);
        }
    }
    if changed {
        drop(session.edit_work(WorkOp::SetStateVar { id, var: next }));
    }
}

/// A condition box: the text, and below it what is wrong and where (a squiggle of words, not a dialog).
/// `Some(text)` when a valid text (or none) was left.
fn condition_box(
    ui: &mut Ui,
    session: &Session,
    key: egui::Id,
    current: &str,
    allow_empty: bool,
) -> Option<String> {
    let (response, done) = committed_text(ui, key, current, ui.available_width() - 8.0);
    let typed =
        ui.data(|d| d.get_temp::<(String, String)>(key)).map_or_else(|| current.to_owned(), |(_, t)| t);
    if !(allow_empty && typed.trim().is_empty())
        && let Err(gc_doc::EditError::Expr { message, start, end }) = session.check_condition(&typed)
    {
        let shown: String = typed.get(start..end).unwrap_or("").to_owned();
        ui.colored_label(gc_ui::colors::ERROR, format!("{message} «{shown}»"));
        return None;
    }
    let _ = response;
    done
}

fn task_properties(ui: &mut Ui, session: &mut Session, lang: Lang, id: Id<Task>) {
    use gc_doc::{Effect, EffectTarget, MoveTarget, RobotChoice, Step};
    let Some(task) = session.doc().get(id).cloned() else { return };
    gc_ui::panel::header(ui, &task.name);
    let mut next = task.clone();
    let mut changed = false;
    for problem in session.task_problems(id) {
        ui.colored_label(gc_ui::colors::ERROR, problem);
    }
    // Where the work is.
    ui.horizontal(|ui| {
        ui.label(tr!(lang, WorkPlace));
        changed |= choice(ui, ("gc_task_loc", id), &mut next.location, &location_options(session, lang));
    });
    // When it may start.
    ui.label(tr!(lang, WorkPrecondition));
    if let Some(text) =
        condition_box(ui, session, egui::Id::unique("gc_task_pre").with(id), &task.precondition, true)
    {
        next.precondition = text;
        changed = true;
    }
    // The steps.
    gc_ui::panel::section(ui, tr!(lang, WorkSteps));
    let roles: Vec<(Id<gc_doc::SketchRole>, String, bool)> =
        session.doc().roles().iter().map(|(rid, r)| (rid, r.name.clone(), r.is_path())).collect();
    let way_roles: Vec<(MoveTarget, String)> = roles
        .iter()
        .filter(|(_, _, path)| !path)
        .map(|(rid, name, _)| (MoveTarget::Role(*rid), name.clone()))
        .chain(match task.location {
            gc_doc::Location::Robot(robot) => session
                .doc()
                .waypoints()
                .iter()
                .filter(|(_, w)| w.robot == robot)
                .map(|(wid, w)| (MoveTarget::Waypoint(wid), w.name.clone()))
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect();
    let path_roles: Vec<(Id<gc_doc::SketchRole>, String)> =
        roles.iter().filter(|(_, _, path)| *path).map(|(rid, name, _)| (*rid, name.clone())).collect();
    let objects: Vec<(Option<Id<WorkObject>>, String)> =
        std::iter::once((None, tr!(lang, WorkNoObject).to_owned()))
            .chain(session.doc().objects().iter().map(|(oid, o)| (Some(oid), o.name.clone())))
            .collect();
    // With kinds of tool defined, signals and actions are chosen from theirs (the names are text otherwise).
    let mut out_names: Vec<(String, String)> = Vec::new();
    let mut action_names: Vec<(String, String)> = Vec::new();
    for (_, kit) in session.doc().tool_kits().iter() {
        for signal in kit.signals.iter().filter(|s| s.direction == gc_doc::IoDirection::Out) {
            if !out_names.iter().any(|(n, _)| *n == signal.name) {
                out_names.push((signal.name.clone(), signal.name.clone()));
            }
        }
        for action in &kit.actions {
            if !action_names.iter().any(|(n, _)| *n == action.name) {
                action_names.push((action.name.clone(), action.name.clone()));
            }
        }
    }
    let count = next.steps.len();
    let mut swap: Option<(usize, usize)> = None;
    let mut remove = None;
    for index in 0..count {
        ui.horizontal(|ui| {
            if ui.add_enabled(index > 0, egui::Button::new("↑").small()).clicked() {
                swap = Some((index, index - 1));
            }
            if ui.add_enabled(index + 1 < count, egui::Button::new("↓").small()).clicked() {
                swap = Some((index, index + 1));
            }
            if ui.small_button("✕").clicked() {
                remove = Some(index);
            }
            let salt = ("gc_task_step", id, index);
            match &mut next.steps[index] {
                Step::MoveJ(target) | Step::MoveL(target) => {
                    let joint = matches!(task.steps[index], Step::MoveJ(_));
                    ui.label(if joint { tr!(lang, WorkMoveJ) } else { tr!(lang, WorkMoveL) });
                    changed |= choice(ui, salt, target, &way_roles);
                }
                Step::FollowPath(role) => {
                    ui.label(tr!(lang, WorkFollowPath));
                    changed |= choice(ui, salt, role, &path_roles);
                }
                Step::SetIo { name, value } => {
                    ui.label(tr!(lang, WorkSetIo));
                    if out_names.is_empty() {
                        let key = egui::Id::unique("gc_task_io").with((id, index));
                        let current = name.clone();
                        if let (_, Some(text)) = committed_text(ui, key, &current, 90.0) {
                            *name = text;
                            changed = true;
                        }
                    } else {
                        let mut options = out_names.clone();
                        if !options.iter().any(|(n, _)| n == name) {
                            options.push((name.clone(), name.clone()));
                        }
                        changed |= choice(ui, salt, name, &options);
                    }
                    changed |= ui.checkbox(value, "").changed();
                }
                Step::WaitIo { condition, timeout } => {
                    ui.label(tr!(lang, WorkWaitIo));
                    let key = egui::Id::unique("gc_task_wait_io").with((id, index));
                    let current = condition.clone();
                    if let (_, Some(text)) = committed_text(ui, key, &current, 100.0) {
                        *condition = text;
                        changed = true;
                    }
                    let r = ui.add(egui::DragValue::new(timeout).speed(0.1).range(0.0..=3600.0).suffix(" s"));
                    changed |= r.drag_stopped() || (r.lost_focus() && r.changed());
                }
                Step::ToolAction { action, target } => {
                    ui.label(tr!(lang, WorkToolAction));
                    if action_names.is_empty() {
                        let key = egui::Id::unique("gc_task_tool").with((id, index));
                        let current = action.clone();
                        if let (_, Some(text)) = committed_text(ui, key, &current, 80.0) {
                            *action = text;
                            changed = true;
                        }
                    } else {
                        let mut options = action_names.clone();
                        if !options.iter().any(|(n, _)| n == action) {
                            options.push((action.clone(), action.clone()));
                        }
                        changed |= choice(ui, ("gc_task_tool_name", id, index), action, &options);
                    }
                    changed |= choice(ui, salt, target, &objects);
                }
                Step::Wait { seconds } => {
                    ui.label(tr!(lang, WorkWait));
                    let r = ui.add(egui::DragValue::new(seconds).speed(0.1).range(0.0..=3600.0).suffix(" s"));
                    changed |= r.drag_stopped() || (r.lost_focus() && r.changed());
                }
            }
        });
    }
    if let Some((a, b)) = swap {
        next.steps.swap(a, b);
        changed = true;
    }
    if let Some(index) = remove {
        next.steps.remove(index);
        changed = true;
    }
    // Add a step of a kind (a move needs a waypoint to go to, a follow a path).
    let mut add: Option<Step> = None;
    egui::ComboBox::from_id_salt(("gc_task_add_step", id)).selected_text(tr!(lang, WorkAddStep)).show_ui(
        ui,
        |ui| {
            if let Some((target, _)) = way_roles.first() {
                if ui.selectable_label(false, tr!(lang, WorkMoveJ)).clicked() {
                    add = Some(Step::MoveJ(*target));
                }
                if ui.selectable_label(false, tr!(lang, WorkMoveL)).clicked() {
                    add = Some(Step::MoveL(*target));
                }
            }
            if let Some((role, _)) = path_roles.first()
                && ui.selectable_label(false, tr!(lang, WorkFollowPath)).clicked()
            {
                add = Some(Step::FollowPath(*role));
            }
            if ui.selectable_label(false, tr!(lang, WorkSetIo)).clicked() {
                add = Some(Step::SetIo { name: "DO1".to_owned(), value: true });
            }
            if ui.selectable_label(false, tr!(lang, WorkWaitIo)).clicked() {
                add = Some(Step::WaitIo { condition: "DI1".to_owned(), timeout: 5.0 });
            }
            if ui.selectable_label(false, tr!(lang, WorkToolAction)).clicked() {
                add = Some(Step::ToolAction { action: "close".to_owned(), target: None });
            }
            if ui.selectable_label(false, tr!(lang, WorkWait)).clicked() {
                add = Some(Step::Wait { seconds: 1.0 });
            }
        },
    );
    if let Some(step) = add {
        next.steps.push(step);
        changed = true;
    }
    // What the task changes.
    gc_ui::panel::section(ui, tr!(lang, WorkEffects));
    let targets: Vec<(EffectTarget, String)> = session
        .doc()
        .objects()
        .iter()
        .map(|(oid, o)| (EffectTarget::Object(oid), tr!(lang, WorkEffectObject, o.name)))
        .chain(
            session
                .doc()
                .state_vars()
                .iter()
                .map(|(vid, v)| (EffectTarget::Var(vid), tr!(lang, WorkEffectVar, v.name))),
        )
        .collect();
    let mut remove_effect = None;
    for index in 0..next.effects.len() {
        ui.horizontal(|ui| {
            if ui.small_button("✕").clicked() {
                remove_effect = Some(index);
            }
            changed |= choice(ui, ("gc_task_effect", id, index), &mut next.effects[index].target, &targets);
            ui.label("=");
            let key = egui::Id::unique("gc_task_effect_value").with((id, index));
            let current = next.effects[index].value.clone();
            if let (_, Some(text)) = committed_text(ui, key, &current, 90.0) {
                next.effects[index].value = text;
                changed = true;
            }
        });
    }
    if let Some(index) = remove_effect {
        next.effects.remove(index);
        changed = true;
    }
    if let Some((target, _)) = targets.first()
        && ui.button(tr!(lang, WorkAddEffect)).clicked()
    {
        // The first state of an object, "false" of a variable: the value is typed in the box.
        let value = match target {
            EffectTarget::Object(o) => session
                .doc()
                .get(*o)
                .and_then(|object| session.doc().get(object.ty).and_then(|t| t.states.first().cloned()))
                .unwrap_or_default(),
            // A variable takes a value of its own kind: yes for a yes/no, zero for a number, the first choice.
            EffectTarget::Var(v) => match session.doc().get(*v).map(|var| &var.value) {
                Some(StateValue::Int(_)) => "0".to_owned(),
                Some(StateValue::Real(_)) => "0.0".to_owned(),
                Some(StateValue::Enum { choices, .. }) => choices.first().cloned().unwrap_or_default(),
                Some(StateValue::Bool(_)) | None => "true".to_owned(),
            },
        };
        next.effects.push(Effect { target: *target, value });
        changed = true;
    }
    // Which robots may do it, and how it ranks.
    gc_ui::panel::section(ui, tr!(lang, WorkRobots));
    let robots: Vec<(Id<gc_doc::Robot>, String)> =
        session.doc().robots().iter().map(|(rid, r)| (rid, r.name.clone())).collect();
    let mode = match next.robots {
        RobotChoice::Any => 0,
        RobotChoice::Only(_) => 1,
        RobotChoice::When(_) => 2,
    };
    let mut chosen = mode;
    ui.horizontal(|ui| {
        ui.radio_value(&mut chosen, 0, tr!(lang, WorkRobotsAny));
        ui.radio_value(&mut chosen, 1, tr!(lang, WorkRobotsOnly));
        ui.radio_value(&mut chosen, 2, tr!(lang, WorkRobotsWhen));
    });
    if chosen != mode {
        next.robots = match chosen {
            0 => RobotChoice::Any,
            1 => RobotChoice::Only(robots.iter().map(|(r, _)| *r).collect()),
            _ => RobotChoice::When(String::new()),
        };
        changed = true;
    }
    match &mut next.robots {
        RobotChoice::Any => {}
        RobotChoice::Only(chosen_robots) => {
            for (rid, name) in &robots {
                let mut on = chosen_robots.contains(rid);
                if ui.checkbox(&mut on, name).changed() {
                    if on {
                        chosen_robots.push(*rid);
                    } else {
                        chosen_robots.retain(|r| r != rid);
                    }
                    changed = true;
                }
            }
        }
        RobotChoice::When(text) => {
            let current = text.clone();
            if let Some(done) =
                condition_box(ui, session, egui::Id::unique("gc_task_robot_when").with(id), &current, true)
            {
                *text = done;
                changed = true;
            }
        }
    }
    // Which robot would do it and how long it takes: planned when asked (the planning is not instant, so it
    // is not done every frame).
    let key = egui::Id::unique("gc_task_candidates").with(id);
    if ui.button(tr!(lang, WorkCandidates)).clicked() {
        let table: Vec<(String, Result<f64, String>)> = session
            .task_candidates(id)
            .into_iter()
            .map(|c| {
                (
                    session.doc().get(c.robot).map_or_else(String::new, |r: &gc_doc::Robot| r.name.clone()),
                    c.time,
                )
            })
            .collect();
        ui.data_mut(|d| d.insert_temp(key, table));
    }
    if let Some(table) = ui.data(|d| d.get_temp::<Vec<(String, Result<f64, String>)>>(key)) {
        if table.is_empty() {
            ui.weak(tr!(lang, WorkNoCandidates));
        }
        egui::Grid::new(("gc_task_candidates_grid", id)).num_columns(2).spacing([12.0, 3.0]).show(ui, |ui| {
            for (robot, time) in &table {
                ui.label(robot);
                match time {
                    Ok(seconds) => ui.label(format!("{seconds:.1} s")),
                    Err(why) => ui.colored_label(gc_ui::colors::ERROR, why),
                };
                ui.end_row();
            }
        });
    }
    ui.horizontal(|ui| {
        ui.label(tr!(lang, WorkPriority));
        let r = ui.add(egui::DragValue::new(&mut next.priority));
        changed |= r.drag_stopped() || (r.lost_focus() && r.changed());
    });
    if changed && next != task {
        // A refusal (a condition that does not read, a name taken) shows in the status bar.
        drop(session.edit_work(WorkOp::SetTask { id, task: next }));
    }
}

/// The editor of a kind of tool: its signals (name, direction, pin), its states and its actions.
fn kit_properties(ui: &mut Ui, session: &mut Session, lang: Lang, id: Id<gc_doc::ToolKit>) {
    use gc_doc::{IoDirection, IoSignal, ToolAction};
    let Some(kit) = session.doc().get(id).cloned() else { return };
    gc_ui::panel::header(ui, &kit.name);
    let mut next = kit.clone();
    let mut changed = false;
    // Signals.
    gc_ui::panel::section(ui, tr!(lang, WorkSignals));
    let mut remove = None;
    for index in 0..next.signals.len() {
        ui.horizontal(|ui| {
            let key = egui::Id::unique("gc_kit_signal").with((id, index));
            let current = next.signals[index].name.clone();
            if let (_, Some(text)) = committed_text(ui, key, &current, 80.0) {
                next.signals[index].name = text;
                changed = true;
            }
            let directions = [
                (IoDirection::In, tr!(lang, WorkSignalIn).to_owned()),
                (IoDirection::Out, tr!(lang, WorkSignalOut).to_owned()),
            ];
            changed |= choice(ui, ("gc_kit_dir", id, index), &mut next.signals[index].direction, &directions);
            let r = ui.add(egui::DragValue::new(&mut next.signals[index].pin).range(0..=255).prefix("#"));
            changed |= r.drag_stopped() || (r.lost_focus() && r.changed());
            if ui.small_button("✕").clicked() {
                remove = Some(index);
            }
        });
    }
    if let Some(index) = remove {
        next.signals.remove(index);
        changed = true;
    }
    if ui.button(tr!(lang, WorkAddSignal)).clicked() {
        let taken: Vec<String> = next.signals.iter().map(|s| s.name.clone()).collect();
        let pin = next.signals.iter().map(|s| s.pin).max().map_or(0, |p| p.saturating_add(1));
        next.signals.push(IoSignal {
            name: free_name(tr!(lang, WorkSignalBase), &taken),
            direction: IoDirection::Out,
            pin,
        });
        changed = true;
    }
    // States of the tool (open, closed).
    gc_ui::panel::section(ui, tr!(lang, WorkStates));
    let mut remove = None;
    for index in 0..next.states.len() {
        ui.horizontal(|ui| {
            let key = egui::Id::unique("gc_kit_state").with((id, index));
            let current = next.states[index].clone();
            if let (_, Some(text)) = committed_text(ui, key, &current, 120.0) {
                next.states[index] = text;
                changed = true;
            }
            if ui.small_button("✕").clicked() {
                remove = Some(index);
            }
        });
    }
    if let Some(index) = remove {
        next.states.remove(index);
        changed = true;
    }
    if ui.button(tr!(lang, WorkAddState)).clicked() {
        let taken = next.states.clone();
        next.states.push(free_name(tr!(lang, WorkStateBase), &taken));
        changed = true;
    }
    // Actions.
    gc_ui::panel::section(ui, tr!(lang, WorkActions));
    let outs: Vec<(Option<String>, String)> = std::iter::once((None, tr!(lang, WorkNoSignal).to_owned()))
        .chain(
            kit.signals
                .iter()
                .filter(|s| s.direction == IoDirection::Out)
                .map(|s| (Some(s.name.clone()), s.name.clone())),
        )
        .collect();
    let ins: Vec<(Option<String>, String)> = std::iter::once((None, tr!(lang, WorkNoSignal).to_owned()))
        .chain(
            kit.signals
                .iter()
                .filter(|s| s.direction == IoDirection::In)
                .map(|s| (Some(s.name.clone()), s.name.clone())),
        )
        .collect();
    let tool_states: Vec<(Option<String>, String)> =
        std::iter::once((None, tr!(lang, WorkNoState).to_owned()))
            .chain(kit.states.iter().map(|s| (Some(s.clone()), s.clone())))
            .collect();
    let types: Vec<(Option<Id<ObjectType>>, String)> =
        std::iter::once((None, tr!(lang, WorkNoObject).to_owned()))
            .chain(session.doc().object_types().iter().map(|(t, ty)| (Some(t), ty.name.clone())))
            .collect();
    let mut remove = None;
    for index in 0..next.actions.len() {
        let action = next.actions[index].clone();
        ui.group(|ui| {
            ui.horizontal(|ui| {
                let key = egui::Id::unique("gc_kit_action").with((id, index));
                if let (_, Some(text)) = committed_text(ui, key, &action.name, 110.0) {
                    next.actions[index].name = text;
                    changed = true;
                }
                if ui.small_button("✕").clicked() {
                    remove = Some(index);
                }
            });
            // The signal it sets (and to what), the one that confirms it.
            let mut set_name = action.set.as_ref().map(|(n, _)| n.clone());
            let mut set_value = action.set.as_ref().is_none_or(|(_, v)| *v);
            ui.horizontal(|ui| {
                ui.label(tr!(lang, WorkActionSet));
                let moved = choice(ui, ("gc_kit_set", id, index), &mut set_name, &outs);
                let toggled = ui.checkbox(&mut set_value, "").changed();
                if moved || toggled {
                    next.actions[index].set = set_name.clone().map(|n| (n, set_value));
                    changed = true;
                }
            });
            let mut confirm_name = action.confirm.as_ref().map(|(n, _)| n.clone());
            let mut confirm_value = action.confirm.as_ref().is_none_or(|(_, v)| *v);
            ui.horizontal(|ui| {
                ui.label(tr!(lang, WorkActionConfirm));
                let moved = choice(ui, ("gc_kit_confirm", id, index), &mut confirm_name, &ins);
                let toggled = ui.checkbox(&mut confirm_value, "").changed();
                if moved || toggled {
                    next.actions[index].confirm = confirm_name.clone().map(|n| (n, confirm_value));
                    changed = true;
                }
            });
            let mut tool_state = action.tool_state.clone();
            ui.horizontal(|ui| {
                ui.label(tr!(lang, WorkActionToolState));
                if choice(ui, ("gc_kit_tool_state", id, index), &mut tool_state, &tool_states) {
                    next.actions[index].tool_state = tool_state.clone();
                    changed = true;
                }
            });
            // The kind of object it works on, and the state that object ends in.
            let mut target = action.target;
            ui.horizontal(|ui| {
                ui.label(tr!(lang, WorkActionTarget));
                if choice(ui, ("gc_kit_target", id, index), &mut target, &types) {
                    next.actions[index].target = target;
                    next.actions[index].target_state = None;
                    changed = true;
                }
            });
            if let Some(ty) = target.and_then(|t| session.doc().get(t).cloned()) {
                let object_states: Vec<(Option<String>, String)> =
                    std::iter::once((None, tr!(lang, WorkNoState).to_owned()))
                        .chain(ty.states.iter().map(|s| (Some(s.clone()), s.clone())))
                        .collect();
                let mut target_state = action.target_state.clone();
                ui.horizontal(|ui| {
                    ui.label(tr!(lang, WorkActionTargetState));
                    if choice(ui, ("gc_kit_target_state", id, index), &mut target_state, &object_states) {
                        next.actions[index].target_state = target_state.clone();
                        changed = true;
                    }
                });
            }
            // When it may be done (tool.state, target.state and the variables), and how long it takes.
            ui.label(tr!(lang, WorkPrecondition));
            let key = egui::Id::unique("gc_kit_pre").with((id, index));
            if let Some(text) = condition_box(ui, session, key, &action.precondition, true) {
                next.actions[index].precondition = text;
                changed = true;
            }
            ui.horizontal(|ui| {
                ui.label(tr!(lang, WorkActionDuration));
                let r = ui.add(
                    egui::DragValue::new(&mut next.actions[index].duration)
                        .speed(0.05)
                        .range(0.0..=3600.0)
                        .suffix(" s"),
                );
                changed |= r.drag_stopped() || (r.lost_focus() && r.changed());
                ui.label(tr!(lang, WorkActionTimeout));
                let r = ui.add(
                    egui::DragValue::new(&mut next.actions[index].timeout)
                        .speed(0.1)
                        .range(0.0..=3600.0)
                        .suffix(" s"),
                );
                changed |= r.drag_stopped() || (r.lost_focus() && r.changed());
            });
        });
    }
    if let Some(index) = remove {
        next.actions.remove(index);
        changed = true;
    }
    if ui.button(tr!(lang, WorkAddAction)).clicked() {
        let taken: Vec<String> = next.actions.iter().map(|a| a.name.clone()).collect();
        next.actions.push(ToolAction {
            name: free_name(tr!(lang, WorkActionBase), &taken),
            target: None,
            precondition: String::new(),
            set: None,
            confirm: None,
            tool_state: None,
            target_state: None,
            duration: 0.0,
            timeout: 0.0,
        });
        changed = true;
    }
    if changed && next != kit {
        // A refusal (a name twice, a signal used twice, an action still used by a task) shows in the status bar.
        drop(session.edit_work(WorkOp::SetToolKit { id, kit: next }));
    }
}

/// The tool kind fitted to a robot: a list to choose from, and the state it starts in. A refusal shows in
/// the status bar.
pub fn robot_tool_kit(ui: &mut Ui, session: &mut Session, lang: Lang, robot: Id<gc_doc::Robot>) {
    let fitted = session.robot_tool_kit(robot);
    // Nothing to choose from while no kind of tool is defined.
    if fitted.is_none() && session.doc().tool_kits().iter().next().is_none() {
        return;
    }
    let kits: Vec<(Option<Id<gc_doc::ToolKit>>, String)> =
        std::iter::once((None, tr!(lang, WorkNoKit).to_owned()))
            .chain(session.doc().tool_kits().iter().map(|(k, kit)| (Some(k), kit.name.clone())))
            .collect();
    let mut kit = fitted.as_ref().map(|(k, _)| *k);
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(tr!(lang, WorkFitKit));
        changed |= choice(ui, ("gc_robot_kit", robot), &mut kit, &kits);
    });
    let mut state = fitted.as_ref().and_then(|(_, s)| s.clone());
    if let Some(kit_id) = kit
        && let Some(def) = session.doc().get(kit_id).cloned()
        && !def.states.is_empty()
    {
        let states: Vec<(Option<String>, String)> =
            def.states.iter().map(|s| (Some(s.clone()), s.clone())).collect();
        if state.is_none() {
            state = def.states.first().cloned();
        }
        ui.horizontal(|ui| {
            ui.label(tr!(lang, WorkToolStateNow));
            changed |= choice(ui, ("gc_robot_kit_state", robot), &mut state, &states);
        });
    } else {
        state = None;
    }
    if changed {
        drop(session.edit_work(WorkOp::SetRobotTool { robot, kit, state }));
    }
}
