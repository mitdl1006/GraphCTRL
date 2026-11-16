//! Cholesky factorisation of symmetric positive definite matrices.

use crate::Matrix;

/// The lower-triangular factor `L` with `A = L Lᵀ`.
#[derive(Clone, Debug)]
pub struct Cholesky {
    l: Matrix,
}

impl Cholesky {
    /// Factors a symmetric matrix (only the lower triangle is read).
    /// Returns `None` when the matrix is not square or not positive definite.
    pub fn new(a: &Matrix) -> Option<Self> {
        let n = a.rows();
        if a.cols() != n {
            return None;
        }
        let mut l = Matrix::zeros(n, n);
        for j in 0..n {
            let mut diagonal = a[(j, j)];
            for k in 0..j {
                diagonal -= l[(j, k)] * l[(j, k)];
            }
            if diagonal.is_nan() || diagonal <= 0.0 {
                return None;
            }
            let pivot = diagonal.sqrt();
            l[(j, j)] = pivot;
            for i in j + 1..n {
                let mut value = a[(i, j)];
                for k in 0..j {
                    value -= l[(i, k)] * l[(j, k)];
                }
                l[(i, j)] = value / pivot;
            }
        }
        Some(Self { l })
    }

    /// The factor `L`.
    pub fn l(&self) -> &Matrix {
        &self.l
    }

    /// Solves `A x = b`.
    pub fn solve(&self, b: &[f64]) -> Vec<f64> {
        let n = self.l.rows();
        assert_eq!(b.len(), n, "right-hand side length");
        let mut x = b.to_vec();
        for i in 0..n {
            for k in 0..i {
                x[i] -= self.l[(i, k)] * x[k];
            }
            x[i] /= self.l[(i, i)];
        }
        for i in (0..n).rev() {
            for k in i + 1..n {
                x[i] -= self.l[(k, i)] * x[k];
            }
            x[i] /= self.l[(i, i)];
        }
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::Rng;

    #[test]
    fn solves_a_positive_definite_system() {
        let mut rng = Rng(7);
        let b = rng.matrix(8, 6);
        let mut a = b.transpose().mul(&b);
        for i in 0..6 {
            a[(i, i)] += 0.5;
        }
        let truth: Vec<f64> = (0..6).map(|_| rng.unit()).collect();
        let x = Cholesky::new(&a).unwrap().solve(&a.mul_vec(&truth));
        for (got, want) in x.iter().zip(&truth) {
            assert!((got - want).abs() < 1e-12);
        }
    }

    #[test]
    fn rejects_matrices_that_are_not_positive_definite() {
        assert!(Cholesky::new(&Matrix::from_rows(&[&[1.0, 2.0], &[2.0, 1.0]])).is_none());
        assert!(Cholesky::new(&Matrix::zeros(2, 3)).is_none());
        assert!(Cholesky::new(&Matrix::from_rows(&[&[f64::NAN]])).is_none());
    }
}
