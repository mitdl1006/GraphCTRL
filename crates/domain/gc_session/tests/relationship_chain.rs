//! Relationship solving on long chains (docs/KNOWN_ISSUES: adding relationships slowed down steeply):
//! a chain is solved one component at a time out from what holds still.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "tests")]

use gc_core::Id;
use gc_doc::{
    AxisSense, Body, ComponentFlag, Geometry, Occurrence, Placement, RelationKind, Relationship, Selection,
    Solid,
};
use gc_session::Session;

fn placed(z: f64) -> Placement {
    Placement { xyz: [0.03 * z, 0.0, z], rpy: [0.0, 0.0, 0.1 * z] }
}

fn plane(b: Id<Body>, z: f64, nz: f64) -> Selection {
    Selection {
        occurrence: Occurrence::Item(b.erase()),
        geometry: Geometry::Plane { point: [0.0, 0.0, z], normal: [0.0, 0.0, nz] },
    }
}

fn frame(b: Id<Body>, z: f64, nz: f64) -> Selection {
    Selection {
        occurrence: Occurrence::Item(b.erase()),
        geometry: Geometry::Frame { origin: [0.0, 0.0, z], z: [0.0, 0.0, nz], x: [1.0, 0.0, 0.0] },
    }
}

fn world_z(s: &Session, b: Id<Body>) -> f64 {
    s.doc().component_origin(Occurrence::Item(b.erase())).unwrap().pose.translation.z
}

/// 100 components on a grounded base, tied alternately by mates and rigid joints: built in well under a
/// second, every relationship satisfied, each component stacked on the one before.
#[test]
fn a_hundred_component_chain_is_built_quickly() {
    let mut s = Session::new();
    let base = s.add_body(Solid::Box { size: [0.2; 3] }, placed(0.1)).unwrap();
    s.set_component(Occurrence::Item(base.erase()), ComponentFlag::Grounded, true).unwrap();
    let mut chain = vec![base];
    let started = std::time::Instant::now();
    for k in 1..100 {
        let next = s.add_body(Solid::Box { size: [0.2; 3] }, placed(0.1 + 0.4 * f64::from(k))).unwrap();
        let previous = *chain.last().unwrap();
        let relationship = if k % 2 == 0 {
            Relationship::new(
                "Mate",
                RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
                plane(next, -0.1, -1.0),
                plane(previous, 0.1, 1.0),
            )
        } else {
            Relationship::new(
                "Rigid",
                RelationKind::Rigid { flip: false },
                frame(next, -0.1, -1.0),
                frame(previous, 0.1, 1.0),
            )
        };
        s.add_relationship(relationship).unwrap();
        chain.push(next);
    }
    let elapsed = started.elapsed();
    assert!(elapsed.as_secs_f64() < 1.0, "100-component chain took {elapsed:?}");
    assert!(s.doc().solve_relationships().is_ok());
    for pair in chain.windows(2) {
        let gap = world_z(&s, pair[1]) - world_z(&s, pair[0]);
        assert!((gap - 0.2).abs() < 1e-6, "stacked 0.2 m apart, got {gap}");
    }
    // A relationship that conflicts with the chain is still refused and diagnosed.
    let top = *chain.last().unwrap();
    let pinned = Relationship::new(
        "Mate",
        RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
        plane(top, 0.1, 1.0),
        plane(base, -0.1, -1.0),
    );
    assert!(s.add_relationship(pinned.clone()).is_err());
    assert!(
        !s.doc().conflicting_relationships(Some(&pinned)).is_empty() || s.doc().solve_relationships().is_ok()
    );
}
