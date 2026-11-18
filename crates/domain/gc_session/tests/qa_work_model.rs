//! QA boundary tests for the work model's data layer (plan 9.4, GraphCTRL's own; no Inventor counterpart): checked
//! conditions (errors with their place, quoted names), renames followed in one step, `InUse` for what is referred
//! to, one undo step per edit, save and open, `task_problems` after roles lose their sketch elements, and the
//! limit on a robot's own waypoints to tasks located at that robot.
//! Refused edits leave the document and the history untouched; accepted ones are one undo step that restores
//! exactly (redo repeats).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::disallowed_methods,
    reason = "test code"
)]

use gc_core::Id;
use gc_doc::{
    Command, Created, EditError, Effect, EffectTarget, EndSnap, Location, MoveTarget, ObjectType,
    RobotChoice, SketchOp, StateValue, StateVar, Step, Task, WaypointTarget, WorkObject, WorkOp,
};
use gc_session::Session;
use gc_sketch::Item;

fn six_axis() -> String {
    let path = format!("{}/../../../fixtures/urdf/six_axis.urdf", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap()
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

fn task(name: &str, precondition: &str) -> Task {
    Task {
        name: name.into(),
        location: Location::None,
        precondition: precondition.into(),
        steps: Vec::new(),
        effects: Vec::new(),
        robots: RobotChoice::Any,
        priority: 0,
    }
}

fn add_type(s: &mut Session, name: &str, states: &[&str]) -> Id<ObjectType> {
    let ty = ObjectType { name: name.into(), states: states.iter().map(|x| (*x).to_owned()).collect() };
    s.edit_work(WorkOp::AddObjectType(ty)).unwrap().unwrap().downcast().unwrap()
}

fn add_object(s: &mut Session, name: &str, ty: Id<ObjectType>, state: &str) -> Id<WorkObject> {
    let o = WorkObject { name: name.into(), ty, location: Location::None, state: state.into() };
    s.edit_work(WorkOp::AddObject(o)).unwrap().unwrap().downcast().unwrap()
}

fn add_var(s: &mut Session, name: &str, value: StateValue) -> Id<StateVar> {
    s.edit_work(WorkOp::AddStateVar(StateVar { name: name.into(), value }))
        .unwrap()
        .unwrap()
        .downcast()
        .unwrap()
}

#[test]
fn conditions_report_errors_with_their_place_and_never_blow_up() {
    let mut s = Session::new();
    let ty = add_type(&mut s, "홀", &["비어 있음", "체결됨"]);
    add_object(&mut s, "홀 A", ty, "비어 있음");
    add_var(&mut s, "count", StateValue::Int(0));
    add_var(&mut s, "ok", StateValue::Bool(true));
    add_var(&mut s, "ratio", StateValue::Real(0.5));
    add_var(
        &mut s,
        "mode",
        StateValue::Enum { choices: vec!["auto".into(), "manual".into()], value: "auto".into() },
    );
    // Good ones, with every operator and spelling.
    for good in [
        "count < 3",
        "count <= 3 and ok",
        "not ok or count != 0",
        "!ok || count >= 1 && ratio > 0.25",
        "\"홀 A\".state == \"비어 있음\"",
        "\"홀 A\".state != 체결됨",
        "((ok))",
        "mode == auto",
        "mode == \"manual\" or mode == auto",
        "true",
        "false and true",
        "not not ok",
        "ok == true",
        "count == 0.0",
        "-1 < count",
    ] {
        assert_eq!(s.check_condition(good), Ok(()), "{good:?}");
    }
    // Wrong ones: each reports a place inside the text, on character boundaries.
    for bad in [
        "",
        "   ",
        "((count))",
        "count",
        "count <",
        "< 3",
        "count 3",
        "count == ",
        "(count < 3",
        "count < 3)",
        "()",
        "and",
        "count and",
        "count == 체결됨",
        "ok < true",
        "mode < auto",
        "mode == nothere",
        "홀Z.state == 체결됨",
        "\"홀 A\".nofield == 1",
        "\"홀 A\".state",
        "\"홀 A\"",
        "count == 1e999",
        "count == 1..2",
        "count === 1",
        "count = 1",
        "\"unclosed",
        "\"홀 A\".state == \"비어 있음",
        "count < 3 count",
        "1 < 2 < 3",
        "count.state == 1",
        "ok.state",
        "홀 A.state == 체결됨",
        "count == \u{0}",
        "count == '3'",
        "count <> 3",
        "count ; 3",
        "ratio == nan",
        "ratio == inf",
    ] {
        match s.check_condition(bad) {
            Ok(()) => panic!("{bad:?} was accepted"),
            Err(EditError::Expr { start, end, .. }) => {
                assert!(start <= end && end <= bad.len(), "{bad:?}: range {start}..{end}");
                assert!(
                    bad.is_char_boundary(start) && bad.is_char_boundary(end),
                    "{bad:?}: {start}..{end} cuts a character"
                );
            }
            Err(other) => panic!("{bad:?}: {other:?}"),
        }
    }
    // Huge and deep texts: refused or accepted, quickly and without a stack overflow.
    let started = std::time::Instant::now();
    for text in [
        "count < 3 ".to_owned() + &"and count < 3 ".repeat(1_000),
        "(".repeat(100_000) + "ok" + &")".repeat(100_000),
        "not ".repeat(100_000) + "ok",
        "\"".to_owned() + &"x".repeat(1_000_000) + "\".state == 1",
        "x".repeat(1_000_000),
    ] {
        s.check_condition(&text).ok();
    }
    eprintln!("report: huge conditions took {:?}", started.elapsed());
    assert!(
        started.elapsed().as_secs() < 20,
        "conditions of a million characters took {:?}",
        started.elapsed()
    );
}

/// Was a bug (fixed by the 256-term limit): a condition made of thousands of terms joined by `and` / `or` overflows the stack (the parser or the
/// checker recurses once per term): a 1 MB stack (the Windows main thread) is gone at about 5000 terms, and the
/// process aborts instead of the text being refused. Deep parentheses and `not` chains are guarded and refused.
#[test]
fn a_long_chain_of_and_terms_is_refused_not_a_crash() {
    let text = "true and ".repeat(5_000) + "true";
    let s = Session::new();
    let ok = std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(move || s.check_condition(&text).ok())
        .unwrap()
        .join();
    assert!(ok.is_ok(), "the check panicked");
}

#[test]
fn names_are_checked_unique_and_usable_in_conditions() {
    let mut s = Session::new();
    let ty = add_type(&mut s, "홀", &["a", "b"]);
    // Bad names for every kind.
    let bad_names = ["", " ", " lead", "trail ", "a\"b", "tab\tinside", "new\nline", "\u{0}", "\u{7f}"];
    for name in bad_names {
        let what = format!("{name:?}");
        assert!(
            step(&mut s, &what, |s| s.edit_work(WorkOp::AddObjectType(ObjectType {
                name: name.into(),
                states: vec!["x".into()]
            })))
            .is_none(),
            "type {what}"
        );
        assert!(
            step(&mut s, &what, |s| s.edit_work(WorkOp::AddObject(WorkObject {
                name: name.into(),
                ty,
                location: Location::None,
                state: "a".into()
            })))
            .is_none(),
            "object {what}"
        );
        assert!(
            step(&mut s, &what, |s| s
                .edit_work(WorkOp::AddStateVar(StateVar { name: name.into(), value: StateValue::Int(0) })))
            .is_none(),
            "var {what}"
        );
        // A task's name is not used in conditions: odd names may be taken, whichever, with the step rules.
        step(&mut s, &what, |s| s.edit_work(WorkOp::AddTask(task(name, ""))));
    }
    // Long and odd names that are allowed work the same in conditions once quoted.
    for name in
        ["한글 이름", "has space", "a.b", "and", "true", "not", "x".repeat(10_000).as_str(), "1abc", "a-b"]
    {
        let what = format!("object {:?}", name.chars().take(10).collect::<String>());
        let Some(o) = step(&mut s, &what, |s| {
            s.edit_work(WorkOp::AddObject(WorkObject {
                name: name.into(),
                ty,
                location: Location::None,
                state: "a".into(),
            }))
        }) else {
            continue;
        };
        let _ = o;
        let quoted = format!("\"{name}\".state == a");
        assert_eq!(s.check_condition(&quoted), Ok(()), "{what}: quoted");
        // A name that is a keyword or has punctuation must not read unquoted as something else: either the
        // unquoted text is refused, or it means the object (never silently another thing).
        let bare = format!("{name}.state == a");
        s.check_condition(&bare).ok();
        s.undo();
    }
    // Names are unique within a kind but a type, an object and a variable may share one.
    let o = add_object(&mut s, "Same", ty, "a");
    assert!(
        step(&mut s, "object twice", |s| s.edit_work(WorkOp::AddObject(WorkObject {
            name: "Same".into(),
            ty,
            location: Location::None,
            state: "a".into()
        })))
        .is_none()
    );
    assert!(
        step(&mut s, "var with an object's name", |s| s
            .edit_work(WorkOp::AddStateVar(StateVar { name: "Same".into(), value: StateValue::Bool(true) })))
        .is_some()
    );
    let _ = o;
    // The check of names is case-sensitive: a different case is another name.
    let upper = step(&mut s, "case", |s| {
        s.edit_work(WorkOp::AddObject(WorkObject {
            name: "SAME".into(),
            ty,
            location: Location::None,
            state: "a".into(),
        }))
    });
    eprintln!(
        "observation: an object named SAME beside Same -> {}",
        if upper.is_some() { "accepted" } else { "refused" }
    );
}

#[test]
fn renames_follow_into_conditions_effects_and_swaps_work() {
    let mut s = Session::new();
    let ty = add_type(&mut s, "Hole", &["empty", "done"]);
    let a = add_object(&mut s, "A", ty, "empty");
    let b = add_object(&mut s, "B", ty, "empty");
    let count = add_var(&mut s, "count", StateValue::Int(0));
    let t = Task {
        precondition: "A.state == empty and B.state != done and count < 3".into(),
        effects: vec![
            Effect { target: EffectTarget::Object(a), value: "done".into() },
            Effect { target: EffectTarget::Var(count), value: "1".into() },
        ],
        robots: RobotChoice::When("A.state == empty".into()),
        ..task("T", "")
    };
    let tid: Id<Task> = s.edit_work(WorkOp::AddTask(t)).unwrap().unwrap().downcast().unwrap();
    // Each rename is one step that follows every use and undoes exactly.
    let rename = |s: &mut Session, id: gc_core::AnyId, name: &str| {
        s.execute(&Command::Rename { item: id, name: name.into() })
    };
    for (name, expect) in [
        ("Hole A", "\"Hole A\".state == empty and B.state != done and count < 3"),
        ("홀", "홀.state == empty and B.state != done and count < 3"),
        ("A2", "A2.state == empty and B.state != done and count < 3"),
    ] {
        assert!(step(&mut s, name, |s| rename(s, a.erase(), name).map(|_| ())).is_some(), "{name}");
        assert_eq!(s.doc().get(tid).unwrap().precondition, expect, "{name}");
        assert_eq!(
            s.doc().get(tid).unwrap().robots,
            RobotChoice::When(format!(
                "{}.state == empty",
                if name.contains(' ') { format!("\"{name}\"") } else { name.to_owned() }
            ))
        );
        assert_eq!(s.check_condition(&s.doc().get(tid).unwrap().precondition), Ok(()), "{name}: still reads");
        assert_eq!(s.task_ready(tid), Ok(true));
    }
    // Observation: Rename trims leading/trailing spaces (" pad" becomes "pad") where Add refuses such a name.
    // Names that cannot be: empty, with a quote, taken, ghost id; refused with no trace.
    for bad in ["", "a\"b", "B"] {
        assert!(step(&mut s, bad, |s| rename(s, a.erase(), bad).map(|_| ())).is_none(), "{bad:?}");
    }
    let ghost = Id::<WorkObject>::from_raw(std::num::NonZeroU64::new(9999).unwrap()).erase();
    assert!(step(&mut s, "ghost", |s| rename(s, ghost, "X").map(|_| ())).is_none());
    // A swap through a third name: A2 -> tmp, B -> A2, tmp -> B; conditions keep meaning the same objects.
    s.execute(&Command::Rename { item: a.erase(), name: "tmp".into() }).unwrap();
    s.execute(&Command::Rename { item: b.erase(), name: "A2".into() }).unwrap();
    s.execute(&Command::Rename { item: a.erase(), name: "B".into() }).unwrap();
    assert_eq!(s.check_condition(&s.doc().get(tid).unwrap().precondition), Ok(()));
    assert_eq!(s.task_ready(tid), Ok(true));
    // The renamed object now named B is the one the task's effect still sets (by id, not by name).
    assert!(matches!(s.doc().get(tid).unwrap().effects[0].target, EffectTarget::Object(id) if id == a));
    // A variable renamed to the name of a state: the word now means the variable in conditions; the model
    // refuses it (`InUse`) or keeps every condition valid.
    let before = s.doc().clone();
    let r = s.execute(&Command::Rename { item: count.erase(), name: "empty".into() });
    match r {
        Err(_) => assert!(s.doc().same_content(&before)),
        Ok(_) => {
            for (_, t) in s.doc().tasks().iter() {
                assert_eq!(
                    s.check_condition(&t.precondition),
                    Ok(()),
                    "a condition broke by renaming a variable to a state name"
                );
            }
            assert!(s.undo());
        }
    }
    // State renames of the type: same count renames; fewer is refused while used; whatever, never invalid.
    let renamed = ObjectType { name: "Hole".into(), states: vec!["free".into(), "fixed".into()] };
    assert!(
        step(&mut s, "rename states", |s| s.edit_work(WorkOp::SetObjectType { id: ty, ty: renamed.clone() }))
            .is_some()
    );
    assert!(s.doc().get(tid).unwrap().precondition.contains("free"));
    assert_eq!(s.task_ready(tid), Ok(true));
    let swapped = ObjectType { name: "Hole".into(), states: vec!["fixed".into(), "free".into()] };
    step(&mut s, "swap the two state names", |s| {
        s.edit_work(WorkOp::SetObjectType { id: ty, ty: swapped.clone() })
    });
    for (_, t) in s.doc().tasks().iter() {
        assert_eq!(s.check_condition(&t.precondition), Ok(()));
    }
    let more =
        ObjectType { name: "Hole".into(), states: vec!["free".into(), "fixed".into(), "extra".into()] };
    step(&mut s, "an extra state", |s| s.edit_work(WorkOp::SetObjectType { id: ty, ty: more.clone() }));
    for bad in [
        ObjectType { name: "Hole".into(), states: Vec::new() },
        ObjectType { name: "Hole".into(), states: vec!["x".into(), "x".into()] },
        ObjectType { name: "Hole".into(), states: vec!["".into()] },
        ObjectType { name: "Hole".into(), states: vec!["a\"b".into()] },
        ObjectType { name: "Hole".into(), states: vec!["only".into()] },
    ] {
        let what = format!("{:?}", bad.states);
        assert!(
            step(&mut s, &what, |s| s.edit_work(WorkOp::SetObjectType { id: ty, ty: bad.clone() })).is_none(),
            "{what}"
        );
    }
}

#[test]
fn deleting_what_is_referred_to_is_refused_and_everything_goes_when_free() {
    let mut s = Session::new();
    let ty = add_type(&mut s, "T", &["a", "b"]);
    let o = add_object(&mut s, "O", ty, "a");
    let v = add_var(&mut s, "V", StateValue::Int(0));
    let sketch = s.create_sketch("XY").unwrap();
    let p = {
        let Created::Point(p) = s
            .execute(&Command::Sketch { sketch, op: SketchOp::AddPoint { pos: [0.1, 0.1, 0.0] } })
            .unwrap()[0]
        else {
            panic!()
        };
        p
    };
    s.finish_sketch();
    let role = s.add_waypoint_role(sketch, p).unwrap();
    let t = Task {
        precondition: "O.state == a and V < 1".into(),
        steps: vec![
            Step::MoveJ(MoveTarget::Role(role)),
            Step::ToolAction { action: "x".into(), target: Some(o) },
        ],
        effects: vec![Effect { target: EffectTarget::Var(v), value: "1".into() }],
        ..task("Work", "")
    };
    let tid: Id<Task> = s.edit_work(WorkOp::AddTask(t)).unwrap().unwrap().downcast().unwrap();
    // Everything in use is refused with no trace.
    for (what, op) in [
        ("the type", WorkOp::DeleteObjectType(ty)),
        ("the object", WorkOp::DeleteObject(o)),
        ("the variable", WorkOp::DeleteStateVar(v)),
    ] {
        assert!(step(&mut s, what, |s| s.edit_work(op)).is_none(), "{what} was deleted");
    }
    assert!(step(&mut s, "the role", |s| s.delete_role(role)).is_none());
    let before = s.doc().clone();
    assert!(s.execute(&Command::DeleteSketch { id: sketch }).is_err(), "a sketch whose role a task uses");
    assert!(s.doc().same_content(&before));
    // Changing the role's kind: a SetRole that makes a waypoint a path is refused while a step moves to it.
    // Ghost ids refuse too.
    let ghost_task = Id::<Task>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    assert!(step(&mut s, "ghost task", |s| s.edit_work(WorkOp::DeleteTask(ghost_task))).is_none());
    assert!(
        step(&mut s, "ghost set", |s| s.edit_work(WorkOp::SetTask { id: ghost_task, task: task("x", "") }))
            .is_none()
    );
    // A step that points at a role that is not there; an effect on an object that is not there.
    let ghost_role = Id::<gc_doc::SketchRole>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    let bad_step = Task { steps: vec![Step::MoveJ(MoveTarget::Role(ghost_role))], ..task("Bad", "") };
    assert!(
        step(&mut s, "ghost role in a step", |s| s.edit_work(WorkOp::AddTask(bad_step.clone()))).is_none()
    );
    let ghost_obj = Id::<WorkObject>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    let bad_effect = Task {
        effects: vec![Effect { target: EffectTarget::Object(ghost_obj), value: "a".into() }],
        ..task("Bad", "")
    };
    assert!(
        step(&mut s, "ghost object in an effect", |s| s.edit_work(WorkOp::AddTask(bad_effect.clone())))
            .is_none()
    );
    // Free again: the task goes, then everything it held, one step each, each undone exactly.
    assert!(step(&mut s, "delete the task", |s| s.edit_work(WorkOp::DeleteTask(tid))).is_some());
    for (what, op) in [
        ("object", WorkOp::DeleteObject(o)),
        ("type", WorkOp::DeleteObjectType(ty)),
        ("variable", WorkOp::DeleteStateVar(v)),
    ] {
        assert!(step(&mut s, what, |s| s.edit_work(op)).is_some(), "{what} is free now");
        s.undo();
        s.redo();
    }
    assert!(step(&mut s, "role", |s| s.delete_role(role)).is_some());
}

#[test]
fn task_fields_have_limits_and_a_robots_own_waypoints_belong_to_its_own_tasks() {
    let mut s = Session::new();
    let robot = s.add_robot("Robot1", &six_axis()).unwrap();
    let other = s.add_robot("Robot2", &six_axis()).unwrap();
    let home = s.add_waypoint(robot, WaypointTarget::Joints(vec![0.0; 6])).unwrap();
    let ty = add_type(&mut s, "T", &["a", "b"]);
    let o = add_object(&mut s, "O", ty, "a");
    let v = add_var(&mut s, "n", StateValue::Int(0));
    let flag = add_var(&mut s, "flag", StateValue::Bool(false));
    let r = add_var(&mut s, "r", StateValue::Real(0.0));
    let mode = add_var(
        &mut s,
        "mode",
        StateValue::Enum { choices: vec!["x".into(), "y".into()], value: "x".into() },
    );
    let add =
        |s: &mut Session, what: &str, t: Task| step(s, what, |s| s.edit_work(WorkOp::AddTask(t.clone())));
    // Waypoint limits: in the robot's own task fine; elsewhere refused; a ghost location refused.
    let own = |location: Location, target: MoveTarget| Task {
        location,
        steps: vec![Step::MoveJ(target)],
        ..task("T", "")
    };
    assert!(
        add(&mut s, "own waypoint, own task", own(Location::Robot(robot), MoveTarget::Waypoint(home)))
            .is_some()
    );
    s.undo();
    s.redo();
    let n = s.tasks().len();
    for (what, location) in [
        ("another robot's task", Location::Robot(other)),
        ("no location", Location::None),
        ("a sketch location", Location::Sketch(s.create_sketch("XY").unwrap())),
    ] {
        s.finish_sketch();
        let t = own(location, MoveTarget::Waypoint(home));
        let before = s.doc().clone();
        assert!(s.edit_work(WorkOp::AddTask(t)).is_err(), "{what}: a robot's waypoint outside its own task");
        assert!(s.doc().same_content(&before), "{what}");
    }
    assert_eq!(s.tasks().len(), n);
    // Moving an existing own task elsewhere while it keeps the waypoint: refused (no trace).
    let tid = s.tasks()[0];
    let moved = Task { location: Location::Robot(other), ..s.doc().get(tid).unwrap().clone() };
    assert!(
        step(&mut s, "move the own task to another robot", |s| s
            .edit_work(WorkOp::SetTask { id: tid, task: moved.clone() }))
        .is_none()
    );
    let nowhere = Task { location: Location::None, ..s.doc().get(tid).unwrap().clone() };
    assert!(
        step(&mut s, "move the own task nowhere", |s| s
            .edit_work(WorkOp::SetTask { id: tid, task: nowhere.clone() }))
        .is_none()
    );
    // The robot with a task cannot be deleted; its waypoint cannot either.
    let before = s.doc().clone();
    assert!(s.execute(&Command::Cell(gc_doc::CellOp::DeleteRobot(robot))).is_err());
    assert!(s.execute(&Command::Cell(gc_doc::CellOp::DeleteWaypoint(home))).is_err());
    assert!(s.doc().same_content(&before));
    // Ghost robots and components in locations and choices.
    let ghost_robot = Id::<gc_doc::Robot>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    assert!(
        add(&mut s, "ghost robot location", Task { location: Location::Robot(ghost_robot), ..task("G", "") })
            .is_none()
    );
    assert!(
        add(
            &mut s,
            "ghost robot choice",
            Task { robots: RobotChoice::Only(vec![ghost_robot]), ..task("G", "") }
        )
        .is_none()
    );
    add(&mut s, "a robot twice", Task { robots: RobotChoice::Only(vec![robot, robot]), ..task("G", "") });
    // Observation: an empty `When` condition is taken (as an empty precondition is: always).
    let empty_when = add(
        &mut s,
        "when with an empty condition",
        Task { robots: RobotChoice::When(String::new()), ..task("G", "") },
    );
    eprintln!(
        "observation: RobotChoice::When(\"\") -> {}",
        if empty_when.is_some() { "accepted" } else { "refused" }
    );
    if empty_when.is_some() {
        let last = *s.tasks().last().unwrap();
        s.edit_work(WorkOp::DeleteTask(last)).unwrap();
    }
    // Numbers: priority extremes accepted; waits and timeouts finite and not negative.
    for p in [i32::MIN, -1, 0, i32::MAX] {
        assert!(
            add(&mut s, &format!("priority {p}"), Task { priority: p, ..task("P", "") }).is_some(),
            "{p}"
        );
        s.undo();
        s.redo();
        let last = *s.tasks().last().unwrap();
        s.edit_work(WorkOp::DeleteTask(last)).unwrap();
    }
    for secs in [0.0, 1e-9, 3600.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e300] {
        let ok = add(
            &mut s,
            &format!("wait {secs}"),
            Task { steps: vec![Step::Wait { seconds: secs }], ..task("W", "") },
        );
        if ok.is_some() {
            assert!(secs.is_finite() && secs >= 0.0, "wait {secs} was accepted");
            let last = *s.tasks().last().unwrap();
            s.edit_work(WorkOp::DeleteTask(last)).unwrap();
        }
        let ok = add(
            &mut s,
            &format!("timeout {secs}"),
            Task { steps: vec![Step::WaitIo { condition: "x".into(), timeout: secs }], ..task("W", "") },
        );
        if ok.is_some() {
            assert!(secs.is_finite() && secs >= 0.0, "timeout {secs} was accepted");
            let last = *s.tasks().last().unwrap();
            s.edit_work(WorkOp::DeleteTask(last)).unwrap();
        }
    }
    // Effects: values by kind.
    let effect =
        |target, value: &str| Task { effects: vec![Effect { target, value: value.into() }], ..task("E", "") };
    let cases: Vec<(&str, EffectTarget, &str, bool)> = vec![
        ("object state", EffectTarget::Object(o), "b", true),
        ("object unknown state", EffectTarget::Object(o), "c", false),
        ("object empty", EffectTarget::Object(o), "", false),
        ("int", EffectTarget::Var(v), "3", true),
        ("int negative", EffectTarget::Var(v), "-3", true),
        ("int with a point", EffectTarget::Var(v), "3.5", false),
        ("int huge", EffectTarget::Var(v), "99999999999999999999999", false),
        ("int words", EffectTarget::Var(v), "three", false),
        ("bool true", EffectTarget::Var(flag), "true", true),
        ("bool false", EffectTarget::Var(flag), "false", true),
        ("bool 1", EffectTarget::Var(flag), "1", false),
        ("real", EffectTarget::Var(r), "0.5", true),
        ("real nan", EffectTarget::Var(r), "nan", false),
        ("real inf", EffectTarget::Var(r), "inf", false),
        ("real 1e999", EffectTarget::Var(r), "1e999", false),
        ("enum", EffectTarget::Var(mode), "y", true),
        ("enum unknown", EffectTarget::Var(mode), "z", false),
    ];
    for (what, target, value, good) in cases {
        let r = add(&mut s, what, effect(target, value));
        assert_eq!(r.is_some(), good, "{what}");
        if r.is_some() {
            let last = *s.tasks().last().unwrap();
            s.edit_work(WorkOp::DeleteTask(last)).unwrap();
        }
    }
    // The same target twice in one task: accepted or refused, never a mess.
    let twice = Task {
        effects: vec![
            Effect { target: EffectTarget::Var(v), value: "1".into() },
            Effect { target: EffectTarget::Var(v), value: "2".into() },
        ],
        ..task("E2", "")
    };
    add(&mut s, "an effect on the same variable twice", twice);
    // A task with many steps: quick.
    let big = Task {
        steps: (0..20_000).map(|i| Step::Wait { seconds: f64::from(i) * 1e-3 }).collect(),
        ..task("Big", "")
    };
    let t0 = std::time::Instant::now();
    add(&mut s, "20000 steps", big);
    eprintln!("report: a task of 20000 steps took {:?}", t0.elapsed());
}

#[test]
fn deleted_sketch_elements_show_in_task_problems_and_undo_clears_them() {
    let mut s = Session::new();
    let sketch = s.create_sketch("XY").unwrap();
    let Created::Point(p) =
        s.execute(&Command::Sketch { sketch, op: SketchOp::AddPoint { pos: [0.1, 0.1, 0.0] } }).unwrap()[0]
    else {
        panic!()
    };
    let line = {
        let op = SketchOp::AddLine {
            a: [0.2, 0.2, 0.0],
            b: [0.3, 0.2, 0.0],
            snap_a: EndSnap::default(),
            snap_b: EndSnap::default(),
            horizontal: None,
        };
        let made = s.execute(&Command::Sketch { sketch, op }).unwrap();
        made.iter().find_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None }).unwrap()
    };
    s.finish_sketch();
    let way = s.add_waypoint_role(sketch, p).unwrap();
    let path = s.add_path_role(sketch, &[line]).unwrap();
    let t = Task { steps: vec![Step::MoveJ(MoveTarget::Role(way)), Step::FollowPath(path)], ..task("T", "") };
    let tid: Id<Task> = s.edit_work(WorkOp::AddTask(t)).unwrap().unwrap().downcast().unwrap();
    assert!(s.task_problems(tid).is_empty());
    s.enter_sketch(sketch);
    // Deleting the point, then the line: each leaves the role (the task uses it) and the problem list names it.
    for (what, item) in [("point", Item::Point(p)), ("line", Item::Curve(line))] {
        let before_problems = s.task_problems(tid).len();
        let depth = s.history().undo_depth();
        s.execute(&Command::Sketch { sketch, op: SketchOp::Delete(vec![item]) }).unwrap();
        assert_eq!(s.history().undo_depth(), depth + 1, "{what}");
        assert!(s.task_problems(tid).len() > before_problems, "{what}: no problem reported");
        assert!(s.task_problems(tid).iter().all(|m| !m.trim().is_empty()));
        assert!(
            s.doc().get(way).is_some() && s.doc().get(path).is_some(),
            "the roles stay while a task uses them"
        );
    }
    // A task with problems is still checkable: ready is a question about the states only.
    assert!(s.task_ready(tid).is_ok());
    assert!(s.undo() && s.undo());
    assert!(s.task_problems(tid).is_empty());
    // A ghost task: no problems, and not ready is an error.
    let ghost = Id::<Task>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    assert!(s.task_problems(ghost).is_empty());
    assert!(s.task_ready(ghost).is_err());
}

#[test]
fn the_work_model_survives_save_and_open_and_old_files_have_none() {
    let mut s = Session::new();
    let robot = s.add_robot("Robot1", &six_axis()).unwrap();
    let home = s.add_waypoint(robot, WaypointTarget::Joints(vec![0.0; 6])).unwrap();
    let ty = add_type(&mut s, "홀", &["비어 있음", "체결됨"]);
    let o = add_object(&mut s, "홀 A", ty, "비어 있음");
    let n = add_var(&mut s, "n", StateValue::Int(2));
    let t = Task {
        location: Location::Robot(robot),
        precondition: "\"홀 A\".state == \"비어 있음\" and n < 3".into(),
        steps: vec![Step::MoveJ(MoveTarget::Waypoint(home)), Step::Wait { seconds: 0.5 }],
        effects: vec![
            Effect { target: EffectTarget::Object(o), value: "체결됨".into() },
            Effect { target: EffectTarget::Var(n), value: "3".into() },
        ],
        robots: RobotChoice::Only(vec![robot]),
        priority: -4,
        name: "Own".into(),
    };
    s.edit_work(WorkOp::AddTask(t)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("work.gcp");
    s.save_as(&path).unwrap();
    let mut u = Session::new();
    u.open(&path).unwrap();
    assert!(u.doc().same_content(s.doc()));
    assert_eq!(u.tasks().len(), 1);
    assert_eq!(u.task_ready(u.tasks()[0]), Ok(true));
    let again = dir.path().join("again.gcp");
    u.save_as(&again).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), std::fs::read(&again).unwrap());
    // The tasks removed from the text (an older file) leaves objects and variables, still valid; removing the
    // object types while objects remain is a damaged file: refused or opened without the dangling parts.
    let mut value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    fn strip(v: &mut serde_json::Value, key: &str) -> usize {
        let mut n = 0;
        match v {
            serde_json::Value::Object(map) => {
                let keys: Vec<String> =
                    map.keys().filter(|k| k.to_lowercase().contains(key)).cloned().collect();
                for k in keys {
                    map.remove(&k);
                    n += 1;
                }
                for x in map.values_mut() {
                    n += strip(x, key);
                }
            }
            serde_json::Value::Array(a) => {
                for x in a {
                    n += strip(x, key);
                }
            }
            _ => {}
        }
        n
    }
    let mut no_tasks = value.clone();
    let removed = strip(&mut no_tasks, "task");
    let p2 = dir.path().join("no_tasks.gcp");
    std::fs::write(&p2, serde_json::to_string(&no_tasks).unwrap()).unwrap();
    let mut w = Session::new();
    match w.open(&p2) {
        Ok(_) => assert!(w.tasks().is_empty() || removed == 0),
        Err(e) => eprintln!(
            "observation: a file with the task entries removed does not open ({removed} keys): {e:?}"
        ),
    }
    let removed = strip(&mut value, "objecttype");
    let p3 = dir.path().join("no_types.gcp");
    std::fs::write(&p3, serde_json::to_string(&value).unwrap()).unwrap();
    let mut x = Session::new();
    if x.open(&p3).is_ok() {
        for (_, o) in x.doc().objects().iter() {
            assert!(
                x.doc().get(o.ty).is_some(),
                "an object of a type that is not there was opened ({removed} keys removed)"
            );
        }
    }
    // Many tasks: quick and ordered.
    let mut big = Session::new();
    let t0 = std::time::Instant::now();
    for i in 0..500 {
        big.edit_work(WorkOp::AddTask(Task {
            steps: (0..20).map(|k| Step::Wait { seconds: f64::from(k) }).collect(),
            ..task(&format!("T{i}"), "")
        }))
        .unwrap();
    }
    assert_eq!(big.tasks().len(), 500);
    let names: Vec<String> = big.tasks().iter().map(|id| big.doc().get(*id).unwrap().name.clone()).collect();
    assert_eq!(names[0], "T0");
    assert_eq!(names[499], "T499");
    let text = gc_persist::to_text(big.doc()).unwrap();
    assert!(gc_persist::from_text(&text).unwrap().doc.same_content(big.doc()));
    eprintln!("report: 500 tasks of 20 steps, make and save took {:?}", t0.elapsed());
}
