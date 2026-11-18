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
