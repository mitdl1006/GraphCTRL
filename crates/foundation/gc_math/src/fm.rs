//! Transcendental functions with the same result on every platform (backed by `libm`).
//!
//! The standard library forwards these to the platform's C library, whose last bits differ between
//! operating systems. Code whose output must be reproducible calls these instead; clippy rejects the
//! `f64` methods (see `clippy.toml`). `sqrt`, `abs`, `floor` and friends are exact everywhere and need no
//! wrapper.

/// Sine of an angle in radians.
pub fn sin(x: f64) -> f64 {
    libm::sin(x)
}

/// Cosine of an angle in radians.
pub fn cos(x: f64) -> f64 {
    libm::cos(x)
}

/// `(sin(x), cos(x))`.
pub fn sin_cos(x: f64) -> (f64, f64) {
    libm::sincos(x)
}

/// Tangent of an angle in radians.
pub fn tan(x: f64) -> f64 {
    libm::tan(x)
}

/// Arcsine; NaN outside `[-1, 1]`.
pub fn asin(x: f64) -> f64 {
    libm::asin(x)
}

/// Arccosine; NaN outside `[-1, 1]`.
pub fn acos(x: f64) -> f64 {
    libm::acos(x)
}

/// Arcsine of `x` clamped to `[-1, 1]`, for values that are in range up to rounding.
pub fn asin_clamped(x: f64) -> f64 {
    libm::asin(x.clamp(-1.0, 1.0))
}

/// Arccosine of `x` clamped to `[-1, 1]`, for values that are in range up to rounding.
pub fn acos_clamped(x: f64) -> f64 {
    libm::acos(x.clamp(-1.0, 1.0))
}

/// Arctangent.
pub fn atan(x: f64) -> f64 {
    libm::atan(x)
}

/// Four-quadrant arctangent of `y / x`.
pub fn atan2(y: f64, x: f64) -> f64 {
    libm::atan2(y, x)
}

/// `e^x`.
pub fn exp(x: f64) -> f64 {
    libm::exp(x)
}

/// Natural logarithm.
pub fn ln(x: f64) -> f64 {
    libm::log(x)
}

/// `x^y`.
pub fn powf(x: f64, y: f64) -> f64 {
    libm::pow(x, y)
}

/// `sqrt(x^2 + y^2)` without intermediate overflow.
pub fn hypot(x: f64, y: f64) -> f64 {
    libm::hypot(x, y)
}

/// Wraps an angle into `(-pi, pi]`.
pub fn wrap_angle(angle: f64) -> f64 {
    use std::f64::consts::{PI, TAU};
    let wrapped = angle - TAU * ((angle + PI) / TAU).floor();
    // `floor` puts exact odd multiples of pi at -pi; the interval is closed at +pi instead.
    if wrapped <= -PI { wrapped + TAU } else { wrapped }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, PI, TAU};

    use super::*;

    #[test]
    fn basic_identities() {
        assert_eq!(sin(0.0), 0.0);
        assert_eq!(cos(0.0), 1.0);
        assert!((atan2(1.0, 0.0) - FRAC_PI_2).abs() < 1e-15);
        assert!((hypot(3.0, 4.0) - 5.0).abs() < 1e-15);
        assert!((ln(exp(1.5)) - 1.5).abs() < 1e-15);
        assert!((powf(2.0, 10.0) - 1024.0).abs() < 1e-12);
        let (s, c) = sin_cos(0.3);
        assert_eq!((s, c), (sin(0.3), cos(0.3)));
    }

    #[test]
    fn clamped_inverses_never_return_nan() {
        assert!(acos(1.0 + 1e-12).is_nan());
        assert_eq!(acos_clamped(1.0 + 1e-12), 0.0);
        assert!((asin_clamped(-1.0 - 1e-12) + FRAC_PI_2).abs() < 1e-15);
    }

    #[test]
    fn wrap_angle_stays_in_the_half_open_interval() {
        assert_eq!(wrap_angle(0.0), 0.0);
        assert_eq!(wrap_angle(PI), PI);
        assert_eq!(wrap_angle(-PI), PI);
        assert!((wrap_angle(3.0 * PI) - PI).abs() < 1e-12);
        assert!((wrap_angle(TAU + 0.25) - 0.25).abs() < 1e-12);
        assert!((wrap_angle(-TAU - 0.25) + 0.25).abs() < 1e-12);
        for k in -20..=20 {
            let w = wrap_angle(f64::from(k) * 0.7);
            assert!(w > -PI && w <= PI);
        }
    }
}
