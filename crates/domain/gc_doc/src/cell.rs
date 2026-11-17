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

/// A piece of equipment or an obstacle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Body {
    /// Name shown in the browser.
    pub name: String,
    /// Where it stands.
    pub placement: Placement,
    /// Its shape.
    pub solid: Solid,
}

impl Entity for Body {
    const KIND: Kind = Kind::Body;
    fn refs(&self, _: &mut Vec<AnyId>) {}
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
            WaypointTarget::Joints(_) => {}
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
    /// Sets a robot's tool centre point.
    SetTcp {
        /// The robot.
        id: Id<Robot>,
        /// The pose of the tool centre point in the tip link's frame.
        tcp: Placement,
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
    /// Deletes a path.
    DeletePath(Id<MotionPath>),
}

fn finite(values: &[f64]) -> bool {
    values.iter().all(|v| v.is_finite())
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
            Ok((vec![Created::Body(txn.insert(body.clone()))], "Add body"))
        }
        CellOp::SetBody { id, body } => {
            placed(&body.placement)?;
            if !body.solid.is_valid() {
                return Err(EditError::BadValue);
            }
            txn.modify(*id, |b: &mut Body, _| b.clone_from(body))?;
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
            Ok((vec![Created::Robot(txn.insert(robot.clone()))], "Add robot"))
        }
        CellOp::PlaceRobot { id, placement } => {
            placed(placement)?;
            txn.modify(*id, |r: &mut Robot, _| r.placement = *placement)?;
            Ok((Vec::new(), "Place robot"))
        }
        CellOp::SetTcp { id, tcp } => {
            placed(tcp)?;
            txn.modify(*id, |r: &mut Robot, _| r.tcp = *tcp)?;
            Ok((Vec::new(), "Set tool centre point"))
        }
        CellOp::SetJoints { id, joints } => {
            let count = txn.doc().get(*id).ok_or(DocError::Missing(id.erase()))?.joints.len();
            if joints.len() != count || !finite(joints) {
                return Err(EditError::BadValue);
            }
            txn.modify(*id, |r: &mut Robot, _| r.joints.clone_from(joints))?;
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
        CellOp::DeletePath(id) => {
            txn.remove(*id)?;
            Ok((Vec::new(), "Delete path"))
        }
    }
}
