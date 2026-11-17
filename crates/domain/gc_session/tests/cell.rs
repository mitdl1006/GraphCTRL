//! Robot-cell scenarios, headless: robot, obstacle, jogging, waypoints, planning, staleness, playback.

#![allow(clippy::float_cmp, clippy::unwrap_used, clippy::panic, reason = "test code")]

use gc_doc::{CellOp, Command, Placement, SketchOp, Solid, WaypointTarget};
use gc_session::{PlanStatus, Session};

fn six_axis() -> String {
    let path = format!("{}/../../../fixtures/urdf/six_axis.urdf", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

const LEFT: [f64; 6] = [-1.0, 0.2, 0.3, 0.0, 0.5, 0.0];
const RIGHT: [f64; 6] = [1.0, 0.2, 0.3, 0.0, 0.5, 0.0];

#[test]
fn a_path_around_a_pillar_is_planned_goes_stale_and_plays_back() {
    let mut session = Session::new();
    let robot = session.add_robot("Robot1", &six_axis()).unwrap();
    assert_eq!(session.doc().get(robot).unwrap().tip_link, "tool0");
    assert_eq!(
        session.link_poses(robot).unwrap().len(),
        session.robot_model(robot).unwrap().model.links.len()
    );

    // Jogging previews without touching the document; ending it is one undo step.
    let depth = session.history().undo_depth();
    let revision = session.doc().revision();
    for step in 1..=10 {
        session.jog(robot, &RIGHT.map(|q| q * f64::from(step) / 10.0));
    }
    assert_eq!(session.doc().revision(), revision);
    assert_eq!(session.robot_joints(robot).unwrap(), RIGHT.to_vec());
    session.end_jog();
    assert_eq!(session.history().undo_depth(), depth + 1);
    let right = session.add_waypoint_here(robot).unwrap();
    session.jog(robot, &[9.0; 6]); // beyond every limit: clamped
    assert_eq!(session.robot_joints(robot).unwrap(), vec![3.0, 2.0, 2.5, 3.1, 2.0, 6.0]);
    session.jog(robot, &LEFT);
    session.end_jog();

    let pillar = session
        .add_body(Solid::Box { size: [0.1, 0.1, 1.4] }, Placement { xyz: [0.55, 0.0, 0.7], rpy: [0.0; 3] })
        .unwrap();
    let path = session.add_path(robot, vec![right]).unwrap();
    assert_eq!(session.plan_status(path), PlanStatus::NotPlanned);
    assert_eq!(session.plan(path), PlanStatus::Ready, "{:?}", session.plan_error(path));
    let planned = session.planned(path).unwrap();
    assert!(!planned.segments[0].straight, "the pillar is in the way");
    assert!(planned.duration() > 1.4 && planned.min_clearance() >= gc_session::PADDING);
    assert_eq!(planned.sample(0.0), LEFT.to_vec());
    assert_eq!(planned.sample(1e9), RIGHT.to_vec());
    assert!(planned.tip_path().count() > 20);
    let duration = planned.duration();

    // Playback shows the robot along the motion without changing the document.
    session.set_playback(Some((path, duration)));
    assert_eq!(session.robot_joints(robot).unwrap(), RIGHT.to_vec());
    assert_eq!(session.doc().get(robot).unwrap().joints, LEFT.to_vec());
    session.set_playback(None);

    // Moving the obstacle makes the plan stale; undoing that makes it current again.
    let moved = gc_doc::Body {
        placement: Placement { xyz: [0.0, 0.9, 0.7], rpy: [0.0; 3] },
        ..session.doc().get(pillar).unwrap().clone()
    };
    session.execute(&Command::Cell(CellOp::SetBody { id: pillar, body: moved })).unwrap();
    assert_eq!(session.plan_status(path), PlanStatus::Stale);
    assert!(session.undo());
    assert_eq!(session.plan_status(path), PlanStatus::Ready);
    assert!(session.redo());
    // Without the pillar in the way the same request is a straight move.
    assert_eq!(session.plan(path), PlanStatus::Ready);
    assert!(session.planned(path).unwrap().segments[0].straight);

    // Another planner is a document edit like any other.
    session
        .execute(&Command::Cell(CellOp::SetPlanner { id: path, planner: "nonsense".into(), seed: 1 }))
        .unwrap();
    assert_eq!(session.plan(path), PlanStatus::Failed);
    assert!(session.plan_error(path).unwrap().contains("nonsense"));
}

#[test]
fn a_waypoint_on_a_sketch_point_follows_the_sketch() {
    let mut session = Session::new();
    let robot = session.add_robot("Robot1", &six_axis()).unwrap();
    // The robot stands 0.6 m in front of the XZ plane and turns to face it.
    let placement = Placement { xyz: [0.0, -0.6, 0.0], rpy: [0.0; 3] };
    session.execute(&Command::Cell(CellOp::PlaceRobot { id: robot, placement })).unwrap();
    session.jog(robot, &[1.5, -0.3, 0.6, 0.0, 0.6, 0.0]);
    session.end_jog();
    // A point on that wall, 0.1 m to the side and 0.5 m up. The tool approaches it against the normal.
    let sketch = session.create_sketch("XZ").unwrap();
    let created = session
        .execute(&Command::Sketch { sketch, op: SketchOp::AddPoint { pos: [0.1, 0.5, 0.0] } })
        .unwrap();
    let gc_doc::Created::Point(point) = created[0] else { panic!("no point") };
    session.finish_sketch();
    let waypoint = session.add_waypoint(robot, WaypointTarget::SketchPoint { sketch, point }).unwrap();
    let path = session.add_path(robot, vec![waypoint]).unwrap();
    assert_eq!(session.plan(path), PlanStatus::Ready, "{:?}", session.plan_error(path));
    let tip = session.planned(path).unwrap().tip_path().last().unwrap();
    assert!((tip[0] - 0.1).abs() < 1e-4 && tip[1].abs() < 1e-4 && (tip[2] - 0.5).abs() < 1e-4, "{tip:?}");

    // Moving the point makes the plan stale; replanning follows it.
    let moved = SketchOp::SetCoords { points: vec![(point, [-0.15, 0.6, 0.0])], radii: Vec::new() };
    session.execute(&Command::Sketch { sketch, op: moved }).unwrap();
    assert_eq!(session.plan_status(path), PlanStatus::Stale);
    assert_eq!(session.plan(path), PlanStatus::Ready, "{:?}", session.plan_error(path));
    let tip = session.planned(path).unwrap().tip_path().last().unwrap();
    assert!((tip[0] + 0.15).abs() < 1e-4 && tip[1].abs() < 1e-4 && (tip[2] - 0.6).abs() < 1e-4, "{tip:?}");

    // A description that cannot be read is refused with the reason.
    assert!(session.add_robot("Broken", "<robot name=\"x\"><link/></robot>").is_err());
}

#[test]
fn planning_in_the_background_can_be_watched_and_cancelled() {
    let (executor, queue) = gc_jobs::Executor::manual();
    let mut session = Session::new();
    session.set_executor(executor);
    let robot = session.add_robot("Robot1", &six_axis()).unwrap();
    session.jog(robot, &RIGHT);
    session.end_jog();
    let right = session.add_waypoint_here(robot).unwrap();
    session.jog(robot, &LEFT);
    session.end_jog();
    let path = session.add_path(robot, vec![right]).unwrap();

    // Nothing is computed until the job runs; the session stays usable meanwhile.
    assert_eq!(session.plan(path), PlanStatus::Planning);
    assert!(session.is_planning() && !session.poll_jobs());
    assert!(session.planned(path).is_none());
    queue.run_all();
    assert!(session.poll_jobs());
    assert_eq!(session.plan_status(path), PlanStatus::Ready);
    assert!(!session.is_planning() && !session.poll_jobs());

    // An edit made while a plan is being computed makes its result out of date, not wrong.
    assert_eq!(session.plan(path), PlanStatus::Planning);
    let pillar = Solid::Box { size: [0.1, 0.1, 1.4] };
    session.add_body(pillar, Placement { xyz: [0.55, 0.0, 0.7], rpy: [0.0; 3] }).unwrap();
    queue.run_all();
    assert!(session.poll_jobs());
    assert_eq!(session.plan_status(path), PlanStatus::Stale);
    assert!(session.planned(path).unwrap().segments[0].straight, "planned before the pillar existed");

    // A cancelled job leaves the previous result alone.
    assert_eq!(session.plan(path), PlanStatus::Planning);
    session.cancel_planning();
    assert_eq!(session.plan_status(path), PlanStatus::Stale);
    queue.run_all();
    assert!(!session.poll_jobs());
    assert!(session.planned(path).unwrap().segments[0].straight);
}

#[test]
fn equipment_from_a_mesh_file_is_an_obstacle() {
    // A pillar modelled in millimetres, as CAD exports usually are.
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("pillar.stl");
    let pillar = gc_mesh::primitives::cuboid([100.0, 100.0, 1400.0]).unwrap();
    std::fs::write(&file, gc_mesh::stl::to_binary(&pillar)).unwrap();

    let mut session = Session::new();
    let robot = session.add_robot("Robot1", &six_axis()).unwrap();
    session.jog(robot, &RIGHT);
    session.end_jog();
    let right = session.add_waypoint_here(robot).unwrap();
    session.jog(robot, &LEFT);
    session.end_jog();
    let body = session.add_mesh_body(&file).unwrap();
    let stored = session.doc().get(body).unwrap().clone();
    assert_eq!(stored.name, "pillar");
    let Solid::Mesh { scale, .. } = stored.solid else { panic!("not a mesh") };
    assert_eq!(scale, 0.001, "a 1.4 km pillar is taken to be in millimetres");
    let mesh = session.body_mesh(&stored.solid).unwrap();
    assert!((mesh.data.aabb().max.z - 0.7).abs() < 1e-6);

    let placement = Placement { xyz: [0.55, 0.0, 0.7], rpy: [0.0; 3] };
    session.preview_body(body, gc_doc::Body { placement, ..stored });
    session.end_body_preview();
    let path = session.add_path(robot, vec![right]).unwrap();
    assert_eq!(session.plan(path), PlanStatus::Ready, "{:?}", session.plan_error(path));
    let planned = session.planned(path).unwrap();
    assert!(!planned.segments[0].straight, "the pillar is in the way");
    assert!(planned.min_clearance() >= gc_session::PADDING);

    // A file that is not there is refused with its name; nothing is added.
    let missing = directory.path().join("nothing.stl");
    let error = session.add_mesh_body(&missing).unwrap_err();
    assert!(error.contains("nothing.stl"), "{error}");
    assert_eq!(session.doc().bodies().len(), 1);
}

#[test]
fn planners_are_compared_over_several_seeds() {
    let mut session = Session::new();
    let robot = session.add_robot("Robot1", &six_axis()).unwrap();
    session.jog(robot, &RIGHT);
    session.end_jog();
    let right = session.add_waypoint_here(robot).unwrap();
    session.jog(robot, &LEFT);
    session.end_jog();
    let pillar = Solid::Box { size: [0.1, 0.1, 1.4] };
    session.add_body(pillar, Placement { xyz: [0.55, 0.0, 0.7], rpy: [0.0; 3] }).unwrap();
    let path = session.add_path(robot, vec![right]).unwrap();
    assert!(session.comparison(path).is_none());

    session.compare(path, 2);
    let (rows, current) = session.comparison(path).unwrap();
    assert!(current);
    assert_eq!(rows.iter().map(|r| r.planner).collect::<Vec<_>>(), gc_session::PLANNERS.map(|(id, _)| id));
    let connect = &rows[0];
    assert_eq!((connect.runs, connect.successes), (2, 2), "RRT-Connect gets around the pillar every time");
    assert!(connect.duration.unwrap() > 1.4 && connect.cost.unwrap() > 0.0 && connect.state_checks > 0);
    // The same comparison again gives the same numbers.
    let first = rows.to_vec();
    session.compare(path, 2);
    assert_eq!(session.comparison(path).unwrap().0, first.as_slice());

    // It goes out of date with the document, like a plan.
    session.jog(robot, &[-0.8, 0.2, 0.3, 0.0, 0.5, 0.0]);
    session.end_jog();
    assert!(!session.comparison(path).unwrap().1);
}

#[test]
fn placing_the_base_previews_live_and_commits_once() {
    let mut session = Session::new();
    let robot = session.add_robot("Robot1", &six_axis()).unwrap();
    let waypoint = session.add_waypoint_here(robot).unwrap();
    let before = session.link_poses(robot).unwrap();
    let tip = session.waypoint_pose(waypoint).unwrap().translation;
    let (revision, depth) = (session.doc().revision(), session.history().undo_depth());
    for step in 1..=4 {
        let placement = Placement { xyz: [0.0, 0.0, 0.2 * f64::from(step)], rpy: [0.0; 3] };
        session.preview_base(robot, placement);
        assert_eq!(session.doc().revision(), revision, "the preview does not touch the document");
    }
    // Every link is shown 0.8 m higher.
    let shown = session.link_poses(robot).unwrap();
    for (was, now) in before.iter().zip(&shown) {
        assert!((now.translation - was.translation - gc_math::DVec3::new(0.0, 0.0, 0.8)).length() < 1e-12);
    }
    session.end_base_preview();
    assert_eq!(session.history().undo_depth(), depth + 1);
    assert_eq!(session.doc().get(robot).unwrap().placement.xyz, [0.0, 0.0, 0.8]);
    // A joint waypoint moves with the robot it belongs to.
    let moved = session.waypoint_pose(waypoint).unwrap().translation;
    assert!((moved - tip - gc_math::DVec3::new(0.0, 0.0, 0.8)).length() < 1e-12);
    assert!(session.undo());
    assert_eq!(session.doc().get(robot).unwrap().placement.xyz, [0.0; 3]);
}

#[test]
fn a_robot_file_finds_its_meshes_around_its_folder() {
    // The usual layout of a robot package: <package>/urdf/robot.urdf and <package>/meshes/*.stl.
    let directory = tempfile::tempdir().unwrap();
    let package = directory.path().join("my_arm");
    std::fs::create_dir_all(package.join("urdf")).unwrap();
    std::fs::create_dir_all(package.join("meshes")).unwrap();
    let column = gc_mesh::primitives::cylinder(0.05, 0.4, 16).unwrap();
    std::fs::write(package.join("meshes/column.stl"), gc_mesh::stl::to_binary(&column)).unwrap();
    let urdf = r#"<?xml version="1.0"?>
<robot name="my_arm">
  <link name="base_link">
    <visual><geometry><mesh filename="package://my_arm/meshes/column.stl"/></geometry></visual>
    <collision><geometry><mesh filename="package://my_arm/meshes/column.stl"/></geometry></collision>
  </link>
  <link name="arm">
    <visual><geometry><mesh filename="package://my_arm/meshes/lost.stl"/></geometry></visual>
    <collision><origin xyz="0.2 0 0"/><geometry><box size="0.4 0.05 0.05"/></geometry></collision>
  </link>
  <joint name="swing" type="revolute">
    <parent link="base_link"/><child link="arm"/>
    <origin xyz="0 0 0.25"/><axis xyz="0 0 1"/>
    <limit lower="-3" upper="3" velocity="2" effort="10"/>
  </joint>
</robot>"#;
    let file = package.join("urdf/my_arm.urdf");
    std::fs::write(&file, urdf).unwrap();

    let mut session = Session::new();
    let robot = session.add_robot_file(&file).unwrap();
    let stored = session.doc().get(robot).unwrap();
    assert_eq!((stored.name.as_str(), stored.tip_link.as_str(), stored.joints.len()), ("my_arm", "arm", 1));
    let loaded = session.robot_model(robot).unwrap();
    assert_eq!(loaded.missing, vec!["package://my_arm/meshes/lost.stl".to_owned()]);
    assert_eq!(loaded.mesh("package://my_arm/meshes/column.stl", [1.0; 3]).len(), 1);
    assert_eq!(loaded.file("package://my_arm/meshes/column.stl").unwrap().triangle_count(), 64);

    // It plans like any other robot, and the document remembers where its files are.
    session.jog(robot, &[2.0]);
    session.end_jog();
    let there = session.add_waypoint_here(robot).unwrap();
    session.jog(robot, &[-2.0]);
    session.end_jog();
    let path = session.add_path(robot, vec![there]).unwrap();
    assert_eq!(session.plan(path), PlanStatus::Ready, "{:?}", session.plan_error(path));
    let saved = directory.path().join("cell.gctrl");
    session.save_as(&saved).unwrap();
    let mut reopened = Session::new();
    reopened.open(&saved).unwrap();
    assert_eq!(
        reopened.robot_model(robot).unwrap().mesh("package://my_arm/meshes/column.stl", [1.0; 3]).len(),
        1
    );

    // A description that does not read is refused with the file name and the place of the problem.
    let broken = package.join("urdf/broken.urdf");
    std::fs::write(&broken, "<robot name=\"x\"><link/></robot>").unwrap();
    let error = session.add_robot_file(&broken).unwrap_err();
    assert!(error.contains("broken.urdf"), "{error}");
}

#[test]
fn a_waypoint_can_be_approached_in_a_straight_line() {
    let mut session = Session::new();
    let robot = session.add_robot("Robot1", &six_axis()).unwrap();
    session.jog(robot, &[0.6, 0.2, 0.3, 0.0, 0.5, 0.0]);
    session.end_jog();
    let there = session.add_waypoint_here(robot).unwrap();
    session.jog(robot, &[-0.6, 0.2, 0.3, 0.0, 0.5, 0.0]);
    session.end_jog();
    let path = session.add_path(robot, vec![there]).unwrap();
    assert_eq!(session.plan(path), PlanStatus::Ready);
    let free: Vec<[f64; 3]> = session.planned(path).unwrap().tip_path().collect();

    // Choosing the straight line is a document edit: the plan goes stale, and planning again follows
    // the chord between the two tip positions instead of the arc the joints would sweep.
    session.execute(&Command::Cell(CellOp::SetLinear { id: path, index: 0, linear: true })).unwrap();
    assert_eq!(session.plan_status(path), PlanStatus::Stale);
    assert_eq!(session.plan(path), PlanStatus::Ready, "{:?}", session.plan_error(path));
    let line: Vec<[f64; 3]> = session.planned(path).unwrap().tip_path().collect();
    let (from, to) = (gc_math::DVec3::from_array(line[0]), gc_math::DVec3::from_array(*line.last().unwrap()));
    let along = (to - from).normalize();
    let off_line = |tip: &[f64; 3]| {
        let offset = gc_math::DVec3::from_array(*tip) - from;
        (offset - along * offset.dot(along)).length()
    };
    assert!(line.iter().map(off_line).fold(0.0, f64::max) < 1e-3);
    assert!(free.iter().map(off_line).fold(0.0, f64::max) > 0.05, "the free move swings round the base");

    // With a pillar on the chord the straight line is refused, saying how far it gets and what is there.
    session
        .add_body(Solid::Box { size: [0.1, 0.1, 1.4] }, Placement { xyz: [0.55, 0.0, 0.7], rpy: [0.0; 3] })
        .unwrap();
    assert_eq!(session.plan(path), PlanStatus::Failed);
    let error = session.plan_error(path).unwrap();
    assert!(error.contains("straight line") && error.contains("Body1"), "{error}");
    // Free again, the planner goes round it.
    session.execute(&Command::Cell(CellOp::SetLinear { id: path, index: 0, linear: false })).unwrap();
    assert_eq!(session.plan(path), PlanStatus::Ready);
    assert!(session.execute(&Command::Cell(CellOp::SetLinear { id: path, index: 3, linear: true })).is_err());
}

#[test]
fn the_shown_pose_is_checked_live() {
    use gc_session::StateProblem;
    let mut session = Session::new();
    let robot = session.add_robot("Robot1", &six_axis()).unwrap();
    assert_eq!(session.robot_problem(robot), None);
    let pillar = session
        .add_body(Solid::Box { size: [0.1, 0.1, 1.4] }, Placement { xyz: [0.55, 0.0, 0.7], rpy: [0.0; 3] })
        .unwrap();
    // Jogged into the pillar: the preview is checked, before anything is committed.
    let revision = session.doc().revision();
    session.jog(robot, &[0.0, 0.2, 0.3, 0.0, 0.5, 0.0]);
    match session.robot_problem(robot) {
        Some(StateProblem::Collision { a, b }) => assert!(a == "Body1" || b == "Body1", "{a} / {b}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(session.doc().revision(), revision);
    session.jog(robot, &LEFT);
    assert_eq!(session.robot_problem(robot), None);
    session.jog(robot, &[0.0, 0.2, 0.3, 0.0, 0.5, 0.0]);
    assert!(session.robot_problem(robot).is_some());
    // Moving the body away (still a preview) clears it; moving the robot's base into it brings it back.
    let mut moved = session.doc().get(pillar).unwrap().clone();
    moved.placement.xyz = [0.0, 1.5, 0.7];
    session.preview_body(pillar, moved);
    assert_eq!(session.robot_problem(robot), None);
    session.preview_base(robot, Placement { xyz: [-0.55, 1.5, 0.0], rpy: [0.0; 3] });
    assert!(matches!(session.robot_problem(robot), Some(StateProblem::Collision { .. })));
}

#[test]
fn waypoints_and_paths_are_about_the_tool_centre_point() {
    let mut session = Session::new();
    let robot = session.add_robot("Robot1", &six_axis()).unwrap();
    session
        .execute(&Command::Cell(CellOp::PlaceRobot {
            id: robot,
            placement: Placement { xyz: [0.0, -0.6, 0.0], rpy: [0.0; 3] },
        }))
        .unwrap();
    session.jog(robot, &[1.5, -0.3, 0.6, 0.0, 0.6, 0.0]);
    session.end_jog();
    let flange = session.tcp_pose(robot).unwrap();
    let here = session.add_waypoint_here(robot).unwrap();

    // A tool 100 mm long: previewed live, committed as one step.
    let (revision, depth) = (session.doc().revision(), session.history().undo_depth());
    let tcp = Placement { xyz: [0.0, 0.0, 0.1], rpy: [0.0; 3] };
    session.preview_tcp(robot, tcp);
    assert_eq!(session.doc().revision(), revision);
    let tip = session.tcp_pose(robot).unwrap();
    assert!((tip.translation - flange * gc_math::DVec3::new(0.0, 0.0, 0.1)).length() < 1e-12);
    session.end_tcp_preview();
    assert_eq!(session.history().undo_depth(), depth + 1);
    assert_eq!(session.doc().get(robot).unwrap().tcp, tcp);
    // A joint waypoint is where the tool centre point is in those joint values.
    assert!((session.waypoint_pose(here).unwrap().translation - tip.translation).length() < 1e-12);

    // A sketch point on the wall is reached by the tool centre point; the flange stays 100 mm short.
    let sketch = session.create_sketch("XZ").unwrap();
    let created = session
        .execute(&Command::Sketch { sketch, op: SketchOp::AddPoint { pos: [0.1, 0.5, 0.0] } })
        .unwrap();
    let gc_doc::Created::Point(point) = created[0] else { panic!("no point") };
    session.finish_sketch();
    let on_wall = session.add_waypoint(robot, WaypointTarget::SketchPoint { sketch, point }).unwrap();
    let path = session.add_path(robot, vec![on_wall]).unwrap();
    assert_eq!(session.plan(path), PlanStatus::Ready, "{:?}", session.plan_error(path));
    let end = session.planned(path).unwrap().tip_path().last().unwrap();
    assert!((end[0] - 0.1).abs() < 1e-4 && end[1].abs() < 1e-4 && (end[2] - 0.5).abs() < 1e-4, "{end:?}");
    let duration = session.planned(path).unwrap().duration();
    session.set_playback(Some((path, duration)));
    let shown = session.tcp_pose(robot).unwrap().translation;
    assert!((shown - gc_math::DVec3::new(0.1, 0.0, 0.5)).length() < 1e-4);
    session.set_playback(None);

    // Another tool makes the plan out of date.
    session
        .execute(&Command::Cell(CellOp::SetTcp {
            id: robot,
            tcp: Placement { xyz: [0.0, 0.0, 0.15], rpy: [0.0; 3] },
        }))
        .unwrap();
    assert_eq!(session.plan_status(path), PlanStatus::Stale);
}

/// A fully constrained sketch point is the origin of a work frame; waypoints defined in the frame, and
/// the paths through them, move with it when a dimension changes.
#[test]
fn waypoints_in_a_work_frame_follow_its_origin() {
    use gc_doc::{EditError, WaypointTarget};
    use gc_math::DVec3;
    use gc_sketch::{DimKind, GeoConstraint};
    let mut session = Session::new();
    let robot = session.add_robot("Robot1", &six_axis()).unwrap();
    session.jog(robot, &[0.0, 0.3, 0.9, 0.0, 0.4, 0.0]);
    session.end_jog();

    // A point on the floor (XY plane), 400 mm along X and 100 mm along Y from the origin.
    let sketch = session.create_sketch("XY").unwrap();
    let origin = *session.active_sketch().unwrap().1.points.keys().next().unwrap();
    let created = session
        .execute(&Command::Sketch { sketch, op: SketchOp::AddPoint { pos: [0.38, 0.12, 0.0] } })
        .unwrap();
    let gc_doc::Created::Point(point) = created[0] else { panic!("no point") };

    // Not yet fully constrained: refused.
    assert_eq!(session.add_work_frame(sketch, point), Err(EditError::Underconstrained));
    let dimension =
        |kind, source: &str| SketchOp::AddDimension { kind, source: source.to_owned(), text_pos: [0.0; 2] };
    let created = session
        .execute(&Command::Sketch {
            sketch,
            op: dimension(DimKind::HDistance { p: origin, q: point, sign: 1.0 }, "400 mm"),
        })
        .unwrap();
    let gc_doc::Created::Dimension(along_x) = created[0] else { panic!("no dimension") };
    assert_eq!(
        session.add_work_frame(sketch, point),
        Err(EditError::Underconstrained),
        "one direction is still free"
    );
    session
        .execute(&Command::Sketch {
            sketch,
            op: dimension(DimKind::VDistance { p: origin, q: point, sign: 1.0 }, "100 mm"),
        })
        .unwrap();
    let frame = session.add_work_frame(sketch, point).unwrap();
    session.finish_sketch();
    let at = session.work_frame_pose(frame).unwrap().translation;
    assert!((at - DVec3::new(0.4, 0.1, 0.0)).length() < 1e-9, "{at:?}");

    // Two waypoints in the frame: 400 mm above the origin, and 100 mm beside that, tool pointing down.
    let down = Placement { xyz: [0.0, 0.0, 0.4], rpy: [std::f64::consts::PI, 0.0, 0.0] };
    let above = session.add_waypoint(robot, WaypointTarget::InFrame { frame, offset: down }).unwrap();
    let beside = Placement { xyz: [0.0, 0.1, 0.4], ..down };
    let next = session.add_waypoint(robot, WaypointTarget::InFrame { frame, offset: beside }).unwrap();
    let pose = session.waypoint_pose(above).unwrap();
    assert!((pose.translation - DVec3::new(0.4, 0.1, 0.4)).length() < 1e-9);
    assert!((pose.rotation * DVec3::Z - (-DVec3::Z)).length() < 1e-9, "the tool points down");
    let path = session.add_path(robot, vec![above, next]).unwrap();
    session.execute(&Command::Cell(CellOp::SetLinear { id: path, index: 1, linear: true })).unwrap();
    assert_eq!(session.plan(path), PlanStatus::Ready, "{:?}", session.plan_error(path));
    let end = session.planned(path).unwrap().tip_path().last().unwrap();
    assert!((DVec3::from_array(end) - DVec3::new(0.4, 0.2, 0.4)).length() < 1e-4, "{end:?}");

    // The origin moves 50 mm along X: both waypoints follow, the path is out of date, and planning
    // again ends at the moved place.
    session.enter_sketch(sketch);
    session.edit_dimension(along_x, "450 mm").unwrap();
    session.finish_sketch();
    assert!((session.waypoint_pose(next).unwrap().translation - DVec3::new(0.45, 0.2, 0.4)).length() < 1e-9);
    assert_eq!(session.plan_status(path), PlanStatus::Stale);
    assert_eq!(session.plan(path), PlanStatus::Ready, "{:?}", session.plan_error(path));
    let end = session.planned(path).unwrap().tip_path().last().unwrap();
    assert!((DVec3::from_array(end) - DVec3::new(0.45, 0.2, 0.4)).length() < 1e-4, "{end:?}");

    // Turning the frame turns everything in it: 90 degrees about its Z, "beside" now lies along -X.
    let mut turned = session.doc().get(frame).unwrap().clone();
    turned.rpy = [0.0, 0.0, std::f64::consts::FRAC_PI_2];
    session.execute(&Command::Cell(CellOp::SetFrame { id: frame, frame: turned })).unwrap();
    assert!((session.waypoint_pose(next).unwrap().translation - DVec3::new(0.35, 0.1, 0.4)).length() < 1e-9);

    // Teaching: the robot's current tool pose, stored relative to the frame, follows the frame too.
    let taught = session.add_waypoint_in_frame(robot, frame).unwrap();
    let here = session.tcp_pose(robot).unwrap();
    assert!((session.waypoint_pose(taught).unwrap().translation - here.translation).length() < 1e-9);
    session.enter_sketch(sketch);
    session.edit_dimension(along_x, "500 mm").unwrap();
    session.finish_sketch();
    let moved = session.waypoint_pose(taught).unwrap().translation;
    assert!((moved - here.translation - DVec3::new(0.05, 0.0, 0.0)).length() < 1e-9);

    // A frame in use cannot be deleted; a fixed point is fully constrained too.
    assert!(session.execute(&Command::Cell(CellOp::DeleteFrame(frame))).is_err());
    session.enter_sketch(sketch);
    let created = session
        .execute(&Command::Sketch { sketch, op: SketchOp::AddPoint { pos: [0.1, 0.3, 0.0] } })
        .unwrap();
    let gc_doc::Created::Point(fixed) = created[0] else { panic!("no point") };
    session
        .execute(&Command::Sketch { sketch, op: SketchOp::AddConstraint(GeoConstraint::Fix(fixed)) })
        .unwrap();
    assert!(session.add_work_frame(sketch, fixed).is_ok());

    // Saved and opened again, the frame and its waypoints are still there.
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("frames.gctrl");
    session.save_as(&file).unwrap();
    let mut reopened = Session::new();
    reopened.open(&file).unwrap();
    assert_eq!(reopened.doc().work_frames().len(), 2);
    assert!((reopened.waypoint_pose(taught).unwrap().translation - moved).length() < 1e-9);
}

/// The axes of a work frame set by sketch points: the frame turns with them, and so does everything in it.
#[test]
fn a_work_frame_turns_with_its_axis_points() {
    use gc_doc::{EditError, WaypointTarget};
    use gc_math::DVec3;
    use gc_sketch::{DimKind, GeoConstraint, Item};
    use gc_tools::ToolEvent;
    let mut session = Session::new();
    let robot = session.add_robot("Robot1", &six_axis()).unwrap();
    let sketch = session.create_sketch("XY").unwrap();
    let sketch_origin = *session.active_sketch().unwrap().1.points.keys().next().unwrap();
    let add_point = |session: &mut Session, pos: [f64; 3]| {
        let created = session.execute(&Command::Sketch { sketch, op: SketchOp::AddPoint { pos } }).unwrap();
        let gc_doc::Created::Point(point) = created[0] else { panic!("no point") };
        point
    };
    let dimension = |session: &mut Session, kind, source: &str| {
        let op = SketchOp::AddDimension { kind, source: source.to_owned(), text_pos: [0.0; 2] };
        let created = session.execute(&Command::Sketch { sketch, op }).unwrap();
        let gc_doc::Created::Dimension(id) = created[0] else { panic!("no dimension") };
        id
    };
    // The origin at (400, 100) mm; the X point 200 mm along and 100 mm up from it.
    let origin = add_point(&mut session, [0.39, 0.11, 0.0]);
    dimension(&mut session, DimKind::HDistance { p: sketch_origin, q: origin, sign: 1.0 }, "400 mm");
    dimension(&mut session, DimKind::VDistance { p: sketch_origin, q: origin, sign: 1.0 }, "100 mm");
    let x_point = add_point(&mut session, [0.61, 0.19, 0.0]);
    dimension(&mut session, DimKind::HDistance { p: origin, q: x_point, sign: 1.0 }, "200 mm");
    let rise = dimension(&mut session, DimKind::VDistance { p: origin, q: x_point, sign: 1.0 }, "100 mm");

    // Picked in order with Ctrl: origin, then the X point.
    session.set_tool(gc_tools::ToolKind::Select);
    for (at, extend) in [([0.4, 0.1], false), ([0.6, 0.2], true)] {
        session.tool_event(ToolEvent::Move { pos: at, px: 1e-4, suppress_snap: false });
        session.tool_event(ToolEvent::Click { pos: at, px: 1e-4, modifier: extend });
    }
    assert_eq!(session.selection_order(), vec![Item::Point(origin), Item::Point(x_point)]);
    let frame = session.add_work_frame_by_points(sketch, &[origin, x_point]).unwrap();
    let pose = session.work_frame_pose(frame).unwrap();
    let x_axis = DVec3::new(2.0, 1.0, 0.0).normalize();
    assert!((pose.translation - DVec3::new(0.4, 0.1, 0.0)).length() < 1e-9);
    assert!((pose.rotation * DVec3::X - x_axis).length() < 1e-9);
    assert!((pose.rotation * DVec3::Z - DVec3::Z).length() < 1e-9, "Z stays the sketch's normal");

    // A waypoint 100 mm along the frame's X, 400 mm up.
    let offset = Placement { xyz: [0.1, 0.0, 0.4], rpy: [std::f64::consts::PI, 0.0, 0.0] };
    let waypoint = session.add_waypoint(robot, WaypointTarget::InFrame { frame, offset }).unwrap();
    let expected = |axis: DVec3| DVec3::new(0.4, 0.1, 0.4) + axis * 0.1;
    assert!((session.waypoint_pose(waypoint).unwrap().translation - expected(x_axis)).length() < 1e-9);

    // The X point rises to 200 mm: the frame turns to 45 degrees and the waypoint turns with it.
    session.edit_dimension(rise, "200 mm").unwrap();
    let turned = DVec3::new(1.0, 1.0, 0.0).normalize();
    assert!((session.work_frame_pose(frame).unwrap().rotation * DVec3::X - turned).length() < 1e-9);
    assert!((session.waypoint_pose(waypoint).unwrap().translation - expected(turned)).length() < 1e-9);

    // A free point is refused, and so is a third point on the line through the first two.
    let free = add_point(&mut session, [0.1, 0.4, 0.0]);
    assert_eq!(session.add_work_frame_by_points(sketch, &[origin, free]), Err(EditError::Underconstrained));
    let on_line = add_point(&mut session, [0.8, 0.5, 0.0]);
    session
        .execute(&Command::Sketch { sketch, op: SketchOp::AddConstraint(GeoConstraint::Fix(on_line)) })
        .unwrap();
    assert_eq!(
        session.add_work_frame_by_points(sketch, &[origin, x_point, on_line]),
        Err(EditError::DegenerateFrame)
    );
    assert_eq!(session.add_work_frame_by_points(sketch, &[origin, origin]), Err(EditError::DegenerateFrame));
    session.finish_sketch();

    // Three points in a 3D sketch give all six numbers: the frame stands on a slope.
    let spatial = session.create_sketch_3d().unwrap();
    let fixed = |session: &mut Session, pos: [f64; 3]| {
        let created =
            session.execute(&Command::Sketch { sketch: spatial, op: SketchOp::AddPoint { pos } }).unwrap();
        let gc_doc::Created::Point(point) = created[0] else { panic!("no point") };
        session
            .execute(&Command::Sketch {
                sketch: spatial,
                op: SketchOp::AddConstraint(GeoConstraint::Fix(point)),
            })
            .unwrap();
        point
    };
    let o = fixed(&mut session, [0.5, 0.0, 0.2]);
    let x = fixed(&mut session, [0.5, 0.3, 0.2]);
    let xy = fixed(&mut session, [0.3, 0.0, 0.4]);
    let slope = session.add_work_frame_by_points(spatial, &[o, x, xy]).unwrap();
    let pose = session.work_frame_pose(slope).unwrap();
    let (ex, ey, ez) = (pose.rotation * DVec3::X, pose.rotation * DVec3::Y, pose.rotation * DVec3::Z);
    assert!((ex - DVec3::Y).length() < 1e-9);
    assert!((ey - DVec3::new(-1.0, 0.0, 1.0).normalize()).length() < 1e-9, "{ey:?}");
    assert!((ez - DVec3::new(1.0, 0.0, 1.0).normalize()).length() < 1e-9, "{ez:?}");

    // The axis points can be changed later: back to the sketch's axes.
    session.set_frame_axes(slope, None, None).unwrap();
    assert!((session.work_frame_pose(slope).unwrap().rotation * DVec3::X - DVec3::X).length() < 1e-9);
}

/// A project keeps its mesh files: saved, moved elsewhere with the original files gone, it opens whole.
#[test]
fn a_project_carries_its_mesh_files() {
    let directory = tempfile::tempdir().unwrap();
    let elsewhere = directory.path().join("downloads");
    // Equipment from a CAD export, and a robot package with a mesh, both outside the project.
    std::fs::create_dir_all(elsewhere.join("arm/urdf")).unwrap();
    std::fs::create_dir_all(elsewhere.join("arm/meshes")).unwrap();
    let table = gc_mesh::primitives::cuboid([800.0, 600.0, 50.0]).unwrap();
    std::fs::write(elsewhere.join("table.stl"), gc_mesh::stl::to_binary(&table)).unwrap();
    let column = gc_mesh::primitives::cylinder(0.05, 0.4, 16).unwrap();
    std::fs::write(elsewhere.join("arm/meshes/column.stl"), gc_mesh::stl::to_binary(&column)).unwrap();
    let urdf = r#"<?xml version="1.0"?>
<robot name="arm">
  <link name="base_link">
    <visual><geometry><mesh filename="package://arm/meshes/column.stl"/></geometry></visual>
    <collision><geometry><mesh filename="package://arm/meshes/column.stl"/></geometry></collision>
  </link>
  <link name="tip"><collision><geometry><box size="0.1 0.1 0.1"/></geometry></collision></link>
  <joint name="swing" type="revolute">
    <parent link="base_link"/><child link="tip"/><origin xyz="0 0 0.3"/><axis xyz="0 0 1"/>
    <limit lower="-3" upper="3" velocity="2" effort="10"/>
  </joint>
</robot>"#;
    std::fs::write(elsewhere.join("arm/urdf/arm.urdf"), urdf).unwrap();

    let mut session = Session::new();
    let robot = session.add_robot_file(&elsewhere.join("arm/urdf/arm.urdf")).unwrap();
    let body = session.add_mesh_body(&elsewhere.join("table.stl")).unwrap();
    // The robot's tool is a mesh file too.
    std::fs::copy(elsewhere.join("table.stl"), elsewhere.join("gripper.stl")).unwrap();
    session.set_tool_mesh(robot, &elsewhere.join("gripper.stl")).unwrap();
    assert!(session.doc().get(robot).unwrap().tool.is_some());
    let project = directory.path().join("cell");
    std::fs::create_dir_all(&project).unwrap();
    let file = project.join("Line 1.gctrl");
    session.save_as(&file).unwrap();
    assert!(!session.is_dirty(), "saving does not change the document");

    // The files went into the project's folder and the file refers to them relative to itself.
    assert!(project.join("Line_1_files/table.stl").is_file());
    assert!(project.join("Line_1_files/gripper.stl").is_file());
    assert!(project.join("Line_1_files/arm/meshes/column.stl").is_file());
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("\"Line_1_files/table.stl\"") && text.contains("\"Line_1_files/arm\""), "{text}");
    assert!(!text.contains(&*directory.path().to_string_lossy()), "no absolute paths in the file");

    // Moved to another place, the originals gone: everything is still there.
    let moved = directory.path().join("moved");
    std::fs::rename(&project, &moved).unwrap();
    std::fs::remove_dir_all(&elsewhere).unwrap();
    let mut reopened = Session::new();
    reopened.open(&moved.join("Line 1.gctrl")).unwrap();
    assert!(!reopened.is_dirty());
    let stored = reopened.doc().get(body).unwrap();
    assert_eq!(reopened.body_mesh(&stored.solid).unwrap().data.triangle_count(), 12);
    let loaded = reopened.robot_model(robot).unwrap();
    assert!(loaded.missing.is_empty(), "{:?}", loaded.missing);
    assert_eq!(loaded.mesh("package://arm/meshes/column.stl", [1.0; 3]).len(), 1);
    let tool = reopened.doc().get(robot).unwrap().tool.clone().unwrap();
    assert_eq!(
        reopened.body_mesh(&tool.solid).unwrap().data.triangle_count(),
        12,
        "the tool's mesh is found"
    );

    // Saving it again where it is copies nothing new and keeps the same paths.
    reopened.save().unwrap();
    assert_eq!(std::fs::read_to_string(moved.join("Line 1.gctrl")).unwrap(), text);
}

#[test]
fn a_waypoint_shows_how_the_robot_would_stand_there() {
    use gc_sketch::DimKind;
    let mut session = Session::new();
    let robot = session.add_robot("Robot1", &six_axis()).unwrap();
    session.jog(robot, &[0.0, 0.3, 0.9, 0.0, 0.4, 0.0]);
    session.end_jog();
    // A work frame on a fully dimensioned floor point, and a waypoint 400 mm above it, tool down.
    let sketch = session.create_sketch("XY").unwrap();
    let origin = *session.active_sketch().unwrap().1.points.keys().next().unwrap();
    let created = session
        .execute(&Command::Sketch { sketch, op: SketchOp::AddPoint { pos: [0.38, 0.12, 0.0] } })
        .unwrap();
    let gc_doc::Created::Point(point) = created[0] else { panic!("no point") };
    for (kind, source) in [
        (DimKind::HDistance { p: origin, q: point, sign: 1.0 }, "400 mm"),
        (DimKind::VDistance { p: origin, q: point, sign: 1.0 }, "100 mm"),
    ] {
        let op = SketchOp::AddDimension { kind, source: source.to_owned(), text_pos: [0.0; 2] };
        session.execute(&Command::Sketch { sketch, op }).unwrap();
    }
    let frame = session.add_work_frame(sketch, point).unwrap();
    session.finish_sketch();
    let down = Placement { xyz: [0.0, 0.0, 0.4], rpy: [std::f64::consts::PI, 0.0, 0.0] };
    let waypoint = session.add_waypoint(robot, WaypointTarget::InFrame { frame, offset: down }).unwrap();
    assert!(session.shown_ghost().is_none(), "nothing selected");

    // Selected, the waypoint shows the arm reaching it; the document is untouched.
    let joints = session.doc().get(robot).unwrap().joints.clone();
    session.select_item(Some(gc_session::CellItem::Waypoint(waypoint)));
    let ghost = session.shown_ghost().unwrap();
    assert_eq!(ghost.robot, robot);
    assert!(ghost.problem.is_none(), "{ghost:?}");
    let links = session.ghost_links(&ghost).unwrap();
    let tip = session.robot_model(robot).unwrap().model.link("tool0").unwrap();
    let stored = session.doc().get(robot).unwrap();
    let reached = links[tip] * gc_math::pose_from_xyz_rpy(stored.tcp.xyz, stored.tcp.rpy);
    let wanted = session.waypoint_pose(waypoint).unwrap();
    assert!((reached.translation - wanted.translation).length() < 1e-5, "{reached:?} {wanted:?}");
    assert!(reached.rotation.angle_between(wanted.rotation) < 1e-5);
    assert_eq!(session.doc().get(robot).unwrap().joints, joints);

    // A box where the arm would be: reached, but with a problem.
    let at = wanted.translation;
    let block = session
        .add_body(Solid::Box { size: [0.2, 0.2, 0.2] }, Placement { xyz: at.to_array(), rpy: [0.0; 3] })
        .unwrap();
    let ghost = session.waypoint_ghost(waypoint).unwrap();
    assert!(ghost.joints.is_some() && ghost.problem.is_some(), "{ghost:?}");
    session.delete_item(gc_session::CellItem::Body(block)).unwrap();

    // The robot moved far away cannot reach it.
    let mut placement = session.doc().get(robot).unwrap().placement;
    placement.xyz[0] += 10.0;
    session.preview_base(robot, placement);
    let ghost = session.waypoint_ghost(waypoint).unwrap();
    assert!(ghost.joints.is_none() && session.ghost_links(&ghost).is_none());
}

#[test]
fn a_paths_waypoints_are_reordered_added_and_taken_out() {
    let mut session = Session::new();
    let robot = session.add_robot("Robot1", &six_axis()).unwrap();
    session.jog(robot, &RIGHT);
    session.end_jog();
    let right = session.add_waypoint_here(robot).unwrap();
    session.jog(robot, &LEFT);
    session.end_jog();
    let left = session.add_waypoint_here(robot).unwrap();
    let path = session.add_path(robot, vec![right, left]).unwrap();
    assert_eq!(session.plan(path), PlanStatus::Ready, "{:?}", session.plan_error(path));

    // Swapped, with the second approached in a straight line: one undo step, and the plan is out of date.
    let depth = session.history().undo_depth();
    let steps = |waypoints: Vec<_>, linear: Vec<bool>| {
        Command::Cell(CellOp::SetPathSteps { id: path, waypoints, linear })
    };
    session.execute(&steps(vec![left, right], vec![false, true])).unwrap();
    let stored = session.doc().get(path).unwrap();
    assert_eq!((stored.waypoints.clone(), stored.linear.clone()), (vec![left, right], vec![false, true]));
    assert_eq!(session.history().undo_depth(), depth + 1);
    assert_eq!(session.plan_status(path), PlanStatus::Stale);
    session.undo();
    assert_eq!(session.doc().get(path).unwrap().waypoints, vec![right, left]);

    // A waypoint twice is fine; an empty path, a waypoint of another robot or a wrong count is refused.
    session.execute(&steps(vec![right, left, right], vec![false; 3])).unwrap();
    assert!(session.execute(&steps(Vec::new(), Vec::new())).is_err());
    assert!(session.execute(&steps(vec![right], vec![false, true])).is_err());
    let other = session.add_robot("Robot2", &six_axis()).unwrap();
    let theirs = session.add_waypoint_here(other).unwrap();
    assert!(session.execute(&steps(vec![right, theirs], vec![false; 2])).is_err());
    assert_eq!(session.doc().get(path).unwrap().waypoints, vec![right, left, right]);
}

#[test]
fn a_tool_on_the_flange_is_checked_for_collisions() {
    use gc_math::DVec3;
    let mut session = Session::new();
    let robot = session.add_robot("Robot1", &six_axis()).unwrap();
    session.jog(robot, &LEFT);
    session.end_jog();
    // A small box 250 mm out along the flange's Z axis: clear of the bare arm.
    let flange = session.tcp_pose(robot).unwrap();
    let at = flange.transform_point(DVec3::new(0.0, 0.0, 0.25));
    session
        .add_body(Solid::Box { size: [0.05; 3] }, Placement { xyz: at.to_array(), rpy: [0.0; 3] })
        .unwrap();
    assert!(session.robot_problem(robot).is_none());

    // A 300 mm tool reaches into it. Previewing changes the check but not the document; releasing makes
    // one undo step.
    let tool = gc_doc::Tool {
        solid: Solid::Cylinder { radius: 0.02, length: 0.3 },
        placement: Placement { xyz: [0.0, 0.0, 0.15], rpy: [0.0; 3] },
    };
    let depth = session.history().undo_depth();
    session.preview_tool(robot, Some(tool.clone()));
    assert!(session.robot_problem(robot).is_some(), "the tool hits the box");
    assert_eq!(session.doc().get(robot).unwrap().tool.as_deref(), None);
    session.end_tool_preview();
    assert_eq!(session.doc().get(robot).unwrap().tool.as_deref(), Some(&tool));
    assert_eq!(session.history().undo_depth(), depth + 1);
    assert!(session.robot_problem(robot).is_some());

    // An invalid tool is ignored; undo takes the tool off again.
    let mut bad = tool;
    bad.solid = Solid::Cylinder { radius: -1.0, length: 0.3 };
    session.preview_tool(robot, Some(bad.clone()));
    assert_ne!(session.shown_tool(robot).unwrap(), Some(bad.clone()));
    assert!(session.execute(&Command::Cell(CellOp::SetTool { id: robot, tool: Some(bad) })).is_err());
    session.undo();
    assert!(session.robot_problem(robot).is_none());
}

#[test]
fn a_straight_move_keeps_to_the_tool_speed_limit() {
    let mut session = Session::new();
    let robot = session.add_robot("Robot1", &six_axis()).unwrap();
    session.jog(robot, &[0.6, 0.2, 0.3, 0.0, 0.5, 0.0]);
    session.end_jog();
    let there = session.add_waypoint_here(robot).unwrap();
    session.jog(robot, &[-0.6, 0.2, 0.3, 0.0, 0.5, 0.0]);
    session.end_jog();
    let path = session.add_path(robot, vec![there]).unwrap();
    session.execute(&Command::Cell(CellOp::SetLinear { id: path, index: 0, linear: true })).unwrap();
    assert_eq!(session.plan(path), PlanStatus::Ready, "{:?}", session.plan_error(path));
    let (fast, peak) = {
        let planned = session.planned(path).unwrap();
        (planned.duration(), planned.peak_tcp_speed())
    };
    assert!(peak > 0.1, "{peak}");

    // Half the speed it reached: the limit is an edit, and the same line takes twice as long.
    let speed = |speed| Command::Cell(CellOp::SetTcpSpeed { id: path, speed });
    session.execute(&speed(Some(peak / 2.0))).unwrap();
    assert_eq!(session.plan_status(path), PlanStatus::Stale);
    assert_eq!(session.plan(path), PlanStatus::Ready, "{:?}", session.plan_error(path));
    let planned = session.planned(path).unwrap();
    assert!((planned.duration() / fast - 2.0).abs() < 1e-6, "{} vs {fast}", planned.duration());
    assert!(planned.peak_tcp_speed() <= peak / 2.0 + 1e-9);
    // The joints stay within their limits: slower everywhere, and the end is still reached.
    let end = planned.sample(planned.duration());
    assert!(end.iter().zip([0.6, 0.2, 0.3, 0.0, 0.5, 0.0]).all(|(a, b)| (a - b).abs() < 1e-6), "{end:?}");

    // A limit above what the move reaches changes nothing; zero or negative limits are refused.
    session.execute(&speed(Some(peak * 10.0))).unwrap();
    session.plan(path);
    assert!((session.planned(path).unwrap().duration() - fast).abs() < 1e-9);
    assert!(session.execute(&speed(Some(0.0))).is_err());
    assert!(session.execute(&speed(Some(-1.0))).is_err());
}

#[test]
fn a_waypoint_is_measured_from_the_frame_chosen_without_moving() {
    use gc_session::Reference;
    let mut session = Session::new();
    let robot = session.add_robot("Robot1", &six_axis()).unwrap();
    session.jog(robot, &RIGHT);
    session.end_jog();
    let waypoint = session.add_waypoint_here(robot).unwrap();
    session.jog(robot, &LEFT);
    session.end_jog();
    // The robot stands away from the origin, turned, so that world and base numbers differ.
    let placed = Placement { xyz: [0.5, -0.2, 0.1], rpy: [0.0, 0.0, 0.7] };
    session.execute(&Command::Cell(CellOp::PlaceRobot { id: robot, placement: placed })).unwrap();
    let world = session.waypoint_pose(waypoint).unwrap();
    assert_eq!(session.waypoint_reference(waypoint), None, "joint values");

    // Measured from the world, then from the base: the same place, other numbers.
    let close = |a: gc_math::DPose3, b: gc_math::DPose3| {
        (a.translation - b.translation).length() < 1e-9
            && [gc_math::DVec3::X, gc_math::DVec3::Z]
                .iter()
                .all(|v| (a.rotation * *v - b.rotation * *v).length() < 1e-9)
    };
    session.express_waypoint(waypoint, Reference::World).unwrap();
    assert_eq!(session.waypoint_reference(waypoint), Some(Reference::World));
    assert!(close(session.waypoint_pose(waypoint).unwrap(), world));
    let in_world = session.doc().get(waypoint).unwrap().target.clone();
    session.express_waypoint(waypoint, Reference::Base).unwrap();
    assert!(close(session.waypoint_pose(waypoint).unwrap(), world));
    let WaypointTarget::InBase { offset } = session.doc().get(waypoint).unwrap().target.clone() else {
        panic!("not in the base frame")
    };
    let WaypointTarget::InWorld { offset: absolute } = in_world.clone() else { panic!("not in the world") };
    assert!((offset.xyz[0] - absolute.xyz[0]).abs() > 0.01, "other numbers: {offset:?} {absolute:?}");
    // The base is the robot's root link here (no link named `base`).
    let base = session.robot_base_frame(robot).unwrap();
    assert!((base.translation - gc_math::DVec3::new(0.5, -0.2, 0.1)).length() < 1e-12);

    // A waypoint in the base frame moves with the robot; one in the world stays.
    let path = session.add_path(robot, vec![waypoint]).unwrap();
    assert_eq!(session.plan(path), PlanStatus::Ready, "{:?}", session.plan_error(path));
    let moved = Placement { xyz: [0.6, -0.2, 0.1], ..placed };
    session.execute(&Command::Cell(CellOp::PlaceRobot { id: robot, placement: moved })).unwrap();
    let shifted = session.waypoint_pose(waypoint).unwrap().translation - world.translation;
    assert!((shifted - gc_math::DVec3::new(0.1, 0.0, 0.0)).length() < 1e-9, "{shifted:?}");
    assert_eq!(session.plan(path), PlanStatus::Ready, "{:?}", session.plan_error(path));
    session.execute(&Command::Cell(CellOp::SetWaypointTarget { id: waypoint, target: in_world })).unwrap();
    assert!(close(session.waypoint_pose(waypoint).unwrap(), world), "in the world it stays");

    // Undo goes back through the changes of reference.
    session.undo();
    assert!(matches!(session.doc().get(waypoint).unwrap().target, WaypointTarget::InBase { .. }));
}

#[test]
fn robots_come_from_the_library_and_projects_refer_to_it() {
    use gc_session::library;
    let directory = tempfile::tempdir().unwrap();
    // A library with one package, and a folder that is not a package.
    let lib = directory.path().join("robots");
    std::fs::create_dir_all(lib.join("arm")).unwrap();
    std::fs::create_dir_all(lib.join("notes")).unwrap();
    std::fs::write(lib.join("arm/arm.urdf"), six_axis()).unwrap();
    std::fs::write(
        lib.join("arm/robot.json"),
        r#"{ "name": "Arm", "vendor": "Test", "description": "arm.urdf", "license": "LICENSE", "license_name": "MIT" }"#,
    )
    .unwrap();
    std::fs::write(lib.join("arm/LICENSE"), "licence text").unwrap();
    let packages = library::scan(&lib);
    assert_eq!(packages.len(), 1);
    assert_eq!((packages[0].id.as_str(), packages[0].info.name.as_str()), ("arm", "Arm"));
    assert_eq!(packages[0].license_text().as_deref(), Some("licence text"));
    assert_eq!(library::find(&[directory.path().join("a/b")]), Some(lib.clone()), "found from below");

    // Added from the library, the robot refers to it by name; a saved project keeps that name.
    let mut session = Session::new();
    session.set_robot_library(Some(lib.clone()));
    let robot = session.add_library_robot(&packages[0]).unwrap();
    assert_eq!(session.doc().get(robot).unwrap().resource_dir, "library://arm");
    assert!(session.robot_model(robot).is_some());
    let file = directory.path().join("project/cell.gctrl");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    session.save_as(&file).unwrap();
    assert!(std::fs::read_to_string(&file).unwrap().contains("\"library://arm\""));
    assert!(!directory.path().join("project/cell_files").exists(), "nothing copied");

    // Opened elsewhere with the library somewhere else, the robot is found there.
    let moved = directory.path().join("other/robots");
    std::fs::create_dir_all(moved.parent().unwrap()).unwrap();
    std::fs::rename(&lib, &moved).unwrap();
    let mut reopened = Session::new();
    reopened.set_robot_library(Some(moved));
    reopened.open(&file).unwrap();
    assert!(reopened.robot_model(robot).is_some());
}
