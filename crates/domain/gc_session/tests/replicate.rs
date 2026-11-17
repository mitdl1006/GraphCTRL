//! Mirror, copy and replace through the session: the names of relationships left suppressed come back for
//! the message (Inventor 2026 help, About Mirrored Assembly Components,
//! GUID-66AA4A06-F63F-49C3-8F39-42FE4999F1FC; Replace assembly components,
//! GUID-E5FAA224-7332-4AC6-A78E-793FDACFEEDD).

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use gc_doc::{
    AxisSense, Geometry, Occurrence, Placement, RelationKind, Relationship, Replacement, Selection, Solid,
};
use gc_session::Session;

fn six_axis() -> String {
    let path = format!("{}/../../../fixtures/urdf/six_axis.urdf", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn replace_reports_the_relationships_that_did_not_survive() {
    let mut session = Session::new();
    let table = session
        .add_body(Solid::Box { size: [1.0, 1.0, 1.0] }, Placement { xyz: [0.0, 0.0, 0.5], rpy: [0.0; 3] })
        .unwrap();
    let block = session
        .add_body(Solid::Box { size: [0.2; 3] }, Placement { xyz: [0.2, 0.0, 1.3], rpy: [0.0; 3] })
        .unwrap();
    let on = |id: gc_core::Id<gc_doc::Body>, z: f64, n: f64| Selection {
        occurrence: Occurrence::Item(id.erase()),
        geometry: Geometry::Plane { point: [0.0, 0.0, z], normal: [0.0, 0.0, n] },
    };
    session
        .add_relationship(Relationship::new(
            "Mate:1",
            RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
            on(block, -0.1, -1.0),
            on(table, 0.5, 1.0),
        ))
        .unwrap();
    let lost = session
        .replace_component(block.erase(), Replacement::Solid(Solid::Sphere { radius: 0.1 }), false)
        .unwrap();
    assert_eq!(lost, ["Mate:1"]);
    session.undo();
    let kept = session
        .replace_component(block.erase(), Replacement::Solid(Solid::Box { size: [0.5, 0.5, 0.2] }), true)
        .unwrap();
    assert!(kept.is_empty());
}

#[test]
fn a_robot_mirrored_across_the_cell_gets_its_own_link_tree() {
    let mut session = Session::new();
    let robot = session.add_robot("Robot1", &six_axis()).unwrap();
    session
        .execute(&gc_doc::Command::Cell(gc_doc::CellOp::PlaceRobot {
            id: robot,
            placement: Placement { xyz: [1.0, 0.0, 0.0], rpy: [0.0; 3] },
        }))
        .unwrap();
    let floor = session
        .add_body(Solid::Box { size: [4.0, 4.0, 0.1] }, Placement { xyz: [0.0, 0.0, -0.05], rpy: [0.0; 3] })
        .unwrap();
    let yz = Selection {
        occurrence: Occurrence::Item(floor.erase()),
        geometry: Geometry::origin(gc_doc::OriginElement::Plane(gc_doc::OriginPlane::Yz)),
    };
    let suppressed = session.mirror_components(&[robot.erase()], yz, true, false).unwrap();
    assert!(suppressed.is_empty());
    let copy = session.doc().robots().iter().map(|(id, _)| id).find(|id| *id != robot).unwrap();
    assert!((session.doc().get(copy).unwrap().placement.xyz[0] + 1.0).abs() < 1e-9);
    assert_eq!(session.doc().get(copy).unwrap().name, "Robot1:1");
    assert!(session.link_poses(copy).is_some());
    assert_eq!(session.robot_links(copy).len(), session.robot_links(robot).len());
}
