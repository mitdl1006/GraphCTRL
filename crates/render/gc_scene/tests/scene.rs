//! The scene of a session: where the solids, traces and markers end up.

#![allow(clippy::float_cmp, clippy::unwrap_used, clippy::panic, reason = "test code")]

use gc_doc::{Placement, Solid};
use gc_scene::{Primitive, Scene, Shape3};
use gc_session::Session;

#[test]
fn robot_links_bodies_and_waypoints_are_placed_in_the_world() {
    let path = format!("{}/../../../fixtures/urdf/six_axis.urdf", env!("CARGO_MANIFEST_DIR"));
    let mut session = Session::new();
    let robot = session.add_robot("Robot1", &std::fs::read_to_string(path).unwrap()).unwrap();
    session.jog(robot, &[-1.0, 0.2, 0.3, 0.0, 0.5, 0.0]);
    session.end_jog();
    let waypoint = session.add_waypoint_here(robot).unwrap();
    session
        .add_body(Solid::Sphere { radius: 0.1 }, Placement { xyz: [1.0, 2.0, 3.0], rpy: [0.0; 3] })
        .unwrap();
    let scene = Scene::of(&session);
    // One solid per link with geometry (seven) and the body.
    assert_eq!(scene.solids.len(), 8);
    assert_eq!(scene.solids[0].shape, Shape3::Primitive(Primitive::Sphere(0.1)));
    assert_eq!(scene.solids[0].pose.translation.to_array(), [1.0, 2.0, 3.0]);
    // The last link's solid sits just below the tool tip, which is where the waypoint is.
    let tip = session.waypoint_pose(waypoint).unwrap().translation;
    assert_eq!(scene.waypoints[0].position, tip);
    let flange = scene.solids.last().unwrap().pose.translation;
    assert!((flange - tip).length() < 0.03, "{flange:?} vs {tip:?}");
    assert!(tip.z > 0.8, "the arm reaches up: {tip:?}");
    assert!(scene.bounds().max.z >= 3.0 && scene.bounds().min.z <= 0.0);
}
