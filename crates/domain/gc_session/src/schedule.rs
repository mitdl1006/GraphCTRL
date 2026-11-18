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
    /// Tasks that had to wait for a moving robot to be out of the way before they could start.
    pub waits: Vec<Wait>,
    /// Tasks whose robot went round a moving robot instead of waiting for it.
    pub detours: Vec<Detour>,
    /// The signals the cell drives as they are set: when, which and to what, in the order the tasks that
    /// set them were started.
    pub signals: Vec<(f64, String, bool)>,
    /// Moves that take an idle robot to its park pose because it stood in the way of a task that was
    /// ready (`gc_doc::WorkOp::SetRobotPark`; without a park pose, where the document has the robot).
    pub parks: Vec<ParkMove>,
    /// True when the schedule was cut off at [`MAX_ASSIGNMENTS`] (tasks that repeat without end).
    pub cut_short: bool,
}

/// A task done by a way round a robot that was moving, because that ended sooner than waiting.
#[derive(Clone, Debug, PartialEq)]
pub struct Detour {
    /// The task.
    pub task: Id<Task>,
    /// The robot that did it.
    pub robot: Id<Robot>,
    /// The moving robot it went round.
    pub around: Id<Robot>,
}

/// A robot moving out of the way.
#[derive(Clone, Debug)]
pub struct ParkMove {
    /// The robot.
    pub robot: Id<Robot>,
    /// When it starts, seconds.
    pub start: f64,
    /// When it is parked.
    pub end: f64,
    /// The motion.
    pub plan: crate::TaskPlan,
}

/// A task that was ready but had to wait for another robot's motion.
#[derive(Clone, Debug, PartialEq)]
pub struct Wait {
    /// The task.
    pub task: Id<Task>,
    /// The robot that did it in the end.
    pub robot: Id<Robot>,
    /// How long it waited, seconds.
    pub seconds: f64,
    /// The robot that was in the way.
    pub cause: Id<Robot>,
}

/// The most assignments a schedule holds: where tasks that repeat without end are cut off.
pub const MAX_ASSIGNMENTS: usize = 1000;

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
        // Its tasks and its moves out of the way: the one that started last by then.
        let tasks = self
            .assignments
            .iter()
            .zip(&self.plans)
            .filter(|(a, _)| a.robot == robot)
            .map(|(a, p)| (a.start, p));
        let parks = self.parks.iter().filter(|p| p.robot == robot).map(|p| (p.start, &p.plan));
        let (start, plan) =
            tasks.chain(parks).filter(|(start, _)| *start <= time).max_by(|a, b| a.0.total_cmp(&b.0))?;
        Some(plan.sample(time - start))
    }

    /// Every robot that has tasks or moves out of the way, with its joint values at `time` ([`Schedule::joints_at`]; before its
    /// first task, where that task starts from).
    pub fn robots_at(&self, time: f64) -> Vec<(Id<Robot>, Vec<f64>)> {
        let mut robots: Vec<Id<Robot>> =
            self.assignments.iter().map(|a| a.robot).chain(self.parks.iter().map(|p| p.robot)).collect();
        robots.sort();
        robots.dedup();
        robots
            .into_iter()
            .filter_map(|robot| {
                let first = || {
                    let tasks = self.assignments.iter().zip(&self.plans).filter(|(a, _)| a.robot == robot);
                    let parks = self.parks.iter().filter(|p| p.robot == robot).map(|p| (p.start, &p.plan));
                    let (_, plan) = tasks
                        .map(|(a, plan)| (a.start, plan))
                        .chain(parks)
                        .min_by(|a, b| a.0.total_cmp(&b.0))?;
                    Some(plan.start.clone())
                };
                Some((robot, self.joints_at(robot, time).or_else(first)?))
            })
            .collect()
    }

    /// When the last task ends.
    pub fn makespan(&self) -> f64 {
        self.assignments.iter().map(|a| a.end).chain(self.parks.iter().map(|p| p.end)).fold(0.0, f64::max)
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
type ToolOutcome = (Option<String>, Vec<(usize, String, String)>);

/// A task laid out in time for a robot: the plan, the state its tool ends in, and what it changes when.
type Fitted = (crate::TaskPlan, Option<String>, Vec<(f64, Change)>, Option<Id<Robot>>);

/// Something that changes at a moment of the schedule.
#[derive(Clone, Debug, PartialEq)]
enum Change {
    /// A signal the cell drives is set.
    Signal(String, bool),
    /// An object gets a state.
    Object(String, String),
}

/// When, from `begin` on, a driven signal has `value`: now, or when a change already on its way sets
/// it. `None` when nothing on its way does.
fn signal_at(world: &World, coming: &[(f64, Change)], name: &str, value: bool, begin: f64) -> Option<f64> {
    let mut changes: Vec<(f64, bool)> = coming
        .iter()
        .filter_map(|(at, change)| match change {
            Change::Signal(n, v) if n == name => Some((*at, *v)),
            _ => None,
        })
        .collect();
    changes.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut current = world.signals.get(name).copied().unwrap_or(false);
    for (_, v) in changes.iter().filter(|(at, _)| *at <= begin) {
        current = *v;
    }
    if current == value {
        return Some(begin);
    }
    changes.iter().find(|(at, v)| *at > begin && *v == value).map(|(at, _)| *at)
}

/// The states as the schedule goes along.
struct World {
    variables: BTreeMap<String, CondValue>,
    objects: BTreeMap<String, String>,
    /// The signals the cell drives (set by steps and tool actions); all start off.
    signals: BTreeMap<String, bool>,
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
            signals: doc.driven_signals().into_iter().map(|name| (name, false)).collect(),
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
                        // A number worked out from the numeric states as they are now (`count + 1`).
                        StateValue::Int(_) | StateValue::Real(_) => {
                            let numbers = |name: &str| match world.variables.get(name) {
                                Some(CondValue::Number(value)) => Some(*value),
                                _ => None,
                            };
                            let whole = matches!(var.value, StateValue::Int(_));
                            gc_doc::effect_number(text, &numbers)
                                .ok()
                                .map(|value| CondValue::Number(if whole { value.round() } else { value }))
                        }
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
        // What is on its way: changes at the ends of steps of tasks that have started.
        let mut timed: Vec<(f64, Change)> = Vec::new();
        // How often each task has been done, and since when (and for whom) a ready task has been waiting.
        let mut done: BTreeMap<Id<Task>, u32> = BTreeMap::new();
        let mut waiting: BTreeMap<Id<Task>, (f64, Id<Robot>)> = BTreeMap::new();
        // Where robots have already been sent away from: each place once.
        let mut moved_away: std::collections::BTreeSet<(Id<Robot>, Vec<u64>)> =
            std::collections::BTreeSet::new();
        // The idle robots found standing in the way of a ready task since the last look.
        let mut blockers: std::collections::BTreeSet<Id<Robot>> = std::collections::BTreeSet::new();
        let mut running: Vec<Assignment> = Vec::new();
        let mut out = Schedule::default();
        let mut reasons: BTreeMap<Id<Task>, String> = BTreeMap::new();
        let mut now = 0.0_f64;
        loop {
            if self.control.cancelled() || out.cut_short {
                break;
            }
            blockers.clear();
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
                let mut best: Option<Fitted> = None;
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
                    let standing: Vec<gc_motion::Obstacle> = parked
                        .iter()
                        .filter_map(|(other, at)| {
                            let name = doc.get(*other).map(|r| r.name.clone())?;
                            Some(scenes.get(other)?.as_obstacles(at, &name))
                        })
                        .flatten()
                        .collect();
                    let bid: Bid = bids
                        .entry(key)
                        .or_insert_with(|| self.task_plan_among(id, robot, joints, &standing))
                        .clone();
                    let plan = match bid {
                        Ok(plan) => plan,
                        Err(why) => {
                            // A robot standing in the way of this task is one to send away if nothing
                            // else helps.
                            blockers.extend(parked.iter().map(|(other, _)| *other).filter(|other| {
                                // The reason names what is in the way ("collides with `Left`", "is 2 mm
                                // from `Left`").
                                doc.get(*other).is_some_and(|r| why.contains(&format!("`{}`", r.name)))
                            }));
                            reasons.insert(id, why);
                            continue;
                        }
                    };
                    // Its tool must be in a state to do the task's tool actions.
                    let tool = tools.get(&robot).cloned().flatten();
                    let (tool, objects) = match self.tool_outcome(task, robot, &world, tool) {
                        Ok(outcome) => outcome,
                        Err(why) => {
                            reasons.insert(id, why);
                            continue;
                        }
                    };
                    // Laid out in time: the signals it waits for must be set or on their way.
                    let (mut plan, mut events) =
                        match self.timeline(task, plan, now, &world, &timed, &objects) {
                            Ok(fitted) => fitted,
                            Err(why) => {
                                reasons.insert(id, why);
                                continue;
                            }
                        };
                    // The robots that are moving must be clear of this motion all the way.
                    let moving: Vec<(Id<Robot>, f64)> = robots
                        .iter()
                        .filter(|(other, (_, free))| **other != robot && *free > now)
                        .map(|(other, (_, free))| (*other, *free))
                        .collect();
                    let crossing = |plan: &crate::TaskPlan| {
                        moving.iter().copied().find(|(other, _)| {
                            let at = |time: f64| out.joints_at(*other, time);
                            crosses(scenes.get(&robot), plan, now, scenes.get(other), &at)
                        })
                    };
                    // The moving robot this motion goes round, when it does.
                    let mut around = None;
                    if let Some((other, free)) = crossing(&plan) {
                        // Waiting for it would end about when it is done plus this motion. A way round it,
                        // planned with it standing where it is at moments of this motion and at its end, is taken when
                        // that is clear of every moving robot and ends sooner.
                        let mut swept = standing.clone();
                        let name = doc.get(other).map(|r| r.name.clone()).unwrap_or_default();
                        // Where it is while this motion would run (nine moments) and where it ends up.
                        let span = plan.duration().min(free - now);
                        let moments = (0..=8).map(|k| now + span * f64::from(k) / 8.0).chain([free]);
                        for at in moments {
                            if let (Some(q), Some(scene)) = (out.joints_at(other, at), scenes.get(&other)) {
                                swept.extend(scene.as_obstacles(&q, &name));
                            }
                        }
                        let round = self
                            .task_plan_among(id, robot, joints, &swept)
                            .ok()
                            .and_then(|round| self.timeline(task, round, now, &world, &timed, &objects).ok())
                            .filter(|(round, _)| {
                                crossing(round).is_none() && now + round.duration() < free + plan.duration()
                            });
                        match round {
                            Some((round, round_events)) => {
                                (plan, events) = (round, round_events);
                                around = Some(other);
                            }
                            None => {
                                reasons.insert(id, format!("it waits for {name} to be out of the way"));
                                waiting.entry(id).or_insert((now, other));
                                continue;
                            }
                        }
                    }
                    if best.as_ref().is_none_or(|(b, ..)| plan.duration() < b.duration()) {
                        best = Some((plan, tool, events, around));
                    }
                }
                match best {
                    Some((plan, tool, events, around)) => {
                        let robot = plan.robot;
                        out.detours.extend(around.map(|around| Detour { task: id, robot, around }));
                        let assignment =
                            Assignment { task: id, robot, start: now, end: now + plan.duration() };
                        robots.insert(robot, (plan.end.clone(), assignment.end));
                        out.plans.push(plan);
                        tools.insert(robot, tool);
                        out.signals.extend(events.iter().filter_map(|(at, change)| match change {
                            Change::Signal(name, value) => Some((*at, name.clone(), *value)),
                            Change::Object(..) => None,
                        }));
                        timed.extend(events);
                        if let Some((since, cause)) = waiting.remove(&id) {
                            out.waits.push(Wait { task: id, robot, seconds: now - since, cause });
                        }
                        running.push(assignment.clone());
                        out.assignments.push(assignment);
                        reasons.remove(&id);
                        pending.remove(index);
                        if out.assignments.len() >= MAX_ASSIGNMENTS {
                            out.cut_short = true;
                        }
                    }
                    None => index += 1,
                }
            }
            // On to the moment the next task ends (its effects may make others ready) or a robot is parked.
            let ends = running
                .iter()
                .map(|a| a.end)
                .chain(out.parks.iter().map(|p| p.end).filter(|e| *e > now))
                .chain(timed.iter().map(|(at, _)| *at).filter(|at| *at >= now));
            let Some(next) = ends.min_by(f64::total_cmp) else {
                // Nothing moves and nothing more can start. A ready task may be held up by a robot that
                // stands in the way with nothing to do: one such robot goes to its park pose, and the
                // tasks are tried again while it moves and when it is there.
                let ready = pending.iter().any(|id| {
                    !done.contains_key(id) && doc.get(*id).is_some_and(|t| world.holds(&t.precondition))
                });
                let parked = ready
                    .then(|| {
                        robots.iter().filter(|(robot, _)| blockers.contains(robot)).find_map(
                            |(robot, (at, _))| {
                                let park = doc.robot_park(*robot)?;
                                if bits(at) == bits(&park) || !moved_away.insert((*robot, bits(at))) {
                                    return None;
                                }
                                let others: Vec<gc_motion::Obstacle> = robots
                                    .iter()
                                    .filter(|(other, _)| *other != robot)
                                    .filter_map(|(other, (stands, _))| {
                                        let name = doc.get(*other).map(|r| r.name.clone())?;
                                        Some(scenes.get(other)?.as_obstacles(stands, &name))
                                    })
                                    .flatten()
                                    .collect();
                                self.park_plan(*robot, at, &park, &others).ok()
                            },
                        )
                    })
                    .flatten();
                let Some(plan) = parked else { break };
                let end = now + plan.duration();
                robots.insert(plan.robot, (plan.end.clone(), end));
                out.parks.push(ParkMove { robot: plan.robot, start: now, end, plan });
                continue;
            };
            now = next;
            // What changes by now: signals set and objects left in a new state at the end of a step.
            timed.sort_by(|a, b| a.0.total_cmp(&b.0));
            let due = timed.partition_point(|(at, _)| *at <= now);
            for (_, change) in timed.drain(..due) {
                match change {
                    Change::Signal(name, value) => drop(world.signals.insert(name, value)),
                    Change::Object(name, state) => drop(world.objects.insert(name, state)),
                }
            }
            let (ended, still): (Vec<Assignment>, Vec<Assignment>) =
                running.into_iter().partition(|a| a.end <= now);
            running = still;
            for assignment in ended {
                // The task's own effects (what its steps set and its tool actions did came at their times).
                if let Some(task) = doc.get(assignment.task) {
                    self.apply_effects(&mut world, task);
                }
                // A task that repeats goes back among those to do, in its place by priority.
                let times = done.entry(assignment.task).or_insert(0);
                *times += 1;
                let again = match doc.task_repeat(assignment.task) {
                    None => false,
                    Some(0) => true,
                    Some(most) => *times < most,
                };
                if again {
                    let rank =
                        |id: &Id<Task>| (std::cmp::Reverse(doc.get(*id).map_or(0, |t| t.priority)), *id);
                    let at = pending.partition_point(|other| rank(other) < rank(&assignment.task));
                    pending.insert(at, assignment.task);
                }
            }
        }
        // What was never done is blocked (a repeating task that was done and is not ready again is not).
        out.blocked = pending
            .into_iter()
            .filter(|id| !done.contains_key(id) && !running.iter().any(|a| a.task == *id))
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
        let mut objects: Vec<(usize, String, String)> = Vec::new();
        if doc.tool_kits().is_empty() {
            return Ok((tool, objects));
        }
        let kit = doc.robot_tool_kit(robot).and_then(|(kit, _)| doc.get(kit));
        for (number, step) in task.steps.iter().enumerate() {
            let gc_doc::Step::ToolAction { action, target } = step else { continue };
            let found =
                kit.and_then(|k| k.action(action)).ok_or_else(|| format!("its tool cannot `{action}`"))?;
            let target = target.and_then(|t| doc.get(t)).map(|o| o.name.clone());
            // The object's state: as an earlier action of this task left it, else as the world has it.
            let state = target.as_ref().and_then(|name| {
                objects
                    .iter()
                    .rev()
                    .find(|(_, n, _)| n == name)
                    .map(|(_, _, s)| s.clone())
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
                objects.push((number, name, next.clone()));
            }
        }
        Ok((tool, objects))
    }

    /// Lays a planned task out in time from `start`: where a step waits for a signal the cell drives, the
    /// robot stands still until the signal has the value (it has it already, or a change on its way sets
    /// it; a wait for any other signal is taken to be answered). Gives the plan with those waits in it and
    /// what the task changes when: the signals its steps and tool actions set and the states its tool
    /// actions leave their objects in, each at the end of its step. Fails when a signal it waits for is
    /// not on its way.
    fn timeline(
        &self,
        task: &Task,
        mut plan: crate::TaskPlan,
        start: f64,
        world: &World,
        coming: &[(f64, Change)],
        objects: &[(usize, String, String)],
    ) -> Result<(crate::TaskPlan, Vec<(f64, Change)>), String> {
        let doc = self.doc;
        let kit = doc.robot_tool_kit(plan.robot).and_then(|(kit, _)| doc.get(kit));
        let mut own: Vec<(f64, Change)> = Vec::new();
        for (number, step) in task.steps.iter().enumerate() {
            if let gc_doc::Step::WaitIo { condition, .. } = step
                && let Some((name, value)) = gc_doc::signal_condition(condition)
                && world.signals.contains_key(&name)
            {
                let begin =
                    start + number.checked_sub(1).and_then(|b| plan.step_ends.get(b)).copied().unwrap_or(0.0);
                let all: Vec<(f64, Change)> = coming.iter().chain(&own).cloned().collect();
                let at = signal_at(world, &all, &name, value, begin)
                    .ok_or_else(|| format!("it waits for the signal `{name}`"))?;
                if at > begin {
                    plan.hold_before(number, at - begin);
                }
            }
            let end = start + plan.step_ends.get(number).copied().unwrap_or(0.0);
            match step {
                gc_doc::Step::SetIo { name, value } => own.push((end, Change::Signal(name.clone(), *value))),
                gc_doc::Step::ToolAction { action, .. } => {
                    own.extend(
                        kit.and_then(|k| k.action(action))
                            .and_then(|a| a.set.clone())
                            .map(|(name, value)| (end, Change::Signal(name, value))),
                    );
                    own.extend(
                        objects
                            .iter()
                            .filter(|(at, ..)| *at == number)
                            .map(|(_, name, state)| (end, Change::Object(name.clone(), state.clone()))),
                    );
                }
                _ => {}
            }
        }
        Ok((plan, own))
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
