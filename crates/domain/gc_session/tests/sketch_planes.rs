//! Sketches on work planes and on faces of imported parts: they follow the plane, survive saving and
//! reopening, and keep what they stand on from being deleted.

#![allow(clippy::float_cmp, clippy::unwrap_used, clippy::panic, reason = "test code")]

use gc_doc::{
    CellOp, Command, Created, OriginPlane, Placement, PlaneRef, SketchOp, WorkFeature, WorkGeometry,
};
use gc_math::DVec3;
use gc_session::Session;
use gc_sketch::Space;

/// The world position of a point of a sketch.
fn world(session: &Session, sketch: gc_core::Id<gc_doc::Sketch>, pos: [f64; 3]) -> DVec3 {
    let Space::Planar(frame) = session.doc().get(sketch).unwrap().space else { panic!("not planar") };
    let pose = session.doc().frame_world_pose(gc_core::Id::from_raw(frame.raw)).unwrap();
    pose.translation + pose.rotation * DVec3::from_array(pos)
}

fn offset_plane(distance: f64) -> WorkFeature {
    WorkFeature {
        name: "Plane1".into(),
        geometry: WorkGeometry::PlaneOffset { base: PlaneRef::Origin(OriginPlane::Xy), distance },
        visible: true,
    }
}

#[test]
fn a_sketch_on_a_work_plane_follows_it_and_survives_a_reload() {
    let mut session = Session::new();
    let Created::WorkFeature(plane) =
        session.execute(&Command::Cell(CellOp::AddWorkFeature(offset_plane(0.3)))).unwrap()[0]
    else {
        panic!()
    };
    let sketch = session.create_sketch_on_work_plane(plane).unwrap();
    assert_eq!(session.active_sketch().map(|(id, _)| id), Some(sketch), "editing it at once");
    session.execute(&Command::Sketch { sketch, op: SketchOp::AddPoint { pos: [0.1, 0.2, 0.0] } }).unwrap();
    assert!((world(&session, sketch, [0.1, 0.2, 0.0]) - DVec3::new(0.1, 0.2, 0.3)).length() < 1e-12);

    // The plane moves: the sketch goes with it.
    session
        .execute(&Command::Cell(CellOp::SetWorkFeature { id: plane, feature: offset_plane(0.5) }))
        .unwrap();
    assert!((world(&session, sketch, [0.1, 0.2, 0.0]) - DVec3::new(0.1, 0.2, 0.5)).length() < 1e-12);
    // It cannot be deleted while the sketch stands on it.
    assert!(session.execute(&Command::Cell(CellOp::DeleteWorkFeature(plane))).is_err());

    // Saved and reopened, the sketch is still attached.
    let directory = tempfile::tempdir().unwrap();
    let project = directory.path().join("planes.gctrl");
    session.save_as(&project).unwrap();
    let mut reopened = Session::new();
    reopened.open(&project).unwrap();
    reopened
        .execute(&Command::Cell(CellOp::SetWorkFeature { id: plane, feature: offset_plane(0.7) }))
        .unwrap();
    assert!((world(&reopened, sketch, [0.0; 3]) - DVec3::new(0.0, 0.0, 0.7)).length() < 1e-12);

    // Deleting the sketch frees the plane.
    reopened.execute(&Command::DeleteSketch { id: sketch }).unwrap();
    reopened.execute(&Command::Cell(CellOp::DeleteWorkFeature(plane))).unwrap();
}

#[test]
fn a_sketch_on_a_part_face_follows_the_assembly() {
    // One part: a triangle in the part's XY plane, 0.5 m up in the assembly, which stands 1 m along X.
    let triangle =
        || gc_mesh::MeshData::new(vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], vec![0, 1, 2]).unwrap();
    let assembly = gc_doc::Assembly {
        name: "Table".into(),
        file: "table.glb".into(),
        placement: Placement { xyz: [1.0, 0.0, 0.0], rpy: [0.0; 3] },
        parts: vec![gc_doc::AssemblyPart {
            name: "Top".into(),
            parent: None,
            matrix: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.5, 1.0]],
            meshes: vec![0],
        }],
        meshes: vec![gc_doc::AssemblyMesh { name: "Top".into(), color: [1.0; 4], triangles: 1 }],
        look: Default::default(),
    };
    let mut session = Session::new();
    let id = session.add_assembly(assembly, vec![triangle()]).unwrap();
    let sketch = session.create_sketch_on_face(id, 0, 0, 0).unwrap();
    // Origin at the triangle's centre, normal up (+Z), X along its first edge.
    let origin = world(&session, sketch, [0.0; 3]);
    assert!((origin - DVec3::new(1.0 + 1.0 / 3.0, 1.0 / 3.0, 0.5)).length() < 1e-12, "{origin:?}");
    assert!((world(&session, sketch, [0.0, 0.0, 1.0]) - origin - DVec3::Z).length() < 1e-12);
    assert!((world(&session, sketch, [1.0, 0.0, 0.0]) - origin - DVec3::X).length() < 1e-12);

    // The assembly moves: the sketch follows; the assembly cannot be deleted under it.
    let placement = Placement { xyz: [2.0, 0.0, 0.0], rpy: [0.0; 3] };
    session.execute(&Command::Cell(CellOp::PlaceAssembly { id, placement })).unwrap();
    assert!((world(&session, sketch, [0.0; 3]) - origin - DVec3::X).length() < 1e-12);
    assert!(session.execute(&Command::Cell(CellOp::DeleteAssembly(id))).is_err());

    // A part, mesh or triangle that is not there is refused.
    assert!(session.create_sketch_on_face(id, 0, 0, 5).is_err());
    assert!(session.create_sketch_on_face(id, 3, 0, 0).is_err());
}
