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

#[test]
fn a_ray_picks_the_nearest_body_or_robot_and_skips_hidden_ones() {
    use gc_math::Ray;
    use gc_scene::{Highlight, Picker};
    use gc_session::{CellItem, Shown};
    let path = format!("{}/../../../fixtures/urdf/six_axis.urdf", env!("CARGO_MANIFEST_DIR"));
    let mut session = Session::new();
    let robot = session.add_robot("Robot1", &std::fs::read_to_string(path).unwrap()).unwrap();
    let ball = session
        .add_body(Solid::Sphere { radius: 0.1 }, Placement { xyz: [1.0, 0.0, 0.1], rpy: [0.0; 3] })
        .unwrap();
    let wall = session
        .add_body(Solid::Box { size: [0.05, 1.0, 1.0] }, Placement { xyz: [1.4, 0.0, 0.5], rpy: [0.0; 3] })
        .unwrap();
    let down = |x: f64, y: f64| Ray { origin: gc_math::DVec3::new(x, y, 5.0), dir: -gc_math::DVec3::Z };
    let mut picker = Picker::default();

    let scene = Scene::of(&session);
    let (t, item) = picker.pick(&scene, &down(1.0, 0.0)).unwrap();
    assert_eq!(item, CellItem::Body(ball));
    assert!((t - 4.8).abs() < 1e-3, "the top of the ball: {t}");
    assert_eq!(picker.pick(&scene, &down(0.0, 0.0)).unwrap().1, CellItem::Robot(robot));
    assert_eq!(picker.pick(&scene, &down(3.0, 3.0)), None);
    // Looking along +X from the robot's side: the ball is in front of the wall.
    let along = Ray { origin: gc_math::DVec3::new(-2.0, 0.0, 0.1), dir: gc_math::DVec3::X };
    let scene_without_robot = {
        session.set_look(Shown::Robot(robot), gc_doc::Look { hidden: true, transparency: 0.0 }).unwrap();
        Scene::of(&session)
    };
    assert_eq!(picker.pick(&scene_without_robot, &along).unwrap().1, CellItem::Body(ball));
    session.set_look(Shown::Body(ball), gc_doc::Look { hidden: true, transparency: 0.0 }).unwrap();
    assert_eq!(
        picker.pick(&Scene::of(&session), &along).unwrap().1,
        CellItem::Body(wall),
        "hidden things are not picked"
    );

    // Selection and hover are drawn as highlights.
    session.select_item(Some(CellItem::Body(wall)));
    session.hover_item(Some(CellItem::Robot(robot)));
    let scene = Scene::of(&session);
    let wall_colour = scene.solids.iter().find(|s| s.owner == Some(CellItem::Body(wall))).unwrap().color;
    assert_eq!(wall_colour, gc_scene::emphasised([0.55, 0.62, 0.70, 1.0], Highlight::Selected));
    // Deleting the selected item clears the selection.
    session.delete_item(CellItem::Body(wall)).unwrap();
    assert_eq!(session.selected_item(), None);
}
