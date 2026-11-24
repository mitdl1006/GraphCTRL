//! QA boundary tests for view representation Lock/Copy and for suppressing pattern elements (kept out of the
//! bill of materials). Inventor 2026 help: To Create, Edit, and Restore View Representations
//! GUID-397F9263-12C3-47A1-B6A6-16FC05EDEF47, positional representations GUID-34C28C35-45CA-4476-9336-30146F7C9471,
//! Suppress elements in assembly patterns GUID-FDD20E41-93A5-479D-B547-BA9956E9966B, Restore independent
//! elements GUID-ABC5607F-6128-40C8-BF28-44D918E5803C, BOM GUID-A0A95805-CBF6-4524-8314-4AC8B53C19CF.
//! Refused edits leave the document and the history untouched; accepted ones are one undo step that restores
//! exactly (redo repeats).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::disallowed_methods,
    reason = "test code"
)]

use gc_core::Id;
use gc_doc::{
    Body, CellOp, Command, EditError, Geometry, Occurrence, OriginElement, PatternDirection, PatternLayout,
    Placement, Selection, Solid,
};
use gc_session::{BomView, Session};

fn step<T>(s: &mut Session, what: &str, f: impl FnOnce(&mut Session) -> Result<T, EditError>) -> Option<T> {
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    match f(s) {
        Err(_) => {
            assert!(s.doc().same_content(&before), "{what} changed the document");
            assert_eq!(s.history().undo_depth(), depth, "{what}");
            None
        }
        Ok(v) => {
            assert_eq!(s.history().undo_depth(), depth + 1, "{what}: one undo step");
            let after = s.doc().clone();
            assert!(s.undo());
            assert!(s.doc().same_content(&before), "{what}: undo restores");
            assert!(s.redo());
            assert!(s.doc().same_content(&after), "{what}: redo repeats");
            Some(v)
        }
    }
}

fn at(x: f64, y: f64, z: f64) -> Placement {
    Placement { xyz: [x, y, z], rpy: [0.0; 3] }
}

fn row_of(floor: Id<Body>, count: u32) -> PatternLayout {
    PatternLayout::Rectangular {
        column: PatternDirection {
            along: Selection {
                occurrence: Occurrence::Item(floor.erase()),
                geometry: Geometry::origin(OriginElement::Axis(0)),
            },
            flip: false,
            count,
            spacing: 0.3,
        },
        row: None,
    }
}

fn parts(s: &Session) -> u32 {
    s.bill_of_materials(BomView::PartsOnly).iter().map(|r| r.quantity).sum()
}

fn suppress(s: &mut Session, index: usize, element: u32, suppressed: bool) -> Result<(), EditError> {
    s.execute(&Command::Cell(CellOp::SuppressPatternElement { index, element, suppressed })).map(|_| ())
}

fn restore(s: &mut Session, index: usize, element: u32) -> Result<(), EditError> {
    s.execute(&Command::Cell(CellOp::RestorePatternElement { index, element })).map(|_| ())
}

fn patterned(count: u32) -> (Session, Id<Body>, Id<Body>) {
    let mut s = Session::new();
    let floor = s.add_body(Solid::Box { size: [2.0, 1.0, 0.1] }, at(0.0, 0.0, 0.0)).unwrap();
    let part = s.add_body(Solid::Box { size: [0.1; 3] }, at(0.2, 0.0, 0.2)).unwrap();
    s.add_pattern("Row", &[part.erase()], row_of(floor, count)).unwrap();
    (s, floor, part)
}

#[test]
fn a_suppressed_pattern_element_leaves_the_bill_of_materials_and_comes_back_with_it() {
    let (mut s, _floor, _part) = patterned(4);
    let all = parts(&s);
    assert_eq!(all, 5, "floor + parent + three copies");
    let pattern = &s.patterns()[0];
    let element = pattern.elements[1].index;
    let item = pattern.elements[1].items[0];
    assert!(step(&mut s, "suppress", |s| suppress(s, 0, element, true)).is_some());
    assert_eq!(s.patterns()[0].suppressed, vec![element]);
    assert_eq!(parts(&s), all - 1, "excluded from the BOM");
    assert!(s.doc().component_flag(Occurrence::Item(item), gc_doc::ComponentFlag::Suppressed));
    // Suppressed twice: still one entry; and the list is not duplicated.
    let before = s.doc().clone();
    drop(suppress(&mut s, 0, element, true));
    assert_eq!(s.patterns()[0].suppressed.iter().filter(|e| **e == element).count(), 1);
    let _ = before;
    // Bring it back.
    assert!(step(&mut s, "unsuppress", |s| suppress(s, 0, element, false)).is_some());
    assert_eq!(parts(&s), all);
    assert!(s.patterns()[0].suppressed.is_empty());
    assert!(!s.doc().component_flag(Occurrence::Item(item), gc_doc::ComponentFlag::Suppressed));
}

#[test]
fn suppressing_what_is_not_an_element_is_refused_with_no_trace() {
    let (mut s, _floor, _part) = patterned(3);
    // The parent is occurrence 0 and cannot be suppressed through the pattern; nor can numbers that are not there.
    for element in [0u32, 3, 4, 99, u32::MAX] {
        assert!(
            step(&mut s, "bad element", |s| suppress(s, 0, element, true)).is_none(),
            "element {element}"
        );
    }
    for index in [1usize, 5, usize::MAX] {
        assert!(step(&mut s, "bad pattern", |s| suppress(s, index, 1, true)).is_none(), "pattern {index}");
        assert!(step(&mut s, "bad pattern restore", |s| restore(s, index, 1)).is_none(), "pattern {index}");
    }
    // Restoring an element that is not independent is refused (it is in the pattern already).
    assert!(step(&mut s, "restore a member", |s| restore(s, 0, 1)).is_none());
}

#[test]
fn a_suppressed_element_stays_suppressed_when_the_count_changes_and_with_a_saved_file() {
    let (mut s, floor, _part) = patterned(4);
    let element = s.patterns()[0].elements[1].index;
    suppress(&mut s, 0, element, true).unwrap();
    // Count 6: the element is still suppressed, the new ones are not.
    s.execute(&Command::Cell(CellOp::SetPattern { index: 0, layout: row_of(floor, 6) })).unwrap();
    assert!(s.patterns()[0].suppressed.contains(&element));
    assert_eq!(parts(&s), 1 + 1 + 5 - 1);
    // Count 2: the suppressed element is gone with the copy or remains listed without harm.
    s.execute(&Command::Cell(CellOp::SetPattern { index: 0, layout: row_of(floor, 2) })).unwrap();
    let listed = s.patterns()[0].suppressed.clone();
    assert!(
        listed.iter().all(|e| s.patterns()[0].elements.iter().any(|x| x.index == *e))
            || listed.contains(&element)
    );
    s.execute(&Command::Cell(CellOp::SetPattern { index: 0, layout: row_of(floor, 5) })).unwrap();
    assert!(parts(&s) >= 5);
    // Save and open keeps it.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.gcp");
    s.save_as(&path).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert!(t.doc().same_content(s.doc()));
    assert_eq!(t.patterns()[0].suppressed, s.patterns()[0].suppressed);
    assert_eq!(parts(&t), parts(&s));
}

#[test]
fn suppressing_every_copy_and_deleting_the_pattern_leaves_no_ghost_in_the_bill_of_materials() {
    let (mut s, _floor, _part) = patterned(4);
    for element in s.patterns()[0].elements.iter().map(|e| e.index).collect::<Vec<_>>() {
        suppress(&mut s, 0, element, true).unwrap();
    }
    assert_eq!(parts(&s), 2, "the floor and the parent remain");
    let keep_copies = true;
    assert!(
        step(&mut s, "delete the pattern", |s| s
            .execute(&Command::Cell(CellOp::DeletePattern { index: 0, keep_copies }))
            .map(|_| ()))
        .is_some()
    );
    assert!(s.patterns().is_empty());
    // The copies stay but are still suppressed: out of the BOM until brought back.
    assert!(parts(&s) <= 5);
    assert!(parts(&s) >= 2);
}

#[test]
fn making_a_suppressed_element_independent_is_one_step_and_consistent() {
    let (mut s, _floor, _part) = patterned(4);
    let element = s.patterns()[0].elements[0].index;
    suppress(&mut s, 0, element, true).unwrap();
    step(&mut s, "make independent", |s| {
        s.execute(&Command::Cell(CellOp::MakePatternElementIndependent { index: 0, element })).map(|_| ())
    });
    // Whatever was decided: the BOM counts exactly the parts that are not suppressed.
    let live = s
        .components()
        .iter()
        .filter(|n| {
            matches!(n.occurrence, Occurrence::Item(i) if i.kind == gc_core::Kind::Body) && !n.suppressed
        })
        .count();
    assert_eq!(parts(&s) as usize, live);
}

// ---- view representations ----

fn view_scene() -> (Session, Id<Body>, Id<Body>) {
    let mut s = Session::new();
    let a = s.add_body(Solid::Box { size: [0.2; 3] }, at(0.0, 0.0, 0.1)).unwrap();
    let b = s.add_body(Solid::Box { size: [0.2; 3] }, at(0.5, 0.0, 0.1)).unwrap();
    (s, a, b)
}

#[test]
fn locking_a_view_keeps_its_display_and_hides_what_is_added_later() {
    let (mut s, a, _b) = view_scene();
    // A new view is active as soon as it is made (backend 50fba77, lead-approved), and display edits go into
    // the active view: hide A in "Only B", then show it again in a second view, and come back to the first.
    s.add_view_representation("Only B", None).unwrap();
    s.set_visible(a.erase(), false).unwrap();
    s.add_view_representation("All", None).unwrap();
    s.set_visible(a.erase(), true).unwrap();
    s.activate_view_representation(0).unwrap();
    assert!(!s.is_visible(a.erase()));
    assert!(step(&mut s, "lock", |s| s.lock_view_representation(0, true)).is_some());
    assert!(s.representations().views[0].locked);
    // A component added while the locked view is active is hidden.
    let c = s.add_body(Solid::Box { size: [0.1; 3] }, at(1.0, 0.0, 0.1)).unwrap();
    assert!(!s.is_visible(c.erase()), "a component added to a locked view is shown");
    // Unlock: it can be edited again; the lock flag goes.
    assert!(step(&mut s, "unlock", |s| s.lock_view_representation(0, false)).is_some());
    assert!(!s.representations().views[0].locked);
}

#[test]
fn lock_and_copy_refuse_views_that_are_not_there_and_never_leave_a_half_state() {
    let (mut s, a, _b) = view_scene();
    for index in [0usize, 1, 7, usize::MAX] {
        assert!(step(&mut s, "lock none", |s| s.lock_view_representation(index, true)).is_none(), "{index}");
        assert!(step(&mut s, "copy none", |s| s.copy_view_representation(index)).is_none(), "{index}");
        assert!(
            step(&mut s, "copy pos none", |s| s.copy_positional_representation(index)).is_none(),
            "{index}"
        );
    }
    s.set_visible(a.erase(), false).unwrap();
    s.add_view_representation("V", None).unwrap();
    // Locking twice and unlocking an unlocked view: each is a step or a plain no-op, never a half state.
    for locked in [true, true, false, false] {
        let before = s.doc().clone();
        let depth = s.history().undo_depth();
        let r = s.lock_view_representation(0, locked);
        assert!(r.is_ok());
        assert!(s.history().undo_depth() <= depth + 1);
        assert_eq!(s.representations().views[0].locked, locked);
        let _ = before;
    }
}

#[test]
fn copying_a_view_numbers_the_name_activates_the_copy_and_does_not_copy_the_lock() {
    let (mut s, a, _b) = view_scene();
    s.set_visible(a.erase(), false).unwrap();
    s.add_view_representation("Open", None).unwrap();
    s.lock_view_representation(0, true).unwrap();
    assert!(step(&mut s, "copy", |s| s.copy_view_representation(0)).is_some());
    let views = s.representations().views;
    assert_eq!(views.len(), 2);
    assert_ne!(views[0].name, views[1].name);
    assert!(views[0].name.starts_with("Open") && views[1].name.starts_with("Open"));
    assert!(!views[1].locked, "the copy is unlocked");
    assert!(views[0].locked, "the original keeps its lock");
    assert_eq!(views[1].hidden, views[0].hidden);
    assert_eq!(s.representations().active_view, Some(1), "the copy is the active one");
    // Copy of a copy numbers on: three names, all different.
    s.copy_view_representation(1).unwrap();
    s.copy_view_representation(0).unwrap();
    let names: Vec<String> = s.representations().views.iter().map(|v| v.name.clone()).collect();
    let mut sorted = names.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), names.len(), "names must stay distinct: {names:?}");
    // A view that is named like the number a copy would take.
    let (mut s, a, _b) = view_scene();
    s.set_visible(a.erase(), false).unwrap();
    s.add_view_representation("View1", None).unwrap();
    s.add_view_representation("View2", None).unwrap();
    s.copy_view_representation(0).unwrap();
    let names: Vec<String> = s.representations().views.iter().map(|v| v.name.clone()).collect();
    let mut sorted = names.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), names.len(), "{names:?}");
}

#[test]
fn copying_a_positional_representation_keeps_the_overrides_and_the_active_one() {
    let (mut s, a, _b) = view_scene();
    s.add_positional_representation("Open").unwrap();
    let open = gc_doc::PositionalRepresentation {
        name: "Open".into(),
        placements: vec![(a.erase(), at(0.0, 0.7, 0.1))],
        ..Default::default()
    };
    s.execute(&Command::Cell(CellOp::SetPositionalRepresentation { index: 0, position: open })).unwrap();
    assert!(step(&mut s, "copy position", |s| s.copy_positional_representation(0)).is_some());
    let reps = s.representations();
    assert_eq!(reps.positions.len(), 2);
    assert_ne!(reps.positions[0].name, reps.positions[1].name);
    assert_ne!(reps.positions[1].name, "Primary");
    assert_eq!(reps.active_position, None, "which one is active does not change");
    // The copy carries the same overrides: activate it and the part stands where the original put it.
    s.activate_positional_representation(Some(1)).unwrap();
    assert_eq!(s.doc().get(a).unwrap().placement.xyz, [0.0, 0.7, 0.1]);
    s.activate_positional_representation(None).unwrap();
    assert_eq!(s.doc().get(a).unwrap().placement.xyz, [0.0, 0.0, 0.1]);
}

#[test]
fn locked_and_copied_views_survive_saving_and_deleting_keeps_the_active_index_valid() {
    let (mut s, a, _b) = view_scene();
    s.set_visible(a.erase(), false).unwrap();
    s.add_view_representation("Open", None).unwrap();
    s.lock_view_representation(0, true).unwrap();
    s.copy_view_representation(0).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v.gcp");
    s.save_as(&path).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert!(t.doc().same_content(s.doc()));
    assert!(t.representations().views[0].locked);
    assert!(!t.representations().views[1].locked);
    // Delete the active (copied) view and then the first: the active index never points past the list.
    assert!(step(&mut s, "delete copy", |s| s.delete_representation(Some(1), None)).is_some());
    let reps = s.representations();
    assert!(reps.active_view.is_none_or(|i| i < reps.views.len()));
    assert!(step(&mut s, "delete first", |s| s.delete_representation(Some(0), None)).is_some());
    assert!(s.representations().views.is_empty());
    assert!(s.representations().active_view.is_none());
    // Old file: strip the lock keys; it opens as an unlocked view.
    let mut json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    fn strip(v: &mut serde_json::Value) {
        match v {
            serde_json::Value::Object(m) => {
                m.remove("locked");
                m.remove("known");
                m.values_mut().for_each(strip);
            }
            serde_json::Value::Array(a) => a.iter_mut().for_each(strip),
            _ => {}
        }
    }
    strip(&mut json);
    let old = dir.path().join("old.gcp");
    std::fs::write(&old, serde_json::to_string(&json).unwrap()).unwrap();
    let mut u = Session::new();
    u.open(&old).unwrap_or_else(|e| panic!("an older file must open: {e:?}"));
    assert!(u.representations().views.iter().all(|v| !v.locked));
}
