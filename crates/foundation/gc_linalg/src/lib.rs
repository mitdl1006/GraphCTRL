//! Small dense linear algebra for the constraint solver and kinematics.
//!
//! In-house and scalar on purpose: results are bit-identical between runs on one platform, there is no
//! threading and no SIMD dispatch. Sizes are small (tens to a few hundred unknowns), so clarity wins over
//! blocked algorithms.
//!
//! Functions that take several matrices or vectors panic when their dimensions do not fit; that is a
//! programming error in the caller, not a property of the data.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::float_cmp, reason = "tests compare exactly representable values"))]

mod cholesky;
mod jacobi;
mod matrix;
mod qr;

pub use cholesky::Cholesky;
pub use jacobi::{SymEigen, singular_values, sym_eigen};
pub use matrix::{Matrix, norm2};
pub use qr::{PivotPolicy, Qr, lstsq_min_norm, qr, qr_pivoted};

#[cfg(test)]
pub(crate) mod test_util {
    use crate::Matrix;

    /// SplitMix64: a tiny deterministic generator for test matrices.
    pub struct Rng(pub u64);

    impl Rng {
        pub fn next_u64(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^ (z >> 31)
        }

        /// Uniform in [-1, 1).
        pub fn unit(&mut self) -> f64 {
            (self.next_u64() >> 11) as f64 / (1_u64 << 52) as f64 - 1.0
        }

        pub fn matrix(&mut self, rows: usize, cols: usize) -> Matrix {
            Matrix::from_fn(rows, cols, |_, _| self.unit())
        }

        /// A `rows x cols` matrix of exactly the given rank (product of two random factors).
        pub fn matrix_of_rank(&mut self, rows: usize, cols: usize, rank: usize) -> Matrix {
            self.matrix(rows, rank).mul(&self.matrix(rank, cols))
        }
    }

    pub fn max_abs_diff(a: &Matrix, b: &Matrix) -> f64 {
        assert_eq!((a.rows(), a.cols()), (b.rows(), b.cols()));
        let mut worst = 0.0_f64;
        for j in 0..a.cols() {
            for i in 0..a.rows() {
                worst = worst.max((a[(i, j)] - b[(i, j)]).abs());
            }
        }
        worst
    }
}
