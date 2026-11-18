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
    Body, CellOp, Command, Created, Document, EditError, MotionPath, Placement, Robot, Sketch, Solid, Tool,
    Waypoint, WaypointTarget, WorkFrame,
};
use gc_jobs::{CancelToken, Executor, JobHandle, JobOutcome};
use gc_math::{DPose3, DVec3, pose_from_xyz_rpy, rpy_from_quat};
use gc_mesh::MeshData;
use gc_motion::{
    Goal, Guarantee, LinkPair, MotionPlan, MotionRequest, Obstacle, PairReason, RobotScene, StateProblem,
};
use gc_plan::{Algorithm, Control, Options};
use gc_robot::{Chain, RobotModel, Shape};
use gc_sketch::SkPoint;

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
    /// The box around the mesh in its own frame.
    pub bounds: gc_math::Aabb,
    /// The triangles.
    pub data: Arc<MeshData>,
}

/// A part of a mesh file as it is drawn: the mesh and the colour the file gives it, if any.
pub type ShownPart = (MeshRef, Option<[f32; 4]>);

/// Something of the robot cell that can be selected, in the viewport or in the browser.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CellItem {
    /// A body.
    Body(Id<Body>),
    /// A robot.
    Robot(Id<Robot>),
    /// A waypoint.
    Waypoint(Id<Waypoint>),
    /// A work frame.
    Frame(Id<WorkFrame>),
    /// A path.
    Path(Id<MotionPath>),
}

impl CellItem {
    /// The look of a body or robot.
    pub fn shown(self) -> Option<Shown> {
        match self {
            Self::Body(id) => Some(Shown::Body(id)),
            Self::Robot(id) => Some(Shown::Robot(id)),
            _ => None,
        }
    }

    /// The edit that deletes it.
    pub fn delete_op(self) -> CellOp {
        match self {
            Self::Body(id) => CellOp::DeleteBody(id),
            Self::Robot(id) => CellOp::DeleteRobot(id),
            Self::Waypoint(id) => CellOp::DeleteWaypoint(id),
            Self::Frame(id) => CellOp::DeleteFrame(id),
            Self::Path(id) => CellOp::DeletePath(id),
        }
    }
}

/// Something whose look can be changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shown {
    /// A body.
    Body(Id<Body>),
    /// A robot.
    Robot(Id<Robot>),
}

/// A robot description that was read successfully.
#[derive(Debug)]
pub struct LoadedRobot {
    urdf: String,
    /// The model.
    pub model: Arc<RobotModel>,
    /// The chain that is jogged and planned for.
    pub chain: Chain,
    /// Mesh files as stored, by the name the description uses: the parts with their colours, and all
    /// parts as one mesh for collision checking.
    files: BTreeMap<String, (Vec<gc_mesh::collada::Part>, Arc<MeshData>)>,
    /// The parts at the scale a geometry asks for, by name and scale, with each part's own colour.
    scaled: BTreeMap<String, Vec<ShownPart>>,
    /// Mesh files the description refers to that could not be supplied or read.
    pub missing: Vec<String>,
    /// The closed-form inverse kinematics a plugin offers for the chain, when one recognises it.
    pub analytic: Option<AnalyticIk>,
}

/// Closed-form inverse kinematics from a plugin, for one robot's chain.
#[derive(Clone)]
pub struct AnalyticIk {
    /// The kinematic family that recognised the chain, e.g. `ur6r`.
    pub family: String,
    solver: Arc<dyn gc_robotics_api::KinematicsSolver>,
}

impl std::fmt::Debug for AnalyticIk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnalyticIk").field("family", &self.family).finish_non_exhaustive()
    }
}

/// Joint values that put the tip of a chain at `flange` (in the chain's base frame), nearest to `start`
/// first: every closed-form solution, each turned by whole turns per joint to lie within the limits and
/// as near `start` as it can, then refined on the real chain (a calibrated arm differs slightly from
/// the nominal model). Empty without a closed form or when nothing is reachable within the limits.
pub(crate) fn analytic_goals(loaded: &LoadedRobot, flange: &DPose3, start: &[f64]) -> Vec<Vec<f64>> {
    use std::f64::consts::TAU;
    let Some(analytic) = &loaded.analytic else { return Vec::new() };
    let target =
        gc_robotics_api::Pose { xyz: flange.translation.to_array(), quat_xyzw: flange.rotation.to_array() };
    let (mut flat, mut branches) = (Vec::new(), Vec::new());
    if analytic.solver.solve_all(&target, Some(start), &mut flat, &mut branches).is_err() {
        return Vec::new();
    }
    let chain = &loaded.chain;
    let dof = chain.dof();
    let ik = gc_robot::NumericIk::new(chain, gc_robot::IkOptions::default());
    let mut goals: Vec<Vec<f64>> = flat
        .chunks(dof.max(1))
        .filter_map(|solution| {
            let turned: Option<Vec<f64>> = solution
                .iter()
                .enumerate()
                .map(|(j, value)| {
                    let (lower, upper) = (chain.lower()[j], chain.upper()[j]);
                    let near = start.get(j).copied().unwrap_or(0.0);
                    let k = ((near - value) / TAU).round();
                    [k, k - 1.0, k + 1.0]
                        .iter()
                        .map(|k| value + k * TAU)
                        .filter(|v| *v >= lower - 1e-9 && *v <= upper + 1e-9)
                        .min_by(|a, b| (a - near).abs().total_cmp(&(b - near).abs()))
                })
                .collect();
            ik.solve_near(flange, &turned?).ok()
        })
        .collect();
    let distance = |q: &Vec<f64>| q.iter().zip(start).map(|(a, b)| (a - b).abs()).fold(0.0_f64, f64::max);
    goals.sort_by(|a, b| distance(a).total_cmp(&distance(b)));
    goals
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
    Some(MeshRef { key: data.content_hash(), radius, bounds, data: Arc::new(data) })
}

fn scaled_key(uri: &str, scale: [f64; 3]) -> String {
    format!("{uri}|{scale:?}")
}

impl LoadedRobot {
    /// A mesh file as stored, all its parts as one mesh.
    pub fn file(&self, uri: &str) -> Option<Arc<MeshData>> {
        self.files.get(uri).map(|(_, merged)| Arc::clone(merged))
    }

    /// The parts of a mesh file at the scale a geometry asks for, each with the colour the file gives it.
    pub fn mesh(&self, uri: &str, scale: [f64; 3]) -> &[ShownPart] {
        self.scaled.get(&scaled_key(uri, scale)).map_or(&[], Vec::as_slice)
    }

    fn read_meshes(&mut self, resources: Option<&Resources>, resource_dir: &str) {
        let model = Arc::clone(&self.model);
        let geometries = model.links.iter().flat_map(|link| link.visuals.iter().chain(&link.collisions));
        for geometry in geometries {
            let Shape::Mesh { uri, scale } = &geometry.shape else { continue };
            if self.missing.contains(uri) || self.scaled.contains_key(&scaled_key(uri, *scale)) {
                continue;
            }
            if !self.files.contains_key(uri) {
                let near = || if resource_dir.is_empty() { None } else { read_near(resource_dir, uri) };
                let read = resources
                    .and_then(|resolve| resolve(uri))
                    .or_else(near)
                    .and_then(|bytes| gc_mesh::read_parts(&bytes).ok())
                    .and_then(|parts| Some((gc_mesh::merge(&parts).ok()?, parts)));
                match read {
                    Some((merged, parts)) => {
                        self.files.insert(uri.clone(), (parts, Arc::new(merged)));
                    }
                    None => {
                        self.missing.push(uri.clone());
                        continue;
                    }
                }
            }
            let Some((parts, _)) = self.files.get(uri) else { continue };
            let shown: Vec<ShownPart> =
                parts.iter().filter_map(|part| Some((scaled(&part.mesh, *scale)?, part.color))).collect();
            self.scaled.insert(scaled_key(uri, *scale), shown);
        }
    }
}

/// A planned motion through all waypoints of a path.
#[derive(Clone, Debug)]
pub struct PlannedPath {
    /// One motion per waypoint, in order.
    pub segments: Vec<MotionPlan>,
    /// The planner id and seed it was planned with.
    strategy: (String, u64),
}

impl PlannedPath {
    /// What the final checks establish for the whole path: the weakest of its motions' guarantees (the
    /// smallest padding and clearance, the largest step), with the number of states summed. `None` for a
    /// path without motions.
    pub fn guarantee(&self) -> Option<Guarantee> {
        let mut all = self.segments.iter().map(|s| &s.guarantee);
        let first = all.next()?.clone();
        Some(all.fold(first, |acc, g| Guarantee {
            padding_nominal: acc.padding_nominal.min(g.padding_nominal),
            padding_min: acc.padding_min.min(g.padding_min),
            step_deg: acc.step_deg.max(g.step_deg),
            step_mm: acc.step_mm.max(g.step_mm),
            clearance: acc.clearance.min(g.clearance),
            states: acc.states + g.states,
            model: acc.model,
            acm_hash: acc.acm_hash,
        }))
    }

    /// The highest speed of the tool tip over the whole motion, m/s.
    pub fn peak_tcp_speed(&self) -> f64 {
        self.segments.iter().map(|s| s.peak_tcp_speed).fold(0.0, f64::max)
    }

    /// Total duration in seconds.
    pub fn duration(&self) -> f64 {
        self.segments.iter().map(|s| s.trajectory.duration()).sum()
    }

    /// The joint values at time `t` (clamped to the motion).
    pub fn sample(&self, t: f64) -> Vec<f64> {
        let mut t = gc_math::clamp_between(t, 0.0, self.duration());
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

/// How one planner did on a path over several seeds.
#[derive(Clone, Debug, PartialEq)]
pub struct CompareRow {
    /// Planner id (see [`PLANNERS`]).
    pub planner: &'static str,
    /// Seeds tried.
    pub runs: usize,
    /// Seeds for which a valid motion was found.
    pub successes: usize,
    /// Median estimated duration of the successful runs, seconds.
    pub duration: Option<f64>,
    /// Median joint-space cost of the successful runs.
    pub cost: Option<f64>,
    /// Median number of state checks per run.
    pub state_checks: u64,
}

fn median(values: &mut [f64]) -> Option<f64> {
    values.sort_by(f64::total_cmp);
    values.get(values.len() / 2).copied()
}

/// A comparison being computed: the path, the fingerprint of its inputs and the job.
type CompareJob = (Id<MotionPath>, String, JobHandle<Option<Vec<CompareRow>>>);

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
/// Pseudo-random IK starts tried for a ghost besides the robot's own pose: as many as planning tries,
/// so that the ghost finds a pose whenever planning would.
const GHOST_STARTS: usize = 32;

/// How a robot would stand at a waypoint.
#[derive(Clone, Debug)]
pub struct Ghost {
    /// The robot the waypoint is for.
    pub robot: Id<Robot>,
    /// Joint values that put the tool on the waypoint, nearest to how the robot is shown; `None` when
    /// the arm cannot reach it.
    pub joints: Option<Vec<f64>>,
    /// What is wrong in that pose: a joint beyond its limit, a collision, too close to a body.
    pub problem: Option<StateProblem>,
    /// How many arm configurations reach the waypoint within the joint limits.
    pub solutions: usize,
    /// The kinematic family whose closed form found them; `None` when they were found numerically.
    pub analytic: Option<String>,
}

/// Where the contact solver stopped a drive or a drag (Inventor 2026 help, About the Contact Solver,
/// GUID-0BBD4ACE-7BD1-40BF-A011-74416B229035: contact pushes components that can move; when one that must
/// push cannot, the motion stops).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContactStop {
    /// For a drive: the value reached before contact.
    pub value: Option<f64>,
    /// The component that was moving.
    pub first: gc_core::AnyId,
    /// The component it met, which could not give way.
    pub second: gc_core::AnyId,
}

/// An assembly file read in the background, its meshes shared and their collision shapes built.
type ImportLoad = Result<(gc_doc::Assembly, Vec<Arc<MeshData>>), String>;

/// Component statuses and the revision they belong to.
pub(crate) type StatusCache =
    (gc_core::Revision, std::collections::BTreeMap<gc_core::AnyId, gc_doc::ComponentStatus>);

/// A mesh file read in the background: the file as read and the mesh at its scale.
type MeshLoad = Result<(Arc<MeshData>, MeshRef), String>;

/// Component poses, as a list and as a map.
type Poses = Vec<(gc_core::AnyId, DPose3)>;
type PoseMap = BTreeMap<gc_core::AnyId, DPose3>;

pub(crate) struct CellState {
    executor: Executor,
    job: Option<PlanJob>,
    /// A schedule being worked out for a revision of the document, and the last one worked out.
    schedule_job: Option<(gc_core::Revision, JobHandle<Option<crate::Schedule>>)>,
    schedule: Option<(gc_core::Revision, crate::Schedule)>,
    /// The moment of the schedule the robots are shown at.
    schedule_time: Option<f64>,
    compare_job: Option<CompareJob>,
    /// An assembly file being read in the background, and what came of the last one.
    import_job: Option<JobHandle<ImportLoad>>,
    import_result: Option<Result<Id<gc_doc::Assembly>, String>>,
    comparisons: BTreeMap<Id<MotionPath>, (String, Vec<CompareRow>)>,
    robots: BTreeMap<Id<Robot>, Arc<LoadedRobot>>,
    jog: Option<(Id<Robot>, Vec<f64>)>,
    /// A pose shown by Drive: never committed, and not touched by the jog previews.
    drive: Option<(Id<Robot>, Vec<f64>)>,
    /// Components shown where Drive puts an assembly relationship (never committed): their placements.
    relation_drive: BTreeMap<gc_core::AnyId, Placement>,
    driven_bodies: BTreeMap<Id<Body>, Body>,
    /// The relationship Drive shows and the value it stands at (after contact).
    drive_value: Option<(Id<gc_doc::Relationship>, f64)>,
    /// Components pushed by contact during the current drive or drag.
    contact_pushed: BTreeMap<gc_core::AnyId, DPose3>,
    /// Where contact stopped the last drive or drag step.
    contact_stop: Option<ContactStop>,
    body: Option<(Id<Body>, Body)>,
    base: Option<(Id<Robot>, Placement)>,
    tcp: Option<(Id<Robot>, Placement)>,
    tool: Option<(Id<Robot>, Option<Tool>)>,
    waypoint: Option<(Id<Waypoint>, WaypointTarget)>,
    frame: Option<(Id<WorkFrame>, WorkFrame)>,
    look: Option<(Shown, gc_doc::Look)>,
    selected: Option<CellItem>,
    /// The selected component occurrence (a body, robot or assembly, an assembly part, a robot link or
    /// tool), as the browser and the Ground toggle work with it.
    occurrence: Option<gc_doc::Occurrence>,
    hovered: Option<CellItem>,
    plans: BTreeMap<Id<MotionPath>, Planned>,
    playback: Option<(Id<MotionPath>, f64)>,
    resources: Option<Resources>,
    /// Meshes of imported assemblies, and how to read their files again.
    pub(crate) assemblies: crate::assembly::AssemblyMeshes,
    /// Plugins that offer closed-form inverse kinematics.
    kinematics: Vec<Arc<dyn gc_robotics_api::KinematicsFactory>>,
    /// Planners from plugins (`MOTION_PLANNERS`), used for ids the built-in table does not have.
    planners: Vec<Arc<dyn gc_robotics_api::MotionPlanner>>,
    /// The robot library folder, where `library://` resource folders point.
    library: Option<std::path::PathBuf>,
    /// The collision scene of the cell as it is shown, with what it was built from, and the last
    /// question asked of it with its answer. Rebuilt when a robot or a body moves.
    scene: std::cell::RefCell<Option<(String, RobotScene)>>,
    asked: std::cell::RefCell<Option<(String, Option<StateProblem>)>>,
    /// The last ghost worked out, with what it was worked out from.
    ghost: std::cell::RefCell<Option<(String, Ghost)>>,
    /// The constraint status of every component, for the document revision it was worked out for (the
    /// browser asks every frame).
    pub(crate) statuses: std::cell::RefCell<Option<StatusCache>>,
    /// Mesh files of bodies by path: the triangles as stored, or why the file could not be read.
    files: BTreeMap<String, Result<Arc<MeshData>, String>>,
    /// Mesh files being read (and their collision shapes built) in the background, by mesh key.
    mesh_jobs: BTreeMap<String, (String, JobHandle<MeshLoad>)>,
    /// The same at the scale a body asks for, by path and scale.
    pub(crate) meshes: BTreeMap<String, MeshRef>,
}

impl Default for CellState {
    fn default() -> Self {
        Self {
            // Jobs run to completion where they are started unless the application supplies threads.
            executor: Executor::inline(),
            job: None,
            schedule_job: None,
            schedule: None,
            schedule_time: None,
            compare_job: None,
            import_job: None,
            import_result: None,
            comparisons: BTreeMap::new(),
            robots: BTreeMap::new(),
            jog: None,
            drive: None,
            relation_drive: BTreeMap::new(),
            driven_bodies: BTreeMap::new(),
            drive_value: None,
            contact_pushed: BTreeMap::new(),
            contact_stop: None,
            body: None,
            base: None,
            tcp: None,
            tool: None,
            waypoint: None,
            frame: None,
            look: None,
            selected: None,
            occurrence: None,
            hovered: None,
            plans: BTreeMap::new(),
            playback: None,
            resources: None,
            assemblies: Default::default(),
            kinematics: Vec::new(),
            planners: Vec::new(),
            library: None,
            files: BTreeMap::new(),
            mesh_jobs: BTreeMap::new(),
            meshes: BTreeMap::new(),
            scene: std::cell::RefCell::new(None),
            asked: std::cell::RefCell::new(None),
            ghost: std::cell::RefCell::new(None),
            statuses: std::cell::RefCell::new(None),
        }
    }
}

impl CellState {
    /// Brings the caches in line with the document.
    pub(crate) fn sync(&mut self, doc: &Document) {
        self.robots.retain(|id, loaded| doc.get(*id).is_some_and(|r| r.urdf == loaded.urdf));
        for (id, robot) in doc.robots().iter() {
            if !self.robots.contains_key(&id)
                && let Ok((mut loaded, ..)) = load(
                    &robot.urdf,
                    Some((&robot.base_link, &robot.tip_link)),
                    self.resources.as_ref(),
                    &self.resolve(&robot.resource_dir),
                )
            {
                // The first plugin that recognises the chain gives its closed form.
                if let Some(spec) = loaded.chain.to_spec() {
                    loaded.analytic = self.kinematics.iter().find_map(|factory| {
                        factory.probe(&spec, None)?;
                        let solver = factory.build(&spec).ok()?;
                        Some(AnalyticIk { family: factory.id().to_owned(), solver: Arc::from(solver) })
                    });
                }
                self.robots.insert(id, Arc::new(loaded));
            }
        }
        for (_, body) in doc.bodies().iter() {
            self.read_body_mesh(&body.solid);
        }
        self.assemblies.sync(doc);
        for (_, robot) in doc.robots().iter() {
            if let Some(tool) = &robot.tool {
                self.read_body_mesh(&tool.solid);
            }
        }
        self.plans.retain(|id, _| doc.get(*id).is_some());
        self.comparisons.retain(|id, _| doc.get(*id).is_some());
        if self.jog.as_ref().is_some_and(|(id, _)| doc.get(*id).is_none()) {
            self.jog = None;
        }
        if self.frame.as_ref().is_some_and(|(id, _)| doc.get(*id).is_none()) {
            self.frame = None;
        }
        if self.waypoint.as_ref().is_some_and(|(id, _)| doc.get(*id).is_none()) {
            self.waypoint = None;
        }
        if self.tcp.as_ref().is_some_and(|(id, _)| doc.get(*id).is_none()) {
            self.tcp = None;
        }
        if self.base.as_ref().is_some_and(|(id, _)| doc.get(*id).is_none()) {
            self.base = None;
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
    let parts = gc_mesh::read_parts(&bytes).map_err(|e| format!("{path}: {e}"))?;
    gc_mesh::merge(&parts).map(Arc::new).map_err(|e| format!("{path}: {e}"))
}

impl CellState {
    /// A robot's resource folder as a path on this machine: `library://<package>` inside the library.
    fn resolve(&self, resource_dir: &str) -> String {
        match (resource_dir.strip_prefix(crate::library::LIBRARY_SCHEME), &self.library) {
            (Some(package), Some(library)) => library.join(package).to_string_lossy().into_owned(),
            _ => resource_dir.to_owned(),
        }
    }

    /// Makes sure the mesh of a body's solid is loaded at its scale.
    /// Starts reading a body's or tool's mesh file in the background (the file as read, scaled, with its
    /// collision shape built), unless it is loaded or on its way. With the inline executor it is there on
    /// return.
    fn read_body_mesh(&mut self, solid: &Solid) {
        let Solid::Mesh { file, scale } = solid else { return };
        let key = scaled_key(file, [*scale; 3]);
        if self.meshes.contains_key(&key) || self.mesh_jobs.contains_key(&key) {
            return;
        }
        let known = match self.files.get(file) {
            Some(Err(_)) => return,
            Some(Ok(data)) => Some(Arc::clone(data)),
            None => None,
        };
        let (path, scale) = (file.clone(), *scale);
        let job = self.executor.spawn("mesh", move |_| {
            let data = match known {
                Some(data) => data,
                None => read_mesh_file(&path)?,
            };
            let mesh = scaled(&data, [scale; 3]).ok_or_else(|| format!("{path}: cannot scale the mesh"))?;
            // Its collision shape too, so that the first drag or measurement does not build it.
            gc_collide::prepare_mesh(&mesh.data);
            Ok((data, mesh))
        });
        self.mesh_jobs.insert(key, (file.clone(), job));
        self.collect_meshes();
    }

    /// Takes in the meshes that have finished loading. True when one arrived (or failed).
    pub(crate) fn collect_meshes(&mut self) -> bool {
        let mut arrived = false;
        let keys: Vec<String> = self.mesh_jobs.keys().cloned().collect();
        for key in keys {
            let Some((file, mut job)) = self.mesh_jobs.remove(&key) else { continue };
            match job.poll() {
                None => {
                    self.mesh_jobs.insert(key, (file, job));
                }
                Some(JobOutcome::Done(Ok((data, mesh)))) => {
                    self.files.insert(file, Ok(data));
                    self.meshes.insert(key, mesh);
                    arrived = true;
                }
                Some(JobOutcome::Done(Err(reason)) | JobOutcome::Failed(reason)) => {
                    self.files.insert(file, Err(reason));
                    arrived = true;
                }
            }
        }
        arrived
    }

    fn body_mesh(&self, solid: &Solid) -> Option<&MeshRef> {
        match solid {
            Solid::Mesh { file, scale } => self.meshes.get(&scaled_key(file, [*scale; 3])),
            _ => None,
        }
    }
}

/// The collision shapes of a robot's tool in its tip link's frame: a mesh as its convex parts. `None`
/// when the tool's mesh is not available.
pub(crate) fn tool_shapes(
    tool: Option<&Tool>,
    meshes: &BTreeMap<String, MeshRef>,
) -> Option<Vec<(DPose3, ShapeDesc)>> {
    let Some(tool) = tool else { return Some(Vec::new()) };
    let pose = pose_of(&tool.placement);
    Some(match shape_of(&tool.solid, meshes)? {
        ShapeDesc::Mesh(mesh) => gc_collide::hulls_of(&mesh).into_iter().map(|hull| (pose, hull)).collect(),
        shape => vec![(pose, shape)],
    })
}

/// The collision shape of a body's solid.
pub(crate) fn shape_of(solid: &Solid, meshes: &BTreeMap<String, MeshRef>) -> Option<ShapeDesc> {
    Some(match solid {
        Solid::Box { size } => ShapeDesc::Box(*size),
        Solid::Cylinder { radius, length } => ShapeDesc::Cylinder { radius: *radius, length: *length },
        Solid::Sphere { radius } => ShapeDesc::Ball(*radius),
        Solid::Mesh { file, scale } => {
            ShapeDesc::Mesh(Arc::clone(&meshes.get(&scaled_key(file, [*scale; 3]))?.data))
        }
    })
}

/// The bytes of a file a robot description refers to, looked for around the folder the description came
/// from: `file://` and plain paths as they are, `package://name/rest` under that folder and up to four
/// folders above it, with and without the package name.
fn read_near(resource_dir: &str, uri: &str) -> Option<Vec<u8>> {
    std::fs::read(find_near(resource_dir, uri)?).ok()
}

/// Where a file a robot description refers to is, looked for as [`read_near`] does.
pub(crate) fn find_near(resource_dir: &str, uri: &str) -> Option<std::path::PathBuf> {
    use std::path::{Path, PathBuf};
    let dir = Path::new(resource_dir);
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(rest) = uri.strip_prefix("package://") {
        let (package, inside) = rest.split_once('/')?;
        for above in dir.ancestors().take(5) {
            candidates.push(above.join(inside));
            candidates.push(above.join(package).join(inside));
        }
    } else {
        let plain = Path::new(uri.strip_prefix("file://").unwrap_or(uri));
        candidates.push(if plain.is_absolute() { plain.to_path_buf() } else { dir.join(plain) });
    }
    candidates.into_iter().find(|path| path.is_file())
}

/// Reads a robot description and picks its planning chain. Returns the tip link's name with it.
fn load(
    urdf: &str,
    chain: Option<(&str, &str)>,
    resources: Option<&Resources>,
    resource_dir: &str,
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
        analytic: None,
    };
    loaded.read_meshes(resources, resource_dir);
    Ok((loaded, base, tip))
}

pub(crate) fn pose_of(placement: &Placement) -> DPose3 {
    pose_from_xyz_rpy(placement.xyz, placement.rpy)
}

/// The world pose the tool takes at a sketch point: tip on the point, held as the point's waypoint role
/// says, or without one with the tool axis against the plane normal.
fn pose_at_point(doc: &Document, sketch: Id<Sketch>, point: Id<SkPoint>) -> Option<DPose3> {
    doc.sketch_point_pose(sketch, point)
}

/// The world pose of a work frame: at its origin point, with the sketch's axes turned by its angles.
fn work_frame_pose(doc: &Document, frame: Id<WorkFrame>) -> Option<DPose3> {
    frame_pose_of(doc, doc.get(frame)?)
}

fn frame_pose_of(doc: &Document, frame: &WorkFrame) -> Option<DPose3> {
    doc.work_frame_pose_of(frame)
}

/// The world pose of the tool centre point at a waypoint target given as a pose; `base` is the world pose
/// of the waypoint robot's base frame. `None` for joint values.
fn target_pose(doc: &Document, target: &WaypointTarget, base: Option<DPose3>) -> Option<DPose3> {
    match target {
        WaypointTarget::Joints(_) => None,
        WaypointTarget::SketchPoint { sketch, point } => pose_at_point(doc, *sketch, *point),
        WaypointTarget::InFrame { frame, offset } => Some(work_frame_pose(doc, *frame)? * pose_of(offset)),
        WaypointTarget::InWorld { offset } => Some(pose_of(offset)),
        WaypointTarget::InBase { offset } => Some(base? * pose_of(offset)),
    }
}

/// The world pose of a robot's base frame when its root link stands at `placement`: its link named
/// `base` (fixed to the root on UR arms, where the teach pendant measures from it), else the root link.
fn base_frame(loaded: &LoadedRobot, placement: &Placement) -> DPose3 {
    let mut poses = Vec::new();
    loaded.model.link_poses(&loaded.chain.expand(&vec![0.0; loaded.chain.dof()]), &mut poses);
    let local = loaded.model.link("base").and_then(|i| poses.get(i).copied()).unwrap_or(DPose3::IDENTITY);
    pose_of(placement) * local
}

/// A placement holding a pose.
fn placement_of(pose: &DPose3) -> Placement {
    Placement { xyz: pose.translation.to_array(), rpy: rpy_from_quat(pose.rotation) }
}

/// What the numbers of a waypoint or a pose are measured from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reference {
    /// The world frame.
    World,
    /// The robot's base frame.
    Base,
    /// A work frame.
    Frame(Id<WorkFrame>),
}

impl Session {
    fn cell_op(&mut self, op: CellOp) -> Result<Vec<Created>, EditError> {
        self.execute(&Command::Cell(op))
    }

    /// Gives the session the planners plugins offer (`MOTION_PLANNERS`). A path whose planner id is not
    /// one of the built-in [`PLANNERS`] is planned with the registered planner of that id. (The built-in
    /// ids run `gc_plan` directly, which gives the same paths as their registered versions.)
    pub fn set_planners(&mut self, planners: Vec<Arc<dyn gc_robotics_api::MotionPlanner>>) {
        self.cell.planners = planners;
    }

    /// Ids and names (Korean, English) of the planners a path can use: the built-in ones, then those of
    /// plugins with other ids.
    pub fn planner_choices(&self) -> Vec<(String, String, String)> {
        let mut choices: Vec<(String, String, String)> = gc_plugin_planners_names();
        for planner in &self.cell.planners {
            let d = planner.descriptor();
            if !choices.iter().any(|(id, ..)| *id == d.id) {
                choices.push((d.id.clone(), d.name.ko.clone(), d.name.en.clone()));
            }
        }
        choices
    }

    /// Sets the plugins that offer closed-form inverse kinematics, and reloads the robots.
    pub fn set_kinematics(&mut self, factories: Vec<Arc<dyn gc_robotics_api::KinematicsFactory>>) {
        self.cell.kinematics = factories;
        self.cell.robots.clear();
        self.cell.sync(&self.doc);
    }

    /// Gives the session a way to read assembly files again, for projects that contain imported
    /// assemblies (the readers live in plugins). Files that failed to read are tried again.
    pub fn set_assembly_reader(&mut self, reader: crate::assembly::AssemblyReader) {
        self.cell.assemblies.reader = Some(reader);
        self.cell.assemblies.by_file.retain(|_, read| read.is_ok());
        self.cell.assemblies.sync(&self.doc);
    }

    /// Adds an assembly imported from a file, with its meshes in the order of `assembly.meshes` (one
    /// undo step). Its parts are obstacles for planning from now on.
    pub fn add_assembly(
        &mut self,
        assembly: gc_doc::Assembly,
        meshes: Vec<MeshData>,
    ) -> Result<Id<gc_doc::Assembly>, String> {
        self.add_assembly_shared(assembly, meshes.into_iter().map(Arc::new).collect())
    }

    fn add_assembly_shared(
        &mut self,
        assembly: gc_doc::Assembly,
        meshes: Vec<Arc<MeshData>>,
    ) -> Result<Id<gc_doc::Assembly>, String> {
        if meshes.len() != assembly.meshes.len() {
            return Err(format!(
                "{}: {} meshes for {} entries",
                assembly.file,
                meshes.len(),
                assembly.meshes.len()
            ));
        }
        let file = assembly.file.clone();
        self.cell.assemblies.insert(&file, meshes);
        match self.cell_op(CellOp::AddAssembly(assembly)).map_err(|e| e.to_string())?.first() {
            Some(Created::Assembly(id)) => Ok(*id),
            _ => Err("the assembly was not added".to_owned()),
        }
    }

    /// Reads an assembly file in the background with `read` (a plugin's reader turned into the document's
    /// form) and adds it when [`Session::poll_jobs`] collects it (with the default executor, at once). An
    /// import still running is cancelled. See [`Session::import_status`].
    pub fn import_assembly(&mut self, file: &str, read: crate::assembly::Importer) {
        if let Some(job) = self.cell.import_job.take() {
            job.cancel();
        }
        self.cell.import_result = None;
        let file = file.to_owned();
        self.cell.import_job = Some(self.cell.executor.spawn("import", move |_| {
            let (assembly, meshes) = read(&file)?;
            // The collision shapes too, on this thread, so that contact and interference find them built.
            let meshes: Vec<Arc<MeshData>> = meshes.into_iter().map(Arc::new).collect();
            meshes.iter().for_each(gc_collide::prepare_mesh);
            Ok((assembly, meshes))
        }));
        self.poll_jobs();
    }

    /// `None` while an import runs (or none was started); then the new assembly, or why it failed.
    pub fn import_status(&self) -> Option<&Result<Id<gc_doc::Assembly>, String>> {
        if self.cell.import_job.is_some() { None } else { self.cell.import_result.as_ref() }
    }

    /// Where a work feature is in the world; `None` when its definition no longer works out (a reference
    /// is gone, three points on one line, two equal points).
    pub fn work_shape(&self, id: Id<gc_doc::WorkFeature>) -> Option<gc_doc::WorkShape> {
        self.doc.work_shape(id)
    }

    /// The meshes of an assembly placed in the world, to draw; or why they are not available.
    pub fn assembly_parts(
        &self,
        id: Id<gc_doc::Assembly>,
    ) -> Result<Vec<crate::assembly::ShownAssemblyPart>, String> {
        let assembly = self.doc.get(id).ok_or("the assembly no longer exists")?;
        let meshes = self.cell.assemblies.of(assembly)?;
        let keep = self.doc.parts_shown(id, false);
        let placement = self.driven_placement(id.erase()).unwrap_or(assembly.placement);
        Ok(crate::assembly::shown_parts(assembly, &pose_of(&placement), meshes, &keep))
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
        self.add_robot_from(name, urdf, "")
    }

    /// Adds a robot from a description file (URDF). Mesh files it refers to are looked for around the
    /// file's folder; the ones not found are listed in [`LoadedRobot::missing`] and simply not drawn or
    /// checked.
    pub fn add_robot_file(&mut self, path: &std::path::Path) -> Result<Id<Robot>, String> {
        let urdf = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let name =
            path.file_stem().map_or_else(|| "Robot".to_owned(), |stem| stem.to_string_lossy().into_owned());
        let dir = path.parent().map(|dir| dir.to_string_lossy().into_owned()).unwrap_or_default();
        self.add_robot_from(&name, &urdf, &dir).map_err(|e| format!("{}:\n{e}", path.display()))
    }

    /// Sets the robot library folder (where `library://` resource folders point) and reloads the robots.
    pub fn set_robot_library(&mut self, dir: Option<std::path::PathBuf>) {
        self.cell.library = dir;
        self.cell.robots.clear();
        self.cell.sync(&self.doc);
    }

    /// The robot library folder, when there is one.
    pub fn robot_library(&self) -> Option<&std::path::Path> {
        self.cell.library.as_deref()
    }

    /// Adds a robot from a package of the robot library at the world origin.
    pub fn add_library_robot(&mut self, package: &crate::library::Package) -> Result<Id<Robot>, String> {
        let name = format!("{}_{}", package.info.name, self.doc.robots().len() + 1);
        self.add_robot_from(&name, &package.urdf()?, &package.resource_dir())
    }

    fn add_robot_from(&mut self, name: &str, urdf: &str, resource_dir: &str) -> Result<Id<Robot>, String> {
        let dir = self.cell.resolve(resource_dir);
        let (loaded, base_link, tip_link) = load(urdf, None, self.cell.resources.as_ref(), &dir)?;
        let robot = Robot {
            name: name.to_owned(),
            urdf: urdf.to_owned(),
            resource_dir: resource_dir.to_owned(),
            base_link,
            tip_link,
            placement: Placement::default(),
            tcp: Placement::default(),
            joints: vec![0.0; loaded.chain.dof()],
            max_acc: 5.0,
            look: Default::default(),
            tool: None,
            acm_overrides: Vec::new(),
            links: link_tree(&loaded.model, &loaded.chain),
        };
        match self.cell_op(CellOp::AddRobot(robot)).map_err(|e| e.to_string())?.first() {
            Some(Created::Robot(id)) => Ok(*id),
            _ => Err("the robot was not created".to_owned()),
        }
    }

    /// Adds a body with a name that is not taken yet.
    pub fn add_body(&mut self, solid: Solid, placement: Placement) -> Result<Id<Body>, EditError> {
        let name = format!("Body{}", self.doc.bodies().len() + 1);
        match self
            .cell_op(CellOp::AddBody(Body { name, placement, solid, look: Default::default() }))?
            .first()
        {
            Some(Created::Body(id)) => Ok(*id),
            _ => Err(EditError::BadValue),
        }
    }

    /// The mesh of a body whose solid is a mesh file; `None` for other solids and for files that could
    /// not be read (see [`Session::mesh_error`]).
    pub fn body_mesh(&self, solid: &Solid) -> Option<&MeshRef> {
        self.cell.body_mesh(solid)
    }

    /// True while a body's or tool's mesh is still being read in the background: draw a placeholder
    /// (its body is in the document already; the mesh arrives with [`Session::poll_jobs`]).
    pub fn mesh_pending(&self, solid: &Solid) -> bool {
        match solid {
            Solid::Mesh { file, scale } => self.cell.mesh_jobs.contains_key(&scaled_key(file, [*scale; 3])),
            _ => false,
        }
    }

    /// How many mesh files are still being read.
    pub fn meshes_loading(&self) -> usize {
        self.cell.mesh_jobs.len()
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
        let body = Body {
            name,
            placement: Placement::default(),
            solid: Solid::Mesh { file, scale },
            look: Default::default(),
        };
        match self.cell_op(CellOp::AddBody(body)).map_err(|e| e.to_string())?.first() {
            Some(Created::Body(id)) => Ok(*id),
            _ => Err("the body was not created".to_owned()),
        }
    }

    /// Makes a mesh file the tool a robot carries, starting at its flange. A file drawn in millimetres
    /// is scaled to metres, as for bodies.
    pub fn set_tool_mesh(&mut self, robot: Id<Robot>, path: &std::path::Path) -> Result<(), String> {
        let file = path.to_string_lossy().into_owned();
        let data = read_mesh_file(&file)?;
        let scale = if data.aabb().diagonal() > 20.0 { 0.001 } else { 1.0 };
        self.cell.files.insert(file.clone(), Ok(data));
        let tool = Tool { solid: Solid::Mesh { file, scale }, placement: Placement::default() };
        self.cell.read_body_mesh(&tool.solid);
        self.cell_op(CellOp::SetTool { id: robot, tool: Some(tool) }).map(|_| ()).map_err(|e| e.to_string())
    }

    /// Mounts a robot's tool on its flange by a rigid joint: the joint origin on the tool (in the tool's
    /// own frame, e.g. the centre of its mounting face with Z out of it) meets the flange frame, Z axes
    /// opposed unless `flip` (assembly P1-11). One undo step; refused when the robot has no tool.
    pub fn mount_tool(
        &mut self,
        robot: Id<Robot>,
        origin: gc_doc::Geometry,
        flip: bool,
    ) -> Result<(), EditError> {
        let mut tool =
            self.doc.get(robot).and_then(|r| r.tool.as_deref().cloned()).ok_or(EditError::BadValue)?;
        let pose = origin.rigid_mount(flip);
        tool.placement =
            Placement { xyz: pose.translation.to_array(), rpy: gc_math::rpy_from_quat(pose.rotation) };
        self.cell_op(CellOp::SetTool { id: robot, tool: Some(tool) }).map(|_| ())
    }

    /// The selected item of the cell.
    pub fn selected_item(&self) -> Option<CellItem> {
        self.cell.selected.filter(|item| self.item_exists(*item) && self.item_in_edit_scope(*item))
    }

    /// Bodies and robots outside the component being edited in place cannot be selected (or grounded,
    /// deleted, ... through the selection); other items are not components.
    fn item_in_edit_scope(&self, item: CellItem) -> bool {
        match item {
            CellItem::Body(id) => self.in_edit_scope(gc_doc::Occurrence::Item(id.erase())),
            CellItem::Robot(id) => self.in_edit_scope(gc_doc::Occurrence::Item(id.erase())),
            CellItem::Waypoint(_) | CellItem::Path(_) | CellItem::Frame(_) => true,
        }
    }

    /// Selects an item of the cell, or nothing. A body or robot is also the selected occurrence; other
    /// items (waypoints, frames, paths) leave no occurrence selected.
    pub fn select_item(&mut self, item: Option<CellItem>) {
        self.cell.selected = item.filter(|item| self.item_exists(*item) && self.item_in_edit_scope(*item));
        self.cell.occurrence = match self.cell.selected {
            Some(CellItem::Body(id)) => Some(gc_doc::Occurrence::Item(id.erase())),
            Some(CellItem::Robot(id)) => Some(gc_doc::Occurrence::Item(id.erase())),
            _ => None,
        };
    }

    /// The selected component occurrence, if it still exists.
    pub fn selected_occurrence(&self) -> Option<gc_doc::Occurrence> {
        self.cell.occurrence.filter(|o| self.doc.has_occurrence(*o) && self.in_edit_scope(*o))
    }

    /// Selects a component occurrence (from the browser or by picking a part, link or tool), or nothing.
    /// The cell item selection follows: a body or robot is selected as itself; a robot's link or tool
    /// selects its robot (whose panels apply); an imported assembly or one of its parts selects no cell
    /// item. Refused (nothing changes) for an occurrence that does not exist or lies outside the component
    /// being edited in place.
    pub fn select_occurrence(&mut self, occurrence: Option<gc_doc::Occurrence>) -> bool {
        if occurrence.is_some_and(|o| !self.doc.has_occurrence(o) || !self.in_edit_scope(o)) {
            return false;
        }
        self.cell.occurrence = occurrence;
        self.cell.selected = occurrence.and_then(|o| match o {
            gc_doc::Occurrence::Item(item) => item
                .downcast::<Body>()
                .map(CellItem::Body)
                .or_else(|| item.downcast::<Robot>().map(CellItem::Robot)),
            gc_doc::Occurrence::Link { robot, .. } | gc_doc::Occurrence::Tool { robot } => {
                Some(CellItem::Robot(robot))
            }
            gc_doc::Occurrence::Part { .. } => None,
        });
        true
    }

    /// True when the Ground toggle applies to the occurrence: a body, robot or imported assembly. (An
    /// assembly part's grounded state can be stored but does not change anything, since parts move with
    /// their assembly; a robot's links and tool cannot be grounded.)
    pub fn can_ground(&self, occurrence: gc_doc::Occurrence) -> bool {
        matches!(occurrence, gc_doc::Occurrence::Item(_)) && self.doc.has_occurrence(occurrence)
    }

    /// The Ground toggle on the selected occurrence: grounds it, or frees it when it is grounded. One
    /// undo step; refused when nothing groundable is selected.
    pub fn toggle_ground_selected(&mut self) -> Result<bool, EditError> {
        let occurrence =
            self.selected_occurrence().filter(|o| self.can_ground(*o)).ok_or(EditError::BadValue)?;
        let grounded = self.doc.component_flag(occurrence, gc_doc::ComponentFlag::Grounded);
        self.set_component(occurrence, gc_doc::ComponentFlag::Grounded, !grounded)?;
        Ok(!grounded)
    }

    /// The item under the cursor (shown highlighted before it is clicked).
    pub fn hovered_item(&self) -> Option<CellItem> {
        self.cell.hovered.filter(|item| self.item_exists(*item))
    }

    /// Tells the session which item is under the cursor. Returns true when that changed.
    pub fn hover_item(&mut self, item: Option<CellItem>) -> bool {
        let changed = self.cell.hovered != item;
        self.cell.hovered = item;
        changed
    }

    /// True while the item is in the document.
    pub fn item_exists(&self, item: CellItem) -> bool {
        match item {
            CellItem::Body(id) => self.doc.get(id).is_some(),
            CellItem::Robot(id) => self.doc.get(id).is_some(),
            CellItem::Waypoint(id) => self.doc.get(id).is_some(),
            CellItem::Frame(id) => self.doc.get(id).is_some(),
            CellItem::Path(id) => self.doc.get(id).is_some(),
        }
    }

    /// The name of an item, as the browser shows it.
    pub fn item_name(&self, item: CellItem) -> Option<String> {
        Some(match item {
            CellItem::Body(id) => self.doc.get(id)?.name.clone(),
            CellItem::Robot(id) => self.doc.get(id)?.name.clone(),
            CellItem::Waypoint(id) => self.doc.get(id)?.name.clone(),
            CellItem::Frame(id) => self.doc.get(id)?.name.clone(),
            CellItem::Path(id) => self.doc.get(id)?.name.clone(),
        })
    }

    /// Deletes an item. What is still in use is refused, and the status bar says so.
    pub fn delete_item(&mut self, item: CellItem) -> Result<(), EditError> {
        self.cell_op(item.delete_op()).map(|_| ())
    }

    /// How a body or a robot is shown: the preview while it is being changed, else the document's.
    pub fn shown_look(&self, item: Shown) -> Option<gc_doc::Look> {
        if let Some((previewed, look)) = self.cell.look
            && previewed == item
        {
            return Some(look);
        }
        match item {
            Shown::Body(id) => self.doc.get(id).map(|b| b.look),
            Shown::Robot(id) => self.doc.get(id).map(|r| r.look),
        }
    }

    /// Previews a look without touching the document (e.g. while a slider is dragged).
    pub fn preview_look(&mut self, item: Shown, look: gc_doc::Look) {
        if look.is_valid() {
            self.cell.look = Some((item, look));
        }
    }

    /// Commits the look preview as one undo step.
    pub fn end_look_preview(&mut self) {
        if let Some((item, look)) = self.cell.look.take() {
            drop(self.set_look(item, look));
        }
    }

    /// Changes how a body or a robot is shown, as one undo step. Nothing else changes: a hidden body is
    /// still an obstacle, and plans stay current.
    pub fn set_look(&mut self, item: Shown, look: gc_doc::Look) -> Result<(), EditError> {
        if self.doc_look(item) == Some(look) {
            return Ok(());
        }
        let op = match item {
            Shown::Body(id) => CellOp::SetBodyLook { id, look },
            Shown::Robot(id) => CellOp::SetRobotLook { id, look },
        };
        self.cell_op(op).map(|_| ())
    }

    fn doc_look(&self, item: Shown) -> Option<gc_doc::Look> {
        match item {
            Shown::Body(id) => self.doc.get(id).map(|b| b.look),
            Shown::Robot(id) => self.doc.get(id).map(|r| r.look),
        }
    }

    /// A body as it is shown: the preview while it is being edited, else the document's.
    pub fn shown_body(&self, body: Id<Body>) -> Option<&Body> {
        match &self.cell.body {
            Some((id, preview)) if *id == body => Some(preview),
            _ if self.doc.is_component_suppressed(body.erase()) => None,
            _ => self.cell.driven_bodies.get(&body).or_else(|| self.doc.get(body)),
        }
    }

    /// Previews an edit of a body without touching the document.
    pub fn preview_body(&mut self, id: Id<Body>, body: Body) {
        if self.doc.get(id).is_none() || !body.solid.is_valid() {
            return;
        }
        self.cell.read_body_mesh(&body.solid);
        self.cell.contact_stop = None;
        let participants = self.doc.contact_participants();
        if participants.len() >= 2 && participants.contains(&id.erase()) {
            // The contact solver: a dragged body pushes free components of the set, and stops where
            // something cannot give way (it stays where it last was).
            let mut pushed = self.cell.contact_pushed.clone();
            let moves = [(id.erase(), pose_of(&body.placement))];
            match self.resolve_contacts(&participants, &moves, &mut pushed) {
                Ok(()) => {
                    self.cell.contact_pushed = pushed;
                    let shown: Vec<(gc_core::AnyId, DPose3)> =
                        self.cell.contact_pushed.iter().map(|(i, p)| (*i, *p)).collect();
                    self.show_relation_drive(shown);
                }
                Err((first, second)) => {
                    self.cell.contact_stop = Some(ContactStop { value: None, first, second });
                    return;
                }
            }
        }
        self.cell.body = Some((id, body));
    }

    /// Commits the body preview as one undo step.
    pub fn end_body_preview(&mut self) {
        let pushed = std::mem::take(&mut self.cell.contact_pushed);
        self.cell.contact_stop = None;
        if !pushed.is_empty() {
            self.cell.relation_drive.clear();
            self.cell.driven_bodies.clear();
        }
        let Some((id, body)) = self.cell.body.take() else { return };
        let Some(stored) = self.doc.get(id) else { return };
        if !pushed.is_empty()
            && stored.solid == body.solid
            && stored.look == body.look
            && stored.name == body.name
        {
            // A drag that pushed others: everything moves in one undo step.
            let mut moves = vec![(id.erase(), body.placement)];
            moves.extend(pushed.iter().map(|(item, pose)| {
                (
                    *item,
                    Placement {
                        xyz: pose.translation.to_array(),
                        rpy: gc_math::rpy_from_quat(pose.rotation),
                    },
                )
            }));
            self.last_error = self.cell_op(CellOp::MoveComponents(moves)).err();
        } else if *stored != body {
            self.last_error = self.cell_op(CellOp::SetBody { id, body }).err();
        }
    }

    /// The joint values a robot is shown with: the playback position, else the jog preview, else the
    /// document's.
    pub fn robot_joints(&self, robot: Id<Robot>) -> Option<Vec<f64>> {
        // A schedule being played back shows every robot where it is at that moment.
        if let (Some(time), Some((_, schedule))) = (self.cell.schedule_time, &self.cell.schedule)
            && let Some(joints) = schedule.joints_at(robot, time)
        {
            return Some(joints);
        }
        if let Some((path, time)) = self.cell.playback
            && self.doc.get(path).is_some_and(|p| p.robot == robot)
            && let Some(planned) = self.planned(path)
        {
            return Some(planned.sample(time));
        }
        if let Some((id, q)) = &self.cell.drive
            && *id == robot
        {
            return Some(q.clone());
        }
        match &self.cell.jog {
            Some((id, q)) if *id == robot => Some(q.clone()),
            _ => self.doc.get(robot).map(|r| r.joints.clone()),
        }
    }

    /// Where a robot's base is shown: the preview while it is being placed, else the document's.
    pub fn shown_base(&self, robot: Id<Robot>) -> Option<Placement> {
        match &self.cell.base {
            Some((id, preview)) if *id == robot => Some(*preview),
            _ => self
                .cell
                .relation_drive
                .get(&robot.erase())
                .copied()
                .or_else(|| self.doc.get(robot).map(|r| r.placement)),
        }
    }

    /// Drive for an assembly relationship: shows the components where the relationship driven to `value`
    /// puts them (the offset in metres of a Mate, Flush or Insert; the turn in radians of a Rotational
    /// joint, from where it stands), without changing the document, until [`Self::end_drive_relationship`].
    /// Refused for a Rigid joint or a value the relationships cannot reach; the shown positions then stay.
    pub fn drive_relationship(&mut self, id: Id<gc_doc::Relationship>, value: f64) -> Result<(), EditError> {
        self.cell.contact_stop = None;
        let participants = self.doc.contact_participants();
        if participants.len() < 2 {
            let moves = self.doc.driven(id, value)?;
            self.show_relation_drive(moves);
            self.cell.drive_value = Some((id, value));
            return Ok(());
        }
        // With the contact solver on, play the drive through in small steps from where it stands, pushing
        // free components of the set and stopping where something cannot give way.
        let continuing = matches!(self.cell.drive_value, Some((d, _)) if d == id);
        let start = match self.cell.drive_value {
            Some((d, v)) if d == id => v,
            _ => self.doc.drive_start(id).unwrap_or(0.0),
        };
        if !continuing {
            self.cell.contact_pushed.clear();
        }
        let angular = matches!(
            self.doc.get(id).map(|r| r.kind),
            Some(gc_doc::RelationKind::Rotational { .. } | gc_doc::RelationKind::Angle { .. })
        );
        let step = if angular { 1f64.to_radians() } else { 0.002 };
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "clamped small count")]
        let steps = ((value - start).abs() / step).ceil().clamp(1.0, 500.0) as usize;
        let mut pushed = self.cell.contact_pushed.clone();
        let mut good: Option<(f64, Poses, PoseMap)> = None;
        for k in 1..=steps {
            #[allow(clippy::cast_precision_loss, reason = "at most 500 steps")]
            let v = start + (value - start) * (k as f64 / steps as f64);
            let moves = match self.doc.driven(id, v) {
                Ok(moves) => moves,
                Err(e) if good.is_none() => return Err(e),
                Err(_) => break,
            };
            match self.resolve_contacts(&participants, &moves, &mut pushed) {
                Ok(()) => good = Some((v, moves, pushed.clone())),
                Err((first, second)) => {
                    let at = good.as_ref().map_or(start, |g| g.0);
                    self.cell.contact_stop = Some(ContactStop { value: Some(at), first, second });
                    break;
                }
            }
        }
        if let Some((v, moves, pushed)) = good {
            let mut shown: BTreeMap<gc_core::AnyId, DPose3> = pushed.clone();
            shown.extend(moves);
            self.show_relation_drive(shown.into_iter().collect());
            self.cell.drive_value = Some((id, v));
            self.cell.contact_pushed = pushed;
        } else if !continuing {
            self.cell.drive_value = Some((id, start));
        }
        Ok(())
    }

    pub(crate) fn show_relation_drive(&mut self, moves: Vec<(gc_core::AnyId, DPose3)>) {
        self.cell.relation_drive = moves
            .into_iter()
            .map(|(item, pose)| {
                let placement = Placement {
                    xyz: pose.translation.to_array(),
                    rpy: gc_math::rpy_from_quat(pose.rotation),
                };
                (item, placement)
            })
            .collect();
        self.cell.driven_bodies = self
            .cell
            .relation_drive
            .iter()
            .filter(|(item, _)| item.kind == gc_core::Kind::Body)
            .filter_map(|(item, placement)| {
                let id = Id::<Body>::from_raw(item.raw);
                let mut body = self.doc.get(id)?.clone();
                body.placement = *placement;
                Some((id, body))
            })
            .collect();
    }

    /// Where the contact solver stopped the last drive or drag step: the value reached (for a drive) and
    /// the two components that met. `None` when nothing stopped it.
    pub fn contact_stop(&self) -> Option<ContactStop> {
        self.cell.contact_stop
    }

    /// The components the contact solver has pushed during the current drive or drag, where they stand.
    pub fn contact_pushed(&self) -> Vec<(gc_core::AnyId, Placement)> {
        self.cell
            .contact_pushed
            .iter()
            .map(|(item, pose)| {
                (
                    *item,
                    Placement {
                        xyz: pose.translation.to_array(),
                        rpy: gc_math::rpy_from_quat(pose.rotation),
                    },
                )
            })
            .collect()
    }

    /// Collision Detection for Drive (Inventor 2026 help, About the Contact Solver and Component
    /// Collision, GUID-0BBD4ACE-7BD1-40BF-A011-74416B229035: separate from the contact solver; a collision
    /// stops the motion): the first pair where a component Drive moved now overlaps another component
    /// deeper than they did where the document has them (touching is not a collision). Hidden, disabled and
    /// suppressed components take no part. `None` when nothing collides.
    pub fn drive_collision(&self) -> Option<(gc_core::AnyId, gc_core::AnyId)> {
        const TOLERANCE: f64 = 1e-6;
        let doc = &self.doc;
        let analysed = |item: gc_core::AnyId| {
            let o = gc_doc::Occurrence::Item(item);
            doc.component_flag(o, gc_doc::ComponentFlag::Visible)
                && doc.component_flag(o, gc_doc::ComponentFlag::Enabled)
                && !doc.is_component_suppressed(item)
        };
        let all: Vec<gc_core::AnyId> = doc
            .bodies()
            .iter()
            .map(|(id, _)| id.erase())
            .chain(doc.robots().iter().map(|(id, _)| id.erase()))
            .chain(doc.assemblies().iter().map(|(id, _)| id.erase()))
            .filter(|item| analysed(*item))
            .collect();
        let stored =
            |item: gc_core::AnyId| doc.component_origin(gc_doc::Occurrence::Item(item)).map(|o| o.pose);
        let shown =
            |item: gc_core::AnyId| self.cell.relation_drive.get(&item).map(pose_of).or_else(|| stored(item));
        for (a, placement) in &self.cell.relation_drive {
            let pose_a = pose_of(placement);
            let moved = stored(*a).is_some_and(|was| {
                (was.translation - pose_a.translation).length() > 1e-9
                    || was.rotation.angle_between(pose_a.rotation) > 1e-9
            });
            if !moved || !analysed(*a) {
                continue;
            }
            let Some(geometry_a) = self.contact_geometry(*a, &pose_a) else { continue };
            for b in all.iter().copied().filter(|b| b != a) {
                let Some(geometry_b) = shown(b).and_then(|p| self.contact_geometry(b, &p)) else { continue };
                let Ok(Some(hit)) = geometry_a.penetration(&geometry_b, TOLERANCE) else { continue };
                if hit.depth > self.stored_depth(*a, b) + TOLERANCE {
                    return Some((*a, b));
                }
            }
        }
        None
    }

    /// The name of a body, robot or assembly.
    pub fn component_name(&self, item: gc_core::AnyId) -> Option<String> {
        match item.kind {
            gc_core::Kind::Body => self.doc.get(Id::<Body>::from_raw(item.raw)).map(|b| b.name.clone()),
            gc_core::Kind::RobotInstance => {
                self.doc.get(Id::<Robot>::from_raw(item.raw)).map(|r| r.name.clone())
            }
            gc_core::Kind::Assembly => {
                self.doc.get(Id::<gc_doc::Assembly>::from_raw(item.raw)).map(|a| a.name.clone())
            }
            _ => None,
        }
    }

    /// How deep two components overlap where the document has them (0 when apart or touching).
    fn stored_depth(&self, a: gc_core::AnyId, b: gc_core::AnyId) -> f64 {
        let stored =
            |item: gc_core::AnyId| self.doc.component_origin(gc_doc::Occurrence::Item(item)).map(|o| o.pose);
        let (Some(pa), Some(pb)) = (stored(a), stored(b)) else { return 0.0 };
        let (Some(ga), Some(gb)) = (self.contact_geometry(a, &pa), self.contact_geometry(b, &pb)) else {
            return 0.0;
        };
        ga.penetration(&gb, 0.0).ok().flatten().map_or(0.0, |p| p.depth)
    }

    /// A component's shapes at a pose, for the contact solver: bodies and imported assemblies (their rigid
    /// parts as shown) at the pose, robots as shown (they are obstacles, never pushed).
    fn contact_geometry(&self, item: gc_core::AnyId, pose: &DPose3) -> Option<gc_collide::Geometry> {
        match item.kind {
            gc_core::Kind::Body => {
                let body = self.doc.get(Id::<Body>::from_raw(item.raw))?;
                let shape = shape_of(&body.solid, &self.cell.meshes)?;
                gc_collide::Geometry::from_shapes(&[(*pose, shape)]).ok()
            }
            gc_core::Kind::RobotInstance => self.item_geometry(CellItem::Robot(Id::from_raw(item.raw))),
            gc_core::Kind::Assembly => {
                let id = Id::<gc_doc::Assembly>::from_raw(item.raw);
                let assembly = self.doc.get(id)?;
                let meshes = self.cell.assemblies.of(assembly).ok()?;
                let keep = self.doc.parts_shown(id, true);
                let shapes: Vec<(DPose3, ShapeDesc)> =
                    crate::assembly::shown_parts(assembly, pose, meshes, &keep)
                        .into_iter()
                        .filter_map(|part| {
                            // Parts with scale in their matrix are left out (their triangles would have to be
                            // transformed for every query).
                            let at = crate::assembly::rigid(&part.matrix)?;
                            Some((at, ShapeDesc::Mesh(Arc::clone(&part.mesh.data))))
                        })
                        .collect();
                if shapes.is_empty() { None } else { gc_collide::Geometry::from_shapes(&shapes).ok() }
            }
            _ => None,
        }
    }

    /// True when contact may push a component: a body or assembly that is not grounded and that no active
    /// relationship holds.
    fn pushable(&self, item: gc_core::AnyId) -> bool {
        matches!(item.kind, gc_core::Kind::Body | gc_core::Kind::Assembly)
            && !self.doc.component_flag(gc_doc::Occurrence::Item(item), gc_doc::ComponentFlag::Grounded)
            && !self.doc.relationships().iter().any(|(id, r)| {
                !self.doc.is_inactive(id.erase())
                    && (r.first.occurrence.owner() == item
                        || r.second.occurrence.owner() == item
                        || r.third().is_some_and(|t| t.occurrence.owner() == item))
            })
    }

    /// Settles contact among the participants with `moves` imposed: overlapped free components are
    /// pushed out (added to `pushed`); `Err` with the pair when something that cannot give way is hit.
    fn resolve_contacts(
        &self,
        participants: &[gc_core::AnyId],
        moves: &[(gc_core::AnyId, DPose3)],
        pushed: &mut BTreeMap<gc_core::AnyId, DPose3>,
    ) -> Result<(), (gc_core::AnyId, gc_core::AnyId)> {
        const TOLERANCE: f64 = 1e-6;
        let stored =
            |item: gc_core::AnyId| self.doc.component_origin(gc_doc::Occurrence::Item(item)).map(|o| o.pose);
        // Only what really moves counts: the solver can report components it touched by a rounding error.
        let imposed: BTreeMap<gc_core::AnyId, DPose3> = moves
            .iter()
            .copied()
            .filter(|(item, pose)| {
                stored(*item).is_none_or(|was| {
                    (was.translation - pose.translation).length() > 1e-9
                        || was.rotation.angle_between(pose.rotation) > 1e-9
                })
            })
            .collect();
        let pose = |item: gc_core::AnyId, pushed: &BTreeMap<gc_core::AnyId, DPose3>| {
            imposed.get(&item).or_else(|| pushed.get(&item)).copied().or_else(|| stored(item))
        };
        let mut last = None;
        for _round in 0..32 {
            let mut changed = false;
            let moving: Vec<gc_core::AnyId> = participants
                .iter()
                .copied()
                .filter(|i| imposed.contains_key(i) || pushed.contains_key(i))
                .collect();
            for a in moving {
                let Some(geometry_a) = pose(a, pushed).and_then(|p| self.contact_geometry(a, &p)) else {
                    continue;
                };
                for b in participants.iter().copied().filter(|b| *b != a) {
                    let Some(pose_b) = pose(b, pushed) else { continue };
                    let Some(geometry_b) = self.contact_geometry(b, &pose_b) else { continue };
                    let Ok(Some(hit)) = geometry_a.penetration(&geometry_b, TOLERANCE) else { continue };
                    if hit.depth <= self.stored_depth(a, b) + TOLERANCE {
                        continue;
                    }
                    if imposed.contains_key(&b) || !self.pushable(b) {
                        return Err((a, b));
                    }
                    let shift = DVec3::from_array(hit.normal) * (hit.depth + TOLERANCE);
                    pushed.insert(b, DPose3::from_parts(pose_b.translation + shift, pose_b.rotation));
                    last = Some((a, b));
                    changed = true;
                }
            }
            if !changed {
                return Ok(());
            }
        }
        // Pushing went round without settling: treat as stuck.
        last.map_or(Ok(()), Err)
    }

    /// Ends Drive for a relationship: the components are shown as the document has them again.
    pub fn end_drive_relationship(&mut self) {
        self.cell.relation_drive.clear();
        self.cell.driven_bodies.clear();
        self.cell.drive_value = None;
        self.cell.contact_pushed.clear();
        self.cell.contact_stop = None;
    }

    /// Where Drive shows a component (body, robot or assembly) right now, if it moves it.
    pub fn driven_placement(&self, item: gc_core::AnyId) -> Option<Placement> {
        self.cell.relation_drive.get(&item).copied()
    }

    /// Previews a placement of a robot's base without touching the document.
    pub fn preview_base(&mut self, robot: Id<Robot>, placement: Placement) {
        if self.doc.get(robot).is_some() {
            self.cell.playback = None;
            self.cell.base = Some((robot, placement));
        }
    }

    /// Commits the base preview as one undo step.
    pub fn end_base_preview(&mut self) {
        if let Some((id, placement)) = self.cell.base.take()
            && self.doc.get(id).is_some_and(|r| r.placement != placement)
        {
            self.last_error = self.cell_op(CellOp::PlaceRobot { id, placement }).err();
        }
    }

    /// What is wrong with the robot as it is shown right now (jog preview, base and body previews
    /// included): a joint beyond its limit, a collision, or a part closer to a body than the padding.
    /// `None` when it is fine.
    pub fn robot_problem(&self, robot: Id<Robot>) -> Option<StateProblem> {
        let joints = self.robot_joints(robot)?;
        self.problem_at(robot, &joints, true)
    }

    /// What is wrong with a robot, as it is placed and among the bodies as they are shown, at these
    /// joint values. `remember` keeps the answer for the next identical question.
    fn problem_at(&self, robot: Id<Robot>, joints: &[f64], remember: bool) -> Option<StateProblem> {
        let loaded = self.robot_model(robot)?;
        let stored = self.doc.get(robot)?;
        let base = self.shown_base(robot)?;
        let mut obstacles = Vec::new();
        let tool = self.shown_tool(robot)?;
        let mut built_from =
            format!("{:?} {} {base:?} {tool:?} {:?}", robot, stored.urdf.len(), stored.acm_overrides);
        for (id, _) in
            self.doc.bodies().iter().filter(|(id, _)| !self.doc.is_component_suppressed(id.erase()))
        {
            let body = self.shown_body(id)?;
            // A body whose mesh is not there cannot be checked; it is left out here and stops planning.
            let Some(shape) = shape_of(&body.solid, &self.cell.meshes) else { continue };
            built_from.push_str(&format!(" {:?} {:?}", body.placement, body.solid));
            obstacles.push(Obstacle { name: body.name.clone(), pose: pose_of(&body.placement), shape });
        }
        // Imported equipment that is computed (and loaded) is in the way too.
        if let Ok(equipment) = crate::assembly::obstacles(&self.doc, &self.cell.assemblies, &pose_of) {
            built_from.push_str(&format!(
                " {:?}",
                crate::assembly::active(&self.doc).map(|(id, a)| (id, &a.placement)).collect::<Vec<_>>()
            ));
            obstacles.extend(equipment);
        }
        let question = format!("{built_from} {joints:?}");
        if let Some((asked, answer)) = self.cell.asked.borrow().as_ref()
            && *asked == question
        {
            return answer.clone();
        }
        let mut scene = self.cell.scene.borrow_mut();
        if scene.as_ref().is_none_or(|(from, _)| *from != built_from) {
            let (built, _) = RobotScene::new(
                Arc::clone(&loaded.model),
                loaded.chain.clone(),
                &stored.tip_link,
                pose_of(&base),
                &obstacles,
                PADDING,
                &|uri| loaded.file(uri),
                &tool_shapes(tool.as_ref(), &self.cell.meshes).unwrap_or_default(),
            )
            .ok()?;
            let built = built.with_pair_choices(&pair_choices(stored));
            *scene = Some((built_from, built));
        }
        let answer = scene.as_ref().and_then(|(_, scene)| scene.state_problem(joints));
        if remember {
            *self.cell.asked.borrow_mut() = Some((question, answer.clone()));
        }
        answer
    }

    /// How the robot of a waypoint would stand there: the joint values nearest to how it is shown now,
    /// and what would be wrong in that pose. Follows previews, so a waypoint being dragged shows where
    /// the arm would go.
    pub fn waypoint_ghost(&self, waypoint: Id<Waypoint>) -> Option<Ghost> {
        let robot = self.doc.get(waypoint)?.robot;
        let loaded = self.robot_model(robot)?;
        let seed = self.robot_joints(robot)?;
        let target = self.shown_target(waypoint)?;
        let world = self.waypoint_pose(waypoint)?;
        let base = self.shown_base(robot)?;
        let tcp = self.shown_tcp(robot)?;
        let bodies: Vec<_> = self.doc.bodies().iter().filter_map(|(id, _)| self.shown_body(id)).collect();
        let tool = self.shown_tool(robot)?;
        let question =
            format!("{robot:?} {target:?} {world:?} {base:?} {tcp:?} {tool:?} {seed:?} {bodies:?}");
        if let Some((asked, ghost)) = self.cell.ghost.borrow().as_ref()
            && *asked == question
        {
            return Some(ghost.clone());
        }
        let candidates = match &target {
            WaypointTarget::Joints(q) => vec![q.clone()],
            // Every arm configuration from the closed form when there is one; otherwise numerical
            // solutions from many starts.
            _ => {
                let flange = pose_of(&base).inverse() * world * pose_of(&tcp).inverse();
                if loaded.analytic.is_some() {
                    analytic_goals(loaded, &flange, &seed)
                } else {
                    let ik = gc_robot::NumericIk::new(&loaded.chain, gc_robot::IkOptions::default());
                    let mut found = ik.solve_many(&flange, Some(&seed), GHOST_STARTS);
                    let distance = |q: &Vec<f64>| {
                        q.iter().zip(&seed).map(|(a, b)| (a - b).abs()).fold(0.0_f64, f64::max)
                    };
                    found.sort_by(|a, b| distance(a).total_cmp(&distance(b)));
                    found
                }
            }
        };
        let solutions = candidates.len();
        // The nearest pose without a problem; failing that, the nearest with what is wrong there.
        let analytic = loaded.analytic.as_ref().map(|a| a.family.clone());
        let mut ghost = Ghost { robot, joints: None, problem: None, solutions, analytic: analytic.clone() };
        for q in candidates {
            let problem = self.problem_at(robot, &q, false);
            let fine = problem.is_none();
            if ghost.joints.is_none() || fine {
                ghost = Ghost { robot, joints: Some(q), problem, solutions, analytic: analytic.clone() };
            }
            if fine {
                break;
            }
        }
        *self.cell.ghost.borrow_mut() = Some((question, ghost.clone()));
        Some(ghost)
    }

    /// The ghost to show: the hovered waypoint's, else the selected waypoint's.
    pub fn shown_ghost(&self) -> Option<Ghost> {
        [self.cell.hovered, self.cell.selected].into_iter().flatten().find_map(|item| match item {
            CellItem::Waypoint(id) => self.waypoint_ghost(id),
            _ => None,
        })
    }

    /// World poses of all links of a robot at a ghost's joint values; `None` when it cannot reach.
    pub fn ghost_links(&self, ghost: &Ghost) -> Option<Vec<DPose3>> {
        let loaded = self.robot_model(ghost.robot)?;
        let base = pose_of(&self.shown_base(ghost.robot)?);
        let mut poses = Vec::new();
        loaded.model.link_poses(&loaded.chain.expand(ghost.joints.as_ref()?), &mut poses);
        Some(poses.iter().map(|pose| base * *pose).collect())
    }

    /// A robot's tool centre point as it is shown (in the tip link's frame): the preview while it is
    /// being edited, else the document's.
    pub fn shown_tcp(&self, robot: Id<Robot>) -> Option<Placement> {
        match &self.cell.tcp {
            Some((id, preview)) if *id == robot => Some(*preview),
            _ => self.doc.get(robot).map(|r| r.tcp),
        }
    }

    /// Previews a tool centre point without touching the document.
    pub fn preview_tcp(&mut self, robot: Id<Robot>, tcp: Placement) {
        if self.doc.get(robot).is_some() {
            self.cell.tcp = Some((robot, tcp));
        }
    }

    /// Commits the tool centre point preview as one undo step.
    pub fn end_tcp_preview(&mut self) {
        if let Some((id, tcp)) = self.cell.tcp.take()
            && self.doc.get(id).is_some_and(|r| r.tcp != tcp)
        {
            self.last_error = self.cell_op(CellOp::SetTcp { id, tcp }).err();
        }
    }

    /// What a robot carries as it is shown: the preview while it is being edited, else the document's.
    /// `None` when the robot does not exist; `Some(None)` when it carries nothing.
    pub fn shown_tool(&self, robot: Id<Robot>) -> Option<Option<Tool>> {
        match &self.cell.tool {
            Some((id, preview)) if *id == robot => Some(preview.clone()),
            _ => self.doc.get(robot).map(|r| r.tool.as_deref().cloned()),
        }
    }

    /// Previews a robot's tool without touching the document. An invalid tool is ignored.
    pub fn preview_tool(&mut self, robot: Id<Robot>, tool: Option<Tool>) {
        if self.doc.get(robot).is_some() && tool.as_ref().is_none_or(Tool::is_valid) {
            if let Some(tool) = &tool {
                self.cell.read_body_mesh(&tool.solid);
            }
            self.cell.tool = Some((robot, tool));
        }
    }

    /// Commits the tool preview as one undo step.
    pub fn end_tool_preview(&mut self) {
        if let Some((id, tool)) = self.cell.tool.take()
            && self.doc.get(id).is_some_and(|r| r.tool.as_deref() != tool.as_ref())
        {
            self.last_error = self.cell_op(CellOp::SetTool { id, tool }).err();
        }
    }

    /// The world pose of a robot's tool centre point as it is shown.
    pub fn tcp_pose(&self, robot: Id<Robot>) -> Option<DPose3> {
        let loaded = self.robot_model(robot)?;
        let tip = loaded.model.link(&self.doc.get(robot)?.tip_link)?;
        Some(*self.link_poses(robot)?.get(tip)? * pose_of(&self.shown_tcp(robot)?))
    }

    /// World poses of all links of a robot as it is shown.
    /// `None` for a suppressed robot: it is neither drawn nor planned for.
    pub fn link_poses(&self, robot: Id<Robot>) -> Option<Vec<DPose3>> {
        if self.doc.is_component_suppressed(robot.erase()) {
            return None;
        }
        let loaded = self.robot_model(robot)?;
        let base = pose_of(&self.shown_base(robot)?);
        let mut poses = Vec::new();
        loaded.model.link_poses(&loaded.chain.expand(&self.robot_joints(robot)?), &mut poses);
        Some(poses.iter().map(|pose| base * *pose).collect())
    }

    /// Makes a fully constrained sketch point the origin of a work frame, with the sketch's axes.
    /// Waypoints defined in the frame move with the point when the sketch changes.
    pub fn add_work_frame(
        &mut self,
        sketch: Id<Sketch>,
        point: Id<SkPoint>,
    ) -> Result<Id<WorkFrame>, EditError> {
        self.add_work_frame_by_points(sketch, &[point])
    }

    /// Makes a work frame from one to three fully constrained points of a sketch, in this order: the
    /// origin, the point the X axis points to, a point on the positive-Y side of the XY plane.
    pub fn add_work_frame_by_points(
        &mut self,
        sketch: Id<Sketch>,
        points: &[Id<SkPoint>],
    ) -> Result<Id<WorkFrame>, EditError> {
        let (Some(&point), true) = (points.first(), points.len() <= 3) else {
            return Err(EditError::BadValue);
        };
        let name = format!("Frame{}", self.doc.work_frames().len() + 1);
        let frame = WorkFrame {
            name,
            sketch,
            point,
            x_point: points.get(1).copied(),
            xy_point: points.get(2).copied(),
            rpy: [0.0; 3],
        };
        if let Err(error) = self.check_frame(&frame) {
            self.last_error = Some(error.clone());
            return Err(error);
        }
        match self.cell_op(CellOp::AddFrame(frame))?.first() {
            Some(Created::Frame(id)) => Ok(*id),
            _ => Err(EditError::BadValue),
        }
    }

    /// Why a frame cannot be used: a point that can still move, or axis points that give no frame.
    fn check_frame(&self, frame: &WorkFrame) -> Result<(), EditError> {
        let stored = self.doc.get(frame.sketch).ok_or(EditError::BadValue)?;
        // The report of the sketch as stored says which points can still move.
        let report = stored.clone().solve().map_err(|_unsolved| EditError::Underconstrained)?;
        let points = [Some(frame.point), frame.x_point, frame.xy_point];
        if points.iter().flatten().any(|p| report.point_is_free(*p)) {
            return Err(EditError::Underconstrained);
        }
        let distinct = frame.x_point != Some(frame.point)
            && frame.xy_point != Some(frame.point)
            && (frame.xy_point.is_none() || frame.xy_point != frame.x_point);
        if !distinct || frame_pose_of(&self.doc, frame).is_none() {
            return Err(EditError::DegenerateFrame);
        }
        Ok(())
    }

    /// Changes the points that set a work frame's axes (`None` for the sketch's own axes).
    pub fn set_frame_axes(
        &mut self,
        id: Id<WorkFrame>,
        x_point: Option<Id<SkPoint>>,
        xy_point: Option<Id<SkPoint>>,
    ) -> Result<(), EditError> {
        let mut frame = self.doc.get(id).ok_or(EditError::BadValue)?.clone();
        frame.x_point = x_point;
        frame.xy_point = xy_point.filter(|_| x_point.is_some());
        if let Err(error) = self.check_frame(&frame) {
            self.last_error = Some(error.clone());
            return Err(error);
        }
        self.cell_op(CellOp::SetFrame { id, frame }).map(|_| ())
    }

    /// The world pose of a work frame as it is shown (with the preview of its angles).
    pub fn work_frame_pose(&self, frame: Id<WorkFrame>) -> Option<DPose3> {
        match &self.cell.frame {
            Some((id, preview)) if *id == frame => frame_pose_of(&self.doc, preview),
            _ => work_frame_pose(&self.doc, frame),
        }
    }

    /// The world pose a work frame would have with these settings (e.g. with its angles set to zero).
    pub fn frame_pose_with(&self, frame: &WorkFrame) -> Option<DPose3> {
        frame_pose_of(&self.doc, frame)
    }

    /// A work frame as it is shown: the preview while it is being edited, else the document's.
    pub fn shown_frame(&self, frame: Id<WorkFrame>) -> Option<WorkFrame> {
        match &self.cell.frame {
            Some((id, preview)) if *id == frame => Some(preview.clone()),
            _ => self.doc.get(frame).cloned(),
        }
    }

    /// Previews a change of a work frame without touching the document.
    pub fn preview_frame(&mut self, id: Id<WorkFrame>, frame: WorkFrame) {
        if self.doc.get(id).is_some() {
            self.cell.frame = Some((id, frame));
        }
    }

    /// Commits the work frame preview as one undo step.
    pub fn end_frame_preview(&mut self) {
        if let Some((id, frame)) = self.cell.frame.take()
            && self.doc.get(id).is_some_and(|f| *f != frame)
        {
            self.last_error = self.cell_op(CellOp::SetFrame { id, frame }).err();
        }
    }

    /// Adds a waypoint in a work frame where the robot's tool centre point is now (teaching): it keeps
    /// its place relative to the frame when the frame moves.
    pub fn add_waypoint_in_frame(
        &mut self,
        robot: Id<Robot>,
        frame: Id<WorkFrame>,
    ) -> Result<Id<Waypoint>, EditError> {
        let tool = self.tcp_pose(robot).ok_or(EditError::BadValue)?;
        let origin = self.work_frame_pose(frame).ok_or(EditError::BadValue)?;
        let local = origin.inverse() * tool;
        let offset = Placement { xyz: local.translation.to_array(), rpy: rpy_from_quat(local.rotation) };
        self.add_waypoint(robot, WaypointTarget::InFrame { frame, offset })
    }

    /// A waypoint's target as it is shown: the preview while it is being edited, else the document's.
    pub fn shown_target(&self, waypoint: Id<Waypoint>) -> Option<WaypointTarget> {
        match &self.cell.waypoint {
            Some((id, target)) if *id == waypoint => Some(target.clone()),
            _ => self.doc.get(waypoint).map(|w| w.target.clone()),
        }
    }

    /// Previews a new target of a waypoint without touching the document.
    pub fn preview_target(&mut self, waypoint: Id<Waypoint>, target: WaypointTarget) {
        if self.doc.get(waypoint).is_some() {
            self.cell.waypoint = Some((waypoint, target));
        }
    }

    /// Commits the waypoint preview as one undo step.
    pub fn end_target_preview(&mut self) {
        if let Some((id, target)) = self.cell.waypoint.take()
            && self.doc.get(id).is_some_and(|w| w.target != target)
        {
            self.last_error = self.cell_op(CellOp::SetWaypointTarget { id, target }).err();
        }
    }

    /// The world pose of a robot's base frame as it is shown: its `base` link when it has one (as on a UR
    /// teach pendant), else its root link.
    pub fn robot_base_frame(&self, robot: Id<Robot>) -> Option<DPose3> {
        Some(base_frame(self.robot_model(robot)?, &self.shown_base(robot)?))
    }

    /// The world pose of a reference frame, for the given robot's base.
    pub fn reference_pose(&self, robot: Id<Robot>, reference: Reference) -> Option<DPose3> {
        match reference {
            Reference::World => Some(DPose3::IDENTITY),
            Reference::Base => self.robot_base_frame(robot),
            Reference::Frame(frame) => self.work_frame_pose(frame),
        }
    }

    /// What a waypoint's numbers are measured from; `None` for joint values and sketch points.
    pub fn waypoint_reference(&self, waypoint: Id<Waypoint>) -> Option<Reference> {
        match self.shown_target(waypoint)? {
            WaypointTarget::InWorld { .. } => Some(Reference::World),
            WaypointTarget::InBase { .. } => Some(Reference::Base),
            WaypointTarget::InFrame { frame, .. } => Some(Reference::Frame(frame)),
            WaypointTarget::Joints(_) | WaypointTarget::SketchPoint { .. } => None,
        }
    }

    /// The target that puts a waypoint at `world` measured from `reference`.
    pub fn target_at(
        &self,
        waypoint: Id<Waypoint>,
        reference: Reference,
        world: DPose3,
    ) -> Option<WaypointTarget> {
        let robot = self.doc.get(waypoint)?.robot;
        let offset = placement_of(&(self.reference_pose(robot, reference)?.inverse() * world));
        Some(match reference {
            Reference::World => WaypointTarget::InWorld { offset },
            Reference::Base => WaypointTarget::InBase { offset },
            Reference::Frame(frame) => WaypointTarget::InFrame { frame, offset },
        })
    }

    /// Measures a waypoint from another reference without moving it: its numbers change, its place does
    /// not. A waypoint given by joint values or a sketch point becomes a pose (and no longer follows the
    /// sketch). One undo step.
    pub fn express_waypoint(
        &mut self,
        waypoint: Id<Waypoint>,
        reference: Reference,
    ) -> Result<(), EditError> {
        let world = self.waypoint_pose(waypoint).ok_or(EditError::BadValue)?;
        let target = self.target_at(waypoint, reference, world).ok_or(EditError::BadValue)?;
        self.cell_op(CellOp::SetWaypointTarget { id: waypoint, target }).map(|_| ())
    }

    /// The world pose of the tool at a waypoint.
    pub fn waypoint_pose(&self, waypoint: Id<Waypoint>) -> Option<DPose3> {
        let target = self.shown_target(waypoint)?;
        let waypoint = self.doc.get(waypoint)?;
        match &target {
            WaypointTarget::SketchPoint { .. } | WaypointTarget::InWorld { .. } => {
                target_pose(&self.doc, &target, None)
            }
            WaypointTarget::InBase { offset } => {
                Some(self.robot_base_frame(waypoint.robot)? * pose_of(offset))
            }
            WaypointTarget::InFrame { frame, offset } => {
                Some(self.work_frame_pose(*frame)? * pose_of(offset))
            }
            WaypointTarget::Joints(q) => {
                let robot = self.doc.get(waypoint.robot)?;
                let loaded = self.robot_model(waypoint.robot)?;
                let mut poses = Vec::new();
                loaded.model.link_poses(&loaded.chain.expand(q), &mut poses);
                let tip = poses.get(loaded.model.link(&robot.tip_link)?)?;
                Some(pose_of(&robot.placement) * *tip * pose_of(&robot.tcp))
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
        let clamped =
            joints.iter().zip(limits).map(|(q, (lo, hi))| gc_math::clamp_between(*q, *lo, *hi)).collect();
        self.cell.playback = None;
        self.cell.jog = Some((robot, clamped));
    }

    /// Shows a robot at joint values without changing the document (Drive): the values are clamped to
    /// the limits, and the pose stays until [`Self::end_drive`].
    pub fn drive_show(&mut self, robot: Id<Robot>, joints: &[f64]) {
        let Some(loaded) = self.robot_model(robot) else { return };
        if joints.len() != loaded.chain.dof() {
            return;
        }
        let limits = loaded.chain.lower().iter().zip(loaded.chain.upper());
        let clamped =
            joints.iter().zip(limits).map(|(q, (lo, hi))| gc_math::clamp_between(*q, *lo, *hi)).collect();
        self.cell.playback = None;
        self.cell.drive = Some((robot, clamped));
    }

    /// Ends Drive: the robot is shown as the document has it again.
    pub fn end_drive(&mut self) {
        self.cell.drive = None;
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

    /// Every pair of the robot's links with collision geometry: whether the description excludes it from
    /// collision checking (and why) and whether it is checked now, after the user's choices.
    pub fn link_pairs(&self, robot: Id<Robot>) -> Option<Vec<LinkPair>> {
        let stored = self.doc.get(robot)?;
        let loaded = self.robot_model(robot)?;
        let (scene, _) = RobotScene::new(
            Arc::clone(&loaded.model),
            loaded.chain.clone(),
            &stored.tip_link,
            gc_math::DPose3::IDENTITY,
            &[],
            PADDING,
            &|uri| loaded.file(uri),
            &tool_shapes(stored.tool.as_deref(), &self.cell.meshes).unwrap_or_default(),
        )
        .ok()?;
        Some(scene.with_pair_choices(&pair_choices(stored)).link_pairs())
    }

    /// Checks a pair of the robot's links for collisions, or stops checking it (one undo step). A choice
    /// equal to what the description implies removes the override.
    pub fn set_pair_checked(
        &mut self,
        robot: Id<Robot>,
        a: &str,
        b: &str,
        checked: bool,
    ) -> Result<(), EditError> {
        let pairs = self.link_pairs(robot).ok_or(EditError::BadValue)?;
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        let pair = pairs.iter().find(|p| p.a == a && p.b == b).ok_or(EditError::BadValue)?;
        let by_default = pair.reason == PairReason::Checked;
        let mut overrides = self.doc.get(robot).ok_or(EditError::BadValue)?.acm_overrides.clone();
        overrides.retain(|o| !((o.a == a && o.b == b) || (o.a == b && o.b == a)));
        if checked != by_default {
            overrides.push(gc_doc::AcmOverride { a: a.to_owned(), b: b.to_owned(), check: checked });
        }
        self.cell_op(CellOp::SetAcmOverrides { id: robot, overrides }).map(|_| ())
    }

    /// Lets a link of the path's robot come closer to an obstacle than the padding near the start and
    /// end of the motion into waypoint `index` (within 50 mm of them, no touching; one undo step). Edit
    /// [`gc_doc::ContactAllowance`]s with [`CellOp::SetContacts`] for the other options.
    pub fn allow_contact(
        &mut self,
        path: Id<MotionPath>,
        index: usize,
        link: &str,
        obstacle: Id<Body>,
    ) -> Result<(), EditError> {
        let mut contacts = self.doc.get(path).ok_or(EditError::BadValue)?.contacts.clone();
        if contacts.iter().any(|c| c.index == index && c.link == link && c.obstacle == obstacle) {
            return Ok(());
        }
        contacts.push(gc_doc::ContactAllowance {
            index,
            link: link.to_owned(),
            obstacle,
            whole_motion: false,
            radius: 0.05,
            allow_touch: false,
        });
        self.cell_op(CellOp::SetContacts { id: path, contacts }).map(|_| ())
    }

    /// Adds a path of `robot` through `waypoints`, planned with the default planner.
    pub fn add_path(
        &mut self,
        robot: Id<Robot>,
        waypoints: Vec<Id<Waypoint>>,
    ) -> Result<Id<MotionPath>, EditError> {
        let name = format!("Path{}", self.doc.paths().len() + 1);
        let path = MotionPath {
            name,
            robot,
            waypoints,
            linear: Vec::new(),
            planner: PLANNERS[0].0.to_owned(),
            seed: 1,
            tcp_speed: None,
            contacts: Vec::new(),
            follow: Vec::new(),
        };
        match self.cell_op(CellOp::AddPath(path))?.first() {
            Some(Created::Path(id)) => Ok(*id),
            _ => Err(EditError::BadValue),
        }
    }

    /// Makes a path of `robot` that follows the curves of a sketch: every chain of open curves (lines,
    /// arcs, elliptical arcs, open splines) joined end to end becomes a path role, unless it already is
    /// one, and the path follows them all in the order they were drawn, moving freely from the end of one
    /// chain to the start of the next. The path reads the curves when it is planned, so it follows when
    /// the sketch is edited. One undo step. Refused for a sketch without open curves.
    pub fn path_from_sketch(
        &mut self,
        robot: Id<Robot>,
        sketch_id: Id<Sketch>,
    ) -> Result<Id<MotionPath>, EditError> {
        let sketch = self.doc.get(sketch_id).ok_or(EditError::BadValue)?;
        // Chains in drawing order: each starts at its first-drawn curve.
        let mut seen: Vec<Id<gc_sketch::Curve>> = Vec::new();
        let mut follow = Vec::new();
        let mut roles = Vec::new();
        let mut count = self.doc.roles().len();
        for curve in sketch.curves.keys() {
            if seen.contains(curve) || sketch.curve_ends(*curve).is_none() {
                continue;
            }
            if let Some(role) = self.doc.path_role(sketch_id, *curve) {
                if let Some(gc_doc::RoleKind::Path { curves, .. }) = self.doc.get(role).map(|r| &r.kind) {
                    seen.extend(curves);
                }
                // Roles there already come before the new ones (the document adds new ones at the end).
                follow.push(role);
                continue;
            }
            let taken: Vec<Id<gc_sketch::Curve>> = sketch
                .curves
                .keys()
                .filter(|c| self.doc.path_role(sketch_id, **c).is_some())
                .copied()
                .collect();
            let mut chain = sketch.chain_at(*curve, &taken);
            // Travel it from the curve drawn first.
            if chain.last() == Some(curve) && chain.len() > 1 {
                chain.reverse();
            }
            seen.extend(&chain);
            count += 1;
            roles.push(gc_doc::SketchRole {
                name: format!("Route{count}"),
                sketch: sketch_id,
                kind: gc_doc::RoleKind::Path {
                    curves: chain,
                    reversed: false,
                    motion: gc_doc::PathMotion::Follow,
                    tcp_speed: None,
                },
                aim: gc_doc::ToolAim::default(),
                travel: gc_doc::PathTravel::default(),
            });
        }
        if follow.is_empty() && roles.is_empty() {
            return Err(EditError::BadValue);
        }
        let path = MotionPath {
            name: format!("Path{}", self.doc.paths().len() + 1),
            robot,
            waypoints: Vec::new(),
            linear: Vec::new(),
            planner: PLANNERS[0].0.to_owned(),
            seed: 1,
            tcp_speed: None,
            contacts: Vec::new(),
            follow,
        };
        match self.cell_op(CellOp::AddFollowPath { path, roles })?.last() {
            Some(Created::Path(id)) => Ok(*id),
            _ => Err(EditError::BadValue),
        }
    }

    /// Everything a plan of this path depends on, as text.
    fn plan_inputs(&self, path_id: Id<MotionPath>) -> Option<String> {
        let doc = &self.doc;
        let path = doc.get(path_id)?;
        let robot = doc.get(path.robot)?;
        let base = self.robot_model(path.robot).map(|loaded| base_frame(loaded, &robot.placement));
        let targets: Vec<String> = path
            .waypoints
            .iter()
            .map(|w| match doc.get(*w).map(|w| &w.target) {
                Some(WaypointTarget::Joints(q)) => format!("{q:?}"),
                Some(target) => format!("{:?}", target_pose(doc, target, base)),
                None => String::new(),
            })
            .collect();
        // What counts is how each waypoint is approached, not how the list happens to be stored.
        let linear: Vec<bool> =
            (0..path.waypoints.len()).map(|index| path.linear.get(index).copied().unwrap_or(false)).collect();
        let bodies: Vec<(&Placement, &Solid)> = doc
            .bodies()
            .iter()
            .filter(|(id, _)| !doc.is_component_suppressed(id.erase()))
            .map(|(_, b)| (&b.placement, &b.solid))
            .collect();
        Some(
            format!(
                "{} {:?} {} {} {:?} {:?} {:?} {} {bodies:?} {targets:?} {:?} {} {} {:?}",
                robot.urdf.len(),
                robot.tool,
                robot.base_link,
                robot.tip_link,
                robot.placement,
                robot.tcp,
                robot.joints,
                robot.max_acc,
                linear,
                path.planner,
                path.seed,
                path.tcp_speed,
            ) + &format!(
                " {:?} {:?} {:?}",
                robot.acm_overrides,
                path.contacts,
                crate::assembly::active(doc)
                    .map(|(id, a)| (id, &a.file, &a.placement, &a.parts))
                    .collect::<Vec<_>>()
            ) + &format!(
                " {:?} {:?}",
                doc.is_inactive(path_id.erase()),
                path.waypoints.iter().map(|w| doc.is_inactive(w.erase())).collect::<Vec<_>>()
            ) + &path
                .follow
                .iter()
                .map(|role| {
                    // The curves as they are now, how they are travelled and how the tool is held.
                    let travel = doc.get(*role).map(|r| match &r.kind {
                        gc_doc::RoleKind::Path { motion, tcp_speed, .. } => (Some(*motion), *tcp_speed),
                        gc_doc::RoleKind::Waypoint { .. } => (None, None),
                    });
                    format!(" {:?} {travel:?} {}", doc.role_poses(*role), doc.is_inactive(role.erase()))
                })
                .collect::<String>(),
        )
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
        self.end_base_preview();
        self.end_tcp_preview();
        self.end_target_preview();
        self.end_frame_preview();
        self.end_body_preview();
        self.cell.playback = None;
        self.cancel_planning();
        let Some(inputs) = self.plan_inputs(path) else { return PlanStatus::NotPlanned };
        // A suppressed robot is not planned for.
        if self.doc.get(path).is_some_and(|p| self.doc.is_component_suppressed(p.robot.erase())) {
            return PlanStatus::NotPlanned;
        }
        let loaded = self.doc.get(path).and_then(|p| self.robot_model(p.robot)).cloned();
        // The job works on its own copy of the document: edits made while it runs do not disturb it,
        // and the fingerprint tells afterwards whether its result is still current.
        let doc = self.doc.clone();
        let meshes = self.cell.meshes.clone();
        // The last good plan: a segment that is still valid for the new input is kept (plan 5.8). A new
        // planner or seed asks for a new plan, so the old one is not offered then.
        let strategy = self.doc.get(path).map(|p| (p.planner.clone(), p.seed));
        let previous = self.planned(path).filter(|old| Some(&old.strategy) == strategy.as_ref()).cloned();
        let planners = self.cell.planners.clone();
        let equipment = crate::assembly::obstacles(&self.doc, &self.cell.assemblies, &pose_of);
        let job = self.cell.executor.spawn("plan", move |ctx| {
            let control = JobControl(ctx.cancel.clone());
            let result = match &loaded {
                Some(loaded) => plan_path(
                    &doc,
                    loaded,
                    &meshes,
                    path,
                    None,
                    previous.as_ref(),
                    &planners,
                    &equipment,
                    &control,
                ),
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

    /// Runs every planner on a path with `seeds` different seeds each (one run per seed, no best-of) and
    /// stores how they did. Like planning, this is a job; see [`Session::comparison`].
    pub fn compare(&mut self, path: Id<MotionPath>, seeds: usize) {
        self.end_jog();
        self.cancel_comparison();
        let Some(inputs) = self.plan_inputs(path) else { return };
        let loaded = self.doc.get(path).and_then(|p| self.robot_model(p.robot)).cloned();
        let doc = self.doc.clone();
        let meshes = self.cell.meshes.clone();
        let equipment = crate::assembly::obstacles(&self.doc, &self.cell.assemblies, &pose_of);
        let job = self.cell.executor.spawn("compare", move |ctx| {
            let control = JobControl(ctx.cancel.clone());
            let loaded = loaded?;
            let mut rows = Vec::with_capacity(PLANNERS.len());
            let total = (PLANNERS.len() * seeds.max(1)) as f32;
            for (index, (planner, algorithm)) in PLANNERS.iter().enumerate() {
                let (mut durations, mut costs, mut checks) = (Vec::new(), Vec::new(), Vec::new());
                for seed in 0..seeds.max(1) {
                    let strategy = Some((*algorithm, seed as u64, 1));
                    if let Ok(planned) =
                        plan_path(&doc, &loaded, &meshes, path, strategy, None, &[], &equipment, &control)
                    {
                        durations.push(planned.duration());
                        costs.push(planned.segments.iter().map(|s| s.cost).sum());
                        checks.push(planned.state_checks() as f64);
                    }
                    if ctx.cancel.is_cancelled() {
                        return None;
                    }
                    ctx.progress.set((index * seeds.max(1) + seed + 1) as f32 / total);
                }
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "a count")]
                let state_checks = median(&mut checks).unwrap_or(0.0) as u64;
                rows.push(CompareRow {
                    planner,
                    runs: seeds.max(1),
                    successes: durations.len(),
                    duration: median(&mut durations),
                    cost: median(&mut costs),
                    state_checks,
                });
            }
            Some(rows)
        });
        self.cell.compare_job = Some((path, inputs, job));
        self.poll_jobs();
    }

    /// How far the running comparison is, from 0 to 1; `None` when none is running.
    pub fn comparison_progress(&self) -> Option<f32> {
        self.cell.compare_job.as_ref().map(|(_, _, job)| job.progress())
    }

    /// Stops the running comparison.
    pub fn cancel_comparison(&mut self) {
        if let Some((_, _, job)) = self.cell.compare_job.take() {
            job.cancel();
        }
    }

    /// The last comparison of a path and whether it still matches the document.
    pub fn comparison(&self, path: Id<MotionPath>) -> Option<(&[CompareRow], bool)> {
        let (inputs, rows) = self.cell.comparisons.get(&path)?;
        Some((rows, self.plan_inputs(path).as_ref() == Some(inputs)))
    }

    /// Collects finished background work. Returns true when something arrived. Call once per frame.
    pub fn poll_jobs(&mut self) -> bool {
        let mut arrived = self.cell.collect_meshes();
        if let Some((path, inputs, mut job)) = self.cell.compare_job.take() {
            match job.poll() {
                None => self.cell.compare_job = Some((path, inputs, job)),
                Some(JobOutcome::Done(Some(rows))) => {
                    self.cell.comparisons.insert(path, (inputs, rows));
                    arrived = true;
                }
                Some(JobOutcome::Done(None) | JobOutcome::Failed(_)) => arrived = true,
            }
        }
        if let Some(mut job) = self.cell.import_job.take() {
            match job.poll() {
                None => self.cell.import_job = Some(job),
                Some(JobOutcome::Done(Ok((assembly, meshes)))) => {
                    self.cell.import_result = Some(self.add_assembly_shared(assembly, meshes));
                    arrived = true;
                }
                Some(JobOutcome::Done(Err(reason)) | JobOutcome::Failed(reason)) => {
                    self.cell.import_result = Some(Err(reason));
                    arrived = true;
                }
            }
        }
        arrived | self.poll_schedule() | self.poll_plan()
    }

    fn poll_plan(&mut self) -> bool {
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

/// Plans a path of `doc` from its robot's current joint values through its waypoints, with the path's own
/// planner and seed or with the given algorithm, seed and number of runs.
/// A robot's choices for link pairs, as the scene takes them.
fn pair_choices(robot: &Robot) -> Vec<(String, String, bool)> {
    robot.acm_overrides.iter().map(|o| (o.a.clone(), o.b.clone(), o.check)).collect()
}

/// The contact allowances of the motion into waypoint `index` of a path; obstacles are found by name.
fn contacts_of(doc: &Document, path: &MotionPath, index: usize) -> Vec<gc_motion::ContactAllowance> {
    path.contacts
        .iter()
        .filter(|c| c.index == index)
        .filter_map(|c| {
            let obstacle = doc.get(c.obstacle)?;
            Some(gc_motion::ContactAllowance {
                body: c.link.clone(),
                obstacle: obstacle.name.clone(),
                scope: if c.whole_motion {
                    gc_motion::ContactScope::Segment
                } else {
                    gc_motion::ContactScope::Waypoint
                },
                radius: c.radius,
                allow_touch: c.allow_touch,
            })
        })
        .collect()
}

/// The built-in planners' ids with their names (Korean, English).
fn gc_plugin_planners_names() -> Vec<(String, String, String)> {
    let names = [
        ("RRT-Connect", "RRT-Connect"),
        ("RRT", "RRT"),
        ("RRT*", "RRT*"),
        ("Informed RRT*", "Informed RRT*"),
        ("PRM", "PRM"),
    ];
    PLANNERS
        .iter()
        .zip(names)
        .map(|((id, _), (ko, en))| ((*id).to_owned(), ko.to_owned(), en.to_owned()))
        .collect()
}

/// How the tool gets to the goal of a step.
enum Travel {
    /// Freely: the straight joint move, or the planner where that is blocked.
    Free,
    /// The tool tip in a straight line.
    Linear,
    /// The tool tip along a curve, through these poses.
    Along(Vec<DPose3>),
}

/// One motion of a path.
struct Step {
    /// What it is called in messages.
    name: String,
    goal: Goal,
    travel: Travel,
    tcp_speed: Option<f64>,
    contacts: Vec<gc_motion::ContactAllowance>,
}

/// The motions of a path, in order: one into each waypoint, or for a path that follows path roles of
/// sketch curves, a free move to the start of each role and then along it as the role says. Suppressed
/// waypoints and roles are skipped.
fn path_steps(
    doc: &Document,
    loaded: &LoadedRobot,
    path: &MotionPath,
    robot: &Robot,
) -> Result<Vec<Step>, String> {
    let mut steps = Vec::new();
    for (index, waypoint_id) in path.waypoints.iter().enumerate() {
        let waypoint = doc.get(*waypoint_id).ok_or("a waypoint no longer exists")?;
        // A suppressed waypoint is skipped: the motion goes on to the next one.
        if doc.is_inactive(waypoint_id.erase()) {
            continue;
        }
        let goal = waypoint_goal(doc, loaded, robot, waypoint)?;
        steps.push(Step {
            name: waypoint.name.clone(),
            goal,
            travel: if path.linear.get(index).copied().unwrap_or(false) {
                Travel::Linear
            } else {
                Travel::Free
            },
            tcp_speed: path.tcp_speed,
            contacts: contacts_of(doc, path, index),
        });
    }
    for (index, role_id) in path.follow.iter().enumerate() {
        if doc.is_inactive(role_id.erase()) {
            continue;
        }
        steps.extend(role_steps(doc, *role_id, path.tcp_speed, &contacts_of(doc, path, index))?);
    }
    Ok(steps)
}

/// The motions along a path role of sketch curves: a free move to its start, then along it as the role
/// says (one motion along the curves, or one per curve in straight lines or joint moves).
fn role_steps(
    doc: &Document,
    role_id: Id<gc_doc::SketchRole>,
    fallback_speed: Option<f64>,
    contacts: &[gc_motion::ContactAllowance],
) -> Result<Vec<Step>, String> {
    let mut steps = Vec::new();
    let role = doc.get(role_id).ok_or("a path role no longer exists")?;
    let gc_doc::RoleKind::Path { motion, tcp_speed, .. } = &role.kind else {
        return Err(format!("{} is not a path", role.name));
    };
    let gone = || format!("{}: its sketch curves no longer exist or no longer join end to end", role.name);
    let runs = doc.role_poses(role_id).ok_or_else(gone)?;
    let first = runs.first().and_then(|run| run.first()).ok_or_else(gone)?;
    let step = |name: String, goal: DPose3, travel: Travel| Step {
        name,
        goal: Goal::Pose(goal),
        travel,
        tcp_speed: tcp_speed.or(fallback_speed),
        contacts: contacts.to_vec(),
    };
    // Poses that differ: the tool has to move or turn to get from one to the other.
    let apart = |a: &DPose3, b: &DPose3| {
        (a.translation - b.translation).length() > 1e-9 || a.rotation.angle_between(b.rotation) > 1e-9
    };
    steps.push(step(format!("{} (start)", role.name), *first, Travel::Free));
    match motion {
        gc_doc::PathMotion::Follow => {
            // One motion along every curve. Where one curve hands over to the next the pose is there
            // once, unless the tool turns on the spot (a corner travelled tangentially).
            let mut along: Vec<DPose3> = Vec::new();
            let mut at = *first;
            for pose in runs.iter().flatten() {
                if apart(&at, pose) {
                    along.push(*pose);
                    at = *pose;
                }
            }
            let last = along.pop().ok_or_else(gone)?;
            steps.push(step(role.name.clone(), last, Travel::Along(along)));
        }
        gc_doc::PathMotion::Linear | gc_doc::PathMotion::Joint => {
            let mut at = *first;
            for (number, run) in runs.iter().enumerate() {
                let (start, end) = (run.first().ok_or_else(gone)?, run.last().ok_or_else(gone)?);
                let linear = *motion == gc_doc::PathMotion::Linear;
                // Travelling tangentially, the tool turns on the spot at a corner first.
                if linear && apart(&at, start) {
                    steps.push(step(format!("{} ({} turn)", role.name, number + 1), *start, Travel::Linear));
                }
                let travel = if linear { Travel::Linear } else { Travel::Free };
                steps.push(step(format!("{} ({})", role.name, number + 1), *end, travel));
                at = *end;
            }
        }
    }
    Ok(steps)
}

/// Where a robot's waypoint is, as a goal: joint values, or the world pose of the tool centre point.
fn waypoint_goal(
    doc: &Document,
    loaded: &LoadedRobot,
    robot: &Robot,
    waypoint: &Waypoint,
) -> Result<Goal, String> {
    Ok(match &waypoint.target {
        WaypointTarget::Joints(q) => Goal::Joints(q.clone()),
        target => Goal::Pose(
            target_pose(doc, target, Some(base_frame(loaded, &robot.placement)))
                .ok_or_else(|| format!("{}: its sketch point or frame no longer exists", waypoint.name))?,
        ),
    })
}

fn plan_path(
    doc: &Document,
    loaded: &LoadedRobot,
    meshes: &BTreeMap<String, MeshRef>,
    path_id: Id<MotionPath>,
    strategy: Option<(Algorithm, u64, usize)>,
    previous: Option<&PlannedPath>,
    registered: &[Arc<dyn gc_robotics_api::MotionPlanner>],
    assembly_obstacles: &Result<Vec<Obstacle>, String>,
    control: &dyn Control,
) -> Result<PlannedPath, String> {
    let path = doc.get(path_id).ok_or("the path no longer exists")?;
    if doc.is_inactive(path_id.erase()) {
        return Err(format!("{} is suppressed", path.name));
    }
    let robot = doc.get(path.robot).ok_or("the robot no longer exists")?;
    let mut plugin: Option<&dyn gc_robotics_api::MotionPlanner> = None;
    let (algorithm, seed, runs) = match strategy {
        Some(strategy) => strategy,
        None => {
            let builtin =
                PLANNERS.iter().find(|(id, _)| *id == path.planner).map(|(_, algorithm)| *algorithm);
            if builtin.is_none() {
                plugin = registered.iter().find(|p| p.descriptor().id == path.planner).map(|p| p.as_ref());
            }
            let algorithm = builtin
                .or(plugin.map(|_| Algorithm::RrtConnect))
                .ok_or_else(|| format!("unknown planner `{}`", path.planner))?;
            (algorithm, path.seed, 4)
        }
    };
    let scene = robot_scene(doc, loaded, meshes, robot, assembly_obstacles)?;
    let steps = path_steps(doc, loaded, path, robot)?;
    let how = (algorithm, seed, runs, plugin);
    let segments = run_steps(loaded, robot, &scene, steps, robot.joints.clone(), how, previous, control)?;
    Ok(PlannedPath { segments, strategy: (path.planner.clone(), seed) })
}

/// The planning scene of a robot: itself with its tool, and every obstacle of the cell.
pub(crate) fn robot_scene(
    doc: &Document,
    loaded: &LoadedRobot,
    meshes: &BTreeMap<String, MeshRef>,
    robot: &Robot,
    assembly_obstacles: &Result<Vec<Obstacle>, String>,
) -> Result<RobotScene, String> {
    let mut obstacles = Vec::new();
    for (_, body) in doc.bodies().iter().filter(|(id, _)| !doc.is_component_suppressed(id.erase())) {
        // An obstacle that cannot be checked must not be planned through as if it were not there.
        let shape = shape_of(&body.solid, meshes)
            .ok_or_else(|| format!("the mesh of `{}` is not available", body.name))?;
        obstacles.push(Obstacle { name: body.name.clone(), pose: pose_of(&body.placement), shape });
    }
    obstacles.extend(assembly_obstacles.clone()?);
    let tool = tool_shapes(robot.tool.as_deref(), meshes)
        .ok_or_else(|| format!("the mesh of the tool of `{}` is not available", robot.name))?;
    let (scene, _) = RobotScene::new(
        Arc::clone(&loaded.model),
        loaded.chain.clone(),
        &robot.tip_link,
        pose_of(&robot.placement),
        &obstacles,
        PADDING,
        &|uri| loaded.file(uri),
        &tool,
    )
    .map_err(|e| e.to_string())?;
    let scene = scene.with_tool(pose_of(&robot.tcp)).with_pair_choices(&pair_choices(robot));
    Ok(scene)
}

/// How motions are planned: the algorithm, its seed, how many seeds to run, and a registered planner used
/// in the algorithm's place.
type Strategy<'a> = (Algorithm, u64, usize, Option<&'a dyn gc_robotics_api::MotionPlanner>);

/// Plans steps one after the other, each starting where the one before ended.
#[allow(clippy::too_many_arguments, reason = "the pieces of one planning run")]
fn run_steps(
    loaded: &LoadedRobot,
    robot: &Robot,
    scene: &RobotScene,
    steps: Vec<Step>,
    mut start: Vec<f64>,
    (algorithm, seed, runs, plugin): Strategy<'_>,
    previous: Option<&PlannedPath>,
    control: &dyn Control,
) -> Result<Vec<MotionPlan>, String> {
    let mut segments = Vec::with_capacity(steps.len());
    for mut step in steps {
        // Already there: the tool stays where it is instead of the arm changing to another configuration
        // that reaches the same pose.
        if let (Goal::Pose(world), Travel::Free) = (&step.goal, &step.travel) {
            let at = scene.tip_in_world(&start);
            if (at.translation - world.translation).length() <= 1e-6
                && at.rotation.angle_between(world.rotation) <= 1e-6
            {
                step.goal = Goal::Joints(start.clone());
            }
        }
        let goal_pose = match &step.goal {
            Goal::Pose(world) => Some(*world),
            Goal::Joints(_) => None,
        };
        let request = MotionRequest {
            start: start.clone(),
            goal: step.goal,
            algorithm,
            options: Options::default(),
            seed,
            runs,
            max_acc: vec![robot.max_acc; start.len()],
            linear: !matches!(step.travel, Travel::Free),
            max_tcp_speed: step.tcp_speed,
            goal_seeds: match &goal_pose {
                Some(world) => analytic_goals(loaded, &scene.flange_target(world), &start),
                None => Vec::new(),
            },
            contacts: step.contacts,
        };
        let planned = match (&step.travel, previous.and_then(|p| p.segments.get(segments.len()))) {
            // A curve is followed anew each time: the same curve gives the same motion.
            (Travel::Along(via), _) => gc_motion::plan_follow(scene, &request, via, control),
            (_, Some(old)) => gc_motion::replan_using(scene, &request, old, plugin, control),
            (_, None) => gc_motion::plan_motion_using(scene, &request, plugin, control),
        };
        let plan = planned.map_err(|e| format!("{}: {e}", step.name))?;
        if let Some(end) = plan.path.last() {
            start.clone_from(end);
        }
        segments.push(plan);
    }
    Ok(segments)
}

/// A robot's link tree as plain data for the document: links from the root outwards, each with its joint
/// frame, axis and how the joint follows the planning chain's values (mimic joints included; joints
/// outside the chain stay at zero).
pub(crate) fn link_tree(model: &gc_robot::RobotModel, chain: &gc_robot::Chain) -> Vec<gc_doc::RobotLink> {
    use gc_robot::JointKind;
    let mut order: Vec<usize> = vec![model.root];
    let mut at = 0;
    while at < order.len() {
        let parent = order[at];
        for joint in &model.joints {
            if joint.parent == parent && !order.contains(&joint.child) {
                order.push(joint.child);
            }
        }
        at += 1;
    }
    let index_of = |link: usize| order.iter().position(|l| *l == link);
    order
        .iter()
        .map(|link| {
            let data = &model.links[*link];
            let Some(j) = data.parent_joint else {
                return gc_doc::RobotLink {
                    name: data.name.clone(),
                    parent: None,
                    origin: Placement::default(),
                    axis: [0.0, 0.0, 1.0],
                    motion: gc_doc::LinkMotion::Fixed,
                };
            };
            let joint = &model.joints[j];
            // Follow mimic joints to the joint that drives them.
            let (mut source, mut scale, mut offset) = (j, 1.0, 0.0);
            while let Some((master, multiplier, shift)) = model.joints[source].mimic {
                offset += scale * shift;
                scale *= multiplier;
                source = master;
            }
            let k = chain.active_joints().iter().position(|a| *a == source);
            #[allow(clippy::cast_possible_truncation, reason = "joint and link counts fit in u32")]
            let motion = match (joint.kind, k) {
                (JointKind::Revolute | JointKind::Continuous, Some(k)) => {
                    gc_doc::LinkMotion::Turn { joint: k as u32, scale, offset }
                }
                (JointKind::Prismatic, Some(k)) => {
                    gc_doc::LinkMotion::Slide { joint: k as u32, scale, offset }
                }
                _ => gc_doc::LinkMotion::Fixed,
            };
            #[allow(clippy::cast_possible_truncation, reason = "link counts fit in u32")]
            let parent = index_of(joint.parent).map(|p| p as u32);
            gc_doc::RobotLink {
                name: data.name.clone(),
                parent,
                origin: Placement {
                    xyz: joint.origin.translation.to_array(),
                    rpy: gc_math::rpy_from_quat(joint.origin.rotation),
                },
                axis: joint.axis.to_array(),
                motion,
            }
        })
        .collect()
}

/// A task planned for one robot (plan 9.5: what the robot would bid for the task).
#[derive(Clone, Debug)]
pub struct TaskPlan {
    /// The robot.
    pub robot: Id<Robot>,
    /// Its motions, in the order of the task's steps (a path step is a move to its start and the motion
    /// along it; steps without motion have none).
    pub segments: Vec<MotionPlan>,
    /// Seconds spent in `Wait` steps and in tool actions (their durations). Signals and signal waits
    /// count as no time.
    pub waiting: f64,
    /// The joint values the robot ends in.
    pub end: Vec<f64>,
    /// The joint values it starts in.
    pub start: Vec<f64>,
    /// Where it stands still (waits, tool actions): after how many of the motions, and for how long.
    pub pauses: Vec<(usize, f64)>,
}

impl TaskPlan {
    /// Expected time in seconds: the motions and the waits.
    pub fn duration(&self) -> f64 {
        self.segments.iter().map(|s| s.trajectory.duration()).sum::<f64>() + self.waiting
    }

    /// The joint values `t` seconds into the task (clamped to it): along the motions, standing still
    /// during the waits and tool actions where they come.
    pub fn sample(&self, t: f64) -> Vec<f64> {
        let mut t = t.max(0.0);
        let mut at = self.start.clone();
        for index in 0..=self.segments.len() {
            // What stands between the motions before this one and it.
            let pause: f64 = self.pauses.iter().filter(|(after, _)| *after == index).map(|(_, s)| s).sum();
            if t <= pause {
                return at;
            }
            t -= pause;
            let Some(segment) = self.segments.get(index) else { break };
            let duration = segment.trajectory.duration();
            if t <= duration {
                return segment.trajectory.sample(t).q;
            }
            t -= duration;
            at = segment.trajectory.sample(duration).q;
        }
        self.end.clone()
    }
}

/// A robot that may do a task, with the time it would take or why it cannot.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskCandidate {
    /// The robot.
    pub robot: Id<Robot>,
    /// Expected seconds from its current state, or the reason it cannot do the task.
    pub time: Result<f64, String>,
}

impl Planning<'_> {
    /// Plans a task for a robot from its current joint values; see [`Session::task_plan_from`].
    pub(crate) fn task_plan(&self, task: Id<gc_doc::Task>, robot: Id<Robot>) -> Result<TaskPlan, String> {
        let start = self.doc.get(robot).ok_or("the robot no longer exists")?.joints.clone();
        self.task_plan_from(task, robot, &start)
    }

    /// Plans a task's steps for a robot that starts at `start`: joint moves and straight moves to the
    /// waypoint roles (the tool centre point on the sketch point, held as the role says), the path roles
    /// as they say, the robot's own waypoints in a task located at it. The poses are where the sketches
    /// are now (a sketch on a component or a work plane follows it), so the plan is for the cell as it
    /// stands, with the robot's tool and every obstacle. Planned with the default planner, here and now
    /// (it can take a moment). Fails with the step and the reason: out of reach, in collision, a role
    /// whose sketch elements are gone, a robot the task is not for.
    pub(crate) fn task_plan_from(
        &self,
        task_id: Id<gc_doc::Task>,
        robot_id: Id<Robot>,
        start: &[f64],
    ) -> Result<TaskPlan, String> {
        self.task_plan_among(task_id, robot_id, start, &self.others_standing(robot_id))
    }

    /// The other robots where the document has them, as obstacles for `robot` (suppressed ones and those
    /// whose shapes are not known are left out).
    pub(crate) fn others_standing(&self, robot: Id<Robot>) -> Vec<Obstacle> {
        self.doc
            .robots()
            .iter()
            .filter(|(other, _)| *other != robot && !self.doc.is_component_suppressed(other.erase()))
            .filter_map(|(other, data)| {
                Some(self.scene_of(other).ok()?.as_obstacles(&data.joints, &data.name))
            })
            .flatten()
            .collect()
    }

    /// The planning scene of a robot in the cell as it stands (other robots are not in it).
    pub(crate) fn scene_of(&self, robot_id: Id<Robot>) -> Result<RobotScene, String> {
        let robot = self.doc.get(robot_id).ok_or("the robot no longer exists")?;
        let loaded = self.robots.get(&robot_id).ok_or("the robot description could not be read")?;
        robot_scene(self.doc, loaded, self.meshes, robot, &self.equipment)
    }

    /// [`Planning::task_plan_from`] with more obstacles in the way: other robots where they stand.
    pub(crate) fn task_plan_among(
        &self,
        task_id: Id<gc_doc::Task>,
        robot_id: Id<Robot>,
        start: &[f64],
        others: &[Obstacle],
    ) -> Result<TaskPlan, String> {
        use gc_doc::{Location, MoveTarget, RobotChoice};
        let doc = self.doc;
        let task = doc.get(task_id).ok_or("the task no longer exists")?;
        let robot = doc.get(robot_id).ok_or("the robot no longer exists")?;
        if doc.is_inactive(task_id.erase()) {
            return Err(format!("{} is suppressed", task.name));
        }
        if doc.is_component_suppressed(robot_id.erase()) {
            return Err(format!("{} is suppressed", robot.name));
        }
        let allowed = match (&task.location, &task.robots) {
            (Location::Robot(own), _) => *own == robot_id,
            (_, RobotChoice::Only(robots)) => robots.contains(&robot_id),
            (_, RobotChoice::Any | RobotChoice::When(_)) => true,
        };
        if !allowed {
            return Err(format!("{} is not for {}", task.name, robot.name));
        }
        let loaded = self.robots.get(&robot_id).ok_or("the robot description could not be read")?;
        let kit = doc.robot_tool_kit(robot_id).and_then(|(kit, _)| doc.get(kit));
        let mut steps = Vec::new();
        let mut waiting = 0.0;
        // Where the robot stands still: after how many motions, and for how long.
        let mut pauses: Vec<(usize, f64)> = Vec::new();
        let free = |name: &str, goal: Goal, travel: Travel| Step {
            name: name.to_owned(),
            goal,
            travel,
            tcp_speed: None,
            contacts: Vec::new(),
        };
        for step in &task.steps {
            let (target, travel) = match step {
                gc_doc::Step::MoveJ(target) => (target, Travel::Free),
                gc_doc::Step::MoveL(target) => (target, Travel::Linear),
                gc_doc::Step::FollowPath(role) => {
                    steps.extend(role_steps(doc, *role, None, &[])?);
                    continue;
                }
                gc_doc::Step::Wait { seconds } => {
                    waiting += seconds;
                    pauses.push((steps.len(), *seconds));
                    continue;
                }
                gc_doc::Step::ToolAction { action, target } => {
                    // Once the document has tool kits, the robot's own must be able to do it.
                    if !doc.tool_kits().is_empty() {
                        let found = kit
                            .and_then(|k| k.action(action))
                            .ok_or_else(|| format!("{}: its tool cannot `{action}`", robot.name))?;
                        let object_type = target.and_then(|t| doc.get(t)).map(|o| o.ty);
                        if found.target != object_type {
                            return Err(format!("`{action}` is not done to this kind of object"));
                        }
                        waiting += found.duration;
                        pauses.push((steps.len(), found.duration));
                    }
                    continue;
                }
                gc_doc::Step::SetIo { name, .. } => {
                    if !doc.tool_kits().is_empty() && !kit.is_some_and(|k| k.has_output(name)) {
                        return Err(format!("{}: its tool has no output signal `{name}`", robot.name));
                    }
                    continue;
                }
                gc_doc::Step::WaitIo { .. } => continue,
            };
            steps.push(match target {
                MoveTarget::Role(role) => {
                    let name = doc.get(*role).map(|r| r.name.clone()).unwrap_or_default();
                    let pose = doc
                        .role_pose(*role)
                        .ok_or_else(|| format!("{name}: its sketch point no longer exists"))?;
                    free(&name, Goal::Pose(pose), travel)
                }
                MoveTarget::Waypoint(waypoint) => {
                    let waypoint = doc.get(*waypoint).ok_or("a waypoint no longer exists")?;
                    free(&waypoint.name, waypoint_goal(doc, loaded, robot, waypoint)?, travel)
                }
            });
        }
        let mut obstacles = self.equipment.clone();
        if let Ok(list) = &mut obstacles {
            list.extend_from_slice(others);
        }
        let scene = robot_scene(doc, loaded, self.meshes, robot, &obstacles)?;
        let how = (PLANNERS[0].1, 1, 4, None);
        let segments = run_steps(loaded, robot, &scene, steps, start.to_vec(), how, None, self.control)?;
        let end = segments.last().and_then(|s| s.path.last()).map_or_else(|| start.to_vec(), Vec::clone);
        Ok(TaskPlan { robot: robot_id, segments, waiting, end, start: start.to_vec(), pauses })
    }

    /// The robots a task may be given to as the states stand: the robot it is located at, the ones it
    /// names, or every robot (none while its robots' condition does not hold). Suppressed robots are
    /// left out. In the order the robots were added.
    pub(crate) fn task_robots(&self, task: Id<gc_doc::Task>) -> Vec<Id<Robot>> {
        use gc_doc::{Location, RobotChoice};
        let doc = self.doc;
        let Some(task) = doc.get(task) else { return Vec::new() };
        let all = || doc.robots().iter().map(|(id, _)| id);
        let robots: Vec<Id<Robot>> = match (&task.location, &task.robots) {
            (Location::Robot(own), _) => vec![*own],
            (_, RobotChoice::Only(robots)) => all().filter(|r| robots.contains(r)).collect(),
            (_, RobotChoice::Any) => all().collect(),
            (_, RobotChoice::When(condition)) => {
                if doc.condition_holds(condition).unwrap_or(false) {
                    all().collect()
                } else {
                    Vec::new()
                }
            }
        };
        robots.into_iter().filter(|r| !doc.is_component_suppressed(r.erase())).collect()
    }

    /// The robots that may do a task ([`Session::task_robots`]), each with the time the task would take
    /// it from its current state or why it cannot (out of reach, in collision): the fastest first, those
    /// that cannot last.
    pub(crate) fn task_candidates(&self, task: Id<gc_doc::Task>) -> Vec<TaskCandidate> {
        let mut candidates: Vec<TaskCandidate> = self
            .task_robots(task)
            .into_iter()
            .map(|robot| TaskCandidate { robot, time: self.task_plan(task, robot).map(|p| p.duration()) })
            .collect();
        candidates.sort_by(|a, b| match (&a.time, &b.time) {
            (Ok(x), Ok(y)) => x.total_cmp(y),
            (Ok(_), Err(_)) => std::cmp::Ordering::Less,
            (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
            (Err(_), Err(_)) => std::cmp::Ordering::Equal,
        });
        candidates
    }
}

/// What planning for tasks reads: the document, the robots' descriptions, the meshes and the equipment
/// as obstacles. A session lends its own; a background job owns a copy ([`PlanningCopy`]).
pub(crate) struct Planning<'a> {
    pub(crate) doc: &'a Document,
    pub(crate) robots: &'a BTreeMap<Id<Robot>, Arc<LoadedRobot>>,
    pub(crate) meshes: &'a BTreeMap<String, MeshRef>,
    pub(crate) equipment: Result<Vec<Obstacle>, String>,
    pub(crate) control: &'a dyn Control,
}

/// A copy of what [`Planning`] reads, for a job that outlives the call.
struct PlanningCopy {
    doc: Document,
    robots: BTreeMap<Id<Robot>, Arc<LoadedRobot>>,
    meshes: BTreeMap<String, MeshRef>,
    equipment: Result<Vec<Obstacle>, String>,
}

impl Session {
    fn planning(&self) -> Planning<'_> {
        Planning {
            doc: &self.doc,
            robots: &self.cell.robots,
            meshes: &self.cell.meshes,
            equipment: crate::assembly::obstacles(&self.doc, &self.cell.assemblies, &pose_of),
            control: &gc_plan::NeverCancel,
        }
    }

    /// Plans a task for a robot from its current joint values; see [`Session::task_plan_from`].
    pub fn task_plan(&self, task: Id<gc_doc::Task>, robot: Id<Robot>) -> Result<TaskPlan, String> {
        self.planning().task_plan(task, robot)
    }

    /// Plans a task's steps for a robot that starts at `start`: joint moves and straight moves to the
    /// waypoint roles (the tool centre point on the sketch point, held as the role says), the path roles
    /// as they say, the robot's own waypoints in a task located at it. The poses are where the sketches
    /// are now (a sketch on a component or a work plane follows it), so the plan is for the cell as it
    /// stands, with the robot's tool and every obstacle. Planned with the default planner, here and now
    /// (it can take a moment). Fails with the step and the reason: out of reach, in collision, a role
    /// whose sketch elements are gone, a robot the task is not for or whose tool cannot do a step.
    pub fn task_plan_from(
        &self,
        task: Id<gc_doc::Task>,
        robot: Id<Robot>,
        start: &[f64],
    ) -> Result<TaskPlan, String> {
        self.planning().task_plan_from(task, robot, start)
    }

    /// The robots a task may be given to as the states stand: the robot it is located at, the ones it
    /// names, or every robot (none while its robots' condition does not hold). Suppressed robots are
    /// left out. In the order the robots were added.
    pub fn task_robots(&self, task: Id<gc_doc::Task>) -> Vec<Id<Robot>> {
        self.planning().task_robots(task)
    }

    /// The robots that may do a task ([`Session::task_robots`]), each with the time the task would take
    /// it from its current state or why it cannot (out of reach, in collision, no such tool action): the
    /// fastest first, those that cannot last.
    pub fn task_candidates(&self, task: Id<gc_doc::Task>) -> Vec<TaskCandidate> {
        self.planning().task_candidates(task)
    }

    /// Works out who does what when, here and now; see [`crate::Schedule`]. Suppressed tasks are left
    /// out. Planning every bid takes a moment in a cell with many tasks and robots: a window should use
    /// [`Session::request_schedule`] instead.
    pub fn schedule(&self) -> crate::Schedule {
        self.planning().schedule()
    }

    /// Starts working out the schedule in the background (on the executor's threads; with the default
    /// executor, here and now). The result is for the document as it is at this moment and arrives
    /// through [`Session::poll_jobs`]; see [`Session::schedule_result`]. A schedule still being worked
    /// out for an older document is cancelled; one for this document is left to finish.
    pub fn request_schedule(&mut self) {
        let revision = self.doc.revision();
        if self.cell.schedule_job.as_ref().is_some_and(|(wanted, _)| *wanted == revision)
            || self.cell.schedule.as_ref().is_some_and(|(made, _)| *made == revision)
        {
            return;
        }
        if let Some((_, job)) = self.cell.schedule_job.take() {
            job.cancel();
        }
        let planning = self.planning();
        let copy = PlanningCopy {
            doc: self.doc.clone(),
            robots: self.cell.robots.clone(),
            meshes: self.cell.meshes.clone(),
            equipment: planning.equipment,
        };
        let job = self.cell.executor.spawn("schedule", move |ctx| {
            let control = JobControl(ctx.cancel.clone());
            let planning = Planning {
                doc: &copy.doc,
                robots: &copy.robots,
                meshes: &copy.meshes,
                equipment: copy.equipment.clone(),
                control: &control,
            };
            let schedule = planning.schedule();
            (!ctx.cancel.is_cancelled()).then_some(schedule)
        });
        self.cell.schedule_job = Some((revision, job));
        self.poll_jobs();
    }

    /// The last schedule worked out by [`Session::request_schedule`], and whether it is still current:
    /// `true` when the document has not changed since it was asked for (an undo back to that state makes
    /// it current again only after it is asked for again).
    pub fn schedule_result(&self) -> Option<(&crate::Schedule, bool)> {
        let (made, schedule) = self.cell.schedule.as_ref()?;
        Some((schedule, *made == self.doc.revision()))
    }

    /// Shows every robot where the last schedule worked out ([`Session::schedule_result`]) has it at
    /// `time` seconds: along its task's motion, standing still in waits, tool actions and between tasks.
    /// The document is not changed. `None` shows the robots where they are. Ignored without a schedule.
    pub fn set_schedule_playback(&mut self, time: Option<f64>) {
        self.cell.schedule_time = time.filter(|t| t.is_finite() && self.cell.schedule.is_some());
        if self.cell.schedule_time.is_some() {
            self.cell.playback = None;
        }
    }

    /// The moment of the schedule being shown, if any.
    pub fn schedule_playback(&self) -> Option<f64> {
        self.cell.schedule_time
    }

    /// True while a schedule is being worked out.
    pub fn schedule_pending(&self) -> bool {
        self.cell.schedule_job.is_some()
    }

    fn poll_schedule(&mut self) -> bool {
        let Some((revision, mut job)) = self.cell.schedule_job.take() else { return false };
        match job.poll() {
            None => {
                self.cell.schedule_job = Some((revision, job));
                false
            }
            Some(JobOutcome::Done(Some(schedule))) => {
                self.cell.schedule = Some((revision, schedule));
                true
            }
            Some(JobOutcome::Done(None) | JobOutcome::Failed(_)) => true,
        }
    }
}
