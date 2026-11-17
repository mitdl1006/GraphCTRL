//! What each agent call does to the session. Runs on the application's thread, between frames, so every
//! edit is an ordinary command: it shows at once and is undone with Ctrl+Z like the user's own.

use gc_core::Id;
use gc_doc::{CellOp, Command, MotionPath, Placement, Robot, Waypoint, WaypointTarget};
use gc_math::DPose3;
use gc_session::library::Package;
use gc_session::{PlanStatus, Reference, Session};
use serde_json::{Value, json};

/// A call from an agent.
#[derive(Clone, Debug, PartialEq)]
pub enum Call {
    /// The cell: robots, bodies, frames, waypoints and paths with their state.
    Overview,
    /// The robots of the robot library.
    LibraryRobots,
    /// Adds a robot from the library by its name.
    AddRobot {
        /// The library name, e.g. `UR5e`.
        name: String,
    },
    /// Places a robot's base in the world.
    PlaceRobot {
        /// The robot's name.
        robot: String,
        /// Position in millimetres.
        xyz_mm: [f64; 3],
        /// Roll, pitch, yaw in degrees.
        rpy_deg: [f64; 3],
    },
    /// Adds a waypoint for a robot, as a pose measured from a frame.
    AddWaypoint {
        /// The robot the waypoint is for.
        robot: String,
        /// `world`, `base` or the name of a work frame.
        reference: String,
        /// Position in millimetres.
        xyz_mm: [f64; 3],
        /// Roll, pitch, yaw in degrees.
        rpy_deg: [f64; 3],
    },
    /// A waypoint's pose seen from every frame, and whether the robot reaches it.
    WaypointInfo {
        /// The waypoint's name.
        name: String,
    },
    /// Makes a path through waypoints; `linear` says which are approached in a straight line.
    CreatePath {
        /// Waypoint names in order.
        waypoints: Vec<String>,
        /// Per waypoint, a straight line of the tool tip instead of a planned motion.
        linear: Vec<bool>,
    },
    /// Plans a path.
    PlanPath {
        /// The path's name.
        path: String,
    },
    /// Undoes the last edit.
    Undo,
    /// Redoes the last undone edit.
    Redo,
}

impl Call {
    /// A short description for the activity log.
    pub fn summary(&self) -> String {
        match self {
            Self::Overview => "overview".to_owned(),
            Self::LibraryRobots => "library_robots".to_owned(),
            Self::AddRobot { name } => format!("add_robot {name}"),
            Self::PlaceRobot { robot, .. } => format!("place_robot {robot}"),
            Self::AddWaypoint { robot, reference, .. } => format!("add_waypoint {robot} in {reference}"),
            Self::WaypointInfo { name } => format!("waypoint_info {name}"),
            Self::CreatePath { waypoints, .. } => format!("create_path {}", waypoints.join(",")),
            Self::PlanPath { path } => format!("plan_path {path}"),
            Self::Undo => "undo".to_owned(),
            Self::Redo => "redo".to_owned(),
        }
    }

    /// True when the call changes the document.
    pub fn edits(&self) -> bool {
        !matches!(self, Self::Overview | Self::LibraryRobots | Self::WaypointInfo { .. })
    }
}

fn mm(v: f64) -> f64 {
    (v * 1000.0 * 1000.0).round() / 1000.0
}

fn pose_json(pose: &DPose3) -> Value {
    let [r, p, y] = gc_math::rpy_from_quat(pose.rotation).map(|a| (a.to_degrees() * 1000.0).round() / 1000.0);
    json!({ "xyz_mm": pose.translation.to_array().map(mm), "rpy_deg": [r, p, y] })
}

fn placement(xyz_mm: [f64; 3], rpy_deg: [f64; 3]) -> Placement {
    Placement { xyz: xyz_mm.map(|v| v / 1000.0), rpy: rpy_deg.map(f64::to_radians) }
}

fn robot_named(session: &Session, name: &str) -> Result<Id<Robot>, String> {
    session
        .doc()
        .robots()
        .iter()
        .find(|(_, r)| r.name == name)
        .map(|(id, _)| id)
        .ok_or_else(|| format!("no robot named `{name}`; call overview for the names"))
}

fn waypoint_named(session: &Session, name: &str) -> Result<Id<Waypoint>, String> {
    session
        .doc()
        .waypoints()
        .iter()
        .find(|(_, w)| w.name == name)
        .map(|(id, _)| id)
        .ok_or_else(|| format!("no waypoint named `{name}`"))
}

fn path_named(session: &Session, name: &str) -> Result<Id<MotionPath>, String> {
    session
        .doc()
        .paths()
        .iter()
        .find(|(_, p)| p.name == name)
        .map(|(id, _)| id)
        .ok_or_else(|| format!("no path named `{name}`"))
}

fn reference_named(session: &Session, name: &str) -> Result<Reference, String> {
    match name {
        "world" => Ok(Reference::World),
        "base" => Ok(Reference::Base),
        frame => session
            .doc()
            .work_frames()
            .iter()
            .find(|(_, f)| f.name == frame)
            .map(|(id, _)| Reference::Frame(id))
            .ok_or_else(|| format!("`{frame}` is not world, base or a work frame")),
    }
}

fn status_name(status: PlanStatus) -> &'static str {
    match status {
        PlanStatus::NotPlanned => "not_planned",
        PlanStatus::Planning => "planning",
        PlanStatus::Ready => "ready",
        PlanStatus::Stale => "stale",
        PlanStatus::Failed => "failed",
    }
}

/// Carries out a call. `library` is the robot library's packages.
pub fn execute(session: &mut Session, library: &[Package], call: &Call) -> Result<Value, String> {
    let edit_error = |e: gc_doc::EditError| e.to_string();
    match call {
        Call::Overview => {
            let doc = session.doc();
            let robots: Vec<Value> = doc
                .robots()
                .iter()
                .map(|(id, r)| {
                    let base = session.shown_base(id).map(|p| pose_json(&gc_math::pose_from_xyz_rpy(p.xyz, p.rpy)));
                    json!({ "name": r.name, "base": base, "joints_deg": r.joints.iter().map(|q| q.to_degrees()).collect::<Vec<_>>() })
                })
                .collect();
            let bodies: Vec<Value> = doc.bodies().iter().map(|(_, b)| json!({ "name": b.name })).collect();
            let frames: Vec<Value> = doc
                .work_frames()
                .iter()
                .map(|(id, f)| json!({ "name": f.name, "pose": session.work_frame_pose(id).map(|p| pose_json(&p)) }))
                .collect();
            let waypoints: Vec<Value> = doc
                .waypoints()
                .iter()
                .map(|(id, w)| {
                    let robot = doc.get(w.robot).map(|r| r.name.clone()).unwrap_or_default();
                    json!({ "name": w.name, "robot": robot, "world": session.waypoint_pose(id).map(|p| pose_json(&p)) })
                })
                .collect();
            let paths: Vec<Value> = doc
                .paths()
                .iter()
                .map(|(id, p)| {
                    let names: Vec<String> =
                        p.waypoints.iter().filter_map(|w| doc.get(*w).map(|w| w.name.clone())).collect();
                    json!({
                        "name": p.name,
                        "waypoints": names,
                        "status": status_name(session.plan_status(id)),
                        "duration_s": session.planned(id).map(|planned| planned.duration()),
                        "error": session.plan_error(id),
                    })
                })
                .collect();
            Ok(json!({ "robots": robots, "bodies": bodies, "frames": frames, "waypoints": waypoints, "paths": paths }))
        }
        Call::LibraryRobots => Ok(json!(library
            .iter()
            .map(|p| json!({ "name": p.info.name, "vendor": p.info.vendor, "reach_m": p.info.reach_m, "payload_kg": p.info.payload_kg }))
            .collect::<Vec<_>>())),
        Call::AddRobot { name } => {
            let package = library
                .iter()
                .find(|p| p.info.name == *name)
                .ok_or_else(|| format!("no library robot named `{name}`; call library_robots"))?;
            let id = session.add_library_robot(package)?;
            Ok(json!({ "robot": session.doc().get(id).map(|r| r.name.clone()) }))
        }
        Call::PlaceRobot { robot, xyz_mm, rpy_deg } => {
            let id = robot_named(session, robot)?;
            let placement = placement(*xyz_mm, *rpy_deg);
            session.execute(&Command::Cell(CellOp::PlaceRobot { id, placement })).map_err(edit_error)?;
            Ok(json!({ "placed": robot }))
        }
        Call::AddWaypoint { robot, reference, xyz_mm, rpy_deg } => {
            let robot_id = robot_named(session, robot)?;
            let offset = placement(*xyz_mm, *rpy_deg);
            let target = match reference_named(session, reference)? {
                Reference::World => WaypointTarget::InWorld { offset },
                Reference::Base => WaypointTarget::InBase { offset },
                Reference::Frame(frame) => WaypointTarget::InFrame { frame, offset },
            };
            let id = session.add_waypoint(robot_id, target).map_err(edit_error)?;
            let name = session.doc().get(id).map(|w| w.name.clone()).unwrap_or_default();
            let ghost = session.waypoint_ghost(id);
            Ok(json!({
                "waypoint": name,
                "reachable": ghost.as_ref().is_some_and(|g| g.joints.is_some()),
                "problem": ghost.and_then(|g| g.problem).map(|p| format!("{p:?}")),
            }))
        }
        Call::WaypointInfo { name } => {
            let id = waypoint_named(session, name)?;
            let robot = session.doc().get(id).map(|w| w.robot).ok_or("the waypoint has no robot")?;
            let world = session.waypoint_pose(id).ok_or("the waypoint has no pose")?;
            let mut seen = serde_json::Map::new();
            let mut references = vec![("world".to_owned(), Reference::World), ("base".to_owned(), Reference::Base)];
            references.extend(session.doc().work_frames().iter().map(|(fid, f)| (f.name.clone(), Reference::Frame(fid))));
            for (label, reference) in references {
                if let Some(origin) = session.reference_pose(robot, reference) {
                    seen.insert(label, pose_json(&(origin.inverse() * world)));
                }
            }
            let ghost = session.waypoint_ghost(id);
            Ok(json!({
                "name": name,
                "seen_from": seen,
                "reachable": ghost.as_ref().is_some_and(|g| g.joints.is_some()),
                "joints_deg": ghost.as_ref().and_then(|g| g.joints.clone()).map(|q| q.iter().map(|v| v.to_degrees()).collect::<Vec<_>>()),
                "problem": ghost.and_then(|g| g.problem).map(|p| format!("{p:?}")),
            }))
        }
        Call::CreatePath { waypoints, linear } => {
            let ids: Vec<Id<Waypoint>> = waypoints.iter().map(|w| waypoint_named(session, w)).collect::<Result<_, _>>()?;
            let robot = ids.first().and_then(|w| session.doc().get(*w)).map(|w| w.robot).ok_or("give at least one waypoint")?;
            let path = session.add_path(robot, ids.clone()).map_err(edit_error)?;
            if linear.iter().any(|l| *l) {
                let linear: Vec<bool> = (0..ids.len()).map(|i| linear.get(i).copied().unwrap_or(false)).collect();
                session
                    .execute(&Command::Cell(CellOp::SetPathSteps { id: path, waypoints: ids, linear }))
                    .map_err(edit_error)?;
            }
            Ok(json!({ "path": session.doc().get(path).map(|p| p.name.clone()) }))
        }
        Call::PlanPath { path } => {
            let id = path_named(session, path)?;
            let status = session.plan(id);
            Ok(json!({
                "status": status_name(status),
                "duration_s": session.planned(id).map(|p| p.duration()),
                "peak_tcp_speed_mm_s": session.planned(id).map(|p| p.peak_tcp_speed() * 1000.0),
                "error": session.plan_error(id),
            }))
        }
        Call::Undo => Ok(json!({ "undone": session.undo() })),
        Call::Redo => Ok(json!({ "redone": session.redo() })),
    }
}
