//! QA boundary tests (kittest) for the work model screens (plan 9.4, GraphCTRL's own): the browser's "작업" node
//! (new, rename, delete, refusals), the task editor (steps that fit, reorder, remove, effects, condition errors),
//! the problems a task shows, and "이 역할로 작업 만들기". A screen action that cannot apply leaves the document and
//! the undo depth as they were; every one that applies is one undo step.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use eframe::egui;
use egui_kittest::kittest::Queryable as _;
use gc_app::{GraphCtrlApp, RunOptions};
use gc_doc::{
    EditError, Effect, EffectTarget, Location, MoveTarget, ObjectType, RobotChoice, SketchOp, StateValue,
    StateVar, Step, Task, WorkObject, WorkOp,
};

const SIZE: egui::Vec2 = egui::vec2(1280.0, 800.0);

type H = egui_kittest::Harness<'static, GraphCtrlApp>;

fn harness() -> H {
    let mut h = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .with_step_dt(0.05)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    h.run();
    h
}

fn depth(h: &H) -> usize {
    h.state().session.history().undo_depth()
}

fn press(h: &mut H, at: egui::Pos2, button: egui::PointerButton) {
    h.event(egui::Event::PointerMoved(at));
    h.run_steps(2);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: at, button, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.run_steps(3);
}

/// The centre of the left-hand (browser) node that has this label.
fn left(h: &H, label: &str) -> egui::Pos2 {
    h.get_all_by_label_contains(label)
        .find(|n| n.rect().min.x < 300.0)
        .unwrap_or_else(|| panic!("no browser row {label:?}"))
        .rect()
        .center()
}

fn open_work_node(h: &mut H) {
    let at = h.get_all_by_label("작업").find(|n| n.rect().min.x < 300.0).unwrap().rect().center();
    press(h, at, egui::PointerButton::Primary);
}

fn group_menu_new(h: &mut H, group: &str) {
    let at = left(h, group);
    press(h, at, egui::PointerButton::Secondary);
    if h.query_all_by_label("새로 만들기").next().is_some() {
        h.query_all_by_label("새로 만들기").last().unwrap().click();
        h.run_steps(3);
    }
}

fn escape(h: &mut H) {
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
}

fn task(name: &str) -> Task {
    Task {
        name: name.into(),
        location: Location::None,
        precondition: String::new(),
        steps: Vec::new(),
        effects: Vec::new(),
        robots: RobotChoice::Any,
        priority: 0,
    }
}

#[test]
fn the_work_node_makes_items_with_free_names_one_step_each() {
    let mut h = harness();
    open_work_node(&mut h);
    let steps = depth(&h);
    // Tasks: two in a row get different names; each is one undo step.
    group_menu_new(&mut h, "작업 목록");
    group_menu_new(&mut h, "작업 목록");
    let names: Vec<String> = h.state().session.doc().tasks().iter().map(|(_, t)| t.name.clone()).collect();
    assert_eq!(names.len(), 2);
    assert_ne!(names[0], names[1], "two new tasks with one name");
    assert_eq!(depth(&h), steps + 2);
    // A work object needs a type: with none the menu item does nothing and the document is untouched.
    let before = depth(&h);
    group_menu_new(&mut h, "작업 객체");
    escape(&mut h);
    assert!(h.state().session.doc().objects().is_empty(), "an object with no type");
    assert_eq!(depth(&h), before);
    // Types, then objects of the first type, then variables.
    group_menu_new(&mut h, "객체 유형");
    assert_eq!(h.state().session.doc().object_types().len(), 1);
    group_menu_new(&mut h, "작업 객체");
    group_menu_new(&mut h, "작업 객체");
    let objects: Vec<String> =
        h.state().session.doc().objects().iter().map(|(_, o)| o.name.clone()).collect();
    assert_eq!(objects.len(), 2);
    assert_ne!(objects[0], objects[1]);
    group_menu_new(&mut h, "변수");
    group_menu_new(&mut h, "변수");
    assert_eq!(h.state().session.doc().state_vars().len(), 2);
    // Everything made is undone one step at a time, exactly.
    let total = depth(&h) - steps;
    for _ in 0..total {
        assert!(h.state_mut().session.undo());
    }
    assert_eq!(depth(&h), steps);
    assert!(h.state().session.doc().tasks().is_empty() && h.state().session.doc().objects().is_empty());
}

#[test]
fn renaming_in_the_browser_accepts_good_names_and_refuses_the_rest() {
    let mut h = harness();
    let ty = h
        .state_mut()
        .session
        .edit_work(WorkOp::AddObjectType(ObjectType {
            name: "홀".into(),
            states: vec!["a".into(), "b".into()],
        }))
        .unwrap()
        .unwrap()
        .downcast()
        .unwrap();
    let mk = |h: &mut H, name: &str| {
        h.state_mut()
            .session
            .edit_work(WorkOp::AddObject(WorkObject {
                name: name.into(),
                ty,
                location: Location::None,
                state: "a".into(),
            }))
            .unwrap()
            .unwrap()
    };
    let a = mk(&mut h, "Alpha");
    let _b = mk(&mut h, "Beta");
    h.run_steps(3);
    open_work_node(&mut h);
    let name_of =
        |h: &H| h.state().session.doc().get(a.downcast::<WorkObject>().unwrap()).unwrap().name.clone();
    let input = egui::accesskit::Role::TextInput;
    let rename = |h: &mut H, typed: &str| {
        // Select, then a second click edits the name.
        let at = left(h, &name_of(h));
        press(h, at, egui::PointerButton::Primary);
        let at = left(h, &name_of(h));
        press(h, at, egui::PointerButton::Primary);
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        if typed.is_empty() {
            h.key_press(egui::Key::Backspace);
        } else if let Some(field) = h.query_all_by_role(input).find(|n| n.rect().min.x < 300.0) {
            field.type_text(typed);
        }
        h.key_press(egui::Key::Enter);
        h.run_steps(4);
    };
    // A good name: one step.
    let steps = depth(&h);
    rename(&mut h, "Gamma");
    assert_eq!(name_of(&h), "Gamma");
    assert_eq!(depth(&h), steps + 1);
    // Names that cannot be: taken, empty, with a quote: the name stays, no step.
    for bad in ["Beta", "", "a\"b"] {
        let before = depth(&h);
        rename(&mut h, bad);
        assert_eq!(name_of(&h), "Gamma", "{bad:?} was taken");
        assert_eq!(depth(&h), before, "{bad:?}");
    }
    // Esc leaves the name as it was.
    let before = depth(&h);
    let at = left(&h, "Gamma");
    press(&mut h, at, egui::PointerButton::Primary);
    let at = left(&h, "Gamma");
    press(&mut h, at, egui::PointerButton::Primary);
    escape(&mut h);
    assert_eq!(name_of(&h), "Gamma");
    assert_eq!(depth(&h), before);
}

#[test]
fn what_is_referred_to_cannot_be_deleted_from_the_browser() {
    let mut h = harness();
    let ty = h
        .state_mut()
        .session
        .edit_work(WorkOp::AddObjectType(ObjectType {
            name: "Hole".into(),
            states: vec!["a".into(), "b".into()],
        }))
        .unwrap()
        .unwrap()
        .downcast()
        .unwrap();
    let o = h
        .state_mut()
        .session
        .edit_work(WorkOp::AddObject(WorkObject {
            name: "O".into(),
            ty,
            location: Location::None,
            state: "a".into(),
        }))
        .unwrap()
        .unwrap()
        .downcast()
        .unwrap();
    let t = Task {
        precondition: "O.state == a".into(),
        effects: vec![Effect { target: EffectTarget::Object(o), value: "b".into() }],
        ..task("Use")
    };
    h.state_mut().session.edit_work(WorkOp::AddTask(t)).unwrap();
    h.run_steps(3);
    open_work_node(&mut h);
    let steps = depth(&h);
    // The delete buttons of the type and of the object are refused (the task needs them); the task row goes.
    for label in ["Hole", "O"] {
        let delete = format!("{label} 삭제");
        let present = h.query_all_by_label(&delete).next().is_some();
        if present {
            h.get_by_label(&delete).click();
            h.run_steps(3);
        }
    }
    assert_eq!(h.state().session.doc().object_types().len(), 1, "the type of an object in use was deleted");
    assert_eq!(h.state().session.doc().objects().len(), 1, "an object a task refers to was deleted");
    assert_eq!(depth(&h), steps, "a refused delete left a step");
    // The same through the session: InUse.
    assert_eq!(h.state_mut().session.edit_work(WorkOp::DeleteObject(o)), Err(EditError::InUse));
}

/// How many steps of a task have a combo box of their own in the editor (moves, follow and tool actions).
fn list_steps(h: &H, tid: gc_core::Id<Task>) -> usize {
    h.state()
        .session
        .doc()
        .get(tid)
        .unwrap()
        .steps
        .iter()
        .map(|s| match s {
            Step::MoveJ(_) | Step::MoveL(_) | Step::FollowPath(_) | Step::ToolAction { .. } => 1,
            _ => 0,
        })
        .sum()
}

#[test]
fn the_task_editor_offers_only_steps_that_fit_and_edits_one_step_at_a_time() {
    let mut h = harness();
    let tid: gc_core::Id<Task> = h
        .state_mut()
        .session
        .edit_work(WorkOp::AddTask(task("Fasten")))
        .unwrap()
        .unwrap()
        .downcast()
        .unwrap();
    h.run_steps(3);
    open_work_node(&mut h);
    let at = left(&h, "Fasten");
    press(&mut h, at, egui::PointerButton::Primary);
    h.get_by_label("시작 조건");
    // The add-step list is the last combo box of the steps section: the combo boxes of the editor in order
    // are the place, one per step, then the add-step list.
    let open_add = |h: &mut H| {
        let combos = |h: &H| {
            let mut v: Vec<egui::Pos2> = h
                .query_all_by_role(egui::accesskit::Role::ComboBox)
                .filter(|n| n.rect().min.x > 700.0)
                .map(|n| n.rect().center())
                .collect();
            v.sort_by(|a, b| a.y.total_cmp(&b.y));
            v
        };
        let steps_count = h.state().session.doc().get(tid).unwrap().steps.len();
        // place + one per step that has a list of its own + the add list; the add list is the first one below
        // the last step row: take the one after the place and the steps that show a list.
        let all = combos(h);
        let at = *all
            .get(1 + list_steps(h, tid))
            .unwrap_or_else(|| panic!("no add-step list among {all:?} ({steps_count} steps)"));
        press(h, at, egui::PointerButton::Primary);
    };
    // No roles yet: no moves and no follow in the list; the others are there.
    open_add(&mut h);
    assert!(h.query_all_by_label("관절 이동").next().is_none(), "MoveJ offered with no waypoint role");
    assert!(h.query_all_by_label("직선 이동").next().is_none());
    assert!(h.query_all_by_label("경로 추종").next().is_none(), "Follow offered with no path role");
    for kind in ["출력 설정", "입력 대기", "툴 동작", "대기"] {
        assert!(h.query_all_by_label(kind).next().is_some(), "{kind} not offered");
    }
    let steps = depth(&h);
    h.query_all_by_label("대기").last().unwrap().click();
    h.run_steps(3);
    assert_eq!(h.state().session.doc().get(tid).unwrap().steps.len(), 1);
    assert_eq!(depth(&h), steps + 1);
    // Add three more kinds, then reorder and remove: each one step.
    for kind in ["출력 설정", "툴 동작"] {
        open_add(&mut h);
        h.query_all_by_label(kind).last().unwrap().click();
        h.run_steps(3);
    }
    let n = h.state().session.doc().get(tid).unwrap().steps.len();
    assert_eq!(n, 3);
    assert_eq!(depth(&h), steps + 3);
    let before: Vec<Step> = h.state().session.doc().get(tid).unwrap().steps.clone();
    // ↑ on the last row swaps it with the one above; the first row's ↑ is disabled.
    let ups: Vec<_> = h.query_all_by_label("↑").collect();
    assert_eq!(ups.len(), 3);
    ups[2].click();
    h.run_steps(3);
    let after = h.state().session.doc().get(tid).unwrap().steps.clone();
    assert_eq!(after, vec![before[0].clone(), before[2].clone(), before[1].clone()]);
    assert_eq!(depth(&h), steps + 4);
    let first_up = h.query_all_by_label("↑").next().unwrap();
    first_up.click();
    h.run_steps(3);
    assert_eq!(h.state().session.doc().get(tid).unwrap().steps, after, "the top row moved up");
    assert_eq!(depth(&h), steps + 4, "a disabled button made a step");
    // ✕ removes a row, one step; undoing all of it restores the empty task exactly.
    h.query_all_by_label("✕").next().unwrap().click();
    h.run_steps(3);
    assert_eq!(h.state().session.doc().get(tid).unwrap().steps.len(), 2);
    for _ in 0..depth(&h) - steps {
        assert!(h.state_mut().session.undo());
    }
    assert!(h.state().session.doc().get(tid).unwrap().steps.is_empty());
}

/// BUG: "효과 추가" on a task adds an effect with the value `true` for a variable, whatever the variable's kind; an
/// integer, real or choice variable refuses it (the model checks the value by kind), so with no work object and an
/// integer variable first in the list the effect can never be added from the editor.
#[test]
fn add_effect_works_for_an_integer_variable() {
    let mut h = harness();
    h.state_mut()
        .session
        .edit_work(WorkOp::AddStateVar(StateVar { name: "count".into(), value: StateValue::Int(0) }))
        .unwrap();
    let tid: gc_core::Id<Task> =
        h.state_mut().session.edit_work(WorkOp::AddTask(task("Count"))).unwrap().unwrap().downcast().unwrap();
    h.run_steps(3);
    open_work_node(&mut h);
    let at = left(&h, "Count");
    press(&mut h, at, egui::PointerButton::Primary);
    h.get_by_label("효과 추가").click();
    h.run_steps(3);
    assert_eq!(
        h.state().session.doc().get(tid).unwrap().effects.len(),
        1,
        "no effect was added for an integer variable"
    );
}

#[test]
fn a_task_shows_its_problems_and_a_bad_condition_is_explained_not_stored() {
    let mut h = harness();
    // A task that moves to a waypoint role of a sketch point.
    h.get_by_label("XY 스케치").click();
    h.run();
    h.state_mut().session.set_tool(gc_tools::ToolKind::Point);
    h.state_mut().session.tool_event(gc_tools::ToolEvent::Move {
        pos: [0.01, 0.01],
        px: 1e-4,
        suppress_snap: true,
    });
    h.state_mut().session.tool_event(gc_tools::ToolEvent::Click {
        pos: [0.01, 0.01],
        px: 1e-4,
        modifier: false,
    });
    h.state_mut().session.set_tool(gc_tools::ToolKind::Select);
    let (sketch, point) = {
        let (id, s) = h.state().session.active_sketch().unwrap();
        (
            id,
            *s.points
                .keys()
                .find(|p| h.state().session.point_pos(**p).is_some_and(|q| (q[0] - 0.01).abs() < 1e-9))
                .unwrap(),
        )
    };
    let role = h.state_mut().session.add_waypoint_role(sketch, point).unwrap();
    h.state_mut().session.finish_sketch();
    let tid: gc_core::Id<Task> = h
        .state_mut()
        .session
        .edit_work(WorkOp::AddTask(Task { steps: vec![Step::MoveJ(MoveTarget::Role(role))], ..task("Move") }))
        .unwrap()
        .unwrap()
        .downcast()
        .unwrap();
    h.run_steps(3);
    open_work_node(&mut h);
    let at = left(&h, "Move");
    press(&mut h, at, egui::PointerButton::Primary);
    assert!(h.state().session.task_problems(tid).is_empty());
    // Delete the sketch point from the document: the role stays, the editor lists the problem in words.
    h.state_mut().session.enter_sketch(sketch);
    h.state_mut()
        .session
        .execute(&gc_doc::Command::Sketch {
            sketch,
            op: SketchOp::Delete(vec![gc_sketch::Item::Point(point)]),
        })
        .unwrap();
    h.state_mut().session.finish_sketch();
    h.run_steps(4);
    let problems = h.state().session.task_problems(tid);
    assert!(!problems.is_empty());
    for p in &problems {
        assert!(h.get_all_by_label_contains(p).next().is_some(), "the problem {p:?} is not shown");
    }
    // A bad condition typed in the box is explained below it and not stored.
    let input = egui::accesskit::Role::TextInput;
    let boxes: Vec<_> = h.query_all_by_role(input).filter(|n| n.rect().min.x > 700.0).collect();
    assert!(!boxes.is_empty(), "no text boxes in the editor");
    let first = boxes.into_iter().min_by(|a, b| a.rect().min.y.total_cmp(&b.rect().min.y)).unwrap();
    first.click();
    h.run_steps(2);
    let stored = h.state().session.doc().get(tid).unwrap().precondition.clone();
    h.query_all_by_role(input)
        .filter(|n| n.rect().min.x > 700.0)
        .min_by(|a, b| a.rect().min.y.total_cmp(&b.rect().min.y))
        .unwrap()
        .type_text("count <");
    h.run_steps(3);
    assert!(h.get_all_by_label_contains("«").next().is_some(), "no explanation of the mistake");
    h.key_press(egui::Key::Tab);
    h.run_steps(3);
    assert_eq!(h.state().session.doc().get(tid).unwrap().precondition, stored, "a bad condition was stored");
}

#[test]
fn a_roles_menu_makes_a_task_from_it_each_time_with_a_new_name() {
    let mut h = harness();
    h.get_by_label("XY 스케치").click();
    h.run();
    let app = h.state_mut();
    app.session.set_tool(gc_tools::ToolKind::Point);
    for p in [[0.01, 0.01], [0.03, 0.01]] {
        app.session.tool_event(gc_tools::ToolEvent::Move { pos: p, px: 1e-4, suppress_snap: true });
        app.session.tool_event(gc_tools::ToolEvent::Click { pos: p, px: 1e-4, modifier: false });
    }
    app.session.set_tool(gc_tools::ToolKind::Select);
    let sketch = app.session.active_sketch().unwrap().0;
    let point = *app.session.active_sketch().unwrap().1.points.keys().next().unwrap();
    let role = app.session.add_waypoint_role(sketch, point).unwrap();
    app.session.finish_sketch();
    h.run_steps(4);
    let name = h.state().session.doc().get(role).unwrap().name.clone();
    let steps = depth(&h);
    for round in 1..=2 {
        let at = left(&h, &name);
        press(&mut h, at, egui::PointerButton::Secondary);
        h.get_by_label("이 역할로 작업 만들기").click();
        h.run_steps(3);
        assert_eq!(h.state().session.doc().tasks().len(), round);
        assert_eq!(depth(&h), steps + round, "one step per task");
    }
    let names: Vec<String> = h.state().session.doc().tasks().iter().map(|(_, t)| t.name.clone()).collect();
    assert_ne!(names[0], names[1], "two tasks with one name");
    for (_, t) in h.state().session.doc().tasks().iter() {
        assert_eq!(t.steps, vec![Step::MoveJ(MoveTarget::Role(role))]);
    }
    // The role is now used: deleting it from the browser is refused.
    assert_eq!(h.state_mut().session.delete_role(role), Err(EditError::InUse));
}
