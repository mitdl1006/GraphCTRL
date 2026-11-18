//! QA boundary tests for subassemblies made in the cell (Demote, Move, Dissolve, Rename) and editing in
//! place. Inventor 2026 help, under <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/>:
//! Create Subassemblies in Place GUID-FCEE3F0E-AAE2-4619-8645-2588FEECD353, Edit components in place
//! GUID-C79C36C9-A6DB-46C5-8864-41F66C750231.
//! Every refused edit leaves the document and the history untouched; every accepted one is one undo step
//! that restores exactly.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::disallowed_methods,
    reason = "test code"
)]

use gc_core::Id;
use gc_doc::{Body, Created, EditError, Occurrence, Placement, Solid, SubassemblyId, SubassemblyOp, TreeKey};
use gc_session::{CellItem, Session};

fn body(s: &mut Session, x: f64) -> Id<Body> {
    s.add_body(Solid::Box { size: [0.1; 3] }, Placement { xyz: [x, 0.0, 0.0], rpy: [0.0; 3] }).unwrap()
}

fn key(b: Id<Body>) -> TreeKey {
    TreeKey::Component(Occurrence::Item(b.erase()))
}

fn occ(b: Id<Body>) -> Occurrence {
    Occurrence::Item(b.erase())
}

fn sub(id: SubassemblyId) -> TreeKey {
    TreeKey::Subassembly(id)
}

fn demote(s: &mut Session, members: Vec<TreeKey>, name: &str) -> SubassemblyId {
    match s.edit_subassemblies(SubassemblyOp::Demote { members, name: name.into() }).unwrap()[..] {
        [Created::Subassembly(id)] => id,
        ref other => panic!("{other:?}"),
    }
}

fn row(s: &Session, k: TreeKey) -> Option<(Option<TreeKey>, u32)> {
    s.component_tree().into_iter().find(|r| r.key == k).map(|r| (r.parent, r.depth))
}

/// Refused: nothing changed.
fn refused(s: &mut Session, what: &str, op: SubassemblyOp) {
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    let result = s.edit_subassemblies(op);
    assert!(result.is_err(), "{what} was accepted: {result:?}");
    assert!(s.doc().same_content(&before), "{what} changed the document");
    assert_eq!(s.history().undo_depth(), depth, "{what}");
}

/// Accepted as one undo step that undoes exactly.
fn accepted(s: &mut Session, what: &str, op: SubassemblyOp) -> Vec<Created> {
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    let created = s.edit_subassemblies(op.clone()).unwrap_or_else(|e| panic!("{what}: {e:?}"));
    assert_eq!(s.history().undo_depth(), depth + 1, "{what}: one undo step");
    let after = s.doc().clone();
    assert!(s.undo());
    assert!(s.doc().same_content(&before), "{what}: undo restores");
    assert!(s.redo());
    assert!(s.doc().same_content(&after), "{what}: redo repeats");
    created
}

// ---- Demote ----

#[test]
fn demoting_nothing_or_what_does_not_exist_is_refused() {
    let mut s = Session::new();
    let (a, b) = (body(&mut s, 0.0), body(&mut s, 0.3));
    refused(&mut s, "empty", SubassemblyOp::Demote { members: vec![], name: "X".into() });
    // A component that was deleted (a ghost), alone or among real ones.
    s.delete_item(CellItem::Body(b)).unwrap();
    refused(&mut s, "deleted component", SubassemblyOp::Demote { members: vec![key(b)], name: "X".into() });
    refused(
        &mut s,
        "deleted among real",
        SubassemblyOp::Demote { members: vec![key(a), key(b)], name: "X".into() },
    );
    refused(
        &mut s,
        "ghost subassembly",
        SubassemblyOp::Demote { members: vec![sub(SubassemblyId(7))], name: "X".into() },
    );
    // Only components placed in the cell and subassemblies can be demoted: not a part or a link.
    let ghost_part = TreeKey::Component(Occurrence::Part {
        assembly: Id::from_raw(std::num::NonZeroU64::new(999).unwrap()),
        index: 0,
    });
    refused(&mut s, "a part", SubassemblyOp::Demote { members: vec![ghost_part], name: "X".into() });
}

/// Members from different levels cannot be demoted together; members already in another subassembly can be
/// demoted among themselves (the new one nests in theirs); a member listed twice is put in once.
#[test]
fn demoting_members_of_other_subassemblies_follows_their_level() {
    let mut s = Session::new();
    let (a, b, c) = (body(&mut s, 0.0), body(&mut s, 0.3), body(&mut s, 0.6));
    let one = demote(&mut s, vec![key(a), key(b)], "One");
    // A is in One, C is at the top: two levels, refused.
    refused(
        &mut s,
        "two levels",
        SubassemblyOp::Demote { members: vec![key(a), key(c)], name: "Mixed".into() },
    );
    // A alone, inside One: nests there.
    let created =
        accepted(&mut s, "nest", SubassemblyOp::Demote { members: vec![key(a)], name: "Nested".into() });
    let [Created::Subassembly(nested)] = created[..] else { panic!("{created:?}") };
    assert_eq!(row(&s, sub(nested)), Some((Some(sub(one)), 1)));
    assert_eq!(row(&s, key(a)), Some((Some(sub(nested)), 2)));
    // A member listed twice.
    let before = s.component_tree().len();
    let twice = demote(&mut s, vec![key(c), key(c)], "Twice");
    assert_eq!(row(&s, key(c)), Some((Some(sub(twice)), 1)));
    assert_eq!(s.component_tree().len(), before + 1, "one new row, C moved under it");
}

#[test]
fn subassembly_names_are_checked() {
    let mut s = Session::new();
    let (a, b, c) = (body(&mut s, 0.0), body(&mut s, 0.3), body(&mut s, 0.6));
    let _ = demote(&mut s, vec![key(a)], "Jig");
    refused(&mut s, "taken", SubassemblyOp::Demote { members: vec![key(b)], name: "Jig".into() });
    refused(&mut s, "taken, padded", SubassemblyOp::Demote { members: vec![key(b)], name: "  Jig  ".into() });
    // Empty and blank names take the next free "Subassembly:N".
    let auto1 = demote(&mut s, vec![key(b)], "");
    let auto2 = demote(&mut s, vec![key(c)], "   ");
    let names: Vec<String> = s.doc().subassemblies().iter().map(|x| x.name.clone()).collect();
    assert!(
        names.contains(&"Subassembly:1".to_owned()) && names.contains(&"Subassembly:2".to_owned()),
        "{names:?}"
    );
    assert_ne!(auto1, auto2);
    // Unicode, very long and odd names are kept as typed (trimmed).
    for name in ["한글 이름", "名前", &"x".repeat(5000), "a/b\\c:d", "tab\there"] {
        let (mut t, _) = (Session::new(), ());
        let d = body(&mut t, 0.0);
        let id = demote(&mut t, vec![key(d)], name);
        assert_eq!(t.doc().subassemblies().iter().find(|x| x.id == id).unwrap().name, name.trim());
    }
}

// ---- Move ----

/// Moving a subassembly into itself or into one of its own descendants is a cycle: refused whole, no trace.
#[test]
fn moving_into_itself_or_a_descendant_is_refused() {
    let mut s = Session::new();
    let a = body(&mut s, 0.0);
    let inner = demote(&mut s, vec![key(a)], "Inner");
    let middle = demote(&mut s, vec![sub(inner)], "Middle");
    let outer = demote(&mut s, vec![sub(middle)], "Outer");
    for (what, member, into) in [
        ("into itself", outer, outer),
        ("into a child", outer, middle),
        ("into a grandchild", outer, inner),
        ("middle into its child", middle, inner),
        ("middle into itself", middle, middle),
    ] {
        refused(&mut s, what, SubassemblyOp::Move { members: vec![sub(member)], into: Some(into) });
    }
    // Several members, one of which would make the cycle: the whole move is refused.
    let b = body(&mut s, 0.3);
    refused(
        &mut s,
        "a cycle among several",
        SubassemblyOp::Move { members: vec![key(b), sub(outer)], into: Some(inner) },
    );
    // Legal moves are fine: up a level and across.
    accepted(&mut s, "up", SubassemblyOp::Move { members: vec![sub(inner)], into: Some(outer) });
    accepted(&mut s, "to the top", SubassemblyOp::Move { members: vec![sub(inner)], into: None });
    accepted(&mut s, "a component in", SubassemblyOp::Move { members: vec![key(b)], into: Some(inner) });
}

#[test]
fn bad_moves_are_refused_without_a_trace() {
    let mut s = Session::new();
    let (a, b) = (body(&mut s, 0.0), body(&mut s, 0.3));
    let one = demote(&mut s, vec![key(a)], "One");
    refused(&mut s, "nothing", SubassemblyOp::Move { members: vec![], into: Some(one) });
    refused(
        &mut s,
        "to a ghost",
        SubassemblyOp::Move { members: vec![key(b)], into: Some(SubassemblyId(50)) },
    );
    refused(
        &mut s,
        "a ghost subassembly",
        SubassemblyOp::Move { members: vec![sub(SubassemblyId(50))], into: None },
    );
    s.delete_item(CellItem::Body(b)).unwrap();
    refused(&mut s, "a deleted component", SubassemblyOp::Move { members: vec![key(b)], into: Some(one) });
    refused(
        &mut s,
        "a deleted among real",
        SubassemblyOp::Move { members: vec![key(a), key(b)], into: None },
    );
    // Moving to where it already is: accepted or refused cleanly, never a duplicate membership.
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    match s.edit_subassemblies(SubassemblyOp::Move { members: vec![key(a)], into: Some(one) }) {
        Ok(_) => assert!(s.doc().same_content(&before) || s.history().undo_depth() == depth + 1),
        Err(_) => assert!(s.doc().same_content(&before) && s.history().undo_depth() == depth),
    }
    let members: usize = s.doc().subassemblies().len();
    assert_eq!(members, 1);
    assert_eq!(s.component_tree().iter().filter(|r| r.key == key(a)).count(), 1, "one row for A");
}

// ---- Dissolve ----

/// Three levels deep: dissolving the middle lifts both its subassembly and its components to its parent;
/// dissolving the outer then the rest leaves everything at the top; every step undoes exactly and Undo of
/// all of it restores the original tree.
#[test]
fn dissolving_three_nested_levels_and_undoing() {
    let mut s = Session::new();
    let (a, b, c, d) = (body(&mut s, 0.0), body(&mut s, 0.3), body(&mut s, 0.6), body(&mut s, 0.9));
    let inner = demote(&mut s, vec![key(a)], "Inner");
    let middle = demote(&mut s, vec![sub(inner), key(b)], "Middle");
    let outer = demote(&mut s, vec![sub(middle), key(c)], "Outer");
    let start = s.doc().clone();
    let tree_start = s.component_tree();
    assert_eq!(row(&s, key(a)).unwrap().1, 3);

    // Middle: Inner and B go up into Outer.
    accepted(&mut s, "dissolve middle", SubassemblyOp::Dissolve(middle));
    assert_eq!(row(&s, sub(inner)), Some((Some(sub(outer)), 1)));
    assert_eq!(row(&s, key(b)), Some((Some(sub(outer)), 1)));
    assert_eq!(row(&s, key(a)), Some((Some(sub(inner)), 2)));
    assert!(row(&s, sub(middle)).is_none());
    // The ghost cannot be dissolved twice.
    refused(&mut s, "twice", SubassemblyOp::Dissolve(middle));
    // Outer at the top: everything goes to the top.
    s.edit_subassemblies(SubassemblyOp::Dissolve(outer)).unwrap();
    assert_eq!(row(&s, sub(inner)), Some((None, 0)));
    assert_eq!(row(&s, key(b)), Some((None, 0)));
    assert_eq!(row(&s, key(c)), Some((None, 0)));
    s.edit_subassemblies(SubassemblyOp::Dissolve(inner)).unwrap();
    assert!(s.doc().subassemblies().is_empty());
    assert!(s.component_tree().iter().all(|r| r.depth == 0 && r.parent.is_none()));
    // D is untouched throughout.
    assert_eq!(row(&s, key(d)), Some((None, 0)));
    // Undo all three dissolves: the original tree, exactly.
    for _ in 0..3 {
        assert!(s.undo());
    }
    assert!(s.doc().same_content(&start));
    assert_eq!(s.component_tree(), tree_start);
}

/// The components keep their placement and states through every subassembly edit (grouping is browser
/// structure, not geometry).
#[test]
fn subassembly_edits_do_not_move_components() {
    let mut s = Session::new();
    let (a, b) = (body(&mut s, 0.0), body(&mut s, 0.3));
    let poses = |s: &Session| [a, b].map(|x| s.doc().get(x).unwrap().placement);
    let before = poses(&s);
    let one = demote(&mut s, vec![key(a), key(b)], "One");
    s.edit_subassemblies(SubassemblyOp::Move { members: vec![key(b)], into: None }).unwrap();
    s.edit_subassemblies(SubassemblyOp::Rename { id: one, name: "Renamed".into() }).unwrap();
    s.edit_subassemblies(SubassemblyOp::Dissolve(one)).unwrap();
    assert_eq!(poses(&s), before);
}

// ---- Rename ----

#[test]
fn rename_checks_names() {
    let mut s = Session::new();
    let (a, b) = (body(&mut s, 0.0), body(&mut s, 0.3));
    let one = demote(&mut s, vec![key(a)], "One");
    let two = demote(&mut s, vec![key(b)], "Two");
    for (what, id, name) in [
        ("empty", one, ""),
        ("blank", one, "   "),
        ("taken", one, "Two"),
        ("taken, padded", one, " Two "),
        ("ghost", SubassemblyId(40), "Fresh"),
    ] {
        refused(&mut s, what, SubassemblyOp::Rename { id, name: name.into() });
    }
    accepted(&mut s, "rename", SubassemblyOp::Rename { id: one, name: "Uno".into() });
    // Renaming to its own name: accepted or cleanly refused, never a "taken" by itself.
    let before = s.doc().clone();
    match s.edit_subassemblies(SubassemblyOp::Rename { id: two, name: "Two".into() }) {
        Ok(_) => {}
        Err(e) => assert!(s.doc().same_content(&before), "{e:?}"),
    }
}

// ---- Deleted components ----

/// A subassembly whose components were all deleted stays (empty), shows no stale rows, can be renamed,
/// dissolved or filled again, and undo of the delete brings the members back into it.
#[test]
fn a_subassembly_whose_component_was_deleted() {
    let mut s = Session::new();
    let (a, b, c) = (body(&mut s, 0.0), body(&mut s, 0.3), body(&mut s, 0.6));
    let one = demote(&mut s, vec![key(a), key(b)], "One");
    let before_delete = s.doc().clone();
    s.delete_item(CellItem::Body(a)).unwrap();
    assert!(row(&s, key(a)).is_none(), "a row for a deleted component");
    assert_eq!(row(&s, key(b)), Some((Some(sub(one)), 1)));
    s.delete_item(CellItem::Body(b)).unwrap();
    // Empty now: still listed, no children, usable.
    assert_eq!(row(&s, sub(one)), Some((None, 0)));
    assert!(s.component_tree().iter().all(|r| r.parent != Some(sub(one))), "stale children");
    s.edit_subassemblies(SubassemblyOp::Rename { id: one, name: "Empty".into() }).unwrap();
    s.edit_subassemblies(SubassemblyOp::Move { members: vec![key(c)], into: Some(one) }).unwrap();
    assert_eq!(row(&s, key(c)), Some((Some(sub(one)), 1)));
    s.edit_subassemblies(SubassemblyOp::Dissolve(one)).unwrap();
    assert_eq!(row(&s, key(c)), Some((None, 0)));
    // Undo everything back to before the deletes: the members are inside the subassembly again.
    while s.undo() {
        if s.doc().same_content(&before_delete) {
            break;
        }
    }
    assert!(s.doc().same_content(&before_delete));
    assert_eq!(row(&s, key(a)), Some((Some(sub(one)), 1)));
    assert_eq!(row(&s, key(b)), Some((Some(sub(one)), 1)));
}

/// Demoting a component that was deleted and then restored by undo works like any other.
#[test]
fn demoting_after_undoing_a_delete() {
    let mut s = Session::new();
    let a = body(&mut s, 0.0);
    s.delete_item(CellItem::Body(a)).unwrap();
    refused(&mut s, "while deleted", SubassemblyOp::Demote { members: vec![key(a)], name: "X".into() });
    assert!(s.undo());
    accepted(&mut s, "after undo", SubassemblyOp::Demote { members: vec![key(a)], name: "X".into() });
}

// ---- Ids ----

/// A subassembly's id is never reused within a document (the doc comment says so): a new one made after the
/// last was dissolved must not take the dissolved one's id, or anything that remembered the old id (the edit
/// target, a stored selection) would silently point at the new subassembly.
#[test]
fn a_dissolved_subassemblys_id_is_not_reused() {
    let mut s = Session::new();
    let (a, b) = (body(&mut s, 0.0), body(&mut s, 0.3));
    let first = demote(&mut s, vec![key(a)], "First");
    s.edit_component(sub(first)).unwrap();
    s.edit_subassemblies(SubassemblyOp::Dissolve(first)).unwrap();
    assert_eq!(s.edit_target(), None, "the edit target went with the dissolved subassembly");
    let second = demote(&mut s, vec![key(b)], "Second");
    assert_ne!(first, second, "the id was reused");
    assert_eq!(s.edit_target(), None, "a new subassembly became the edit target by id reuse");
}

// ---- Save, open, old and damaged files ----

fn project(s: &mut Session) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cell.gcp");
    s.save_as(&path).unwrap();
    (dir, path)
}

#[test]
fn nested_subassemblies_survive_save_and_open_exactly() {
    let mut s = Session::new();
    let (a, b, c) = (body(&mut s, 0.0), body(&mut s, 0.3), body(&mut s, 0.6));
    let inner = demote(&mut s, vec![key(a)], "내부");
    let outer = demote(&mut s, vec![sub(inner), key(b)], "Outer");
    let (_dir, path) = project(&mut s);
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert!(t.doc().same_content(s.doc()), "the reopened document differs");
    assert_eq!(t.component_tree(), s.component_tree());
    assert_eq!(row(&t, key(a)), Some((Some(sub(inner)), 2)));
    assert_eq!(row(&t, sub(inner)), Some((Some(sub(outer)), 1)));
    assert_eq!(row(&t, key(c)), Some((None, 0)));
    // The reopened document keeps working: another Demote gets a fresh id and a name that is not taken.
    let more = demote(&mut t, vec![key(c)], "");
    assert!(more != inner && more != outer);
    // Saving again gives the same bytes.
    let again = _dir.path().join("again.gcp");
    s.save_as(&again).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), std::fs::read(&again).unwrap());
}

/// A project saved before subassemblies existed has neither field: it opens with everything at the top level.
#[test]
fn an_old_project_without_subassembly_fields_opens_flat() {
    let mut s = Session::new();
    let (a, b) = (body(&mut s, 0.0), body(&mut s, 0.3));
    let one = demote(&mut s, vec![key(a), key(b)], "One");
    let _ = one;
    let (_dir, path) = project(&mut s);
    let mut value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    fn strip(v: &mut serde_json::Value, found: &mut usize) {
        match v {
            serde_json::Value::Object(map) => {
                for key in ["subassemblies", "placed_in"] {
                    if map.remove(key).is_some() {
                        *found += 1;
                    }
                }
                map.values_mut().for_each(|x| strip(x, found));
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(|x| strip(x, found)),
            _ => {}
        }
    }
    let mut found = 0;
    strip(&mut value, &mut found);
    assert!(found >= 2, "the project does not store subassemblies where expected ({found})");
    std::fs::write(&path, serde_json::to_string(&value).unwrap()).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert!(t.doc().subassemblies().is_empty());
    assert_eq!(t.component_tree().len(), 2);
    assert!(t.component_tree().iter().all(|r| r.depth == 0 && r.parent.is_none()));
    // And it can be demoted afresh.
    let _ = demote(&mut t, vec![key(a), key(b)], "Again");
}

/// A damaged project: a parent loop between subassemblies, a membership of a subassembly that does not exist,
/// a membership of a component that does not exist. Opening is clean (or an error), the tree is built
/// without hanging, and no row is listed twice.
#[test]
fn a_damaged_project_does_not_hang_or_list_rows_twice() {
    let mut s = Session::new();
    let (a, b) = (body(&mut s, 0.0), body(&mut s, 0.3));
    let one = demote(&mut s, vec![key(a)], "One");
    let two = demote(&mut s, vec![key(b)], "Two");
    let (_dir, path) = project(&mut s);
    let text = std::fs::read_to_string(&path).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
    fn find<'a>(v: &'a mut serde_json::Value, name: &str) -> Option<&'a mut serde_json::Value> {
        match v {
            serde_json::Value::Object(map) => {
                if map.contains_key(name) {
                    return map.get_mut(name);
                }
                map.values_mut().find_map(|x| find(x, name))
            }
            serde_json::Value::Array(items) => items.iter_mut().find_map(|x| find(x, name)),
            _ => None,
        }
    }
    {
        let subs = find(&mut value, "subassemblies").unwrap().as_array_mut().unwrap();
        // One's parent is Two and Two's parent is One: a loop.
        for entry in subs.iter_mut() {
            let id = entry["id"].clone();
            let other = if id == serde_json::to_value(one).unwrap() { two } else { one };
            entry["parent"] = serde_json::to_value(other).unwrap();
        }
    }
    {
        let placed = find(&mut value, "placed_in").unwrap().as_array_mut().unwrap();
        // A membership of a subassembly that is not there.
        let mut extra = placed[0].clone();
        extra[1] = serde_json::to_value(SubassemblyId(77)).unwrap();
        placed.push(extra);
    }
    let damaged = dir_file(&_dir, "damaged.gcp", &serde_json::to_string(&value).unwrap());
    let mut t = Session::new();
    if t.open(&damaged).is_err() {
        return; // refusing a damaged file is fine
    }
    let started = std::time::Instant::now();
    let tree = t.component_tree();
    assert!(
        started.elapsed().as_secs() < 5,
        "building the tree of a damaged project took {:?}",
        started.elapsed()
    );
    let mut keys: Vec<TreeKey> = tree.iter().map(|r| r.key).collect();
    let n = keys.len();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), n, "a row is listed twice");
    // The editing and moving APIs also return on it.
    std::hint::black_box(t.in_edit_scope(occ(a)));
    std::hint::black_box(t.pick_unit(occ(a)));
    drop(t.edit_component(sub(one)));
    drop(t.edit_subassemblies(SubassemblyOp::Move { members: vec![sub(one)], into: Some(two) }));
}

fn dir_file(dir: &tempfile::TempDir, name: &str, text: &str) -> std::path::PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, text).unwrap();
    path
}

// ---- Editing in place ----

/// Components outside the edit target are not picked: `pick_unit` gives nothing; the edit target moves with
/// Return, Return to Parent, Return to Top; a target that is dissolved or undone away counts as none.
#[test]
fn the_edit_target_follows_the_tree_and_survives_its_changes() {
    let mut s = Session::new();
    let (a, b, c) = (body(&mut s, 0.0), body(&mut s, 0.3), body(&mut s, 0.6));
    let inner = demote(&mut s, vec![key(a)], "Inner");
    let outer = demote(&mut s, vec![sub(inner), key(b)], "Outer");
    // Rows that do not exist cannot be edited.
    assert_eq!(s.edit_component(sub(SubassemblyId(66))), Err(EditError::BadValue));
    s.edit_component(sub(outer)).unwrap();
    assert!(s.in_edit_scope(occ(a)) && s.in_edit_scope(occ(b)) && !s.in_edit_scope(occ(c)));
    assert_eq!(s.pick_unit(occ(c)), None, "a component outside the target was picked");
    // Editing the same target again changes nothing (no stack growth that Return trips over).
    s.edit_component(sub(outer)).unwrap();
    s.return_edit();
    assert_eq!(s.edit_target(), None, "Return after a repeated edit goes back to the top, one step");

    // Dissolving the target: it counts as none and everything is in scope again.
    s.edit_component(sub(outer)).unwrap();
    s.edit_subassemblies(SubassemblyOp::Dissolve(outer)).unwrap();
    assert_eq!(s.edit_target(), None);
    assert!(s.in_edit_scope(occ(c)));
    // Undo brings the subassembly back, and with it the possibility to edit it again.
    assert!(s.undo());
    s.edit_component(sub(outer)).unwrap();
    assert_eq!(s.edit_target(), Some(sub(outer)));
    // Undoing the Demote that made the target away: none again.
    assert!(s.undo());
    assert_eq!(s.edit_target(), None);
    // Return to Parent and Return to Top at the top level do nothing and do not panic; Return with an
    // empty history stays at the top.
    s.return_to_parent();
    s.return_to_top();
    s.return_edit();
    s.return_edit();
    assert_eq!(s.edit_target(), None);
}

/// A thousand edits of the target do not grow without bound or panic, and Return still comes back.
#[test]
fn editing_in_place_many_times_stays_bounded() {
    let mut s = Session::new();
    let (a, b) = (body(&mut s, 0.0), body(&mut s, 0.3));
    for _ in 0..2000 {
        s.edit_component(key(a)).unwrap();
        s.edit_component(key(b)).unwrap();
    }
    for _ in 0..4001 {
        s.return_edit();
    }
    assert_eq!(s.edit_target(), None);
}

/// Selecting a component outside the edit target: the viewport goes through `pick_unit` (None outside), but
/// the session's own selection API is the other way in. Help (Edit components in place): the components
/// outside the one being edited are not available.
#[test]
fn selecting_a_component_outside_the_edit_target_is_refused() {
    let mut s = Session::new();
    let (a, c) = (body(&mut s, 0.0), body(&mut s, 0.6));
    let one = demote(&mut s, vec![key(a)], "One");
    s.edit_component(sub(one)).unwrap();
    assert!(!s.in_edit_scope(occ(c)));
    assert!(!s.select_occurrence(Some(occ(c))), "a component outside the edit target was selected");
    // Ground on it must not change it either.
    let before = s.doc().clone();
    drop(s.toggle_ground_selected());
    assert!(s.doc().same_content(&before), "an out-of-scope component was grounded");
}
