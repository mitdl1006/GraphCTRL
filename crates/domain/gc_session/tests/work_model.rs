//! The work model's data (plan 9.4, GraphCTRL's own): object types, objects, state variables and tasks
//! are saved, one undo step each, their conditions are checked and follow renames, and what is referred
//! to cannot be deleted.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use gc_core::Id;
use gc_doc::{
    Command, Created, EditError, Effect, EffectTarget, EndSnap, Location, MoveTarget, ObjectType,
    RobotChoice, SketchOp, StateValue, StateVar, Step, Task, WaypointTarget, WorkObject, WorkOp,
};
use gc_session::Session;
use gc_sketch::Item;

fn six_axis() -> String {
    let path = format!("{}/../../../fixtures/urdf/six_axis.urdf", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn made<T: gc_core::Entity>(s: &mut Session, op: WorkOp) -> Id<T> {
    let depth = s.history().undo_depth();
    let id = s.edit_work(op).unwrap().unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1, "one undo step");
    id.downcast().unwrap()
}

#[test]
fn a_screw_driving_task_is_defined_on_the_structure() {
    let mut s = Session::new();
    let robot = s.add_robot("Robot1", &six_axis()).unwrap();
    // A sketch with a point (a waypoint) and a line (a path).
    let sketch = s.create_sketch("XY").unwrap();
    let created =
        s.execute(&Command::Sketch { sketch, op: SketchOp::AddPoint { pos: [0.4, 0.1, 0.0] } }).unwrap();
    let Created::Point(point) = created[0] else { panic!() };
    let line = SketchOp::AddLine {
        a: [0.4, 0.2, 0.0],
        b: [0.5, 0.2, 0.0],
        snap_a: EndSnap::default(),
        snap_b: EndSnap::default(),
        horizontal: None,
    };
    let created = s.execute(&Command::Sketch { sketch, op: line }).unwrap();
    let curve =
        created.iter().find_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None }).unwrap();
    s.finish_sketch();
    let at_hole = s.add_waypoint_role(sketch, point).unwrap();
    let seam = s.add_path_role(sketch, &[curve]).unwrap();

    // Types, objects and state variables; names may hold spaces.
    let states = vec!["비어 있음".to_owned(), "체결됨".to_owned()];
    let hole: Id<ObjectType> =
        made(&mut s, WorkOp::AddObjectType(ObjectType { name: "홀".into(), states: states.clone() }));
    let object = |name: &str| WorkObject {
        name: name.into(),
        ty: hole,
        location: Location::Sketch(sketch),
        state: "비어 있음".into(),
    };
    let hole_a: Id<WorkObject> = made(&mut s, WorkOp::AddObject(object("홀 A")));
    let hole_b: Id<WorkObject> = made(&mut s, WorkOp::AddObject(object("홀B")));
    let count: Id<StateVar> =
        made(&mut s, WorkOp::AddStateVar(StateVar { name: "count".into(), value: StateValue::Int(0) }));
    // Names are unique and usable in conditions; states are of the type.
    assert_eq!(s.edit_work(WorkOp::AddObject(object("홀B"))), Err(EditError::NameTaken));
    assert!(matches!(s.edit_work(WorkOp::AddObject(object("a\"b"))), Err(EditError::BadName(_))));
    let wrong = WorkObject { state: "녹음".into(), ..object("홀C") };
    assert_eq!(s.edit_work(WorkOp::AddObject(wrong)), Err(EditError::BadValue));

    // Conditions are checked as they are typed, with the place of the mistake.
    assert_eq!(s.check_condition("\"홀 A\".state == \"비어 있음\" and count < 3"), Ok(()));
    let Err(EditError::Expr { start, end, .. }) = s.check_condition("홀B.state == 녹음") else { panic!() };
    assert_eq!((start, end), ("홀B.state == ".len(), "홀B.state == 녹음".len()));
    assert!(matches!(s.check_condition("홀Z.state == 체결됨"), Err(EditError::Expr { start: 0, .. })));
    assert!(
        matches!(s.check_condition("count == 체결됨"), Err(EditError::Expr { .. })),
        "a number and a name"
    );
    assert!(matches!(s.check_condition("count <"), Err(EditError::Expr { .. })));

    // The task: where, when, what, what it changes, who.
    let task = Task {
        name: "Fasten A".into(),
        location: Location::Sketch(sketch),
        precondition: "\"홀 A\".state == \"비어 있음\" and count < 3".into(),
        steps: vec![
            Step::MoveJ(MoveTarget::Role(at_hole)),
            Step::MoveL(MoveTarget::Role(at_hole)),
            Step::FollowPath(seam),
            Step::SetIo { name: "screwdriver".into(), value: true },
            Step::WaitIo { condition: "torque_ok".into(), timeout: 5.0 },
            Step::ToolAction { action: "fasten".into(), target: Some(hole_a) },
            Step::Wait { seconds: 0.2 },
        ],
        effects: vec![
            Effect { target: EffectTarget::Object(hole_a), value: "체결됨".into() },
            Effect { target: EffectTarget::Var(count), value: "1".into() },
        ],
        robots: RobotChoice::When("홀B.state == \"비어 있음\"".into()),
        priority: 2,
    };
    let fasten: Id<Task> = made(&mut s, WorkOp::AddTask(task.clone()));
    assert_eq!(s.task_ready(fasten), Ok(true));
    assert!(s.task_problems(fasten).is_empty());
    assert!(s.location_pose(&task.location).is_some());

    // What does not fit is refused: a path as a waypoint, a waypoint as a path, a state of another kind,
    // a robot's own waypoint in a task that is not that robot's.
    let refused = |s: &mut Session, edit: &dyn Fn(&mut Task)| {
        let mut bad = task.clone();
        edit(&mut bad);
        let before = s.doc().clone();
        let result = s.edit_work(WorkOp::AddTask(bad));
        assert!(s.doc().same_content(&before));
        result.unwrap_err()
    };
    assert_eq!(refused(&mut s, &|t| t.steps[0] = Step::MoveJ(MoveTarget::Role(seam))), EditError::BadValue);
    assert_eq!(refused(&mut s, &|t| t.steps[2] = Step::FollowPath(at_hole)), EditError::BadValue);
    assert_eq!(refused(&mut s, &|t| t.effects[0].value = "녹음".into()), EditError::BadValue);
    assert_eq!(refused(&mut s, &|t| t.effects[1].value = "many".into()), EditError::BadValue);
    assert_eq!(refused(&mut s, &|t| t.robots = RobotChoice::Only(Vec::new())), EditError::BadValue);
    assert!(matches!(refused(&mut s, &|t| t.precondition = "count <".into()), EditError::Expr { .. }));
    let home = s.add_waypoint(robot, WaypointTarget::Joints(vec![0.0; 6])).unwrap();
    assert_eq!(
        refused(&mut s, &|t| t.steps[0] = Step::MoveJ(MoveTarget::Waypoint(home))),
        EditError::BadValue
    );
    // The robot's own task: its own waypoints are fine.
    let own = Task {
        name: "Home".into(),
        location: Location::Robot(robot),
        precondition: String::new(),
        steps: vec![Step::MoveJ(MoveTarget::Waypoint(home))],
        effects: Vec::new(),
        robots: RobotChoice::Any,
        priority: 0,
    };
    let go_home: Id<Task> = made(&mut s, WorkOp::AddTask(own));
    assert_eq!(s.task_ready(go_home), Ok(true), "no precondition: always");
    assert_eq!(s.tasks(), vec![fasten, go_home], "in the order they were made");

    // Renames are followed, in one undo step: an object, a state variable, a state of the type.
    let depth = s.history().undo_depth();
    s.execute(&Command::Rename { item: hole_a.erase(), name: "HoleA".into() }).unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1);
    assert_eq!(s.doc().get(fasten).unwrap().precondition, "HoleA.state == \"비어 있음\" and count < 3");
    s.execute(&Command::Rename { item: count.erase(), name: "screws done".into() }).unwrap();
    assert_eq!(
        s.doc().get(fasten).unwrap().precondition,
        "HoleA.state == \"비어 있음\" and \"screws done\" < 3"
    );
    let renamed = ObjectType { name: "홀".into(), states: vec!["empty".into(), "체결됨".into()] };
    s.edit_work(WorkOp::SetObjectType { id: hole, ty: renamed }).unwrap();
    let stored = s.doc().get(fasten).unwrap().clone();
    assert_eq!(stored.precondition, "HoleA.state == empty and \"screws done\" < 3");
    assert_eq!(stored.robots, RobotChoice::When("홀B.state == empty".into()));
    assert_eq!(s.doc().get(hole_b).unwrap().state, "empty");
    assert_eq!(s.task_ready(fasten), Ok(true));
    assert!(s.undo());
    assert_eq!(s.doc().get(hole_b).unwrap().state, "비어 있음");
    assert!(s.redo());

    // What is referred to stays: a state, an object, a variable, a type, a role, the robot's waypoint.
    let fewer = ObjectType { name: "홀".into(), states: vec!["empty".into()] };
    assert_eq!(s.edit_work(WorkOp::SetObjectType { id: hole, ty: fewer }), Err(EditError::InUse));
    assert_eq!(s.edit_work(WorkOp::DeleteObject(hole_a)), Err(EditError::InUse));
    assert_eq!(s.edit_work(WorkOp::DeleteObject(hole_b)), Err(EditError::InUse), "named in a condition");
    assert_eq!(s.edit_work(WorkOp::DeleteStateVar(count)), Err(EditError::InUse));
    assert_eq!(s.edit_work(WorkOp::DeleteObjectType(hole)), Err(EditError::InUse));
    assert_eq!(s.delete_role(at_hole), Err(EditError::InUse));
    assert_eq!(s.delete_role(seam), Err(EditError::InUse));
    assert!(s.execute(&Command::Cell(gc_doc::CellOp::DeleteWaypoint(home))).is_err());
    // The object in another state: the task is not ready.
    let done = WorkObject { state: "체결됨".into(), ..s.doc().get(hole_a).unwrap().clone() };
    s.edit_work(WorkOp::SetObject { id: hole_a, object: done }).unwrap();
    assert_eq!(s.task_ready(fasten), Ok(false));

    // Saved and read back the same.
    let text = gc_persist::to_text(s.doc()).unwrap();
    assert!(gc_persist::from_text(&text).unwrap().doc.same_content(s.doc()));

    // Deleting the sketch point leaves the role for the task, which says what is wrong.
    s.execute(&Command::Sketch { sketch, op: SketchOp::Delete(vec![Item::Point(point)]) }).unwrap();
    assert!(s.doc().get(at_hole).is_some());
    assert!(!s.task_problems(fasten).is_empty());
    assert!(s.undo());
    assert!(s.task_problems(fasten).is_empty());

    // Without the tasks everything can go again.
    s.edit_work(WorkOp::DeleteTask(fasten)).unwrap();
    s.edit_work(WorkOp::DeleteObject(hole_a)).unwrap();
    s.edit_work(WorkOp::DeleteObject(hole_b)).unwrap();
    s.edit_work(WorkOp::DeleteObjectType(hole)).unwrap();
    s.edit_work(WorkOp::DeleteStateVar(count)).unwrap();
    s.delete_role(seam).unwrap();
}
