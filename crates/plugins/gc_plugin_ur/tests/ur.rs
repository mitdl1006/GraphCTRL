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

/// The closed-form solver: every UR model is recognised exactly, every solution reaches the pose, the
/// configuration a pose came from is among its solutions, and a general pose has eight.
#[test]
fn the_closed_form_solver_finds_every_arm_configuration() {
    use gc_robotics_api::{Fit, KINEMATICS_FACTORIES};
    let mut registry = Registry::default();
    registry.load(&UrPlugin).unwrap();
    let provider = &registry.get(ROBOT_MODEL_PROVIDERS)[0];
    let factory = &registry.get(KINEMATICS_FACTORIES)[0];
    assert_eq!(factory.id(), "ur6r");
    for model in ["ur3e", "ur5e", "ur10e", "ur16e"] {
        let (robot, _) = import(&provider.urdf(model).unwrap()).unwrap();
        let spec = robot.default_chain().unwrap().to_spec().unwrap();
        assert_eq!(factory.probe(&spec, None), Some(Fit::Exact), "{model}");
        let solver = factory.build(&spec).unwrap();
        let (mut q_out, mut branches) = (Vec::new(), Vec::new());
        for k in 0..40 {
            let q: Vec<f64> = (0..6).map(|j| (((k * 5 + j * 7) % 19) as f64 / 19.0 * 5.6) - 2.8).collect();
            let target = spec.fk(&q).unwrap();
            let count = solver.solve_all(&target, Some(&q), &mut q_out, &mut branches).unwrap();
            assert!((1..=8).contains(&count) && q_out.len() == count * 6, "{model} {q:?}");
            for solution in q_out.chunks(6) {
                let reached = spec.fk(solution).unwrap();
                let error =
                    reached.xyz.iter().zip(&target.xyz).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
                assert!(error < 1e-6, "{model}: {solution:?} misses by {error}");
            }
            let wrap =
                |a: f64| (a + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
            let found = q_out.chunks(6).any(|s| s.iter().zip(&q).all(|(a, b)| wrap(a - b).abs() < 1e-6));
            assert!(found, "{model}: {q:?} not among {q_out:?}");
            assert!(solver.classify(&q).is_some());
        }
    }

    // The reference pose of the plan (UR5e, tool pointing down): eight solutions, eight branches.
    let (robot, _) = import(&provider.urdf("ur5e").unwrap()).unwrap();
    let spec = robot.default_chain().unwrap().to_spec().unwrap();
    let solver = factory.build(&spec).unwrap();
    let q = [
        0.0,
        -std::f64::consts::FRAC_PI_2,
        std::f64::consts::FRAC_PI_2,
        -std::f64::consts::FRAC_PI_2,
        -std::f64::consts::FRAC_PI_2,
        0.0,
    ];
    let (mut q_out, mut branches) = (Vec::new(), Vec::new());
    assert_eq!(solver.solve_all(&spec.fk(&q).unwrap(), None, &mut q_out, &mut branches), Ok(8));
    branches.sort_unstable();
    branches.dedup();
    assert_eq!(branches.len(), 8);
    assert!(!solver.branch_label(branches[0]).is_empty());

    // Out of reach: no solution, said so.
    let far = gc_robotics_api::Pose::from_translation([3.0, 0.0, 0.5]);
    assert!(solver.solve_all(&far, None, &mut q_out, &mut branches).is_err());

    // The sample six-axis arm is not a UR: no closed form.
    let path = format!("{}/../../../fixtures/urdf/six_axis.urdf", env!("CARGO_MANIFEST_DIR"));
    let (sample, _) = import(&std::fs::read_to_string(path).unwrap()).unwrap();
    let spec = sample.default_chain().unwrap().to_spec().unwrap();
    assert_eq!(factory.probe(&spec, None), None);
}
