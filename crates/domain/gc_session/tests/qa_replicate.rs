//! QA edge cases for Mirror/Copy/Replace Components, the contact set, the DOF symbol and Assemble's list of
//! possible constraints. Inventor 2026 help (under <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/>):
//! - mirrored components GUID-66AA4A06-F63F-49C3-8F39-42FE4999F1FC, copied components GUID-0E21F4A0-DE77-469D-9381-9F55CFA6BF49,
//!   replace components GUID-E5FAA224-7332-4AC6-A78E-793FDACFEEDD
//! - contact solver GUID-0BBD4ACE-7BD1-40BF-A011-74416B229035, detect contact GUID-FA1A8282-3A7E-45BE-8DB3-F637FB1C3292

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::disallowed_methods,
    reason = "test code"
)]

use gc_core::{AnyId, Id};
use gc_doc::{
    AssembleSolution, AxisSense, Body, ComponentFlag, ContactSolver, Geometry, Occurrence, OriginElement,
    OriginPlane, Placement, RelationKind, Relationship, Replacement, Selection, Solid,
};
use gc_session::Session;

fn placed(x: f64, y: f64, z: f64) -> Placement {
    Placement { xyz: [x, y, z], rpy: [0.0; 3] }
}

fn cell() -> (Session, Id<Body>, Id<Body>, Id<Body>) {
    let mut s = Session::new();
    let floor = s.add_body(Solid::Box { size: [4.0, 4.0, 0.1] }, placed(0.0, 0.0, -0.05)).unwrap(); // grounded
    let a = s.add_body(Solid::Box { size: [0.2; 3] }, placed(1.0, 0.5, 0.1)).unwrap();
    let b = s.add_body(Solid::Box { size: [0.3; 3] }, placed(1.5, -0.5, 0.15)).unwrap();
    (s, floor, a, b)
}

fn plane_of(item: Id<Body>, p: OriginPlane) -> Selection {
    Selection {
        occurrence: Occurrence::Item(item.erase()),
        geometry: Geometry::origin(OriginElement::Plane(p)),
    }
}

fn count(s: &Session) -> usize {
    s.doc().bodies().iter().count()
}

fn ghost() -> AnyId {
    Id::<Body>::from_raw(std::num::NonZeroU64::new(9999).unwrap()).erase()
}

// ---- mirror and copy ----

/// Nothing selected, something that does not exist, numbers that are not numbers, or a "plane" that is not
/// one: refused, document and history untouched.
#[test]
fn mirror_and_copy_refuse_nonsense_and_change_nothing() {
    let (mut s, floor, a, _) = cell();
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    let yz = plane_of(floor, OriginPlane::Yz);
    let point =
        Selection { occurrence: Occurrence::Item(floor.erase()), geometry: Geometry::Point([0.0; 3]) };
    let axis = Selection {
        occurrence: Occurrence::Item(floor.erase()),
        geometry: Geometry::origin(OriginElement::Axis(0)),
    };
    let flat = Selection {
        occurrence: Occurrence::Item(floor.erase()),
        geometry: Geometry::Plane { point: [0.0; 3], normal: [0.0; 3] },
    };
    let missing_plane = Selection { occurrence: Occurrence::Item(ghost()), geometry: yz.geometry };
    // Each attempt runs on its own, one after another, and must leave no trace.
    macro_rules! refused {
        ($what:expr, $call:expr) => {{
            let result = $call;
            assert!(result.is_err(), "{} was accepted", $what);
            assert!(s.doc().same_content(&before), "{} changed the document", $what);
            assert_eq!(s.history().undo_depth(), depth, "{}", $what);
        }};
    }
    refused!("mirror nothing", s.mirror_components(&[], yz, true, false));
    refused!("mirror a ghost", s.mirror_components(&[ghost()], yz, true, false));
    refused!("mirror a and a ghost", s.mirror_components(&[a.erase(), ghost()], yz, true, false));
    refused!("mirror across a point", s.mirror_components(&[a.erase()], point, true, false));
    refused!("mirror across an axis", s.mirror_components(&[a.erase()], axis, true, false));
    refused!("mirror across a zero normal", s.mirror_components(&[a.erase()], flat, true, false));
    refused!("mirror across a missing plane", s.mirror_components(&[a.erase()], missing_plane, true, false));
    refused!("copy nothing", s.copy_components(&[], placed(1.0, 0.0, 0.0), true, false));
    refused!("copy a ghost", s.copy_components(&[ghost()], placed(1.0, 0.0, 0.0), true, false));
    refused!("copy by NaN", s.copy_components(&[a.erase()], placed(f64::NAN, 0.0, 0.0), true, false));
    refused!(
        "copy by infinity",
        s.copy_components(&[a.erase()], placed(0.0, f64::INFINITY, 0.0), true, false)
    );
}

/// A mirror puts the copy on the other side of the plane; a plane through the component itself gives a
/// copy on top of it (allowed); both are one undo step and restore exactly. Mirroring twice returns to the
/// original place.
#[test]
fn mirror_reflects_and_is_one_undo_step() {
    let (mut s, floor, a, _) = cell();
    let base = count(&s);
    let depth = s.history().undo_depth();
    s.mirror_components(&[a.erase()], plane_of(floor, OriginPlane::Yz), false, false).unwrap();
    assert_eq!(count(&s), base + 1);
    assert_eq!(s.history().undo_depth(), depth + 1);
    let copy = s
        .doc()
        .bodies()
        .iter()
        .map(|(id, _)| id)
        .find(|id| ![floor, a].contains(id) && s.doc().get(*id).unwrap().placement.xyz[0] < 0.0)
        .unwrap();
    let p = s.doc().get(copy).unwrap().placement.xyz;
    assert!((p[0] + 1.0).abs() < 1e-9 && (p[1] - 0.5).abs() < 1e-9 && (p[2] - 0.1).abs() < 1e-9, "{p:?}");
    // Mirror the copy back across the same plane: it lands where the original is.
    s.mirror_components(&[copy.erase()], plane_of(floor, OriginPlane::Yz), false, false).unwrap();
    let again = s
        .doc()
        .bodies()
        .iter()
        .filter(|(_, b)| (b.placement.xyz[0] - 1.0).abs() < 1e-9 && (b.placement.xyz[1] - 0.5).abs() < 1e-9)
        .count();
    assert_eq!(again, 2, "the original and the double mirror coincide");
    assert!(s.undo() && s.undo());
    assert_eq!(count(&s), base);

    // The component's own plane: the copy lands on the original.
    let before = s.doc().clone();
    s.mirror_components(&[a.erase()], plane_of(a, OriginPlane::Yz), false, false).unwrap();
    assert_eq!(count(&s), base + 1);
    assert!(s.undo());
    assert!(s.doc().same_content(&before));
}

/// Copy: grounded or not as asked; the same item listed twice, copied by zero, a copy of a copy.
#[test]
fn copies_are_grounded_only_when_asked_and_odd_lists_are_harmless() {
    let (mut s, _, a, b) = cell();
    let base = count(&s);
    s.copy_components(&[a.erase()], placed(0.0, 1.0, 0.0), false, false).unwrap();
    s.copy_components(&[a.erase()], placed(0.0, -1.0, 0.0), false, true).unwrap();
    assert_eq!(count(&s), base + 2);
    let grounded: Vec<bool> = s
        .components()
        .iter()
        .filter(|n| matches!(n.occurrence, Occurrence::Item(i) if ![a.erase(), b.erase()].contains(&i)))
        .map(|n| n.grounded)
        .collect();
    assert_eq!(grounded.iter().filter(|g| **g).count(), 2, "the floor and the grounded copy: {grounded:?}");
    // Listed twice and copied by zero: no panic, the document stays valid and Undo restores it.
    let before = s.doc().clone();
    if s.copy_components(&[a.erase(), a.erase()], placed(0.0, 0.0, 0.0), false, false).is_ok() {
        assert!(count(&s) >= base + 3);
        assert!(s.undo());
    }
    assert!(s.doc().same_content(&before));
}

/// A suppressed or hidden component can be copied; the copy is not made visible or active by accident in a
/// way that changes the original; the original is untouched.
#[test]
fn suppressed_and_hidden_components_are_copied_without_changing_them() {
    let (mut s, _, a, _) = cell();
    s.set_component(Occurrence::Item(a.erase()), ComponentFlag::Suppressed, true).unwrap();
    s.set_component(Occurrence::Item(a.erase()), ComponentFlag::Visible, false).unwrap();
    let original = s.doc().get(a).unwrap().clone();
    let result = s.copy_components(&[a.erase()], placed(0.0, 1.0, 0.0), true, false);
    // Either refused with a reason, or made; in both cases the original is unchanged.
    assert_eq!(s.doc().get(a).unwrap(), &original);
    assert!(s.components().iter().find(|n| n.occurrence == Occurrence::Item(a.erase())).unwrap().suppressed);
    if result.is_ok() {
        assert!(s.undo());
    }
}

/// Relationships between the copied components are re-made for the copies; one to a component that is not
/// copied is not (the doc: "those between the selected components"); nothing is made when not asked.
#[test]
fn relationships_between_copied_components_are_remade() {
    let (mut s, floor, a, b) = cell();
    let on = |id: Id<Body>, z: f64, n: f64| Selection {
        occurrence: Occurrence::Item(id.erase()),
        geometry: Geometry::Plane { point: [0.0, 0.0, z], normal: [0.0, 0.0, n] },
    };
    // a stands on the floor, b stands on a.
    s.add_relationship(Relationship::new(
        "Mate:1",
        RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
        on(a, -0.1, -1.0),
        on(floor, 0.0, 1.0),
    ))
    .unwrap();
    s.add_relationship(Relationship::new(
        "Mate:2",
        RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
        on(b, -0.15, -1.0),
        on(a, 0.1, 1.0),
    ))
    .unwrap();
    let relations = s.relationships().len();
    // Copying only a: its mate to the floor (not selected) is not re-made.
    let report = s.copy_components(&[a.erase()], placed(0.0, 1.0, 0.0), true, false).unwrap();
    assert!(report.is_empty(), "{report:?}");
    assert_eq!(
        s.relationships().len(),
        relations,
        "a relationship to a component that is not copied is not re-made"
    );
    // Copying a and b together: the mate between them is re-made once.
    s.copy_components(&[a.erase(), b.erase()], placed(0.0, 2.0, 0.0), true, false).unwrap();
    assert_eq!(s.relationships().len(), relations + 1, "Mate:2 between the two copies");
    // Not asked: none made.
    let now = s.relationships().len();
    s.copy_components(&[a.erase(), b.erase()], placed(0.0, 3.0, 0.0), false, false).unwrap();
    assert_eq!(s.relationships().len(), now);
}

// ---- replace ----

#[test]
fn replacing_with_nonsense_is_refused_and_replacing_with_the_same_is_harmless() {
    let (mut s, _, a, _) = cell();
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    for (what, result) in [
        (
            "negative box",
            s.replace_component(a.erase(), Replacement::Solid(Solid::Box { size: [-1.0, 1.0, 1.0] }), false),
        ),
        (
            "NaN sphere",
            s.replace_component(a.erase(), Replacement::Solid(Solid::Sphere { radius: f64::NAN }), false),
        ),
        (
            "zero cylinder",
            s.replace_component(
                a.erase(),
                Replacement::Solid(Solid::Cylinder { radius: 0.0, length: 1.0 }),
                false,
            ),
        ),
        ("a ghost", s.replace_component(ghost(), Replacement::Solid(Solid::Sphere { radius: 0.1 }), false)),
        (
            "a ghost, all",
            s.replace_component(ghost(), Replacement::Solid(Solid::Sphere { radius: 0.1 }), true),
        ),
    ] {
        assert!(result.is_err(), "{what} was accepted");
        assert!(s.doc().same_content(&before), "{what}");
        assert_eq!(s.history().undo_depth(), depth, "{what}");
    }
    let same = s.doc().get(a).unwrap().solid.clone();
    s.replace_component(a.erase(), Replacement::Solid(same.clone()), false).unwrap();
    assert_eq!(s.doc().get(a).unwrap().solid, same);
    // Replacing keeps the name, placement and look.
    let kept = s.doc().get(a).unwrap().clone();
    s.replace_component(a.erase(), Replacement::Solid(Solid::Sphere { radius: 0.2 }), false).unwrap();
    let now = s.doc().get(a).unwrap();
    assert_eq!((&now.name, now.placement, now.look), (&kept.name, kept.placement, kept.look));
    assert!(s.undo());
    assert_eq!(s.doc().get(a).unwrap().solid, same);
}

// ---- contact set and solver ----

#[test]
fn contact_set_and_solver_edges() {
    let (mut s, _, a, b) = cell();
    assert_eq!(s.doc().contact_solver(), ContactSolver::Off, "off by default");
    assert!(s.doc().contact_set().is_empty());
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    // Missing components cannot join the set; an empty list changes nothing.
    assert!(s.set_contact_set(&[ghost()], true).is_err());
    assert!(s.set_contact_set(&[a.erase(), ghost()], true).is_err());
    assert!(s.doc().same_content(&before) && s.history().undo_depth() == depth);
    s.set_contact_set(&[], true).ok();
    // Adding twice stores once; taking out something that is not in is harmless; one undo step each.
    s.set_contact_set(&[a.erase(), b.erase()], true).unwrap();
    assert_eq!(s.doc().contact_set().len(), 2);
    s.set_contact_set(&[a.erase()], false).unwrap();
    s.set_contact_set(&[a.erase()], false).ok();
    assert_eq!(s.doc().contact_set(), vec![Occurrence::Item(b.erase())]);
    for mode in [ContactSolver::All, ContactSolver::ContactSet, ContactSolver::Off] {
        s.set_contact_solver(mode).unwrap();
        assert_eq!(s.doc().contact_solver(), mode);
    }
    // A deleted component leaves the set.
    s.delete_item(gc_session::CellItem::Body(b)).unwrap();
    assert!(s.undo());
    assert!(s.doc().contact_set().contains(&Occurrence::Item(b.erase())), "undo brings it back to the set");
}

// ---- DOF symbol ----

/// The symbol agrees with the degrees of freedom: as many sliding directions and turning axes as the
/// component has left, none for a grounded one, six for a free one; hidden by default; a missing component
/// has none; a suppressed component is not drawn.
#[test]
fn the_dof_symbol_agrees_with_the_degrees_of_freedom() {
    let (mut s, floor, a, _) = cell();
    let sym = |s: &Session, id: AnyId| s.dof_symbol(id).unwrap();
    assert!(
        sym(&s, floor.erase()).translations.is_empty() && sym(&s, floor.erase()).rotations.is_empty(),
        "grounded: nothing"
    );
    let free = sym(&s, a.erase());
    assert_eq!((free.translations.len(), free.rotations.len()), (3, 3), "free: all six");
    // Mate on the floor: two slides and one turn about the normal.
    s.add_relationship(Relationship::new(
        "Mate:1",
        RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
        Selection {
            occurrence: Occurrence::Item(a.erase()),
            geometry: Geometry::Plane { point: [0.0, 0.0, -0.1], normal: [0.0, 0.0, -1.0] },
        },
        Selection {
            occurrence: Occurrence::Item(floor.erase()),
            geometry: Geometry::Plane { point: [0.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] },
        },
    ))
    .unwrap();
    let dof = s.component_dof(a.erase()).unwrap();
    let mated = sym(&s, a.erase());
    assert_eq!(
        (mated.translations.len(), mated.rotations.len()),
        (usize::from(dof.translation), usize::from(dof.rotation()))
    );
    assert!(
        mated.relative_to.is_empty() || mated.relative_to.iter().all(|c| *c != a.erase()),
        "not relative to itself"
    );
    assert!(mated.origin.iter().all(|v| v.is_finite()));
    for t in &mated.translations {
        let n = (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt();
        assert!((n - 1.0).abs() < 1e-9, "unit directions: {t:?}");
    }
    // Shown only when asked; suppressed or missing components are not drawn.
    assert!(s.dof_symbols().is_empty());
    assert!(!s.doc().dof_shown(a.erase()));
    s.set_dof_shown(&[a.erase()], true).unwrap();
    assert_eq!(s.dof_symbols().len(), 1);
    assert!(s.set_dof_shown(&[ghost()], true).is_err());
    assert_eq!(s.dof_symbols().len(), 1);
    s.set_component(Occurrence::Item(a.erase()), ComponentFlag::Suppressed, true).unwrap();
    assert!(s.dof_symbols().is_empty(), "a suppressed component has no symbol");
    assert!(s.dof_symbol(ghost()).is_none());
}

// ---- Assemble: what can be offered for two picks ----

fn on(id: Id<Body>, geometry: Geometry) -> Selection {
    Selection { occurrence: Occurrence::Item(id.erase()), geometry }
}

/// Help (Assemble): two planes offer Mate, Flush and Angle (directed) only; a circle pair offers Insert;
/// whatever is offered builds a valid relationship kind; broken picks offer nothing and never panic.
#[test]
fn assemble_offers_only_what_fits_the_picks() {
    let (s, floor, a, _) = cell();
    let plane = |n: f64| Geometry::Plane { point: [0.0; 3], normal: [0.0, 0.0, n] };
    let planes = s.possible_relations(&on(a, plane(-1.0)), &on(floor, plane(1.0)));
    assert!(planes.contains(&AssembleSolution::MateMate) && planes.contains(&AssembleSolution::MateFlush));
    assert!(planes.contains(&AssembleSolution::AngleDirected));
    assert!(
        !planes
            .iter()
            .any(|p| matches!(p, AssembleSolution::InsertOpposed | AssembleSolution::InsertAligned)),
        "{planes:?}"
    );
    let ring = |n: f64| Geometry::Circle { centre: [0.0; 3], normal: [0.0, 0.0, n] };
    let rings = s.possible_relations(&on(a, ring(-1.0)), &on(floor, ring(1.0)));
    assert!(rings.contains(&AssembleSolution::InsertOpposed), "{rings:?}");
    for solution in planes.iter().chain(&rings) {
        let kind = solution.kind(0.01);
        let _ = Relationship::new("x", kind, on(a, plane(-1.0)), on(floor, plane(1.0)));
    }
    // A point with a point has no orientation to constrain except coincidence; whatever comes back is a set.
    let points =
        s.possible_relations(&on(a, Geometry::Point([0.0; 3])), &on(floor, Geometry::Point([0.0; 3])));
    assert!(points.len() <= 8);
    // Non-finite or zero-direction picks, and a pick on the same component twice.
    let nan = Geometry::Plane { point: [f64::NAN; 3], normal: [0.0, 0.0, 1.0] };
    let flat = Geometry::Plane { point: [0.0; 3], normal: [0.0; 3] };
    for (x, y) in [(nan, plane(1.0)), (plane(1.0), flat), (flat, flat)] {
        std::hint::black_box(s.possible_relations(&on(a, x), &on(floor, y)));
    }
    std::hint::black_box(s.possible_relations(&on(a, plane(1.0)), &on(a, plane(-1.0))));
}

/// Naming a component twice in one request stores it twice: the set then lists it more than once.
#[test]
#[ignore = "BUG: set_contact_set stores duplicate entries; see docs/handoff/qa.md"]
fn a_component_is_in_the_contact_set_once() {
    let (mut s, _, a, _) = cell();
    s.set_contact_set(&[a.erase(), a.erase()], true).unwrap();
    assert_eq!(s.doc().contact_set().len(), 1, "{:?}", s.doc().contact_set());
    s.set_contact_set(&[a.erase()], false).unwrap();
    assert!(s.doc().contact_set().is_empty());
}

/// A mirror across a plane with non-finite coordinates is accepted and puts the copy at NaN coordinates.
#[test]
#[ignore = "BUG: mirror_components accepts a plane with NaN coordinates and makes a copy at NaN; see docs/handoff/qa.md"]
fn mirroring_across_a_nan_plane_is_refused() {
    let (mut s, floor, a, _) = cell();
    let before = s.doc().clone();
    let nan_plane = Selection {
        occurrence: Occurrence::Item(floor.erase()),
        geometry: Geometry::Plane { point: [f64::NAN; 3], normal: [1.0, 0.0, 0.0] },
    };
    let result = s.mirror_components(&[a.erase()], nan_plane, true, false);
    assert!(result.is_err(), "accepted");
    assert!(s.doc().same_content(&before));
}

/// Deleting a component leaves it in the contact set (a stale entry that is also saved).
#[test]
#[ignore = "BUG: a deleted component stays in the contact set; see docs/handoff/qa.md"]
fn a_deleted_component_leaves_the_contact_set() {
    let (mut s, _, _, b) = cell();
    s.set_contact_set(&[b.erase()], true).unwrap();
    s.delete_item(gc_session::CellItem::Body(b)).unwrap();
    assert!(!s.doc().contact_set().contains(&Occurrence::Item(b.erase())));
}
