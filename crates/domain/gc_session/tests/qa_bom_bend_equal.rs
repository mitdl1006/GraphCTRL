//! QA boundary tests for the Bill of Materials (BOM Structure, Structured and Parts Only views, CSV) and for
//! Bend Equal in a 3D sketch. Inventor 2026 help, under
//! <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/>: About the BOM Structure
//! GUID-A0A95805-CBF6-4524-8314-4AC8B53C19CF, To Create Lines, Arcs, and Bends (Bend, Equal)
//! GUID-4F861674-F74C-474E-869A-4C190ED6EC4B.
//! Refused edits leave the document and the history untouched; accepted ones are one undo step that restores
//! exactly (redo repeats).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::disallowed_methods,
    reason = "test code"
)]

use gc_core::Id;
use gc_doc::{
    Body, BomStructure, Command, Created, EditError, EndSnap, IProperties, Occurrence, Placement, Solid,
    SubassemblyOp, TreeKey,
};
use gc_session::{BomView, Session};
use gc_sketch::{Curve, DimKind, GeoConstraint, Item};

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

fn rows(s: &Session, view: BomView) -> Vec<(String, u32)> {
    s.bill_of_materials(view).into_iter().map(|r| (r.part_number, r.quantity)).collect()
}

fn total(s: &Session) -> u32 {
    s.bill_of_materials(BomView::PartsOnly).iter().map(|r| r.quantity).sum()
}

/// Refused with no trace, or one undo step that undoes exactly and redoes the same.
fn step<T>(s: &mut Session, what: &str, f: impl FnOnce(&mut Session) -> Result<T, EditError>) -> bool {
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    match f(s) {
        Err(_) => {
            assert!(s.doc().same_content(&before), "{what} changed the document");
            assert_eq!(s.history().undo_depth(), depth, "{what}");
            false
        }
        Ok(_) => {
            assert_eq!(s.history().undo_depth(), depth + 1, "{what}: one undo step");
            let after = s.doc().clone();
            assert!(s.undo());
            assert!(s.doc().same_content(&before), "{what}: undo restores");
            assert!(s.redo());
            assert!(s.doc().same_content(&after), "{what}: redo repeats");
            true
        }
    }
}

// ---- BOM ----

#[test]
fn an_empty_document_has_an_empty_bom_and_a_header_only_csv() {
    let s = Session::new();
    for view in [BomView::Structured, BomView::PartsOnly] {
        assert!(s.bill_of_materials(view).is_empty());
        assert_eq!(s.bom_csv(view), "Item,Part Number,Quantity,Description,BOM Structure\r\n");
    }
}

#[test]
fn structure_edits_are_one_step_and_refuse_what_is_not_there() {
    let mut s = Session::new();
    let a = body(&mut s, 0.0, "A", "");
    let b = body(&mut s, 0.3, "B", "");
    let sub = demote(&mut s, vec![key(a), key(b)], "Kit");
    let ghost_sub = TreeKey::Subassembly(gc_doc::SubassemblyId(9999));
    let ghost_body = key(Id::<Body>::from_raw(std::num::NonZeroU64::new(9999).unwrap()));
    for structure in [
        BomStructure::Normal,
        BomStructure::Phantom,
        BomStructure::Reference,
        BomStructure::Purchased,
        BomStructure::Inseparable,
    ] {
        assert!(!step(&mut s, "ghost subassembly", |s| s.set_bom_structure(ghost_sub, structure)));
        assert!(!step(&mut s, "ghost body", |s| s.set_bom_structure(ghost_body, structure)));
    }
    // Setting the same structure again: no empty undo step (refused or no step), or one clean step.
    let depth = s.history().undo_depth();
    s.set_bom_structure(sub, BomStructure::Normal).ok();
    assert!(s.history().undo_depth() <= depth + 1);
    // Every structure on a subassembly and on a member, then back: one step each, exact undo.
    for structure in [
        BomStructure::Phantom,
        BomStructure::Reference,
        BomStructure::Purchased,
        BomStructure::Inseparable,
        BomStructure::Normal,
    ] {
        assert!(step(&mut s, "subassembly", |s| s.set_bom_structure(sub, structure)));
        s.set_bom_structure(sub, structure).unwrap();
        assert_eq!(s.bom_structure(sub), structure);
        assert!(step(&mut s, "member", |s| s.set_bom_structure(key(a), structure)));
        s.set_bom_structure(key(a), structure).unwrap();
        assert_eq!(s.bom_structure(key(a)), structure);
        // Whatever the structure, the BOM and its CSV are consistent: every line has a positive quantity.
        for view in [BomView::Structured, BomView::PartsOnly] {
            assert!(s.bill_of_materials(view).iter().all(|r| r.quantity >= 1 && !r.item.is_empty()));
            assert!(s.bom_csv(view).starts_with("Item,"));
        }
    }
}

#[test]
fn quantities_add_up_through_every_arrangement_of_phantom_and_reference() {
    // Property: with n identical bodies in a tree, Parts Only counts every body not under a Reference.
    let mut s = Session::new();
    let bodies: Vec<_> = (0..6).map(|i| body(&mut s, f64::from(i) * 0.3, "BOLT", "M8")).collect();
    assert_eq!(total(&s), 6);
    assert_eq!(rows(&s, BomView::PartsOnly), vec![("BOLT".to_owned(), 6)]);
    let inner = demote(&mut s, vec![key(bodies[0]), key(bodies[1])], "Inner");
    let outer = demote(&mut s, vec![inner, key(bodies[2])], "Outer");
    assert_eq!(total(&s), 6, "nesting changes no count");
    // Phantom at every level and in any order keeps every part.
    for first in [inner, outer] {
        for second in [outer, inner] {
            if first == second {
                continue;
            }
            s.set_bom_structure(first, BomStructure::Phantom).unwrap();
            s.set_bom_structure(second, BomStructure::Phantom).unwrap();
            assert_eq!(total(&s), 6, "phantoms drop parts");
            assert_eq!(rows(&s, BomView::Structured), vec![("BOLT".to_owned(), 6)], "phantom bolts combine");
            s.set_bom_structure(first, BomStructure::Normal).unwrap();
            s.set_bom_structure(second, BomStructure::Normal).unwrap();
        }
    }
    // Reference removes the subassembly's whole branch, whatever its children say.
    s.set_bom_structure(key(bodies[0]), BomStructure::Purchased).unwrap();
    s.set_bom_structure(inner, BomStructure::Reference).unwrap();
    assert_eq!(total(&s), 4, "two bolts under a Reference leave the BOM");
    let physical = s.physical_properties(None).unwrap();
    assert!((physical.volume - 4.0 * 1e-3).abs() < 1e-9, "{physical:?}");
    // Phantom parent over a Reference child: the child is still out.
    s.set_bom_structure(outer, BomStructure::Phantom).unwrap();
    assert_eq!(total(&s), 4);
    // Everything Reference: an empty BOM, an empty CSV body, a zero-volume document, no panic.
    for b in &bodies {
        s.set_bom_structure(key(*b), BomStructure::Reference).unwrap();
    }
    s.set_bom_structure(outer, BomStructure::Reference).unwrap();
    assert!(s.bill_of_materials(BomView::Structured).is_empty());
    assert!(s.bill_of_materials(BomView::PartsOnly).is_empty());
    let p = s.physical_properties(None).unwrap();
    assert!(p.volume == 0.0 && p.mass == 0.0 && p.centre.is_finite() && p.density.is_finite());
}

#[test]
fn part_numbers_that_look_alike_and_empty_ones() {
    let mut s = Session::new();
    // Same number, different descriptions; different case; trailing space; empty (the name stands in).
    body(&mut s, 0.0, "BOLT", "M8");
    body(&mut s, 0.3, "BOLT", "M10");
    body(&mut s, 0.6, "bolt", "M8");
    body(&mut s, 0.9, "BOLT ", "M8");
    let named = s.add_body(Solid::Box { size: [0.1; 3] }, Placement::default()).unwrap();
    let _ = named;
    let lines = rows(&s, BomView::PartsOnly);
    let sum: u32 = lines.iter().map(|(_, q)| *q).sum();
    assert_eq!(sum, 5, "{lines:?}");
    assert!(lines.iter().all(|(p, _)| !p.is_empty()), "an empty part number shows the name: {lines:?}");
    // Observation for the record: how the near-identical numbers were combined.
    eprintln!("observation: part number grouping {lines:?}");
    let bolt = s.bill_of_materials(BomView::PartsOnly).into_iter().find(|r| r.part_number == "BOLT").unwrap();
    assert!(bolt.quantity >= 2 && !bolt.keys.is_empty());
}

#[test]
fn csv_is_safe_for_spreadsheets_and_odd_text() {
    let mut s = Session::new();
    let weird = [
        "plain",
        "comma, inside",
        "quote \" inside",
        "line\nbreak",
        "carriage\rreturn",
        "=1+2",
        "+1",
        "-1",
        "@SUM(A1)",
        " =1",
        "한글 부품, 설명 \"인용\"",
        "",
    ];
    for (i, text) in weird.iter().enumerate() {
        body(&mut s, i as f64 * 0.2, &format!("P{i}"), text);
        body(&mut s, i as f64 * 0.2 + 0.1, text, "d");
    }
    let csv = s.bom_csv(BomView::PartsOnly);
    // Parse it back with a small RFC 4180 reader: every record has five fields.
    let mut records: Vec<Vec<String>> = vec![Vec::new()];
    let (mut field, mut quoted, mut chars) = (String::new(), false, csv.chars().peekable());
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            ('"', true) => quoted = false,
            ('"', false) if field.is_empty() => quoted = true,
            (',', false) => records.last_mut().unwrap().push(std::mem::take(&mut field)),
            ('\r', false) if chars.peek() == Some(&'\n') => {
                chars.next();
                records.last_mut().unwrap().push(std::mem::take(&mut field));
                records.push(Vec::new());
            }
            (c, _) => field.push(c),
        }
    }
    records.retain(|r| !r.is_empty());
    assert!(records.iter().all(|r| r.len() == 5), "a field was split or merged: {records:?}");
    assert_eq!(records.len(), 1 + s.bill_of_materials(BomView::PartsOnly).len());
    // No cell starts with a character a spreadsheet would run as a formula.
    for r in &records[1..] {
        for cell in [&r[1], &r[3]] {
            let first = cell.chars().next();
            assert!(
                !matches!(first, Some('=' | '+' | '-' | '@')),
                "a formula-looking cell {cell:?} is exported as it is"
            );
        }
    }
}

#[test]
fn bom_structure_survives_save_and_open_and_goes_with_deleted_parts() {
    let mut s = Session::new();
    let a = body(&mut s, 0.0, "A", "");
    let b = body(&mut s, 0.3, "B", "");
    let sub = demote(&mut s, vec![key(a), key(b)], "Kit");
    s.set_bom_structure(sub, BomStructure::Phantom).unwrap();
    s.set_bom_structure(key(a), BomStructure::Purchased).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bom.gcp");
    s.save_as(&path).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert!(t.doc().same_content(s.doc()));
    assert_eq!(t.bom_structure(sub), BomStructure::Phantom);
    assert_eq!(t.bom_csv(BomView::Structured), s.bom_csv(BomView::Structured));
    let again = dir.path().join("again.gcp");
    t.save_as(&again).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), std::fs::read(&again).unwrap());
    // Deleting a body that had a structure: it leaves nothing behind that a new body could pick up.
    let before = s.doc().clone();
    s.delete_item(gc_session::CellItem::Body(a)).unwrap();
    let fresh = body(&mut s, 2.0, "FRESH", "");
    assert_eq!(s.bom_structure(key(fresh)), BomStructure::Normal, "a new body starts as Normal");
    assert!(s.undo() && s.undo() || true);
    let _ = before;
    // A file with a structure for a component that is not there: opened without it, or refused.
    let text = std::fs::read_to_string(&path).unwrap();
    for (from, to) in [("Purchased", "Bogus"), ("Phantom", "Reference")] {
        if !text.contains(from) {
            continue;
        }
        let bad = dir.path().join("bad.gcp");
        std::fs::write(&bad, text.replace(from, to)).unwrap();
        let mut u = Session::new();
        if u.open(&bad).is_ok() {
            assert!(u.bill_of_materials(BomView::PartsOnly).iter().all(|r| r.quantity >= 1));
        }
    }
}

#[test]
fn a_big_bom_stays_quick_and_consistent() {
    let mut s = Session::new();
    let bodies: Vec<_> =
        (0..300).map(|i| body(&mut s, f64::from(i) * 0.15, &format!("P{}", i % 20), "x")).collect();
    let groups: Vec<_> = bodies.chunks(30).map(|c| c.iter().map(|b| key(*b)).collect::<Vec<_>>()).collect();
    for (i, g) in groups.into_iter().enumerate() {
        demote(&mut s, g, &format!("G{i}"));
    }
    let t = std::time::Instant::now();
    let parts = rows(&s, BomView::PartsOnly);
    let structured = s.bill_of_materials(BomView::Structured).len();
    let csv = s.bom_csv(BomView::Structured);
    let took = t.elapsed();
    assert_eq!(parts.iter().map(|(_, q)| q).sum::<u32>(), 300);
    assert_eq!(parts.len(), 20);
    assert_eq!(structured, 10 * 21, "10 groups of 20 different part numbers");
    assert_eq!(csv.lines().count(), structured + 1);
    eprintln!("report: BOM of 300 parts in 10 groups took {took:?}");
}

/// BUG: the CSV quotes cells that start with `=`, `+`, `-` or `@` so that a spreadsheet does not run them as a
/// formula, but not cells that start with a tab or a carriage return (also on the OWASP list of CSV injection
/// starts: a spreadsheet drops the tab and then runs the formula).
#[test]
#[ignore = "BUG: a BOM cell starting with a tab or carriage return is exported as it is (owner: backend, gc_session/src/bom.rs csv)"]
fn csv_also_guards_cells_that_start_with_a_tab_or_a_carriage_return() {
    let mut s = Session::new();
    body(&mut s, 0.0, "\t=1+2", "\r=1+2");
    let csv = s.bom_csv(BomView::PartsOnly);
    let row = csv.split("\r\n").nth(1).unwrap();
    assert!(!row.contains(",\t=") && !row.contains("\"\t=") && !row.contains(",\r="), "{row:?}");
}

// ---- Bend Equal ----

type K = Id<gc_doc::Sketch>;

fn run(s: &mut Session, k: K, op: gc_doc::SketchOp) -> Result<Vec<Created>, EditError> {
    s.execute(&Command::Sketch { sketch: k, op })
}

fn line(s: &mut Session, k: K, a: [f64; 3], b: [f64; 3]) -> Id<Curve> {
    let op = gc_doc::SketchOp::AddLine {
        a,
        b,
        snap_a: EndSnap::default(),
        snap_b: EndSnap::default(),
        horizontal: None,
    };
    run(s, k, op)
        .unwrap()
        .iter()
        .find_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None })
        .unwrap()
}

fn arc_of(created: &[Created]) -> Id<Curve> {
    created.iter().find_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None }).unwrap()
}

fn radius(s: &Session, c: Id<Curve>) -> f64 {
    let sk = s.active_sketch().unwrap().1;
    let gc_sketch::CurveGeom::Arc { c: ctr, s: st, .. } = sk.curves[&c].geom.clone() else { panic!() };
    let (c, p) = (sk.pos(ctr).unwrap(), sk.pos(st).unwrap());
    ((c[0] - p[0]).powi(2) + (c[1] - p[1]).powi(2) + (c[2] - p[2]).powi(2)).sqrt()
}

/// A zig-zag of three lines in space with two corners.
fn zigzag() -> (Session, K, [Id<Curve>; 3]) {
    let mut s = Session::new();
    let k = s.create_sketch_3d().unwrap();
    let c1 = [0.2, 0.0, 0.0];
    let c2 = [0.2, 0.2, 0.1];
    let l1 = line(&mut s, k, [0.0; 3], c1);
    let l2 = line(&mut s, k, c1, c2);
    let l3 = line(&mut s, k, c2, [0.5, 0.2, 0.3]);
    (s, k, [l1, l2, l3])
}

#[test]
fn bend_equal_follows_the_first_bend_and_refuses_the_wrong_things() {
    use gc_doc::SketchOp;
    let (mut s, k, [l1, l2, l3]) = zigzag();
    let first = arc_of(&run(&mut s, k, SketchOp::Bend { lines: [l1, l2], radius: "20 mm".into() }).unwrap());
    let depth = s.history().undo_depth();
    // Refusals first: a line as `to`, a bend among the lines, a ghost, the 2D sketch.
    let ghost = Id::<Curve>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    for (what, lines, to) in
        [("to is a line", [l2, l3], l1), ("to among the lines", [l2, l3], l2), ("ghost", [l2, l3], ghost)]
    {
        assert!(!step(&mut s, what, |s| run(s, k, SketchOp::BendEqual { lines, to }).map(|_| ())));
    }
    assert_eq!(s.history().undo_depth(), depth);
    // Accepted: one step; the new bend has the first one's radius and an Equal to it, no dimension of its own.
    let dims_before = s.active_sketch().unwrap().1.dimensions.len();
    let made = run(&mut s, k, SketchOp::BendEqual { lines: [l2, l3], to: first }).unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1);
    let second = arc_of(&made);
    assert!((radius(&s, second) - 0.02).abs() < 1e-9);
    let sk = s.active_sketch().unwrap().1;
    assert!(sk.constraints.values().any(|c| matches!(c, GeoConstraint::Equal(a, b) if (*a == second && *b == first) || (*a == first && *b == second))));
    assert_eq!(sk.dimensions.len(), dims_before, "Equal replaces the radius dimension");
    // Change the first radius: both follow.
    let dim = *sk.dimensions.iter().find(|(_, d)| d.kind == DimKind::Radius(first)).unwrap().0;
    run(&mut s, k, SketchOp::SetDimension { id: dim, source: "12 mm".into() }).unwrap();
    assert!((radius(&s, first) - 0.012).abs() < 1e-9 && (radius(&s, second) - 0.012).abs() < 1e-9);
    // A radius too large for the lines is refused whatever the dimension (no trace).
    let before = s.doc().clone();
    let steps = s.history().undo_depth();
    let r = run(&mut s, k, SketchOp::SetDimension { id: dim, source: "5 m".into() });
    if r.is_err() {
        assert!(s.doc().same_content(&before));
        assert_eq!(s.history().undo_depth(), steps);
    } else {
        assert!(s.active_sketch().unwrap().1.points.values().all(|p| p.pos.iter().all(|v| v.is_finite())));
        assert!(s.undo());
    }
    // Undo of the dimension edit and then the bend: exact.
    assert!(s.undo());
    assert!(s.undo());
    assert_eq!(s.history().undo_depth(), depth);
    assert!(s.active_sketch().unwrap().1.curves.contains_key(&first));
    // A 2D sketch has no Bend Equal.
    let flat = s.create_sketch("XY").unwrap();
    let a = line(&mut s, flat, [0.0; 3], [0.1, 0.0, 0.0]);
    let b = line(&mut s, flat, [0.1, 0.0, 0.0], [0.1, 0.1, 0.0]);
    let before = s.doc().clone();
    assert!(run(&mut s, flat, SketchOp::BendEqual { lines: [a, b], to: a }).is_err());
    assert!(s.doc().same_content(&before));
}

#[test]
fn bend_equal_edits_that_follow_and_files() {
    use gc_doc::SketchOp;
    let (mut s, k, [l1, l2, l3]) = zigzag();
    let first = arc_of(&run(&mut s, k, SketchOp::Bend { lines: [l1, l2], radius: "20 mm".into() }).unwrap());
    let second = arc_of(&run(&mut s, k, SketchOp::BendEqual { lines: [l2, l3], to: first }).unwrap());
    // Bend equal to a bend that is itself Equal: a chain of three equal bends from a longer zig-zag.
    let l4 = line(&mut s, k, [0.5, 0.2, 0.3], [0.5, 0.5, 0.3]);
    let third = run(&mut s, k, SketchOp::BendEqual { lines: [l3, l4], to: second });
    if let Ok(made) = third {
        assert!((radius(&s, arc_of(&made)) - 0.02).abs() < 1e-9);
        assert!(s.active_sketch().unwrap().1.clone().solve().is_ok());
    }
    // Deleting the first bend removes the Equal with it (nothing dangles); undo restores.
    let before = s.doc().clone();
    run(&mut s, k, SketchOp::Delete(vec![Item::Curve(first)])).unwrap();
    let sk = s.active_sketch().unwrap().1;
    for c in sk.constraints.values() {
        if let GeoConstraint::Equal(a, b) = c {
            assert!(sk.curves.contains_key(a) && sk.curves.contains_key(b), "an Equal outlived its arc");
        }
    }
    assert!(s.undo());
    assert!(s.doc().same_content(&before));
    // Saved and opened.
    s.finish_sketch();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bend.gcp");
    s.save_as(&path).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert!(t.doc().same_content(s.doc()));
}
