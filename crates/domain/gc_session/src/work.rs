//! The work model in a session (plan 9.4): object types, objects, state variables and tasks. Data only:
//! nothing here schedules or runs a task. See `gc_doc::WorkOp`.

use gc_core::{AnyId, Id};
use gc_doc::{Command, Created, EditError, Location, Task, WorkOp};
use gc_math::DPose3;

use crate::Session;

impl Session {
    /// Changes the work model: adds, replaces or deletes an object type, an object, a state variable or
    /// a task. One undo step; saved with the document. Adding returns what was made. Refused with
    /// `InUse` when a condition, an effect or a step would be left pointing at something that is gone,
    /// with `NameTaken`/`BadName` for names, with `Expr` (and where) for a condition that does not read.
    pub fn edit_work(&mut self, op: WorkOp) -> Result<Option<AnyId>, EditError> {
        let created = self.execute(&Command::Cell(gc_doc::CellOp::Work(op)))?;
        Ok(created.first().and_then(|c| match c {
            Created::ObjectType(id) => Some(id.erase()),
            Created::WorkObject(id) => Some(id.erase()),
            Created::StateVar(id) => Some(id.erase()),
            Created::Task(id) => Some(id.erase()),
            _ => None,
        }))
    }

    /// Checks a condition as it is typed, against the work model: `Err(EditError::Expr { message, start,
    /// end })` says what is wrong and where (bytes of the text). Empty text is a mistake here; a task's
    /// empty precondition means "always" and is not checked.
    pub fn check_condition(&self, text: &str) -> Result<(), EditError> {
        self.doc().check_condition(text)
    }

    /// Whether a task's precondition holds for the starting states (an empty one always does).
    pub fn task_ready(&self, task: Id<Task>) -> Result<bool, EditError> {
        let task = self.doc().get(task).ok_or(EditError::BadValue)?;
        self.doc().condition_holds(&task.precondition)
    }

    /// What is wrong with a task as the document stands (a role whose sketch elements were deleted), in
    /// words; empty when nothing is.
    pub fn task_problems(&self, task: Id<Task>) -> Vec<String> {
        self.doc().task_problems(task)
    }

    /// The tasks in the order they were made (the order does not change when one is edited).
    pub fn tasks(&self) -> Vec<Id<Task>> {
        self.doc().tasks().iter().map(|(id, _)| id).collect()
    }

    /// The world pose of where an object or a task is: the component's origin, the work frame, the
    /// sketch's plane, the robot's base frame. `None` for nowhere or something that no longer exists.
    pub fn location_pose(&self, location: &Location) -> Option<DPose3> {
        match *location {
            Location::None => None,
            Location::Component(occurrence) => Some(self.component_origin(occurrence)?.pose),
            Location::Frame(frame) => self.work_frame_pose(frame),
            Location::Sketch(sketch) => {
                let s = self.doc().get(sketch)?;
                let (gc_sketch::Space::Planar(frame) | gc_sketch::Space::Spatial(frame)) = s.space;
                self.doc().frame_world_pose(Id::from_raw(frame.raw))
            }
            Location::Robot(robot) => self.robot_base_frame(robot),
        }
    }
}
