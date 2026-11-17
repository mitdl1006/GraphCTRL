//! Inventor-style sketch tools driven through the session as the user interface drives them
//! (docs/ux/inventor-sketch-parity.md, P1-2 to P1-6 and P1-13). Each scenario checks what is drawn, the
//! constraints that come with it and the remaining degrees of freedom.

#![allow(
    clippy::float_cmp,
    clippy::unwrap_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code: exact values are compared, failures should panic, std math checks results"
)]

use gc_core::Id;
use gc_session::Session;
use gc_sketch::{Curve, CurveGeom, DimKind, GeoConstraint};
use gc_tools::{HudField, Inference, Key, ToolCommand, ToolEvent, ToolKind};

/// One screen pixel in sketch units (metres): 0.1 mm.
const PX: f64 = 1e-4;

fn mm(x: f64, y: f64) -> [f64; 2] {
    [x * 1e-3, y * 1e-3]
}

fn move_to(session: &mut Session, at: [f64; 2]) {
    session.tool_event(ToolEvent::Move { pos: at, px: PX, suppress_snap: false });
}

fn click(session: &mut Session, at: [f64; 2]) {
    move_to(session, at);
    session.tool_event(ToolEvent::Click { pos: at, px: PX, modifier: false });
}

fn new_session() -> Session {
    let mut session = Session::new();
    session.create_sketch("XY").unwrap();
    session
}

fn curves(session: &Session) -> Vec<Id<Curve>> {
    session.active_sketch().unwrap().1.curves.keys().copied().collect()
}

fn dof(session: &Session) -> usize {
    session.report().unwrap().dof
}

fn constraints(session: &Session) -> Vec<GeoConstraint> {
    session.active_sketch().unwrap().1.constraints.values().copied().collect()
}

fn line_ends(session: &Session, curve: Id<Curve>) -> ([f64; 3], [f64; 3]) {
    let sketch = session.active_sketch().unwrap().1;
    let (a, b) = sketch.line(curve).unwrap();
    (sketch.pos(a).unwrap(), sketch.pos(b).unwrap())
}

#[test]
fn p1_2_a_double_click_ends_the_chain_and_the_tool_stays_on() {
    let mut session = new_session();
    session.set_tool(ToolKind::Line);
    click(&mut session, mm(10.0, 10.0));
    click(&mut session, mm(40.0, 10.0));
    // The second click of a double click: the segment, then the end of the chain.
    session.tool_event(ToolEvent::DoubleClick { pos: mm(40.0, 30.0), px: PX, modifier: false });
    assert_eq!(curves(&session).len(), 2);
    assert_eq!(session.tool(), ToolKind::Line, "the tool stays on");
    assert!(session.preview().lines.is_empty(), "no rubber band: the next click starts a new chain");
    click(&mut session, mm(60.0, 60.0));
    click(&mut session, mm(80.0, 60.0));
    assert_eq!(curves(&session).len(), 3);
}

#[test]
fn p1_2_backspace_takes_back_the_last_segment_and_close_closes_the_chain() {
    let mut session = new_session();
    session.set_tool(ToolKind::Line);
    for at in [mm(10.0, 10.0), mm(40.0, 10.0), mm(40.0, 30.0), mm(70.0, 50.0)] {
        click(&mut session, at);
    }
    assert_eq!(curves(&session).len(), 3);
    session.tool_event(ToolEvent::Key(Key::Backspace));
    assert_eq!(curves(&session).len(), 2, "the last segment is gone");
    // The chain goes on from the previous end.
    move_to(&mut session, mm(20.0, 30.0));
    assert_eq!(session.preview().lines[0].0, mm(40.0, 30.0));
    click(&mut session, mm(20.0, 30.0));
    // Close: a segment back to the chain's start, joined at both ends; the tool stays on.
    session.tool_event(ToolEvent::Command(ToolCommand::Close));
    let lines = curves(&session);
    assert_eq!(lines.len(), 4);
    let (a, b) = line_ends(&session, *lines.last().unwrap());
    assert!(
        (a[0] - 0.020).abs() < 1e-9 && (b[0] - 0.010).abs() < 1e-9 && (b[1] - 0.010).abs() < 1e-9,
        "{a:?} {b:?}"
    );
    let joins = constraints(&session).iter().filter(|c| matches!(c, GeoConstraint::Coincident(..))).count();
    assert_eq!(joins, 4, "a closed loop of four segments has four joints");
    assert_eq!(session.tool(), ToolKind::Line);
    // A closed quadrilateral: 4 lines × 4 − 4 joints × 2 = 8, less the horizontal and vertical segments
    // inferred while drawing ((10,10)→(40,10), (40,10)→(40,30), (40,30)→(20,30)) = 5 freedoms.
    assert_eq!(dof(&session), 5);
}

#[test]
fn p1_2_dragging_out_of_the_chain_end_draws_a_tangent_arc_and_the_chain_goes_on() {
    let mut session = new_session();
    session.set_tool(ToolKind::Line);
    click(&mut session, mm(10.0, 10.0));
    click(&mut session, mm(40.0, 10.0));
    session.tool_event(ToolEvent::DragStart { pos: mm(40.0, 10.0), px: PX });
    session.tool_event(ToolEvent::Drag { pos: mm(50.0, 20.0) });
    assert_eq!(session.preview().arcs.len(), 1, "the arc is previewed while dragging");
    session.tool_event(ToolEvent::DragEnd);
    let sketch = session.active_sketch().unwrap().1;
    let arc = *sketch.curves.iter().find(|(_, c)| matches!(c.geom, CurveGeom::Arc { .. })).unwrap().0;
    let CurveGeom::Arc { c, .. } = sketch.curves[&arc].geom else { panic!() };
    // Leaving the line (along +X) tangentially: the centre is straight above the joint.
    assert!((sketch.pos(c).unwrap()[0] - 0.040).abs() < 1e-9);
    assert!(constraints(&session).iter().any(|k| matches!(k, GeoConstraint::Tangent { .. })));
    // A click goes on from the arc's end.
    click(&mut session, mm(50.0, 40.0));
    let lines: Vec<_> = curves(&session)
        .into_iter()
        .filter(|c| session.active_sketch().unwrap().1.line(*c).is_some())
        .collect();
    assert_eq!(lines.len(), 2);
    let (a, _) = line_ends(&session, lines[1]);
    assert!((a[0] - 0.050).abs() < 1e-9 && (a[1] - 0.020).abs() < 1e-9, "{a:?}");
}

#[test]
fn p1_3_three_point_and_tangent_arcs() {
    let mut session = new_session();
    session.set_tool(ToolKind::Arc3);
    click(&mut session, mm(10.0, 10.0));
    click(&mut session, mm(50.0, 10.0));
    move_to(&mut session, mm(30.0, 25.0));
    assert_eq!(session.preview().arcs.len(), 1);
    click(&mut session, mm(30.0, 25.0));
    assert_eq!(curves(&session).len(), 1);
    // A free arc: centre (2) + two ends on the circle (3).
    assert_eq!(dof(&session), 5);

    // A tangent arc from the end of a line.
    session.set_tool(ToolKind::Line);
    click(&mut session, mm(0.0, 60.0));
    click(&mut session, mm(30.0, 60.0));
    session.tool_event(ToolEvent::Key(Key::Escape));
    session.set_tool(ToolKind::ArcTangent);
    click(&mut session, mm(30.0, 60.0));
    click(&mut session, mm(40.0, 70.0));
    let tangents =
        constraints(&session).iter().filter(|c| matches!(c, GeoConstraint::Tangent { .. })).count();
    assert_eq!(tangents, 1, "the tangent arc starts tangent to the line");
}

#[test]
fn p1_4_a_three_point_rectangle_at_an_angle_needs_five_dimensions() {
    let mut session = new_session();
    session.set_tool(ToolKind::Rectangle3);
    click(&mut session, mm(10.0, 10.0));
    click(&mut session, mm(40.0, 30.0));
    move_to(&mut session, mm(30.0, 50.0));
    assert_eq!(session.preview().lines.len(), 4);
    click(&mut session, mm(30.0, 50.0));
    assert_eq!(curves(&session).len(), 4);
    assert_eq!(dof(&session), 5, "position, angle, length and width");
}

#[test]
fn p1_5_inference_midpoint_intersection_quadrant_alignment_and_circle_tangent() {
    let mut session = new_session();
    session.set_tool(ToolKind::Line);
    click(&mut session, mm(10.0, 10.0));
    click(&mut session, mm(50.0, 10.0));
    session.tool_event(ToolEvent::Key(Key::Escape));
    // Over the line's midpoint: the midpoint inference, and a midpoint constraint when clicked.
    move_to(&mut session, mm(30.3, 10.4));
    assert_eq!(session.preview().snap.unwrap().inference, Some(Inference::Midpoint));
    click(&mut session, mm(30.3, 10.4));
    click(&mut session, mm(30.0, 40.0));
    assert!(constraints(&session).iter().any(|c| matches!(c, GeoConstraint::Midpoint(..))));
    session.tool_event(ToolEvent::Key(Key::Escape));

    // Level with an existing point (the line's start at y = 10): a dotted alignment line.
    click(&mut session, mm(70.0, 60.0));
    move_to(&mut session, mm(90.0, 40.2));
    let preview = session.preview();
    assert_eq!(preview.snap.unwrap().inference, Some(Inference::Aligned));
    assert_eq!(preview.alignments.len(), 1);
    assert!((preview.snap.unwrap().pos[1] - 0.040).abs() < 1e-12, "snapped level with (30, 40)");
    session.tool_event(ToolEvent::Key(Key::Escape));

    // A circle whose rim lies on the first line becomes tangent to it.
    session.set_tool(ToolKind::Circle);
    click(&mut session, mm(45.0, 25.0));
    click(&mut session, mm(45.0, 10.0));
    let tangent = constraints(&session).iter().any(|c| matches!(c, GeoConstraint::Tangent { .. }));
    assert!(tangent, "{:?}", constraints(&session));

    // Where a new line crosses the first one (no point there): an intersection snap.
    session.set_tool(ToolKind::Line);
    session.tool_event(ToolEvent::Click { pos: mm(20.0, 0.0), px: PX, modifier: true });
    session.tool_event(ToolEvent::Click { pos: mm(20.0, 30.0), px: PX, modifier: true });
    session.tool_event(ToolEvent::Key(Key::Escape));
    move_to(&mut session, mm(20.2, 10.3));
    let snap = session.preview().snap.unwrap();
    assert_eq!(snap.inference, Some(Inference::Intersection), "{snap:?}");
    assert!(snap.curve.is_some() && snap.also_on.is_some());
    // A circle's quadrant point (the circle's top, at (45, 40)).
    move_to(&mut session, mm(45.3, 40.2));
    assert_eq!(session.preview().snap.unwrap().inference, Some(Inference::Quadrant));
    // Ctrl: no inference at all.
    session.tool_event(ToolEvent::Move { pos: mm(45.3, 40.2), px: PX, suppress_snap: true });
    assert_eq!(session.preview().snap.unwrap().inference, None);
}

#[test]
fn p1_6_typed_values_shape_the_line_and_become_dimensions() {
    let mut session = new_session();
    session.set_tool(ToolKind::Line);
    click(&mut session, mm(10.0, 10.0));
    move_to(&mut session, mm(30.0, 25.0));
    let hud = session.tool_hud();
    assert_eq!(hud.iter().map(|s| s.field).collect::<Vec<_>>(), vec![HudField::Length, HudField::Angle]);
    assert!((hud[0].measured - 0.025).abs() < 1e-12);
    session.tool_event(ToolEvent::Hud { field: HudField::Length, value: Some("50".into()) });
    session.tool_event(ToolEvent::Hud { field: HudField::Angle, value: Some("90".into()) });
    assert_eq!(session.tool_hud()[0].locked.as_deref(), Some("50"));
    session.tool_event(ToolEvent::Key(Key::Enter));
    let line = curves(&session)[0];
    let (a, b) = line_ends(&session, line);
    assert!((b[0] - a[0]).abs() < 1e-9 && (b[1] - a[1] - 0.050).abs() < 1e-9, "{a:?} {b:?}");
    // The length is a driving dimension of 50 mm.
    let sketch = session.active_sketch().unwrap().1;
    let dimension = sketch.dimensions.values().next().unwrap();
    assert!(matches!(dimension.kind, DimKind::Distance { .. }) && dimension.driving);
    assert_eq!(dimension.source, "50");
    // One undo step takes back the line and its dimension together.
    assert!(session.undo());
    assert!(session.active_sketch().unwrap().1.curves.is_empty());
    assert!(session.active_sketch().unwrap().1.dimensions.is_empty());
}

#[test]
fn p1_13_construction_toggle_before_and_after_drawing() {
    let mut session = new_session();
    session.set_tool_construction(true).unwrap();
    session.set_tool(ToolKind::Rectangle);
    click(&mut session, mm(10.0, 10.0));
    click(&mut session, mm(40.0, 30.0));
    let sketch = session.active_sketch().unwrap().1;
    assert_eq!(sketch.curves.len(), 4, "{:?}", session.last_error());
    assert!(sketch.curves.values().all(|c| c.construction), "drawn as construction");
    session.set_tool_construction(false).unwrap();
    click(&mut session, mm(60.0, 10.0));
    click(&mut session, mm(80.0, 30.0));
    let regular = session.active_sketch().unwrap().1.curves.values().filter(|c| !c.construction).count();
    assert_eq!(regular, 4);

    // Selected curves are converted, as one undo step.
    session.set_tool(ToolKind::Select);
    let first = curves(&session)[0];
    session.tool_event(ToolEvent::Click { pos: mm(25.0, 10.0), px: PX, modifier: false });
    assert!(session.selection().contains(&gc_sketch::Item::Curve(first)));
    session.set_tool_construction(false).unwrap();
    assert!(!session.active_sketch().unwrap().1.curves[&first].construction);
    assert!(session.undo());
    assert!(session.active_sketch().unwrap().1.curves[&first].construction);
}
