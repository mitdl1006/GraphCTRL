//! Project Geometry (assembly P1-5): component points, edges and axes and work features projected onto a
//! sketch follow their source; deleting the source leaves fixed geometry with its constraints.

#![allow(clippy::float_cmp, clippy::unwrap_used, clippy::panic, reason = "test code")]

use gc_doc::{CellOp, Command, Created, EndSnap, Occurrence, Placement, ProjectSource, SketchOp};
use gc_session::Session;
use gc_sketch::{CurveGeom, GeoConstraint, PointKind};

fn project(
    session: &mut Session,
    sketch: gc_core::Id<gc_doc::Sketch>,
    source: ProjectSource,
    construction: bool,
) -> Vec<Created> {
    session.execute(&Command::Sketch { sketch, op: SketchOp::Project { source, construction } }).unwrap()
}

#[test]
fn a_projected_point_follows_the_component_and_a_line_tied_to_it_follows_too() {
    let mut session = Session::new();
    let body = session
        .add_body(gc_doc::Solid::Box { size: [0.2; 3] }, Placement { xyz: [0.3, 0.1, 0.5], rpy: [0.0; 3] })
        .unwrap();
    let occurrence = Occurrence::Item(body.erase());
    let sketch = session.create_sketch("XY").unwrap();
    let created = project(&mut session, sketch, ProjectSource::Point { occurrence, local: [0.0; 3] }, false);
    let Created::Point(centre) = created[0] else { panic!() };
    let pos = |session: &Session, p| session.doc().get(sketch).unwrap().pos(p).unwrap();
    assert_eq!(pos(&session, centre), [0.3, 0.1, 0.0], "projected straight down onto XY");
    // A line from the projected point.
    let snap = EndSnap { point: Some(centre), ..EndSnap::default() };
    let op = SketchOp::AddLine {
        a: [0.3, 0.1, 0.0],
        b: [0.0, 0.0, 0.0],
        snap_a: snap,
        snap_b: EndSnap::default(),
        horizontal: None,
    };
    let created = session.execute(&Command::Sketch { sketch, op }).unwrap();
    let Created::Curve(line) = created[0] else { panic!() };
    session.finish_sketch();

    // The body moves: the projection and the line's end go with it.
    let mut moved = session.doc().get(body).unwrap().clone();
    moved.placement.xyz = [0.6, -0.2, 0.5];
    session.execute(&Command::Cell(CellOp::SetBody { id: body, body: moved })).unwrap();
    assert_eq!(pos(&session, centre), [0.6, -0.2, 0.0]);
    let (a, _) = session.doc().get(sketch).unwrap().line(line).unwrap();
    let a = pos(&session, a);
    assert!((a[0] - 0.6).abs() < 1e-9 && (a[1] + 0.2).abs() < 1e-9, "{a:?}");

    // Deleting the body leaves a fixed point; the line keeps its coincidence.
    session.execute(&Command::Cell(CellOp::DeleteBody(body))).unwrap();
    let s = session.doc().get(sketch).unwrap();
    assert_eq!(s.points[&centre].kind, PointKind::Free);
    assert!(s.constraints.values().any(|c| *c == GeoConstraint::Fix(centre)));
    assert!(s.constraints.values().any(
        |c| matches!(c, GeoConstraint::Coincident(p, _) | GeoConstraint::Coincident(_, p) if *p == centre)
    ));
    assert_eq!(pos(&session, centre), [0.6, -0.2, 0.0]);
}

#[test]
fn edges_axes_and_work_features_project_as_reference_lines() {
    let mut session = Session::new();
    let body = session
        .add_body(
            gc_doc::Solid::Box { size: [0.2; 3] },
            Placement { xyz: [0.0, 0.0, 0.3], rpy: [0.0, 0.0, std::f64::consts::FRAC_PI_2] },
        )
        .unwrap();
    let occurrence = Occurrence::Item(body.erase());
    let sketch = session.create_sketch("XY").unwrap();
    // An edge along the body's X (the world's Y after the quarter turn), as construction.
    let created = project(
        &mut session,
        sketch,
        ProjectSource::Edge { occurrence, a: [-0.1, 0.1, 0.1], b: [0.1, 0.1, 0.1] },
        true,
    );
    let Created::Curve(edge) = created[2] else { panic!("{created:?}") };
    let s = session.doc().get(sketch).unwrap();
    assert!(s.curves[&edge].construction);
    let CurveGeom::Line { a, b } = s.curves[&edge].geom else { panic!() };
    let (a, b) = (s.pos(a).unwrap(), s.pos(b).unwrap());
    assert!((a[0] + 0.1).abs() < 1e-9 && (a[1] + 0.1).abs() < 1e-9, "{a:?}");
    assert!((b[0] + 0.1).abs() < 1e-9 && (b[1] - 0.1).abs() < 1e-9, "{b:?}");
    // The body's Z axis is seen end-on from above: it projects to a single point.
    let created = project(&mut session, sketch, ProjectSource::OriginAxis { occurrence, axis: 2 }, false);
    assert_eq!(created.len(), 1);
    // A work axis along world X through (0, 0.2, 0).
    let axis = gc_doc::WorkFeature {
        name: "Axis1".into(),
        geometry: gc_doc::WorkGeometry::AxisThroughPoints {
            points: [gc_doc::PointRef::Fixed([0.0, 0.2, 0.0]), gc_doc::PointRef::Fixed([1.0, 0.2, 0.0])],
        },
        visible: true,
    };
    let Created::WorkFeature(axis) =
        session.execute(&Command::Cell(CellOp::AddWorkFeature(axis))).unwrap()[0]
    else {
        panic!()
    };
    let created = project(&mut session, sketch, ProjectSource::WorkAxis(axis), false);
    assert_eq!(created.len(), 3, "two reference points and a line");
    let Created::Point(p) = created[0] else { panic!() };
    assert!((session.doc().get(sketch).unwrap().pos(p).unwrap()[1] - 0.2).abs() < 1e-9);
}

fn plane_at(session: &mut Session, height: f64) -> gc_core::Id<gc_doc::WorkFeature> {
    let feature = gc_doc::WorkFeature {
        name: format!("Plane{height}"),
        geometry: gc_doc::WorkGeometry::PlaneOffset {
            base: gc_doc::PlaneRef::Origin(gc_doc::OriginPlane::Xy),
            distance: height,
        },
        visible: true,
    };
    let Created::WorkFeature(id) =
        session.execute(&Command::Cell(CellOp::AddWorkFeature(feature))).unwrap()[0]
    else {
        panic!()
    };
    id
}

#[test]
fn project_cut_edges_traces_where_the_plane_cuts_a_box_as_a_fixed_snapshot() {
    let mut session = Session::new();
    let body = session
        .add_body(
            gc_doc::Solid::Box { size: [0.2, 0.4, 1.0] },
            Placement { xyz: [1.0, 0.0, 0.5], rpy: [0.0; 3] },
        )
        .unwrap();
    let occurrence = Occurrence::Item(body.erase());
    let plane = plane_at(&mut session, 0.3);
    let sketch = session.create_sketch_on_work_plane(plane).unwrap();
    assert_eq!(session.project_cut_edges(occurrence, false).unwrap(), 4, "a rectangle: the box's four sides");
    let s = session.doc().get(sketch).unwrap();
    let mut lengths: Vec<f64> = s
        .curves
        .keys()
        .map(|c| {
            let (a, b) = s.line(*c).unwrap();
            let (a, b) = (s.pos(a).unwrap(), s.pos(b).unwrap());
            ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
        })
        .collect();
    lengths.sort_by(f64::total_cmp);
    // Meshes are single precision.
    assert!((lengths[0] - 0.2).abs() < 1e-6 && (lengths[3] - 0.4).abs() < 1e-6, "{lengths:?}");
    assert_eq!(session.report().unwrap().dof, 0, "fixed geometry");
    // A snapshot: moving the box leaves the trace where it was.
    let before = session.doc().get(sketch).unwrap().points.clone();
    session.finish_sketch();
    let mut moved = session.doc().get(body).unwrap().clone();
    moved.placement.xyz = [2.0, 0.0, 0.5];
    session.execute(&Command::Cell(CellOp::SetBody { id: body, body: moved })).unwrap();
    assert_eq!(session.doc().get(sketch).unwrap().points, before);

    // A plane that misses the box adds nothing.
    let high = plane_at(&mut session, 2.0);
    session.create_sketch_on_work_plane(high).unwrap();
    assert_eq!(session.project_cut_edges(occurrence, false).unwrap(), 0);
}
