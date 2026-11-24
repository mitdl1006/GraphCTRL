//! QA boundary tests (kittest) for the sketch constraint glyphs: hovering one is only a highlight, and a right-click on it
//! deletes that constraint in one undo step (Inventor 2026 help, To Work with Geometric Constraints
//! GUID-B06FA7B7-5CD5-4D58-B1C5-0C5ABAFFD84E). A screen action that cannot apply leaves the document and the undo depth as
//! they were; one that applies is one undo step that restores exactly.

#![allow(clippy::unwrap_used, clippy::panic, clippy::float_cmp, reason = "test code")]

use eframe::egui;
use egui_kittest::kittest::Queryable as _;
use gc_app::{GraphCtrlApp, RunOptions};
use gc_tools::{Key, ToolEvent, ToolKind};

const SIZE: egui::Vec2 = egui::vec2(1280.0, 800.0);

type H = egui_kittest::Harness<'static, GraphCtrlApp>;

/// A sketch with one nearly horizontal line that became horizontal: one H constraint, glyph near the middle.
fn scene() -> H {
    let mut h = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .with_step_dt(0.05)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    h.run();
    h.get_by_label("XY 스케치").click();
    h.run_steps(3);
    let session = &mut h.state_mut().session;
    session.set_tool(ToolKind::Line);
    for at in [[0.0, 0.0], [0.05, 0.0004]] {
        session.tool_event(ToolEvent::Move { pos: at, px: 1e-4, suppress_snap: false });
        session.tool_event(ToolEvent::Click { pos: at, px: 1e-4, modifier: false });
    }
    session.tool_event(ToolEvent::Key(Key::Escape));
    session.set_tool(ToolKind::Select);
    h.state_mut().fit_view();
    h.run_steps(3);
    h
}

fn constraints(h: &H) -> usize {
    h.state().session.active_sketch().unwrap().1.constraints.len()
}

fn depth(h: &H) -> usize {
    h.state().session.history().undo_depth()
}

fn glyph_at(h: &H) -> egui::Pos2 {
    let mid = h
        .state()
        .session
        .active_sketch()
        .unwrap()
        .1
        .points
        .values()
        .map(|p| p.pos)
        .fold([0.0, 0.0], |acc, p| [acc[0] + p[0] / 2.0, acc[1] + p[1] / 2.0]);
    let screen = h.state().viewport().screen_of(gc_math::DVec3::new(mid[0], mid[1], 0.0)).unwrap();
    screen + egui::vec2(11.5, -14.0)
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

#[test]
fn hovering_a_glyph_and_leaving_it_changes_nothing_however_often() {
    let mut h = scene();
    let before = h.state().session.doc().clone();
    let steps = depth(&h);
    let glyph = glyph_at(&h);
    for _ in 0..20 {
        h.event(egui::Event::PointerMoved(glyph));
        h.run_steps(2);
        h.event(egui::Event::PointerMoved(glyph + egui::vec2(120.0, 90.0)));
        h.run_steps(2);
    }
    h.event(egui::Event::PointerGone);
    h.run_steps(3);
    assert!(h.state().session.doc().same_content(&before), "hovering changed the document");
    assert_eq!(depth(&h), steps);
}

#[test]
fn a_right_click_on_a_glyph_deletes_its_constraint_in_one_step_that_undo_restores_exactly() {
    let mut h = scene();
    let before = h.state().session.doc().clone();
    let steps = depth(&h);
    let n = constraints(&h);
    let dof = h.state().session.report().unwrap().dof;
    assert!(n >= 1);
    let glyph = glyph_at(&h);
    secondary(&mut h, glyph);
    assert!(has(&h, "구속 삭제"), "the glyph's menu offers deleting the constraint");
    h.get_by_label("구속 삭제").click();
    h.run_steps(3);
    assert_eq!(constraints(&h), n - 1);
    assert_eq!(depth(&h), steps + 1, "one undo step");
    assert!(h.state().session.report().unwrap().dof > dof, "the line is freer without its constraint");
    // The geometry stays where it was; Undo brings the constraint back exactly, Redo takes it away again.
    assert!(h.state_mut().session.undo());
    assert!(h.state().session.doc().same_content(&before));
    assert!(h.state_mut().session.redo());
    assert_eq!(constraints(&h), n - 1);
}

#[test]
fn a_right_click_away_from_every_glyph_offers_no_constraint_deletion_and_a_dismissed_menu_deletes_nothing() {
    let mut h = scene();
    let n = constraints(&h);
    let steps = depth(&h);
    // Far from the glyph: no "구속 삭제".
    secondary(&mut h, egui::pos2(900.0, 650.0));
    assert!(!has(&h, "구속 삭제"));
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    // On the glyph, then the menu is dismissed (Esc, then a click elsewhere): nothing is deleted.
    let glyph = glyph_at(&h);
    secondary(&mut h, glyph);
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    secondary(&mut h, glyph);
    let away = egui::pos2(900.0, 650.0);
    h.event(egui::Event::PointerMoved(away));
    h.run_steps(2);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: away,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.run_steps(3);
    assert_eq!(constraints(&h), n, "a dismissed menu deleted a constraint");
    assert_eq!(depth(&h), steps);
}

#[test]
fn deleting_the_same_glyph_twice_in_a_row_deletes_one_constraint_only() {
    let mut h = scene();
    let n = constraints(&h);
    let glyph = glyph_at(&h);
    secondary(&mut h, glyph);
    h.get_by_label("구속 삭제").click();
    h.run_steps(3);
    let after_first = constraints(&h);
    assert_eq!(after_first, n - 1);
    // The glyph is gone: a second right-click on the same spot has nothing to delete (or deletes another glyph there, but
    // never more than one).
    secondary(&mut h, glyph);
    if has(&h, "구속 삭제") {
        h.get_by_label("구속 삭제").click();
        h.run_steps(3);
    }
    assert!(constraints(&h) >= after_first.saturating_sub(1));
}
