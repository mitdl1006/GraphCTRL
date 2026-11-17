//! The robot cell in the document: commands, undo and redo, and what may not be deleted.

#![allow(clippy::float_cmp, clippy::unwrap_used, clippy::panic, reason = "test code")]

use gc_doc::{
    Body, CellOp, Command, Created, Document, EditError, History, MotionPath, Placement, Robot, Solid,
    Waypoint, WaypointTarget, apply,
};

fn cell(doc: &mut Document, history: &mut History, op: CellOp) -> Result<Vec<Created>, EditError> {
    let applied = apply(doc, &Command::Cell(op))?;
    history.record(applied.label, applied.delta);
    Ok(applied.created)
}

fn robot() -> Robot {
    Robot {
        name: "Robot1".into(),
        urdf: "<robot name=\"r\"/>".into(),
        resource_dir: String::new(),
        base_link: "base_link".into(),
        tip_link: "tool0".into(),
        placement: Placement::default(),
        tcp: Placement::default(),
        joints: vec![0.0; 6],
        max_acc: 5.0,
        look: Default::default(),
        tool: None,
        acm_overrides: Vec::new(),
        links: Vec::new(),
    }
}

#[test]
fn a_cell_is_built_undone_and_redone() {
    let mut doc = Document::new();
    let mut history = History::new();
    let empty = doc.clone();
    let body = Body {
        name: "Table".into(),
        placement: Placement::default(),
        solid: Solid::Box { size: [1.0, 0.6, 0.05] },
        look: Default::default(),
    };
    cell(&mut doc, &mut history, CellOp::AddBody(body)).unwrap();
    let Created::Robot(robot_id) = cell(&mut doc, &mut history, CellOp::AddRobot(robot())).unwrap()[0] else {
        panic!()
    };
    cell(&mut doc, &mut history, CellOp::SetJoints { id: robot_id, joints: vec![0.1; 6] }).unwrap();
    let waypoint =
        Waypoint { name: "P1".into(), robot: robot_id, target: WaypointTarget::Joints(vec![0.5; 6]) };
    let Created::Waypoint(w) = cell(&mut doc, &mut history, CellOp::AddWaypoint(waypoint)).unwrap()[0] else {
        panic!()
    };
    let path = MotionPath {
        name: "Path1".into(),
        robot: robot_id,
        waypoints: vec![w],
        linear: Vec::new(),
        planner: "rrt_connect".into(),
        seed: 1,
        tcp_speed: None,
        contacts: Vec::new(),
    };
    let Created::Path(path_id) = cell(&mut doc, &mut history, CellOp::AddPath(path)).unwrap()[0] else {
        panic!()
    };
    cell(&mut doc, &mut history, CellOp::SetPlanner { id: path_id, planner: "prm".into(), seed: 3 }).unwrap();
    assert_eq!(doc.get(robot_id).unwrap().joints, vec![0.1; 6]);
    assert_eq!(doc.get(path_id).unwrap().planner, "prm");

    let built = doc.clone();
    while history.undo(&mut doc).is_some() {}
    assert!(doc.same_content(&empty));
    while history.redo(&mut doc).is_some() {}
    assert!(doc.same_content(&built));
}

#[test]
fn what_is_in_use_stays_and_bad_values_change_nothing() {
    let mut doc = Document::new();
    let mut history = History::new();
    let Created::Robot(robot_id) = cell(&mut doc, &mut history, CellOp::AddRobot(robot())).unwrap()[0] else {
        panic!()
    };
    let waypoint =
        Waypoint { name: "P1".into(), robot: robot_id, target: WaypointTarget::Joints(vec![0.5; 6]) };
    let Created::Waypoint(w) =
        cell(&mut doc, &mut history, CellOp::AddWaypoint(waypoint.clone())).unwrap()[0]
    else {
        panic!()
    };
    let path = MotionPath {
        name: "Path1".into(),
        robot: robot_id,
        waypoints: vec![w],
        linear: Vec::new(),
        planner: "rrt".into(),
        seed: 0,
        tcp_speed: None,
        contacts: Vec::new(),
    };
    let Created::Path(path_id) = cell(&mut doc, &mut history, CellOp::AddPath(path.clone())).unwrap()[0]
    else {
        panic!()
    };

    let before = doc.clone();
    let refused = [
        (CellOp::DeleteRobot(robot_id), EditError::InUse),
        (CellOp::DeleteWaypoint(w), EditError::InUse),
        (CellOp::SetJoints { id: robot_id, joints: vec![0.0; 5] }, EditError::BadValue),
        (CellOp::SetJoints { id: robot_id, joints: vec![f64::NAN; 6] }, EditError::BadValue),
        (
            CellOp::AddWaypoint(Waypoint { target: WaypointTarget::Joints(vec![0.0; 2]), ..waypoint }),
            EditError::BadValue,
        ),
        (CellOp::AddPath(MotionPath { waypoints: Vec::new(), ..path }), EditError::BadValue),
        (
            CellOp::AddBody(Body {
                name: "b".into(),
                placement: Placement::default(),
                solid: Solid::Sphere { radius: 0.0 },
                look: Default::default(),
            }),
            EditError::BadValue,
        ),
    ];
    for (op, expected) in refused {
        match cell(&mut doc, &mut history, op.clone()) {
            Err(error) => {
                assert_eq!(std::mem::discriminant(&error), std::mem::discriminant(&expected), "{op:?}")
            }
            Ok(_) => panic!("{op:?} was accepted"),
        }
        assert!(doc.same_content(&before), "{op:?}");
        assert_eq!(doc.revision(), before.revision());
    }
    // In the right order everything can go.
    cell(&mut doc, &mut history, CellOp::DeletePath(path_id)).unwrap();
    cell(&mut doc, &mut history, CellOp::DeleteWaypoint(w)).unwrap();
    cell(&mut doc, &mut history, CellOp::DeleteRobot(robot_id)).unwrap();
    assert!(doc.robots().is_empty() && doc.waypoints().is_empty() && doc.paths().is_empty());
}
