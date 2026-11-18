//! Tool kits (plan 9.4, GraphCTRL's own): a kind of tool with its signals, states and actions; robots are
//! fitted with one; a task's tool action needs a robot whose tool can do it, takes the action's time and
//! changes the tool's and the object's state in the schedule.

#![allow(clippy::unwrap_used, clippy::panic, clippy::float_cmp, reason = "test code")]

use gc_core::Id;
use gc_doc::{
    CellOp, Command, Created, EditError, IoDirection, IoSignal, Location, MoveTarget, ObjectType, Placement,
    Robot, RobotChoice, SketchOp, SketchRole, Step, Task, ToolAction, ToolKit, WorkObject, WorkOp,
};
use gc_session::Session;

fn six_axis() -> String {
    let path = format!("{}/../../../fixtures/urdf/six_axis.urdf", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn made<T: gc_core::Entity>(s: &mut Session, op: WorkOp) -> Id<T> {
    s.edit_work(op).unwrap().unwrap().downcast().unwrap()
}

fn action(name: &str) -> ToolAction {
    ToolAction {
        name: name.into(),
        target: None,
        precondition: String::new(),
        set: None,
        confirm: None,
        tool_state: None,
        target_state: None,
        duration: 0.0,
        timeout: 0.0,
    }
}

fn task(name: &str, steps: Vec<Step>) -> Task {
    Task {
        name: name.into(),
        location: Location::None,
        precondition: String::new(),
        steps,
        effects: Vec::new(),
        robots: RobotChoice::Any,
        priority: 0,
    }
}

/// Two robots in front of the XZ wall and a waypoint on it.
fn cell() -> (Session, [Id<Robot>; 2], Id<SketchRole>) {
    let mut s = Session::new();
    let robot = |s: &mut Session, name: &str, x: f64| {
        let id = s.add_robot(name, &six_axis()).unwrap();
        let placement = Placement { xyz: [x, -0.6, 0.0], rpy: [0.0; 3] };
        s.execute(&Command::Cell(CellOp::PlaceRobot { id, placement })).unwrap();
        s.jog(id, &[1.5, -0.3, 0.6, 0.0, 0.6, 0.0]);
        s.end_jog();
        id
    };
    let robots = [robot(&mut s, "Left", 0.0), robot(&mut s, "Right", 0.7)];
    let sketch = s.create_sketch("XZ").unwrap();
    let created =
        s.execute(&Command::Sketch { sketch, op: SketchOp::AddPoint { pos: [0.3, 0.5, 0.0] } }).unwrap();
    let Created::Point(point) = created[0] else { panic!() };
    s.finish_sketch();
    let role = s.add_waypoint_role(sketch, point).unwrap();
    (s, robots, role)
}

#[test]
fn a_screw_driver_picks_then_fastens_and_only_the_robot_that_carries_it_can() {
    let (mut s, [left, right], at_hole) = cell();
    let states = vec!["empty".to_owned(), "done".to_owned()];
    let hole: Id<ObjectType> =
        made(&mut s, WorkOp::AddObjectType(ObjectType { name: "Hole".into(), states }));
    let a: Id<WorkObject> = made(
        &mut s,
        WorkOp::AddObject(WorkObject {
            name: "A".into(),
            ty: hole,
            location: Location::None,
            state: "empty".into(),
        }),
    );
    assert!(matches!(
        s.edit_work(WorkOp::AddObject(WorkObject {
            name: "tool".into(),
            ty: hole,
            location: Location::None,
            state: "empty".into()
        })),
        Err(EditError::BadName(_))
    ));

    // The screw driver: two signals, two states, two actions.
    let signal = |name: &str, direction, pin| IoSignal { name: name.into(), direction, pin };
    let driver = ToolKit {
        name: "Screw driver".into(),
        signals: vec![signal("start", IoDirection::Out, 0), signal("torque ok", IoDirection::In, 1)],
        states: vec!["idle".into(), "loaded".into()],
        actions: vec![
            ToolAction {
                precondition: "tool.state == idle".into(),
                tool_state: Some("loaded".into()),
                duration: 0.5,
                ..action("pick")
            },
            ToolAction {
                target: Some(hole),
                precondition: "tool.state == loaded and target.state == empty".into(),
                set: Some(("start".into(), true)),
                confirm: Some(("torque ok".into(), true)),
                tool_state: Some("idle".into()),
                target_state: Some("done".into()),
                duration: 2.0,
                timeout: 5.0,
                ..action("fasten")
            },
        ],
    };
    // What does not fit is refused: a signal the wrong way, a state the tool or the object does not have,
    // a condition with a word nothing knows.
    let refused = |s: &mut Session, edit: &dyn Fn(&mut ToolKit)| {
        let mut bad = driver.clone();
        edit(&mut bad);
        s.edit_work(WorkOp::AddToolKit(bad)).unwrap_err()
    };
    assert_eq!(
        refused(&mut s, &|k| k.actions[1].set = Some(("torque ok".into(), true))),
        EditError::BadValue
    );
    assert_eq!(refused(&mut s, &|k| k.actions[0].tool_state = Some("hot".into())), EditError::BadValue);
    assert_eq!(refused(&mut s, &|k| k.actions[0].target_state = Some("done".into())), EditError::BadValue);
    assert_eq!(refused(&mut s, &|k| k.actions[1].duration = -1.0), EditError::BadValue);
    assert!(matches!(
        refused(&mut s, &|k| k.actions[0].precondition = "tool.state == hot".into()),
        EditError::Expr { .. }
    ));
    assert_eq!(refused(&mut s, &|k| k.signals[1].name = "start".into()), EditError::NameTaken);
    let clash = |k: &mut ToolKit| k.signals.push(signal("stop", IoDirection::Out, 0));
    assert_eq!(refused(&mut s, &clash), EditError::BadValue, "two outputs on pin 0");
    // `tool.state` and `target.state` belong to a tool action's condition only.
    assert!(matches!(s.check_condition("tool.state == idle"), Err(EditError::Expr { start: 0, .. })));
    assert!(matches!(s.check_condition("target.state == empty"), Err(EditError::Expr { .. })));
    let depth = s.history().undo_depth();
    let kit: Id<ToolKit> = made(&mut s, WorkOp::AddToolKit(driver.clone()));
    assert_eq!(s.history().undo_depth(), depth + 1);

    // The left robot carries it; the right one has no tool kit.
    assert_eq!(s.robot_tool_kit(left), None);
    s.edit_work(WorkOp::SetRobotTool { robot: left, kit: Some(kit), state: None }).unwrap();
    assert_eq!(s.robot_tool_kit(left), Some((kit, Some("idle".to_owned()))));
    assert_eq!(
        s.edit_work(WorkOp::SetRobotTool { robot: right, kit: Some(kit), state: Some("hot".into()) }),
        Err(EditError::BadValue)
    );

    // Fasten comes first by priority but needs a loaded tool; Pick loads it; Inspect needs A done.
    let go = Step::MoveJ(MoveTarget::Role(at_hole));
    let fasten = Task {
        priority: 9,
        ..task("Fasten A", vec![go.clone(), Step::ToolAction { action: "fasten".into(), target: Some(a) }])
    };
    let fasten: Id<Task> = made(&mut s, WorkOp::AddTask(fasten));
    let pick: Id<Task> = made(
        &mut s,
        WorkOp::AddTask(task(
            "Pick",
            vec![
                Step::ToolAction { action: "pick".into(), target: None },
                Step::SetIo { name: "start".into(), value: false },
            ],
        )),
    );
    let inspect: Id<Task> = made(
        &mut s,
        WorkOp::AddTask(Task { precondition: "A.state == done".into(), ..task("Inspect", vec![go]) }),
    );
    assert!(s.task_problems(fasten).is_empty() && s.task_problems(pick).is_empty());

    // The bid has the action's time in it; the robot without the tool cannot.
    let plan = s.task_plan(fasten, left).unwrap();
    assert!((plan.waiting - 2.0).abs() < 1e-12);
    let why = s.task_plan(fasten, right).unwrap_err();
    assert!(why.contains("Right") && why.contains("fasten"), "{why}");
    assert!(s.task_plan(pick, right).is_err());
    let candidates = s.task_candidates(fasten);
    assert_eq!(
        (candidates[0].robot, candidates[0].time.is_ok(), candidates[1].time.is_ok()),
        (left, true, false)
    );

    // The schedule: pick (0.5 s), then fasten on the same robot, then inspect once A is done.
    let schedule = s.schedule();
    assert!(schedule.blocked.is_empty(), "{:?}", schedule.blocked);
    let of = |task: Id<Task>| schedule.assignments.iter().find(|x| x.task == task).unwrap();
    assert_eq!((of(pick).robot, of(pick).start, of(pick).end), (left, 0.0, 0.5));
    assert_eq!((of(fasten).robot, of(fasten).start), (left, 0.5));
    assert!(of(fasten).end - of(fasten).start > 2.0);
    assert_eq!(of(inspect).start, of(fasten).end, "the action left A done");
    // Without the pick the tool is never loaded: fasten is blocked and says why, and so is inspect.
    s.set_suppressed(pick.erase(), true).unwrap();
    let schedule = s.schedule();
    assert!(schedule.assignments.is_empty());
    let why = &schedule.blocked.iter().find(|(id, _)| *id == fasten).unwrap().1;
    assert!(why.contains("fasten"), "{why}");
    s.set_suppressed(pick.erase(), false).unwrap();

    // Renaming an action is followed by the steps; what steps need cannot go; a fitted kit stays.
    let mut renamed = driver.clone();
    renamed.actions[1].name = "drive screw".into();
    s.edit_work(WorkOp::SetToolKit { id: kit, kit: renamed.clone() }).unwrap();
    assert_eq!(
        s.doc().get(fasten).unwrap().steps[1],
        Step::ToolAction { action: "drive screw".into(), target: Some(a) }
    );
    let mut fewer = renamed.clone();
    fewer.actions.pop();
    assert_eq!(s.edit_work(WorkOp::SetToolKit { id: kit, kit: fewer }), Err(EditError::InUse));
    let mut no_signal = renamed.clone();
    no_signal.signals.remove(0);
    no_signal.actions[1].set = None;
    assert_eq!(s.edit_work(WorkOp::SetToolKit { id: kit, kit: no_signal }), Err(EditError::InUse));
    assert_eq!(s.edit_work(WorkOp::DeleteToolKit(kit)), Err(EditError::InUse));
    assert!(s.edit_work(WorkOp::DeleteObjectType(hole)).is_err(), "the action is done to holes");
    // A step that names what no tool has is pointed out.
    let odd: Id<Task> = made(
        &mut s,
        WorkOp::AddTask(task("Odd", vec![Step::ToolAction { action: "weld".into(), target: None }])),
    );
    assert!(s.task_problems(odd)[0].contains("weld"));

    // Saved and read back the same; browser rename works.
    let text = gc_persist::to_text(s.doc()).unwrap();
    assert!(gc_persist::from_text(&text).unwrap().doc.same_content(s.doc()));
    s.execute(&Command::Rename { item: kit.erase(), name: "Driver".into() }).unwrap();
    assert_eq!(s.doc().get(kit).unwrap().name, "Driver");
    // Taken off the robot, the kit can go once no step needs it.
    s.edit_work(WorkOp::SetRobotTool { robot: left, kit: None, state: None }).unwrap();
    assert_eq!(s.robot_tool_kit(left), None);
    assert_eq!(s.edit_work(WorkOp::DeleteToolKit(kit)), Err(EditError::InUse), "steps still need it");
    for id in [fasten, pick, inspect, odd] {
        s.edit_work(WorkOp::DeleteTask(id)).unwrap();
    }
    s.edit_work(WorkOp::DeleteToolKit(kit)).unwrap();
}

/// One robot sets a signal, the other waits for it: the wait holds the second task back until the first
/// has ended (a handshake), and the program waits on the input pin.
#[test]
fn a_wait_for_a_signal_another_robot_sets_is_a_handshake() {
    use gc_doc::signal_condition;
    assert_eq!(signal_condition("ready"), Some(("ready".into(), true)));
    assert_eq!(signal_condition("not ready"), Some(("ready".into(), false)));
    assert_eq!(signal_condition("\"part ready\" == true"), Some(("part ready".into(), true)));
    assert_eq!(signal_condition("false != ready"), Some(("ready".into(), true)));
    assert_eq!(signal_condition("ready == done"), None);
    assert_eq!(signal_condition("door closed"), None);

    let (mut s, [left, right], at_hole) = cell();
    let signal = |name: &str, direction, pin| IoSignal { name: name.into(), direction, pin };
    let kit = |name: &str, signals: Vec<IoSignal>| ToolKit {
        name: name.into(),
        signals,
        states: Vec::new(),
        actions: Vec::new(),
    };
    let feeder: Id<ToolKit> =
        made(&mut s, WorkOp::AddToolKit(kit("Feeder", vec![signal("part ready", IoDirection::Out, 1)])));
    let taker: Id<ToolKit> = made(
        &mut s,
        WorkOp::AddToolKit(kit(
            "Taker",
            vec![signal("part ready", IoDirection::In, 4), signal("clamp closed", IoDirection::In, 5)],
        )),
    );
    s.edit_work(WorkOp::SetRobotTool { robot: left, kit: Some(feeder), state: None }).unwrap();
    s.edit_work(WorkOp::SetRobotTool { robot: right, kit: Some(taker), state: None }).unwrap();

    // Take comes first by priority but waits for the signal Feed sets; the clamp is a device's signal,
    // which nothing in the cell drives, so it does not hold the task back.
    let take = Task {
        priority: 5,
        robots: RobotChoice::Only(vec![right]),
        ..task(
            "Take",
            vec![
                Step::WaitIo { condition: "\"part ready\" == true".into(), timeout: 5.0 },
                Step::WaitIo { condition: "\"clamp closed\"".into(), timeout: 2.0 },
                Step::Wait { seconds: 0.25 },
            ],
        )
    };
    let take: Id<Task> = made(&mut s, WorkOp::AddTask(take));
    let feed: Id<Task> = made(
        &mut s,
        WorkOp::AddTask(task(
            "Feed",
            vec![
                Step::MoveJ(MoveTarget::Role(at_hole)),
                Step::SetIo { name: "part ready".into(), value: true },
            ],
        )),
    );
    assert!(s.task_problems(take).is_empty() && s.task_problems(feed).is_empty());
    assert_eq!(s.doc().driven_signals(), vec!["part ready".to_owned()]);

    let schedule = s.schedule();
    assert!(schedule.blocked.is_empty(), "{:?}", schedule.blocked);
    let of = |task: Id<Task>| schedule.assignments.iter().find(|x| x.task == task).unwrap();
    assert_eq!((of(feed).robot, of(feed).start), (left, 0.0), "only the feeder has the output");
    assert_eq!((of(take).robot, of(take).start), (right, of(feed).end), "after the signal is set");
    // Without the task that sets it, the wait never ends and says which signal.
    s.set_suppressed(feed.erase(), true).unwrap();
    let schedule = s.schedule();
    assert_eq!(schedule.blocked.len(), 1);
    assert!(schedule.blocked[0].1.contains("part ready"), "{}", schedule.blocked[0].1);
    s.set_suppressed(feed.erase(), false).unwrap();

    // The program waits on the robot's own input pins and stops when the signal does not come.
    let text = s.task_urscript(take, right).unwrap();
    assert!(text.contains("while (get_digital_in(4) != True and waited < 5.000):"), "{text}");
    assert!(text.contains("while (get_digital_in(5) != True and waited < 2.000):"));
    assert!(text.contains("popup(\"part_ready: not seen on input 4\", \"GraphCTRL\", False, True)"));
    assert!(text.contains("  halt"));
    // A wait that cannot be read as a signal and a value is pointed out.
    let odd: Id<Task> = made(
        &mut s,
        WorkOp::AddTask(task("Odd", vec![Step::WaitIo { condition: "door closed".into(), timeout: 1.0 }])),
    );
    assert!(s.task_problems(odd)[0].contains("door closed"));
}
