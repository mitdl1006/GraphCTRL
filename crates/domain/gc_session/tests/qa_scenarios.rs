//! QA gap fillers for appendix C (S7-S9) and edge cases of recent features, driven through the session.

#![allow(clippy::float_cmp, clippy::unwrap_used, clippy::panic, reason = "test code")]

use gc_doc::{CellOp, Command, Created, Placement, SketchOp, WaypointTarget};
use gc_session::{PlanStatus, Session};
use gc_sketch::{GeoConstraint, Item};
use gc_tools::{Key, ToolEvent, ToolKind};

const PX: f64 = 1e-4;

fn six_axis() -> String {
    let path = format!("{}/../../../fixtures/urdf/six_axis.urdf", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn click(session: &mut Session, at: [f64; 2]) {
    session.tool_event(ToolEvent::Move { pos: at, px: PX, suppress_snap: false });
    session.tool_event(ToolEvent::Click { pos: at, px: PX, modifier: false });
}

fn dof(session: &Session) -> usize {
    session.report().unwrap().dof
}

/// S9: deleting a glyph (a constraint) frees the degrees of freedom it removed: horizontal +1,
/// the corner coincident +2.
#[test]
fn s9_deleting_constraint_glyphs_frees_degrees_of_freedom() {
    let mut session = Session::new();
    session.create_sketch("XY").unwrap();
    session.set_tool(ToolKind::Line);
    click(&mut session, [0.0, 0.0]);
    click(&mut session, [0.030, 0.0]);
    session.set_tool(ToolKind::Select);
    assert_eq!(dof(&session), 1);
    let find = |session: &Session, horizontal: bool| {
        let sketch = session.active_sketch().unwrap().1;
        sketch
            .constraints
            .iter()
            .find(|(_, c)| matches!(c, GeoConstraint::Horizontal(..)) == horizontal)
            .map(|(id, _)| *id)
            .unwrap()
    };
    let delete = |session: &mut Session, id| {
        session.tool_event(ToolEvent::ClickItem {
            item: Some(Item::Constraint(id)),
            pos: [0.0; 2],
            modifier: false,
        });
        session.tool_event(ToolEvent::Key(Key::Delete));
    };
    let h = find(&session, true);
    delete(&mut session, h);
    assert_eq!(dof(&session), 2, "horizontal removed one");
    let c = find(&session, false);
    delete(&mut session, c);
    assert_eq!(dof(&session), 4, "the corner coincident removed two");
    assert!(session.undo() && session.undo());
    assert_eq!(dof(&session), 1);
}

/// S7: Tab cycles the 3D working plane XY -> YZ -> ZX -> XY.
#[test]
fn s7_tab_cycles_the_working_plane() {
    let mut session = Session::new();
    session.create_sketch_3d().unwrap();
    let start = session.spatial_plane();
    assert_eq!(start, 2);
    let mut seen = vec![start];
    for _ in 0..3 {
        session.cycle_spatial_plane();
        seen.push(session.spatial_plane());
    }
    assert_eq!(seen, vec![2, 0, 1, 2]);
}

/// S8: deleting a sketch point under a waypoint detaches nothing silently: either it is refused, or the
/// waypoint survives. The waypoint is never deleted behind the user's back, and undo restores everything.
#[test]
fn s8_deleting_a_sketch_point_keeps_the_waypoint() {
    let mut session = Session::new();
    let robot = session.add_robot("Robot1", &six_axis()).unwrap();
    let placement = Placement { xyz: [0.0, -0.6, 0.0], rpy: [0.0; 3] };
    session.execute(&Command::Cell(CellOp::PlaceRobot { id: robot, placement })).unwrap();
    let sketch = session.create_sketch("XZ").unwrap();
    let created = session
        .execute(&Command::Sketch { sketch, op: SketchOp::AddPoint { pos: [0.1, 0.5, 0.0] } })
        .unwrap();
    let Created::Point(point) = created[0] else { panic!("no point") };
    session.finish_sketch();
    let waypoint = session.add_waypoint(robot, WaypointTarget::SketchPoint { sketch, point }).unwrap();
    let path = session.add_path(robot, vec![waypoint]).unwrap();
    session.jog(robot, &[1.5, -0.3, 0.6, 0.0, 0.6, 0.0]);
    session.end_jog();
    assert_eq!(session.plan(path), PlanStatus::Ready, "{:?}", session.plan_error(path));

    let before = session.doc().clone();
    let result = session.execute(&Command::Sketch { sketch, op: SketchOp::Delete(vec![Item::Point(point)]) });
    assert!(result.is_ok(), "{result:?}");
    {
        assert!(session.doc().get(waypoint).is_some(), "the waypoint must survive");
        assert!(matches!(
            session.plan_status(path),
            PlanStatus::Stale | PlanStatus::Failed | PlanStatus::NotPlanned
        ));
        assert!(session.undo());
    }
    assert!(session.doc().same_content(&before), "undo (or refusal) restores the document exactly");
}
