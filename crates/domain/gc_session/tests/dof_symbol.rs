//! The degrees-of-freedom symbol (Inventor 2026 help, View degrees of freedom in assemblies,
//! GUID-3BCBF614-CF92-4980-A965-80AEAFEF768E): the directions a component can still slide along and the
//! axes it can still turn about, and a cube when it is constrained to an under-constrained part.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use gc_core::Id;
use gc_doc::{
    AxisSense, Body, Geometry, Occurrence, Placement, RelationKind, Relationship, Selection, Solid,
};
use gc_math::DVec3;
use gc_session::Session;

fn on(id: Id<Body>, geometry: Geometry) -> Selection {
    Selection { occurrence: Occurrence::Item(id.erase()), geometry }
}

fn parallel(a: [f64; 3], b: DVec3) -> bool {
    DVec3::from_array(a).cross(b).length() < 1e-6
}

/// A grounded table (first body) and a free block above it.
fn bench() -> (Session, Id<Body>, Id<Body>) {
    let mut session = Session::new();
    let table = session
        .add_body(Solid::Box { size: [2.0, 2.0, 1.0] }, Placement { xyz: [0.0, 0.0, 0.5], rpy: [0.0; 3] })
        .unwrap();
    let block = session
        .add_body(Solid::Box { size: [0.2; 3] }, Placement { xyz: [0.3, 0.2, 1.5], rpy: [0.0; 3] })
        .unwrap();
    (session, table, block)
}

#[test]
fn a_free_component_shows_three_slides_and_three_turns_a_grounded_one_none() {
    let (session, table, block) = bench();
    let free = session.dof_symbol(block.erase()).unwrap();
    assert_eq!((free.translations.len(), free.rotations.len()), (3, 3));
    assert!(
        free.rotations
            .iter()
            .all(|a| (DVec3::from_array(a.point) - DVec3::new(0.3, 0.2, 1.5)).length() < 1e-9)
    );
    let grounded = session.dof_symbol(table.erase()).unwrap();
    assert!(grounded.translations.is_empty() && grounded.rotations.is_empty());
}

#[test]
fn a_mate_leaves_two_slides_in_the_plane_and_a_turn_about_its_normal() {
    let (mut session, table, block) = bench();
    session
        .add_relationship(Relationship::new(
            "Mate:1",
            RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
            on(block, Geometry::Plane { point: [0.0, 0.0, -0.1], normal: [0.0, 0.0, -1.0] }),
            on(table, Geometry::Plane { point: [0.0, 0.0, 0.5], normal: [0.0, 0.0, 1.0] }),
        ))
        .unwrap();
    let symbol = session.dof_symbol(block.erase()).unwrap();
    assert_eq!(symbol.translations.len(), 2);
    assert!(symbol.translations.iter().all(|t| t[2].abs() < 1e-6), "{symbol:?}");
    assert_eq!(symbol.rotations.len(), 1);
    assert!(parallel(symbol.rotations[0].direction, DVec3::Z));
    let dof = session.component_dof(block.erase()).unwrap();
    assert_eq!((dof.translation, dof.rotation()), (2, 1));
    assert!(symbol.relative_to.is_empty(), "the table is grounded");
}

#[test]
fn a_hinge_turns_about_its_own_line_and_a_cylinder_also_slides_along_it() {
    let (mut session, table, block) = bench();
    // Hinge line: vertical, through (0.5, 0.5) on the table top.
    let frame = Geometry::Frame { origin: [0.5, 0.5, 0.5], z: [0.0, 0.0, 1.0], x: [1.0, 0.0, 0.0] };
    let hinge = session
        .add_relationship(Relationship::new(
            "Rotational:1",
            RelationKind::Rotational { flip: true },
            on(block, Geometry::Frame { origin: [0.1, 0.0, -0.1], z: [0.0, 0.0, 1.0], x: [1.0, 0.0, 0.0] }),
            on(table, frame),
        ))
        .unwrap();
    let symbol = session.dof_symbol(block.erase()).unwrap();
    assert!(symbol.translations.is_empty(), "{symbol:?}");
    assert_eq!(symbol.rotations.len(), 1);
    let axis = symbol.rotations[0];
    assert!(parallel(axis.direction, DVec3::Z));
    // The axis passes through the hinge line, not the block's origin.
    let p = DVec3::from_array(axis.point);
    assert!((DVec3::new(p.x, p.y, 0.0) - DVec3::new(0.5, 0.5, 0.0)).length() < 1e-5, "{p}");

    let cylindrical = Relationship::new(
        "Cylindrical:1",
        RelationKind::Cylindrical { flip: true },
        on(block, Geometry::Frame { origin: [0.1, 0.0, -0.1], z: [0.0, 0.0, 1.0], x: [1.0, 0.0, 0.0] }),
        on(table, frame),
    );
    session.set_relationship(hinge, cylindrical).unwrap();
    let symbol = session.dof_symbol(block.erase()).unwrap();
    assert_eq!(symbol.translations.len(), 1);
    assert!(parallel(symbol.translations[0], DVec3::Z));
    assert_eq!(symbol.rotations.len(), 1);
}

#[test]
fn constrained_to_an_under_constrained_part_shows_the_cube_and_symbols_follow_the_setting() {
    let (mut session, _table, block) = bench();
    let lid = session
        .add_body(Solid::Box { size: [0.2; 3] }, Placement { xyz: [0.3, 0.2, 2.0], rpy: [0.0; 3] })
        .unwrap();
    session
        .add_relationship(Relationship::new(
            "Mate:1",
            RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
            on(lid, Geometry::Plane { point: [0.0, 0.0, -0.1], normal: [0.0, 0.0, -1.0] }),
            on(block, Geometry::Plane { point: [0.0, 0.0, 0.1], normal: [0.0, 0.0, 1.0] }),
        ))
        .unwrap();
    assert_eq!(session.dof_symbol(lid.erase()).unwrap().relative_to, vec![block.erase()]);

    assert!(session.dof_symbols().is_empty(), "none shown by default");
    session.set_dof_shown(&[lid.erase()], true).unwrap();
    assert_eq!(session.dof_symbols().iter().map(|(i, _)| *i).collect::<Vec<_>>(), vec![lid.erase()]);
    session.execute(&gc_doc::Command::Cell(gc_doc::CellOp::SetDofShownAll(true))).unwrap();
    assert_eq!(session.dof_symbols().len(), 3);
    session.undo();
    session.undo();
    assert!(session.dof_symbols().is_empty());
}
