//! QA boundary tests (kittest) for the component row menu's Ground and Root (Inventor 2026 help, To Ground and Root /
//! Ground at Origin, GUID-98FA88B2-AA32-4A27-946A-DCECB36A6696) and for the signal rows of the schedule's Gantt
//! chart (plan 9.5, GraphCTRL's own). A screen action that cannot apply leaves the document and the undo depth as
//! they were; one that applies is one undo step that restores exactly.

#![allow(clippy::unwrap_used, clippy::panic, clippy::float_cmp, reason = "test code")]

use eframe::egui;
use egui_kittest::kittest::Queryable as _;
use gc_app::{GraphCtrlApp, RunOptions};
use gc_doc::{ComponentFlag, Occurrence, Placement, Solid, Step};

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

fn secondary(h: &mut H, at: egui::Pos2) {
    h.event(egui::Event::PointerMoved(at));
    h.run_steps(2);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.run_steps(3);
}

fn has(h: &H, label: &str) -> bool {
    h.query_all_by_label(label).next().is_some()
}

fn row_of(h: &H, name: &str) -> egui::Pos2 {
    h.get_all_by_label_contains(name)
        .find(|n| n.rect().min.x < 100.0)
        .unwrap_or_else(|| panic!("no browser row {name:?}"))
        .rect()
        .center()
}

fn grounded(h: &H, a: gc_core::Id<gc_doc::Body>) -> bool {
    h.state().session.doc().component_flag(Occurrence::Item(a.erase()), ComponentFlag::Grounded)
}

const GROUND: &str = "원점에 고정 (Ground and Root)";

#[test]
fn ground_and_root_from_the_row_menu_is_one_step_and_exact_to_undo() {
    let mut h = harness();
    let a = h
        .state_mut()
        .session
        .add_body(Solid::Box { size: [0.1; 3] }, Placement { xyz: [0.5, 0.2, 0.3], rpy: [0.3, -0.4, 1.1] })
        .unwrap();
    h.run();
    let name = h.state().session.doc().get(a).unwrap().name.clone();
    let before = h.state().session.doc().clone();
    let steps = depth(&h);
    // The menu opens and closes without changing anything; Place at Component Origin is listed (not pressed: it asks
    // for a file).
    let at = row_of(&h, &name);
    secondary(&mut h, at);
    assert!(has(&h, GROUND) && has(&h, "이 컴포넌트의 원점에 배치…"));
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert!(h.state().session.doc().same_content(&before));
    assert_eq!(depth(&h), steps);
    // Ground and Root: at the origin, aligned, grounded; one step; undo restores the pose and the flag exactly.
    let at = row_of(&h, &name);
    secondary(&mut h, at);
    h.get_by_label(GROUND).click();
    h.run_steps(3);
    let body = h.state().session.doc().get(a).unwrap().clone();
    assert!(
        body.placement.xyz.iter().chain(&body.placement.rpy).all(|v| v.abs() < 1e-9),
        "{:?}",
        body.placement
    );
    assert!(grounded(&h, a));
    assert_eq!(depth(&h), steps + 1);
    // Again from the menu: the document stays as it is (a harmless repeat), at most one more step.
    let at = row_of(&h, &name);
    secondary(&mut h, at);
    let after = h.state().session.doc().clone();
    if has(&h, GROUND) {
        h.get_by_label(GROUND).click();
        h.run_steps(3);
    }
    assert!(h.state().session.doc().same_content(&after), "a second Ground and Root changed the document");
    let extra = depth(&h) - (steps + 1);
    assert!(extra <= 1);
    for _ in 0..extra + 1 {
        assert!(h.state_mut().session.undo());
    }
    assert!(
        h.state().session.doc().same_content(&before),
        "undo does not restore the placement and the flag"
    );
    assert!(h.state_mut().session.redo());
    assert!(grounded(&h, a));
}

#[test]
fn ground_and_root_takes_robots_and_several_at_once_and_refuses_what_is_not_a_component() {
    let mut h = harness();
    let (a, b) = {
        let s = &mut h.state_mut().session;
        (
            s.add_body(Solid::Box { size: [0.1; 3] }, Placement { xyz: [1.0, 0.0, 0.0], rpy: [0.0; 3] })
                .unwrap(),
            s.add_body(Solid::Sphere { radius: 0.05 }, Placement { xyz: [0.0, 1.0, 0.0], rpy: [0.0; 3] })
                .unwrap(),
        )
    };
    let urdf = std::fs::read_to_string(format!(
        "{}/../../../fixtures/urdf/six_axis.urdf",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let robot = h.state_mut().session.add_robot("Robot1", &urdf).unwrap();
    h.state_mut()
        .session
        .execute(&gc_doc::Command::Cell(gc_doc::CellOp::PlaceRobot {
            id: robot,
            placement: Placement { xyz: [0.0, 0.0, 2.0], rpy: [0.0, 0.0, 0.5] },
        }))
        .unwrap();
    h.run();
    let before = h.state().session.doc().clone();
    let steps = depth(&h);
    // Several at once: one undo step; every one goes to the origin (they overlap there: the request is not
    // refused, which is as Inventor's Ground at Origin is).
    h.state_mut().session.ground_at_origin(&[a.erase(), b.erase(), robot.erase()]).unwrap();
    assert_eq!(depth(&h), steps + 1);
    for id in [a, b] {
        let body = h.state().session.doc().get(id).unwrap();
        assert!(body.placement.xyz.iter().all(|v| v.abs() < 1e-9));
        assert!(grounded(&h, id));
    }
    let placed = h.state().session.doc().get(robot).unwrap().placement;
    assert!(placed.xyz.iter().chain(&placed.rpy).all(|v| v.abs() < 1e-9), "{placed:?}");
    h.run_steps(3);
    assert!(h.state_mut().session.undo());
    assert!(h.state().session.doc().same_content(&before));
    // Not a component, nothing, a ghost: refused with no trace.
    let sketch = h.state_mut().session.create_sketch("XY").unwrap();
    h.state_mut().session.finish_sketch();
    let ghost = gc_core::Id::<gc_doc::Body>::from_raw(std::num::NonZeroU64::new(9999).unwrap()).erase();
    let before = h.state().session.doc().clone();
    let steps = depth(&h);
    for (what, items) in [
        ("a sketch", vec![sketch.erase()]),
        ("nothing", Vec::new()),
        ("a ghost", vec![ghost]),
        ("a real one and a ghost", vec![a.erase(), ghost]),
    ] {
        assert!(h.state_mut().session.ground_at_origin(&items).is_err(), "{what}");
        assert!(h.state().session.doc().same_content(&before), "{what} changed the document");
        assert_eq!(depth(&h), steps, "{what}");
    }
}

#[test]
fn ground_and_root_on_a_constrained_component_keeps_the_assembly_solvable() {
    let mut h = harness();
    let (fixed, moving) = {
        let s = &mut h.state_mut().session;
        (
            s.add_body(Solid::Box { size: [0.1; 3] }, Placement { xyz: [0.0, 0.0, 0.0], rpy: [0.0; 3] })
                .unwrap(),
            s.add_body(Solid::Box { size: [0.1; 3] }, Placement { xyz: [0.0, 0.0, 0.4], rpy: [0.0; 3] })
                .unwrap(),
        )
    };
    let plane = gc_doc::Geometry::origin(gc_doc::OriginElement::Plane(gc_doc::OriginPlane::Xy));
    let pick = |body: gc_core::Id<gc_doc::Body>| gc_doc::Selection {
        occurrence: Occurrence::Item(body.erase()),
        geometry: plane,
    };
    h.state_mut()
        .session
        .add_relationship(gc_doc::Relationship::new(
            "Mate:1",
            gc_doc::RelationKind::Mate { offset: 0.01, sense: gc_doc::AxisSense::Opposed },
            pick(moving),
            pick(fixed),
        ))
        .unwrap();
    h.run();
    let before = h.state().session.doc().clone();
    let steps = depth(&h);
    // The moving one is held 10 mm above the fixed one by a Mate; grounding it at the origin fights that: refused with
    // no trace, or accepted and the assembly still solves.
    let r = h.state_mut().session.ground_at_origin(&[moving.erase()]);
    match r {
        Err(_) => {
            assert!(h.state().session.doc().same_content(&before));
            assert_eq!(depth(&h), steps);
        }
        Ok(()) => {
            assert_eq!(depth(&h), steps + 1);
            let body = h.state().session.doc().get(moving).unwrap();
            assert!(body.placement.xyz.iter().all(|v| v.is_finite()));
            assert!(h.state().session.component_dof(moving.erase()).is_some());
            assert!(h.state_mut().session.undo());
            assert!(h.state().session.doc().same_content(&before));
        }
    }
    h.run_steps(3);
}

// ---- Gantt signal rows ----

fn with_robots(h: &mut H, count: usize) {
    let urdf = std::fs::read_to_string(format!(
        "{}/../../../fixtures/urdf/six_axis.urdf",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    for i in 0..count {
        h.state_mut().session.add_robot(&format!("Robot{}", i + 1), &urdf).unwrap();
    }
    h.run();
}

fn signal_task(name: &str, steps: Vec<Step>) -> gc_doc::Task {
    gc_doc::Task {
        name: name.into(),
        location: gc_doc::Location::None,
        precondition: String::new(),
        steps,
        effects: Vec::new(),
        robots: gc_doc::RobotChoice::Any,
        priority: 0,
    }
}

fn open_schedule_and_compute(h: &mut H) {
    h.get_by_label("도구").click();
    h.run();
    h.get_by_label("스케줄").click();
    h.run_steps(3);
    h.get_by_label("스케줄 계산").click();
    h.run_steps(6);
}

#[test]
fn signal_rows_are_one_per_distinct_name_and_follow_their_changes() {
    let mut h = harness();
    with_robots(&mut h, 2);
    let set = |name: &str, value: bool| Step::SetIo { name: name.into(), value };
    for (name, steps) in [
        ("First", vec![set("door_open", true), Step::Wait { seconds: 1.0 }, set("door_open", false)]),
        ("Second", vec![set("door_open", true), set("한글 신호", true), Step::Wait { seconds: 0.5 }]),
        ("Third", vec![set("never off", true), Step::Wait { seconds: 2.0 }]),
    ] {
        h.state_mut().session.edit_work(gc_doc::WorkOp::AddTask(signal_task(name, steps))).unwrap();
    }
    open_schedule_and_compute(&mut h);
    let (schedule, current) = h.state().session.schedule_result().map(|(s, c)| (s.clone(), c)).unwrap();
    assert!(current);
    let end = schedule.makespan();
    // The changes as the schedule records them: finite, inside the schedule, and each signal's last value is the
    // last one a task set.
    for (at, name, _) in &schedule.signals {
        assert!(at.is_finite() && *at >= 0.0 && *at <= end + 1e-9, "{name} changes at {at}, makespan {end}");
    }
    let last = |name: &str| {
        schedule
            .signals
            .iter()
            .filter(|(_, n, _)| n == name)
            .max_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, _, v)| *v)
    };
    assert_eq!(last("never off"), Some(true));
    // One row per distinct name, whatever the number of changes; a name with spaces and one in Korean are rows too.
    for name in ["door_open", "한글 신호", "never off"] {
        assert_eq!(h.query_all_by_label(name).count(), 1, "{name} must be one row");
    }
    // The schedule gone stale (an edit) keeps the rows; closing and reopening shows them again after computing.
    h.state_mut()
        .session
        .edit_work(gc_doc::WorkOp::AddTask(signal_task("Fourth", vec![set("late", true)])))
        .unwrap();
    h.run_steps(3);
    assert!(has(&h, "door_open"), "stale rows vanished");
    assert!(!has(&h, "late"), "a signal of a task not in the schedule yet is already a row");
    h.get_by_label("스케줄 계산").click();
    h.run_steps(6);
    assert!(has(&h, "late"));
}

#[test]
fn no_signals_no_rows_and_a_great_many_do_not_hang_the_panel() {
    let mut h = harness();
    with_robots(&mut h, 1);
    h.state_mut()
        .session
        .edit_work(gc_doc::WorkOp::AddTask(signal_task("Quiet", vec![Step::Wait { seconds: 1.0 }])))
        .unwrap();
    open_schedule_and_compute(&mut h);
    assert!(h.state().session.schedule_result().is_some_and(|(s, _)| s.signals.is_empty()));
    assert!(!has(&h, "door_open"));
    // 300 signals of one task, names of every odd kind that is allowed, one that is very long.
    let mut steps: Vec<Step> =
        (0..300).map(|i| Step::SetIo { name: format!("sig{i}"), value: i % 2 == 0 }).collect();
    steps.push(Step::SetIo { name: "x".repeat(2000), value: true });
    steps.push(Step::SetIo { name: "line\nbreak".into(), value: true });
    steps.push(Step::Wait { seconds: 1.0 });
    h.state_mut().session.edit_work(gc_doc::WorkOp::AddTask(signal_task("Many", steps))).unwrap();
    let started = std::time::Instant::now();
    h.get_by_label("스케줄 계산").click();
    h.run_steps(8);
    let took = started.elapsed();
    assert!(took.as_secs() < 60, "the panel with 300 signals took {took:?}");
    let schedule = h.state().session.schedule_result().map(|(s, _)| s.clone()).unwrap();
    let mut names: Vec<&str> = schedule.signals.iter().map(|(_, n, _)| n.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    assert!(names.len() >= 300, "{} distinct signals", names.len());
    // Frames keep running (scrolling, a pointer over the rows) without a panic.
    h.event(egui::Event::PointerMoved(egui::pos2(400.0, 700.0)));
    h.run_steps(5);
}
