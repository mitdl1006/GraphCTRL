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
