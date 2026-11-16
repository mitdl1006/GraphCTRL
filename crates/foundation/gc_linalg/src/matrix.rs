//! Dense column-major matrix.

use std::ops::{Index, IndexMut};

/// Euclidean norm, scaled so that very large or very small entries neither overflow nor vanish.
pub fn norm2(values: &[f64]) -> f64 {
    let largest = values.iter().fold(0.0_f64, |acc, v| acc.max(v.abs()));
    if largest == 0.0 || !largest.is_finite() {
        return largest;
    }
    let sum: f64 = values.iter().map(|v| (v / largest) * (v / largest)).sum();
    largest * sum.sqrt()
}

/// A dense matrix of `f64`, stored column by column.
#[derive(Clone, Debug, PartialEq)]
pub struct Matrix {
    rows: usize,
    cols: usize,
    data: Vec<f64>,
}

impl Matrix {
    /// A matrix of zeros.
    pub fn zeros(rows: usize, cols: usize) -> Self {
        Self { rows, cols, data: vec![0.0; rows * cols] }
    }

    /// The identity matrix.
    pub fn identity(n: usize) -> Self {
        Self::from_fn(n, n, |i, j| if i == j { 1.0 } else { 0.0 })
    }

    /// Builds a matrix from `f(row, column)`, called column by column.
    pub fn from_fn(rows: usize, cols: usize, mut f: impl FnMut(usize, usize) -> f64) -> Self {
        let mut data = Vec::with_capacity(rows * cols);
        for j in 0..cols {
            for i in 0..rows {
                data.push(f(i, j));
            }
        }
        Self { rows, cols, data }
    }

    /// Builds a matrix from rows. The first row sets the width; shorter rows are padded with zeros.
    pub fn from_rows(rows: &[&[f64]]) -> Self {
        let cols = rows.first().map_or(0, |r| r.len());
        Self::from_fn(rows.len(), cols, |i, j| rows[i].get(j).copied().unwrap_or(0.0))
    }

    /// Number of rows.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Number of columns.
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Column `j` as a slice.
    pub fn col(&self, j: usize) -> &[f64] {
        &self.data[j * self.rows..(j + 1) * self.rows]
    }

    /// Column `j` as a mutable slice.
    pub fn col_mut(&mut self, j: usize) -> &mut [f64] {
        &mut self.data[j * self.rows..(j + 1) * self.rows]
    }

    /// Columns `a` and `b` (with `a < b`) at once: `a` shared, `b` mutable.
    pub fn col_pair_mut(&mut self, a: usize, b: usize) -> (&[f64], &mut [f64]) {
        assert!(a < b, "col_pair_mut needs a < b");
        let (left, right) = self.data.split_at_mut(b * self.rows);
        (&left[a * self.rows..(a + 1) * self.rows], &mut right[..self.rows])
    }

    /// Both columns `a` and `b` (with `a < b`) mutably.
    pub fn two_cols_mut(&mut self, a: usize, b: usize) -> (&mut [f64], &mut [f64]) {
        assert!(a < b, "two_cols_mut needs a < b");
        let (left, right) = self.data.split_at_mut(b * self.rows);
        (&mut left[a * self.rows..(a + 1) * self.rows], &mut right[..self.rows])
    }

    /// Exchanges two columns.
    pub fn swap_cols(&mut self, a: usize, b: usize) {
        if a != b {
            let (low, high) = self.two_cols_mut(a.min(b), a.max(b));
            low.swap_with_slice(high);
        }
    }

    /// The transpose.
    pub fn transpose(&self) -> Self {
        Self::from_fn(self.cols, self.rows, |i, j| self[(j, i)])
    }

    /// The product `self * rhs`.
    pub fn mul(&self, rhs: &Self) -> Self {
        assert_eq!(self.cols, rhs.rows, "matrix product dimensions");
        let mut out = Self::zeros(self.rows, rhs.cols);
        for j in 0..rhs.cols {
            let target = &mut out.data[j * self.rows..(j + 1) * self.rows];
            for (k, factor) in rhs.col(j).iter().enumerate() {
                for (t, a) in target.iter_mut().zip(self.col(k)) {
                    *t += a * factor;
                }
            }
        }
        out
    }

    /// The product `self * x`.
    pub fn mul_vec(&self, x: &[f64]) -> Vec<f64> {
        assert_eq!(self.cols, x.len(), "matrix-vector product dimensions");
        let mut out = vec![0.0; self.rows];
        for (j, factor) in x.iter().enumerate() {
            for (o, a) in out.iter_mut().zip(self.col(j)) {
                *o += a * factor;
            }
        }
        out
    }

    /// The product `selfᵀ * y`.
    pub fn tr_mul_vec(&self, y: &[f64]) -> Vec<f64> {
        assert_eq!(self.rows, y.len(), "transposed matrix-vector product dimensions");
        (0..self.cols).map(|j| self.col(j).iter().zip(y).map(|(a, b)| a * b).sum()).collect()
    }

    /// The Frobenius norm (square root of the sum of squared entries).
    pub fn frobenius_norm(&self) -> f64 {
        norm2(&self.data)
    }

    /// `top` stacked on `bottom`, e.g. the Levenberg-Marquardt system `[J; sqrt(lambda) I]`.
    pub fn stack_rows(top: &Self, bottom: &Self) -> Self {
        assert_eq!(top.cols, bottom.cols, "stacked matrices need equal widths");
        Self::from_fn(top.rows + bottom.rows, top.cols, |i, j| {
            if i < top.rows { top[(i, j)] } else { bottom[(i - top.rows, j)] }
        })
    }
}

impl Index<(usize, usize)> for Matrix {
    type Output = f64;

    fn index(&self, (row, col): (usize, usize)) -> &f64 {
        assert!(row < self.rows && col < self.cols, "matrix index out of range");
        &self.data[col * self.rows + row]
    }
}

impl IndexMut<(usize, usize)> for Matrix {
    fn index_mut(&mut self, (row, col): (usize, usize)) -> &mut f64 {
        assert!(row < self.rows && col < self.cols, "matrix index out of range");
        &mut self.data[col * self.rows + row]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn norm_survives_extreme_magnitudes() {
        assert_eq!(norm2(&[]), 0.0);
        assert!((norm2(&[3.0, 4.0]) - 5.0).abs() < 1e-15);
        assert!((norm2(&[3e200, 4e200]) / 5e200 - 1.0).abs() < 1e-15);
        assert!((norm2(&[3e-200, 4e-200]) / 5e-200 - 1.0).abs() < 1e-15);
    }

    #[test]
    fn products_and_transpose() {
        let a = Matrix::from_rows(&[&[1.0, 2.0, 3.0], &[4.0, 5.0, 6.0]]);
        let b = Matrix::from_rows(&[&[1.0, 0.0], &[0.0, 1.0], &[2.0, -1.0]]);
        assert_eq!(a.mul(&b), Matrix::from_rows(&[&[7.0, -1.0], &[16.0, -1.0]]));
        assert_eq!(a.mul_vec(&[1.0, 1.0, 1.0]), vec![6.0, 15.0]);
        assert_eq!(a.tr_mul_vec(&[1.0, 1.0]), vec![5.0, 7.0, 9.0]);
        assert_eq!(a.transpose()[(2, 1)], 6.0);
        assert_eq!(Matrix::stack_rows(&a, &a).rows(), 4);
    }

    #[test]
    fn column_access_and_swap() {
        let mut a = Matrix::from_rows(&[&[1.0, 2.0], &[3.0, 4.0]]);
        a.swap_cols(1, 0);
        assert_eq!(a.col(0), &[2.0, 4.0]);
        let (first, second) = a.col_pair_mut(0, 1);
        second[0] += first[0];
        assert_eq!(a[(0, 1)], 3.0);
    }
}
