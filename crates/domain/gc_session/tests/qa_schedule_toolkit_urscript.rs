//! QA boundary tests for the schedule (determinism, repeat cap, waits and parking, playback), tool kinds
//! (pins, `tool.state` conditions, renames) and URScript text (identifiers, injection, numbers, empty tasks)
//! (plan 9.4–9.6, GraphCTRL's own; no Inventor counterpart). Refused edits leave the document and the history
//! untouched; accepted ones are one undo step that restores exactly (redo repeats).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::disallowed_methods,
    reason = "test code"
)]

use gc_core::Id;
use gc_doc::{
    CellOp, Command, Created, EditError, Effect, EffectTarget, IoDirection, IoSignal, Location, MoveTarget,
    ObjectType, Placement, Robot, RobotChoice, Sketch, SketchOp, SketchRole, StateValue, StateVar, Step,
    Task, ToolAction, ToolKit, WorkObject, WorkOp,
};
use gc_session::Session;
use gc_sketch::SkPoint;

fn six_axis() -> String {
    let path = format!("{}/../../../fixtures/urdf/six_axis.urdf", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap()
}

struct Cell {
    s: Session,
    left: Id<Robot>,
    right: Id<Robot>,
    sketch: Id<Sketch>,
    points: [Id<SkPoint>; 2],
    roles: [Id<SketchRole>; 2],
}

/// Two robots side by side in front of the XZ wall, a waypoint role in front of each.
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

/// Refused with no trace, or one undo step that undoes exactly and redoes the same (the edit stays applied).
fn step<T>(s: &mut Session, what: &str, f: impl FnOnce(&mut Session) -> Result<T, EditError>) -> Option<T> {
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    match f(s) {
        Err(_) => {
            assert!(s.doc().same_content(&before), "{what} changed the document");
            assert_eq!(s.history().undo_depth(), depth, "{what}");
            None
        }
        Ok(v) => {
            assert_eq!(s.history().undo_depth(), depth + 1, "{what}: one undo step");
            let after = s.doc().clone();
            assert!(s.undo());
            assert!(s.doc().same_content(&before), "{what}: undo restores");
            assert!(s.redo());
            assert!(s.doc().same_content(&after), "{what}: redo repeats");
            Some(v)
        }
    }
}

fn same(a: &gc_session::Schedule, b: &gc_session::Schedule) -> bool {
    a.assignments == b.assignments
        && a.blocked == b.blocked
        && a.waits == b.waits
        && a.cut_short == b.cut_short
        && a.parks.len() == b.parks.len()
        && a.parks
            .iter()
            .zip(&b.parks)
            .all(|(p, q)| p.robot == q.robot && p.start == q.start && p.end == q.end)
}

// ---- Schedule ----

#[test]
fn a_schedule_is_deterministic_and_the_same_after_save_and_open() {
    let Cell { mut s, left, right, sketch, roles, .. } = cell();
    // Nothing to schedule: empty, nothing blocked, makespan zero.
    let empty = Session::new().schedule();
    assert!(empty.assignments.is_empty() && empty.blocked.is_empty() && empty.makespan() == 0.0);
    let ty = s
        .edit_work(WorkOp::AddObjectType(ObjectType {
            name: "Hole".into(),
            states: vec!["empty".into(), "done".into()],
        }))
        .unwrap()
        .unwrap()
        .downcast()
        .unwrap();
    let hole = |s: &mut Session, name: &str| -> Id<WorkObject> {
        s.edit_work(WorkOp::AddObject(WorkObject {
            name: name.into(),
            ty,
            location: Location::Sketch(sketch),
            state: "empty".into(),
        }))
        .unwrap()
        .unwrap()
        .downcast()
        .unwrap()
    };
    let (a, b) = (hole(&mut s, "A"), hole(&mut s, "B"));
    // Ties in priority and in time, preconditions that chain, a task for one robot, one nobody can do.
    for (i, name) in ["T1", "T2", "T3", "T4"].into_iter().enumerate() {
        add(&mut s, Task { priority: i32::from(i % 2 == 0), ..task(name, roles[i % 2]) });
    }
    add(
        &mut s,
        Task {
            precondition: "A.state == empty".into(),
            effects: vec![Effect { target: EffectTarget::Object(a), value: "done".into() }],
            ..task("FastenA", roles[0])
        },
    );
    add(
        &mut s,
        Task {
            precondition: "A.state == done".into(),
            effects: vec![Effect { target: EffectTarget::Object(b), value: "done".into() }],
            ..task("After A", roles[1])
        },
    );
    add(&mut s, Task { robots: RobotChoice::Only(vec![right]), ..task("Right only", roles[1]) });
    add(
        &mut s,
        Task { precondition: "B.state == empty and A.state == empty".into(), ..task("Never", roles[0]) },
    );
    let first = s.schedule();
    for _ in 0..3 {
        assert!(same(&first, &s.schedule()), "the schedule changed between runs");
    }
    // A copy of the document (saved and opened) schedules the same way, with the same task and robot per slot.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sched.gcp");
    s.save_as(&path).unwrap();
    let mut t = Session::new();
    // The robots are loaded from the document, not the original files: opening needs none of ours.
    t.open(&path).unwrap();
    let reopened = t.schedule();
    assert_eq!(first.assignments.len(), reopened.assignments.len());
    for (x, y) in first.assignments.iter().zip(&reopened.assignments) {
        assert_eq!((x.task, x.robot), (y.task, y.robot));
        assert!((x.start - y.start).abs() < 1e-6 && (x.end - y.end).abs() < 1e-6, "{x:?} vs {y:?}");
    }
    assert_eq!(first.blocked.len(), reopened.blocked.len());
    // Every assignment is sane: ends after it starts, in order of start, no robot does two at once.
    assert!(first.assignments.windows(2).all(|w| w[0].start <= w[1].start));
    for x in &first.assignments {
        assert!(x.start.is_finite() && x.end.is_finite() && x.end >= x.start, "{x:?}");
        for y in &first.assignments {
            if x.task != y.task && x.robot == y.robot {
                assert!(x.end <= y.start + 1e-9 || y.end <= x.start + 1e-9, "{x:?} overlaps {y:?}");
            }
        }
    }
    // The tie-breaking: of equal priority, the task made first starts first.
    let order: Vec<Id<Task>> = first.assignments.iter().map(|a| a.task).collect();
    let pos = |name: &str| order.iter().position(|t| s.doc().get(*t).unwrap().name == name);
    assert!(pos("T1") < pos("T3") || pos("T1").is_none(), "equal priority, made first goes first");
    let _ = left;
}

#[test]
fn no_robots_or_no_robot_that_can_do_a_task_blocks_it_with_a_reason() {
    let mut s = Session::new();
    // Tasks with no robot in the document and no role to go to.
    add(
        &mut s,
        Task {
            steps: vec![Step::Wait { seconds: 1.0 }],
            ..task("Wait only", Id::from_raw(std::num::NonZeroU64::new(1).unwrap()))
        },
    );
    let schedule = s.schedule();
    assert!(schedule.assignments.is_empty(), "a task with no robot was assigned");
    assert!(!schedule.blocked.is_empty() && schedule.blocked.iter().all(|(_, why)| !why.trim().is_empty()));
    // Robots cannot be suppressed (BadValue); a task suppressed is left out, and one nobody may do is blocked.
    let Cell { mut s, left, right, roles, .. } = cell();
    let t = add(&mut s, Task { robots: RobotChoice::Only(vec![left]), ..task("Left only", roles[0]) });
    let other =
        add(&mut s, Task { robots: RobotChoice::When("false == true".into()), ..task("Nobody", roles[1]) });
    let _ = right;
    let schedule = s.schedule();
    assert!(schedule.assignments.iter().any(|a| a.task == t));
    assert!(
        schedule.blocked.iter().any(|(id, why)| *id == other && !why.is_empty()),
        "{:?}",
        schedule.blocked
    );
    s.set_suppressed(t.erase(), true).unwrap();
    assert!(s.schedule().assignments.iter().all(|a| a.task != t));
}

#[test]
fn repeats_are_capped_exact_and_a_numeric_effect_cannot_crash() {
    let Cell { mut s, roles, .. } = cell();
    let count: Id<StateVar> = s
        .edit_work(WorkOp::AddStateVar(StateVar { name: "n".into(), value: StateValue::Int(1) }))
        .unwrap()
        .unwrap()
        .downcast()
        .unwrap();
    let t = add(
        &mut s,
        Task {
            precondition: "n < 1000".into(),
            effects: vec![Effect { target: EffectTarget::Var(count), value: "n + 1".into() }],
            steps: vec![Step::Wait { seconds: 0.5 }],
            ..task("Count", roles[0])
        },
    );
    // `Some(n)` up to n times; `Some(0)` as long as its condition holds; ghost task refused.
    let ghost = Id::<Task>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    assert!(
        step(&mut s, "ghost repeat", |s| s.edit_work(WorkOp::SetTaskRepeat { task: ghost, times: Some(2) }))
            .is_none()
    );
    for (times, expect) in [(None, 1usize), (Some(1), 1), (Some(5), 5), (Some(0), 999)] {
        s.edit_work(WorkOp::SetTaskRepeat { task: t, times }).unwrap();
        let schedule = s.schedule();
        assert_eq!(schedule.assignments.len(), expect, "{times:?}");
        assert!(!schedule.cut_short, "{times:?}");
        assert!(
            schedule.assignments.windows(2).all(|w| w[0].end <= w[1].start + 1e-9),
            "one after the other"
        );
    }
    // u32::MAX repeats of a task with no condition: cut off at the cap, deterministically, quickly.
    let forever = add(&mut s, Task { steps: vec![Step::Wait { seconds: 0.1 }], ..task("Forever", roles[1]) });
    s.edit_work(WorkOp::SetTaskRepeat { task: forever, times: Some(u32::MAX) }).unwrap();
    let started = std::time::Instant::now();
    let first = s.schedule();
    let took = started.elapsed();
    assert!(first.cut_short && first.assignments.len() == gc_session::MAX_ASSIGNMENTS);
    assert!(same(&first, &s.schedule()));
    eprintln!("report: a schedule cut at {} assignments took {took:?}", gc_session::MAX_ASSIGNMENTS);
    s.edit_work(WorkOp::SetTaskRepeat { task: forever, times: None }).unwrap();
    // A numeric effect that overflows (doubling an integer 63+ times), one that divides by zero, and a huge
    // real: the schedule is made (or the edit refused), never a panic.
    let big: Id<StateVar> = s
        .edit_work(WorkOp::AddStateVar(StateVar { name: "big".into(), value: StateValue::Int(1) }))
        .unwrap()
        .unwrap()
        .downcast()
        .unwrap();
    let real: Id<StateVar> = s
        .edit_work(WorkOp::AddStateVar(StateVar { name: "real".into(), value: StateValue::Real(1.0) }))
        .unwrap()
        .unwrap()
        .downcast()
        .unwrap();
    for (what, target, value) in [
        ("doubling", big, "big * 2"),
        ("squaring", big, "big * big"),
        ("divide by zero", big, "big / 0"),
        ("modulo zero", big, "big % 0"),
        ("negative power", big, "0 - big * 1000000000000"),
        ("real overflow", real, "real * 1e300"),
        ("nan", real, "0.0 / 0.0"),
    ] {
        let looping = Task {
            effects: vec![Effect { target: EffectTarget::Var(target), value: value.into() }],
            steps: vec![Step::Wait { seconds: 0.1 }],
            ..task(what, roles[0])
        };
        match s.edit_work(WorkOp::AddTask(looping)) {
            Err(_) => {}
            Ok(id) => {
                let id: Id<Task> = id.unwrap().downcast().unwrap();
                s.edit_work(WorkOp::SetTaskRepeat { task: id, times: Some(100) }).unwrap();
                let schedule = s.schedule();
                assert!(
                    schedule.assignments.iter().all(|a| a.start.is_finite() && a.end.is_finite()),
                    "{what}"
                );
                s.edit_work(WorkOp::DeleteTask(id)).unwrap();
            }
        }
    }
}

#[test]
fn playback_joints_are_finite_continuous_and_bounded_in_time() {
    let Cell { mut s, left, right, roles, .. } = cell();
    let mut first = task("First", roles[0]);
    first.steps.insert(0, Step::Wait { seconds: 1.0 });
    first.robots = RobotChoice::Only(vec![left]);
    add(&mut s, first);
    add(&mut s, Task { robots: RobotChoice::Only(vec![right]), ..task("Second", roles[1]) });
    add(&mut s, Task { robots: RobotChoice::Only(vec![left]), ..task("Third", roles[1]) });
    let schedule = s.schedule();
    assert!(schedule.assignments.len() >= 2, "{:?}", schedule.blocked);
    let end = schedule.makespan();
    // Sample every 10 ms and past both ends: finite, and no robot jumps more than a joint can in 10 ms.
    for robot in [left, right] {
        let mut previous: Option<Vec<f64>> = None;
        let mut t = 0.0;
        while t <= end + 1.0 {
            if let Some(q) = schedule.joints_at(robot, t) {
                assert!(q.iter().all(|v| v.is_finite()), "{robot:?} at {t}: {q:?}");
                if let Some(p) = &previous {
                    let jump = q.iter().zip(p).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
                    assert!(jump < 0.2, "{robot:?} jumped {jump} rad in 10 ms at {t}");
                }
                previous = Some(q);
            }
            t += 0.01;
        }
    }
    // Before any task nothing is said; NaN, negative and infinite times are answered, never a panic.
    for t in [-1.0, f64::NAN, f64::NEG_INFINITY] {
        assert!(schedule.joints_at(left, t).is_none() || t == f64::INFINITY, "{t}");
    }
    let late = schedule.joints_at(left, f64::INFINITY);
    assert!(late.is_none_or(|q| q.iter().all(|v| v.is_finite())));
    assert!(schedule.robots_at(f64::NAN).iter().all(|(_, q)| q.iter().all(|v| v.is_finite())));
    let ghost = Id::<Robot>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    assert!(schedule.joints_at(ghost, 1.0).is_none());
    // The session's playback: ignored with no schedule asked for, NaN/infinite ignored, negative shows the
    // robots as the document has them; the document is never changed.
    let revision = s.doc().revision();
    s.set_schedule_playback(Some(0.5));
    assert_eq!(s.schedule_playback(), None, "no schedule yet");
    s.request_schedule();
    for t in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        s.set_schedule_playback(Some(t));
        assert_eq!(s.schedule_playback(), None, "{t} was taken");
    }
    for t in [-5.0, 0.0, end / 2.0, end, end * 10.0] {
        s.set_schedule_playback(Some(t));
        for robot in [left, right] {
            assert!(s.robot_joints(robot).is_none_or(|q| q.iter().all(|v| v.is_finite())), "{t}");
        }
    }
    s.set_schedule_playback(None);
    assert_eq!(s.doc().revision(), revision, "playback changed the document");
}

#[test]
fn two_robots_for_one_point_wait_or_get_out_of_the_way_and_never_overlap() {
    let Cell { mut s, left, right, sketch, points, roles } = cell();
    let between = SketchOp::SetCoords { points: vec![(points[0], [0.35, 0.5, 0.0])], radii: Vec::new() };
    s.execute(&Command::Sketch { sketch, op: between }).unwrap();
    let only = |name: &str, robot: Id<Robot>, priority: i32| Task {
        robots: RobotChoice::Only(vec![robot]),
        priority,
        ..task(name, roles[0])
    };
    add(&mut s, only("L", left, 2));
    add(&mut s, only("R", right, 1));
    let started = std::time::Instant::now();
    let schedule = s.schedule();
    assert!(started.elapsed().as_secs() < 60, "slow: {:?}", started.elapsed());
    // Whatever the outcome: no robot does two things at once and every wait names a robot that was moving.
    for w in &schedule.waits {
        assert!(w.seconds.is_finite() && w.seconds >= 0.0 && w.robot != w.cause);
    }
    let both: Vec<_> = schedule.assignments.iter().collect();
    for x in &both {
        for y in &both {
            if x.task != y.task && x.robot != y.robot {
                // Two robots may work at the same time only if the planner found them clear (checked every
                // 50 ms): here the target is one point, so they must not overlap.
                assert!(
                    x.end <= y.start + 1e-9 || y.end <= x.start + 1e-9,
                    "both at the point at once: {x:?} {y:?}"
                );
            }
        }
    }
    // No robot is sent to a park pose while it is moving a task.
    for p in &schedule.parks {
        for a in schedule.assignments.iter().filter(|a| a.robot == p.robot) {
            assert!(p.end <= a.start + 1e-9 || p.start >= a.end - 1e-9, "{p:?} overlaps {a:?}");
        }
    }
    // The park pose: wrong length, NaN and infinite joints are refused with no trace.
    for (what, joints) in [
        ("short", vec![0.0; 2]),
        ("long", vec![0.0; 9]),
        ("nan", vec![f64::NAN; 6]),
        ("inf", vec![f64::INFINITY; 6]),
        ("empty", Vec::new()),
    ] {
        assert!(
            step(&mut s, what, |s| s
                .edit_work(WorkOp::SetRobotPark { robot: left, joints: Some(joints.clone()) }))
            .is_none(),
            "{what}"
        );
    }
    // Far outside the limits: accepted or refused; the schedule is still made.
    step(&mut s, "park far outside the limits", |s| {
        s.edit_work(WorkOp::SetRobotPark { robot: left, joints: Some(vec![100.0; 6]) })
    });
    let _ = s.schedule();
}

// ---- Tool kinds ----

fn kit(name: &str) -> ToolKit {
    ToolKit { name: name.into(), signals: Vec::new(), states: Vec::new(), actions: Vec::new() }
}

fn signal(name: &str, direction: IoDirection, pin: u8) -> IoSignal {
    IoSignal { name: name.into(), direction, pin }
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

#[test]
fn tool_kit_pins_signals_and_actions_have_limits() {
    let mut s = Session::new();
    let add_kit = |s: &mut Session, k: ToolKit| s.edit_work(WorkOp::AddToolKit(k));
    // Pins: the same pin twice in one direction is refused; the other direction, pin 0 and 255 are fine.
    let mut k = kit("Gripper");
    k.signals = vec![signal("close", IoDirection::Out, 3), signal("open", IoDirection::Out, 3)];
    assert!(step(&mut s, "one pin, two outputs", |s| add_kit(s, k.clone())).is_none());
    let mut k = kit("Gripper");
    k.signals = vec![signal("close", IoDirection::Out, 3), signal("closed", IoDirection::In, 3)];
    assert!(step(&mut s, "one pin, both directions", |s| add_kit(s, k.clone())).is_some());
    for pin in [0u8, 255] {
        let mut k = kit(&format!("Pin{pin}"));
        k.signals = vec![signal("x", IoDirection::Out, pin)];
        assert!(step(&mut s, &format!("pin {pin}"), |s| add_kit(s, k.clone())).is_some(), "pin {pin}");
    }
    // Names: empty, quote, spaces around, twice in a kit, the kit's own name twice.
    for bad in ["", " x", "x ", "a\"b", "new\nline"] {
        let mut k = kit("Bad");
        k.signals = vec![signal(bad, IoDirection::Out, 1)];
        assert!(step(&mut s, bad, |s| add_kit(s, k.clone())).is_none(), "signal {bad:?}");
        assert!(step(&mut s, bad, |s| add_kit(s, kit(bad))).is_none(), "kit {bad:?}");
    }
    let mut twice = kit("Twice");
    twice.signals = vec![signal("a", IoDirection::Out, 1), signal("a", IoDirection::In, 2)];
    assert!(step(&mut s, "signal twice", |s| add_kit(s, twice.clone())).is_none());
    assert!(step(&mut s, "the kit's name again", |s| add_kit(s, kit("Gripper"))).is_none());
    // Actions: signals of the right direction, states of the kit, numbers finite and not negative.
    let base = |a: ToolAction| {
        let mut k = kit("Act");
        k.signals = vec![signal("out", IoDirection::Out, 1), signal("in", IoDirection::In, 2)];
        k.states = vec!["open".into(), "closed".into()];
        k.actions = vec![a];
        k
    };
    for (what, a, good) in [
        ("plain", action("go"), true),
        ("set an output", ToolAction { set: Some(("out".into(), true)), ..action("go") }, true),
        ("set an input", ToolAction { set: Some(("in".into(), true)), ..action("go") }, false),
        ("confirm an output", ToolAction { confirm: Some(("out".into(), true)), ..action("go") }, false),
        ("set a ghost signal", ToolAction { set: Some(("zzz".into(), true)), ..action("go") }, false),
        ("confirm", ToolAction { confirm: Some(("in".into(), false)), timeout: 1.0, ..action("go") }, true),
        ("tool state", ToolAction { tool_state: Some("closed".into()), ..action("go") }, true),
        ("a ghost tool state", ToolAction { tool_state: Some("half".into()), ..action("go") }, false),
        (
            "a target state with no target",
            ToolAction { target_state: Some("x".into()), ..action("go") },
            false,
        ),
        ("negative duration", ToolAction { duration: -1.0, ..action("go") }, false),
        ("nan duration", ToolAction { duration: f64::NAN, ..action("go") }, false),
        ("inf timeout", ToolAction { timeout: f64::INFINITY, ..action("go") }, false),
        ("huge duration", ToolAction { duration: 1e300, ..action("go") }, true),
    ] {
        let r = step(&mut s, what, |s| add_kit(s, base(a.clone())));
        assert_eq!(r.is_some(), good, "{what}");
        if r.is_some() {
            let last = *s.doc().tool_kits().iter().map(|(id, _)| id).collect::<Vec<_>>().last().unwrap();
            s.edit_work(WorkOp::DeleteToolKit(last)).unwrap();
        }
    }
}

#[test]
fn tool_state_in_conditions_and_renames_follow_into_steps() {
    let Cell { mut s, left, roles, .. } = cell();
    let ty: Id<ObjectType> = s
        .edit_work(WorkOp::AddObjectType(ObjectType {
            name: "Part".into(),
            states: vec!["loose".into(), "held".into()],
        }))
        .unwrap()
        .unwrap()
        .downcast()
        .unwrap();
    // `tool.state` and `target.state` are known inside an action's precondition only.
    let mk = |precondition: &str, target: Option<Id<ObjectType>>| {
        let mut k = kit("Hand");
        k.signals = vec![signal("grip", IoDirection::Out, 1), signal("sensed", IoDirection::In, 2)];
        k.states = vec!["open".into(), "closed".into()];
        k.actions = vec![ToolAction {
            target,
            precondition: precondition.into(),
            set: Some(("grip".into(), true)),
            confirm: Some(("sensed".into(), true)),
            tool_state: Some("closed".into()),
            duration: 0.5,
            timeout: 2.0,
            ..action("pick")
        }];
        k
    };
    for (what, pre, target, good) in [
        ("tool state", "tool.state == open", None, true),
        ("tool and target", "tool.state == open and target.state == loose", Some(ty), true),
        ("target with no target type", "target.state == loose", None, false),
        ("a word that is nothing", "tool.state == half", None, false),
        ("tool member that is unknown", "tool.mode == open", None, false),
        ("an object that is not there", "Nobody.state == open", None, false),
        ("not a condition", "tool.state", None, false),
        ("a number", "1 + 1", None, false),
        ("deep", &"(".repeat(50_000), None, false),
    ] {
        let r = step(&mut s, what, |s| s.edit_work(WorkOp::AddToolKit(mk(pre, target))));
        assert_eq!(r.is_some(), good, "{what}");
        if r.is_some() {
            let last = *s.doc().tool_kits().iter().map(|(id, _)| id).collect::<Vec<_>>().last().unwrap();
            s.edit_work(WorkOp::DeleteToolKit(last)).unwrap();
        }
    }
    // A kit used by a robot and by a step: renames of signals, states and actions follow, one step each.
    let kit_id: Id<ToolKit> =
        s.edit_work(WorkOp::AddToolKit(mk("tool.state == open", None))).unwrap().unwrap().downcast().unwrap();
    s.edit_work(WorkOp::SetRobotTool { robot: left, kit: Some(kit_id), state: None }).unwrap();
    let t = add(
        &mut s,
        Task {
            steps: vec![
                Step::MoveJ(MoveTarget::Role(roles[0])),
                Step::ToolAction { action: "pick".into(), target: None },
                Step::SetIo { name: "grip".into(), value: false },
            ],
            ..task("Pick", roles[0])
        },
    );
    assert!(s.doc().task_problems(t).is_empty(), "{:?}", s.doc().task_problems(t));
    let mut renamed = s.doc().get(kit_id).unwrap().clone();
    renamed.actions[0].name = "grab".into();
    renamed.signals[0].name = "squeeze".into();
    renamed.states[1] = "shut".into();
    // The kit's own references are the editor's to keep: the action's signal and state follow here.
    renamed.actions[0].set = Some(("squeeze".into(), true));
    renamed.actions[0].tool_state = Some("shut".into());
    assert!(
        step(&mut s, "rename an action, a signal and a state", |s| s
            .edit_work(WorkOp::SetToolKit { id: kit_id, kit: renamed.clone() }))
        .is_some()
    );
    let stored = s.doc().get(t).unwrap();
    assert!(
        matches!(&stored.steps[1], Step::ToolAction { action, .. } if action == "grab"),
        "{:?}",
        stored.steps
    );
    assert!(matches!(&stored.steps[2], Step::SetIo { name, .. } if name == "squeeze"), "{:?}", stored.steps);
    assert!(s.doc().task_problems(t).is_empty());
    let kit_now = s.doc().get(kit_id).unwrap();
    assert_eq!(kit_now.actions[0].set, Some(("squeeze".into(), true)), "the action's own signal follows");
    assert_eq!(kit_now.actions[0].tool_state, Some("shut".into()), "the action's own state follows");
    // Removing what a step needs is refused (InUse); deleting a kit a robot carries is refused; a ghost kit.
    let mut without = kit_now.clone();
    without.actions.clear();
    assert!(
        step(&mut s, "remove the action a step uses", |s| s
            .edit_work(WorkOp::SetToolKit { id: kit_id, kit: without.clone() }))
        .is_none()
    );
    assert!(step(&mut s, "delete the fitted kit", |s| s.edit_work(WorkOp::DeleteToolKit(kit_id))).is_none());
    let ghost_kit = Id::<ToolKit>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    assert!(
        step(&mut s, "ghost kit", |s| s.edit_work(WorkOp::SetRobotTool {
            robot: left,
            kit: Some(ghost_kit),
            state: None
        }))
        .is_none()
    );
    let ghost_robot = Id::<Robot>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    assert!(
        step(&mut s, "ghost robot", |s| s.edit_work(WorkOp::SetRobotTool {
            robot: ghost_robot,
            kit: Some(kit_id),
            state: None
        }))
        .is_none()
    );
    assert!(
        step(&mut s, "a state the kit does not have", |s| s.edit_work(WorkOp::SetRobotTool {
            robot: left,
            kit: Some(kit_id),
            state: Some("half".into())
        }))
        .is_none()
    );
    assert!(
        step(&mut s, "the second state", |s| s.edit_work(WorkOp::SetRobotTool {
            robot: left,
            kit: Some(kit_id),
            state: Some("shut".into())
        }))
        .is_some()
    );
    // A kit state removed that the robot stands in: it falls back to the kit's first state.
    let mut fewer = s.doc().get(kit_id).unwrap().clone();
    fewer.actions[0].tool_state = None;
    fewer.states = vec!["open".into()];
    step(&mut s, "drop the state the robot is in", |s| {
        s.edit_work(WorkOp::SetToolKit { id: kit_id, kit: fewer.clone() })
    });
    assert!(
        s.doc().robot_tool_kit(left).is_none_or(|(_, state)| state.as_deref().is_none_or(|x| x == "open"))
    );
    // Saved and opened.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kit.gcp");
    s.save_as(&path).unwrap();
    let mut u = Session::new();
    u.open(&path).unwrap();
    assert!(u.doc().same_content(s.doc()));
}

// ---- URScript ----

fn program_lines_are_commands(text: &str) {
    // Top-level `# ` lines (the generator header, lead request) may come before the first `def`.
    let header = text.lines().take_while(|l| l.starts_with("# ")).count();
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        let ok = i < header
            || i == header && line.starts_with("def ")
            || line == "end"
            || line.starts_with("  ")
                && [
                    "movej(",
                    "movel(",
                    "set_tcp(",
                    "set_digital_out(",
                    "sleep(",
                    "# ",
                    "waited = ",
                    "while (",
                    "end",
                ]
                .iter()
                .any(|p| trimmed.starts_with(p))
            || trimmed.is_empty();
        assert!(ok, "line {i} is not a URScript command or a comment line: {line:?}\n{text}");
    }
}

#[test]
fn urscript_identifiers_numbers_and_empty_tasks() {
    let Cell { mut s, left, right, roles, .. } = cell();
    let ident = |text: &str| -> String {
        // The program starts with a generator comment (lead request); the first `def` names it.
        let line = text.lines().find(|l| l.starts_with("def ")).unwrap();
        line.strip_prefix("def ").unwrap().strip_suffix("():").unwrap().to_owned()
    };
    for name in [
        "Fasten A-1",
        "홀 A",
        "한글",
        "1abc",
        "a b",
        "end",
        "def",
        "x;y",
        "🙂",
        "a\"b",
        "a.b",
        "_",
        "-",
        &"x".repeat(5000),
        "tab\t",
    ] {
        let t = add(&mut s, task(name, roles[0]));
        match s.task_urscript(t, left) {
            Ok(text) => {
                let id = ident(&text);
                assert!(
                    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
                    "{name:?} -> {id:?}"
                );
                assert!(!id.chars().next().unwrap().is_ascii_digit(), "{name:?} -> {id:?}");
                assert!(text.ends_with("end\n"));
                program_lines_are_commands(&text);
            }
            Err(why) => assert!(!why.is_empty()),
        }
        s.edit_work(WorkOp::DeleteTask(t)).unwrap();
    }
    // A name that is only non-ASCII becomes a row of underscores: two different names can give the same one.
    let (a, b) = (add(&mut s, task("작업", roles[0])), add(&mut s, task("경로", roles[0])));
    let (ta, tb) = (s.task_urscript(a, left).unwrap(), s.task_urscript(b, left).unwrap());
    eprintln!("observation: Korean-only task names give identifiers {:?} and {:?}", ident(&ta), ident(&tb));
    // An empty task (no steps), one that only waits: a program or a reason; no NaN anywhere.
    let empty = add(&mut s, Task { steps: Vec::new(), ..task("Empty", roles[0]) });
    for (what, t) in [("empty", empty)] {
        match s.task_urscript(t, left) {
            Ok(text) => {
                assert!(text.contains("\ndef Empty():\n") && text.ends_with("end\n"), "{what}: {text}");
                program_lines_are_commands(&text);
            }
            Err(why) => assert!(!why.is_empty(), "{what}"),
        }
    }
    // Numbers: every planned number is finite and written without exponent or NaN, in every kind of step.
    let mut work = task("Numbers", roles[0]);
    work.steps = vec![
        Step::MoveJ(MoveTarget::Role(roles[0])),
        Step::MoveL(MoveTarget::Role(roles[1])),
        Step::Wait { seconds: 1e-9 },
        Step::Wait { seconds: 3600.0 },
        Step::Wait { seconds: 0.0 },
    ];
    work.robots = RobotChoice::Only(vec![left]);
    let n = add(&mut s, work);
    if let Ok(text) = s.task_urscript(n, left) {
        for bad in ["NaN", "nan", "inf", "Infinity", "e-", "e+", "E-"] {
            assert!(!text.contains(bad), "{bad:?} in the program:\n{text}");
        }
        program_lines_are_commands(&text);
        // Same input, same text.
        assert_eq!(text, s.task_urscript(n, left).unwrap());
    }
    // A robot with a turned tool centre point (half a turn: the rotation vector's hard case) stays finite.
    for rpy in [
        [0.0, 0.0, 0.0],
        [std::f64::consts::PI, 0.0, 0.0],
        [0.0, std::f64::consts::PI, 0.0],
        [std::f64::consts::PI, std::f64::consts::PI, 0.0],
        [1e-12, 0.0, 0.0],
    ] {
        s.execute(&Command::Cell(CellOp::SetTcp { id: left, tcp: Placement { xyz: [0.0, 0.0, 0.1], rpy } }))
            .unwrap();
        if let Ok(text) = s.task_urscript(n, left) {
            assert!(!text.contains("NaN") && !text.contains("inf"), "{rpy:?}:\n{text}");
        }
    }
    // A schedule with nothing to do gives no programs; with a task, one per robot that has one.
    assert!(Session::new().schedule_urscript(&Session::new().schedule()).is_empty());
    let programs = s.schedule_urscript(&s.schedule());
    for (robot, text) in &programs {
        assert!(
            text.starts_with("# Generated by GraphCTRL")
                && text.contains("\ndef ")
                && text.ends_with("end\n")
        );
        program_lines_are_commands(text);
        let _ = robot;
    }
    let _ = right;
}

/// BUG: URScript text is made by writing names and conditions into comments; a task name, a robot name, a signal
/// condition or a step's text with a line break ends the comment and the rest of the text becomes a command of
/// the program. Task names, robot names and `WaitIo` conditions take any text (no line break check), so a name
/// pasted from elsewhere can put a `movej` / `popup` / `set_digital_out` line into a program meant for a robot.
#[test]
fn text_with_a_line_break_cannot_become_a_command() {
    let Cell { mut s, left, roles, .. } = cell();
    let payload = "x\n  movej([9.0, 9.0, 9.0, 9.0, 9.0, 9.0], a=9, v=9, r=0)";
    // A WaitIo condition goes into `# TODO wait for: ...`.
    let t = add(
        &mut s,
        Task {
            steps: vec![
                Step::MoveJ(MoveTarget::Role(roles[0])),
                Step::WaitIo { condition: payload.into(), timeout: 1.0 },
            ],
            ..task("Wait", roles[0])
        },
    );
    let text = s.task_urscript(t, left).unwrap();
    assert!(
        !text.lines().any(|l| l.trim_start().starts_with("movej([9.0")),
        "the program has the pasted command:\n{text}"
    );
    // A task name goes into the schedule program's `# <title>` line.
    let named = add(&mut s, task(payload, roles[0]));
    let _ = named;
    let programs = s.schedule_urscript(&s.schedule());
    for (_, text) in programs {
        assert!(
            !text.lines().any(|l| l.trim_start().starts_with("movej([9.0")),
            "the schedule program has the pasted command:\n{text}"
        );
    }
}
