//! iProperties of the document and components (Inventor 2026 help, iProperties Reference,
//! GUID-8ABF17C5-B9EB-47B5-92CF-9FBD335334E3), and the Physical tab worked out from the shapes (iProperties -
//! Physical Tab - Reference, GUID-877564AC-78CE-4216-9913-003A2DE8D698).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "tests")]

use gc_doc::{IProperties, Occurrence, Placement, Solid};
use gc_math::DVec3;
use gc_session::Session;

fn body(s: &mut Session, solid: Solid, xyz: [f64; 3]) -> gc_core::Id<gc_doc::Body> {
    s.add_body(solid, Placement { xyz, rpy: [0.0; 3] }).unwrap()
}

#[test]
fn summary_and_project_properties_are_saved_with_the_document() {
    let mut s = Session::new();
    let b = body(&mut s, Solid::Box { size: [0.1; 3] }, [0.0; 3]);
    let occ = Occurrence::Item(b.erase());
    // No part number of its own: the name stands for it.
    let name = s.part_number(occ);
    assert!(!name.is_empty());
    let doc = IProperties { title: "Cell 7".into(), author: "Planner".into(), ..IProperties::default() };
    let part = IProperties {
        part_number: "FX-100".into(),
        description: "Welding fixture".into(),
        density: Some(7850.0),
        ..IProperties::default()
    };
    let steps = s.history().undo_depth();
    s.set_properties(None, doc.clone()).unwrap();
    s.set_properties(Some(occ), part.clone()).unwrap();
    assert_eq!(s.history().undo_depth(), steps + 2);
    assert_eq!(s.properties(None), doc);
    assert_eq!(s.properties(Some(occ)), part);
    assert_eq!(s.part_number(occ), "FX-100");
    // Not valid: refused.
    let before = s.doc().clone();
    assert!(s.set_properties(Some(occ), IProperties { density: Some(-1.0), ..part.clone() }).is_err());
    assert!(
        s.set_properties(Some(occ), IProperties { mass_override: Some(f64::NAN), ..part.clone() }).is_err()
    );
    let ghost = Occurrence::Item(
        gc_core::Id::<gc_doc::Body>::from_raw(std::num::NonZeroU64::new(999).unwrap()).erase(),
    );
    assert!(s.set_properties(Some(ghost), part.clone()).is_err());
    assert!(s.doc().same_content(&before));
    // Saved and loaded.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("props.gcp");
    s.save_as(&path).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert_eq!(t.properties(None), doc);
    assert_eq!(t.properties(Some(occ)), part);
    // One undo step each.
    s.undo();
    assert_eq!(s.properties(Some(occ)), IProperties::default());
}

#[test]
fn physical_properties_come_from_the_shapes_and_densities() {
    let mut s = Session::new();
    let a = body(&mut s, Solid::Box { size: [0.1, 0.2, 0.3] }, [1.0, 0.0, 0.0]);
    let b = body(&mut s, Solid::Sphere { radius: 0.1 }, [0.0, 2.0, 0.0]);
    let (oa, ob) = (Occurrence::Item(a.erase()), Occurrence::Item(b.erase()));
    // The default material: 1 kg/m³.
    let p = s.physical_properties(Some(oa)).unwrap();
    assert!((p.volume - 0.006).abs() < 1e-12 && (p.mass - 0.006).abs() < 1e-12);
    assert!((p.area - 2.0 * (0.02 + 0.06 + 0.03)).abs() < 1e-12);
    assert!((p.centre - DVec3::new(1.0, 0.0, 0.0)).length() < 1e-12);
    // Steel.
    s.set_properties(Some(oa), IProperties { density: Some(7850.0), ..IProperties::default() }).unwrap();
    let p = s.physical_properties(Some(oa)).unwrap();
    assert!((p.mass - 47.1).abs() < 1e-9 && (p.density - 7850.0).abs() < 1e-9);
    // A mass override wins.
    s.set_properties(Some(ob), IProperties { mass_override: Some(47.1), ..IProperties::default() }).unwrap();
    let sphere = s.physical_properties(Some(ob)).unwrap();
    assert!(sphere.overridden && (sphere.mass - 47.1).abs() < 1e-12);
    assert!((sphere.volume - 4.0 / 3.0 * std::f64::consts::PI * 1e-3).abs() < 1e-12);
    // The whole document: masses add, the centre of gravity halfway between equal masses.
    let all = s.physical_properties(None).unwrap();
    assert!((all.mass - 94.2).abs() < 1e-9 && all.overridden);
    assert!((all.centre - DVec3::new(0.5, 1.0, 0.0)).length() < 1e-9, "{:?}", all.centre);
}

#[test]
fn a_robot_has_physical_properties_from_its_links() {
    let path = format!("{}/../../../fixtures/urdf/six_axis.urdf", env!("CARGO_MANIFEST_DIR"));
    let mut s = Session::new();
    let robot = s.add_robot("Robot1", &std::fs::read_to_string(path).unwrap()).unwrap();
    let p = s.physical_properties(Some(Occurrence::Item(robot.erase()))).unwrap();
    assert!(p.volume > 0.0 && p.area > 0.0 && p.mass > 0.0, "{p:?}");
    // Above the base, the arm standing up.
    assert!(p.centre.z > 0.0, "{p:?}");
}
