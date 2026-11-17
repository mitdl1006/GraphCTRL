//! The planners: RRT-Connect, RRT, RRT*, Informed RRT* and PRM.

use std::collections::BTreeSet;

use crate::rng::Rng;
use crate::simplify::path_cost;
use crate::space::Space;
use crate::{Control, Validity};

/// Which planner to run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Algorithm {
    /// Two trees grown towards each other. Fast; the path is not optimised.
    RrtConnect,
    /// One tree from the start with a bias towards the goals.
    Rrt,
    /// A tree that keeps rewiring itself towards shorter paths until the budget is used up.
    RrtStar,
    /// RRT* that, once it has a path, samples only where a shorter one could be.
    InformedRrtStar,
    /// A roadmap of random states searched with A*.
    Prm,
}

/// Planner settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    /// The most any single dimension may change in one tree extension.
    pub range: f64,
    /// RRT: probability of steering towards a goal instead of a random sample.
    pub goal_bias: f64,
    /// Upper bound on planner iterations.
    pub max_iterations: u64,
    /// Upper bound on single-state validity checks (motion checks count the states they examine).
    pub max_state_checks: u64,
    /// PRM: number of roadmap states.
    pub prm_samples: usize,
    /// PRM: neighbours each state is connected to.
    pub prm_neighbours: usize,
    /// Sampling is first confined to the bounding box of start and goals grown by this much.
    pub window_margin: f64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            range: 0.5,
            goal_bias: 0.05,
            max_iterations: 20_000,
            max_state_checks: 200_000,
            prm_samples: 400,
            prm_neighbours: 10,
            window_margin: std::f64::consts::PI,
        }
    }
}

/// A planning query.
#[derive(Clone, Copy, Debug)]
pub struct Request<'a> {
    /// The space to search.
    pub space: &'a Space,
    /// Where the path starts.
    pub start: &'a [f64],
    /// Where it may end: reaching any one goal is success.
    pub goals: &'a [Vec<f64>],
    /// The planner to use.
    pub algorithm: Algorithm,
    /// Its settings.
    pub options: Options,
    /// Seed of the random numbers.
    pub seed: u64,
}

/// Counters of a planning run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Planner iterations.
    pub iterations: u64,
    /// Single-state validity checks, including those inside motion checks.
    pub state_checks: u64,
    /// Motion validity checks.
    pub motion_checks: u64,
    /// States stored in the search structure.
    pub nodes: u64,
}

/// A path that was found.
#[derive(Clone, Debug, PartialEq)]
pub struct Solution {
    /// The states of the path, start first. Consecutive states are joined by valid straight motions.
    pub path: Vec<Vec<f64>>,
    /// Which goal the path ends at.
    pub goal_index: usize,
    /// Weighted length of the path.
    pub cost: f64,
    /// Counters of the run.
    pub stats: Stats,
}

/// Why no path was returned.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum Failure {
    /// The request does not make sense.
    #[error("invalid planning request: {0}")]
    InvalidRequest(String),
    /// The start state is out of bounds or not valid.
    #[error("the start state is not valid")]
    StartInvalid,
    /// Every goal is out of bounds or not valid.
    #[error("no goal state is valid")]
    NoValidGoal,
    /// The budget ran out.
    #[error("no path found within the budget")]
    NoPath {
        /// Counters of the run.
        stats: Stats,
        /// How close the search came to a goal.
        nearest_goal_distance: f64,
    },
    /// The caller cancelled.
    #[error("planning was cancelled")]
    Cancelled,
}

/// Counts validity checks and knows when to stop.
struct Monitor<'a> {
    validity: &'a dyn Validity,
    control: &'a dyn Control,
    stats: Stats,
    options: Options,
}

impl Monitor<'_> {
    fn state(&mut self, q: &[f64]) -> bool {
        self.stats.state_checks += 1;
        self.validity.state_valid(q)
    }

    fn motion(&mut self, a: &[f64], b: &[f64]) -> bool {
        self.stats.motion_checks += 1;
        self.stats.state_checks += self.validity.motion_cost(a, b);
        self.validity.motion_valid(a, b)
    }

    /// Starts an iteration; false when the budget is used up or the caller cancelled.
    fn proceed(&mut self) -> bool {
        if self.stats.iterations >= self.options.max_iterations
            || self.stats.state_checks >= self.options.max_state_checks
            || self.control.cancelled()
        {
            return false;
        }
        self.stats.iterations += 1;
        true
    }
}

/// A search tree (or forest, when it has several roots) with its states stored contiguously.
struct Tree {
    dof: usize,
    points: Vec<f64>,
    parent: Vec<usize>,
    cost: Vec<f64>,
    /// For a tree grown from the goals: which goal each node descends from.
    origin: Vec<usize>,
}

impl Tree {
    fn new(dof: usize) -> Self {
        Self { dof, points: Vec::new(), parent: Vec::new(), cost: Vec::new(), origin: Vec::new() }
    }

    fn len(&self) -> usize {
        self.parent.len()
    }

    fn get(&self, node: usize) -> &[f64] {
        &self.points[node * self.dof..(node + 1) * self.dof]
    }

    fn add_root(&mut self, q: &[f64], origin: usize) -> usize {
        self.points.extend_from_slice(q);
        self.parent.push(self.len());
        self.cost.push(0.0);
        self.origin.push(origin);
        self.len() - 1
    }

    fn add(&mut self, q: &[f64], parent: usize, step_cost: f64) -> usize {
        self.points.extend_from_slice(q);
        self.parent.push(parent);
        self.cost.push(self.cost[parent] + step_cost);
        self.origin.push(self.origin[parent]);
        self.len() - 1
    }

    /// The node closest to `q` (lowest index on ties).
    fn nearest(&self, space: &Space, q: &[f64]) -> usize {
        let mut best = (0, f64::INFINITY);
        for node in 0..self.len() {
            let d = space.distance(self.get(node), q);
            if d < best.1 {
                best = (node, d);
            }
        }
        best.0
    }

    /// The states from the root down to `node`.
    fn branch(&self, mut node: usize) -> Vec<Vec<f64>> {
        let mut states = vec![self.get(node).to_vec()];
        while self.parent[node] != node {
            node = self.parent[node];
            states.push(self.get(node).to_vec());
        }
        states.reverse();
        states
    }

    fn closest_distance(&self, space: &Space, targets: &[Vec<f64>]) -> f64 {
        (0..self.len())
            .flat_map(|node| targets.iter().map(move |t| (node, t)))
            .map(|(node, t)| space.distance(self.get(node), t))
            .fold(f64::INFINITY, f64::min)
    }
}

/// True when `to` can be reached from `from` in one extension step.
fn within_range(from: &[f64], to: &[f64], range: f64) -> bool {
    from.iter().zip(to).all(|(a, b)| (b - a).abs() <= range)
}

struct Search<'a> {
    space: &'a Space,
    window: Space,
    start: &'a [f64],
    /// The valid goals with their index in the request.
    goals: Vec<(usize, &'a [f64])>,
    options: Options,
}

type Found = (Vec<Vec<f64>>, usize);

impl Search<'_> {
    fn goal_states(&self) -> Vec<Vec<f64>> {
        self.goals.iter().map(|(_, g)| g.to_vec()).collect()
    }

    fn rrt_connect(&self, monitor: &mut Monitor<'_>, rng: &mut Rng) -> Result<Found, f64> {
        let dof = self.space.dof();
        let mut from_start = Tree::new(dof);
        from_start.add_root(self.start, 0);
        let mut from_goals = Tree::new(dof);
        for (index, goal) in &self.goals {
            from_goals.add_root(goal, *index);
        }
        let mut growing_start = true;
        while monitor.proceed() {
            let (grow, other) = if growing_start {
                (&mut from_start, &mut from_goals)
            } else {
                (&mut from_goals, &mut from_start)
            };
            growing_start = !growing_start;
            let sample = self.window.sample(rng);
            let near = grow.nearest(self.space, &sample);
            let (new, _) = self.space.steer(grow.get(near), &sample, self.options.range);
            if !(monitor.state(&new) && monitor.motion(grow.get(near), &new)) {
                continue;
            }
            let added = grow.add(&new, near, self.space.distance(grow.get(near), &new));
            // Greedily extend the other tree towards the new state.
            let mut at = other.nearest(self.space, &new);
            loop {
                let (step, reached) = self.space.steer(other.get(at), &new, self.options.range);
                if !((reached || monitor.state(&step)) && monitor.motion(other.get(at), &step)) {
                    break;
                }
                if reached {
                    let (start_side, goal_side, goal_node) = if growing_start {
                        (other.branch(at), grow.branch(added), added)
                    } else {
                        (grow.branch(added), other.branch(at), at)
                    };
                    // `growing_start` was flipped above: when true, `grow` is the goal tree.
                    let goal_index =
                        if growing_start { grow.origin[goal_node] } else { other.origin[goal_node] };
                    let mut path = start_side;
                    path.extend(goal_side.into_iter().rev());
                    path.dedup();
                    monitor.stats.nodes = (from_start.len() + from_goals.len()) as u64;
                    return Ok((path, goal_index));
                }
                at = other.add(&step, at, self.space.distance(other.get(at), &step));
                if monitor.stats.state_checks >= self.options.max_state_checks {
                    break;
                }
            }
        }
        monitor.stats.nodes = (from_start.len() + from_goals.len()) as u64;
        Err(from_start.closest_distance(self.space, &self.goal_states()))
    }

    fn rrt(&self, monitor: &mut Monitor<'_>, rng: &mut Rng) -> Result<Found, f64> {
        let mut tree = Tree::new(self.space.dof());
        tree.add_root(self.start, 0);
        while monitor.proceed() {
            let target = if rng.unit() < self.options.goal_bias {
                self.goals[rng.below(self.goals.len())].1.to_vec()
            } else {
                self.window.sample(rng)
            };
            let near = tree.nearest(self.space, &target);
            let (new, _) = self.space.steer(tree.get(near), &target, self.options.range);
            if !(monitor.state(&new) && monitor.motion(tree.get(near), &new)) {
                continue;
            }
            let added = tree.add(&new, near, self.space.distance(tree.get(near), &new));
            for (index, goal) in &self.goals {
                if within_range(&new, goal, self.options.range) && monitor.motion(&new, goal) {
                    let mut path = tree.branch(added);
                    path.push(goal.to_vec());
                    path.dedup();
                    monitor.stats.nodes = tree.len() as u64;
                    return Ok((path, *index));
                }
            }
        }
        monitor.stats.nodes = tree.len() as u64;
        Err(tree.closest_distance(self.space, &self.goal_states()))
    }

    fn rrt_star(&self, monitor: &mut Monitor<'_>, rng: &mut Rng, informed: bool) -> Result<Found, f64> {
        let dof = self.space.dof();
        let mut tree = Tree::new(dof);
        tree.add_root(self.start, 0);
        let mut children: Vec<Vec<usize>> = vec![Vec::new()];
        // The best connection to a goal so far: (tree node, goal position in `self.goals`).
        let mut best: Option<(usize, usize)> = None;
        let total = |tree: &Tree, link: (usize, usize)| {
            tree.cost[link.0] + self.space.distance(tree.get(link.0), self.goals[link.1].1)
        };
        while monitor.proceed() {
            let mut sample = self.window.sample(rng);
            // Until a goal is connected, steer towards one now and then, as plain RRT does: uniform
            // samples alone rarely land within one step of a goal in six dimensions.
            if best.is_none() && rng.unit() < self.options.goal_bias {
                sample = self.goals[rng.below(self.goals.len())].1.to_vec();
            }
            if let (true, Some(link)) = (informed, best) {
                // Only states through which a shorter path could pass are worth sampling.
                let bound = total(&tree, link);
                let through = |x: &[f64]| {
                    self.space.distance(self.start, x)
                        + self
                            .goals
                            .iter()
                            .map(|(_, g)| self.space.distance(x, g))
                            .fold(f64::INFINITY, f64::min)
                };
                for _ in 0..64 {
                    if through(&sample) <= bound {
                        break;
                    }
                    sample = self.window.sample(rng);
                }
            }
            let near = tree.nearest(self.space, &sample);
            let (new, _) = self.space.steer(tree.get(near), &sample, self.options.range);
            if !monitor.state(&new) {
                continue;
            }
            // The k nearest nodes, with k growing logarithmically as the theory of RRT* asks.
            let n = tree.len() as f64;
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "small positive count"
            )]
            let k = (((std::f64::consts::E + std::f64::consts::E / dof as f64) * libm::log(n + 1.0)).ceil()
                as usize)
                .clamp(1, 40);
            let mut neighbours: Vec<(f64, usize)> =
                (0..tree.len()).map(|i| (self.space.distance(tree.get(i), &new), i)).collect();
            neighbours.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            neighbours.truncate(k);
            // Parent: the neighbour giving the cheapest valid way to the new state.
            let mut by_cost: Vec<(f64, usize, f64)> =
                neighbours.iter().map(|(d, i)| (tree.cost[*i] + d, *i, *d)).collect();
            by_cost.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            let Some((_, parent, step)) =
                by_cost.into_iter().find(|(_, i, _)| monitor.motion(tree.get(*i), &new))
            else {
                continue;
            };
            let added = tree.add(&new, parent, step);
            children.push(Vec::new());
            children[parent].push(added);
            // Rewire: neighbours that are cheaper to reach through the new state.
            for (d, neighbour) in &neighbours {
                let through_new = tree.cost[added] + d;
                if *neighbour == parent
                    || through_new >= tree.cost[*neighbour]
                    || !monitor.motion(&new, tree.get(*neighbour))
                {
                    continue;
                }
                let old_parent = tree.parent[*neighbour];
                children[old_parent].retain(|c| c != neighbour);
                children[added].push(*neighbour);
                tree.parent[*neighbour] = added;
                let saving = tree.cost[*neighbour] - through_new;
                let mut stack = vec![*neighbour];
                while let Some(node) = stack.pop() {
                    tree.cost[node] -= saving;
                    stack.extend(&children[node]);
                }
            }
            for (position, (_, goal)) in self.goals.iter().enumerate() {
                let link = (added, position);
                let better = best.is_none_or(|b| total(&tree, link) < total(&tree, b));
                if better && within_range(&new, goal, self.options.range) && monitor.motion(&new, goal) {
                    best = Some(link);
                }
            }
        }
        monitor.stats.nodes = tree.len() as u64;
        match best {
            Some((node, position)) => {
                let mut path = tree.branch(node);
                path.push(self.goals[position].1.to_vec());
                path.dedup();
                Ok((path, self.goals[position].0))
            }
            None => Err(tree.closest_distance(self.space, &self.goal_states())),
        }
    }

    fn prm(&self, monitor: &mut Monitor<'_>, rng: &mut Rng) -> Result<Found, f64> {
        // Node 0 is the start, nodes 1..=g are the goals, the rest are samples.
        let mut nodes: Vec<Vec<f64>> = vec![self.start.to_vec()];
        nodes.extend(self.goal_states());
        while nodes.len() < self.options.prm_samples + 1 + self.goals.len() && monitor.proceed() {
            let sample = self.window.sample(rng);
            if monitor.state(&sample) {
                nodes.push(sample);
            }
        }
        monitor.stats.nodes = nodes.len() as u64;
        let mut edges: Vec<Vec<(usize, f64)>> = vec![Vec::new(); nodes.len()];
        let mut tried: BTreeSet<(usize, usize)> = BTreeSet::new();
        for a in 0..nodes.len() {
            let mut near: Vec<(f64, usize)> = (0..nodes.len())
                .filter(|b| *b != a)
                .map(|b| (self.space.distance(&nodes[a], &nodes[b]), b))
                .collect();
            near.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
            for (d, b) in near.into_iter().take(self.options.prm_neighbours) {
                if monitor.stats.state_checks >= self.options.max_state_checks || monitor.control.cancelled()
                {
                    break;
                }
                if tried.insert((a.min(b), a.max(b))) && monitor.motion(&nodes[a], &nodes[b]) {
                    edges[a].push((b, d));
                    edges[b].push((a, d));
                }
            }
        }
        // Dijkstra from the start (the graph is small; a heuristic would gain little).
        let mut cost = vec![f64::INFINITY; nodes.len()];
        let mut previous = vec![usize::MAX; nodes.len()];
        let mut done = vec![false; nodes.len()];
        cost[0] = 0.0;
        loop {
            let next = (0..nodes.len())
                .filter(|i| !done[*i] && cost[*i].is_finite())
                .min_by(|a, b| cost[*a].total_cmp(&cost[*b]).then(a.cmp(b)));
            let Some(at) = next else { break };
            done[at] = true;
            for (to, d) in &edges[at] {
                if cost[at] + d < cost[*to] {
                    cost[*to] = cost[at] + d;
                    previous[*to] = at;
                }
            }
        }
        let reached = (1..=self.goals.len())
            .filter(|g| cost[*g].is_finite())
            .min_by(|a, b| cost[*a].total_cmp(&cost[*b]).then(a.cmp(b)));
        match reached {
            Some(goal_node) => {
                let mut path = Vec::new();
                let mut at = goal_node;
                while at != usize::MAX {
                    path.push(nodes[at].clone());
                    at = previous[at];
                }
                path.reverse();
                Ok((path, self.goals[goal_node - 1].0))
            }
            None => {
                let connected = (0..nodes.len()).filter(|i| cost[*i].is_finite());
                let closest = connected
                    .flat_map(|i| self.goals.iter().map(move |(_, g)| (i, *g)))
                    .map(|(i, g)| self.space.distance(&nodes[i], g))
                    .fold(f64::INFINITY, f64::min);
                Err(closest)
            }
        }
    }

    fn run(&self, algorithm: Algorithm, monitor: &mut Monitor<'_>, rng: &mut Rng) -> Result<Found, f64> {
        match algorithm {
            Algorithm::RrtConnect => self.rrt_connect(monitor, rng),
            Algorithm::Rrt => self.rrt(monitor, rng),
            Algorithm::RrtStar => self.rrt_star(monitor, rng, false),
            Algorithm::InformedRrtStar => self.rrt_star(monitor, rng, true),
            Algorithm::Prm => self.prm(monitor, rng),
        }
    }
}

/// Plans a path from the start to any of the goals.
pub fn plan(
    request: &Request<'_>,
    validity: &dyn Validity,
    control: &dyn Control,
) -> Result<Solution, Failure> {
    let (space, options) = (request.space, request.options);
    let dof = space.dof();
    let finite = |q: &[f64]| q.len() == dof && q.iter().all(|v| v.is_finite());
    if !space.is_consistent() {
        return Err(Failure::InvalidRequest("the space is not consistent".to_owned()));
    }
    if !finite(request.start) || request.goals.is_empty() || !request.goals.iter().all(|g| finite(g)) {
        return Err(Failure::InvalidRequest(
            "start and goals must be finite states of the space's dimension".to_owned(),
        ));
    }
    if !(options.range > 0.0 && options.range.is_finite()) {
        return Err(Failure::InvalidRequest("the extension range must be positive".to_owned()));
    }
    let mut monitor = Monitor { validity, control, stats: Stats::default(), options };
    if !(space.contains(request.start) && monitor.state(request.start)) {
        return Err(Failure::StartInvalid);
    }
    let goals: Vec<(usize, &[f64])> = request
        .goals
        .iter()
        .enumerate()
        .filter(|(_, g)| space.contains(g) && monitor.state(g))
        .map(|(i, g)| (i, g.as_slice()))
        .collect();
    if goals.is_empty() {
        return Err(Failure::NoValidGoal);
    }
    let around = std::iter::once(request.start).chain(goals.iter().map(|(_, g)| *g));
    let window = space.window(around, options.window_margin);
    let mut rng = Rng::new(request.seed);
    let whole_space_is_wider = window != *space;
    let mut search = Search { space, window, start: request.start, goals, options };
    if whole_space_is_wider {
        // The window gets half of the budget; the rest is kept for the whole space.
        monitor.options.max_iterations = options.max_iterations / 2;
        monitor.options.max_state_checks = options.max_state_checks / 2;
    }
    let mut outcome = search.run(request.algorithm, &mut monitor, &mut rng);
    monitor.options = options;
    if outcome.is_err() && whole_space_is_wider && !control.cancelled() {
        // The window was too tight (the way leads around something big): search the whole space once.
        search.window = space.clone();
        outcome = search.run(request.algorithm, &mut monitor, &mut rng);
    }
    match outcome {
        Ok((path, goal_index)) => {
            let cost = path_cost(space, &path);
            Ok(Solution { path, goal_index, cost, stats: monitor.stats })
        }
        Err(_) if control.cancelled() => Err(Failure::Cancelled),
        Err(nearest_goal_distance) => Err(Failure::NoPath { stats: monitor.stats, nearest_goal_distance }),
    }
}
