//! Document behaviour: transactions, undo and redo, commands, parameters driving sketches.

#![allow(
    clippy::float_cmp,
    clippy::unwrap_used,
    clippy::panic,
    clippy::type_complexity,
    reason = "test code: exact values are compared and failures should panic"
)]

use gc_core::Id;
use gc_doc::{
    Command, Created, DocError, Document, EditError, EndSnap, Frame, FrameKind, History, Param, Sketch,
    SketchOp, Txn, apply,
};
use gc_expr::Unit;
use gc_sketch::{Curve, DimKind, Dimension, GeoConstraint, SkPoint, SketchError, Status};

fn new_sketch(doc: &mut Document, history: &mut History) -> Id<Sketch> {
    let frame = doc.origin_plane("XY").unwrap();
    let applied =
        apply(doc, &Command::CreateSketch { name: "Sketch1".into(), frame, spatial: false }).unwrap();
    let Created::Sketch(id) = applied.created[0] else { panic!("no sketch") };
    history.record(applied.label, applied.delta);
    id
}

fn run(doc: &mut Document, history: &mut History, sketch: Id<Sketch>, op: SketchOp) -> Vec<Created> {
    let applied = apply(doc, &Command::Sketch { sketch, op }).unwrap();
    history.record(applied.label, applied.delta);
    applied.created
}

/// A rectangle from (0,0) to (10 mm, 5 mm) with its first corner on the origin and a width dimension `w`.
fn rectangle_driven_by_parameter() -> (Document, History, Id<Sketch>, Id<Param>, Id<Dimension>, Vec<Id<Curve>>)
{
    let mut doc = Document::new();
    let mut history = History::new();
    let sketch = new_sketch(&mut doc, &mut history);
    let applied =
        apply(&mut doc, &Command::AddParam { name: "w".into(), source: "20 mm".into(), angle: false })
            .unwrap();
    let Created::Param(param) = applied.created[0] else { panic!("no param") };
    history.record(applied.label, applied.delta);
    let created = run(
        &mut doc,
        &mut history,
        sketch,
        SketchOp::AddRectangle { a: [0.0, 0.0], b: [0.010, 0.005], snap_a: EndSnap::default() },
    );
    let lines: Vec<Id<Curve>> =
        created.iter().filter_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None }).collect();
    let s = doc.get(sketch).unwrap();
    let origin = *s.points.keys().next().unwrap();
    let (a, b) = s.line(lines[0]).unwrap();
    run(&mut doc, &mut history, sketch, SketchOp::AddConstraint(GeoConstraint::Coincident(a, origin)));
    let created = run(
        &mut doc,
        &mut history,
        sketch,
        SketchOp::AddDimension {
            kind: DimKind::Distance { p: a, q: b },
            source: "w * 2".into(),
            text_pos: [0.005, -0.002],
        },
    );
    let Created::Dimension(dimension) = created[0] else { panic!("no dimension") };
    (doc, history, sketch, param, dimension, lines)
}

fn end_of(doc: &Document, sketch: Id<Sketch>, line: Id<Curve>) -> [f64; 3] {
    let s = doc.get(sketch).unwrap();
    s.pos(s.line(line).unwrap().1).unwrap()
}

#[test]
fn a_new_document_has_the_world_and_three_origin_planes() {
    let doc = Document::new();
    assert_eq!(doc.frames().len(), 4);
    assert!(doc.world().is_some());
    for name in ["XY", "XZ", "YZ"] {
        let plane = doc.get(doc.origin_plane(name).unwrap()).unwrap();
        assert_eq!((plane.kind, plane.parent), (FrameKind::OriginPlane, doc.world()));
    }
}

#[test]
fn a_dropped_transaction_leaves_no_trace_and_a_committed_one_bumps_the_revision() {
    let mut doc = Document::new();
    let before = doc.clone();
    {
        let mut txn = Txn::new(&mut doc);
        let id = txn.insert(Param { name: "a".into(), source: "1".into(), unit: Unit::Mm, value: 0.001 });
        txn.modify(id, |p: &mut Param, _| p.value = 5.0).unwrap();
        let world = txn.doc().world().unwrap();
        txn.modify(world, |f: &mut Frame, _| f.name = "changed".into()).unwrap();
        assert_eq!(txn.doc().params().len(), 1);
    }
    assert!(doc.same_content(&before));
    assert_eq!(doc.revision(), before.revision());

    let mut txn = Txn::new(&mut doc);
    let id = txn.insert(Param { name: "a".into(), source: "1".into(), unit: Unit::Mm, value: 0.001 });
    let (delta, changed) = txn.commit().unwrap();
    assert!(!delta.is_empty());
    assert!(changed.contains(&id.erase()));
    assert!(doc.revision() > before.revision());
}

#[test]
fn a_commit_that_would_leave_a_dangling_reference_is_rolled_back() {
    let mut doc = Document::new();
    let mut history = History::new();
    new_sketch(&mut doc, &mut history);
    let before = doc.clone();
    let plane = doc.origin_plane("XY").unwrap();
    let mut txn = Txn::new(&mut doc);
    txn.remove(plane).unwrap();
    assert!(matches!(txn.commit(), Err(DocError::Dangling(..))));
    assert!(doc.same_content(&before));
}

#[test]
fn undo_and_redo_are_exact_inverses() {
    let (mut doc, mut history, ..) = rectangle_driven_by_parameter();
    let final_state = doc.clone();
    let mut states = vec![doc.clone()];
    while history.undo(&mut doc).is_some() {
        states.push(doc.clone());
    }
    assert!(doc.same_content(&Document::new()), "undoing everything gives an empty document");
    for expected in states.iter().rev().skip(1) {
        history.redo(&mut doc).unwrap();
        assert!(doc.same_content(expected));
    }
    assert!(doc.same_content(&final_state));
    assert_eq!(history.redo_depth(), 0);
}

#[test]
fn recording_a_new_step_clears_the_redo_stack() {
    let (mut doc, mut history, sketch, ..) = rectangle_driven_by_parameter();
    history.undo(&mut doc).unwrap();
    assert_eq!(history.redo_depth(), 1);
    run(&mut doc, &mut history, sketch, SketchOp::AddPoint { pos: [0.02, 0.02, 0.0] });
    assert_eq!(history.redo_depth(), 0);
    assert_eq!(history.undo_label(), Some("Point"));
}

#[test]
fn a_parameter_drives_the_sketch_through_an_expression() {
    let (mut doc, mut history, sketch, param, dimension, lines) = rectangle_driven_by_parameter();
    assert!((end_of(&doc, sketch, lines[0])[0] - 0.040).abs() < 1e-12, "w * 2 = 40 mm");
    let applied = apply(&mut doc, &Command::SetParam { id: param, source: "35 mm".into() }).unwrap();
    assert!(applied.changed.contains(&sketch.erase()) && applied.changed.contains(&param.erase()));
    history.record(applied.label, applied.delta);
    assert!((end_of(&doc, sketch, lines[0])[0] - 0.070).abs() < 1e-12);
    assert_eq!(doc.get(sketch).unwrap().dimensions[&dimension].source, "w * 2", "the expression is kept");
    history.undo(&mut doc).unwrap();
    assert!((end_of(&doc, sketch, lines[0])[0] - 0.040).abs() < 1e-12);
    assert_eq!(doc.get(param).unwrap().source, "20 mm");
}

#[test]
fn a_command_that_cannot_be_solved_changes_nothing() {
    let (mut doc, _, sketch, param, dimension, lines) = rectangle_driven_by_parameter();
    let before = doc.clone();
    let attempts = [
        Command::SetParam { id: param, source: "-5 mm".into() },
        Command::SetParam { id: param, source: "5 +".into() },
        Command::SetParam { id: param, source: "w + 1".into() },
        Command::SetParam { id: param, source: "30 deg".into() },
        Command::Sketch { sketch, op: SketchOp::SetDimension { id: dimension, source: "nope * 2".into() } },
        Command::Sketch { sketch, op: SketchOp::SetDimension { id: dimension, source: "0".into() } },
        Command::DeleteParam { id: param },
        Command::AddParam { name: "w".into(), source: "1".into(), angle: false },
        Command::AddParam { name: "mm".into(), source: "1".into(), angle: false },
    ];
    for command in attempts {
        let error = apply(&mut doc, &command).unwrap_err();
        assert!(doc.same_content(&before), "{command:?} failed with {error:?} but changed the document");
        assert_eq!(doc.revision(), before.revision());
    }
    assert!(matches!(
        apply(&mut doc, &Command::SetParam { id: param, source: "w + 1".into() }),
        Err(EditError::Cycle)
    ));
    assert!(matches!(
        apply(&mut doc, &Command::SetParam { id: param, source: "-5 mm".into() }),
        Err(EditError::Sketch(SketchError::InvalidValue))
    ));
    assert!(matches!(apply(&mut doc, &Command::DeleteParam { id: param }), Err(EditError::InUse)));
    // The height is still free, so a second, different width is what conflicts.
    let s = doc.get(sketch).unwrap();
    let (a, b) = s.line(lines[0]).unwrap();
    let redundant = SketchOp::AddDimension {
        kind: DimKind::HDistance { p: a, q: b, sign: 1.0 },
        source: "40".into(),
        text_pos: [0.0; 2],
    };
    assert!(matches!(
        apply(&mut doc, &Command::Sketch { sketch, op: redundant }),
        Err(EditError::Redundant(_))
    ));
    let conflicting = SketchOp::AddDimension {
        kind: DimKind::HDistance { p: a, q: b, sign: 1.0 },
        source: "41".into(),
        text_pos: [0.0; 2],
    };
    assert!(matches!(
        apply(&mut doc, &Command::Sketch { sketch, op: conflicting }),
        Err(EditError::Conflicting(_))
    ));
    assert!(doc.same_content(&before));
}

#[test]
fn lines_pick_up_the_constraints_implied_by_how_they_were_drawn() {
    let mut doc = Document::new();
    let mut history = History::new();
    let sketch = new_sketch(&mut doc, &mut history);
    let origin: Id<SkPoint> = *doc.get(sketch).unwrap().points.keys().next().unwrap();
    let created = run(
        &mut doc,
        &mut history,
        sketch,
        SketchOp::AddLine {
            a: [0.0, 0.0, 0.0],
            b: [0.03, 0.0004, 0.0],
            snap_a: EndSnap { point: Some(origin), curve: None },
            snap_b: EndSnap::default(),
            horizontal: Some(true),
        },
    );
    assert_eq!(
        created.iter().filter(|c| matches!(c, Created::Constraint(_))).count(),
        2,
        "coincident + horizontal"
    );
    let Created::Curve(line) = created[0] else { panic!("no line") };
    assert_eq!(end_of(&doc, sketch, line)[1], 0.0, "the line was made exactly horizontal");
    let report = doc.get(sketch).unwrap().clone().solve().unwrap();
    assert_eq!((report.status, report.dof), (Status::Solved, 1), "only the length is left");
    // One click, one undo step.
    history.undo(&mut doc).unwrap();
    assert!(doc.get(sketch).unwrap().curves.is_empty());
}

#[test]
fn drag_results_are_written_in_one_step_and_must_satisfy_the_constraints() {
    let (mut doc, mut history, sketch, _, _, lines) = rectangle_driven_by_parameter();
    let s = doc.get(sketch).unwrap();
    let top_right = s.line(lines[1]).unwrap().1;
    let mut drag = s.begin_drag(gc_sketch::SketchGrab::Point(top_right), [0.040, 0.005]).unwrap();
    drag.update([0.040, 0.012]);
    let mut preview = s.clone();
    drag.apply(&mut preview);
    let points: Vec<(Id<SkPoint>, [f64; 3])> = preview.points.iter().map(|(id, p)| (*id, p.pos)).collect();
    let depth = history.undo_depth();
    run(&mut doc, &mut history, sketch, SketchOp::SetCoords { points, radii: Vec::new() });
    assert_eq!(history.undo_depth(), depth + 1);
    assert!((end_of(&doc, sketch, lines[1])[1] - 0.012).abs() < 1e-9);

    // Coordinates that break a constraint are refused.
    let before = doc.clone();
    let bad = SketchOp::SetCoords { points: vec![(top_right, [0.5, 0.5, 0.0])], radii: Vec::new() };
    let result = apply(&mut doc, &Command::Sketch { sketch, op: bad });
    // The solver may repair them (then the sketch is consistent) or refuse; it must never commit a violation.
    assert!(doc.get(sketch).unwrap().violation().unwrap() <= 1.0);
    if result.is_err() {
        assert!(doc.same_content(&before));
    }
}
