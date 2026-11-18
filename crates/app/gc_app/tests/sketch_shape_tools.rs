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

/// Chooses a variant of a split button of the sketch ribbon: the arrow ("<group> 더 보기") opens the list,
/// the variant's name picks it. (The arrow is drawn with `ui.interact`, so it takes pointer events.)
fn pick_variant(harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>, group: &str, variant: &str) {
    let at = harness.get_by_label(&format!("{group} 더 보기")).rect().center();
    harness.event(egui::Event::PointerMoved(at));
    harness.run_steps(3);
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run_steps(3);
    }
    harness.get_by_label(variant).click();
    harness.run_steps(3);
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
    pick_variant(&mut harness, "슬롯", "전체 슬롯");
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
    pick_variant(&mut harness, "슬롯", "중심점 호 슬롯");
    assert_eq!(harness.state().session.tool(), ToolKind::ArcSlot(gc_doc::ArcSlotKind::CenterPoint));
    harness.get_by_label("슬롯 중심 호의 중심을 클릭하십시오");
    for at in [mm(0.0, 0.0), mm(30.0, 0.0), mm(0.0, 30.0), mm(35.0, 0.0)] {
        click(harness.state_mut(), at, false);
    }
    harness.run();
    assert_eq!(curves(&harness), 4, "outer, inner and two ends");
    pick_variant(&mut harness, "슬롯", "3점 호 슬롯");
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
    pick_variant(&mut harness, "스케치", "3D 스케치");
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
    for (label, kind) in [("중심점 호", ToolKind::Arc), ("3점 호", ToolKind::Arc3)] {
        pick_variant(&mut harness, "호", label);
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
    for (group, label, kind) in [
        ("선", "중점 선", ToolKind::MidpointLine),
        ("원", "접선 원", ToolKind::TangentCircle),
        ("직사각형", "2점 중심 직사각형", ToolKind::CenterRectangle),
        ("직사각형", "3점 중심 직사각형", ToolKind::CenterRectangle3),
    ] {
        pick_variant(&mut harness, group, label);
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

#[test]
fn move_copy_and_rotate_on_the_ribbon_open_their_dialogs() {
    let mut harness = sketching();
    for (label, kind) in [("이동", ToolKind::Move), ("복사", ToolKind::Copy), ("회전", ToolKind::Rotate)]
    {
        harness.get_by_label(label).click();
        harness.run();
        assert_eq!(harness.state().session.tool(), kind);
        harness.get_by_label("선택: 곡선 0개(명령 전에 고르기)");
    }
    harness.get_by_label("구속 풀기(치수 다시 계산·구속 끊기)").click();
    harness.run();
    assert!(harness.state().session.transform_options().1);
    harness.get_by_label("완료").click();
    harness.run();
    assert_eq!(harness.state().session.tool(), ToolKind::Select);
}

/// Automatic Dimensions and Constraints (Inventor 2026 help, GUID-1DC71C6F-9F6D-4846-8E73-9CB7D3452882):
/// Apply fully constrains a sketch, Remove takes it back.
#[test]
fn auto_dimension_applies_and_removes() {
    let mut harness = sketching();
    harness.state_mut().session.set_tool(ToolKind::Rectangle);
    click(harness.state_mut(), mm(10.0, 10.0), false);
    click(harness.state_mut(), mm(40.0, 30.0), false);
    harness.state_mut().session.set_tool(ToolKind::Select);
    harness.run();
    harness.get_by_label("자동 치수 및 구속").click();
    harness.run();
    harness.get_by_label("자동 치수");
    assert!(harness.state().session.dimensions_required() > 0);
    harness.get_by_label("적용").click();
    harness.run();
    assert_eq!(harness.state().session.dimensions_required(), 0);
    harness.get_by_label("필요한 치수: 0");
    harness.get_by_label("제거").click();
    harness.run();
    assert!(harness.state().session.dimensions_required() > 0, "taken back");
    harness.get_by_label("완료").click();
    harness.run();
    assert!(harness.query_by_label("자동 치수").is_none());
}

/// Flip Direction (To Work with Sketch Dimensions, GUID-026FE8D3-AFBB-4834-BC18-73D9CFF78185): enabled
/// for a selected dimension with a direction, it puts the geometry on the other side.
#[test]
fn flip_direction_on_the_ribbon() {
    use gc_doc::SketchOp;
    let mut harness = sketching();
    let app = harness.state_mut();
    app.session.set_tool(ToolKind::Point);
    click(app, mm(0.0, 20.0), false);
    let (_, sketch) = app.session.active_sketch().unwrap();
    let origin = *sketch.points.keys().find(|p| sketch.pos(**p) == Some([0.0; 3])).unwrap();
    let point = *sketch.points.keys().find(|p| sketch.pos(**p).is_some_and(|q| q[1] > 0.0)).unwrap();
    let created = app
        .session
        .edit_active_sketch(SketchOp::AddDimension {
            kind: gc_sketch::DimKind::VDistance { p: origin, q: point, sign: 1.0 },
            source: "20".into(),
            text_pos: [0.0; 2],
        })
        .unwrap();
    let dim = created
        .iter()
        .find_map(|c| if let gc_doc::Created::Dimension(d) = c { Some(*d) } else { None })
        .unwrap();
    app.session.set_tool(ToolKind::Select);
    app.session.tool_event(ToolEvent::ClickItem {
        item: Some(gc_sketch::Item::Dimension(dim)),
        pos: [0.0; 2],
        modifier: false,
    });
    assert!(app.session.can_flip_selected());
    harness.run();
    harness.get_by_label("방향 뒤집기").click();
    harness.run();
    let (_, sketch) = harness.state().session.active_sketch().unwrap();
    assert!((sketch.pos(point).unwrap()[1] + 0.02).abs() < 1e-9, "below the origin now");
}

#[test]
fn scale_and_stretch_on_the_ribbon() {
    let mut harness = sketching();
    for (label, kind) in [("축척", ToolKind::Scale), ("신축", ToolKind::Stretch)] {
        harness.get_by_label(label).click();
        harness.run();
        assert_eq!(harness.state().session.tool(), kind);
        harness.get_by_label("기준점을 클릭하십시오(선택한 형상)");
    }
}

/// Trim shows the piece a click would remove, in red and dashed (Inventor 2026 help, To Trim, Extend, or
/// Split Sketch Geometry, GUID-DD00037F-8EE7-4C7C-9CF6-7B7B3C0F1F36 family: the part to go is highlighted
/// under the cursor). The screenshot is the evidence; the preview itself is the session's.
#[test]
fn trim_highlights_the_piece_that_would_go() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("XY 스케치").click();
    harness.run();
    let app = harness.state_mut();
    line(app, mm(-30.0, 0.0), mm(30.0, 0.0));
    line(app, mm(10.0, -20.0), mm(10.0, 20.0));
    app.session.set_tool(ToolKind::Trim);
    app.session.tool_event(ToolEvent::Move { pos: mm(-5.0, 0.0), px: PX, suppress_snap: true });
    assert_eq!(app.session.preview().cut.len(), 1, "one piece would go");
    harness.run_steps(3);
    if gpu {
        let image = harness.render().unwrap();
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        image.save(out.join("review-57-trim-preview.png")).unwrap();
    }
}

/// The Spline (Interpolation) tool is in the Line split button; clicks place fit points, Enter finishes
/// (backend: spline_tool.rs); the curve is drawn through the points. The screenshot is the evidence.
#[test]
fn the_spline_tool_is_in_the_line_split_button_and_draws_a_curve() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("XY 스케치").click();
    harness.run();
    pick_variant(&mut harness, "선", "스플라인(보간)");
    assert_eq!(harness.state().session.tool(), ToolKind::Spline);
    let app = harness.state_mut();
    for p in [mm(-60.0, 0.0), mm(-20.0, 40.0), mm(20.0, -30.0), mm(60.0, 20.0)] {
        click(app, p, false);
    }
    app.session.tool_event(ToolEvent::Key(gc_tools::Key::Enter));
    harness.run_steps(3);
    let (_, sketch) = harness.state().session.active_sketch().unwrap();
    assert!(
        sketch.curves.values().any(|c| matches!(c.geom, gc_sketch::CurveGeom::Spline { .. })),
        "a spline was made"
    );
    if gpu {
        let image = harness.render().unwrap();
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        image.save(out.join("review-60-spline.png")).unwrap();
    }
}

/// Right-clicking an inner fit point of a spline offers Split Spline (Inventor 2026 help, To Create, Lock
/// Spline Length, and Edit Splines, GUID-DB15C479-66F2-456A-9E54-40A60F83DC92: "To split a spline,
/// right-click a point and choose Split Spline").
#[test]
fn the_right_click_menu_of_a_spline_fit_point_splits_the_spline() {
    let mut harness = sketching();
    harness.state_mut().session.set_tool(ToolKind::Spline);
    let points = [mm(-60.0, 0.0), mm(-20.0, 40.0), mm(20.0, -30.0), mm(60.0, 20.0)];
    for p in points {
        click(harness.state_mut(), p, false);
    }
    harness.state_mut().session.tool_event(ToolEvent::Key(gc_tools::Key::Enter));
    harness.state_mut().session.set_tool(ToolKind::Select);
    harness.run_steps(3);
    let splines = |h: &Harness<'_>| {
        let (_, sketch) = h.state().session.active_sketch().unwrap();
        sketch.curves.values().filter(|c| matches!(c.geom, gc_sketch::CurveGeom::Spline { .. })).count()
    };
    assert_eq!(splines(&harness), 1);
    // A press and release without moving opens the menu list.
    let at =
        harness.state().viewport().screen_of(gc_math::DVec3::new(points[1][0], points[1][1], 0.0)).unwrap();
    harness.event(egui::Event::PointerMoved(at));
    harness.run_steps(2);
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    harness.run_steps(3);
    harness.get_by_label("스플라인 분할").click();
    harness.run_steps(3);
    assert_eq!(splines(&harness), 2, "the spline was cut in two at the point");
}

/// The Offset tool takes an ellipse and a spline like any other curve (Inventor 2026 help, To Create 2D Shape
/// Geometry, GUID-D489CE6D-7299-4211-A43A-F3580A4BA357). The screenshot, with the previews of both and the
/// results, is the evidence.
#[test]
fn offset_works_on_an_ellipse_and_a_spline_on_screen() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("XY 스케치").click();
    harness.run();
    let app = harness.state_mut();
    app.session.set_tool(ToolKind::Ellipse);
    for p in [mm(-40.0, 30.0), mm(0.0, 30.0), mm(-40.0, 50.0)] {
        click(app, p, false);
    }
    app.session.set_tool(ToolKind::Spline);
    for p in [mm(20.0, 10.0), mm(40.0, 50.0), mm(60.0, 20.0), mm(80.0, 55.0)] {
        click(app, p, false);
    }
    app.session.tool_event(ToolEvent::Key(gc_tools::Key::Enter));
    let curves = |app: &GraphCtrlApp| {
        let (_, sketch) = app.session.active_sketch().unwrap();
        sketch.curves.values().filter(|c| !c.construction).count()
    };
    let before = curves(app);
    app.session.set_tool(ToolKind::Offset);
    // Pick the ellipse (on its right end), then the side and distance; the same for the spline.
    for p in [mm(0.0, 30.0), mm(10.0, 30.0)] {
        app.session.tool_event(ToolEvent::Move { pos: p, px: PX, suppress_snap: true });
        app.session.tool_event(ToolEvent::Click { pos: p, px: PX, modifier: false });
    }
    // The spline's fit point at (40, 50) mm, then a click above it.
    for p in [mm(40.0, 50.0), mm(40.0, 58.0)] {
        app.session.tool_event(ToolEvent::Move { pos: p, px: PX, suppress_snap: true });
        app.session.tool_event(ToolEvent::Click { pos: p, px: PX, modifier: false });
    }
    assert!(curves(app) >= before + 2, "an offset of each: {before} -> {}", curves(app));
    harness.run_steps(3);
    if gpu {
        let image = harness.render().unwrap();
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        image.save(out.join("review-62-offset.png")).unwrap();
    }
}

/// A press and release of the right button without moving opens the menu list at `at`.
fn right_click(harness: &mut Harness<'_>, at: egui::Pos2) {
    harness.event(egui::Event::PointerMoved(at));
    harness.run_steps(2);
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    harness.run_steps(3);
}

fn spline_with_four_points() -> (Harness<'static>, [[f64; 2]; 4]) {
    let mut harness = sketching();
    harness.state_mut().session.set_tool(ToolKind::Spline);
    let points = [mm(-60.0, 0.0), mm(-20.0, 40.0), mm(20.0, -30.0), mm(60.0, 20.0)];
    for p in points {
        click(harness.state_mut(), p, false);
    }
    harness.state_mut().session.tool_event(ToolEvent::Key(gc_tools::Key::Enter));
    harness.state_mut().session.set_tool(ToolKind::Select);
    harness.run_steps(3);
    (harness, points)
}

fn spline_points(harness: &Harness<'_>) -> Vec<gc_core::Id<gc_sketch::SkPoint>> {
    let (_, sketch) = harness.state().session.active_sketch().unwrap();
    sketch
        .curves
        .values()
        .find_map(|c| {
            if let gc_sketch::CurveGeom::Spline { points } = &c.geom { Some(points.clone()) } else { None }
        })
        .unwrap_or_default()
}

/// Close Curve at the start of an open spline, Insert Point on it (Inventor 2026 help, To Create, Lock Spline
/// Length, and Edit Splines, GUID-DB15C479-66F2-456A-9E54-40A60F83DC92: "To close a curve, right-click the
/// starting point and choose Close Curve"; "right-click the spline and choose Insert Point. Then click the
/// curve").
#[test]
fn the_spline_menu_inserts_points_and_closes_the_curve() {
    let (mut harness, points) = spline_with_four_points();
    assert_eq!(spline_points(&harness).len(), 4);
    let screen = |h: &Harness<'_>, p: [f64; 2]| {
        h.state().viewport().screen_of(gc_math::DVec3::new(p[0], p[1], 0.0)).unwrap()
    };

    // Insert Point: the menu on the curve between two fit points, then clicks on the curve, Esc ends.
    let between = [(points[0][0] + points[1][0]) / 2.0 + 0.0, (points[0][1] + points[1][1]) / 2.0];
    // The curve passes near the chord's middle; find a point of its outline.
    let (_, sketch) = harness.state().session.active_sketch().unwrap();
    let outline = sketch.curves.values().find_map(|c| sketch.spline_outline(&c.geom, 16)).unwrap();
    let on_curve = outline[outline.len() / 6];
    let _ = between;
    let at = screen(&harness, on_curve);
    right_click(&mut harness, at);
    harness.get_by_label("점 삽입").click();
    harness.run_steps(3);
    // The mode shows its hint as a label, so a screen test can see it.
    harness.get_by_label("스플라인 위를 클릭해 점을 삽입합니다 (Esc로 끝)");
    let target = screen(&harness, outline[outline.len() / 2]);
    // A click far from the curve inserts nothing.
    let far = egui::pos2(target.x, target.y + 160.0);
    harness.event(egui::Event::PointerMoved(far));
    harness.run_steps(2);
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: far,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run_steps(2);
    }
    assert_eq!(spline_points(&harness).len(), 4, "far from the curve nothing is inserted");
    harness.event(egui::Event::PointerMoved(target));
    harness.run_steps(2);
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: target,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run_steps(2);
    }
    assert_eq!(spline_points(&harness).len(), 5, "a fit point was inserted");
    harness.key_press(egui::Key::Escape);
    harness.run_steps(3);
    // Esc ended it: a further click on the curve inserts nothing.
    let again = screen(&harness, outline[outline.len() * 2 / 3]);
    harness.event(egui::Event::PointerMoved(again));
    harness.run_steps(2);
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: again,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run_steps(2);
    }
    assert_eq!(spline_points(&harness).len(), 5);

    // Close Curve: right-click the start point.
    let start = screen(&harness, points[0]);
    right_click(&mut harness, start);
    harness.get_by_label("곡선 닫기").click();
    harness.run_steps(3);
    let fit = spline_points(&harness);
    assert_eq!(fit.first(), fit.last(), "the spline is closed");
}

/// A curve the Offset tool has picked is highlighted, and Extend takes an open spline up to a boundary
/// (Inventor 2026 help, GUID-D489CE6D-7299-4211-A43A-F3580A4BA357 and trim/extend GUID-DD00037F-6BBA-4B95-AA90-4F99DCEB667D).
/// The screenshots are the evidence.
#[test]
fn offset_highlights_its_pick_and_extend_takes_an_open_spline_to_a_boundary() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("XY 스케치").click();
    harness.run();
    let app = harness.state_mut();
    app.session.set_tool(ToolKind::Spline);
    for p in [mm(0.0, 0.0), mm(20.0, 25.0), mm(40.0, 10.0)] {
        click(app, p, false);
    }
    app.session.tool_event(ToolEvent::Key(gc_tools::Key::Enter));
    line(app, mm(70.0, -80.0), mm(70.0, 60.0));
    let evidence = |harness: &mut Harness<'_>, name: &str| {
        if gpu {
            harness.run_steps(3);
            let image = harness.render().unwrap();
            let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
            std::fs::create_dir_all(&out).unwrap();
            image.save(out.join(name)).unwrap();
        }
    };
    // Offset: the picked spline is in the tool's preview and drawn heavy.
    let app = harness.state_mut();
    app.session.set_tool(ToolKind::Offset);
    let on_spline = mm(20.0, 25.0);
    app.session.tool_event(ToolEvent::Move { pos: on_spline, px: PX, suppress_snap: true });
    app.session.tool_event(ToolEvent::Click { pos: on_spline, px: PX, modifier: false });
    app.session.tool_event(ToolEvent::Move { pos: mm(20.0, 32.0), px: PX, suppress_snap: true });
    assert!(
        app.session.preview().picks.iter().any(|i| matches!(i, gc_sketch::Item::Curve(_))),
        "the picked curve is handed to the view"
    );
    evidence(&mut harness, "review-66-offset-pick.png");
    harness.state_mut().session.set_tool(ToolKind::Select);

    // Extend: near the spline's end the piece up to the boundary is previewed; a click extends it.
    let app = harness.state_mut();
    app.session.set_tool(ToolKind::Extend);
    // A point of the curve near its end (the click must be on the curve, on the end's side).
    let near_end = {
        let (_, sketch) = app.session.active_sketch().unwrap();
        let outline = sketch.curves.values().find_map(|c| sketch.spline_outline(&c.geom, 16)).unwrap();
        outline[outline.len() - 4]
    };
    app.session.tool_event(ToolEvent::Move { pos: near_end, px: PX, suppress_snap: true });
    evidence(&mut harness, "review-67-extend-spline.png");
    let app = harness.state_mut();
    app.session.tool_event(ToolEvent::Click { pos: near_end, px: PX, modifier: false });
    let fit = spline_points(&harness);
    let last = harness.state().session.point_pos(*fit.last().unwrap()).unwrap();
    assert!((last[0] - 0.07).abs() < 1e-6, "the spline reaches the boundary at x = 70 mm: {last:?}");
}

/// The Dimension tool measures from a line to a circle itself, not to its centre (Inventor 2026 help, To Work with Sketch Dimensions, GUID-026FE8D3-AFBB-4834-BC18-73D9CFF78185).
/// The screenshot is the evidence.
#[test]
fn the_dimension_tool_measures_a_line_to_a_circle_itself_on_screen() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("XY 스케치").click();
    harness.run();
    let app = harness.state_mut();
    line(app, mm(-60.0, 0.0), mm(60.0, 0.0));
    app.session.set_tool(ToolKind::Circle);
    for p in [mm(-20.0, 40.0), mm(-20.0, 50.0), mm(30.0, 40.0), mm(30.0, 48.0)] {
        click(app, p, false);
    }
    app.session.set_tool(ToolKind::Dimension);
    let dimensions = |app: &GraphCtrlApp| app.session.active_sketch().unwrap().1.dimensions.len();
    let before = dimensions(app);
    // Line (on its length), the first circle (on its rim), then where the text goes.
    for p in [mm(-40.0, 0.0), mm(-30.0, 40.0), mm(-45.0, 20.0)] {
        click(app, p, false);
    }
    app.session.submit_value("25 mm");
    let kinds: Vec<_> = app.session.active_sketch().unwrap().1.dimensions.values().map(|d| d.kind).collect();
    assert!(dimensions(app) > before, "a dimension was made: {kinds:?}");
    assert!(
        kinds.iter().any(|k| matches!(k, gc_sketch::DimKind::LineArc { .. })),
        "line to circle itself: {kinds:?}"
    );
    harness.run_steps(3);
    if gpu {
        let image = harness.render().unwrap();
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        image.save(out.join("review-68-arc-distance.png")).unwrap();
    }
}

/// Smooth (G2) in the Constrain panel of a 2D sketch (Inventor 2026 help, To Apply Geometric Constraints,
/// GUID-A85A7A30-7D81-4C75-8769-CAD034EEA930): the button starts the tool.
#[test]
fn the_constrain_panel_has_a_smooth_g2_button() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.state_mut().session.create_sketch("XY").unwrap();
    harness.run();
    harness.get_by_label("부드럽게(G2)").click();
    harness.run();
    assert_eq!(harness.state().session.tool(), ToolKind::Constraint(gc_tools::ConstraintKind::Smooth));
}

/// Sketch roles (plan 9.1): the right-click menu of a sketch point and of a curve offers "웨이포인트로 지정" and
/// "경로로 지정"; the chain of curves joined to the one clicked becomes one path.
#[test]
fn the_sketch_menu_makes_waypoints_and_paths() {
    let mut harness = sketching();
    // Two lines that join: one polyline, drawn with the Line tool.
    harness.state_mut().session.set_tool(ToolKind::Line);
    for p in [mm(-60.0, 0.0), mm(0.0, 40.0), mm(60.0, 0.0)] {
        click(harness.state_mut(), p, false);
    }
    harness.state_mut().session.tool_event(ToolEvent::Key(gc_tools::Key::Escape));
    harness.state_mut().session.set_tool(ToolKind::Select);
    harness.run_steps(3);
    let screen = |h: &Harness<'_>, p: [f64; 2]| {
        h.state().viewport().screen_of(gc_math::DVec3::new(p[0], p[1], 0.0)).unwrap()
    };
    let sketch = harness.state().session.active_sketch().unwrap().0;
    assert!(harness.state().session.doc().roles().is_empty());

    // A point: Make Waypoint.
    let at = screen(&harness, mm(60.0, 0.0));
    right_click(&mut harness, at);
    harness.get_by_label("웨이포인트로 지정").click();
    harness.run_steps(3);
    assert_eq!(harness.state().session.sketch_roles(sketch).len(), 1);

    // The middle of the first line: Make Path takes the whole chain (both lines).
    let at = screen(&harness, mm(-30.0, 20.0));
    right_click(&mut harness, at);
    harness.get_by_label("경로로 지정").click();
    harness.run_steps(3);
    let roles = harness.state().session.sketch_roles(sketch);
    assert_eq!(roles.len(), 2);
    let path = roles
        .iter()
        .find_map(|id| match &harness.state().session.doc().get(*id)?.kind {
            gc_doc::RoleKind::Path { curves, .. } => Some(curves.len()),
            _ => None,
        })
        .unwrap();
    assert_eq!(path, 2, "both lines belong to the path");
    // A curve that is in a path already offers nothing more.
    let at = screen(&harness, mm(30.0, 20.0));
    right_click(&mut harness, at);
    assert!(harness.query_by_label("경로로 지정").is_none());
}

/// The browser lists a sketch's roles below it; clicking one selects it, and the view marks it.
#[test]
fn the_browser_lists_sketch_roles_and_selects_them() {
    let mut harness = sketching();
    let sketch = harness.state().session.active_sketch().unwrap().0;
    harness.state_mut().session.set_tool(ToolKind::Point);
    click(harness.state_mut(), mm(10.0, 10.0), false);
    harness.state_mut().session.set_tool(ToolKind::Select);
    let point = *harness
        .state()
        .session
        .active_sketch()
        .unwrap()
        .1
        .points
        .keys()
        .find(|p| {
            harness
                .state()
                .session
                .point_pos(**p)
                .is_some_and(|pos| (pos[0] - 0.01).abs() < 1e-9 && (pos[1] - 0.01).abs() < 1e-9)
        })
        .unwrap();
    let role = harness.state_mut().session.add_waypoint_role(sketch, point).unwrap();
    harness.run_steps(3);
    let name = harness.state().session.doc().get(role).unwrap().name.clone();
    harness.get_by_label(&name).click();
    harness.run_steps(3);
    assert_eq!(harness.state().viewport().role_selected, Some(role));
}

/// The properties of a selected path role: motion, direction and speed are edited in the panel.
#[test]
fn the_properties_panel_edits_a_path_role() {
    let mut harness = sketching();
    harness.state_mut().session.set_tool(ToolKind::Line);
    for p in [mm(-60.0, 0.0), mm(60.0, 0.0)] {
        click(harness.state_mut(), p, false);
    }
    harness.state_mut().session.tool_event(ToolEvent::Key(gc_tools::Key::Escape));
    harness.state_mut().session.set_tool(ToolKind::Select);
    let (sketch, curve) = {
        let (id, s) = harness.state().session.active_sketch().unwrap();
        (id, *s.curves.keys().next().unwrap())
    };
    let role = harness.state_mut().session.add_path_role(sketch, &[curve]).unwrap();
    harness.state_mut().viewport_mut().role_selected = Some(role);
    harness.run_steps(3);
    harness.get_by_label("직선 이동").click();
    harness.run_steps(2);
    harness.get_by_label("경로 방향 뒤집기").click();
    harness.run_steps(2);
    let kind = harness.state().session.doc().get(role).unwrap().kind.clone();
    match kind {
        gc_doc::RoleKind::Path { motion, reversed, .. } => {
            assert_eq!(motion, gc_doc::PathMotion::Linear);
            assert!(reversed);
        }
        other => panic!("{other:?}"),
    }
}
