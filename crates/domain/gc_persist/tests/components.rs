//! Component states of the assembly tree survive saving and opening (assembly P1-1).

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use gc_doc::{
    Assembly, AssemblyMesh, AssemblyPart, CellOp, Command, ComponentFlag, Created, Document, Occurrence,
    Placement, apply,
};

#[test]
fn component_states_and_the_tree_are_the_same_after_saving_and_opening() {
    let identity = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];
    let assembly = Assembly {
        name: "Machine".into(),
        file: "machine.glb".into(),
        placement: Placement::default(),
        parts: vec![
            AssemblyPart { name: "Frame".into(), parent: None, matrix: identity, meshes: vec![0] },
            AssemblyPart { name: "Door".into(), parent: Some(0), matrix: identity, meshes: vec![0] },
        ],
        meshes: vec![AssemblyMesh { name: "Plate".into(), color: [0.5; 4], triangles: 12 }],
        look: Default::default(),
    };
    let mut doc = Document::new();
    let created = apply(&mut doc, &Command::Cell(CellOp::AddAssembly(assembly))).unwrap().created;
    let Created::Assembly(id) = created[0] else { panic!() };
    let door = Occurrence::Part { assembly: id, index: 1 };
    for (flag, on) in [
        (ComponentFlag::Grounded, true),
        (ComponentFlag::Enabled, false),
        (ComponentFlag::Visible, false),
        (ComponentFlag::Suppressed, true),
    ] {
        apply(&mut doc, &Command::SetComponent { occurrence: door, flag, on }).unwrap();
    }
    let text = gc_persist::to_text(&doc).unwrap();
    let loaded = gc_persist::from_text(&text).unwrap().doc;
    assert_eq!(loaded.components(), doc.components());
    assert!(loaded.component_flag(Occurrence::Item(id.erase()), ComponentFlag::Grounded));
    assert!(loaded.component_flag(door, ComponentFlag::Suppressed));
}

#[test]
fn assembly_constraints_are_saved_and_opened() {
    use gc_doc::{AxisSense, Body, Geometry, RelationKind, Relationship, Selection, Solid};
    let mut doc = Document::new();
    let mut add = |name: &str, z: f64| {
        let body = Body {
            name: name.into(),
            placement: Placement { xyz: [0.0, 0.0, z], rpy: [0.0; 3] },
            solid: Solid::Box { size: [0.2; 3] },
            look: Default::default(),
        };
        let Created::Body(id) = apply(&mut doc, &Command::Cell(CellOp::AddBody(body))).unwrap().created[0]
        else {
            panic!()
        };
        id
    };
    let (base, block) = (add("Base", 0.0), add("Block", 1.0));
    let plane = |z: f64, n: f64| Geometry::Plane { point: [0.0, 0.0, z], normal: [0.0, 0.0, n] };
    let mate = Relationship {
        name: "Mate:1".into(),
        kind: RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
        first: Selection { occurrence: Occurrence::Item(block.erase()), geometry: plane(-0.1, -1.0) },
        second: Selection { occurrence: Occurrence::Item(base.erase()), geometry: plane(0.1, 1.0) },
    };
    apply(&mut doc, &Command::Cell(CellOp::AddRelationship(mate))).unwrap();
    let loaded = gc_persist::from_text(&gc_persist::to_text(&doc).unwrap()).unwrap().doc;
    assert_eq!(loaded.relationships(), doc.relationships());
    assert!((loaded.get(block).unwrap().placement.xyz[2] - 0.2).abs() < 1e-7);
}
