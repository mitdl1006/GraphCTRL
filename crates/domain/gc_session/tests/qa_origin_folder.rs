//! QA edge cases for the component origin folder (assembly parity P1-2/P1-3). Lead decisions 2026-11-17:
//! the folder holds `YZ Plane, XZ Plane, XY Plane, X Axis, Y Axis, Z Axis, Center Point` (names are the
//! browser's job); everything is hidden by default; hidden elements are not selectable in the viewport but
//! are in the browser. Inventor 2026 help, To Display Reference Planes / About Reference Planes:
//! <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/> (ids A52, A53 in
//! docs/ux/inventor-assembly-parity.md).

#![allow(clippy::unwrap_used, clippy::panic, clippy::float_cmp, reason = "test code")]

use std::path::Path;

use gc_doc::{CellOp, Command, Occurrence, OriginElement, OriginPlane, Placement, Solid};
use gc_math::DVec3;
use gc_session::{CellItem, Session, library};

const ALL: [OriginElement; 7] = [
    OriginElement::Plane(OriginPlane::Yz),
    OriginElement::Plane(OriginPlane::Xz),
    OriginElement::Plane(OriginPlane::Xy),
    OriginElement::Axis(0),
    OriginElement::Axis(1),
    OriginElement::Axis(2),
    OriginElement::Point,
];

fn placed(x: f64, y: f64, z: f64, yaw: f64) -> Placement {
    Placement { xyz: [x, y, z], rpy: [0.0, 0.0, yaw] }
}

fn robot_session() -> (Session, gc_core::Id<gc_doc::Robot>) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../robots");
    let packages = library::scan(&dir);
    let mut s = Session::new();
    s.set_robot_library(Some(dir));
    let ur5e = packages.iter().find(|p| p.info.name == "UR5e").unwrap();
    let robot = s.add_library_robot(ur5e).unwrap();
    (s, robot)
}

/// Every kind of component has an origin, all seven elements are hidden to begin with, and the origin
/// is exactly where the component is.
#[test]
fn every_component_kind_has_a_hidden_origin_at_its_pose() {
    let (mut s, robot) = robot_session();
    let body = s.add_body(Solid::Box { size: [0.1; 3] }, placed(0.3, 0.2, 0.1, 0.5)).unwrap();
    s.execute(&Command::Cell(CellOp::PlaceRobot { id: robot, placement: placed(1.0, 2.0, 0.0, 1.0) }))
        .unwrap();
    for (occurrence, point, yaw) in [
        (Occurrence::Item(body.erase()), DVec3::new(0.3, 0.2, 0.1), 0.5),
        (Occurrence::Item(robot.erase()), DVec3::new(1.0, 2.0, 0.0), 1.0),
    ] {
        for element in ALL {
            assert!(!s.origin_visible(occurrence, element), "{occurrence:?} {element:?} starts hidden");
        }
        let origin = s.component_origin(occurrence).unwrap();
        assert!((origin.point() - point).length() < 1e-12);
        // The X axis is the component's X turned by its yaw; Z stays up.
        assert!(
            (origin.axis(0) - DVec3::new(gc_math::fm::cos(yaw), gc_math::fm::sin(yaw), 0.0)).length() < 1e-12
        );
        assert!((origin.axis(2) - DVec3::Z).length() < 1e-12);
        // Axes are unit, mutually square, right-handed.
        let (x, y, z) = (origin.axis(0), origin.axis(1), origin.axis(2));
        assert!(
            (x.length() - 1.0).abs() < 1e-12 && x.dot(y).abs() < 1e-12 && (x.cross(y) - z).length() < 1e-12
        );
    }
}

/// The origin planes use the document's convention: XY normal +Z, XZ normal −Y, YZ normal +X.
#[test]
fn origin_plane_normals_follow_the_documents_convention() {
    let mut s = Session::new();
    let body = s.add_body(Solid::Box { size: [0.1; 3] }, placed(0.0, 0.0, 0.0, 0.0)).unwrap();
    let origin = s.component_origin(Occurrence::Item(body.erase())).unwrap();
    let normal = |p| origin.plane(p).rotation * DVec3::Z;
    assert!((normal(OriginPlane::Xy) - DVec3::Z).length() < 1e-12);
    assert!((normal(OriginPlane::Xz) - DVec3::NEG_Y).length() < 1e-12);
    assert!((normal(OriginPlane::Yz) - DVec3::X).length() < 1e-12);
    // And they agree with the document's own origin planes for a component placed at the origin.
    for (name, plane) in [("XY", OriginPlane::Xy), ("XZ", OriginPlane::Xz), ("YZ", OriginPlane::Yz)] {
        let frame = s.doc().origin_plane(name).unwrap();
        let world = s.doc().frame_world_pose(frame).unwrap();
        assert!((world.rotation * DVec3::Z - normal(plane)).length() < 1e-12, "{name}");
    }
}

/// Moving a component moves its origin (and what was shown stays shown); showing and hiding are one
/// undo step each and restore the document exactly.
#[test]
fn the_origin_follows_the_component_and_visibility_is_one_undo_step() {
    let mut s = Session::new();
    let body = s.add_body(Solid::Box { size: [0.1; 3] }, placed(0.0, 0.0, 0.0, 0.0)).unwrap();
    let occ = Occurrence::Item(body.erase());
    s.set_origin_visible(occ, &ALL, true).unwrap();
    assert!(ALL.iter().all(|e| s.origin_visible(occ, *e)), "the whole folder at once");
    let mut moved = s.doc().get(body).unwrap().clone();
    moved.placement = placed(2.0, 0.0, 0.0, std::f64::consts::FRAC_PI_2);
    s.execute(&Command::Cell(CellOp::SetBody { id: body, body: moved })).unwrap();
    let origin = s.component_origin(occ).unwrap();
    assert!((origin.point() - DVec3::new(2.0, 0.0, 0.0)).length() < 1e-12);
    assert!((origin.axis(0) - DVec3::Y).length() < 1e-12);
    assert!(ALL.iter().all(|e| s.origin_visible(occ, *e)), "still shown after the move");

    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    s.set_origin_visible(occ, &[OriginElement::Axis(1)], false).unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1);
    assert!(!s.origin_visible(occ, OriginElement::Axis(1)) && s.origin_visible(occ, OriginElement::Axis(0)));
    assert!(s.undo());
    assert!(s.doc().same_content(&before));
}

/// Odd requests: an empty list, the same element twice, an element already in that state, an axis that does
/// not exist, a component that does not exist. Refused or harmless; a refusal leaves everything untouched.
#[test]
fn odd_origin_requests_are_harmless() {
    let mut s = Session::new();
    let body = s.add_body(Solid::Box { size: [0.1; 3] }, placed(0.0, 0.0, 0.0, 0.0)).unwrap();
    let occ = Occurrence::Item(body.erase());
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    for bad in [OriginElement::Axis(3), OriginElement::Axis(255)] {
        assert!(s.set_origin_visible(occ, &[bad], true).is_err(), "{bad:?}");
    }
    // One bad element in a list spoils the whole list: nothing is half shown.
    assert!(s.set_origin_visible(occ, &[OriginElement::Point, OriginElement::Axis(9)], true).is_err());
    let ghost = Occurrence::Item(
        gc_core::Id::<gc_doc::Body>::from_raw(std::num::NonZeroU64::new(9999).unwrap()).erase(),
    );
    assert!(s.set_origin_visible(ghost, &[OriginElement::Point], true).is_err());
    assert!(s.component_origin(ghost).is_none());
    assert!(!s.origin_visible(ghost, OriginElement::Point));
    assert!(s.doc().same_content(&before));
    assert_eq!(s.history().undo_depth(), depth);

    // Duplicates in one request, and showing again: hiding once hides it, however often it was shown.
    s.set_origin_visible(occ, &[OriginElement::Point, OriginElement::Point], true).unwrap();
    s.set_origin_visible(occ, &[OriginElement::Point], true).unwrap();
    s.set_origin_visible(occ, &[OriginElement::Point], false).unwrap();
    assert!(!s.origin_visible(occ, OriginElement::Point));
    // Undoing everything gets back to the start, however many no-op steps were recorded.
    while s.history().undo_depth() > depth {
        assert!(s.undo());
    }
    assert!(s.doc().same_content(&before));
    // An empty list is not an error and shows nothing.
    drop(s.set_origin_visible(occ, &[], true));
    assert!(ALL.iter().all(|e| !s.origin_visible(occ, *e)));
}

/// Deleting a component takes its origin with it; undoing the deletion brings back the origin and what was
/// shown; saved and reopened, the shown set is the same.
#[test]
fn deleting_and_reopening_keeps_the_shown_set_consistent() {
    let mut s = Session::new();
    let keep = s.add_body(Solid::Box { size: [0.1; 3] }, placed(0.0, 0.0, 0.0, 0.0)).unwrap();
    let gone = s.add_body(Solid::Box { size: [0.1; 3] }, placed(0.5, 0.0, 0.0, 0.0)).unwrap();
    let (keep_occ, gone_occ) = (Occurrence::Item(keep.erase()), Occurrence::Item(gone.erase()));
    s.set_origin_visible(keep_occ, &[OriginElement::Point], true).unwrap();
    s.set_origin_visible(gone_occ, &ALL, true).unwrap();
    let full = s.doc().clone();
    s.delete_item(CellItem::Body(gone)).unwrap();
    assert!(s.component_origin(gone_occ).is_none());
    assert!(s.origin_visible(keep_occ, OriginElement::Point));
    assert!(s.undo());
    assert!(s.doc().same_content(&full));
    assert!(ALL.iter().all(|e| s.origin_visible(gone_occ, *e)));

    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("origins.gctrl");
    s.save_as(&file).unwrap();
    let mut reopened = Session::new();
    reopened.open(&file).unwrap();
    assert!(ALL.iter().all(|e| reopened.origin_visible(gone_occ, *e)));
    assert!(reopened.origin_visible(keep_occ, OriginElement::Point));
    assert!(!reopened.origin_visible(keep_occ, OriginElement::Axis(0)));
    assert!(reopened.doc().same_content(s.doc()));
}

/// Showing an element that is already shown (or naming it twice) records it twice: the document differs
/// from the one-time state and the list of shown elements grows with every repeat.
#[test]
fn showing_an_element_twice_stores_it_once() {
    let mut s = Session::new();
    let body = s.add_body(Solid::Box { size: [0.1; 3] }, placed(0.0, 0.0, 0.0, 0.0)).unwrap();
    let occ = Occurrence::Item(body.erase());
    s.set_origin_visible(occ, &[OriginElement::Point], true).unwrap();
    let once = s.doc().clone();
    s.set_origin_visible(occ, &[OriginElement::Point], true).unwrap();
    assert!(s.doc().same_content(&once), "the same state again leaves the document as it was");
    let mut t = Session::new();
    let body = t.add_body(Solid::Box { size: [0.1; 3] }, placed(0.0, 0.0, 0.0, 0.0)).unwrap();
    let occ = Occurrence::Item(body.erase());
    t.set_origin_visible(occ, &[OriginElement::Point, OriginElement::Point], true).unwrap();
    let single = {
        let mut u = Session::new();
        let b = u.add_body(Solid::Box { size: [0.1; 3] }, placed(0.0, 0.0, 0.0, 0.0)).unwrap();
        u.set_origin_visible(Occurrence::Item(b.erase()), &[OriginElement::Point], true).unwrap();
        u
    };
    assert!(t.doc().same_content(single.doc()));
}

/// After a component is deleted, nothing about it is "shown" any more (an entry for it must not linger
/// in the list of shown origin elements, where it would also be saved).
#[test]
fn a_deleted_components_origin_is_not_shown() {
    let mut s = Session::new();
    let body = s.add_body(Solid::Box { size: [0.1; 3] }, placed(0.0, 0.0, 0.0, 0.0)).unwrap();
    let occ = Occurrence::Item(body.erase());
    s.set_origin_visible(occ, &ALL, true).unwrap();
    s.delete_item(CellItem::Body(body)).unwrap();
    assert!(ALL.iter().all(|e| !s.origin_visible(occ, *e)));
}
