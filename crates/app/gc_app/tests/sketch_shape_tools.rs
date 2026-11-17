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
