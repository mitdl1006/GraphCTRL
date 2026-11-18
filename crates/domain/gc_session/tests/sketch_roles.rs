//! Sketch elements as robot waypoints and paths (plan 9.1 "sketch → robot paths and waypoints"): roles
//! are saved, one undo step each, follow the sketch, and a robot's path that follows them is planned
//! along the curves and goes out of date when the sketch or the role changes.

#![allow(clippy::float_cmp, clippy::unwrap_used, clippy::panic, reason = "test code")]

use gc_core::Id;
use gc_doc::{
    AimReference, CellOp, Command, Created, EditError, EndSnap, PathMotion, Placement, RoleKind, Sketch,
    SketchOp, ToolAim, WaypointTarget,
};
use gc_math::DVec3;
use gc_session::{PlanStatus, Session};
use gc_sketch::{Curve, Item, SkPoint};

fn six_axis() -> String {
    let path = format!("{}/../../../fixtures/urdf/six_axis.urdf", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn curve_of(created: &[Created]) -> Id<Curve> {
    created.iter().find_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None }).unwrap()
}

fn line(session: &mut Session, sketch: Id<Sketch>, a: [f64; 2], b: [f64; 2]) -> Id<Curve> {
    let op = SketchOp::AddLine {
        a: [a[0], a[1], 0.0],
        b: [b[0], b[1], 0.0],
        snap_a: EndSnap::default(),
        snap_b: EndSnap::default(),
        horizontal: None,
    };
    curve_of(&session.execute(&Command::Sketch { sketch, op }).unwrap())
}

/// A robot 0.6 m in front of the XZ plane, facing it, and on that wall a line to the right, a half circle
/// bulging right and a line back to the left above it: (−0.1, 0.5) → (0.1, 0.5) ⌒ (0.1, 0.6) → (−0.1, 0.6).
fn wall() -> (Session, Id<gc_doc::Robot>, Id<Sketch>, [Id<Curve>; 3]) {
    let mut session = Session::new();
    let robot = session.add_robot("Robot1", &six_axis()).unwrap();
    let placement = Placement { xyz: [0.0, -0.6, 0.0], rpy: [0.0; 3] };
    session.execute(&Command::Cell(CellOp::PlaceRobot { id: robot, placement })).unwrap();
    session.jog(robot, &[1.5, -0.3, 0.6, 0.0, 0.6, 0.0]);
    session.end_jog();
    let sketch = session.create_sketch("XZ").unwrap();
    let below = line(&mut session, sketch, [-0.1, 0.5], [0.1, 0.5]);
    let arc = SketchOp::AddArc { centre: [0.1, 0.55, 0.0], start: [0.1, 0.5, 0.0], end: [0.1, 0.6, 0.0] };
    let arc = curve_of(&session.execute(&Command::Sketch { sketch, op: arc }).unwrap());
    let above = line(&mut session, sketch, [0.1, 0.6], [-0.1, 0.6]);
    session.finish_sketch();
    (session, robot, sketch, [below, arc, above])
}

fn near(a: [f64; 3], b: [f64; 3], tolerance: f64) -> bool {
    (DVec3::from_array(a) - DVec3::from_array(b)).length() < tolerance
}

/// The smallest distance from the tool tip's planned path to a world point.
fn closest(session: &Session, path: Id<gc_doc::MotionPath>, to: [f64; 3]) -> f64 {
    session
        .planned(path)
        .unwrap()
        .tip_path()
        .map(|p| (DVec3::from_array(p) - DVec3::from_array(to)).length())
        .fold(f64::INFINITY, f64::min)
}

#[test]
fn a_chain_of_curves_becomes_a_path_the_robot_follows() {
    let (mut session, robot, sketch, [below, arc, above]) = wall();
    // One click on the arc finds the whole chain; given in any order it is stored end to end, starting
    // at the free end of the first curve given that has one.
    assert_eq!(session.chain_at(sketch, arc), vec![below, arc, above]);
    let depth = session.history().undo_depth();
    let role = session.add_path_role(sketch, &[arc, below, above]).unwrap();
    assert_eq!(session.history().undo_depth(), depth + 1);
    let RoleKind::Path { curves, motion, .. } = &session.doc().get(role).unwrap().kind else { panic!() };
    assert_eq!((curves.as_slice(), *motion), ([below, arc, above].as_slice(), PathMotion::Follow));
    assert_eq!(session.path_role(sketch, arc), Some(role));
    assert_eq!(session.sketch_roles(sketch), vec![role]);
    // A curve is in one path role; a chain it is in is no longer offered.
    assert_eq!(session.add_path_role(sketch, &[arc]), Err(EditError::InUse));
    assert!(session.chain_at(sketch, below).is_empty());

    // On the wall (the XZ plane, y = 0): start, the arc's apex, end. The tool points at the wall (+Y).
    let (start, end) = session.role_ends(role).unwrap();
    assert!(near(start.translation.to_array(), [-0.1, 0.0, 0.5], 1e-9));
    assert!(near(end.translation.to_array(), [-0.1, 0.0, 0.6], 1e-9));
    assert!((start.rotation * DVec3::Z - DVec3::Y).length() < 1e-9);
    let shown = session.role_polyline(role).unwrap();
    assert!(shown.iter().any(|p| near(*p, [0.15, 0.0, 0.55], 1e-9)), "the apex is drawn");

    // A robot follows it: a free move to the start, then one motion along the curves.
    let path = session.follow_roles(robot, &[role]).unwrap();
    assert_eq!(session.delete_role(role), Err(EditError::InUse));
    assert_eq!(session.plan(path), PlanStatus::Ready, "{:?}", session.plan_error(path));
    let planned = session.planned(path).unwrap();
    assert_eq!(planned.segments.len(), 2);
    assert!(planned.segments[1].linear);
    assert!(near(planned.tip_path().last().unwrap(), [-0.1, 0.0, 0.6], 1e-4));
    assert!(closest(&session, path, [0.15, 0.0, 0.55]) < 5e-3, "the tip goes round the arc");
    assert!(closest(&session, path, [0.1, 0.0, 0.5]) < 5e-3, "and through the corner before it");

    // Straight lines between the curves' ends cut the arc short; that is a new plan.
    session.set_role_motion(role, PathMotion::Linear).unwrap();
    assert_eq!(session.plan_status(path), PlanStatus::Stale);
    assert_eq!(session.plan(path), PlanStatus::Ready, "{:?}", session.plan_error(path));
    assert_eq!(session.planned(path).unwrap().segments.len(), 4);
    assert!(closest(&session, path, [0.15, 0.0, 0.55]) > 0.04);
    assert_eq!(session.role_polyline(role).unwrap().len(), 4);
    assert!(session.undo());
    assert_eq!(session.plan_status(path), PlanStatus::Stale, "the stored plan is the linear one");
    assert_eq!(session.plan(path), PlanStatus::Ready);

    // A speed limit of the role slows the followed motion down.
    let fast = session.planned(path).unwrap().duration();
    assert!(session.planned(path).unwrap().segments[1].peak_tcp_speed > 0.05);
    session.set_role_speed(role, Some(0.05)).unwrap();
    assert_eq!(session.set_role_speed(role, Some(0.0)), Err(EditError::BadValue));
    assert_eq!(session.plan(path), PlanStatus::Ready, "{:?}", session.plan_error(path));
    assert!(session.planned(path).unwrap().segments[1].peak_tcp_speed <= 0.05 + 1e-9);
    assert!(session.planned(path).unwrap().duration() > fast);

    // The other way round it ends where it started.
    session.reverse_role(role).unwrap();
    assert_eq!(session.plan(path), PlanStatus::Ready, "{:?}", session.plan_error(path));
    assert!(near(session.planned(path).unwrap().tip_path().last().unwrap(), [-0.1, 0.0, 0.5], 1e-4));

    // Editing the sketch moves the path: the plan is out of date and planning again follows.
    let (a, _) = session.doc().get(sketch).unwrap().curve_ends(below).unwrap();
    let moved = SketchOp::SetCoords { points: vec![(a, [-0.15, 0.5, 0.0])], radii: Vec::new() };
    session.execute(&Command::Sketch { sketch, op: moved }).unwrap();
    assert_eq!(session.plan_status(path), PlanStatus::Stale);
    assert_eq!(session.plan(path), PlanStatus::Ready, "{:?}", session.plan_error(path));
    assert!(near(session.planned(path).unwrap().tip_path().last().unwrap(), [-0.15, 0.0, 0.5], 1e-4));
}

#[test]
fn a_point_becomes_a_waypoint_with_its_own_tool_attitude() {
    let (mut session, robot, sketch, [below, ..]) = wall();
    let (a, b) = session.doc().get(sketch).unwrap().curve_ends(below).unwrap();
    let role = session.add_waypoint_role(sketch, a).unwrap();
    assert_eq!(session.waypoint_role(sketch, a), Some(role));
    assert_eq!(session.add_waypoint_role(sketch, a), Err(EditError::InUse));
    let pose = session.role_pose(role).unwrap();
    assert!(near(pose.translation.to_array(), [-0.1, 0.0, 0.5], 1e-9));
    assert!((pose.rotation * DVec3::Z - DVec3::Y).length() < 1e-9, "against the plane's normal");

    // A robot's waypoint on the point is held as the role says, and a plan through it goes out of date
    // when the attitude changes.
    let waypoint = session.add_waypoint(robot, WaypointTarget::SketchPoint { sketch, point: a }).unwrap();
    let path = session.add_path(robot, vec![waypoint]).unwrap();
    assert_eq!(session.plan(path), PlanStatus::Ready, "{:?}", session.plan_error(path));
    let depth = session.history().undo_depth();
    let tilted = ToolAim { reference: AimReference::SketchPlane, rpy: [0.0, 0.3, 0.0] };
    session.set_role_aim(role, tilted).unwrap();
    assert_eq!(session.history().undo_depth(), depth + 1);
    assert_eq!(session.plan_status(path), PlanStatus::Stale);
    let held = session.waypoint_pose(waypoint).unwrap();
    assert_eq!(held, session.role_pose(role).unwrap());
    let off = (held.rotation * DVec3::Z).angle_between(DVec3::Y);
    assert!((off - 0.3).abs() < 1e-9, "{off}");
    assert!(near(held.translation.to_array(), [-0.1, 0.0, 0.5], 1e-9), "the tip stays on the point");
    assert_eq!(session.plan(path), PlanStatus::Ready, "{:?}", session.plan_error(path));

    // A selection gets its roles in one step: the points not yet waypoints, and the curves as one path.
    let depth = session.history().undo_depth();
    let made = session.assign_roles(sketch, &[a, b], &[below]).unwrap();
    assert_eq!(made.len(), 2);
    assert_eq!(session.history().undo_depth(), depth + 1);
    assert_eq!(session.waypoint_role(sketch, b), Some(made[0]));
    assert_eq!(session.path_role(sketch, below), Some(made[1]));
    let names: Vec<&str> = session.doc().roles().iter().map(|(_, r)| r.name.as_str()).collect();
    assert_eq!(names.len(), 3);
    assert!(names.iter().all(|n| names.iter().filter(|m| m == &n).count() == 1), "{names:?}");
    assert!(session.undo());
    assert_eq!(session.doc().roles().len(), 1);
    assert!(session.redo());

    // Deleting the sketch elements takes their roles along in the same step; undo brings both back.
    let depth = session.history().undo_depth();
    session.execute(&Command::Sketch { sketch, op: SketchOp::Delete(vec![Item::Curve(below)]) }).unwrap();
    assert_eq!(session.history().undo_depth(), depth + 1);
    assert_eq!(session.path_role(sketch, below), None);
    assert!(session.doc().get(made[1]).is_none());
    assert!(session.undo());
    assert_eq!(session.path_role(sketch, below), Some(made[1]));

    // Renaming and suppressing work as for other browser items.
    session.execute(&Command::Rename { item: role.erase(), name: "Pick".into() }).unwrap();
    assert_eq!(session.doc().get(role).unwrap().name, "Pick");
    session.delete_role(role).unwrap();
    assert_eq!(session.waypoint_role(sketch, a), None);
}

#[test]
fn create_path_between_two_points_and_from_a_whole_sketch() {
    let (mut session, robot, sketch, [below, arc, above]) = wall();
    let ends = |session: &Session, curve| session.doc().get(sketch).unwrap().curve_ends(curve).unwrap();
    let (a, b): (Id<SkPoint>, Id<SkPoint>) = ends(&session, below);
    let waypoints = session.doc().waypoints().len();

    // "Create path": two points and a planner give a planned path in one undo step.
    let depth = session.history().undo_depth();
    let path = session.route_between(robot, (sketch, a), (sketch, b), "rrt_star", false).unwrap();
    assert_eq!(session.history().undo_depth(), depth + 1);
    assert_eq!(session.plan_status(path), PlanStatus::Ready, "{:?}", session.plan_error(path));
    let stored = session.doc().get(path).unwrap();
    assert_eq!((stored.planner.as_str(), stored.waypoints.len()), ("rrt_star", 2));
    assert_eq!(session.doc().waypoints().len(), waypoints + 2);
    assert!(near(session.planned(path).unwrap().tip_path().last().unwrap(), [0.1, 0.0, 0.5], 1e-4));
    // Again the other way: the robot's waypoints on the points are used again; a straight tool move.
    let back = session.route_between(robot, (sketch, b), (sketch, a), "rrt_connect", true).unwrap();
    assert_eq!(session.doc().waypoints().len(), waypoints + 2);
    assert_eq!(session.plan_status(back), PlanStatus::Ready, "{:?}", session.plan_error(back));
    assert!(session.planned(back).unwrap().segments[1].linear);
    assert_eq!(
        session.route_between(robot, (sketch, a), (sketch, b), "no such planner", false),
        Err(EditError::BadValue)
    );
    assert_eq!(
        session.route_between(robot, (sketch, a), (sketch, a), "rrt", false),
        Err(EditError::BadValue)
    );

    // The whole sketch as a path: its one chain becomes a path role and the path follows it; one step.
    let depth = session.history().undo_depth();
    let whole = session.path_from_sketch(robot, sketch).unwrap();
    assert_eq!(session.history().undo_depth(), depth + 1);
    let follow = session.doc().get(whole).unwrap().follow.clone();
    assert_eq!(follow.len(), 1);
    assert_eq!(session.path_role(sketch, arc), Some(follow[0]));
    assert_eq!(session.path_role(sketch, above), Some(follow[0]));
    assert_eq!(session.plan(whole), PlanStatus::Ready, "{:?}", session.plan_error(whole));
    assert!(near(session.planned(whole).unwrap().tip_path().last().unwrap(), [-0.1, 0.0, 0.6], 1e-4));
    // Asked again, the role that is there is followed; no second one is made.
    let again = session.path_from_sketch(robot, sketch).unwrap();
    assert_eq!(session.doc().get(again).unwrap().follow, follow);
    assert_eq!(session.doc().roles().len(), 1);

    // A followed role outlives the deletion of a curve (the path then says what is wrong); the curves
    // left over still form a chain, so the role keeps them.
    session.execute(&Command::Sketch { sketch, op: SketchOp::Delete(vec![Item::Curve(above)]) }).unwrap();
    let RoleKind::Path { curves, .. } = &session.doc().get(follow[0]).unwrap().kind else { panic!() };
    assert_eq!(curves.as_slice(), [below, arc].as_slice());
    assert_eq!(session.plan_status(whole), PlanStatus::Stale);
    assert_eq!(session.plan(whole), PlanStatus::Ready, "{:?}", session.plan_error(whole));
    assert!(near(session.planned(whole).unwrap().tip_path().last().unwrap(), [0.1, 0.0, 0.6], 1e-4));

    // Saved and read back, the roles and the paths that follow them are the same.
    let text = gc_persist::to_text(session.doc()).unwrap();
    let loaded = gc_persist::from_text(&text).unwrap();
    assert!(loaded.doc.same_content(session.doc()));
}

#[test]
fn a_spline_in_a_3d_sketch_is_a_path_and_the_tool_can_be_aimed_from_a_work_frame() {
    let mut session = Session::new();
    let sketch = session.create_sketch_3d().unwrap();
    let fit = vec![[0.3, 0.0, 0.2], [0.4, 0.1, 0.3], [0.5, 0.0, 0.4]];
    let op = SketchOp::AddSpline { points: fit.clone(), snaps: Vec::new() };
    let spline = curve_of(&session.execute(&Command::Sketch { sketch, op }).unwrap());
    session.finish_sketch();
    let (first, last) = session.doc().get(sketch).unwrap().curve_ends(spline).unwrap();

    let role = session.add_path_role(sketch, &[spline]).unwrap();
    let shown = session.role_polyline(role).unwrap();
    assert!(near(shown[0], fit[0], 1e-12) && near(*shown.last().unwrap(), fit[2], 1e-12));
    assert!(shown.len() > 20 && shown.iter().any(|p| near(*p, fit[1], 1e-9)), "through the middle fit point");
    // In a 3D sketch the tool points down (against world Z).
    let (start, _) = session.role_ends(role).unwrap();
    assert!((start.rotation * DVec3::Z + DVec3::Z).length() < 1e-9);
    session.reverse_role(role).unwrap();
    assert!(near(session.role_polyline(role).unwrap()[0], fit[2], 1e-12));

    // Aimed from a work frame whose Z axis is world X, the tool points against that.
    let frame = gc_doc::WorkFrame {
        name: "Frame1".into(),
        sketch,
        point: first,
        x_point: None,
        xy_point: None,
        rpy: [0.0, std::f64::consts::FRAC_PI_2, 0.0],
    };
    let Created::Frame(frame) = session.execute(&Command::Cell(CellOp::AddFrame(frame))).unwrap()[0] else {
        panic!()
    };
    assert!(session.role_aim_references().contains(&AimReference::Frame(frame)));
    let waypoint = session.add_waypoint_role(sketch, last).unwrap();
    session.set_role_aim(waypoint, ToolAim { reference: AimReference::Frame(frame), rpy: [0.0; 3] }).unwrap();
    let pose = session.role_pose(waypoint).unwrap();
    assert!((pose.rotation * DVec3::Z + DVec3::X).length() < 1e-9);
    assert!(near(pose.translation.to_array(), fit[2], 1e-12));
    // The frame cannot go while a role is aimed from it.
    assert_eq!(session.execute(&Command::Cell(CellOp::DeleteFrame(frame))), Err(EditError::InUse));

    // A closed curve cannot be part of a chain with others.
    let sketch2 = session.create_sketch("XY").unwrap();
    let circle = SketchOp::AddCircle {
        centre: [0.0; 3],
        radius: 0.1,
        centre_snap: EndSnap::default(),
        rim_snap: EndSnap::default(),
    };
    let circle = curve_of(&session.execute(&Command::Sketch { sketch: sketch2, op: circle }).unwrap());
    let other = line(&mut session, sketch2, [0.1, 0.0], [0.2, 0.0]);
    session.finish_sketch();
    assert_eq!(session.add_path_role(sketch2, &[circle, other]), Err(EditError::BadValue));
    assert_eq!(session.chain_at(sketch2, circle), vec![circle]);
}

/// A circle is a path once round, from the place nearest a chosen sketch point, and the tool can turn with
/// the path so that its X axis points the way it goes.
#[test]
fn a_closed_curve_is_travelled_once_round_and_the_tool_can_follow_the_tangent() {
    let mut session = Session::new();
    let robot = session.add_robot("Robot1", &six_axis()).unwrap();
    let placement = Placement { xyz: [0.0, -0.6, 0.0], rpy: [0.0; 3] };
    session.execute(&Command::Cell(CellOp::PlaceRobot { id: robot, placement })).unwrap();
    session.jog(robot, &[1.5, -0.3, 0.6, 0.0, 0.6, 0.0]);
    session.end_jog();
    // On the wall (XZ plane): a circle of 50 mm round (0, 0.55) and a point above it.
    let sketch = session.create_sketch("XZ").unwrap();
    let circle = SketchOp::AddCircle {
        centre: [0.0, 0.55, 0.0],
        radius: 0.05,
        centre_snap: EndSnap::default(),
        rim_snap: EndSnap::default(),
    };
    let circle = curve_of(&session.execute(&Command::Sketch { sketch, op: circle }).unwrap());
    let top = session
        .execute(&Command::Sketch { sketch, op: SketchOp::AddPoint { pos: [0.0, 0.7, 0.0] } })
        .unwrap();
    let Created::Point(top) = top[0] else { panic!("no point") };
    session.finish_sketch();

    let role = session.add_path_role(sketch, &[circle]).unwrap();
    assert!(session.role_closed(role));
    // Without a start point it starts at the circle's +X point; counter-clockwise in the sketch.
    let round = session.role_polyline(role).unwrap();
    assert!(near(round[0], [0.05, 0.0, 0.55], 1e-9) && near(*round.last().unwrap(), round[0], 1e-9));
    assert!(round[1][2] > 0.55, "counter-clockwise seen along the sketch's normal: up first");
    // From the point above: it starts at the top of the circle; reversed it goes the other way round.
    let depth = session.history().undo_depth();
    session.set_role_start(role, Some(top)).unwrap();
    assert_eq!(session.history().undo_depth(), depth + 1);
    let round = session.role_polyline(role).unwrap();
    assert!(near(round[0], [0.0, 0.0, 0.6], 1e-9) && near(*round.last().unwrap(), round[0], 1e-9));
    assert!(round[1][0] < 0.0);
    session.reverse_role(role).unwrap();
    assert!(session.role_polyline(role).unwrap()[1][0] > 0.0);
    session.reverse_role(role).unwrap();

    // The robot goes once round and ends where it started.
    let path = session.follow_roles(robot, &[role]).unwrap();
    assert_eq!(session.plan(path), PlanStatus::Ready, "{:?}", session.plan_error(path));
    assert!(near(session.planned(path).unwrap().tip_path().last().unwrap(), [0.0, 0.0, 0.6], 1e-4));
    assert!(closest(&session, path, [0.0, 0.0, 0.5]) < 5e-3, "through the bottom");

    // Tangent: the tool still points at the wall, and its X axis is the way it travels (−X at the top,
    // going counter-clockwise). The plan is out of date.
    let (start, _) = session.role_ends(role).unwrap();
    session.set_role_tangent(role, true).unwrap();
    assert_eq!(session.plan_status(path), PlanStatus::Stale);
    let (turned, _) = session.role_ends(role).unwrap();
    assert!((turned.rotation * DVec3::Z - start.rotation * DVec3::Z).length() < 1e-9);
    assert!((turned.rotation * DVec3::X + DVec3::X).length() < 1e-6, "{:?}", turned.rotation * DVec3::X);
    // A waypoint has no travel to follow.
    let waypoint = session.add_waypoint_role(sketch, top).unwrap();
    assert_eq!(session.set_role_tangent(waypoint, true), Err(EditError::BadValue));
    assert_eq!(session.set_role_start(waypoint, Some(top)), Err(EditError::BadValue));
}

#[test]
fn a_robot_follows_a_chain_with_the_tool_turned_along_it() {
    let (mut session, robot, sketch, curves) = wall();
    let role = session.add_path_role(sketch, &curves).unwrap();
    session.set_role_tangent(role, true).unwrap();
    // Along the lower line the tool's X is +X, along the upper one −X: it turns half round on the arc.
    let (start, end) = session.role_ends(role).unwrap();
    assert!((start.rotation * DVec3::X - DVec3::X).length() < 1e-6);
    assert!((end.rotation * DVec3::X + DVec3::X).length() < 1e-6);
    let path = session.follow_roles(robot, &[role]).unwrap();
    assert_eq!(session.plan(path), PlanStatus::Ready, "{:?}", session.plan_error(path));
    assert!(near(session.planned(path).unwrap().tip_path().last().unwrap(), [-0.1, 0.0, 0.6], 1e-4));
    assert!(closest(&session, path, [0.15, 0.0, 0.55]) < 5e-3);
    // In straight lines between the ends the tool turns on the spot at each corner first.
    session.set_role_motion(role, PathMotion::Linear).unwrap();
    assert_eq!(session.plan(path), PlanStatus::Ready, "{:?}", session.plan_error(path));
    assert!(session.planned(path).unwrap().segments.len() > 4, "turns are motions of their own");
}
