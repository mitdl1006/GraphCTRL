//! A rigid transform as plain numbers.

use serde::{Deserialize, Serialize};

/// A rigid transform: rotate by the unit quaternion, then translate.
///
/// Deliberately independent of any maths library so that plugins and the host agree on bytes, not on types.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pose {
    /// Translation in metres.
    pub xyz: [f64; 3],
    /// Rotation as a unit quaternion, scalar last.
    pub quat_xyzw: [f64; 4],
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

impl Pose {
    /// The transform that changes nothing.
    pub const IDENTITY: Self = Self { xyz: [0.0; 3], quat_xyzw: [0.0, 0.0, 0.0, 1.0] };

    /// A pure translation.
    pub fn from_translation(xyz: [f64; 3]) -> Self {
        Self { xyz, ..Self::IDENTITY }
    }

    /// A pure rotation by `angle` radians about `axis` (normalised here; a zero axis gives the identity).
    pub fn from_axis_angle(axis: [f64; 3], angle: f64) -> Self {
        let length = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
        if length == 0.0 {
            return Self::IDENTITY;
        }
        let (s, c) = (libm::sin(angle * 0.5), libm::cos(angle * 0.5));
        let k = s / length;
        Self { xyz: [0.0; 3], quat_xyzw: [axis[0] * k, axis[1] * k, axis[2] * k, c] }
    }

    /// Rotates a vector.
    pub fn rotate(&self, v: [f64; 3]) -> [f64; 3] {
        let [x, y, z, w] = self.quat_xyzw;
        let u = [x, y, z];
        // v + 2 u x (u x v + w v)
        let t = cross(u, v);
        let inner = [t[0] + w * v[0], t[1] + w * v[1], t[2] + w * v[2]];
        let outer = cross(u, inner);
        [v[0] + 2.0 * outer[0], v[1] + 2.0 * outer[1], v[2] + 2.0 * outer[2]]
    }

    /// Transforms a point.
    pub fn transform_point(&self, p: [f64; 3]) -> [f64; 3] {
        let r = self.rotate(p);
        [r[0] + self.xyz[0], r[1] + self.xyz[1], r[2] + self.xyz[2]]
    }

    /// The composition `self * rhs`: apply `rhs` first, then `self`.
    #[must_use]
    pub fn mul(&self, rhs: &Self) -> Self {
        let [ax, ay, az, aw] = self.quat_xyzw;
        let [bx, by, bz, bw] = rhs.quat_xyzw;
        Self {
            xyz: self.transform_point(rhs.xyz),
            quat_xyzw: [
                aw * bx + ax * bw + ay * bz - az * by,
                aw * by - ax * bz + ay * bw + az * bx,
                aw * bz + ax * by - ay * bx + az * bw,
                aw * bw - ax * bx - ay * by - az * bz,
            ],
        }
    }

    /// The inverse transform (the quaternion is assumed to be of unit length).
    #[must_use]
    pub fn inverse(&self) -> Self {
        let [x, y, z, w] = self.quat_xyzw;
        let inverse_rotation = Self { xyz: [0.0; 3], quat_xyzw: [-x, -y, -z, w] };
        let t = inverse_rotation.rotate(self.xyz);
        Self { xyz: [-t[0], -t[1], -t[2]], ..inverse_rotation }
    }

    /// Distance between the translations and angle between the rotations of two poses.
    pub fn error_to(&self, other: &Self) -> (f64, f64) {
        let d = [self.xyz[0] - other.xyz[0], self.xyz[1] - other.xyz[1], self.xyz[2] - other.xyz[2]];
        let position = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        let relative = self.inverse().mul(other).quat_xyzw;
        let sin_half =
            (relative[0] * relative[0] + relative[1] * relative[1] + relative[2] * relative[2]).sqrt();
        (position, 2.0 * libm::atan2(sin_half, relative[3].abs()))
    }
}

impl Default for Pose {
    fn default() -> Self {
        Self::IDENTITY
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::FRAC_PI_2;

    use super::*;

    fn close(a: [f64; 3], b: [f64; 3]) -> bool {
        a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1e-12)
    }

    #[test]
    fn rotation_follows_the_right_hand_rule() {
        let quarter_z = Pose::from_axis_angle([0.0, 0.0, 2.0], FRAC_PI_2);
        assert!(close(quarter_z.rotate([1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]));
        assert!(close(
            Pose::from_axis_angle([1.0, 0.0, 0.0], FRAC_PI_2).rotate([0.0, 1.0, 0.0]),
            [0.0, 0.0, 1.0]
        ));
        assert_eq!(Pose::from_axis_angle([0.0; 3], 1.0), Pose::IDENTITY);
    }

    #[test]
    fn composition_applies_the_right_operand_first() {
        let a = Pose { xyz: [1.0, 0.0, 0.0], ..Pose::from_axis_angle([0.0, 0.0, 1.0], FRAC_PI_2) };
        let b = Pose::from_translation([0.0, 2.0, 0.0]);
        // b moves the origin to (0, 2, 0); a rotates that to (-2, 0, 0) and shifts by (1, 0, 0).
        assert!(close(a.mul(&b).transform_point([0.0; 3]), [-1.0, 0.0, 0.0]));
        assert!(close(
            a.mul(&b).transform_point([1.0, 0.0, 0.0]),
            a.transform_point(b.transform_point([1.0, 0.0, 0.0]))
        ));
    }

    #[test]
    fn inverse_undoes_the_transform() {
        let pose = Pose { xyz: [0.3, -1.2, 2.0], ..Pose::from_axis_angle([1.0, 2.0, -0.5], 1.1) };
        let round_trip = pose.mul(&pose.inverse());
        let (position, angle) = round_trip.error_to(&Pose::IDENTITY);
        assert!(position < 1e-12 && angle < 1e-12);
        assert!(close(
            pose.inverse().transform_point(pose.transform_point([4.0, 5.0, 6.0])),
            [4.0, 5.0, 6.0]
        ));
    }

    #[test]
    fn error_measures_distance_and_angle() {
        let a = Pose::IDENTITY;
        let b = Pose { xyz: [3.0, 4.0, 0.0], ..Pose::from_axis_angle([0.0, 1.0, 0.0], 0.25) };
        let (position, angle) = a.error_to(&b);
        assert!((position - 5.0).abs() < 1e-12);
        assert!((angle - 0.25).abs() < 1e-12);
        // q and -q are the same rotation.
        let negated = Pose { quat_xyzw: b.quat_xyzw.map(|c| -c), ..b };
        assert!(b.error_to(&negated).1 < 1e-12);
    }
}
