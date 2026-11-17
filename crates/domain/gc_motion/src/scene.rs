//! A robot in its cell: the collision model and the validity checks built on it.

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

/// Where a [`ContactAllowance`] applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContactScope {
    /// Near the waypoints of the motion: while the tool centre point is within the radius of the start
    /// or the goal.
    Waypoint,
    /// Along the whole motion.
    Segment,
}

/// Lets a robot part come closer to an obstacle than the padding where the work needs it (plan 5.7):
/// the pair is checked without padding there, and touching is refused unless `allow_touch`.
#[derive(Clone, Debug, PartialEq)]
pub struct ContactAllowance {
    /// The robot link (a tool is part of the link that carries it).
    pub body: String,
    /// The obstacle.
    pub obstacle: String,
    /// Where it applies.
    pub scope: ContactScope,
    /// For [`ContactScope::Waypoint`]: how far from a waypoint the tool centre point may be, metres
    /// (50 mm by default).
    pub radius: f64,
    /// Overlap is allowed too, not only a gap below the padding.
    pub allow_touch: bool,
}

impl ContactAllowance {
    /// The usual allowance: no padding within 50 mm of a waypoint, no touching.
    pub fn near_waypoint(body: &str, obstacle: &str) -> Self {
        Self {
            body: body.to_owned(),
            obstacle: obstacle.to_owned(),
            scope: ContactScope::Waypoint,
            radius: 0.05,
            allow_touch: false,
        }
    }
}

/// A contact allowance with its bodies found and its waypoints placed.
#[derive(Clone, Debug)]
struct Contact {
    link: BodyId,
    obstacle: BodyId,
    /// `None`: everywhere. Otherwise the points the tool centre point must be near, and how near.
    near: Option<(Vec<gc_math::DVec3>, f64)>,
    allow_touch: bool,
    /// The pair was excluded from checking anyway.
    excluded: bool,
}

/// A robot placed in a cell with obstacles.
#[derive(Clone)]
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
    /// The tool's body (placed with the tip link), when there is a tool.
    tool_body: Option<BodyId>,
    /// Contact allowances in force.
    contacts: Vec<Contact>,
    /// Link pairs excluded from checking by default, and why.
    exclusions: Vec<(BodyId, BodyId, PairReason)>,
}

/// Why a pair of robot links is or is not checked by default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PairReason {
    /// Joined by a joint: not checked.
    Adjacent,
    /// Touching in the zero configuration, so designed to: not checked.
    TouchingAtZero,
    /// Checked.
    Checked,
}

/// A pair of robot links and how it is checked ([`RobotScene::link_pairs`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkPair {
    /// One link.
    pub a: String,
    /// The other link (`a < b`).
    pub b: String,
    /// What the description implies.
    pub reason: PairReason,
    /// Whether the pair is checked now, after the user's choices.
    pub checked: bool,
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
            // The tool moves with the link that carries it: it counts for how far the link sweeps.
            let carried = if index == tip_link { tool } else { &[] };
            link_radius.push(
                shapes
                    .iter()
                    .chain(carried)
                    .map(|(pose, shape)| pose.translation.length() + shape.radius())
                    .fold(0.0, f64::max),
            );
            link_bodies.push(if shapes.is_empty() {
                None
            } else {
                Some(world.add_body(&link.name, true, padding, &shapes)?)
            });
        }
        let mut exclusions = Vec::new();
        for joint in &model.joints {
            if let (Some(a), Some(b)) = (link_bodies[joint.parent], link_bodies[joint.child]) {
                world.allow(a, b);
                exclusions.push((a, b, PairReason::Adjacent));
            }
        }
        // The tool is a body of its own, carried by the tip link. It is never checked against the tip
        // link and the nearest link with geometry above it (it is mounted there), and checked against
        // every other link: a tool that reaches back along the arm collides with it.
        let tool_body = if tool.is_empty() {
            None
        } else {
            let body = world.add_body("tool", true, padding, tool)?;
            let mut at = Some(tip_link);
            let mut mounted = Vec::new();
            for _ in 0..=model.joints.len() {
                let Some(link) = at else { break };
                if let Some(carrier) = link_bodies[link] {
                    mounted.push(carrier);
                    if link != tip_link || mounted.len() == 2 {
                        break;
                    }
                }
                at = model.joints.iter().find(|j| j.child == link).map(|j| j.parent);
            }
            for carrier in mounted {
                world.allow(body, carrier);
                exclusions.push((body, carrier, PairReason::Adjacent));
            }
            Some(body)
        };
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
                    .filter(|&link| link_bodies[link].is_some() || (link == tip_link && tool_body.is_some()))
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
            contacts: Vec::new(),
            exclusions,
            tool_body,
        };
        // Links that touch in the zero configuration are designed to: they are not checked either.
        scene.place(&vec![0.0; scene.chain.dof()]);
        let bodies: Vec<BodyId> = scene.link_bodies.iter().flatten().copied().collect();
        for (i, a) in bodies.iter().enumerate() {
            for b in &bodies[i + 1..] {
                if !scene.world.is_allowed(*a, *b) && scene.world.proximity(*a, *b)? == Proximity::Colliding {
                    scene.world.allow(*a, *b);
                    scene.exclusions.push((*a, *b, PairReason::TouchingAtZero));
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
        let poses = self.link_poses(q);
        self.place_poses(world, &poses);
    }

    fn place_poses(&self, world: &mut World, poses: &[DPose3]) {
        for (pose, body) in poses.iter().zip(&self.link_bodies) {
            if let Some(body) = body {
                world.set_pose(*body, *pose);
            }
        }
        if let (Some(tool), Some(carrier)) = (self.tool_body, poses.get(self.tip_link)) {
            world.set_pose(tool, *carrier);
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
        let poses = self.link_poses(q);
        self.place_poses(world, &poses);
        if !self.contacts.is_empty() {
            let tip = (poses[self.tip_link] * self.tool).translation;
            for contact in &self.contacts {
                let active = contact
                    .near
                    .as_ref()
                    .is_none_or(|(points, radius)| points.iter().any(|p| (*p - tip).length() <= *radius));
                match (active, contact.allow_touch) {
                    (true, true) => world.allow(contact.link, contact.obstacle),
                    (true, false) => world.set_pair_padding(contact.link, contact.obstacle, Some(0.0)),
                    (false, _) => {
                        world.set_pair_padding(contact.link, contact.obstacle, None);
                        if !contact.excluded {
                            world.disallow(contact.link, contact.obstacle);
                        }
                    }
                }
            }
        }
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

    /// The scene with contact allowances in force for a motion whose waypoints put the tool centre point
    /// at `waypoints` (world positions). Returns the allowances naming a body or obstacle the scene does
    /// not have; they are left out.
    #[must_use]
    pub fn with_contacts(
        mut self,
        allowances: &[ContactAllowance],
        waypoints: &[gc_math::DVec3],
    ) -> (Self, Vec<ContactAllowance>) {
        let mut unknown = Vec::new();
        self.contacts.clear();
        for allowance in allowances {
            let robot = |b: &BodyId| self.link_bodies.contains(&Some(*b)) || self.tool_body == Some(*b);
            let link = self.world.find(&allowance.body).filter(robot);
            let obstacle = self.world.find(&allowance.obstacle).filter(|b| !robot(b));
            let (Some(link), Some(obstacle)) = (link, obstacle) else {
                unknown.push(allowance.clone());
                continue;
            };
            let near = match allowance.scope {
                ContactScope::Segment => None,
                ContactScope::Waypoint => Some((waypoints.to_vec(), allowance.radius.max(0.0))),
            };
            // The tip link's allowance covers the tool it carries.
            let carried = (self.link_bodies.get(self.tip_link) == Some(&Some(link)))
                .then_some(self.tool_body)
                .flatten();
            for body in std::iter::once(link).chain(carried) {
                let excluded = self.world.is_allowed(body, obstacle);
                self.contacts.push(Contact {
                    link: body,
                    obstacle,
                    near: near.clone(),
                    allow_touch: allowance.allow_touch,
                    excluded,
                });
            }
        }
        (self, unknown)
    }

    /// Every pair of robot links with collision geometry: why it is or is not checked by default, and
    /// whether it is checked now. Sorted by name.
    pub fn link_pairs(&self) -> Vec<LinkPair> {
        let bodies: Vec<BodyId> = self.link_bodies.iter().flatten().copied().collect();
        let mut pairs = Vec::new();
        for (i, a) in bodies.iter().enumerate() {
            for b in &bodies[i + 1..] {
                let reason = self
                    .exclusions
                    .iter()
                    .find(|(x, y, _)| (x == a && y == b) || (x == b && y == a))
                    .map_or(PairReason::Checked, |(_, _, reason)| *reason);
                let (na, nb) = (self.world.name(*a).to_owned(), self.world.name(*b).to_owned());
                let (na, nb) = if na <= nb { (na, nb) } else { (nb, na) };
                pairs.push(LinkPair { a: na, b: nb, reason, checked: !self.world.is_allowed(*a, *b) });
            }
        }
        pairs.sort_by(|x, y| (&x.a, &x.b).cmp(&(&y.a, &y.b)));
        pairs
    }

    /// The scene with the user's choices for link pairs: `(a, b, true)` checks a pair excluded by default,
    /// `(a, b, false)` stops checking one. Names that are not links with geometry are ignored.
    #[must_use]
    pub fn with_pair_choices(mut self, choices: &[(String, String, bool)]) -> Self {
        let link = |scene: &Self, name: &str| {
            scene.world.find(name).filter(|body| scene.link_bodies.contains(&Some(*body)))
        };
        for (a, b, check) in choices {
            if let (Some(x), Some(y)) = (link(&self, a), link(&self, b))
                && x != y
            {
                if *check {
                    self.world.disallow(x, y);
                } else {
                    self.world.allow(x, y);
                }
            }
        }
        self
    }

    /// True when contact allowances are in force: some pairs are then checked without padding somewhere.
    pub fn has_contacts(&self) -> bool {
        !self.contacts.is_empty()
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
        Checker { scene: self, world: std::sync::Mutex::new(world), margin, resolution }
    }
}

/// Answers the planners' validity questions for a [`RobotScene`].
pub struct Checker<'a> {
    scene: &'a RobotScene,
    world: std::sync::Mutex<World>,
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
        // A poisoned lock means a check panicked: nothing is valid then.
        self.world.lock().is_ok_and(|mut world| self.scene.problem_in(&mut world, q).is_none())
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
