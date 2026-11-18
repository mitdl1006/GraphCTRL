//! The work model (plan 9.4; GraphCTRL's own, no Inventor counterpart): what is worked on and what the
//! work is, defined on the structure and not on a robot.
//!
//! - An [`ObjectType`] names the states its objects can be in (a hole: empty, fastened).
//! - A [`WorkObject`] is one such thing at a place, with the state it starts in.
//! - A [`StateVar`] is a state that belongs to no object.
//! - A [`Task`] is work: where it is, when it may start (a condition over the states), its steps (moves to
//!   waypoint roles, along path roles, signals, waits), what it changes, and which robots may do it.
//!
//! Conditions are text (`gc_expr::Condition`): `"Hole A".state == fastened and count < 3`. They are
//! checked when they are written and after every change of the work model; names in them follow renames.
//! This is data only: nothing here schedules or runs a task.

use gc_core::{AnyId, Entity, Id, Kind};
use gc_expr::{CondScope, CondValue, Condition, Span};
use gc_sketch::Sketch;
use serde::{Deserialize, Serialize};

use crate::cell::{CellOp, Robot, Waypoint, WorkFrame};
use crate::command::{Created, EditError};
use crate::design::Occurrence;
use crate::document::Document;
use crate::role::SketchRole;
use crate::txn::Txn;

/// A type of work object: the states its objects can be in.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObjectType {
    /// Name shown in the browser.
    pub name: String,
    /// The states, at least one, each once; a new object starts in the first.
    pub states: Vec<String>,
}

impl Entity for ObjectType {
    const KIND: Kind = Kind::ObjectType;
    fn refs(&self, _: &mut Vec<AnyId>) {}
}

/// Where an object or a task is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Location {
    /// Nowhere in particular.
    #[default]
    None,
    /// A component of the assembly (a part of the equipment); it follows the component.
    Component(Occurrence),
    /// A work frame.
    Frame(Id<WorkFrame>),
    /// A sketch (its plane).
    Sketch(Id<Sketch>),
    /// A robot itself: a task there is that robot's own motion.
    Robot(Id<Robot>),
}

impl Location {
    fn refs(self, out: &mut Vec<AnyId>) {
        match self {
            Self::None => {}
            Self::Component(Occurrence::Item(item)) => out.push(item),
            Self::Component(Occurrence::Part { assembly, .. }) => out.push(assembly.erase()),
            Self::Component(Occurrence::Link { robot, .. } | Occurrence::Tool { robot })
            | Self::Robot(robot) => out.push(robot.erase()),
            Self::Frame(frame) => out.push(frame.erase()),
            Self::Sketch(sketch) => out.push(sketch.erase()),
        }
    }

    fn exists(self, doc: &Document) -> bool {
        match self {
            Self::None => true,
            Self::Component(occurrence) => doc.has_occurrence(occurrence),
            Self::Frame(frame) => doc.get(frame).is_some(),
            Self::Sketch(sketch) => doc.get(sketch).is_some(),
            Self::Robot(robot) => doc.get(robot).is_some(),
        }
    }
}

/// A thing work is done on, with the state it starts in.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkObject {
    /// Name; conditions refer to it (`name.state`).
    pub name: String,
    /// Its type.
    pub ty: Id<ObjectType>,
    /// Where it is.
    #[serde(default)]
    pub location: Location,
    /// The state it starts in: one of its type's.
    pub state: String,
}

impl Entity for WorkObject {
    const KIND: Kind = Kind::WorkObject;
    fn refs(&self, out: &mut Vec<AnyId>) {
        out.push(self.ty.erase());
        self.location.refs(out);
    }
}

/// The value a state variable starts with, which also says what kind it is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum StateValue {
    /// True or false.
    Bool(bool),
    /// A whole number.
    Int(i64),
    /// A number.
    Real(f64),
    /// One of a list of names.
    Enum {
        /// The names, at least one, each once.
        choices: Vec<String>,
        /// The one it starts as.
        value: String,
    },
}

/// A state that belongs to no object.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StateVar {
    /// Name; conditions refer to it by this word.
    pub name: String,
    /// What it starts as.
    pub value: StateValue,
}

impl Entity for StateVar {
    const KIND: Kind = Kind::StateVar;
    fn refs(&self, _: &mut Vec<AnyId>) {}
}

/// Where a move step goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoveTarget {
    /// A waypoint role of a sketch point: any robot can be sent there.
    Role(Id<SketchRole>),
    /// A robot's own waypoint (joint values, a pose in its base...): only in a task located at that
    /// robot ([`Location::Robot`]).
    Waypoint(Id<Waypoint>),
}

/// One step of a task.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Step {
    /// A joint move to a waypoint.
    MoveJ(MoveTarget),
    /// A straight move of the tool tip to a waypoint.
    MoveL(MoveTarget),
    /// Along a path role, as the role says.
    FollowPath(Id<SketchRole>),
    /// Sets an output signal.
    SetIo {
        /// The signal's name.
        name: String,
        /// On or off.
        value: bool,
    },
    /// Waits for signals.
    WaitIo {
        /// What to wait for, as text (signals have no model yet; it is kept as written).
        condition: String,
        /// Give up after this many seconds.
        timeout: f64,
    },
    /// Has the tool do something.
    ToolAction {
        /// The action's name (tool actions have no model yet).
        action: String,
        /// The object it is done to, if any.
        target: Option<Id<WorkObject>>,
    },
    /// Waits.
    Wait {
        /// Seconds.
        seconds: f64,
    },
}

/// What an effect changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectTarget {
    /// An object's state.
    Object(Id<WorkObject>),
    /// A state variable.
    Var(Id<StateVar>),
}

/// A change a task makes when it is done.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Effect {
    /// What changes.
    pub target: EffectTarget,
    /// What it becomes: a state of the object's type, or a value of the variable's kind (`true`, `3`,
    /// `0.5`, one of its names).
    pub value: String,
}

/// Which robots may do a task.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum RobotChoice {
    /// Any robot.
    #[default]
    Any,
    /// These robots (at least one).
    Only(Vec<Id<Robot>>),
    /// Any robot while this condition over the states holds.
    When(String),
}

/// Work defined on the structure, for whichever robot is given it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Task {
    /// Name shown in the browser.
    pub name: String,
    /// Where the work is. At a robot ([`Location::Robot`]) the task is that robot's own motion: only it
    /// can do it (`robots` is not looked at) and its steps may go to its own waypoints.
    #[serde(default)]
    pub location: Location,
    /// When it may start: a condition over the states; empty for always.
    #[serde(default)]
    pub precondition: String,
    /// What is done, in order.
    #[serde(default)]
    pub steps: Vec<Step>,
    /// What it changes when it is done.
    #[serde(default)]
    pub effects: Vec<Effect>,
    /// Which robots may do it.
    #[serde(default)]
    pub robots: RobotChoice,
    /// Higher goes first.
    #[serde(default)]
    pub priority: i32,
}

impl Entity for Task {
    const KIND: Kind = Kind::Task;
    fn refs(&self, out: &mut Vec<AnyId>) {
        self.location.refs(out);
        for step in &self.steps {
            match step {
                Step::MoveJ(MoveTarget::Role(role))
                | Step::MoveL(MoveTarget::Role(role))
                | Step::FollowPath(role) => out.push(role.erase()),
                Step::MoveJ(MoveTarget::Waypoint(waypoint)) | Step::MoveL(MoveTarget::Waypoint(waypoint)) => {
                    out.push(waypoint.erase());
                }
                Step::ToolAction { target: Some(object), .. } => out.push(object.erase()),
                Step::ToolAction { target: None, .. }
                | Step::SetIo { .. }
                | Step::WaitIo { .. }
                | Step::Wait { .. } => {}
            }
        }
        for effect in &self.effects {
            match effect.target {
                EffectTarget::Object(object) => out.push(object.erase()),
                EffectTarget::Var(var) => out.push(var.erase()),
            }
        }
        if let RobotChoice::Only(robots) = &self.robots {
            out.extend(robots.iter().map(|r| r.erase()));
        }
    }
}

impl Task {
    /// The conditions the task holds, as text: its precondition and its robots' condition.
    fn conditions(&self) -> impl Iterator<Item = &String> {
        let when = match &self.robots {
            RobotChoice::When(text) => Some(text),
            RobotChoice::Any | RobotChoice::Only(_) => None,
        };
        std::iter::once(&self.precondition).chain(when).filter(|text| !text.trim().is_empty())
    }

    fn conditions_mut(&mut self) -> impl Iterator<Item = &mut String> {
        let when = match &mut self.robots {
            RobotChoice::When(text) => Some(text),
            RobotChoice::Any | RobotChoice::Only(_) => None,
        };
        std::iter::once(&mut self.precondition).chain(when).filter(|text| !text.trim().is_empty())
    }

    /// True when a step goes to or along the role.
    pub fn uses_role(&self, role: Id<SketchRole>) -> bool {
        self.steps.iter().any(|step| {
            matches!(step, Step::MoveJ(MoveTarget::Role(r)) | Step::MoveL(MoveTarget::Role(r)) | Step::FollowPath(r) if *r == role)
        })
    }
}

/// Changes of the work model. Each is one undo step. A change that would leave a condition, an effect or
/// a step pointing at something that is gone is refused (`InUse`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum WorkOp {
    /// Adds an object type.
    AddObjectType(ObjectType),
    /// Replaces an object type. With the same number of states, a state whose name changed is renamed
    /// wherever it is used (objects' starting states, effects, conditions); a state that is left out must
    /// not be used.
    SetObjectType {
        /// The type.
        id: Id<ObjectType>,
        /// Its new data.
        ty: ObjectType,
    },
    /// Deletes an object type no object has.
    DeleteObjectType(Id<ObjectType>),
    /// Adds an object.
    AddObject(WorkObject),
    /// Replaces an object; a new name is followed by the conditions that refer to it.
    SetObject {
        /// The object.
        id: Id<WorkObject>,
        /// Its new data.
        object: WorkObject,
    },
    /// Deletes an object no condition, effect or step refers to.
    DeleteObject(Id<WorkObject>),
    /// Adds a state variable.
    AddStateVar(StateVar),
    /// Replaces a state variable; a new name is followed by the conditions that refer to it.
    SetStateVar {
        /// The variable.
        id: Id<StateVar>,
        /// Its new data.
        var: StateVar,
    },
    /// Deletes a state variable no condition or effect refers to.
    DeleteStateVar(Id<StateVar>),
    /// Adds a task.
    AddTask(Task),
    /// Replaces a task.
    SetTask {
        /// The task.
        id: Id<Task>,
        /// Its new data.
        task: Task,
    },
    /// Deletes a task.
    DeleteTask(Id<Task>),
    /// How many times a task is done: `None` for once (the default), `Some(0)` for as often as its
    /// precondition comes to hold, `Some(n)` for up to `n` times.
    SetTaskRepeat {
        /// The task.
        task: Id<Task>,
        /// How many times at most.
        times: Option<u32>,
    },
    /// Adds a tool kit.
    AddToolKit(crate::toolkit::ToolKit),
    /// Replaces a tool kit. With the same number of actions (signals), one whose name changed is renamed
    /// in the steps that use it, unless another tool kit still has the old name.
    SetToolKit {
        /// The tool kit.
        id: Id<crate::toolkit::ToolKit>,
        /// Its new data.
        kit: crate::toolkit::ToolKit,
    },
    /// Deletes a tool kit no robot is fitted with and whose actions and signals no step needs.
    DeleteToolKit(Id<crate::toolkit::ToolKit>),
    /// Fits a robot with a tool kit, or takes it off (`None`).
    SetRobotTool {
        /// The robot.
        robot: Id<Robot>,
        /// The tool kit.
        kit: Option<Id<crate::toolkit::ToolKit>>,
        /// The state the tool starts in; `None` for the kit's first.
        state: Option<String>,
    },
}

impl CondScope for Document {
    fn variable(&self, name: &str) -> Option<CondValue> {
        let (_, var) = self.state_vars().iter().find(|(_, v)| v.name == name)?;
        Some(match &var.value {
            StateValue::Bool(b) => CondValue::Bool(*b),
            #[allow(clippy::cast_precision_loss, reason = "counts, far below 2^53")]
            StateValue::Int(i) => CondValue::Number(*i as f64),
            StateValue::Real(x) => CondValue::Number(*x),
            StateValue::Enum { value, .. } => CondValue::Name(value.clone()),
        })
    }

    fn member(&self, object: &str, field: &str) -> Option<CondValue> {
        let (_, object) = self.objects().iter().find(|(_, o)| o.name == object)?;
        (field == "state").then(|| CondValue::Name(object.state.clone()))
    }
}

fn expr_error(error: &gc_expr::CondError) -> EditError {
    EditError::Expr { message: error.message.clone(), start: error.span.start, end: error.span.end }
}

impl Document {
    /// Checks a condition against the work model: it must read, every object, field and word in it must
    /// be known (a word is a state variable, a state of some object type or a name of some enumeration)
    /// and it must work out to true or false for the starting states. The error says where it is wrong.
    pub fn check_condition(&self, text: &str) -> Result<(), EditError> {
        let condition = Condition::parse(text).map_err(|e| expr_error(&e))?;
        let mut unknown: Option<(String, Span)> = None;
        condition.walk(&mut |part| {
            if unknown.is_some() {
                return;
            }
            match part {
                Condition::Word(word, span) => {
                    let known = self.state_vars().iter().any(|(_, v)| {
                        v.name == *word
                            || matches!(&v.value, StateValue::Enum { choices, .. } if choices.contains(word))
                    }) || self.object_types().iter().any(|(_, t)| t.states.contains(word));
                    if !known {
                        unknown = Some((format!("`{word}` is not known"), *span));
                    }
                }
                Condition::Member { object, field, span, .. } if self.member(object, field).is_none() => {
                    unknown = Some((format!("`{object}.{field}` is not known"), *span));
                }
                _ => {}
            }
        });
        if let Some((message, span)) = unknown {
            return Err(EditError::Expr { message, start: span.start, end: span.end });
        }
        condition.eval(self).map(|_| ()).map_err(|e| expr_error(&e))
    }

    /// Whether a condition holds for the starting states; empty text holds.
    pub fn condition_holds(&self, text: &str) -> Result<bool, EditError> {
        if text.trim().is_empty() {
            return Ok(true);
        }
        self.check_condition(text)?;
        Condition::parse(text).and_then(|c| c.eval(self)).map_err(|e| expr_error(&e))
    }

    /// True when a task's steps go to or along the role.
    pub fn role_in_task(&self, role: Id<SketchRole>) -> bool {
        self.tasks().iter().any(|(_, t)| t.uses_role(role))
    }

    /// What is wrong with a task as the document stands, in words; empty when nothing is. (An edit that
    /// would break a task is refused, so this finds only what other edits left behind: a role whose
    /// sketch elements were deleted.)
    pub fn task_problems(&self, task: Id<Task>) -> Vec<String> {
        let Some(task) = self.get(task) else { return Vec::new() };
        let mut problems = Vec::new();
        if let Err(error) = task_ok(self, task) {
            problems.push(error.to_string());
        }
        // Without any tool kit the steps' names are just names (documents from before tool kits).
        if !self.tool_kits().is_empty() {
            problems.extend(self.tool_step_problems(task));
        }
        for step in &task.steps {
            let role = match step {
                Step::MoveJ(MoveTarget::Role(r)) | Step::MoveL(MoveTarget::Role(r)) | Step::FollowPath(r) => {
                    *r
                }
                _ => continue,
            };
            let broken = match self.get(role) {
                None => true,
                Some(r) if r.is_path() => self.role_runs(role).is_none(),
                Some(_) => self.role_pose(role).is_none(),
            };
            if broken {
                let name = self.get(role).map(|r| r.name.clone()).unwrap_or_default();
                problems.push(format!("{name}: its sketch elements no longer exist"));
            }
        }
        problems
    }
}

fn name_ok(name: &str) -> Result<(), EditError> {
    if gc_expr::is_valid_state_name(name) { Ok(()) } else { Err(EditError::BadName(name.to_owned())) }
}

/// Names of a list: valid, each once, at least one.
fn names_ok(names: &[String]) -> Result<(), EditError> {
    if names.is_empty() {
        return Err(EditError::BadValue);
    }
    for (index, name) in names.iter().enumerate() {
        name_ok(name)?;
        if names[..index].contains(name) {
            return Err(EditError::NameTaken);
        }
    }
    Ok(())
}

fn type_ok(doc: &Document, ty: &ObjectType, except: Option<Id<ObjectType>>) -> Result<(), EditError> {
    name_ok(&ty.name)?;
    if doc.object_types().iter().any(|(id, t)| Some(id) != except && t.name == ty.name) {
        return Err(EditError::NameTaken);
    }
    names_ok(&ty.states)
}

fn object_ok(doc: &Document, object: &WorkObject, except: Option<Id<WorkObject>>) -> Result<(), EditError> {
    name_ok(&object.name)?;
    // In a tool action's condition these two stand for the tool and the object it is done to.
    if matches!(object.name.as_str(), "tool" | "target") {
        return Err(EditError::BadName(object.name.clone()));
    }
    if doc.objects().iter().any(|(id, o)| Some(id) != except && o.name == object.name) {
        return Err(EditError::NameTaken);
    }
    let ty = doc.get(object.ty).ok_or(EditError::BadValue)?;
    if !ty.states.contains(&object.state) || !object.location.exists(doc) {
        return Err(EditError::BadValue);
    }
    Ok(())
}

fn var_ok(doc: &Document, var: &StateVar, except: Option<Id<StateVar>>) -> Result<(), EditError> {
    name_ok(&var.name)?;
    if doc.state_vars().iter().any(|(id, v)| Some(id) != except && v.name == var.name) {
        return Err(EditError::NameTaken);
    }
    match &var.value {
        StateValue::Bool(_) | StateValue::Int(_) => Ok(()),
        StateValue::Real(x) if x.is_finite() => Ok(()),
        StateValue::Real(_) => Err(EditError::BadValue),
        StateValue::Enum { choices, value } => {
            names_ok(choices)?;
            if choices.contains(value) { Ok(()) } else { Err(EditError::BadValue) }
        }
    }
}

/// The number an effect gives a numeric state variable: `text` is an expression over numbers and the
/// numeric state variables (`count + 1`), which `lookup` gives the values of. The error says where it is
/// wrong (bytes of `text`).
pub fn effect_number(text: &str, lookup: &dyn Fn(&str) -> Option<f64>) -> Result<f64, EditError> {
    struct Numbers<'a>(&'a dyn Fn(&str) -> Option<f64>);
    impl gc_expr::Scope for Numbers<'_> {
        fn lookup(&self, name: &str) -> Option<gc_expr::Quantity> {
            (self.0)(name).map(gc_expr::Quantity::scalar)
        }
    }
    let wrong = |message: String, span: Span| EditError::Expr { message, start: span.start, end: span.end };
    let expr = gc_expr::parse(text).map_err(|e| wrong(e.kind.to_string(), e.span))?;
    let value = expr
        .eval_as(&Numbers(lookup), gc_expr::Unit::Scalar)
        .map_err(|e| wrong(e.kind.to_string(), e.span))?;
    if value.is_finite() { Ok(value) } else { Err(wrong("this is not a number".to_owned(), expr.span())) }
}

impl Document {
    /// The starting value of a numeric state variable (whole numbers and numbers).
    fn number_of(&self, name: &str) -> Option<f64> {
        match self.variable(name)? {
            CondValue::Number(value) => Some(value),
            CondValue::Bool(_) | CondValue::Name(_) => None,
        }
    }

    /// How many times a task is done at most: `None` for once, `Some(0)` for as often as its precondition
    /// comes to hold, `Some(n)` for up to `n` times.
    pub fn task_repeat(&self, task: Id<Task>) -> Option<u32> {
        let (_, timeline) = self.timeline()?;
        self.get(task)?;
        timeline.components.task_repeats.iter().find(|(id, _)| *id == task).map(|(_, times)| *times)
    }
}

/// Checks the value an effect gives a variable of this kind.
fn value_fits(doc: &Document, kind: &StateValue, text: &str) -> Result<(), EditError> {
    let fits = match kind {
        StateValue::Bool(_) => matches!(text.trim(), "true" | "false"),
        // A number, or an expression over the numeric state variables.
        StateValue::Int(_) | StateValue::Real(_) => {
            return effect_number(text, &|name| doc.number_of(name)).map(|_| ());
        }
        StateValue::Enum { choices, .. } => choices.iter().any(|c| c == text.trim()),
    };
    if fits { Ok(()) } else { Err(EditError::BadValue) }
}

fn task_ok(doc: &Document, task: &Task) -> Result<(), EditError> {
    if task.name.trim().is_empty() || !task.location.exists(doc) {
        return Err(EditError::BadValue);
    }
    for text in task.conditions() {
        doc.check_condition(text)?;
    }
    let own_robot = match task.location {
        Location::Robot(robot) => Some(robot),
        _ => None,
    };
    for step in &task.steps {
        let fine = match step {
            Step::MoveJ(MoveTarget::Role(role)) | Step::MoveL(MoveTarget::Role(role)) => {
                doc.get(*role).is_some_and(|r| !r.is_path())
            }
            // A robot's own waypoint only in that robot's own task.
            Step::MoveJ(MoveTarget::Waypoint(w)) | Step::MoveL(MoveTarget::Waypoint(w)) => {
                doc.get(*w).is_some_and(|w| Some(w.robot) == own_robot)
            }
            Step::FollowPath(role) => doc.get(*role).is_some_and(SketchRole::is_path),
            Step::SetIo { name, .. } => !name.trim().is_empty(),
            Step::WaitIo { condition, timeout } => {
                !condition.trim().is_empty() && timeout.is_finite() && *timeout > 0.0
            }
            Step::ToolAction { action, target } => {
                !action.trim().is_empty() && target.is_none_or(|t| doc.get(t).is_some())
            }
            Step::Wait { seconds } => seconds.is_finite() && *seconds >= 0.0,
        };
        if !fine {
            return Err(EditError::BadValue);
        }
    }
    for effect in &task.effects {
        let fine = match effect.target {
            EffectTarget::Object(object) => doc
                .get(object)
                .and_then(|o| doc.get(o.ty))
                .is_some_and(|ty| ty.states.contains(&effect.value)),
            EffectTarget::Var(var) => {
                let var = doc.get(var).ok_or(EditError::BadValue)?;
                value_fits(doc, &var.value, &effect.value)?;
                true
            }
        };
        if !fine {
            return Err(EditError::BadValue);
        }
    }
    match &task.robots {
        // "While this holds" needs something to hold.
        RobotChoice::When(text) if text.trim().is_empty() => Err(EditError::BadValue),
        RobotChoice::Any | RobotChoice::When(_) => Ok(()),
        RobotChoice::Only(robots) => {
            let distinct = robots.iter().enumerate().all(|(i, r)| !robots[..i].contains(r));
            if robots.is_empty() || !distinct || robots.iter().any(|r| doc.get(*r).is_none()) {
                Err(EditError::BadValue)
            } else {
                Ok(())
            }
        }
    }
}

/// After a change of the work model: every task must still be sound; what it would break is in use.
pub(crate) fn still_sound(doc: &Document) -> Result<(), EditError> {
    let objects_fit =
        doc.objects().iter().all(|(_, o)| doc.get(o.ty).is_some_and(|ty| ty.states.contains(&o.state)));
    if !objects_fit || doc.tasks().iter().any(|(_, t)| task_ok(doc, t).is_err()) {
        return Err(EditError::InUse);
    }
    Ok(())
}

/// Rewrites every condition of every task with `rewrite` (the parsed condition and its source give the new
/// source, or `None` to leave it).
fn rewrite_conditions(
    txn: &mut Txn<'_>,
    rewrite: &dyn Fn(&Document, &Condition, &str) -> Option<String>,
) -> Result<(), EditError> {
    let mut changes: Vec<(Id<Task>, Task)> = Vec::new();
    for (id, task) in txn.doc().tasks().iter() {
        let mut edited = task.clone();
        for text in edited.conditions_mut() {
            if let Ok(parsed) = Condition::parse(text)
                && let Some(new) = rewrite(txn.doc(), &parsed, text)
            {
                *text = new;
            }
        }
        if edited != *task {
            changes.push((id, edited));
        }
    }
    for (id, task) in changes {
        txn.modify(id, |t: &mut Task, _| *t = task)?;
    }
    Ok(())
}

/// The source with the text at each span replaced by `new` (written as a condition writes a name).
fn replaced(source: &str, mut spans: Vec<Span>, new: &str) -> Option<String> {
    if spans.is_empty() {
        return None;
    }
    spans.sort_by_key(|span| span.start);
    let written = gc_expr::quoted(new);
    let mut out = String::with_capacity(source.len());
    let mut at = 0;
    for span in spans {
        out.push_str(source.get(at..span.start)?);
        out.push_str(&written);
        at = span.end;
    }
    out.push_str(source.get(at..)?);
    Some(out)
}

/// A state of an object type gets a new name: in the objects that start in it, in effects, and in
/// conditions where it is compared with the state of an object of that type.
fn rename_state(txn: &mut Txn<'_>, ty: Id<ObjectType>, old: &str, new: &str) -> Result<(), EditError> {
    let of_type: Vec<Id<WorkObject>> =
        txn.doc().objects().iter().filter(|(_, o)| o.ty == ty).map(|(id, _)| id).collect();
    for id in &of_type {
        txn.modify(*id, |o: &mut WorkObject, _| {
            if o.state == old {
                new.clone_into(&mut o.state);
            }
        })?;
    }
    let tasks: Vec<Id<Task>> = txn.doc().tasks().iter().map(|(id, _)| id).collect();
    for id in tasks {
        txn.modify(id, |t: &mut Task, _| {
            for effect in &mut t.effects {
                if matches!(effect.target, EffectTarget::Object(o) if of_type.contains(&o))
                    && effect.value == old
                {
                    new.clone_into(&mut effect.value);
                }
            }
        })?;
    }
    rewrite_conditions(txn, &|doc, parsed, source| {
        let is_of_type = |part: &Condition| match part {
            Condition::Member { object, .. } => {
                doc.objects().iter().any(|(id, o)| o.name == *object && of_type.contains(&id))
            }
            _ => false,
        };
        let mut spans = Vec::new();
        parsed.walk(&mut |part| {
            if let Condition::Compare(_, a, b) = part {
                for (state, other) in [(a, b), (b, a)] {
                    if let Condition::Word(word, span) = &**state
                        && word == old
                        && is_of_type(other)
                    {
                        spans.push(*span);
                    }
                }
            }
        });
        replaced(source, spans, new)
    })
}

/// Renames an object type, an object or a state variable from the browser (`Command::Rename`).
pub(crate) fn rename(
    txn: &mut Txn<'_>,
    item: AnyId,
    name: &str,
) -> Result<(Vec<Created>, &'static str), EditError> {
    let doc = txn.doc();
    let op = match item.kind {
        Kind::ObjectType => {
            let id = Id::<ObjectType>::from_raw(item.raw);
            let ty = doc.get(id).ok_or(EditError::BadValue)?;
            WorkOp::SetObjectType { id, ty: ObjectType { name: name.to_owned(), ..ty.clone() } }
        }
        Kind::WorkObject => {
            let id = Id::<WorkObject>::from_raw(item.raw);
            let object = doc.get(id).ok_or(EditError::BadValue)?;
            WorkOp::SetObject { id, object: WorkObject { name: name.to_owned(), ..object.clone() } }
        }
        Kind::StateVar => {
            let id = Id::<StateVar>::from_raw(item.raw);
            let var = doc.get(id).ok_or(EditError::BadValue)?;
            WorkOp::SetStateVar { id, var: StateVar { name: name.to_owned(), ..var.clone() } }
        }
        _ => return Err(EditError::BadValue),
    };
    run(txn, &op).map(|(created, _)| (created, "Rename"))
}

pub(crate) fn run(txn: &mut Txn<'_>, op: &WorkOp) -> Result<(Vec<Created>, &'static str), EditError> {
    let (created, label) = match op {
        WorkOp::AddObjectType(ty) => {
            type_ok(txn.doc(), ty, None)?;
            (vec![Created::ObjectType(txn.insert(ty.clone()))], "Add object type")
        }
        WorkOp::SetObjectType { id, ty } => {
            type_ok(txn.doc(), ty, Some(*id))?;
            let old = txn.doc().get(*id).ok_or(EditError::BadValue)?.clone();
            txn.modify(*id, |t: &mut ObjectType, _| t.clone_from(ty))?;
            // The same number of states: the ones whose names differ were renamed.
            if old.states.len() == ty.states.len() {
                let renames: Vec<(&String, &String)> =
                    old.states.iter().zip(&ty.states).filter(|(a, b)| a != b).collect();
                // Renamed through names no state has, so that two states can swap names.
                for (index, (from, _)) in renames.iter().enumerate() {
                    rename_state(txn, *id, from, &format!("\u{1}{index}"))?;
                }
                for (index, (_, to)) in renames.iter().enumerate() {
                    rename_state(txn, *id, &format!("\u{1}{index}"), to)?;
                }
            }
            (Vec::new(), "Edit object type")
        }
        WorkOp::DeleteObjectType(id) => {
            if txn.doc().objects().iter().any(|(_, o)| o.ty == *id) {
                return Err(EditError::InUse);
            }
            txn.remove(*id)?;
            (Vec::new(), "Delete object type")
        }
        WorkOp::AddObject(object) => {
            object_ok(txn.doc(), object, None)?;
            (vec![Created::WorkObject(txn.insert(object.clone()))], "Add object")
        }
        WorkOp::SetObject { id, object } => {
            object_ok(txn.doc(), object, Some(*id))?;
            let old = txn.doc().get(*id).ok_or(EditError::BadValue)?.name.clone();
            txn.modify(*id, |o: &mut WorkObject, _| o.clone_from(object))?;
            if old != object.name {
                rewrite_conditions(txn, &|_, parsed, source| {
                    parsed.renamed(source, true, &old, &object.name)
                })?;
            }
            (Vec::new(), "Edit object")
        }
        WorkOp::DeleteObject(id) => {
            // Its name in a condition, or an effect or a step on it, keeps it.
            let name = txn.doc().get(*id).ok_or(EditError::BadValue)?.name.clone();
            let used = txn.doc().tasks().iter().any(|(_, t)| {
                let mut refs = Vec::new();
                t.refs(&mut refs);
                refs.contains(&id.erase())
                    || t.conditions().any(|text| {
                        Condition::parse(text).is_ok_and(|c| c.renamed(text, true, &name, "x").is_some())
                    })
            });
            if used {
                return Err(EditError::InUse);
            }
            txn.remove(*id)?;
            (Vec::new(), "Delete object")
        }
        WorkOp::AddStateVar(var) => {
            var_ok(txn.doc(), var, None)?;
            (vec![Created::StateVar(txn.insert(var.clone()))], "Add state")
        }
        WorkOp::SetStateVar { id, var } => {
            var_ok(txn.doc(), var, Some(*id))?;
            let old = txn.doc().get(*id).ok_or(EditError::BadValue)?.name.clone();
            txn.modify(*id, |v: &mut StateVar, _| v.clone_from(var))?;
            if old != var.name {
                rewrite_conditions(txn, &|_, parsed, source| parsed.renamed(source, false, &old, &var.name))?;
                // Effects that work a number out from it follow too.
                let tasks: Vec<Id<Task>> = txn.doc().tasks().iter().map(|(id, _)| id).collect();
                for task in tasks {
                    let numeric: Vec<bool> = txn
                        .doc()
                        .get(task)
                        .map(|t| {
                            t.effects
                                .iter()
                                .map(|e| match e.target {
                                    EffectTarget::Var(v) => txn.doc().get(v).is_some_and(|v| {
                                        matches!(v.value, StateValue::Int(_) | StateValue::Real(_))
                                    }),
                                    EffectTarget::Object(_) => false,
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    txn.modify(task, |t: &mut Task, _| {
                        for (effect, numeric) in t.effects.iter_mut().zip(&numeric) {
                            if *numeric
                                && let Some(new) = crate::command::rename_in(&effect.value, &old, &var.name)
                            {
                                effect.value = new;
                            }
                        }
                    })?;
                }
            }
            (Vec::new(), "Edit state")
        }
        WorkOp::DeleteStateVar(id) => {
            let name = txn.doc().get(*id).ok_or(EditError::BadValue)?.name.clone();
            let used = txn.doc().tasks().iter().any(|(_, t)| {
                t.effects.iter().any(|e| e.target == EffectTarget::Var(*id))
                    || t.conditions().any(|text| {
                        Condition::parse(text).is_ok_and(|c| c.renamed(text, false, &name, "x").is_some())
                    })
            });
            if used {
                return Err(EditError::InUse);
            }
            txn.remove(*id)?;
            (Vec::new(), "Delete state")
        }
        WorkOp::AddTask(task) => {
            task_ok(txn.doc(), task)?;
            (vec![Created::Task(txn.insert(task.clone()))], "Add task")
        }
        WorkOp::SetTask { id, task } => {
            task_ok(txn.doc(), task)?;
            txn.modify(*id, |t: &mut Task, _| t.clone_from(task))?;
            (Vec::new(), "Edit task")
        }
        WorkOp::DeleteTask(id) => {
            txn.remove(*id)?;
            (Vec::new(), "Delete task")
        }
        WorkOp::SetTaskRepeat { task, times } => {
            txn.doc().get(*task).ok_or(EditError::BadValue)?;
            let doc_tasks: Vec<Id<Task>> = txn.doc().tasks().iter().map(|(id, _)| id).collect();
            crate::cell::set_timeline(txn, |t| {
                // Tasks that are gone drop out here too.
                t.components.task_repeats.retain(|(id, _)| *id != *task && doc_tasks.contains(id));
                t.components.task_repeats.extend(times.map(|times| (*task, times)));
            })?;
            (Vec::new(), "Repeat task")
        }
        WorkOp::AddToolKit(_)
        | WorkOp::SetToolKit { .. }
        | WorkOp::DeleteToolKit(_)
        | WorkOp::SetRobotTool { .. } => crate::toolkit::run(txn, op)?,
    };
    still_sound(txn.doc())?;
    Ok((created, label))
}

/// The operation as a cell operation.
impl From<WorkOp> for CellOp {
    fn from(op: WorkOp) -> Self {
        CellOp::Work(op)
    }
}
