//! Editing scenarios driven through the session exactly as the user interface drives it
//! (S1-S6 in docs/PLAN.md, appendix C).

#![allow(
    clippy::float_cmp,
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: exact values are compared and failures should panic"
)]

use gc_core::Id;
use gc_doc::EditError;
use gc_session::Session;
use gc_sketch::{Curve, GeoConstraint, Item, SkPoint};
use gc_tools::{ConstraintKind, Key, Prompt, ToolEvent, ToolKind};

/// One screen pixel in sketch units (metres): 0.1 mm.
const PX: f64 = 1e-4;

fn mm(x: f64, y: f64) -> [f64; 2] {
    [x * 1e-3, y * 1e-3]
}

fn click(session: &mut Session, at: [f64; 2]) {
    session.tool_event(ToolEvent::Move { pos: at, px: PX, suppress_snap: false });
    session.tool_event(ToolEvent::Click { pos: at, px: PX, modifier: false });
}

fn drag(session: &mut Session, from: [f64; 2], to: [f64; 2], frames: usize) {
    session.tool_event(ToolEvent::DragStart { pos: from, px: PX });
    for k in 1..=frames {
        let t = k as f64 / frames as f64;
        session.tool_event(ToolEvent::Drag {
            pos: [from[0] + (to[0] - from[0]) * t, from[1] + (to[1] - from[1]) * t],
        });
    }
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

fn end_of(session: &Session, curve: Id<Curve>) -> (Id<SkPoint>, [f64; 3]) {
    let sketch = session.active_sketch().unwrap().1;
    let (_, b) = sketch.line(curve).unwrap();
    (b, sketch.pos(b).unwrap())
}

#[test]
fn s1_a_line_from_the_origin_drawn_level_gets_coincident_and_horizontal() {
    let mut session = new_session();
    session.set_tool(ToolKind::Line);
    assert_eq!(session.prompt(), Prompt::LineStart);
    click(&mut session, mm(0.3, 0.2)); // within 8 px of the origin
    assert_eq!(session.prompt(), Prompt::LineEnd);
    session.tool_event(ToolEvent::Move { pos: mm(30.0, 0.2), px: PX, suppress_snap: false });
    assert_eq!(
        session.preview().lines,
        vec![([0.0, 0.0], [0.030, 0.0])],
        "the preview already shows the snaps"
    );
    click(&mut session, mm(30.0, 0.2));
    let sketch = session.active_sketch().unwrap().1;
    assert_eq!(sketch.curves.len(), 1);
    let kinds: Vec<_> = sketch.constraints.values().collect();
    assert!(kinds.iter().any(|c| matches!(c, GeoConstraint::Coincident(..))));
    assert!(kinds.iter().any(|c| matches!(c, GeoConstraint::Horizontal(..))));
    assert_eq!(dof(&session), 1);
    assert_eq!(session.history().undo_depth(), 2, "new sketch + one line");

    // The chain continues from the end of the first line; Escape ends the chain, then the tool.
    click(&mut session, mm(30.0, 20.0));
    assert_eq!(curves(&session).len(), 2);
    let sketch = session.active_sketch().unwrap().1;
    assert_eq!(sketch.constraints.len(), 4, "second line: coincident with the first + vertical");
    session.tool_event(ToolEvent::Key(Key::Escape));
    assert_eq!(session.tool(), ToolKind::Line);
    session.tool_event(ToolEvent::Key(Key::Escape));
    assert_eq!(session.tool(), ToolKind::Select);
}

#[test]
fn s1_ctrl_suppresses_inference() {
    let mut session = new_session();
    session.set_tool(ToolKind::Line);
    session.tool_event(ToolEvent::Click { pos: mm(0.3, 0.2), px: PX, modifier: true });
    session.tool_event(ToolEvent::Click { pos: mm(30.0, 0.3), px: PX, modifier: true });
    let sketch = session.active_sketch().unwrap().1;
    assert_eq!((sketch.curves.len(), sketch.constraints.len()), (1, 0));
    assert_eq!(dof(&session), 4);
}

/// A horizontal line from the origin, 30 mm long as drawn.
fn session_with_line() -> (Session, Id<Curve>) {
    let mut session = new_session();
    session.set_tool(ToolKind::Line);
    click(&mut session, mm(0.0, 0.0));
    click(&mut session, mm(30.0, 0.0));
    session.set_tool(ToolKind::Select);
    let line = curves(&session)[0];
    (session, line)
}

#[test]
fn s2_a_dimension_is_placed_typed_and_the_tool_stays_active() {
    let (mut session, line) = session_with_line();
    session.set_tool(ToolKind::Dimension);
    click(&mut session, mm(15.0, 0.0)); // pick the line
    assert_eq!(session.prompt(), Prompt::DimensionPlace);
    click(&mut session, mm(15.0, -8.0)); // place the text
    let request = *session.value_request().unwrap();
    assert!((request.measured - 0.030).abs() < 1e-12);
    assert_eq!(session.prompt(), Prompt::DimensionValue);
    session.submit_value("25 mm");
    assert!(session.value_request().is_none());
    assert_eq!(dof(&session), 0);
    assert!((end_of(&session, line).1[0] - 0.025).abs() < 1e-12);
    assert_eq!(session.tool(), ToolKind::Dimension, "ready for the next dimension");
    assert_eq!(session.prompt(), Prompt::DimensionPick);
    assert!(!session.report().unwrap().curve_is_free(session.active_sketch().unwrap().1, line));
}

#[test]
fn s3_an_over_constraining_dimension_offers_a_reference_dimension() {
    let (mut session, _) = session_with_line();
    session.set_tool(ToolKind::Dimension);
    for value in ["25 mm", "25 mm"] {
        click(&mut session, mm(12.0, 0.0));
        click(&mut session, mm(12.0, -8.0));
        session.submit_value(value);
    }
    let offer = session.reference_offer().unwrap().clone();
    assert!(!offer.with.is_empty());
    let before = session.doc().clone();
    // Declining leaves the document untouched.
    session.answer_reference_offer(false);
    assert!(session.doc().same_content(&before));
    assert!(session.reference_offer().is_none());
    // Accepting adds a dimension that only measures.
    click(&mut session, mm(12.0, 0.0));
    click(&mut session, mm(12.0, -14.0));
    session.submit_value("40 mm");
    assert!(session.reference_offer().is_some(), "a conflicting value is offered as a reference too");
    session.answer_reference_offer(true);
    let sketch = session.active_sketch().unwrap().1;
    let reference: Vec<_> = sketch.dimensions.values().filter(|d| !d.driving).collect();
    assert_eq!(reference.len(), 1);
    assert!((reference[0].display_value(sketch).unwrap() - 0.025).abs() < 1e-12, "it shows the real length");
}

#[test]
fn s4_a_drag_is_one_undo_step_and_escape_restores_every_bit() {
    let (mut session, line) = session_with_line();
    let (_, start) = end_of(&session, line);
    let (depth, revision) = (session.history().undo_depth(), session.doc().revision());
    let before = session.doc().clone();

    drag(&mut session, [start[0], start[1]], mm(45.0, 0.0), 30);
    assert!(session.is_dragging());
    assert_eq!(session.doc().revision(), revision, "the document is untouched while dragging");
    let (end_point, _) = end_of(&session, line);
    assert!(
        (session.point_pos(end_point).unwrap()[0] - 0.045).abs() < 1e-9,
        "the preview follows the cursor"
    );
    session.tool_event(ToolEvent::Key(Key::Escape));
    assert!(!session.is_dragging());
    assert!(session.doc().same_content(&before));
    assert_eq!(session.history().undo_depth(), depth);

    drag(&mut session, [start[0], start[1]], mm(45.0, 0.0), 30);
    session.tool_event(ToolEvent::DragEnd);
    assert_eq!(session.history().undo_depth(), depth + 1);
    assert!((end_of(&session, line).1[0] - 0.045).abs() < 1e-9);
    assert_eq!(end_of(&session, line).1[1], 0.0, "the line stayed horizontal");
    assert!(session.undo());
    assert!(session.doc().same_content(&before));
}

#[test]
fn s5_dragging_fully_constrained_geometry_changes_nothing() {
    let (mut session, line) = session_with_line();
    session.set_tool(ToolKind::Dimension);
    click(&mut session, mm(15.0, 0.0));
    click(&mut session, mm(15.0, -8.0));
    session.submit_value("30 mm");
    session.set_tool(ToolKind::Select);
    assert_eq!(dof(&session), 0);
    let (depth, revision) = (session.history().undo_depth(), session.doc().revision());
    drag(&mut session, mm(30.0, 0.0), mm(50.0, 20.0), 10);
    assert!(!session.is_dragging(), "nothing can move, so no drag starts");
    session.tool_event(ToolEvent::DragEnd);
    assert_eq!((session.history().undo_depth(), session.doc().revision()), (depth, revision));
    assert!((end_of(&session, line).1[0] - 0.030).abs() < 1e-12);
}

#[test]
fn s6_a_bad_dimension_edit_is_reported_and_changes_nothing() {
    let (mut session, _) = session_with_line();
    session.set_tool(ToolKind::Dimension);
    click(&mut session, mm(15.0, 0.0));
    click(&mut session, mm(15.0, -8.0));
    session.submit_value("30 mm");
    let dimension = *session.active_sketch().unwrap().1.dimensions.keys().next().unwrap();
    let before = session.doc().clone();
    for bad in ["-5", "30 deg", "2 *", "unknown + 1"] {
        assert!(session.edit_dimension(dimension, bad).is_err(), "{bad}");
        assert!(session.doc().same_content(&before), "{bad}");
        assert!(session.last_error().is_some());
    }
    assert!(matches!(session.edit_dimension(dimension, "2 *"), Err(EditError::Expr { start: 3, .. })));
    session.edit_dimension(dimension, "12.5 mm * 2").unwrap();
    assert!(session.last_error().is_none());
}

#[test]
fn rectangle_circle_constraints_selection_and_delete() {
    let mut session = new_session();
    session.set_tool(ToolKind::Rectangle);
    click(&mut session, mm(10.0, 10.0));
    session.tool_event(ToolEvent::Move { pos: mm(40.0, 30.0), px: PX, suppress_snap: false });
    assert_eq!(session.preview().lines.len(), 4);
    click(&mut session, mm(40.0, 30.0));
    assert_eq!(curves(&session).len(), 4);
    assert_eq!(dof(&session), 4);

    session.set_tool(ToolKind::Circle);
    click(&mut session, mm(60.0, 20.0));
    click(&mut session, mm(70.0, 20.0));
    assert_eq!(curves(&session).len(), 5);
    assert_eq!(dof(&session), 7);

    // Tangent: the right side of the rectangle and the circle.
    let (right, circle) = (curves(&session)[1], curves(&session)[4]);
    session.set_tool(ToolKind::Constraint(ConstraintKind::Tangent));
    click(&mut session, mm(40.0, 20.0));
    click(&mut session, mm(60.0, 30.0)); // top of the circle
    assert_eq!(dof(&session), 6);
    let sketch = session.active_sketch().unwrap().1;
    assert!(
        sketch
            .constraints
            .values()
            .any(|c| matches!(c, GeoConstraint::Tangent { a, b, .. } if *a == right && *b == circle))
    );

    // An equal-length constraint between opposite sides is already implied: refused, document unchanged.
    let before = session.doc().clone();
    session.set_tool(ToolKind::Constraint(ConstraintKind::Equal));
    click(&mut session, mm(25.0, 10.0));
    click(&mut session, mm(25.0, 30.0));
    assert!(matches!(session.last_error(), Some(EditError::Redundant(_))));
    assert!(session.doc().same_content(&before));

    // Select the circle, add a line to the selection with Ctrl, delete both.
    session.set_tool(ToolKind::Select);
    let rim = session.active_sketch().unwrap().1.round(circle).unwrap();
    let centre = session.point_pos(rim.0).unwrap();
    click(&mut session, [centre[0], centre[1] - rim.1]);
    assert_eq!(session.selection().iter().copied().collect::<Vec<_>>(), vec![Item::Curve(circle)]);
    session.tool_event(ToolEvent::Click { pos: mm(25.0, 10.0), px: PX, modifier: true });
    assert_eq!(session.selection().len(), 2);
    session.tool_event(ToolEvent::Key(Key::Delete));
    assert_eq!(curves(&session).len(), 3);
    assert!(session.selection().is_empty());
    assert!(session.undo());
    assert_eq!(curves(&session).len(), 5);
}

#[test]
fn undoing_the_sketch_itself_leaves_sketch_mode() {
    let mut session = new_session();
    assert!(session.active_sketch().is_some());
    assert!(session.undo());
    assert!(session.active_sketch().is_none());
    assert!(session.report().is_none());
    assert!(session.redo());
    assert_eq!(session.doc().sketches().len(), 1);
}

/// The arc tool: centre, start, then sweep to the end. The arc runs the way the cursor went round.
#[test]
fn an_arc_follows_the_sweep_of_the_cursor() {
    use gc_sketch::CurveGeom;
    let sweep = |session: &mut Session, through: &[[f64; 2]]| {
        session.set_tool(ToolKind::Arc);
        assert_eq!(session.prompt(), Prompt::ArcCentre);
        click(session, mm(40.0, 30.0));
        assert_eq!(session.prompt(), Prompt::ArcStart);
        click(session, mm(50.0, 30.0));
        assert_eq!(session.prompt(), Prompt::ArcEnd);
        for at in through {
            session.tool_event(ToolEvent::Move { pos: *at, px: PX, suppress_snap: true });
        }
        assert_eq!(session.preview().arcs.len(), 1, "the arc is previewed while it is swept");
        let last = *through.last().unwrap();
        session.tool_event(ToolEvent::Click { pos: last, px: PX, modifier: true });
        assert_eq!(session.prompt(), Prompt::ArcCentre, "the tool stays active for the next arc");
        let (_, sketch) = session.active_sketch().unwrap();
        let (id, arc) = sketch.curves.iter().last().unwrap();
        let CurveGeom::Arc { c, s, e } = arc.geom else { panic!("not an arc") };
        let at = |p| sketch.pos(p).map(|p| [p[0] * 1e3, p[1] * 1e3]).unwrap();
        (*id, at(c), at(s), at(e))
    };
    let near = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).abs() < 1e-6 && (a[1] - b[1]).abs() < 1e-6;
    let mut session = new_session();

    // A quarter turn counter-clockwise: from (50, 30) up to (40, 40).
    let (first, c, s, e) = sweep(&mut session, &[mm(48.0, 36.0), mm(40.0, 45.0)]);
    assert!(near(c, [40.0, 30.0]) && near(s, [50.0, 30.0]) && near(e, [40.0, 40.0]), "{s:?} {e:?}");
    assert!((session.radius(first).unwrap() - 0.010).abs() < 1e-12);

    // Clockwise by a quarter turn: stored counter-clockwise, so it starts at (40, 20) and ends at (50, 30).
    let (_, _, s, e) = sweep(&mut session, &[mm(48.0, 24.0), mm(40.0, 15.0)]);
    assert!(near(s, [40.0, 20.0]) && near(e, [50.0, 30.0]), "{s:?} {e:?}");

    // Three quarters of a turn counter-clockwise, past half a turn: it ends at (40, 20).
    let (_, _, s, e) = sweep(&mut session, &[mm(40.0, 42.0), mm(28.0, 30.0), mm(40.0, 12.0)]);
    assert!(near(s, [50.0, 30.0]) && near(e, [40.0, 20.0]), "{s:?} {e:?}");
    assert_eq!(curves(&session).len(), 3);

    // Escape drops an arc in progress; each arc is one undo step.
    click(&mut session, mm(0.0, 50.0));
    click(&mut session, mm(10.0, 50.0));
    session.tool_event(ToolEvent::Key(Key::Escape));
    assert_eq!(session.prompt(), Prompt::ArcCentre);
    assert!(session.undo());
    assert_eq!(curves(&session).len(), 2);
}
