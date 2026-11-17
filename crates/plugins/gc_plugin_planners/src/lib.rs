//! Registers the built-in planners (RRT-Connect, RRT, RRT*, Informed RRT*, PRM from `gc_plan`) through
//! the robotics API, so that the host finds them in the same registry as planners from other plugins.
//!
//! Each planner keeps the id documents already store (`rrt_connect`, `rrt`, `rrt_star`,
//! `informed_rrt_star`, `prm`).

#![forbid(unsafe_code)]

use std::sync::Arc;

use gc_plan::{Algorithm, Control, Failure, Options, Request, Space, Validity, plan};
use gc_plugin_api::{
    GC_PLUGIN_API_VERSION, ParamKind, ParamSchema, ParamSpec, ParamValue, Plugin, PluginError,
    PluginManifest, Registrar, Text,
};
use gc_robotics_api::{
    MOTION_PLANNERS, MotionPlanner, PlanControl, PlanFailure, PlanOutput, PlanRequest, PlanStats,
    PlannerCaps, PlannerDescriptor, StateChecker,
};

/// The built-in planners as a plugin.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlannersPlugin;

impl Plugin for PlannersPlugin {
    fn manifest(&self) -> &PluginManifest {
        const MANIFEST: PluginManifest = PluginManifest {
            id: "gc.planners",
            name: "Built-in planners",
            version: "0.1.0",
            api: GC_PLUGIN_API_VERSION,
        };
        &MANIFEST
    }

    fn register(&self, reg: &mut Registrar<'_>) -> Result<(), PluginError> {
        for planner in builtin() {
            reg.add(MOTION_PLANNERS, Arc::new(planner) as Arc<dyn MotionPlanner>);
        }
        Ok(())
    }
}

/// One of the built-in planners.
#[derive(Clone, Debug)]
pub struct BuiltinPlanner {
    algorithm: Algorithm,
    descriptor: PlannerDescriptor,
}

impl BuiltinPlanner {
    /// The `gc_plan` algorithm behind it.
    pub fn algorithm(&self) -> Algorithm {
        self.algorithm
    }
}

/// The five built-in planners, the default (RRT-Connect) first.
pub fn builtin() -> Vec<BuiltinPlanner> {
    let defaults = Options::default();
    let spec = |key: &str, ko: &str, en: &str, kind| ParamSpec {
        key: key.to_owned(),
        label: Text::new(ko, en),
        kind,
    };
    let range = spec(
        "range",
        "확장 거리(관절당, rad)",
        "Extension range (per joint, rad)",
        ParamKind::Float { default: defaults.range, min: 0.01, max: 3.0 },
    );
    let bias = spec(
        "goal_bias",
        "목표 편향",
        "Goal bias",
        ParamKind::Float { default: defaults.goal_bias, min: 0.0, max: 1.0 },
    );
    let samples = spec(
        "prm_samples",
        "로드맵 표본 수(회당)",
        "Roadmap samples (per round)",
        ParamKind::Int { default: to_i64(defaults.prm_samples), min: 10, max: 100_000 },
    );
    let neighbours = spec(
        "prm_neighbours",
        "이웃 수",
        "Neighbours",
        ParamKind::Int { default: to_i64(defaults.prm_neighbours), min: 2, max: 100 },
    );
    let planner =
        |algorithm, id: &str, ko: &str, en: &str, params: Vec<ParamSpec>, optimizing| BuiltinPlanner {
            algorithm,
            descriptor: PlannerDescriptor {
                id: id.to_owned(),
                name: Text::new(ko, en),
                version: 2,
                params: ParamSchema { params },
                caps: PlannerCaps { optimizing, multi_goal: true },
            },
        };
    vec![
        planner(
            Algorithm::RrtConnect,
            "rrt_connect",
            "RRT-Connect",
            "RRT-Connect",
            vec![range.clone()],
            false,
        ),
        planner(Algorithm::Rrt, "rrt", "RRT", "RRT", vec![range.clone(), bias.clone()], false),
        planner(Algorithm::RrtStar, "rrt_star", "RRT*", "RRT*", vec![range.clone(), bias.clone()], true),
        planner(
            Algorithm::InformedRrtStar,
            "informed_rrt_star",
            "Informed RRT*",
            "Informed RRT*",
            vec![range.clone(), bias],
            true,
        ),
        planner(Algorithm::Prm, "prm", "PRM", "PRM", vec![samples, neighbours, range], false),
    ]
}

fn to_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn to_usize(value: i64) -> usize {
    usize::try_from(value.max(0)).unwrap_or(usize::MAX)
}

/// The host's checker as `gc_plan` sees it.
struct Checker<'a>(&'a dyn StateChecker);

impl Validity for Checker<'_> {
    fn state_valid(&self, q: &[f64]) -> bool {
        self.0.state_valid(q)
    }

    fn motion_valid(&self, a: &[f64], b: &[f64]) -> bool {
        self.0.motion_valid(a, b)
    }

    // The API's checker does not say how many states a motion check examines (see
    // docs/handoff/backend.md): each motion counts as one state check until it does.
}

struct Cancel<'a>(&'a dyn PlanControl);

impl Control for Cancel<'_> {
    fn cancelled(&self) -> bool {
        self.0.is_cancelled()
    }
}

impl MotionPlanner for BuiltinPlanner {
    fn descriptor(&self) -> &PlannerDescriptor {
        &self.descriptor
    }

    fn plan(
        &self,
        req: &PlanRequest<'_>,
        checker: &dyn StateChecker,
        control: &dyn PlanControl,
    ) -> Result<PlanOutput, PlanFailure> {
        let space = req.space;
        if !space.is_consistent() {
            return Err(PlanFailure::InvalidRequest("the space is not consistent".to_owned()));
        }
        if space.wrapping.iter().any(|w| *w) {
            return Err(PlanFailure::InvalidRequest("wrapping dimensions are not supported".to_owned()));
        }
        let dof = space.dof();
        if dof == 0 || req.start.len() != dof || req.goals.is_empty() || !req.goals.len().is_multiple_of(dof)
        {
            return Err(PlanFailure::InvalidRequest(
                "start and goals must have the space's dimension".to_owned(),
            ));
        }
        let mut options = Options {
            max_iterations: req.budget.max_iterations,
            max_state_checks: req.budget.max_state_checks,
            ..Options::default()
        };
        for (key, value) in req.params {
            match (key.as_str(), value) {
                ("range", ParamValue::Float(v)) => options.range = *v,
                ("goal_bias", ParamValue::Float(v)) => options.goal_bias = *v,
                ("prm_samples", ParamValue::Int(v)) => options.prm_samples = to_usize(*v),
                ("prm_neighbours", ParamValue::Int(v)) => options.prm_neighbours = to_usize(*v),
                _ => return Err(PlanFailure::InvalidRequest(format!("unknown parameter `{key}`"))),
            }
        }
        let plan_space = Space { lo: space.lo.clone(), hi: space.hi.clone(), weights: space.weights.clone() };
        let goals: Vec<Vec<f64>> = req.goals.chunks(dof).map(<[f64]>::to_vec).collect();
        let request = Request {
            space: &plan_space,
            start: req.start,
            goals: &goals,
            algorithm: self.algorithm,
            options,
            seed: req.seed,
        };
        let stats = |s: gc_plan::Stats| PlanStats {
            iterations: s.iterations,
            state_checks: s.state_checks,
            motion_checks: s.motion_checks,
            nodes: s.nodes,
        };
        match plan(&request, &Checker(checker), &Cancel(control)) {
            Ok(solution) => Ok(PlanOutput {
                path: solution.path.concat(),
                goal_index: solution.goal_index,
                cost: solution.cost,
                stats: stats(solution.stats),
            }),
            Err(Failure::InvalidRequest(reason)) => Err(PlanFailure::InvalidRequest(reason)),
            Err(Failure::StartInvalid) => Err(PlanFailure::StartInvalid),
            Err(Failure::NoValidGoal) => Err(PlanFailure::NoValidGoal),
            Err(Failure::NoPath { stats: s, nearest_goal_distance }) => {
                Err(PlanFailure::NoPathWithinBudget { stats: stats(s), nearest_goal_distance })
            }
            Err(Failure::Cancelled) => Err(PlanFailure::Cancelled),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

    use gc_plugin_api::Registry;
    use gc_robotics_api::{PlanBudget, SpaceSpec};

    use super::*;

    /// A point in the plane with a wall that has a gap at the top.
    struct Wall;

    impl StateChecker for Wall {
        fn state_valid(&self, q: &[f64]) -> bool {
            !(q[0] >= 4.5 && q[0] <= 5.5 && q[1] <= 8.0)
        }

        fn motion_valid(&self, a: &[f64], b: &[f64]) -> bool {
            (0..=200).all(|k| {
                let t = f64::from(k) / 200.0;
                StateChecker::state_valid(self, &[a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t])
            })
        }

        fn clearance(&self, _q: &[f64]) -> Option<f64> {
            None
        }
    }

    impl Validity for Wall {
        fn state_valid(&self, q: &[f64]) -> bool {
            StateChecker::state_valid(self, q)
        }

        fn motion_valid(&self, a: &[f64], b: &[f64]) -> bool {
            StateChecker::motion_valid(self, a, b)
        }
    }

    struct Never;

    impl PlanControl for Never {
        fn is_cancelled(&self) -> bool {
            false
        }
    }

    #[test]
    fn the_planners_register_and_plan_like_the_library() {
        let mut registry = Registry::default();
        registry.load(&PlannersPlugin).unwrap();
        let planners = registry.get(MOTION_PLANNERS);
        let ids: Vec<&str> = planners.iter().map(|p| p.descriptor().id.as_str()).collect();
        assert_eq!(ids, ["rrt_connect", "rrt", "rrt_star", "informed_rrt_star", "prm"]);

        let space = SpaceSpec {
            lo: vec![0.0; 2],
            hi: vec![10.0; 2],
            weights: vec![1.0; 2],
            wrapping: vec![false; 2],
        };
        let budget = PlanBudget { max_iterations: 6_000, max_state_checks: 100_000 };
        for (planner, builtin) in planners.iter().zip(builtin()) {
            let params = planner.descriptor().params.defaults();
            let request = PlanRequest {
                space: &space,
                start: &[1.0, 1.0],
                goals: &[9.0, 1.0],
                params: &params,
                seed: 4,
                budget,
            };
            let output = planner.plan(&request, &Wall, &Never).unwrap();
            // The same as calling the library with the same settings.
            let library = plan(
                &Request {
                    space: &Space { lo: vec![0.0; 2], hi: vec![10.0; 2], weights: vec![1.0; 2] },
                    start: &[1.0, 1.0],
                    goals: &[vec![9.0, 1.0]],
                    algorithm: builtin.algorithm(),
                    options: Options {
                        max_iterations: 6_000,
                        max_state_checks: 100_000,
                        ..Options::default()
                    },
                    seed: 4,
                },
                &Wall,
                &gc_plan::NeverCancel,
            )
            .unwrap();
            assert_eq!(output.path, library.path.concat(), "{}", planner.descriptor().id);
            assert_eq!(output.stats.state_checks, library.stats.state_checks);
        }

        // Parameters the planner does not take, and wrapping joints, are refused.
        let mut params = planners[0].descriptor().params.defaults();
        params.insert("nonsense".to_owned(), ParamValue::Bool(true));
        let request = PlanRequest {
            space: &space,
            start: &[1.0, 1.0],
            goals: &[9.0, 1.0],
            params: &params,
            seed: 0,
            budget,
        };
        assert!(matches!(planners[0].plan(&request, &Wall, &Never), Err(PlanFailure::InvalidRequest(_))));
    }
}
