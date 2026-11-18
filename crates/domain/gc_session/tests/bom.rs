//! Bill of Materials views and CSV export (Inventor 2026 help, About the BOM Structure,
//! GUID-A0A95805-CBF6-4524-8314-4AC8B53C19CF).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "tests")]

use gc_core::Id;
use gc_doc::{
    Body, BomStructure, Created, IProperties, Occurrence, Placement, Solid, SubassemblyOp, TreeKey,
};
use gc_session::{BomView, Session};

fn body(s: &mut Session, x: f64, part: &str, description: &str) -> Id<Body> {
    let b =
        s.add_body(Solid::Box { size: [0.1; 3] }, Placement { xyz: [x, 0.0, 0.0], rpy: [0.0; 3] }).unwrap();
    let props =
        IProperties { part_number: part.into(), description: description.into(), ..IProperties::default() };
    s.set_properties(Some(Occurrence::Item(b.erase())), props).unwrap();
    b
}

fn key(b: Id<Body>) -> TreeKey {
    TreeKey::Component(Occurrence::Item(b.erase()))
}

fn demote(s: &mut Session, members: Vec<TreeKey>, name: &str) -> TreeKey {
    match s.edit_subassemblies(SubassemblyOp::Demote { members, name: name.into() }).unwrap()[..] {
        [Created::Subassembly(id)] => TreeKey::Subassembly(id),
        ref other => panic!("{other:?}"),
    }
}

fn lines(s: &Session, view: BomView) -> Vec<(String, String, u32)> {
    s.bill_of_materials(view).into_iter().map(|r| (r.item, r.part_number, r.quantity)).collect()
}

fn l(item: &str, part: &str, quantity: u32) -> (String, String, u32) {
    (item.to_owned(), part.to_owned(), quantity)
}

/// Frame {bolt, plate}, Kit {bolt, motor}, base.
fn cell() -> (Session, TreeKey, TreeKey, [Id<Body>; 5]) {
    let mut s = Session::new();
    let a = body(&mut s, 0.0, "BOLT", "M8 bolt");
    let c = body(&mut s, 0.3, "PLATE", "");
    let b = body(&mut s, 0.6, "BOLT", "M8 bolt");
    let d = body(&mut s, 0.9, "MOTOR", "Servo, 400 W");
    let e = body(&mut s, 1.2, "BASE", "");
    let frame = demote(&mut s, vec![key(a), key(c)], "Frame");
    let kit = demote(&mut s, vec![key(b), key(d)], "Kit");
    (s, frame, kit, [a, b, c, d, e])
}

#[test]
fn structured_view_lists_every_level_and_parts_only_adds_up_quantities() {
    let (s, _, _, _) = cell();
    assert_eq!(
        lines(&s, BomView::Structured),
        vec![
            l("1", "Frame", 1),
            l("1.1", "BOLT", 1),
            l("1.2", "PLATE", 1),
            l("2", "Kit", 1),
            l("2.1", "BOLT", 1),
            l("2.2", "MOTOR", 1),
            l("3", "BASE", 1),
        ]
    );
    let levels: Vec<u32> = s.bill_of_materials(BomView::Structured).iter().map(|r| r.level).collect();
    assert_eq!(levels, vec![0, 1, 1, 0, 1, 1, 0]);
    // Parts only: no assemblies, the two bolts are one line.
    assert_eq!(
        lines(&s, BomView::PartsOnly),
        vec![l("1", "BOLT", 2), l("2", "PLATE", 1), l("3", "MOTOR", 1), l("4", "BASE", 1)]
    );
    let bolt = &s.bill_of_materials(BomView::PartsOnly)[0];
    assert_eq!(bolt.description, "M8 bolt");
    assert_eq!(bolt.keys.len(), 2);
    assert_eq!(bolt.structure, BomStructure::Normal);
}

#[test]
fn phantom_promotes_children_and_reference_is_left_out_with_its_mass() {
    let (mut s, frame, kit, [_, _, _, _, e]) = cell();
    let steps = s.history().undo_depth();
    s.set_bom_structure(kit, BomStructure::Phantom).unwrap();
    assert_eq!(s.history().undo_depth(), steps + 1);
    assert_eq!(s.bom_structure(kit), BomStructure::Phantom);
    assert_eq!(
        lines(&s, BomView::Structured),
        vec![
            l("1", "Frame", 1),
            l("1.1", "BOLT", 1),
            l("1.2", "PLATE", 1),
            l("2", "BOLT", 1),
            l("3", "MOTOR", 1),
            l("4", "BASE", 1),
        ]
    );
    // Phantom bolts combine with matching items at the level they move to.
    s.set_bom_structure(frame, BomStructure::Phantom).unwrap();
    assert_eq!(
        lines(&s, BomView::Structured),
        vec![l("1", "BOLT", 2), l("2", "PLATE", 1), l("3", "MOTOR", 1), l("4", "BASE", 1)]
    );
    s.set_bom_structure(frame, BomStructure::Normal).unwrap();

    // Reference: gone from both views and from the document's mass and volume.
    let before = s.physical_properties(None).unwrap();
    s.set_bom_structure(key(e), BomStructure::Reference).unwrap();
    assert!(s.bill_of_materials(BomView::Structured).iter().all(|r| r.part_number != "BASE"));
    assert!(s.bill_of_materials(BomView::PartsOnly).iter().all(|r| r.part_number != "BASE"));
    let after = s.physical_properties(None).unwrap();
    assert!((before.volume - after.volume - 0.001).abs() < 1e-9, "{before:?} {after:?}");
    // A reference subassembly takes its children with it.
    s.set_bom_structure(kit, BomStructure::Reference).unwrap();
    assert_eq!(lines(&s, BomView::PartsOnly), vec![l("1", "BOLT", 1), l("2", "PLATE", 1)]);
    assert!((after.volume - s.physical_properties(None).unwrap().volume - 0.002).abs() < 1e-9);
    assert!(s.undo());
    assert_eq!(s.bom_structure(kit), BomStructure::Phantom);
}

#[test]
fn purchased_and_inseparable_are_single_parts_in_parts_only() {
    let (mut s, frame, _, [_, _, c, _, _]) = cell();
    s.set_bom_structure(frame, BomStructure::Purchased).unwrap();
    // Structured: its children still listed.
    assert_eq!(
        lines(&s, BomView::Structured)[..3],
        [l("1", "Frame", 1), l("1.1", "BOLT", 1), l("1.2", "PLATE", 1)]
    );
    assert_eq!(
        lines(&s, BomView::PartsOnly),
        vec![l("1", "Frame", 1), l("2", "BOLT", 1), l("3", "MOTOR", 1), l("4", "BASE", 1)]
    );
    // Inseparable: its normal children hide, a purchased child still shows.
    s.set_bom_structure(frame, BomStructure::Inseparable).unwrap();
    s.set_bom_structure(key(c), BomStructure::Purchased).unwrap();
    assert_eq!(
        lines(&s, BomView::PartsOnly),
        vec![
            l("1", "Frame", 1),
            l("2", "PLATE", 1),
            l("3", "BOLT", 1),
            l("4", "MOTOR", 1),
            l("5", "BASE", 1)
        ]
    );
    assert_eq!(s.bill_of_materials(BomView::PartsOnly)[0].structure, BomStructure::Inseparable);
    // Back to Normal stores nothing.
    s.set_bom_structure(frame, BomStructure::Normal).unwrap();
    s.set_bom_structure(key(c), BomStructure::Normal).unwrap();
    assert!(s.doc().timeline().is_none_or(|(_, t)| t.components.bom.is_empty()));
}

#[test]
fn csv_export_quotes_fields_and_unknown_rows_are_refused() {
    let (mut s, _, _, _) = cell();
    let csv = s.bom_csv(BomView::PartsOnly);
    let rows: Vec<&str> = csv.split("\r\n").collect();
    assert_eq!(rows[0], "Item,Part Number,Quantity,Description,BOM Structure");
    assert_eq!(rows[1], "1,BOLT,2,M8 bolt,Normal");
    assert_eq!(rows[3], "3,MOTOR,1,\"Servo, 400 W\",Normal");
    assert_eq!(rows.len(), 6, "{csv}");
    assert!(s.bom_csv(BomView::Structured).contains("\r\n1.2,PLATE,1,,Normal\r\n"));
    // A formula-looking value is not run by a spreadsheet.
    let m = s.bill_of_materials(BomView::PartsOnly)[2].keys[0];
    let TreeKey::Component(occ) = m else { panic!() };
    s.set_properties(Some(occ), IProperties { part_number: "=1+2".into(), ..IProperties::default() })
        .unwrap();
    assert!(s.bom_csv(BomView::PartsOnly).contains("\r\n3,'=1+2,1,,Normal\r\n"));
    // A row that is not in the tree.
    let gone = TreeKey::Subassembly(gc_doc::SubassemblyId(99));
    assert!(s.set_bom_structure(gone, BomStructure::Phantom).is_err());
}
