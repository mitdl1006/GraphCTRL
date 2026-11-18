//! Geometry types (glam, f64), planes, rays, boxes and rotation conventions.
//!
//! This is the only crate that names the vector maths library. Everything else imports the types from
//! here, so replacing or upgrading the library touches one place.
//!
//! Conventions: SI units (metres, radians), right-handed coordinates, Z up.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::float_cmp, reason = "tests compare exactly representable values"))]

pub mod fm;
mod rotation;
mod shapes;

pub use glamx::{DAffine3, DMat3, DMat4, DPose3, DQuat, DVec2, DVec3, DVec4};
pub use rotation::{pose_from_xyz_rpy, quat_from_rotvec, quat_from_rpy, rotvec_from_quat, rpy_from_quat};
pub use shapes::{Aabb, Plane, Ray};

/// Single-precision types for data sent to the GPU.
pub mod gpu {
    pub use glamx::{Mat4, Quat, Vec2, Vec3, Vec4};
}

/// `value` kept between `a` and `b` in either order, without panicking (unlike `f64::clamp`, which panics
/// when the bounds are out of order or NaN): with a NaN bound the value is left as it is; a NaN value
/// comes out as the lower bound.
pub fn clamp_between(value: f64, a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        return value;
    }
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    value.max(lo).min(hi)
}

#[cfg(test)]
mod clamp_tests {
    use super::clamp_between;

    #[test]
    fn clamp_between_never_panics() {
        assert_eq!(clamp_between(5.0, 0.0, 1.0), 1.0);
        assert_eq!(clamp_between(5.0, 1.0, 0.0), 1.0, "bounds out of order");
        assert_eq!(clamp_between(-5.0, 1.0, 0.0), 0.0);
        assert_eq!(clamp_between(0.5, f64::NAN, 1.0), 0.5, "a NaN bound leaves the value");
        assert_eq!(clamp_between(5.0, f64::NAN, 1.0), 5.0);
        assert_eq!(clamp_between(f64::NAN, 0.0, 1.0), 0.0);
    }
}
