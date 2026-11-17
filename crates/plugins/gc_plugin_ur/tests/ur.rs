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
    assert_eq!(provider.catalog()[0].id, "ur5e");
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
