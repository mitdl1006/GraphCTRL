//! Component patterns: rectangular and circular copies that follow the parent; count changes add or remove
//! occurrences; an occurrence can be made independent; one undo step each.

#![allow(clippy::float_cmp, clippy::unwrap_used, clippy::panic, reason = "test code")]

use gc_core::Id;
use gc_doc::{
    Body, CellOp, Command, Created, Document, Geometry, History, Occurrence, OriginElement, PatternDirection,
    PatternLayout, Placement, Selection, Solid, apply,
};
use gc_math::DVec3;

fn run(doc: &mut Document, history: &mut History, op: CellOp) -> Vec<Created> {
    let applied = apply(doc, &Command::Cell(op)).unwrap();
    history.record(applied.label, applied.delta);
    applied.created
}

fn body(doc: &mut Document, history: &mut History, xyz: [f64; 3]) -> Id<Body> {
    let body = Body {
        name: "Box".into(),
        placement: Placement { xyz, rpy: [0.0; 3] },
        solid: Solid::Box { size: [0.1; 3] },
        look: Default::default(),
    };
    let Created::Body(id) = run(doc, history, CellOp::AddBody(body))[0] else { panic!() };
    id
}

fn axis_of(item: Id<Body>, axis: u8) -> Selection {
    Selection {
        occurrence: Occurrence::Item(item.erase()),
        geometry: Geometry::origin(OriginElement::Axis(axis)),
    }
}

fn at(doc: &Document, item: gc_core::AnyId) -> DVec3 {
    let p = doc.get(Id::<Body>::from_raw(item.raw)).unwrap().placement;
    DVec3::from_array(p.xyz)
}

fn close(a: DVec3, b: DVec3) -> bool {
    (a - b).length() < 1e-9
}

#[test]
fn a_rectangular_pattern_lays_copies_in_columns_and_rows_and_follows_the_parent() {
    let (mut doc, mut history) = (Document::new(), History::default());
    let floor = body(&mut doc, &mut history, [0.0; 3]);
    let part = body(&mut doc, &mut history, [1.0, 0.0, 0.2]);
    let layout = PatternLayout::Rectangular {
        column: PatternDirection { along: axis_of(floor, 0), flip: false, count: 3, spacing: 0.5 },
        row: Some(PatternDirection { along: axis_of(floor, 1), flip: true, count: 2, spacing: 0.4 }),
    };
    let depth = history.undo_depth();
    let created = run(
        &mut doc,
        &mut history,
        CellOp::AddPattern { name: "Pattern1".into(), parents: vec![part.erase()], layout },
    );
    assert_eq!(created.len(), 5, "3 × 2 occurrences, the parent included");
    assert_eq!(history.undo_depth(), depth + 1);
    let pattern = &doc.patterns()[0];
    let copy = |index: u32| pattern.elements.iter().find(|e| e.index == index).unwrap().items[0];
    assert!(close(at(&doc, copy(2)), DVec3::new(2.0, 0.0, 0.2)), "two columns along");
    assert!(close(at(&doc, copy(4)), DVec3::new(1.5, -0.4, 0.2)), "a row over, flipped");
    // The parent moves: the copies follow.
    let mut moved = doc.get(part).unwrap().clone();
    moved.placement.xyz = [1.0, 1.0, 0.2];
    run(&mut doc, &mut history, CellOp::SetBody { id: part, body: moved });
    assert!(close(at(&doc, copy(2)), DVec3::new(2.0, 1.0, 0.2)));
    // Two columns, one row: three copies go.
    let smaller = PatternLayout::Rectangular {
        column: PatternDirection { along: axis_of(floor, 0), flip: false, count: 2, spacing: 0.5 },
        row: None,
    };
    run(&mut doc, &mut history, CellOp::SetPattern { index: 0, layout: smaller });
    assert_eq!(doc.bodies().iter().count(), 3, "floor, part, one copy");
    history.undo(&mut doc).unwrap();
    assert_eq!(doc.bodies().iter().count(), 7);
}

#[test]
fn a_circular_pattern_turns_copies_round_the_axis_rotational_or_fixed() {
    let (mut doc, mut history) = (Document::new(), History::default());
    let hub = body(&mut doc, &mut history, [0.0; 3]);
    let part = body(&mut doc, &mut history, [1.0, 0.0, 0.0]);
    for (rotational, fitted, angle) in [(true, false, 90f64), (false, true, 360f64)] {
        let layout = PatternLayout::Circular {
            axis: axis_of(hub, 2),
            flip: false,
            count: 4,
            angle: angle.to_radians(),
            fitted,
            rotational,
            midplane: false,
        };
        run(
            &mut doc,
            &mut history,
            CellOp::AddPattern { name: "Round".into(), parents: vec![part.erase()], layout },
        );
        let pattern = doc.patterns().last().unwrap().clone();
        let first = pattern.elements.iter().find(|e| e.index == 1).unwrap().items[0];
        let p = at(&doc, first);
        assert!(p.x.abs() < 1e-9 && (p.y - 1.0).abs() < 1e-9, "a quarter turn round Z either way: {p:?}");
        let yaw = doc.get(Id::<Body>::from_raw(first.raw)).unwrap().placement.rpy[2];
        if rotational {
            assert!((yaw - 90f64.to_radians()).abs() < 1e-9, "turned with it");
        } else {
            assert!(yaw.abs() < 1e-9, "fixed orientation");
        }
        // Make occurrence 2 independent: it stays put when the parent moves.
        let second = pattern.elements.iter().find(|e| e.index == 2).unwrap().items[0];
        let index = doc.patterns().len() - 1;
        run(&mut doc, &mut history, CellOp::MakePatternElementIndependent { index, element: 2 });
        let before = at(&doc, second);
        let mut moved = doc.get(part).unwrap().clone();
        moved.placement.xyz[2] += 0.1;
        run(&mut doc, &mut history, CellOp::SetBody { id: part, body: moved });
        assert!(close(at(&doc, second), before));
        assert!((at(&doc, first).z - doc.get(part).unwrap().placement.xyz[2]).abs() < 1e-12);
        // Deleting the pattern but keeping the copies.
        run(&mut doc, &mut history, CellOp::DeletePattern { index, keep_copies: true });
        assert!(doc.get(Id::<Body>::from_raw(first.raw)).is_some());
    }
}

/// Suppress elements in assembly patterns (Inventor 2026 help, GUID-FDD20E41-93A5-479D-B547-BA9956E9966B)
/// and restoring an independent element (About Assembly Components in Patterns,
/// GUID-ABC5607F-6128-40C8-BF28-44D918E5803C).
#[test]
fn pattern_elements_can_be_suppressed_and_independent_ones_restored() {
    use gc_doc::ComponentFlag;
    let (mut doc, mut history) = (Document::new(), History::default());
    let floor = body(&mut doc, &mut history, [0.0; 3]);
    let part = body(&mut doc, &mut history, [1.0, 0.0, 0.2]);
    let columns = |count: u32| PatternLayout::Rectangular {
        column: PatternDirection { along: axis_of(floor, 0), flip: false, count, spacing: 0.5 },
        row: None,
    };
    let add = CellOp::AddPattern { name: "Row".into(), parents: vec![part.erase()], layout: columns(4) };
    run(&mut doc, &mut history, add);
    let copy = |doc: &Document, index: u32| {
        doc.patterns()[0].elements.iter().find(|e| e.index == index).map(|e| e.items[0])
    };
    let suppressed = |doc: &Document, item: gc_core::AnyId| {
        doc.component_flag(Occurrence::Item(item), ComponentFlag::Suppressed)
    };
    // Element 2 is suppressed: its component with it, in one undo step.
    let second = copy(&doc, 2).unwrap();
    let depth = history.undo_depth();
    run(&mut doc, &mut history, CellOp::SuppressPatternElement { index: 0, element: 2, suppressed: true });
    assert_eq!(history.undo_depth(), depth + 1);
    assert!(suppressed(&doc, second) && !suppressed(&doc, copy(&doc, 1).unwrap()));
    assert_eq!(doc.patterns()[0].suppressed, [2]);
    // The count shrinks and grows again: the place is still suppressed.
    run(&mut doc, &mut history, CellOp::SetPattern { index: 0, layout: columns(2) });
    assert!(copy(&doc, 2).is_none());
    run(&mut doc, &mut history, CellOp::SetPattern { index: 0, layout: columns(4) });
    let again = copy(&doc, 2).unwrap();
    assert!(suppressed(&doc, again) && !suppressed(&doc, copy(&doc, 3).unwrap()));
    run(&mut doc, &mut history, CellOp::SuppressPatternElement { index: 0, element: 2, suppressed: false });
    assert!(!suppressed(&doc, again) && doc.patterns()[0].suppressed.is_empty());
    // The source and places that are not there cannot be suppressed.
    for element in [0, 9] {
        let op = CellOp::SuppressPatternElement { index: 0, element, suppressed: true };
        assert!(apply(&mut doc, &Command::Cell(op)).is_err(), "{element}");
    }

    // Independent, then restored: the pattern fills the place again and the component that left stays.
    let third = copy(&doc, 3).unwrap();
    run(&mut doc, &mut history, CellOp::MakePatternElementIndependent { index: 0, element: 3 });
    let bodies = doc.bodies().iter().count();
    let created = run(&mut doc, &mut history, CellOp::RestorePatternElement { index: 0, element: 3 });
    assert_eq!(created.len(), 1);
    assert_eq!(doc.bodies().iter().count(), bodies + 1);
    let restored = copy(&doc, 3).unwrap();
    assert_ne!(restored, third);
    assert!(close(at(&doc, restored), at(&doc, third)), "two parts in one place, as the help warns");
    assert!(doc.patterns()[0].independent.is_empty());
    let twice = CellOp::RestorePatternElement { index: 0, element: 3 };
    assert!(apply(&mut doc, &Command::Cell(twice)).is_err());
}
