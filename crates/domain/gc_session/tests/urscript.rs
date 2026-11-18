//! URScript programs from planned tasks (plan 9.6): free motions as the planned joint values, straight
//! motions as poses in the robot's base frame, waits, signals and tool actions. Text only; nothing is sent.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use gc_core::Id;
use gc_doc::{
    CellOp, Command, Created, IoDirection, IoSignal, Location, MoveTarget, Placement, Robot, RobotChoice,
    SketchOp, Step, Task, ToolAction, ToolKit, WorkOp,
};
use gc_session::Session;

fn six_axis() -> String {
    let path = format!("{}/../../../fixtures/urdf/six_axis.urdf", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn made<T: gc_core::Entity>(s: &mut Session, op: WorkOp) -> Id<T> {
    s.edit_work(op).unwrap().unwrap().downcast().unwrap()
}

#[test]
fn a_task_is_written_as_a_urscript_program() {
    let mut s = Session::new();
    let robot: Id<Robot> = s.add_robot("UR 1", &six_axis()).unwrap();
    let placement = Placement { xyz: [0.0, -0.6, 0.0], rpy: [0.0; 3] };
    s.execute(&Command::Cell(CellOp::PlaceRobot { id: robot, placement })).unwrap();
    s.jog(robot, &[1.5, -0.3, 0.6, 0.0, 0.6, 0.0]);
    s.end_jog();
    let tcp = Placement { xyz: [0.0, 0.0, 0.1], rpy: [0.0; 3] };
    s.execute(&Command::Cell(CellOp::SetTcp { id: robot, tcp })).unwrap();
    // On the wall: two points.
    let sketch = s.create_sketch("XZ").unwrap();
    let point = |s: &mut Session, x: f64| {
        let created =
            s.execute(&Command::Sketch { sketch, op: SketchOp::AddPoint { pos: [x, 0.5, 0.0] } }).unwrap();
        let Created::Point(point) = created[0] else { panic!() };
        point
    };
    let (a, b) = (point(&mut s, 0.1), point(&mut s, 0.2));
    s.finish_sketch();
    let (at_a, at_b) = (s.add_waypoint_role(sketch, a).unwrap(), s.add_waypoint_role(sketch, b).unwrap());
    // A tool with an output on pin 3 and a confirming input on pin 5.
    let signal = |name: &str, direction, pin| IoSignal { name: name.into(), direction, pin };
    let kit: Id<ToolKit> = made(
        &mut s,
        WorkOp::AddToolKit(ToolKit {
            name: "Driver".into(),
            signals: vec![signal("start", IoDirection::Out, 3), signal("ok", IoDirection::In, 5)],
            states: Vec::new(),
            actions: vec![ToolAction {
                name: "fasten".into(),
                target: None,
                precondition: String::new(),
                set: Some(("start".into(), true)),
                confirm: Some(("ok".into(), true)),
                tool_state: None,
                target_state: None,
                duration: 2.0,
                timeout: 4.0,
            }],
        }),
    );
    s.edit_work(WorkOp::SetRobotTool { robot, kit: Some(kit), state: None }).unwrap();
    let task: Id<Task> = made(
        &mut s,
        WorkOp::AddTask(Task {
            name: "Fasten A-1".into(),
            location: Location::None,
            precondition: String::new(),
            steps: vec![
                Step::MoveJ(MoveTarget::Role(at_a)),
                Step::MoveL(MoveTarget::Role(at_b)),
                Step::ToolAction { action: "fasten".into(), target: None },
                Step::SetIo { name: "start".into(), value: false },
                Step::Wait { seconds: 0.5 },
                Step::WaitIo { condition: "door closed".into(), timeout: 3.0 },
            ],
            effects: Vec::new(),
            robots: RobotChoice::Any,
            priority: 0,
        }),
    );

    let revision = s.doc().revision();
    let text = s.task_urscript(task, robot).unwrap();
    assert_eq!(s.doc().revision(), revision);
    let lines: Vec<&str> = text.lines().collect();
    // A function named after the task, the tool centre point first, then to where the plan starts.
    assert_eq!(lines[0], "def Fasten_A_1():");
    assert_eq!(lines[1], "  set_tcp(p[0.000000, 0.000000, 0.100000, 0.000000, 0.000000, 0.000000])");
    assert_eq!(
        lines[2],
        "  movej([1.500000, -0.300000, 0.600000, 0.000000, 0.600000, 0.000000], a=1.4, v=1.05, r=0)"
    );
    assert_eq!(*lines.last().unwrap(), "end");
    assert!(text.ends_with("end\n"));
    // The joint move ends on the joint values the plan found for the first point.
    let plan = s.task_plan(task, robot).unwrap();
    let reached: Vec<String> =
        plan.segments[0].path.last().unwrap().iter().map(|q| format!("{q:.6}")).collect();
    let joint_end = format!("  movej([{}], a=1.4, v=1.05, r=0)", reached.join(", "));
    assert!(lines.contains(&joint_end.as_str()), "{text}");
    // The straight move goes to the second point as a pose in the robot's base frame.
    let in_base = s.robot_base_frame(robot).unwrap().inverse() * s.role_pose(at_b).unwrap();
    let position = format!(
        "movel(p[{:.6}, {:.6}, {:.6},",
        in_base.translation.x, in_base.translation.y, in_base.translation.z
    );
    let straight = lines.iter().find(|l| l.contains("movel(")).unwrap();
    assert!(straight.contains(&position) && straight.ends_with("a=1.2, v=0.25, r=0)"), "{straight}");
    // In order after it: the action sets pin 3 and waits for pin 5, the signal goes off, a wait, a note.
    let after = |needle: &str| {
        lines.iter().position(|l| l.contains(needle)).unwrap_or_else(|| panic!("{needle}\n{text}"))
    };
    let order = [
        after("movel("),
        after("# tool action: fasten"),
        after("set_digital_out(3, True)"),
        after("while (get_digital_in(5) != True and waited < 4):"),
        after("set_digital_out(3, False)"),
        after("sleep(0.5)"),
        after("# TODO wait for: door closed"),
    ];
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{order:?}\n{text}");

    // A schedule gives each robot with something to do one program.
    let programs = s.schedule_urscript(&s.schedule());
    assert_eq!(programs.len(), 1);
    assert_eq!(programs[0].0, robot);
    assert!(programs[0].1.starts_with("def UR_1():\n") && programs[0].1.contains("  # Fasten A-1\n"));
    // A robot that cannot do the task gives the reason instead of a program.
    s.edit_work(WorkOp::SetRobotTool { robot, kit: None, state: None }).unwrap();
    assert!(s.task_urscript(task, robot).unwrap_err().contains("fasten"));
}
