//! What a session looks like in 3D: the solids to draw and the lines and markers on top of them.
//!
//! This is the one place that turns document and session state into drawable items. It knows nothing
//! about the GPU or the user interface toolkit.

#![forbid(unsafe_code)]

use std::sync::Arc;

use gc_doc::Solid;
use gc_math::{Aabb, DPose3, DVec3, pose_from_xyz_rpy};
use gc_mesh::{MeshData, MeshError, primitives};
use gc_robot::Shape;
use gc_session::{CellItem, MeshRef, PlanStatus, Session, Shown};

/// A basic solid, centred on its own origin. Solids with equal sizes share one uploaded mesh.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Primitive {
    /// A box with these edge lengths.
    Box([f64; 3]),
    /// A cylinder along Z.
    Cylinder {
        /// Radius.
        radius: f64,
        /// Length.
        length: f64,
    },
    /// A sphere.
    Sphere(f64),
}

impl Primitive {
    /// A key that is equal for equal solids (FNV-1a over the kind and the bit patterns of the sizes).
    pub fn key(&self) -> u64 {
        let (kind, sizes): (u64, [f64; 3]) = match *self {
            Self::Box(size) => (1, size),
            Self::Cylinder { radius, length } => (2, [radius, length, 0.0]),
            Self::Sphere(radius) => (3, [radius, 0.0, 0.0]),
        };
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for word in [kind, sizes[0].to_bits(), sizes[1].to_bits(), sizes[2].to_bits()] {
            for byte in word.to_le_bytes() {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        hash
    }

    /// The triangles of the solid.
    pub fn mesh(&self) -> Result<MeshData, MeshError> {
        match *self {
            Self::Box(size) => primitives::cuboid(size),
            Self::Cylinder { radius, length } => primitives::cylinder(radius, length, 48),
            Self::Sphere(radius) => primitives::sphere(radius, 32),
        }
    }

    /// The radius of a sphere around the origin that contains the solid.
    pub fn radius(&self) -> f64 {
        match *self {
            Self::Box(size) => DVec3::from_array(size).length() * 0.5,
            Self::Cylinder { radius, length } => DVec3::new(radius, radius, length * 0.5).length(),
            Self::Sphere(radius) => radius,
        }
    }
}

/// What a solid looks like.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape3 {
    /// A basic solid.
    Primitive(Primitive),
    /// A mesh from a file.
    Mesh(MeshRef),
}

impl Shape3 {
    /// A key that is equal for equal shapes; the renderer uploads each key once.
    pub fn key(&self) -> u64 {
        match self {
            Self::Primitive(primitive) => primitive.key(),
            Self::Mesh(mesh) => mesh.key,
        }
    }

    /// The triangles of the shape.
    pub fn mesh(&self) -> Result<Arc<MeshData>, MeshError> {
        match self {
            Self::Primitive(primitive) => primitive.mesh().map(Arc::new),
            Self::Mesh(mesh) => Ok(Arc::clone(&mesh.data)),
        }
    }

    /// The radius of a sphere around the origin that contains the shape.
    pub fn radius(&self) -> f64 {
        match self {
            Self::Primitive(primitive) => primitive.radius(),
            Self::Mesh(mesh) => mesh.radius,
        }
    }

    /// A box around the shape in its own frame.
    pub fn local_bounds(&self) -> Aabb {
        let around = |half: DVec3| Aabb::from_points([-half, half]);
        match self {
            Self::Primitive(Primitive::Box(size)) => around(DVec3::from_array(*size) * 0.5),
            Self::Primitive(Primitive::Cylinder { radius, length }) => {
                around(DVec3::new(*radius, *radius, length * 0.5))
            }
            Self::Primitive(Primitive::Sphere(radius)) => around(DVec3::splat(*radius)),
            Self::Mesh(mesh) => mesh.bounds,
        }
    }
}

/// One solid to draw.
#[derive(Clone, Debug, PartialEq)]
pub struct Solid3 {
    /// Its shape.
    pub shape: Shape3,
    /// Its pose in the world.
    pub pose: DPose3,
    /// Linear RGBA.
    pub color: [f32; 4],
    /// What it belongs to, for picking.
    pub owner: Option<CellItem>,
}

/// The tool-tip trace of a planned path.
#[derive(Clone, Debug, PartialEq)]
pub struct Trace {
    /// World positions along the motion.
    pub points: Vec<DVec3>,
    /// False when the plan is out of date.
    pub current: bool,
    /// The path it belongs to.
    pub id: gc_core::Id<gc_doc::MotionPath>,
    /// Whether the path is under the cursor or selected.
    pub highlight: Highlight,
}

/// A named place in the world.
#[derive(Clone, Debug, PartialEq)]
pub struct Marker {
    /// The name shown beside it.
    pub name: String,
    /// Where it is.
    pub position: DVec3,
    /// How the tool is turned there: the tool centre point's axes.
    pub rotation: gc_math::DQuat,
    /// The waypoint.
    pub id: gc_core::Id<gc_doc::Waypoint>,
    /// Selected or under the cursor.
    pub highlight: Highlight,
}

/// How an item is emphasised.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Highlight {
    /// Not emphasised.
    #[default]
    None,
    /// Under the cursor.
    Hovered,
    /// Selected.
    Selected,
}

/// The colour of the cursor's highlight.
pub const HOVER: [f32; 3] = [1.0, 0.55, 0.1];
/// The colour of the selection.
pub const SELECTED: [f32; 3] = [0.15, 0.55, 1.0];

/// How opaque what lies outside the component being edited in place stays.
const OUT_OF_SCOPE: f32 = 0.25;

/// A colour mixed towards the highlight colour.
pub fn emphasised(color: [f32; 4], highlight: Highlight) -> [f32; 4] {
    let mix = |towards: [f32; 3]| {
        let k = 0.5;
        [
            color[0] + (towards[0] - color[0]) * k,
            color[1] + (towards[1] - color[1]) * k,
            color[2] + (towards[2] - color[2]) * k,
            color[3],
        ]
    };
    match highlight {
        Highlight::None => color,
        Highlight::Hovered => mix(HOVER),
        Highlight::Selected => mix(SELECTED),
    }
}

/// Everything 3D in a session.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    /// Bodies and robot links.
    pub solids: Vec<Solid3>,
    /// Tool-tip traces of planned paths.
    pub traces: Vec<Trace>,
    /// Waypoints.
    pub waypoints: Vec<Marker>,
    /// The tool centre point of every robot.
    pub tool_points: Vec<DPose3>,
    /// Work frames: name and pose.
    pub frames: Vec<(String, DPose3)>,
    /// The work frames' ids and highlights, in the order of `frames`.
    pub frame_items: Vec<(gc_core::Id<gc_doc::WorkFrame>, Highlight)>,
}

const BODY: [f32; 4] = [0.55, 0.62, 0.70, 1.0];
/// Links without colours of their own: steel grey alternating with dark grey, like an industrial arm.
const LINK_LIGHT: [f32; 4] = [0.60, 0.65, 0.72, 1.0];
/// A ghost robot: light blue, see-through.
const GHOST: [f32; 4] = [0.45, 0.70, 1.0, 0.35];
/// A ghost robot in a pose with a problem.
const GHOST_BAD: [f32; 4] = [1.0, 0.35, 0.30, 0.40];
/// A tool on a robot's flange: brushed metal.
const TOOL: [f32; 4] = [0.72, 0.70, 0.62, 1.0];
const LINK_DARK: [f32; 4] = [0.27, 0.29, 0.32, 1.0];

impl Scene {
    /// Builds the scene of a session as it is shown right now (jog preview and playback included).
    pub fn of(session: &Session) -> Self {
        let doc = session.doc();
        let mut scene = Self::default();
        let highlight = |item: CellItem| {
            if session.selected_item() == Some(item) {
                Highlight::Selected
            } else if session.hovered_item() == Some(item) {
                Highlight::Hovered
            } else {
                Highlight::None
            }
        };
        for (id, _) in doc.bodies().iter() {
            let Some(body) = session.shown_body(id) else { continue };
            // A file that could not be read is reported in the panel and not drawn.
            let Some(shape) = solid_shape(session, &body.solid) else { continue };
            let pose = pose_from_xyz_rpy(body.placement.xyz, body.placement.rpy);
            let look = session.shown_look(Shown::Body(id)).unwrap_or_default();
            if look.hidden {
                continue;
            }
            let mut color = BODY;
            color[3] = 1.0 - look.transparency as f32;
            // Editing a component or subassembly in place shows the rest faded (Inventor 2026 help, Edit
            // components in place, GUID-C79C36C9-A6DB-46C5-8864-41F66C750231).
            if !session.in_edit_scope(gc_doc::Occurrence::Item(id.erase())) {
                color[3] *= OUT_OF_SCOPE;
            }
            let owner = CellItem::Body(id);
            let color = emphasised(color, highlight(owner));
            scene.solids.push(Solid3 { shape, pose, color, owner: Some(owner) });
        }
        for (id, _) in doc.robots().iter() {
            let (Some(loaded), Some(poses)) = (session.robot_model(id), session.link_poses(id)) else {
                continue;
            };
            let look = session.shown_look(Shown::Robot(id)).unwrap_or_default();
            if look.hidden {
                continue;
            }
            let mut alpha = 1.0 - look.transparency as f32;
            if !session.in_edit_scope(gc_doc::Occurrence::Item(id.erase())) {
                alpha *= OUT_OF_SCOPE;
            }
            // A robot that collides as shown is drawn reddish.
            let colliding = session.robot_problem(id).is_some();
            let owner = CellItem::Robot(id);
            let emphasis = highlight(owner);
            let shown = |color: [f32; 4]| {
                let color = emphasised([color[0], color[1], color[2], color[3] * alpha], emphasis);
                if colliding {
                    [0.55 + 0.45 * color[0], 0.35 * color[1], 0.35 * color[2], color[3]]
                } else {
                    color
                }
            };
            let tool = robot_tool(session, id);
            push_robot(&mut scene, loaded, &poses, tool.as_ref(), Some(owner), &shown);
        }
        // How the arm would stand at the waypoint under the mouse or selected: see-through, reddish when
        // something is wrong there.
        if let Some(ghost) = session.shown_ghost()
            && let (Some(loaded), Some(poses)) =
                (session.robot_model(ghost.robot), session.ghost_links(&ghost))
        {
            let tint = if ghost.problem.is_some() { GHOST_BAD } else { GHOST };
            let shown = |color: [f32; 4]| {
                let grey = (color[0] + color[1] + color[2]) / 3.0;
                [0.5 * tint[0] + 0.5 * grey, 0.5 * tint[1] + 0.5 * grey, 0.5 * tint[2] + 0.5 * grey, tint[3]]
            };
            let tool = robot_tool(session, ghost.robot);
            push_robot(&mut scene, loaded, &poses, tool.as_ref(), None, &shown);
        }
        scene.tool_points = doc.robots().iter().filter_map(|(id, _)| session.tcp_pose(id)).collect();
        // Hidden, suppressed and rolled-back items are kept in the document but not drawn.
        let drawn = |item: gc_core::AnyId| doc.is_visible(item) && !doc.is_inactive(item);
        for (id, frame) in doc.work_frames().iter().filter(|(id, _)| drawn(id.erase())) {
            if let Some(pose) = session.work_frame_pose(id) {
                scene.frames.push((frame.name.clone(), pose));
                scene.frame_items.push((id, highlight(CellItem::Frame(id))));
            }
        }
        for (id, _) in doc.paths().iter().filter(|(id, _)| drawn(id.erase())) {
            if let Some(planned) = session.planned(id) {
                scene.traces.push(Trace {
                    points: planned.tip_path().map(DVec3::from_array).collect(),
                    current: session.plan_status(id) == PlanStatus::Ready,
                    id,
                    highlight: highlight(CellItem::Path(id)),
                });
            }
        }
        // The waypoints of the selected path are numbered in their order and shown selected with it.
        let chosen = match session.selected_item() {
            Some(CellItem::Path(path)) => doc.get(path).map(|p| p.waypoints.clone()),
            _ => None,
        };
        for (id, waypoint) in doc.waypoints().iter().filter(|(id, _)| drawn(id.erase())) {
            if let Some(pose) = session.waypoint_pose(id) {
                let steps: Vec<String> = chosen
                    .iter()
                    .flatten()
                    .enumerate()
                    .filter(|(_, w)| **w == id)
                    .map(|(index, _)| (index + 1).to_string())
                    .collect();
                let (name, highlight) = if steps.is_empty() {
                    (waypoint.name.clone(), highlight(CellItem::Waypoint(id)))
                } else {
                    (format!("{}·{}", steps.join(","), waypoint.name), Highlight::Selected)
                };
                scene.waypoints.push(Marker {
                    name,
                    position: pose.translation,
                    rotation: pose.rotation,
                    id,
                    highlight,
                });
            }
        }
        scene
    }

    /// A box around everything in the scene; empty when there is nothing.
    pub fn bounds(&self) -> Aabb {
        let mut bounds = Aabb::EMPTY;
        for solid in &self.solids {
            // The eight corners of the solid's own box, placed in the world.
            let local = solid.shape.local_bounds();
            if local.is_empty() {
                continue;
            }
            for corner in 0..8 {
                let pick = |bit: usize, lo: f64, hi: f64| if corner >> bit & 1 == 0 { lo } else { hi };
                let point = DVec3::new(
                    pick(0, local.min.x, local.max.x),
                    pick(1, local.min.y, local.max.y),
                    pick(2, local.min.z, local.max.z),
                );
                bounds = bounds.including(solid.pose * point);
            }
        }
        for point in
            self.traces.iter().flat_map(|t| &t.points).chain(self.waypoints.iter().map(|w| &w.position))
        {
            bounds = bounds.including(*point);
        }
        bounds
    }
}

/// Finds what a ray hits first among the solids of a scene. Each mesh's search tree is built the first
/// time it is needed and kept.
#[derive(Default)]
pub struct Picker {
    trees: std::collections::BTreeMap<u64, Arc<gc_mesh::MeshBvh>>,
    /// Triangle adjacency per mesh, for finding the face around a hit (built on the first pick of it).
    adjacency: std::collections::BTreeMap<u64, Arc<gc_mesh::region::Adjacency>>,
}

/// Where a ray meets the surface of a solid, for measuring edges and faces.
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceHit {
    /// What the solid belongs to.
    pub owner: CellItem,
    /// The hit point in the world.
    pub point: DVec3,
    /// The flat face around the hit, in the world: normal, a point of its plane, area and outline.
    pub face: Option<(DVec3, DVec3, f64, Vec<[DVec3; 2]>)>,
    /// The cylindrical surface around the hit, when it is one, in the world: centre, unit axis, radius
    /// and length.
    pub cylinder: Option<(DVec3, DVec3, f64, f64)>,
}

impl Picker {
    /// The nearest surface the ray hits, with the flat face around the hit point.
    pub fn pick_surface(&mut self, scene: &Scene, ray: &gc_math::Ray) -> Option<SurfaceHit> {
        let mut best: Option<(f64, usize, gc_mesh::RayHit)> = None;
        for (index, solid) in scene.solids.iter().enumerate() {
            if solid.owner.is_none() {
                continue;
            }
            let centre = solid.pose.translation - ray.origin;
            let along = centre.dot(ray.dir);
            if (centre - ray.dir * along).length() > solid.shape.radius() + 1e-9 {
                continue;
            }
            let key = solid.shape.key();
            let tree = match self.trees.entry(key) {
                std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::btree_map::Entry::Vacant(entry) => {
                    let Ok(mesh) = solid.shape.mesh() else { continue };
                    entry.insert(Arc::new(gc_mesh::MeshBvh::build(&mesh)))
                }
            };
            let to_local = solid.pose.inverse();
            let local = gc_math::Ray { origin: to_local * ray.origin, dir: to_local.rotation * ray.dir };
            let limit = best.as_ref().map_or(f64::INFINITY, |(t, _, _)| *t);
            if let Some(hit) = tree.raycast(&local, limit) {
                best = Some((hit.t, index, hit));
            }
        }
        let (_, index, hit) = best?;
        let solid = scene.solids.get(index)?;
        let pose = solid.pose;
        // Half a degree and ten micrometres: tessellated flat faces stay one face, curved ones split.
        let mesh = solid.shape.mesh().ok();
        let adjacency = mesh.as_ref().map(|mesh| {
            Arc::clone(
                self.adjacency
                    .entry(solid.shape.key())
                    .or_insert_with(|| Arc::new(gc_mesh::region::Adjacency::of(mesh))),
            )
        });
        let (face, cylinder) = match (mesh.as_ref(), adjacency.as_ref()) {
            (Some(mesh), Some(adjacency)) => {
                let face = gc_mesh::region::planar_region_in(
                    mesh,
                    adjacency,
                    hit.triangle,
                    0.5_f64.to_radians(),
                    1e-5,
                );
                // A smooth surface that bends less than 20° per edge and fits a circle within 1 %.
                let cylinder = gc_mesh::region::cylindrical_region_in(
                    mesh,
                    adjacency,
                    hit.triangle,
                    20_f64.to_radians(),
                    0.01,
                )
                .filter(|c| c.triangles.len() > face.as_ref().map_or(0, |f| f.triangles.len()));
                (face, cylinder)
            }
            _ => (None, None),
        };
        Some(SurfaceHit {
            owner: solid.owner?,
            point: pose * hit.point,
            face: face.map(|f| {
                let outline = f.outline.iter().map(|[a, b]| [pose * *a, pose * *b]).collect();
                (pose.rotation * f.normal, pose * f.point, f.area, outline)
            }),
            cylinder: cylinder.map(|c| (pose * c.centre, pose.rotation * c.axis, c.radius, c.length)),
        })
    }

    /// Every owner whose solid the ray passes through, nearest first, each once: what Select Other lists.
    pub fn pick_all(&mut self, scene: &Scene, ray: &gc_math::Ray) -> Vec<(f64, CellItem)> {
        let mut hits: Vec<(f64, CellItem)> = Vec::new();
        for solid in &scene.solids {
            let Some(owner) = solid.owner else { continue };
            let centre = solid.pose.translation - ray.origin;
            let along = centre.dot(ray.dir);
            if (centre - ray.dir * along).length() > solid.shape.radius() + 1e-9 {
                continue;
            }
            let key = solid.shape.key();
            let tree = match self.trees.entry(key) {
                std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::btree_map::Entry::Vacant(entry) => {
                    let Ok(mesh) = solid.shape.mesh() else { continue };
                    entry.insert(Arc::new(gc_mesh::MeshBvh::build(&mesh)))
                }
            };
            let to_local = solid.pose.inverse();
            let local = gc_math::Ray { origin: to_local * ray.origin, dir: to_local.rotation * ray.dir };
            if let Some(hit) = tree.raycast(&local, f64::INFINITY) {
                match hits.iter_mut().find(|(_, item)| *item == owner) {
                    Some(known) => known.0 = known.0.min(hit.t),
                    None => hits.push((hit.t, owner)),
                }
            }
        }
        hits.sort_by(|a, b| a.0.total_cmp(&b.0));
        hits
    }

    /// The owner of the nearest solid the ray hits, and the distance along the ray.
    pub fn pick(&mut self, scene: &Scene, ray: &gc_math::Ray) -> Option<(f64, CellItem)> {
        let mut best: Option<(f64, CellItem)> = None;
        for solid in &scene.solids {
            let Some(owner) = solid.owner else { continue };
            // Cheap test first: the ray must pass within the solid's bounding sphere.
            let centre = solid.pose.translation - ray.origin;
            let along = centre.dot(ray.dir);
            if (centre - ray.dir * along).length() > solid.shape.radius() + 1e-9 {
                continue;
            }
            let key = solid.shape.key();
            let tree = match self.trees.entry(key) {
                std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::btree_map::Entry::Vacant(entry) => {
                    let Ok(mesh) = solid.shape.mesh() else { continue };
                    entry.insert(Arc::new(gc_mesh::MeshBvh::build(&mesh)))
                }
            };
            let to_local = solid.pose.inverse();
            let local = gc_math::Ray { origin: to_local * ray.origin, dir: to_local.rotation * ray.dir };
            let limit = best.map_or(f64::INFINITY, |(t, _)| t);
            if let Some(hit) = tree.raycast(&local, limit) {
                best = Some((hit.t, owner));
            }
        }
        best
    }
}

/// Pushes the parts of a robot with its links at `poses`, coloured by `shown`.
fn push_robot(
    scene: &mut Scene,
    loaded: &gc_session::LoadedRobot,
    poses: &[DPose3],
    tool: Option<&(usize, DPose3, Shape3)>,
    owner: Option<CellItem>,
    shown: &dyn Fn([f32; 4]) -> [f32; 4],
) {
    if let Some((tip, placement, shape)) = tool
        && let Some(tip_pose) = poses.get(*tip)
    {
        let pose = *tip_pose * *placement;
        scene.solids.push(Solid3 { shape: shape.clone(), pose, color: shown(TOOL), owner });
    }
    for (index, (link, link_pose)) in loaded.model.links.iter().zip(poses).enumerate() {
        let geometries = if link.visuals.is_empty() { &link.collisions } else { &link.visuals };
        let color = if index % 2 == 0 { LINK_DARK } else { LINK_LIGHT };
        for geometry in geometries {
            let pose = *link_pose * geometry.pose;
            let shape = match &geometry.shape {
                Shape::Box(size) => Shape3::Primitive(Primitive::Box(*size)),
                Shape::Cylinder { radius, length } => {
                    Shape3::Primitive(Primitive::Cylinder { radius: *radius, length: *length })
                }
                Shape::Sphere(radius) => Shape3::Primitive(Primitive::Sphere(*radius)),
                // A mesh file is drawn part by part in the colours it gives; a file that could not be
                // supplied is listed as missing and not drawn.
                Shape::Mesh { uri, scale } => {
                    for (mesh, part_color) in loaded.mesh(uri, *scale) {
                        let color = shown(part_color.unwrap_or(color));
                        scene.solids.push(Solid3 { shape: Shape3::Mesh(mesh.clone()), pose, color, owner });
                    }
                    continue;
                }
            };
            scene.solids.push(Solid3 { shape, pose, color: shown(color), owner });
        }
    }
}

/// What a body's or a tool's solid is drawn as; `None` for a mesh file that could not be read.
fn solid_shape(session: &Session, solid: &Solid) -> Option<Shape3> {
    Some(match solid {
        Solid::Box { size } => Shape3::Primitive(Primitive::Box(*size)),
        Solid::Cylinder { radius, length } => {
            Shape3::Primitive(Primitive::Cylinder { radius: *radius, length: *length })
        }
        Solid::Sphere { radius } => Shape3::Primitive(Primitive::Sphere(*radius)),
        Solid::Mesh { .. } => Shape3::Mesh(session.body_mesh(solid)?.clone()),
    })
}

/// A robot's tool: the index of the link that carries it, its pose there, and its shape.
fn robot_tool(session: &Session, robot: gc_core::Id<gc_doc::Robot>) -> Option<(usize, DPose3, Shape3)> {
    let stored = session.doc().get(robot)?;
    let tool = session.shown_tool(robot)??;
    let tip = session.robot_model(robot)?.model.link(&stored.tip_link)?;
    let placement = pose_from_xyz_rpy(tool.placement.xyz, tool.placement.rpy);
    let tool = &tool;
    Some((tip, placement, solid_shape(session, &tool.solid)?))
}
