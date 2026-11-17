//! Many relationships (QA, docs/KNOWN_ISSUES.md: adding a relationship to a cell of 400 constrained
//! components took 9.5 s, 1000 more than five minutes): each addition re-solves only the island of
//! components it ties together, and the browser's statuses are worked out once per document change.

#![allow(clippy::unwrap_used, reason = "test code")]

use gc_doc::{
    AxisSense, ComponentStatus, Geometry, Occurrence, Placement, RelationKind, Relationship, Selection, Solid,
};
use gc_session::Session;

fn on(id: gc_core::Id<gc_doc::Body>, z: f64, n: f64) -> Selection {
    Selection {
        occurrence: Occurrence::Item(id.erase()),
        geometry: Geometry::Plane { point: [0.0, 0.0, z], normal: [0.0, 0.0, n] },
    }
}

#[test]
fn hundreds_of_mates_are_added_quickly_and_statuses_follow_edits() {
    let mut session = Session::new();
    let floor = session
        .add_body(Solid::Box { size: [40.0, 40.0, 0.1] }, Placement { xyz: [0.0, 0.0, -0.05], rpy: [0.0; 3] })
        .unwrap();
    let start = std::time::Instant::now();
    let mut bodies = Vec::new();
    #[allow(clippy::cast_precision_loss, reason = "small counts")]
    for i in 0..300 {
        let placement = Placement { xyz: [(i % 20) as f64 * 0.5, (i / 20) as f64 * 0.5, 0.4], rpy: [0.0; 3] };
        let body = session.add_body(Solid::Box { size: [0.2; 3] }, placement).unwrap();
        session
            .add_relationship(Relationship::new(
                format!("Mate:{i}"),
                RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
                on(body, -0.1, -1.0),
                on(floor, 0.05, 1.0),
            ))
            .unwrap();
        bodies.push(body);
    }
    let elapsed = start.elapsed();
    // Before: minutes in this (debug) profile. Generous, to stay stable on slow machines.
    assert!(elapsed.as_secs_f64() < 20.0, "{elapsed:?}");
    // Every body sits on the floor.
    assert!(bodies.iter().all(|b| (session.doc().get(*b).unwrap().placement.xyz[2] - 0.1).abs() < 1e-7));

    // Statuses: asked twice without a change, the same; after an edit, up to date; after undo, back.
    let first = session.component_statuses();
    assert_eq!(session.component_statuses(), first);
    assert_eq!(first.get(&bodies[0].erase()), Some(&ComponentStatus::Under));
    session
        .set_component(Occurrence::Item(bodies[0].erase()), gc_doc::ComponentFlag::Grounded, true)
        .unwrap();
    assert_eq!(session.component_statuses().get(&bodies[0].erase()), Some(&ComponentStatus::Full));
    session.undo();
    assert_eq!(session.component_statuses(), first);
    // A new document is not answered from the old one's statuses.
    session.new_document();
    assert!(session.component_statuses().is_empty());
}
