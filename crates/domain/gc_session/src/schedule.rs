//! A minimal scheduler over the work model (plan 9.5), for looking at a timeline: synchronous and
//! deterministic, nothing is run. Every task is done once. At each moment the tasks whose precondition
//! holds are taken by priority (higher first), then in the order they were made; each goes to the idle
//! robot that would finish it soonest from where that robot then is (its bid: the planned time of the
//! task's motions plus its waits). When a task ends, its effects change the states, which can make other
//! tasks ready.
//!
//! Robots keep clear of each other: a robot that stands idle is an obstacle where it stands for the one
//! being planned, and a planned motion is checked, every 50 ms of it, against the robots that are moving
//! at that time. A motion that would come too close waits: the task stays ready and is tried again when
//! the next task ends. (Checked at those moments, not in between; the padding covers the rest.)

use std::collections::BTreeMap;

use gc_core::Id;
use gc_doc::{EffectTarget, Robot, StateValue, Task};
use gc_expr::{CondScope, CondValue, Condition};

use crate::cell::Planning;

/// A task given to a robot for a stretch of time.
#[derive(Clone, Debug, PartialEq)]
pub struct Assignment {
    /// The task.
    pub task: Id<Task>,
    /// The robot that does it.
    pub robot: Id<Robot>,
    /// When it starts, seconds from the start of the schedule.
    pub start: f64,
    /// When it ends.
    pub end: f64,
}

/// What [`Session::schedule`] worked out. Two schedules are equal when they give the same tasks to the
/// same robots at the same times and block the same tasks for the same reasons.
#[derive(Clone, Debug, Default)]
pub struct Schedule {
    /// The tasks that are done, in the order they start (ties: priority, then the order made).
    pub assignments: Vec<Assignment>,
    /// The tasks that are never done, each with why: its precondition never holds, or no robot can do it
    /// (with the robot's reason).
    pub blocked: Vec<(Id<Task>, String)>,
    /// The planned motions of each assignment, in the same order.
    pub plans: Vec<crate::TaskPlan>,
}

impl PartialEq for Schedule {
    fn eq(&self, other: &Self) -> bool {
        self.assignments == other.assignments && self.blocked == other.blocked
    }
}

impl Schedule {
    /// The joint values of a robot at `time` seconds into the schedule: along the motion of the task it
    /// is doing, or standing where its last task left it. `None` before its first task (it is where the
    /// document has it) and for a robot without tasks.
    pub fn joints_at(&self, robot: Id<Robot>, time: f64) -> Option<Vec<f64>> {
        let (assignment, plan) = self
            .assignments
            .iter()
            .zip(&self.plans)
            .filter(|(a, _)| a.robot == robot && a.start <= time)
            .max_by(|(a, _), (b, _)| a.start.total_cmp(&b.start))?;
        Some(plan.sample(time - assignment.start))
    }

    /// Every robot that has tasks, with its joint values at `time` ([`Schedule::joints_at`]; before its
    /// first task, where that task starts from).
    pub fn robots_at(&self, time: f64) -> Vec<(Id<Robot>, Vec<f64>)> {
        let mut robots: Vec<Id<Robot>> = self.assignments.iter().map(|a| a.robot).collect();
        robots.sort();
        robots.dedup();
        robots
            .into_iter()
            .filter_map(|robot| {
                let first = || {
                    let (_, plan) =
                        self.assignments.iter().zip(&self.plans).find(|(a, _)| a.robot == robot)?;
                    Some(plan.start.clone())
                };
                Some((robot, self.joints_at(robot, time).or_else(first)?))
            })
            .collect()
    }

    /// When the last task ends.
    pub fn makespan(&self) -> f64 {
        self.assignments.iter().map(|a| a.end).fold(0.0, f64::max)
    }
}

/// True when a robot's planned motion, started at `start`, comes closer than the padding to another robot
/// that is where `other_at` says at each moment (checked every 50 ms). Unknown shapes count as crossing.
fn crosses(
    scene: Option<&gc_motion::RobotScene>,
    plan: &crate::TaskPlan,
    start: f64,
    other: Option<&gc_motion::RobotScene>,
    other_at: &dyn Fn(f64) -> Option<Vec<f64>>,
) -> bool {
    const STEP: f64 = 0.05;
    let (Some(scene), Some(other)) = (scene, other) else { return true };
    let duration = plan.duration();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "a count of 50 ms steps")]
    let steps = (duration / STEP).ceil().clamp(1.0, 100_000.0) as usize;
    (0..=steps).any(|k| {
        #[allow(clippy::cast_precision_loss, reason = "a count of 50 ms steps")]
        let t = (k as f64 * STEP).min(duration);
        let Some(theirs) = other_at(start + t) else { return true };
        let mine = scene.geometry(&plan.sample(t));
        match mine.closest(&other.geometry(&theirs)) {
            Ok(Some(closest)) => closest.distance < crate::PADDING,
            Ok(None) => false,
            Err(_) => true,
        }
    })
}

/// A tool's state after a task's tool actions, and the states they leave their objects in.
type ToolOutcome = (Option<String>, Vec<(String, String)>);

/// The states as the schedule goes along.
struct World {
    variables: BTreeMap<String, CondValue>,
    objects: BTreeMap<String, String>,
}

impl CondScope for World {
    fn variable(&self, name: &str) -> Option<CondValue> {
        self.variables.get(name).cloned()
    }

    fn member(&self, object: &str, field: &str) -> Option<CondValue> {
        (field == "state").then(|| self.objects.get(object).cloned().map(CondValue::Name)).flatten()
    }
}

impl World {
    fn holds(&self, condition: &str) -> bool {
        condition.trim().is_empty() || Condition::parse(condition).is_ok_and(|c| c.eval(self) == Ok(true))
    }
}

impl Planning<'_> {
    /// The states the document starts with.
    fn starting_world(&self) -> World {
        let doc = self.doc;
        World {
            variables: doc
                .state_vars()
                .iter()
                .filter_map(|(_, v)| Some((v.name.clone(), doc.variable(&v.name)?)))
                .collect(),
            objects: doc.objects().iter().map(|(_, o)| (o.name.clone(), o.state.clone())).collect(),
        }
    }

    /// Changes the states as a finished task says.
    fn apply_effects(&self, world: &mut World, task: &Task) {
        let doc = self.doc;
        for effect in &task.effects {
            match effect.target {
                EffectTarget::Object(object) => {
                    if let Some(object) = doc.get(object) {
                        world.objects.insert(object.name.clone(), effect.value.clone());
                    }
                }
                EffectTarget::Var(var) => {
                    let Some(var) = doc.get(var) else { continue };
                    let text = effect.value.trim();
                    let value = match &var.value {
                        StateValue::Bool(_) => Some(CondValue::Bool(text == "true")),
                        StateValue::Int(_) | StateValue::Real(_) => text.parse().ok().map(CondValue::Number),
                        StateValue::Enum { .. } => Some(CondValue::Name(text.to_owned())),
                    };
                    if let Some(value) = value {
                        world.variables.insert(var.name.clone(), value);
                    }
                }
            }
        }
    }

    /// Works out who does what when (see the module's description). Suppressed tasks are left out.
    /// Stops early, with what it has, when the job is cancelled.
    pub(crate) fn schedule(&self) -> Schedule {
        let doc = self.doc;
        let mut world = self.starting_world();
        let mut pending: Vec<Id<Task>> =
            doc.tasks().iter().map(|(id, _)| id).filter(|id| !doc.is_inactive(id.erase())).collect();
        // Higher priority first; the sort keeps the order made among equals.
        pending.sort_by_key(|id| std::cmp::Reverse(doc.get(*id).map_or(0, |t| t.priority)));
        // Where each robot is and when it is free.
        let mut robots: BTreeMap<Id<Robot>, (Vec<f64>, f64)> =
            doc.robots().iter().map(|(id, r)| (id, (r.joints.clone(), 0.0))).collect();
        // Bids already worked out: a task for a robot from given joint values.
        type Bid = Result<crate::TaskPlan, String>;
        type BidKey = (Id<Task>, Id<Robot>, Vec<u64>, Vec<(Id<Robot>, Vec<u64>)>);
        // Each robot's own scene, for standing in another's way and for checking two motions.
        let scenes: BTreeMap<Id<Robot>, gc_motion::RobotScene> =
            doc.robots().iter().filter_map(|(id, _)| Some((id, self.scene_of(id).ok()?))).collect();
        let bits = |joints: &[f64]| joints.iter().map(|q| q.to_bits()).collect::<Vec<u64>>();
        let mut bids: BTreeMap<BidKey, Bid> = BTreeMap::new();
        // The state of each robot's tool, and what the tool actions of a running task do to their objects
        // when it ends.
        let mut tools: BTreeMap<Id<Robot>, Option<String>> = doc
            .robots()
            .iter()
            .map(|(id, _)| (id, doc.robot_tool_kit(id).and_then(|(_, state)| state)))
            .collect();
        let mut changes: BTreeMap<Id<Task>, Vec<(String, String)>> = BTreeMap::new();
        let mut running: Vec<Assignment> = Vec::new();
        let mut out = Schedule::default();
        let mut reasons: BTreeMap<Id<Task>, String> = BTreeMap::new();
        let mut now = 0.0_f64;
        loop {
            if self.control.cancelled() {
                break;
            }
            let mut index = 0;
            while index < pending.len() {
                let id = pending[index];
                let Some(task) = doc.get(id) else {
                    pending.remove(index);
                    continue;
                };
                if !world.holds(&task.precondition) {
                    reasons.insert(id, "its precondition does not hold".to_owned());
                    index += 1;
                    continue;
                }
                let allowed = self.task_robots_in(id, &world);
                if allowed.is_empty() {
                    reasons.insert(id, "no robot may do it".to_owned());
                }
                // The idle robot that would finish soonest; the first of equals.
                let mut best: Option<(crate::TaskPlan, ToolOutcome)> = None;
                for robot in allowed {
                    let Some((joints, free_at)) = robots.get(&robot) else { continue };
                    if *free_at > now {
                        reasons.entry(id).or_insert_with(|| "its robots are busy".to_owned());
                        continue;
                    }
                    // The robots that stand idle are in the way where they stand.
                    let parked: Vec<(Id<Robot>, &Vec<f64>)> = robots
                        .iter()
                        .filter(|(other, (_, free))| **other != robot && *free <= now)
                        .map(|(other, (at, _))| (*other, at))
                        .collect();
                    let key =
                        (id, robot, bits(joints), parked.iter().map(|(o, at)| (*o, bits(at))).collect());
                    let bid = bids.entry(key).or_insert_with(|| {
                        let others: Vec<gc_motion::Obstacle> = parked
                            .iter()
                            .filter_map(|(other, at)| {
                                let name = doc.get(*other).map(|r| r.name.clone())?;
                                Some(scenes.get(other)?.as_obstacles(at, &name))
                            })
                            .flatten()
                            .collect();
                        self.task_plan_among(id, robot, joints, &others)
                    });
                    // The robots that are moving must be clear of this motion all the way.
                    if let Ok(plan) = bid.as_ref() {
                        let moving: Vec<Id<Robot>> = robots
                            .iter()
                            .filter(|(other, (_, free))| **other != robot && *free > now)
                            .map(|(other, _)| *other)
                            .collect();
                        if let Some(other) = moving.into_iter().find(|other| {
                            let at = |time: f64| out.joints_at(*other, time);
                            crosses(scenes.get(&robot), plan, now, scenes.get(other), &at)
                        }) {
                            let name = doc.get(other).map(|r| r.name.clone()).unwrap_or_default();
                            reasons.insert(id, format!("it waits for {name} to be out of the way"));
                            continue;
                        }
                    }
                    // Its tool must be in a state to do the task's tool actions.
                    let tool = tools.get(&robot).cloned().flatten();
                    let outcome = match self.tool_outcome(task, robot, &world, tool) {
                        Ok(outcome) => outcome,
                        Err(why) => {
                            reasons.insert(id, why);
                            continue;
                        }
                    };
                    match bid {
                        Ok(plan) if best.as_ref().is_none_or(|(b, _)| plan.duration() < b.duration()) => {
                            best = Some((plan.clone(), outcome));
                        }
                        Ok(_) => {}
                        Err(why) => {
                            reasons.insert(id, why.clone());
                        }
                    }
                }
                match best {
                    Some((plan, (tool, objects))) => {
                        let robot = plan.robot;
                        let assignment =
                            Assignment { task: id, robot, start: now, end: now + plan.duration() };
                        robots.insert(robot, (plan.end.clone(), assignment.end));
                        out.plans.push(plan);
                        tools.insert(robot, tool);
                        changes.insert(id, objects);
                        running.push(assignment.clone());
                        out.assignments.push(assignment);
                        reasons.remove(&id);
                        pending.remove(index);
                    }
                    None => index += 1,
                }
            }
            // On to the moment the next task ends; its effects may make others ready.
            let Some(next) = running.iter().map(|a| a.end).min_by(f64::total_cmp) else { break };
            now = next;
            let (ended, still): (Vec<Assignment>, Vec<Assignment>) =
                running.into_iter().partition(|a| a.end <= now);
            running = still;
            for assignment in ended {
                // What its tool actions did to their objects, then the task's own effects.
                world.objects.extend(changes.remove(&assignment.task).unwrap_or_default());
                if let Some(task) = doc.get(assignment.task) {
                    self.apply_effects(&mut world, task);
                }
            }
        }
        out.blocked = pending
            .into_iter()
            .map(|id| (id, reasons.remove(&id).unwrap_or_else(|| "it cannot be done".to_owned())))
            .collect();
        out
    }

    /// Goes through a task's tool actions for a robot whose tool is in `tool`: each must be possible when
    /// its turn comes (its precondition, with the states as the actions before it left them). Gives the
    /// tool's state afterwards and the objects' new states, or why an action cannot be done. Without tool
    /// kits in the document, nothing is checked.
    fn tool_outcome(
        &self,
        task: &Task,
        robot: Id<Robot>,
        world: &World,
        mut tool: Option<String>,
    ) -> Result<ToolOutcome, String> {
        let doc = self.doc;
        let mut objects: Vec<(String, String)> = Vec::new();
        if doc.tool_kits().is_empty() {
            return Ok((tool, objects));
        }
        let kit = doc.robot_tool_kit(robot).and_then(|(kit, _)| doc.get(kit));
        for step in &task.steps {
            let gc_doc::Step::ToolAction { action, target } = step else { continue };
            let found =
                kit.and_then(|k| k.action(action)).ok_or_else(|| format!("its tool cannot `{action}`"))?;
            let target = target.and_then(|t| doc.get(t)).map(|o| o.name.clone());
            // The object's state: as an earlier action of this task left it, else as the world has it.
            let state = target.as_ref().and_then(|name| {
                objects
                    .iter()
                    .rev()
                    .find(|(n, _)| n == name)
                    .map(|(_, s)| s.clone())
                    .or_else(|| world.objects.get(name).cloned())
            });
            let scope = gc_doc::ActionScope { outer: world, tool: tool.as_deref(), target: state.as_deref() };
            let possible = found.precondition.trim().is_empty()
                || Condition::parse(&found.precondition).is_ok_and(|c| c.eval(&scope) == Ok(true));
            if !possible {
                return Err(format!("`{action}` cannot be done: its precondition does not hold"));
            }
            if let Some(next) = &found.tool_state {
                tool = Some(next.clone());
            }
            if let (Some(name), Some(next)) = (target, &found.target_state) {
                objects.push((name, next.clone()));
            }
        }
        Ok((tool, objects))
    }

    /// [`Session::task_robots`] for the states of a schedule in progress.
    fn task_robots_in(&self, task: Id<Task>, world: &World) -> Vec<Id<Robot>> {
        match self.doc.get(task).map(|t| (&t.location, &t.robots)) {
            Some((gc_doc::Location::Robot(_), _)) | None => self.task_robots(task),
            Some((_, gc_doc::RobotChoice::When(condition))) if !world.holds(condition) => Vec::new(),
            Some((_, gc_doc::RobotChoice::When(_))) => {
                let doc = self.doc;
                doc.robots()
                    .iter()
                    .map(|(id, _)| id)
                    .filter(|r| !doc.is_component_suppressed(r.erase()))
                    .collect()
            }
            Some(_) => self.task_robots(task),
        }
    }
}
