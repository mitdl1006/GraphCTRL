//! Jacobi iterations: symmetric eigenvalues, and singular values as a test oracle.

use crate::{Matrix, norm2};

const MAX_SWEEPS: usize = 64;

/// Eigenvalues and eigenvectors of a symmetric matrix.
#[derive(Clone, Debug)]
pub struct SymEigen {
    /// Eigenvalues in ascending order.
    pub values: Vec<f64>,
    /// Unit eigenvectors as columns, in the order of `values`.
    pub vectors: Matrix,
}

/// Rotation `(c, s)` that zeroes the off-diagonal term, from `zeta = (a_qq - a_pp) / (2 a_pq)`.
fn rotation(zeta: f64) -> (f64, f64) {
    let t = zeta.signum() / (zeta.abs() + (zeta * zeta + 1.0).sqrt());
    let c = 1.0 / (t * t + 1.0).sqrt();
    (c, t * c)
}

/// Cyclic Jacobi eigen-decomposition of a symmetric matrix (the upper triangle is trusted).
pub fn sym_eigen(a: &Matrix) -> SymEigen {
    let n = a.rows();
    assert_eq!(a.cols(), n, "sym_eigen needs a square matrix");
    let mut a = a.clone();
    let mut v = Matrix::identity(n);
    let scale = a.frobenius_norm();
    for _ in 0..MAX_SWEEPS {
        let mut off = 0.0_f64;
        for q in 1..n {
            for p in 0..q {
                off = off.max(a[(p, q)].abs());
            }
        }
        if off <= f64::EPSILON * scale {
            break;
        }
        for q in 1..n {
            for p in 0..q {
                let apq = a[(p, q)];
                if apq == 0.0 {
                    continue;
                }
                let (c, s) = rotation((a[(q, q)] - a[(p, p)]) / (2.0 * apq));
                for k in 0..n {
                    let (akp, akq) = (a[(k, p)], a[(k, q)]);
                    a[(k, p)] = c * akp - s * akq;
                    a[(k, q)] = s * akp + c * akq;
                }
                for k in 0..n {
                    let (apk, aqk) = (a[(p, k)], a[(q, k)]);
                    a[(p, k)] = c * apk - s * aqk;
                    a[(q, k)] = s * apk + c * aqk;
                }
                for k in 0..n {
                    let (vkp, vkq) = (v[(k, p)], v[(k, q)]);
                    v[(k, p)] = c * vkp - s * vkq;
                    v[(k, q)] = s * vkp + c * vkq;
                }
            }
        }
    }
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&i, &j| a[(i, i)].total_cmp(&a[(j, j)]));
    SymEigen {
        values: order.iter().map(|&i| a[(i, i)]).collect(),
        vectors: Matrix::from_fn(n, n, |i, j| v[(i, order[j])]),
    }
}

/// Singular values in descending order, by one-sided Jacobi.
///
/// Accurate to high relative precision, which makes it the reference the pivoted QR rank is tested against.
/// The solver itself does not call it.
pub fn singular_values(a: &Matrix) -> Vec<f64> {
    let mut u = if a.rows() >= a.cols() { a.clone() } else { a.transpose() };
    let n = u.cols();
    for _ in 0..MAX_SWEEPS {
        let mut rotated = false;
        for q in 1..n {
            for p in 0..q {
                let (up, uq) = u.two_cols_mut(p, q);
                let alpha: f64 = up.iter().map(|x| x * x).sum();
                let beta: f64 = uq.iter().map(|x| x * x).sum();
                let gamma: f64 = up.iter().zip(uq.iter()).map(|(x, y)| x * y).sum();
                if gamma == 0.0 || gamma.abs() <= f64::EPSILON * (alpha * beta).sqrt() {
                    continue;
                }
                rotated = true;
                let (c, s) = rotation((beta - alpha) / (2.0 * gamma));
                for (x, y) in up.iter_mut().zip(uq.iter_mut()) {
                    let (xp, xq) = (*x, *y);
                    *x = c * xp - s * xq;
                    *y = s * xp + c * xq;
                }
            }
        }
        if !rotated {
            break;
        }
    }
    let mut values: Vec<f64> = (0..n).map(|j| norm2(u.col(j))).collect();
    values.sort_by(|a, b| b.total_cmp(a));
    values
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{Rng, max_abs_diff};

    #[test]
    fn eigenvalues_of_a_known_matrix() {
        let e = sym_eigen(&Matrix::from_rows(&[&[2.0, 1.0], &[1.0, 2.0]]));
        assert!((e.values[0] - 1.0).abs() < 1e-14);
        assert!((e.values[1] - 3.0).abs() < 1e-14);
    }

    #[test]
    fn eigenvectors_diagonalise_random_symmetric_matrices() {
        let mut rng = Rng(11);
        for n in [1, 2, 3, 6, 12] {
            let b = rng.matrix(n, n);
            let a = b.transpose().mul(&b);
            let e = sym_eigen(&a);
            let lambda = Matrix::from_fn(n, n, |i, j| if i == j { e.values[i] } else { 0.0 });
            let rebuilt = e.vectors.mul(&lambda).mul(&e.vectors.transpose());
            assert!(max_abs_diff(&rebuilt, &a) < 1e-12, "n = {n}");
            let gram = e.vectors.transpose().mul(&e.vectors);
            assert!(max_abs_diff(&gram, &Matrix::identity(n)) < 1e-13, "n = {n}");
            assert!(e.values.windows(2).all(|w| w[0] <= w[1]));
        }
    }

    #[test]
    fn singular_values_match_the_eigenvalues_of_the_gram_matrix() {
        let mut rng = Rng(5);
        for (m, n) in [(5, 3), (3, 5), (6, 6)] {
            let a = rng.matrix(m, n);
            let sigma = singular_values(&a);
            let gram = if m >= n { a.transpose().mul(&a) } else { a.mul(&a.transpose()) };
            let mut expected: Vec<f64> = sym_eigen(&gram).values.iter().map(|v| v.max(0.0).sqrt()).collect();
            expected.reverse();
            assert_eq!(sigma.len(), m.min(n));
            for (got, want) in sigma.iter().zip(&expected) {
                assert!((got - want).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn singular_values_of_a_diagonal_matrix() {
        let a = Matrix::from_rows(&[&[0.0, 2.0], &[-3.0, 0.0], &[0.0, 0.0]]);
        assert_eq!(singular_values(&a), vec![3.0, 2.0]);
    }
}
