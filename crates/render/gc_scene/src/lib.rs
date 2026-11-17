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
use gc_session::{MeshRef, PlanStatus, Session};

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
}

/// The tool-tip trace of a planned path.
#[derive(Clone, Debug, PartialEq)]
pub struct Trace {
    /// World positions along the motion.
    pub points: Vec<DVec3>,
    /// False when the plan is out of date.
    pub current: bool,
}

/// A named place in the world.
#[derive(Clone, Debug, PartialEq)]
pub struct Marker {
    /// The name shown beside it.
    pub name: String,
    /// Where it is.
    pub position: DVec3,
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
}

const BODY: [f32; 4] = [0.55, 0.62, 0.70, 1.0];
const LINK_LIGHT: [f32; 4] = [0.90, 0.47, 0.13, 1.0];
const LINK_DARK: [f32; 4] = [0.27, 0.29, 0.32, 1.0];

impl Scene {
    /// Builds the scene of a session as it is shown right now (jog preview and playback included).
    pub fn of(session: &Session) -> Self {
        let doc = session.doc();
        let mut scene = Self::default();
        for (id, _) in doc.bodies().iter() {
            let Some(body) = session.shown_body(id) else { continue };
            let shape = match body.solid {
                Solid::Box { size } => Primitive::Box(size),
                Solid::Cylinder { radius, length } => Primitive::Cylinder { radius, length },
                Solid::Sphere { radius } => Primitive::Sphere(radius),
            };
            let pose = pose_from_xyz_rpy(body.placement.xyz, body.placement.rpy);
            scene.solids.push(Solid3 { shape: Shape3::Primitive(shape), pose, color: BODY });
        }
        for (id, _) in doc.robots().iter() {
            let (Some(loaded), Some(poses)) = (session.robot_model(id), session.link_poses(id)) else {
                continue;
            };
            for (index, (link, link_pose)) in loaded.model.links.iter().zip(&poses).enumerate() {
                let geometries = if link.visuals.is_empty() { &link.collisions } else { &link.visuals };
                let color = if index % 2 == 0 { LINK_DARK } else { LINK_LIGHT };
                for geometry in geometries {
                    let shape = match &geometry.shape {
                        Shape::Box(size) => Shape3::Primitive(Primitive::Box(*size)),
                        Shape::Cylinder { radius, length } => {
                            Shape3::Primitive(Primitive::Cylinder { radius: *radius, length: *length })
                        }
                        Shape::Sphere(radius) => Shape3::Primitive(Primitive::Sphere(*radius)),
                        // A mesh file that could not be supplied is listed as missing and not drawn.
                        Shape::Mesh { uri, scale } => match loaded.mesh(uri, *scale) {
                            Some(mesh) => Shape3::Mesh(mesh.clone()),
                            None => continue,
                        },
                    };
                    scene.solids.push(Solid3 { shape, pose: *link_pose * geometry.pose, color });
                }
            }
        }
        for (id, _) in doc.paths().iter() {
            if let Some(planned) = session.planned(id) {
                scene.traces.push(Trace {
                    points: planned.tip_path().map(DVec3::from_array).collect(),
                    current: session.plan_status(id) == PlanStatus::Ready,
                });
            }
        }
        for (id, waypoint) in doc.waypoints().iter() {
            if let Some(pose) = session.waypoint_pose(id) {
                scene.waypoints.push(Marker { name: waypoint.name.clone(), position: pose.translation });
            }
        }
        scene
    }

    /// A box around everything in the scene; empty when there is nothing.
    pub fn bounds(&self) -> Aabb {
        let mut bounds = Aabb::EMPTY;
        for solid in &self.solids {
            let reach = DVec3::splat(solid.shape.radius());
            bounds =
                bounds.including(solid.pose.translation - reach).including(solid.pose.translation + reach);
        }
        for point in
            self.traces.iter().flat_map(|t| &t.points).chain(self.waypoints.iter().map(|w| &w.position))
        {
            bounds = bounds.including(*point);
        }
        bounds
    }
}
