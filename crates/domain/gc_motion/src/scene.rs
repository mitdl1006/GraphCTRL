//! A robot in its cell: the collision model and the validity checks built on it.

use std::cell::RefCell;
use std::sync::Arc;

use gc_collide::{BodyId, CollideError, Proximity, ShapeDesc, World, hulls_of};
use gc_math::DPose3;
use gc_mesh::MeshData;
use gc_plan::Validity;
use gc_robot::{Chain, RobotModel, Shape};

/// A static object in the cell.
#[derive(Clone, Debug)]
pub struct Obstacle {
    /// Name used in messages.
    pub name: String,
    /// Pose in the world.
    pub pose: DPose3,
    /// Shape.
    pub shape: ShapeDesc,
}

/// Why a configuration is not allowed.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum StateProblem {
    /// A joint is outside its limits.
    #[error("joint `{joint}` is outside its limits")]
    JointLimit {
        /// The joint.
        joint: String,
    },
    /// Two bodies overlap.
    #[error("`{a}` collides with `{b}`")]
    Collision {
        /// First body.
        a: String,
        /// Second body.
        b: String,
    },
    /// A robot part is closer to an obstacle than the safety padding allows.
    #[error("`{a}` is {gap:.4} m from `{b}`, closer than the padding")]
    WithinPadding {
        /// The robot part.
        a: String,
        /// The obstacle.
        b: String,
        /// The remaining gap in metres.
        gap: f64,
    },
    /// The collision check itself failed; the state is treated as not allowed.
    #[error("the state could not be checked")]
    Unknown,
}

/// A robot placed in a cell with obstacles.
pub struct RobotScene {
    model: Arc<RobotModel>,
    chain: Chain,
    base: DPose3,
    world: World,
    /// The distance robot links keep from obstacles.
    padding: f64,
    /// The collision body of each link, where the link has collision geometry.
    link_bodies: Vec<Option<BodyId>>,
    tip_link: usize,
    /// The tool centre point in the tip link's frame.
    tool: DPose3,
    /// Largest joint step between checked states of a motion, before refinement.
    resolution: f64,
    /// Per link: the largest distance of a point of its collision body from the link's origin (0 without
    /// a body).
    link_radius: Vec<f64>,
    /// Per chain coordinate: the model joint, whether it slides, and the links (with bodies) it moves.
    movers: Vec<(usize, bool, Vec<usize>)>,
}

impl RobotScene {
    /// Builds the scene.
    ///
    /// - `base` is the pose of the robot's root link in the world.
    /// - `padding` is the distance robot links keep from obstacles.
    /// - `meshes` supplies the mesh a URDF `filename` refers to; links whose mesh is not supplied get no
    ///   collision geometry for it, and its name is returned in the list of missing meshes.
    /// - `tool` lists shapes, posed in the tip link's frame, that the tip link carries (convex: give a
    ///   mesh as its hulls).
    ///
    /// Link pairs joined by a joint, and pairs that already touch in the zero configuration, are never
    /// checked against each other.
    pub fn new(
        model: Arc<RobotModel>,
        chain: Chain,
        tip_link: &str,
        base: DPose3,
        obstacles: &[Obstacle],
        padding: f64,
        meshes: &dyn Fn(&str) -> Option<Arc<MeshData>>,
        tool: &[(DPose3, ShapeDesc)],
    ) -> Result<(Self, Vec<String>), CollideError> {
        let mut world = World::new();
        let tip_link = model.link(tip_link).unwrap_or(model.root);
        let mut missing = Vec::new();
        let mut link_bodies = Vec::with_capacity(model.links.len());
        let mut link_radius = Vec::with_capacity(model.links.len());
        for (index, link) in model.links.iter().enumerate() {
            let mut shapes = Vec::new();
            // A tool is part of the link that carries it.
            if index == tip_link {
                shapes.extend(tool.iter().cloned());
            }
            for geometry in &link.collisions {
                match &geometry.shape {
                    Shape::Box(size) => shapes.push((geometry.pose, ShapeDesc::Box(*size))),
                    Shape::Sphere(radius) => shapes.push((geometry.pose, ShapeDesc::Ball(*radius))),
                    Shape::Cylinder { radius, length } => {
                        shapes
                            .push((geometry.pose, ShapeDesc::Cylinder { radius: *radius, length: *length }));
                    }
                    Shape::Mesh { uri, scale } => match meshes(uri) {
                        Some(mesh) => {
                            // Collision meshes are small; scaling a copy keeps the stored asset untouched.
                            #[allow(clippy::cast_possible_truncation, reason = "mesh coordinates are f32")]
                            let factors = scale.map(|f| f as f32);
                            let positions = mesh
                                .positions()
                                .iter()
                                .map(|p| [p[0] * factors[0], p[1] * factors[1], p[2] * factors[2]])
                                .collect();
                            match MeshData::new(positions, mesh.indices().to_vec()) {
                                Ok(scaled) => {
                                    shapes.extend(
                                        hulls_of(&scaled).into_iter().map(|hull| (geometry.pose, hull)),
                                    );
                                }
                                Err(_unusable) => missing.push(uri.clone()),
                            }
                        }
                        None => missing.push(uri.clone()),
                    },
                }
            }
            link_radius.push(
                shapes
                    .iter()
                    .map(|(pose, shape)| pose.translation.length() + shape.radius())
                    .fold(0.0, f64::max),
            );
            link_bodies.push(if shapes.is_empty() {
                None
            } else {
                Some(world.add_body(&link.name, true, padding, &shapes)?)
            });
        }
        for joint in &model.joints {
            if let (Some(a), Some(b)) = (link_bodies[joint.parent], link_bodies[joint.child]) {
                world.allow(a, b);
            }
        }
        // Links that no chain joint moves (the base, typically) keep their distance to every obstacle
        // whatever the configuration: checking them tells nothing about a motion, and a base standing on
        // a table would otherwise make every configuration invalid.
        let static_links: Vec<BodyId> = {
            let mut still = Vec::new();
            let mut moved = Vec::new();
            model.link_poses(&chain.expand(&vec![0.0; chain.dof()]), &mut still);
            model.link_poses(&chain.expand(&vec![0.7; chain.dof()]), &mut moved);
            link_bodies
                .iter()
                .zip(still.iter().zip(&moved))
                .filter_map(|(body, (a, b))| body.filter(|_| a == b))
                .collect()
        };
        for obstacle in obstacles {
            let body =
                world.add_body(&obstacle.name, false, 0.0, &[(DPose3::IDENTITY, obstacle.shape.clone())])?;
            world.set_pose(body, obstacle.pose);
            for link in &static_links {
                world.allow(*link, body);
            }
        }
        // The links each chain joint moves: those below its child link in the tree.
        let parent_joint: Vec<Option<usize>> = (0..model.links.len())
            .map(|link| model.joints.iter().position(|joint| joint.child == link))
            .collect();
        let movers = chain
            .active_joints()
            .iter()
            .map(|&joint| {
                let below = (0..model.links.len())
                    .filter(|&link| link_bodies[link].is_some())
                    .filter(|&link| {
                        let mut at = link;
                        // Walk up the tree (at most once per joint, so a malformed tree cannot loop).
                        for _ in 0..=model.joints.len() {
                            match parent_joint[at] {
                                Some(j) if j == joint => return true,
                                Some(j) => at = model.joints[j].parent,
                                None => return false,
                            }
                        }
                        false
                    })
                    .collect();
                (joint, model.joints[joint].kind == gc_robot::JointKind::Prismatic, below)
            })
            .collect();
        let tool = DPose3::IDENTITY;
        let mut scene = Self {
            model,
            chain,
            base,
            world,
            link_bodies,
            tip_link,
            tool,
            resolution: 0.02,
            padding,
            link_radius,
            movers,
        };
        // Links that touch in the zero configuration are designed to: they are not checked either.
        scene.place(&vec![0.0; scene.chain.dof()]);
        let bodies: Vec<BodyId> = scene.link_bodies.iter().flatten().copied().collect();
        for (i, a) in bodies.iter().enumerate() {
            for b in &bodies[i + 1..] {
                if !scene.world.is_allowed(*a, *b) && scene.world.proximity(*a, *b)? == Proximity::Colliding {
                    scene.world.allow(*a, *b);
                }
            }
        }
        Ok((scene, missing))
    }

    /// The planning chain.
    pub fn chain(&self) -> &Chain {
        &self.chain
    }

    /// The pose of the robot's root link in the world.
    pub fn base(&self) -> &DPose3 {
        &self.base
    }

    /// The robot model.
    pub fn model(&self) -> &Arc<RobotModel> {
        &self.model
    }

    /// World poses of all links for a chain configuration.
    pub fn link_poses(&self, q: &[f64]) -> Vec<DPose3> {
        let mut poses = Vec::new();
        self.model.link_poses(&self.chain.expand(q), &mut poses);
        poses.iter().map(|pose| self.base * *pose).collect()
    }

    /// Sets the tool centre point: its pose in the tip link's frame. Goals and paths are about this
    /// point; without a tool it is the tip link itself.
    #[must_use]
    pub fn with_tool(mut self, tool: DPose3) -> Self {
        self.tool = tool;
        self
    }

    /// World pose of the tool centre point.
    pub fn tip_in_world(&self, q: &[f64]) -> DPose3 {
        self.link_poses(q)[self.tip_link] * self.tool
    }

    /// The pose the tip link must take, in the robot's base frame, for the tool centre point to be at
    /// `world`.
    pub fn flange_target(&self, world: &DPose3) -> DPose3 {
        self.base.inverse() * *world * self.tool.inverse()
    }

    fn place_in(&self, world: &mut World, q: &[f64]) {
        for (pose, body) in self.link_poses(q).iter().zip(&self.link_bodies) {
            if let Some(body) = body {
                world.set_pose(*body, *pose);
            }
        }
    }

    fn place(&mut self, q: &[f64]) {
        let mut world = std::mem::take(&mut self.world);
        self.place_in(&mut world, q);
        self.world = world;
    }

    fn problem_in(&self, world: &mut World, q: &[f64]) -> Option<StateProblem> {
        if q.len() != self.chain.dof() {
            return Some(StateProblem::Unknown);
        }
        let limits = self.chain.lower().iter().zip(self.chain.upper());
        if let Some(joint) =
            q.iter().zip(limits).position(|(v, (lo, hi))| !(*v >= lo - 1e-9 && *v <= hi + 1e-9))
        {
            return Some(StateProblem::JointLimit { joint: self.chain.joint_names()[joint].clone() });
        }
        self.place_in(world, q);
        match world.first_hit() {
            Ok(None) => None,
            Ok(Some(hit)) => {
                let (a, b) = (world.name(hit.a).to_owned(), world.name(hit.b).to_owned());
                Some(match hit.proximity {
                    Proximity::WithinPadding(gap) => StateProblem::WithinPadding { a, b, gap },
                    _ => StateProblem::Collision { a, b },
                })
            }
            Err(_) => Some(StateProblem::Unknown),
        }
    }

    /// Why configuration `q` is not allowed, or `None` when it is.
    pub fn state_problem(&self, q: &[f64]) -> Option<StateProblem> {
        self.problem_in(&mut self.world.clone(), q)
    }

    /// The smallest distance between any checked pair of bodies at `q` (0 when colliding).
    pub fn clearance(&self, q: &[f64]) -> f64 {
        let mut world = self.world.clone();
        self.place_in(&mut world, q);
        world.clearance().unwrap_or(0.0)
    }

    /// The distance robot links keep from obstacles.
    pub fn padding(&self) -> f64 {
        self.padding
    }

    /// An upper bound of how far any point of a robot body moves on the joint-space segment from `a` to
    /// `b` (metres): `Σ_j r_j·|Δq_j|`, where `r_j` is the largest distance from joint `j` to a point of a
    /// body it moves, taken at both ends (a sliding joint moves everything below it by `|Δq_j|`).
    pub fn sweep_bound(&self, a: &[f64], b: &[f64]) -> f64 {
        let ends = [self.link_poses(a), self.link_poses(b)];
        let mut total = 0.0;
        for (k, (joint, slides, below)) in self.movers.iter().enumerate() {
            let delta = (b.get(k).copied().unwrap_or(0.0) - a.get(k).copied().unwrap_or(0.0)).abs();
            if *slides {
                total += delta;
                continue;
            }
            let pivot_link = self.model.joints[*joint].child;
            let radius = ends
                .iter()
                .flat_map(|poses| {
                    let pivot = poses[pivot_link].translation;
                    below.iter().map(move |link| {
                        (poses[*link].translation - pivot).length() + self.link_radius[*link]
                    })
                })
                .fold(0.0, f64::max);
            total += radius * delta;
        }
        total
    }

    /// The body pairs that are never checked (neighbouring links, links that touch in the zero
    /// configuration, the base against obstacles, ...), by name.
    pub fn excluded_pairs(&self) -> Vec<(String, String)> {
        self.world.allowed_pairs()
    }

    /// A fingerprint of [`RobotScene::excluded_pairs`] (FNV-1a), recorded in a [`crate::Guarantee`].
    pub fn excluded_pairs_hash(&self) -> u64 {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for (a, b) in self.excluded_pairs() {
            for byte in a.bytes().chain([0]).chain(b.bytes()).chain([0]) {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x0100_0000_01b3);
            }
        }
        hash
    }

    /// A validity checker for planning at a fraction of the normal checking step (`fineness` 1 is the
    /// normal step, 0.25 a quarter of it). It owns a working copy of the collision world.
    ///
    /// The checker keeps more than the required padding: the distance a link can travel in one checking
    /// step. A path that is valid for it therefore still keeps the required padding between the states it
    /// checked, and a finer checker needs less extra room.
    pub fn checker(&self, fineness: f64) -> Checker<'_> {
        let resolution = self.resolution * fineness;
        self.checker_with(resolution * self.chain.reach(), resolution)
    }

    /// A checker for the required padding alone, used for the final check of a timed motion.
    pub fn strict_checker(&self) -> Checker<'_> {
        self.checker_with(0.0, self.resolution)
    }

    fn checker_with(&self, margin: f64, resolution: f64) -> Checker<'_> {
        let mut world = self.world.clone();
        for body in self.link_bodies.iter().flatten() {
            world.set_padding(*body, self.padding + margin);
        }
        Checker { scene: self, world: RefCell::new(world), margin, resolution }
    }
}

/// Answers the planners' validity questions for a [`RobotScene`].
pub struct Checker<'a> {
    scene: &'a RobotScene,
    world: RefCell<World>,
    margin: f64,
    /// Largest joint step between checked states of a motion.
    resolution: f64,
}

impl Checker<'_> {
    /// The padding this checker keeps on top of the required one.
    pub fn margin(&self) -> f64 {
        self.margin
    }
}

impl Validity for Checker<'_> {
    fn state_valid(&self, q: &[f64]) -> bool {
        self.scene.problem_in(&mut self.world.borrow_mut(), q).is_none()
    }

    fn motion_valid(&self, a: &[f64], b: &[f64]) -> bool {
        let pieces = self.motion_cost(a, b);
        // The ends are known to be valid; check the states in between.
        (1..pieces).all(|k| {
            let t = k as f64 / pieces as f64;
            let q: Vec<f64> = a.iter().zip(b).map(|(x, y)| x + (y - x) * t).collect();
            self.state_valid(&q)
        })
    }

    fn motion_cost(&self, a: &[f64], b: &[f64]) -> u64 {
        let largest = a.iter().zip(b).fold(0.0_f64, |acc, (x, y)| acc.max((y - x).abs()));
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "a small positive count")]
        let pieces = (largest / self.resolution).ceil().clamp(1.0, 1e6) as u64;
        pieces
    }
}
