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
