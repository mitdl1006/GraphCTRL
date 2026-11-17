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
