//! Dragging points of a 3D sketch: the preview follows a cursor in space, constraints hold, and the end
//! of the drag is one undo step.

#![allow(clippy::float_cmp, clippy::unwrap_used, clippy::panic, reason = "test code")]

use gc_doc::{Command, Created, SketchOp};
use gc_session::Session;

fn add_point(
    session: &mut Session,
    sketch: gc_core::Id<gc_doc::Sketch>,
    pos: [f64; 3],
) -> gc_core::Id<gc_sketch::SkPoint> {
    match session.execute(&Command::Sketch { sketch, op: SketchOp::AddPoint { pos } }).unwrap()[0] {
        Created::Point(point) => point,
        _ => panic!("no point"),
    }
}

#[test]
fn a_3d_sketch_point_follows_a_cursor_in_space_and_ends_as_one_undo_step() {
    let mut session = Session::new();
    let sketch = session.create_sketch_3d().unwrap();
    let point = add_point(&mut session, sketch, [0.1, 0.2, 0.3]);
    let depth = session.history().undo_depth();

    assert!(session.spatial_drag_start(point, [0.1, 0.2, 0.3]));
    for step in 1..=10 {
        let t = f64::from(step) / 10.0;
        session.spatial_drag_move([0.1 + 0.2 * t, 0.2, 0.3 + 0.1 * t]);
    }
    // The preview follows; the document does not change until the end.
    let preview = session.point_pos(point).unwrap();
    assert!((preview[0] - 0.3).abs() < 1e-6 && (preview[2] - 0.4).abs() < 1e-6, "{preview:?}");
    assert_eq!(session.doc().get(sketch).unwrap().points[&point].pos, [0.1, 0.2, 0.3]);
    session.spatial_drag_end();
    let moved = session.doc().get(sketch).unwrap().points[&point].pos;
    assert!((moved[0] - 0.3).abs() < 1e-6 && (moved[1] - 0.2).abs() < 1e-6 && (moved[2] - 0.4).abs() < 1e-6);
    assert_eq!(session.history().undo_depth(), depth + 1);
    assert!(session.undo());
    assert_eq!(session.doc().get(sketch).unwrap().points[&point].pos, [0.1, 0.2, 0.3]);

    // Cancelled, nothing changes.
    assert!(session.spatial_drag_start(point, [0.1, 0.2, 0.3]));
    session.spatial_drag_move([1.0, 1.0, 1.0]);
    session.spatial_drag_cancel();
    assert_eq!(session.doc().get(sketch).unwrap().points[&point].pos, [0.1, 0.2, 0.3]);
    assert!(!session.is_dragging());
}

#[test]
fn a_point_of_a_planar_sketch_is_not_dragged_in_space() {
    let mut session = Session::new();
    let sketch = session.create_sketch("XY").unwrap();
    let point = add_point(&mut session, sketch, [0.1, 0.2, 0.0]);
    assert!(!session.spatial_drag_start(point, [0.1, 0.2, 0.0]));
    assert_eq!(session.spatial_drag_move([0.5, 0.5, 0.5]), None);
}
