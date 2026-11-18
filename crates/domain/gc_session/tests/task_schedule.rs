//! Tasks planned for robots and a minimal schedule (plan 9.4 and 9.5, GraphCTRL's own): a task's steps
//! become motions of whichever robot is asked, with its tool and where the sketch is now; the bids order
//! the robots; the schedule gives ready tasks to the idle robot that finishes soonest and lets effects
//! make other tasks ready.

#![allow(clippy::unwrap_used, clippy::panic, clippy::float_cmp, reason = "test code")]

use gc_core::Id;
use gc_doc::{
    CellOp, Command, Created, Effect, EffectTarget, Location, MoveTarget, ObjectType, Placement, Robot,
    RobotChoice, Sketch, SketchOp, SketchRole, Step, Task, WaypointTarget, WorkObject, WorkOp,
};
use gc_math::DVec3;
use gc_session::Session;
use gc_sketch::SkPoint;

fn six_axis() -> String {
    let path = format!("{}/../../../fixtures/urdf/six_axis.urdf", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

struct Cell {
    s: Session,
    left: Id<Robot>,
    right: Id<Robot>,
    sketch: Id<Sketch>,
    points: [Id<SkPoint>; 2],
    roles: [Id<SketchRole>; 2],
}

/// Two robots side by side, 0.6 m in front of the XZ wall, 0.7 m apart, and on the wall a waypoint in
/// front of each.
fn cell() -> Cell {
    let mut s = Session::new();
    let robot = |s: &mut Session, name: &str, x: f64| {
        let id = s.add_robot(name, &six_axis()).unwrap();
        let placement = Placement { xyz: [x, -0.6, 0.0], rpy: [0.0; 3] };
        s.execute(&Command::Cell(CellOp::PlaceRobot { id, placement })).unwrap();
        s.jog(id, &[1.5, -0.3, 0.6, 0.0, 0.6, 0.0]);
        s.end_jog();
        id
    };
    let (left, right) = (robot(&mut s, "Left", 0.0), robot(&mut s, "Right", 0.7));
    let sketch = s.create_sketch("XZ").unwrap();
    let point = |s: &mut Session, x: f64| {
        let created =
            s.execute(&Command::Sketch { sketch, op: SketchOp::AddPoint { pos: [x, 0.5, 0.0] } }).unwrap();
        let Created::Point(point) = created[0] else { panic!() };
        point
    };
    let points = [point(&mut s, 0.1), point(&mut s, 0.8)];
    s.finish_sketch();
    let roles = points.map(|p| s.add_waypoint_role(sketch, p).unwrap());
    Cell { s, left, right, sketch, points, roles }
}

fn task(name: &str, at: Id<SketchRole>) -> Task {
    Task {
        name: name.into(),
        location: Location::None,
        precondition: String::new(),
        steps: vec![Step::MoveJ(MoveTarget::Role(at))],
        effects: Vec::new(),
        robots: RobotChoice::Any,
        priority: 0,
    }
}

fn add(s: &mut Session, task: Task) -> Id<Task> {
    s.edit_work(WorkOp::AddTask(task)).unwrap().unwrap().downcast().unwrap()
}

fn near(a: [f64; 3], b: [f64; 3], tolerance: f64) -> bool {
    (DVec3::from_array(a) - DVec3::from_array(b)).length() < tolerance
}

#[test]
fn a_task_is_planned_for_the_robot_asked_with_its_tool_and_the_sketch_as_it_is() {
    let Cell { mut s, left, right, sketch, points, roles } = cell();
    let mut work = task("Touch", roles[0]);
    work.steps.push(Step::Wait { seconds: 1.5 });
    work.steps.push(Step::SetIo { name: "valve".into(), value: true });
    let touch = add(&mut s, work);

    // The tool centre point ends on the sketch point; the waits are part of the time; nothing is edited.
    let revision = s.doc().revision();
    let plan = s.task_plan(touch, left).unwrap();
    assert_eq!(s.doc().revision(), revision);
    assert_eq!(plan.segments.len(), 1);
    assert!(near(*plan.segments[0].tip_path.last().unwrap(), [0.1, 0.0, 0.5], 1e-4));
    assert!((plan.duration() - plan.segments[0].trajectory.duration() - 1.5).abs() < 1e-12);
    assert_eq!(&plan.end, plan.segments[0].path.last().unwrap());

    // With a 100 mm tool the tool centre point is still on the point: the flange ends 100 mm short.
    let tcp = Placement { xyz: [0.0, 0.0, 0.1], rpy: [0.0; 3] };
    s.execute(&Command::Cell(CellOp::SetTcp { id: left, tcp })).unwrap();
    let with_tool = s.task_plan(touch, left).unwrap();
    assert!(near(*with_tool.segments[0].tip_path.last().unwrap(), [0.1, 0.0, 0.5], 1e-4));
    assert_ne!(with_tool.end, plan.end, "the arm stands differently behind the tool");
    s.jog(left, &with_tool.end);
    let flange = s.link_poses(left).unwrap().last().unwrap().translation;
    assert!((flange.y + 0.1).abs() < 1e-3, "the flange is 100 mm in front of the wall: {flange:?}");
    s.end_jog();
    assert!(s.undo(), "the jog");

    // The sketch moves (a dimension, the component it is on): the plan goes where the point is now.
    let moved = SketchOp::SetCoords { points: vec![(points[0], [-0.1, 0.6, 0.0])], radii: Vec::new() };
    s.execute(&Command::Sketch { sketch, op: moved }).unwrap();
    let after = s.task_plan(touch, left).unwrap();
    assert!(near(*after.segments[0].tip_path.last().unwrap(), [-0.1, 0.0, 0.6], 1e-4));
    assert!(s.undo());

    // The bids: both robots may do it; each is planned with the other standing where it is, so the right
    // one cannot get to the point in front of the left one. Those that can come first, the faster first.
    let candidates = s.task_candidates(touch);
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].robot, left);
    assert_eq!(candidates[0].time, Ok(s.task_plan(touch, left).unwrap().duration()));
    let why = candidates[1].time.as_ref().unwrap_err();
    assert!(why.contains("Left"), "{why}");
    let again = s.task_plan_from(touch, left, &with_tool.end).unwrap();
    assert!(again.duration() < 1.5 + 0.05, "{}", again.duration());

    // Who may: only the named robots; the robot itself for its own task, with its own waypoints.
    let mut named = task("Only right", roles[1]);
    named.robots = RobotChoice::Only(vec![right]);
    let named = add(&mut s, named);
    assert_eq!(s.task_robots(named), vec![right]);
    assert!(s.task_plan(named, left).unwrap_err().contains("not for"));
    let home = s.add_waypoint(right, WaypointTarget::Joints(vec![0.0; 6])).unwrap();
    let own = Task {
        location: Location::Robot(right),
        steps: vec![Step::MoveJ(MoveTarget::Waypoint(home))],
        ..task("Home", roles[0])
    };
    let own = add(&mut s, own);
    assert_eq!(s.task_robots(own), vec![right]);
    assert_eq!(s.task_plan(own, right).unwrap().end, vec![0.0; 6]);
    assert!(s.task_plan(own, left).is_err());
    // A point no robot reaches: the reason says which step.
    let far = SketchOp::SetCoords { points: vec![(points[1], [5.0, 0.5, 0.0])], radii: Vec::new() };
    s.execute(&Command::Sketch { sketch, op: far }).unwrap();
    let why = s.task_plan(named, right).unwrap_err();
    assert!(why.starts_with("W2"), "{why}");
    assert!(s.task_candidates(named)[0].time.is_err());
}

#[test]
fn ready_tasks_go_to_the_idle_robot_that_finishes_soonest_and_effects_ready_the_next() {
    let Cell { mut s, left, right, sketch, roles, .. } = cell();
    let states = vec!["empty".to_owned(), "done".to_owned()];
    let hole =
        s.edit_work(WorkOp::AddObjectType(ObjectType { name: "Hole".into(), states })).unwrap().unwrap();
    let hole: Id<ObjectType> = hole.downcast().unwrap();
    let object = |s: &mut Session, name: &str| -> Id<WorkObject> {
        let object = WorkObject {
            name: name.into(),
            ty: hole,
            location: Location::Sketch(sketch),
            state: "empty".into(),
        };
        s.edit_work(WorkOp::AddObject(object)).unwrap().unwrap().downcast().unwrap()
    };
    let (a, _b) = (object(&mut s, "A"), object(&mut s, "B"));

    // Inspect A needs A done; Fasten A makes it so; Touch right is free to go at once; Never waits for B.
    let inspect = add(&mut s, Task { precondition: "A.state == done".into(), ..task("Inspect A", roles[0]) });
    let fasten = add(
        &mut s,
        Task {
            precondition: "A.state == empty".into(),
            effects: vec![Effect { target: EffectTarget::Object(a), value: "done".into() }],
            steps: vec![Step::MoveJ(MoveTarget::Role(roles[0])), Step::Wait { seconds: 2.0 }],
            priority: 5,
            ..task("Fasten A", roles[0])
        },
    );
    let touch = add(&mut s, task("Touch right", roles[1]));
    let never = add(&mut s, Task { precondition: "B.state == done".into(), ..task("Never", roles[0]) });

    let revision = s.doc().revision();
    let schedule = s.schedule();
    // Whichever robot bid lower got the first task (each bid is planned with the other robot standing
    // where it is).
    let first = schedule.assignments[0].robot;
    let other = if first == left { right } else { left };
    assert_eq!(s.doc().revision(), revision, "nothing is run or edited");
    assert_eq!(schedule, s.schedule(), "the same every time");
    let of = |task: Id<Task>| schedule.assignments.iter().find(|x| x.task == task).unwrap();

    // The higher priority goes first and to the robot with the lower bid; the other task starts at the
    // same moment on the other robot.
    assert_eq!(schedule.assignments[0].task, fasten);
    assert_eq!((of(fasten).robot, of(fasten).start), (first, 0.0));
    assert_eq!((of(touch).robot, of(touch).start), (other, 0.0));
    assert!(of(fasten).end - of(fasten).start > 2.0, "the wait is part of it");
    // Inspect starts when Fasten ends (its effect), with a robot that is idle then.
    assert_eq!(of(inspect).start, of(fasten).end);
    assert!(of(inspect).end >= of(inspect).start);
    assert_eq!(schedule.makespan(), schedule.assignments.iter().map(|x| x.end).fold(0.0, f64::max));
    // What is never done says why.
    assert_eq!(schedule.blocked.len(), 1);
    assert_eq!(schedule.blocked[0].0, never);
    assert!(schedule.blocked[0].1.contains("precondition"), "{}", schedule.blocked[0].1);

    // A suppressed task is left out; a task no robot can reach is blocked with the robot's reason.
    s.set_suppressed(touch.erase(), true).unwrap();
    assert!(s.schedule().assignments.iter().all(|x| x.task != touch));
}

#[test]
fn a_schedule_asked_for_in_the_background_arrives_for_the_document_it_was_asked_for() {
    let Cell { mut s, roles, .. } = cell();
    let (executor, queue) = gc_jobs::Executor::manual();
    s.set_executor(executor);
    let first = add(&mut s, task("First", roles[0]));
    assert!(s.schedule_result().is_none() && !s.schedule_pending());

    // Nothing is worked out until the job runs; asking again for the same document starts nothing new.
    s.request_schedule();
    s.request_schedule();
    assert!(s.schedule_pending() && s.schedule_result().is_none() && !s.poll_jobs());
    queue.run_all();
    assert!(s.poll_jobs());
    assert!(!s.schedule_pending());
    let (schedule, current) = s.schedule_result().unwrap();
    assert!(current);
    assert_eq!(schedule, &s.schedule(), "the same as working it out here");
    assert_eq!(schedule.assignments.len(), 1);

    // An edit makes it out of date, not wrong; asked again, the new one replaces it.
    let second = add(&mut s, task("Second", roles[1]));
    let (old, current) = s.schedule_result().unwrap();
    assert!(!current && old.assignments.iter().all(|a| a.task == first));
    s.request_schedule();
    // An edit while it is being worked out: the result is for the document that was asked about.
    s.set_suppressed(second.erase(), true).unwrap();
    queue.run_all();
    assert!(s.poll_jobs());
    let (late, current) = s.schedule_result().unwrap();
    assert!(!current && late.assignments.iter().any(|a| a.task == second));
    // Asked again before the old job ran: the old one is cancelled and only the new result arrives.
    s.request_schedule();
    s.set_suppressed(second.erase(), false).unwrap();
    s.request_schedule();
    queue.run_all();
    assert!(s.poll_jobs());
    let (schedule, current) = s.schedule_result().unwrap();
    assert!(current && schedule.assignments.len() == 2);
}

#[test]
fn a_schedule_is_played_back_with_every_robot_where_it_is_at_that_moment() {
    let Cell { mut s, left, right, roles, .. } = cell();
    let mut first = task("First", roles[0]);
    first.steps.insert(0, Step::Wait { seconds: 1.0 });
    first.robots = RobotChoice::Only(vec![left]);
    let mut second = task("Second", roles[1]);
    second.robots = RobotChoice::Only(vec![right]);
    add(&mut s, first);
    add(&mut s, second);
    let home = s.doc().get(left).unwrap().joints.clone();

    // Nothing to play back before a schedule was asked for.
    s.set_schedule_playback(Some(0.5));
    assert_eq!(s.schedule_playback(), None);
    s.request_schedule();
    let (schedule, _) = s.schedule_result().unwrap();
    let schedule = schedule.clone();
    assert_eq!(schedule.plans.len(), schedule.assignments.len());
    let of = |robot: Id<Robot>| {
        schedule.assignments.iter().zip(&schedule.plans).find(|(a, _)| a.robot == robot).unwrap()
    };
    let (left_task, left_plan) = of(left);
    let (_, right_plan) = of(right);

    // The left robot waits a second where it stands, then moves; the right one moves at once.
    assert_eq!(schedule.joints_at(left, 0.5), Some(home.clone()));
    assert_ne!(schedule.joints_at(right, 0.5), Some(right_plan.start.clone()));
    assert_eq!(schedule.joints_at(left, left_task.end + 10.0), Some(left_plan.end.clone()), "it stays there");
    assert_eq!(schedule.robots_at(0.0).len(), 2);
    // The session shows them so without changing the document, and goes back.
    let revision = s.doc().revision();
    let middle = 1.0 + (left_task.end - 1.0) / 2.0;
    s.set_schedule_playback(Some(middle));
    let shown = s.robot_joints(left).unwrap();
    assert!(shown != home && shown != left_plan.end, "on its way");
    assert_eq!(shown, left_plan.sample(middle));
    assert_eq!(s.robot_joints(right), schedule.joints_at(right, middle));
    assert_eq!(s.doc().revision(), revision);
    s.set_schedule_playback(None);
    assert_eq!(s.robot_joints(left), Some(home));
}

#[test]
fn robots_keep_clear_of_each_other_and_the_one_in_the_way_is_waited_for() {
    let Cell { mut s, left, right, sketch, points, roles } = cell();
    // Both robots are to touch the same point, between them.
    let between = SketchOp::SetCoords { points: vec![(points[0], [0.35, 0.5, 0.0])], radii: Vec::new() };
    s.execute(&Command::Sketch { sketch, op: between }).unwrap();
    let only = |name: &str, robot: Id<Robot>, priority: i32| Task {
        robots: RobotChoice::Only(vec![robot]),
        priority,
        ..task(name, roles[0])
    };
    let first = add(&mut s, only("Left touches", left, 2));
    let second = add(&mut s, only("Right touches", right, 1));
    // On their own, each can.
    assert!(s.task_plan(first, left).is_ok() && s.task_plan(second, right).is_ok());

    // Left goes first and then stands on the point with nothing to do: it is sent back to where it
    // stood (its park pose by default), and right goes once it is out of the way.
    let schedule = s.schedule();
    assert!(schedule.blocked.is_empty(), "{:?}", schedule.blocked);
    assert_eq!((schedule.assignments[0].task, schedule.assignments[0].robot), (first, left));
    assert_eq!(schedule.parks.len(), 1);
    let park = &schedule.parks[0];
    assert_eq!((park.robot, park.start), (left, schedule.assignments[0].end));
    assert_eq!(park.plan.end, s.doc().get(left).unwrap().joints);
    let right_goes = schedule.assignments.iter().find(|x| x.task == second).unwrap();
    assert!(right_goes.start >= park.start && right_goes.robot == right);
    assert_eq!(schedule.joints_at(left, park.end + 1.0), Some(park.plan.end.clone()));
    assert!(schedule.makespan() >= park.end);
    // A park pose of its own: saved, one undo step, the right number of joints.
    use gc_doc::EditError;
    assert_eq!(
        s.edit_work(WorkOp::SetRobotPark { robot: left, joints: Some(vec![0.0; 2]) }),
        Err(EditError::BadValue)
    );
    let elsewhere = vec![1.2, -0.3, 0.6, 0.0, 0.6, 0.0];
    let depth = s.history().undo_depth();
    s.edit_work(WorkOp::SetRobotPark { robot: left, joints: Some(elsewhere.clone()) }).unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1);
    assert_eq!(s.doc().robot_park(left), Some(elsewhere.clone()));
    assert_eq!(s.schedule().parks[0].plan.end, elsewhere);
    s.edit_work(WorkOp::SetRobotPark { robot: left, joints: None }).unwrap();

    // With a task that takes left back to where it started, right waits for it and then goes.
    let home = s.add_waypoint_here(left).unwrap();
    let away = add(
        &mut s,
        Task {
            location: Location::Robot(left),
            steps: vec![Step::MoveJ(MoveTarget::Waypoint(home))],
            ..task("Left away", roles[0])
        },
    );
    let schedule = s.schedule();
    assert!(schedule.blocked.is_empty(), "{:?}", schedule.blocked);
    let of = |task: Id<Task>| schedule.assignments.iter().find(|x| x.task == task).unwrap();
    assert_eq!((of(first).start, of(away).start), (0.0, of(first).end));
    assert_eq!(of(second).start, of(away).end, "right starts when left is out of the way");
    // The wait is on record: right was ready from the start and waited for left.
    assert_eq!(schedule.waits.len(), 1);
    let wait = &schedule.waits[0];
    assert_eq!((wait.task, wait.robot, wait.cause), (second, right, left));
    assert_eq!(wait.seconds, of(second).start);
    assert_eq!(of(second).robot, right);
}

#[test]
fn a_task_repeats_while_its_condition_holds_and_counts_with_an_expression() {
    use gc_doc::{EditError, StateValue, StateVar};
    let Cell { mut s, roles, .. } = cell();
    let count = s
        .edit_work(WorkOp::AddStateVar(StateVar { name: "count".into(), value: StateValue::Int(0) }))
        .unwrap()
        .unwrap()
        .downcast()
        .unwrap();
    let counting = |value: &str| Task {
        precondition: "count < 3".into(),
        effects: vec![Effect { target: EffectTarget::Var(count), value: value.into() }],
        ..task("Count", roles[0])
    };
    // The value is an expression over the numeric states; mistakes are refused with their place.
    assert!(matches!(
        s.edit_work(WorkOp::AddTask(counting("count +"))),
        Err(EditError::Expr { start: 7, .. })
    ));
    assert!(matches!(
        s.edit_work(WorkOp::AddTask(counting("apples + 1"))),
        Err(EditError::Expr { start: 0, .. })
    ));
    let id = add(&mut s, counting("count + 1"));

    // Done once it stops there: the condition still holds but the task is not repeated.
    assert_eq!(s.schedule().assignments.len(), 1);
    // Repeated: three times, until the count it raises makes its condition false. One undo step; saved.
    let depth = s.history().undo_depth();
    s.edit_work(WorkOp::SetTaskRepeat { task: id, times: Some(0) }).unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1);
    assert_eq!(s.doc().task_repeat(id), Some(0));
    let schedule = s.schedule();
    assert_eq!(schedule.assignments.len(), 3);
    assert!(schedule.blocked.is_empty() && !schedule.cut_short);
    assert!(schedule.assignments.windows(2).all(|w| w[0].end <= w[1].start), "one after the other");
    // At most twice.
    s.edit_work(WorkOp::SetTaskRepeat { task: id, times: Some(2) }).unwrap();
    assert_eq!(s.schedule().assignments.len(), 2);
    let text = gc_persist::to_text(s.doc()).unwrap();
    assert_eq!(gc_persist::from_text(&text).unwrap().doc.task_repeat(id), Some(2));
    // Renaming the variable is followed by the expression.
    s.execute(&Command::Rename { item: gc_core::Id::<StateVar>::erase(count), name: "n".into() }).unwrap();
    let stored = s.doc().get(id).unwrap();
    assert_eq!((stored.precondition.as_str(), stored.effects[0].value.as_str()), ("n < 3", "n + 1"));
    assert_eq!(s.schedule().assignments.len(), 2);

    // A task that repeats without end is cut off.
    let forever = add(&mut s, Task { steps: vec![Step::Wait { seconds: 1.0 }], ..task("Forever", roles[0]) });
    s.edit_work(WorkOp::SetTaskRepeat { task: forever, times: Some(0) }).unwrap();
    let schedule = s.schedule();
    assert!(schedule.cut_short);
    assert_eq!(schedule.assignments.len(), gc_session::MAX_ASSIGNMENTS);
}
