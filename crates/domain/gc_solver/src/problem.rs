//! One connected component in solvable form: unknowns, rows, Levenberg-Marquardt and diagnosis.

use std::collections::BTreeSet;

use gc_linalg::{Matrix, PivotPolicy, Qr, norm2, qr, qr_pivoted};

use crate::rows::{DEGENERATE, RowKind};
use crate::scalar::{Dual, MAX_VARS, Scalar};
use crate::{ConstraintId, tol};

/// How a row input gets its value.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Input {
    /// A substituted constant.
    Const(f64),
    /// Unknown number `local` of the problem plus a substitution offset.
    Unknown { local: usize, offset: f64 },
}

#[derive(Clone, Debug)]
pub(crate) struct Row {
    pub constraint: ConstraintId,
    pub kind: RowKind,
    pub inputs: Vec<Input>,
}

#[derive(Clone, Debug)]
pub(crate) struct Problem {
    pub rows: Vec<Row>,
    /// Number of unknowns.
    pub n: usize,
    /// Sketch size the tolerances are scaled by.
    pub size: f64,
    /// A deterministic pseudo-random number per unknown, in `[-1, 1]`, for nudges.
    pub jitter: Vec<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum End {
    Converged,
    /// A local minimum of the residual norm that is not a solution.
    Stationary,
    /// The iteration budget ran out.
    Budget,
}

/// A row whose gradient is a combination of the gradients of older rows.
pub(crate) struct DependentRow {
    pub row: usize,
    /// The rows it depends on, with their weights.
    pub terms: Vec<(usize, f64)>,
    /// Its residual minus the same combination of the other residuals: zero when merely redundant.
    pub combination: f64,
    /// True when the row depends on no unknown at all.
    pub zero_gradient: bool,
}

/// The result of diagnosing a problem at a point.
pub(crate) struct Diagnosis {
    pub rank: usize,
    pub dependent: Vec<DependentRow>,
    pub independent: BTreeSet<usize>,
    pub ill_conditioned: BTreeSet<ConstraintId>,
    pub degenerate: BTreeSet<ConstraintId>,
    /// Per unknown: how much of the null space it carries (0 = cannot move).
    pub mobility: Vec<f64>,
    /// Orthonormal basis of the null space of the Jacobian, one column per degree of freedom.
    pub null: Matrix,
}

pub(crate) fn max_abs(values: &[f64]) -> f64 {
    values.iter().fold(0.0_f64, |acc, v| acc.max(v.abs()))
}

impl Problem {
    fn inputs<S: Scalar>(row: &Row, x: &[f64], make: impl Fn(f64, usize) -> S) -> ([S; MAX_VARS], usize) {
        let mut values = [S::constant(0.0); MAX_VARS];
        for (slot, input) in row.inputs.iter().enumerate() {
            values[slot] = match *input {
                Input::Const(value) => S::constant(value),
                Input::Unknown { local, offset } => make(x[local] + offset, slot),
            };
        }
        (values, row.inputs.len())
    }

    /// Residuals at `x`.
    pub fn residuals(&self, x: &[f64]) -> Vec<f64> {
        let mut flag = false;
        self.rows
            .iter()
            .map(|row| {
                let (values, count) = Self::inputs(row, x, |value, _| value);
                row.kind.eval(&values[..count], self.size, &mut flag)
            })
            .collect()
    }

    /// Residuals, Jacobian (rows x unknowns) and the constraints that are degenerate at `x`.
    pub fn linearize(&self, x: &[f64]) -> (Vec<f64>, Matrix, BTreeSet<ConstraintId>) {
        let mut jacobian = Matrix::zeros(self.rows.len(), self.n);
        let mut residuals = Vec::with_capacity(self.rows.len());
        let mut degenerate = BTreeSet::new();
        for (i, row) in self.rows.iter().enumerate() {
            let (values, count) = Self::inputs(row, x, Dual::variable);
            let mut flag = false;
            let dual = row.kind.eval(&values[..count], self.size, &mut flag);
            let plain: Vec<f64> = values[..count].iter().map(|d| d.v).collect();
            if flag || frozen_scale_degenerate(&row.kind, &plain, self.size) {
                degenerate.insert(row.constraint);
            }
            residuals.push(dual.v);
            for (slot, input) in row.inputs.iter().enumerate() {
                if let Input::Unknown { local, .. } = input {
                    // Several inputs can be the same unknown after substitution; their derivatives add.
                    jacobian[(i, *local)] += dual.d[slot];
                }
            }
        }
        (residuals, jacobian, degenerate)
    }

    /// Levenberg-Marquardt on all rows with identity damping, so steps stay in the row space and
    /// unconstrained geometry does not drift. Takes no step when `x` already satisfies the rows.
    pub fn solve(&self, x: &mut Vec<f64>) -> End {
        const MAX_ACCEPTED: usize = 200;
        const MAX_TRIALS: usize = 600;
        let tolerance = tol::RESIDUAL * self.size;
        let mut r = self.residuals(x);
        if max_abs(&r) <= tolerance {
            return End::Converged;
        }
        let mut lambda = 1e-6_f64;
        let mut accepted = 0;
        for _ in 0..MAX_TRIALS {
            let (_, j, _) = self.linearize(x);
            let gradient = j.tr_mul_vec(&r);
            if norm2(&gradient) <= tol::STATIONARY * j.frobenius_norm() * norm2(&r) {
                return End::Stationary;
            }
            let damping = Matrix::from_fn(self.n, self.n, |a, b| if a == b { lambda.sqrt() } else { 0.0 });
            let mut rhs: Vec<f64> = r.iter().map(|v| -v).collect();
            rhs.resize(r.len() + self.n, 0.0);
            let step = qr(&Matrix::stack_rows(&j, &damping)).solve_least_squares(&rhs);
            let trial = step.as_ref().map(|step| {
                let moved: Vec<f64> = x.iter().zip(step).map(|(a, b)| a + b).collect();
                let residuals = self.residuals(&moved);
                (moved, residuals)
            });
            match (step, trial) {
                (Some(step), Some((moved, r_new)))
                    if r_new.iter().all(|v| v.is_finite())
                        && norm2(&r_new) < norm2(&r)
                        && !self.angle_jump(&r, &r_new) =>
                {
                    *x = moved;
                    r = r_new;
                    lambda = (lambda * 0.1).max(1e-12);
                    accepted += 1;
                    if max_abs(&r) <= tolerance {
                        return End::Converged;
                    }
                    if max_abs(&step) < tol::MIN_STEP * self.size {
                        return End::Stationary;
                    }
                    if accepted >= MAX_ACCEPTED {
                        return End::Budget;
                    }
                }
                _ => {
                    lambda *= 10.0;
                    if lambda > 1e12 {
                        return End::Stationary;
                    }
                }
            }
        }
        End::Budget
    }

    /// True when a step turned an angle dimension by more than a quarter turn: such a step may have
    /// jumped to another branch and is retried shorter.
    fn angle_jump(&self, before: &[f64], after: &[f64]) -> bool {
        self.rows.iter().zip(before.iter().zip(after)).any(|(row, (a, b))| match row.kind {
            RowKind::Angle { scale, .. } => (b - a).abs() > std::f64::consts::FRAC_PI_2 * scale,
            _ => false,
        })
    }

    /// Pivoted QR of the transposed Jacobian with one block per constraint, oldest first: older
    /// constraints become pivots, so the newest of a dependent set is the one reported.
    fn factor(&self, jacobian: &Matrix) -> Qr {
        let mut block_ends = Vec::new();
        for (i, row) in self.rows.iter().enumerate() {
            if i > 0 && self.rows[i - 1].constraint != row.constraint {
                block_ends.push(i);
            }
        }
        qr_pivoted(&jacobian.transpose(), PivotPolicy::Blocked { block_ends: &block_ends }, tol::RANK)
    }

    /// Rank, dependent rows, mobility and null space at `x`.
    pub fn diagnose(&self, x: &[f64]) -> Diagnosis {
        let (r, j, degenerate) = self.linearize(x);
        let factor = self.factor(&j);
        let rank = factor.rank();
        let perm = factor.perm();
        let transposed = j.transpose();
        let mut dependent = Vec::new();
        for position in rank..self.rows.len() {
            let row = perm[position];
            let zero_gradient = norm2(transposed.col(row)) <= tol::ZERO_GRADIENT;
            let weights = factor.solve_r11(&factor.r_column(position)).unwrap_or_default();
            let mut combination = r[row];
            let mut terms = Vec::new();
            for (pivot, weight) in weights.iter().enumerate() {
                combination -= weight * r[perm[pivot]];
                if weight.abs() > 1e-9 {
                    terms.push((perm[pivot], *weight));
                }
            }
            dependent.push(DependentRow { row, terms, combination, zero_gradient });
        }
        dependent.sort_by_key(|entry| entry.row);
        let diagonal = factor.r_diag();
        let ill_conditioned = diagonal
            .iter()
            .enumerate()
            .filter(|(_, pivot)| **pivot < tol::ILL_CONDITIONED * diagonal[0])
            .map(|(k, _)| self.rows[perm[k]].constraint)
            .collect();
        let null = factor.q_columns(rank..self.n);
        let mobility = (0..self.n)
            .map(|i| (0..null.cols()).map(|k| null[(i, k)] * null[(i, k)]).sum::<f64>().sqrt())
            .collect();
        Diagnosis {
            rank,
            dependent,
            independent: perm[..rank].iter().copied().collect(),
            ill_conditioned,
            degenerate,
            mobility,
            null,
        }
    }

    /// A point `distance` away from `x` in a deterministic pseudo-random direction inside the null space
    /// of the Jacobian. `None` when the null space is empty.
    pub fn nudged(&self, x: &[f64], distance: f64) -> Option<Vec<f64>> {
        let null = self.diagnose(x).null;
        let projected = null.mul_vec(&null.tr_mul_vec(&self.jitter));
        let length = norm2(&projected);
        (length > 1e-12).then(|| x.iter().zip(&projected).map(|(a, p)| a + distance * p / length).collect())
    }
}

/// True when a line whose initial length was frozen into the row has collapsed.
fn frozen_scale_degenerate(kind: &RowKind, v: &[f64], size: f64) -> bool {
    let short = |a: usize, b: usize| {
        let (dx, dy) = (v[b] - v[a], v[b + 1] - v[a + 1]);
        (dx * dx + dy * dy).sqrt() <= DEGENERATE * size
    };
    match kind {
        RowKind::PointLine { frozen: Some(_), .. } => short(2, 4),
        RowKind::Cross { .. } | RowKind::Dot { .. } => short(0, 2) || short(4, 6),
        RowKind::MidOnLine { .. } => short(4, 6),
        _ => false,
    }
}
