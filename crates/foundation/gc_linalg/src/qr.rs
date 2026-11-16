//! Householder QR, with and without column pivoting.

use std::ops::Range;

use crate::{Matrix, norm2};

/// Which column may become the next pivot of [`qr_pivoted`].
#[derive(Clone, Copy, Debug)]
pub enum PivotPolicy<'a> {
    /// The remaining column with the largest norm (ties: lowest original index).
    Global,
    /// Columns are grouped into consecutive blocks: block `b` ends before column `block_ends[b]`, and
    /// columns after the last end form a final block. Pivots are taken from the current block only, largest
    /// norm first, until none of its columns exceeds the tolerance; then the next block starts.
    ///
    /// Earlier blocks therefore win: a column is reported dependent only on columns of its own or earlier
    /// blocks. The constraint solver uses this to blame the newest constraints.
    Blocked {
        /// Exclusive end column of each block, ascending.
        block_ends: &'a [usize],
    },
}

/// A QR factorisation `A[:, perm] = Q R` in compact form.
#[derive(Clone, Debug)]
pub struct Qr {
    /// `R` on and above the diagonal; below it the Householder vectors (their leading 1 is implicit).
    packed: Matrix,
    tau: Vec<f64>,
    perm: Vec<usize>,
    rank: usize,
}

/// Turns column `k` into a Householder reflector and applies it to the columns right of it.
fn householder_step(a: &mut Matrix, k: usize) -> f64 {
    let (head, norm) = {
        let column = &a.col(k)[k..];
        (column[0], norm2(column))
    };
    if norm == 0.0 {
        return 0.0;
    }
    let beta = if head >= 0.0 { -norm } else { norm };
    let tau = (beta - head) / beta;
    let scale = 1.0 / (head - beta);
    {
        let column = &mut a.col_mut(k)[k..];
        column[0] = beta;
        for value in &mut column[1..] {
            *value *= scale;
        }
    }
    for j in k + 1..a.cols() {
        let (reflector, target) = a.col_pair_mut(k, j);
        reflect(&reflector[k + 1..], tau, &mut target[k..]);
    }
    tau
}

/// Applies `I - tau v vᵀ` to `target`, where `v = [1, tail...]`.
fn reflect(tail: &[f64], tau: f64, target: &mut [f64]) {
    let Some((first, rest)) = target.split_first_mut() else { return };
    let dot = *first + tail.iter().zip(rest.iter()).map(|(v, t)| v * t).sum::<f64>();
    let factor = tau * dot;
    *first -= factor;
    for (t, v) in rest.iter_mut().zip(tail) {
        *t -= factor * v;
    }
}

/// Unpivoted QR. Meant for matrices that have full column rank by construction, such as the
/// Levenberg-Marquardt stack `[J; sqrt(lambda) I]`.
pub fn qr(a: &Matrix) -> Qr {
    let mut packed = a.clone();
    let steps = a.rows().min(a.cols());
    let tau = (0..steps).map(|k| householder_step(&mut packed, k)).collect();
    Qr { packed, tau, perm: (0..a.cols()).collect(), rank: steps }
}

/// Rank-revealing QR with column pivoting.
///
/// Elimination stops (or, with [`PivotPolicy::Blocked`], moves to the next block) once no candidate column
/// has a remaining norm above `rel_tol * |R_11|`. The number of pivots taken is the numerical rank.
pub fn qr_pivoted(a: &Matrix, policy: PivotPolicy<'_>, rel_tol: f64) -> Qr {
    let (m, n) = (a.rows(), a.cols());
    let mut packed = a.clone();
    let mut perm: Vec<usize> = (0..n).collect();
    let mut tau = Vec::with_capacity(m.min(n));
    let block_ends: &[usize] = match policy {
        PivotPolicy::Global => &[],
        PivotPolicy::Blocked { block_ends } => block_ends,
    };
    let block_of = |column: usize| block_ends.iter().take_while(|end| column >= **end).count();
    let mut block = 0;
    let mut reference: Option<f64> = None;
    while tau.len() < m.min(n) {
        let k = tau.len();
        let mut best: Option<(usize, f64)> = None;
        for position in k..n {
            if block_of(perm[position]) != block {
                continue;
            }
            let norm = norm2(&packed.col(position)[k..]);
            let better = match best {
                None => true,
                Some((at, most)) => match norm.total_cmp(&most) {
                    std::cmp::Ordering::Greater => true,
                    std::cmp::Ordering::Equal => perm[position] < perm[at],
                    std::cmp::Ordering::Less => false,
                },
            };
            if better {
                best = Some((position, norm));
            }
        }
        let threshold = reference.map_or(0.0, |r| rel_tol * r);
        match best {
            Some((position, norm)) if norm > threshold => {
                packed.swap_cols(k, position);
                perm.swap(k, position);
                tau.push(householder_step(&mut packed, k));
                reference.get_or_insert(norm);
            }
            _ if block < block_ends.len() => block += 1,
            _ => break,
        }
    }
    let rank = tau.len();
    Qr { packed, tau, perm, rank }
}

impl Qr {
    /// Numerical rank: the number of pivots taken (for [`qr`], simply `min(rows, cols)`).
    pub fn rank(&self) -> usize {
        self.rank
    }

    /// `perm()[p]` is the original index of the column at position `p`. The first `rank` positions are the
    /// independent columns in pivot order; the rest are the dependent ones.
    pub fn perm(&self) -> &[usize] {
        &self.perm
    }

    /// `|R_kk|` for each pivot, in pivot order. A small last-to-first ratio signals ill-conditioning.
    pub fn r_diag(&self) -> Vec<f64> {
        (0..self.rank).map(|k| self.packed[(k, k)].abs()).collect()
    }

    /// Entry of `R` (zero below the diagonal and below row `rank`).
    pub fn r(&self, row: usize, col: usize) -> f64 {
        if row <= col && row < self.rank { self.packed[(row, col)] } else { 0.0 }
    }

    /// Replaces `b` by `Qᵀ b`.
    pub fn apply_qt(&self, b: &mut [f64]) {
        assert_eq!(b.len(), self.packed.rows(), "vector length");
        for (k, tau) in self.tau.iter().enumerate() {
            reflect(&self.packed.col(k)[k + 1..], *tau, &mut b[k..]);
        }
    }

    /// Replaces `b` by `Q b`.
    pub fn apply_q(&self, b: &mut [f64]) {
        assert_eq!(b.len(), self.packed.rows(), "vector length");
        for (k, tau) in self.tau.iter().enumerate().rev() {
            reflect(&self.packed.col(k)[k + 1..], *tau, &mut b[k..]);
        }
    }

    /// The given columns of the square matrix `Q`.
    ///
    /// For the QR of `Jᵀ`, columns `rank..` are an orthonormal basis of the null space of `J`.
    pub fn q_columns(&self, columns: Range<usize>) -> Matrix {
        let m = self.packed.rows();
        let mut out = Matrix::zeros(m, columns.len());
        for (j, column) in columns.enumerate() {
            let target = out.col_mut(j);
            target[column] = 1.0;
            self.apply_q(target);
        }
        out
    }

    /// Solves `R11 x = rhs`, where `R11` is the leading `rank x rank` block of `R`.
    /// Returns `None` when `rhs` has the wrong length or a diagonal entry is zero.
    pub fn solve_r11(&self, rhs: &[f64]) -> Option<Vec<f64>> {
        if rhs.len() != self.rank {
            return None;
        }
        let mut x = rhs.to_vec();
        for i in (0..self.rank).rev() {
            for k in i + 1..self.rank {
                x[i] -= self.packed[(i, k)] * x[k];
            }
            let pivot = self.packed[(i, i)];
            if pivot == 0.0 {
                return None;
            }
            x[i] /= pivot;
        }
        Some(x)
    }

    /// Solves `R11ᵀ x = rhs`. Returns `None` under the same conditions as [`Qr::solve_r11`].
    pub fn solve_r11_transposed(&self, rhs: &[f64]) -> Option<Vec<f64>> {
        if rhs.len() != self.rank {
            return None;
        }
        let mut x = rhs.to_vec();
        for i in 0..self.rank {
            for k in 0..i {
                x[i] -= self.packed[(k, i)] * x[k];
            }
            let pivot = self.packed[(i, i)];
            if pivot == 0.0 {
                return None;
            }
            x[i] /= pivot;
        }
        Some(x)
    }

    /// The first `rank` entries of the column of `R` at `position`.
    ///
    /// For a dependent column (`position >= rank`), `solve_r11` of this gives its coefficients in terms of
    /// the independent columns: `A[:, perm[position]] = sum_i x[i] * A[:, perm[i]]` up to the tolerance.
    pub fn r_column(&self, position: usize) -> Vec<f64> {
        self.packed.col(position)[..self.rank].to_vec()
    }

    /// The least-squares solution of `A x = b` for a matrix of full column rank.
    /// Returns `None` when the factorisation is rank deficient.
    pub fn solve_least_squares(&self, b: &[f64]) -> Option<Vec<f64>> {
        let n = self.packed.cols();
        if self.rank != n {
            return None;
        }
        let mut qtb = b.to_vec();
        self.apply_qt(&mut qtb);
        let z = self.solve_r11(&qtb[..n])?;
        let mut x = vec![0.0; n];
        for (position, value) in z.into_iter().enumerate() {
            x[self.perm[position]] = value;
        }
        Some(x)
    }
}

/// The least-squares solution of `A x = b` with the smallest Euclidean norm.
///
/// Works for any shape and rank; `rel_tol` decides the numerical rank as in [`qr_pivoted`].
pub fn lstsq_min_norm(a: &Matrix, b: &[f64], rel_tol: f64) -> Vec<f64> {
    assert_eq!(b.len(), a.rows(), "right-hand side length");
    let n = a.cols();
    let factor = qr_pivoted(a, PivotPolicy::Global, rel_tol);
    let rank = factor.rank();
    let mut x = vec![0.0; n];
    if rank == 0 {
        return x;
    }
    let mut qtb = b.to_vec();
    factor.apply_qt(&mut qtb);
    // A P = Q [R11 R12; 0 0]. Among all y with [R11 R12] y = c, the shortest lies in the row space, which
    // the QR of [R11 R12]ᵀ = Z T exposes: y = Z [T⁻ᵀ c; 0].
    let rows = Matrix::from_fn(n, rank, |i, j| factor.r(j, i));
    let row_space = qr(&rows);
    let Some(w) = row_space.solve_r11_transposed(&qtb[..rank]) else { return x };
    let mut y = vec![0.0; n];
    y[..rank].copy_from_slice(&w);
    row_space.apply_q(&mut y);
    for (position, value) in y.into_iter().enumerate() {
        x[factor.perm()[position]] = value;
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::singular_values;
    use crate::test_util::{Rng, max_abs_diff};

    const TOL: f64 = 1e-8;

    /// `Q R` with the columns put back in their original order.
    fn rebuild(f: &Qr, rows: usize, cols: usize) -> Matrix {
        let q = f.q_columns(0..rows);
        let r = Matrix::from_fn(rows, cols, |i, j| f.r(i, j));
        let qr = q.mul(&r);
        let mut out = Matrix::zeros(rows, cols);
        for (position, original) in f.perm().iter().enumerate() {
            out.col_mut(*original).copy_from_slice(qr.col(position));
        }
        out
    }

    fn svd_rank(a: &Matrix, rel_tol: f64) -> usize {
        let sigma = singular_values(a);
        sigma.iter().filter(|s| **s > rel_tol * sigma[0]).count()
    }

    #[test]
    fn unpivoted_qr_reconstructs_and_q_is_orthonormal() {
        let mut rng = Rng(1);
        for (m, n) in [(1, 1), (4, 4), (7, 3), (3, 7), (20, 12)] {
            let a = rng.matrix(m, n);
            let f = qr(&a);
            assert!(max_abs_diff(&rebuild(&f, m, n), &a) < 1e-13, "{m}x{n}");
            let q = f.q_columns(0..m);
            assert!(max_abs_diff(&q.transpose().mul(&q), &Matrix::identity(m)) < 1e-13, "{m}x{n}");
        }
    }

    #[test]
    fn apply_qt_inverts_apply_q() {
        let mut rng = Rng(2);
        let f = qr(&rng.matrix(9, 5));
        let original: Vec<f64> = (0..9).map(|_| rng.unit()).collect();
        let mut b = original.clone();
        f.apply_q(&mut b);
        f.apply_qt(&mut b);
        for (got, want) in b.iter().zip(&original) {
            assert!((got - want).abs() < 1e-14);
        }
    }

    #[test]
    fn least_squares_residual_is_orthogonal_to_the_columns() {
        let mut rng = Rng(3);
        let a = rng.matrix(12, 5);
        let b: Vec<f64> = (0..12).map(|_| rng.unit()).collect();
        let x = qr(&a).solve_least_squares(&b).unwrap();
        let residual: Vec<f64> = a.mul_vec(&x).iter().zip(&b).map(|(p, q)| p - q).collect();
        assert!(a.tr_mul_vec(&residual).iter().all(|g| g.abs() < 1e-13));
        let pivoted = qr_pivoted(&a, PivotPolicy::Global, TOL).solve_least_squares(&b).unwrap();
        for (p, q) in pivoted.iter().zip(&x) {
            assert!((p - q).abs() < 1e-12);
        }
    }

    #[test]
    fn pivoted_rank_equals_svd_rank_on_rank_deficient_matrices() {
        let mut rng = Rng(4);
        for (m, n, rank) in
            [(6, 6, 0), (6, 6, 1), (8, 5, 3), (5, 8, 3), (10, 10, 9), (30, 20, 11), (12, 40, 12)]
        {
            for _ in 0..8 {
                let a = rng.matrix_of_rank(m, n, rank);
                let f = qr_pivoted(&a, PivotPolicy::Global, TOL);
                assert_eq!(f.rank(), rank, "{m}x{n} rank {rank}");
                assert_eq!(svd_rank(&a, TOL), rank, "{m}x{n} rank {rank}");
                assert!(max_abs_diff(&rebuild(&f, m, n), &a) < 1e-12);
            }
        }
    }

    #[test]
    fn pivoted_rank_equals_svd_rank_on_a_kahan_matrix() {
        // Upper triangular, graded: the classic matrix on which naive rank estimates go wrong.
        let (n, c, s) = (30, libm::cos(1.2), libm::sin(1.2));
        let kahan = Matrix::from_fn(n, n, |i, j| {
            let scale = s.powi(i32::try_from(i).unwrap());
            match i.cmp(&j) {
                std::cmp::Ordering::Less => -c * scale,
                std::cmp::Ordering::Equal => scale,
                std::cmp::Ordering::Greater => 0.0,
            }
        });
        for tol in [1e-6, 1e-8, 1e-10] {
            assert_eq!(
                qr_pivoted(&kahan, PivotPolicy::Global, tol).rank(),
                svd_rank(&kahan, tol),
                "tol {tol}"
            );
        }
    }

    #[test]
    fn dependent_columns_are_combinations_of_the_pivots() {
        let mut rng = Rng(6);
        let a = rng.matrix_of_rank(9, 7, 4);
        let f = qr_pivoted(&a, PivotPolicy::Global, TOL);
        assert_eq!(f.rank(), 4);
        for position in f.rank()..7 {
            let coefficients = f.solve_r11(&f.r_column(position)).unwrap();
            let mut combination = [0.0; 9];
            for (i, c) in coefficients.iter().enumerate() {
                for (acc, v) in combination.iter_mut().zip(a.col(f.perm()[i])) {
                    *acc += c * v;
                }
            }
            for (got, want) in combination.iter().zip(a.col(f.perm()[position])) {
                assert!((got - want).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn trailing_q_columns_span_the_null_space_of_the_transpose() {
        let mut rng = Rng(8);
        let j = rng.matrix_of_rank(5, 9, 3); // 5 constraint rows, 9 parameters, rank 3
        let f = qr_pivoted(&j.transpose(), PivotPolicy::Global, TOL);
        let null = f.q_columns(f.rank()..9);
        assert_eq!(null.cols(), 6);
        assert!(j.mul(&null).frobenius_norm() < 1e-12);
        assert!(max_abs_diff(&null.transpose().mul(&null), &Matrix::identity(6)) < 1e-13);
    }

    #[test]
    fn blocked_pivoting_prefers_earlier_blocks() {
        let mut rng = Rng(9);
        let base = rng.matrix(8, 3);
        // Columns 0..3 are independent; column 3 is ten times column 0; column 4 is new; column 5 = col 1 + col 4.
        let fourth = rng.matrix(8, 1);
        let a = Matrix::from_fn(8, 6, |i, j| match j {
            0..=2 => base[(i, j)],
            3 => 10.0 * base[(i, 0)],
            4 => fourth[(i, 0)],
            _ => base[(i, 1)] + fourth[(i, 0)],
        });
        let global = qr_pivoted(&a, PivotPolicy::Global, TOL);
        assert_eq!(global.rank(), 4);
        assert_eq!(global.perm()[0], 3, "global pivoting starts with the largest column");

        let blocked = qr_pivoted(&a, PivotPolicy::Blocked { block_ends: &[3, 5] }, TOL);
        assert_eq!(blocked.rank(), 4);
        let mut pivots = blocked.perm()[..4].to_vec();
        pivots.sort_unstable();
        assert_eq!(pivots, vec![0, 1, 2, 4], "later duplicates are the dependent ones");
        let mut first_block = blocked.perm()[..3].to_vec();
        first_block.sort_unstable();
        assert_eq!(first_block, vec![0, 1, 2], "the first block is exhausted before the second starts");
        assert!(max_abs_diff(&rebuild(&blocked, 8, 6), &a) < 1e-12);
    }

    #[test]
    fn blocked_pivoting_handles_empty_and_all_zero_blocks() {
        let a = Matrix::from_rows(&[&[0.0, 0.0, 1.0], &[0.0, 0.0, 2.0]]);
        let f = qr_pivoted(&a, PivotPolicy::Blocked { block_ends: &[0, 2, 2] }, TOL);
        assert_eq!(f.rank(), 1);
        assert_eq!(f.perm()[0], 2);
        assert_eq!(qr_pivoted(&Matrix::zeros(3, 3), PivotPolicy::Global, TOL).rank(), 0);
        assert_eq!(qr_pivoted(&Matrix::zeros(0, 3), PivotPolicy::Global, TOL).rank(), 0);
    }

    #[test]
    fn min_norm_solution_solves_the_normal_equations_inside_the_row_space() {
        let mut rng = Rng(10);
        for (m, n, rank) in [(8, 5, 5), (8, 5, 3), (4, 9, 4), (4, 9, 2), (6, 6, 0)] {
            let a = rng.matrix_of_rank(m, n, rank);
            let b: Vec<f64> = (0..m).map(|_| rng.unit()).collect();
            let x = lstsq_min_norm(&a, &b, TOL);
            let residual: Vec<f64> = a.mul_vec(&x).iter().zip(&b).map(|(p, q)| p - q).collect();
            assert!(a.tr_mul_vec(&residual).iter().all(|g| g.abs() < 1e-11), "{m}x{n} rank {rank}");
            // Minimum norm: no component in the null space of A.
            let f = qr_pivoted(&a.transpose(), PivotPolicy::Global, TOL);
            let null = f.q_columns(f.rank()..n);
            assert!(null.tr_mul_vec(&x).iter().all(|c| c.abs() < 1e-11), "{m}x{n} rank {rank}");
        }
    }
}
