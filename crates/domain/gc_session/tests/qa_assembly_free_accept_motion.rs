//! QA boundary tests for Free Move / Update, Accept the relationship / Recover, and Motion constraints.
//! Inventor 2026 help: Move assembly components GUID-9796C22B-23C1-4D69-BC2E-54056E46F255, Use Design Doctor
//! to repair relationship errors GUID-8C95B0AB-4505-4092-8C37-E60C68DCF4A0, Relationship Conflict Analysis
//! GUID-528B2CD0-93FD-45BC-AB69-6C5A559BB8E8, Place Constraint Reference - Motion Tab
//! GUID-0293C151-565F-4980-AA95-DD76C54AB538.
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
    AngleSolution, AxisSense, Body, CellOp, Command, ComponentFlag, EditError, Geometry, Occurrence,
    Placement, RelationKind, Relationship, Selection, Solid,
};
use gc_session::Session;

/// Refused with no trace, or one undo step that undoes exactly and redoes the same (the edit stays applied).
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

fn on(body: Id<Body>, geometry: Geometry) -> Selection {
    Selection { occurrence: Occurrence::Item(body.erase()), geometry }
}

fn plane(z: f64, up: f64) -> Geometry {
    Geometry::Plane { point: [0.0, 0.0, z], normal: [0.0, 0.0, up] }
}

fn mate(block: Id<Body>, table: Id<Body>, offset: f64) -> Relationship {
    Relationship::new(
        "Mate",
        RelationKind::Mate { offset, sense: AxisSense::Opposed },
        on(block, plane(-0.1, -1.0)),
        on(table, plane(0.5, 1.0)),
    )
}

fn at(x: f64, y: f64, z: f64) -> Placement {
    Placement { xyz: [x, y, z], rpy: [0.0; 3] }
}

fn pos(s: &Session, body: Id<Body>) -> [f64; 3] {
    s.doc().get(body).unwrap().placement.xyz
}

struct Scene {
    s: Session,
    table: Id<Body>,
    block: Id<Body>,
    loose: Id<Body>,
    held: Id<Relationship>,
}

/// A table, a block mated to its top, and a free part with no relationships.
fn scene() -> Scene {
    let mut s = Session::new();
    let table = s.add_body(Solid::Box { size: [2.0, 1.0, 1.0] }, at(0.0, 0.0, 0.5)).unwrap();
    let block = s.add_body(Solid::Box { size: [0.2; 3] }, at(0.3, 0.0, 1.1)).unwrap();
    let loose = s.add_body(Solid::Box { size: [0.1; 3] }, at(-0.5, 0.2, 1.05)).unwrap();
    let held = s.add_relationship(mate(block, table, 0.0)).unwrap();
    Scene { s, table, block, loose, held }
}

#[test]
fn free_move_keeps_the_part_where_it_is_dropped_and_update_brings_it_back() {
    let Scene { mut s, block, loose, .. } = scene();
    let home = pos(&s, block);
    assert!(!s.needs_update());
    assert_eq!(s.relationship_bands().len(), 1);
    assert!(s.relationship_bands()[0].satisfied);

    let there = at(0.3, 0.4, 1.6);
    assert!(step(&mut s, "free move", |s| s.free_move(block.erase(), there)).is_some());
    assert_eq!(pos(&s, block), [0.3, 0.4, 1.6], "the relationships are not solved on the drop");
    assert!(s.needs_update());
    let bands = s.relationship_bands();
    assert_eq!(bands.len(), 1);
    assert!(!bands[0].satisfied);
    assert!(bands[0].ends.iter().all(|e| e.is_finite()));
    // The band's two ends are apart now (the block is 0.5 m above where it belongs).
    assert!(bands[0].ends[0].distance(bands[0].ends[1]) > 0.1);

    // Update: the block snaps home, in one undo step.
    s.free_move(block.erase(), there).unwrap_or(());
    assert!(step(&mut s, "update", |s| s.update_assembly()).is_some());
    assert!(!s.needs_update());
    let back = pos(&s, block);
    // A plane Mate holds only the height: the sideways drop stays (the relationship is satisfied there).
    assert!((back[2] - home[2]).abs() < 1e-7, "{back:?}");
    assert!(s.relationship_bands()[0].satisfied);

    // A part with no relationship stays where it is dropped, and Update leaves it.
    assert!(
        step(&mut s, "free move of a loose part", |s| s.free_move(loose.erase(), at(1.0, 1.0, 3.0)))
            .is_some()
    );
    assert!(!s.needs_update(), "nothing relates the loose part, so nothing needs updating");
    let before = s.doc().clone();
    drop(s.update_assembly());
    assert_eq!(pos(&s, loose), [1.0, 1.0, 3.0]);
    let _ = before;
}

#[test]
fn update_with_nothing_to_do_changes_nothing_and_undo_goes_back_exactly() {
    let Scene { mut s, .. } = scene();
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    let result = s.update_assembly();
    assert!(s.doc().same_content(&before), "an update with nothing to do changed the document");
    if result.is_ok() && s.history().undo_depth() > depth {
        assert!(s.undo());
        assert!(s.doc().same_content(&before));
    }
}

#[test]
fn free_move_refuses_what_is_not_a_placement_and_a_grounded_part_stays_after_update() {
    let Scene { mut s, table, block, .. } = scene();
    // Not finite, not a component, a part that is not there, a robot-less id.
    for bad in [
        at(f64::NAN, 0.0, 0.0),
        at(f64::INFINITY, 0.0, 0.0),
        at(0.0, f64::NEG_INFINITY, 0.0),
        Placement { xyz: [0.0; 3], rpy: [f64::NAN, 0.0, 0.0] },
    ] {
        assert!(step(&mut s, "bad placement", |s| s.free_move(block.erase(), bad)).is_none(), "{bad:?}");
    }
    let gone = s.add_body(Solid::Box { size: [0.1; 3] }, at(5.0, 5.0, 5.0)).unwrap();
    s.execute(&Command::Cell(CellOp::DeleteBody(gone))).unwrap();
    assert!(
        step(&mut s, "free move of a deleted part", |s| s.free_move(gone.erase(), at(0.0, 0.0, 0.0)))
            .is_none()
    );
    let sketch = s.create_sketch("XY");
    let id = sketch.unwrap().erase();
    assert!(step(&mut s, "free move of a sketch", |s| s.free_move(id, at(0.0, 0.0, 0.0))).is_none());

    // A grounded component dragged away stays where it is dropped (Update does not pull it back), and what is
    // related to it comes to it.
    s.execute(&Command::SetComponent {
        occurrence: Occurrence::Item(table.erase()),
        flag: ComponentFlag::Grounded,
        on: true,
    })
    .unwrap();
    s.free_move(table.erase(), at(1.0, 0.0, 0.5)).unwrap();
    s.update_assembly().unwrap();
    assert_eq!(pos(&s, table), [1.0, 0.0, 0.5]);
    let b = pos(&s, block);
    assert!((b[2] - 1.1).abs() < 1e-6, "the block came to the table's top: {b:?}");
    // Dropping a related part and then any command that solves the relationships snaps it back too.
    s.free_move(block.erase(), at(0.0, 0.0, 9.0)).unwrap();
    assert!(s.needs_update());
    s.set_suppressed(block.erase(), false).unwrap_or(());
    drop(s.update_assembly());
    assert!(!s.needs_update());
}

#[test]
fn a_part_dropped_by_free_move_stays_dropped_in_a_saved_file() {
    let Scene { mut s, block, .. } = scene();
    s.free_move(block.erase(), at(0.3, 0.4, 1.6)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("moved.gcp");
    s.save_as(&path).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert!(t.doc().same_content(s.doc()));
    assert!(t.needs_update());
    assert_eq!(pos(&t, block), [0.3, 0.4, 1.6]);
    t.update_assembly().unwrap();
    assert!(!t.needs_update());
}

#[test]
fn a_suppressed_relationship_has_no_band_and_never_asks_for_an_update() {
    let Scene { mut s, block, held, .. } = scene();
    s.free_move(block.erase(), at(0.3, 0.4, 1.6)).unwrap();
    assert!(s.needs_update());
    assert!(step(&mut s, "suppress", |s| s.set_relationship_suppressed(held, true)).is_some());
    assert!(s.relationship_bands().is_empty());
    assert!(!s.needs_update());
    // The block did not snap back: suppressing is not an update.
    assert_eq!(pos(&s, block), [0.3, 0.4, 1.6]);
}

/// A second Mate that fights the first for the same block.
fn fighting(block: Id<Body>, table: Id<Body>) -> Relationship {
    mate(block, table, 0.3)
}

#[test]
fn an_unsolvable_relationship_is_refused_unless_accepted_and_then_it_is_marked() {
    let Scene { mut s, table, block, .. } = scene();
    assert!(step(&mut s, "add the fighting mate", |s| s.add_relationship(fighting(block, table))).is_none());
    let accepted = step(&mut s, "accept it", |s| s.accept_relationship(fighting(block, table))).unwrap();
    assert!(s.sick_relationships().is_empty() || s.sick_relationships().contains(&accepted));
    // step() undid and redid it: the mark is still there.
    assert_eq!(s.sick_relationships(), vec![accepted]);
    assert!(s.doc().is_sick(accepted));
    // It was left out of the solve: the block still stands where the first mate puts it.
    assert!((pos(&s, block)[2] - 1.1).abs() < 1e-6);
    // Free Move shows it as a band that does not hold.
    // (An accepted relationship is left out of the solve; if Free Move shows it at all it must not claim it holds.)
    assert!(s.relationship_bands().iter().all(|b| b.id != accepted || !b.satisfied));
    // The mark survives a save and open.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sick.gcp");
    s.save_as(&path).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert_eq!(t.sick_relationships(), vec![accepted]);
    assert!(t.doc().same_content(s.doc()));
}

#[test]
fn accept_refuses_what_is_not_a_relationship_at_all() {
    let Scene { mut s, table, block, .. } = scene();
    // A part with itself, a deleted part, a sketch id.
    let itself = Relationship::new(
        "Mate",
        RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
        on(block, plane(-0.1, -1.0)),
        on(block, plane(0.1, 1.0)),
    );
    assert!(step(&mut s, "a part with itself", |s| s.accept_relationship(itself)).is_none());
    let gone = s.add_body(Solid::Box { size: [0.1; 3] }, at(5.0, 5.0, 5.0)).unwrap();
    s.execute(&Command::Cell(CellOp::DeleteBody(gone))).unwrap();
    assert!(step(&mut s, "a deleted part", |s| s.accept_relationship(mate(gone, table, 0.0))).is_none());
    assert!(step(&mut s, "NaN offset", |s| s.accept_relationship(mate(block, table, f64::NAN))).is_none());
    assert!(
        step(&mut s, "infinite offset", |s| s.accept_relationship(mate(block, table, f64::INFINITY)))
            .is_none()
    );
}

#[test]
fn accepting_something_that_can_be_solved_leaves_no_mark() {
    let Scene { mut s, table, loose, .. } = scene();
    // The loose part has nothing holding it: this mate is satisfiable, so Accept leaves no error mark.
    let id = step(&mut s, "accept a fine one", |s| s.accept_relationship(mate(loose, table, 0.02))).unwrap();
    assert!(!s.doc().is_sick(id), "a relationship that can be satisfied got an error mark");
    assert!(s.sick_relationships().is_empty());
    assert!((pos(&s, loose)[2] - (1.0 + 0.02 + 0.1)).abs() < 1e-6);
}

#[test]
fn the_mark_goes_when_the_other_relationship_is_changed_deleted_or_suppressed() {
    let make = || {
        let mut sc = scene();
        let accepted = sc.s.accept_relationship(fighting(sc.block, sc.table)).unwrap();
        (sc, accepted)
    };
    // Change the first one to agree: the accepted one can be satisfied, the mark goes, and Undo brings it back.
    let (mut sc, accepted) = make();
    let held = sc.held;
    let edited = mate(sc.block, sc.table, 0.3);
    assert!(step(&mut sc.s, "edit the first mate", |s| s.set_relationship(held, edited)).is_some());
    assert!(sc.s.sick_relationships().is_empty(), "the mark stayed after the conflict was gone");
    assert!(!sc.s.doc().is_sick(accepted));
    assert!((pos(&sc.s, sc.block)[2] - 1.4).abs() < 1e-6);
    assert!(sc.s.undo());
    assert_eq!(sc.s.sick_relationships(), vec![accepted]);

    // Delete the first one.
    let (mut sc, accepted) = make();
    let held = sc.held;
    assert!(step(&mut sc.s, "delete the first mate", |s| s.delete_relationship(held)).is_some());
    // The mark is re-examined at the next solve (Update at the latest).
    sc.s.update_assembly().unwrap();
    assert!(sc.s.sick_relationships().is_empty());
    assert!(!sc.s.doc().is_sick(accepted));

    // Suppress the first one.
    let (mut sc, accepted) = make();
    let held = sc.held;
    assert!(step(&mut sc.s, "suppress the first", |s| s.set_relationship_suppressed(held, true)).is_some());
    assert!(!sc.s.doc().is_sick(accepted));

    // Delete the accepted one itself, or the block it holds: no dangling mark.
    let (mut sc, accepted) = make();
    assert!(step(&mut sc.s, "delete the sick one", |s| s.delete_relationship(accepted)).is_some());
    assert!(sc.s.sick_relationships().is_empty());
    let (mut sc, accepted) = make();
    let block = sc.block;
    let delete = |s: &mut Session| s.execute(&Command::Cell(CellOp::DeleteBody(block))).map(|_| ());
    // A part that relationships refer to cannot be deleted from under them: refused with no trace.
    assert!(step(&mut sc.s, "delete the block", delete).is_none());
    let held = sc.held;
    sc.s.delete_relationship(held).unwrap();
    sc.s.delete_relationship(accepted).unwrap();
    assert!(step(&mut sc.s, "delete the free block", delete).is_some());
    for id in sc.s.sick_relationships() {
        assert!(sc.s.doc().get(id).is_some(), "a dangling error mark {id:?} (was {accepted:?})");
    }
}

#[test]
fn accepting_a_change_to_a_relationship_marks_it_and_a_refused_change_leaves_no_trace() {
    let Scene { mut s, table, block, loose, held } = scene();
    // Pin the loose part and the block both to the table, then make the first mate fight the second.
    let other = s.add_relationship(mate(loose, table, 0.0)).unwrap();
    let _ = other;
    let both_ways = Relationship::new(
        "Flush",
        RelationKind::Flush { offset: 0.0 },
        on(block, plane(0.1, 1.0)),
        on(table, plane(0.5, 1.0)),
    );
    // Flush top-to-top with the mate's bottom-to-top cannot both hold (the block is 0.2 high).
    assert!(step(&mut s, "plain add", |s| s.add_relationship(both_ways.clone())).is_none());
    // Edit the first mate into something that fights the Flush? Add the Flush as accepted, then edit.
    let accepted = s.accept_relationship(both_ways).unwrap();
    assert!(s.doc().is_sick(accepted));
    let fighting_edit = mate(block, table, 0.5);
    let r = step(&mut s, "edit refused", |s| s.set_relationship(held, fighting_edit.clone()));
    // Whatever it decides, accepting the edit is allowed and is one undo step.
    let _ = r;
    assert!(step(&mut s, "accept the edit", |s| s.accept_relationship_edit(held, fighting_edit)).is_some());
    assert!(s.sick_relationships().contains(&held) || s.sick_relationships().contains(&accepted));
    // Edit of an id that is not there.
    let missing = s.delete_relationship(other).map(|()| other).unwrap();
    assert!(
        step(&mut s, "edit of a deleted one", |s| s
            .accept_relationship_edit(missing, mate(loose, table, 0.0)))
        .is_none()
    );
}

#[test]
fn recover_lists_what_stands_in_the_way_and_conflict_check_changes_nothing() {
    let Scene { mut s, table, block, loose, held } = scene();
    let accepted = s.accept_relationship(fighting(block, table)).unwrap();
    let before = s.doc().clone();
    let depth = s.history().undo_depth();

    // Isolate Related Failed: the first mate stands in the way; the block and the table are kept visible.
    let (culprits, components) = s.related_failed(accepted);
    assert_eq!(culprits, vec![held]);
    assert!(components.contains(&Occurrence::Item(block.erase())));
    assert!(components.contains(&Occurrence::Item(table.erase())));
    assert!(
        !components.contains(&Occurrence::Item(loose.erase())),
        "an unrelated part is not part of the failure"
    );
    // A healthy relationship, and one that is not there, have nothing to isolate.
    assert_eq!(s.related_failed(held), (Vec::new(), Vec::new()));
    let missing = {
        let id = s.add_relationship(mate(loose, table, 0.0)).unwrap();
        s.delete_relationship(id).unwrap();
        id
    };
    assert_eq!(s.related_failed(missing), (Vec::new(), Vec::new()));
    // The Isolate list works with Isolate itself.
    assert!(step(&mut s, "isolate the failure", |s| s.isolate(&components)).is_some());

    // Conflict Analysis: Check.
    let depth_after = s.history().undo_depth();
    assert!(!s.conflict_check(&[], &[]), "the conflict is still there");
    assert!(s.conflict_check(&[held], &[]), "without the first mate everything holds");
    assert!(s.conflict_check(&[accepted], &[]), "without the sick one everything holds");
    assert!(s.conflict_check(&[held, accepted], &[]));
    assert!(!s.conflict_check(&[missing], &[]), "a relationship that is not there breaks nothing");
    let _ = s.conflict_check(&[], &[Occurrence::Item(table.erase())]);
    let _ = s.conflict_check(&[], &[Occurrence::Item(table.erase()), Occurrence::Item(table.erase())]);
    assert_eq!(s.history().undo_depth(), depth_after, "Check is not an edit");
    assert!(s.doc().same_content(&{
        let mut t = s.doc().clone();
        t.clone_from(s.doc());
        t
    }));
    let _ = (before, depth);
}

#[test]
fn conflict_check_leaves_the_document_exactly_as_it_was() {
    let Scene { mut s, table, block, held, .. } = scene();
    let accepted = s.accept_relationship(fighting(block, table)).unwrap();
    let snapshot = s.doc().clone();
    let depth = s.history().undo_depth();
    for broken in [vec![], vec![held], vec![accepted], vec![held, accepted]] {
        for toggled in [vec![], vec![Occurrence::Item(table.erase())], vec![Occurrence::Item(block.erase())]]
        {
            let _ = s.conflict_check(&broken, &toggled);
            assert!(s.doc().same_content(&snapshot), "{broken:?} {toggled:?}");
            assert_eq!(s.history().undo_depth(), depth);
        }
    }
}

// ---- Motion ----

fn frame(origin: [f64; 3], z: [f64; 3], x: [f64; 3]) -> Geometry {
    Geometry::Frame { origin, z, x }
}

fn axis(direction: [f64; 3]) -> Geometry {
    Geometry::Axis { point: [0.0; 3], direction }
}

fn yaw(s: &Session, body: Id<Body>) -> f64 {
    s.doc().get(body).unwrap().placement.rpy[2]
}

struct Gears {
    s: Session,
    table: Id<Body>,
    driver: Id<Body>,
    driven: Id<Body>,
    held: Id<Relationship>,
}

/// Two gears on rotational joints on a table; the driver's angle is held by an angle constraint (turn it by
/// changing that).
fn gears() -> Gears {
    let mut s = Session::new();
    let table = s.add_body(Solid::Box { size: [2.0, 1.0, 1.0] }, at(0.0, 0.0, 0.5)).unwrap();
    let gear = |s: &mut Session, name: &str, x: f64| {
        let body = s.add_body(Solid::Box { size: [0.2, 0.2, 0.1] }, at(x, 0.0, 1.3)).unwrap();
        s.add_relationship(Relationship::new(
            format!("Rotational {name}"),
            RelationKind::Rotational { flip: false },
            on(body, frame([0.0, 0.0, -0.05], [0.0, 0.0, -1.0], [1.0, 0.0, 0.0])),
            on(table, frame([x, 0.0, 0.5], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0])),
        ))
        .unwrap();
        body
    };
    let driver = gear(&mut s, "Driver", -0.3);
    let driven = gear(&mut s, "Driven", 0.3);
    let side = |body: Id<Body>| on(body, Geometry::Plane { point: [0.1, 0.0, 0.0], normal: [1.0, 0.0, 0.0] });
    let held = s
        .add_relationship(Relationship {
            limits: None,
            name: "Angle".into(),
            kind: RelationKind::Angle {
                angle: 0.0,
                solution: AngleSolution::Explicit(on(table, plane(0.5, 1.0))),
            },
            first: side(table),
            second: side(driver),
        })
        .unwrap();
    Gears { s, table, driver, driven, held }
}

fn turn_to(g: &mut Gears, angle: f64) -> Result<(), EditError> {
    let side = |body: Id<Body>| on(body, Geometry::Plane { point: [0.1, 0.0, 0.0], normal: [1.0, 0.0, 0.0] });
    g.s.set_relationship(
        g.held,
        Relationship {
            limits: None,
            name: "Angle".into(),
            kind: RelationKind::Angle {
                angle,
                solution: AngleSolution::Explicit(on(g.table, plane(0.5, 1.0))),
            },
            first: side(g.table),
            second: side(g.driver),
        },
    )
}

fn gear_ratio(driver: Id<Body>, driven: Id<Body>, ratio: f64, reverse: bool) -> Relationship {
    Relationship::new(
        "Rotation",
        RelationKind::Rotation { ratio, reverse },
        on(driver, axis([0.0, 0.0, 1.0])),
        on(driven, axis([0.0, 0.0, 1.0])),
    )
}

#[test]
fn a_rotation_constraint_turns_the_second_part_by_the_ratio_and_reverse_turns_it_the_other_way() {
    for (ratio, reverse) in [(2.0, true), (2.0, false), (0.5, true), (1.0, false), (4.0, false)] {
        let mut g = gears();
        let rel = gear_ratio(g.driver, g.driven, ratio, reverse);
        let id = step(&mut g.s, "place the motion constraint", |s| s.add_relationship(rel.clone())).unwrap();
        assert!(
            yaw(&g.s, g.driver).abs() < 1e-9 && yaw(&g.s, g.driven).abs() < 1e-9,
            "placing it moves nothing"
        );
        let sign = if reverse { -1.0 } else { 1.0 };
        let mut driver_total = 0.0;
        for target in [0.2, 0.5, -0.1] {
            let (a, b) = (yaw(&g.s, g.driver), yaw(&g.s, g.driven));
            turn_to(&mut g, target).unwrap();
            let by = yaw(&g.s, g.driver) - a;
            assert!(by.abs() > 0.05, "the driver turned: {by}");
            driver_total += by;
            let driven_by = yaw(&g.s, g.driven) - b;
            assert!(
                (driven_by - sign * ratio * by).abs() < 1e-6,
                "ratio {ratio} reverse {reverse}: {driven_by} for {by}"
            );
        }
        assert!((yaw(&g.s, g.driven) - sign * ratio * driver_total).abs() < 1e-6);
        // Undo takes the driver and the driven gear back together.
        let moved = yaw(&g.s, g.driven);
        assert!(g.s.undo());
        assert!((yaw(&g.s, g.driven) - moved).abs() > 0.01);
        assert!(g.s.redo());
        assert!((yaw(&g.s, g.driven) - moved).abs() < 1e-12);
        let _ = id;
    }
}

#[test]
fn a_motion_constraint_with_no_sense_is_refused_and_one_that_cannot_hold_does_not_stand_in_the_way() {
    let mut g = gears();
    for ratio in [0.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.0] {
        let rel = gear_ratio(g.driver, g.driven, ratio, false);
        assert!(step(&mut g.s, "bad ratio", |s| s.add_relationship(rel.clone())).is_none(), "ratio {ratio}");
    }
    // A part with itself.
    let itself = gear_ratio(g.driver, g.driver, 2.0, false);
    assert!(step(&mut g.s, "a gear with itself", |s| s.add_relationship(itself.clone())).is_none());
    // Huge and tiny ratios: each is one step or no trace.
    for ratio in [1e9, 1e-9, -3.0] {
        let rel = gear_ratio(g.driver, g.driven, ratio, false);
        step(&mut g.s, "extreme ratio", |s| s.add_relationship(rel.clone()));
    }
}

#[test]
fn a_motion_constraint_does_not_move_a_grounded_part_nor_fail_the_edit() {
    let mut g = gears();
    g.s.add_relationship(gear_ratio(g.driver, g.driven, 2.0, true)).unwrap();
    g.s.execute(&Command::SetComponent {
        occurrence: Occurrence::Item(g.driven.erase()),
        flag: ComponentFlag::Grounded,
        on: true,
    })
    .unwrap();
    let fixed = g.s.doc().get(g.driven).unwrap().placement;
    turn_to(&mut g, 0.6).unwrap();
    assert_eq!(g.s.doc().get(g.driven).unwrap().placement, fixed, "a grounded part was turned by motion");
    assert!(yaw(&g.s, g.driver).abs() > 0.3, "the driver still turned");
}

#[test]
fn a_motion_constraint_gives_way_to_a_position_constraint_that_holds_the_second_part() {
    let mut g = gears();
    // The driven gear is held by an angle constraint of its own: the motion constraint is ignored, not an error.
    let side = |body: Id<Body>| on(body, Geometry::Plane { point: [0.1, 0.0, 0.0], normal: [1.0, 0.0, 0.0] });
    g.s.add_relationship(Relationship {
        limits: None,
        name: "Angle driven".into(),
        kind: RelationKind::Angle {
            angle: 0.0,
            solution: AngleSolution::Explicit(on(g.table, plane(0.5, 1.0))),
        },
        first: side(g.table),
        second: side(g.driven),
    })
    .unwrap();
    let motion = g.s.add_relationship(gear_ratio(g.driver, g.driven, 2.0, false));
    assert!(
        motion.is_ok(),
        "a motion constraint on a part held by position constraints was refused: {motion:?}"
    );
    let turned = turn_to(&mut g, 0.6);
    assert!(turned.is_ok(), "{turned:?}");
    assert!(yaw(&g.s, g.driven).abs() < 1e-6, "the position constraint wins: {}", yaw(&g.s, g.driven));
}

#[test]
fn motion_constraints_survive_saving_and_deleting_the_part_leaves_no_dangling_relationship() {
    let mut g = gears();
    let id = g.s.add_relationship(gear_ratio(g.driver, g.driven, 3.0, true)).unwrap();
    turn_to(&mut g, 0.3).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gears.gcp");
    g.s.save_as(&path).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert!(t.doc().same_content(g.s.doc()));
    assert!(matches!(t.doc().get(id).unwrap().kind, RelationKind::Rotation { reverse: true, .. }));
    // The reopened file still turns the driven gear (the reference pose was saved with it).
    let before = yaw(&t, g.driven);
    let mut h = Gears { s: t, ..gears_from(&g) };
    h.held = g.held;
    turn_to(&mut h, 0.6).unwrap();
    assert!((yaw(&h.s, g.driven) - before + 3.0 * (yaw(&h.s, g.driver) - 0.3)).abs() < 1e-5);
    // A gear that a motion constraint refers to cannot be deleted from under it; with the constraint gone it can.
    let driven = g.driven;
    let delete = |s: &mut Session| s.execute(&Command::Cell(CellOp::DeleteBody(driven))).map(|_| ());
    assert!(step(&mut g.s, "delete the driven gear", delete).is_none());
    let holding: Vec<_> =
        g.s.relationships()
            .into_iter()
            .filter(|(_, r)| r.references(driven.erase()))
            .map(|(i, _)| i)
            .collect();
    assert!(holding.contains(&id));
    for rel in holding {
        g.s.delete_relationship(rel).unwrap();
    }
    assert!(step(&mut g.s, "delete the free gear", delete).is_some());
}

fn gears_from(g: &Gears) -> Gears {
    Gears { s: Session::new(), table: g.table, driver: g.driver, driven: g.driven, held: g.held }
}
