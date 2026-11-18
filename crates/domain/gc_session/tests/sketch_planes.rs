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

#[test]
fn the_tree_wrappers_rename_hide_suppress_reorder_and_roll_back() {
    let mut session = Session::new();
    let sketch = session.create_sketch("XY").unwrap();
    session.finish_sketch();
    let Created::WorkFeature(plane) =
        session.execute(&Command::Cell(CellOp::AddWorkFeature(offset_plane(0.2)))).unwrap()[0]
    else {
        panic!()
    };
    let (s, p) = (sketch.erase(), plane.erase());
    session.rename(s, "Base").unwrap();
    assert_eq!(session.doc().get(sketch).unwrap().name, "Base");
    // Names are unique within a kind: another sketch cannot be "Base", a work plane can.
    let other = session.create_sketch("XZ").unwrap();
    session.finish_sketch();
    assert!(matches!(session.rename(other.erase(), "Base"), Err(gc_doc::EditError::NameTaken)));
    session.rename(p, "Base").unwrap();
    session.execute(&Command::DeleteSketch { id: other }).unwrap();
    session.set_visible(s, false).unwrap();
    assert!(!session.is_visible(s));
    session.set_suppressed(p, true).unwrap();
    assert!(session.is_suppressed(p));
    session.set_suppressed(p, false).unwrap();
    assert_eq!(session.model_order(), vec![s, p]);
    session.move_item(p, Some(s)).unwrap();
    assert_eq!(session.model_order(), vec![p, s]);
    session.set_rollback(Some(0)).unwrap();
    assert_eq!(session.rollback_index(), Some(0));
    assert!(session.is_suppressed(s), "after the marker");
    session.set_rollback(None).unwrap();
    assert!(session.set_rollback(Some(9)).is_err());
    assert!(session.dependents_of(p).is_empty());
}

fn machine() -> gc_doc::Assembly {
    // The table part stands 1 m along X and is turned a quarter about Z.
    let table = [[0.0, 1.0, 0.0, 0.0], [-1.0, 0.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [1.0, 0.0, 0.0, 1.0]];
    gc_doc::Assembly {
        name: "Machine".into(),
        file: "machine.glb".into(),
        placement: Placement::default(),
        parts: vec![gc_doc::AssemblyPart {
            name: "Table".into(),
            parent: None,
            matrix: table,
            meshes: vec![0],
        }],
        meshes: vec![gc_doc::AssemblyMesh { name: "Plate".into(), color: [0.5; 4], triangles: 12 }],
        look: Default::default(),
    }
}

#[test]
fn every_component_has_an_origin_and_a_sketch_on_its_plane_moves_with_it() {
    use gc_doc::Occurrence;
    let mut session = Session::new();
    let Created::Assembly(id) = session.execute(&Command::Cell(CellOp::AddAssembly(machine()))).unwrap()[0]
    else {
        panic!()
    };
    let table = Occurrence::Part { assembly: id, index: 0 };
    let origin = session.component_origin(table).unwrap();
    assert!((origin.point() - DVec3::new(1.0, 0.0, 0.0)).length() < 1e-12);
    assert!((origin.axis(0) - DVec3::Y).length() < 1e-12, "the part's X axis is the world's Y");
    // Its XZ plane: normal along the part's −Y, which is the world's +X.
    let xz = origin.plane(OriginPlane::Xz);
    assert!((xz.rotation * DVec3::Z - DVec3::X).length() < 1e-12);

    // A sketch on the part's XY plane follows when the assembly moves.
    let sketch = session.create_sketch_on_component(table, OriginPlane::Xy).unwrap();
    session.execute(&Command::Sketch { sketch, op: SketchOp::AddPoint { pos: [0.1, 0.0, 0.0] } }).unwrap();
    assert!((world(&session, sketch, [0.1, 0.0, 0.0]) - DVec3::new(1.0, 0.1, 0.0)).length() < 1e-12);
    session.finish_sketch();
    let placement = Placement { xyz: [0.0, 0.0, 0.5], rpy: [0.0; 3] };
    session.execute(&Command::Cell(CellOp::PlaceAssembly { id, placement })).unwrap();
    assert!((world(&session, sketch, [0.1, 0.0, 0.0]) - DVec3::new(1.0, 0.1, 0.5)).length() < 1e-12);
    // The assembly cannot be deleted while the sketch stands on it.
    assert!(session.execute(&Command::Cell(CellOp::DeleteAssembly(id))).is_err());

    // A work plane offset from the assembly's own XY plane follows it too.
    let feature = WorkFeature {
        name: "Above".into(),
        geometry: WorkGeometry::PlaneOffset {
            base: PlaneRef::Component(Occurrence::Item(id.erase()), OriginPlane::Xy),
            distance: 0.2,
        },
        visible: true,
    };
    let Created::WorkFeature(above) =
        session.execute(&Command::Cell(CellOp::AddWorkFeature(feature))).unwrap()[0]
    else {
        panic!()
    };
    let Some(gc_doc::WorkShape::Plane(pose)) = session.work_shape(above) else { panic!() };
    assert!((pose.translation - DVec3::new(0.0, 0.0, 0.7)).length() < 1e-12);
    // A missing component is refused.
    assert!(session.component_origin(Occurrence::Part { assembly: id, index: 3 }).is_none());

    // Origin elements are hidden by default; showing some is one undo step and is saved.
    use gc_doc::OriginElement;
    let xy = OriginElement::Plane(OriginPlane::Xy);
    assert!(!session.origin_visible(table, xy));
    session.set_origin_visible(table, &[xy, OriginElement::Axis(2), OriginElement::Point], true).unwrap();
    assert!(session.origin_visible(table, xy) && session.origin_visible(table, OriginElement::Point));
    session.set_origin_visible(table, &[OriginElement::Point], false).unwrap();
    assert!(!session.origin_visible(table, OriginElement::Point));
    session.undo();
    assert!(session.origin_visible(table, OriginElement::Point));
    assert!(session.set_origin_visible(table, &[OriginElement::Axis(3)], true).is_err());
    let directory = tempfile::tempdir().unwrap();
    let project = directory.path().join("origins.gctrl");
    session.save_as(&project).unwrap();
    let mut reopened = Session::new();
    reopened.open(&project).unwrap();
    assert!(reopened.origin_visible(table, OriginElement::Axis(2)));
    assert!(!reopened.origin_visible(table, OriginElement::Plane(OriginPlane::Yz)));
}

/// Autoproject edges for sketch creation (Inventor 2026 help, Sketch Tab Reference,
/// GUID-1898C432-CFD2-46F1-8BAE-631392DFBF1D; About Projecting Sketch Geometry,
/// GUID-B61431DA-FABC-41EE-945C-6E54B4806582): a sketch on a face takes the face's edges.
#[test]
fn a_sketch_on_a_face_takes_the_face_outline() {
    // A square top of two triangles, and a side triangle going down from one edge.
    let mesh = || {
        gc_mesh::MeshData::new(
            vec![[0.0; 3], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]],
            vec![0, 1, 2, 0, 2, 3, 0, 4, 1],
        )
        .unwrap()
    };
    let assembly = || gc_doc::Assembly {
        name: "Plate".into(),
        file: "plate.glb".into(),
        placement: Placement::default(),
        parts: vec![gc_doc::AssemblyPart {
            name: "Plate".into(),
            parent: None,
            matrix: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
            meshes: vec![0],
        }],
        meshes: vec![gc_doc::AssemblyMesh { name: "Plate".into(), color: [1.0; 4], triangles: 3 }],
        look: Default::default(),
    };
    let mut session = Session::new();
    let id = session.add_assembly(assembly(), vec![mesh()]).unwrap();
    session.create_sketch_on_face(id, 0, 0, 0).unwrap();
    let (_, sketch) = session.active_sketch().unwrap();
    let lines: Vec<f64> = sketch
        .curves
        .keys()
        .filter_map(|c| sketch.line(*c))
        .map(|(a, b)| {
            let (a, b) = (sketch.pos(a).unwrap(), sketch.pos(b).unwrap());
            ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt()
        })
        .collect();
    assert_eq!(lines.len(), 4, "the square's four sides, not its diagonal or the side face: {lines:?}");
    assert!(lines.iter().all(|l| (l - 1.0).abs() < 1e-9));
    // Cancel Sketch takes the sketch and its outline back together.
    let sketches = session.doc().sketches().len();
    assert!(session.cancel_sketch());
    assert_eq!(session.doc().sketches().len(), sketches - 1);

    // With the option off, the sketch starts empty.
    session.set_autoproject_face_edges(false);
    session.create_sketch_on_face(id, 0, 0, 0).unwrap();
    assert!(session.active_sketch().unwrap().1.curves.is_empty());
}
