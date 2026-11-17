//! The robot cell in a session: loaded robot models, jogging, waypoints, planning and playback.
//!
//! Planned motions are derived data. They are kept beside the document, never in it, together with a
//! fingerprint of everything they were computed from; when the fingerprint no longer matches, the plan is
//! shown as out of date.

use std::collections::BTreeMap;
use std::sync::Arc;

use gc_collide::ShapeDesc;
use gc_core::Id;
use gc_doc::{
    Body, CellOp, Command, Created, Document, EditError, MotionPath, Placement, Robot, Sketch, Solid,
    Waypoint, WaypointTarget,
};
use gc_math::{DPose3, DQuat, DVec3, pose_from_xyz_rpy};
use gc_motion::{Goal, MotionPlan, MotionRequest, Obstacle, RobotScene, plan_motion};
use gc_plan::{Algorithm, NeverCancel, Options};
use gc_robot::{Chain, RobotModel};
use gc_sketch::{SkPoint, Space};

use crate::Session;

/// The planners a path can use: id stored in the document, and the algorithm behind it.
pub const PLANNERS: [(&str, Algorithm); 5] = [
    ("rrt_connect", Algorithm::RrtConnect),
    ("rrt", Algorithm::Rrt),
    ("rrt_star", Algorithm::RrtStar),
    ("informed_rrt_star", Algorithm::InformedRrtStar),
    ("prm", Algorithm::Prm),
];

/// The distance robot links keep from obstacles, metres.
pub const PADDING: f64 = 0.005;

/// A robot description that was read successfully.
#[derive(Debug)]
pub struct LoadedRobot {
    urdf: String,
    /// The model.
    pub model: Arc<RobotModel>,
    /// The chain that is jogged and planned for.
    pub chain: Chain,
}

/// A planned motion through all waypoints of a path.
#[derive(Clone, Debug)]
pub struct PlannedPath {
    /// One motion per waypoint, in order.
    pub segments: Vec<MotionPlan>,
}

impl PlannedPath {
    /// Total duration in seconds.
    pub fn duration(&self) -> f64 {
        self.segments.iter().map(|s| s.trajectory.duration()).sum()
    }

    /// The joint values at time `t` (clamped to the motion).
    pub fn sample(&self, t: f64) -> Vec<f64> {
        let mut t = t.clamp(0.0, self.duration());
        for segment in &self.segments {
            let duration = segment.trajectory.duration();
            if t <= duration {
                return segment.trajectory.sample(t).q;
            }
            t -= duration;
        }
        self.segments.last().map(|s| s.trajectory.sample(s.trajectory.duration()).q).unwrap_or_default()
    }

    /// World positions of the tool tip along the whole motion.
    pub fn tip_path(&self) -> impl Iterator<Item = [f64; 3]> + '_ {
        self.segments.iter().flat_map(|s| s.tip_path.iter().copied())
    }

    /// The smallest clearance along the motion.
    pub fn min_clearance(&self) -> f64 {
        self.segments.iter().map(|s| s.min_clearance).fold(f64::INFINITY, f64::min)
    }

    /// State checks the planners spent.
    pub fn state_checks(&self) -> u64 {
        self.segments.iter().map(|s| s.stats.state_checks).sum()
    }
}

/// How a path's plan relates to the document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanStatus {
    /// Never planned.
    NotPlanned,
    /// Planned, and nothing it depends on changed since.
    Ready,
    /// Planned, but something it depends on changed.
    Stale,
    /// The last attempt failed.
    Failed,
}

struct Planned {
    inputs: String,
    outcome: Result<PlannedPath, String>,
}

/// Robot-cell state of a session that is not part of the document.
#[derive(Default)]
pub(crate) struct CellState {
    robots: BTreeMap<Id<Robot>, Arc<LoadedRobot>>,
    jog: Option<(Id<Robot>, Vec<f64>)>,
    plans: BTreeMap<Id<MotionPath>, Planned>,
    playback: Option<(Id<MotionPath>, f64)>,
}

impl CellState {
    /// Brings the caches in line with the document.
    pub(crate) fn sync(&mut self, doc: &Document) {
        self.robots.retain(|id, loaded| doc.get(*id).is_some_and(|r| r.urdf == loaded.urdf));
        for (id, robot) in doc.robots().iter() {
            if !self.robots.contains_key(&id)
                && let Ok(loaded) = load(&robot.urdf, Some((&robot.base_link, &robot.tip_link)))
            {
                self.robots.insert(id, Arc::new(loaded.0));
            }
        }
        self.plans.retain(|id, _| doc.get(*id).is_some());
        if self.jog.as_ref().is_some_and(|(id, _)| doc.get(*id).is_none()) {
            self.jog = None;
        }
        if self.playback.is_some_and(|(id, _)| doc.get(id).is_none()) {
            self.playback = None;
        }
    }
}

/// Reads a robot description and picks its planning chain. Returns the tip link's name with it.
fn load(urdf: &str, chain: Option<(&str, &str)>) -> Result<(LoadedRobot, String, String), String> {
    let (model, _) = gc_robot::import(urdf).map_err(|findings| {
        findings.iter().map(|f| format!("{}:{}: {}", f.line, f.col, f.message)).collect::<Vec<_>>().join("\n")
    })?;
    let root = model.links[model.root].name.clone();
    let (chain, base, tip) = match chain {
        Some((base, tip)) => {
            (model.chain(base, tip).map_err(|e| e.to_string())?, base.to_owned(), tip.to_owned())
        }
        None => {
            let chain = model.default_chain().map_err(|e| e.to_string())?;
            let tip = model
                .links
                .iter()
                .map(|link| link.name.as_str())
                .find(|name| {
                    model.chain(&root, name).is_ok_and(|c| c == chain)
                        && (*name == "tool0" || model.link("tool0").is_none())
                })
                .unwrap_or("tool0")
                .to_owned();
            (chain, root, tip)
        }
    };
    Ok((LoadedRobot { urdf: urdf.to_owned(), model: Arc::new(model), chain }, base, tip))
}

fn pose_of(placement: &Placement) -> DPose3 {
    pose_from_xyz_rpy(placement.xyz, placement.rpy)
}

/// The world pose the tool takes at a sketch point: tip on the point, tool axis against the plane normal.
fn pose_at_point(doc: &Document, sketch: Id<Sketch>, point: Id<SkPoint>) -> Option<DPose3> {
    let sketch = doc.get(sketch)?;
    let (Space::Planar(frame) | Space::Spatial(frame)) = sketch.space;
    let frame = doc.frame_world_pose(Id::from_raw(frame.raw))?;
    let pos = sketch.points.get(&point)?.pos;
    let world = frame.translation + frame.rotation * DVec3::from_array(pos);
    Some(DPose3::from_parts(world, frame.rotation * DQuat::from_rotation_x(std::f64::consts::PI)))
}

impl Session {
    fn cell_op(&mut self, op: CellOp) -> Result<Vec<Created>, EditError> {
        self.execute(&Command::Cell(op))
    }

    /// The loaded model of a robot; `None` when its description could not be read.
    pub fn robot_model(&self, robot: Id<Robot>) -> Option<&Arc<LoadedRobot>> {
        self.cell.robots.get(&robot)
    }

    /// Adds a robot from a description (URDF text) at the world origin. The error lists what is wrong
    /// with the description.
    pub fn add_robot(&mut self, name: &str, urdf: &str) -> Result<Id<Robot>, String> {
        let (loaded, base_link, tip_link) = load(urdf, None)?;
        let robot = Robot {
            name: name.to_owned(),
            urdf: urdf.to_owned(),
            base_link,
            tip_link,
            placement: Placement::default(),
            joints: vec![0.0; loaded.chain.dof()],
            max_acc: 5.0,
        };
        match self.cell_op(CellOp::AddRobot(robot)).map_err(|e| e.to_string())?.first() {
            Some(Created::Robot(id)) => Ok(*id),
            _ => Err("the robot was not created".to_owned()),
        }
    }

    /// Adds a body with a name that is not taken yet.
    pub fn add_body(&mut self, solid: Solid, placement: Placement) -> Result<Id<Body>, EditError> {
        let name = format!("Body{}", self.doc.bodies().len() + 1);
        match self.cell_op(CellOp::AddBody(Body { name, placement, solid }))?.first() {
            Some(Created::Body(id)) => Ok(*id),
            _ => Err(EditError::BadValue),
        }
    }

    /// The joint values a robot is shown with: the playback position, else the jog preview, else the
    /// document's.
    pub fn robot_joints(&self, robot: Id<Robot>) -> Option<Vec<f64>> {
        if let Some((path, time)) = self.cell.playback
            && self.doc.get(path).is_some_and(|p| p.robot == robot)
            && let Some(planned) = self.planned(path)
        {
            return Some(planned.sample(time));
        }
        match &self.cell.jog {
            Some((id, q)) if *id == robot => Some(q.clone()),
            _ => self.doc.get(robot).map(|r| r.joints.clone()),
        }
    }

    /// World poses of all links of a robot as it is shown.
    pub fn link_poses(&self, robot: Id<Robot>) -> Option<Vec<DPose3>> {
        let loaded = self.robot_model(robot)?;
        let base = pose_of(&self.doc.get(robot)?.placement);
        let mut poses = Vec::new();
        loaded.model.link_poses(&loaded.chain.expand(&self.robot_joints(robot)?), &mut poses);
        Some(poses.iter().map(|pose| base * *pose).collect())
    }

    /// Previews joint values without touching the document. Values are clamped to the joint limits.
    pub fn jog(&mut self, robot: Id<Robot>, joints: &[f64]) {
        let Some(loaded) = self.robot_model(robot) else { return };
        if joints.len() != loaded.chain.dof() {
            return;
        }
        let limits = loaded.chain.lower().iter().zip(loaded.chain.upper());
        let clamped = joints.iter().zip(limits).map(|(q, (lo, hi))| q.clamp(*lo, *hi)).collect();
        self.cell.playback = None;
        self.cell.jog = Some((robot, clamped));
    }

    /// Commits the jog preview as one undo step.
    pub fn end_jog(&mut self) {
        if let Some((id, joints)) = self.cell.jog.take()
            && self.doc.get(id).is_some_and(|r| r.joints != joints)
        {
            self.last_error = self.cell_op(CellOp::SetJoints { id, joints }).err();
        }
    }

    /// Adds a waypoint at the robot's current joint values.
    pub fn add_waypoint_here(&mut self, robot: Id<Robot>) -> Result<Id<Waypoint>, EditError> {
        let joints = self.robot_joints(robot).ok_or(EditError::BadValue)?;
        self.add_waypoint(robot, WaypointTarget::Joints(joints))
    }

    /// Adds a waypoint.
    pub fn add_waypoint(
        &mut self,
        robot: Id<Robot>,
        target: WaypointTarget,
    ) -> Result<Id<Waypoint>, EditError> {
        let name = format!("P{}", self.doc.waypoints().len() + 1);
        match self.cell_op(CellOp::AddWaypoint(Waypoint { name, robot, target }))?.first() {
            Some(Created::Waypoint(id)) => Ok(*id),
            _ => Err(EditError::BadValue),
        }
    }

    /// Adds a path of `robot` through `waypoints`, planned with the default planner.
    pub fn add_path(
        &mut self,
        robot: Id<Robot>,
        waypoints: Vec<Id<Waypoint>>,
    ) -> Result<Id<MotionPath>, EditError> {
        let name = format!("Path{}", self.doc.paths().len() + 1);
        let path = MotionPath { name, robot, waypoints, planner: PLANNERS[0].0.to_owned(), seed: 1 };
        match self.cell_op(CellOp::AddPath(path))?.first() {
            Some(Created::Path(id)) => Ok(*id),
            _ => Err(EditError::BadValue),
        }
    }

    /// Everything a plan of this path depends on, as text.
    fn plan_inputs(&self, path: Id<MotionPath>) -> Option<String> {
        let doc = &self.doc;
        let path = doc.get(path)?;
        let robot = doc.get(path.robot)?;
        let targets: Vec<String> = path
            .waypoints
            .iter()
            .map(|w| match doc.get(*w).map(|w| &w.target) {
                Some(WaypointTarget::Joints(q)) => format!("{q:?}"),
                Some(WaypointTarget::SketchPoint { sketch, point }) => {
                    format!("{:?}", pose_at_point(doc, *sketch, *point))
                }
                None => String::new(),
            })
            .collect();
        let bodies: Vec<(&Placement, &Solid)> =
            doc.bodies().iter().map(|(_, b)| (&b.placement, &b.solid)).collect();
        Some(format!(
            "{} {} {} {:?} {:?} {} {bodies:?} {targets:?} {} {}",
            robot.urdf.len(),
            robot.base_link,
            robot.tip_link,
            robot.placement,
            robot.joints,
            robot.max_acc,
            path.planner,
            path.seed
        ))
    }

    /// How the stored plan of a path relates to the document.
    pub fn plan_status(&self, path: Id<MotionPath>) -> PlanStatus {
        match self.cell.plans.get(&path) {
            None => PlanStatus::NotPlanned,
            Some(planned) if self.plan_inputs(path).as_ref() != Some(&planned.inputs) => PlanStatus::Stale,
            Some(Planned { outcome: Ok(_), .. }) => PlanStatus::Ready,
            Some(Planned { outcome: Err(_), .. }) => PlanStatus::Failed,
        }
    }

    /// The last successful plan of a path (possibly out of date; see [`Session::plan_status`]).
    pub fn planned(&self, path: Id<MotionPath>) -> Option<&PlannedPath> {
        self.cell.plans.get(&path).and_then(|p| p.outcome.as_ref().ok())
    }

    /// Why the last planning attempt of a path failed.
    pub fn plan_error(&self, path: Id<MotionPath>) -> Option<&str> {
        self.cell.plans.get(&path).and_then(|p| p.outcome.as_ref().err()).map(String::as_str)
    }

    /// Plans a path from the robot's current joint values through its waypoints and stores the result.
    pub fn plan(&mut self, path: Id<MotionPath>) -> PlanStatus {
        self.end_jog();
        self.cell.playback = None;
        let Some(inputs) = self.plan_inputs(path) else { return PlanStatus::NotPlanned };
        let outcome = self.plan_now(path);
        self.cell.plans.insert(path, Planned { inputs, outcome });
        self.plan_status(path)
    }

    fn plan_now(&self, path_id: Id<MotionPath>) -> Result<PlannedPath, String> {
        let doc = &self.doc;
        let path = doc.get(path_id).ok_or("the path no longer exists")?;
        let robot = doc.get(path.robot).ok_or("the robot no longer exists")?;
        let loaded = self.robot_model(path.robot).ok_or("the robot description could not be read")?;
        let algorithm = PLANNERS
            .iter()
            .find(|(id, _)| *id == path.planner)
            .map(|(_, algorithm)| *algorithm)
            .ok_or_else(|| format!("unknown planner `{}`", path.planner))?;
        let obstacles: Vec<Obstacle> = doc
            .bodies()
            .iter()
            .map(|(_, body)| Obstacle {
                name: body.name.clone(),
                pose: pose_of(&body.placement),
                shape: match body.solid {
                    Solid::Box { size } => ShapeDesc::Box(size),
                    Solid::Cylinder { radius, length } => ShapeDesc::Cylinder { radius, length },
                    Solid::Sphere { radius } => ShapeDesc::Ball(radius),
                },
            })
            .collect();
        let (scene, _) = RobotScene::new(
            Arc::clone(&loaded.model),
            loaded.chain.clone(),
            &robot.tip_link,
            pose_of(&robot.placement),
            &obstacles,
            PADDING,
            &|_| None,
        )
        .map_err(|e| e.to_string())?;
        let mut start = robot.joints.clone();
        let mut segments = Vec::with_capacity(path.waypoints.len());
        for waypoint_id in &path.waypoints {
            let waypoint = doc.get(*waypoint_id).ok_or("a waypoint no longer exists")?;
            let goal = match &waypoint.target {
                WaypointTarget::Joints(q) => Goal::Joints(q.clone()),
                WaypointTarget::SketchPoint { sketch, point } => Goal::Pose(
                    pose_at_point(doc, *sketch, *point)
                        .ok_or_else(|| format!("{}: its sketch point no longer exists", waypoint.name))?,
                ),
            };
            let request = MotionRequest {
                start: start.clone(),
                goal,
                algorithm,
                options: Options::default(),
                seed: path.seed,
                runs: 4,
                max_acc: vec![robot.max_acc; start.len()],
            };
            let plan =
                plan_motion(&scene, &request, &NeverCancel).map_err(|e| format!("{}: {e}", waypoint.name))?;
            if let Some(end) = plan.path.last() {
                start.clone_from(end);
            }
            segments.push(plan);
        }
        Ok(PlannedPath { segments })
    }

    /// Shows the robot of `path` at time `t` of its planned motion; `None` shows it where it is.
    pub fn set_playback(&mut self, playback: Option<(Id<MotionPath>, f64)>) {
        self.cell.playback = playback.filter(|(path, _)| self.planned(*path).is_some());
    }

    /// The path being played back and the time shown.
    pub fn playback(&self) -> Option<(Id<MotionPath>, f64)> {
        self.cell.playback
    }
}
