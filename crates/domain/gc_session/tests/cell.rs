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
