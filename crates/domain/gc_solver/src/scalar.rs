//! Forward-mode automatic differentiation for constraint residuals.
//!
//! Each residual is written once, generic over [`Scalar`]. Evaluated with `f64` it gives the value;
//! evaluated with [`Dual`] it also gives the partial derivatives with respect to its inputs.

use std::ops::{Add, Div, Mul, Neg, Sub};

/// The most inputs a single residual row reads (two lines of two 3D points each).
pub(crate) const MAX_VARS: usize = 12;

pub(crate) trait Scalar:
    Copy + Add<Output = Self> + Sub<Output = Self> + Mul<Output = Self> + Div<Output = Self> + Neg<Output = Self>
{
    fn constant(value: f64) -> Self;
    fn value(self) -> f64;
    fn sqrt(self) -> Self;
    fn atan2(y: Self, x: Self) -> Self;
    /// The same derivatives with another value; used by guards that replace an undefined derivative.
    fn with_value(self, value: f64) -> Self;
}

impl Scalar for f64 {
    fn constant(value: f64) -> Self {
        value
    }
    fn value(self) -> f64 {
        self
    }
    fn sqrt(self) -> Self {
        f64::sqrt(self)
    }
    fn atan2(y: Self, x: Self) -> Self {
        libm::atan2(y, x)
    }
    fn with_value(self, value: f64) -> Self {
        value
    }
}

/// A value together with its partial derivatives with respect to up to [`MAX_VARS`] inputs.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Dual {
    pub v: f64,
    pub d: [f64; MAX_VARS],
}

impl Dual {
    /// Input number `slot` with the given value.
    pub fn variable(value: f64, slot: usize) -> Self {
        let mut d = [0.0; MAX_VARS];
        d[slot] = 1.0;
        Self { v: value, d }
    }

    fn map(self, value: f64, factor: f64) -> Self {
        Self { v: value, d: self.d.map(|x| x * factor) }
    }

    fn zip(self, other: Self, value: f64, fa: f64, fb: f64) -> Self {
        let mut d = [0.0; MAX_VARS];
        for (out, (a, b)) in d.iter_mut().zip(self.d.iter().zip(&other.d)) {
            *out = fa * a + fb * b;
        }
        Self { v: value, d }
    }
}

impl Add for Dual {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        self.zip(rhs, self.v + rhs.v, 1.0, 1.0)
    }
}
impl Sub for Dual {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        self.zip(rhs, self.v - rhs.v, 1.0, -1.0)
    }
}
impl Mul for Dual {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        self.zip(rhs, self.v * rhs.v, rhs.v, self.v)
    }
}
impl Div for Dual {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        let q = self.v / rhs.v;
        self.zip(rhs, q, 1.0 / rhs.v, -q / rhs.v)
    }
}
impl Neg for Dual {
    type Output = Self;
    fn neg(self) -> Self {
        self.map(-self.v, -1.0)
    }
}

impl Scalar for Dual {
    fn constant(value: f64) -> Self {
        Self { v: value, d: [0.0; MAX_VARS] }
    }
    fn value(self) -> f64 {
        self.v
    }
    fn sqrt(self) -> Self {
        let root = self.v.sqrt();
        self.map(root, 0.5 / root)
    }
    fn atan2(y: Self, x: Self) -> Self {
        let n2 = x.v * x.v + y.v * y.v;
        let angle = libm::atan2(y.v, x.v);
        if n2 == 0.0 {
            // The direction is undefined; callers flag the constraint as degenerate.
            return Self::constant(angle);
        }
        y.zip(x, angle, x.v / n2, -y.v / n2)
    }
    fn with_value(self, value: f64) -> Self {
        Self { v: value, d: self.d }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f<S: Scalar>(x: S, y: S) -> S {
        let two = S::constant(2.0);
        S::atan2(y * x, x - y / two) + (x * x + y * y).sqrt() / (x + two) - (-y)
    }

    #[test]
    fn derivatives_match_central_differences() {
        let (x, y, h) = (1.3, -0.7, 1e-6);
        let dual = f(Dual::variable(x, 0), Dual::variable(y, 1));
        assert!((dual.v - f(x, y)).abs() < 1e-15);
        let dx = (f(x + h, y) - f(x - h, y)) / (2.0 * h);
        let dy = (f(x, y + h) - f(x, y - h)) / (2.0 * h);
        assert!((dual.d[0] - dx).abs() < 1e-8, "{} vs {dx}", dual.d[0]);
        assert!((dual.d[1] - dy).abs() < 1e-8, "{} vs {dy}", dual.d[1]);
    }

    #[test]
    fn atan2_at_the_origin_has_no_nan() {
        let angle = Dual::atan2(Dual::variable(0.0, 0), Dual::variable(0.0, 1));
        assert!(angle.v.is_finite() && angle.d.iter().all(|d| d.is_finite()));
    }
}
