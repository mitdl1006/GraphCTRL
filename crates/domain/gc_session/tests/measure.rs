//! Measure (Inventor 2026 help, "To Measure Distance, Length, Angle, Loop, or Area in Models or Drawings":
//! https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-1805AB8F-F782-4B24-A332-B4F2E10A3D00.htm):
//! two components give their minimum distance, two points the distance and the delta.

#![allow(clippy::unwrap_used, clippy::panic, clippy::float_cmp, reason = "test code: exact values are exact")]

use std::path::Path;

use gc_doc::{Placement, Solid};
use gc_session::{CellItem, Session, library};

#[test]
fn two_boxes_measure_their_gap_centres_and_angle() {
    let mut session = Session::new();
    let a = session
        .add_body(Solid::Box { size: [0.2, 0.2, 0.2] }, Placement { xyz: [0.0; 3], rpy: [0.0; 3] })
        .unwrap();
    let b = session
        .add_body(
            Solid::Box { size: [0.2, 0.2, 0.2] },
            Placement { xyz: [0.5, 0.0, 0.0], rpy: [0.0, std::f64::consts::FRAC_PI_2, 0.0] },
        )
        .unwrap();
    let pair = session.measure_pair(CellItem::Body(a), CellItem::Body(b)).unwrap();
    let minimum = pair.minimum.unwrap();
    // Faces 0.1 m from each centre: 0.5 − 0.2 = 0.3 m apart.
    assert!((minimum.distance - 0.3).abs() < 1e-9, "{minimum:?}");
    assert!((minimum.on_first[0] - 0.1).abs() < 1e-9 && (minimum.on_second[0] - 0.4).abs() < 1e-9);
    assert!((pair.centre_to_centre - 0.5).abs() < 1e-12);
    assert_eq!(pair.delta, [0.5, 0.0, 0.0]);
    assert!((pair.angle.unwrap() - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    let item = session.measure_item(CellItem::Body(b)).unwrap();
    assert_eq!(item.position, [0.5, 0.0, 0.0]);

    // Overlapping bodies measure 0.
    let c = session
        .add_body(Solid::Box { size: [0.2, 0.2, 0.2] }, Placement { xyz: [0.05, 0.0, 0.0], rpy: [0.0; 3] })
        .unwrap();
    let overlap = session.measure_pair(CellItem::Body(a), CellItem::Body(c)).unwrap();
    assert_eq!(overlap.minimum.unwrap().distance, 0.0);
}

#[test]
fn a_robot_and_a_pillar_measure_what_the_collision_check_sees() {
    let library_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../robots");
    let packages = library::scan(&library_dir);
    let ur5e = packages.iter().find(|p| p.info.name == "UR5e").unwrap();
    let mut session = Session::new();
    session.set_robot_library(Some(library_dir.clone()));
    let robot = session.add_library_robot(ur5e).unwrap();
    session.jog(robot, &[0.0, -1.9, 1.7, -1.37, -1.57, 0.0]);
    session.end_jog();
    let pillar = session
        .add_body(Solid::Box { size: [0.08, 0.08, 0.9] }, Placement { xyz: [0.7, 0.1, 0.45], rpy: [0.0; 3] })
        .unwrap();

    let pair = session.measure_pair(CellItem::Robot(robot), CellItem::Body(pillar)).unwrap();
    let gap = pair.minimum.unwrap().distance;
    assert!(gap > 0.0 && gap < 0.7, "{gap}");
    // The nearest point of the pillar is on its surface: on the face toward the robot.
    let on_pillar = pair.minimum.unwrap().on_second;
    assert!(on_pillar[0] >= 0.66 - 1e-9 && on_pillar[0] <= 0.74 + 1e-9, "{on_pillar:?}");

    // Moving the pillar toward the robot by d shrinks the gap by about d (the nearest face faces it).
    let mut moved = session.doc().get(pillar).unwrap().clone();
    moved.placement.xyz = [0.65, 0.1, 0.45];
    session.execute(&gc_doc::Command::Cell(gc_doc::CellOp::SetBody { id: pillar, body: moved })).unwrap();
    let closer =
        session.measure_pair(CellItem::Robot(robot), CellItem::Body(pillar)).unwrap().minimum.unwrap();
    assert!(closer.distance < gap - 0.01, "{} vs {gap}", closer.distance);
    assert!(session.measure_item(CellItem::Robot(robot)).is_some());
}

/// Analyze Interference (Inventor 2026 help, "To Analyze Interference Among Components":
/// https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-80DED0A6-AC65-4C7C-AD85-0144EE6DAB5B.htm):
/// one set checks its members among themselves, two sets check one against the other only.
#[test]
fn interference_is_found_within_one_set_or_between_two() {
    let mut session = Session::new();
    let place = |x: f64| Placement { xyz: [x, 0.0, 0.0], rpy: [0.0; 3] };
    let cube = Solid::Box { size: [0.2, 0.2, 0.2] };
    let a = CellItem::Body(session.add_body(cube.clone(), place(0.0)).unwrap());
    let b = CellItem::Body(session.add_body(cube.clone(), place(0.15)).unwrap());
    let c = CellItem::Body(session.add_body(cube, place(0.3)).unwrap());

    // a∩b and b∩c overlap by 0.05 × 0.2 × 0.2 m; a and c are apart.
    let all = session.interference(&[a, b, c], &[]);
    assert_eq!(all.len(), 2);
    for row in &all {
        assert!((row.overlap.volume - 0.002).abs() < 1e-9, "{row:?}");
    }
    assert_eq!((all[0].first, all[0].second), (a, b));
    // Set 1 = {a}, set 2 = {b, c}: only a∩b; members of set 2 are not compared with each other.
    let between = session.interference(&[a], &[b, c]);
    assert_eq!(between.len(), 1);
    assert_eq!(between[0].second, b);
    assert!(session.interference(&[a], &[c]).is_empty());
}

/// "Select Treat subassemblies as components to treat sub-assemblies as single components and ignore
/// the interferences within the sub-assemblies" (same help page).
#[test]
fn interference_within_a_subassembly_is_left_out_when_it_counts_as_one_component() {
    use gc_doc::{Created, Occurrence, SubassemblyOp, TreeKey};
    use gc_session::InterferenceOptions;
    let mut session = Session::new();
    let place = |x: f64| Placement { xyz: [x, 0.0, 0.0], rpy: [0.0; 3] };
    let cube = Solid::Box { size: [0.2, 0.2, 0.2] };
    let body = |session: &mut Session, x: f64| session.add_body(cube.clone(), place(x)).unwrap();
    let (a, b, c) = (body(&mut session, 0.0), body(&mut session, 0.15), body(&mut session, 0.3));
    let key = |id: gc_core::Id<gc_doc::Body>| TreeKey::Component(Occurrence::Item(id.erase()));
    // a and b in one subassembly, that one inside another; c at the top level.
    let demote = |session: &mut Session, members: Vec<TreeKey>, name: &str| match session
        .edit_subassemblies(SubassemblyOp::Demote { members, name: name.into() })
        .unwrap()[..]
    {
        [Created::Subassembly(id)] => TreeKey::Subassembly(id),
        ref other => panic!("{other:?}"),
    };
    let inner = demote(&mut session, vec![key(a), key(b)], "Inner");
    demote(&mut session, vec![inner], "Outer");
    let items = [CellItem::Body(a), CellItem::Body(b), CellItem::Body(c)];
    assert_eq!(session.interference(&items, &[]).len(), 2);
    let options = InterferenceOptions { subassemblies_as_components: true };
    let rows = session.interference_with(&items, &[], options);
    assert_eq!(rows.len(), 1, "only b against c: a and b are one component");
    assert_eq!((rows[0].first, rows[0].second), (CellItem::Body(b), CellItem::Body(c)));
    // Between two sets the same holds.
    assert!(session.interference_with(&[CellItem::Body(a)], &[CellItem::Body(b)], options).is_empty());
}
