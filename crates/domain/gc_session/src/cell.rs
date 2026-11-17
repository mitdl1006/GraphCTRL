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
use gc_jobs::{CancelToken, Executor, JobHandle, JobOutcome};
use gc_math::{DPose3, DQuat, DVec3, pose_from_xyz_rpy};
use gc_mesh::MeshData;
use gc_motion::{Goal, MotionPlan, MotionRequest, Obstacle, RobotScene, plan_motion};
use gc_plan::{Algorithm, Control, Options};
use gc_robot::{Chain, RobotModel, Shape};
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

/// Supplies the bytes of a file a robot description refers to (e.g. `package://.../forearm.stl`).
pub type Resources = Arc<dyn Fn(&str) -> Option<Vec<u8>> + Send + Sync>;

/// A mesh of a robot link, ready to draw.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshRef {
    /// Equal for equal meshes: the content hash.
    pub key: u64,
    /// The radius of a sphere around the mesh's origin that contains it.
    pub radius: f64,
    /// The triangles.
    pub data: Arc<MeshData>,
}

/// A robot description that was read successfully.
#[derive(Debug)]
pub struct LoadedRobot {
    urdf: String,
    /// The model.
    pub model: Arc<RobotModel>,
    /// The chain that is jogged and planned for.
    pub chain: Chain,
    /// Mesh files as stored, by the name the description uses.
    files: BTreeMap<String, Arc<MeshData>>,
    /// Meshes at the scale a geometry asks for, by name and scale.
    scaled: BTreeMap<String, MeshRef>,
    /// Mesh files the description refers to that could not be supplied or read.
    pub missing: Vec<String>,
}

/// A mesh file's triangles with its coordinates multiplied by `scale`.
fn scaled(file: &MeshData, scale: [f64; 3]) -> Option<MeshRef> {
    #[allow(clippy::cast_possible_truncation, reason = "mesh coordinates are f32")]
    let factors = scale.map(|f| f as f32);
    let positions: Vec<[f32; 3]> =
        file.positions().iter().map(|p| [p[0] * factors[0], p[1] * factors[1], p[2] * factors[2]]).collect();
    let data = MeshData::new(positions, file.indices().to_vec()).ok()?;
    let bounds = data.aabb();
    let radius = bounds.min.abs().max(bounds.max.abs()).length();
    Some(MeshRef { key: data.content_hash(), radius, data: Arc::new(data) })
}

fn scaled_key(uri: &str, scale: [f64; 3]) -> String {
    format!("{uri}|{scale:?}")
}

impl LoadedRobot {
    /// A mesh file as stored.
    pub fn file(&self, uri: &str) -> Option<Arc<MeshData>> {
        self.files.get(uri).cloned()
    }

    /// A mesh at the scale a geometry asks for.
    pub fn mesh(&self, uri: &str, scale: [f64; 3]) -> Option<&MeshRef> {
        self.scaled.get(&scaled_key(uri, scale))
    }

    fn read_meshes(&mut self, resources: Option<&Resources>) {
        let model = Arc::clone(&self.model);
        let geometries = model.links.iter().flat_map(|link| link.visuals.iter().chain(&link.collisions));
        for geometry in geometries {
            let Shape::Mesh { uri, scale } = &geometry.shape else { continue };
            if self.missing.contains(uri) || self.scaled.contains_key(&scaled_key(uri, *scale)) {
                continue;
            }
            let file = match self.files.get(uri) {
                Some(file) => Some(Arc::clone(file)),
                None => resources
                    .and_then(|resolve| resolve(uri))
                    .and_then(|bytes| gc_mesh::stl::parse(&bytes).ok())
                    .map(Arc::new),
            };
            let Some(file) = file else {
                self.missing.push(uri.clone());
                continue;
            };
            self.files.insert(uri.clone(), Arc::clone(&file));
            let Some(mesh) = scaled(&file, *scale) else {
                self.missing.push(uri.clone());
                continue;
            };
            self.scaled.insert(scaled_key(uri, *scale), mesh);
        }
    }
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
    /// Being planned right now.
    Planning,
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

/// Lets a running planner see that its job was cancelled.
struct JobControl(CancelToken);

impl Control for JobControl {
    fn cancelled(&self) -> bool {
        self.0.is_cancelled()
    }
}

/// A plan being computed: the path, the fingerprint of its inputs and the job. A cancelled job yields
/// `None`.
type PlanJob = (Id<MotionPath>, String, JobHandle<Option<Result<PlannedPath, String>>>);

/// Robot-cell state of a session that is not part of the document.
pub(crate) struct CellState {
    executor: Executor,
    job: Option<PlanJob>,
    robots: BTreeMap<Id<Robot>, Arc<LoadedRobot>>,
    jog: Option<(Id<Robot>, Vec<f64>)>,
    body: Option<(Id<Body>, Body)>,
    plans: BTreeMap<Id<MotionPath>, Planned>,
    playback: Option<(Id<MotionPath>, f64)>,
    resources: Option<Resources>,
    /// Mesh files of bodies by path: the triangles as stored, or why the file could not be read.
    files: BTreeMap<String, Result<Arc<MeshData>, String>>,
    /// The same at the scale a body asks for, by path and scale.
    meshes: BTreeMap<String, MeshRef>,
}

impl Default for CellState {
    fn default() -> Self {
        Self {
            // Jobs run to completion where they are started unless the application supplies threads.
            executor: Executor::inline(),
            job: None,
            robots: BTreeMap::new(),
            jog: None,
            body: None,
            plans: BTreeMap::new(),
            playback: None,
            resources: None,
            files: BTreeMap::new(),
            meshes: BTreeMap::new(),
        }
    }
}

impl CellState {
    /// Brings the caches in line with the document.
    pub(crate) fn sync(&mut self, doc: &Document) {
        self.robots.retain(|id, loaded| doc.get(*id).is_some_and(|r| r.urdf == loaded.urdf));
        for (id, robot) in doc.robots().iter() {
            if !self.robots.contains_key(&id)
                && let Ok(loaded) =
                    load(&robot.urdf, Some((&robot.base_link, &robot.tip_link)), self.resources.as_ref())
            {
                self.robots.insert(id, Arc::new(loaded.0));
            }
        }
        for (_, body) in doc.bodies().iter() {
            self.read_body_mesh(&body.solid);
        }
        self.plans.retain(|id, _| doc.get(*id).is_some());
        if self.jog.as_ref().is_some_and(|(id, _)| doc.get(*id).is_none()) {
            self.jog = None;
        }
        if self.body.as_ref().is_some_and(|(id, _)| doc.get(*id).is_none()) {
            self.body = None;
        }
        if self.playback.is_some_and(|(id, _)| doc.get(id).is_none()) {
            self.playback = None;
        }
    }
}

fn read_mesh_file(path: &str) -> Result<Arc<MeshData>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    gc_mesh::stl::parse(&bytes).map(Arc::new).map_err(|e| format!("{path}: {e}"))
}

impl CellState {
    /// Makes sure the mesh of a body's solid is loaded at its scale.
    fn read_body_mesh(&mut self, solid: &Solid) {
        let Solid::Mesh { file, scale } = solid else { return };
        let key = scaled_key(file, [*scale; 3]);
        if self.meshes.contains_key(&key) {
            return;
        }
        let stored = self.files.entry(file.clone()).or_insert_with(|| read_mesh_file(file));
        if let Ok(data) = stored
            && let Some(mesh) = scaled(data, [*scale; 3])
        {
            self.meshes.insert(key, mesh);
        }
    }

    fn body_mesh(&self, solid: &Solid) -> Option<&MeshRef> {
        match solid {
            Solid::Mesh { file, scale } => self.meshes.get(&scaled_key(file, [*scale; 3])),
            _ => None,
        }
    }
}

/// The collision shape of a body's solid.
fn shape_of(solid: &Solid, meshes: &BTreeMap<String, MeshRef>) -> Option<ShapeDesc> {
    Some(match solid {
        Solid::Box { size } => ShapeDesc::Box(*size),
        Solid::Cylinder { radius, length } => ShapeDesc::Cylinder { radius: *radius, length: *length },
        Solid::Sphere { radius } => ShapeDesc::Ball(*radius),
        Solid::Mesh { file, scale } => {
            ShapeDesc::Mesh(Arc::clone(&meshes.get(&scaled_key(file, [*scale; 3]))?.data))
        }
    })
}

/// Reads a robot description and picks its planning chain. Returns the tip link's name with it.
fn load(
    urdf: &str,
    chain: Option<(&str, &str)>,
    resources: Option<&Resources>,
) -> Result<(LoadedRobot, String, String), String> {
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
    let mut loaded = LoadedRobot {
        urdf: urdf.to_owned(),
        model: Arc::new(model),
        chain,
        files: BTreeMap::new(),
        scaled: BTreeMap::new(),
        missing: Vec::new(),
    };
    loaded.read_meshes(resources);
    Ok((loaded, base, tip))
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

    /// Sets where the files robot descriptions refer to come from, and reloads the robots.
    pub fn set_resources(&mut self, resources: Resources) {
        self.cell.resources = Some(resources);
        self.cell.robots.clear();
        self.cell.sync(&self.doc);
    }

    /// The loaded model of a robot; `None` when its description could not be read.
    pub fn robot_model(&self, robot: Id<Robot>) -> Option<&Arc<LoadedRobot>> {
        self.cell.robots.get(&robot)
    }

    /// Adds a robot from a description (URDF text) at the world origin. The error lists what is wrong
    /// with the description.
    pub fn add_robot(&mut self, name: &str, urdf: &str) -> Result<Id<Robot>, String> {
        let (loaded, base_link, tip_link) = load(urdf, None, self.cell.resources.as_ref())?;
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

    /// The mesh of a body whose solid is a mesh file; `None` for other solids and for files that could
    /// not be read (see [`Session::mesh_error`]).
    pub fn body_mesh(&self, solid: &Solid) -> Option<&MeshRef> {
        self.cell.body_mesh(solid)
    }

    /// Why a mesh file could not be read.
    pub fn mesh_error(&self, file: &str) -> Option<&str> {
        self.cell.files.get(file).and_then(|stored| stored.as_ref().err()).map(String::as_str)
    }

    /// Adds a body from a mesh file (STL). A file whose size only makes sense in millimetres is scaled
    /// to metres; the scale can be changed afterwards.
    pub fn add_mesh_body(&mut self, path: &std::path::Path) -> Result<Id<Body>, String> {
        let file = path.to_string_lossy().into_owned();
        let data = read_mesh_file(&file)?;
        // Nothing in a robot cell is twenty metres across.
        let scale = if data.aabb().diagonal() > 20.0 { 0.001 } else { 1.0 };
        self.cell.files.insert(file.clone(), Ok(data));
        let name =
            path.file_stem().map_or_else(|| "Mesh".to_owned(), |stem| stem.to_string_lossy().into_owned());
        let body = Body { name, placement: Placement::default(), solid: Solid::Mesh { file, scale } };
        match self.cell_op(CellOp::AddBody(body)).map_err(|e| e.to_string())?.first() {
            Some(Created::Body(id)) => Ok(*id),
            _ => Err("the body was not created".to_owned()),
        }
    }

    /// A body as it is shown: the preview while it is being edited, else the document's.
    pub fn shown_body(&self, body: Id<Body>) -> Option<&Body> {
        match &self.cell.body {
            Some((id, preview)) if *id == body => Some(preview),
            _ => self.doc.get(body),
        }
    }

    /// Previews an edit of a body without touching the document.
    pub fn preview_body(&mut self, id: Id<Body>, body: Body) {
        if self.doc.get(id).is_some() && body.solid.is_valid() {
            self.cell.read_body_mesh(&body.solid);
            self.cell.body = Some((id, body));
        }
    }

    /// Commits the body preview as one undo step.
    pub fn end_body_preview(&mut self) {
        if let Some((id, body)) = self.cell.body.take()
            && self.doc.get(id).is_some_and(|b| *b != body)
        {
            self.last_error = self.cell_op(CellOp::SetBody { id, body }).err();
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

    /// The world pose of the tool at a waypoint.
    pub fn waypoint_pose(&self, waypoint: Id<Waypoint>) -> Option<DPose3> {
        let waypoint = self.doc.get(waypoint)?;
        match &waypoint.target {
            WaypointTarget::SketchPoint { sketch, point } => pose_at_point(&self.doc, *sketch, *point),
            WaypointTarget::Joints(q) => {
                let robot = self.doc.get(waypoint.robot)?;
                let loaded = self.robot_model(waypoint.robot)?;
                let mut poses = Vec::new();
                loaded.model.link_poses(&loaded.chain.expand(q), &mut poses);
                let tip = poses.get(loaded.model.link(&robot.tip_link)?)?;
                Some(pose_of(&robot.placement) * *tip)
            }
        }
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
        if self.cell.job.as_ref().is_some_and(|(planning, ..)| *planning == path) {
            return PlanStatus::Planning;
        }
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

    /// Where planning runs. The default runs it inside [`Session::plan`]; an application passes an
    /// executor with threads so that its window stays responsive.
    pub fn set_executor(&mut self, executor: Executor) {
        self.cell.executor = executor;
    }

    /// Starts planning a path from the robot's current joint values through its waypoints. With the
    /// default executor the result is there when this returns; otherwise the status is
    /// [`PlanStatus::Planning`] until [`Session::poll_jobs`] collects it. A plan that was still being
    /// computed is cancelled.
    pub fn plan(&mut self, path: Id<MotionPath>) -> PlanStatus {
        self.end_jog();
        self.cell.playback = None;
        self.cancel_planning();
        let Some(inputs) = self.plan_inputs(path) else { return PlanStatus::NotPlanned };
        let loaded = self.doc.get(path).and_then(|p| self.robot_model(p.robot)).cloned();
        // The job works on its own copy of the document: edits made while it runs do not disturb it,
        // and the fingerprint tells afterwards whether its result is still current.
        let doc = self.doc.clone();
        let meshes = self.cell.meshes.clone();
        let job = self.cell.executor.spawn("plan", move |ctx| {
            let control = JobControl(ctx.cancel.clone());
            let result = match &loaded {
                Some(loaded) => plan_path(&doc, loaded, &meshes, path, &control),
                None => Err("the robot description could not be read".to_owned()),
            };
            (!ctx.cancel.is_cancelled()).then_some(result)
        });
        self.cell.job = Some((path, inputs, job));
        self.poll_jobs();
        self.plan_status(path)
    }

    /// True while a plan is being computed.
    pub fn is_planning(&self) -> bool {
        self.cell.job.is_some()
    }

    /// Stops the plan being computed, keeping the previous result.
    pub fn cancel_planning(&mut self) {
        if let Some((_, _, job)) = self.cell.job.take() {
            job.cancel();
        }
    }

    /// Collects finished background work. Returns true when something arrived. Call once per frame.
    pub fn poll_jobs(&mut self) -> bool {
        let Some((path, inputs, mut job)) = self.cell.job.take() else { return false };
        let outcome = match job.poll() {
            None => {
                self.cell.job = Some((path, inputs, job));
                return false;
            }
            Some(JobOutcome::Done(None)) => return true,
            Some(JobOutcome::Done(Some(outcome))) => outcome,
            Some(JobOutcome::Failed(reason)) => Err(format!("planning stopped unexpectedly: {reason}")),
        };
        if self.doc.get(path).is_some() {
            self.cell.plans.insert(path, Planned { inputs, outcome });
        }
        true
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

/// Plans a path of `doc` from its robot's current joint values through its waypoints.
fn plan_path(
    doc: &Document,
    loaded: &LoadedRobot,
    meshes: &BTreeMap<String, MeshRef>,
    path_id: Id<MotionPath>,
    control: &dyn Control,
) -> Result<PlannedPath, String> {
    let path = doc.get(path_id).ok_or("the path no longer exists")?;
    let robot = doc.get(path.robot).ok_or("the robot no longer exists")?;
    let algorithm = PLANNERS
        .iter()
        .find(|(id, _)| *id == path.planner)
        .map(|(_, algorithm)| *algorithm)
        .ok_or_else(|| format!("unknown planner `{}`", path.planner))?;
    let mut obstacles = Vec::new();
    for (_, body) in doc.bodies().iter() {
        // An obstacle that cannot be checked must not be planned through as if it were not there.
        let shape = shape_of(&body.solid, meshes)
            .ok_or_else(|| format!("the mesh of `{}` is not available", body.name))?;
        obstacles.push(Obstacle { name: body.name.clone(), pose: pose_of(&body.placement), shape });
    }
    let (scene, _) = RobotScene::new(
        Arc::clone(&loaded.model),
        loaded.chain.clone(),
        &robot.tip_link,
        pose_of(&robot.placement),
        &obstacles,
        PADDING,
        &|uri| loaded.file(uri),
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
        let plan = plan_motion(&scene, &request, control).map_err(|e| format!("{}: {e}", waypoint.name))?;
        if let Some(end) = plan.path.last() {
            start.clone_from(end);
        }
        segments.push(plan);
    }
    Ok(PlannedPath { segments })
}
