//! The robot cell: bodies, robots, waypoints and paths, and the edits that change them.

use gc_core::{AnyId, Entity, Id, Kind};
use gc_sketch::{SkPoint, Sketch};
use serde::{Deserialize, Serialize};

use crate::command::{Created, EditError};
use crate::txn::{DocError, Txn};

/// A pose in the world: position in metres and roll, pitch, yaw in radians (the URDF convention).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    /// Position.
    pub xyz: [f64; 3],
    /// Roll, pitch, yaw.
    pub rpy: [f64; 3],
}

/// The shape of a body, in its own frame.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Solid {
    /// A mesh file (STL). The document stores where the file is, not its triangles.
    Mesh {
        /// Path of the file.
        file: String,
        /// Factor applied to the file's coordinates to get metres (0.001 for a file in millimetres).
        scale: f64,
    },
    /// A box with these edge lengths.
    Box {
        /// Edge lengths along X, Y, Z.
        size: [f64; 3],
    },
    /// A cylinder along Z.
    Cylinder {
        /// Radius.
        radius: f64,
        /// Length.
        length: f64,
    },
    /// A sphere.
    Sphere {
        /// Radius.
        radius: f64,
    },
}

impl Solid {
    /// True when every size is positive and finite.
    pub fn is_valid(&self) -> bool {
        let ok = |v: f64| v.is_finite() && v > 0.0;
        match self {
            Self::Mesh { file, scale } => !file.is_empty() && ok(*scale),
            Self::Box { size } => size.iter().copied().all(ok),
            Self::Cylinder { radius, length } => ok(*radius) && ok(*length),
            Self::Sphere { radius } => ok(*radius),
        }
    }
}

/// How a body or a robot is shown. It changes nothing else: a hidden or see-through body is still an
/// obstacle for planning.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Look {
    /// Not drawn at all.
    pub hidden: bool,
    /// From 0 (opaque) to 0.95 (nearly invisible).
    pub transparency: f64,
}

impl Look {
    /// True when the values are in range.
    pub fn is_valid(&self) -> bool {
        (0.0..=0.95).contains(&self.transparency)
    }
}

/// A piece of equipment or an obstacle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Body {
    /// Name shown in the browser.
    pub name: String,
    /// Where it stands.
    pub placement: Placement,
    /// Its shape.
    pub solid: Solid,
    /// How it is shown.
    #[serde(default)]
    pub look: Look,
}

impl Entity for Body {
    const KIND: Kind = Kind::Body;
    fn refs(&self, _: &mut Vec<AnyId>) {}
}

/// A link pair of a robot whose collision check the user chose.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcmOverride {
    /// One link, by name.
    pub a: String,
    /// The other link.
    pub b: String,
    /// True: check the pair even though it is excluded by default. False: never check it.
    pub check: bool,
}

/// Lets a robot part come closer to an obstacle than the padding (plan 5.7, contact allowance).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContactAllowance {
    /// Position in the path of the waypoint whose motion it applies to.
    pub index: usize,
    /// The robot link (a tool is part of the link that carries it), by name.
    pub link: String,
    /// The obstacle. A body that no longer exists makes the allowance void.
    pub obstacle: Id<Body>,
    /// True: along the whole motion. False: only near its start and end (within `radius`).
    pub whole_motion: bool,
    /// How near the start or end the tool centre point must be, metres.
    pub radius: f64,
    /// Overlap is allowed too, not only a gap below the padding.
    pub allow_touch: bool,
}

/// A robot placed in the cell.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Robot {
    /// Name shown in the browser.
    pub name: String,
    /// The robot description (URDF text). Kept in the document so that a file opens anywhere.
    pub urdf: String,
    /// The folder the description was read from, where the mesh files it refers to are looked for.
    /// Empty for robots from the library.
    #[serde(default)]
    pub resource_dir: String,
    /// The link the planning chain starts at.
    pub base_link: String,
    /// The link the planning chain ends at (the tool flange).
    pub tip_link: String,
    /// Where the robot's root link stands.
    pub placement: Placement,
    /// The tool centre point: its pose in the tip link's frame. Waypoints and paths are about this
    /// point. The default is the tip link itself (no tool).
    #[serde(default)]
    pub tcp: Placement,
    /// The current joint values of the planning chain.
    pub joints: Vec<f64>,
    /// Acceleration limit applied to every joint, rad/s² (robot descriptions carry none).
    pub max_acc: f64,
    /// How it is shown.
    #[serde(default)]
    pub look: Look,
    /// What it carries on its tip link, if anything (boxed: most robots carry nothing).
    #[serde(default)]
    pub tool: Option<Box<Tool>>,
    /// Link pairs whose collision check the user switched on or off, against what is derived from the
    /// description (neighbouring links, links touching in the zero pose, ...).
    #[serde(default)]
    pub acm_overrides: Vec<AcmOverride>,
    /// The link tree as plain data (from the description), so that the document can place every link's
    /// frame for the robot's joint values without reading the description: links as sub-components with
    /// their own origin folders. Filled when the robot is added or the project is opened.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<RobotLink>,
}

/// A link of a robot's tree, parents before children.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RobotLink {
    /// Name from the description.
    pub name: String,
    /// The parent link (index into [`Robot::links`], always lower); `None` for the root, which stands at
    /// the robot's placement.
    pub parent: Option<u32>,
    /// The joint frame in the parent link's frame.
    pub origin: Placement,
    /// Unit axis of the joint's motion in the joint frame.
    pub axis: [f64; 3],
    /// How the joint moves with the robot's joint values.
    pub motion: LinkMotion,
}

/// How a link's joint follows the robot's joint values ([`Robot::joints`]).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum LinkMotion {
    /// It does not move (fixed joints, and joints outside the planning chain, at zero).
    Fixed,
    /// It turns about the axis by `scale * joints[joint] + offset` radians.
    Turn {
        /// Index into [`Robot::joints`].
        joint: u32,
        /// Multiplier (1 unless the joint copies another).
        scale: f64,
        /// Offset.
        offset: f64,
    },
    /// It slides along the axis by `scale * joints[joint] + offset` metres.
    Slide {
        /// Index into [`Robot::joints`].
        joint: u32,
        /// Multiplier.
        scale: f64,
        /// Offset.
        offset: f64,
    },
}

impl Robot {
    /// The pose of every link in the robot's own frame (its placement) for the stored joint values;
    /// empty when the link tree is not known.
    pub fn link_frames(&self) -> Vec<gc_math::DPose3> {
        use gc_math::{DPose3, DVec3};
        let mut out: Vec<DPose3> = Vec::with_capacity(self.links.len());
        for link in &self.links {
            let value = |joint: u32, scale: f64, offset: f64| {
                scale * self.joints.get(joint as usize).copied().unwrap_or(0.0) + offset
            };
            let axis = DVec3::from_array(self.axis_of(link));
            let motion = match link.motion {
                LinkMotion::Fixed => DPose3::IDENTITY,
                LinkMotion::Turn { joint, scale, offset } => DPose3::from_parts(
                    DVec3::ZERO,
                    gc_math::quat_from_rotvec(axis * value(joint, scale, offset)),
                ),
                LinkMotion::Slide { joint, scale, offset } => {
                    DPose3::from_parts(axis * value(joint, scale, offset), gc_math::DQuat::IDENTITY)
                }
            };
            let local = gc_math::pose_from_xyz_rpy(link.origin.xyz, link.origin.rpy) * motion;
            let parent = link.parent.and_then(|p| out.get(p as usize).copied());
            out.push(parent.map_or(local, |p| p * local));
        }
        out
    }

    fn axis_of(&self, link: &RobotLink) -> [f64; 3] {
        let a = gc_math::DVec3::from_array(link.axis);
        if a.length() > 1e-12 { a.normalize().to_array() } else { [0.0, 0.0, 1.0] }
    }
}

/// Something a robot carries on its tip link (a gripper, a torch): drawn with the robot and checked for
/// collisions as part of it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tool {
    /// Its shape.
    pub solid: Solid,
    /// Where it sits in the tip link's frame.
    pub placement: Placement,
}

impl Tool {
    /// True when the shape has positive sizes and the placement is finite.
    pub fn is_valid(&self) -> bool {
        self.solid.is_valid() && finite(&self.placement.xyz) && finite(&self.placement.rpy)
    }
}

impl Entity for Robot {
    const KIND: Kind = Kind::RobotInstance;
    fn refs(&self, _: &mut Vec<AnyId>) {}
}

/// A work coordinate frame defined by up to three points of a sketch, as robot controllers define user
/// frames: the origin; a point the X axis points to; a point on the positive-Y side of the XY plane.
/// Without the X point the axes are the sketch's; without the XY point the Z axis is the sketch's normal
/// (in a 3D sketch, world Z). The axes found are then turned by `rpy`. Waypoints defined in the frame move
/// and turn with it when the sketch changes; the points are meant to be fully constrained, so that the
/// frame is where the dimensions say.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkFrame {
    /// Name shown in the browser.
    pub name: String,
    /// The sketch the origin point is in.
    pub sketch: Id<Sketch>,
    /// The origin.
    pub point: Id<SkPoint>,
    /// The point the X axis points to.
    #[serde(default)]
    pub x_point: Option<Id<SkPoint>>,
    /// A point on the positive-Y side of the frame's XY plane.
    #[serde(default)]
    pub xy_point: Option<Id<SkPoint>>,
    /// Roll, pitch, yaw applied on top of the axes the points give.
    pub rpy: [f64; 3],
}

impl Entity for WorkFrame {
    const KIND: Kind = Kind::WorkFrame;
    fn refs(&self, out: &mut Vec<AnyId>) {
        out.push(self.sketch.erase());
    }
}

/// Where a waypoint is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum WaypointTarget {
    /// A joint configuration.
    Joints(Vec<f64>),
    /// A sketch point: the tool tip goes there, pointing against the sketch plane's normal. The waypoint
    /// follows the point when dimensions change.
    SketchPoint {
        /// The sketch.
        sketch: Id<Sketch>,
        /// The point in it.
        point: Id<SkPoint>,
    },
    /// A pose of the tool centre point given in a work frame. When the frame's origin moves, the
    /// waypoint moves with it.
    InFrame {
        /// The frame.
        frame: Id<WorkFrame>,
        /// Position (m) and roll, pitch, yaw of the tool centre point in the frame.
        offset: Placement,
    },
    /// A pose of the tool centre point in the world frame.
    InWorld {
        /// Position (m) and roll, pitch, yaw of the tool centre point in the world.
        offset: Placement,
    },
    /// A pose of the tool centre point in the robot's base frame (its `base` link when it has one, as
    /// on a UR teach pendant; otherwise its root link). It moves with the robot.
    InBase {
        /// Position (m) and roll, pitch, yaw of the tool centre point in the base frame.
        offset: Placement,
    },
}

/// A place the robot should go to.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Waypoint {
    /// Name shown in the browser.
    pub name: String,
    /// The robot it belongs to.
    pub robot: Id<Robot>,
    /// Where it is.
    pub target: WaypointTarget,
}

impl Entity for Waypoint {
    const KIND: Kind = Kind::Waypoint;
    fn refs(&self, out: &mut Vec<AnyId>) {
        out.push(self.robot.erase());
        match &self.target {
            WaypointTarget::SketchPoint { sketch, .. } => out.push(sketch.erase()),
            WaypointTarget::InFrame { frame, .. } => out.push(frame.erase()),
            WaypointTarget::Joints(_) | WaypointTarget::InWorld { .. } | WaypointTarget::InBase { .. } => {}
        }
    }
}

/// A motion of one robot from its current state through waypoints.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MotionPath {
    /// Name shown in the browser.
    pub name: String,
    /// The robot that moves.
    pub robot: Id<Robot>,
    /// The waypoints in order.
    pub waypoints: Vec<Id<Waypoint>>,
    /// For each waypoint, whether the tool tip goes there in a straight line (a linear move) instead of
    /// by a freely planned motion. Missing entries mean free.
    #[serde(default)]
    pub linear: Vec<bool>,
    /// Id of the planner used where the straight move is blocked.
    pub planner: String,
    /// Seed of the planner's random numbers.
    pub seed: u64,
    /// The fastest the tool tip may go on straight moves, m/s; `None` for as fast as the joints allow.
    #[serde(default)]
    pub tcp_speed: Option<f64>,
    /// Robot parts allowed closer to obstacles than the padding, per motion.
    #[serde(default)]
    pub contacts: Vec<ContactAllowance>,
}

impl Entity for MotionPath {
    const KIND: Kind = Kind::Path;
    fn refs(&self, out: &mut Vec<AnyId>) {
        out.push(self.robot.erase());
        out.extend(self.waypoints.iter().map(|w| w.erase()));
    }
}

/// An edit of the robot cell.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CellOp {
    /// Adds a work plane, axis or point at the rollback marker of the model tree.
    AddWorkFeature(crate::design::WorkFeature),
    /// Replaces a work feature.
    SetWorkFeature {
        /// The work feature.
        id: Id<crate::design::WorkFeature>,
        /// Its new definition.
        feature: crate::design::WorkFeature,
    },
    /// Deletes a work feature nothing depends on.
    DeleteWorkFeature(Id<crate::design::WorkFeature>),
    /// Adds a modelling feature at the rollback marker.
    AddFeature(crate::design::Feature),
    /// Replaces a modelling feature.
    SetFeature {
        /// The feature.
        id: Id<crate::design::Feature>,
        /// Its new definition.
        feature: crate::design::Feature,
    },
    /// Deletes a modelling feature (its sketch stays).
    DeleteFeature(Id<crate::design::Feature>),
    /// Adds an imported assembly.
    AddAssembly(crate::design::Assembly),
    /// Moves an imported assembly.
    PlaceAssembly {
        /// The assembly.
        id: Id<crate::design::Assembly>,
        /// Where its origin stands.
        placement: Placement,
    },
    /// Changes how an imported assembly is shown.
    SetAssemblyLook {
        /// The assembly.
        id: Id<crate::design::Assembly>,
        /// How it is shown.
        look: Look,
    },
    /// Deletes an imported assembly.
    DeleteAssembly(Id<crate::design::Assembly>),
    /// Adds an assembly constraint and moves the components it ties (the ones not grounded) into place.
    /// Refused when it cannot be satisfied together with the others.
    AddRelationship(crate::relate::Relationship),
    /// Changes an assembly constraint (kind, offset, selections) and solves again.
    SetRelationship {
        /// The constraint.
        id: Id<crate::relate::Relationship>,
        /// The new data.
        relationship: crate::relate::Relationship,
    },
    /// Deletes an assembly constraint; the components stay where they are.
    DeleteRelationship(Id<crate::relate::Relationship>),
    /// Patterns components: copies arranged in columns and rows or round an axis, following the parents.
    AddPattern {
        /// Name shown in the browser.
        name: String,
        /// The components patterned (bodies, robots, assemblies).
        parents: Vec<AnyId>,
        /// The arrangement.
        layout: crate::pattern::PatternLayout,
    },
    /// Changes a pattern's arrangement (count, spacing, direction): occurrences are added or removed.
    SetPattern {
        /// Index into the patterns.
        index: usize,
        /// The new arrangement.
        layout: crate::pattern::PatternLayout,
    },
    /// Deletes a pattern; its copies go too unless `keep_copies`.
    DeletePattern {
        /// Index into the patterns.
        index: usize,
        /// Keep the copies as ordinary components.
        keep_copies: bool,
    },
    /// Takes an occurrence out of its pattern: its copies stay where they are as ordinary components.
    MakePatternElementIndependent {
        /// Index into the patterns.
        index: usize,
        /// The occurrence (1, 2, …).
        element: u32,
    },
    /// Saves the current visibility (and the viewport's camera and section, as it writes them) as a view
    /// representation.
    AddViewRepresentation {
        /// Its name.
        name: String,
        /// The camera, as the viewport writes it.
        camera: Option<String>,
        /// The section view, as the viewport writes it.
        section: Option<String>,
    },
    /// Replaces a view representation.
    SetViewRepresentation {
        /// Index into the views.
        index: usize,
        /// The new contents.
        view: crate::representation::ViewRepresentation,
    },
    /// Shows and hides components as a view representation says.
    ActivateViewRepresentation(usize),
    /// Adds a positional representation (a copy of [Primary]).
    AddPositionalRepresentation(String),
    /// Replaces a positional representation's overrides.
    SetPositionalRepresentation {
        /// Index into the positions.
        index: usize,
        /// The overrides.
        position: crate::representation::PositionalRepresentation,
    },
    /// Activates a positional representation (`None`: [Primary]).
    ActivatePositionalRepresentation(Option<usize>),
    /// Deletes a view and/or a positional representation.
    DeleteRepresentation {
        /// A view to delete.
        view: Option<usize>,
        /// A position to delete.
        position: Option<usize>,
    },
    /// Ground and Root, "Ground at origin": moves the components (bodies, robots, assemblies) to the
    /// assembly's origin, aligned with it, and grounds them. Refused when constraints then cannot be met.
    GroundAtOrigin(Vec<AnyId>),
    /// Place at Component Origin: moves a component onto the origin of another (aligned and coincident)
    /// and constrains it there with Flush constraints between their origin planes.
    PlaceAtComponentOrigin {
        /// The component that is placed (a body, robot or assembly).
        item: AnyId,
        /// The component whose origin it goes to (a part or robot link included).
        target: crate::design::Occurrence,
    },
    /// Moves a design item in the model tree to just before another (`None`: to the end). Refused when
    /// it would come before something it depends on, or after something that depends on it.
    MoveInTree {
        /// The item to move.
        item: AnyId,
        /// The item it goes before.
        before: Option<AnyId>,
    },
    /// Suppresses a design item, waypoint or path (kept, but not computed or planned) or brings it back.
    SetSuppressed {
        /// The item.
        item: AnyId,
        /// True to suppress.
        suppressed: bool,
    },
    /// Puts the rollback marker after an item (`None`: at the end).
    SetRollback {
        /// The last item that is computed.
        after: Option<AnyId>,
    },
    /// Adds a body.
    AddBody(Body),
    /// Replaces a body's data.
    SetBody {
        /// The body.
        id: Id<Body>,
        /// The new data.
        body: Body,
    },
    /// Deletes a body.
    DeleteBody(Id<Body>),
    /// Adds a robot.
    AddRobot(Robot),
    /// Moves a robot's base.
    PlaceRobot {
        /// The robot.
        id: Id<Robot>,
        /// The new placement.
        placement: Placement,
    },
    /// Changes how a body is shown.
    SetBodyLook {
        /// The body.
        id: Id<Body>,
        /// How it is shown.
        look: Look,
    },
    /// Changes how a robot is shown.
    SetRobotLook {
        /// The robot.
        id: Id<Robot>,
        /// How it is shown.
        look: Look,
    },
    /// Changes the folder a robot's mesh files are looked for in.
    SetRobotFiles {
        /// The robot.
        id: Id<Robot>,
        /// The folder.
        resource_dir: String,
    },
    /// Sets a robot's tool centre point.
    SetTcp {
        /// The robot.
        id: Id<Robot>,
        /// The pose of the tool centre point in the tip link's frame.
        tcp: Placement,
    },
    /// Sets a robot's link tree (from its description; see [`Robot::links`]).
    SetRobotLinks {
        /// The robot.
        id: Id<Robot>,
        /// The links, parents before children.
        links: Vec<RobotLink>,
    },
    /// Sets a robot's joint values.
    SetJoints {
        /// The robot.
        id: Id<Robot>,
        /// The values, one per joint of the planning chain.
        joints: Vec<f64>,
    },
    /// Deletes a robot that no waypoint or path uses.
    DeleteRobot(Id<Robot>),
    /// Adds a work frame.
    AddFrame(WorkFrame),
    /// Replaces a work frame's data.
    SetFrame {
        /// The frame.
        id: Id<WorkFrame>,
        /// The new data.
        frame: WorkFrame,
    },
    /// Deletes a work frame that no waypoint uses.
    DeleteFrame(Id<WorkFrame>),
    /// Adds a waypoint.
    AddWaypoint(Waypoint),
    /// Changes where a waypoint is.
    SetWaypointTarget {
        /// The waypoint.
        id: Id<Waypoint>,
        /// The new target.
        target: WaypointTarget,
    },
    /// Deletes a waypoint that no path uses.
    DeleteWaypoint(Id<Waypoint>),
    /// Adds a path.
    AddPath(MotionPath),
    /// Chooses a path's planner and seed.
    SetPlanner {
        /// The path.
        id: Id<MotionPath>,
        /// Planner id.
        planner: String,
        /// Seed.
        seed: u64,
    },
    /// Chooses how one waypoint of a path is approached.
    SetLinear {
        /// The path.
        id: Id<MotionPath>,
        /// Position of the waypoint in the path.
        index: usize,
        /// True for a straight line of the tool tip.
        linear: bool,
    },
    /// Replaces a path's contact allowances.
    SetContacts {
        /// The path.
        id: Id<MotionPath>,
        /// The allowances; each `index` must be a position of the path.
        contacts: Vec<ContactAllowance>,
    },
    /// Replaces a robot's collision-check choices for link pairs.
    SetAcmOverrides {
        /// The robot.
        id: Id<Robot>,
        /// The choices, at most one per pair.
        overrides: Vec<AcmOverride>,
    },
    /// Replaces a path's waypoints and how each is approached: reordering, adding and removing.
    SetPathSteps {
        /// The path.
        id: Id<MotionPath>,
        /// The waypoints in order; at least one, all for the path's robot.
        waypoints: Vec<Id<Waypoint>>,
        /// True for a straight line of the tool tip into the waypoint of the same position.
        linear: Vec<bool>,
    },
    /// Gives a robot a tool, or takes it away.
    SetTool {
        /// The robot.
        id: Id<Robot>,
        /// The tool; `None` for none.
        tool: Option<Tool>,
    },
    /// Limits the tool tip's speed on a path's straight moves, or lifts the limit.
    SetTcpSpeed {
        /// The path.
        id: Id<MotionPath>,
        /// m/s, positive; `None` for no limit.
        speed: Option<f64>,
    },
    /// Deletes a path.
    DeletePath(Id<MotionPath>),
}

fn finite(values: &[f64]) -> bool {
    values.iter().all(|v| v.is_finite())
}

/// Changes the model tree's order, creating it on first use (with the current order written out).
pub(crate) fn set_timeline(
    txn: &mut Txn<'_>,
    edit: impl FnOnce(&mut crate::design::Timeline),
) -> Result<(), EditError> {
    let existing = txn.doc().timeline().map(|(id, _)| id);
    let order = txn.doc().model_order();
    // States of components that no longer exist are dropped whenever the state is written (undoing the
    // delete brings the component back together with the state as it was before this write).
    let gone: Vec<crate::design::Occurrence> = txn
        .doc()
        .timeline()
        .map(|(_, t)| {
            let c = &t.components;
            c.grounded
                .iter()
                .chain(&c.disabled)
                .chain(&c.hidden)
                .chain(&c.suppressed)
                .copied()
                .chain(c.origins_shown.iter().map(|(o, _)| *o))
                .filter(|o| !txn.doc().has_occurrence(*o))
                .collect()
        })
        .unwrap_or_default();
    let edit = |t: &mut crate::design::Timeline| {
        if !gone.is_empty() {
            let c = &mut t.components;
            for list in [&mut c.grounded, &mut c.disabled, &mut c.hidden, &mut c.suppressed] {
                list.retain(|o| !gone.contains(o));
            }
            c.origins_shown.retain(|(o, _)| !gone.contains(o));
        }
        edit(t);
    };
    match existing {
        Some(id) => {
            txn.modify(id, |t: &mut crate::design::Timeline, _| {
                t.items = order;
                edit(t);
            })?;
        }
        None => {
            let mut timeline = crate::design::Timeline { items: order, ..Default::default() };
            edit(&mut timeline);
            txn.insert(timeline);
        }
    }
    Ok(())
}

/// Puts a newly added design item right after the rollback marker, when the marker is not at the end
/// (otherwise it is at the end already).
fn place_new(txn: &mut Txn<'_>, item: AnyId) -> Result<(), EditError> {
    let Some(marker) = txn.doc().timeline().and_then(|(_, t)| t.rollback_after) else { return Ok(()) };
    if !txn.doc().contains(marker) {
        return Ok(());
    }
    set_timeline(txn, |t| {
        t.items.retain(|i| *i != item);
        let at = t.items.iter().position(|i| *i == marker).map_or(t.items.len(), |m| m + 1);
        t.items.insert(at, item);
        // The new item is where the user works: the marker moves past it.
        t.rollback_after = Some(item);
    })
}

/// Checks a work feature: finite numbers, references of the right kind, no reference to itself.
fn work_feature_ok(
    txn: &Txn<'_>,
    feature: &crate::design::WorkFeature,
    itself: Option<Id<crate::design::WorkFeature>>,
) -> Result<(), EditError> {
    use crate::design::{PlaneRef, PointRef, WorkGeometry};
    let doc = txn.doc();
    if !feature.geometry.is_finite() {
        return Err(EditError::BadValue);
    }
    let plane_ok = |plane: &PlaneRef| match plane {
        PlaneRef::Origin(_) => true,
        PlaneRef::Work(id) => Some(*id) != itself && doc.get(*id).is_some_and(|w| w.geometry.is_plane()),
        PlaneRef::Component(occurrence, _) => doc.component_origin(*occurrence).is_some(),
    };
    let point_ok = |point: &PointRef| match point {
        PointRef::Origin | PointRef::Fixed(_) => true,
        PointRef::SketchPoint { sketch, point } => {
            doc.get(*sketch).is_some_and(|s| s.points.contains_key(point))
        }
        PointRef::Work(id) => Some(*id) != itself && doc.get(*id).is_some_and(|w| w.geometry.is_point()),
        PointRef::Component(occurrence) => doc.component_origin(*occurrence).is_some(),
    };
    let ok = match &feature.geometry {
        WorkGeometry::PlaneOffset { base, .. } => plane_ok(base),
        WorkGeometry::PlaneThroughPoints { points } => points.iter().all(point_ok),
        WorkGeometry::AxisThroughPoints { points } => points.iter().all(point_ok) && points[0] != points[1],
        WorkGeometry::AxisNormalToPlane { plane, through } => plane_ok(plane) && point_ok(through),
        WorkGeometry::Point { at } => point_ok(at),
    };
    if ok { Ok(()) } else { Err(EditError::BadValue) }
}

/// Checks a modelling feature: its sketch exists, the profile's curves are in it, the extent is usable.
fn feature_ok(txn: &Txn<'_>, feature: &crate::design::Feature) -> Result<(), EditError> {
    let crate::design::FeatureKind::Extrude(extrude) = &feature.kind;
    let sketch = txn.doc().get(extrude.sketch).ok_or(EditError::BadValue)?;
    if extrude.extent.range(extrude.reversed).is_none()
        || !extrude.profile.iter().all(|c| sketch.curves.contains_key(c))
    {
        return Err(EditError::BadValue);
    }
    Ok(())
}

fn placed(placement: &Placement) -> Result<(), EditError> {
    if finite(&placement.xyz) && finite(&placement.rpy) { Ok(()) } else { Err(EditError::BadValue) }
}

fn frame_ok(txn: &Txn<'_>, frame: &WorkFrame) -> Result<(), EditError> {
    let sketch = txn.doc().get(frame.sketch).ok_or(DocError::Missing(frame.sketch.erase()))?;
    let points = [Some(frame.point), frame.x_point, frame.xy_point];
    let all_there = points.iter().flatten().all(|p| sketch.points.contains_key(p));
    let distinct = frame.x_point != Some(frame.point)
        && frame.xy_point != Some(frame.point)
        && (frame.xy_point.is_none() || frame.xy_point != frame.x_point);
    if !all_there || !distinct || (frame.xy_point.is_some() && frame.x_point.is_none()) || !finite(&frame.rpy)
    {
        return Err(EditError::BadValue);
    }
    Ok(())
}

fn target_ok(txn: &Txn<'_>, robot: Id<Robot>, target: &WaypointTarget) -> Result<(), EditError> {
    let robot = txn.doc().get(robot).ok_or(DocError::Missing(robot.erase()))?;
    match target {
        WaypointTarget::Joints(q) => {
            if q.len() != robot.joints.len() || !finite(q) {
                return Err(EditError::BadValue);
            }
        }
        WaypointTarget::SketchPoint { sketch, point } => {
            let sketch = txn.doc().get(*sketch).ok_or(DocError::Missing(sketch.erase()))?;
            if !sketch.points.contains_key(point) {
                return Err(EditError::BadValue);
            }
        }
        WaypointTarget::InFrame { frame, offset } => {
            txn.doc().get(*frame).ok_or(DocError::Missing(frame.erase()))?;
            placed(offset)?;
        }
        WaypointTarget::InWorld { offset } | WaypointTarget::InBase { offset } => placed(offset)?,
    }
    Ok(())
}

pub(crate) fn run(txn: &mut Txn<'_>, op: &CellOp) -> Result<(Vec<Created>, &'static str), EditError> {
    match op {
        CellOp::AddBody(body) => {
            placed(&body.placement)?;
            if !body.solid.is_valid() {
                return Err(EditError::BadValue);
            }
            let first = txn.doc().has_no_components();
            let id = txn.insert(body.clone());
            crate::tree::ground_if_first(txn, first, id.erase())?;
            Ok((vec![Created::Body(id)], "Add body"))
        }
        CellOp::SetBody { id, body } => {
            placed(&body.placement)?;
            if !body.solid.is_valid() {
                return Err(EditError::BadValue);
            }
            txn.modify(*id, |b: &mut Body, _| b.clone_from(body))?;
            settle(txn)?;
            Ok((Vec::new(), "Edit body"))
        }
        CellOp::DeleteBody(id) => {
            txn.remove(*id)?;
            Ok((Vec::new(), "Delete body"))
        }
        CellOp::AddRobot(robot) => {
            placed(&robot.placement)?;
            if !finite(&robot.joints) || !(robot.max_acc.is_finite() && robot.max_acc > 0.0) {
                return Err(EditError::BadValue);
            }
            let first = txn.doc().has_no_components();
            let id = txn.insert(robot.clone());
            crate::tree::ground_if_first(txn, first, id.erase())?;
            Ok((vec![Created::Robot(id)], "Add robot"))
        }
        CellOp::PlaceRobot { id, placement } => {
            placed(placement)?;
            txn.modify(*id, |r: &mut Robot, _| r.placement = *placement)?;
            settle(txn)?;
            Ok((Vec::new(), "Place robot"))
        }
        CellOp::SetBodyLook { id, look } => {
            if !look.is_valid() {
                return Err(EditError::BadValue);
            }
            txn.modify(*id, |b: &mut Body, _| b.look = *look)?;
            Ok((Vec::new(), "Change display"))
        }
        CellOp::SetRobotLook { id, look } => {
            if !look.is_valid() {
                return Err(EditError::BadValue);
            }
            txn.modify(*id, |r: &mut Robot, _| r.look = *look)?;
            Ok((Vec::new(), "Change display"))
        }
        CellOp::SetRobotFiles { id, resource_dir } => {
            txn.modify(*id, |r: &mut Robot, _| r.resource_dir.clone_from(resource_dir))?;
            Ok((Vec::new(), "Move robot files"))
        }
        CellOp::SetTcp { id, tcp } => {
            placed(tcp)?;
            txn.modify(*id, |r: &mut Robot, _| r.tcp = *tcp)?;
            Ok((Vec::new(), "Set tool centre point"))
        }
        CellOp::SetRobotLinks { id, links } => {
            let joints = txn.doc().get(*id).ok_or(DocError::Missing(id.erase()))?.joints.len();
            let well_formed = links.iter().enumerate().all(|(i, l)| {
                l.parent.is_none_or(|p| (p as usize) < i)
                    && finite(&l.origin.xyz)
                    && finite(&l.origin.rpy)
                    && finite(&l.axis)
                    && match l.motion {
                        LinkMotion::Fixed => true,
                        LinkMotion::Turn { joint, scale, offset }
                        | LinkMotion::Slide { joint, scale, offset } => {
                            (joint as usize) < joints && scale.is_finite() && offset.is_finite()
                        }
                    }
            });
            if !well_formed {
                return Err(EditError::BadValue);
            }
            txn.modify(*id, |r: &mut Robot, _| r.links.clone_from(links))?;
            Ok((Vec::new(), "Robot links"))
        }
        CellOp::SetJoints { id, joints } => {
            let count = txn.doc().get(*id).ok_or(DocError::Missing(id.erase()))?.joints.len();
            if joints.len() != count || !finite(joints) {
                return Err(EditError::BadValue);
            }
            txn.modify(*id, |r: &mut Robot, _| r.joints.clone_from(joints))?;
            settle_holding(txn, &[id.erase()])?;
            Ok((Vec::new(), "Move joints"))
        }
        CellOp::DeleteRobot(id) => {
            let doc = txn.doc();
            let used = doc.waypoints().iter().any(|(_, w)| w.robot == *id)
                || doc.paths().iter().any(|(_, p)| p.robot == *id);
            if used {
                return Err(EditError::InUse);
            }
            txn.remove(*id)?;
            Ok((Vec::new(), "Delete robot"))
        }
        CellOp::AddFrame(frame) => {
            frame_ok(txn, frame)?;
            Ok((vec![Created::Frame(txn.insert(frame.clone()))], "Add work frame"))
        }
        CellOp::SetFrame { id, frame } => {
            frame_ok(txn, frame)?;
            txn.modify(*id, |f: &mut WorkFrame, _| f.clone_from(frame))?;
            Ok((Vec::new(), "Edit work frame"))
        }
        CellOp::DeleteFrame(id) => {
            let used = txn
                .doc()
                .waypoints()
                .iter()
                .any(|(_, w)| matches!(w.target, WaypointTarget::InFrame { frame, .. } if frame == *id));
            if used {
                return Err(EditError::InUse);
            }
            txn.remove(*id)?;
            Ok((Vec::new(), "Delete work frame"))
        }
        CellOp::SetWaypointTarget { id, target } => {
            let robot = txn.doc().get(*id).ok_or(DocError::Missing(id.erase()))?.robot;
            target_ok(txn, robot, target)?;
            txn.modify(*id, |w: &mut Waypoint, _| w.target.clone_from(target))?;
            Ok((Vec::new(), "Move waypoint"))
        }
        CellOp::AddWaypoint(waypoint) => {
            target_ok(txn, waypoint.robot, &waypoint.target)?;
            Ok((vec![Created::Waypoint(txn.insert(waypoint.clone()))], "Add waypoint"))
        }
        CellOp::DeleteWaypoint(id) => {
            if txn.doc().paths().iter().any(|(_, p)| p.waypoints.contains(id)) {
                return Err(EditError::InUse);
            }
            txn.remove(*id)?;
            Ok((Vec::new(), "Delete waypoint"))
        }
        CellOp::AddPath(path) => {
            let doc = txn.doc();
            let same_robot =
                path.waypoints.iter().all(|w| doc.get(*w).is_some_and(|w| w.robot == path.robot));
            if path.waypoints.is_empty() || !same_robot {
                return Err(EditError::BadValue);
            }
            Ok((vec![Created::Path(txn.insert(path.clone()))], "Add path"))
        }
        CellOp::SetPlanner { id, planner, seed } => {
            txn.modify(*id, |p: &mut MotionPath, _| {
                p.planner.clone_from(planner);
                p.seed = *seed;
            })?;
            Ok((Vec::new(), "Choose planner"))
        }
        CellOp::SetLinear { id, index, linear } => {
            let count = txn.doc().get(*id).ok_or(DocError::Missing(id.erase()))?.waypoints.len();
            if *index >= count {
                return Err(EditError::BadValue);
            }
            txn.modify(*id, |p: &mut MotionPath, _| {
                p.linear.resize(count, false);
                p.linear[*index] = *linear;
            })?;
            Ok((Vec::new(), "Choose motion"))
        }
        CellOp::SetPathSteps { id, waypoints, linear } => {
            let doc = txn.doc();
            let robot = doc.get(*id).ok_or(DocError::Missing(id.erase()))?.robot;
            let same_robot = waypoints.iter().all(|w| doc.get(*w).is_some_and(|w| w.robot == robot));
            if waypoints.is_empty() || !same_robot || linear.len() != waypoints.len() {
                return Err(EditError::BadValue);
            }
            txn.modify(*id, |p: &mut MotionPath, _| {
                p.waypoints.clone_from(waypoints);
                p.linear.clone_from(linear);
                // Allowances belong to positions; those beyond the new end go.
                p.contacts.retain(|c| c.index < waypoints.len());
            })?;
            Ok((Vec::new(), "Edit path"))
        }
        CellOp::AddWorkFeature(feature) => {
            work_feature_ok(txn, feature, None)?;
            let id = txn.insert(feature.clone());
            place_new(txn, id.erase())?;
            Ok((vec![Created::WorkFeature(id)], "Add work feature"))
        }
        CellOp::SetWorkFeature { id, feature } => {
            work_feature_ok(txn, feature, Some(*id))?;
            txn.modify(*id, |f: &mut crate::design::WorkFeature, _| f.clone_from(feature))?;
            Ok((Vec::new(), "Edit work feature"))
        }
        CellOp::DeleteWorkFeature(id) => {
            txn.remove(*id)?;
            Ok((Vec::new(), "Delete work feature"))
        }
        CellOp::AddFeature(feature) => {
            feature_ok(txn, feature)?;
            let id = txn.insert(feature.clone());
            place_new(txn, id.erase())?;
            Ok((vec![Created::Feature(id)], "Add feature"))
        }
        CellOp::SetFeature { id, feature } => {
            feature_ok(txn, feature)?;
            txn.modify(*id, |f: &mut crate::design::Feature, _| f.clone_from(feature))?;
            Ok((Vec::new(), "Edit feature"))
        }
        CellOp::DeleteFeature(id) => {
            txn.remove(*id)?;
            Ok((Vec::new(), "Delete feature"))
        }
        CellOp::AddAssembly(assembly) => {
            placed(&assembly.placement)?;
            if !assembly.is_valid() {
                return Err(EditError::BadValue);
            }
            let first = txn.doc().has_no_components();
            let id = txn.insert(assembly.clone());
            place_new(txn, id.erase())?;
            crate::tree::ground_if_first(txn, first, id.erase())?;
            Ok((vec![Created::Assembly(id)], "Import assembly"))
        }
        CellOp::PlaceAssembly { id, placement } => {
            placed(placement)?;
            txn.modify(*id, |a: &mut crate::design::Assembly, _| a.placement = *placement)?;
            settle(txn)?;
            Ok((Vec::new(), "Move assembly"))
        }
        CellOp::SetAssemblyLook { id, look } => {
            txn.modify(*id, |a: &mut crate::design::Assembly, _| a.look = *look)?;
            Ok((Vec::new(), "Change look"))
        }
        CellOp::DeleteAssembly(id) => {
            txn.remove(*id)?;
            Ok((Vec::new(), "Delete assembly"))
        }
        CellOp::AddRelationship(relationship) => {
            if !relationship.is_valid()
                || !txn.doc().has_occurrence(relationship.first.occurrence)
                || !txn.doc().has_occurrence(relationship.second.occurrence)
            {
                return Err(EditError::BadValue);
            }
            let id = txn.insert(relationship.clone());
            settle(txn)?;
            Ok((vec![Created::Relationship(id)], "Constrain"))
        }
        CellOp::SetRelationship { id, relationship } => {
            if !relationship.is_valid()
                || !txn.doc().has_occurrence(relationship.first.occurrence)
                || !txn.doc().has_occurrence(relationship.second.occurrence)
            {
                return Err(EditError::BadValue);
            }
            txn.modify(*id, |r: &mut crate::relate::Relationship, _| r.clone_from(relationship))?;
            settle(txn)?;
            Ok((Vec::new(), "Edit constraint"))
        }
        CellOp::DeleteRelationship(id) => {
            txn.remove(*id)?;
            Ok((Vec::new(), "Delete constraint"))
        }
        CellOp::AddPattern { name, parents, layout } => crate::pattern::add(txn, name, parents, layout),
        CellOp::SetPattern { index, layout } => crate::pattern::set(txn, *index, layout),
        CellOp::DeletePattern { index, keep_copies } => crate::pattern::delete(txn, *index, *keep_copies),
        CellOp::MakePatternElementIndependent { index, element } => {
            crate::pattern::make_independent(txn, *index, *element)
        }
        CellOp::AddViewRepresentation { name, camera, section } => {
            crate::representation::add_view(txn, name, camera.as_deref(), section.as_deref())
        }
        CellOp::SetViewRepresentation { index, view } => crate::representation::set_view(txn, *index, view),
        CellOp::ActivateViewRepresentation(index) => crate::representation::activate_view(txn, *index),
        CellOp::AddPositionalRepresentation(name) => crate::representation::add_position(txn, name),
        CellOp::SetPositionalRepresentation { index, position } => {
            crate::representation::set_position(txn, *index, position)
        }
        CellOp::ActivatePositionalRepresentation(index) => {
            crate::representation::activate_position(txn, *index)
        }
        CellOp::DeleteRepresentation { view, position } => {
            crate::representation::delete(txn, *view, *position)
        }
        CellOp::GroundAtOrigin(items) => {
            use crate::design::Occurrence;
            if items.is_empty() || !items.iter().all(|i| txn.doc().owner_pose(*i).is_some()) {
                return Err(EditError::BadValue);
            }
            for item in items {
                set_owner_pose(txn, *item, gc_math::DPose3::IDENTITY)?;
                if !txn.doc().component_flag(Occurrence::Item(*item), crate::design::ComponentFlag::Grounded)
                {
                    crate::tree::set_component(
                        txn,
                        Occurrence::Item(*item),
                        crate::design::ComponentFlag::Grounded,
                        true,
                    )?;
                }
            }
            settle(txn)?;
            Ok((Vec::new(), "Ground and root"))
        }
        CellOp::PlaceAtComponentOrigin { item, target } => {
            use crate::design::{Occurrence, OriginElement, OriginPlane};
            use crate::relate::{Geometry, RelationKind, Relationship, Selection};
            let pose = txn.doc().component_origin(*target).ok_or(EditError::BadValue)?.pose;
            if txn.doc().owner_pose(*item).is_none() || target.owner() == *item {
                return Err(EditError::BadValue);
            }
            set_owner_pose(txn, *item, pose)?;
            let mut created = Vec::new();
            for (plane, name) in [(OriginPlane::Xy, "XY"), (OriginPlane::Xz, "XZ"), (OriginPlane::Yz, "YZ")] {
                let geometry = Geometry::origin(OriginElement::Plane(plane));
                let relationship = Relationship {
                    limits: None,
                    name: format!("Flush:{name}"),
                    kind: RelationKind::Flush { offset: 0.0 },
                    first: Selection { occurrence: Occurrence::Item(*item), geometry },
                    second: Selection { occurrence: *target, geometry },
                };
                created.push(Created::Relationship(txn.insert(relationship)));
            }
            settle(txn)?;
            Ok((created, "Place at component origin"))
        }
        CellOp::MoveInTree { item, before } => {
            let doc = txn.doc();
            let mut order = doc.model_order();
            let Some(from) = order.iter().position(|o| o == item) else { return Err(EditError::BadValue) };
            if before.is_some_and(|b| !order.contains(&b) || b == *item) {
                return Err(EditError::BadValue);
            }
            order.remove(from);
            let to = before.and_then(|b| order.iter().position(|o| *o == b)).unwrap_or(order.len());
            order.insert(to, *item);
            // Every item must come after what it depends on.
            let position = |id: AnyId| order.iter().position(|o| *o == id);
            let broken = doc.references().iter().any(
                |(user, used)| matches!((position(*user), position(*used)), (Some(u), Some(d)) if u < d),
            );
            if broken {
                return Err(EditError::InUse);
            }
            set_timeline(txn, |t| t.items = order)?;
            Ok((Vec::new(), "Reorder"))
        }
        CellOp::SetSuppressed { item, suppressed } => {
            // Waypoints, paths and assembly relationships are not in the model order but can be suppressed.
            let outside = matches!(item.kind, Kind::Waypoint | Kind::Path | Kind::Relationship)
                && txn.doc().contains(*item);
            if !outside && !txn.doc().model_order().contains(item) {
                return Err(EditError::BadValue);
            }
            set_timeline(txn, |t| {
                t.suppressed.retain(|s| s != item);
                if *suppressed {
                    t.suppressed.push(*item);
                }
            })?;
            if item.kind == Kind::Relationship {
                settle(txn)?;
            }
            Ok((Vec::new(), if *suppressed { "Suppress" } else { "Unsuppress" }))
        }
        CellOp::SetRollback { after } => {
            if after.is_some_and(|a| !txn.doc().model_order().contains(&a)) {
                return Err(EditError::BadValue);
            }
            set_timeline(txn, |t| t.rollback_after = *after)?;
            Ok((Vec::new(), "Move end of part"))
        }
        CellOp::SetContacts { id, contacts } => {
            let count = txn.doc().get(*id).ok_or(DocError::Missing(id.erase()))?.waypoints.len();
            let sound = |c: &ContactAllowance| c.index < count && c.radius.is_finite() && c.radius >= 0.0;
            if !contacts.iter().all(sound) {
                return Err(EditError::BadValue);
            }
            txn.modify(*id, |p: &mut MotionPath, _| p.contacts.clone_from(contacts))?;
            Ok((Vec::new(), "Allow contact"))
        }
        CellOp::SetAcmOverrides { id, overrides } => {
            let pairs: std::collections::BTreeSet<(&str, &str)> = overrides
                .iter()
                .map(|o| if o.a <= o.b { (o.a.as_str(), o.b.as_str()) } else { (o.b.as_str(), o.a.as_str()) })
                .collect();
            if pairs.len() != overrides.len() || overrides.iter().any(|o| o.a == o.b) {
                return Err(EditError::BadValue);
            }
            txn.modify(*id, |r: &mut Robot, _| r.acm_overrides.clone_from(overrides))?;
            Ok((Vec::new(), "Choose collision checks"))
        }
        CellOp::SetTool { id, tool } => {
            if tool.as_ref().is_some_and(|t| !t.is_valid()) {
                return Err(EditError::BadValue);
            }
            txn.modify(*id, |r: &mut Robot, _| r.tool = tool.clone().map(Box::new))?;
            Ok((Vec::new(), "Set tool"))
        }
        CellOp::SetTcpSpeed { id, speed } => {
            if speed.is_some_and(|v| !(v.is_finite() && v > 0.0)) {
                return Err(EditError::BadValue);
            }
            txn.modify(*id, |p: &mut MotionPath, _| p.tcp_speed = *speed)?;
            Ok((Vec::new(), "Limit tool speed"))
        }
        CellOp::DeletePath(id) => {
            txn.remove(*id)?;
            Ok((Vec::new(), "Delete path"))
        }
    }
}

/// Moves the components that assembly constraints tie into place after an edit; refused when the
/// constraints cannot all be satisfied.
pub(crate) fn settle(txn: &mut Txn<'_>) -> Result<(), EditError> {
    settle_holding(txn, &[])
}

/// [`settle`] with some components held still.
pub(crate) fn settle_holding(txn: &mut Txn<'_>, held: &[AnyId]) -> Result<(), EditError> {
    if txn.doc().relationships().is_empty() {
        return Ok(());
    }
    let moves = txn.doc().solve_relationships_holding(held).map_err(|u| {
        let names =
            u.sick.iter().map(|id| txn.doc().get(*id).map_or_else(String::new, |r| r.name.clone())).collect();
        EditError::Unsolvable { ids: u.sick, names }
    })?;
    for (item, pose) in moves {
        let placement =
            Placement { xyz: pose.translation.to_array(), rpy: gc_math::rpy_from_quat(pose.rotation) };
        match item.kind {
            Kind::Body => {
                txn.modify(Id::<Body>::from_raw(item.raw), |b: &mut Body, _| b.placement = placement)?
            }
            Kind::RobotInstance => {
                txn.modify(Id::<Robot>::from_raw(item.raw), |r: &mut Robot, _| r.placement = placement)?;
            }
            Kind::Assembly => txn.modify(
                Id::<crate::design::Assembly>::from_raw(item.raw),
                |a: &mut crate::design::Assembly, _| {
                    a.placement = placement;
                },
            )?,
            _ => {}
        }
    }
    Ok(())
}

/// Sets the world pose of a body, robot or assembly.
pub(crate) fn set_owner_pose(txn: &mut Txn<'_>, item: AnyId, pose: gc_math::DPose3) -> Result<(), EditError> {
    let placement =
        Placement { xyz: pose.translation.to_array(), rpy: gc_math::rpy_from_quat(pose.rotation) };
    match item.kind {
        Kind::Body => {
            txn.modify(Id::<Body>::from_raw(item.raw), |b: &mut Body, _| b.placement = placement)?
        }
        Kind::RobotInstance => {
            txn.modify(Id::<Robot>::from_raw(item.raw), |r: &mut Robot, _| r.placement = placement)?
        }
        Kind::Assembly => txn.modify(
            Id::<crate::design::Assembly>::from_raw(item.raw),
            |a: &mut crate::design::Assembly, _| {
                a.placement = placement;
            },
        )?,
        _ => return Err(EditError::BadValue),
    }
    Ok(())
}
