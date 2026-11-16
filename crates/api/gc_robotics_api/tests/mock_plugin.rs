//! A plugin written against the API crates alone: closed-form inverse kinematics of a planar two-link arm.
//!
//! This is the conformance test of the contract: if it compiles and passes, a kinematics plugin needs
//! nothing but `gc_plugin_api` and `gc_robotics_api`.

#![allow(clippy::float_cmp, reason = "the test compares exactly representable values")]

use std::f64::consts::PI;
use std::sync::Arc;

use gc_plugin_api::{GC_PLUGIN_API_VERSION, Plugin, PluginError, PluginManifest, Registrar, Registry};
use gc_robotics_api::{
    ChainSpec, FamilyMetadata, Fit, IkFailure, IkFailureReason, JointKind, JointSpec, KINEMATICS_FACTORIES,
    KinError, KinematicsFactory, KinematicsSolver, MotionPlanner, Pose, RobotModelProvider, StateChecker,
    TimeParameterizer,
};

struct Planar2R {
    l1: f64,
    l2: f64,
}

impl KinematicsSolver for Planar2R {
    fn solve_all(
        &self,
        target: &Pose,
        _seed: Option<&[f64]>,
        out_q: &mut Vec<f64>,
        out_branch: &mut Vec<u32>,
    ) -> Result<usize, IkFailure> {
        out_q.clear();
        out_branch.clear();
        let [x, y, _] = target.xyz;
        let cos_elbow = (x * x + y * y - self.l1 * self.l1 - self.l2 * self.l2) / (2.0 * self.l1 * self.l2);
        if !(-1.0..=1.0).contains(&cos_elbow) {
            return Err(IkFailure {
                reason: IkFailureReason::OutOfReach,
                best_q: None,
                pos_err: f64::NAN,
                rot_err: f64::NAN,
            });
        }
        for (branch, sign) in [(0_u32, 1.0), (1, -1.0)] {
            let elbow = sign * libm::acos(cos_elbow);
            let shoulder = libm::atan2(y, x)
                - libm::atan2(self.l2 * libm::sin(elbow), self.l1 + self.l2 * libm::cos(elbow));
            out_q.extend([shoulder, elbow]);
            out_branch.push(branch);
        }
        Ok(2)
    }

    fn classify(&self, q: &[f64]) -> Option<u32> {
        match q.get(1) {
            Some(elbow) if *elbow > 0.0 => Some(0),
            Some(elbow) if *elbow < 0.0 => Some(1),
            _ => None,
        }
    }

    fn branch_label(&self, branch: u32) -> String {
        if branch == 0 { "elbow down".to_owned() } else { "elbow up".to_owned() }
    }
}

struct Planar2RFactory;

/// Link lengths of a chain made of two parallel Z-axis revolute joints with offsets along X.
fn link_lengths(chain: &ChainSpec) -> Option<(f64, f64)> {
    let [a, b] = chain.joints.as_slice() else { return None };
    let planar = |j: &JointSpec| {
        j.kind == JointKind::Revolute
            && j.axis == [0.0, 0.0, 1.0]
            && j.to_next.xyz[1] == 0.0
            && j.to_next.xyz[2] == 0.0
    };
    (planar(a) && planar(b) && chain.tip == Pose::IDENTITY).then_some((a.to_next.xyz[0], b.to_next.xyz[0]))
}

impl KinematicsFactory for Planar2RFactory {
    fn id(&self) -> &str {
        "test.planar2r"
    }

    fn probe(&self, chain: &ChainSpec, _meta: Option<&FamilyMetadata>) -> Option<Fit> {
        link_lengths(chain).map(|_| Fit::Exact)
    }

    fn build(&self, chain: &ChainSpec) -> Result<Box<dyn KinematicsSolver>, KinError> {
        let (l1, l2) =
            link_lengths(chain).ok_or_else(|| KinError::Unsupported("not a planar 2R chain".to_owned()))?;
        Ok(Box::new(Planar2R { l1, l2 }))
    }
}

struct MockPlugin;

impl Plugin for MockPlugin {
    fn manifest(&self) -> &PluginManifest {
        const MANIFEST: PluginManifest =
            PluginManifest { id: "test.mock", name: "Mock", version: "0.0.1", api: GC_PLUGIN_API_VERSION };
        &MANIFEST
    }

    fn register(&self, reg: &mut Registrar<'_>) -> Result<(), PluginError> {
        reg.add(KINEMATICS_FACTORIES, Arc::new(Planar2RFactory));
        Ok(())
    }
}

fn planar_chain(l1: f64, l2: f64) -> ChainSpec {
    let joint = |name: &str, length: f64| JointSpec {
        name: name.to_owned(),
        kind: JointKind::Revolute,
        axis: [0.0, 0.0, 1.0],
        lower: -PI,
        upper: PI,
        max_vel: Some(1.0),
        to_next: Pose::from_translation([length, 0.0, 0.0]),
    };
    ChainSpec {
        base_to_first: Pose::IDENTITY,
        joints: vec![joint("shoulder", l1), joint("elbow", l2)],
        tip: Pose::IDENTITY,
    }
}

#[test]
fn a_kinematics_plugin_round_trips_through_the_registry_and_the_reference_fk() {
    let mut registry = Registry::new();
    registry.load(&MockPlugin).unwrap();
    let factories = registry.get(KINEMATICS_FACTORIES);
    assert_eq!(factories.len(), 1);

    let chain = planar_chain(0.5, 0.3);
    let factory = &factories[0];
    assert_eq!(factory.probe(&chain, None), Some(Fit::Exact));
    let solver = factory.build(&chain).unwrap();

    let truth = [0.4, -1.1];
    let target = chain.fk(&truth).unwrap();
    let (mut qs, mut branches) = (Vec::new(), Vec::new());
    assert_eq!(solver.solve_all(&target, None, &mut qs, &mut branches).unwrap(), 2);
    assert_eq!(branches, vec![0, 1]);
    for (solution, branch) in qs.chunks_exact(2).zip(&branches) {
        let (position_error, _) = chain.fk(solution).unwrap().error_to(&target);
        assert!(position_error < 1e-12, "every solution reaches the target");
        assert_eq!(solver.classify(solution), Some(*branch));
    }
    let elbow_up = &qs[2..4];
    assert!((elbow_up[0] - truth[0]).abs() < 1e-12 && (elbow_up[1] - truth[1]).abs() < 1e-12);
    assert_eq!(solver.branch_label(1), "elbow up");

    let far = Pose::from_translation([2.0, 0.0, 0.0]);
    assert_eq!(
        solver.solve_all(&far, None, &mut qs, &mut branches).unwrap_err().reason,
        IkFailureReason::OutOfReach
    );
    assert!(qs.is_empty());
}

#[test]
fn a_factory_refuses_chains_of_another_family() {
    let mut chain = planar_chain(0.5, 0.3);
    chain.joints[1].kind = JointKind::Prismatic;
    assert_eq!(Planar2RFactory.probe(&chain, None), None);
    assert!(Planar2RFactory.build(&chain).is_err());
}

#[test]
fn api_traits_are_object_safe() {
    const _: Option<&dyn KinematicsFactory> = None;
    const _: Option<&dyn KinematicsSolver> = None;
    const _: Option<&dyn MotionPlanner> = None;
    const _: Option<&dyn TimeParameterizer> = None;
    const _: Option<&dyn StateChecker> = None;
    const _: Option<&dyn RobotModelProvider> = None;
}
