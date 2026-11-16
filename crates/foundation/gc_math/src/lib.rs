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
