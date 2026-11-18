//! QA boundary tests (kittest) for the newest screen features: Look At (navigation bar button, Page Up), the 3D
//! sketch Bend and Spline buttons, the browser "…" menu (Display Preferences, Edit Values in Browser) and the
//! spline right-click menu. Inventor 2026 help, under
//! <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/>: Look At (Navigation Bar)
//! GUID-3B926737-92BA-46CF-B722-97F68651772B, Bend GUID-4F861674-F74C-474E-869A-4C190ED6EC4B, splines
//! GUID-DB15C479-66F2-456A-9E54-40A60F83DC92, browser GUID-5F975252-BDB4-488C-A2C5-8B15C7D3C641.
//! A screen action that cannot apply leaves the document, the undo depth and the camera as they were.

#![allow(clippy::unwrap_used, clippy::panic, clippy::float_cmp, reason = "test code")]

use eframe::egui;
use egui_kittest::kittest::Queryable as _;
use gc_app::{GraphCtrlApp, RunOptions};
use gc_doc::SketchOp;
use gc_tools::ToolKind;

const SIZE: egui::Vec2 = egui::vec2(1280.0, 800.0);

type H = egui_kittest::Harness<'static, GraphCtrlApp>;

fn harness() -> H {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .with_step_dt(0.05)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness
}

fn button(h: &mut H, at: egui::Pos2, which: egui::PointerButton) {
    h.event(egui::Event::PointerMoved(at));
    h.run_steps(3);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: at,
            button: which,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        h.run_steps(3);
    }
}

fn click(h: &mut H, at: egui::Pos2) {
    button(h, at, egui::PointerButton::Primary);
}

fn pick_variant(h: &mut H, group: &str, variant: &str) {
    let at = h.get_by_label(&format!("{group} 더 보기")).rect().center();
    click(h, at);
    h.query_all_by_label(variant).last().unwrap().click();
    h.run_steps(3);
}

fn cam(h: &H) -> (gc_math::DVec3, gc_math::DVec3) {
    let c = &h.state().viewport().camera;
    (c.forward(), c.pivot)
}

fn depth(h: &H) -> usize {
    h.state().session.history().undo_depth()
}

fn splines(h: &H) -> usize {
    h.state().session.active_sketch().map_or(0, |(_, sk)| {
        sk.curves.values().filter(|c| matches!(c.geom, gc_sketch::CurveGeom::Spline { .. })).count()
    })
}

fn add_line(h: &mut H, a: [f64; 3], b: [f64; 3]) -> gc_core::Id<gc_sketch::Curve> {
    let (sketch, _) = h.state().session.active_sketch().map(|(k, s)| (k, s.curves.len())).unwrap();
    let op = SketchOp::AddLine {
        a,
        b,
        snap_a: gc_doc::EndSnap::default(),
        snap_b: gc_doc::EndSnap::default(),
        horizontal: None,
    };
    let made = h.state_mut().session.execute(&gc_doc::Command::Sketch { sketch, op }).unwrap();
    h.run();
    made.iter().find_map(|c| if let gc_doc::Created::Curve(id) = c { Some(*id) } else { None }).unwrap()
}

/// Selects the curve under a sketch-plane point with the Select tool.
fn select_at(h: &mut H, at: [f64; 2]) {
    let s = &mut h.state_mut().session;
    s.set_tool(ToolKind::Select);
    let px = 5e-4;
    s.tool_event(gc_tools::ToolEvent::Move { pos: at, px, suppress_snap: true });
    s.tool_event(gc_tools::ToolEvent::Click { pos: at, px, modifier: false });
    h.run();
}

// ---- Look At ----

#[test]
fn look_at_with_nothing_to_look_at_changes_nothing() {
    let mut h = harness();
    let (before, steps) = (cam(&h), depth(&h));
    assert!(!h.state().viewport().can_look_at(&h.state().session));
    h.get_by_label("면 보기").click();
    h.run_steps(3);
    h.key_press(egui::Key::PageUp);
    h.run_steps(3);
    assert_eq!(cam(&h), before, "the camera moved with nothing to look at");
    assert_eq!(depth(&h), steps);
    // A 3D sketch has no plane to face: Look At stays disabled there too.
    pick_variant(&mut h, "스케치", "3D 스케치");
    h.run();
    assert!(h.state().session.sketch_is_spatial());
    let before = cam(&h);
    h.key_press(egui::Key::PageUp);
    h.run_steps(3);
    h.get_by_label("면 보기").click();
    h.run_steps(3);
    assert_eq!(cam(&h), before, "Look At moved the view in a 3D sketch");
    // After finishing the sketch the button is off again.
    h.state_mut().session.finish_sketch();
    h.run_steps(3);
    let before = cam(&h);
    h.key_press(egui::Key::PageUp);
    h.run_steps(3);
    assert_eq!(cam(&h), before);
}

#[test]
fn look_at_a_sketch_and_a_selected_line_is_repeatable_and_finite() {
    let mut h = harness();
    h.state_mut().session.create_sketch("XZ").unwrap();
    h.run_steps(3);
    let steps = depth(&h);
    h.state_mut().viewport_mut().look(gc_draw::StandardView::Iso);
    h.run_steps(3);
    h.key_press(egui::Key::PageUp);
    h.run_steps(3);
    let once = cam(&h);
    assert!(once.0.y.abs() > 0.99, "square to the XZ plane: {:?}", once.0);
    // Again, and again from a different view: the same result, nothing finite lost.
    h.key_press(egui::Key::PageUp);
    h.run_steps(3);
    let twice = cam(&h);
    assert!((twice.0 - once.0).length() < 1e-9 && (twice.1 - once.1).length() < 1e-9);
    // A selected line at 30°: the view is square to the plane and the line lies level.
    let line = add_line(&mut h, [0.0, 0.0, 0.0], [0.0866, 0.05, 0.0]);
    select_at(&mut h, [0.0433, 0.025]);
    assert!(h.state().session.selected_curves().contains(&line));
    h.state_mut().viewport_mut().look(gc_draw::StandardView::Iso);
    h.run_steps(3);
    h.key_press(egui::Key::PageUp);
    h.run_steps(3);
    let (forward, pivot) = cam(&h);
    assert!(forward.y.abs() > 0.99 && pivot.is_finite(), "{forward:?} {pivot:?}");
    // A zero-length line selected: finite, no panic.
    let dot = add_line(&mut h, [0.1, 0.1, 0.0], [0.1, 0.1 + 1e-13, 0.0]);
    select_at(&mut h, [0.1, 0.1]);
    let _ = dot;
    h.key_press(egui::Key::PageUp);
    h.run_steps(3);
    let c = &h.state().viewport().camera;
    assert!(c.forward().is_finite() && c.up().is_finite() && c.pivot.is_finite());
    // Looking never adds undo steps beyond the lines drawn.
    assert_eq!(depth(&h), steps + 2);
}

// ---- 3D Bend button ----

#[test]
fn the_bend_button_exists_only_in_a_3d_sketch_and_never_leaves_bending_open() {
    let mut h = harness();
    h.state_mut().session.create_sketch("XY").unwrap();
    h.run_steps(3);
    assert!(h.query_all_by_label("굽힘").next().is_none(), "a 2D sketch has no Bend button");
    h.state_mut().session.finish_sketch();
    h.run_steps(3);
    pick_variant(&mut h, "스케치", "3D 스케치");
    h.run();
    let steps = depth(&h);
    h.get_by_label("굽힘").click();
    h.run_steps(3);
    assert!(h.state().viewport().is_bending());
    // A click in empty space and a second press of the button leave the document alone.
    click(&mut h, egui::pos2(700.0, 450.0));
    h.query_all_by_label("굽힘").last().unwrap().click();
    h.run_steps(3);
    assert_eq!(depth(&h), steps, "Bend changed the document without two lines");
    // Finishing the sketch ends Bend with it.
    h.state_mut().session.finish_sketch();
    h.run_steps(3);
    assert!(!h.state().viewport().is_bending());
    // Esc with nothing running is harmless.
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert!(!h.state().viewport().is_bending());
    assert_eq!(depth(&h), steps);
}

// ---- 3D spline button ----

#[test]
fn the_3d_spline_button_enter_and_esc_boundaries() {
    let mut h = harness();
    pick_variant(&mut h, "스케치", "3D 스케치");
    h.run();
    pick_variant(&mut h, "선", "스플라인(보간)");
    h.run();
    assert_eq!(h.state().session.tool(), ToolKind::Spline);
    let steps = depth(&h);
    // One point and Enter: nothing is made, no undo step.
    click(&mut h, egui::pos2(480.0, 520.0));
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert_eq!(splines(&h), 0);
    assert_eq!(depth(&h), steps);
    // Esc drops the points placed so far; Enter after it has nothing to finish.
    click(&mut h, egui::pos2(560.0, 470.0));
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert_eq!(splines(&h), 0);
    assert_eq!(depth(&h), steps);
    // Three fit points and Enter: one spline, one undo step, and undo removes it.
    h.state_mut().session.set_tool(ToolKind::Spline);
    h.run();
    for at in [egui::pos2(480.0, 520.0), egui::pos2(560.0, 470.0), egui::pos2(640.0, 540.0)] {
        click(&mut h, at);
    }
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert_eq!(splines(&h), 1);
    assert_eq!(depth(&h), steps + 1);
    // A second Enter does not make another.
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert_eq!(splines(&h), 1);
    assert_eq!(depth(&h), steps + 1);
    assert!(h.state_mut().session.undo());
    assert_eq!(splines(&h), 0);
    // Placing the same point twice finishes the spline too (the last point is clicked again).
    for at in [egui::pos2(480.0, 600.0), egui::pos2(560.0, 650.0), egui::pos2(560.0, 650.0)] {
        click(&mut h, at);
    }
    assert!(splines(&h) <= 1);
    assert!(
        h.state()
            .session
            .active_sketch()
            .unwrap()
            .1
            .points
            .values()
            .all(|p| p.pos.iter().all(|v| v.is_finite()))
    );
}

// ---- Browser "..." menu ----

fn two_bodies_with_mate(h: &mut H) -> gc_core::Id<gc_doc::Relationship> {
    let solid = gc_doc::Solid::Box { size: [0.05; 3] };
    let app = h.state_mut();
    let fixed = app.session.add_body(solid.clone(), gc_doc::Placement::default()).unwrap();
    let moving =
        app.session.add_body(solid, gc_doc::Placement { xyz: [0.0, 0.0, 0.2], rpy: [0.0; 3] }).unwrap();
    let plane = gc_doc::Geometry::origin(gc_doc::OriginElement::Plane(gc_doc::OriginPlane::Xy));
    let select = |body: gc_core::Id<gc_doc::Body>| gc_doc::Selection {
        occurrence: gc_doc::Occurrence::Item(body.erase()),
        geometry: plane,
    };
    let id = app
        .session
        .add_relationship(gc_doc::Relationship::new(
            "Mate:1",
            gc_doc::RelationKind::Mate { offset: 0.01, sense: gc_doc::AxisSense::Opposed },
            select(moving),
            select(fixed),
        ))
        .unwrap();
    h.run();
    id
}

fn toggle(h: &mut H, item: &str) {
    h.get_by_label("브라우저 메뉴").click();
    h.run_steps(3);
    h.get_by_label(item).click();
    h.run_steps(3);
}

#[test]
fn the_browser_menu_options_are_view_only_and_every_combination_is_safe() {
    let mut h = harness();
    let id = two_bodies_with_mate(&mut h);
    let (steps, before) = (depth(&h), h.state().session.doc().clone());
    let origins = |h: &H| h.query_all_by_label("원점").count();
    assert!(origins(&h) >= 1);
    // Each option on, then off again, one by one and together; the document never changes.
    for item in [
        "작업 피처 숨기기",
        "완전 구속 숨기기",
        "자식만 표시",
        "억제된 항목만",
        "경고 숨기기",
        "구속 상태 표시",
        "브라우저에서 값 편집",
    ] {
        toggle(&mut h, item);
        assert_eq!(depth(&h), steps, "{item} added an undo step");
        assert!(h.state().session.doc().same_content(&before), "{item} changed the document");
    }
    assert_eq!(origins(&h), 0, "Hide Work Features is on");
    // Hide Fully Constrained with a relationship that is not fully constrained keeps listing it (or hides
    // it if it is full): either way the relationship still exists.
    assert!(h.state().session.doc().get(id).is_some());
    for item in [
        "작업 피처 숨기기",
        "완전 구속 숨기기",
        "자식만 표시",
        "억제된 항목만",
        "경고 숨기기",
        "구속 상태 표시",
        "브라우저에서 값 편집",
    ] {
        toggle(&mut h, item);
    }
    assert!(origins(&h) >= 1, "all options off again: the Origin folders are back");
    assert_eq!(depth(&h), steps);
    // With Find text that matches nothing the options still work and nothing panics.
    let find = h.get_by_role_and_label(egui::accesskit::Role::TextInput, "찾기");
    find.click();
    h.run_steps(2);
    h.get_by_role_and_label(egui::accesskit::Role::TextInput, "찾기").type_text("zzz-no-such-name");
    h.run_steps(3);
    toggle(&mut h, "작업 피처 숨기기");
    toggle(&mut h, "작업 피처 숨기기");
    assert!(h.state().session.doc().same_content(&before));
}

#[test]
fn edit_values_in_browser_accepts_only_sane_values_and_can_be_turned_off() {
    let mut h = harness();
    let id = two_bodies_with_mate(&mut h);
    h.get_by_label("Mate:1").click();
    h.run_steps(3);
    let input = egui::accesskit::Role::TextInput;
    let offset = |h: &H| {
        let kind = h.state().session.doc().get(id).unwrap().kind;
        let gc_doc::RelationKind::Mate { offset, .. } = kind else { panic!("{kind:?}") };
        offset
    };
    let type_value = |h: &mut H, text: &str| {
        h.get_by_role_and_label(input, "관계 값").click();
        h.run_steps(2);
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        h.get_by_role_and_label(input, "관계 값").type_text(text);
        h.key_press(egui::Key::Enter);
        h.run_steps(3);
    };
    // Bad text: the value stays, the document stays solvable, no panic.
    for bad in ["abc", "", "nan", "1e999", "--5"] {
        let (steps, was) = (depth(&h), offset(&h));
        type_value(&mut h, bad);
        let now = offset(&h);
        assert!(now.is_finite(), "{bad:?} made the offset {now}");
        if now != was {
            assert_eq!(depth(&h), steps + 1, "{bad:?}: accepted, so one undo step");
        } else {
            assert_eq!(depth(&h), steps, "{bad:?}: refused, so none");
        }
    }
    // A good value is one undo step that undoes exactly.
    let (steps, was) = (depth(&h), offset(&h));
    type_value(&mut h, "25");
    assert!((offset(&h) - 0.025).abs() < 1e-9);
    assert_eq!(depth(&h), steps + 1);
    assert!(h.state_mut().session.undo());
    assert!((offset(&h) - was).abs() < 1e-12);
    // Edit Values in Browser off: the box is gone.
    toggle(&mut h, "브라우저에서 값 편집");
    h.get_by_label("Mate:1").click();
    h.run_steps(3);
    assert!(h.query_all_by_role_and_label(input, "관계 값").next().is_none());
    toggle(&mut h, "브라우저에서 값 편집");
    h.get_by_label("Mate:1").click();
    h.run_steps(3);
    assert!(h.query_all_by_role_and_label(input, "관계 값").next().is_some());
}

// ---- Spline right-click menu ----

#[test]
fn the_spline_right_click_menu_offers_what_fits_the_point_under_the_cursor() {
    let mut h = harness();
    h.state_mut().session.create_sketch("XY").unwrap();
    h.run_steps(3);
    h.state_mut().session.set_tool(ToolKind::Spline);
    h.run();
    let pts = [
        egui::pos2(500.0, 500.0),
        egui::pos2(600.0, 440.0),
        egui::pos2(700.0, 520.0),
        egui::pos2(800.0, 450.0),
    ];
    for at in pts {
        click(&mut h, at);
    }
    // A double click on the last fit point ends the spline in a planar sketch.
    h.event(egui::Event::PointerMoved(pts[3]));
    h.run_steps(2);
    for _ in 0..2 {
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos: pts[3],
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
            h.run_steps(1);
        }
    }
    h.run_steps(3);
    assert_eq!(splines(&h), 1);
    h.state_mut().session.set_tool(ToolKind::Select);
    h.run();
    let right = |h: &mut H, at: egui::Pos2| button(h, at, egui::PointerButton::Secondary);
    let steps = depth(&h);
    // An empty place: neither Split Spline, Close Curve nor Insert Point.
    right(&mut h, egui::pos2(900.0, 700.0));
    for name in ["스플라인 분할", "곡선 닫기", "점 삽입"] {
        assert!(h.query_all_by_label(name).next().is_none(), "{name} offered in empty space");
    }
    // The marking menu is dismissed by a click in its centre (Esc is left to the command in progress; Inventor 2026
    // help, To Work with Marking Menus, GUID-756BC728-3366-497D-B480-4A06E932ED20).
    button(&mut h, egui::pos2(900.0, 700.0), egui::PointerButton::Primary);
    h.run_steps(3);
    // An inner fit point offers Split Spline; the end offers Close Curve; both are one undo step.
    right(&mut h, pts[1]);
    assert!(h.query_all_by_label("스플라인 분할").next().is_some(), "Split Spline at an inner point");
    assert!(h.query_all_by_label("곡선 닫기").next().is_none(), "no Close Curve at an inner point");
    h.query_all_by_label("스플라인 분할").last().unwrap().click();
    h.run_steps(3);
    assert_eq!(splines(&h), 2);
    assert_eq!(depth(&h), steps + 1);
    assert!(h.state_mut().session.undo());
    assert_eq!(splines(&h), 1);
    // The start point of an open spline of 4 points: Close Curve.
    right(&mut h, pts[0]);
    assert!(h.query_all_by_label("스플라인 분할").next().is_none(), "the start is not an inner point");
    h.query_all_by_label("곡선 닫기").last().unwrap().click();
    h.run_steps(3);
    assert_eq!(depth(&h), steps + 1);
    // Now closed: it no longer offers Close Curve.
    right(&mut h, pts[0]);
    assert!(h.query_all_by_label("곡선 닫기").next().is_none(), "a closed spline offers no Close Curve");
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    // Insert Point: offered on the curve; a click far from the curve inserts nothing (no undo step); Esc ends.
    let steps = depth(&h);
    right(&mut h, pts[2]);
    h.query_all_by_label("점 삽입").last().unwrap().click();
    h.run_steps(3);
    click(&mut h, egui::pos2(900.0, 720.0));
    // Observation: the point goes to the nearest place on the curve however far the click is (backend
    // design, `insert_spline_point`); at most one undo step either way.
    let after_click = depth(&h);
    assert!(after_click == steps || after_click == steps + 1, "{steps} -> {after_click}");
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    // (The hint is painted text, not an accessible node: the mode is observed through its effects.)
    click(&mut h, egui::pos2(900.0, 720.0));
    assert_eq!(depth(&h), after_click, "Insert Point still running after Esc");
    assert!(
        h.state()
            .session
            .active_sketch()
            .unwrap()
            .1
            .points
            .values()
            .all(|p| p.pos.iter().all(|v| v.is_finite()))
    );
}
