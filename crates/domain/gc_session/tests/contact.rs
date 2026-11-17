//! The contact solver (Inventor 2026 help: About the Contact Solver,
//! GUID-0BBD4ACE-7BD1-40BF-A011-74416B229035; Detect contact between assembly components,
//! GUID-FA1A8282-3A7E-45BE-8DB3-F637FB1C3292): components in the contact set push the ones that can move
//! while a relationship is driven or a component dragged; when the one pushed cannot move, the motion stops.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use gc_core::Id;
use gc_doc::{
    AxisSense, Body, ContactSolver, Geometry, Occurrence, OriginElement, OriginPlane, Placement,
    RelationKind, Relationship, Selection, Solid,
};
use gc_session::Session;

struct Line {
    session: Session,
    floor: Id<Body>,
    pusher: Id<Body>,
    slide: Id<Relationship>,
}

fn on(id: Id<Body>, geometry: Geometry) -> Selection {
    Selection { occurrence: Occurrence::Item(id.erase()), geometry }
}

fn at(x: f64, z: f64) -> Placement {
    Placement { xyz: [x, 0.0, z], rpy: [0.0; 3] }
}

/// A grounded floor; a pusher (0.2 m cube) sitting on it, slid along X by a Flush whose offset is its X.
fn line() -> Line {
    let mut session = Session::new();
    let floor = session.add_body(Solid::Box { size: [6.0, 2.0, 0.1] }, at(0.0, -0.05)).unwrap();
    let pusher = session.add_body(Solid::Box { size: [0.2; 3] }, at(0.0, 0.1)).unwrap();
    let plane = |p: OriginPlane| Geometry::origin(OriginElement::Plane(p));
    session
        .add_relationship(Relationship::new(
            "Mate:1",
            RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
            on(pusher, Geometry::Plane { point: [0.0, 0.0, -0.1], normal: [0.0, 0.0, -1.0] }),
            on(floor, Geometry::Plane { point: [0.0, 0.0, 0.05], normal: [0.0, 0.0, 1.0] }),
        ))
        .unwrap();
    session
        .add_relationship(Relationship::new(
            "Flush:1",
            RelationKind::Flush { offset: 0.0 },
            on(pusher, plane(OriginPlane::Xz)),
            on(floor, plane(OriginPlane::Xz)),
        ))
        .unwrap();
    let slide = session
        .add_relationship(Relationship::new(
            "Flush:2",
            RelationKind::Flush { offset: 0.0 },
            on(pusher, plane(OriginPlane::Yz)),
            on(floor, plane(OriginPlane::Yz)),
        ))
        .unwrap();
    Line { session, floor, pusher, slide }
}

fn x_of(session: &Session, body: Id<Body>) -> f64 {
    session.shown_body(body).unwrap().placement.xyz[0]
}

#[test]
fn a_driven_pusher_pushes_a_free_box_of_the_contact_set() {
    let Line { mut session, floor, pusher, slide } = line();
    let parcel = session.add_body(Solid::Box { size: [0.2; 3] }, at(0.5, 0.1)).unwrap();
    session.set_contact_set(&[floor.erase(), pusher.erase(), parcel.erase()], true).unwrap();
    session.set_contact_solver(ContactSolver::ContactSet).unwrap();
    let before = session.doc().clone();
    // In two frames, as the Drive dialog plays it.
    session.drive_relationship(slide, 0.5).unwrap();
    session.drive_relationship(slide, 1.0).unwrap();
    assert!(session.contact_stop().is_none());
    assert!((x_of(&session, pusher) - 1.0).abs() < 1e-6);
    // The parcel was pushed ahead: its face against the pusher's.
    let parcel_x = x_of(&session, parcel);
    assert!((parcel_x - 1.2).abs() < 1e-3, "{parcel_x}");
    assert!((session.shown_body(parcel).unwrap().placement.xyz[2] - 0.1).abs() < 1e-6, "still on the floor");
    assert_eq!(session.contact_pushed().len(), 1);
    assert!(session.doc().same_content(&before), "drive changes nothing");
    session.end_drive_relationship();
    assert!((x_of(&session, parcel) - 0.5).abs() < 1e-12);
}

#[test]
fn the_motion_stops_at_a_component_that_cannot_give_way() {
    let Line { mut session, floor, pusher, slide } = line();
    let wall = session.add_body(Solid::Box { size: [0.1, 2.0, 1.0] }, at(0.75, 0.5)).unwrap();
    session.set_component(Occurrence::Item(wall.erase()), gc_doc::ComponentFlag::Grounded, true).unwrap();
    session.set_contact_set(&[floor.erase(), pusher.erase(), wall.erase()], true).unwrap();
    session.set_contact_solver(ContactSolver::ContactSet).unwrap();
    session.drive_relationship(slide, 1.0).unwrap();
    let stop = session.contact_stop().unwrap();
    assert_eq!((stop.first, stop.second), (pusher.erase(), wall.erase()));
    let reached = stop.value.unwrap();
    assert!(reached <= 0.6 && reached > 0.59, "{reached}");
    assert!((x_of(&session, pusher) - reached).abs() < 1e-9);

    // Outside the contact set, or with the solver off, the pusher goes through.
    session.end_drive_relationship();
    session.set_contact_set(&[wall.erase()], false).unwrap();
    session.drive_relationship(slide, 1.0).unwrap();
    assert!(session.contact_stop().is_none());
    assert!((x_of(&session, pusher) - 1.0).abs() < 1e-6);
    session.end_drive_relationship();
    session.set_contact_set(&[wall.erase()], true).unwrap();
    session.set_contact_solver(ContactSolver::Off).unwrap();
    session.drive_relationship(slide, 1.0).unwrap();
    assert!((x_of(&session, pusher) - 1.0).abs() < 1e-6);
}

#[test]
fn a_dragged_body_pushes_and_the_drag_is_one_undo_step() {
    let mut session = Session::new();
    let floor = session.add_body(Solid::Box { size: [6.0, 2.0, 0.1] }, at(0.0, -0.05)).unwrap();
    let block = session.add_body(Solid::Box { size: [0.2; 3] }, at(0.0, 0.1)).unwrap();
    let parcel = session.add_body(Solid::Box { size: [0.2; 3] }, at(0.4, 0.1)).unwrap();
    session.set_contact_solver(ContactSolver::All).unwrap();
    let _ = floor;
    let depth = session.history().undo_depth();
    let mut dragged = session.doc().get(block).unwrap().clone();
    for x in [0.1, 0.2, 0.3] {
        dragged.placement = at(x, 0.1);
        session.preview_body(block, dragged.clone());
    }
    assert!((x_of(&session, parcel) - 0.5).abs() < 1e-3, "{}", x_of(&session, parcel));
    session.end_body_preview();
    assert_eq!(session.history().undo_depth(), depth + 1);
    assert!((session.doc().get(parcel).unwrap().placement.xyz[0] - 0.5).abs() < 1e-3);
    assert!((session.doc().get(block).unwrap().placement.xyz[0] - 0.3).abs() < 1e-12);
    session.undo();
    assert!((session.doc().get(parcel).unwrap().placement.xyz[0] - 0.4).abs() < 1e-12);
}

/// An imported assembly of one 0.2 m cube part, 0.1 m up in its own frame, standing at `x`.
fn crate_assembly(session: &mut Session, x: f64) -> gc_core::Id<gc_doc::Assembly> {
    let assembly = gc_doc::Assembly {
        name: "Crate".into(),
        file: "crate.glb".into(),
        placement: at(x, 0.0),
        parts: vec![gc_doc::AssemblyPart {
            name: "Box".into(),
            parent: None,
            matrix: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.1, 1.0]],
            meshes: vec![0],
        }],
        meshes: vec![gc_doc::AssemblyMesh { name: "Box".into(), color: [1.0; 4], triangles: 12 }],
        look: Default::default(),
    };
    session.add_assembly(assembly, vec![gc_mesh::primitives::cuboid([0.2; 3]).unwrap()]).unwrap()
}

#[test]
fn assembly_parts_take_part_pushed_when_free_and_stopping_the_drive_when_grounded() {
    let Line { mut session, floor, pusher, slide } = line();
    let crate_ = crate_assembly(&mut session, 0.5);
    session.set_contact_set(&[floor.erase(), pusher.erase(), crate_.erase()], true).unwrap();
    session.set_contact_solver(ContactSolver::ContactSet).unwrap();
    session.drive_relationship(slide, 0.6).unwrap();
    assert!(session.contact_stop().is_none());
    let pushed = session.driven_placement(crate_.erase()).unwrap();
    assert!((pushed.xyz[0] - 0.8).abs() < 1e-3, "{pushed:?}");
    // Its parts are drawn where it was pushed to.
    let part = &session.assembly_parts(crate_).unwrap()[0];
    assert!((part.matrix[3][0] - 0.8).abs() < 1e-3);
    session.end_drive_relationship();

    session.set_component(Occurrence::Item(crate_.erase()), gc_doc::ComponentFlag::Grounded, true).unwrap();
    session.drive_relationship(slide, 0.6).unwrap();
    let stop = session.contact_stop().unwrap();
    assert_eq!((stop.first, stop.second), (pusher.erase(), crate_.erase()));
    assert!(stop.value.is_some_and(|v| (0.29..=0.3).contains(&v)), "{stop:?}");
}

#[test]
fn collision_detection_reports_new_overlap_not_touching_and_needs_no_contact_set() {
    let Line { mut session, floor, pusher, slide } = line();
    let wall = session.add_body(Solid::Box { size: [0.1, 2.0, 1.0] }, at(0.75, 0.5)).unwrap();
    // Resting on the floor (touching) is no collision.
    session.drive_relationship(slide, 0.3).unwrap();
    assert_eq!(session.drive_collision(), None);
    // Into the wall: a collision, with the solver off and nothing in a contact set.
    session.drive_relationship(slide, 0.65).unwrap();
    assert_eq!(session.drive_collision(), Some((pusher.erase(), wall.erase())));
    assert_eq!(session.component_name(wall.erase()).as_deref(), Some("Body3"));
    // A hidden wall is left out of the analysis.
    session.end_drive_relationship();
    session.set_component(Occurrence::Item(wall.erase()), gc_doc::ComponentFlag::Visible, false).unwrap();
    session.drive_relationship(slide, 0.65).unwrap();
    assert_eq!(session.drive_collision(), None);
    let _ = floor;
}
