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
    Waypoint, WaypointTarget, WorkFrame,
};
use gc_jobs::{CancelToken, Executor, JobHandle, JobOutcome};
use gc_math::{DPose3, DQuat, DVec3, pose_from_xyz_rpy, rpy_from_quat};
use gc_mesh::MeshData;
use gc_motion::{Goal, MotionPlan, MotionRequest, Obstacle, RobotScene, StateProblem, plan_motion};
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
}

pub(crate) struct CellState {
    executor: Executor,
    job: Option<PlanJob>,
    compare_job: Option<CompareJob>,
    comparisons: BTreeMap<Id<MotionPath>, (String, Vec<CompareRow>)>,
    robots: BTreeMap<Id<Robot>, Arc<LoadedRobot>>,
    jog: Option<(Id<Robot>, Vec<f64>)>,
    body: Option<(Id<Body>, Body)>,
    base: Option<(Id<Robot>, Placement)>,
    tcp: Option<(Id<Robot>, Placement)>,
    waypoint: Option<(Id<Waypoint>, WaypointTarget)>,
    frame: Option<(Id<WorkFrame>, WorkFrame)>,
    look: Option<(Shown, gc_doc::Look)>,
    selected: Option<CellItem>,
    hovered: Option<CellItem>,
    plans: BTreeMap<Id<MotionPath>, Planned>,
    playback: Option<(Id<MotionPath>, f64)>,
    resources: Option<Resources>,
    /// The collision scene of the cell as it is shown, with what it was built from, and the last
    /// question asked of it with its answer. Rebuilt when a robot or a body moves.
    scene: std::cell::RefCell<Option<(String, RobotScene)>>,
    asked: std::cell::RefCell<Option<(String, Option<StateProblem>)>>,
    /// The last ghost worked out, with what it was worked out from.
    ghost: std::cell::RefCell<Option<(String, Ghost)>>,
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
            compare_job: None,
            comparisons: BTreeMap::new(),
            robots: BTreeMap::new(),
            jog: None,
            body: None,
            base: None,
            tcp: None,
            waypoint: None,
            frame: None,
            look: None,
            selected: None,
            hovered: None,
            plans: BTreeMap::new(),
            playback: None,
            resources: None,
            files: BTreeMap::new(),
            meshes: BTreeMap::new(),
            scene: std::cell::RefCell::new(None),
            asked: std::cell::RefCell::new(None),
            ghost: std::cell::RefCell::new(None),
        }
    }
}

impl CellState {
    /// Brings the caches in line with the document.
    pub(crate) fn sync(&mut self, doc: &Document) {
        self.robots.retain(|id, loaded| doc.get(*id).is_some_and(|r| r.urdf == loaded.urdf));
        for (id, robot) in doc.robots().iter() {
            if !self.robots.contains_key(&id)
                && let Ok(loaded) = load(
                    &robot.urdf,
                    Some((&robot.base_link, &robot.tip_link)),
                    self.resources.as_ref(),
                    &robot.resource_dir,
                )
            {
                self.robots.insert(id, Arc::new(loaded.0));
            }
        }
        for (_, body) in doc.bodies().iter() {
            self.read_body_mesh(&body.solid);
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
    };
    loaded.read_meshes(resources, resource_dir);
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

/// The world pose of a work frame: at its origin point, with the sketch's axes turned by its angles.
fn work_frame_pose(doc: &Document, frame: Id<WorkFrame>) -> Option<DPose3> {
    frame_pose_of(doc, doc.get(frame)?)
}

fn frame_pose_of(doc: &Document, frame: &WorkFrame) -> Option<DPose3> {
    let sketch = doc.get(frame.sketch)?;
    let (Space::Planar(base) | Space::Spatial(base)) = sketch.space;
    let base = doc.frame_world_pose(Id::from_raw(base.raw))?;
    let world = |point: Id<SkPoint>| {
        Some(base.translation + base.rotation * DVec3::from_array(sketch.points.get(&point)?.pos))
    };
    let origin = world(frame.point)?;
    let axes = match frame.x_point {
        None => base.rotation,
        Some(x_point) => {
            // Points closer than a micrometre, or within a microradian of one line, give no direction.
            let towards = world(x_point)? - origin;
            let x = towards.try_normalize().filter(|_| towards.length() > 1e-6)?;
            // The XY plane: through the third point, or square to the sketch's normal.
            let z = match frame.xy_point {
                Some(xy_point) => {
                    let side = world(xy_point)? - origin;
                    let normal = x.cross(side);
                    normal.try_normalize().filter(|_| normal.length() > 1e-6 * side.length().max(1e-6))?
                }
                None => {
                    let normal = base.rotation * DVec3::Z;
                    (normal - x * normal.dot(x)).try_normalize()?
                }
            };
            DQuat::from_mat3(&gc_math::DMat3::from_cols(x, z.cross(x), z)).normalize()
        }
    };
    Some(DPose3::from_parts(origin, axes * gc_math::quat_from_rpy(frame.rpy[0], frame.rpy[1], frame.rpy[2])))
}

/// The world pose of the tool centre point at a waypoint target that does not need the robot.
fn target_pose(doc: &Document, target: &WaypointTarget) -> Option<DPose3> {
    match target {
        WaypointTarget::Joints(_) => None,
        WaypointTarget::SketchPoint { sketch, point } => pose_at_point(doc, *sketch, *point),
        WaypointTarget::InFrame { frame, offset } => Some(work_frame_pose(doc, *frame)? * pose_of(offset)),
    }
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

    fn add_robot_from(&mut self, name: &str, urdf: &str, resource_dir: &str) -> Result<Id<Robot>, String> {
        let (loaded, base_link, tip_link) = load(urdf, None, self.cell.resources.as_ref(), resource_dir)?;
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

    /// The selected item of the cell.
    pub fn selected_item(&self) -> Option<CellItem> {
        self.cell.selected.filter(|item| self.item_exists(*item))
    }

    /// Selects an item of the cell, or nothing.
    pub fn select_item(&mut self, item: Option<CellItem>) {
        self.cell.selected = item.filter(|item| self.item_exists(*item));
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

    /// Where a robot's base is shown: the preview while it is being placed, else the document's.
    pub fn shown_base(&self, robot: Id<Robot>) -> Option<Placement> {
        match &self.cell.base {
            Some((id, preview)) if *id == robot => Some(*preview),
            _ => self.doc.get(robot).map(|r| r.placement),
        }
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
        let mut built_from = format!("{:?} {} {base:?}", robot, stored.urdf.len());
        for (id, _) in self.doc.bodies().iter() {
            let body = self.shown_body(id)?;
            // A body whose mesh is not there cannot be checked; it is left out here and stops planning.
            let Some(shape) = shape_of(&body.solid, &self.cell.meshes) else { continue };
            built_from.push_str(&format!(" {:?} {:?}", body.placement, body.solid));
            obstacles.push(Obstacle { name: body.name.clone(), pose: pose_of(&body.placement), shape });
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
            )
            .ok()?;
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
        let question = format!("{robot:?} {target:?} {world:?} {base:?} {tcp:?} {seed:?} {bodies:?}");
        if let Some((asked, ghost)) = self.cell.ghost.borrow().as_ref()
            && *asked == question
        {
            return Some(ghost.clone());
        }
        let candidates = match &target {
            WaypointTarget::Joints(q) => vec![q.clone()],
            _ => {
                let flange = pose_of(&base).inverse() * world * pose_of(&tcp).inverse();
                let ik = gc_robot::NumericIk::new(&loaded.chain, gc_robot::IkOptions::default());
                let mut found = ik.solve_many(&flange, Some(&seed), GHOST_STARTS);
                let distance =
                    |q: &Vec<f64>| q.iter().zip(&seed).map(|(a, b)| (a - b).abs()).fold(0.0_f64, f64::max);
                found.sort_by(|a, b| distance(a).total_cmp(&distance(b)));
                found
            }
        };
        // The nearest pose without a problem; failing that, the nearest with what is wrong there.
        let mut ghost = Ghost { robot, joints: None, problem: None };
        for q in candidates {
            let problem = self.problem_at(robot, &q, false);
            let fine = problem.is_none();
            if ghost.joints.is_none() || fine {
                ghost = Ghost { robot, joints: Some(q), problem };
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

    /// The world pose of a robot's tool centre point as it is shown.
    pub fn tcp_pose(&self, robot: Id<Robot>) -> Option<DPose3> {
        let loaded = self.robot_model(robot)?;
        let tip = loaded.model.link(&self.doc.get(robot)?.tip_link)?;
        Some(*self.link_poses(robot)?.get(tip)? * pose_of(&self.shown_tcp(robot)?))
    }

    /// World poses of all links of a robot as it is shown.
    pub fn link_poses(&self, robot: Id<Robot>) -> Option<Vec<DPose3>> {
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

    /// The world pose of the tool at a waypoint.
    pub fn waypoint_pose(&self, waypoint: Id<Waypoint>) -> Option<DPose3> {
        let target = self.shown_target(waypoint)?;
        let waypoint = self.doc.get(waypoint)?;
        match &target {
            WaypointTarget::SketchPoint { .. } => target_pose(&self.doc, &target),
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
        let path = MotionPath {
            name,
            robot,
            waypoints,
            linear: Vec::new(),
            planner: PLANNERS[0].0.to_owned(),
            seed: 1,
        };
        match self.cell_op(CellOp::AddPath(path))?.first() {
            Some(Created::Path(id)) => Ok(*id),
            _ => Err(EditError::BadValue),
        }
    }

    /// Makes a path that follows the lines of a sketch in the order they were drawn: a waypoint on the
    /// first point, then one at the end of every line, approached in a straight line. Where a line does
    /// not start at the end of the one before, the robot moves there freely first. The waypoints stay
    /// attached to the sketch points, so the path follows when the sketch is edited.
    pub fn path_from_sketch(
        &mut self,
        robot: Id<Robot>,
        sketch_id: Id<Sketch>,
    ) -> Result<Id<MotionPath>, EditError> {
        let sketch = self.doc.get(sketch_id).ok_or(EditError::BadValue)?;
        let mut stops: Vec<(Id<SkPoint>, bool)> = Vec::new();
        let mut end: Option<[f64; 3]> = None;
        for curve in sketch.curves.keys() {
            let Some((a, b)) = sketch.line(*curve) else { continue };
            let (Some(start), Some(finish)) = (sketch.pos(a), sketch.pos(b)) else { continue };
            let joined = end.is_some_and(|e| (0..3).all(|k| (e[k] - start[k]).abs() < 1e-9));
            if !joined {
                stops.push((a, false));
            }
            stops.push((b, true));
            end = Some(finish);
        }
        if stops.is_empty() {
            return Err(EditError::BadValue);
        }
        let mut waypoints = Vec::with_capacity(stops.len());
        for (point, _) in &stops {
            waypoints.push(
                self.add_waypoint(robot, WaypointTarget::SketchPoint { sketch: sketch_id, point: *point })?,
            );
        }
        let name = format!("Path{}", self.doc.paths().len() + 1);
        let linear = stops.iter().map(|(_, linear)| *linear).collect();
        let path = MotionPath { name, robot, waypoints, linear, planner: PLANNERS[0].0.to_owned(), seed: 1 };
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
                Some(target) => format!("{:?}", target_pose(doc, target)),
                None => String::new(),
            })
            .collect();
        // What counts is how each waypoint is approached, not how the list happens to be stored.
        let linear: Vec<bool> =
            (0..path.waypoints.len()).map(|index| path.linear.get(index).copied().unwrap_or(false)).collect();
        let bodies: Vec<(&Placement, &Solid)> =
            doc.bodies().iter().map(|(_, b)| (&b.placement, &b.solid)).collect();
        Some(format!(
            "{} {} {} {:?} {:?} {:?} {} {bodies:?} {targets:?} {:?} {} {}",
            robot.urdf.len(),
            robot.base_link,
            robot.tip_link,
            robot.placement,
            robot.tcp,
            robot.joints,
            robot.max_acc,
            linear,
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
        self.end_base_preview();
        self.end_tcp_preview();
        self.end_target_preview();
        self.end_frame_preview();
        self.end_body_preview();
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
                Some(loaded) => plan_path(&doc, loaded, &meshes, path, None, &control),
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
        let job = self.cell.executor.spawn("compare", move |ctx| {
            let control = JobControl(ctx.cancel.clone());
            let loaded = loaded?;
            let mut rows = Vec::with_capacity(PLANNERS.len());
            let total = (PLANNERS.len() * seeds.max(1)) as f32;
            for (index, (planner, algorithm)) in PLANNERS.iter().enumerate() {
                let (mut durations, mut costs, mut checks) = (Vec::new(), Vec::new(), Vec::new());
                for seed in 0..seeds.max(1) {
                    let strategy = Some((*algorithm, seed as u64, 1));
                    if let Ok(planned) = plan_path(&doc, &loaded, &meshes, path, strategy, &control) {
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
        let mut arrived = false;
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
        arrived | self.poll_plan()
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
fn plan_path(
    doc: &Document,
    loaded: &LoadedRobot,
    meshes: &BTreeMap<String, MeshRef>,
    path_id: Id<MotionPath>,
    strategy: Option<(Algorithm, u64, usize)>,
    control: &dyn Control,
) -> Result<PlannedPath, String> {
    let path = doc.get(path_id).ok_or("the path no longer exists")?;
    let robot = doc.get(path.robot).ok_or("the robot no longer exists")?;
    let (algorithm, seed, runs) = match strategy {
        Some(strategy) => strategy,
        None => {
            let algorithm = PLANNERS
                .iter()
                .find(|(id, _)| *id == path.planner)
                .map(|(_, algorithm)| *algorithm)
                .ok_or_else(|| format!("unknown planner `{}`", path.planner))?;
            (algorithm, path.seed, 4)
        }
    };
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
    let scene = scene.with_tool(pose_of(&robot.tcp));
    let mut start = robot.joints.clone();
    let mut segments = Vec::with_capacity(path.waypoints.len());
    for (index, waypoint_id) in path.waypoints.iter().enumerate() {
        let waypoint = doc.get(*waypoint_id).ok_or("a waypoint no longer exists")?;
        let goal =
            match &waypoint.target {
                WaypointTarget::Joints(q) => Goal::Joints(q.clone()),
                target => Goal::Pose(target_pose(doc, target).ok_or_else(|| {
                    format!("{}: its sketch point or frame no longer exists", waypoint.name)
                })?),
            };
        let request = MotionRequest {
            start: start.clone(),
            goal,
            algorithm,
            options: Options::default(),
            seed,
            runs,
            max_acc: vec![robot.max_acc; start.len()],
            linear: path.linear.get(index).copied().unwrap_or(false),
        };
        let plan = plan_motion(&scene, &request, control).map_err(|e| format!("{}: {e}", waypoint.name))?;
        if let Some(end) = plan.path.last() {
            start.clone_from(end);
        }
        segments.push(plan);
    }
    Ok(PlannedPath { segments })
}
