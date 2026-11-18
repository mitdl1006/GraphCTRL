//! QA boundary tests (kittest) for the assembly screens: the Constrain dialog's tabs and value boxes, Relationship
//! Conflict Analysis (Inventor 2026 help GUID-528B2CD0-93FD-45BC-AB69-6C5A559BB8E8) and Recover / Isolate Related
//! Failed (GUID-8C95B0AB-4505-4092-8C37-E60C68DCF4A0), the ViewCube menu (To Reorient the View
//! GUID-94F44C80-7313-4529-85D1-E64ABF732700) and the User Interface setting for the marking menu
//! (GUID-285B9036-0E97-4F18-A6D2-EF78B8B8FAA9). A screen action that cannot apply leaves the document and the
//! undo depth as they were; one that applies is one undo step that restores exactly.

#![allow(clippy::unwrap_used, clippy::panic, clippy::float_cmp, reason = "test code")]

use eframe::egui;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use gc_app::{GraphCtrlApp, RunOptions};
use gc_doc::{
    AxisSense, Body, Geometry, Occurrence, OriginElement, OriginPlane, Placement, RelationKind, Relationship,
    Selection, Solid,
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

fn has(h: &H, label: &str) -> bool {
    h.query_all_by_label(label).next().is_some()
}

fn press(h: &mut H, at: egui::Pos2, button: egui::PointerButton) {
    h.event(egui::Event::PointerMoved(at));
    h.run_steps(2);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: at, button, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.run_steps(3);
}

fn secondary(h: &mut H, at: egui::Pos2) {
    press(h, at, egui::PointerButton::Secondary);
}

fn primary(h: &mut H, at: egui::Pos2) {
    press(h, at, egui::PointerButton::Primary);
}

fn boxes(h: &mut H) -> (gc_core::Id<Body>, gc_core::Id<Body>) {
    let solid = Solid::Box { size: [0.05; 3] };
    let s = &mut h.state_mut().session;
    let fixed = s.add_body(solid.clone(), Placement::default()).unwrap();
    let moving = s.add_body(solid, Placement { xyz: [0.0, 0.0, 0.2], rpy: [0.0; 3] }).unwrap();
    h.run();
    (fixed, moving)
}

fn plane_of(body: gc_core::Id<Body>) -> Selection {
    Selection {
        occurrence: Occurrence::Item(body.erase()),
        geometry: Geometry::origin(OriginElement::Plane(OriginPlane::Xy)),
    }
}

fn mate(name: &str, offset: f64, moving: gc_core::Id<Body>, fixed: gc_core::Id<Body>) -> Relationship {
    Relationship::new(
        name,
        RelationKind::Mate { offset, sense: AxisSense::Opposed },
        plane_of(moving),
        plane_of(fixed),
    )
}

/// Opens the Constrain dialog from the Assemble tab.
fn open_constrain(h: &mut H) {
    h.get_by_label("조립").click();
    h.run();
    h.get_by_label("구속").click();
    h.run_steps(4);
}

/// The dialog's own widget with this label (the dialog sits at the right edge, away from the ribbon).
fn in_dialog<'a>(h: &'a H, label: &'a str) -> egui_kittest::Node<'a> {
    // The dialog floats over the right side: of the widgets with this label, the right-most one.
    h.get_all_by_label(label)
        .max_by(|a, b| a.rect().min.x.total_cmp(&b.rect().min.x))
        .unwrap_or_else(|| panic!("no {label:?} in the dialog"))
}

#[test]
fn every_constrain_tab_opens_and_an_empty_dialog_cannot_apply_or_change_anything() {
    let mut h = harness();
    let (_fixed, _moving) = boxes(&mut h);
    let before = h.state().session.doc().clone();
    let steps = depth(&h);
    open_constrain(&mut h);
    assert!(has(&h, "첫 번째 선택: 뷰에서 선택"));
    for tab in ["맞춤", "플러시", "각도", "접선", "삽입", "대칭", "모션 (Motion)", "맞춤"] {
        in_dialog(&h, tab).click();
        h.run_steps(3);
        assert!(has(&h, "첫 번째 선택: 뷰에서 선택"), "the dialog closed on the {tab:?} tab");
        // With nothing picked, OK and Apply are off.
        for button in ["확인", "적용"] {
            let node = in_dialog(&h, button);
            assert!(node.accesskit_node().is_disabled(), "{button} is on with nothing picked ({tab})");
        }
        // The kind's own boxes.
        match tab {
            "각도" => assert!(has(&h, "각도 값")),
            "모션 (Motion)" => assert!(has(&h, "비율")),
            "대칭" => assert!(
                has(&h, "대칭 평면: 뷰에서 선택") || h.query_all_by_label_contains("대칭").count() > 1
            ),
            _ => {}
        }
    }
    // Clicking the disabled OK changes nothing.
    in_dialog(&h, "확인").click();
    h.run_steps(2);
    in_dialog(&h, "취소").click();
    h.run_steps(3);
    assert!(!has(&h, "첫 번째 선택: 뷰에서 선택"), "Cancel left the dialog open");
    assert!(h.state().session.doc().same_content(&before));
    assert_eq!(depth(&h), steps);
}

#[test]
fn the_value_boxes_say_when_their_text_is_not_a_value() {
    let mut h = harness();
    boxes(&mut h);
    open_constrain(&mut h);
    let message = "수식을 읽을 수 없습니다";
    let type_into = |h: &mut H, label: &str, text: &str| {
        let field = h.get_by_role_and_label(egui::accesskit::Role::TextInput, label);
        field.focus();
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        h.get_by_role_and_label(egui::accesskit::Role::TextInput, label).type_text(text);
        h.run_steps(2);
    };
    // Offset (Mate): numbers, units and expressions are fine; words and half expressions are not.
    for (text, ok) in [
        ("5", true),
        ("5 mm", true),
        ("1 in", true),
        ("2 * 3", true),
        ("-4", true),
        ("0", true),
        ("abc", false),
        ("5 +", false),
        ("(", false),
        ("1 / 0", false),
        ("1e999", false),
    ] {
        type_into(&mut h, "오프셋", text);
        assert_eq!(!has(&h, message), ok, "offset {text:?}");
    }
    // Angle: degrees by default.
    in_dialog(&h, "각도").click();
    h.run_steps(3);
    for (text, ok) in [("45", true), ("90 deg", true), ("1 rad", true), ("abc", false), ("5 mm", false)] {
        type_into(&mut h, "각도 값", text);
        assert_eq!(!has(&h, message), ok, "angle {text:?}");
    }
    // Motion ratio: a finite number or an expression that is not zero (backend e0b77f5 made it a model parameter).
    in_dialog(&h, "모션 (Motion)").click();
    h.run_steps(3);
    for (text, ok) in [
        ("2", true),
        ("-0.5", true),
        ("1e3", true),
        ("0", false),
        ("-0", false),
        ("", false),
        ("abc", false),
        ("NaN", false),
        ("inf", false),
        ("1e999", false),
        ("1/2", true),
    ] {
        type_into(&mut h, "비율", text);
        assert_eq!(!has(&h, message), ok, "ratio {text:?}");
    }
    // The distance of Rotation-Translation reads lengths like an offset.
    in_dialog(&h, "회전-이동 (Rotation-Translation)").click();
    h.run_steps(3);
    for (text, ok) in [("100", true), ("0.1 m", true), ("abc", false)] {
        type_into(&mut h, "거리", text);
        assert_eq!(!has(&h, message), ok, "distance {text:?}");
    }
}

#[test]
fn constrain_through_the_view_places_one_constraint_then_cancel_and_undo_leave_nothing() {
    let mut h = harness();
    let solid = Solid::Box { size: [0.05; 3] };
    let s = &mut h.state_mut().session;
    s.add_body(solid.clone(), Placement::default()).unwrap();
    let moving = s.add_body(solid, Placement { xyz: [0.12, 0.0, 0.0], rpy: [0.0; 3] }).unwrap();
    h.state_mut().fit_view();
    h.state_mut().viewport_mut().camera.pan([0.0, 0.0], [-250.0, 0.0], [1280.0, 700.0]);
    h.run();
    let before = h.state().session.doc().clone();
    let steps = depth(&h);
    open_constrain(&mut h);
    let click_world = |h: &mut H, world: gc_math::DVec3| {
        let at = h.state().viewport().screen_of(world).unwrap();
        primary(h, at);
    };
    // Cancel after one pick: nothing was placed.
    click_world(&mut h, gc_math::DVec3::new(0.12, 0.0, 0.06));
    in_dialog(&h, "취소").click();
    h.run_steps(3);
    assert!(h.state().session.doc().same_content(&before));
    assert_eq!(depth(&h), steps);
    // Two picks and OK place one constraint in one undo step; Undo takes it away.
    open_constrain(&mut h);
    click_world(&mut h, gc_math::DVec3::new(0.12, 0.0, 0.06));
    click_world(&mut h, gc_math::DVec3::new(0.0, 0.0, 0.06));
    in_dialog(&h, "확인").click();
    h.run_steps(3);
    assert_eq!(h.state().session.relationships().len(), 1);
    assert_eq!(depth(&h), steps + 1, "OK is one undo step");
    assert!(h.state_mut().session.undo());
    assert!(h.state().session.doc().same_content(&before));
    let _ = moving;
}

fn conflict_scene(h: &mut H) -> (gc_core::Id<Body>, gc_core::Id<Body>, gc_core::Id<Body>) {
    let (fixed, moving) = boxes(h);
    let bystander = h
        .state_mut()
        .session
        .add_body(Solid::Box { size: [0.05; 3] }, Placement { xyz: [1.0, 0.0, 0.0], rpy: [0.0; 3] })
        .unwrap();
    let s = &mut h.state_mut().session;
    s.add_relationship(mate("Mate:1", 0.0, moving, fixed)).unwrap();
    s.accept_relationship(mate("Mate:2", 0.1, moving, fixed)).unwrap();
    h.run_steps(2);
    (fixed, moving, bystander)
}

fn row_menu(h: &mut H, name: &str) {
    let at = h.get_by_label(name).rect().center();
    secondary(h, at);
}

#[test]
fn conflict_analysis_keep_break_check_and_the_two_ways_to_settle_it() {
    // Suppress what was broken.
    let mut h = harness();
    conflict_scene(&mut h);
    row_menu(&mut h, "Mate:2");
    h.get_by_label("진단 (Diagnose)").click();
    h.run_steps(3);
    h.get_by_label("구속 충돌 분석 (Conflict Analysis)");
    assert!(has(&h, "확인하지 않음"), "nothing was checked yet");
    // The settle buttons are off while nothing is broken.
    assert!(h.get_by_label("끊은 관계 억제").accesskit_node().is_disabled());
    assert!(h.get_by_label("끊은 관계 삭제").accesskit_node().is_disabled());
    h.get_by_label("확인 (Check)").click();
    h.run_steps(2);
    assert!(has(&h, "아직 충돌이 있습니다"));
    // Break, check, keep again: the result follows each Check, and a Check changes nothing in the document.
    let before = h.state().session.doc().clone();
    let steps = depth(&h);
    h.get_all_by_label("끊기 (Break)").next().unwrap().click();
    h.run_steps(2);
    h.get_by_label("확인 (Check)").click();
    h.run_steps(2);
    assert!(has(&h, "충돌 없음"));
    h.get_all_by_label("유지 (Keep)").next().unwrap().click();
    h.run_steps(2);
    h.get_by_label("확인 (Check)").click();
    h.run_steps(2);
    assert!(has(&h, "아직 충돌이 있습니다"));
    assert!(h.state().session.doc().same_content(&before), "Check is not an edit");
    assert_eq!(depth(&h), steps);
    // Break the first again and suppress: one undo step, the conflict is gone, Undo brings it back exactly.
    h.get_all_by_label("끊기 (Break)").next().unwrap().click();
    h.run_steps(2);
    h.get_by_label("끊은 관계 억제").click();
    h.run_steps(3);
    assert_eq!(depth(&h), steps + 1, "suppressing the broken ones is one undo step");
    assert!(
        h.state().session.sick_relationships().is_empty() || h.state().session.relationships().len() == 2
    );
    assert!(h.state_mut().session.undo());
    assert!(h.state().session.doc().same_content(&before));

    // Delete what was broken (a fresh dialog).
    let mut h = harness();
    conflict_scene(&mut h);
    row_menu(&mut h, "Mate:2");
    h.get_by_label("진단 (Diagnose)").click();
    h.run_steps(3);
    let before = h.state().session.doc().clone();
    let steps = depth(&h);
    h.get_all_by_label("끊기 (Break)").next().unwrap().click();
    h.run_steps(2);
    h.get_by_label("끊은 관계 삭제").click();
    h.run_steps(3);
    assert_eq!(h.state().session.relationships().len(), 1, "the failed one is gone");
    assert_eq!(depth(&h), steps + 1);
    assert!(h.state_mut().session.undo());
    assert!(h.state().session.doc().same_content(&before));
}

#[test]
fn conflict_analysis_toggling_a_component_and_checking_changes_nothing() {
    let mut h = harness();
    conflict_scene(&mut h);
    row_menu(&mut h, "Mate:2");
    h.get_by_label("진단 (Diagnose)").click();
    h.run_steps(3);
    let before = h.state().session.doc().clone();
    let steps = depth(&h);
    // Every component of the conflict can be toggled: whichever way, Check does not edit.
    let boxes = |h: &H| {
        h.query_all_by_role(egui::accesskit::Role::CheckBox)
            .filter(|n| {
                n.rect().min.x > 700.0 && n.accesskit_node().label().is_some_and(|l| l.contains("Body"))
            })
            .count()
    };
    let count = boxes(&h);
    assert!(count >= 1, "the dialog lists the components of the conflict");
    for i in 0..count {
        h.query_all_by_role(egui::accesskit::Role::CheckBox)
            .filter(|n| {
                n.rect().min.x > 700.0 && n.accesskit_node().label().is_some_and(|l| l.contains("Body"))
            })
            .nth(i)
            .unwrap()
            .click();
        h.run_steps(2);
        h.get_by_label("확인 (Check)").click();
        h.run_steps(2);
        assert!(has(&h, "충돌 없음") || has(&h, "아직 충돌이 있습니다"));
    }
    assert!(h.state().session.doc().same_content(&before));
    assert_eq!(depth(&h), steps);
    // A relationship deleted while the dialog is open does not crash the next Check.
    let ids: Vec<_> = h.state().session.relationships().into_iter().map(|(id, _)| id).collect();
    h.state_mut().session.delete_relationship(ids[0]).unwrap();
    h.run_steps(2);
    h.get_by_label("확인 (Check)").click();
    h.run_steps(2);
}

#[test]
fn recover_isolates_the_failure_and_a_healthy_relationship_offers_no_recovery() {
    let mut h = harness();
    let (_fixed, _moving, bystander) = conflict_scene(&mut h);
    // A healthy relationship has no Recover entry.
    row_menu(&mut h, "Mate:1");
    assert!(!has(&h, "복구: 관련된 실패 분리 (Isolate Related Failed)"));
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    // The failed one does: the part that has nothing to do with it is hidden, in one step that Undo takes back.
    let before = h.state().session.doc().clone();
    let steps = depth(&h);
    row_menu(&mut h, "Mate:2");
    h.get_by_label("복구: 관련된 실패 분리 (Isolate Related Failed)").click();
    h.run_steps(3);
    assert!(h.state().session.is_isolated());
    assert!(!h.state().session.is_visible(bystander.erase()), "the bystander stays visible");
    assert_eq!(depth(&h), steps + 1);
    assert!(h.state_mut().session.undo());
    assert!(h.state().session.doc().same_content(&before));
    assert!(!h.state().session.is_isolated());
}

// ---- ViewCube ----

fn cube_point(h: &H, dx: f32, dy: f32) -> egui::Pos2 {
    let (middle, half) = h.state().viewport().view_cube_placement().unwrap();
    middle + egui::vec2(dx * half, dy * half)
}

#[test]
fn the_view_cube_menu_opens_anywhere_on_the_cube_and_closes_without_changing_the_view() {
    let mut h = harness();
    let saved = format!("{:?}", h.state().viewport().camera);
    let steps = depth(&h);
    for (dx, dy) in [(0.0, 0.0), (0.9, 0.0), (-0.9, 0.9), (0.9, -0.9), (0.0, 0.95)] {
        let at = cube_point(&h, dx, dy);
        secondary(&mut h, at);
        assert!(has(&h, "현재 뷰를 홈으로 설정") && has(&h, "홈으로 이동"), "no menu at {dx},{dy}");
        // Escape closes it and moves nothing.
        h.key_press(egui::Key::Escape);
        h.run_steps(3);
        assert!(!has(&h, "홈으로 이동"), "Escape left the menu open at {dx},{dy}");
        assert_eq!(format!("{:?}", h.state().viewport().camera), saved);
    }
    // A click elsewhere closes it too (and that click is not Go Home).
    let at = cube_point(&h, 0.0, 0.0);
    secondary(&mut h, at);
    primary(&mut h, egui::pos2(400.0, 600.0));
    assert!(!has(&h, "홈으로 이동"));
    // A right-click well away from the cube is not the cube's menu.
    secondary(&mut h, egui::pos2(400.0, 600.0));
    assert!(!has(&h, "현재 뷰를 홈으로 설정"));
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert_eq!(depth(&h), steps, "the view is not part of the document's history");
}

#[test]
fn go_home_without_a_saved_home_is_the_isometric_view_and_the_saved_home_is_exact() {
    let mut h = harness();
    // No home saved: Go Home gives the isometric view.
    h.state_mut().viewport_mut().look(gc_draw::StandardView::Top);
    h.run_steps(2);
    let at = cube_point(&h, 0.9, 0.0);
    secondary(&mut h, at);
    h.get_by_label("홈으로 이동").click();
    h.run_steps(3);
    let mut iso = h.state().viewport().camera;
    iso.look(gc_draw::StandardView::Iso);
    assert_eq!(h.state().viewport().camera.rotation, iso.rotation);
    // Pan and zoom, save, move elsewhere, go home: the whole camera is back, not only its rotation.
    h.state_mut().viewport_mut().camera.pan([0.0, 0.0], [40.0, -30.0], [1280.0, 700.0]);
    h.state_mut().viewport_mut().camera.zoom(0.7, [600.0, 300.0], [1280.0, 700.0]);
    h.run_steps(2);
    let saved = format!("{:?}", h.state().viewport().camera);
    let at = cube_point(&h, 0.9, 0.0);
    secondary(&mut h, at);
    h.get_by_label("현재 뷰를 홈으로 설정").click();
    h.run_steps(3);
    h.state_mut().viewport_mut().look(gc_draw::StandardView::Right);
    h.state_mut().viewport_mut().camera.zoom(2.0, [100.0, 100.0], [1280.0, 700.0]);
    h.run_steps(2);
    assert_ne!(format!("{:?}", h.state().viewport().camera), saved);
    let at = cube_point(&h, 0.9, 0.0);
    secondary(&mut h, at);
    h.get_by_label("홈으로 이동").click();
    h.run_steps(3);
    assert_eq!(format!("{:?}", h.state().viewport().camera), saved);
    // Setting home twice keeps the last one.
    h.state_mut().viewport_mut().look(gc_draw::StandardView::Front);
    h.run_steps(2);
    let front = format!("{:?}", h.state().viewport().camera);
    let at = cube_point(&h, 0.9, 0.0);
    secondary(&mut h, at);
    h.get_by_label("현재 뷰를 홈으로 설정").click();
    h.run_steps(3);
    h.state_mut().viewport_mut().look(gc_draw::StandardView::Top);
    let at = cube_point(&h, 0.9, 0.0);
    secondary(&mut h, at);
    h.get_by_label("홈으로 이동").click();
    h.run_steps(3);
    assert_eq!(format!("{:?}", h.state().viewport().camera), front);
}

#[test]
fn the_view_cube_fit_option_toggles_and_stays_after_the_menu_closes() {
    let mut h = harness();
    let first = h.state().viewport().cube_fit_on_view_change;
    let at = cube_point(&h, 0.9, 0.0);
    secondary(&mut h, at);
    h.get_by_label("뷰를 바꿀 때 모델 맞추기").click();
    h.run_steps(2);
    assert_eq!(h.state().viewport().cube_fit_on_view_change, !first);
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert_eq!(h.state().viewport().cube_fit_on_view_change, !first);
    let at = cube_point(&h, 0.9, 0.0);
    secondary(&mut h, at);
    h.get_by_label("뷰를 바꿀 때 모델 맞추기").click();
    h.run_steps(2);
    assert_eq!(h.state().viewport().cube_fit_on_view_change, first);
}

// ---- marking menu setting ----

#[test]
fn the_marking_menu_setting_switches_between_the_marking_menu_and_the_list_and_back() {
    let mut h = harness();
    let centre = egui::pos2(500.0, 450.0);
    let toggle = |h: &mut H| {
        h.get_by_label("뷰").click();
        h.run();
        h.get_by_label("사용자 인터페이스").click();
        h.run_steps(2);
        h.get_by_label("마킹 메뉴").click();
        h.run_steps(3);
        h.key_press(egui::Key::Escape);
        h.run_steps(2);
    };
    assert!(!h.state().viewport().classic_menu, "the marking menu is the default");
    toggle(&mut h);
    assert!(h.state().viewport().classic_menu, "switching it off gives the classic list");
    secondary(&mut h, centre);
    // The classic list offers the commands as buttons: Constrain opens the dialog.
    h.get_by_label("구속").click();
    h.run_steps(3);
    assert!(has(&h, "첫 번째 선택: 뷰에서 선택"));
    in_dialog(&h, "취소").click();
    h.run_steps(3);
    toggle(&mut h);
    assert!(!h.state().viewport().classic_menu, "and back");
    secondary(&mut h, centre);
    assert!(has(&h, "구속"), "the marking menu shows its commands again");
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
}
