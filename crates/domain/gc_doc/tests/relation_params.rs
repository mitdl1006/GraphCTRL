//! Model parameters of assembly constraints (Inventor 2026 help, About Parameters in Models,
//! GUID-8E5C63EA-0540-4EBE-9B17-B8159C7FE40C): a constraint's offset or angle gets a default name `dN`,
//! takes expressions, can be named in other expressions and follows the parameters it names.

#![allow(clippy::float_cmp, clippy::unwrap_used, clippy::panic, reason = "test code")]

use gc_core::Id;
use gc_doc::{
    AxisSense, Body, CellOp, Command, Created, Document, EditError, Geometry, History, Occurrence, Param,
    Placement, RelationKind, Relationship, Selection, Solid, apply,
};

struct Bench {
    doc: Document,
    history: History,
}

impl Bench {
    fn run(&mut self, command: Command) -> Result<Vec<Created>, EditError> {
        let applied = apply(&mut self.doc, &command)?;
        self.history.record(applied.label, applied.delta);
        Ok(applied.created)
    }

    fn cell(&mut self, op: CellOp) -> Result<Vec<Created>, EditError> {
        self.run(Command::Cell(op))
    }

    fn body(&mut self, name: &str, size: [f64; 3], xyz: [f64; 3]) -> Id<Body> {
        let placement = Placement { xyz, rpy: [0.0; 3] };
        let body =
            Body { name: name.into(), placement, solid: Solid::Box { size }, look: Default::default() };
        let Created::Body(id) = self.cell(CellOp::AddBody(body)).unwrap()[0] else { panic!() };
        id
    }

    fn param(&mut self, name: &str, source: &str) -> Id<Param> {
        let command = Command::AddParam { name: name.into(), source: source.into(), angle: false };
        let Created::Param(id) = self.run(command).unwrap()[0] else { panic!() };
        id
    }

    fn height(&self, body: Id<Body>) -> f64 {
        self.doc.get(body).unwrap().placement.xyz[2]
    }

    fn offset(&self, id: Id<Relationship>) -> f64 {
        let RelationKind::Mate { offset, .. } = self.doc.get(id).unwrap().kind else { panic!() };
        offset
    }
}

fn on(body: Id<Body>, z: f64, up: f64) -> Selection {
    let geometry = Geometry::Plane { point: [0.0, 0.0, z], normal: [0.0, 0.0, up] };
    Selection { occurrence: Occurrence::Item(body.erase()), geometry }
}

/// A grounded table 1 m high with a 0.2 m block mated on top at `offset`, and a second block likewise.
fn bench() -> (Bench, [Id<Body>; 2], [Id<Relationship>; 2]) {
    let mut bench = Bench { doc: Document::new(), history: History::default() };
    let table = bench.body("Table", [2.0, 1.0, 1.0], [0.0, 0.0, 0.5]);
    let made = |bench: &mut Bench, name: &str, x: f64, offset: f64| {
        let block = bench.body(name, [0.2, 0.2, 0.2], [x, 0.0, 1.5]);
        let mate = Relationship {
            limits: None,
            name: format!("Mate:{name}"),
            kind: RelationKind::Mate { offset, sense: AxisSense::Opposed },
            first: on(block, -0.1, -1.0),
            second: on(table, 0.5, 1.0),
        };
        let Created::Relationship(id) = bench.cell(CellOp::AddRelationship(mate)).unwrap()[0] else {
            panic!()
        };
        (block, id)
    };
    let (a, first) = made(&mut bench, "A", -0.5, 0.0);
    let (b, second) = made(&mut bench, "B", 0.5, 0.0125);
    (bench, [a, b], [first, second])
}

#[test]
fn a_constraint_value_gets_a_default_name_and_its_text() {
    let (mut bench, _, [first, second]) = bench();
    let names: Vec<(String, String, f64)> = bench
        .doc
        .relation_params()
        .into_iter()
        .map(|(_, p, angle)| {
            assert!(!angle);
            (p.name.clone(), p.source.clone(), p.value)
        })
        .collect();
    assert_eq!(names, [("d0".to_owned(), "0 mm".to_owned(), 0.0), ("d1".into(), "12.5 mm".into(), 0.0125)]);
    // A parameter the user makes cannot take a constraint's name, and the next default name is free.
    let taken = Command::AddParam { name: "d1".into(), source: "1".into(), angle: false };
    assert!(matches!(bench.run(taken), Err(EditError::BadName(_))));
    // Setting the value as a number (the dialog's old way) writes the text anew.
    let mut changed = bench.doc.get(first).unwrap().clone();
    changed.kind = RelationKind::Mate { offset: 0.03, sense: AxisSense::Opposed };
    bench.cell(CellOp::SetRelationship { id: first, relationship: changed }).unwrap();
    let param = bench.doc.relation_param(first).unwrap();
    assert_eq!((param.name.as_str(), param.source.as_str(), param.value), ("d0", "30 mm", 0.03));
    // Deleting the constraint takes its parameter along; undo brings both back.
    bench.cell(CellOp::DeleteRelationship(second)).unwrap();
    assert!(bench.doc.relation_param(second).is_none());
    bench.history.undo(&mut bench.doc).unwrap();
    assert_eq!(bench.doc.relation_param(second).unwrap().name, "d1");
    // A name nothing has is reported against the text.
    assert!(matches!(
        bench.cell(CellOp::SetRelationValue { id: second, source: "x".into() }),
        Err(EditError::Expr { .. })
    ));
}

#[test]
fn a_constraint_follows_the_parameters_its_expression_names() {
    let (mut bench, [a, b], [first, second]) = bench();
    let gap = bench.param("gap", "20 mm");
    let depth = bench.history.undo_depth();
    bench.cell(CellOp::SetRelationValue { id: first, source: "gap / 2 + 5 mm".into() }).unwrap();
    assert_eq!(bench.history.undo_depth(), depth + 1);
    assert!((bench.offset(first) - 0.015).abs() < 1e-12);
    assert!((bench.height(a) - 1.115).abs() < 1e-7, "{}", bench.height(a));
    let param = bench.doc.relation_param(first).unwrap();
    assert_eq!((param.name.as_str(), param.source.as_str()), ("d0", "gap / 2 + 5 mm"));
    // `NAME = expression` names the parameter; another constraint may then use it.
    bench.cell(CellOp::SetRelationValue { id: first, source: "lift = gap / 2".into() }).unwrap();
    bench.cell(CellOp::SetRelationValue { id: second, source: "lift * 3".into() }).unwrap();
    assert!((bench.height(a) - 1.11).abs() < 1e-7 && (bench.height(b) - 1.13).abs() < 1e-7);
    // Changing the parameter moves both, in one undo step.
    let depth = bench.history.undo_depth();
    bench.run(Command::SetParam { id: gap, source: "40 mm".into() }).unwrap();
    assert_eq!(bench.history.undo_depth(), depth + 1);
    assert!((bench.offset(first) - 0.02).abs() < 1e-12 && (bench.offset(second) - 0.06).abs() < 1e-12);
    assert!((bench.height(a) - 1.12).abs() < 1e-7 && (bench.height(b) - 1.16).abs() < 1e-7);
    bench.history.undo(&mut bench.doc).unwrap();
    assert!((bench.height(a) - 1.11).abs() < 1e-7 && (bench.height(b) - 1.13).abs() < 1e-7);
    assert_eq!(bench.doc.relation_param(first).unwrap().value, 0.01);

    // Renames follow in both directions.
    bench.run(Command::RenameParam { id: gap, name: "clearance".into() }).unwrap();
    assert_eq!(bench.doc.relation_param(first).unwrap().source, "clearance / 2");
    bench.cell(CellOp::RenameRelationParam { id: first, name: "raise".into() }).unwrap();
    assert_eq!(bench.doc.relation_param(second).unwrap().source, "raise * 3");
    let clash = CellOp::RenameRelationParam { id: second, name: "clearance".into() };
    assert!(matches!(bench.cell(clash), Err(EditError::BadName(_))));
    // A parameter a constraint uses cannot be deleted; a loop is refused; bad text is reported.
    assert_eq!(bench.run(Command::DeleteParam { id: gap }), Err(EditError::InUse));
    let d1 = bench.doc.relation_param(second).unwrap().name.clone();
    let looped = CellOp::SetRelationValue { id: first, source: format!("{d1} + 1 mm") };
    assert_eq!(bench.cell(looped), Err(EditError::Cycle));
    let broken = CellOp::SetRelationValue { id: first, source: "clearance +".into() };
    assert!(matches!(bench.cell(broken), Err(EditError::Expr { .. })));
    assert_eq!(bench.doc.relation_param(first).unwrap().source, "clearance / 2");

    // A parameter may name a constraint's value. When that constraint goes, the expressions that named
    // it keep its last value.
    let double = bench.param("double", "raise * 2");
    assert!((bench.doc.get(double).unwrap().value - 0.02).abs() < 1e-12);
    bench.cell(CellOp::DeleteRelationship(first)).unwrap();
    assert_eq!(bench.doc.get(double).unwrap().source, "(10 mm) * 2");
    assert_eq!(bench.doc.relation_param(second).unwrap().source, "(10 mm) * 3");
    bench.run(Command::SetParam { id: gap, source: "1 mm".into() }).unwrap();
    assert!((bench.offset(second) - 0.03).abs() < 1e-12);
}

#[test]
fn an_angle_is_an_angle_parameter_and_a_units_change_keeps_sizes() {
    let (mut bench, [a, _], [first, _]) = bench();
    let table = Id::<Body>::from_raw(bench.doc.bodies().iter().next().unwrap().0.raw());
    bench.cell(CellOp::DeleteRelationship(first)).unwrap();
    let angle = Relationship {
        limits: None,
        name: "Angle:1".into(),
        kind: RelationKind::Angle { angle: 0.0, solution: gc_doc::AngleSolution::Directed },
        first: on(a, 0.1, 1.0),
        second: on(table, 0.5, 1.0),
    };
    let Created::Relationship(id) = bench.cell(CellOp::AddRelationship(angle)).unwrap()[0] else { panic!() };
    bench.cell(CellOp::SetRelationValue { id, source: "tilt = 10 + 5".into() }).unwrap();
    let (_, param, is_angle) =
        bench.doc.relation_params().into_iter().find(|(other, ..)| *other == id).unwrap();
    assert!(is_angle);
    assert_eq!(param.name, "tilt");
    assert!((param.value - 15f64.to_radians()).abs() < 1e-12);
    let RelationKind::Angle { angle, .. } = bench.doc.get(id).unwrap().kind else { panic!() };
    assert!((angle - 15f64.to_radians()).abs() < 1e-12);

    // A bare number means the document's unit; changing the units keeps what was typed the same size.
    let other = bench.doc.relation_params()[0].0;
    bench.cell(CellOp::SetRelationValue { id: other, source: "25".into() }).unwrap();
    let mut settings = bench.doc.settings();
    settings.length_unit = gc_doc::LengthUnit::In;
    bench.cell(CellOp::SetDocSettings(settings)).unwrap();
    let param = bench.doc.relation_param(other).unwrap();
    assert_eq!((param.source.as_str(), param.value), ("25 mm", 0.025));
    assert_eq!(bench.doc.relation_param(id).unwrap().source, "10 + 5");
}
