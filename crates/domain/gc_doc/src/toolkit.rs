//! Tools as devices (plan 9.4, GraphCTRL's own): a [`ToolKit`] is a kind of tool (a gripper, a screw
//! driver) with its signal names, its own states and the actions it can do; robots are fitted with one.
//! A task's `ToolAction` step names an action; the robot that does the task must carry a tool kit that
//! has it. The tool's shape and tool centre point stay with the robot (`Robot::tool`, `Robot::tcp`).
//! Data only: no signal is sent.

use gc_core::{AnyId, Entity, Id, Kind};
use gc_expr::{CondScope, CondValue, Condition};
use serde::{Deserialize, Serialize};

use crate::cell::Robot;
use crate::command::{Created, EditError};
use crate::document::Document;
use crate::txn::Txn;
use crate::work::{ObjectType, Step, Task, WorkOp};

/// Which way a signal goes, seen from the robot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IoDirection {
    /// The robot reads it.
    In,
    /// The robot sets it.
    Out,
}

/// A named digital signal of a tool.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IoSignal {
    /// The name steps refer to.
    pub name: String,
    /// In or out.
    pub direction: IoDirection,
    /// The controller's pin number.
    pub pin: u8,
}

/// Something a tool does to an object (or by itself).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolAction {
    /// The name `ToolAction` steps refer to.
    pub name: String,
    /// The type of object it is done to; `None` when it needs no object.
    #[serde(default)]
    pub target: Option<Id<ObjectType>>,
    /// When it can be done: a condition over the states, where `tool.state` is the tool's own state and
    /// `target.state` the object's; empty for always.
    #[serde(default)]
    pub precondition: String,
    /// The output signal it sets, and to what.
    #[serde(default)]
    pub set: Option<(String, bool)>,
    /// The input signal that confirms it, and the value that does.
    #[serde(default)]
    pub confirm: Option<(String, bool)>,
    /// The tool's state when it is done.
    #[serde(default)]
    pub tool_state: Option<String>,
    /// The object's state when it is done.
    #[serde(default)]
    pub target_state: Option<String>,
    /// How long it takes, seconds.
    #[serde(default)]
    pub duration: f64,
    /// Give up waiting for the confirmation after this many seconds (0: do not wait).
    #[serde(default)]
    pub timeout: f64,
}

/// A kind of tool as a device.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolKit {
    /// Name shown in the browser.
    pub name: String,
    /// Its signals.
    #[serde(default)]
    pub signals: Vec<IoSignal>,
    /// The states the tool itself can be in (open, closed); a tool fitted to a robot starts in the first.
    /// May be empty.
    #[serde(default)]
    pub states: Vec<String>,
    /// What it can do.
    #[serde(default)]
    pub actions: Vec<ToolAction>,
}

impl Entity for ToolKit {
    const KIND: Kind = Kind::ToolKit;
    fn refs(&self, out: &mut Vec<AnyId>) {
        out.extend(self.actions.iter().filter_map(|a| a.target).map(|t| t.erase()));
    }
}

impl ToolKit {
    /// The action of this name.
    pub fn action(&self, name: &str) -> Option<&ToolAction> {
        self.actions.iter().find(|a| a.name == name)
    }

    /// True when the tool has an output signal of this name.
    pub fn has_output(&self, name: &str) -> bool {
        self.signals.iter().any(|s| s.name == name && s.direction == IoDirection::Out)
    }
}

/// A robot fitted with a tool kit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FittedTool {
    /// The robot.
    pub robot: Id<Robot>,
    /// The tool kit.
    pub kit: Id<ToolKit>,
    /// The state the tool starts in; `None` for the kit's first state.
    #[serde(default)]
    pub state: Option<String>,
}

/// The states an action's precondition sees: the document's, with `tool.state` and `target.state`.
pub struct ActionScope<'a> {
    /// Everything else.
    pub outer: &'a dyn CondScope,
    /// The tool's state.
    pub tool: Option<&'a str>,
    /// The target object's state.
    pub target: Option<&'a str>,
}

impl CondScope for ActionScope<'_> {
    fn variable(&self, name: &str) -> Option<CondValue> {
        self.outer.variable(name)
    }

    fn member(&self, object: &str, field: &str) -> Option<CondValue> {
        let own = match (object, field) {
            ("tool", "state") => self.tool,
            ("target", "state") => self.target,
            _ => return self.outer.member(object, field),
        };
        own.map(|state| CondValue::Name(state.to_owned()))
    }
}

impl Document {
    /// The tool kit a robot is fitted with and the state its tool starts in (`None` for a kit without
    /// states).
    pub fn robot_tool_kit(&self, robot: Id<Robot>) -> Option<(Id<ToolKit>, Option<String>)> {
        let (_, timeline) = self.timeline()?;
        let fitted = timeline.components.tool_kits.iter().find(|f| f.robot == robot)?;
        let kit = self.get(fitted.kit)?;
        self.get(robot)?;
        let state =
            fitted.state.clone().filter(|s| kit.states.contains(s)).or_else(|| kit.states.first().cloned());
        Some((fitted.kit, state))
    }

    /// The tool kits that have an action of this name.
    pub fn kits_with_action(&self, action: &str) -> Vec<Id<ToolKit>> {
        self.tool_kits().iter().filter(|(_, k)| k.action(action).is_some()).map(|(id, _)| id).collect()
    }

    /// What a task's tool steps lack as the document stands: an action or an output signal no tool kit
    /// has, an object of the wrong type for the action.
    pub(crate) fn tool_step_problems(&self, task: &Task) -> Vec<String> {
        let mut problems = Vec::new();
        for step in &task.steps {
            match step {
                Step::ToolAction { action, target } => {
                    let kits = self.kits_with_action(action);
                    if kits.is_empty() {
                        problems.push(format!("no tool has the action `{action}`"));
                        continue;
                    }
                    let object_type = target.and_then(|t| self.get(t)).map(|o| o.ty);
                    let fits = kits.iter().any(|k| {
                        self.get(*k).and_then(|k| k.action(action)).is_some_and(|a| a.target == object_type)
                    });
                    if !fits {
                        problems.push(format!("`{action}` is not done to this kind of object"));
                    }
                }
                Step::SetIo { name, .. } if !self.tool_kits().iter().any(|(_, k)| k.has_output(name)) => {
                    problems.push(format!("no tool has the output signal `{name}`"));
                }
                Step::WaitIo { condition, .. } if signal_condition(condition).is_none() => {
                    problems.push(format!("`{condition}` is not a signal and a value to wait for"));
                }
                _ => {}
            }
        }
        problems
    }
}

/// A signal wait read as "this signal has this value": `ready`, `not ready`, `ready == true`,
/// `"door closed" == false` (a name with spaces in double quotes). `None` for anything else.
pub fn signal_condition(text: &str) -> Option<(String, bool)> {
    let truth = |part: &Condition| match part {
        Condition::Literal(CondValue::Bool(value), _) => Some(*value),
        _ => None,
    };
    match Condition::parse(text).ok()? {
        Condition::Word(name, _) => Some((name, true)),
        Condition::Not(inner, _) => match *inner {
            Condition::Word(name, _) => Some((name, false)),
            _ => None,
        },
        Condition::Compare(op, a, b) => {
            let equal = match op {
                gc_expr::Compare::Eq => true,
                gc_expr::Compare::Ne => false,
                _ => return None,
            };
            match (&*a, &*b) {
                (Condition::Word(name, _), other) | (other, Condition::Word(name, _)) => {
                    Some((name.clone(), truth(other)? == equal))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

impl Document {
    /// The signals the cell itself drives: those a task's step sets and those a tool action sets. A wait
    /// for one of them waits for a task to have set it; a wait for any other signal (a sensor, a device)
    /// is taken to be answered.
    pub fn driven_signals(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for (_, task) in self.tasks().iter() {
            for step in &task.steps {
                if let Step::SetIo { name, .. } = step {
                    names.push(name.clone());
                }
            }
        }
        for (_, kit) in self.tool_kits().iter() {
            names.extend(kit.actions.iter().filter_map(|a| a.set.as_ref()).map(|(name, _)| name.clone()));
        }
        names.sort();
        names.dedup();
        names
    }
}

fn name_ok(name: &str) -> Result<(), EditError> {
    if gc_expr::is_valid_state_name(name) { Ok(()) } else { Err(EditError::BadName(name.to_owned())) }
}

fn unique<'a>(names: impl Iterator<Item = &'a String>) -> Result<(), EditError> {
    let mut seen: Vec<&String> = Vec::new();
    for name in names {
        name_ok(name)?;
        if seen.contains(&name) {
            return Err(EditError::NameTaken);
        }
        seen.push(name);
    }
    Ok(())
}

/// Checks an action's precondition: it reads, its words are known (the kit's states and the target
/// type's count) and it works out for some states.
fn action_condition_ok(doc: &Document, kit: &ToolKit, action: &ToolAction) -> Result<(), EditError> {
    let text = &action.precondition;
    if text.trim().is_empty() {
        return Ok(());
    }
    let wrong =
        |message: String, span: gc_expr::Span| EditError::Expr { message, start: span.start, end: span.end };
    let parsed = Condition::parse(text).map_err(|e| wrong(e.message.clone(), e.span))?;
    let target_states = action.target.and_then(|t| doc.get(t)).map(|t| t.states.clone()).unwrap_or_default();
    let scope = ActionScope {
        outer: doc,
        tool: kit.states.first().map(String::as_str),
        target: target_states.first().map(String::as_str),
    };
    let mut unknown = None;
    parsed.walk(&mut |part| {
        if unknown.is_some() {
            return;
        }
        match part {
            Condition::Word(word, span)
                if !(kit.states.contains(word)
                    || target_states.contains(word)
                    || doc.variable(word).is_some()
                    || doc.object_types().iter().any(|(_, t)| t.states.contains(word))) =>
            {
                unknown = Some((format!("`{word}` is not known"), *span));
            }
            Condition::Member { object, field, span, .. } if scope.member(object, field).is_none() => {
                unknown = Some((format!("`{object}.{field}` is not known"), *span));
            }
            _ => {}
        }
    });
    if let Some((message, span)) = unknown {
        return Err(wrong(message, span));
    }
    parsed.eval(&scope).map(|_| ()).map_err(|e| wrong(e.message.clone(), e.span))
}

fn kit_ok(doc: &Document, kit: &ToolKit, except: Option<Id<ToolKit>>) -> Result<(), EditError> {
    name_ok(&kit.name)?;
    if doc.tool_kits().iter().any(|(id, k)| Some(id) != except && k.name == kit.name) {
        return Err(EditError::NameTaken);
    }
    unique(kit.signals.iter().map(|s| &s.name))?;
    // One signal per pin and direction.
    let pins_clash =
        kit.signals.iter().enumerate().any(|(index, s)| {
            kit.signals[..index].iter().any(|o| o.pin == s.pin && o.direction == s.direction)
        });
    if pins_clash {
        return Err(EditError::BadValue);
    }
    unique(kit.states.iter())?;
    unique(kit.actions.iter().map(|a| &a.name))?;
    for action in &kit.actions {
        let target = match action.target {
            Some(ty) => Some(doc.get(ty).ok_or(EditError::BadValue)?),
            None => None,
        };
        let signal = |wanted: &Option<(String, bool)>, direction| {
            wanted.as_ref().is_none_or(|(name, _)| {
                kit.signals.iter().any(|s| s.name == *name && s.direction == direction)
            })
        };
        let fine = signal(&action.set, IoDirection::Out)
            && signal(&action.confirm, IoDirection::In)
            && action.tool_state.as_ref().is_none_or(|s| kit.states.contains(s))
            && action.target_state.as_ref().is_none_or(|s| target.is_some_and(|t| t.states.contains(s)))
            && action.duration.is_finite()
            && action.duration >= 0.0
            && action.timeout.is_finite()
            && action.timeout >= 0.0;
        if !fine {
            return Err(EditError::BadValue);
        }
        action_condition_ok(doc, kit, action)?;
    }
    Ok(())
}

/// The names steps use that the tool kits provide: actions and output signals.
fn provided(doc: &Document) -> (Vec<String>, Vec<String>) {
    let mut actions = Vec::new();
    let mut outputs = Vec::new();
    for (_, kit) in doc.tool_kits().iter() {
        actions.extend(kit.actions.iter().map(|a| a.name.clone()));
        outputs
            .extend(kit.signals.iter().filter(|s| s.direction == IoDirection::Out).map(|s| s.name.clone()));
    }
    (actions, outputs)
}

/// The names of actions and output signals the tasks' steps use.
fn used(doc: &Document) -> (Vec<String>, Vec<String>) {
    let mut actions = Vec::new();
    let mut outputs = Vec::new();
    for (_, task) in doc.tasks().iter() {
        for step in &task.steps {
            match step {
                Step::ToolAction { action, .. } => actions.push(action.clone()),
                Step::SetIo { name, .. } => outputs.push(name.clone()),
                _ => {}
            }
        }
    }
    (actions, outputs)
}

/// Renames in every task's steps: actions (`action: true`) or output signals.
fn rename_in_steps(txn: &mut Txn<'_>, action: bool, old: &str, new: &str) -> Result<(), EditError> {
    let tasks: Vec<Id<Task>> = txn.doc().tasks().iter().map(|(id, _)| id).collect();
    for id in tasks {
        txn.modify(id, |t: &mut Task, _| {
            for step in &mut t.steps {
                match step {
                    Step::ToolAction { action: name, .. } if action && name == old => new.clone_into(name),
                    Step::SetIo { name, .. } if !action && name == old => new.clone_into(name),
                    _ => {}
                }
            }
        })?;
    }
    Ok(())
}

/// Runs the tool kit operations of [`WorkOp`]; any other is refused.
pub(crate) fn run(txn: &mut Txn<'_>, op: &WorkOp) -> Result<(Vec<Created>, &'static str), EditError> {
    // What the steps could rely on before: none of it may go missing.
    let (had_actions, had_outputs) = provided(txn.doc());
    let (created, label) = match op {
        WorkOp::AddToolKit(kit) => {
            kit_ok(txn.doc(), kit, None)?;
            (vec![Created::ToolKit(txn.insert(kit.clone()))], "Add tool")
        }
        WorkOp::SetToolKit { id, kit } => {
            kit_ok(txn.doc(), kit, Some(*id))?;
            let old = txn.doc().get(*id).ok_or(EditError::BadValue)?.clone();
            txn.modify(*id, |k: &mut ToolKit, _| k.clone_from(kit))?;
            // The same number of actions (signals): those whose names differ were renamed, and the steps
            // follow unless another tool still has the old name.
            let (now_actions, now_outputs) = provided(txn.doc());
            if old.actions.len() == kit.actions.len() {
                for (from, to) in old.actions.iter().zip(&kit.actions) {
                    if from.name != to.name && !now_actions.contains(&from.name) {
                        rename_in_steps(txn, true, &from.name, &to.name)?;
                    }
                }
            }
            if old.signals.len() == kit.signals.len() {
                for (from, to) in old.signals.iter().zip(&kit.signals) {
                    if from.name != to.name && !now_outputs.contains(&from.name) {
                        rename_in_steps(txn, false, &from.name, &to.name)?;
                    }
                }
            }
            (Vec::new(), "Edit tool")
        }
        WorkOp::DeleteToolKit(id) => {
            let fitted = txn
                .doc()
                .timeline()
                .is_some_and(|(_, t)| t.components.tool_kits.iter().any(|f| f.kit == *id));
            if fitted {
                return Err(EditError::InUse);
            }
            txn.remove(*id)?;
            (Vec::new(), "Delete tool")
        }
        WorkOp::SetRobotTool { robot, kit, state } => {
            let doc = txn.doc();
            doc.get(*robot).ok_or(EditError::BadValue)?;
            let fitted = match kit {
                None => None,
                Some(kit_id) => {
                    let kit = doc.get(*kit_id).ok_or(EditError::BadValue)?;
                    if state.as_ref().is_some_and(|s| !kit.states.contains(s)) {
                        return Err(EditError::BadValue);
                    }
                    Some(FittedTool { robot: *robot, kit: *kit_id, state: state.clone() })
                }
            };
            crate::cell::set_timeline(txn, |t| {
                // Robots and kits that are gone drop out here too.
                t.components.tool_kits.retain(|f| f.robot != *robot);
                t.components.tool_kits.extend(fitted);
            })?;
            let doc = txn.doc();
            let stale: Vec<Id<Robot>> = doc
                .timeline()
                .map(|(_, t)| {
                    t.components
                        .tool_kits
                        .iter()
                        .filter(|f| doc.get(f.robot).is_none() || doc.get(f.kit).is_none())
                        .map(|f| f.robot)
                        .collect()
                })
                .unwrap_or_default();
            if !stale.is_empty() {
                crate::cell::set_timeline(txn, |t| {
                    t.components.tool_kits.retain(|f| !stale.contains(&f.robot))
                })?;
            }
            (Vec::new(), if kit.is_some() { "Fit tool" } else { "Remove tool" })
        }
        _ => return Err(EditError::BadValue),
    };
    // An action or an output signal a step uses must not go missing.
    let (now_actions, now_outputs) = provided(txn.doc());
    let (used_actions, used_outputs) = used(txn.doc());
    let lost = |had: &[String], now: &[String], used: &[String]| {
        used.iter().any(|name| had.contains(name) && !now.contains(name))
    };
    if lost(&had_actions, &now_actions, &used_actions) || lost(&had_outputs, &now_outputs, &used_outputs) {
        return Err(EditError::InUse);
    }
    Ok((created, label))
}
