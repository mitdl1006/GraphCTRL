//! The constraint system: parameters, constraints, substitution, solving and diagnosis.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::constraint::{Constraint, P2, P3};
use crate::problem::{End, Input, Problem, Row, max_abs};
use crate::rows::{DEGENERATE, RowKind};
use crate::{ConstraintId, ParamId, tol};

/// How a solve ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Every constraint is satisfied.
    Solved,
    /// The constraints contradict each other; the report names the conflicting set.
    Inconsistent,
    /// The iteration budget ran out without a verdict.
    NotConverged,
    /// A solution exists but on another branch than the drawn one (see [`SolveReport::flipped`]).
    Flipped,
}

/// A constraint, or one of its rows, that takes part in a dependency.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Blame {
    /// Row `.1` of constraint `.0`.
    Row(ConstraintId, u8),
    /// A constraint that fixed or merged the parameters involved.
    Merged(ConstraintId),
}

/// What kind of dependency a row has on others.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DepKind {
    /// The row follows from the others: removing its constraint changes nothing.
    Redundant,
    /// The row contradicts the others.
    Conflicting,
    /// The row follows from the others, but its constraint has further rows that do remove freedom.
    PartiallyRedundant,
    /// The row is dependent only at this exact position (an infinitesimal coincidence).
    SingularHere,
}

/// A row that depends on other rows or on substituted parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct Dependency {
    /// The dependent row: constraint and row number.
    pub row: (ConstraintId, u8),
    /// What it depends on.
    pub with: Vec<Blame>,
    /// How.
    pub kind: DepKind,
}

/// A branch change that made the solver reject a result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flip {
    /// A radius would become zero or negative.
    Radius(ParamId),
    /// An arc's sweep would jump (for example from 10° to 350°) or collapse.
    ArcSweep(ConstraintId),
    /// A tangent junction would turn from smooth into a cusp or the reverse.
    EndTangent(ConstraintId),
}

/// The result of a solve.
#[derive(Clone, Debug, PartialEq)]
pub struct SolveReport {
    /// How the solve ended. Unless it is [`Status::Solved`], parameter values are unchanged.
    pub status: Status,
    /// Remaining degrees of freedom.
    pub dof: usize,
    /// Redundant and conflicting rows.
    pub dependencies: Vec<Dependency>,
    /// Constraints that refer to collapsed geometry (a zero-length line, a point at a circle's centre).
    pub degenerate: Vec<ConstraintId>,
    /// Constraints that are close to being dependent.
    pub ill_conditioned: Vec<ConstraintId>,
    /// Why the status is [`Status::Flipped`].
    pub flipped: Vec<Flip>,
    /// True when parameter values changed.
    pub changed: bool,
    mobile: Vec<bool>,
}

impl SolveReport {
    /// True when the parameter can still move without violating a constraint.
    pub fn is_mobile(&self, param: ParamId) -> bool {
        self.mobile.get(param.0 as usize).copied().unwrap_or(false)
    }

    /// The dependencies whose dependent row belongs to `constraint`.
    pub fn dependencies_of(&self, constraint: ConstraintId) -> impl Iterator<Item = &Dependency> {
        self.dependencies.iter().filter(move |d| d.row.0 == constraint)
    }
}

/// The result of [`System::try_add`].
#[derive(Clone, Debug, PartialEq)]
pub enum AddOutcome {
    /// The constraint removes freedom and was added; the system is solved.
    Added(SolveReport),
    /// The constraint was added although it removes no freedom to first order: it does so only at this
    /// exact position (for example a point pinned at the top of a circle it lies on).
    AddedSingular(SolveReport),
    /// The constraint follows from existing ones; nothing was changed.
    Redundant {
        /// What it follows from.
        with: Vec<Blame>,
    },
    /// The constraint contradicts existing ones; nothing was changed.
    Conflicting {
        /// What it contradicts.
        with: Vec<Blame>,
    },
    /// The system could not be solved with the constraint for another reason; nothing was changed.
    Failed(Status),
}

/// How a parameter is read after substitution.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Ref {
    Fixed(f64),
    Unknown { index: usize, offset: f64 },
}

pub(crate) struct SpecRow {
    pub constraint: ConstraintId,
    pub index: u8,
    pub kind: RowKind,
    pub params: Vec<ParamId>,
}

/// The system after substitution.
pub(crate) struct Compiled {
    pub refs: Vec<Ref>,
    /// The representative (lowest) parameter of each unknown.
    pub reps: Vec<ParamId>,
    pub rows: Vec<SpecRow>,
    pub merge_deps: Vec<Dependency>,
    pub conflict: bool,
    /// Per parameter: the class it was merged into (the ground class for fixed parameters).
    class: Vec<usize>,
    merger: Merger,
    /// Groups of (unknowns, rows) that share no unknown with each other. Rows that read no unknown at all
    /// form a group without unknowns.
    pub components: Vec<(Vec<usize>, Vec<usize>)>,
}

impl Compiled {
    /// The constraints whose substitutions tie the given parameters together (or, for a single
    /// parameter, to its fixed value). This is why a row that reads only these has no gradient.
    fn substitution_causes(&self, params: &[ParamId]) -> BTreeSet<ConstraintId> {
        let mut causes = BTreeSet::new();
        let node = |p: &ParamId| p.0 as usize;
        if let [only] = params {
            if self.class[node(only)] == self.merger.ground() {
                causes.extend(self.merger.path(node(only), self.merger.ground()));
            }
            return causes;
        }
        for (i, a) in params.iter().enumerate() {
            for b in &params[i + 1..] {
                if a != b && self.class[node(a)] == self.class[node(b)] {
                    causes.extend(self.merger.path(node(a), node(b)));
                }
            }
        }
        causes
    }
}

/// Weighted union-find: `value(node) = value(parent) + offset`. The last node is "ground" (value 0).
struct Merger {
    parent: Vec<usize>,
    offset: Vec<f64>,
    edges: Vec<(usize, usize, ConstraintId)>,
}

impl Merger {
    fn new(params: usize) -> Self {
        Self { parent: (0..=params).collect(), offset: vec![0.0; params + 1], edges: Vec::new() }
    }

    fn ground(&self) -> usize {
        self.parent.len() - 1
    }

    /// Root of `node` and `value(node) - value(root)`.
    fn find(&mut self, node: usize) -> (usize, f64) {
        let mut path = Vec::new();
        let mut at = node;
        while self.parent[at] != at {
            path.push(at);
            at = self.parent[at];
        }
        let root = at;
        let mut total = 0.0;
        for step in path.into_iter().rev() {
            total += self.offset[step];
            self.parent[step] = root;
            self.offset[step] = total;
        }
        (root, if node == root { 0.0 } else { self.offset[node] })
    }

    /// Imposes `value(b) = value(a) + delta`. When both are already in one class, returns how far the
    /// existing relation is from the requested one.
    fn union(&mut self, a: usize, b: usize, delta: f64, cause: ConstraintId) -> Option<f64> {
        let (root_a, offset_a) = self.find(a);
        let (root_b, offset_b) = self.find(b);
        if root_a == root_b {
            return Some((offset_b - offset_a) - delta);
        }
        // value(root_b) = value(root_a) + shift
        let shift = offset_a + delta - offset_b;
        let ground = self.ground();
        if root_b == ground || (root_a != ground && root_b < root_a) {
            self.parent[root_a] = root_b;
            self.offset[root_a] = -shift;
        } else {
            self.parent[root_b] = root_a;
            self.offset[root_b] = shift;
        }
        self.edges.push((a, b, cause));
        None
    }

    /// The constraints along a chain of merges from `a` to `b` (breadth-first, so a shortest chain).
    fn path(&self, a: usize, b: usize) -> Vec<ConstraintId> {
        let mut previous: BTreeMap<usize, (usize, ConstraintId)> = BTreeMap::new();
        let mut queue = VecDeque::from([a]);
        while let Some(node) = queue.pop_front() {
            if node == b {
                break;
            }
            for (x, y, cause) in &self.edges {
                let next = if *x == node {
                    *y
                } else if *y == node {
                    *x
                } else {
                    continue;
                };
                if next != a && !previous.contains_key(&next) {
                    previous.insert(next, (node, *cause));
                    queue.push_back(next);
                }
            }
        }
        let mut causes = BTreeSet::new();
        let mut at = b;
        while let Some((before, cause)) = previous.get(&at) {
            causes.insert(*cause);
            at = *before;
        }
        causes.into_iter().collect()
    }
}

/// SplitMix64 mapped to `[-1, 1]`: the deterministic "random" direction of nudges.
fn jitter(seed: u32) -> f64 {
    let mut z = u64::from(seed).wrapping_add(1).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^= z >> 31;
    (z >> 11) as f64 / (1_u64 << 52) as f64 - 1.0
}

/// A set of parameters and the constraints between them.
#[derive(Clone, Debug, Default)]
pub struct System {
    values: Vec<f64>,
    /// Parameters that must stay positive (radii).
    positive: Vec<bool>,
    /// Registered points, for estimating the sketch size.
    points: Vec<Vec<ParamId>>,
    constraints: BTreeMap<ConstraintId, Constraint>,
    /// During [`System::try_add`]: the constraint whose effect is being measured.
    candidate: Option<ConstraintId>,
}

impl System {
    /// An empty system.
    pub fn new() -> Self {
        Self::default()
    }

    fn add_param(&mut self, value: f64, positive: bool) -> ParamId {
        let id = ParamId(u32::try_from(self.values.len()).unwrap_or(u32::MAX));
        self.values.push(value);
        self.positive.push(positive);
        id
    }

    /// Adds a 2D point.
    pub fn add_point2(&mut self, x: f64, y: f64) -> P2 {
        let point = [self.add_param(x, false), self.add_param(y, false)];
        self.points.push(point.to_vec());
        point
    }

    /// Adds a 3D point.
    pub fn add_point3(&mut self, x: f64, y: f64, z: f64) -> P3 {
        let point = [self.add_param(x, false), self.add_param(y, false), self.add_param(z, false)];
        self.points.push(point.to_vec());
        point
    }

    /// Adds a radius. The solver refuses results in which it is not positive.
    pub fn add_radius(&mut self, value: f64) -> ParamId {
        self.add_param(value, true)
    }

    /// Current value of a parameter (0 for an unknown id).
    pub fn value(&self, param: ParamId) -> f64 {
        self.values.get(param.0 as usize).copied().unwrap_or(0.0)
    }

    /// Current position of a 2D point.
    pub fn point2(&self, point: P2) -> [f64; 2] {
        point.map(|p| self.value(p))
    }

    /// Current position of a 3D point.
    pub fn point3(&self, point: P3) -> [f64; 3] {
        point.map(|p| self.value(p))
    }

    /// Overwrites a parameter value, e.g. where the user drew a point or where a reference point moved to.
    pub fn set_value(&mut self, param: ParamId, value: f64) {
        if let Some(slot) = self.values.get_mut(param.0 as usize) {
            *slot = value;
        }
    }

    /// Number of parameters.
    pub fn param_count(&self) -> usize {
        self.values.len()
    }

    /// Adds or replaces a constraint without checking it. Call [`System::solve`] afterwards.
    pub fn add(&mut self, id: ConstraintId, constraint: Constraint) -> Option<Constraint> {
        self.constraints.insert(id, constraint)
    }

    /// Removes a constraint.
    pub fn remove(&mut self, id: ConstraintId) -> Option<Constraint> {
        self.constraints.remove(&id)
    }

    /// The constraint stored under `id`.
    pub fn constraint(&self, id: ConstraintId) -> Option<&Constraint> {
        self.constraints.get(&id)
    }

    /// All constraints, oldest first.
    pub fn constraints(&self) -> impl Iterator<Item = (ConstraintId, &Constraint)> {
        self.constraints.iter().map(|(id, c)| (*id, c))
    }

    /// Sketch size over the given points and constraints: the larger of the points' bounding-box diagonal
    /// and the largest length involved, never below a millimetre.
    fn size_of(
        &self,
        points: &BTreeSet<usize>,
        constraints: &BTreeSet<ConstraintId>,
        radii: &[ParamId],
    ) -> f64 {
        let mut low = [f64::INFINITY; 3];
        let mut high = [f64::NEG_INFINITY; 3];
        for point in points.iter().filter_map(|i| self.points.get(*i)) {
            for (axis, param) in point.iter().enumerate() {
                let value = self.value(*param);
                low[axis] = low[axis].min(value);
                high[axis] = high[axis].max(value);
            }
        }
        let diagonal =
            low.iter().zip(&high).filter(|(l, h)| l <= h).map(|(l, h)| (h - l) * (h - l)).sum::<f64>().sqrt();
        let lengths = constraints
            .iter()
            .filter_map(|id| self.constraints.get(id).and_then(Constraint::length_value))
            .chain(radii.iter().map(|r| self.value(*r)))
            .fold(0.0_f64, |acc, v| acc.max(v.abs()));
        diagonal.max(lengths).max(tol::MIN_SIZE)
    }

    fn whole_size(&self) -> f64 {
        let radii: Vec<ParamId> =
            (0..self.values.len()).filter(|i| self.positive[*i]).map(|i| ParamId(i as u32)).collect();
        self.size_of(&(0..self.points.len()).collect(), &self.constraints.keys().copied().collect(), &radii)
    }

    /// Runs substitution and groups what is left into independent components.
    pub(crate) fn compile(&self) -> Compiled {
        let count = self.values.len();
        let get = |p: ParamId| self.value(p);
        let size = self.whole_size();
        let mut merger = Merger::new(count);
        let ground = merger.ground();
        let mut rows = Vec::new();
        let mut merge_deps = Vec::new();
        let mut conflict = false;
        let ordered = self
            .constraints
            .iter()
            .filter(|(id, _)| Some(**id) != self.candidate)
            .chain(self.candidate.and_then(|id| self.constraints.get_key_value(&id)));
        for (id, constraint) in ordered {
            let lowered = if Some(*id) == self.candidate {
                constraint.lower_explicit(&get)
            } else {
                constraint.lower(&get)
            };
            let relations = lowered
                .merges
                .iter()
                .map(|(a, b, offset)| (a.0 as usize, b.0 as usize, *offset))
                .chain(lowered.fixes.iter().map(|(param, value)| (ground, param.0 as usize, *value)));
            let mut index = 0_u8;
            for (a, b, offset) in relations {
                if a.max(b) > count || a.min(b) >= count {
                    continue; // a parameter id this system does not have
                }
                if let Some(discrepancy) = merger.union(a, b, offset, *id) {
                    let conflicting = discrepancy.abs() > tol::RESIDUAL * size;
                    conflict |= conflicting;
                    merge_deps.push(Dependency {
                        row: (*id, index),
                        with: merger.path(a, b).into_iter().filter(|c| c != id).map(Blame::Merged).collect(),
                        kind: if conflicting { DepKind::Conflicting } else { DepKind::Redundant },
                    });
                }
                index = index.saturating_add(1);
            }
            for (kind, params) in lowered.rows {
                if params.iter().all(|p| (p.0 as usize) < count) {
                    rows.push(SpecRow { constraint: *id, index, kind, params });
                }
                index = index.saturating_add(1);
            }
        }

        let mut refs = Vec::with_capacity(count);
        let mut reps = Vec::new();
        let mut index_of_root: BTreeMap<usize, usize> = BTreeMap::new();
        let mut class = Vec::with_capacity(count);
        for param in 0..count {
            let (root, offset) = merger.find(param);
            class.push(root);
            refs.push(if root == ground {
                Ref::Fixed(offset)
            } else {
                let index = *index_of_root.entry(root).or_insert_with(|| {
                    reps.push(ParamId(root as u32));
                    reps.len() - 1
                });
                Ref::Unknown { index, offset }
            });
        }

        // Components: unknowns connected through rows.
        let mut group: Vec<usize> = (0..reps.len()).collect();
        fn root_of(group: &mut [usize], mut i: usize) -> usize {
            while group[i] != i {
                group[i] = group[group[i]];
                i = group[i];
            }
            i
        }
        let unknowns_of = |row: &SpecRow| -> Vec<usize> {
            row.params
                .iter()
                .filter_map(|p| match refs[p.0 as usize] {
                    Ref::Unknown { index, .. } => Some(index),
                    Ref::Fixed(_) => None,
                })
                .collect()
        };
        for row in &rows {
            let unknowns = unknowns_of(row);
            for pair in unknowns.windows(2) {
                let (a, b) = (root_of(&mut group, pair[0]), root_of(&mut group, pair[1]));
                group[a.max(b)] = a.min(b);
            }
        }
        let mut by_root: BTreeMap<Option<usize>, (Vec<usize>, Vec<usize>)> = BTreeMap::new();
        for (i, row) in rows.iter().enumerate() {
            let key = unknowns_of(row).first().map(|u| root_of(&mut group, *u));
            by_root.entry(key).or_default().1.push(i);
        }
        for unknown in 0..reps.len() {
            let key = Some(root_of(&mut group, unknown));
            if let Some(entry) = by_root.get_mut(&key) {
                entry.0.push(unknown);
            }
        }
        Compiled {
            refs,
            reps,
            rows,
            merge_deps,
            conflict,
            class,
            merger,
            components: by_root.into_values().collect(),
        }
    }

    /// Builds the solvable problem for one component and its starting point.
    pub(crate) fn problem(
        &self,
        compiled: &Compiled,
        unknowns: &[usize],
        rows: &[usize],
    ) -> (Problem, Vec<f64>) {
        let local: BTreeMap<usize, usize> = unknowns.iter().enumerate().map(|(l, g)| (*g, l)).collect();
        let mut points = BTreeSet::new();
        let mut constraints = BTreeSet::new();
        let mut radii = Vec::new();
        let point_of: BTreeMap<ParamId, usize> = self
            .points
            .iter()
            .enumerate()
            .flat_map(|(i, point)| point.iter().map(move |p| (*p, i)))
            .collect();
        let mut problem_rows = Vec::with_capacity(rows.len());
        for spec in rows.iter().map(|i| &compiled.rows[*i]) {
            constraints.insert(spec.constraint);
            let inputs = spec
                .params
                .iter()
                .map(|param| {
                    if let Some(point) = point_of.get(param) {
                        points.insert(*point);
                    } else if self.positive[param.0 as usize] {
                        radii.push(*param);
                    }
                    match compiled.refs[param.0 as usize] {
                        Ref::Fixed(value) => Input::Const(value),
                        Ref::Unknown { index, offset } => match local.get(&index) {
                            Some(local) => Input::Unknown { local: *local, offset },
                            // Not part of this problem: held at its current value.
                            None => Input::Const(self.value(compiled.reps[index]) + offset),
                        },
                    }
                })
                .collect();
            problem_rows.push(Row { constraint: spec.constraint, kind: spec.kind.clone(), inputs });
        }
        let problem = Problem {
            rows: problem_rows,
            n: unknowns.len(),
            size: self.size_of(&points, &constraints, &radii),
            jitter: unknowns.iter().map(|u| jitter(compiled.reps[*u].0)).collect(),
        };
        let start = unknowns.iter().map(|u| self.value(compiled.reps[*u])).collect();
        (problem, start)
    }

    /// Sweep of every arc (counter-clockwise from start to end, in `[0, 2*pi)`), where it is defined.
    fn sweeps(&self, values: &[f64]) -> BTreeMap<ConstraintId, Option<f64>> {
        let at = |p: ParamId| values.get(p.0 as usize).copied().unwrap_or(0.0);
        self.constraints
            .iter()
            .filter_map(|(id, constraint)| match constraint {
                Constraint::Arc { round, start, end } => {
                    let (sx, sy) = (at(start[0]) - at(round.c[0]), at(start[1]) - at(round.c[1]));
                    let (ex, ey) = (at(end[0]) - at(round.c[0]), at(end[1]) - at(round.c[1]));
                    let defined = sx * sx + sy * sy > 0.0 && ex * ex + ey * ey > 0.0;
                    let sweep =
                        libm::atan2(sx * ey - sy * ex, sx * ex + sy * ey).rem_euclid(std::f64::consts::TAU);
                    Some((*id, defined.then_some(sweep)))
                }
                _ => None,
            })
            .collect()
    }

    /// The branch changes between the current values and `after`.
    pub(crate) fn flips(&self, after: &[f64], size: f64) -> Vec<Flip> {
        let mut flips = Vec::new();
        for (i, value) in after.iter().enumerate() {
            if self.positive[i] && *value <= DEGENERATE * size {
                flips.push(Flip::Radius(ParamId(i as u32)));
            }
        }
        let before = self.sweeps(&self.values);
        for (id, sweep) in self.sweeps(after) {
            let jumped = match (before.get(&id).copied().flatten(), sweep) {
                (Some(old), Some(new)) => {
                    (new - old).abs() >= std::f64::consts::PI
                        || !(1e-9..=std::f64::consts::TAU - 1e-9).contains(&new)
                }
                _ => false,
            };
            if jumped {
                flips.push(Flip::ArcSweep(id));
            }
        }
        let at = |p: ParamId| after.get(p.0 as usize).copied().unwrap_or(0.0);
        for (id, constraint) in &self.constraints {
            let Constraint::EndTangent { line, round, at: shared, smooth, leaving } = constraint else {
                continue;
            };
            let point = |p: &P2| [at(p[0]), at(p[1])];
            let (a, b, s, c) = (point(&line.a), point(&line.b), point(shared), point(&round.c));
            let far = |p: [f64; 2]| (p[0] - s[0]).powi(2) + (p[1] - s[1]).powi(2);
            // Direction of the line leaving the shared point, and of the arc continued beyond that end.
            let away = if far(a) >= far(b) { [a[0] - b[0], a[1] - b[1]] } else { [b[0] - a[0], b[1] - a[1]] };
            let onward = [leaving * (s[1] - c[1]), -leaving * (s[0] - c[0])];
            let along = away[0] * onward[0] + away[1] * onward[1];
            if along != 0.0 && (along > 0.0) != *smooth {
                flips.push(Flip::EndTangent(*id));
            }
        }
        flips
    }

    /// Solves the system.
    ///
    /// On [`Status::Solved`] parameter values are updated (and left untouched, bit for bit, if they
    /// already satisfied everything). On any other status they are unchanged.
    pub fn solve(&mut self) -> SolveReport {
        let compiled = self.compile();
        let mut report = SolveReport {
            status: Status::Solved,
            dof: 0,
            dependencies: compiled.merge_deps.clone(),
            degenerate: Vec::new(),
            ill_conditioned: Vec::new(),
            flipped: Vec::new(),
            changed: false,
            mobile: vec![false; self.values.len()],
        };
        if compiled.conflict {
            report.status = Status::Inconsistent;
            return report;
        }
        let mut x: Vec<f64> = compiled.reps.iter().map(|p| self.value(*p)).collect();
        let mut mobility = vec![1.0; compiled.reps.len()]; // unknowns without rows are free
        let mut in_component = vec![false; compiled.reps.len()];
        let mut degenerate = BTreeSet::new();
        let mut ill_conditioned = BTreeSet::new();
        for (unknowns, rows) in &compiled.components {
            let (problem, mut local) = self.problem(&compiled, unknowns, rows);
            let mut end = problem.solve(&mut local);
            if end == End::Stationary
                && let Some(mut nudged) = problem.nudged(&local, tol::NUDGE * problem.size)
                && problem.solve(&mut nudged) == End::Converged
            {
                local = nudged;
                end = End::Converged;
            }
            match end {
                End::Converged => {}
                End::Stationary => report.status = Status::Inconsistent,
                End::Budget if report.status == Status::Solved => report.status = Status::NotConverged,
                End::Budget => {}
            }
            let diagnosis = problem.diagnose(&local);
            report.dof += problem.n - diagnosis.rank;
            for crate::problem::DependentRow { row, terms, combination, zero_gradient } in
                &diagnosis.dependent
            {
                let spec = &compiled.rows[rows[*row]];
                let limit = if *zero_gradient { tol::RESIDUAL } else { tol::COMBO } * problem.size;
                let mut with: BTreeSet<Blame> = terms
                    .iter()
                    .map(|(other, _)| &compiled.rows[rows[*other]])
                    .map(|other| Blame::Row(other.constraint, other.index))
                    .collect();
                if *zero_gradient {
                    with.extend(
                        compiled
                            .substitution_causes(&spec.params)
                            .into_iter()
                            .filter(|c| *c != spec.constraint)
                            .map(Blame::Merged),
                    );
                }
                let has_independent_row = diagnosis
                    .independent
                    .iter()
                    .any(|other| compiled.rows[rows[*other]].constraint == spec.constraint);
                let kind = if combination.abs() > limit {
                    DepKind::Conflicting
                } else if has_independent_row {
                    DepKind::PartiallyRedundant
                } else {
                    DepKind::Redundant
                };
                report.dependencies.push(Dependency {
                    row: (spec.constraint, spec.index),
                    with: with.into_iter().collect(),
                    kind,
                });
            }
            if end == End::Stationary {
                // At a stall the gradient J^T r vanishes although r does not: the rows that still carry
                // residual are exactly the ones that cannot be satisfied together. The newest is blamed.
                let residuals = problem.residuals(&local);
                let worst = max_abs(&residuals);
                let mut involved: Vec<&SpecRow> = residuals
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| r.abs() > 1e-3 * worst)
                    .map(|(row, _)| &compiled.rows[rows[row]])
                    .collect();
                if let Some(subject) = involved.pop() {
                    let known = report.dependencies.iter().any(|d| {
                        d.row == (subject.constraint, subject.index) && d.kind == DepKind::Conflicting
                    });
                    if !known {
                        report.dependencies.push(Dependency {
                            row: (subject.constraint, subject.index),
                            with: involved.iter().map(|row| Blame::Row(row.constraint, row.index)).collect(),
                            kind: DepKind::Conflicting,
                        });
                    }
                }
            }
            degenerate.extend(diagnosis.degenerate);
            ill_conditioned.extend(diagnosis.ill_conditioned);
            for (local_index, unknown) in unknowns.iter().enumerate() {
                x[*unknown] = local[local_index];
                mobility[*unknown] = diagnosis.mobility[local_index];
                in_component[*unknown] = true;
            }
        }
        report.dof += in_component.iter().filter(|inside| !**inside).count();
        report.degenerate = degenerate.into_iter().collect();
        report.ill_conditioned = ill_conditioned.into_iter().collect();
        let mut after = Vec::with_capacity(self.values.len());
        for (param, reference) in compiled.refs.iter().enumerate() {
            match *reference {
                Ref::Fixed(value) => after.push(value),
                Ref::Unknown { index, offset } => {
                    report.mobile[param] = mobility[index] > tol::MOBILITY;
                    after.push(x[index] + offset);
                }
            }
        }
        if report.status == Status::Solved {
            report.flipped = self.flips(&after, self.whole_size());
            if report.flipped.is_empty() {
                report.changed = after.iter().zip(&self.values).any(|(a, b)| a.to_bits() != b.to_bits());
                self.values = after;
            } else {
                report.status = Status::Flipped;
            }
        }
        report
    }

    /// The largest residual at the current values, relative to the tolerance: at most 1 when every
    /// constraint is satisfied. Does not change anything.
    pub fn violation(&self) -> f64 {
        let compiled = self.compile();
        if compiled.conflict {
            return f64::INFINITY;
        }
        let mut worst = 0.0_f64;
        // A parameter that disagrees with its substitution counts as a violation too.
        let size = self.whole_size();
        for (param, reference) in compiled.refs.iter().enumerate() {
            let expected = match *reference {
                Ref::Fixed(value) => value,
                Ref::Unknown { index, offset } => self.value(compiled.reps[index]) + offset,
            };
            worst = worst.max((self.values[param] - expected).abs() / (tol::RESIDUAL * size));
        }
        for (unknowns, rows) in &compiled.components {
            let (problem, start) = self.problem(&compiled, unknowns, rows);
            worst = worst.max(max_abs(&problem.residuals(&start)) / (tol::RESIDUAL * problem.size));
        }
        worst
    }

    /// Adds a constraint if it is consistent with the existing ones and removes freedom.
    ///
    /// The system must be solved. Unless the outcome is `Added` or `AddedSingular`, nothing changes.
    pub fn try_add(&mut self, id: ConstraintId, constraint: Constraint) -> AddOutcome {
        let before = self.clone().solve();
        if before.status != Status::Solved {
            return AddOutcome::Failed(before.status);
        }
        let mut trial = self.clone();
        trial.constraints.insert(id, constraint.clone());
        trial.candidate = Some(id);
        let report = trial.solve();
        let blame = |report: &SolveReport, kinds: &[DepKind]| -> Vec<Blame> {
            let own: BTreeSet<Blame> = report
                .dependencies_of(id)
                .filter(|d| kinds.contains(&d.kind))
                .flat_map(|d| d.with.iter().copied())
                .collect();
            own.into_iter().collect()
        };
        match report.status {
            Status::Solved => {}
            Status::Inconsistent => {
                let mut with = blame(&report, &[DepKind::Conflicting]);
                if with.is_empty() {
                    // The contradiction surfaced on other rows: report everything that conflicts.
                    let all: BTreeSet<Blame> = report
                        .dependencies
                        .iter()
                        .filter(|d| d.kind == DepKind::Conflicting)
                        .flat_map(|d| d.with.iter().copied().chain([Blame::Row(d.row.0, d.row.1)]))
                        .filter(|b| !matches!(b, Blame::Row(c, _) | Blame::Merged(c) if *c == id))
                        .collect();
                    with = all.into_iter().collect();
                }
                return AddOutcome::Conflicting { with };
            }
            other => return AddOutcome::Failed(other),
        }
        let singular = report.dof >= before.dof;
        if singular && !trial.gains_rank_nearby(id) {
            return AddOutcome::Redundant {
                with: blame(&report, &[DepKind::Redundant, DepKind::PartiallyRedundant]),
            };
        }
        let mut committed = self.clone();
        committed.constraints.insert(id, constraint);
        let mut final_report = committed.solve();
        if final_report.status != Status::Solved {
            return AddOutcome::Failed(final_report.status);
        }
        *self = committed;
        if singular {
            for dependency in final_report.dependencies.iter_mut().filter(|d| d.row.0 == id) {
                dependency.kind = DepKind::SingularHere;
            }
            AddOutcome::AddedSingular(final_report)
        } else {
            AddOutcome::Added(final_report)
        }
    }

    /// The witness test: step a little inside the null space, project back onto the constraints and see
    /// whether the candidate's rows have become independent there.
    fn gains_rank_nearby(&self, candidate: ConstraintId) -> bool {
        let compiled = self.compile();
        compiled.components.iter().any(|(unknowns, rows)| {
            if !rows.iter().any(|row| compiled.rows[*row].constraint == candidate) {
                return false;
            }
            let (problem, start) = self.problem(&compiled, unknowns, rows);
            let rank = problem.diagnose(&start).rank;
            let Some(mut moved) = problem.nudged(&start, tol::NUDGE * problem.size) else { return false };
            problem.solve(&mut moved) == End::Converged && problem.diagnose(&moved).rank > rank
        })
    }

    /// Changes the value of a driving dimension and re-solves.
    ///
    /// The value is moved from old to new in steps that are halved whenever a step fails or would flip a
    /// branch, so the geometry follows continuously. If even a 1/1024 step fails, everything is restored
    /// and the failing report is returned.
    pub fn set_dimension(&mut self, id: ConstraintId, value: f64) -> SolveReport {
        let Some(old) = self.constraints.get(&id).and_then(Constraint::value) else {
            return self.solve();
        };
        let backup = self.clone();
        let (mut done, mut step) = (0.0_f64, 1.0_f64);
        loop {
            let fraction = (done + step).min(1.0);
            let target = if fraction >= 1.0 { value } else { old + (value - old) * fraction };
            if let Some(constraint) = self.constraints.get_mut(&id) {
                constraint.set_value(target);
            }
            let report = self.solve();
            if report.status == Status::Solved {
                if fraction >= 1.0 {
                    return report;
                }
                done = fraction;
            } else {
                step /= 2.0;
                if step < 1.0 / 1024.0 {
                    *self = backup;
                    return report;
                }
            }
        }
    }
}
