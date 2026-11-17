//! Tier B planning quality bench (plan, section 7 and appendix E).
//!
//! Runs every planner on the scenes in `fixtures/scenes/` with a UR5e for a sweep of seeds and writes a
//! Markdown report: success rate, state checks, path length, best-of-4 consistency, independent
//! re-validation (Q1) and bit-for-bit repeatability (Q9). Each planner run is single-threaded; seeds are
//! spread over threads. Run it through `cargo xtask bench-quality`, which builds it with `--release`.
//!
//! ```text
//! bench_quality [--seeds N] [--out FILE] [--scenes DIR] [--only S2]
//! bench_quality --ik FILE x y z roll pitch yaw     # list the IK solutions of a TCP pose in a scene
//! ```

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a measurement tool run by hand: printing is its output and bad input should stop it"
)]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use gc_collide::ShapeDesc;
use gc_math::{DPose3, DVec3, quat_from_rpy};
use gc_motion::{Obstacle, RobotScene};
use gc_plan::{Algorithm, NeverCancel, Options, Request, Rng, Space, Validity, path_cost, plan, shortcut};
use gc_robot::{IkOptions, NumericIk, import};

const ALGORITHMS: [(Algorithm, &str); 5] = [
    (Algorithm::RrtConnect, "RRT-Connect"),
    (Algorithm::Rrt, "RRT"),
    (Algorithm::RrtStar, "RRT*"),
    (Algorithm::InformedRrtStar, "Informed RRT*"),
    (Algorithm::Prm, "PRM"),
];

/// Shortcut attempts after planning, as in the motion pipeline.
const SHORTCUTS: usize = 150;
/// Seeds that are planned twice to check repeatability.
const REPEATED_SEEDS: u64 = 10;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// A scene fixture.
struct Fixture {
    id: String,
    title: String,
    padding: f64,
    budget: u64,
    /// Tool: a cylinder (radius, length) along the flange's Z axis; the TCP is at its far end.
    tool: Option<(f64, f64)>,
    obstacles: Vec<Obstacle>,
    start: Vec<f64>,
    goal_pose: Option<DPose3>,
    goal_joints: Option<Vec<f64>>,
    /// Reference goal: the IK solution nearest to this is listed first (and is the straight-line goal).
    targets: String,
}

fn numbers(words: &[&str], count: usize, line: &str) -> Vec<f64> {
    assert!(words.len() >= count, "expected {count} numbers in `{line}`");
    words[..count]
        .iter()
        .map(|w| w.parse::<f64>().unwrap_or_else(|_| panic!("bad number `{w}` in `{line}`")))
        .collect()
}

fn read_fixture(path: &Path) -> Fixture {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut fixture = Fixture {
        id: path.file_stem().unwrap().to_string_lossy().split('_').next().unwrap().to_uppercase(),
        title: String::new(),
        padding: 0.005,
        budget: 20_000,
        tool: None,
        obstacles: Vec::new(),
        start: Vec::new(),
        goal_pose: None,
        goal_joints: None,
        targets: String::new(),
    };
    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        let words: Vec<&str> = line.split_whitespace().collect();
        let Some((key, rest)) = words.split_first() else { continue };
        match *key {
            "title" => fixture.title = rest.join(" "),
            "padding" => fixture.padding = numbers(rest, 1, line)[0],
            "budget" => fixture.budget = numbers(rest, 1, line)[0] as u64,
            "tool" => {
                let v = numbers(rest, 2, line);
                fixture.tool = Some((v[0], v[1]));
            }
            "box" => {
                let (name, values) = rest.split_first().expect("box needs a name");
                let v = numbers(values, 6, line);
                fixture.obstacles.push(Obstacle {
                    name: (*name).to_owned(),
                    pose: DPose3::from_translation(DVec3::new(v[0], v[1], v[2])),
                    shape: ShapeDesc::Box([v[3], v[4], v[5]]),
                });
            }
            "start" => fixture.start = numbers(rest, 6, line),
            "goal_joints" => fixture.goal_joints = Some(numbers(rest, 6, line)),
            "goal_pose" => {
                let v = numbers(rest, 6, line);
                fixture.goal_pose = Some(DPose3::from_parts(
                    DVec3::new(v[0], v[1], v[2]),
                    quat_from_rpy(v[3].to_radians(), v[4].to_radians(), v[5].to_radians()),
                ));
                fixture.targets = format!(
                    "TCP ({:.3}, {:.3}, {:.3}) m, RPY ({}, {}, {})°",
                    v[0], v[1], v[2], v[3], v[4], v[5]
                );
            }
            other => panic!("{}: unknown key `{other}`", path.display()),
        }
    }
    assert_eq!(fixture.start.len(), 6, "{}: no start", path.display());
    assert!(fixture.goal_pose.is_some() || fixture.goal_joints.is_some(), "{}: no goal", path.display());
    fixture
}

fn build_scene(fixture: &Fixture) -> RobotScene {
    // The robot library (`robots/ur5e/`); the package name of a mesh URI is a folder of the library.
    let library = root().join("robots");
    let text = std::fs::read_to_string(library.join("ur5e/ur5e.urdf")).unwrap();
    let (model, _) = import(&text).unwrap_or_else(|findings| panic!("{findings:?}"));
    let chain = model.chain("base_link", "tool0").unwrap();
    let meshes = |uri: &str| {
        let relative = uri.strip_prefix("package://")?;
        let bytes = std::fs::read(library.join(relative)).ok()?;
        gc_mesh::stl::parse(&bytes).ok().map(Arc::new)
    };
    let tool: Vec<(DPose3, ShapeDesc)> = fixture
        .tool
        .iter()
        .map(|(radius, length)| {
            (
                DPose3::from_translation(DVec3::new(0.0, 0.0, length / 2.0)),
                ShapeDesc::Cylinder { radius: *radius, length: *length },
            )
        })
        .collect();
    let (scene, missing) = RobotScene::new(
        Arc::new(model),
        chain,
        "tool0",
        DPose3::IDENTITY,
        &fixture.obstacles,
        fixture.padding,
        &meshes,
        &tool,
    )
    .unwrap();
    assert!(missing.iter().all(|uri| uri.contains("/visual/")), "missing collision meshes: {missing:?}");
    let tcp = fixture.tool.map_or(0.0, |(_, length)| length);
    scene.with_tool(DPose3::from_translation(DVec3::new(0.0, 0.0, tcp)))
}

/// The planning space of the motion pipeline: joint limits, joints weighted by how slow they are.
fn space_of(scene: &RobotScene) -> Space {
    let chain = scene.chain();
    let max_vel: Vec<f64> = chain.max_vel().iter().map(|v| v.unwrap()).collect();
    let slowest = max_vel.iter().fold(0.0_f64, |acc, v| acc.max(1.0 / v));
    Space {
        lo: chain.lower().to_vec(),
        hi: chain.upper().to_vec(),
        weights: max_vel.iter().map(|v| (1.0 / v) / slowest).collect(),
    }
}

/// The goal set, as the motion pipeline builds it: valid IK solutions, nearest to the start first, at
/// most eight.
fn goals_of(scene: &RobotScene, fixture: &Fixture, space: &Space) -> Vec<Vec<f64>> {
    let mut goals = match (&fixture.goal_joints, &fixture.goal_pose) {
        (Some(q), _) => vec![q.clone()],
        (None, Some(pose)) => {
            let ik = NumericIk::new(scene.chain(), IkOptions::default());
            ik.solve_many(&scene.flange_target(pose), Some(&fixture.start), 32)
        }
        (None, None) => Vec::new(),
    };
    goals.retain(|q| scene.state_problem(q).is_none());
    goals.sort_by(|a, b| space.distance(&fixture.start, a).total_cmp(&space.distance(&fixture.start, b)));
    goals.truncate(8);
    goals
}

/// The outcome of one planner run.
#[derive(Clone, Debug)]
struct Run {
    success: bool,
    checks: u64,
    /// Weighted joint-space length after shortcutting.
    cost: f64,
    /// The path failed the independent re-validation.
    invalid: bool,
    millis: f64,
    path: Vec<Vec<f64>>,
}

/// The checking fineness the motion pipeline would end up planning at: the coarsest at which the start
/// and at least one goal keep the planning margin.
fn fineness_of(scene: &RobotScene, start: &[f64], goals: &[Vec<f64>]) -> Option<f64> {
    [1.0, 0.25, 0.0625].into_iter().find(|f| {
        let checker = scene.checker(*f);
        checker.state_valid(start) && goals.iter().any(|g| checker.state_valid(g))
    })
}

#[allow(clippy::too_many_arguments, reason = "one measurement's inputs")]
fn run_once(
    scene: &RobotScene,
    space: &Space,
    start: &[f64],
    goals: &[Vec<f64>],
    fineness: f64,
    algorithm: Algorithm,
    budget: u64,
    seed: u64,
) -> Run {
    let checker = scene.checker(fineness);
    let strict = scene.strict_checker();
    let goals: Vec<Vec<f64>> = goals.iter().filter(|g| checker.state_valid(g)).cloned().collect();
    let goals = goals.as_slice();
    let options =
        Options { max_state_checks: budget, max_iterations: budget.saturating_mul(10), ..Options::default() };
    let request = Request { space, start, goals, algorithm, options, seed };
    let clock = Instant::now();
    // As in the motion pipeline: the straight move to the nearest goal is taken at once; a straight move
    // to a goal further away only competes on length with the planner.
    let mut straight_checks = 0;
    let mut straight = None;
    for goal in goals {
        straight_checks += checker.motion_cost(start, goal);
        if checker.motion_valid(start, goal) {
            straight = Some(vec![start.to_vec(), goal.clone()]);
            break;
        }
    }
    let planned = match &straight {
        Some(line) if line[1] == goals[0] => Ok((line.clone(), straight_checks)),
        _ => match plan(&request, &checker, &NeverCancel) {
            Ok(solution) => {
                let checks = solution.stats.state_checks + straight_checks;
                let mut path = solution.path;
                shortcut(space, &mut path, &checker, &mut Rng::new(seed), SHORTCUTS);
                match &straight {
                    Some(line) if path_cost(space, line) <= path_cost(space, &path) => {
                        Ok((line.clone(), checks))
                    }
                    _ => Ok((path, checks)),
                }
            }
            Err(gc_plan::Failure::NoPath { stats, .. }) if straight.is_some() => {
                Ok((straight.clone().unwrap_or_default(), stats.state_checks + straight_checks))
            }
            Err(error) => Err(error),
        },
    };
    match planned {
        Ok((path, checks)) => {
            let millis = clock.elapsed().as_secs_f64() * 1e3;
            // Independent re-validation: the required padding, every state and every motion, and the ends.
            let ends = path.first().is_some_and(|q| q.as_slice() == start)
                && path.last().is_some_and(|q| goals.iter().any(|g| g == q));
            let valid = ends
                && path.iter().all(|q| space.contains(q) && strict.state_valid(q))
                && path.windows(2).all(|pair| strict.motion_valid(&pair[0], &pair[1]));
            Run { success: true, checks, cost: path_cost(space, &path), invalid: !valid, millis, path }
        }
        Err(gc_plan::Failure::NoPath { stats, .. }) => Run {
            success: false,
            checks: stats.state_checks + straight_checks,
            cost: f64::NAN,
            invalid: false,
            millis: clock.elapsed().as_secs_f64() * 1e3,
            path: Vec::new(),
        },
        Err(other) => panic!("planning request rejected: {other}"),
    }
}

fn median(values: &mut [f64]) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    values.sort_by(f64::total_cmp);
    let n = values.len();
    if n % 2 == 1 { values[n / 2] } else { (values[n / 2 - 1] + values[n / 2]) / 2.0 }
}

fn coefficient_of_variation(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return f64::NAN;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (values.len() - 1) as f64;
    var.sqrt() / mean
}

fn bits(path: &[Vec<f64>]) -> Vec<u64> {
    path.iter().flatten().map(|v| v.to_bits()).collect()
}

/// Spreads the seeds over threads; results come back in seed order.
fn sweep(
    scene: &RobotScene,
    space: &Space,
    fixture: &Fixture,
    goals: &[Vec<f64>],
    fineness: f64,
    algorithm: Algorithm,
    seeds: u64,
) -> Vec<Run> {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).min(seeds as usize).max(1);
    let mut runs: Vec<Option<Run>> = vec![None; seeds as usize];
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads)
            .map(|t| {
                scope.spawn(move || {
                    (0..seeds)
                        .filter(|seed| *seed as usize % threads == t)
                        .map(|seed| {
                            (
                                seed,
                                run_once(
                                    scene,
                                    space,
                                    &fixture.start,
                                    goals,
                                    fineness,
                                    algorithm,
                                    fixture.budget,
                                    seed,
                                ),
                            )
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for handle in handles {
            for (seed, run) in handle.join().unwrap() {
                runs[seed as usize] = Some(run);
            }
        }
    });
    runs.into_iter().map(Option::unwrap).collect()
}

struct Row {
    planner: &'static str,
    success: usize,
    seeds: usize,
    checks_median: f64,
    checks_max: u64,
    cost_median: f64,
    cost_best: f64,
    best_of_4_cv: f64,
    invalid: usize,
    repeat_ok: bool,
    millis_median: f64,
}

fn ik_probe(args: &[String]) {
    let fixture = read_fixture(Path::new(&args[0]));
    let v: Vec<f64> = args[1..7].iter().map(|w| w.parse().unwrap()).collect();
    let pose = DPose3::from_parts(
        DVec3::new(v[0], v[1], v[2]),
        quat_from_rpy(v[3].to_radians(), v[4].to_radians(), v[5].to_radians()),
    );
    let scene = build_scene(&fixture);
    let ik = NumericIk::new(scene.chain(), IkOptions::default());
    for q in ik.solve_many(&scene.flange_target(&pose), Some(&fixture.start), 32) {
        let rounded: Vec<String> = q.iter().map(|x| format!("{x:.4}")).collect();
        println!("{}  -> {:?}", rounded.join(" "), scene.state_problem(&q));
    }
    let tip = scene.tip_in_world(&fixture.start).translation;
    println!(
        "start TCP: ({:.4}, {:.4}, {:.4}) problem {:?}",
        tip.x,
        tip.y,
        tip.z,
        scene.state_problem(&fixture.start)
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--speed") {
        // Time single-state checks at random configurations.
        let fixture = read_fixture(Path::new(&args[1]));
        let scene = build_scene(&fixture);
        let space = space_of(&scene);
        let checker = scene.checker(1.0);
        let mut rng = Rng::new(1);
        let states: Vec<Vec<f64>> = (0..20_000).map(|_| space.sample(&mut rng)).collect();
        let clock = Instant::now();
        let valid = states.iter().filter(|q| checker.state_valid(q)).count();
        let micros = clock.elapsed().as_secs_f64() * 1e6 / states.len() as f64;
        println!("{}: {micros:.1} µs per state check, {valid} of {} valid", fixture.id, states.len());
        return;
    }
    if args.first().map(String::as_str) == Some("--ik") {
        ik_probe(&args[1..]);
        return;
    }
    let mut seeds = 100_u64;
    let mut out: Option<PathBuf> = None;
    let mut dir = root().join("fixtures/scenes");
    let mut only: Option<String> = None;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--seeds" => seeds = it.next().and_then(|v| v.parse().ok()).expect("--seeds N"),
            "--out" => out = it.next().map(PathBuf::from),
            "--scenes" => dir = it.next().map(PathBuf::from).expect("--scenes DIR"),
            "--only" => only = it.next().cloned(),
            other => panic!("unknown argument `{other}`"),
        }
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "scene"))
        .collect();
    files.sort();

    let mut report = String::new();
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    writeln!(report, "Robot UR5e (collision meshes as convex hulls), {seeds} seeds per planner (0..{seeds}), single-threaded planner runs spread over {threads} threads. Each run: `gc_plan::plan` at the motion pipeline's checking step and planning margin → {SHORTCUTS} shortcut attempts → independent re-validation against the required padding. Budget = state checks (motion checks count the states they examine).\n").unwrap();
    let total = Instant::now();
    for file in &files {
        let fixture = read_fixture(file);
        if only.as_ref().is_some_and(|o| !o.eq_ignore_ascii_case(&fixture.id)) {
            continue;
        }
        let scene = build_scene(&fixture);
        let space = space_of(&scene);
        assert!(
            scene.state_problem(&fixture.start).is_none(),
            "{}: start is not valid: {:?}",
            fixture.id,
            scene.state_problem(&fixture.start)
        );
        let goals = goals_of(&scene, &fixture, &space);
        assert!(!goals.is_empty(), "{}: no valid goal", fixture.id);
        let fineness = fineness_of(&scene, &fixture.start, &goals)
            .expect("the start and a goal keep no planning margin");
        let straight = space.distance(&fixture.start, &goals[0]);
        let straight_free = scene.checker(fineness).motion_valid(&fixture.start, &goals[0]);
        eprintln!("{}: {} goals, straight {straight:.3} (free: {straight_free})", fixture.id, goals.len());

        let mut rows = Vec::new();
        for (algorithm, name) in ALGORITHMS {
            let clock = Instant::now();
            let runs = sweep(&scene, &space, &fixture, &goals, fineness, algorithm, seeds);
            let repeat_ok = (0..REPEATED_SEEDS.min(seeds)).all(|seed| {
                let again = run_once(
                    &scene,
                    &space,
                    &fixture.start,
                    &goals,
                    fineness,
                    algorithm,
                    fixture.budget,
                    seed,
                );
                let first = &runs[seed as usize];
                again.success == first.success
                    && again.checks == first.checks
                    && bits(&again.path) == bits(&first.path)
            });
            let mut costs: Vec<f64> = runs.iter().filter(|r| r.success).map(|r| r.cost).collect();
            let best_of_4: Vec<f64> = runs
                .chunks(4)
                .filter_map(|group| group.iter().filter(|r| r.success).map(|r| r.cost).min_by(f64::total_cmp))
                .collect();
            let mut checks: Vec<f64> = runs.iter().map(|r| r.checks as f64).collect();
            let mut millis: Vec<f64> = runs.iter().map(|r| r.millis).collect();
            let row = Row {
                planner: name,
                success: runs.iter().filter(|r| r.success && !r.invalid).count(),
                seeds: runs.len(),
                checks_median: median(&mut checks),
                checks_max: runs.iter().map(|r| r.checks).max().unwrap_or(0),
                cost_best: costs.iter().copied().fold(f64::INFINITY, f64::min),
                cost_median: median(&mut costs),
                best_of_4_cv: coefficient_of_variation(&best_of_4),
                invalid: runs.iter().filter(|r| r.invalid).count(),
                repeat_ok,
                millis_median: median(&mut millis),
            };
            eprintln!("  {name}: {}/{} in {:.1} s", row.success, row.seeds, clock.elapsed().as_secs_f64());
            rows.push(row);
        }
        let best = rows.iter().map(|r| r.cost_best).fold(f64::INFINITY, f64::min);
        writeln!(report, "## {} — {}\n", fixture.id, fixture.title).unwrap();
        writeln!(
            report,
            "Padding {:.0} mm, planning margin {:.1} mm (checking step × {fineness}), budget {} state checks, {} goal configuration(s){}; straight joint line to the nearest goal: {:.3} ({}).\n",
            fixture.padding * 1e3,
            scene.checker(fineness).margin() * 1e3,
            fixture.budget,
            goals.len(),
            if fixture.targets.is_empty() { String::new() } else { format!(" for {}", fixture.targets) },
            straight,
            if straight_free { "free" } else { "blocked" },
        )
        .unwrap();
        writeln!(report, "| Planner | Q3 success | checks median / max | cost median | median / best found | Q5 best-of-4 CV | Q1 invalid | Q9 repeat | time median (ms) |").unwrap();
        writeln!(report, "|---|---|---|---|---|---|---|---|---|").unwrap();
        for r in &rows {
            writeln!(
                report,
                "| {} | {}/{} ({:.0} %) | {:.0} / {} | {:.3} | {:.2} | {:.1} % | {} | {} | {:.1} |",
                r.planner,
                r.success,
                r.seeds,
                100.0 * r.success as f64 / r.seeds as f64,
                r.checks_median,
                r.checks_max,
                r.cost_median,
                r.cost_median / best,
                100.0 * r.best_of_4_cv,
                r.invalid,
                if r.repeat_ok { "identical" } else { "**differs**" },
                r.millis_median,
            )
            .unwrap();
        }
        writeln!(report, "\nBest cost found by any run: {best:.3}.\n").unwrap();
    }
    writeln!(report, "Total wall time: {:.0} s.", total.elapsed().as_secs_f64()).unwrap();
    match out {
        Some(path) => {
            let mut previous = std::fs::read_to_string(&path).unwrap_or_default();
            if let Some(at) = previous.find("<!-- results -->") {
                previous.truncate(at + "<!-- results -->".len());
                previous.push_str("\n\n");
            } else {
                previous = String::from("# Planning quality report\n\n<!-- results -->\n\n");
            }
            previous.push_str(&report);
            std::fs::write(&path, previous).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            eprintln!("wrote {}", path.display());
        }
        None => print!("{report}"),
    }
}
