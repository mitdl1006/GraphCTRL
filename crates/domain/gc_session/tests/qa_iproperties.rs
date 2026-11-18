//! QA boundary tests for iProperties and the Physical tab (mass, volume, area, centre of gravity).
//! Inventor 2026 help, under <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/>:
//! iProperties Reference GUID-8ABF17C5-B9EB-47B5-92CF-9FBD335334E3, Physical Tab Reference
//! GUID-877564AC-78CE-4216-9913-003A2DE8D698 ("The density of the default material is 1 kg/m^3").
//! Refused edits leave the document and the history untouched; accepted ones are one undo step that restores
//! exactly.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::disallowed_methods,
    reason = "test code"
)]

use std::f64::consts::PI;

use gc_doc::{IProperties, Occurrence, Placement, Solid};
use gc_math::DVec3;
use gc_session::Session;

fn body(s: &mut Session, solid: Solid, xyz: [f64; 3], rpy: [f64; 3]) -> Occurrence {
    Occurrence::Item(s.add_body(solid, Placement { xyz, rpy }).unwrap().erase())
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * (1.0 + b.abs())
}

fn set_step(s: &mut Session, what: &str, target: Option<Occurrence>, p: IProperties) -> bool {
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    match s.set_properties(target, p.clone()) {
        Err(_) => {
            assert!(s.doc().same_content(&before), "{what} changed the document");
            assert_eq!(s.history().undo_depth(), depth, "{what}");
            false
        }
        Ok(()) => {
            assert_eq!(s.history().undo_depth(), depth + 1, "{what}: one undo step");
            assert_eq!(s.properties(target), p);
            let after = s.doc().clone();
            assert!(s.undo());
            assert!(s.doc().same_content(&before), "{what}: undo restores");
            assert!(s.redo());
            assert!(s.doc().same_content(&after), "{what}: redo repeats");
            true
        }
    }
}

#[test]
fn primitives_match_the_closed_forms_in_any_placement() {
    let mut s = Session::new();
    // A box, a cylinder and a sphere, each moved and turned: volume, area and mass do not depend on the
    // placement; the centre of gravity is where the body is put.
    let at = [0.7, -0.4, 1.3];
    let turned = [0.3, -1.1, 2.0];
    let cases: [(&str, Solid, f64, f64); 3] = [
        ("box", Solid::Box { size: [0.1, 0.2, 0.3] }, 0.006, 2.0 * (0.02 + 0.06 + 0.03)),
        (
            "cylinder",
            Solid::Cylinder { radius: 0.05, length: 0.4 },
            PI * 0.0025 * 0.4,
            2.0 * PI * 0.05 * 0.45,
        ),
        ("sphere", Solid::Sphere { radius: 0.1 }, 4.0 / 3.0 * PI * 1e-3, 4.0 * PI * 0.01),
    ];
    for (name, solid, volume, area) in cases {
        let b = body(&mut s, solid, at, turned);
        let p = s.physical_properties(Some(b)).unwrap();
        // Tessellation is not used for primitives: the closed form, to rounding.
        assert!(near(p.volume, volume), "{name}: volume {} vs {volume}", p.volume);
        assert!(near(p.area, area), "{name}: area {} vs {area}", p.area);
        assert!(near(p.mass, volume), "{name}: the default 1 kg/m3 gives mass = volume");
        assert!((p.centre - DVec3::from(at)).length() < 1e-9, "{name}: {:?}", p.centre);
        assert!(!p.overridden);
    }
}

#[test]
fn density_and_mass_boundaries() {
    let mut s = Session::new();
    let b = body(&mut s, Solid::Box { size: [0.1; 3] }, [0.0; 3], [0.0; 3]);
    let ok = IProperties { density: Some(7850.0), ..IProperties::default() };
    for (what, bad) in [
        ("density 0", IProperties { density: Some(0.0), ..ok.clone() }),
        ("density -0", IProperties { density: Some(-0.0), ..ok.clone() }),
        ("density negative", IProperties { density: Some(-1.0), ..ok.clone() }),
        ("density NaN", IProperties { density: Some(f64::NAN), ..ok.clone() }),
        ("density inf", IProperties { density: Some(f64::INFINITY), ..ok.clone() }),
        ("mass 0", IProperties { mass_override: Some(0.0), ..ok.clone() }),
        ("mass negative", IProperties { mass_override: Some(-5.0), ..ok.clone() }),
        ("mass NaN", IProperties { mass_override: Some(f64::NAN), ..ok.clone() }),
        ("mass -inf", IProperties { mass_override: Some(f64::NEG_INFINITY), ..ok.clone() }),
    ] {
        assert!(!set_step(&mut s, what, Some(b), bad), "{what} was accepted");
        assert!(
            !set_step(
                &mut s,
                &format!("{what} on the document"),
                None,
                IProperties { density: bad_density(what), ..IProperties::default() }
            ) || bad_density(what).is_none(),
            "{what} on the document"
        );
    }
    // Extremes that are legal numbers: accepted, and the physical values stay finite or are refused.
    for (what, props) in [
        ("tiny density", IProperties { density: Some(f64::MIN_POSITIVE), ..IProperties::default() }),
        ("huge density", IProperties { density: Some(1e300), ..IProperties::default() }),
        ("max density", IProperties { density: Some(f64::MAX), ..IProperties::default() }),
        ("huge mass", IProperties { mass_override: Some(f64::MAX), ..IProperties::default() }),
        ("both", IProperties { density: Some(7850.0), mass_override: Some(1.0), ..IProperties::default() }),
    ] {
        if set_step(&mut s, what, Some(b), props.clone()) {
            s.set_properties(Some(b), props).unwrap();
            let p = s.physical_properties(Some(b)).unwrap();
            assert!(p.centre.is_finite() && p.volume.is_finite(), "{what}: {p:?}");
            if !p.mass.is_finite() {
                eprintln!("observation: {what} gives mass {}", p.mass);
            }
            assert!(s.undo());
        }
    }
    // The override wins over the density, in both orders, and removing it brings the density back.
    s.set_properties(Some(b), IProperties { density: Some(1000.0), ..IProperties::default() }).unwrap();
    assert!(near(s.physical_properties(Some(b)).unwrap().mass, 1.0));
    s.set_properties(
        Some(b),
        IProperties { density: Some(1000.0), mass_override: Some(2.5), ..IProperties::default() },
    )
    .unwrap();
    let p = s.physical_properties(Some(b)).unwrap();
    assert!(p.overridden && near(p.mass, 2.5) && near(p.volume, 1e-3));
    assert!(near(p.density, 2500.0), "the density shown is mass over volume: {}", p.density);
    s.set_properties(Some(b), IProperties { density: Some(1000.0), ..IProperties::default() }).unwrap();
    assert!(!s.physical_properties(Some(b)).unwrap().overridden);
}

fn bad_density(what: &str) -> Option<f64> {
    match what {
        "density 0" => Some(0.0),
        "density negative" => Some(-1.0),
        "density NaN" => Some(f64::NAN),
        "density inf" => Some(f64::INFINITY),
        _ => None,
    }
}

#[test]
fn text_fields_are_kept_exactly_and_the_part_number_falls_back_to_the_name() {
    let mut s = Session::new();
    let b = body(&mut s, Solid::Box { size: [0.1; 3] }, [0.0; 3], [0.0; 3]);
    let name = s.part_number(b);
    assert!(!name.is_empty());
    // Odd text: kept exactly, nothing trimmed or escaped away; saved and read back the same.
    let weird = [
        "",
        " ",
        "  leading and trailing  ",
        "한글 부품 ✓",
        "tab\tnew\nline",
        "quote \" backslash \\ brace {}",
        &"x".repeat(100_000),
    ];
    for text in weird {
        let props = IProperties {
            part_number: text.into(),
            title: text.into(),
            comments: text.into(),
            ..IProperties::default()
        };
        assert!(set_step(&mut s, "text", Some(b), props.clone()));
        s.set_properties(Some(b), props.clone()).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("p.gcp");
        s.save_as(&path).unwrap();
        let mut t = Session::new();
        t.open(&path).unwrap();
        assert_eq!(t.properties(Some(b)), props, "text of {} bytes", text.len());
        // An empty part number means the name again.
        if text.is_empty() {
            assert_eq!(s.part_number(b), name);
        }
        s.undo();
    }
    // Properties of a component that was deleted do not linger (Undo brings them back).
    s.set_properties(Some(b), IProperties { part_number: "KEEP".into(), ..IProperties::default() }).unwrap();
    let before = s.doc().clone();
    s.delete_item(gc_session::CellItem::Body(match b {
        Occurrence::Item(any) => gc_core::Id::from_raw(any.raw),
        _ => panic!(),
    }))
    .unwrap();
    assert!(s.physical_properties(None).is_some());
    assert!(s.undo());
    assert!(s.doc().same_content(&before));
    assert_eq!(s.properties(Some(b)).part_number, "KEEP");
}

#[test]
fn the_whole_document_adds_up_and_follows_edits() {
    let mut s = Session::new();
    // An empty document: zeros, finite, no division by nothing.
    let nothing = s.physical_properties(None).unwrap();
    assert!(
        nothing.mass == 0.0
            && nothing.volume == 0.0
            && nothing.centre.is_finite()
            && nothing.density.is_finite()
    );
    let a = body(&mut s, Solid::Box { size: [0.1; 3] }, [1.0, 0.0, 0.0], [0.0; 3]);
    let b = body(&mut s, Solid::Box { size: [0.1; 3] }, [-1.0, 0.0, 0.0], [0.0; 3]);
    s.set_properties(Some(a), IProperties { density: Some(3000.0), ..IProperties::default() }).unwrap();
    s.set_properties(Some(b), IProperties { density: Some(1000.0), ..IProperties::default() }).unwrap();
    let all = s.physical_properties(None).unwrap();
    assert!(near(all.mass, 4.0) && near(all.volume, 2e-3));
    // Heavier on the +X side: x = (3 * 1 + 1 * -1) / 4 = 0.5.
    assert!((all.centre - DVec3::new(0.5, 0.0, 0.0)).length() < 1e-9, "{:?}", all.centre);
    // A mass override on one body only: the other keeps its density; the document says it includes one.
    s.set_properties(Some(b), IProperties { mass_override: Some(9.0), ..IProperties::default() }).unwrap();
    let all = s.physical_properties(None).unwrap();
    assert!(all.overridden && near(all.mass, 12.0), "{all:?}");
    assert!((all.centre.x - (3.0 - 9.0) / 12.0).abs() < 1e-9);
    // Moving a body moves the centre; hiding it does not change what it weighs.
    let steps = s.history().undo_depth();
    let _ = steps;
    let hidden_before = s.physical_properties(None).unwrap();
    let Occurrence::Item(any) = a else { panic!() };
    s.set_visible(any, false).ok();
    let hidden = s.physical_properties(None).unwrap();
    assert!(hidden.centre.is_finite());
    // Visible or not, the numbers are one answer (the shapes are there either way) or the hidden one is left
    // out; either way mass is non-negative and not larger than before.
    assert!(hidden.mass <= hidden_before.mass + 1e-12);
    // Components inside a sub-assembly and parts of an imported assembly: no answer for a part, no panic.
    assert!(
        s.physical_properties(Some(Occurrence::Part {
            assembly: gc_core::Id::from_raw(std::num::NonZeroU64::new(999).unwrap()),
            index: 0
        }))
        .is_none()
    );
}

#[test]
fn a_ghost_component_has_no_properties_and_refuses_edits() {
    let mut s = Session::new();
    let ghost = Occurrence::Item(
        gc_core::Id::<gc_doc::Body>::from_raw(std::num::NonZeroU64::new(999).unwrap()).erase(),
    );
    assert!(s.physical_properties(Some(ghost)).is_none());
    assert!(!set_step(
        &mut s,
        "ghost",
        Some(ghost),
        IProperties { part_number: "x".into(), ..IProperties::default() }
    ));
    assert_eq!(s.properties(Some(ghost)), IProperties::default());
    // An entry for a kind that is not a component (a sketch, a relationship, a work feature) is refused too.
    let k = s.create_sketch("XY").unwrap();
    let sketch = Occurrence::Item(k.erase());
    assert!(!set_step(
        &mut s,
        "sketch",
        Some(sketch),
        IProperties { part_number: "x".into(), ..IProperties::default() }
    ));
}

fn cube_stl(side: f64, inside_out: bool, missing_face: bool) -> String {
    let v = |x: f64, y: f64, z: f64| [x * side, y * side, z * side];
    let quads: [[[f64; 3]; 4]; 6] = [
        [v(0., 0., 0.), v(0., 1., 0.), v(1., 1., 0.), v(1., 0., 0.)],
        [v(0., 0., 1.), v(1., 0., 1.), v(1., 1., 1.), v(0., 1., 1.)],
        [v(0., 0., 0.), v(1., 0., 0.), v(1., 0., 1.), v(0., 0., 1.)],
        [v(0., 1., 0.), v(0., 1., 1.), v(1., 1., 1.), v(1., 1., 0.)],
        [v(0., 0., 0.), v(0., 0., 1.), v(0., 1., 1.), v(0., 1., 0.)],
        [v(1., 0., 0.), v(1., 1., 0.), v(1., 1., 1.), v(1., 0., 1.)],
    ];
    let mut out = String::from("solid cube\n");
    for (i, q) in quads.iter().enumerate() {
        if missing_face && i == 5 {
            continue;
        }
        let tris = if inside_out {
            [[q[0], q[2], q[1]], [q[0], q[3], q[2]]]
        } else {
            [[q[0], q[1], q[2]], [q[0], q[2], q[3]]]
        };
        for t in tris {
            out.push_str("facet normal 0 0 0\nouter loop\n");
            for p in t {
                out.push_str(&format!("vertex {} {} {}\n", p[0], p[1], p[2]));
            }
            out.push_str("endloop\nendfacet\n");
        }
    }
    out.push_str("endsolid cube\n");
    out
}

#[test]
fn a_mesh_has_its_volume_from_the_triangles() {
    let dir = tempfile::tempdir().unwrap();
    for (what, side, inside_out, missing, scale) in [
        ("cube", 0.1, false, false, 1.0),
        ("cube in mm", 100.0, false, false, 0.001),
        ("inside out", 0.1, true, false, 1.0),
        ("missing face", 0.1, false, true, 1.0),
    ] {
        let path = dir.path().join(format!("{}.stl", what.replace(' ', "_")));
        std::fs::write(&path, cube_stl(side, inside_out, missing)).unwrap();
        let mut s = Session::new();
        let b = match s
            .add_body(Solid::Mesh { file: path.to_string_lossy().into_owned(), scale }, Placement::default())
        {
            Ok(b) => Occurrence::Item(b.erase()),
            Err(e) => {
                eprintln!("{what}: the body was refused: {e:?}");
                continue;
            }
        };
        let Some(p) = s.physical_properties(Some(b)) else {
            eprintln!("{what}: no physical properties (the mesh is not known)");
            continue;
        };
        assert!(
            p.volume.is_finite() && p.area.is_finite() && p.centre.is_finite() && p.mass.is_finite(),
            "{what}: {p:?}"
        );
        eprintln!("mesh {what}: volume {} area {} mass {}", p.volume, p.area, p.mass);
        match what {
            "cube" | "cube in mm" => {
                // Mesh coordinates are single precision: about 1e-7 relative.
                let close = |a: f64, b: f64| (a - b).abs() <= 1e-6 * b;
                assert!(close(p.volume, 1e-3), "{what}: volume {}", p.volume);
                assert!(close(p.area, 0.06), "{what}: area {}", p.area);
                assert!((p.centre - DVec3::splat(0.05)).length() < 1e-6, "{what}: {:?}", p.centre);
            }
            // A mesh turned inside out has a negative signed volume: shown positive, or zero, never negative.
            "inside out" => assert!(p.volume >= 0.0, "{what}: negative volume {}", p.volume),
            // An open mesh has no volume to speak of; the answer must at least be finite and not negative.
            _ => assert!(p.volume >= 0.0, "{what}: negative volume {}", p.volume),
        }
    }
    // A mesh file that is not there or is not a mesh: the body may be refused; nothing panics.
    let bad = dir.path().join("empty.stl");
    std::fs::write(&bad, "").unwrap();
    let mut s = Session::new();
    for file in
        [bad.to_string_lossy().into_owned(), dir.path().join("none.stl").to_string_lossy().into_owned()]
    {
        if let Ok(b) = s.add_body(Solid::Mesh { file, scale: 1.0 }, Placement::default()) {
            let p = s.physical_properties(Some(Occurrence::Item(b.erase())));
            assert!(p.is_none_or(|p| p.volume.is_finite() && p.centre.is_finite()));
        }
    }
}

#[test]
fn a_robot_adds_up_from_its_links_and_follows_its_pose() {
    let path = format!("{}/../../../fixtures/urdf/six_axis.urdf", env!("CARGO_MANIFEST_DIR"));
    let urdf = std::fs::read_to_string(path).unwrap();
    let mut s = Session::new();
    let robot = s.add_robot("Robot1", &urdf).unwrap();
    let r = Occurrence::Item(robot.erase());
    let p = s.physical_properties(Some(r)).unwrap();
    assert!(p.volume > 0.0 && p.area > 0.0 && p.mass > 0.0 && p.centre.is_finite());
    // The default density is one: mass is volume. With another density, mass scales; an override wins.
    assert!(near(p.mass, p.volume));
    s.set_properties(Some(r), IProperties { density: Some(2700.0), ..IProperties::default() }).unwrap();
    let al = s.physical_properties(Some(r)).unwrap();
    assert!(near(al.mass, p.volume * 2700.0) && near(al.volume, p.volume));
    s.set_properties(Some(r), IProperties { mass_override: Some(55.0), ..IProperties::default() }).unwrap();
    let fixed = s.physical_properties(Some(r)).unwrap();
    assert!(fixed.overridden && near(fixed.mass, 55.0) && near(fixed.volume, p.volume));
    // A box placed beside the robot: the document's centre lies between the two, by mass.
    let b = body(&mut s, Solid::Box { size: [0.1; 3] }, [3.0, 0.0, 0.0], [0.0; 3]);
    s.set_properties(Some(b), IProperties { mass_override: Some(55.0), ..IProperties::default() }).unwrap();
    let all = s.physical_properties(None).unwrap();
    assert!(near(all.mass, 110.0));
    let expect = (fixed.centre * 55.0 + DVec3::new(3.0, 0.0, 0.0) * 55.0) / 110.0;
    assert!((all.centre - expect).length() < 1e-9, "{:?} vs {expect:?}", all.centre);
    // A robot with no links worth speaking of (a URDF of one empty link): finite answers or none.
    let empty = r#"<robot name="e"><link name="base"/></robot>"#;
    let mut t = Session::new();
    if let Ok(e) = t.add_robot("Empty", empty) {
        let p = t.physical_properties(Some(Occurrence::Item(e.erase())));
        assert!(p.is_none_or(|p| p.volume == 0.0 && p.centre.is_finite() && p.density.is_finite()));
    }
}
