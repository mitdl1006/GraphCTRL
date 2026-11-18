//! The Point, Polygon, Slot and Offset tools and the sketch Mirror and Pattern dialogs through the ribbon
//! (Inventor 2026 help: To Work with Points, GUID-D4A4288F-32F1-4E23-A382-930EBA152E87; To Create 2D Shape
//! Geometry, GUID-D489CE6D-7299-4211-A43A-F3580A4BA357; To Stretch, Scale, or Offset 2D Sketch Geometry,
//! GUID-962CEE71-8BE4-4DA6-B51E-341319C64FA4; To Create and Edit Sketch Patterns,
//! GUID-AAD3A699-FED8-4614-A31D-0A2316328CEE).

#![allow(clippy::unwrap_used, reason = "test code: failures should panic")]

use eframe::egui;
use egui_kittest::kittest::Queryable as _;
use gc_app::{GraphCtrlApp, RunOptions};
use gc_doc::SlotKind;
use gc_tools::{ToolEvent, ToolKind};

const SIZE: egui::Vec2 = egui::vec2(1280.0, 800.0);
const PX: f64 = 1e-4;

type Harness<'a> = egui_kittest::Harness<'a, GraphCtrlApp>;

fn mm(x: f64, y: f64) -> [f64; 2] {
    [x * 1e-3, y * 1e-3]
}

/// A click in the sketch; `add` holds Ctrl (adds to the selection with the Select tool).
fn click(app: &mut GraphCtrlApp, at: [f64; 2], add: bool) {
    app.session.tool_event(ToolEvent::Move { pos: at, px: PX, suppress_snap: true });
    app.session.tool_event(ToolEvent::Click { pos: at, px: PX, modifier: add });
}

fn sketching() -> Harness<'static> {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("XY 스케치").click();
    harness.run();
    harness
}

fn curves(harness: &Harness<'_>) -> usize {
    let (_, sketch) = harness.state().session.active_sketch().unwrap();
    sketch.curves.values().filter(|c| !c.construction).count()
}

fn line(app: &mut GraphCtrlApp, a: [f64; 2], b: [f64; 2]) {
    app.session.set_tool(ToolKind::Line);
    click(app, a, false);
    click(app, b, false);
    app.session.set_tool(ToolKind::Select);
}

#[test]
fn the_point_button_places_points_and_the_center_point_toggle_switches_their_kind() {
    let mut harness = sketching();
    harness.get_by_label("점").click();
    harness.run();
    assert_eq!(harness.state().session.tool(), ToolKind::Point);
    harness.get_by_label("점을 놓을 곳을 클릭하십시오");
    click(harness.state_mut(), mm(10.0, 10.0), false);
    harness.get_by_label("중심점").click();
    harness.run();
    assert!(!harness.state().session.center_point());
    click(harness.state_mut(), mm(30.0, 10.0), false);
    let (_, sketch) = harness.state().session.active_sketch().unwrap();
    let placed: Vec<bool> = sketch.points.values().filter(|p| p.pos[1] > 0.005).map(|p| p.center).collect();
    assert_eq!(placed.len(), 2);
    assert_eq!(placed.iter().filter(|c| **c).count(), 1, "one centre point, one sketch point");
}

#[test]
fn the_polygon_button_opens_its_options_and_draws_the_polygon() {
    let mut harness = sketching();
    harness.get_by_label("다각형").click();
    harness.run();
    assert_eq!(harness.state().session.tool(), ToolKind::Polygon);
    harness.get_by_label("다각형 옵션");
    harness.get_by_label("외접").click();
    harness.run();
    assert_eq!(harness.state().session.polygon_settings(), (6, true));
    click(harness.state_mut(), mm(0.0, 0.0), false);
    click(harness.state_mut(), mm(20.0, 5.0), false);
    harness.run();
    assert_eq!(curves(&harness), 6);
    harness.get_by_label("완료").click();
    harness.run();
    assert_eq!(harness.state().session.tool(), ToolKind::Select, "Done leaves the tool");
}

#[test]
fn the_slot_buttons_start_each_slot_kind() {
    let mut harness = sketching();
    harness.get_by_label("슬롯").click();
    harness.run();
    assert_eq!(harness.state().session.tool(), ToolKind::Slot(SlotKind::CenterToCenter));
    harness.get_by_label("전체 슬롯").click();
    harness.run();
    assert_eq!(harness.state().session.tool(), ToolKind::Slot(SlotKind::Overall));
    harness.get_by_label("슬롯의 첫 점을 클릭하십시오");
    click(harness.state_mut(), mm(0.0, 0.0), false);
    click(harness.state_mut(), mm(40.0, 0.0), false);
    click(harness.state_mut(), mm(20.0, 5.0), false);
    harness.run();
    assert_eq!(curves(&harness), 4, "two lines and two arcs");
}

#[test]
fn the_offset_button_offsets_a_picked_loop_and_shows_its_toggles() {
    let mut harness = sketching();
    harness.state_mut().session.set_tool(ToolKind::Rectangle);
    click(harness.state_mut(), mm(0.0, 0.0), false);
    click(harness.state_mut(), mm(40.0, 20.0), false);
    harness.get_by_label("간격띄우기").click();
    harness.run();
    assert_eq!(harness.state().session.tool(), ToolKind::Offset);
    harness.get_by_label("간격띄우기 옵션");
    harness.get_by_label("간격띄우기할 곡선을 선택하십시오");
    click(harness.state_mut(), mm(20.0, 0.0), false);
    click(harness.state_mut(), mm(20.0, -5.0), false);
    harness.run();
    assert_eq!(curves(&harness), 8, "the whole loop is offset");
}

#[test]
fn the_mirror_dialog_takes_clicks_in_the_viewport_while_it_is_open() {
    let mut harness = sketching();
    line(harness.state_mut(), mm(0.0, 0.0), mm(10.0, 10.0));
    line(harness.state_mut(), mm(20.0, -10.0), mm(20.0, 30.0));
    click(harness.state_mut(), mm(5.0, 5.0), false);
    harness.get_by_label("미러").click();
    harness.run();
    harness.get_by_label("미러 패턴");
    harness.get_by_label("형상: 곡선 1개");
    harness.get_by_label("패턴할 형상을 클릭하십시오(다시 클릭하면 빠짐)");
    // The Mirror Line selector, then the line in the graphics window.
    harness.get_by_label("대칭선: 선택 안 됨").click();
    harness.run();
    harness.get_by_label("선을 클릭하십시오");
    click(harness.state_mut(), mm(20.0, 10.0), false);
    harness.run();
    harness.get_by_label("대칭선: 선택됨");
    harness.get_by_label("형상: 곡선 1개");
    harness.get_by_label("적용").click();
    harness.run();
    assert_eq!(curves(&harness), 3);
    harness.get_by_label("완료").click();
    harness.run();
    assert!(harness.query_by_label("미러 패턴").is_none(), "Done closes the dialog");
    assert!(harness.state().session.sketch_pick().is_none(), "and ends picking");
}

#[test]
fn the_rectangular_pattern_dialog_collects_geometry_by_plain_clicks() {
    let mut harness = sketching();
    line(harness.state_mut(), mm(0.0, 0.0), mm(0.0, 10.0));
    line(harness.state_mut(), mm(5.0, 0.0), mm(5.0, 10.0));
    line(harness.state_mut(), mm(0.0, -10.0), mm(40.0, -10.0));
    harness.get_by_label("직사각형").click();
    harness.run();
    harness.get_by_label("직사각형 패턴");
    // Each click adds (no Ctrl), a click on nothing keeps the picks, a second click removes.
    click(harness.state_mut(), mm(0.0, 5.0), false);
    click(harness.state_mut(), mm(5.0, 5.0), false);
    click(harness.state_mut(), mm(30.0, 30.0), false);
    harness.run();
    harness.get_by_label("형상: 곡선 2개");
    click(harness.state_mut(), mm(5.0, 5.0), false);
    harness.run();
    harness.get_by_label("형상: 곡선 1개");
    harness.get_by_label("방향 1: 선택 안 됨").click();
    harness.run();
    click(harness.state_mut(), mm(20.0, -10.0), false);
    harness.run();
    harness.get_by_label("방향 1: 선택됨");
    harness.get_by_label("적용").click();
    harness.run();
    assert_eq!(curves(&harness), 4, "count 2: one copy of the one picked line");
}

#[test]
fn the_circular_pattern_dialog_takes_a_circle_click_as_its_centre_axis() {
    let mut harness = sketching();
    harness.state_mut().session.set_tool(ToolKind::Circle);
    click(harness.state_mut(), mm(50.0, 50.0), false);
    click(harness.state_mut(), mm(52.0, 50.0), false);
    click(harness.state_mut(), mm(70.0, 50.0), false);
    click(harness.state_mut(), mm(75.0, 50.0), false);
    harness.state_mut().session.set_tool(ToolKind::Select);
    harness.get_by_label("원형").click();
    harness.run();
    harness.get_by_label("원형 패턴");
    click(harness.state_mut(), mm(75.0, 50.0), false);
    harness.get_by_label("축: 선택 안 됨").click();
    harness.run();
    click(harness.state_mut(), mm(52.0, 50.0), false);
    harness.run();
    harness.get_by_label("축: 선택됨");
    harness.get_by_label("형상: 곡선 1개");
    harness.get_by_label("적용").click();
    harness.run();
    assert_eq!(curves(&harness), 7, "the small circle and six around it");
}

#[test]
fn another_command_closes_the_dialog() {
    let mut harness = sketching();
    harness.get_by_label("미러").click();
    harness.run();
    harness.get_by_label("미러 패턴");
    harness.get_by_label("선").click();
    harness.run();
    harness.run();
    assert!(harness.query_by_label("미러 패턴").is_none());
}

#[test]
fn the_arc_slot_buttons_draw_arc_slots() {
    let mut harness = sketching();
    harness.get_by_label("중심점 호 슬롯").click();
    harness.run();
    assert_eq!(harness.state().session.tool(), ToolKind::ArcSlot(gc_doc::ArcSlotKind::CenterPoint));
    harness.get_by_label("슬롯 중심 호의 중심을 클릭하십시오");
    for at in [mm(0.0, 0.0), mm(30.0, 0.0), mm(0.0, 30.0), mm(35.0, 0.0)] {
        click(harness.state_mut(), at, false);
    }
    harness.run();
    assert_eq!(curves(&harness), 4, "outer, inner and two ends");
    harness.get_by_label("3점 호 슬롯").click();
    harness.run();
    assert_eq!(harness.state().session.tool(), ToolKind::ArcSlot(gc_doc::ArcSlotKind::ThreePoint));
}

/// Edit Rectangular and Circular Patterns (GUID-AAD3A699-FED8-4614-A31D-0A2316328CEE): the dialog opened
/// on a pattern edits it, suppresses elements and deletes it.
#[test]
fn a_selected_pattern_opens_its_dialog_to_edit_suppress_and_delete() {
    let mut harness = sketching();
    harness.state_mut().session.set_tool(ToolKind::Point);
    click(harness.state_mut(), mm(50.0, 50.0), false);
    harness.state_mut().session.set_tool(ToolKind::Circle);
    click(harness.state_mut(), mm(70.0, 50.0), false);
    click(harness.state_mut(), mm(75.0, 50.0), false);
    harness.state_mut().session.set_tool(ToolKind::Select);
    click(harness.state_mut(), mm(75.0, 50.0), false);
    harness.get_by_label("원형").click();
    harness.run();
    harness.get_by_label("축: 선택 안 됨").click();
    harness.run();
    click(harness.state_mut(), mm(50.0, 50.0), false);
    harness.run();
    harness.get_by_label("적용").click();
    harness.run();
    harness.get_by_label("완료").click();
    harness.run();
    assert_eq!(curves(&harness), 6);

    // The copy opposite the original, then the Circular button: the dialog edits the pattern.
    click(harness.state_mut(), mm(25.0, 50.0), false);
    harness.get_by_label("원형").click();
    harness.run();
    harness.get_by_label("억제: 요소 0개");
    harness.get_by_label("축: 선택됨");
    click(harness.state_mut(), mm(25.0, 50.0), false);
    harness.run();
    harness.get_by_label("억제: 요소 1개");
    harness.get_by_label("확인").click();
    harness.run();
    assert_eq!(curves(&harness), 5, "one element suppressed (dashed)");
    assert!(harness.query_by_label("원형 패턴").is_none(), "OK closes the dialog");

    click(harness.state_mut(), mm(75.0, 50.0), false);
    harness.get_by_label("원형").click();
    harness.run();
    harness.get_by_label("억제: 요소 1개");
    harness.get_by_label("패턴 삭제").click();
    harness.run();
    assert_eq!(curves(&harness), 1, "the original alone");
    let (_, sketch) = harness.state().session.active_sketch().unwrap();
    assert!(sketch.patterns.is_empty());
}

/// Draw Points Using Dynamic Input (Inventor 2026 help, GUID-33A8B11D-A04C-4809-BC82-C0419215915F): the
/// X and Y boxes follow the cursor; typed values place the point.
#[test]
fn the_point_tool_takes_typed_x_and_y_in_the_heads_up_boxes() {
    let mut harness = sketching();
    harness.get_by_label("점").click();
    harness.run();
    harness.event(egui::Event::PointerMoved(egui::pos2(640.0, 400.0)));
    harness.run();
    harness.get_by_label("HUD X");
    harness.get_by_label("HUD Y");
    for text in ["1", "0"] {
        harness.event(egui::Event::Text(text.into()));
        harness.run();
    }
    harness.key_press(egui::Key::Tab);
    harness.run();
    for text in ["-", "5"] {
        harness.event(egui::Event::Text(text.into()));
        harness.run();
    }
    harness.key_press(egui::Key::Enter);
    harness.run();
    let (_, sketch) = harness.state().session.active_sketch().unwrap();
    let placed = sketch.points.values().find(|p| p.pos[1] < 0.0).unwrap();
    assert!((placed.pos[0] - 0.01).abs() < 1e-9 && (placed.pos[1] + 0.005).abs() < 1e-9, "{:?}", placed.pos);
}

/// The 3D Sketch tab's Constrain panel (Inventor 2026 help, To Apply Geometric Constraints,
/// GUID-A85A7A30-7D81-4C75-8769-CAD034EEA930) has Parallel, Perpendicular, Collinear and Equal too.
#[test]
fn the_three_dimensional_sketch_offers_parallel_perpendicular_and_equal() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("3D 스케치").click();
    harness.run();
    assert!(harness.state().session.active_sketch().unwrap().1.is_spatial());
    for (label, kind) in [
        ("평행", gc_tools::ConstraintKind::Parallel),
        ("직각", gc_tools::ConstraintKind::Perpendicular),
        ("동일", gc_tools::ConstraintKind::Equal),
    ] {
        harness.get_by_label(label).click();
        harness.run();
        assert_eq!(harness.state().session.tool(), ToolKind::Constraint(kind));
    }
    // And Center Point Arc and Three Point Arc (To Create Lines, Arcs, and Bends,
    // GUID-4F861674-F74C-474E-869A-4C190ED6EC4B).
    for (label, kind) in [("호", ToolKind::Arc), ("3점 호", ToolKind::Arc3)] {
        harness.get_by_label(label).click();
        harness.run();
        assert_eq!(harness.state().session.tool(), kind);
    }
}

/// Cancel a Sketch (Inventor 2026 help, To Create and Edit Sketches,
/// GUID-E96FB3C4-136B-4551-BF45-F54594543ECB): the ribbon's Cancel Sketch throws a new sketch away.
#[test]
fn cancel_sketch_on_the_ribbon_throws_a_new_sketch_away() {
    let mut harness = sketching();
    harness.state_mut().session.set_tool(ToolKind::Circle);
    click(harness.state_mut(), mm(0.0, 0.0), false);
    click(harness.state_mut(), mm(5.0, 0.0), false);
    harness.run();
    harness.get_by_label("스케치 취소").click();
    harness.run();
    assert!(harness.state().session.active_sketch().is_none());
    assert_eq!(harness.state().session.doc().sketches().len(), 0);
    harness.get_by_label("XY 스케치");
}

#[test]
fn the_ribbon_starts_midpoint_line_tangent_circle_and_the_center_rectangles() {
    let mut harness = sketching();
    for (label, kind) in [
        ("중점 선", ToolKind::MidpointLine),
        ("접선 원", ToolKind::TangentCircle),
        ("2점 중심 직사각형", ToolKind::CenterRectangle),
        ("3점 중심 직사각형", ToolKind::CenterRectangle3),
    ] {
        harness.get_by_label(label).click();
        harness.run();
        assert_eq!(harness.state().session.tool(), kind);
    }
    harness.get_by_label("직사각형의 중심을 클릭하십시오");
}

#[test]
fn split_and_chamfer_on_the_ribbon_and_the_chamfer_dialog() {
    let mut harness = sketching();
    harness.get_by_label("분할").click();
    harness.run();
    assert_eq!(harness.state().session.tool(), ToolKind::Split);
    harness.get_by_label("모따기").click();
    harness.run();
    assert_eq!(harness.state().session.tool(), ToolKind::Chamfer);
    harness.get_by_label("2D 모따기");
    harness.get_by_label("다른 거리").click();
    harness.run();
    assert!(matches!(harness.state().session.chamfer_options().0, gc_doc::ChamferKind::Unequal(..)));
    harness.get_by_label("거리 2");
    harness.get_by_label("확인").click();
    harness.run();
    assert_eq!(harness.state().session.tool(), ToolKind::Select, "OK leaves the tool");
}
