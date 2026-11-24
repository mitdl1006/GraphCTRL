//! QA boundary tests for the newest BOM and iProperties data: BOM settings (merge rows on part number, excluded part
//! numbers, hide suppressed), Static Quantity, "Varies", iProperties Status and Custom, Include QTY Overrides and
//! the inertial properties. Inventor 2026 help: To Work with Bill of Materials GUID-7A1FC6F1-8833-4A76-9E2B-AA6FE5FDF223,
//! Part Number Row Merge Settings GUID-EBBCAEA4-B876-4B39-8D33-F3DC9E18E620, About BOM Quantity
//! GUID-37BA8DB6-E39C-4C96-AAF1-E7B342F20D0E, iProperties Reference GUID-8ABF17C5-B9EB-47B5-92CF-9FBD335334E3,
//! Physical Tab Reference GUID-877564AC-78CE-4216-9913-003A2DE8D698. Refused edits leave the document and the history
//! untouched; accepted ones are one undo step that restores exactly (redo repeats).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::disallowed_methods,
    reason = "test code"
)]

use gc_core::Id;
use gc_doc::{
    Body, BomSettings, CustomProperty, EditError, IProperties, Occurrence, Placement, Solid, TreeKey,
};
use gc_session::{BomView, Session};

fn step<T>(s: &mut Session, what: &str, f: impl FnOnce(&mut Session) -> Result<T, EditError>) -> Option<T> {
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    match f(s) {
        Err(_) => {
            assert!(s.doc().same_content(&before), "{what} changed the document");
            assert_eq!(s.history().undo_depth(), depth, "{what}");
            None
        }
        Ok(v) => {
            assert_eq!(s.history().undo_depth(), depth + 1, "{what}: one undo step");
            let after = s.doc().clone();
            assert!(s.undo());
            assert!(s.doc().same_content(&before), "{what}: undo restores");
            assert!(s.redo());
            assert!(s.doc().same_content(&after), "{what}: redo repeats");
            Some(v)
        }
    }
}

fn occ(b: Id<Body>) -> Occurrence {
    Occurrence::Item(b.erase())
}

fn key(b: Id<Body>) -> TreeKey {
    TreeKey::Component(occ(b))
}

fn part(s: &mut Session, x: f64, number: &str, description: &str) -> Id<Body> {
    let b = s
        .add_body(Solid::Box { size: [0.1, 0.2, 0.3] }, Placement { xyz: [x, 0.0, 0.0], rpy: [0.0; 3] })
        .unwrap();
    let props =
        IProperties { part_number: number.into(), description: description.into(), ..IProperties::default() };
    s.set_properties(Some(occ(b)), props).unwrap();
    b
}

fn rows(s: &Session) -> Vec<(String, u32)> {
    s.bill_of_materials(BomView::PartsOnly).into_iter().map(|r| (r.part_number, r.quantity)).collect()
}

// ---- BOM settings ----

#[test]
fn rows_merge_on_a_part_number_unless_merging_is_off_or_the_number_is_excluded() {
    let mut s = Session::new();
    part(&mut s, 0.0, "P-1", "bolt");
    part(&mut s, 0.2, "P-1", "bolt");
    part(&mut s, 0.4, "P-2", "nut");
    assert_eq!(rows(&s), [("P-1".to_owned(), 2), ("P-2".to_owned(), 1)]);
    let base = s.bom_settings();
    // Off: one row per instance.
    let off = BomSettings { merge_part_numbers: false, ..base.clone() };
    assert!(step(&mut s, "merge off", |s| s.set_bom_settings(off.clone())).is_some());
    assert_eq!(rows(&s).len(), 3);
    assert!(rows(&s).iter().all(|(_, n)| *n == 1));
    // On, but P-1 is excluded: it is not merged, P-2 is the only one that would be.
    let excluded = BomSettings { merge_excluded: vec!["P-1".into()], ..base.clone() };
    assert!(step(&mut s, "exclude", |s| s.set_bom_settings(excluded.clone())).is_some());
    assert_eq!(rows(&s).len(), 3);
    // An excluded number nobody has, an empty string and a very long list change nothing about the rows.
    for list in [
        vec!["nobody".to_owned()],
        vec![String::new()],
        vec!["P-1".to_owned(); 3],
        (0..500).map(|i| format!("X{i}")).collect(),
    ] {
        let settings = BomSettings { merge_excluded: list.clone(), ..base.clone() };
        step(&mut s, "list", |s| s.set_bom_settings(settings.clone()));
    }
    // Setting the same settings again is no step or one harmless step.
    let before = s.doc().clone();
    let same = s.bom_settings();
    drop(s.set_bom_settings(same));
    assert!(s.doc().same_content(&before));
}

#[test]
fn a_suppressed_component_counts_zero_and_hide_suppressed_removes_the_row() {
    let mut s = Session::new();
    let a = part(&mut s, 0.0, "P-1", "bolt");
    let b = part(&mut s, 0.2, "P-1", "bolt");
    let c = part(&mut s, 0.4, "P-2", "nut");
    s.set_component(occ(a), gc_doc::ComponentFlag::Suppressed, true).unwrap();
    assert_eq!(rows(&s), [("P-1".to_owned(), 1), ("P-2".to_owned(), 1)], "the suppressed one is not counted");
    s.set_component(occ(b), gc_doc::ComponentFlag::Suppressed, true).unwrap();
    assert_eq!(rows(&s), [("P-1".to_owned(), 0), ("P-2".to_owned(), 1)], "all suppressed: a 0 QTY row");
    let hide = BomSettings { hide_suppressed: true, ..s.bom_settings() };
    assert!(step(&mut s, "hide suppressed", |s| s.set_bom_settings(hide.clone())).is_some());
    assert_eq!(rows(&s), [("P-2".to_owned(), 1)], "the 0 QTY row is gone");
    s.set_component(occ(a), gc_doc::ComponentFlag::Suppressed, false).unwrap();
    assert_eq!(rows(&s), [("P-1".to_owned(), 1), ("P-2".to_owned(), 1)]);
    // Everything suppressed and hidden: an empty BOM, no panic, CSV is the header alone.
    for x in [a, b, c] {
        s.set_component(occ(x), gc_doc::ComponentFlag::Suppressed, true).unwrap();
    }
    assert!(rows(&s).is_empty());
    assert_eq!(s.bom_csv(BomView::PartsOnly).lines().count(), 1);
}

#[test]
fn merged_rows_whose_descriptions_differ_say_varies_and_the_settings_are_saved() {
    let mut s = Session::new();
    part(&mut s, 0.0, "P-1", "bolt");
    part(&mut s, 0.2, "P-1", "screw");
    part(&mut s, 0.4, "P-2", "nut");
    let all = s.bill_of_materials(BomView::PartsOnly);
    assert_eq!(all[0].description, "Varies");
    assert_eq!(all[1].description, "nut");
    // Unmerged, each keeps its own description.
    let off =
        BomSettings { merge_part_numbers: false, hide_suppressed: true, merge_excluded: vec!["P-2".into()] };
    s.set_bom_settings(off.clone()).unwrap();
    let descriptions: Vec<_> =
        s.bill_of_materials(BomView::PartsOnly).into_iter().map(|r| r.description).collect();
    assert_eq!(descriptions, ["bolt", "screw", "nut"]);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bom.gcp");
    s.save_as(&path).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert_eq!(t.bom_settings(), off);
    assert!(t.doc().same_content(s.doc()));
    // The default settings are not written; an older file without them opens with the defaults.
    let mut plain = Session::new();
    part(&mut plain, 0.0, "P-1", "bolt");
    plain.save_as(&dir.path().join("plain.gcp")).unwrap();
    let text = std::fs::read_to_string(dir.path().join("plain.gcp")).unwrap();
    assert!(!text.contains("hide_suppressed\":true"));
    let mut u = Session::new();
    u.open(&dir.path().join("plain.gcp")).unwrap();
    assert_eq!(u.bom_settings(), BomSettings::default());
}

// ---- Static Quantity ----

#[test]
fn a_static_quantity_replaces_the_counted_one_and_is_cleared_with_none() {
    let mut s = Session::new();
    let a = part(&mut s, 0.0, "P-1", "bolt");
    part(&mut s, 0.2, "P-1", "bolt");
    let c = part(&mut s, 0.4, "P-2", "nut");
    assert!(step(&mut s, "static 10", |s| s.set_bom_quantity(key(a), Some(10))).is_some());
    let row = |s: &Session, n: &str| {
        s.bill_of_materials(BomView::PartsOnly).into_iter().find(|r| r.part_number == n).unwrap()
    };
    assert_eq!((row(&s, "P-1").quantity, row(&s, "P-1").static_quantity), (10, true));
    assert!(!row(&s, "P-2").static_quantity);
    // Any component of the row sets it: the other one of P-1 shows the same.
    assert!(s.bom_csv(BomView::PartsOnly).contains("P-1,10"), "{}", s.bom_csv(BomView::PartsOnly));
    // Zero, one, the largest number, a second value over the first, then back to counting.
    for n in [0, 1, u32::MAX, 7] {
        assert!(step(&mut s, "static n", |s| s.set_bom_quantity(key(a), Some(n))).is_some(), "{n}");
        assert_eq!(row(&s, "P-1").quantity, n);
    }
    assert!(step(&mut s, "clear", |s| s.set_bom_quantity(key(a), None)).is_some());
    assert_eq!((row(&s, "P-1").quantity, row(&s, "P-1").static_quantity), (2, false));
    // Clearing what has none: no step, or a harmless one. A key that is not a row is refused.
    let before = s.doc().clone();
    drop(s.set_bom_quantity(key(c), None));
    assert!(s.doc().same_content(&before));
    let gone = part(&mut s, 5.0, "G", "x");
    s.execute(&gc_doc::Command::Cell(gc_doc::CellOp::DeleteBody(gone))).unwrap();
    assert!(step(&mut s, "a deleted component", |s| s.set_bom_quantity(key(gone), Some(3))).is_none());
    // Saved and opened.
    s.set_bom_quantity(key(c), Some(4)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("q.gcp");
    s.save_as(&path).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert_eq!(row(&t, "P-2").quantity, 4);
    assert!(t.doc().same_content(s.doc()));
}

#[test]
fn a_static_quantity_survives_what_happens_to_its_row() {
    let mut s = Session::new();
    let a = part(&mut s, 0.0, "P-1", "bolt");
    let b = part(&mut s, 0.2, "P-1", "bolt");
    s.set_bom_quantity(key(a), Some(9)).unwrap();
    // One of the two goes: the row still has a number that is the static one or counted again, never garbage.
    s.execute(&gc_doc::Command::Cell(gc_doc::CellOp::DeleteBody(a))).unwrap();
    let q = rows(&s);
    assert_eq!(q.len(), 1);
    assert!(q[0].1 == 9 || q[0].1 == 1, "{q:?}");
    // Renumbered to another part number: the quantity follows the component or goes, not the old number.
    let other = IProperties { part_number: "P-9".into(), ..s.properties(Some(occ(b))) };
    s.set_properties(Some(occ(b)), other).unwrap();
    assert!(rows(&s).iter().all(|(n, _)| n == "P-9"));
    // Suppressed with a static quantity: the row follows the setting.
    s.set_component(occ(b), gc_doc::ComponentFlag::Suppressed, true).unwrap();
    let _ = rows(&s);
}

// ---- iProperties Status and Custom ----

#[test]
fn status_and_custom_properties_are_read_back_exactly_and_bad_names_are_refused() {
    let mut s = Session::new();
    let a = part(&mut s, 0.0, "P-1", "bolt");
    let set = |s: &mut Session, props: IProperties| s.set_properties(Some(occ(a)), props);
    let base = s.properties(Some(occ(a)));
    let with = |custom: Vec<(&str, &str)>, status: &str| IProperties {
        status: status.into(),
        custom: custom.into_iter().map(|(n, v)| CustomProperty { name: n.into(), value: v.into() }).collect(),
        ..base.clone()
    };
    let good = with(vec![("Material", "Steel"), ("보관 위치", "A-3")], "Released");
    assert!(step(&mut s, "good", |s| set(s, good.clone())).is_some());
    let back = s.properties(Some(occ(a)));
    assert_eq!(back.status, "Released");
    assert_eq!(back.custom_value("Material"), Some("Steel"));
    assert_eq!(back.custom_value("보관 위치"), Some("A-3"));
    assert_eq!(back.custom_value("material"), None, "names are case sensitive");
    // Empty or blank names and repeated names are refused with no trace.
    for bad in
        [with(vec![("", "x")], ""), with(vec![("   ", "x")], ""), with(vec![("A", "1"), ("A", "2")], "")]
    {
        assert!(step(&mut s, "bad custom", |s| set(s, bad.clone())).is_none(), "{:?}", bad.custom);
    }
    // Names that differ only by case or by a trailing space are two names or refused; either way all or nothing.
    for odd in [with(vec![("A", "1"), ("a", "2")], ""), with(vec![("A", "1"), ("A ", "2")], "")] {
        step(&mut s, "odd names", |s| set(s, odd.clone()));
    }
    // Long and strange values: kept exactly, saved and opened exactly.
    let nasty = with(
        vec![
            ("Long", &"é".repeat(5000)),
            ("Lines", "a\r\nb\n\tc"),
            ("Quote", "he said \"hi\", then ,;,"),
            ("Formula", "=1+1"),
        ],
        "상태 \u{1F600}",
    );
    assert!(step(&mut s, "nasty", |s| set(s, nasty.clone())).is_some());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.gcp");
    s.save_as(&path).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    let got = t.properties(Some(occ(a)));
    assert_eq!(got.status, "상태 \u{1F600}");
    assert_eq!(got.custom_value("Lines"), Some("a\r\nb\n\tc"));
    assert_eq!(got.custom_value("Long").unwrap().chars().count(), 5000);
    // The document's own properties take them too.
    let doc_level = with(vec![("Project code", "Z9")], "Draft");
    assert!(step(&mut s, "document level", |s| s.set_properties(None, doc_level.clone())).is_some());
    assert_eq!(s.properties(None).status, "Draft");
    // An older file with no status and no custom keys opens empty.
    let mut json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    fn strip(v: &mut serde_json::Value) {
        match v {
            serde_json::Value::Object(m) => {
                m.remove("status");
                m.remove("custom");
                m.values_mut().for_each(strip);
            }
            serde_json::Value::Array(a) => a.iter_mut().for_each(strip),
            _ => {}
        }
    }
    strip(&mut json);
    let old = dir.path().join("old.gcp");
    std::fs::write(&old, serde_json::to_string(&json).unwrap()).unwrap();
    let mut u = Session::new();
    u.open(&old).unwrap();
    assert!(u.properties(Some(occ(a))).status.is_empty() && u.properties(Some(occ(a))).custom.is_empty());
}

// ---- inertia and Include QTY Overrides ----

fn box_part(s: &mut Session, size: [f64; 3], at: Placement, density: f64) -> Id<Body> {
    let b = s.add_body(Solid::Box { size }, at).unwrap();
    let props = IProperties { density: Some(density), ..IProperties::default() };
    s.set_properties(Some(occ(b)), props).unwrap();
    b
}

#[test]
fn the_inertia_of_a_box_is_the_textbook_one_about_the_cog_and_about_the_origin() {
    let mut s = Session::new();
    let (a, b, c) = (0.2, 0.4, 0.6);
    let rho = 2500.0;
    let p = Placement { xyz: [0.3, -0.2, 0.5], rpy: [0.0; 3] };
    let part = box_part(&mut s, [a, b, c], p, rho);
    let m = rho * a * b * c;
    let physical = s.physical_properties(Some(occ(part))).unwrap();
    assert!((physical.mass - m).abs() < 1e-9 * m);
    let inertia = s.inertial_properties(Some(occ(part))).unwrap();
    let close = |x: f64, y: f64| (x - y).abs() <= 1e-8 * y.abs().max(1e-12) + 1e-12;
    // About the centre of gravity, along the axes: m(b²+c²)/12 and so on, no products for an aligned box.
    assert!(close(inertia.cog.ixx, m * (b * b + c * c) / 12.0), "{}", inertia.cog.ixx);
    assert!(close(inertia.cog.iyy, m * (a * a + c * c) / 12.0));
    assert!(close(inertia.cog.izz, m * (a * a + b * b) / 12.0));
    assert!(inertia.cog.ixy.abs() < 1e-9 && inertia.cog.ixz.abs() < 1e-9 && inertia.cog.iyz.abs() < 1e-9);
    // About the origin: the parallel-axis theorem.
    let (x, y, z) = (p.xyz[0], p.xyz[1], p.xyz[2]);
    assert!(close(inertia.global.ixx, inertia.cog.ixx + m * (y * y + z * z)));
    assert!(close(inertia.global.iyy, inertia.cog.iyy + m * (x * x + z * z)));
    assert!(close(inertia.global.izz, inertia.cog.izz + m * (x * x + y * y)));
    assert!(
        close(inertia.global.ixy, m * x * y)
            && close(inertia.global.ixz, m * x * z)
            && close(inertia.global.iyz, m * y * z)
    );
    // The principal moments are the three, smallest first or in some order; their set is the textbook set.
    let mut got = inertia.principal;
    got.sort_by(f64::total_cmp);
    let mut want = [m * (b * b + c * c) / 12.0, m * (a * a + c * c) / 12.0, m * (a * a + b * b) / 12.0];
    want.sort_by(f64::total_cmp);
    for (g, w) in got.iter().zip(want) {
        assert!(close(*g, w), "{got:?} vs {want:?}");
    }
    // The same box turned about Z: the moments about the COG along the axes change, the principal ones do not.
    let turned = Placement { xyz: p.xyz, rpy: [0.0, 0.0, 0.7] };
    s.free_move(part.erase(), turned).unwrap();
    let again = s.inertial_properties(Some(occ(part))).unwrap();
    let mut got = again.principal;
    got.sort_by(f64::total_cmp);
    for (g, w) in got.iter().zip(want) {
        assert!(close(*g, w), "turned: {got:?} vs {want:?}");
    }
    assert!(again.cog.ixy.abs() > 1e-6, "a turned box has a product of inertia about the axes");
    assert!(close(again.cog.izz, m * (a * a + b * b) / 12.0), "turning about Z leaves Izz");
}

#[test]
fn nothing_or_a_zero_mass_has_no_inertia_and_a_cube_has_three_equal_principal_moments() {
    let mut s = Session::new();
    assert!(s.inertial_properties(None).is_none(), "an empty document");
    let cube = box_part(&mut s, [0.3; 3], Placement { xyz: [1.0, 2.0, 3.0], rpy: [0.4, 0.5, 0.6] }, 1000.0);
    let i = s.inertial_properties(Some(occ(cube))).unwrap();
    let m = 1000.0 * 0.027;
    for v in i.principal {
        assert!((v - m * 0.3 * 0.3 / 6.0).abs() < 1e-9, "{v}");
    }
    assert!(i.rotation_to_principal.iter().all(|v| v.is_finite()));
    // A sphere and a cylinder have closed forms too.
    let ball = s.add_body(Solid::Sphere { radius: 0.2 }, Placement::default()).unwrap();
    let props = IProperties { density: Some(1000.0), ..IProperties::default() };
    s.set_properties(Some(occ(ball)), props).unwrap();
    let mb = 1000.0 * 4.0 / 3.0 * std::f64::consts::PI * 0.2f64.powi(3);
    let ib = s.inertial_properties(Some(occ(ball))).unwrap();
    assert!((ib.cog.ixx - 0.4 * mb * 0.04).abs() < 1e-6 * mb, "{}", ib.cog.ixx);
    // A single-component target is not changed by the QTY option; a bad target is None.
    let gone = s.add_body(Solid::Box { size: [0.1; 3] }, Placement::default()).unwrap();
    s.execute(&gc_doc::Command::Cell(gc_doc::CellOp::DeleteBody(gone))).unwrap();
    assert!(s.inertial_properties(Some(occ(gone))).is_none());
    assert!(s.physical_properties_with(Some(occ(gone)), true).is_none());
}

#[test]
fn include_qty_overrides_scales_the_mass_of_the_row_and_leaves_the_inertia_alone() {
    let mut s = Session::new();
    let a = box_part(&mut s, [0.1; 3], Placement { xyz: [0.0; 3], rpy: [0.0; 3] }, 1000.0);
    let b = box_part(&mut s, [0.1; 3], Placement { xyz: [0.5, 0.0, 0.0], rpy: [0.0; 3] }, 1000.0);
    for x in [a, b] {
        let props = IProperties { part_number: "P-1".into(), ..s.properties(Some(occ(x))) };
        s.set_properties(Some(occ(x)), props).unwrap();
    }
    let counted = s.physical_properties_with(None, true).unwrap();
    assert_eq!(
        counted.mass,
        s.physical_properties(None).unwrap().mass,
        "no static quantity: the option changes nothing"
    );
    s.set_bom_quantity(key(a), Some(10)).unwrap();
    let off = s.physical_properties_with(None, false).unwrap();
    let on = s.physical_properties_with(None, true).unwrap();
    assert!((off.mass - 2.0).abs() < 1e-9, "{}", off.mass);
    assert!((on.mass - 10.0).abs() < 1e-9, "ten of the 1 kg parts: {}", on.mass);
    assert!((on.volume - 5.0 * off.volume).abs() < 1e-12 && (on.area - 5.0 * off.area).abs() < 1e-12);
    // One component asked alone: unchanged.
    let one_off = s.physical_properties_with(Some(occ(a)), false).unwrap();
    let one_on = s.physical_properties_with(Some(occ(a)), true).unwrap();
    assert_eq!(one_off.mass, one_on.mass);
    // The inertia ignores the overrides ("Inertial Properties and Center of Gravity do not include ... overrides").
    let i1 = s.inertial_properties(None).unwrap();
    s.set_bom_quantity(key(a), None).unwrap();
    let i2 = s.inertial_properties(None).unwrap();
    assert_eq!(i1, i2);
    // Zero static quantity: the row weighs nothing (or the parts are left as counted); never a negative or NaN mass.
    s.set_bom_quantity(key(a), Some(0)).unwrap();
    let zero = s.physical_properties_with(None, true);
    if let Some(z) = zero {
        assert!(z.mass.is_finite() && z.mass >= 0.0, "{}", z.mass);
    }
    // A static quantity on an assembly with a mass override: still finite.
    let props = IProperties { mass_override: Some(5.0), ..s.properties(Some(occ(b))) };
    s.set_properties(Some(occ(b)), props).unwrap();
    s.set_bom_quantity(key(a), Some(3)).unwrap();
    let mixed = s.physical_properties_with(None, true).unwrap();
    assert!(mixed.mass.is_finite() && mixed.mass > 0.0);
}
