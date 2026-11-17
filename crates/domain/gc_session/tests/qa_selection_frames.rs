//! QA edge cases for: component selection and the Ground toggle (lead e892df8), the representations folder
//! (browser API), Measure's Maximum Distance (386dca2), and frames on component planes (7a591e0). Inventor 2026
//! help (under <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/>): Ground and Root
//! GUID-98FA88B2-AA32-4A27-946A-DCECB36A6696; Measure GUID-1805AB8F-F782-4B24-A332-B4F2E10A3D00 (Minimum,
//! Maximum and Center-to-center distance).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::disallowed_methods,
    reason = "test code"
)]

use std::path::Path;

use gc_core::Id;
use gc_doc::{Body, Occurrence, OriginPlane, Placement, Robot, Solid};
use gc_math::DVec3;
use gc_session::{CellItem, Session, library};

fn placed(x: f64, y: f64, z: f64) -> Placement {
    Placement { xyz: [x, y, z], rpy: [0.0; 3] }
}

fn two_bodies() -> (Session, Id<Body>, Id<Body>) {
    let mut s = Session::new();
    let a = s.add_body(Solid::Box { size: [0.2; 3] }, placed(0.0, 0.0, 0.0)).unwrap();
    let b = s.add_body(Solid::Box { size: [0.2; 3] }, placed(1.0, 0.0, 0.0)).unwrap();
    (s, a, b)
}

fn robot_session() -> (Session, Id<Robot>) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../robots");
    let packages = library::scan(&dir);
    let mut s = Session::new();
    s.set_robot_library(Some(dir));
    let robot = s.add_library_robot(packages.iter().find(|p| p.info.name == "UR5e").unwrap()).unwrap();
    (s, robot)
}

fn grounded(s: &Session, occurrence: Occurrence) -> bool {
    s.components().iter().find(|n| n.occurrence == occurrence).unwrap().grounded
}

fn ghost_body() -> Id<Body> {
    Id::<Body>::from_raw(std::num::NonZeroU64::new(9999).unwrap())
}

// ---- selection and Ground ----

#[test]
fn selecting_occurrences_follows_what_exists() {
    let (mut s, a, b) = two_bodies();
    let (oa, ob) = (Occurrence::Item(a.erase()), Occurrence::Item(b.erase()));
    assert_eq!(s.selected_occurrence(), None);
    assert!(s.select_occurrence(Some(oa)));
    assert_eq!(s.selected_occurrence(), Some(oa));
    assert_eq!(s.selected_item(), Some(CellItem::Body(a)), "the cell selection follows");
    // A ghost is refused and nothing changes.
    assert!(!s.select_occurrence(Some(Occurrence::Item(ghost_body().erase()))));
    assert_eq!(s.selected_occurrence(), Some(oa));
    assert!(!s.select_occurrence(Some(Occurrence::Part {
        assembly: Id::from_raw(std::num::NonZeroU64::new(9999).unwrap()),
        index: 0
    })));
    assert!(!s.select_occurrence(Some(Occurrence::Link {
        robot: Id::from_raw(std::num::NonZeroU64::new(9999).unwrap()),
        index: 0
    })));
    assert!(!s.select_occurrence(Some(Occurrence::Tool {
        robot: Id::from_raw(std::num::NonZeroU64::new(9999).unwrap())
    })));
    assert_eq!(s.selected_occurrence(), Some(oa));
    // Clearing; selecting another; deleting the selected one makes the selection go away.
    assert!(s.select_occurrence(None));
    assert_eq!(s.selected_occurrence(), None);
    assert!(s.select_occurrence(Some(ob)));
    s.delete_item(CellItem::Body(b)).unwrap();
    assert_eq!(s.selected_occurrence(), None, "a deleted component is not selected any more");
    assert!(s.undo());
    // Selecting a robot's link or tool selects the robot (whose panels apply).
    let (mut r, robot) = robot_session();
    assert!(r.select_occurrence(Some(Occurrence::Link { robot, index: 1 })));
    assert_eq!(r.selected_item(), Some(CellItem::Robot(robot)));
    assert!(
        !r.select_occurrence(Some(Occurrence::Link { robot, index: 9999 })),
        "a link that does not exist"
    );
}

/// Only whole components can be grounded; the toggle flips, is one undo step each time, and is refused
/// (document untouched) with nothing or something ungroundable selected.
#[test]
fn the_ground_toggle_flips_and_refuses_what_cannot_be_grounded() {
    let (mut s, a, b) = two_bodies();
    let ob = Occurrence::Item(b.erase());
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    assert!(s.toggle_ground_selected().is_err(), "nothing selected");
    assert!(s.doc().same_content(&before) && s.history().undo_depth() == depth);
    assert!(s.can_ground(ob));
    assert!(!s.can_ground(Occurrence::Item(ghost_body().erase())));
    assert!(!s.can_ground(Occurrence::Link {
        robot: Id::from_raw(std::num::NonZeroU64::new(9999).unwrap()),
        index: 0
    }));
    // The first component starts grounded; b does not. Toggling b grounds it, again frees it.
    assert!(grounded(&s, Occurrence::Item(a.erase())) && !grounded(&s, ob));
    s.select_occurrence(Some(ob));
    assert!(s.toggle_ground_selected().unwrap());
    assert!(grounded(&s, ob));
    assert_eq!(s.history().undo_depth(), depth + 1);
    assert!(!s.toggle_ground_selected().unwrap());
    assert!(!grounded(&s, ob));
    assert!(s.undo() && s.undo());
    assert!(s.doc().same_content(&before));
    // A selection whose component is then deleted is refused.
    s.select_occurrence(Some(ob));
    s.delete_item(CellItem::Body(b)).unwrap();
    assert!(s.toggle_ground_selected().is_err());
    // A robot's link or tool cannot be grounded.
    let (mut r, robot) = robot_session();
    let link = Occurrence::Link { robot, index: 1 };
    r.select_occurrence(Some(link));
    assert!(!r.can_ground(link));
    let robot_before = r.doc().clone();
    assert!(r.toggle_ground_selected().is_err());
    assert!(r.doc().same_content(&robot_before));
}

// ---- representations folder ----

#[test]
fn the_representation_folder_lists_primary_first_and_marks_the_active_ones() {
    let (mut s, _, b) = two_bodies();
    let folder = s.representation_folder();
    assert!(folder.views.is_empty());
    assert_eq!(folder.positions.len(), 1);
    assert_eq!((folder.positions[0].name.as_str(), folder.positions[0].active), ("Primary", true));
    // Names that cannot be used.
    let before = s.doc().clone();
    for bad in ["", "  ", "Primary"] {
        assert!(s.add_positional_representation(bad).is_err(), "{bad:?}");
        assert!(s.doc().same_content(&before));
    }
    s.add_positional_representation("Open").unwrap();
    s.add_positional_representation("Closed").unwrap();
    s.add_view_representation("Top", Some("cam".into())).unwrap();
    assert!(s.add_positional_representation("Open").is_err(), "a name that is taken");
    let folder = s.representation_folder();
    assert_eq!(
        folder.positions.iter().map(|n| n.name.as_str()).collect::<Vec<_>>(),
        ["Primary", "Open", "Closed"]
    );
    assert_eq!(folder.views.iter().map(|n| n.name.as_str()).collect::<Vec<_>>(), ["Top"]);
    // Exactly one position is active at a time.
    s.activate_positional_representation(Some(1)).unwrap();
    let active: Vec<bool> = s.representation_folder().positions.iter().map(|n| n.active).collect();
    assert_eq!(
        active,
        [false, false, true],
        "index 1 of the stored positions is the second one, after Primary"
    );
    s.activate_positional_representation(Some(0)).unwrap();
    let active: Vec<bool> = s.representation_folder().positions.iter().map(|n| n.active).collect();
    assert_eq!(active, [false, true, false]);
    assert!(s.activate_positional_representation(Some(9)).is_err());
    s.activate_positional_representation(None).unwrap();
    assert!(s.representation_folder().positions[0].active);
    // Activating a view marks it; an out-of-range one is refused and leaves the mark.
    s.activate_view_representation(0).unwrap();
    assert!(s.representation_folder().views[0].active);
    assert!(s.activate_view_representation(3).is_err());
    assert!(s.representation_folder().views[0].active);
    // Delete the active position: Primary is active again and the list shrinks.
    s.activate_positional_representation(Some(1)).unwrap();
    s.delete_representation(None, Some(1)).unwrap();
    let folder = s.representation_folder();
    assert_eq!(
        folder.positions.iter().map(|n| (n.name.as_str(), n.active)).collect::<Vec<_>>(),
        [("Primary", true), ("Open", false)]
    );
    // Deleting Primary or nothing is refused or a no-op; deleting the view while it is active clears the mark.
    let state = s.doc().clone();
    assert!(s.delete_representation(None, Some(7)).is_err());
    assert!(s.doc().same_content(&state));
    s.delete_representation(Some(0), None).unwrap();
    assert!(s.representation_folder().views.is_empty());
    let _ = b;
}

// ---- Measure: maximum distance ----

#[test]
fn the_maximum_distance_is_at_least_the_minimum_and_matches_box_corners() {
    let (mut s, a, b) = two_bodies();
    let pair = s.measure_pair(CellItem::Body(a), CellItem::Body(b)).unwrap();
    let (min, max) = (pair.minimum.unwrap(), pair.maximum.unwrap());
    assert!((min.distance - 0.8).abs() < 1e-9, "faces 0.8 m apart: {}", min.distance);
    // The far corners: x from -0.1 to 1.1, y and z each across 0.2.
    let expected = (1.2_f64.powi(2) + 0.2_f64.powi(2) + 0.2_f64.powi(2)).sqrt();
    assert!((max.distance - expected).abs() < 1e-9, "{} vs {expected}", max.distance);
    assert!(max.distance >= min.distance);
    // The points reported are the distance apart, and swapping the pair gives the same number.
    let apart = (DVec3::from_array(max.on_first) - DVec3::from_array(max.on_second)).length();
    assert!((apart - max.distance).abs() < 1e-9);
    let swapped = s.measure_pair(CellItem::Body(b), CellItem::Body(a)).unwrap().maximum.unwrap();
    assert!((swapped.distance - max.distance).abs() < 1e-9);
    // An item against itself: the maximum is the diagonal, the minimum 0.
    let own = s.measure_pair(CellItem::Body(a), CellItem::Body(a)).unwrap();
    assert!(own.minimum.unwrap().distance.abs() < 1e-12);
    assert!((own.maximum.unwrap().distance - (3.0_f64).sqrt() * 0.2).abs() < 1e-9, "{:?}", own.maximum);
    // Rotated or overlapping boxes never make the maximum smaller than the minimum or non-finite.
    let mut tilted = s.doc().get(b).unwrap().clone();
    tilted.placement = Placement { xyz: [0.05, 0.0, 0.0], rpy: [0.3, 0.4, 0.5] };
    s.execute(&gc_doc::Command::Cell(gc_doc::CellOp::SetBody { id: b, body: tilted })).unwrap();
    let overlap = s.measure_pair(CellItem::Body(a), CellItem::Body(b)).unwrap();
    assert!(
        overlap.maximum.unwrap().distance.is_finite()
            && overlap.maximum.unwrap().distance >= overlap.minimum.unwrap().distance
    );
    // A deleted component measures nothing.
    s.delete_item(CellItem::Body(b)).unwrap();
    assert!(s.measure_pair(CellItem::Body(a), CellItem::Body(b)).is_none());
}

/// A robot against a body: finite, and at least the distance between the nearest parts.
#[test]
fn the_maximum_distance_to_a_robot_is_finite_and_at_least_the_minimum() {
    let (mut s, robot) = robot_session();
    let pillar = s.add_body(Solid::Box { size: [0.1; 3] }, placed(0.8, 0.2, 0.3)).unwrap();
    let pair = s.measure_pair(CellItem::Robot(robot), CellItem::Body(pillar)).unwrap();
    let (min, max) = (pair.minimum.unwrap().distance, pair.maximum.unwrap().distance);
    assert!(max.is_finite() && max >= min && max > 0.5, "{min} {max}");
}

// ---- frames on component planes ----

#[test]
fn a_component_frame_follows_the_component_and_refuses_what_is_not_there() {
    let (mut s, a, _) = two_bodies();
    let oa = Occurrence::Item(a.erase());
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    let ghost = Occurrence::Item(ghost_body().erase());
    assert!(s.add_component_frame(ghost, OriginPlane::Xy).is_err());
    assert!(s.doc().same_content(&before) && s.history().undo_depth() == depth);
    let frame = s.add_component_frame(oa, OriginPlane::Xy).unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1, "one undo step for the frame and its sketch");
    let at = |s: &Session| s.work_frame_pose(frame).unwrap().translation;
    assert!((at(&s) - DVec3::new(0.0, 0.0, 0.0)).length() < 1e-9);
    // The component moves and turns: the frame goes with it.
    let mut moved = s.doc().get(a).unwrap().clone();
    moved.placement = Placement { xyz: [0.3, -0.2, 0.7], rpy: [0.0, 0.0, 1.0] };
    s.execute(&gc_doc::Command::Cell(gc_doc::CellOp::SetBody { id: a, body: moved })).unwrap();
    assert!((at(&s) - DVec3::new(0.3, -0.2, 0.7)).length() < 1e-9, "{:?}", at(&s));
    // Names stay unique when made again and on other planes.
    let second = s.add_component_frame(oa, OriginPlane::Xz).unwrap();
    let third = s.add_component_frame(oa, OriginPlane::Yz).unwrap();
    let names: Vec<String> =
        [frame, second, third].iter().map(|f| s.doc().get(*f).unwrap().name.clone()).collect();
    let mut unique = names.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 3, "{names:?}");
    // The component cannot be deleted while a frame stands on it (or the frame goes with it): either way
    // nothing dangles.
    let result = s.delete_item(CellItem::Body(a));
    if result.is_ok() {
        assert!(
            s.doc().work_frames().iter().all(|(id, _)| s.work_frame_pose(id).is_some()),
            "a frame lost its component"
        );
        assert!(s.undo());
    }
    // Undo walks back one frame at a time.
    // Frame, move, frame, frame: four steps back to the start.
    assert!(s.undo() && s.undo() && s.undo() && s.undo());
    assert!(s.doc().work_frames().iter().count() == before.work_frames().iter().count());
}

// ---- build time of constrained cells (report only; never asserted: wall-clock) ----

/// Prints how long it takes to build a chain of N bodies each mated to the previous one (docs/KNOWN_ISSUES.md:
/// "adding relationships slows down steeply"). Run on demand:
/// `cargo test -p gc_session --test qa_selection_frames -- --ignored --nocapture relationship_build_times`
#[test]
#[ignore = "timing report, run on demand"]
#[allow(clippy::print_stdout, reason = "a report")]
fn relationship_build_times() {
    use gc_doc::{AxisSense, Geometry, RelationKind, Relationship, Selection};
    for n in [5usize, 10, 15, 20, 25, 30] {
        let mut s = Session::new();
        let mut previous = s.add_body(Solid::Box { size: [0.2; 3] }, placed(0.0, 0.0, 0.1)).unwrap();
        let started = std::time::Instant::now();
        for k in 1..n {
            let next =
                s.add_body(Solid::Box { size: [0.2; 3] }, placed(0.0, 0.0, 0.1 + 0.4 * k as f64)).unwrap();
            let on = |b: Id<Body>, z: f64, nz: f64| Selection {
                occurrence: Occurrence::Item(b.erase()),
                geometry: Geometry::Plane { point: [0.0, 0.0, z], normal: [0.0, 0.0, nz] },
            };
            s.add_relationship(Relationship::new(
                "Mate",
                RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
                on(next, -0.1, -1.0),
                on(previous, 0.1, 1.0),
            ))
            .unwrap();
            previous = next;
        }
        println!("{n} bodies in a mated chain: {:.1} ms", started.elapsed().as_secs_f64() * 1e3);
    }
}
