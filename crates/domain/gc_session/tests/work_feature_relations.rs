//! Relationships on work features of the assembly: Mate, Flush, Angle and the rest take "a point, curve,
//! face, or work feature" (Inventor 2026 help, To Place Mate or Flush Constraints in Assemblies,
//! GUID-0F500089-4F69-41C5-AFE8-B58AC9E8E15E). A work feature holds still in the world.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "tests")]

use gc_core::Id;
use gc_doc::{
    AxisSense, Body, CellOp, Command, Created, EditError, Geometry, Occurrence, OriginPlane, Placement,
    PlaneRef, PointRef, RelationKind, Relationship, Selection, Solid, WorkFeature, WorkGeometry,
    possible_relations,
};
use gc_session::Session;

fn body(s: &mut Session, xyz: [f64; 3]) -> Id<Body> {
    s.add_body(Solid::Box { size: [0.2; 3] }, Placement { xyz, rpy: [0.0; 3] }).unwrap()
}

fn work(s: &mut Session, geometry: WorkGeometry) -> Id<WorkFeature> {
    let feature = WorkFeature { name: "W".into(), geometry, visible: true };
    match s.execute(&Command::Cell(CellOp::AddWorkFeature(feature))).unwrap()[..] {
        [Created::WorkFeature(id)] => id,
        ref other => panic!("{other:?}"),
    }
}

fn bottom(b: Id<Body>) -> Selection {
    Selection {
        occurrence: Occurrence::Item(b.erase()),
        geometry: Geometry::Plane { point: [0.0, 0.0, -0.1], normal: [0.0, 0.0, -1.0] },
    }
}

fn z_of(s: &Session, b: Id<Body>) -> f64 {
    s.doc().component_origin(Occurrence::Item(b.erase())).unwrap().pose.translation.z
}

fn lifted(z: f64) -> WorkGeometry {
    WorkGeometry::PlaneOffset { base: PlaneRef::Origin(OriginPlane::Xy), distance: z }
}

#[test]
fn a_component_mates_to_a_work_plane_and_follows_it() {
    let mut s = Session::new();
    let _ground = body(&mut s, [2.0, 0.0, 0.0]);
    let b = body(&mut s, [0.0, 0.0, 1.0]);
    let plane = work(&mut s, lifted(0.3));
    let on_plane = s.work_feature_selection(plane).unwrap();
    // Plane with plane: Mate and Flush are offered.
    let offered = possible_relations(&bottom(b), &on_plane);
    assert!(!offered.is_empty(), "{offered:?}");
    s.add_relationship(Relationship::new(
        "Mate",
        RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
        bottom(b),
        on_plane,
    ))
    .unwrap();
    assert!((z_of(&s, b) - 0.4).abs() < 1e-6, "the bottom face sits on the plane: {}", z_of(&s, b));
    // Moving the work plane moves the component with it.
    let feature = WorkFeature { name: "W".into(), geometry: lifted(0.5), visible: true };
    s.execute(&Command::Cell(CellOp::SetWorkFeature { id: plane, feature })).unwrap();
    assert!((z_of(&s, b) - 0.6).abs() < 1e-6, "{}", z_of(&s, b));
    // The work plane never moves to meet the component.
    assert!(s.doc().solve_relationships().is_ok());
}

#[test]
fn axes_and_points_take_relationships_too() {
    let mut s = Session::new();
    let _ground = body(&mut s, [2.0, 0.0, 0.0]);
    let b = body(&mut s, [0.3, 0.4, 0.0]);
    let axis = work(
        &mut s,
        WorkGeometry::AxisThroughPoints {
            points: [PointRef::Fixed([1.0, 1.0, 0.0]), PointRef::Fixed([1.0, 1.0, 1.0])],
        },
    );
    let body_axis = Selection {
        occurrence: Occurrence::Item(b.erase()),
        geometry: Geometry::Axis { point: [0.0; 3], direction: [0.0, 0.0, 1.0] },
    };
    s.add_relationship(Relationship::new(
        "Mate",
        RelationKind::Mate { offset: 0.0, sense: AxisSense::Aligned },
        body_axis,
        s.work_feature_selection(axis).unwrap(),
    ))
    .unwrap();
    let at = s.doc().component_origin(Occurrence::Item(b.erase())).unwrap().pose.translation;
    assert!((at.x - 1.0).abs() < 1e-6 && (at.y - 1.0).abs() < 1e-6, "on the axis: {at:?}");

    let c = body(&mut s, [0.0, -0.5, 0.2]);
    let point = work(&mut s, WorkGeometry::Point { at: PointRef::Fixed([-0.5, -0.5, 0.5]) });
    let corner =
        Selection { occurrence: Occurrence::Item(c.erase()), geometry: Geometry::Point([0.1, 0.1, 0.1]) };
    s.add_relationship(Relationship::new(
        "Mate",
        RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
        corner,
        s.work_feature_selection(point).unwrap(),
    ))
    .unwrap();
    let at = s.doc().component_origin(Occurrence::Item(c.erase())).unwrap().pose;
    let corner_world = at.translation + at.rotation * gc_math::DVec3::splat(0.1);
    assert!((corner_world - gc_math::DVec3::new(-0.5, -0.5, 0.5)).length() < 1e-6, "{corner_world:?}");
}

#[test]
fn a_work_feature_in_a_relationship_is_not_deleted_and_round_trips() {
    let mut s = Session::new();
    let _ground = body(&mut s, [2.0, 0.0, 0.0]);
    let b = body(&mut s, [0.0, 0.0, 1.0]);
    let plane = work(&mut s, lifted(0.3));
    let r = s
        .add_relationship(Relationship::new(
            "Flush",
            RelationKind::Flush { offset: 0.0 },
            bottom(b),
            s.work_feature_selection(plane).unwrap(),
        ))
        .unwrap();
    let before = s.doc().clone();
    assert_eq!(s.execute(&Command::Cell(CellOp::DeleteWorkFeature(plane))), Err(EditError::InUse));
    assert!(s.doc().same_content(&before));
    // Saved and opened: the relationship still holds the component to the plane.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cell.gcp");
    s.save_as(&path).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert!(t.doc().solve_relationships().is_ok());
    assert!((z_of(&t, b) - z_of(&s, b)).abs() < 1e-9);
    // Without the relationship it can go.
    s.delete_relationship(r).unwrap();
    s.execute(&Command::Cell(CellOp::DeleteWorkFeature(plane))).unwrap();
    // A missing work feature cannot be selected.
    assert!(s.work_feature_selection(plane).is_none());
}
