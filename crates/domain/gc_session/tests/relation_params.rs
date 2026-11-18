//! Constraint values as model parameters through the session and a file (Inventor 2026 help, About
//! Parameters in Models, GUID-8E5C63EA-0540-4EBE-9B17-B8159C7FE40C).

#![allow(clippy::unwrap_used, clippy::panic, clippy::float_cmp, reason = "test code")]

use gc_core::Id;
use gc_doc::{
    AxisSense, Body, CellOp, Command, Created, Geometry, Occurrence, Placement, RelationKind, Relationship,
    Selection, Solid,
};
use gc_session::Session;

fn on(body: Id<Body>, z: f64, up: f64) -> Selection {
    let geometry = Geometry::Plane { point: [0.0, 0.0, z], normal: [0.0, 0.0, up] };
    Selection { occurrence: Occurrence::Item(body.erase()), geometry }
}

#[test]
fn constraint_parameters_survive_saving_and_opening() {
    let mut s = Session::new();
    let table = s
        .add_body(Solid::Box { size: [2.0, 1.0, 1.0] }, Placement { xyz: [0.0, 0.0, 0.5], rpy: [0.0; 3] })
        .unwrap();
    let block =
        s.add_body(Solid::Box { size: [0.2; 3] }, Placement { xyz: [0.3, 0.0, 1.5], rpy: [0.0; 3] }).unwrap();
    let lid = s
        .add_body(Solid::Box { size: [0.2; 3] }, Placement { xyz: [-0.3, 0.0, 1.5], rpy: [0.0; 3] })
        .unwrap();
    let mate = |body: Id<Body>| {
        Relationship::new(
            "Mate",
            RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
            on(body, -0.1, -1.0),
            on(table, 0.5, 1.0),
        )
    };
    let first = s.add_relationship(mate(block)).unwrap();
    let second = s.add_relationship(mate(lid)).unwrap();
    let created =
        s.execute(&Command::AddParam { name: "gap".into(), source: "1 in".into(), angle: false }).unwrap();
    let Created::Param(gap) = created[0] else { panic!() };
    // Units as in a dimension: a bare number is the document's unit (mm), others are converted.
    s.execute(&Command::Cell(CellOp::SetRelationValue { id: first, source: "lift = gap + 4.6".into() }))
        .unwrap();
    s.execute(&Command::Cell(CellOp::SetRelationValue { id: second, source: "0.5 cm".into() })).unwrap();
    assert!((s.doc().relation_param(first).unwrap().value - 0.03).abs() < 1e-12);
    assert_eq!(s.doc().relation_param(second).unwrap().value, 0.005);
    assert!((s.doc().get(block).unwrap().placement.xyz[2] - 1.13).abs() < 1e-7);

    // Saved and opened: names, texts and values are the same, and the parameters still drive.
    let text = gc_persist::to_text(s.doc()).unwrap();
    let mut opened = gc_persist::from_text(&text).unwrap().doc;
    assert!(opened.same_content(s.doc()));
    let rows = |doc: &gc_doc::Document| -> Vec<(String, String, f64)> {
        doc.relation_params()
            .into_iter()
            .map(|(_, p, _)| (p.name.clone(), p.source.clone(), p.value))
            .collect()
    };
    assert_eq!(rows(&opened), rows(s.doc()));
    assert_eq!(rows(&opened)[0].0, "lift");
    assert_eq!(rows(&opened)[0].1, "gap + 4.6");
    gc_doc::apply(&mut opened, &Command::SetParam { id: gap, source: "10 mm".into() }).unwrap();
    assert!((opened.relation_param(first).unwrap().value - 0.0146).abs() < 1e-12);
    assert!((opened.get(block).unwrap().placement.xyz[2] - 1.1146).abs() < 1e-7);
    // Undo and redo in the session keep constraint and parameter together.
    assert!(s.undo());
    assert_eq!(s.doc().relation_param(second).unwrap().source, "0 mm");
    assert!(s.redo());
    assert_eq!(s.doc().relation_param(second).unwrap().source, "0.5 cm");
}
