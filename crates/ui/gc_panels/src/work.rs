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
}

impl WorkItem {
    fn any(self) -> AnyId {
        match self {
            Self::Type(id) => id.erase(),
            Self::Object(id) => id.erase(),
            Self::Var(id) => id.erase(),
            Self::Task(id) => id.erase(),
        }
    }

    fn delete(self) -> WorkOp {
        match self {
            Self::Type(id) => WorkOp::DeleteObjectType(id),
            Self::Object(id) => WorkOp::DeleteObject(id),
            Self::Var(id) => WorkOp::DeleteStateVar(id),
            Self::Task(id) => WorkOp::DeleteTask(id),
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
