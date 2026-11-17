//! The example cell offered in the File menu: a UR5e beside a pillar, a work frame on a dimensioned
//! sketch point, two waypoints taught in that frame on either side of the pillar, and a path through them.

use gc_doc::{Command, Created, Placement, SketchOp, Solid, WaypointTarget};
use gc_session::Session;
use gc_sketch::DimKind;

/// Where the arm starts: upright, tool pointing down.
const HOME: [f64; 6] = [0.0, -1.9, 1.7, -1.37, -1.57, 0.0];

/// The library robot the example cell is built around.
pub(crate) const ROBOT: &str = "UR5e";

/// Replaces the session's document with the example cell and starts planning its path. `robot` is the
/// library's [`ROBOT`].
pub(crate) fn open(session: &mut Session, robot: &crate::app::LibraryRobot) -> Result<(), String> {
    let failed = |e: gc_doc::EditError| e.to_string();
    session.new_document();
    let robot = robot.add(session)?;
    session.jog(robot, &HOME);
    session.end_jog();

    // A pillar in the way, in front of the robot.
    session
        .add_body(Solid::Box { size: [0.08, 0.08, 0.9] }, Placement { xyz: [0.45, 0.1, 0.45], rpy: [0.0; 3] })
        .map_err(failed)?;

    // A floor point 450 mm in front of the robot and 100 mm to its left, held by two dimensions: the origin of a work frame.
    let sketch = session.create_sketch("XY").map_err(failed)?;
    let origin = session
        .active_sketch()
        .and_then(|(_, sketch)| sketch.points.keys().next().copied())
        .ok_or("the sketch has no origin point")?;
    let created = session
        .execute(&Command::Sketch { sketch, op: SketchOp::AddPoint { pos: [0.45, 0.1, 0.0] } })
        .map_err(failed)?;
    let Some(Created::Point(point)) = created.first().copied() else {
        return Err("the point was not created".to_owned());
    };
    for (kind, source) in [
        (DimKind::HDistance { p: origin, q: point, sign: 1.0 }, "450 mm"),
        (DimKind::VDistance { p: origin, q: point, sign: 1.0 }, "100 mm"),
    ] {
        let op = SketchOp::AddDimension { kind, source: source.to_owned(), text_pos: [0.0; 2] };
        session.execute(&Command::Sketch { sketch, op }).map_err(failed)?;
    }
    let frame = session.add_work_frame(sketch, point).map_err(failed)?;
    session.finish_sketch();

    // Two waypoints in the frame, on either side of the pillar, tool pointing down; the path between
    // them has to go around it.
    let down = |y: f64| Placement { xyz: [0.0, y, 0.35], rpy: [std::f64::consts::PI, 0.0, 0.0] };
    let left =
        session.add_waypoint(robot, WaypointTarget::InFrame { frame, offset: down(0.25) }).map_err(failed)?;
    let right = session
        .add_waypoint(robot, WaypointTarget::InFrame { frame, offset: down(-0.25) })
        .map_err(failed)?;
    let path = session.add_path(robot, vec![left, right]).map_err(failed)?;
    session.plan(path);
    Ok(())
}
