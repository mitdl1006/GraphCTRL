//! Subassemblies made in the cell, the browser tree with indentation, and editing in place (Inventor 2026
//! help: Create Subassemblies in Place, GUID-FCEE3F0E-AAE2-4619-8645-2588FEECD353; Edit components in
//! place, GUID-C79C36C9-A6DB-46C5-8864-41F66C750231).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "tests")]

use gc_core::Id;
use gc_doc::{Body, Created, EditError, Occurrence, Placement, Solid, SubassemblyId, SubassemblyOp, TreeKey};
use gc_session::Session;

fn body(s: &mut Session, x: f64) -> Id<Body> {
    s.add_body(Solid::Box { size: [0.1; 3] }, Placement { xyz: [x, 0.0, 0.0], rpy: [0.0; 3] }).unwrap()
}

fn key(b: Id<Body>) -> TreeKey {
    TreeKey::Component(Occurrence::Item(b.erase()))
}

fn occ(b: Id<Body>) -> Occurrence {
    Occurrence::Item(b.erase())
}

fn demote(s: &mut Session, members: Vec<TreeKey>, name: &str) -> SubassemblyId {
    match s.edit_subassemblies(SubassemblyOp::Demote { members, name: name.into() }).unwrap()[..] {
        [Created::Subassembly(id)] => id,
        ref other => panic!("{other:?}"),
    }
}

fn row(s: &Session, k: TreeKey) -> (Option<TreeKey>, u32) {
    let r = s.component_tree().into_iter().find(|r| r.key == k).expect("a row");
    (r.parent, r.depth)
}

#[test]
fn demote_makes_an_indented_level_in_one_undo_step() {
    let mut s = Session::new();
    let (a, b, c) = (body(&mut s, 0.0), body(&mut s, 0.3), body(&mut s, 0.6));
    let sub = demote(&mut s, vec![key(a), key(b)], "");
    let tree = s.component_tree();
    let names: Vec<(String, u32)> = tree.iter().map(|r| (r.name.clone(), r.depth)).collect();
    // The subassembly first at its level, then the loose component.
    assert_eq!(names[0], ("Subassembly:1".to_owned(), 0), "{names:?}");
    assert_eq!(row(&s, key(a)), (Some(TreeKey::Subassembly(sub)), 1));
    assert_eq!(row(&s, key(b)), (Some(TreeKey::Subassembly(sub)), 1));
    assert_eq!(row(&s, key(c)), (None, 0));
    assert!(tree.iter().find(|r| r.key == TreeKey::Subassembly(sub)).unwrap().component.is_none());

    // Nested: the subassembly and C into another one.
    let outer = demote(&mut s, vec![TreeKey::Subassembly(sub), key(c)], "Cell frame");
    assert_eq!(row(&s, TreeKey::Subassembly(sub)), (Some(TreeKey::Subassembly(outer)), 1));
    assert_eq!(row(&s, key(a)).1, 2);
    assert_eq!(row(&s, key(c)), (Some(TreeKey::Subassembly(outer)), 1));

    // Each Demote is one undo step.
    assert!(s.undo());
    assert_eq!(row(&s, key(c)), (None, 0));
    assert_eq!(row(&s, key(a)).1, 1);
    assert!(s.undo());
    assert!(s.component_tree().iter().all(|r| r.depth == 0));
}

#[test]
fn bad_demotes_and_moves_are_refused_and_leave_no_trace() {
    let mut s = Session::new();
    let (a, b, c) = (body(&mut s, 0.0), body(&mut s, 0.3), body(&mut s, 0.6));
    let inner = demote(&mut s, vec![key(a)], "Inner");
    let outer = demote(&mut s, vec![TreeKey::Subassembly(inner)], "Outer");
    let before = s.doc().clone();
    let refused = |s: &mut Session, op: SubassemblyOp| {
        assert!(s.edit_subassemblies(op.clone()).is_err(), "{op:?} was accepted");
        assert!(s.doc().same_content(&before), "{op:?} left a trace");
    };
    // Members of different levels, nothing, a part of nothing, a ghost subassembly.
    refused(&mut s, SubassemblyOp::Demote { members: vec![key(a), key(b)], name: String::new() });
    refused(&mut s, SubassemblyOp::Demote { members: vec![], name: String::new() });
    refused(
        &mut s,
        SubassemblyOp::Demote { members: vec![TreeKey::Subassembly(SubassemblyId(99))], name: String::new() },
    );
    // A name already taken.
    refused(&mut s, SubassemblyOp::Demote { members: vec![key(b)], name: "Outer".into() });
    // Into itself or its own.
    refused(&mut s, SubassemblyOp::Move { members: vec![TreeKey::Subassembly(outer)], into: Some(outer) });
    refused(&mut s, SubassemblyOp::Move { members: vec![TreeKey::Subassembly(outer)], into: Some(inner) });
    refused(&mut s, SubassemblyOp::Move { members: vec![key(c)], into: Some(SubassemblyId(99)) });
    // Renames: empty, taken, ghost.
    refused(&mut s, SubassemblyOp::Rename { id: inner, name: "  ".into() });
    refused(&mut s, SubassemblyOp::Rename { id: inner, name: "Outer".into() });
    refused(&mut s, SubassemblyOp::Dissolve(SubassemblyId(99)));
}

#[test]
fn moving_dissolving_and_deleting_keep_the_tree_whole() {
    let mut s = Session::new();
    let (a, b) = (body(&mut s, 0.0), body(&mut s, 0.3));
    let inner = demote(&mut s, vec![key(a)], "Inner");
    let outer = demote(&mut s, vec![TreeKey::Subassembly(inner)], "Outer");
    // Drag B into Inner, then back to the top.
    s.edit_subassemblies(SubassemblyOp::Move { members: vec![key(b)], into: Some(inner) }).unwrap();
    assert_eq!(row(&s, key(b)), (Some(TreeKey::Subassembly(inner)), 2));
    s.edit_subassemblies(SubassemblyOp::Move { members: vec![key(b)], into: None }).unwrap();
    assert_eq!(row(&s, key(b)), (None, 0));
    // Dissolving Outer lifts Inner to the top; dissolving Inner lifts A.
    s.edit_subassemblies(SubassemblyOp::Dissolve(outer)).unwrap();
    assert_eq!(row(&s, TreeKey::Subassembly(inner)), (None, 0));
    s.edit_subassemblies(SubassemblyOp::Rename { id: inner, name: "Gripper".into() }).unwrap();
    assert!(s.component_tree().iter().any(|r| r.name == "Gripper"));
    s.edit_subassemblies(SubassemblyOp::Dissolve(inner)).unwrap();
    assert_eq!(row(&s, key(a)), (None, 0));
    // A deleted component leaves no row, and its membership goes with it.
    s.undo();
    s.undo();
    assert_eq!(row(&s, key(a)).1, 1);
    s.delete_item(gc_session::CellItem::Body(a)).unwrap();
    assert!(s.component_tree().iter().all(|r| r.key != key(a)));
}

#[test]
fn subassemblies_are_saved_and_loaded() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cell.gcp");
    let mut s = Session::new();
    let (a, b) = (body(&mut s, 0.0), body(&mut s, 0.3));
    let sub = demote(&mut s, vec![key(a), key(b)], "Jig");
    s.save_as(&path).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert_eq!(row(&t, key(a)), (Some(TreeKey::Subassembly(sub)), 1));
    assert!(t.component_tree().iter().any(|r| r.name == "Jig"));
}

#[test]
fn editing_in_place_scopes_picks_and_returns() {
    let mut s = Session::new();
    let (a, b, c) = (body(&mut s, 0.0), body(&mut s, 0.3), body(&mut s, 0.6));
    let inner = demote(&mut s, vec![key(a)], "Inner");
    let outer = demote(&mut s, vec![TreeKey::Subassembly(inner), key(b)], "Outer");

    // At the top level everything is in scope and a subassembly is picked as one unit.
    assert_eq!(s.edit_target(), None);
    assert!(s.in_edit_scope(occ(c)));
    assert_eq!(s.pick_unit(occ(a)), Some(TreeKey::Subassembly(outer)));
    assert_eq!(s.pick_unit(occ(c)), Some(key(c)));

    // Double-click Outer: C is outside (faded, not picked); Inner is the unit inside.
    s.edit_component(TreeKey::Subassembly(outer)).unwrap();
    assert_eq!(s.edit_target(), Some(TreeKey::Subassembly(outer)));
    assert!(s.in_edit_scope(occ(a)) && s.in_edit_scope(occ(b)));
    assert!(!s.in_edit_scope(occ(c)));
    assert_eq!(s.pick_unit(occ(a)), Some(TreeKey::Subassembly(inner)));
    assert_eq!(s.pick_unit(occ(b)), Some(key(b)));
    assert_eq!(s.pick_unit(occ(c)), None);

    // Then Inner, then A itself.
    s.edit_component(TreeKey::Subassembly(inner)).unwrap();
    assert_eq!(s.pick_unit(occ(a)), Some(key(a)));
    s.edit_component(key(a)).unwrap();
    assert!(!s.in_edit_scope(occ(b)));
    assert_eq!(s.pick_unit(occ(a)), Some(key(a)));

    // Return to Parent: Inner. Return: back to A. Return to Top: none. Return again: A.
    s.return_to_parent();
    assert_eq!(s.edit_target(), Some(TreeKey::Subassembly(inner)));
    s.return_edit();
    assert_eq!(s.edit_target(), Some(key(a)));
    s.return_to_top();
    assert_eq!(s.edit_target(), None);
    s.return_edit();
    assert_eq!(s.edit_target(), Some(key(a)));

    // A target that goes away counts as none; a missing row cannot be edited.
    s.delete_item(gc_session::CellItem::Body(a)).unwrap();
    assert_eq!(s.edit_target(), None);
    assert_eq!(s.edit_component(key(a)), Err(EditError::BadValue));
    assert!(s.in_edit_scope(occ(c)));
}

/// A dissolved subassembly's id is never given to a new one (an id names one subassembly for the life of
/// the document, like other document ids).
#[test]
fn subassembly_ids_are_not_reused() {
    let mut s = Session::new();
    let (a, b) = (body(&mut s, 0.0), body(&mut s, 0.3));
    let first = demote(&mut s, vec![key(a)], "First");
    s.edit_subassemblies(SubassemblyOp::Dissolve(first)).unwrap();
    let second = demote(&mut s, vec![key(b)], "Second");
    assert_ne!(first, second);
}

/// While a subassembly is edited in place, components outside it cannot be selected, so the Ground toggle,
/// Delete and the like cannot reach them.
#[test]
fn components_outside_the_edit_target_cannot_be_selected() {
    let mut s = Session::new();
    let (a, c) = (body(&mut s, 0.0), body(&mut s, 0.6));
    let sub = demote(&mut s, vec![key(a)], "Jig");
    assert!(s.select_occurrence(Some(occ(c))));
    s.edit_component(TreeKey::Subassembly(sub)).unwrap();
    // A selection made before entering no longer counts.
    assert_eq!(s.selected_occurrence(), None);
    assert_eq!(s.selected_item(), None);
    assert!(!s.select_occurrence(Some(occ(c))));
    s.select_item(Some(gc_session::CellItem::Body(c)));
    assert_eq!(s.selected_item(), None);
    assert!(s.select_occurrence(Some(occ(a))));
    assert_eq!(s.selected_item(), Some(gc_session::CellItem::Body(a)));
    s.return_to_top();
    assert!(s.select_occurrence(Some(occ(c))));
}

/// Help (Sketches in assemblies, GUID-2C180C3A-2064-4671-B1C2-F74B383B5D39): a sketch made in the assembly
/// belongs to the top-level assembly; one made while a component is edited in place belongs to that
/// component and sits on its origin plane (it moves with it).
#[test]
fn sketches_belong_to_the_assembly_or_to_the_component_being_edited() {
    let mut s = Session::new();
    let (a, b) = (body(&mut s, 0.2), body(&mut s, 0.5));
    let top = s.create_sketch("XY").unwrap();
    s.finish_sketch();
    assert_eq!(s.sketch_owner(top), None);

    s.edit_component(key(a)).unwrap();
    let steps = s.history().undo_depth();
    let own = s.create_sketch("XZ").unwrap();
    assert_eq!(s.history().undo_depth(), steps + 1, "a sketch with its owner is one undo step");
    s.finish_sketch();
    assert_eq!(s.sketch_owner(own), Some(key(a)));
    assert_eq!(s.sketches_of(Some(key(a))), vec![own]);
    assert_eq!(s.sketches_of(None), vec![top]);
    // On A's own XZ plane: it stands where A stands (x = 0.2), not at the world's origin.
    let frame_x = |s: &Session, k: Id<gc_doc::Sketch>| {
        let (gc_sketch::Space::Planar(f) | gc_sketch::Space::Spatial(f)) = s.doc().get(k).unwrap().space;
        s.doc().frame_world_pose(Id::from_raw(f.raw)).unwrap().translation.x
    };
    assert!((frame_x(&s, own) - 0.2).abs() < 1e-9);
    assert!(frame_x(&s, top).abs() < 1e-9);
    s.return_to_top();

    // Editing a subassembly: the sketch belongs to it.
    let sub = demote(&mut s, vec![key(b)], "Jig");
    s.edit_component(TreeKey::Subassembly(sub)).unwrap();
    let in_sub = s.create_sketch("XY").unwrap();
    s.finish_sketch();
    assert_eq!(s.sketch_owner(in_sub), Some(TreeKey::Subassembly(sub)));
    s.return_to_top();
    // An owner that goes away hands its sketches to the top level.
    s.edit_subassemblies(SubassemblyOp::Dissolve(sub)).unwrap();
    assert_eq!(s.sketch_owner(in_sub), None);
    // Undoing the sketch takes its ownership with it; redo brings both back.
    s.undo();
    assert_eq!(s.sketch_owner(in_sub), Some(TreeKey::Subassembly(sub)));
}

/// A sketch created with an empty name takes the next free "SketchN" (names are trimmed, as Rename does).
#[test]
fn an_empty_sketch_name_is_numbered() {
    let mut s = Session::new();
    let frame = s.doc().origin_plane("XY").unwrap();
    let make = |s: &mut Session, name: &str| match s
        .execute(&gc_doc::Command::CreateSketch { name: name.into(), frame, spatial: false })
        .unwrap()[..]
    {
        [Created::Sketch(id)] => s.doc().get(id).unwrap().name.clone(),
        ref other => panic!("{other:?}"),
    };
    assert_eq!(make(&mut s, ""), "Sketch1");
    assert_eq!(make(&mut s, "   "), "Sketch2");
    assert_eq!(make(&mut s, "  Base  "), "Base");
}

/// A sketch an owner's delete would take is refused when something else uses it (a work axis on its line).
#[test]
fn an_owned_sketch_in_use_keeps_its_owner() {
    let mut s = Session::new();
    let _first = body(&mut s, 1.0);
    let a = body(&mut s, 0.0);
    s.edit_component(key(a)).unwrap();
    let k = s.create_sketch("XY").unwrap();
    let op = gc_doc::SketchOp::AddLine {
        a: [0.0, 0.0, 0.0],
        b: [0.05, 0.0, 0.0],
        snap_a: gc_doc::EndSnap::default(),
        snap_b: gc_doc::EndSnap::default(),
        horizontal: None,
    };
    let line = match s.execute(&gc_doc::Command::Sketch { sketch: k, op }).unwrap()[..] {
        [Created::Curve(c), ..] => c,
        ref other => panic!("{other:?}"),
    };
    s.finish_sketch();
    s.return_to_top();
    let axis = gc_doc::WorkFeature {
        name: "Axis".into(),
        geometry: gc_doc::WorkGeometry::AxisOnLine { line: gc_doc::CurveRef { sketch: k, curve: line } },
        visible: true,
    };
    s.execute(&gc_doc::Command::Cell(gc_doc::CellOp::AddWorkFeature(axis))).unwrap();
    let before = s.doc().clone();
    assert_eq!(s.delete_item(gc_session::CellItem::Body(a)), Err(EditError::InUse));
    assert!(s.doc().same_content(&before));
}

/// A subassembly moves as one unit (help: "treated as a single unit in the parent assembly"): every
/// component in it, nested levels too, by the same move; one undo step; refused when one is grounded.
#[test]
fn a_subassembly_moves_as_one_unit() {
    use gc_math::{DPose3, DQuat, DVec3};
    let mut s = Session::new();
    let _first = body(&mut s, 5.0); // grounded, outside
    let (a, b, c) = (body(&mut s, 0.0), body(&mut s, 0.3), body(&mut s, 0.6));
    let inner = demote(&mut s, vec![key(a)], "Inner");
    let outer = demote(&mut s, vec![TreeKey::Subassembly(inner), key(b)], "Outer");
    let origin = |s: &Session, x: Id<Body>| s.doc().component_origin(occ(x)).unwrap().pose;
    let (pa, pb, pc) = (origin(&s, a), origin(&s, b), origin(&s, c));
    // A quarter turn about Z and a lift.
    let delta =
        DPose3::from_parts(DVec3::new(0.0, 0.0, 0.5), DQuat::from_rotation_z(std::f64::consts::FRAC_PI_2));
    let moves = s.subassembly_moved(outer, delta).unwrap();
    assert_eq!(moves.len(), 2, "A (nested) and B");
    let steps = s.history().undo_depth();
    s.move_subassembly(outer, delta).unwrap();
    assert_eq!(s.history().undo_depth(), steps + 1);
    for (x, before) in [(a, pa), (b, pb)] {
        let now = origin(&s, x);
        let want = delta * before;
        assert!(
            (now.translation - want.translation).length() < 1e-9,
            "{:?} vs {:?}",
            now.translation,
            want.translation
        );
    }
    // The two keep their distance (a rigid unit); C outside did not move.
    assert!(((origin(&s, a).translation - origin(&s, b).translation).length() - 0.3).abs() < 1e-9);
    assert_eq!(origin(&s, c).translation, pc.translation);
    s.undo();
    assert!((origin(&s, a).translation - pa.translation).length() < 1e-12);
    // A grounded member holds the unit; an empty or missing subassembly has nothing to move.
    s.set_component(occ(b), gc_doc::ComponentFlag::Grounded, true).unwrap();
    assert!(s.subassembly_moved(outer, delta).is_none());
    assert_eq!(s.move_subassembly(outer, delta), Err(EditError::BadValue));
    assert!(s.subassembly_moved(SubassemblyId(99), delta).is_none());
}

/// Previewing several components at once (a subassembly dragged as a unit) leaves the document alone and
/// shows them all; ending the preview shows the document again.
#[test]
fn several_components_preview_together_without_touching_the_document() {
    use gc_math::{DPose3, DQuat, DVec3};
    let mut s = Session::new();
    let _first = body(&mut s, 5.0);
    let (a, b) = (body(&mut s, 0.0), body(&mut s, 0.3));
    let sub = demote(&mut s, vec![key(a), key(b)], "Jig");
    let delta = DPose3::from_parts(DVec3::new(0.0, 0.0, 0.4), DQuat::IDENTITY);
    let moves = s.subassembly_moved(sub, delta).unwrap();
    let before = s.doc().clone();
    let revision = s.history().undo_depth();
    s.preview_components(&moves).unwrap();
    assert!(s.doc().same_content(&before));
    assert_eq!(s.history().undo_depth(), revision);
    for x in [a, b] {
        let shown = s.shown_body(x).unwrap().placement;
        assert!((shown.xyz[2] - 0.4).abs() < 1e-9, "{shown:?}");
        assert!(s.driven_placement(x.erase()).is_some());
    }
    s.end_component_preview();
    assert!(s.shown_body(a).unwrap().placement.xyz[2].abs() < 1e-9);
    // Not a component, or not finite: refused, nothing shown.
    let bad = DPose3::from_parts(DVec3::new(f64::NAN, 0.0, 0.0), DQuat::IDENTITY);
    assert_eq!(s.preview_components(&[(a.erase(), bad)]), Err(EditError::BadValue));
    assert_eq!(s.preview_components(&[(sub_key_item(), delta)]), Err(EditError::BadValue));
    assert!(s.driven_placement(a.erase()).is_none());
}

fn sub_key_item() -> gc_core::AnyId {
    gc_core::Id::<gc_doc::WorkFeature>::from_raw(std::num::NonZeroU64::new(9999).unwrap()).erase()
}
