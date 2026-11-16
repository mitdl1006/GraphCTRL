//! Interactive dragging.
//!
//! While the user drags, every frame solves a small optimisation in strict priority order:
//! 1. all constraints hold;
//! 2. the grabbed geometry is as close to the cursor as the constraints allow;
//! 3. everything else stays as close as possible to where it was **when the drag began**.
//!
//! Anchoring the third objective at the start of the drag, not at the previous frame, makes the result a
//! function of the cursor position alone: moving the cursor in a loop brings the sketch back exactly,
//! however many frames the loop took.

use gc_linalg::{Matrix, PivotPolicy, lstsq_min_norm, qr_pivoted};

use crate::constraint::{P2, Round};
use crate::problem::{Input, Problem, max_abs};
use crate::rows::{DEGENERATE, RowKind};
use crate::system::{Flip, Ref, System};
use crate::{ParamId, tol};

/// Projection steps per cursor sub-step while dragging: bounds the work of a frame.
const FRAME_STEPS: usize = 6;
/// Projection steps allowed when the drag is committed and the result must be exact.
const SETTLE_STEPS: usize = 2000;

/// What the user took hold of.
#[derive(Clone, Debug, PartialEq)]
pub enum Grab {
    /// A point: it follows the cursor.
    Point(P2),
    /// The body of a line or arc: its end points keep their offset from the cursor.
    Body(Vec<P2>),
    /// The rim of a circle or arc: the radius (or whatever else is free) changes so that the rim passes
    /// through the cursor.
    Rim(Round),
}

/// Why a drag did not start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragRefused {
    /// Nothing of what was grabbed can move: it is fully constrained.
    NothingMobile,
    /// The system has contradicting constraints and must be repaired first.
    Unsolved,
}

/// What one cursor update did.
#[derive(Clone, Debug, PartialEq)]
pub enum FrameOutcome {
    /// The grabbed geometry is at the cursor.
    Moved,
    /// The constraints do not allow reaching the cursor; the nearest reachable position is shown.
    Clamped,
    /// Following the cursor would change a branch (see the flips); the sketch stays at the last good frame.
    Flipped(Vec<Flip>),
    /// The frame could not be solved; the sketch stays at the last good frame.
    NotConverged,
}

#[derive(Clone, Debug)]
enum Target {
    /// Unknowns that follow the cursor: `(local unknown, offset, axis, value at drag start)`.
    Coordinates { entries: Vec<(usize, f64, usize, f64)>, relative: bool },
    /// A rim that passes through the cursor.
    Rim { centre: [Input; 2], radius: Input },
}

/// A drag in progress. Owns the system; [`Dragging::commit`] or [`Dragging::cancel`] gives it back.
#[derive(Clone, Debug)]
pub struct Dragging {
    system: System,
    start_values: Vec<f64>,
    problem: Problem,
    /// Per parameter: its unknown in `problem` and offset, when it takes part in the drag.
    local_of: Vec<Option<(usize, f64)>>,
    target: Target,
    x_start: Vec<f64>,
    x: Vec<f64>,
    cursor_begin: [f64; 2],
    cursor: [f64; 2],
    max_substep: f64,
}

impl System {
    /// Starts dragging `grab` with the cursor at `cursor` (sketch coordinates). The system must be solved.
    ///
    /// Grabbed coordinates that cannot move are ignored; the drag is refused only when none can.
    pub fn begin_drag(self, grab: &Grab, cursor: [f64; 2]) -> Result<Dragging, (Self, DragRefused)> {
        let compiled = self.compile();
        if compiled.conflict {
            return Err((self, DragRefused::Unsolved));
        }
        let grabbed: Vec<ParamId> = match grab {
            Grab::Point(p) => p.to_vec(),
            Grab::Body(points) => points.iter().flatten().copied().collect(),
            Grab::Rim(round) => vec![round.c[0], round.c[1], round.r],
        };
        let unknown_of = |param: ParamId| match compiled.refs.get(param.0 as usize) {
            Some(Ref::Unknown { index, offset }) => Some((*index, *offset)),
            _ => None,
        };
        // The problem: every component that contains a grabbed unknown, plus grabbed unknowns without rows.
        let wanted: Vec<usize> =
            grabbed.iter().filter_map(|p| unknown_of(*p)).map(|(index, _)| index).collect();
        let mut unknowns: Vec<usize> = wanted.clone();
        let mut rows = Vec::new();
        for (component_unknowns, component_rows) in &compiled.components {
            if component_unknowns.iter().any(|u| wanted.contains(u)) {
                unknowns.extend(component_unknowns);
                rows.extend(component_rows);
            }
        }
        unknowns.sort_unstable();
        unknowns.dedup();
        rows.sort_unstable();
        let (problem, x) = self.problem(&compiled, &unknowns, &rows);
        let local = |global: usize| unknowns.binary_search(&global).ok();
        let mobility = problem.diagnose(&x).mobility;
        let mobile = |param: ParamId| {
            unknown_of(param)
                .and_then(|(g, offset)| local(g).map(|l| (l, offset)))
                .filter(|(l, _)| mobility[*l] > tol::MOBILITY)
        };
        let input = |param: ParamId| match compiled.refs[param.0 as usize] {
            Ref::Fixed(value) => Input::Const(value),
            Ref::Unknown { index, offset } => match local(index) {
                Some(local) => Input::Unknown { local, offset },
                None => Input::Const(self.value(param)),
            },
        };
        let target = match grab {
            Grab::Point(_) | Grab::Body(_) => {
                // A point takes part with both coordinates as soon as one of them can move: mobility is
                // only a first-order statement, and a point on a circle moves in both over a finite drag.
                let located =
                    |p: ParamId| unknown_of(p).and_then(|(g, offset)| local(g).map(|l| (l, offset)));
                let entries: Vec<(usize, f64, usize, f64)> = grabbed
                    .chunks(2)
                    .filter(|point| point.iter().any(|p| mobile(*p).is_some()))
                    .flat_map(|point| point.iter().enumerate())
                    .filter_map(|(axis, p)| located(*p).map(|(l, offset)| (l, offset, axis, self.value(*p))))
                    .collect();
                if entries.is_empty() {
                    return Err((self, DragRefused::NothingMobile));
                }
                Target::Coordinates { entries, relative: matches!(grab, Grab::Body(_)) }
            }
            Grab::Rim(round) => {
                if grabbed.iter().all(|p| mobile(*p).is_none()) {
                    return Err((self, DragRefused::NothingMobile));
                }
                Target::Rim { centre: [input(round.c[0]), input(round.c[1])], radius: input(round.r) }
            }
        };
        let local_of = (0..self.param_count())
            .map(|param| {
                unknown_of(ParamId(param as u32)).and_then(|(g, offset)| local(g).map(|l| (l, offset)))
            })
            .collect();
        let max_substep =
            smallest_length(&problem, &x).map_or(f64::INFINITY, |l| (l / 4.0).max(tol::NUDGE * problem.size));
        Ok(Dragging {
            start_values: (0..self.param_count()).map(|p| self.value(ParamId(p as u32))).collect(),
            system: self,
            problem,
            local_of,
            target,
            x_start: x.clone(),
            x,
            cursor_begin: cursor,
            cursor,
            max_substep,
        })
    }
}

/// The shortest segment or radius the rows of a problem refer to.
fn smallest_length(problem: &Problem, x: &[f64]) -> Option<f64> {
    let mut smallest: Option<f64> = None;
    for row in &problem.rows {
        let value = |slot: usize| match row.inputs[slot] {
            Input::Const(v) => v,
            Input::Unknown { local, offset } => x[local] + offset,
        };
        let segment = |a: usize, b: usize| {
            ((value(b) - value(a)).powi(2) + (value(b + 1) - value(a + 1)).powi(2)).sqrt()
        };
        let lengths: Vec<f64> = match row.kind {
            RowKind::Distance { dim: 2, .. } => vec![segment(0, 2)],
            RowKind::PointLine { .. } => vec![segment(2, 4)],
            RowKind::Cross { .. } | RowKind::Dot { .. } | RowKind::EqualLen | RowKind::Angle { .. } => {
                vec![segment(0, 2), segment(4, 6)]
            }
            RowKind::MidOnLine { .. } => vec![segment(0, 2), segment(4, 6)],
            RowKind::LineCircleTangent { .. } => vec![segment(0, 2), value(6)],
            RowKind::PointCircle => vec![value(4)],
            RowKind::CircleCircle { .. } => vec![value(2), value(5)],
            _ => Vec::new(),
        };
        for length in lengths.into_iter().filter(|l| *l > DEGENERATE * problem.size) {
            smallest = Some(smallest.map_or(length, |s| s.min(length)));
        }
    }
    smallest
}

impl Dragging {
    /// The system at the last good frame, for drawing the preview.
    pub fn system(&self) -> &System {
        &self.system
    }

    /// Ends the drag and keeps the last good frame.
    ///
    /// During the drag each frame does a bounded amount of work, so the "stay close to the start"
    /// objective may not be fully settled yet. It is settled here, which is what makes the committed
    /// result independent of the path the cursor took.
    pub fn commit(mut self) -> System {
        let mut x = self.x.clone();
        if let Some((_, true)) = self.project(&mut x, self.cursor, SETTLE_STEPS) {
            let values = self.values_at(&x);
            if self.system.flips(&values, self.problem.size).is_empty() {
                for (param, value) in values.iter().enumerate() {
                    self.system.set_value(ParamId(param as u32), *value);
                }
            }
        }
        self.system
    }

    /// Ends the drag and restores the state from before it began, bit for bit.
    pub fn cancel(mut self) -> System {
        for (param, value) in self.start_values.iter().enumerate() {
            self.system.set_value(ParamId(param as u32), *value);
        }
        self.system
    }

    /// Parameter values that correspond to the unknowns `x`.
    fn values_at(&self, x: &[f64]) -> Vec<f64> {
        self.local_of
            .iter()
            .enumerate()
            .map(|(param, local)| match local {
                Some((l, offset)) => x[*l] + offset,
                None => self.system.value(ParamId(param as u32)),
            })
            .collect()
    }

    /// Target mismatch `target - T(x)` and its Jacobian `dT/dx` for the cursor position `cursor`.
    fn target_error(&self, x: &[f64], cursor: [f64; 2]) -> (Vec<f64>, Matrix) {
        let n = self.problem.n;
        match &self.target {
            Target::Coordinates { entries, relative } => {
                let mut jacobian = Matrix::zeros(entries.len(), n);
                let mut error = Vec::with_capacity(entries.len());
                for (k, (local, offset, axis, start)) in entries.iter().enumerate() {
                    let goal = if *relative {
                        start + cursor[*axis] - self.cursor_begin[*axis]
                    } else {
                        cursor[*axis]
                    };
                    error.push(goal - (x[*local] + offset));
                    jacobian[(k, *local)] = 1.0;
                }
                (error, jacobian)
            }
            Target::Rim { centre, radius } => {
                let value = |input: &Input| match *input {
                    Input::Const(v) => v,
                    Input::Unknown { local, offset } => x[local] + offset,
                };
                let (dx, dy) = (cursor[0] - value(&centre[0]), cursor[1] - value(&centre[1]));
                let reach = (dx * dx + dy * dy).sqrt();
                let mut jacobian = Matrix::zeros(1, n);
                // T = radius - |cursor - centre|, wanted to be 0.
                if let Input::Unknown { local, .. } = radius {
                    jacobian[(0, *local)] += 1.0;
                }
                if reach > 0.0 {
                    for (input, d) in centre.iter().zip([dx, dy]) {
                        if let Input::Unknown { local, .. } = input {
                            jacobian[(0, *local)] += d / reach;
                        }
                    }
                }
                (vec![reach - value(radius)], jacobian)
            }
        }
    }

    /// One step of the prioritised projection for the cursor position `cursor`.
    ///
    /// Returns the new point, the length of the step taken and whether the cursor is reached, or `None`
    /// when no step keeps the constraints in reach.
    fn step(&self, x: &[f64], cursor: [f64; 2]) -> Option<(Vec<f64>, f64, bool)> {
        let problem = &self.problem;
        let n = problem.n;
        let tolerance = tol::RESIDUAL * problem.size;
        let (r, j, _) = problem.linearize(x);
        let factor = qr_pivoted(&j.transpose(), PivotPolicy::Global, tol::RANK);
        let rank = factor.rank();
        // 1. Back onto the constraints by the shortest correction: with J^T P = Q R, the minimum-norm
        //    solution of J d = -r is d = Q1 R11^-T (-P^T r).
        let rhs: Vec<f64> = factor.perm()[..rank].iter().map(|row| -r[*row]).collect();
        let mut correction = factor.solve_r11_transposed(&rhs)?;
        correction.resize(n, 0.0);
        factor.apply_q(&mut correction);
        let corrected: Vec<f64> = x.iter().zip(&correction).map(|(a, b)| a + b).collect();
        // 2. Towards the cursor, inside the null space. Long moves are cut to the sub-step length so that
        //    an unreachable cursor pulls the geometry steadily instead of throwing it about.
        let null = factor.q_columns(rank..n);
        let (error, target_jacobian) = self.target_error(&corrected, cursor);
        let reduced = target_jacobian.mul(&null);
        let limit = |v: Vec<f64>| -> Vec<f64> {
            let length = max_abs(&v);
            if length > self.max_substep {
                v.iter().map(|c| c * self.max_substep / length).collect()
            } else {
                v
            }
        };
        let towards = limit(null.mul_vec(&lstsq_min_norm(&reduced, &error, tol::RANK)));
        // 3. Back towards the drag-start state, inside what the first two leave free.
        let reduced_factor = qr_pivoted(&reduced.transpose(), PivotPolicy::Global, tol::RANK);
        let free = null.mul(&reduced_factor.q_columns(reduced_factor.rank()..null.cols()));
        let lag: Vec<f64> = (0..n).map(|i| self.x_start[i] - (corrected[i] + towards[i])).collect();
        let home = limit(free.mul_vec(&free.tr_mul_vec(&lag)));
        let step: Vec<f64> = (0..n).map(|i| correction[i] + towards[i] + home[i]).collect();

        let before = max_abs(&r);
        let mut scale = 1.0;
        loop {
            let trial: Vec<f64> = x.iter().zip(&step).map(|(a, s)| a + scale * s).collect();
            let residual = max_abs(&problem.residuals(&trial));
            // Motion along the null space leaves the constraints only at second order; allow that much,
            // the next correction removes it.
            let allowed = before.max(tolerance).max(0.25 * scale * max_abs(&step));
            if residual.is_finite() && residual <= allowed {
                let (error, _) = self.target_error(&trial, cursor);
                return Some((trial, scale * max_abs(&step), max_abs(&error) <= 1e-7 * problem.size));
            }
            scale *= 0.5;
            if scale < 1.0 / 1024.0 {
                return None;
            }
        }
    }

    /// Moves `x` towards the constrained optimum for one cursor position with at most `budget` steps,
    /// then puts it exactly onto the constraints. Returns `(cursor reached, optimum reached)`, or `None`
    /// when the constraints cannot be restored.
    fn project(&self, x: &mut Vec<f64>, cursor: [f64; 2], budget: usize) -> Option<(bool, bool)> {
        let size = self.problem.size;
        let mut reached = false;
        let mut settled = false;
        for _ in 0..budget {
            let (moved, length, hit) = self.step(x, cursor)?;
            *x = moved;
            reached = hit;
            if length <= 1e-10 * size {
                settled = true;
                break;
            }
        }
        if !settled {
            // The last step may have left the constraints at second order: put the point back onto them
            // and see where that leaves the grabbed geometry.
            if self.problem.solve(x) != crate::problem::End::Converged {
                return None;
            }
            let (error, _) = self.target_error(x, cursor);
            reached = max_abs(&error) <= 1e-7 * size;
        }
        Some((reached, settled))
    }

    /// Moves the cursor to `cursor` and updates the sketch. Long cursor moves are followed in small steps
    /// so the geometry cannot jump to another branch on the way.
    pub fn update(&mut self, cursor: [f64; 2]) -> FrameOutcome {
        // Geometry may have shrunk since the drag began (a radius dragged small): steps follow its size.
        self.max_substep = smallest_length(&self.problem, &self.x)
            .map_or(f64::INFINITY, |l| (l / 4.0).max(tol::NUDGE * self.problem.size));
        let travel = ((cursor[0] - self.cursor[0]).powi(2) + (cursor[1] - self.cursor[1]).powi(2)).sqrt();
        let steps_wanted = (travel / self.max_substep).ceil();
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "clamped to 1..=512 first"
        )]
        let steps = if steps_wanted.is_finite() { steps_wanted.clamp(1.0, 512.0) as usize } else { 512 };
        let mut x = self.x.clone();
        let mut shadow = self.system.clone();
        let mut reached = true;
        for k in 1..=steps {
            let t = k as f64 / steps as f64;
            let sub_cursor = [
                self.cursor[0] + (cursor[0] - self.cursor[0]) * t,
                self.cursor[1] + (cursor[1] - self.cursor[1]) * t,
            ];
            let Some((hit, _)) = self.project(&mut x, sub_cursor, FRAME_STEPS) else {
                return FrameOutcome::NotConverged;
            };
            reached = hit;
            let values = self.values_at(&x);
            let flips = shadow.flips(&values, self.problem.size);
            if !flips.is_empty() {
                return FrameOutcome::Flipped(flips);
            }
            for (param, value) in values.iter().enumerate() {
                shadow.set_value(ParamId(param as u32), *value);
            }
        }
        self.x = x;
        self.system = shadow;
        self.cursor = cursor;
        if reached { FrameOutcome::Moved } else { FrameOutcome::Clamped }
    }
}
