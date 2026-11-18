//! QA boundary tests for who owns a sketch made in an assembly (`Command::Owned`, `sketch_owner`,
//! `sketches_of`). Inventor 2026 help, under <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/>:
//! Sketches in assemblies GUID-2C180C3A-2064-4671-B1C2-F74B383B5D39, Edit components in place
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
use gc_doc::{
    Body, CellOp, Command, Created, Occurrence, Placement, Solid, SubassemblyId, SubassemblyOp, TreeKey,
};
use gc_session::{CellItem, Session};
use gc_sketch::Space;

fn body(s: &mut Session, x: f64) -> Id<Body> {
    s.add_body(Solid::Box { size: [0.1; 3] }, Placement { xyz: [x, 0.0, 0.0], rpy: [0.0; 3] }).unwrap()
}

fn key(b: Id<Body>) -> TreeKey {
    TreeKey::Component(Occurrence::Item(b.erase()))
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

/// Where the sketch's plane is in the world.
fn plane_origin(s: &Session, sketch: Id<gc_doc::Sketch>) -> gc_math::DVec3 {
    let (Space::Planar(frame) | Space::Spatial(frame)) = s.doc().get(sketch).unwrap().space;
    s.doc().frame_world_pose(Id::from_raw(frame.raw)).unwrap().translation
}

/// A sketch made while editing `target` in place (and leaving the sketch so the session is back in the tree).
fn make_sketch(s: &mut Session, target: Option<TreeKey>, plane: &str) -> Id<gc_doc::Sketch> {
    match target {
        Some(t) => s.edit_component(t).unwrap(),
        None => s.return_to_top(),
    }
    let sketch = s.create_sketch(plane).unwrap();
    s.finish_sketch();
    sketch
}

// ---- Owners ----

#[test]
fn sketches_belong_to_the_component_being_edited_in_place() {
    let mut s = Session::new();
    let (a, b) = (body(&mut s, 0.0), body(&mut s, 0.5));
    let top = make_sketch(&mut s, None, "XY");
    let depth = s.history().undo_depth();
    let in_a = make_sketch(&mut s, Some(key(a)), "XY");
    let in_b = make_sketch(&mut s, Some(key(b)), "XZ");
    assert_eq!(s.history().undo_depth(), depth + 2, "one undo step per sketch");
    assert_eq!(s.sketch_owner(top), None);
    assert_eq!(s.sketch_owner(in_a), Some(key(a)));
    assert_eq!(s.sketch_owner(in_b), Some(key(b)));
    assert_eq!(s.sketches_of(None), vec![top]);
    assert_eq!(s.sketches_of(Some(key(a))), vec![in_a]);
    assert_eq!(s.sketches_of(Some(key(b))), vec![in_b]);
    // Undo removes the sketch and its owner entry (no stale record); redo brings both back.
    let before = s.doc().clone();
    assert!(s.undo());
    assert!(s.doc().get(in_b).is_none() && s.sketches_of(Some(key(b))).is_empty());
    assert!(s.redo());
    assert!(s.doc().same_content(&before));
    assert_eq!(s.sketch_owner(in_b), Some(key(b)));
}

/// A sketch of a component made in place sits on that component's own origin plane, so it follows the
/// component when it is moved (the lead's decision; help: sketches in assemblies belong to the component).
#[test]
fn an_owned_xy_sketch_follows_its_component() {
    let mut s = Session::new();
    let (a, b) = (body(&mut s, 0.0), body(&mut s, 0.5));
    let in_a = make_sketch(&mut s, Some(key(a)), "XY");
    let in_b = make_sketch(&mut s, Some(key(b)), "XY");
    let (pa, pb) = (plane_origin(&s, in_a), plane_origin(&s, in_b));
    assert!((pa.x - 0.0).abs() < 1e-9 && (pb.x - 0.5).abs() < 1e-9, "{pa:?} {pb:?}");
    // Move A: its sketch goes with it, B's stays.
    let mut moved = s.doc().get(a).unwrap().clone();
    moved.placement.xyz = [1.5, 0.2, 0.3];
    s.execute(&Command::Cell(CellOp::SetBody { id: a, body: moved })).unwrap();
    let after = plane_origin(&s, in_a);
    assert!(
        (after.x - 1.5).abs() < 1e-9 && (after.y - 0.2).abs() < 1e-9 && (after.z - 0.3).abs() < 1e-9,
        "{after:?}"
    );
    assert!((plane_origin(&s, in_b) - pb).length() < 1e-9, "the other component's sketch moved");
    // Rotated: the plane's orientation follows too (the XZ sketch of B turns with B).
    let mut turned = s.doc().get(b).unwrap().clone();
    turned.placement.rpy = [0.0, 0.0, std::f64::consts::FRAC_PI_2];
    s.execute(&Command::Cell(CellOp::SetBody { id: b, body: turned })).unwrap();
    assert!(s.doc().get(in_b).is_some());
    // Undo of the moves restores the sketch's place exactly.
    assert!(s.undo() && s.undo());
    assert!((plane_origin(&s, in_a) - pa).length() < 1e-9);
}

/// Editing a subassembly in place: its sketches are owned by it (on the world plane; a subassembly has no
/// origin of its own).
#[test]
fn a_subassembly_owns_the_sketches_made_while_it_is_edited() {
    let mut s = Session::new();
    let (a, b) = (body(&mut s, 0.0), body(&mut s, 0.5));
    let one = demote(&mut s, vec![key(a), key(b)], "One");
    let owned = make_sketch(&mut s, Some(sub(one)), "XY");
    assert_eq!(s.sketch_owner(owned), Some(sub(one)));
    assert_eq!(s.sketches_of(Some(sub(one))), vec![owned]);
    assert!(s.sketches_of(None).is_empty());
}

// ---- Owner dissolved or deleted ----

/// The owner goes away: the sketch is the top level's (its owner row no longer exists), nothing dangles; Undo
/// brings the owner back and with it the ownership; it survives save and open.
#[test]
fn a_dissolved_owner_hands_its_sketches_to_the_top_and_undo_gives_them_back() {
    let mut s = Session::new();
    let a = body(&mut s, 0.0);
    let one = demote(&mut s, vec![key(a)], "One");
    let owned = make_sketch(&mut s, Some(sub(one)), "XY");
    let with_owner = s.doc().clone();
    s.edit_subassemblies(SubassemblyOp::Dissolve(one)).unwrap();
    assert_eq!(s.sketch_owner(owned), None, "the sketch still names a dissolved subassembly");
    assert_eq!(s.sketches_of(None), vec![owned]);
    assert!(s.doc().get(owned).is_some(), "dissolving the owner deleted the sketch");
    // A subassembly made later (a fresh id) does not inherit it.
    let two = demote(&mut s, vec![key(a)], "Two");
    assert_ne!(two, one, "the id of a dissolved subassembly was reused");
    assert_eq!(s.sketch_owner(owned), None, "the new subassembly took over the old one's sketch");
    assert!(s.undo() && s.undo());
    assert!(s.doc().same_content(&with_owner));
    assert_eq!(s.sketch_owner(owned), Some(sub(one)));

    // Save and open keep the ownership; the reopened document is the same.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("owners.gcp");
    s.save_as(&path).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert!(t.doc().same_content(s.doc()));
    assert_eq!(t.sketch_owner(owned), Some(sub(one)));
    assert_eq!(t.sketches_of(Some(sub(one))), vec![owned]);
}

/// Deleting a component deletes the sketches it owns in the same step, as Inventor deletes a part's
/// sketches with the part (lead decision, 2026-11-18); one undo restores the owner, the sketch and the
/// ownership exactly. (Before: the delete was refused because the sketch's frame refers to the owner.)
#[test]
fn deleting_a_component_deletes_the_sketches_it_owns() {
    let mut s = Session::new();
    let (a, b) = (body(&mut s, 0.0), body(&mut s, 0.5));
    let owned = make_sketch(&mut s, Some(key(a)), "XY");
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    s.delete_item(CellItem::Body(a)).unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1, "one undo step");
    assert!(s.doc().get(owned).is_none(), "the owned sketch stayed");
    assert!(s.sketches_of(Some(key(a))).is_empty());
    // The other component's delete does not touch it.
    assert!(s.undo());
    assert!(s.doc().same_content(&before));
    assert_eq!(s.sketch_owner(owned), Some(key(a)));
    s.delete_item(CellItem::Body(b)).unwrap();
    assert!(s.doc().get(owned).is_some());
    assert!(s.undo());
    // Sketch first, then the component: both go; undo of both restores everything.
    s.execute(&Command::DeleteSketch { id: owned }).unwrap();
    s.delete_item(CellItem::Body(a)).unwrap();
    assert!(s.undo() && s.undo());
    assert!(s.doc().same_content(&before));
    assert_eq!(s.sketch_owner(owned), Some(key(a)));
}

// ---- Command::Owned directly ----

#[test]
fn owned_with_a_missing_owner_or_a_failing_command_leaves_no_trace() {
    let mut s = Session::new();
    let a = body(&mut s, 0.0);
    let one = demote(&mut s, vec![key(a)], "One");
    let frame = s.doc().origin_plane("XY").unwrap();
    let create = Command::CreateSketch { name: "Direct".into(), frame, spatial: false };
    let owned =
        |owner: Option<TreeKey>, command: Command| Command::Owned { owner, command: Box::new(command) };
    let ghost_owner = Some(sub(SubassemblyId(99)));
    let ghost_component = Some(TreeKey::Component(Occurrence::Item(
        Id::<Body>::from_raw(std::num::NonZeroU64::new(9999).unwrap()).erase(),
    )));
    for (what, command) in [
        ("ghost subassembly", owned(ghost_owner, create.clone())),
        ("ghost component", owned(ghost_component, create.clone())),
        (
            "failing command",
            owned(
                Some(sub(one)),
                Command::CreateSketch {
                    name: "Ghost frame".into(),
                    frame: Id::from_raw(std::num::NonZeroU64::new(9999).unwrap()),
                    spatial: false,
                },
            ),
        ),
        (
            "unknown plane",
            owned(
                Some(sub(one)),
                Command::CreateSketchOn {
                    name: "X".into(),
                    source: gc_doc::PlaneSource::Component {
                        occurrence: ghost_component
                            .map(|k| match k {
                                TreeKey::Component(o) => o,
                                TreeKey::Subassembly(_) => unreachable!(),
                            })
                            .unwrap(),
                        plane: gc_doc::OriginPlane::Xy,
                    },
                },
            ),
        ),
    ] {
        let before = s.doc().clone();
        let depth = s.history().undo_depth();
        let result = s.execute(&command);
        assert!(result.is_err(), "{what} was accepted: {result:?}");
        assert!(s.doc().same_content(&before), "{what} left a trace");
        assert_eq!(s.history().undo_depth(), depth, "{what}");
    }
    // A command that makes no sketch needs no owner: accepted, nothing recorded.
    let before = s.doc().clone();
    let created = s
        .execute(&owned(
            Some(sub(one)),
            Command::AddParam { name: "p".into(), source: "5 mm".into(), angle: false },
        ))
        .unwrap();
    assert!(!created.is_empty());
    assert!(s.undo());
    assert!(s.doc().same_content(&before));
    // A sketch made twice under different owners has one owner entry each, never two for one sketch.
    let direct = s.execute(&owned(Some(sub(one)), create)).unwrap();
    let Some(Created::Sketch(sketch)) = direct.into_iter().find(|c| matches!(c, Created::Sketch(_))) else {
        panic!("no sketch");
    };
    assert_eq!(s.sketch_owner(sketch), Some(sub(one)));
    // Nested Owned: the outer owner wins or the inner does; there is exactly one.
    let nested = s
        .execute(&owned(
            Some(sub(one)),
            owned(Some(key(a)), Command::CreateSketch { name: "Nested".into(), frame, spatial: false }),
        ))
        .unwrap();
    let Some(Created::Sketch(inner)) = nested.into_iter().find(|c| matches!(c, Created::Sketch(_))) else {
        panic!("no sketch");
    };
    let owners: Vec<_> = [None, Some(sub(one)), Some(key(a))]
        .into_iter()
        .filter(|o| s.sketches_of(*o).contains(&inner))
        .collect();
    assert_eq!(owners.len(), 1, "a sketch with several owners: {owners:?}");
}

// ---- Old files ----

/// A project saved before sketch owners existed has no `sketch_owners`: it opens, every sketch is the top
/// level's.
#[test]
fn an_old_project_without_sketch_owners_opens_with_top_level_sketches() {
    let mut s = Session::new();
    let a = body(&mut s, 0.0);
    let owned = make_sketch(&mut s, Some(key(a)), "XY");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.gcp");
    s.save_as(&path).unwrap();
    let mut value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    fn strip(v: &mut serde_json::Value, found: &mut usize) {
        match v {
            serde_json::Value::Object(map) => {
                if map.remove("sketch_owners").is_some() {
                    *found += 1;
                }
                map.values_mut().for_each(|x| strip(x, found));
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(|x| strip(x, found)),
            _ => {}
        }
    }
    let mut found = 0;
    strip(&mut value, &mut found);
    assert_eq!(found, 1, "the project does not store sketch owners where expected");
    std::fs::write(&path, serde_json::to_string(&value).unwrap()).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert_eq!(t.sketch_owner(owned), None);
    assert_eq!(t.sketches_of(None), vec![owned]);
}
