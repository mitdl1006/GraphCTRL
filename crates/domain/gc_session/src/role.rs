//! Roles of sketch elements in a session (plan 9.1, "sketch → robot paths and waypoints"): a sketch point
//! becomes a waypoint, a chain of sketch curves a path, for whichever robot is given the work. See
//! `gc_doc::SketchRole`. Every edit here is one undo step.

use gc_core::Id;
use gc_doc::{
    AimReference, CellOp, Command, Created, EditError, MotionPath, PathMotion, Robot, RoleKind, Sketch,
    SketchRole, ToolAim,
};
use gc_math::DPose3;
use gc_sketch::{Curve, SkPoint};

use crate::{PLANNERS, Session};

impl Session {
    fn role_op(&mut self, op: CellOp) -> Result<Vec<Created>, EditError> {
        self.execute(&Command::Cell(op))
    }

    /// A name no role has yet: `prefix` and a number.
    fn role_name(&self, prefix: &str, taken: &[String]) -> String {
        (self.doc().roles().len() + 1..)
            .map(|n| format!("{prefix}{n}"))
            .find(|name| !taken.contains(name) && !self.doc().roles().iter().any(|(_, r)| &r.name == name))
            .unwrap_or_default()
    }

    fn new_waypoint_role(&self, sketch: Id<Sketch>, point: Id<SkPoint>, taken: &[String]) -> SketchRole {
        SketchRole {
            name: self.role_name("W", taken),
            sketch,
            kind: RoleKind::Waypoint { point },
            aim: ToolAim::default(),
            travel: gc_doc::PathTravel::default(),
        }
    }

    fn new_path_role(&self, sketch: Id<Sketch>, curves: &[Id<Curve>], taken: &[String]) -> SketchRole {
        SketchRole {
            name: self.role_name("Route", taken),
            sketch,
            kind: RoleKind::Path {
                curves: curves.to_vec(),
                reversed: false,
                motion: PathMotion::Follow,
                tcp_speed: None,
            },
            aim: ToolAim::default(),
            travel: gc_doc::PathTravel::default(),
        }
    }

    /// Makes a sketch point (2D or 3D) a waypoint: the tool tip goes there, pointing against the sketch
    /// plane's normal until [`Session::set_role_aim`] says otherwise. Refused (`InUse`) when the point
    /// already is one.
    pub fn add_waypoint_role(
        &mut self,
        sketch: Id<Sketch>,
        point: Id<SkPoint>,
    ) -> Result<Id<SketchRole>, EditError> {
        let role = self.new_waypoint_role(sketch, point, &[]);
        match self.role_op(CellOp::AddRole(role))?.first() {
            Some(Created::Role(id)) => Ok(*id),
            _ => Err(EditError::BadValue),
        }
    }

    /// Makes open sketch curves (lines, arcs, elliptical arcs, open splines) that join end to end a path,
    /// travelled along the curves from the free end of the first curve given. The curves may be given in
    /// any order. Refused (`BadValue`) when they do not form one chain or one is closed (a circle, a
    /// whole ellipse, a closed spline), and (`InUse`) when one already belongs to a path role.
    pub fn add_path_role(
        &mut self,
        sketch: Id<Sketch>,
        curves: &[Id<Curve>],
    ) -> Result<Id<SketchRole>, EditError> {
        let role = self.new_path_role(sketch, curves, &[]);
        match self.role_op(CellOp::AddRole(role))?.first() {
            Some(Created::Role(id)) => Ok(*id),
            _ => Err(EditError::BadValue),
        }
    }

    /// Gives a selection of one sketch its roles in one undo step: every point that is not a waypoint yet
    /// becomes one, and the curves (when there are any) become one path. The new roles, waypoints first.
    /// Refused as a whole when the curves do not form one chain or nothing new would be made.
    pub fn assign_roles(
        &mut self,
        sketch: Id<Sketch>,
        points: &[Id<SkPoint>],
        curves: &[Id<Curve>],
    ) -> Result<Vec<Id<SketchRole>>, EditError> {
        let mut names = Vec::new();
        let mut ops = Vec::new();
        for point in points {
            if self.doc().waypoint_role(sketch, *point).is_none() {
                let role = self.new_waypoint_role(sketch, *point, &names);
                names.push(role.name.clone());
                ops.push(Command::Cell(CellOp::AddRole(role)));
            }
        }
        if !curves.is_empty() {
            ops.push(Command::Cell(CellOp::AddRole(self.new_path_role(sketch, curves, &names))));
        }
        let created = self.execute(&Command::Sequence(ops))?;
        Ok(created
            .into_iter()
            .filter_map(|c| match c {
                Created::Role(id) => Some(id),
                _ => None,
            })
            .collect())
    }

    /// The chain a sketch curve belongs to, for picking a whole path with one click: the curve and the
    /// open curves joined to it end to end, up to a free end, a branch, or a curve that already belongs to
    /// a path role. Empty for a closed curve or one that has a path role.
    pub fn chain_at(&self, sketch: Id<Sketch>, curve: Id<Curve>) -> Vec<Id<Curve>> {
        let Some(s) = self.doc().get(sketch) else { return Vec::new() };
        let taken: Vec<Id<Curve>> =
            s.curves.keys().filter(|c| self.doc().path_role(sketch, **c).is_some()).copied().collect();
        s.chain_at(curve, &taken)
    }

    /// The waypoint role of a sketch point, if it has one.
    pub fn waypoint_role(&self, sketch: Id<Sketch>, point: Id<SkPoint>) -> Option<Id<SketchRole>> {
        self.doc().waypoint_role(sketch, point)
    }

    /// The path role a sketch curve belongs to, if any.
    pub fn path_role(&self, sketch: Id<Sketch>, curve: Id<Curve>) -> Option<Id<SketchRole>> {
        self.doc().path_role(sketch, curve)
    }

    /// The roles of a sketch's elements, waypoints and paths, in the order they were made.
    pub fn sketch_roles(&self, sketch: Id<Sketch>) -> Vec<Id<SketchRole>> {
        self.doc().roles().iter().filter(|(_, r)| r.sketch == sketch).map(|(id, _)| id).collect()
    }

    /// Replaces a role's data (elements, travel, speed, attitude, name).
    pub fn set_role(&mut self, id: Id<SketchRole>, role: SketchRole) -> Result<(), EditError> {
        self.role_op(CellOp::SetRole { id, role }).map(|_| ())
    }

    fn edit_role(&mut self, id: Id<SketchRole>, edit: impl FnOnce(&mut SketchRole)) -> Result<(), EditError> {
        let mut role = self.doc().get(id).ok_or(EditError::BadValue)?.clone();
        let before = role.clone();
        edit(&mut role);
        if role == before {
            return Ok(());
        }
        self.set_role(id, role)
    }

    /// How the tool is held at a waypoint role or along a path role.
    pub fn set_role_aim(&mut self, id: Id<SketchRole>, aim: ToolAim) -> Result<(), EditError> {
        self.edit_role(id, |r| r.aim = aim)
    }

    /// How a path role is travelled: along the curves, in straight lines between their ends, or by joint
    /// moves between their ends. Refused for a waypoint role.
    pub fn set_role_motion(&mut self, id: Id<SketchRole>, to: PathMotion) -> Result<(), EditError> {
        self.path_role_edit(id, |motion, _, _| *motion = to)
    }

    /// The fastest the tool tip may go along a path role, m/s; `None` for as fast as the joints allow.
    /// Refused for a waypoint role or a speed that is not positive.
    pub fn set_role_speed(&mut self, id: Id<SketchRole>, speed: Option<f64>) -> Result<(), EditError> {
        self.path_role_edit(id, |_, tcp_speed, _| *tcp_speed = speed)
    }

    /// Whether the tool turns with a path role, its X axis pointing the way it travels (seen in the plane
    /// the attitude is measured from), instead of keeping one attitude. Refused for a waypoint role.
    pub fn set_role_tangent(&mut self, id: Id<SketchRole>, tangent: bool) -> Result<(), EditError> {
        if !self.doc().get(id).is_some_and(SketchRole::is_path) {
            return Err(EditError::BadValue);
        }
        self.edit_role(id, |r| r.travel.tangent = tangent)
    }

    /// Where a closed path role (a circle, a whole ellipse, a closed spline, a closed loop of curves)
    /// starts and ends: at its place nearest to this point of its sketch; `None` for where the curve
    /// itself starts. Refused for a waypoint role or a point that is not in the sketch.
    pub fn set_role_start(
        &mut self,
        id: Id<SketchRole>,
        start: Option<Id<SkPoint>>,
    ) -> Result<(), EditError> {
        if !self.doc().get(id).is_some_and(SketchRole::is_path) {
            return Err(EditError::BadValue);
        }
        self.edit_role(id, |r| r.travel.start = start)
    }

    /// True when a path role is closed: it ends where it starts ([`Session::set_role_start`] applies).
    pub fn role_closed(&self, id: Id<SketchRole>) -> bool {
        self.doc().role_runs(id).is_some_and(|runs| {
            match (runs.first().and_then(|r| r.first()), runs.last().and_then(|r| r.last())) {
                (Some(first), Some(last)) => (*first - *last).length() <= 1e-7,
                _ => false,
            }
        })
    }

    /// Travels a path role from its other end. Refused for a waypoint role.
    pub fn reverse_role(&mut self, id: Id<SketchRole>) -> Result<(), EditError> {
        self.path_role_edit(id, |_, _, reversed| *reversed = !*reversed)
    }

    fn path_role_edit(
        &mut self,
        id: Id<SketchRole>,
        edit: impl FnOnce(&mut PathMotion, &mut Option<f64>, &mut bool),
    ) -> Result<(), EditError> {
        if !self.doc().get(id).is_some_and(SketchRole::is_path) {
            return Err(EditError::BadValue);
        }
        self.edit_role(id, |r| {
            if let RoleKind::Path { motion, tcp_speed, reversed, .. } = &mut r.kind {
                edit(motion, tcp_speed, reversed);
            }
        })
    }

    /// Takes a role away from its sketch elements (the elements stay). Refused (`InUse`) while a robot's
    /// path follows it.
    pub fn delete_role(&mut self, id: Id<SketchRole>) -> Result<(), EditError> {
        self.role_op(CellOp::DeleteRole(id)).map(|_| ())
    }

    /// The world pose of the tool at a waypoint role, or at the start of a path role.
    pub fn role_pose(&self, id: Id<SketchRole>) -> Option<DPose3> {
        self.role_ends(id).map(|(start, _)| start)
    }

    /// The world poses of the tool where a path role starts and ends (both the same for a waypoint role).
    pub fn role_ends(&self, id: Id<SketchRole>) -> Option<(DPose3, DPose3)> {
        let doc = self.doc();
        if let Some(pose) = doc.role_pose(id) {
            return Some((pose, pose));
        }
        let runs = doc.role_poses(id)?;
        Some((*runs.first()?.first()?, *runs.last()?.last()?))
    }

    /// World points along a path role in the direction of travel, for drawing it: the curves themselves
    /// when it is followed, the straight lines between the curves' ends for linear and joint moves (where
    /// a joint move only passes through those ends). `None` for a waypoint role or a broken chain.
    pub fn role_polyline(&self, id: Id<SketchRole>) -> Option<Vec<[f64; 3]>> {
        let doc = self.doc();
        let runs = doc.role_runs(id)?;
        let follow = matches!(doc.get(id)?.kind, RoleKind::Path { motion: PathMotion::Follow, .. });
        let mut points = vec![runs.first()?.first()?.to_array()];
        for run in &runs {
            if follow {
                points.extend(run.iter().skip(1).map(|p| p.to_array()));
            } else {
                points.push(run.last()?.to_array());
            }
        }
        Some(points)
    }

    /// The work frames a role's attitude can be measured from, besides the sketch plane.
    pub fn role_aim_references(&self) -> Vec<AimReference> {
        std::iter::once(AimReference::SketchPlane)
            .chain(self.doc().work_frames().iter().map(|(id, _)| AimReference::Frame(id)))
            .collect()
    }

    /// Adds a path of `robot` that follows path roles in order, moving freely to the start of each. It is
    /// planned with [`Session::plan`] like any path and reads the roles then, so it follows the sketch and
    /// the roles' settings (its plan is out of date after either changes).
    pub fn follow_roles(
        &mut self,
        robot: Id<Robot>,
        roles: &[Id<SketchRole>],
    ) -> Result<Id<MotionPath>, EditError> {
        let path = self.new_path(robot, PLANNERS[0].0, roles.to_vec());
        match self.role_op(CellOp::AddPath(path))?.first() {
            Some(Created::Path(id)) => Ok(*id),
            _ => Err(EditError::BadValue),
        }
    }

    /// Replaces the path roles a path follows.
    pub fn set_path_follow(
        &mut self,
        path: Id<MotionPath>,
        roles: &[Id<SketchRole>],
    ) -> Result<(), EditError> {
        self.role_op(CellOp::SetPathFollow { id: path, follow: roles.to_vec() }).map(|_| ())
    }

    fn new_path(&self, robot: Id<Robot>, planner: &str, follow: Vec<Id<SketchRole>>) -> MotionPath {
        MotionPath {
            name: format!("Path{}", self.doc().paths().len() + 1),
            robot,
            waypoints: Vec::new(),
            linear: Vec::new(),
            planner: planner.to_owned(),
            seed: 1,
            tcp_speed: None,
            contacts: Vec::new(),
            follow,
        }
    }

    /// "Create path" between two sketch points: adds a path of `robot` from its current state to the
    /// first point and on to the second, found with the planner of this id ([`Session::planner_choices`]),
    /// or with `linear` in a straight line of the tool tip between the points, and starts planning it
    /// ([`Session::plan`]; the result arrives like any plan's). The robot's waypoints on the points are
    /// re-used when it has them and made otherwise; the tool is held as the points' waypoint roles say.
    /// One undo step. Refused for an unknown planner, the same point twice or a point that does not exist.
    pub fn route_between(
        &mut self,
        robot: Id<Robot>,
        from: (Id<Sketch>, Id<SkPoint>),
        to: (Id<Sketch>, Id<SkPoint>),
        planner: &str,
        linear: bool,
    ) -> Result<Id<MotionPath>, EditError> {
        if from == to || !self.planner_choices().iter().any(|(id, ..)| id == planner) {
            return Err(EditError::BadValue);
        }
        let mut path = self.new_path(robot, planner, Vec::new());
        path.linear = vec![false, linear];
        let created = self.role_op(CellOp::AddPathThrough { path, stops: vec![from, to] })?;
        let Some(Created::Path(id)) = created.last() else { return Err(EditError::BadValue) };
        self.plan(*id);
        Ok(*id)
    }
}
