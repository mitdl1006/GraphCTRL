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
    /// Path roles of sketch curves the robot follows, in order, instead of going through waypoints
    /// (`waypoints` is then empty). The curves, how they are travelled, the speed and the tool's attitude
    /// are read from the roles when the path is planned; the robot moves freely to the start of each.
    /// A contact allowance's `index` is then a position in this list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub follow: Vec<Id<crate::role::SketchRole>>,
}

impl Entity for MotionPath {
    const KIND: Kind = Kind::Path;
    fn refs(&self, out: &mut Vec<AnyId>) {
        out.push(self.robot.erase());
        out.extend(self.waypoints.iter().map(|w| w.erase()));
        out.extend(self.follow.iter().map(|r| r.erase()));
    }
}

/// An edit of the robot cell.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CellOp {
    /// iProperties (iProperties Reference, GUID-8ABF17C5-B9EB-47B5-92CF-9FBD335334E3): replaces those of a
    /// component, or of the document (`None`). Refused for a component that does not exist or numbers that
    /// are not valid.
    SetProperties {
        /// The component, or `None` for the document.
        target: Option<crate::design::Occurrence>,
        /// The new iProperties.
        properties: crate::iproperties::IProperties,
    },
    /// The BOM Structure of a component or subassembly (About the BOM Structure,
    /// GUID-A0A95805-CBF6-4524-8314-4AC8B53C19CF). Refused for a row that is not in the tree.
    SetBomStructure {
        /// The component or subassembly.
        key: crate::subassembly::TreeKey,
        /// Its structure.
        structure: crate::iproperties::BomStructure,
    },
    /// Document Settings (To Change Document Settings, GUID-FB49837F-7832-4713-928E-1CEDBDB0C3BD): replaces
    /// the settings saved with the document. Refused when they are not valid (`DocSettings::is_valid`).
    SetDocSettings(crate::settings::DocSettings),
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
    /// Mirrors components across a plane (a planar face, work plane or origin plane): new instances
    /// placed by a rotation and translation; relationships between them re-made when asked (those that
    /// cannot be satisfied or whose orientation is ambiguous are made suppressed).
    MirrorComponents {
        /// The components (bodies, robots, assemblies).
        items: Vec<AnyId>,
        /// The mirror plane.
        plane: crate::relate::Selection,
        /// Mirror Relationships.
        relationships: bool,
        /// Ground New Components.
        ground: bool,
    },
    /// Copies components, moved by `offset` (a world transform); relationships between them copied when
    /// asked.
    CopyComponents {
        /// The components.
        items: Vec<AnyId>,
        /// How far the copies are moved from the sources.
        offset: Placement,
        /// Copy Relationships.
        relationships: bool,
        /// Ground New Components.
        ground: bool,
    },
    /// Replaces components with another shape or file (Replace; Replace All with every component of the
    /// same source): each stays where it was, relationships kept where possible and suppressed otherwise.
    ReplaceComponents {
        /// The components replaced (all bodies, or all assemblies).
        items: Vec<AnyId>,
        /// What they become.
        with: crate::replicate::Replacement,
    },
    /// Moves several components at once (a drag that pushed others through contact): one undo step.
    MoveComponents(Vec<(AnyId, Placement)>),
    /// Grip Snap's Commit and Ground: moves the components and grounds them, in one undo step (Inventor
    /// 2026 help, Grip Snap move or rotate, GUID-E1CA109A-D5A1-4131-AC3A-AE217716DD8C).
    MoveAndGround(Vec<(AnyId, Placement)>),
    /// Shows or hides the degrees-of-freedom symbol of components (iProperties, Occurrence tab).
    SetDofShown {
        /// The components.
        items: Vec<AnyId>,
        /// Shown.
        on: bool,
    },
    /// Shows or hides the degrees-of-freedom symbol of every component (View tab, Degrees of Freedom).
    SetDofShownAll(bool),
    /// A work frame on a component's origin plane in one step: a sketch on the plane and a frame at its
    /// origin. The frame (and every waypoint in it) follows the component, as projected geometry does.
    AddComponentFrame {
        /// Name of the frame.
        name: String,
        /// Name of the sketch that holds its origin.
        sketch: String,
        /// The component (a body, robot, assembly, assembly part, robot link or tool).
        occurrence: crate::design::Occurrence,
        /// Which of its origin planes.
        plane: crate::design::OriginPlane,
    },
    /// Adds components to, or takes them out of, the contact set.
    SetContactSet {
        /// The components (bodies, robots, assemblies).
        items: Vec<AnyId>,
        /// In the set.
        on: bool,
    },
    /// Interactive Contact: the contact solver off, for the contact set only, or for all components.
    SetContactSolver(crate::design::ContactSolver),
    /// Lock and Protect on a joint (Inventor 2026 help, To Define and Manage Joint Relationships,
    /// GUID-21DC3336-5C51-42C1-90FB-4299CD66E0C6). Refused for constraints.
    SetJointCondition {
        /// The joint.
        id: Id<crate::relate::Relationship>,
        /// Lock it where it stands now.
        lock: bool,
        /// Protect its freedom.
        protect: bool,
    },
    /// Subassemblies made in the cell: Demote, move between levels, dissolve, rename.
    Subassembly(crate::subassembly::SubassemblyOp),
    /// A joint's Limits (Inventor 2026 help, To Define and Manage Joint Relationships,
    /// GUID-21DC3336-5C51-42C1-90FB-4299CD66E0C6): angular for Rotational and Cylindrical, linear for
    /// Slider and Cylindrical; `None` clears one. Refused for other kinds, bad ranges, or when the
    /// assembly cannot then be satisfied.
    SetJointLimits {
        /// The joint.
        id: Id<crate::relate::Relationship>,
        /// The turn about its Z axis, radians.
        angular: Option<crate::relate::JointRange>,
        /// The slide along its Z axis, metres.
        linear: Option<crate::relate::JointRange>,
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
    /// Changes user-defined browser folders.
    Folder(crate::folder::FolderOp),
    /// Isolate (Inventor 2026 help, Frame Generator browser, GUID-36AE9264-912D-4D3C-BB24-7EC16FA8777E:
    /// "Isolate turns off visibility of components", all except the selected ones). Every visible
    /// component that is not one of `keep`, not inside one and not around one is hidden, and remembered
    /// for [`CellOp::UndoIsolate`]. Refused without components to keep or for one that does not exist.
    Isolate {
        /// The components that stay visible.
        keep: Vec<crate::design::Occurrence>,
    },
    /// Undo Isolate: "restores visibility of components to the pre-isolation state. It also keeps intact
    /// those operations performed in the isolated state" (same page). Shows what Isolate hid; refused
    /// when nothing is isolated.
    UndoIsolate,
    /// Gives sketch elements a role: a point becomes a waypoint, a chain of open curves a path. Refused
    /// (`InUse`) when the point already is a waypoint or a curve already belongs to a path role, and
    /// (`BadValue`) when the curves are closed or do not join into one chain.
    AddRole(crate::role::SketchRole),
    /// Replaces a role: its elements, how the path is travelled, the speed, the tool's attitude, its name.
    SetRole {
        /// The role.
        id: Id<crate::role::SketchRole>,
        /// Its new data.
        role: crate::role::SketchRole,
    },
    /// Takes a role away from its sketch elements. Refused while a robot's path follows it.
    DeleteRole(Id<crate::role::SketchRole>),
    /// Replaces the path roles a path follows (a path that goes through waypoints cannot follow roles).
    SetPathFollow {
        /// The path.
        id: Id<MotionPath>,
        /// The path roles, in order, each once.
        follow: Vec<Id<crate::role::SketchRole>>,
    },
    /// Adds path roles and a path that follows them (after those in `path.follow`), in one step.
    AddFollowPath {
        /// The path; without waypoints.
        path: MotionPath,
        /// Path roles to add first.
        roles: Vec<crate::role::SketchRole>,
    },
    /// Adds a path through sketch points, in one step: after the waypoints in `path.waypoints`, the
    /// robot's waypoint on each point (the one it has, or a new one).
    AddPathThrough {
        /// The path; it follows no roles.
        path: MotionPath,
        /// The sketch points, in order; at least one.
        stops: Vec<(Id<Sketch>, Id<SkPoint>)>,
    },
}

fn finite(values: &[f64]) -> bool {
    values.iter().all(|v| v.is_finite())
}

/// Changes the model tree's order, creating it on first use (with the current order written out).
/// Protect: a relationship that takes freedom away from a protected joint's component is refused.
fn check_protected(txn: &Txn<'_>, before: &[(Id<crate::relate::Relationship>, u8)]) -> Result<(), EditError> {
    let after = txn.doc().protected_freedom();
    for (id, was) in before {
        if after.iter().any(|(other, now)| other == id && now < was) {
            let name = txn.doc().get(*id).map(|r| r.name.clone()).unwrap_or_default();
            return Err(EditError::Protected(name));
        }
    }
    Ok(())
}

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
                .chain(&c.contact_set)
                .chain(&c.dof_shown)
                .copied()
                .chain(c.origins_shown.iter().map(|(o, _)| *o))
                .chain(c.placed_in.iter().map(|(o, _)| *o))
                .chain(c.properties.iter().map(|(o, _)| *o))
                .filter(|o| !txn.doc().has_occurrence(*o))
                .collect()
        })
        .unwrap_or_default();
    // Conditions of joints that no longer exist go too.
    let dropped: Vec<Id<crate::relate::Relationship>> = txn
        .doc()
        .timeline()
        .map(|(_, t)| {
            t.components.joint_conditions.keys().copied().filter(|id| txn.doc().get(*id).is_none()).collect()
        })
        .unwrap_or_default();
    // Owners of sketches that are gone, and owners that are gone (their sketches go to the top level).
    let orphaned: Vec<Id<Sketch>> = txn
        .doc()
        .timeline()
        .map(|(_, t)| {
            t.components
                .sketch_owners
                .iter()
                .filter(|(s, o)| txn.doc().get(*s).is_none() || !txn.doc().has_tree_row(*o))
                .map(|(s, _)| *s)
                .collect()
        })
        .unwrap_or_default();
    // BOM Structures of rows that are gone.
    let unlisted: Vec<crate::subassembly::TreeKey> = txn
        .doc()
        .timeline()
        .map(|(_, t)| {
            t.components.bom.iter().map(|(k, _)| *k).filter(|k| !txn.doc().has_tree_row(*k)).collect()
        })
        .unwrap_or_default();
    let edit = |t: &mut crate::design::Timeline| {
        t.components.sketch_owners.retain(|(s, _)| !orphaned.contains(s));
        t.components.bom.retain(|(k, _)| !unlisted.contains(k));
        if !gone.is_empty() {
            let c = &mut t.components;
            for list in [&mut c.grounded, &mut c.disabled, &mut c.hidden, &mut c.suppressed] {
                list.retain(|o| !gone.contains(o));
            }
            c.origins_shown.retain(|(o, _)| !gone.contains(o));
            c.contact_set.retain(|o| !gone.contains(o));
            c.dof_shown.retain(|o| !gone.contains(o));
            c.placed_in.retain(|(o, _)| !gone.contains(o));
            c.properties.retain(|(o, _)| !gone.contains(o));
        }
        t.components.joint_conditions.retain(|id, _| !dropped.contains(id));
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
    use crate::design::{LineRef, PlaneRef, PointRef, WorkGeometry};
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
    // A sketch line, or a circle or arc.
    let curve_ok = |r: &crate::design::CurveRef, line: bool| {
        doc.get(r.sketch).is_some_and(|s| {
            if line {
                s.line(r.curve).is_some()
            } else {
                s.line(r.curve).is_none() && s.curves.contains_key(&r.curve)
            }
        })
    };
    let line_ok = |line: &LineRef| match line {
        LineRef::Sketch(r) => curve_ok(r, true),
        LineRef::Work(id) => Some(*id) != itself && doc.get(*id).is_some_and(|w| w.geometry.is_axis()),
        LineRef::Origin(_) => true,
        LineRef::Component(occurrence, _) => doc.component_origin(*occurrence).is_some(),
    };
    let ok = match &feature.geometry {
        WorkGeometry::PlaneOffset { base, .. } => plane_ok(base),
        WorkGeometry::PlaneThroughPoints { points } => points.iter().all(point_ok),
        WorkGeometry::AxisThroughPoints { points } => points.iter().all(point_ok) && points[0] != points[1],
        WorkGeometry::AxisNormalToPlane { plane, through } => plane_ok(plane) && point_ok(through),
        WorkGeometry::AxisOnLine { line } => curve_ok(line, true),
        WorkGeometry::AxisParallelThrough { line, through } => curve_ok(line, true) && point_ok(through),
        WorkGeometry::AxisPlanesIntersection { planes } => {
            planes.iter().all(plane_ok) && planes[0] != planes[1]
        }
        WorkGeometry::AxisThroughCircle { curve } | WorkGeometry::PointCurveCentre { curve } => {
            curve_ok(curve, false)
        }
        WorkGeometry::Point { at } => point_ok(at),
        WorkGeometry::PlaneParallelThrough { base, through } => plane_ok(base) && point_ok(through),
        WorkGeometry::PlaneMidplane { planes } => planes.iter().all(plane_ok) && planes[0] != planes[1],
        WorkGeometry::PlaneAngleToPlane { base, line, .. } => plane_ok(base) && line_ok(line),
        WorkGeometry::PlaneLineAndPoint { line, through }
        | WorkGeometry::PlaneNormalToLine { line, through } => line_ok(line) && point_ok(through),
        WorkGeometry::PlaneNormalToCurve { curve, at } => curve_ok(curve, false) && point_ok(at),
        WorkGeometry::PointLinesIntersection { lines } => lines.iter().all(line_ok) && lines[0] != lines[1],
        WorkGeometry::PointPlaneLine { plane, line } => plane_ok(plane) && line_ok(line),
        WorkGeometry::PointThreePlanes { planes } => {
            planes.iter().all(plane_ok)
                && planes[0] != planes[1]
                && planes[1] != planes[2]
                && planes[0] != planes[2]
        }
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
        CellOp::AddRole(_)
        | CellOp::SetRole { .. }
        | CellOp::DeleteRole(_)
        | CellOp::SetPathFollow { .. }
        | CellOp::AddFollowPath { .. }
        | CellOp::AddPathThrough { .. } => crate::role::run(txn, op),
        CellOp::Folder(op) => crate::folder::run(txn, op),
        CellOp::Isolate { keep } => crate::tree::isolate(txn, keep),
        CellOp::UndoIsolate => crate::tree::undo_isolate(txn),
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
            crate::subassembly::delete_owned_sketches(txn, id.erase())?;
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
            crate::subassembly::delete_owned_sketches(txn, id.erase())?;
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
            // A sketch role whose tool attitude is measured from the frame keeps it too.
            let aimed = txn.doc().roles().iter().any(
                |(_, r)| matches!(r.aim.reference, crate::role::AimReference::Frame(frame) if frame == *id),
            );
            if used || aimed {
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
            // A path goes through waypoints or follows path roles, not both.
            let follows = !path.follow.is_empty() && crate::role::follow_ok(doc, &path.follow);
            if path.waypoints.is_empty() != follows || !same_robot || doc.get(path.robot).is_none() {
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
        CellOp::SetProperties { target, properties } => {
            if !properties.is_valid() || target.is_some_and(|o| !txn.doc().has_occurrence(o)) {
                return Err(EditError::BadValue);
            }
            let properties = properties.clone();
            set_timeline(txn, |t| match target {
                None => t.properties = properties,
                Some(o) => {
                    t.components.properties.retain(|(p, _)| p != o);
                    if !properties.is_empty() {
                        t.components.properties.push((*o, properties));
                    }
                }
            })?;
            Ok((Vec::new(), "iProperties"))
        }
        CellOp::SetBomStructure { key, structure } => {
            if !txn.doc().has_tree_row(*key) {
                return Err(EditError::BadValue);
            }
            set_timeline(txn, |t| {
                t.components.bom.retain(|(k, _)| k != key);
                if *structure != crate::iproperties::BomStructure::Normal {
                    t.components.bom.push((*key, *structure));
                }
            })?;
            Ok((Vec::new(), "BOM Structure"))
        }
        CellOp::SetDocSettings(settings) => {
            if !settings.is_valid() {
                return Err(EditError::BadValue);
            }
            crate::command::keep_sizes(txn, (settings.length(), settings.angle()))?;
            set_timeline(txn, |t| t.settings = *settings)?;
            Ok((Vec::new(), "Document Settings"))
        }
        CellOp::AddWorkFeature(feature) => {
            work_feature_ok(txn, feature, None)?;
            let id = txn.insert(feature.clone());
            // A definition that does not work out (parallel planes, a line of no length) is refused.
            if txn.doc().work_shape(id).is_none() {
                return Err(EditError::BadValue);
            }
            place_new(txn, id.erase())?;
            Ok((vec![Created::WorkFeature(id)], "Add work feature"))
        }
        CellOp::SetWorkFeature { id, feature } => {
            work_feature_ok(txn, feature, Some(*id))?;
            txn.modify(*id, |f: &mut crate::design::WorkFeature, _| f.clone_from(feature))?;
            if txn.doc().work_shape(*id).is_none() {
                return Err(EditError::BadValue);
            }
            // Components constrained to it follow (refused when they cannot).
            settle(txn)?;
            Ok((Vec::new(), "Edit work feature"))
        }
        CellOp::DeleteWorkFeature(id) => {
            // A relationship on it keeps it (Inventor asks to delete the constraint first).
            if txn.doc().relationships().iter().any(|(_, r)| r.references(id.erase())) {
                return Err(EditError::InUse);
            }
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
            crate::subassembly::delete_owned_sketches(txn, id.erase())?;
            txn.remove(*id)?;
            Ok((Vec::new(), "Delete assembly"))
        }
        CellOp::AddRelationship(relationship) => {
            if !relationship.is_valid()
                || !txn.doc().relatable(relationship.first.occurrence)
                || !txn.doc().relatable(relationship.second.occurrence)
            {
                return Err(EditError::BadValue);
            }
            let before = txn.doc().protected_freedom();
            let id = txn.insert(relationship.clone());
            settle(txn)?;
            check_protected(txn, &before)?;
            Ok((vec![Created::Relationship(id)], "Constrain"))
        }
        CellOp::SetRelationship { id, relationship } => {
            if !relationship.is_valid()
                || !txn.doc().relatable(relationship.first.occurrence)
                || !txn.doc().relatable(relationship.second.occurrence)
            {
                return Err(EditError::BadValue);
            }
            let before = txn.doc().protected_freedom();
            txn.modify(*id, |r: &mut crate::relate::Relationship, _| r.clone_from(relationship))?;
            settle(txn)?;
            check_protected(txn, &before)?;
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
        CellOp::MirrorComponents { items, plane, relationships, ground } => {
            crate::replicate::mirror(txn, items, plane, *relationships, *ground)
        }
        CellOp::CopyComponents { items, offset, relationships, ground } => {
            crate::replicate::copy(txn, items, offset, *relationships, *ground)
        }
        CellOp::ReplaceComponents { items, with } => crate::replicate::replace(txn, items, with),
        CellOp::SetContactSet { items, on } => {
            use crate::design::Occurrence;
            if items.is_empty() || !items.iter().all(|i| txn.doc().owner_pose(*i).is_some()) {
                return Err(EditError::BadValue);
            }
            set_timeline(txn, |t| {
                let set = &mut t.components.contact_set;
                set.retain(|o| !items.iter().any(|i| *o == Occurrence::Item(*i)));
                if *on {
                    // Each component once, however often it is named.
                    for item in items {
                        if !set.contains(&Occurrence::Item(*item)) {
                            set.push(Occurrence::Item(*item));
                        }
                    }
                }
            })?;
            Ok((Vec::new(), if *on { "Contact set" } else { "Remove from contact set" }))
        }
        CellOp::MoveComponents(moves) => {
            if moves.is_empty() {
                return Err(EditError::BadValue);
            }
            for (item, placement) in moves {
                placed(placement)?;
                set_owner_pose(txn, *item, gc_math::pose_from_xyz_rpy(placement.xyz, placement.rpy))?;
            }
            let held: Vec<AnyId> = moves.iter().map(|(i, _)| *i).collect();
            settle_holding(txn, &held)?;
            Ok((Vec::new(), "Move components"))
        }
        CellOp::MoveAndGround(moves) => {
            if moves.is_empty() {
                return Err(EditError::BadValue);
            }
            for (item, placement) in moves {
                placed(placement)?;
                set_owner_pose(txn, *item, gc_math::pose_from_xyz_rpy(placement.xyz, placement.rpy))?;
            }
            let held: Vec<AnyId> = moves.iter().map(|(i, _)| *i).collect();
            settle_holding(txn, &held)?;
            set_timeline(txn, |t| {
                for item in &held {
                    let o = crate::design::Occurrence::Item(*item);
                    if !t.components.grounded.contains(&o) {
                        t.components.grounded.push(o);
                    }
                }
            })?;
            Ok((Vec::new(), "Commit and ground"))
        }
        CellOp::SetDofShown { items, on } => {
            use crate::design::Occurrence;
            if items.is_empty() || !items.iter().all(|i| txn.doc().owner_pose(*i).is_some()) {
                return Err(EditError::BadValue);
            }
            set_timeline(txn, |t| {
                let shown = &mut t.components.dof_shown;
                shown.retain(|o| !items.iter().any(|i| *o == Occurrence::Item(*i)));
                if *on {
                    for item in items {
                        if !shown.contains(&Occurrence::Item(*item)) {
                            shown.push(Occurrence::Item(*item));
                        }
                    }
                }
            })?;
            Ok((Vec::new(), "Degrees of freedom"))
        }
        CellOp::SetDofShownAll(on) => {
            set_timeline(txn, |t| t.components.dof_all = *on)?;
            Ok((Vec::new(), "Degrees of freedom"))
        }
        CellOp::AddComponentFrame { name, sketch, occurrence, plane } => {
            let source = crate::design::PlaneSource::Component { occurrence: *occurrence, plane: *plane };
            let (mut created, _) =
                crate::command::run(txn, &crate::Command::CreateSketchOn { name: sketch.clone(), source })?;
            let Some(Created::Sketch(sketch)) = created.first().copied() else {
                return Err(EditError::BadValue);
            };
            // The sketch's own origin point, fixed at the plane's origin.
            let point = txn
                .doc()
                .get(sketch)
                .and_then(|s| {
                    s.points
                        .iter()
                        .find(|(_, p)| {
                            matches!(
                                p.kind,
                                gc_sketch::PointKind::External(gc_sketch::ExternalRef::FrameOrigin(_))
                            )
                        })
                        .map(|(id, _)| *id)
                })
                .ok_or(EditError::BadValue)?;
            let frame =
                WorkFrame { name: name.clone(), sketch, point, x_point: None, xy_point: None, rpy: [0.0; 3] };
            frame_ok(txn, &frame)?;
            created.push(Created::Frame(txn.insert(frame)));
            Ok((created, "Work frame on component"))
        }
        CellOp::SetContactSolver(mode) => {
            set_timeline(txn, |t| t.components.contact_solver = *mode)?;
            Ok((Vec::new(), "Interactive contact"))
        }
        CellOp::Subassembly(op) => crate::subassembly::run(txn, op),
        CellOp::SetJointLimits { id, angular, linear } => {
            let kind = txn.doc().get(*id).ok_or(EditError::BadValue)?.kind;
            let fits = |range: &Option<crate::relate::JointRange>, angular: bool| {
                range.is_none_or(|r| r.is_valid() && crate::relate::takes_limits(&kind, angular))
            };
            if !fits(angular, true) || !fits(linear, false) {
                return Err(EditError::BadValue);
            }
            let mut condition = txn.doc().joint_condition(*id).unwrap_or_default();
            condition.angular = *angular;
            condition.linear = *linear;
            set_timeline(txn, |t| {
                if condition == crate::relate::JointCondition::default() {
                    t.components.joint_conditions.remove(id);
                } else {
                    t.components.joint_conditions.insert(*id, condition);
                }
            })?;
            settle(txn)?;
            Ok((Vec::new(), "Joint limits"))
        }
        CellOp::SetJointCondition { id, lock, protect } => {
            let relationship = txn.doc().get(*id).ok_or(EditError::BadValue)?.clone();
            if !crate::relate::is_joint_kind(&relationship.kind) {
                return Err(EditError::BadValue);
            }
            let kept = txn.doc().joint_condition(*id).and_then(|c| c.locked);
            let locked = match (*lock, kept) {
                (false, _) => None,
                (true, Some(kept)) => Some(kept),
                (true, None) => Some(txn.doc().lock_pose(&relationship).ok_or(EditError::BadValue)?),
            };
            let ranges = txn.doc().joint_condition(*id).unwrap_or_default();
            let condition = crate::relate::JointCondition {
                locked,
                protected: *protect,
                angular: ranges.angular,
                linear: ranges.linear,
            };
            set_timeline(txn, |t| {
                if condition == crate::relate::JointCondition::default() {
                    t.components.joint_conditions.remove(id);
                } else {
                    t.components.joint_conditions.insert(*id, condition);
                }
            })?;
            settle(txn)?;
            Ok((Vec::new(), "Joint condition"))
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
            let outside =
                matches!(item.kind, Kind::Waypoint | Kind::Path | Kind::Relationship | Kind::SketchRole)
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
            if tool.is_none() {
                // The tool's relationships go with it, as a deleted component's do.
                let on_tool = crate::design::Occurrence::Tool { robot: *id };
                let gone: Vec<Id<crate::relate::Relationship>> = txn
                    .doc()
                    .relationships()
                    .iter()
                    .filter(|(_, r)| {
                        r.first.occurrence == on_tool
                            || r.second.occurrence == on_tool
                            || r.third().is_some_and(|t| t.occurrence == on_tool)
                    })
                    .map(|(id, _)| id)
                    .collect();
                for relationship in gone {
                    txn.remove(relationship)?;
                }
            }
            // What is tied to the tool follows a new mount.
            settle_holding(txn, &[id.erase()])?;
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
