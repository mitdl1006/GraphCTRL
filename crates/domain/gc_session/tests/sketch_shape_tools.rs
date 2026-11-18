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

#[test]
fn a_dialog_selector_takes_the_select_tools_clicks_by_its_own_rules() {
    use gc_session::SketchPick;
    use gc_sketch::Item;
    let mut session = new_session();
    session.set_tool(ToolKind::Line);
    click(&mut session, mm(0.0, 0.0));
    click(&mut session, mm(20.0, 0.0));
    session.set_tool(ToolKind::Circle);
    click(&mut session, mm(50.0, 0.0));
    click(&mut session, mm(55.0, 0.0));
    let plain = |session: &mut Session, at| {
        session.tool_event(ToolEvent::Move { pos: at, px: PX, suppress_snap: true });
        session.tool_event(ToolEvent::Click { pos: at, px: PX, modifier: false });
    };

    // Geometry: plain clicks toggle, a click on nothing keeps the picks.
    session.set_sketch_pick(Some(SketchPick::Curves), Vec::new());
    assert_eq!(session.tool(), ToolKind::Select);
    assert_eq!(session.prompt(), Prompt::PickGeometry);
    plain(&mut session, mm(10.0, 0.0));
    plain(&mut session, mm(55.0, 0.0));
    plain(&mut session, mm(30.0, 30.0));
    assert_eq!(session.selected_curves().len(), 2);
    plain(&mut session, mm(10.0, 0.0));
    assert_eq!(session.selected_curves().len(), 1);

    // A line selector ignores the circle; a point selector takes the circle's centre.
    session.set_sketch_pick(Some(SketchPick::Line), Vec::new());
    plain(&mut session, mm(55.0, 0.0));
    assert!(session.selection().is_empty());
    plain(&mut session, mm(10.0, 0.0));
    assert_eq!(session.selected_curves().len(), 1);
    session.set_sketch_pick(Some(SketchPick::Point), Vec::new());
    plain(&mut session, mm(55.0, 0.0));
    assert!(matches!(session.selection().iter().next(), Some(Item::Point(_))));
    assert_eq!(session.selection().len(), 1);

    // Another command ends picking.
    session.set_tool(ToolKind::Line);
    assert!(session.sketch_pick().is_none());
}

/// Inventor 2026 help, To Create 2D Shape Geometry (GUID-D489CE6D-7299-4211-A43A-F3580A4BA357): Three
/// Point Arc Slot (start, end, a point of the centre arc, width) and Center Point Arc Slot (centre, start,
/// end, width).
#[test]
fn the_arc_slot_tools_take_three_points_then_the_width() {
    use gc_doc::ArcSlotKind;
    for (kind, clicks) in [
        (ArcSlotKind::ThreePoint, [mm(30.0, 0.0), mm(0.0, 30.0), mm(21.2132, 21.2132)]),
        (ArcSlotKind::CenterPoint, [mm(0.0, 0.0), mm(30.0, 0.0), mm(0.0, 30.0)]),
    ] {
        let mut session = new_session();
        session.set_tool(ToolKind::ArcSlot(kind));
        let first = if kind == ArcSlotKind::CenterPoint { Prompt::ArcSlotCentre } else { Prompt::SlotFirst };
        assert_eq!(session.prompt(), first);
        click(&mut session, clicks[0]);
        click(&mut session, clicks[1]);
        let third =
            if kind == ArcSlotKind::CenterPoint { Prompt::ArcSlotEnd } else { Prompt::ArcSlotThrough };
        assert_eq!(session.prompt(), third);
        click(&mut session, clicks[2]);
        assert_eq!(session.prompt(), Prompt::SlotWidth);
        session.tool_event(ToolEvent::Move { pos: mm(35.0, 0.0), px: PX, suppress_snap: true });
        let hud = session.tool_hud();
        assert!((hud[0].measured - 0.01).abs() < 1e-6, "{kind:?}: {hud:?}");
        assert_eq!(session.preview().arcs.len(), 4, "{kind:?}: the slot follows the cursor");
        click(&mut session, mm(35.0, 0.0));
        let (_, sketch) = session.active_sketch().unwrap();
        let mut radii: Vec<f64> =
            drawn(&session).iter().filter_map(|c| sketch.round(*c).map(|(_, r)| r)).collect();
        radii.sort_by(f64::total_cmp);
        let want = [0.005, 0.005, 0.025, 0.035];
        assert!(radii.iter().zip(want).all(|(r, w)| (r - w).abs() < 1e-6), "{kind:?}: {radii:?}");
        assert_eq!(session.prompt(), first, "ready for the next");
    }
}

/// Inventor 2026 help, Dynamic Input (GUID-33A8B11D-A04C-4809-BC82-C0419215915F), Draw Points Using
/// Dynamic Input: the X and Y boxes show the cursor; Tab, X, Tab, Y, Enter places the point there.
#[test]
fn the_point_tool_shows_x_and_y_and_places_typed_coordinates() {
    let mut session = new_session();
    session.set_tool(ToolKind::Point);
    session.tool_event(ToolEvent::Move { pos: mm(12.0, 34.0), px: PX, suppress_snap: true });
    let hud = session.tool_hud();
    assert_eq!(hud.iter().map(|s| s.field).collect::<Vec<_>>(), vec![HudField::X, HudField::Y]);
    assert!((hud[0].measured - 0.012).abs() < 1e-12 && (hud[1].measured - 0.034).abs() < 1e-12);
    session.tool_event(ToolEvent::Hud { field: HudField::X, value: Some("10".into()) });
    session.tool_event(ToolEvent::Hud { field: HudField::Y, value: Some("-5".into()) });
    session.tool_event(ToolEvent::Key(Key::Enter));
    let (_, sketch) = session.active_sketch().unwrap();
    let placed = sketch.points.values().find(|p| p.pos[1] < 0.0).unwrap();
    assert!((placed.pos[0] - 0.01).abs() < 1e-12 && (placed.pos[1] + 0.005).abs() < 1e-12);
    assert!(session.tool_hud().iter().all(|s| s.locked.is_none()), "the next point starts afresh");
}

/// docs/KNOWN_ISSUES.md "직사각형 스케치 패턴 60×60 이상 지연": a 100 × 100 rectangular pattern is made,
/// and its spacing edited, each within a second; the copies follow the spacing.
#[test]
fn large_rectangular_patterns_are_quick_to_make_and_to_edit() {
    use gc_doc::{SketchOp, SketchPatternDirection};
    for n in [60u32, 100] {
        let mut session = new_session();
        for (a, b) in [(mm(0.0, -50.0), mm(100.0, -50.0)), (mm(-50.0, 100.0), mm(-50.0, 200.0))] {
            session.set_tool(ToolKind::Line);
            click(&mut session, a);
            click(&mut session, b);
            session.tool_event(ToolEvent::Key(Key::Escape));
        }
        session.set_tool(ToolKind::Circle);
        click(&mut session, mm(0.0, 0.0));
        click(&mut session, mm(1.0, 0.0));
        let (_, sketch) = session.active_sketch().unwrap();
        let lines: Vec<Id<Curve>> =
            sketch.curves.keys().copied().filter(|c| sketch.line(*c).is_some()).collect();
        assert_eq!(lines.len(), 2);
        let hole = *sketch.curves.keys().find(|c| sketch.round(**c).is_some()).unwrap();
        let direction =
            |line, count| SketchPatternDirection { line, count, spacing: "5".into(), flip: false };
        let started = std::time::Instant::now();
        session
            .edit_active_sketch(SketchOp::RectangularPattern {
                curves: vec![hole],
                first: direction(lines[0], n),
                second: Some(direction(lines[1], n)),
                fitted: false,
            })
            .unwrap();
        let made = started.elapsed();
        assert!(made.as_secs_f64() < 1.0, "{n} × {n} made in {made:?}");
        let (_, sketch) = session.active_sketch().unwrap();
        let dimension = *sketch.dimensions.keys().next().unwrap();
        let started = std::time::Instant::now();
        session.edit_active_sketch(SketchOp::SetDimension { id: dimension, source: "6".into() }).unwrap();
        let edited = started.elapsed();
        assert!(edited.as_secs_f64() < 1.0, "{n} × {n} spacing edited in {edited:?}");
        let (_, sketch) = session.active_sketch().unwrap();
        let far = sketch.points.values().map(|p| p.pos[0].abs().max(p.pos[1].abs())).fold(0.0, f64::max);
        // 5 mm apart before, 6 mm after (the drawn directions are near the axes).
        assert!(far > 0.0058 * f64::from(n - 1), "{n}: the copies followed the new spacing ({far})");
        println!("{n} × {n}: made {made:?}, spacing edited {edited:?}");
    }
}

/// Inventor 2026 help, To Create Lines, Arcs, and Bends (GUID-4F861674-F74C-474E-869A-4C190ED6EC4B): on the
/// 3D Sketch tab, Three Point Arc (two ends, then a point on the arc) and Center Point Arc (centre, start,
/// end). Here a 3D arc lies in the plane of its points and turns less than half a circle.
#[test]
fn three_dimensional_arcs_by_three_points_or_by_centre() {
    use gc_doc::SketchOp;
    use gc_sketch::DimKind;
    let tilted = |x: f64, y: f64| [x, y * 0.6, y * 0.8]; // a plane through X, tilted up
    for tool in [ToolKind::Arc3, ToolKind::Arc] {
        let mut session = Session::new();
        session.create_sketch_3d().unwrap();
        session.set_tool(tool);
        let r = 0.05;
        let clicks = match tool {
            // Ends at 0° and 90°, through 45°.
            ToolKind::Arc3 => [
                tilted(r, 0.0),
                tilted(0.0, r),
                tilted(r * std::f64::consts::FRAC_1_SQRT_2, r * std::f64::consts::FRAC_1_SQRT_2),
            ],
            _ => [tilted(0.0, 0.0), tilted(r, 0.0), tilted(0.0, 2.0 * r)],
        };
        for (i, at) in clicks.iter().enumerate() {
            session.spatial_hover(Some((*at, None)));
            if i == 2 {
                assert!(
                    session.spatial_arc_preview().is_some_and(|p| p.len() > 2),
                    "{tool:?}: the arc follows"
                );
            }
            session.spatial_click(*at, None, None);
        }
        let (_, sketch) = session.active_sketch().unwrap();
        let (arc, curve) = sketch.curves.iter().next().unwrap();
        let gc_sketch::CurveGeom::Arc { c, s, e } = curve.geom else { panic!("{tool:?}: an arc") };
        let (c, s, e) = (sketch.pos(c).unwrap(), sketch.pos(s).unwrap(), sketch.pos(e).unwrap());
        let dist = |p: [f64; 3], q: [f64; 3]| (0..3).map(|k| (p[k] - q[k]).powi(2)).sum::<f64>().sqrt();
        assert!(dist(c, [0.0; 3]) < 1e-9, "{tool:?}: centre {c:?}");
        assert!((dist(c, s) - r).abs() < 1e-9 && (dist(c, e) - r).abs() < 1e-9, "{tool:?}");
        assert!(dist(e, tilted(0.0, r)) < 1e-9, "{tool:?}: end {e:?}");
        // Centre 3 + start 3 + end 3 + radius, less the two radius rows.
        let dof = session.report().unwrap().dof;
        assert_eq!(dof, 8, "{tool:?}");
        let arc = *arc;
        session
            .edit_active_sketch(SketchOp::AddDimension {
                kind: DimKind::Radius(arc),
                source: "40".into(),
                text_pos: [0.0; 2],
            })
            .unwrap();
        assert_eq!(session.report().unwrap().dof, 7, "{tool:?}: a radius dimension");
        let (_, sketch) = session.active_sketch().unwrap();
        assert!((sketch.round(arc).unwrap().1 - 0.04).abs() < 1e-9, "{tool:?}");
    }

    // Through a point on the long way round (more than half a circle): refused, nothing drawn.
    let mut session = Session::new();
    session.create_sketch_3d().unwrap();
    session.set_tool(ToolKind::Arc3);
    for at in [[0.05, 0.0, 0.0], [0.0, 0.05, 0.0], [-0.05, 0.0, 0.0]] {
        session.spatial_click(at, None, None);
    }
    assert!(session.active_sketch().unwrap().1.curves.is_empty());
}
