//! Regression guard for large cells: 200 components and 300 relationships built, saved, reopened and
//! undone. Wall-clock time is never the pass condition for speed (it varies by machine); the test asserts
//! what must stay true (counts, an exact save/open round trip, exact undo and redo) and only a very generous
//! upper bound that catches a blow-up (a quadratic or exponential step), and prints the times for the
//! report. Compare against docs/KNOWN_ISSUES.md (relationship build cost) when a number moves.
//! Run with `cargo test -p gc_session --test qa_large_cell -- --nocapture`.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::disallowed_methods,
    clippy::print_stdout,
    reason = "test code and a report"
)]

use std::time::{Duration, Instant};

use gc_core::Id;
use gc_doc::{
    AxisSense, Body, Geometry, Occurrence, Placement, RelationKind, Relationship, Selection, Solid,
};
use gc_session::Session;

/// A bound that only a blow-up reaches, in any CI machine.
const BLOW_UP: Duration = Duration::from_secs(120);

fn placed(xyz: [f64; 3]) -> Placement {
    Placement { xyz, rpy: [0.0; 3] }
}

fn plane(body: Id<Body>, point: [f64; 3], normal: [f64; 3]) -> Selection {
    Selection { occurrence: Occurrence::Item(body.erase()), geometry: Geometry::Plane { point, normal } }
}

fn relate(s: &mut Session, name: &str, kind: RelationKind, first: Selection, second: Selection) {
    s.add_relationship(Relationship::new(name, kind, first, second))
        .unwrap_or_else(|e| panic!("{name}: {e:?}"));
}

struct Cell {
    session: Session,
    components: usize,
    relationships: usize,
    build: Duration,
}

/// A table and 66 stacks of three boxes: A on the table (and flushed to the table's mid plane), B on A, C
/// on B; the first 36 stacks also flush C. 1 + 3 * 66 = 199 components... plus one spare box = 200;
/// 66 * 4 + 36 = 300 relationships. Every body hangs from the grounded table in at most three steps, so the
/// solver sees many small groups, not one long chain (a long chain is a separate known issue).
fn build() -> Cell {
    let mut s = Session::new();
    let table = s.add_body(Solid::Box { size: [40.0, 40.0, 1.0] }, placed([0.0, 0.0, 0.5])).unwrap();
    let started = Instant::now();
    let mut relationships = 0;
    let mut components = 1;
    for i in 0..66 {
        let x = f64::from(i % 10) * 0.6 - 3.0;
        let y = f64::from(i / 10) * 0.6 - 2.0;
        let a = s.add_body(Solid::Box { size: [0.2; 3] }, placed([x, y, 1.1])).unwrap();
        let b = s.add_body(Solid::Box { size: [0.2; 3] }, placed([x, y, 1.3])).unwrap();
        let c = s.add_body(Solid::Box { size: [0.2; 3] }, placed([x, y, 1.5])).unwrap();
        components += 3;
        let mate = RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed };
        let bottom = |b: Id<Body>| plane(b, [0.0, 0.0, -0.1], [0.0, 0.0, -1.0]);
        let top = |b: Id<Body>, z: f64| plane(b, [0.0, 0.0, z], [0.0, 0.0, 1.0]);
        relate(&mut s, "A on table", mate, bottom(a), top(table, 0.5));
        relate(&mut s, "B on A", mate, bottom(b), top(a, 0.1));
        relate(&mut s, "C on B", mate, bottom(c), top(b, 0.1));
        // A is held along X by a flush with the table's mid plane (offset = its distance from it).
        relate(
            &mut s,
            "A flush",
            RelationKind::Flush { offset: x },
            plane(a, [0.0; 3], [1.0, 0.0, 0.0]),
            plane(table, [0.0; 3], [1.0, 0.0, 0.0]),
        );
        relationships += 4;
        if i < 36 {
            relate(
                &mut s,
                "C flush",
                RelationKind::Flush { offset: x },
                plane(c, [0.0; 3], [1.0, 0.0, 0.0]),
                plane(table, [0.0; 3], [1.0, 0.0, 0.0]),
            );
            relationships += 1;
        }
    }
    s.add_body(Solid::Box { size: [0.2; 3] }, placed([5.0, 5.0, 1.1])).unwrap();
    components += 1;
    Cell { session: s, components, relationships, build: started.elapsed() }
}

fn timed<T>(label: &str, f: impl FnOnce() -> T) -> (T, Duration) {
    let started = Instant::now();
    let value = f();
    let took = started.elapsed();
    println!("{label}: {:.1} ms", took.as_secs_f64() * 1e3);
    assert!(took < BLOW_UP, "{label} took {took:?}: something is quadratic or worse");
    (value, took)
}

#[test]
fn a_large_cell_builds_saves_reopens_and_undoes_exactly() {
    let cell = build();
    println!(
        "build ({} components, {} relationships): {:.1} ms",
        cell.components,
        cell.relationships,
        cell.build.as_secs_f64() * 1e3
    );
    assert!(cell.build < BLOW_UP, "building took {:?}", cell.build);
    let Cell { mut session, components, relationships, .. } = cell;
    assert_eq!(components, 200);
    assert_eq!(relationships, 300);
    assert_eq!(session.doc().bodies().iter().count(), components);
    assert_eq!(session.relationships().len(), relationships);

    // Every body is in a defined state; the ones in the stacks lost freedom.
    let ((), _) = timed("status of every component", || {
        let statuses = session.component_statuses();
        assert_eq!(statuses.len(), components, "a status per component");
    });

    // Save and open: an exact round trip.
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("large.gcproj");
    let (saved, _) = timed("save", || {
        session.save_as(&file).unwrap();
        session.doc().clone()
    });
    let size = std::fs::metadata(&file).unwrap().len();
    println!("file size: {} KiB", size / 1024);
    assert!(size < 50 * 1024 * 1024, "a 200-component project is {size} bytes");
    let mut reopened = Session::new();
    let (_, _) = timed("open", || {
        reopened.open(&file).unwrap();
    });
    assert!(reopened.doc().same_content(&saved), "the reopened project differs from what was saved");
    assert_eq!(reopened.relationships().len(), relationships);
    // Saving the reopened project again gives the same bytes (stable output).
    let again = dir.path().join("again.gcproj");
    reopened.save_as(&again).unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), std::fs::read(&again).unwrap(), "saving twice differs");

    // Undo everything the build did, step by step, then redo it all.
    let steps = session.history().undo_depth();
    println!("undo steps: {steps}");
    // The history keeps the last 200 steps (gc_doc History), however many the build took.
    assert_eq!(steps, 200, "the build made {} steps; the history keeps 200", components - 1 + relationships);
    let (undone, _) = timed("undo all", || {
        let mut n = 0;
        while session.undo() {
            n += 1;
        }
        n
    });
    assert_eq!(undone, steps);
    assert!(
        session.doc().bodies().iter().count() < components,
        "undo took the last relationships and bodies away"
    );
    assert!(!session.doc().same_content(&saved));
    let (redone, _) = timed("redo all", || {
        let mut n = 0;
        while session.redo() {
            n += 1;
        }
        n
    });
    assert_eq!(redone, steps);
    assert!(session.doc().same_content(&saved), "redo of everything is not the saved state");
}

/// One more relationship in a full cell, one undo, one component move: the cost of a single edit stays far
/// below the cost of building everything (an edit must not re-solve the world every time).
#[test]
fn single_edits_in_a_large_cell_stay_cheap() {
    let Cell { mut session, build, components, .. } = build();
    let per_relationship = build.as_secs_f64() / 300.0;
    let table = session.doc().bodies().iter().next().map(|(id, _)| id).unwrap();
    let extra = session.add_body(Solid::Box { size: [0.2; 3] }, placed([-5.0, -5.0, 1.1])).unwrap();
    let (_, add) = timed("one more relationship", || {
        relate(
            &mut session,
            "extra",
            RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
            plane(extra, [0.0, 0.0, -0.1], [0.0, 0.0, -1.0]),
            plane(table, [0.0, 0.0, 0.5], [0.0, 0.0, 1.0]),
        );
    });
    let (_, undo) = timed("undo it", || assert!(session.undo()));
    let (_, redo) = timed("redo it", || assert!(session.redo()));
    let (count, _) = timed("status of every component", || session.component_statuses().len());
    assert_eq!(count, components + 1);
    println!(
        "per relationship during build {:.2} ms; one more in the full cell {:.2} ms; undo {:.2} ms; redo {:.2} ms",
        per_relationship * 1e3,
        add.as_secs_f64() * 1e3,
        undo.as_secs_f64() * 1e3,
        redo.as_secs_f64() * 1e3
    );
    // Not a speed assertion: a single edit may not cost more than the whole build did.
    assert!(add < build.max(Duration::from_millis(50)), "one edit costs more than the whole build");
}

// ---- Long chains (docs/KNOWN_ISSUES.md: mated chains used to blow up; fixed by backend dd8b494) ----

mod chain {
    use super::*;
    use gc_doc::{CellOp, Command, ComponentFlag, EditError};

    pub(super) fn world_z(s: &Session, b: Id<Body>) -> f64 {
        s.doc().component_origin(Occurrence::Item(b.erase())).unwrap().pose.translation.z
    }

    pub(super) fn world_x(s: &Session, b: Id<Body>) -> f64 {
        s.doc().component_origin(Occurrence::Item(b.erase())).unwrap().pose.translation.x
    }

    pub(super) struct Chain {
        pub(super) s: Session,
        pub(super) bodies: Vec<Id<Body>>,
        pub(super) mates: Vec<Id<gc_doc::Relationship>>,
        pub(super) build: Duration,
        /// Keeps the project folder alive.
        pub(super) _dir: tempfile::TempDir,
    }

    pub(super) fn mate(next: Id<Body>, previous: Id<Body>) -> Relationship {
        Relationship::new(
            "Mate",
            RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
            plane(next, [0.0, 0.0, -0.1], [0.0, 0.0, -1.0]),
            plane(previous, [0.0, 0.0, 0.1], [0.0, 0.0, 1.0]),
        )
    }

    pub(super) fn side(a: Id<Body>, b: Id<Body>, offset: f64, name: &str) -> Relationship {
        Relationship::new(
            name,
            RelationKind::Flush { offset },
            plane(a, [0.1, 0.0, 0.0], [1.0, 0.0, 0.0]),
            plane(b, [0.1, 0.0, 0.0], [1.0, 0.0, 0.0]),
        )
    }

    /// `n` boxes of 0.2 m stacked on a grounded base, each mated to the one below.
    pub(super) fn build(n: usize) -> Chain {
        let mut s = Session::new();
        let base = s.add_body(Solid::Box { size: [0.2; 3] }, placed([0.0, 0.0, 0.1])).unwrap();
        s.set_component(Occurrence::Item(base.erase()), ComponentFlag::Grounded, true).unwrap();
        let mut bodies = vec![base];
        let mut mates = Vec::new();
        let started = Instant::now();
        for k in 1..n {
            let next =
                s.add_body(Solid::Box { size: [0.2; 3] }, placed([0.0, 0.0, 0.1 + 0.2 * k as f64])).unwrap();
            let id = s.add_relationship(mate(next, *bodies.last().unwrap())).unwrap();
            bodies.push(next);
            mates.push(id);
        }
        let build = started.elapsed();
        // The history keeps 200 steps and a chain of 100 uses 199: save and reopen for an empty history, so
        // that every undo in the tests below reaches back to this exact state (and the round trip is checked).
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("chain.gcproj");
        s.save_as(&file).unwrap();
        let built = s.doc().clone();
        s.open(&file).unwrap();
        assert!(s.doc().same_content(&built), "the chain does not survive save and open");
        assert_eq!(s.history().undo_depth(), 0);
        Chain { s, bodies, mates, build, _dir: dir }
    }

    pub(super) fn all_stacked(c: &Chain, offsets: &[(usize, f64)]) {
        // Body k sits at 0.1 + 0.2 k + the sum of the offsets of the mates below it.
        for (k, b) in c.bodies.iter().enumerate() {
            let extra: f64 = offsets.iter().filter(|(m, _)| *m < k).map(|(_, o)| o).sum();
            let want = 0.1 + 0.2 * k as f64 + extra;
            let got = world_z(&c.s, *b);
            assert!((got - want).abs() < 1e-6, "body {k}: z {got}, wanted {want}");
        }
    }

    pub(super) fn is_unsolvable(e: &EditError) -> bool {
        matches!(e, EditError::Unsolvable { .. } | EditError::Conflicting(_) | EditError::Redundant(_))
    }

    pub(super) fn edit(c: &Chain, index: usize, offset: f64) -> Relationship {
        let rel = c.s.relationships().into_iter().find(|(id, _)| *id == c.mates[index]).unwrap().1.clone();
        let RelationKind::Mate { sense, .. } = rel.kind else { panic!("a mate") };
        Relationship { kind: RelationKind::Mate { offset, sense }, ..rel }
    }

    pub(super) fn set(
        s: &mut Session,
        id: Id<gc_doc::Relationship>,
        relationship: Relationship,
    ) -> Result<(), EditError> {
        s.execute(&Command::Cell(CellOp::SetRelationship { id, relationship })).map(|_| ())
    }
}

/// 100 components in a mated chain build within the blow-up bound (it took minutes before the fix), every
/// body sits on the one below, and the status of all of them is available at once.
#[test]
fn a_hundred_component_chain_builds_and_stays_stacked() {
    let c = chain::build(100);
    println!("chain of 100: {:.1} ms", c.build.as_secs_f64() * 1e3);
    assert!(c.build < BLOW_UP, "a 100-component chain took {:?}", c.build);
    chain::all_stacked(&c, &[]);
    assert_eq!(c.s.relationships().len(), 99);
    assert_eq!(c.s.component_statuses().len(), 100);
    // A plane-on-plane mate leaves three freedoms (slide in X and Y, turn about Z), no more.
    for b in &c.bodies[1..] {
        assert_eq!(c.s.component_dof(b.erase()).map(|d| d.total), Some(3), "a stacked body's freedom");
    }
    assert_eq!(c.s.component_dof(c.bodies[0].erase()).map(|d| d.total), Some(0), "the base is grounded");
}

/// A relationship at the end of the chain that cannot hold with it is refused whole — nothing changes, one
/// diagnosis names the relationships in the way — and a second try gives the same answer.
#[test]
fn a_conflicting_mate_at_the_end_of_a_chain_is_refused_and_diagnosed() {
    let mut c = chain::build(100);
    let top = *c.bodies.last().unwrap();
    let base = c.bodies[0];
    // The top's upper face onto the base's lower face: the chain says they are 19.8 m apart.
    let pinned = Relationship::new(
        "Closing mate",
        RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
        plane(top, [0.0, 0.0, 0.1], [0.0, 0.0, 1.0]),
        plane(base, [0.0, 0.0, -0.1], [0.0, 0.0, -1.0]),
    );
    let before = c.s.doc().clone();
    let depth = c.s.history().undo_depth();
    for attempt in 0..2 {
        let (error, took) = timed(&format!("refused closing mate {attempt}"), || {
            c.s.add_relationship(pinned.clone()).unwrap_err()
        });
        let _ = took;
        assert!(chain::is_unsolvable(&error), "{error:?}");
        assert!(c.s.doc().same_content(&before), "the refusal changed the cell");
        assert_eq!(c.s.history().undo_depth(), depth);
    }
    // The diagnosis (what stands in the way) is computed, every culprit is a real relationship, and each
    // one alone, suppressed, lets the new one in.
    let (culprits, _) = timed("diagnosis", || c.s.conflicting_relationships(Some(&pinned)));
    println!("culprits: {}", culprits.len());
    assert!(!culprits.is_empty(), "no culprit named for a conflicting mate");
    let known: Vec<_> = c.s.relationships().iter().map(|(id, _)| *id).collect();
    assert!(culprits.iter().all(|id| known.contains(id)));
    // Suppressing one culprit lets the closing mate in; undoing both steps restores the chain exactly.
    c.s.set_relationship_suppressed(culprits[0], true).unwrap();
    c.s.add_relationship(pinned).unwrap();
    assert!(c.s.undo() && c.s.undo());
    assert!(c.s.doc().same_content(&before));
}

/// Editing a relationship in the middle moves everything above it by the change, in one undo step; deleting
/// it leaves the components where they are; both undo exactly.
#[test]
fn editing_and_deleting_a_relationship_in_the_middle_of_a_chain_undoes_exactly() {
    let mut c = chain::build(100);
    let before = c.s.doc().clone();
    let depth = c.s.history().undo_depth();
    let middle = 49; // the mate between body 49 and body 50
    let edited = chain::edit(&c, middle, 0.05);
    let (result, _) = timed("edit in the middle", || chain::set(&mut c.s, c.mates[middle], edited));
    result.unwrap();
    assert_eq!(c.s.history().undo_depth(), depth + 1, "one undo step");
    chain::all_stacked(&c, &[(middle, 0.05)]);
    assert!(c.s.undo());
    assert!(c.s.doc().same_content(&before));
    chain::all_stacked(&c, &[]);
    assert!(c.s.redo());
    chain::all_stacked(&c, &[(middle, 0.05)]);
    assert!(c.s.undo());

    // A refused edit (an offset the rest of the chain cannot take does not exist here, so use a broken
    // one): nothing changes.
    for bad in [f64::NAN, f64::INFINITY] {
        let broken = chain::edit(&c, middle, bad);
        let depth = c.s.history().undo_depth();
        assert!(chain::set(&mut c.s, c.mates[middle], broken).is_err(), "offset {bad}");
        assert!(c.s.doc().same_content(&before));
        assert_eq!(c.s.history().undo_depth(), depth);
    }

    // Deleting it: the components stay, the relationship count drops, undo brings it back.
    let (deleted, _) = timed("delete in the middle", || c.s.delete_relationship(c.mates[middle]));
    deleted.unwrap();
    assert_eq!(c.s.relationships().len(), 98);
    chain::all_stacked(&c, &[]);
    assert!(c.s.doc().solve_relationships().is_ok(), "the chain with a gap still solves");
    assert!(c.s.undo());
    assert!(c.s.doc().same_content(&before));
}

/// One loop inside the chain: relationships that tie body 10 to body 13 along X (loop 10-11-12-13-10 in the
/// relationship graph). A consistent loop is accepted and changes nothing; the same relationship twice is
/// harmless or refused cleanly; a loop that cannot hold is refused with a diagnosis; the chain still solves.
#[test]
fn a_loop_inside_a_chain_is_accepted_when_consistent_and_diagnosed_when_not() {
    let mut c = chain::build(100);
    let (b10, b11, b13) = (c.bodies[10], c.bodies[11], c.bodies[13]);
    let before = c.s.doc().clone();
    let depth = c.s.history().undo_depth();

    // Consistent: the +X faces of 10 and 13 flush (they are on the same line, offset 0).
    let (added, _) =
        timed("a loop in the chain", || c.s.add_relationship(chain::side(b10, b13, 0.0, "Side 10-13")));
    let loop_id = added.unwrap();
    assert_eq!(c.s.history().undo_depth(), depth + 1);
    chain::all_stacked(&c, &[]);
    assert!((chain::world_x(&c.s, b13) - chain::world_x(&c.s, b10)).abs() < 1e-9);
    assert!(c.s.doc().solve_relationships().is_ok());

    // The same relationship again: a duplicate is refused or harmless; no panic, no stray movement.
    let again = chain::side(b10, b13, 0.0, "Side 10-13 again");
    let doc = c.s.doc().clone();
    let depth_now = c.s.history().undo_depth();
    match c.s.add_relationship(again) {
        Ok(_) => assert_eq!(c.s.history().undo_depth(), depth_now + 1),
        Err(e) => {
            assert!(chain::is_unsolvable(&e) || matches!(e, gc_doc::EditError::BadValue), "{e:?}");
            assert!(c.s.doc().same_content(&doc));
            assert_eq!(c.s.history().undo_depth(), depth_now);
        }
    }
    while c.s.history().undo_depth() > depth_now {
        assert!(c.s.undo());
    }

    // Not consistent: 11's side 50 mm off 13's, while 10~13 are lined up and 10~11 are free in X: that holds
    // (11 simply moves), so close the loop properly — 11 flush with 10 and with 13 at different offsets.
    c.s.add_relationship(chain::side(b10, b11, 0.0, "Side 10-11")).unwrap();
    let impossible = chain::side(b11, b13, 0.05, "Side 11-13");
    let before_conflict = c.s.doc().clone();
    let depth_conflict = c.s.history().undo_depth();
    let error = c.s.add_relationship(impossible.clone()).unwrap_err();
    assert!(chain::is_unsolvable(&error), "{error:?}");
    assert!(c.s.doc().same_content(&before_conflict), "the refused loop changed the cell");
    assert_eq!(c.s.history().undo_depth(), depth_conflict);
    let culprits = c.s.conflicting_relationships(Some(&impossible));
    assert!(!culprits.is_empty(), "no culprit for the impossible loop");
    let names: Vec<String> =
        c.s.relationships()
            .iter()
            .filter(|(id, _)| culprits.contains(id))
            .map(|(_, r)| r.name.clone())
            .collect();
    println!("loop culprits: {names:?}");
    assert!(
        names.iter().any(|n| n.starts_with("Side")),
        "the diagnosis should point into the loop, got {names:?}"
    );
    // Deleting the consistent loop relationship brings the rest back to a solvable state, and undo of all
    // of it restores the original chain.
    c.s.delete_relationship(loop_id).unwrap();
    chain::all_stacked(&c, &[]);
    while c.s.history().undo_depth() > depth {
        assert!(c.s.undo());
    }
    assert!(c.s.doc().same_content(&before));
}

/// The error of an impossible loop lists the relationships that are not satisfied. Before the chain
/// solver change (dd8b494) it listed exactly the relationships of the loop that disagree; after it, the
/// chain's own Mates that lie on the loop's path are listed too (three "Mate" entries with identical names
/// beside the four Flush ones), although they are all satisfied and only the Flush ones disagree in X.
/// The culprit list (what alone suppressed would fix it) is unchanged.
#[test]
#[ignore = "BUG: the unsatisfied list of an impossible loop now includes satisfied chain Mates (owner: backend, relate.rs chain solver residual); the culprit list is unchanged"]
fn the_unsatisfied_list_of_a_loop_names_only_the_relationships_that_disagree() {
    let mut c = chain::build(12);
    let (b3, b4, b6) = (c.bodies[3], c.bodies[4], c.bodies[6]);
    c.s.add_relationship(chain::side(b3, b6, 0.0, "S3-6")).unwrap();
    c.s.add_relationship(chain::side(b3, b4, 0.0, "S3-4")).unwrap();
    let error = c.s.add_relationship(chain::side(b4, b6, 0.05, "S4-6")).unwrap_err();
    let gc_doc::EditError::Unsolvable { names, .. } = error else { panic!("{error:?}") };
    assert!(names.iter().all(|n| n.starts_with('S')), "satisfied Mates are listed as unsatisfied: {names:?}");
}
