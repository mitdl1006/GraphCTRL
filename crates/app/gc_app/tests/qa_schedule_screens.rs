//! QA boundary tests (kittest) for the schedule panel (work it out, Play / Pause, Reset, the stale note, blocked tasks)
//! and for the numeric effect box of the task editor (an expression over the numeric variables, errors shown and not
//! stored) (plan 9.4/9.5, GraphCTRL's own). A screen action that cannot apply leaves the document and the undo
//! depth as they were; playback never changes the document.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use eframe::egui;
use egui_kittest::kittest::Queryable as _;
use gc_app::{GraphCtrlApp, RunOptions};
use gc_doc::{Effect, EffectTarget, Location, RobotChoice, StateValue, StateVar, Step, Task, WorkOp};

const SIZE: egui::Vec2 = egui::vec2(1280.0, 800.0);

type H = egui_kittest::Harness<'static, GraphCtrlApp>;

fn harness() -> H {
    let mut h = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .with_step_dt(0.05)
        .with_accessibility_check(false)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    h.run();
    h
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

fn with_robot(h: &mut H) {
    h.get_by_label("로봇").click();
    h.run();
    h.get_by_label("로봇 추가").click();
    h.run();
    h.get_by_label("UR5e 로봇").click();
    h.run();
}

fn open_schedule(h: &mut H) {
    h.get_by_label("도구").click();
    h.run();
    h.get_by_label("스케줄").click();
    h.run_steps(3);
}

fn has(h: &H, label: &str) -> bool {
    h.query_all_by_label(label).next().is_some()
}

#[test]
fn the_schedule_panel_with_nothing_to_do_and_with_a_task() {
    let mut h = harness();
    let revision = h.state().session.doc().revision();
    open_schedule(&mut h);
    // Not worked out yet: the note, the compute button, no play controls.
    assert!(has(&h, "스케줄을 아직 계산하지 않았습니다."));
    assert!(!has(&h, "스케줄 재생"));
    // Work out an empty cell: a makespan of zero, Play does nothing (it is disabled), no robot is shown moved.
    h.get_by_label("스케줄 계산").click();
    h.run_steps(4);
    assert!(h.state().session.schedule_result().is_some());
    if has(&h, "스케줄 재생") {
        h.get_by_label("스케줄 재생").click();
        h.run_steps(3);
        assert!(
            h.state().session.schedule_playback().is_none()
                || h.state().session.schedule_result().is_some_and(|(s, _)| s.makespan() > 0.0),
            "Play started on an empty schedule"
        );
    }
    // A cell with a robot and a task: play, pause, reset; the document never changes.
    with_robot(&mut h);
    h.state_mut()
        .session
        .edit_work(WorkOp::AddTask(task("Pause", vec![Step::Wait { seconds: 2.0 }])))
        .unwrap();
    h.run_steps(3);
    let revision_with_task = h.state().session.doc().revision();
    assert!(revision_with_task > revision);
    h.get_by_label("스케줄 계산").click();
    h.run_steps(4);
    assert!(h.state().session.schedule_result().is_some_and(|(s, c)| c && s.makespan() >= 2.0));
    let steps = h.state().session.history().undo_depth();
    h.get_by_label("스케줄 재생").click();
    h.run_steps(4);
    assert!(h.state().session.schedule_playback().is_some());
    // While it plays the button says Pause; pressing it stops the clock where it is.
    assert!(has(&h, "스케줄 일시정지"), "the Play button did not become Pause");
    h.get_by_label("스케줄 일시정지").click();
    h.run_steps(2);
    let paused = h.state().session.schedule_playback();
    h.run_steps(10);
    assert_eq!(h.state().session.schedule_playback(), paused, "the clock moved while paused");
    assert!(has(&h, "스케줄 재생"), "the button did not go back to Play");
    // Reset: back to where the document has the robots, and nothing in the document changed.
    h.get_by_label("스케줄 원위치").click();
    h.run_steps(3);
    assert!(h.state().session.schedule_playback().is_none());
    assert_eq!(h.state().session.doc().revision(), revision_with_task, "playback changed the document");
    assert_eq!(h.state().session.history().undo_depth(), steps);
    // Play to the end: the clock stops at the end by itself and Play starts again from the start.
    h.get_by_label("스케줄 재생").click();
    h.run_steps(200);
    let end = h.state().session.schedule_result().map(|(s, _)| s.makespan()).unwrap();
    let at = h.state().session.schedule_playback().unwrap();
    assert!(at <= end + 1e-9, "the clock ran past the end: {at} > {end}");
    assert!(!has(&h, "스케줄 일시정지"), "still playing at the end");
    h.get_by_label("스케줄 재생").click();
    h.run_steps(3);
    assert!(
        h.state().session.schedule_playback().is_some_and(|t| t < end),
        "Play at the end did not start over"
    );
}

#[test]
fn an_edit_makes_the_schedule_stale_and_closing_the_panel_puts_the_robots_back() {
    let mut h = harness();
    with_robot(&mut h);
    h.state_mut()
        .session
        .edit_work(WorkOp::AddTask(task("Pause", vec![Step::Wait { seconds: 1.0 }])))
        .unwrap();
    open_schedule(&mut h);
    h.get_by_label("스케줄 계산").click();
    h.run_steps(4);
    assert!(!has(&h, "문서가 바뀌었습니다 — 다시 계산하십시오"));
    // An edit (and its undo): the note shows, the old result is kept, asking again clears it.
    h.state_mut()
        .session
        .edit_work(WorkOp::AddTask(task("Second", vec![Step::Wait { seconds: 1.0 }])))
        .unwrap();
    h.run_steps(3);
    assert!(has(&h, "문서가 바뀌었습니다 — 다시 계산하십시오"), "no stale note after an edit");
    assert!(h.state().session.schedule_result().is_some_and(|(_, current)| !current));
    // Playing a stale schedule still works and does not change the document.
    let revision = h.state().session.doc().revision();
    h.get_by_label("스케줄 재생").click();
    h.run_steps(4);
    assert_eq!(h.state().session.doc().revision(), revision);
    h.get_by_label("스케줄 계산").click();
    h.run_steps(4);
    assert!(!has(&h, "문서가 바뀌었습니다 — 다시 계산하십시오"));
    // Closing the panel with ✕ ends playback (the robots go back); the schedule can be opened again.
    h.state_mut().session.set_schedule_playback(Some(0.5));
    h.run_steps(2);
    let close = h.query_all_by_label("✕").last().unwrap().rect().center();
    h.event(egui::Event::PointerMoved(close));
    h.run_steps(2);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: close,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        h.run_steps(2);
    }
    assert!(
        h.state().session.schedule_playback().is_none(),
        "robots still shown at a moment of the schedule after closing"
    );
    assert!(!has(&h, "스케줄 계산"), "the panel is still open");
    open_schedule(&mut h);
    assert!(has(&h, "스케줄 계산"));
}

#[test]
fn blocked_tasks_and_endless_repeats_are_explained_on_the_panel() {
    let mut h = harness();
    with_robot(&mut h);
    let never =
        Task { precondition: "false == true".into(), ..task("Never", vec![Step::Wait { seconds: 1.0 }]) };
    h.state_mut().session.edit_work(WorkOp::AddTask(never)).unwrap();
    let forever = h
        .state_mut()
        .session
        .edit_work(WorkOp::AddTask(task("Forever", vec![Step::Wait { seconds: 0.1 }])))
        .unwrap()
        .unwrap()
        .downcast()
        .unwrap();
    h.state_mut().session.edit_work(WorkOp::SetTaskRepeat { task: forever, times: Some(0) }).unwrap();
    open_schedule(&mut h);
    h.get_by_label("스케줄 계산").click();
    h.run_steps(6);
    // The blocked list names the task and says why (a precondition that never holds); the cut-off is explained.
    assert!(has(&h, "하지 못하는 작업"), "no list of tasks that cannot be done");
    assert!(h.get_all_by_label_contains("Never").next().is_some(), "the blocked task is not named");
    assert!(
        has(&h, "끝없이 반복하는 작업이 있어 스케줄을 중간에서 잘랐습니다."),
        "no note that the schedule was cut off"
    );
    // A schedule of a thousand bars does not hang the panel: a few more frames still run.
    h.run_steps(5);
}

/// BUG: the schedule panel's time slider and its value box have no accessible name (kittest's accessibility check
/// fails once a schedule is shown and played), so a screen reader reads two unnamed controls.
#[test]
fn the_schedule_panels_slider_has_an_accessible_name() {
    // A harness with the accessibility check on (the default) and the clock stepping.
    let mut h = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .with_step_dt(0.05)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    h.run();
    open_schedule(&mut h);
    h.get_by_label("스케줄 계산").click();
    h.run_steps(4);
    with_robot(&mut h);
    h.state_mut()
        .session
        .edit_work(WorkOp::AddTask(task("Pause", vec![Step::Wait { seconds: 2.0 }])))
        .unwrap();
    h.run_steps(3);
    h.get_by_label("스케줄 계산").click();
    h.run_steps(4);
    h.get_by_label("스케줄 재생").click();
    h.run_steps(4);
}

#[test]
fn a_numeric_effect_is_an_expression_and_a_wrong_one_is_explained_and_not_stored() {
    let mut h = harness();
    let count: gc_core::Id<StateVar> = h
        .state_mut()
        .session
        .edit_work(WorkOp::AddStateVar(StateVar { name: "count".into(), value: StateValue::Int(0) }))
        .unwrap()
        .unwrap()
        .downcast()
        .unwrap();
    let tid: gc_core::Id<Task> = h
        .state_mut()
        .session
        .edit_work(WorkOp::AddTask(Task {
            effects: vec![Effect { target: EffectTarget::Var(count), value: "1".into() }],
            ..task("Count", Vec::new())
        }))
        .unwrap()
        .unwrap()
        .downcast()
        .unwrap();
    h.run_steps(3);
    // Open the work node and select the task.
    let at = h.get_all_by_label("작업").find(|n| n.rect().min.x < 300.0).unwrap().rect().center();
    press(&mut h, at);
    let row = h.get_all_by_label_contains("Count").find(|n| n.rect().min.x < 300.0).unwrap().rect().center();
    press(&mut h, row);
    let input = egui::accesskit::Role::TextInput;
    let value_box = |h: &H| {
        // The effect's value box: the text input at the right whose text is the stored value.
        h.query_all_by_role(input)
            .filter(|n| n.rect().min.x > 700.0)
            .max_by(|a, b| a.rect().min.y.total_cmp(&b.rect().min.y))
            .unwrap()
            .rect()
            .center()
    };
    let steps = h.state().session.history().undo_depth();
    let stored = |h: &H| h.state().session.doc().get(tid).unwrap().effects[0].value.clone();
    // A good expression is stored as typed, one step.
    let at = value_box(&h);
    press(&mut h, at);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.query_all_by_role(input)
        .filter(|n| n.rect().min.x > 700.0)
        .max_by(|a, b| a.rect().min.y.total_cmp(&b.rect().min.y))
        .unwrap()
        .type_text("count * 2 + 1");
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert_eq!(stored(&h), "count * 2 + 1");
    assert_eq!(h.state().session.history().undo_depth(), steps + 1);
    assert!(h.state_mut().session.undo());
    assert_eq!(stored(&h), "1");
    // The same box again with wrong texts afterwards (the box holds what was typed last, not the stored one).
    // Each wrong text: an explanation with «…», nothing stored, no undo step.
    for bad in [
        "count +",
        "apples + 1",
        "1 +* 2",
        "count / ",
        "(",
        "1e999",
        "0/0",
        "count count",
        "\"",
        "count == 1",
    ] {
        let at = value_box(&h);
        press(&mut h, at);
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        h.query_all_by_role(input)
            .filter(|n| n.rect().min.x > 700.0)
            .max_by(|a, b| a.rect().min.y.total_cmp(&b.rect().min.y))
            .unwrap()
            .type_text(bad);
        h.run_steps(2);
        let explained = h.get_all_by_label_contains("«").next().is_some();
        h.key_press(egui::Key::Tab);
        h.run_steps(3);
        assert!(explained, "{bad:?}: no explanation under the box");
        assert_eq!(stored(&h), "1", "{bad:?} was stored");
        assert_eq!(h.state().session.history().undo_depth(), steps, "{bad:?}");
        // A good expression typed after a wrong one is stored (the first try after a refusal too).
        let at = value_box(&h);
        press(&mut h, at);
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        h.query_all_by_role(input)
            .filter(|n| n.rect().min.x > 700.0)
            .max_by(|a, b| a.rect().min.y.total_cmp(&b.rect().min.y))
            .unwrap()
            .type_text("count + 5");
        h.key_press(egui::Key::Tab);
        h.run_steps(3);
        assert_eq!(stored(&h), "count + 5", "after {bad:?}");
        assert!(h.state_mut().session.undo());
        h.run_steps(2);
        assert_eq!(stored(&h), "1");
    }
}

fn press(h: &mut H, at: egui::Pos2) {
    h.event(egui::Event::PointerMoved(at));
    h.run_steps(2);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.run_steps(3);
}
