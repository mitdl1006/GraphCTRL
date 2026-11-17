//! QA acceptance tests for the Inventor-parity sketch tools (docs/ux/inventor-sketch-parity.md, P1),
//! written from the help text, not from the implementation, and aimed at the edges: nothing to trim,
//! nothing to extend, impossible fillets, degenerate arcs and rectangles, unusable typed values, Ctrl, and
//! how many Undo steps each gesture costs.
//!
//! Sources (Inventor 2026 help, <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/>):
//! - S2 lines/arcs: GUID-4F861674-F74C-474E-869A-4C190ED6EC4B.htm
//! - S3 shapes/fillet: GUID-D489CE6D-7299-4211-A43A-F3580A4BA357.htm
//! - S4 dynamic input: GUID-33A8B11D-A04C-4809-BC82-C0419215915F.htm
//! - S9 sketch undo: GUID-F57563E3-CB1B-4BE5-B7D1-025F4476A02A.htm
//! - S19 split/trim/extend: GUID-DD00037F-6BBA-4B95-AA90-4F99DCEB667D.htm
//! - S23 inference and Ctrl: GUID-653A9867-4F4F-48AE-B5DE-9DED76BD6DF2.htm

#![allow(
    clippy::float_cmp,
    clippy::unwrap_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code"
)]

use gc_core::Id;
use gc_session::Session;
use gc_sketch::{Curve, GeoConstraint};
use gc_tools::{HudField, Key, ToolCommand, ToolEvent, ToolKind};

const PX: f64 = 1e-4;

fn mm(x: f64, y: f64) -> [f64; 2] {
    [x * 1e-3, y * 1e-3]
}

fn move_to(s: &mut Session, at: [f64; 2]) {
    s.tool_event(ToolEvent::Move { pos: at, px: PX, suppress_snap: false });
}

fn click(s: &mut Session, at: [f64; 2]) {
    move_to(s, at);
    s.tool_event(ToolEvent::Click { pos: at, px: PX, modifier: false });
}

fn ctrl_click(s: &mut Session, at: [f64; 2]) {
    s.tool_event(ToolEvent::Move { pos: at, px: PX, suppress_snap: true });
    s.tool_event(ToolEvent::Click { pos: at, px: PX, modifier: true });
}

fn new_session() -> Session {
    let mut s = Session::new();
    s.create_sketch("XY").unwrap();
    s
}

fn curves(s: &Session) -> Vec<Id<Curve>> {
    s.active_sketch().unwrap().1.curves.keys().copied().collect()
}

fn constraints(s: &Session) -> Vec<GeoConstraint> {
    s.active_sketch().unwrap().1.constraints.values().copied().collect()
}

fn add_line(s: &mut Session, a: [f64; 2], b: [f64; 2]) -> Id<Curve> {
    let sketch = s.active_sketch().unwrap().0;
    let op = gc_doc::SketchOp::AddLine {
        a: [a[0] * 1e-3, a[1] * 1e-3, 0.0],
        b: [b[0] * 1e-3, b[1] * 1e-3, 0.0],
        snap_a: gc_doc::EndSnap::default(),
        snap_b: gc_doc::EndSnap::default(),
        horizontal: None,
    };
    let created = s.execute(&gc_doc::Command::Sketch { sketch, op }).unwrap();
    created.iter().find_map(|c| if let gc_doc::Created::Curve(id) = c { Some(*id) } else { None }).unwrap()
}

/// S19: "If you select a curve with no physical or virtual intersections, the Trim command deletes the
/// curve." One click, one Undo step, and Undo brings the curve back exactly.
#[test]
fn trim_deletes_a_curve_that_crosses_nothing() {
    let mut s = new_session();
    let alone = add_line(&mut s, [0.0, 0.0], [30.0, 0.0]);
    let other = add_line(&mut s, [0.0, 20.0], [30.0, 20.0]); // parallel: no crossing
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    s.set_tool(ToolKind::Trim);
    click(&mut s, mm(15.0, 0.0));
    assert_eq!(curves(&s), vec![other], "the lone line is deleted, the other stays");
    assert!(!curves(&s).contains(&alone));
    assert_eq!(s.history().undo_depth(), depth + 1);
    assert_eq!(s.tool(), ToolKind::Trim, "the tool stays on");
    assert!(s.undo());
    assert!(s.doc().same_content(&before));
}

/// S19: Extend goes to "the nearest intersecting curve". With none, nothing happens (document and
/// history untouched, no error that sticks).
#[test]
fn extend_with_nothing_to_reach_changes_nothing() {
    let mut s = new_session();
    add_line(&mut s, [0.0, 0.0], [10.0, 0.0]);
    add_line(&mut s, [0.0, 20.0], [10.0, 20.0]);
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    s.set_tool(ToolKind::Extend);
    click(&mut s, mm(9.0, 0.0));
    click(&mut s, mm(1.0, 20.0));
    assert!(s.doc().same_content(&before));
    assert_eq!(s.history().undo_depth(), depth);
}

/// S3: a fillet needs two lines that meet or can be joined by an arc of the radius. Parallel lines, a
/// radius that does not fit, zero, negative and unreadable radii: refused, document untouched.
#[test]
fn impossible_fillets_are_refused_and_change_nothing() {
    // (An empty box means "nothing typed": the remembered radius is used, so it is not in the list.)
    for radius in ["0", "-5", "abc", "1e400", "5 mm mm"] {
        let mut s = new_session();
        s.set_tool(ToolKind::Rectangle);
        click(&mut s, mm(0.0, 0.0));
        click(&mut s, mm(40.0, 20.0));
        let before = s.doc().clone();
        let depth = s.history().undo_depth();
        s.set_tool(ToolKind::Fillet);
        s.tool_event(ToolEvent::Hud { field: HudField::Radius, value: Some(radius.into()) });
        click(&mut s, mm(20.0, 0.0));
        click(&mut s, mm(40.0, 10.0));
        assert!(s.doc().same_content(&before), "radius {radius:?} changed the sketch");
        assert_eq!(s.history().undo_depth(), depth, "radius {radius:?}");
    }
    // Parallel sides, and a radius larger than the shorter side.
    let mut s = new_session();
    s.set_tool(ToolKind::Rectangle);
    click(&mut s, mm(0.0, 0.0));
    click(&mut s, mm(40.0, 20.0));
    let before = s.doc().clone();
    s.set_tool(ToolKind::Fillet);
    s.tool_event(ToolEvent::Hud { field: HudField::Radius, value: Some("5".into()) });
    click(&mut s, mm(20.0, 0.0)); // bottom
    click(&mut s, mm(20.0, 20.0)); // top: parallel to the bottom
    assert!(s.doc().same_content(&before), "parallel lines cannot be filleted");
    s.tool_event(ToolEvent::Key(Key::Escape));
    s.set_tool(ToolKind::Fillet);
    s.tool_event(ToolEvent::Hud { field: HudField::Radius, value: Some("500".into()) });
    click(&mut s, mm(20.0, 0.0));
    click(&mut s, mm(40.0, 10.0));
    assert!(s.doc().same_content(&before), "a 500 mm radius does not fit a 40 x 20 rectangle");
}

/// S9: Undo removes line segments one by one, in reverse. A chain of three segments costs three steps.
#[test]
fn a_line_chain_undoes_one_segment_at_a_time() {
    let mut s = new_session();
    let base = s.history().undo_depth();
    s.set_tool(ToolKind::Line);
    for at in [mm(0.0, 0.0), mm(20.0, 5.0), mm(30.0, 25.0), mm(10.0, 40.0)] {
        click(&mut s, at);
    }
    assert_eq!(curves(&s).len(), 3);
    assert_eq!(s.history().undo_depth(), base + 3);
    for left in (0..3).rev() {
        assert!(s.undo());
        assert_eq!(curves(&s).len(), left);
    }
}

/// Backspace, Close and double-click with nothing to act on are harmless.
#[test]
fn line_chain_keys_with_nothing_to_act_on_are_harmless() {
    let mut s = new_session();
    s.set_tool(ToolKind::Line);
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    s.tool_event(ToolEvent::Key(Key::Backspace)); // no chain
    s.tool_event(ToolEvent::Command(ToolCommand::Close)); // no chain
    s.tool_event(ToolEvent::DoubleClick { pos: mm(5.0, 5.0), px: PX, modifier: false }); // first click only
    assert!(s.doc().same_content(&before));
    assert_eq!(s.history().undo_depth(), depth);
    assert_eq!(s.tool(), ToolKind::Line);

    // One segment only: Close would draw a zero-length line back onto itself, so it does nothing; Backspace
    // removes the segment and ends the chain.
    let mut s = new_session();
    s.set_tool(ToolKind::Line);
    click(&mut s, mm(0.0, 0.0));
    click(&mut s, mm(20.0, 10.0));
    s.tool_event(ToolEvent::Command(ToolCommand::Close));
    assert_eq!(curves(&s).len(), 1, "closing a one-segment chain would be a zero-length line: nothing");
    s.tool_event(ToolEvent::Key(Key::Backspace));
    s.tool_event(ToolEvent::Key(Key::Backspace));
    s.tool_event(ToolEvent::Key(Key::Backspace)); // more than there is
    assert!(curves(&s).len() <= 1);
    assert_eq!(s.tool(), ToolKind::Line);
}

/// A click on the chain's own end (zero length) makes no line.
#[test]
fn a_zero_length_click_draws_nothing() {
    let mut s = new_session();
    s.set_tool(ToolKind::Line);
    click(&mut s, mm(10.0, 10.0));
    click(&mut s, mm(10.0, 10.0));
    click(&mut s, mm(10.0, 10.0));
    assert!(curves(&s).is_empty());
}

/// S2: a three-point arc through three points on one line has no circle: nothing is made. The same point
/// twice too.
#[test]
fn degenerate_three_point_arcs_and_rectangles_make_nothing() {
    for third in [mm(30.0, 0.0), mm(10.0, 0.0), mm(50.0, 0.0), mm(0.0, 0.0)] {
        let mut s = new_session();
        let before = s.doc().clone();
        s.set_tool(ToolKind::Arc3);
        click(&mut s, mm(10.0, 0.0));
        click(&mut s, mm(50.0, 0.0));
        click(&mut s, third);
        assert!(curves(&s).is_empty(), "a degenerate arc was made with third point {third:?}");
        assert!(s.doc().same_content(&before) || curves(&s).is_empty());
    }
    // A three-point rectangle with no width (third point on the first side's line), or with the first
    // two corners equal.
    for (a, b, c) in
        [(mm(0.0, 0.0), mm(40.0, 0.0), mm(20.0, 0.0)), (mm(5.0, 5.0), mm(5.0, 5.0), mm(20.0, 20.0))]
    {
        let mut s = new_session();
        s.set_tool(ToolKind::Rectangle3);
        click(&mut s, a);
        click(&mut s, b);
        click(&mut s, c);
        assert!(curves(&s).is_empty(), "a flat rectangle was made from {a:?} {b:?} {c:?}");
    }
}

/// S23: holding Ctrl while creating geometry suppresses the inference constraints (no horizontal on a
/// level line). A tangent arc is tangent by definition, so Ctrl does not take that away.
#[test]
fn ctrl_suppresses_inferred_constraints_but_a_tangent_arc_stays_tangent() {
    let mut s = new_session();
    s.set_tool(ToolKind::Line);
    ctrl_click(&mut s, mm(0.0, 60.0));
    ctrl_click(&mut s, mm(30.0, 60.0));
    s.tool_event(ToolEvent::Key(Key::Escape));
    assert!(constraints(&s).is_empty(), "no horizontal inferred with Ctrl: {:?}", constraints(&s));
    s.set_tool(ToolKind::ArcTangent);
    ctrl_click(&mut s, mm(30.0, 60.0));
    ctrl_click(&mut s, mm(40.0, 70.0));
    assert!(
        constraints(&s).iter().any(|c| matches!(c, GeoConstraint::Tangent { .. })),
        "{:?}",
        constraints(&s)
    );
}

/// S4: values typed into the input boxes must be usable numbers; anything else is kept out of the shape
/// and the shape is not finished with it.
#[test]
fn unusable_typed_values_never_make_a_shape() {
    for value in ["", "abc", "-5", "0", "1e400", "5 +", "NaN", "inf"] {
        let mut s = new_session();
        s.set_tool(ToolKind::Line);
        click(&mut s, mm(10.0, 10.0));
        move_to(&mut s, mm(30.0, 25.0));
        s.tool_event(ToolEvent::Hud { field: HudField::Length, value: Some(value.into()) });
        s.tool_event(ToolEvent::Key(Key::Enter));
        for line in curves(&s) {
            let sketch = s.active_sketch().unwrap().1;
            let (a, b) = sketch.line(line).unwrap();
            let (a, b) = (sketch.pos(a).unwrap(), sketch.pos(b).unwrap());
            let len = (b[0] - a[0]).hypot(b[1] - a[1]);
            assert!(len.is_finite() && len > 0.0, "value {value:?} made a line of length {len}");
        }
        assert!(s.report().is_some_and(|r| r.dof < 100), "value {value:?}");
    }
}

/// Esc with a value typed in drops the typed value and the pending shape, and leaves the document alone.
#[test]
fn escape_drops_typed_values_without_touching_the_document() {
    let mut s = new_session();
    s.set_tool(ToolKind::Line);
    click(&mut s, mm(10.0, 10.0));
    move_to(&mut s, mm(30.0, 25.0));
    s.tool_event(ToolEvent::Hud { field: HudField::Length, value: Some("50".into()) });
    let before = s.doc().clone();
    s.tool_event(ToolEvent::Key(Key::Escape));
    assert!(s.doc().same_content(&before));
    assert!(s.tool_hud().iter().all(|slot| slot.locked.is_none()), "typed values are gone");
    assert!(s.preview().lines.is_empty());
}
