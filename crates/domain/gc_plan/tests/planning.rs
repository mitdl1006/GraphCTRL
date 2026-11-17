//! Planner tests in worlds whose answers are known: a point moving among rectangular obstacles.

#![allow(
    clippy::float_cmp,
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: exact values are compared and failures should panic"
)]

use std::cell::Cell;

use gc_plan::{
    Algorithm, Control, Failure, NeverCancel, Options, Request, Rng, Solution, Space, Validity, densify,
    path_cost, plan, shortcut, smooth_corners, validate,
};

const ALL: [Algorithm; 5] =
    [Algorithm::RrtConnect, Algorithm::Rrt, Algorithm::RrtStar, Algorithm::InformedRrtStar, Algorithm::Prm];

/// A point in the plane; obstacles are axis-aligned rectangles `[x0, y0, x1, y1]`.
struct World {
    obstacles: Vec<[f64; 4]>,
    step: f64,
}

impl Validity for World {
    fn state_valid(&self, q: &[f64]) -> bool {
        !self.obstacles.iter().any(|o| q[0] >= o[0] && q[0] <= o[2] && q[1] >= o[1] && q[1] <= o[3])
    }

    fn motion_valid(&self, a: &[f64], b: &[f64]) -> bool {
        let pieces = self.motion_cost(a, b);
        (0..=pieces).all(|k| {
            let t = k as f64 / pieces as f64;
            self.state_valid(&[a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t])
        })
    }

    fn motion_cost(&self, a: &[f64], b: &[f64]) -> u64 {
        let length = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        ((length / self.step).ceil() as u64).max(1)
    }
}

fn arena() -> Space {
    Space { lo: vec![0.0, 0.0], hi: vec![10.0, 10.0], weights: vec![1.0, 1.0] }
}

/// A wall across the arena with one gap near the top.
fn wall() -> World {
    World { obstacles: vec![[4.5, 0.0, 5.5, 8.0]], step: 0.02 }
}

fn options() -> Options {
    Options { max_iterations: 6_000, max_state_checks: 600_000, window_margin: 20.0, ..Options::default() }
}

fn solve(world: &World, algorithm: Algorithm, seed: u64, goals: &[Vec<f64>]) -> Result<Solution, Failure> {
    let space = arena();
    let request = Request { space: &space, start: &[1.0, 1.0], goals, algorithm, options: options(), seed };
    plan(&request, world, &NeverCancel)
}

#[test]
fn every_planner_finds_a_valid_path_through_the_gap() {
    let world = wall();
    let goal = vec![vec![9.0, 1.0]];
    for algorithm in ALL {
        for seed in 0..5 {
            let solution = solve(&world, algorithm, seed, &goal)
                .unwrap_or_else(|e| panic!("{algorithm:?} seed {seed}: {e}"));
            assert_eq!(solution.path.first().unwrap(), &vec![1.0, 1.0]);
            assert_eq!(solution.path.last().unwrap(), &goal[0]);
            assert!(validate(&solution.path, &world), "{algorithm:?} seed {seed}");
            assert!((solution.cost - path_cost(&arena(), &solution.path)).abs() < 1e-12);
            // No path can be shorter than the way over the wall.
            let lower_bound = 2.0 * ((3.5_f64).powi(2) + 7.0_f64.powi(2)).sqrt();
            assert!(solution.cost >= lower_bound - 1e-9, "{algorithm:?}: {}", solution.cost);
            assert!(solution.stats.state_checks > 0 && solution.stats.nodes > 0);
        }
    }
}

#[test]
fn the_same_seed_gives_the_same_path_bit_for_bit() {
    let world = wall();
    let goal = vec![vec![9.0, 1.0]];
    for algorithm in ALL {
        let (first, second) =
            (solve(&world, algorithm, 42, &goal).unwrap(), solve(&world, algorithm, 42, &goal).unwrap());
        assert_eq!(first, second, "{algorithm:?}");
        let other = solve(&world, algorithm, 43, &goal).unwrap();
        assert_ne!(first.path, other.path, "{algorithm:?}: another seed explores differently");
    }
}

#[test]
fn optimising_planners_approach_the_shortest_path() {
    let empty = World { obstacles: Vec::new(), step: 0.02 };
    let goal = vec![vec![9.0, 7.0]];
    let straight = (8.0_f64.powi(2) + 6.0_f64.powi(2)).sqrt();
    for algorithm in [Algorithm::RrtStar, Algorithm::InformedRrtStar] {
        let costs: Vec<f64> =
            (0..4).map(|seed| solve(&empty, algorithm, seed, &goal).unwrap().cost).collect();
        let worst = costs.iter().fold(0.0_f64, |a, b| a.max(*b));
        assert!(worst <= straight * 1.10, "{algorithm:?}: {costs:?} vs straight {straight}");
    }
    let world = wall();
    let goal = vec![vec![9.0, 1.0]];
    let mean =
        |algorithm| (0..4).map(|seed| solve(&world, algorithm, seed, &goal).unwrap().cost).sum::<f64>() / 4.0;
    assert!(mean(Algorithm::RrtStar) < mean(Algorithm::Rrt), "RRT* must beat plain RRT on average");
}

#[test]
fn failures_say_what_went_wrong() {
    let world = wall();
    let space = arena();
    let request = |start: &'static [f64], goals: &'static [Vec<f64>]| Request {
        space: arena_static(),
        start,
        goals,
        algorithm: Algorithm::RrtConnect,
        options: Options { max_iterations: 300, ..options() },
        seed: 1,
    };
    fn arena_static() -> &'static Space {
        Box::leak(Box::new(arena()))
    }
    let inside_wall: &'static [Vec<f64>] = Box::leak(Box::new(vec![vec![5.0, 4.0]]));
    let fine: &'static [Vec<f64>] = Box::leak(Box::new(vec![vec![9.0, 1.0]]));
    let none: &'static [Vec<f64>] = Box::leak(Box::new(Vec::new()));
    assert_eq!(plan(&request(&[5.0, 4.0], fine), &world, &NeverCancel), Err(Failure::StartInvalid));
    assert_eq!(plan(&request(&[20.0, 1.0], fine), &world, &NeverCancel), Err(Failure::StartInvalid));
    assert_eq!(plan(&request(&[1.0, 1.0], inside_wall), &world, &NeverCancel), Err(Failure::NoValidGoal));
    assert!(matches!(
        plan(&request(&[1.0, 1.0], none), &world, &NeverCancel),
        Err(Failure::InvalidRequest(_))
    ));
    assert!(matches!(plan(&request(&[1.0], fine), &world, &NeverCancel), Err(Failure::InvalidRequest(_))));

    // A goal walled in on all sides: the budget runs out and the report says how close the search came.
    let sealed = World { obstacles: vec![[4.5, 0.0, 5.5, 10.0]], step: 0.02 };
    for algorithm in ALL {
        let request = Request {
            space: &space,
            start: &[1.0, 1.0],
            goals: fine,
            algorithm,
            options: Options { max_iterations: 400, ..options() },
            seed: 3,
        };
        match plan(&request, &sealed, &NeverCancel) {
            Err(Failure::NoPath { stats, nearest_goal_distance }) => {
                assert!(stats.iterations <= 800, "{algorithm:?}: the budget bounds the work");
                assert!(
                    (3.5..9.0).contains(&nearest_goal_distance),
                    "{algorithm:?}: {nearest_goal_distance}"
                );
            }
            other => panic!("{algorithm:?}: {other:?}"),
        }
    }
}

#[test]
fn with_several_goals_a_reachable_one_is_chosen() {
    let sealed_right = World { obstacles: vec![[4.5, 0.0, 5.5, 10.0]], step: 0.02 };
    let goals = vec![vec![9.0, 1.0], vec![2.0, 9.0], vec![5.0, 5.0]];
    for algorithm in ALL {
        let solution =
            solve(&sealed_right, algorithm, 5, &goals).unwrap_or_else(|e| panic!("{algorithm:?}: {e}"));
        assert_eq!(solution.goal_index, 1, "{algorithm:?}");
        assert_eq!(solution.path.last().unwrap(), &goals[1]);
    }
}

#[test]
fn cancelling_stops_the_search_at_once() {
    struct After(Cell<u32>);
    impl Control for After {
        fn cancelled(&self) -> bool {
            let left = self.0.get();
            self.0.set(left.saturating_sub(1));
            left == 0
        }
    }
    let sealed = World { obstacles: vec![[4.5, 0.0, 5.5, 10.0]], step: 0.02 };
    let space = arena();
    let goals = vec![vec![9.0, 1.0]];
    for algorithm in ALL {
        let request = Request {
            space: &space,
            start: &[1.0, 1.0],
            goals: &goals,
            algorithm,
            options: options(),
            seed: 1,
        };
        assert_eq!(plan(&request, &sealed, &After(Cell::new(10))), Err(Failure::Cancelled), "{algorithm:?}");
    }
}

#[test]
fn simplification_shortens_without_ever_breaking_the_path() {
    let world = wall();
    let space = arena();
    for seed in 0..6 {
        let solution = solve(&world, Algorithm::RrtConnect, seed, &[vec![9.0, 1.0]]).unwrap();
        let mut path = solution.path.clone();
        let mut rng = Rng::new(seed);
        let taken = shortcut(&space, &mut path, &world, &mut rng, 200);
        assert!(taken > 0);
        let after_shortcut = path_cost(&space, &path);
        assert!(after_shortcut < solution.cost);
        assert!(validate(&path, &world), "seed {seed}");
        // Close to the best possible way through the gap.
        let best = 2.0 * (3.5_f64.powi(2) + 7.0_f64.powi(2)).sqrt();
        assert!(after_shortcut < best * 1.25, "seed {seed}: {after_shortcut} vs {best}");

        smooth_corners(&space, &mut path, &world, 2);
        assert!(validate(&path, &world));
        assert!(path_cost(&space, &path) <= after_shortcut + 1e-9, "cutting corners never lengthens");
        assert_eq!((path.first().unwrap(), path.last().unwrap()), (&vec![1.0, 1.0], &vec![9.0, 1.0]));

        let dense = densify(&space, &path, 0.1);
        assert!(dense.windows(2).all(|p| (p[1][0] - p[0][0]).abs() <= 0.1 + 1e-12 && (p[1][1] - p[0][1]).abs() <= 0.1 + 1e-12));
        assert!((path_cost(&space, &dense) - path_cost(&space, &path)).abs() < 1e-9);
    }
}

#[test]
fn a_tight_sampling_window_is_widened_when_the_way_leads_outside_it() {
    // Start and goal are close together but separated by a wall whose only gap is far away.
    let world = World { obstacles: vec![[4.9, 0.0, 5.1, 9.0]], step: 0.02 };
    let space = arena();
    let goals = vec![vec![5.6, 1.0]];
    let request = Request {
        space: &space,
        start: &[4.4, 1.0],
        goals: &goals,
        algorithm: Algorithm::RrtConnect,
        options: Options {
            window_margin: 0.5,
            max_iterations: 3_000,
            max_state_checks: 2_000_000,
            ..Options::default()
        },
        seed: 2,
    };
    let solution = plan(&request, &world, &NeverCancel).unwrap();
    assert!(validate(&solution.path, &world));
    assert!(solution.path.iter().any(|q| q[1] > 9.0), "the path goes over the wall");
}
