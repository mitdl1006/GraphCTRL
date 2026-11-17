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
