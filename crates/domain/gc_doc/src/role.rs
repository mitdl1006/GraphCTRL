//! What sketch elements are for a robot (plan 9.1, "sketch → robot paths and waypoints"; plan 9.4: work
//! is defined on the structure, not on a robot).
//!
//! A [`SketchRole`] marks a sketch point as a **waypoint** or a chain of sketch curves as a **path**, with
//! how the tool is held there and, for a path, how it is travelled. A role belongs to no robot: a robot's
//! waypoint on the point ([`crate::WaypointTarget::SketchPoint`]) and a robot's path that follows the role
//! ([`crate::MotionPath::follow`]) read the role when they are planned, so they follow the sketch's
//! dimensions and the role's settings.

use gc_core::{AnyId, Entity, Id, Kind};
use gc_math::{DPose3, DQuat, DVec3};
use gc_sketch::{Curve, SkPoint, Sketch, Space};
use serde::{Deserialize, Serialize};

use crate::cell::{CellOp, MotionPath, Waypoint, WaypointTarget, WorkFrame};
use crate::command::{Created, EditError};
use crate::document::Document;
use crate::txn::Txn;

/// What the tool's attitude is measured from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AimReference {
    /// The sketch: its plane, or for a 3D sketch its coordinate frame.
    #[default]
    SketchPlane,
    /// A work frame.
    Frame(Id<WorkFrame>),
}

/// How the tool is held at a waypoint or along a path: its Z axis points against the reference's Z axis
/// (into the sketch plane, the tool's X along the reference's X), then turned by `rpy`. The default
/// is the tool pointing straight against the sketch plane's normal.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ToolAim {
    /// What the attitude is measured from.
    #[serde(default)]
    pub reference: AimReference,
    /// Roll, pitch, yaw (radians) on top of pointing against the reference's Z axis.
    #[serde(default)]
    pub rpy: [f64; 3],
}

/// How the tool travels a path.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PathMotion {
    /// The tool tip follows the curves themselves (lines, arcs, splines).
    #[default]
    Follow,
    /// Straight lines of the tool tip from one end of each curve to the other (a linear move).
    Linear,
    /// Joint moves from one end of each curve to the other, planned freely (the tip leaves the curve).
    Joint,
}

/// What a sketch element is for a robot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum RoleKind {
    /// A sketch point (2D or 3D) the tool tip goes to.
    Waypoint {
        /// The point.
        point: Id<SkPoint>,
    },
    /// Open sketch curves joined end to end (lines, arcs, elliptical arcs, open splines) the tool tip
    /// travels along.
    Path {
        /// The curves in the order they are travelled (kept in chain order, see `Sketch::chain`).
        curves: Vec<Id<Curve>>,
        /// Travel the chain from its other end.
        #[serde(default)]
        reversed: bool,
        /// How it is travelled.
        #[serde(default)]
        motion: PathMotion,
        /// The fastest the tool tip may go, m/s; `None` for as fast as the joints allow.
        #[serde(default)]
        tcp_speed: Option<f64>,
    },
}

/// A waypoint or path role of sketch elements.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SketchRole {
    /// Name shown in the browser.
    pub name: String,
    /// The sketch the elements are in.
    pub sketch: Id<Sketch>,
    /// Waypoint or path.
    pub kind: RoleKind,
    /// How the tool is held.
    #[serde(default)]
    pub aim: ToolAim,
}

impl SketchRole {
    /// True for a path role.
    pub fn is_path(&self) -> bool {
        matches!(self.kind, RoleKind::Path { .. })
    }
}

impl Entity for SketchRole {
    const KIND: Kind = Kind::SketchRole;
    fn refs(&self, out: &mut Vec<AnyId>) {
        out.push(self.sketch.erase());
        if let AimReference::Frame(frame) = self.aim.reference {
            out.push(frame.erase());
        }
    }
}

impl Document {
    /// The world pose of a sketch's coordinate frame (its plane, X and Y in the plane).
    fn sketch_pose(&self, sketch: &Sketch) -> Option<DPose3> {
        let (Space::Planar(frame) | Space::Spatial(frame)) = sketch.space;
        self.frame_world_pose(Id::from_raw(frame.raw))
    }

    /// The world pose of a work frame given by its data: at its origin point, with the axes its points
    /// give (the sketch's without them), turned by its angles. `None` when a point is missing or the
    /// points give no direction.
    pub fn work_frame_pose_of(&self, frame: &WorkFrame) -> Option<DPose3> {
        let sketch = self.get(frame.sketch)?;
        let base = self.sketch_pose(sketch)?;
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
        Some(DPose3::from_parts(
            origin,
            axes * gc_math::quat_from_rpy(frame.rpy[0], frame.rpy[1], frame.rpy[2]),
        ))
    }

    /// The world attitude of the tool for elements of `sketch` held as `aim` says.
    pub fn aim_rotation(&self, sketch: Id<Sketch>, aim: &ToolAim) -> Option<DQuat> {
        let base = match aim.reference {
            AimReference::SketchPlane => self.sketch_pose(self.get(sketch)?)?.rotation,
            AimReference::Frame(frame) => self.work_frame_pose_of(self.get(frame)?)?.rotation,
        };
        let against = base * DQuat::from_rotation_x(std::f64::consts::PI);
        // Untouched angles leave the attitude exactly as it is without them.
        if aim.rpy.iter().all(|v| v.abs() == 0.0) {
            return Some(against);
        }
        Some((against * gc_math::quat_from_rpy(aim.rpy[0], aim.rpy[1], aim.rpy[2])).normalize())
    }

    /// The waypoint role of a sketch point, if it has one.
    pub fn waypoint_role(&self, sketch: Id<Sketch>, point: Id<SkPoint>) -> Option<Id<SketchRole>> {
        self.roles()
            .iter()
            .find(|(_, r)| {
                r.sketch == sketch && matches!(r.kind, RoleKind::Waypoint { point: p } if p == point)
            })
            .map(|(id, _)| id)
    }

    /// The path role a sketch curve belongs to, if any.
    pub fn path_role(&self, sketch: Id<Sketch>, curve: Id<Curve>) -> Option<Id<SketchRole>> {
        self.roles()
            .iter()
            .find(|(_, r)| {
                r.sketch == sketch
                    && matches!(&r.kind, RoleKind::Path { curves, .. } if curves.contains(&curve))
            })
            .map(|(id, _)| id)
    }

    /// The world pose the tool takes at a sketch point: its tip on the point, held as the point's waypoint
    /// role says, or without one pointing against the sketch plane's normal.
    pub fn sketch_point_pose(&self, sketch: Id<Sketch>, point: Id<SkPoint>) -> Option<DPose3> {
        let aim = self.waypoint_role(sketch, point).and_then(|id| self.get(id)).map(|r| r.aim);
        self.point_pose_aimed(sketch, point, &aim.unwrap_or_default())
    }

    fn point_pose_aimed(&self, sketch_id: Id<Sketch>, point: Id<SkPoint>, aim: &ToolAim) -> Option<DPose3> {
        let sketch = self.get(sketch_id)?;
        let frame = self.sketch_pose(sketch)?;
        let pos = sketch.points.get(&point)?.pos;
        let world = frame.translation + frame.rotation * DVec3::from_array(pos);
        Some(DPose3::from_parts(world, self.aim_rotation(sketch_id, aim)?))
    }

    /// The world pose of the tool at a waypoint role. `None` for a path role or a missing point.
    pub fn role_pose(&self, role: Id<SketchRole>) -> Option<DPose3> {
        let role = self.get(role)?;
        match role.kind {
            RoleKind::Waypoint { point } => self.point_pose_aimed(role.sketch, point, &role.aim),
            RoleKind::Path { .. } => None,
        }
    }

    /// A path role's curves as the tool travels them: for each curve, world points along it from where
    /// the tool enters it to where it leaves (two for a line), each curve starting where the one before
    /// ended. `None` for a waypoint role, a missing curve, or curves that no longer join into a chain.
    pub fn role_runs(&self, role: Id<SketchRole>) -> Option<Vec<Vec<DVec3>>> {
        let role = self.get(role)?;
        let RoleKind::Path { curves, reversed, .. } = &role.kind else { return None };
        let sketch = self.get(role.sketch)?;
        let frame = self.sketch_pose(sketch)?;
        let mut chain = sketch.chain(curves)?;
        if *reversed {
            chain.reverse();
            chain.iter_mut().for_each(|(_, flipped)| *flipped = !*flipped);
        }
        let mut runs = Vec::with_capacity(chain.len());
        for (curve, flipped) in chain {
            let mut run = sketch.curve_run(curve)?;
            if flipped {
                run.reverse();
            }
            runs.push(
                run.into_iter()
                    .map(|p| frame.translation + frame.rotation * DVec3::from_array(p))
                    .collect::<Vec<DVec3>>(),
            );
        }
        Some(runs)
    }

    /// The world attitude of the tool along a path role (the same all the way), or at a waypoint role.
    pub fn role_rotation(&self, role: Id<SketchRole>) -> Option<DQuat> {
        let role = self.get(role)?;
        self.aim_rotation(role.sketch, &role.aim)
    }

    /// True when a robot's path follows the role.
    pub fn role_followed(&self, role: Id<SketchRole>) -> bool {
        self.paths().iter().any(|(_, p)| p.follow.contains(&role))
    }
}

/// Checks a role and returns it as it is stored (a path's curves in chain order). `except` is the role
/// being replaced.
fn checked(
    doc: &Document,
    role: &SketchRole,
    except: Option<Id<SketchRole>>,
) -> Result<SketchRole, EditError> {
    let sketch = doc.get(role.sketch).ok_or(EditError::BadValue)?;
    if !role.aim.rpy.iter().all(|v| v.is_finite()) {
        return Err(EditError::BadValue);
    }
    if let AimReference::Frame(frame) = role.aim.reference
        && doc.get(frame).is_none()
    {
        return Err(EditError::BadValue);
    }
    let others = || doc.roles().iter().filter(|(id, r)| Some(*id) != except && r.sketch == role.sketch);
    let mut stored = role.clone();
    match &mut stored.kind {
        RoleKind::Waypoint { point } => {
            if !sketch.points.contains_key(point) {
                return Err(EditError::BadValue);
            }
            // One waypoint role per point.
            if others().any(|(_, r)| matches!(r.kind, RoleKind::Waypoint { point: p } if p == *point)) {
                return Err(EditError::InUse);
            }
        }
        RoleKind::Path { curves, tcp_speed, .. } => {
            if tcp_speed.is_some_and(|v| !(v.is_finite() && v > 0.0)) {
                return Err(EditError::BadValue);
            }
            let chain = sketch.chain(curves).filter(|c| !c.is_empty()).ok_or(EditError::BadValue)?;
            // A curve is travelled by one path role.
            let taken = others().any(|(_, r)| {
                matches!(&r.kind, RoleKind::Path { curves: theirs, .. } if theirs.iter().any(|c| curves.contains(c)))
            });
            if taken {
                return Err(EditError::InUse);
            }
            *curves = chain.into_iter().map(|(id, _)| id).collect();
        }
    }
    Ok(stored)
}

/// Checks the roles a path follows: path roles that exist, each once.
pub(crate) fn follow_ok(doc: &Document, follow: &[Id<SketchRole>]) -> bool {
    follow
        .iter()
        .enumerate()
        .all(|(index, id)| doc.get(*id).is_some_and(SketchRole::is_path) && !follow[..index].contains(id))
}

/// A robot's waypoint on a sketch point: the one it has, or a new one.
fn waypoint_on(
    txn: &mut Txn<'_>,
    robot: Id<crate::cell::Robot>,
    sketch: Id<Sketch>,
    point: Id<SkPoint>,
    created: &mut Vec<Created>,
) -> Result<Id<Waypoint>, EditError> {
    let target = WaypointTarget::SketchPoint { sketch, point };
    if let Some((id, _)) = txn.doc().waypoints().iter().find(|(_, w)| w.robot == robot && w.target == target)
    {
        return Ok(id);
    }
    if !txn.doc().get(sketch).is_some_and(|s| s.points.contains_key(&point)) {
        return Err(EditError::BadValue);
    }
    let name = format!("P{}", txn.doc().waypoints().len() + 1);
    let id = txn.insert(Waypoint { name, robot, target });
    created.push(Created::Waypoint(id));
    Ok(id)
}

/// Runs the role operations of [`CellOp`]; any other operation is refused.
pub(crate) fn run(txn: &mut Txn<'_>, op: &CellOp) -> Result<(Vec<Created>, &'static str), EditError> {
    match op {
        CellOp::AddRole(role) => {
            let role = checked(txn.doc(), role, None)?;
            Ok((vec![Created::Role(txn.insert(role))], "Add sketch role"))
        }
        CellOp::SetRole { id, role } => {
            let old = txn.doc().get(*id).ok_or(EditError::BadValue)?;
            // A path a robot follows stays a path.
            if old.is_path() && !role.is_path() && txn.doc().role_followed(*id) {
                return Err(EditError::InUse);
            }
            let role = checked(txn.doc(), role, Some(*id))?;
            txn.modify(*id, |r: &mut SketchRole, _| *r = role)?;
            Ok((Vec::new(), "Edit sketch role"))
        }
        CellOp::DeleteRole(id) => {
            if txn.doc().role_followed(*id) {
                return Err(EditError::InUse);
            }
            txn.remove(*id)?;
            Ok((Vec::new(), "Delete sketch role"))
        }
        CellOp::SetPathFollow { id, follow } => {
            let path = txn.doc().get(*id).ok_or(EditError::BadValue)?;
            // A path goes through waypoints or follows roles; it must keep something to do.
            if !follow_ok(txn.doc(), follow) || (follow.is_empty() && path.waypoints.is_empty()) {
                return Err(EditError::BadValue);
            }
            if !follow.is_empty() && !path.waypoints.is_empty() {
                return Err(EditError::BadValue);
            }
            txn.modify(*id, |p: &mut MotionPath, _| {
                p.follow.clone_from(follow);
                p.contacts.retain(|c| c.index < follow.len());
            })?;
            Ok((Vec::new(), "Edit path"))
        }
        CellOp::AddFollowPath { path, roles } => {
            let mut created = Vec::new();
            let mut follow = path.follow.clone();
            for role in roles {
                if !role.is_path() {
                    return Err(EditError::BadValue);
                }
                let role = checked(txn.doc(), role, None)?;
                let id = txn.insert(role);
                created.push(Created::Role(id));
                follow.push(id);
            }
            if follow.is_empty()
                || !follow_ok(txn.doc(), &follow)
                || !path.waypoints.is_empty()
                || txn.doc().get(path.robot).is_none()
            {
                return Err(EditError::BadValue);
            }
            let path = MotionPath { follow, ..path.clone() };
            created.push(Created::Path(txn.insert(path)));
            Ok((created, "Add path"))
        }
        CellOp::AddPathThrough { path, stops } => {
            if stops.is_empty() || !path.follow.is_empty() || txn.doc().get(path.robot).is_none() {
                return Err(EditError::BadValue);
            }
            let same_robot =
                path.waypoints.iter().all(|w| txn.doc().get(*w).is_some_and(|w| w.robot == path.robot));
            if !same_robot {
                return Err(EditError::BadValue);
            }
            let mut created = Vec::new();
            let mut waypoints = path.waypoints.clone();
            for (sketch, point) in stops {
                waypoints.push(waypoint_on(txn, path.robot, *sketch, *point, &mut created)?);
            }
            let path = MotionPath { waypoints, ..path.clone() };
            created.push(Created::Path(txn.insert(path)));
            Ok((created, "Add path"))
        }
        _ => Err(EditError::BadValue),
    }
}

/// After an edit: roles whose sketch elements are gone go too (in the same undo step). A path role that
/// lost some of its curves keeps the rest when they still form one chain. A role a robot's path follows
/// stays as it is; that path then fails to plan and says why.
pub(crate) fn prune(txn: &mut Txn<'_>) -> Result<(), EditError> {
    enum Fix {
        Remove,
        Keep(Vec<Id<Curve>>),
    }
    let doc = txn.doc();
    let mut fixes = Vec::new();
    for (id, role) in doc.roles().iter() {
        let sketch = doc.get(role.sketch);
        let fix = match (&role.kind, sketch) {
            (_, None) => Some(Fix::Remove),
            (RoleKind::Waypoint { point }, Some(s)) => (!s.points.contains_key(point)).then_some(Fix::Remove),
            (RoleKind::Path { curves, .. }, Some(s)) => {
                if curves.iter().all(|c| s.curves.contains_key(c)) {
                    None
                } else {
                    let left: Vec<Id<Curve>> =
                        curves.iter().filter(|c| s.curves.contains_key(c)).copied().collect();
                    match s.chain(&left) {
                        Some(chain) if !chain.is_empty() => {
                            Some(Fix::Keep(chain.into_iter().map(|(c, _)| c).collect()))
                        }
                        _ => Some(Fix::Remove),
                    }
                }
            }
        };
        if let Some(fix) = fix {
            fixes.push((id, fix));
        }
    }
    for (id, fix) in fixes {
        match fix {
            Fix::Keep(left) => txn.modify(id, |r: &mut SketchRole, _| {
                if let RoleKind::Path { curves, .. } = &mut r.kind {
                    *curves = left;
                }
            })?,
            Fix::Remove if txn.doc().role_followed(id) => {}
            Fix::Remove => {
                txn.remove(id)?;
                // It is no longer suppressed or hidden either.
                if txn.doc().timeline().is_some_and(|(_, t)| {
                    t.suppressed.contains(&id.erase()) || t.hidden.contains(&id.erase())
                }) {
                    crate::cell::set_timeline(txn, |t| {
                        t.suppressed.retain(|s| *s != id.erase());
                        t.hidden.retain(|h| *h != id.erase());
                    })?;
                }
            }
        }
    }
    Ok(())
}
