//! Replacing a robot (Inventor 2026 help, Replace assembly components,
//! GUID-E5FAA224-7332-4AC6-A78E-793FDACFEEDD): it stays where it stood, keeps what is about its place and
//! its work, and takes the new description.

#![allow(clippy::unwrap_used, clippy::panic, clippy::float_cmp, reason = "test code")]

use gc_doc::{CellOp, Command, Placement, WaypointTarget};
use gc_session::Session;

fn urdf(name: &str) -> String {
    let path = format!("{}/../../../fixtures/urdf/{name}.urdf", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn a_replaced_robot_stays_in_place_and_keeps_its_work() {
    let mut s = Session::new();
    let robot = s.add_robot("Arm", &urdf("six_axis")).unwrap();
    let placement = Placement { xyz: [0.4, -0.2, 0.1], rpy: [0.0, 0.0, 0.5] };
    s.execute(&Command::Cell(CellOp::PlaceRobot { id: robot, placement })).unwrap();
    let tcp = Placement { xyz: [0.0, 0.0, 0.12], rpy: [0.0; 3] };
    s.execute(&Command::Cell(CellOp::SetTcp { id: robot, tcp })).unwrap();
    s.jog(robot, &[0.2, -0.4, 0.6, 0.0, 0.3, 0.0]);
    s.end_jog();
    let target = WaypointTarget::InWorld { offset: Placement { xyz: [0.7, 0.0, 0.5], rpy: [0.0; 3] } };
    let pose_point = s.add_waypoint(robot, target).unwrap();
    let old_links = s.doc().get(robot).unwrap().links.len();

    // Another six-axis arm: same place, same tool centre point, same joint values, its own description.
    let depth = s.history().undo_depth();
    let lost = s.replace_robot(robot, "UR5e", &urdf("ur5e_kinematics"), "", false).unwrap();
    assert!(lost.is_empty());
    assert_eq!(s.history().undo_depth(), depth + 1);
    let now = s.doc().get(robot).unwrap().clone();
    assert_eq!((now.name.as_str(), now.placement, now.tcp), ("UR5e", placement, tcp));
    assert_eq!(now.joints, [0.2, -0.4, 0.6, 0.0, 0.3, 0.0]);
    assert!(now.urdf.contains("ur5e") || now.urdf != urdf("six_axis"));
    assert!(s.doc().get(pose_point).is_some(), "the waypoint is still the robot's");
    assert_eq!(s.robot_joints(robot).unwrap().len(), 6);
    assert!(s.undo());
    assert_eq!(s.doc().get(robot).unwrap().name, "Arm");
    assert_eq!(s.doc().get(robot).unwrap().links.len(), old_links);
    assert!(s.redo());

    // A waypoint given as joint values belongs to an arm with that many joints.
    let joint_point = s.add_waypoint(robot, WaypointTarget::Joints(vec![0.1; 6])).unwrap();
    let refused = s.replace_robot(robot, "Seven", &urdf("seven_axis"), "", false);
    assert!(refused.is_err(), "{refused:?}");
    assert_eq!(s.doc().get(robot).unwrap().name, "UR5e");
    s.execute(&Command::Cell(CellOp::DeleteWaypoint(joint_point))).unwrap();
    s.replace_robot(robot, "Seven", &urdf("seven_axis"), "", false).unwrap();
    let now = s.doc().get(robot).unwrap().clone();
    assert_eq!((now.name.as_str(), now.joints.len(), now.placement), ("Seven", 7, placement));
    assert_eq!(s.robot_joints(robot).unwrap().len(), 7);
    // A description that does not read changes nothing.
    assert!(s.replace_robot(robot, "Bad", "<robot", "", false).is_err());
    assert_eq!(s.doc().get(robot).unwrap().name, "Seven");
}

/// Replace All: every robot with the same description.
#[test]
fn replace_all_takes_every_robot_of_the_same_description() {
    let mut s = Session::new();
    let a = s.add_robot("A", &urdf("six_axis")).unwrap();
    let b = s.add_robot("B", &urdf("six_axis")).unwrap();
    let c = s.add_robot("C", &urdf("seven_axis")).unwrap();
    let depth = s.history().undo_depth();
    s.replace_robot(a, "UR5e", &urdf("ur5e_kinematics"), "", true).unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1);
    let name = |s: &Session, id: gc_core::Id<gc_doc::Robot>| s.doc().get(id).unwrap().name.clone();
    assert_eq!(
        (name(&s, a), name(&s, b), name(&s, c)),
        ("UR5e".to_owned(), "UR5e".to_owned(), "C".to_owned())
    );
}
