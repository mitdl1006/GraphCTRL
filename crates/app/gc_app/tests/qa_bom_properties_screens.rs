//! QA boundary tests (kittest) for the Bill of Materials window's settings and Static Quantity box and for the
//! iProperties dialog's Status, Custom and Physical (Include QTY Overrides, inertia) tabs. Inventor 2026 help: To Work
//! with Bill of Materials GUID-7A1FC6F1-8833-4A76-9E2B-AA6FE5FDF223, About BOM Quantity
//! GUID-37BA8DB6-E39C-4C96-AAF1-E7B342F20D0E, iProperties Reference GUID-8ABF17C5-B9EB-47B5-92CF-9FBD335334E3,
//! Physical Tab Reference GUID-877564AC-78CE-4216-9913-003A2DE8D698. A screen action that cannot apply leaves the
//! document and the undo depth as they were; one that applies is one undo step that restores exactly.

#![allow(clippy::unwrap_used, clippy::panic, clippy::float_cmp, reason = "test code")]

use eframe::egui;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use gc_app::{GraphCtrlApp, RunOptions};
use gc_core::Id;
use gc_doc::{Body, ComponentFlag, IProperties, Occurrence, Placement, Solid, TreeKey};
use gc_session::BomView;

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

fn occ(b: Id<Body>) -> Occurrence {
    Occurrence::Item(b.erase())
}

fn part(h: &mut H, x: f64, number: &str, density: f64) -> Id<Body> {
    let s = &mut h.state_mut().session;
    let b =
        s.add_body(Solid::Box { size: [0.1; 3] }, Placement { xyz: [x, 0.0, 0.0], rpy: [0.0; 3] }).unwrap();
    let props = IProperties { part_number: number.into(), density: Some(density), ..IProperties::default() };
    s.set_properties(Some(occ(b)), props).unwrap();
    b
}

fn rows(h: &H) -> Vec<(String, u32)> {
    h.state()
        .session
        .bill_of_materials(BomView::PartsOnly)
        .into_iter()
        .map(|r| (r.part_number, r.quantity))
        .collect()
}

/// True when a label reads exactly this (the mass is written with a trailing space).
fn shows(h: &H, text: &str) -> bool {
    h.query_all_by_label_contains(text.trim()).any(|n| {
        let node = n.accesskit_node();
        let text_of = |s: Option<String>| s.is_some_and(|l| l.trim() == text.trim());
        text_of(node.label()) || text_of(node.value())
    })
}

fn open_bom(h: &mut H) {
    h.get_by_label("도구").click();
    h.run();
    h.get_by_label("자재 명세서(BOM)").click();
    h.run_steps(3);
}

#[test]
fn the_bom_settings_checkboxes_are_one_undo_step_each_and_the_excluded_list_is_read_as_a_list() {
    let mut h = harness();
    let a = part(&mut h, 0.0, "P-1", 1000.0);
    part(&mut h, 0.2, "P-1", 1000.0);
    part(&mut h, 0.4, "P-2", 1000.0);
    h.run();
    open_bom(&mut h);
    h.get_by_label("BOM 설정").click();
    h.run_steps(2);
    assert_eq!(rows(&h).len(), 2);
    let steps = depth(&h);
    h.get_by_label("부품 번호가 같으면 행 합치기 (Merge Rows on Part Number Match)").click();
    h.run_steps(3);
    assert_eq!(rows(&h).len(), 3, "merging is off: a row per instance");
    assert_eq!(depth(&h), steps + 1);
    h.get_by_label("부품 번호가 같으면 행 합치기 (Merge Rows on Part Number Match)").click();
    h.run_steps(3);
    assert_eq!(rows(&h).len(), 2);
    assert_eq!(depth(&h), steps + 2);
    // The excluded list: commas, spaces and empty entries are tidied; it is applied when the box loses focus.
    h.get_by_label("합치지 않을 부품 번호(쉼표로 구분)").click();
    h.run_steps(2);
    h.get_by_label("합치지 않을 부품 번호(쉼표로 구분)").type_text("P-1, ,P-2 ,,");
    h.key_press(egui::Key::Tab);
    h.run_steps(3);
    let excluded = h.state().session.bom_settings().merge_excluded;
    assert_eq!(excluded, ["P-1", "P-2"], "{excluded:?}");
    assert_eq!(rows(&h).len(), 3, "P-1 is excluded: its two instances stay apart");
    // Undo takes the list back and the screen follows.
    assert!(h.state_mut().session.undo());
    h.run_steps(3);
    assert!(h.state().session.bom_settings().merge_excluded.is_empty());
    assert_eq!(rows(&h).len(), 2);
    // Hide Suppressed: the row of a part that is suppressed goes (it would show 0 otherwise).
    h.state_mut().session.set_component(occ(a), ComponentFlag::Suppressed, true).unwrap();
    h.run_steps(2);
    assert_eq!(rows(&h)[0], ("P-1".to_owned(), 1));
    h.get_by_label("억제된 컴포넌트를 BOM에서 숨기기").click();
    h.run_steps(3);
    assert!(h.state().session.bom_settings().hide_suppressed);
}

/// The Static Quantity box is a drag/typing box (`DragValue`): kittest can neither open its text edit with a click nor
/// drag it, so what is typed or dragged there is left to the manual checklist. What the table shows and the cross that
/// puts the counted quantity back are checked here.
#[test]
fn a_static_quantity_is_shown_with_a_cross_that_counts_again_in_one_step_and_looking_changes_nothing() {
    let mut h = harness();
    let a = part(&mut h, 0.0, "P-1", 1000.0);
    part(&mut h, 0.2, "P-2", 1000.0);
    h.run();
    open_bom(&mut h);
    assert!(!has(&h, "×"), "no cross while every quantity is counted");
    let key = TreeKey::Component(occ(a));
    for n in [7, 0, 99_999, 100_000, u32::MAX] {
        h.state_mut().session.set_bom_quantity(key, Some(n)).unwrap();
        h.run_steps(3);
        let before = h.state().session.doc().clone();
        let steps = depth(&h);
        // Showing the row (and the box clamping what it shows to its range) must not write anything back.
        h.run_steps(5);
        assert!(h.state().session.doc().same_content(&before), "looking at {n} changed the document");
        assert_eq!(depth(&h), steps);
        assert_eq!(rows(&h)[0], ("P-1".to_owned(), n));
        assert!(has(&h, "×"), "a Static Quantity of {n} has its cross");
    }
    let steps = depth(&h);
    h.get_by_label("×").click();
    h.run_steps(3);
    assert_eq!(rows(&h)[0], ("P-1".to_owned(), 1));
    assert_eq!(depth(&h), steps + 1);
    assert!(!has(&h, "×"));
    assert_eq!(rows(&h)[1], ("P-2".to_owned(), 1), "the other row is untouched");
}

fn open_properties(h: &mut H) {
    h.get_by_label("도구").click();
    h.run();
    h.get_by_label("iProperties…").click();
    h.run_steps(3);
}

#[test]
fn the_physical_tab_shows_the_textbook_box_and_the_qty_option_changes_only_the_mass() {
    let mut h = harness();
    // 0.1 m boxes of 1000 kg/m3 weigh 1 kg; two of them with one part number, the first with a Static Quantity of ten.
    let a = part(&mut h, 0.0, "P-1", 1000.0);
    part(&mut h, 0.5, "P-1", 1000.0);
    h.state_mut().session.set_bom_quantity(TreeKey::Component(occ(a)), Some(10)).unwrap();
    h.run();
    open_properties(&mut h);
    h.get_by_label("물리적").click();
    h.run_steps(3);
    assert!(shows(&h, "2 kg"), "counted: two parts");
    let inertia_before: Vec<String> =
        h.query_all_by_label_contains("e-").map(|n| format!("{:?}", n.accesskit_node().label())).collect();
    h.get_by_label("수량 재정의 포함 (Include QTY Overrides)").click();
    h.get_by_label("업데이트").click();
    h.run_steps(3);
    assert!(shows(&h, "10 kg"), "ten of the 1 kg parts count");
    // The inertia does not follow the overrides.
    let inertia_after: Vec<String> =
        h.query_all_by_label_contains("e-").map(|n| format!("{:?}", n.accesskit_node().label())).collect();
    assert_eq!(inertia_before, inertia_after);
    h.get_by_label("수량 재정의 포함 (Include QTY Overrides)").click();
    h.get_by_label("업데이트").click();
    h.run_steps(3);
    assert!(shows(&h, "2 kg"));
}

#[test]
fn the_negative_integral_option_flips_only_the_products_of_inertia() {
    let mut h = harness();
    let p = part(&mut h, 0.0, "P-1", 1000.0);
    h.state_mut().session.free_move(p.erase(), Placement { xyz: [0.3, 0.2, 0.5], rpy: [0.0; 3] }).unwrap();
    h.run();
    open_properties(&mut h);
    h.get_by_label("물리적").click();
    h.run_steps(3);
    // m = 1 kg at (0.3, 0.2, 0.5): the products about the origin are m x y = 0.06 and so on.
    let m = h.state().session.inertial_properties(None).unwrap();
    assert!((m.global.ixy - 0.06).abs() < 1e-9);
    let shown = |h: &H, v: f64| has(h, &format!("{v:.6e}"));
    assert!(shown(&h, m.global.ixy), "the product of inertia is shown as an integral");
    h.get_by_label("관성곱을 음의 적분으로 표시 (Negative integral)").click();
    h.run_steps(3);
    assert!(shown(&h, -m.global.ixy), "negated");
    assert!(shown(&h, m.global.ixx), "the moments themselves are not negated");
    assert!(!shown(&h, -m.global.ixx));
}

#[test]
fn status_and_custom_properties_are_refused_whole_and_the_dialog_keeps_what_was_typed() {
    let mut h = harness();
    part(&mut h, 0.0, "P-1", 1000.0);
    h.run();
    let before = h.state().session.doc().clone();
    let steps = depth(&h);
    open_properties(&mut h);
    h.get_by_label("상태").click();
    h.run_steps(2);
    h.get_by_role_and_label(egui::accesskit::Role::TextInput, "상태").click();
    h.get_by_role_and_label(egui::accesskit::Role::TextInput, "상태").type_text("Released");
    h.run_steps(2);
    // Two properties with one name: OK is refused and nothing of the dialog is stored.
    h.get_by_label("사용자 정의").click();
    h.run_steps(2);
    for _ in 0..2 {
        h.get_by_label("속성 추가").click();
        h.run_steps(2);
    }
    for (label, text) in [("이름 1", "Batch"), ("값 1", "7"), ("이름 2", "Batch"), ("값 2", "8")] {
        h.get_by_label(label).click();
        h.run_steps(2);
        h.get_by_label(label).type_text(text);
        h.run_steps(2);
    }
    h.get_by_label("확인").click();
    h.run_steps(3);
    assert!(h.state().session.doc().same_content(&before), "a refused dialog stored something");
    assert_eq!(depth(&h), steps);
    assert!(has(&h, "속성 추가"), "the dialog stays open with what was typed");
    // Remove the second row with its cross: now OK stores the status and one property in one undo step.
    h.get_all_by_label("×").nth(1).unwrap().click();
    h.run_steps(3);
    h.get_by_label("확인").click();
    h.run_steps(3);
    let props = h.state().session.properties(None);
    assert_eq!(props.status, "Released");
    assert_eq!(props.custom.len(), 1);
    assert_eq!(props.custom_value("Batch"), Some("7"));
    assert_eq!(depth(&h), steps + 1, "OK is one undo step");
    assert!(h.state_mut().session.undo());
    assert!(h.state().session.doc().same_content(&before));
}

#[test]
fn closing_the_properties_dialog_with_edits_stores_nothing() {
    let mut h = harness();
    part(&mut h, 0.0, "P-1", 1000.0);
    h.run();
    let before = h.state().session.doc().clone();
    let steps = depth(&h);
    open_properties(&mut h);
    h.get_by_label("사용자 정의").click();
    h.run_steps(2);
    h.get_by_label("속성 추가").click();
    h.run_steps(2);
    h.get_by_label("이름 1").click();
    h.get_by_label("이름 1").type_text("Typed but not kept");
    h.run_steps(2);
    h.get_by_label("취소").click();
    h.run_steps(3);
    assert!(h.state().session.doc().same_content(&before));
    assert_eq!(depth(&h), steps);
    // A new dialog starts from the document, not from the cancelled draft.
    open_properties(&mut h);
    h.get_by_label("사용자 정의").click();
    h.run_steps(2);
    assert!(!has(&h, "이름 1"), "the cancelled draft came back");
}
