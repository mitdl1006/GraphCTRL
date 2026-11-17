//! A robot's links as sub-components (lead's order ②, from the user's ask that even a robot's frames be
//! reachable like a component's XY plane): every link has an origin folder that follows the joint values,
//! and constraints, joints and sketches can use it.

#![allow(clippy::float_cmp, clippy::unwrap_used, clippy::panic, reason = "test code")]

use gc_doc::{
    CellOp, Command, Geometry, Occurrence, OriginPlane, Placement, RelationKind, Relationship, Selection,
    Solid,
};
use gc_math::DVec3;
use gc_session::Session;

fn six_axis() -> String {
    let path = format!("{}/../../../fixtures/urdf/six_axis.urdf", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn close(a: DVec3, b: DVec3) -> bool {
    (a - b).length() < 1e-7
}

#[test]
fn every_link_is_a_sub_component_whose_origin_follows_the_joints() {
    let mut session = Session::new();
    let robot = session.add_robot("Robot1", &six_axis()).unwrap();
    let placement = Placement { xyz: [1.0, 0.5, 0.2], rpy: [0.0, 0.0, 0.3] };
    session.execute(&Command::Cell(CellOp::PlaceRobot { id: robot, placement })).unwrap();
    let links = session.doc().get(robot).unwrap().links.clone();
    assert!(links.len() >= 7, "base and six moving links: {}", links.len());

    // The tree: the robot, then its links under it, each under its parent link.
    let under = session.robot_links(robot);
    assert_eq!(under.len(), links.len());
    assert_eq!(under[0].parent, Some(Occurrence::Item(robot.erase())));

    // Each link's origin is where the session's own kinematics put it, for any joint values.
    let check = |session: &Session| {
        let poses = session.link_poses(robot).unwrap();
        let model = session.robot_model(robot).unwrap();
        for (index, link) in session.doc().get(robot).unwrap().links.iter().enumerate() {
            let expected = poses[model.model.link(&link.name).unwrap()];
            #[allow(clippy::cast_possible_truncation, reason = "few links")]
            let origin = session.component_origin(Occurrence::Link { robot, index: index as u32 }).unwrap();
            assert!(close(origin.point(), expected.translation), "{}", link.name);
            assert!(close(origin.axis(2), expected.rotation * DVec3::Z), "{}", link.name);
        }
    };
    check(&session);
    let joints = vec![0.3, -0.8, 1.1, -0.4, 0.9, 0.2];
    session.execute(&Command::Cell(CellOp::SetJoints { id: robot, joints })).unwrap();
    check(&session);
}

#[test]
fn a_body_jointed_to_the_last_link_follows_the_arm_and_a_sketch_on_a_link_plane_too() {
    let mut session = Session::new();
    let robot = session.add_robot("Robot1", &six_axis()).unwrap();
    let last = u32::try_from(session.doc().get(robot).unwrap().links.len() - 1).unwrap();
    let flange = Occurrence::Link { robot, index: last };
    let part = session
        .add_body(Solid::Box { size: [0.05; 3] }, Placement { xyz: [2.0, 0.0, 0.0], rpy: [0.0; 3] })
        .unwrap();
    // The part's origin on the last link's origin, Z the same way.
    session
        .add_relationship(Relationship {
            name: "Rigid:1".into(),
            kind: RelationKind::Rigid { flip: true },
            first: Selection {
                occurrence: Occurrence::Item(part.erase()),
                geometry: Geometry::origin_frame(),
            },
            second: Selection { occurrence: flange, geometry: Geometry::origin_frame() },
        })
        .unwrap();
    let part_at = |session: &Session| {
        let p = session.doc().get(part).unwrap().placement;
        DVec3::from_array(p.xyz)
    };
    assert!(close(part_at(&session), session.component_origin(flange).unwrap().point()));
    let base = session.doc().get(robot).unwrap().placement;

    // A sketch on the last link's XY plane.
    let sketch = session.create_sketch_on_component(flange, OriginPlane::Xy).unwrap();
    session.finish_sketch();

    // The arm moves: the part goes with the last link, the robot's base stays.
    let joints = vec![0.5, -0.6, 0.9, 0.0, 0.7, -0.3];
    session.execute(&Command::Cell(CellOp::SetJoints { id: robot, joints })).unwrap();
    let origin = session.component_origin(flange).unwrap();
    assert!(close(part_at(&session), origin.point()), "{:?} vs {:?}", part_at(&session), origin.point());
    assert_eq!(session.doc().get(robot).unwrap().placement, base);
    let gc_sketch::Space::Planar(frame) = session.doc().get(sketch).unwrap().space else { panic!() };
    let pose = session.doc().frame_world_pose(gc_core::Id::from_raw(frame.raw)).unwrap();
    assert!(close(pose.translation, origin.point()));
}

#[test]
fn an_older_project_gets_its_link_tree_when_opened_without_counting_as_changed() {
    let mut session = Session::new();
    let robot = session.add_robot("Robot1", &six_axis()).unwrap();
    // As saved by a version without link trees.
    session.execute(&Command::Cell(CellOp::SetRobotLinks { id: robot, links: Vec::new() })).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let project = directory.path().join("old.gctrl");
    session.save_as(&project).unwrap();
    let mut reopened = Session::new();
    reopened.open(&project).unwrap();
    assert!(!reopened.doc().get(robot).unwrap().links.is_empty());
    assert!(!reopened.is_dirty(), "filling in the tree is not an edit");
}
