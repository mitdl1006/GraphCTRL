//! Robot model tests: URDF semantics against published UR5e values, agreement with the reference
//! kinematics of the plugin API, Jacobians, inverse kinematics and validation messages.

#![allow(
    clippy::float_cmp,
    clippy::unwrap_used,
    clippy::panic,
    clippy::type_complexity,
    reason = "test code: exact values are compared and failures should panic"
)]

use std::f64::consts::{FRAC_PI_2, PI};

use gc_math::{DMat3, DPose3, DVec3};
use gc_robot::{ChainError, IkOptions, JointKind, NumericIk, RobotModel, Severity, import};
use gc_robotics_api::IkFailureReason;

fn fixture(name: &str) -> RobotModel {
    let path = format!("{}/../../../fixtures/urdf/{name}.urdf", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    match import(&text) {
        Ok((model, _)) => model,
        Err(findings) => panic!("{name}: {findings:?}"),
    }
}

/// Deterministic pseudo-random values in `[0, 1)`.
struct Rng(u64);
impl Rng {
    fn unit(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 11) as f64 / (1_u64 << 53) as f64
    }
    fn within(&mut self, lower: &[f64], upper: &[f64], margin: f64) -> Vec<f64> {
        lower
            .iter()
            .zip(upper)
            .map(|(lo, hi)| {
                let (lo, hi) = (lo.max(-PI) + margin, hi.min(PI) - margin);
                lo + (hi - lo) * self.unit()
            })
            .collect()
    }
}

#[test]
fn ur5e_forward_kinematics_matches_the_published_poses() {
    let model = fixture("ur5e_kinematics");
    let chain = model.chain("base_link", "tool0").unwrap();
    assert_eq!(chain.dof(), 6);
    assert_eq!(chain.joint_names()[0], "shoulder_pan_joint");
    let base = model.chain("base_link", "shoulder_link").unwrap(); // only to reach `base` below
    assert_eq!(base.dof(), 1);
    // `base` is the frame robot controllers report in: base_link turned by half a turn about Z.
    let base_in_base_link = DPose3::from_parts(DVec3::ZERO, gc_math::quat_from_rpy(0.0, 0.0, PI));
    let (a2, a3, d1, d4, d5, d6) = (0.425, 0.3922, 0.1625, 0.1333, 0.0997, 0.0996);
    let cases: [([f64; 6], [f64; 3], [[f64; 3]; 3]); 3] = [
        ([0.0; 6], [-(a2 + a3), -(d4 + d6), d1 - d5], [[1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]]),
        (
            [0.0, -FRAC_PI_2, 0.0, -FRAC_PI_2, 0.0, 0.0],
            [0.0, -(d4 + d6), d1 + a2 + a3 + d5],
            [[-1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, -1.0, 0.0]],
        ),
        (
            [0.0, -FRAC_PI_2, FRAC_PI_2, -FRAC_PI_2, -FRAC_PI_2, 0.0],
            [-(a3 + d5), -d4, d1 + a2 - d6],
            [[0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]],
        ),
    ];
    for (q, position, rows) in cases {
        let pose = base_in_base_link.inverse() * chain.fk(&q);
        let expected = DVec3::from_array(position);
        assert!(
            (pose.translation - expected).length() < 1e-9,
            "q {q:?}: {:?} vs {expected:?}",
            pose.translation
        );
        let rotation = DMat3::from_quat(pose.rotation);
        for (i, row) in rows.iter().enumerate() {
            let got = rotation.row(i);
            assert!((got - DVec3::from_array(*row)).length() < 1e-9, "q {q:?} row {i}: {got:?}");
        }
    }
}

#[test]
fn chain_kinematics_agree_with_the_reference_of_the_plugin_api() {
    let mut rng = Rng(1);
    for (name, tip) in [
        ("ur5e_kinematics", "tool0"),
        ("six_axis", "tool0"),
        ("seven_axis", "link_7"),
        ("gantry_arm", "wrist"),
    ] {
        let model = fixture(name);
        let root = model.links[model.root].name.clone();
        let chain = model.chain(&root, tip).unwrap();
        let spec = chain.to_spec().unwrap();
        assert_eq!(spec.dof(), chain.dof());
        for _ in 0..50 {
            let q = rng.within(chain.lower(), chain.upper(), 0.0);
            let (ours, theirs) = (chain.fk(&q), spec.fk(&q).unwrap());
            let position = (ours.translation - DVec3::from_array(theirs.xyz)).length();
            let [x, y, z, w] = theirs.quat_xyzw;
            let alignment = ours.rotation.dot(gc_math::DQuat::from_xyzw(x, y, z, w)).abs();
            assert!(position < 1e-12 && alignment > 1.0 - 1e-12, "{name}: {position} {alignment}");
            // The tip of the chain is where the whole-model kinematics puts that link.
            let mut poses = Vec::new();
            model.link_poses(&chain.expand(&q), &mut poses);
            let tip_pose = poses[model.link(tip).unwrap()];
            assert!((tip_pose.translation - ours.translation).length() < 1e-12, "{name}");
        }
    }
}

#[test]
fn jacobians_match_central_differences() {
    let mut rng = Rng(2);
    let mimic_on_chain = r#"<robot name="m">
        <link name="a"/><link name="b"/><link name="c"/><link name="d"/>
        <joint name="j1" type="revolute"><parent link="a"/><child link="b"/><origin xyz="0 0 0.1"/><axis xyz="0 0 1"/><limit lower="-2" upper="2" velocity="1"/></joint>
        <joint name="j2" type="prismatic"><parent link="b"/><child link="c"/><origin xyz="0.2 0 0" rpy="0.3 0 0"/><axis xyz="0 1 0"/><limit lower="-0.5" upper="0.5" velocity="1"/></joint>
        <joint name="j3" type="revolute"><parent link="c"/><child link="d"/><origin xyz="0.3 0 0"/><axis xyz="0 1 0"/><limit lower="-2" upper="2" velocity="1"/><mimic joint="j1" multiplier="0.5" offset="0.1"/></joint>
    </robot>"#;
    let (mimic_model, _) = import(mimic_on_chain).unwrap();
    let models = [
        (fixture("six_axis"), "base_link", "tool0"),
        (fixture("gantry_arm"), "rail", "wrist"),
        (fixture("seven_axis"), "link_0", "link_7"),
        (mimic_model, "a", "d"),
    ];
    for (model, base, tip) in &models {
        let chain = model.chain(base, tip).unwrap();
        for _ in 0..10 {
            let q = rng.within(chain.lower(), chain.upper(), 0.05);
            let jacobian = chain.jacobian(&q);
            for k in 0..chain.dof() {
                let h = 1e-6;
                let (mut plus, mut minus) = (q.clone(), q.clone());
                plus[k] += h;
                minus[k] -= h;
                let (a, b) = (chain.fk(&plus), chain.fk(&minus));
                let linear = (a.translation - b.translation) / (2.0 * h);
                let angular = gc_math::rotvec_from_quat(a.rotation * b.rotation.inverse()) / (2.0 * h);
                for row in 0..3 {
                    assert!(
                        (jacobian[(row, k)] - linear[row]).abs() < 1e-6,
                        "{} linear {row},{k}",
                        model.name
                    );
                    assert!(
                        (jacobian[(row + 3, k)] - angular[row]).abs() < 1e-6,
                        "{} angular {row},{k}",
                        model.name
                    );
                }
            }
        }
    }
    assert!(models[3].0.chain("a", "d").unwrap().to_spec().is_none(), "mimic chains have no plain form");
    assert_eq!(models[3].0.chain("b", "d").err(), Some(ChainError::MimicOutsideChain("j3".to_owned())));
}

#[test]
fn numerical_ik_reaches_poses_the_arm_can_reach() {
    let mut rng = Rng(3);
    for name in ["six_axis", "ur5e_kinematics", "seven_axis"] {
        let model = fixture(name);
        let chain = model.default_chain().or_else(|_| model.chain("link_0", "link_7")).unwrap();
        let ik = NumericIk::new(&chain, IkOptions::default());
        let mut solved = 0;
        for _ in 0..20 {
            let truth = rng.within(chain.lower(), chain.upper(), 0.3);
            let target = chain.fk(&truth);
            let seed: Vec<f64> = truth.iter().map(|v| v + 0.3 * (rng.unit() - 0.5)).collect();
            if let Ok(q) = ik.solve_near(&target, &seed) {
                let (position, rotation) = ik.errors(&target, &q);
                assert!(position <= 1e-6 && rotation <= 1e-6);
                assert!(chain.within_limits(&q));
                solved += 1;
            }
        }
        assert!(solved >= 19, "{name}: solved {solved} of 20");
    }
}

#[test]
fn several_solutions_are_found_and_unreachable_targets_are_explained() {
    let model = fixture("six_axis");
    let chain = model.default_chain().unwrap();
    let ik = NumericIk::new(&chain, IkOptions::default());
    let target = chain.fk(&[0.4, 0.5, -1.0, 0.3, 0.8, -0.2]);
    let solutions = ik.solve_many(&target, None, 48);
    assert!(solutions.len() >= 2, "found {}", solutions.len());
    for q in &solutions {
        assert!(ik.errors(&target, q).0 <= 1e-6);
    }
    // Deterministic: the same call gives the same solutions in the same order.
    assert_eq!(ik.solve_many(&target, None, 48), solutions);

    let far = DPose3::from_translation(DVec3::new(5.0, 0.0, 0.0));
    let failure = ik.solve_near(&far, &[0.0; 6]).unwrap_err();
    assert_eq!(failure.reason, IkFailureReason::OutOfReach);
    assert!(failure.best_q.is_some() && failure.pos_err > 3.0);
    assert_eq!(ik.solve_near(&far, &[0.0; 3]).unwrap_err().reason, IkFailureReason::BadInput);

    // The stretched-out arm is singular; a bent one is not.
    assert!(ik.conditioning(&[0.0, 0.0, 0.0, 0.0, 0.0, 0.0]) < 1e-6);
    assert!(ik.conditioning(&[0.4, 0.5, -1.0, 0.3, 0.8, -0.2]) > 1e-3);
}

#[test]
fn planning_chains_are_chosen_sensibly_and_mimic_joints_follow() {
    let ur = fixture("ur5e_kinematics");
    assert_eq!(ur.default_chain().unwrap().dof(), 6, "tool0 is preferred when it exists");
    let seven = fixture("seven_axis");
    let chain = seven.default_chain().unwrap();
    assert_eq!(chain.dof(), 7);
    assert_eq!(chain.kinds()[6], JointKind::Continuous);
    assert!(chain.upper()[6].is_infinite());
    // The mimic finger is not an independent joint, so the other finger has the longest path.
    let gantry = fixture("gantry_arm");
    assert_eq!(gantry.default_chain().unwrap().joint_names().last().unwrap(), "finger_left_slide");
    // Two equal branches: the user has to choose.
    let fork = r#"<robot name="fork"><link name="root"/><link name="left"/><link name="right"/>
        <joint name="l" type="revolute"><parent link="root"/><child link="left"/><limit lower="-1" upper="1" velocity="1"/></joint>
        <joint name="r" type="revolute"><parent link="root"/><child link="right"/><limit lower="-1" upper="1" velocity="1"/></joint></robot>"#;
    let (fork, _) = import(fork).unwrap();
    assert!(matches!(fork.default_chain(), Err(ChainError::Ambiguous(tips)) if tips.len() == 2));
    assert!(matches!(gantry.chain("rail", "nowhere"), Err(ChainError::UnknownLink(_))));
    assert!(matches!(gantry.chain("wrist", "rail"), Err(ChainError::NotConnected { .. })));

    // Opening the left finger by 2 cm closes the right one by the same amount (multiplier -1).
    let mut values = vec![0.0; gantry.joints.len()];
    values[gantry.joint("finger_left_slide").unwrap()] = 0.02;
    let mut poses = Vec::new();
    gantry.link_poses(&values, &mut poses);
    let (left, right) =
        (poses[gantry.link("finger_left").unwrap()], poses[gantry.link("finger_right").unwrap()]);
    assert!((left.translation.y - 0.04).abs() < 1e-12 && (right.translation.y + 0.04).abs() < 1e-12);
}

fn errors_of(text: &str) -> Vec<String> {
    match import(text) {
        Ok(_) => panic!("the file should have been refused"),
        Err(findings) => {
            findings.into_iter().filter(|f| f.severity == Severity::Error).map(|f| f.message).collect()
        }
    }
}

#[test]
fn broken_files_are_refused_with_a_message_that_names_the_problem() {
    let joint = |extra: &str| {
        format!(
            r#"<robot name="r"><link name="a"/><link name="b"/>
            <joint name="j" type="revolute"><parent link="a"/><child link="b"/>{extra}</joint></robot>"#
        )
    };
    let has = |text: &str, needle: &str| {
        let errors = errors_of(text);
        assert!(errors.iter().any(|e| e.contains(needle)), "expected `{needle}` in {errors:?}");
    };
    has("<robot", "not valid XML");
    has("<model/>", "not <robot>");
    has(r#"<robot name="r"><link name="a"/><joint name="${prefix}j" type="fixed"/></robot>"#, "xacro");
    has(&joint(""), "needs a <limit>");
    has(&joint(r#"<limit lower="1" upper="-1" velocity="1"/>"#), "above upper limit");
    has(
        &joint(r#"<axis xyz="0 0 0"/><limit lower="-1" upper="1" velocity="1"/>"#),
        "axis of joint `j` is zero",
    );
    has(&joint(r#"<limit lower="-1,5" upper="1" velocity="1"/>"#), "decimal separator");
    has(&joint(r#"<origin xyz="0 0"/><limit lower="-1" upper="1" velocity="1"/>"#), "three numbers");
    has(
        &joint(r#"<limit lower="-1" upper="1" velocity="1"/><mimic joint="ghost"/>"#),
        "unknown joint `ghost`",
    );
    has(r#"<robot name="r"><link name="a"/><link name="a"/></robot>"#, "defined twice");
    has(r#"<robot name="r"><link name="a"/><link name="b"/></robot>"#, "exactly one root");
    has(
        r#"<robot name="r"><link name="a"/><joint name="j" type="fixed"><parent link="a"/><child link="zz"/></joint></robot>"#,
        "unknown child link `zz`",
    );
    has(
        r#"<robot name="r"><link name="a"/><link name="b"/>
        <joint name="j" type="floating"><parent link="a"/><child link="b"/></joint></robot>"#,
        "not supported",
    );
    has(
        r#"<robot name="r"><link name="root"/><link name="a"/><link name="b"/>
        <joint name="j1" type="fixed"><parent link="a"/><child link="b"/></joint>
        <joint name="j2" type="fixed"><parent link="b"/><child link="a"/></joint></robot>"#,
        "loop",
    );
    // Findings carry the place in the file.
    let findings =
        import("<robot name=\"r\">\n  <link name=\"a\"/>\n  <link name=\"a\"/>\n</robot>").unwrap_err();
    assert_eq!((findings[0].line, findings[0].col), (3, 3));
}

#[test]
fn questionable_but_usable_files_load_with_warnings() {
    let text = r#"<robot name="r"><link name="a"><visual><geometry><box size="1 1 1"/></geometry></visual></link><link name="b"/>
        <joint name="j" type="continuous"><parent link="a"/><child link="b"/><axis xyz="0 0 2"/><limit lower="-1" upper="1"/></joint></robot>"#;
    let (model, warnings) = import(text).unwrap();
    assert_eq!(model.joints[0].axis, DVec3::Z, "the axis was normalised");
    assert!(model.joints[0].lower.is_infinite());
    let messages: Vec<&str> = warnings.iter().map(|w| w.message.as_str()).collect();
    for needle in [
        "not a unit vector",
        "position limits; they are ignored",
        "no velocity limit",
        "no collision geometry",
    ] {
        assert!(messages.iter().any(|m| m.contains(needle)), "expected `{needle}` in {messages:?}");
    }
    assert!(warnings.iter().all(|w| w.severity == Severity::Warning));
}
