//! The packaged UR5e: its description reads cleanly, its meshes are all there and fit the kinematics.

#![allow(
    clippy::float_cmp,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "test code"
)]

use gc_math::DVec3;
use gc_plugin_api::Registry;
use gc_plugin_ur::UrPlugin;
use gc_robot::{Shape, import};
use gc_robotics_api::ROBOT_MODEL_PROVIDERS;

#[test]
fn the_ur5e_reads_cleanly_and_its_meshes_follow_the_arm() {
    let mut registry = Registry::default();
    registry.load(&UrPlugin).unwrap();
    let providers = registry.get(ROBOT_MODEL_PROVIDERS);
    assert_eq!(providers.len(), 1);
    let provider = &providers[0];
    assert_eq!(
        provider.catalog().iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        ["ur3e", "ur5e", "ur10e", "ur16e"]
    );
    assert!(provider.urdf("ur99").is_err());
    assert!(provider.resource("ur5e", "package://nothing.stl").is_err());

    let (model, findings) = import(&provider.urdf("ur5e").unwrap()).unwrap();
    assert!(findings.is_empty(), "{findings:?}");
    let chain = model.default_chain().unwrap();
    assert_eq!(chain.dof(), 6);
    // The published pose of tool0 in base_link with the arm straight up (base = base_link turned half a
    // turn about Z, so x and y change sign).
    let q = [0.0, -std::f64::consts::FRAC_PI_2, 0.0, -std::f64::consts::FRAC_PI_2, 0.0, 0.0];
    let tip = chain.fk(&q).translation;
    assert!((tip - DVec3::new(0.0, 0.2329, 1.0794)).length() < 1e-9, "{tip:?}");

    // Every link's mesh, placed by the link pose, stays near the line from its joint to the next one:
    // a wrong mesh offset or rotation would put it far off.
    let mut poses = Vec::new();
    model.link_poses(&chain.expand(&q), &mut poses);
    let mut meshes = 0;
    let (mut lowest, mut highest) = (f64::INFINITY, f64::NEG_INFINITY);
    for (link, pose) in model.links.iter().zip(&poses) {
        assert_eq!(link.visuals.len(), link.collisions.len());
        for geometry in &link.collisions {
            let Shape::Mesh { uri, .. } = &geometry.shape else { panic!("{}: not a mesh", link.name) };
            let mesh = gc_mesh::stl::parse(&provider.resource("ur5e", uri).unwrap()).unwrap();
            assert!(mesh.triangle_count() > 100, "{uri}");
            meshes += 1;
            for corner in mesh.positions() {
                let local = DVec3::new(f64::from(corner[0]), f64::from(corner[1]), f64::from(corner[2]));
                let world = *pose * (geometry.pose * local);
                // Straight up, the whole arm stays within a slim column around the base axis.
                assert!(world.x.abs() < 0.12 && world.y > -0.1 && world.y < 0.3, "{}: {world:?}", link.name);
                lowest = lowest.min(world.z);
                highest = highest.max(world.z);
            }
        }
    }
    assert_eq!(meshes, 7);

    // The same for what is drawn: the visual meshes, read part by part with the colours of the real arm.
    let started = std::time::Instant::now();
    let (mut parts, mut triangles) = (0, 0);
    let mut colors = std::collections::BTreeSet::new();
    for (link, pose) in model.links.iter().zip(&poses) {
        for geometry in &link.visuals {
            let Shape::Mesh { uri, .. } = &geometry.shape else { panic!("{}: not a mesh", link.name) };
            assert!(uri.ends_with(".dae"), "{uri}");
            for part in gc_mesh::read_parts(&provider.resource("ur5e", uri).unwrap()).unwrap() {
                parts += 1;
                triangles += part.mesh.triangle_count();
                colors.insert(part.color.unwrap().map(|c| (c * 255.0).round() as u8));
                for corner in part.mesh.positions() {
                    let local = DVec3::new(f64::from(corner[0]), f64::from(corner[1]), f64::from(corner[2]));
                    let world = *pose * (geometry.pose * local);
                    assert!(
                        world.x.abs() < 0.12 && world.y > -0.1 && world.y < 0.3,
                        "{}: {world:?}",
                        link.name
                    );
                    assert!(world.z > -0.005 && world.z < 1.2, "{}: {world:?}", link.name);
                }
            }
        }
    }
    println!("visual meshes: {parts} parts, {triangles} triangles, read in {:?}", started.elapsed());
    assert_eq!(parts, 20);
    assert!(triangles > 120_000, "{triangles}");
    // Light grey tubes, dark grey joints, black seals and the blue caps.
    let expected = [[8, 8, 8, 255], [71, 71, 71, 255], [125, 173, 204, 255], [209, 209, 209, 255]];
    assert_eq!(colors.into_iter().collect::<Vec<_>>(), expected);
    assert!(lowest > -0.005 && lowest < 0.005, "the base stands on the floor: {lowest}");
    assert!(highest > 1.05 && highest < 1.2, "the wrist tops out just above the tool: {highest}");
}

/// Every model reads cleanly, finds every mesh it names, and its tool flange is where the published DH
/// parameters put it.
#[test]
fn every_model_matches_its_dh_parameters() {
    use gc_math::{DPose3, DQuat};
    // d1, a2, a3, d4, d5, d6 (Universal Robots' DH tables).
    let tables = [
        ("ur3e", [0.15185, -0.24355, -0.2132, 0.13105, 0.08535, 0.0921]),
        ("ur5e", [0.1625, -0.425, -0.3922, 0.1333, 0.0997, 0.0996]),
        ("ur10e", [0.1807, -0.6127, -0.57155, 0.17415, 0.11985, 0.11655]),
        ("ur16e", [0.1807, -0.4784, -0.36, 0.17415, 0.11985, 0.11655]),
    ];
    let mut registry = Registry::default();
    registry.load(&UrPlugin).unwrap();
    let provider = &registry.get(ROBOT_MODEL_PROVIDERS)[0];
    for (model, [d1, a2, a3, d4, d5, d6]) in tables {
        let (robot, findings) = import(&provider.urdf(model).unwrap()).unwrap();
        assert!(findings.is_empty(), "{model}: {findings:?}");
        for link in &robot.links {
            for geometry in link.visuals.iter().chain(&link.collisions) {
                let Shape::Mesh { uri, .. } = &geometry.shape else { panic!("{model}: not a mesh") };
                let bytes = provider.resource(model, uri).unwrap_or_else(|e| panic!("{model}: {e}"));
                assert!(gc_mesh::read_parts(&bytes).unwrap()[0].mesh.triangle_count() > 50, "{uri}");
            }
        }
        let chain = robot.default_chain().unwrap();
        let (a, d) = ([0.0, a2, a3, 0.0, 0.0, 0.0], [d1, 0.0, 0.0, d4, d5, d6]);
        let alpha = [
            std::f64::consts::FRAC_PI_2,
            0.0,
            0.0,
            std::f64::consts::FRAC_PI_2,
            -std::f64::consts::FRAC_PI_2,
            0.0,
        ];
        for k in 0..8 {
            let q: Vec<f64> = (0..6).map(|j| ((k * 7 + j * 3) % 11) as f64 * 0.5 - 2.5).collect();
            // base_link is the DH base turned half a turn about Z.
            let mut dh = DPose3::from_parts(DVec3::ZERO, DQuat::from_rotation_z(std::f64::consts::PI));
            for j in 0..6 {
                dh = dh
                    * DPose3::from_parts(DVec3::new(0.0, 0.0, d[j]), DQuat::from_rotation_z(q[j]))
                    * DPose3::from_parts(DVec3::new(a[j], 0.0, 0.0), DQuat::from_rotation_x(alpha[j]));
            }
            let urdf = chain.fk(&q);
            assert!((urdf.translation - dh.translation).length() < 1e-9, "{model} {q:?}");
            // Compared through the axes: the angle between two nearly equal rotations is imprecise.
            for axis in [DVec3::X, DVec3::Y, DVec3::Z] {
                assert!((urdf.rotation * axis - dh.rotation * axis).length() < 1e-9, "{model} {q:?}");
            }
        }
    }
}
