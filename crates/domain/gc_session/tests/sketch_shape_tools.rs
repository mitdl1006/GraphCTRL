//! The Point, Polygon, Slot and Offset tools driven through the session as the viewport drives them
//! (docs/ux/inventor-sketch-parity.md; Inventor 2026 help GUID-D4A4288F-32F1-4E23-A382-930EBA152E87,
//! GUID-D489CE6D-7299-4211-A43A-F3580A4BA357, GUID-962CEE71-8BE4-4DA6-B51E-341319C64FA4).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::disallowed_methods,
    reason = "test code"
)]

use gc_core::Id;
use gc_doc::SlotKind;
use gc_session::Session;
use gc_sketch::Curve;
use gc_tools::{HudField, Key, Prompt, ToolEvent, ToolKind};

const PX: f64 = 1e-4;

fn mm(x: f64, y: f64) -> [f64; 2] {
    [x * 1e-3, y * 1e-3]
}

fn click(session: &mut Session, at: [f64; 2]) {
    session.tool_event(ToolEvent::Move { pos: at, px: PX, suppress_snap: true });
    session.tool_event(ToolEvent::Click { pos: at, px: PX, modifier: true });
}

fn new_session() -> Session {
    let mut session = Session::new();
    session.create_sketch("XY").unwrap();
    session
}

fn drawn(session: &Session) -> Vec<Id<Curve>> {
    let (_, sketch) = session.active_sketch().unwrap();
    sketch.curves.iter().filter(|(_, c)| !c.construction).map(|(id, _)| *id).collect()
}

#[test]
fn the_point_tool_places_centre_points_or_sketch_points_and_the_toggle_survives_tool_changes() {
    let mut session = new_session();
    session.set_tool(ToolKind::Point);
    assert_eq!(session.prompt(), Prompt::PointPlace);
    click(&mut session, mm(10.0, 20.0));
    click(&mut session, mm(30.0, 20.0));
    let (_, sketch) = session.active_sketch().unwrap();
    let centres = sketch.points.values().filter(|p| p.center).count();
    assert_eq!(centres, 2, "Inventor's default");
    session.set_center_point(false).unwrap();
    session.set_tool(ToolKind::Line);
    session.set_tool(ToolKind::Point);
    assert!(!session.center_point(), "kept across tools");
    click(&mut session, mm(50.0, 20.0));
    let (_, sketch) = session.active_sketch().unwrap();
    assert_eq!(sketch.points.values().filter(|p| p.center).count(), 2);
}

#[test]
fn the_polygon_tool_takes_the_centre_then_a_corner() {
    let mut session = new_session();
    session.set_polygon_settings(5, false);
    session.set_tool(ToolKind::Polygon);
    assert_eq!(session.prompt(), Prompt::PolygonCentre);
    click(&mut session, mm(0.0, 0.0));
    assert_eq!(session.prompt(), Prompt::PolygonCorner);
    session.tool_event(ToolEvent::Move { pos: mm(20.0, 0.0), px: PX, suppress_snap: true });
    assert_eq!(session.preview().lines.len(), 5, "the polygon follows the cursor");
    click(&mut session, mm(20.0, 0.0));
    assert_eq!(drawn(&session).len(), 5);
    assert_eq!(session.prompt(), Prompt::PolygonCentre, "ready for the next");
    assert_eq!(session.polygon_settings(), (5, false));
    session.set_polygon_settings(500, true);
    assert_eq!(session.polygon_settings(), (120, true), "clamped");
}

#[test]
fn the_slot_tool_takes_two_points_then_the_width_clicked_or_typed() {
    let mut session = new_session();
    session.set_tool(ToolKind::Slot(SlotKind::CenterToCenter));
    click(&mut session, mm(0.0, 0.0));
    assert_eq!(session.prompt(), Prompt::SlotSecond);
    click(&mut session, mm(40.0, 0.0));
    assert_eq!(session.prompt(), Prompt::SlotWidth);
    session.tool_event(ToolEvent::Move { pos: mm(20.0, 5.0), px: PX, suppress_snap: true });
    let hud = session.tool_hud();
    assert_eq!(hud.len(), 1);
    assert_eq!(hud[0].field, HudField::Width);
    assert!((hud[0].measured - 0.01).abs() < 1e-9, "twice the distance from the centre line");
    // Typed: 16 mm wide, then Enter.
    session.tool_event(ToolEvent::Hud { field: HudField::Width, value: Some("16".into()) });
    session.tool_event(ToolEvent::Key(Key::Enter));
    let (_, sketch) = session.active_sketch().unwrap();
    let radii: Vec<f64> = drawn(&session).iter().filter_map(|c| sketch.round(*c).map(|(_, r)| r)).collect();
    assert_eq!(radii.len(), 2);
    assert!(radii.iter().all(|r| (r - 0.008).abs() < 1e-9), "{radii:?}");
}

#[test]
fn the_offset_tool_picks_a_curve_then_offsets_the_loop_to_the_cursors_side() {
    let mut session = new_session();
    session.set_tool(ToolKind::Rectangle);
    click(&mut session, mm(0.0, 0.0));
    click(&mut session, mm(40.0, 20.0));
    session.set_tool(ToolKind::Offset);
    assert_eq!(session.prompt(), Prompt::OffsetPick);
    click(&mut session, mm(20.0, 0.0)); // the bottom side
    assert_eq!(session.prompt(), Prompt::OffsetPlace);
    // Outside, 5 mm below the bottom side: the whole rectangle grows by 5 mm.
    session.tool_event(ToolEvent::Move { pos: mm(20.0, -5.0), px: PX, suppress_snap: true });
    assert_eq!(session.preview().lines.len(), 4, "the loop's offset is previewed");
    let hud = session.tool_hud();
    assert!((hud[0].measured - 0.005).abs() < 1e-9, "{hud:?}");
    session.tool_event(ToolEvent::Click { pos: mm(20.0, -5.0), px: PX, modifier: true });
    let (_, sketch) = session.active_sketch().unwrap();
    let xs: Vec<f64> = drawn(&session)
        .iter()
        .filter_map(|c| sketch.line(*c))
        .flat_map(|(a, b)| [sketch.pos(a).unwrap()[0], sketch.pos(b).unwrap()[0]])
        .collect();
    assert_eq!(drawn(&session).len(), 8);
    assert!(
        xs.iter().any(|x| (x + 0.005).abs() < 1e-9) && xs.iter().any(|x| (x - 0.045).abs() < 1e-9),
        "{xs:?}"
    );
    // Typed inside: 2 mm, with Loop Select off only the picked side.
    session.set_offset_options(false, true);
    session.set_tool(ToolKind::Offset);
    click(&mut session, mm(20.0, 20.0)); // the top side
    session.tool_event(ToolEvent::Move { pos: mm(20.0, 15.0), px: PX, suppress_snap: true });
    session.tool_event(ToolEvent::Hud { field: HudField::Distance, value: Some("2".into()) });
    session.tool_event(ToolEvent::Click { pos: mm(20.0, 15.0), px: PX, modifier: true });
    assert_eq!(drawn(&session).len(), 9);
    // Esc drops the pick.
    click(&mut session, mm(0.0, 10.0));
    session.tool_event(ToolEvent::Key(Key::Escape));
    assert_eq!(session.prompt(), Prompt::OffsetPick);
}
