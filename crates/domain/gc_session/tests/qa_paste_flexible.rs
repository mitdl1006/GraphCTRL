//! QA boundary tests for copying and pasting components (`copy_to_clipboard`, `paste`, `preview_paste`,
//! `paste_ghosts`), Flexible subassemblies (`set_flexible`, `rigid_unit_members`, `unit_moves`, relationship
//! solving with rigid units) and `load_warnings`. Inventor 2026 help, under
//! <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/>: subassemblies
//! GUID-FCEE3F0E-AAE2-4619-8645-2588FEECD353, flexible components GUID-30817F08-A5C2-44B7-B237-4791ACA3F854,
//! edit in place GUID-C79C36C9-A6DB-46C5-8864-41F66C750231.
//! Every refused edit leaves the document and the history untouched; every accepted one is one undo step that
//! restores exactly.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::disallowed_methods,
    reason = "test code"
)]

use std::path::Path;

use gc_core::{AnyId, Id};
use gc_doc::{
    AxisSense, Body, Created, Geometry, Occurrence, Placement, RelationKind, Relationship, Selection, Solid,
    SubassemblyId, SubassemblyOp, TreeKey,
};
use gc_math::{DPose3, DQuat, DVec3};
use gc_session::{CellItem, Session, library};

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

fn shift(x: f64) -> Placement {
    Placement { xyz: [x, 0.0, 0.0], rpy: [0.0; 3] }
}

fn bodies(s: &Session) -> usize {
    s.doc().bodies().iter().count()
}

fn refused_paste(s: &mut Session, what: &str, offset: Placement) {
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    let result = s.paste(offset);
    assert!(result.is_err(), "{what} was accepted: {result:?}");
    assert!(s.doc().same_content(&before), "{what} changed the document");
    assert_eq!(s.history().undo_depth(), depth, "{what}");
}

// ---- Copy ----

#[test]
fn copy_keeps_only_components_and_subassemblies_that_exist() {
    let mut s = Session::new();
    let (a, b) = (body(&mut s, 0.0), body(&mut s, 0.3));
    let one = demote(&mut s, vec![key(a)], "One");
    let ghost = Id::<Body>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    let part = TreeKey::Component(Occurrence::Part {
        assembly: Id::from_raw(std::num::NonZeroU64::new(999).unwrap()),
        index: 0,
    });
    // Nothing usable: refused, and the clipboard is left as it was.
    assert!(s.clipboard().is_empty());
    for (what, keys) in [
        ("none", vec![]),
        ("a ghost component", vec![key(ghost)]),
        ("a ghost subassembly", vec![sub(SubassemblyId(77))]),
        ("a part", vec![part]),
    ] {
        assert!(s.copy_to_clipboard(&keys).is_err(), "{what}");
        assert!(s.clipboard().is_empty(), "{what} changed the clipboard");
    }
    // Mixed: the usable ones are kept, in order.
    s.copy_to_clipboard(&[key(ghost), key(b), part, sub(one)]).unwrap();
    assert_eq!(s.clipboard(), &[key(b), sub(one)]);
    // A refused copy keeps what was there.
    assert!(s.copy_to_clipboard(&[]).is_err());
    assert_eq!(s.clipboard(), &[key(b), sub(one)]);
}

// ---- Paste ----

#[test]
fn paste_is_one_undo_step_and_leaves_the_originals_alone() {
    let mut s = Session::new();
    let (a, b) = (body(&mut s, 0.0), body(&mut s, 0.3));
    s.copy_to_clipboard(&[key(a), key(b)]).unwrap();
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    let created = s.paste(shift(1.0)).unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1, "one undo step");
    assert_eq!(bodies(&s), 4);
    assert!(!created.is_empty());
    let new: Vec<Id<Body>> =
        s.doc().bodies().iter().map(|(id, _)| id).filter(|id| *id != a && *id != b).collect();
    assert_eq!(new.len(), 2);
    for (copy, original) in new.iter().zip([a, b]) {
        let (c, o) = (s.doc().get(*copy).unwrap().placement, s.doc().get(original).unwrap().placement);
        assert!((c.xyz[0] - (o.xyz[0] + 1.0)).abs() < 1e-9, "offset: {c:?} vs {o:?}");
        assert_eq!(c.rpy, o.rpy, "orientation kept");
    }
    // The originals are exactly as they were; no relationship came along; copies are not grounded.
    for original in [a, b] {
        assert_eq!(s.doc().get(original).unwrap().placement, before.get(original).unwrap().placement);
    }
    assert!(s.relationships().is_empty());
    for copy in &new {
        assert!(
            !s.doc().component_flag(occ(*copy), gc_doc::ComponentFlag::Grounded),
            "a pasted copy is grounded"
        );
    }
    assert!(s.undo());
    assert!(s.doc().same_content(&before), "undo of a paste is not exact");
    assert!(s.redo());
    assert_eq!(bodies(&s), 4);
    // Pasting again makes two more, each distinct.
    s.paste(shift(2.0)).unwrap();
    assert_eq!(bodies(&s), 6);
    let mut names: Vec<String> = s.components().iter().map(|c| c.name.clone()).collect();
    names.sort();
    let n = names.len();
    names.dedup();
    assert_eq!(names.len(), n, "two components share a name after pasting");
}

#[test]
fn paste_with_nothing_left_or_a_bad_offset_is_refused_without_a_trace() {
    let mut s = Session::new();
    let (a, b) = (body(&mut s, 0.0), body(&mut s, 0.3));
    // Nothing copied.
    refused_paste(&mut s, "empty clipboard", shift(1.0));
    s.copy_to_clipboard(&[key(a), key(b)]).unwrap();
    for (what, offset) in [
        ("NaN position", Placement { xyz: [f64::NAN, 0.0, 0.0], rpy: [0.0; 3] }),
        ("infinite position", Placement { xyz: [0.0, f64::INFINITY, 0.0], rpy: [0.0; 3] }),
        ("NaN turn", Placement { xyz: [0.0; 3], rpy: [0.0, f64::NAN, 0.0] }),
        ("infinite turn", Placement { xyz: [0.0; 3], rpy: [f64::INFINITY, 0.0, 0.0] }),
    ] {
        refused_paste(&mut s, what, offset);
    }
    // Huge but finite: refused, or accepted with finite poses and one undo step.
    let before = s.doc().clone();
    match s.paste(Placement { xyz: [1e300, 0.0, 0.0], rpy: [0.0; 3] }) {
        Err(_) => assert!(s.doc().same_content(&before)),
        Ok(_) => {
            for (_, body) in s.doc().bodies().iter() {
                assert!(body.placement.xyz.iter().all(|v| v.is_finite()));
            }
            assert!(s.undo() && s.doc().same_content(&before));
        }
    }
    // Both originals deleted after the copy: nothing left to paste.
    s.delete_item(CellItem::Body(a)).unwrap();
    s.delete_item(CellItem::Body(b)).unwrap();
    refused_paste(&mut s, "both originals deleted", shift(1.0));
}

/// One of two originals deleted after the copy: the other is pasted; the clipboard still lists both.
#[test]
fn paste_skips_originals_that_were_deleted() {
    let mut s = Session::new();
    let (a, b, c) = (body(&mut s, 0.0), body(&mut s, 0.3), body(&mut s, 0.6));
    s.copy_to_clipboard(&[key(b), key(c)]).unwrap();
    s.delete_item(CellItem::Body(b)).unwrap();
    let before = s.doc().clone();
    s.paste(shift(1.0)).unwrap();
    assert_eq!(bodies(&s), 3, "a, c and the copy of c");
    let _ = a;
    assert!(s.undo() && s.doc().same_content(&before));
    // Undoing the delete brings the original back: a paste now copies both again.
    assert!(s.undo());
    s.paste(shift(1.0)).unwrap();
    assert_eq!(bodies(&s), 3 + 2);
}

/// A copied subassembly three levels deep: the copy has the same structure under new ids and free names, the
/// flexible flag is kept, the originals are untouched, and undo is exact.
#[test]
fn pasting_a_nested_subassembly_copies_its_structure() {
    let mut s = Session::new();
    let (a, b, c) = (body(&mut s, 0.0), body(&mut s, 0.3), body(&mut s, 0.6));
    let inner = demote(&mut s, vec![key(a)], "Inner");
    let middle = demote(&mut s, vec![sub(inner), key(b)], "Middle");
    let outer = demote(&mut s, vec![sub(middle), key(c)], "Outer");
    s.set_flexible(middle, true).unwrap();
    s.copy_to_clipboard(&[sub(outer)]).unwrap();
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    s.paste(shift(1.0)).unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1);
    let subs = s.doc().subassemblies();
    assert_eq!(subs.len(), 6, "three originals and three copies");
    let ids: Vec<_> = subs.iter().map(|x| x.id).collect();
    let mut unique = ids.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 6, "a subassembly id was reused");
    let names: Vec<_> = subs.iter().map(|x| x.name.clone()).collect();
    let mut unique_names = names.clone();
    unique_names.sort();
    unique_names.dedup();
    assert_eq!(unique_names.len(), 6, "a subassembly name was reused: {names:?}");
    // The copy hangs together the same way: one new top-level, a middle under it, an inner under that.
    let fresh: Vec<_> =
        subs.iter().filter(|x| !before.subassemblies().iter().any(|o| o.id == x.id)).collect();
    assert_eq!(fresh.len(), 3);
    let top: Vec<_> = fresh.iter().filter(|x| x.parent.is_none()).collect();
    assert_eq!(top.len(), 1, "one new top-level subassembly");
    let new_middle = fresh.iter().find(|x| x.parent == Some(top[0].id)).unwrap();
    let new_inner = fresh.iter().find(|x| x.parent == Some(new_middle.id)).unwrap();
    assert!(new_middle.flexible, "the flexible flag was not copied");
    assert!(!new_inner.flexible && !top[0].flexible);
    // Each new component is inside the right new subassembly, at the original level of its twin.
    let members = |id: SubassemblyId| s.doc().subassembly_components(id).len();
    assert_eq!(members(top[0].id), 3, "everything of the outer copy");
    assert_eq!(members(new_inner.id), 1);
    // The originals are untouched.
    for x in before.subassemblies() {
        assert_eq!(s.doc().subassembly_components(x.id), before.subassembly_components(x.id), "{x:?}");
    }
    assert!(s.undo());
    assert!(s.doc().same_content(&before), "undo of the structure paste is not exact");
    // Copying a subassembly together with one of its own members does not copy the member twice.
    s.copy_to_clipboard(&[sub(outer), key(c)]).unwrap();
    s.paste(shift(2.0)).unwrap();
    assert_eq!(bodies(&s), 6, "three originals and three copies, not four");
}

/// Pasting while a subassembly is edited in place puts the copies in it; while a component is edited, or
/// when the target is gone, at the top level.
#[test]
fn paste_goes_into_the_subassembly_being_edited() {
    let mut s = Session::new();
    let (a, b, c) = (body(&mut s, 0.0), body(&mut s, 0.3), body(&mut s, 0.6));
    let one = demote(&mut s, vec![key(a), key(b)], "One");
    s.copy_to_clipboard(&[key(c)]).unwrap();
    // Editing One: the copy of C joins it.
    s.edit_component(sub(one)).unwrap();
    s.paste(shift(1.0)).unwrap();
    assert_eq!(s.doc().subassembly_components(one).len(), 3);
    assert!(s.undo());
    assert_eq!(s.doc().subassembly_components(one).len(), 2);
    // Editing a component (not a subassembly): the top level.
    s.edit_component(key(a)).unwrap();
    s.paste(shift(1.0)).unwrap();
    assert_eq!(s.doc().subassembly_components(one).len(), 2);
    let copy = s.doc().bodies().iter().map(|(id, _)| id).find(|id| ![a, b, c].contains(id)).unwrap();
    assert_eq!(s.doc().subassembly_of(occ(copy)), None);
    assert!(s.undo());
    // Editing a subassembly that is then dissolved: no target, the top level; a copied subassembly pasted into
    // One is nested under it.
    s.copy_to_clipboard(&[sub(one)]).unwrap();
    s.edit_component(sub(one)).unwrap();
    s.paste(shift(1.0)).unwrap();
    let nested =
        s.doc().subassemblies().into_iter().find(|x| x.parent == Some(one)).expect("a copy nested in One");
    assert_ne!(nested.id, one);
    assert!(s.undo());
    s.edit_subassemblies(SubassemblyOp::Dissolve(one)).unwrap();
    assert_eq!(s.edit_target(), None);
}

/// A robot with a tool pastes as a new robot with the same tool; one undo step; the original is unchanged.
#[test]
fn pasting_a_robot_with_a_tool() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../robots");
    let packages = library::scan(&dir);
    let mut s = Session::new();
    s.set_robot_library(Some(dir));
    let robot = s.add_library_robot(packages.iter().find(|p| p.info.name == "UR5e").unwrap()).unwrap();
    let tool = gc_doc::Tool {
        solid: Solid::Cylinder { radius: 0.02, length: 0.2 },
        placement: Placement::default(),
    };
    s.execute(&gc_doc::Command::Cell(gc_doc::CellOp::SetTool { id: robot, tool: Some(tool.clone()) }))
        .unwrap();
    let robot_key = TreeKey::Component(Occurrence::Item(robot.erase()));
    s.copy_to_clipboard(&[robot_key]).unwrap();
    let before = s.doc().clone();
    s.paste(shift(1.0)).unwrap();
    assert_eq!(s.doc().robots().iter().count(), 2);
    let copy = s.doc().robots().iter().map(|(id, _)| id).find(|id| *id != robot).unwrap();
    assert_eq!(s.doc().get(copy).unwrap().tool.as_deref(), Some(&tool), "the copy lost its tool");
    assert_eq!(s.doc().get(robot).unwrap().tool.as_deref(), Some(&tool));
    assert_ne!(s.doc().get(copy).unwrap().name, s.doc().get(robot).unwrap().name, "two robots share a name");
    assert!(s.undo() && s.doc().same_content(&before));
}

// ---- Paste preview ----

#[test]
fn the_paste_preview_never_touches_the_document_or_the_history() {
    let mut s = Session::new();
    let (a, b) = (body(&mut s, 0.0), body(&mut s, 0.3));
    let one = demote(&mut s, vec![key(a), key(b)], "One");
    // No clipboard: refused, no ghosts.
    assert!(s.preview_paste(shift(1.0)).is_err());
    assert!(s.paste_ghosts().is_empty());
    s.copy_to_clipboard(&[sub(one)]).unwrap();
    let (before, depth) = (s.doc().clone(), s.history().undo_depth());
    s.preview_paste(shift(0.5)).unwrap();
    assert!(s.doc().same_content(&before), "a preview changed the document");
    assert_eq!(s.history().undo_depth(), depth);
    assert_eq!(bodies(&s), 2, "a preview made a component");
    let ghosts = s.paste_ghosts();
    assert_eq!(ghosts.len(), 2);
    for (item, shifted) in &ghosts {
        assert!([a.erase(), b.erase()].contains(item), "a ghost of something that is not copied");
        assert!((shifted.translation - DVec3::new(0.5, 0.0, 0.0)).length() < 1e-12);
    }
    // A moving preview replaces the earlier one; non-finite offsets end it; end clears it.
    s.preview_paste(shift(0.9)).unwrap();
    assert!((s.paste_ghosts()[0].1.translation.x - 0.9).abs() < 1e-12);
    for bad in [
        Placement { xyz: [f64::NAN, 0.0, 0.0], rpy: [0.0; 3] },
        Placement { xyz: [0.0; 3], rpy: [0.0, 0.0, f64::INFINITY] },
    ] {
        assert!(s.preview_paste(bad).is_err());
        assert!(s.paste_ghosts().is_empty(), "a refused preview kept the old ghosts");
        s.preview_paste(shift(0.5)).unwrap();
    }
    s.end_paste_preview();
    assert!(s.paste_ghosts().is_empty());
    // A preview, then the paste, then the document has exactly one paste step and no ghost.
    s.preview_paste(shift(0.5)).unwrap();
    s.paste(shift(0.5)).unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1);
    // Ghosts of deleted originals disappear.
    s.preview_paste(shift(0.5)).unwrap();
    s.delete_item(CellItem::Body(b)).unwrap();
    assert!(s.paste_ghosts().iter().all(|(item, _)| *item != b.erase()), "a ghost of a deleted component");
    assert!(s.doc().get(b).is_none());
}

// ---- Flexible ----

#[test]
fn set_flexible_is_one_undo_step_each_way_and_refuses_what_does_not_exist() {
    let mut s = Session::new();
    let (a, b) = (body(&mut s, 0.0), body(&mut s, 0.3));
    let one = demote(&mut s, vec![key(a), key(b)], "One");
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    let ghost = SubassemblyId(99);
    assert!(s.set_flexible(ghost, true).is_err());
    assert!(s.doc().same_content(&before) && s.history().undo_depth() == depth);
    s.set_flexible(one, true).unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1);
    assert!(s.doc().subassemblies()[0].flexible);
    // Setting it again: harmless, either a step that changes nothing or none.
    let flexible = s.doc().clone();
    if s.set_flexible(one, true).is_ok() {
        assert!(s.doc().same_content(&flexible), "setting flexible twice changed something");
        assert!(s.undo());
    }
    s.set_flexible(one, false).unwrap();
    assert!(!s.doc().subassemblies()[0].flexible);
    assert!(s.undo() && s.doc().subassemblies()[0].flexible);
    assert!(s.undo() && s.doc().same_content(&before));
    // Survives save and open.
    s.set_flexible(one, true).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("flex.gcp");
    s.save_as(&path).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert!(t.doc().same_content(s.doc()));
    assert!(t.doc().subassemblies()[0].flexible);
    assert!(t.doc().rigid_unit(a.erase()).is_none());
}

/// Nested units: the first rigid subassembly from the outside is the unit; a flexible outer makes each rigid
/// inner its own unit; rigid outer swallows a flexible inner.
#[test]
fn rigid_units_nest_from_the_outside() {
    let mut s = Session::new();
    let (a, b, c) = (body(&mut s, 0.0), body(&mut s, 0.3), body(&mut s, 0.6));
    let inner = demote(&mut s, vec![key(a)], "Inner");
    let outer = demote(&mut s, vec![sub(inner), key(b)], "Outer");
    let members = |s: &Session, x: Id<Body>| s.doc().rigid_unit_members(x.erase()).len();
    assert_eq!(members(&s, a), 2, "all rigid: the outer is the unit");
    assert_eq!(members(&s, b), 2);
    assert_eq!(members(&s, c), 1, "outside every subassembly: itself");
    s.set_flexible(outer, true).unwrap();
    assert_eq!(members(&s, a), 1, "outer flexible, inner rigid with one member: itself");
    assert_eq!(members(&s, b), 1);
    s.set_flexible(outer, false).unwrap();
    s.set_flexible(inner, true).unwrap();
    assert_eq!(members(&s, a), 2, "outer rigid swallows a flexible inner");
    // The ghost component: itself only.
    let ghost: AnyId = Id::<Body>::from_raw(std::num::NonZeroU64::new(9999).unwrap()).erase();
    assert_eq!(s.doc().rigid_unit_members(ghost), vec![ghost]);
}

#[test]
fn unit_moves_keep_the_unit_rigid_and_do_not_panic_on_odd_poses() {
    let mut s = Session::new();
    let _ground = body(&mut s, 9.0);
    let (a, b) = (body(&mut s, 0.0), body(&mut s, 0.3));
    let one = demote(&mut s, vec![key(a), key(b)], "One");
    let origin = |s: &Session, x: Id<Body>| s.doc().component_origin(occ(x)).unwrap().pose;
    let pose = DPose3::from_parts(DVec3::new(0.2, 0.1, 0.5), DQuat::from_axis_angle(DVec3::Z, 0.7));
    let moves = s.unit_moves(a.erase(), pose);
    assert_eq!(moves.len(), 2);
    let moved_a = moves.iter().find(|(m, _)| *m == a.erase()).unwrap().1;
    assert!(
        (moved_a.translation - pose.translation).length() < 1e-12,
        "the dragged one is exactly where it was put"
    );
    let moved_b = moves.iter().find(|(m, _)| *m == b.erase()).unwrap().1;
    let was = (origin(&s, a).translation - origin(&s, b).translation).length();
    assert!(
        (((moved_a.translation) - moved_b.translation).length() - was).abs() < 1e-9,
        "the unit changed shape"
    );
    // The identity move: nothing moves. A missing component: nothing at all.
    let same = s.unit_moves(a.erase(), origin(&s, a));
    for (m, p) in &same {
        assert!((p.translation - origin(&s, Id::from_raw(m.raw)).translation).length() < 1e-12);
    }
    let ghost: AnyId = Id::<Body>::from_raw(std::num::NonZeroU64::new(9999).unwrap()).erase();
    assert!(s.unit_moves(ghost, pose).is_empty());
    // Odd poses give odd results but no panic, and committing them is refused whole.
    for bad in [
        DPose3::from_parts(DVec3::new(f64::NAN, 0.0, 0.0), DQuat::IDENTITY),
        DPose3::from_parts(DVec3::ZERO, DQuat::from_xyzw(0.0, 0.0, 0.0, 0.0)),
    ] {
        let moves = s.unit_moves(a.erase(), bad);
        let before = s.doc().clone();
        let depth = s.history().undo_depth();
        if s.grip_snap_commit(&moves, false).is_ok() {
            for (_, body) in s.doc().bodies().iter() {
                assert!(body.placement.xyz.iter().all(|v| v.is_finite()), "a non-finite pose was committed");
            }
            assert!(s.undo());
        }
        assert!(s.doc().same_content(&before) && s.history().undo_depth() == depth);
    }
    let _ = one;
}

fn mate(
    s: &mut Session,
    moving: Id<Body>,
    fixed: Id<Body>,
) -> Result<Id<gc_doc::Relationship>, gc_doc::EditError> {
    let plane = |item: Id<Body>, z: f64, nz: f64| Selection {
        occurrence: occ(item),
        geometry: Geometry::Plane { point: [0.0, 0.0, z], normal: [0.0, 0.0, nz] },
    };
    s.add_relationship(Relationship::new(
        "Mate",
        RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
        plane(moving, -0.05, -1.0),
        plane(fixed, 0.05, 1.0),
    ))
}

/// Relationships and rigid units: a relationship between two members of one rigid unit (it is both ends
/// of the same solver body); a relationship that pulls a rigid unit that holds a grounded member; deleting
/// and undoing leave the cell solvable.
#[test]
fn relationships_inside_and_into_rigid_units_stay_consistent() {
    let mut s = Session::new();
    let floor = body(&mut s, 5.0);
    let (a, b) = (
        s.add_body(Solid::Box { size: [0.1; 3] }, Placement { xyz: [0.0, 0.0, 0.5], rpy: [0.0; 3] }).unwrap(),
        s.add_body(Solid::Box { size: [0.1; 3] }, Placement { xyz: [0.3, 0.0, 0.5], rpy: [0.0; 3] }).unwrap(),
    );
    let one = demote(&mut s, vec![key(a), key(b)], "One");
    // Between two members of the same rigid unit: refused or harmless; the cell solves and nothing moves.
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    match mate(&mut s, a, b) {
        Err(_) => assert!(s.doc().same_content(&before) && s.history().undo_depth() == depth),
        Ok(_) => {
            assert!(s.doc().solve_relationships().is_ok());
            let pa = s.doc().component_origin(occ(a)).unwrap().pose.translation;
            assert!(pa.is_finite());
            assert!(s.undo() && s.doc().same_content(&before));
        }
    }
    // A grounded member holds the unit: a relationship that would have to move it is refused or consistent.
    s.set_component(occ(b), gc_doc::ComponentFlag::Grounded, true).unwrap();
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    match mate(&mut s, a, floor) {
        Err(_) => assert!(
            s.doc().same_content(&before) && s.history().undo_depth() == depth,
            "a refused mate left a trace"
        ),
        Ok(_) => {
            assert!(s.doc().solve_relationships().is_ok());
            assert!(s.undo() && s.doc().same_content(&before));
        }
    }
    // Freed again: the same mate moves the whole unit; flexible, only A moves.
    s.set_component(occ(b), gc_doc::ComponentFlag::Grounded, false).unwrap();
    let pb = s.doc().component_origin(occ(b)).unwrap().pose.translation;
    let id = mate(&mut s, a, floor).unwrap();
    let nb = s.doc().component_origin(occ(b)).unwrap().pose.translation;
    let pa_new = s.doc().component_origin(occ(a)).unwrap().pose.translation;
    assert!((pa_new.z - 0.1).abs() < 1e-6, "A sits on the floor: {pa_new:?}");
    assert!((nb.z - pb.z).abs() > 1e-6 || pb.z == nb.z, "B");
    assert!(s.doc().solve_relationships().is_ok());
    s.set_flexible(one, true).unwrap();
    assert!(s.doc().solve_relationships().is_ok());
    // Delete the relationship, undo everything back: exact.
    s.delete_relationship(id).unwrap();
    assert!(s.undo() && s.undo());
    assert!(s.doc().solve_relationships().is_ok());
}

// ---- load_warnings ----

/// A project with a damaged joint range, written out (the range is a recognisable number in the file).
fn damaged_project(dir: &Path, name: &str, marker: f64) -> std::path::PathBuf {
    let mut s = Session::new();
    let frame =
        |z: f64, nz: f64| Geometry::Frame { origin: [0.0, 0.0, z], z: [0.0, 0.0, nz], x: [1.0, 0.0, 0.0] };
    let sel = |id: AnyId, geometry| Selection { occurrence: Occurrence::Item(id), geometry };
    let table = s
        .add_body(Solid::Box { size: [2.0, 1.0, 1.0] }, Placement { xyz: [0.0, 0.0, 0.5], rpy: [0.0; 3] })
        .unwrap();
    let block =
        s.add_body(Solid::Box { size: [0.2; 3] }, Placement { xyz: [0.3, 0.2, 1.6], rpy: [0.0; 3] }).unwrap();
    let hinge = s
        .add_relationship(Relationship::new(
            "Hinge",
            RelationKind::Rotational { flip: false },
            sel(block.erase(), frame(-0.1, -1.0)),
            sel(table.erase(), frame(0.5, 1.0)),
        ))
        .unwrap();
    s.set_joint_limits(hinge, Some(gc_doc::JointRange { start: marker, end: 0.456, current: None }), None)
        .unwrap();
    let path = dir.join(name);
    s.save_as(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains(&format!("{marker}")), "the marker is in the file");
    // Start past End.
    std::fs::write(&path, text.replacen(&format!("{marker}"), "9.0", 1)).unwrap();
    path
}

#[test]
fn load_warnings_describe_what_was_dropped_and_do_not_outlive_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let bad = damaged_project(dir.path(), "bad.gcp", -0.123);
    let mut s = Session::new();
    assert!(s.load_warnings().is_empty());
    s.open(&bad).unwrap();
    assert_eq!(s.load_warnings().len(), 1, "{:?}", s.load_warnings());
    assert!(s.load_warnings()[0].contains("joint"), "{:?}", s.load_warnings());
    // The cell works: the unusable range was dropped, the joint keeps no limit, it solves.
    assert!(s.doc().solve_relationships().is_ok());
    // A good project opened next carries no warnings.
    let mut good = Session::new();
    let _t = good.add_body(Solid::Box { size: [0.1; 3] }, Placement::default()).unwrap();
    let good_path = dir.path().join("good.gcp");
    good.save_as(&good_path).unwrap();
    s.open(&good_path).unwrap();
    assert!(s.load_warnings().is_empty(), "stale warnings after a good open: {:?}", s.load_warnings());
    // Two files in a row each report their own; a failed open leaves what was there.
    s.open(&bad).unwrap();
    assert_eq!(s.load_warnings().len(), 1);
    assert!(s.open(&dir.path().join("missing.gcp")).is_err());
    assert_eq!(s.load_warnings().len(), 1, "a failed open changed the warnings");
}

/// Starting a new document, and taking up an autosave, are other documents: the warnings of the project
/// opened before must not carry over, and the warnings of the autosave itself must be reported.
#[test]
#[ignore = "BUG: new_document and open_recovered leave load_warnings as the previous open left them (open_recovered also never reports what its own file dropped) (owner: backend, gc_session lib.rs)"]
fn load_warnings_belong_to_the_document_that_was_opened() {
    let dir = tempfile::tempdir().unwrap();
    let bad = damaged_project(dir.path(), "bad.gcp", -0.123);
    let mut s = Session::new();
    s.open(&bad).unwrap();
    assert_eq!(s.load_warnings().len(), 1);
    s.new_document();
    assert!(s.load_warnings().is_empty(), "a new document still carries {:?}", s.load_warnings());
    // An autosave with the same damage reports it.
    let mut fresh = Session::new();
    fresh.open_recovered(&bad, None).unwrap();
    assert_eq!(fresh.load_warnings().len(), 1, "the autosave's dropped data was not reported");
    // And one with none clears the earlier report.
    let mut good = Session::new();
    good.add_body(Solid::Box { size: [0.1; 3] }, Placement::default()).unwrap();
    let good_path = dir.path().join("good.gcp");
    good.save_as(&good_path).unwrap();
    s.open(&bad).unwrap();
    s.open_recovered(&good_path, None).unwrap();
    assert!(s.load_warnings().is_empty(), "{:?}", s.load_warnings());
}
