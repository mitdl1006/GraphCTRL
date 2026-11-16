//! Rotation conventions used by robot description files and robot controllers.

use crate::{DMat3, DPose3, DQuat, DVec3, fm};

/// Rotation from URDF roll-pitch-yaw angles: `R = Rz(yaw) * Ry(pitch) * Rx(roll)` (fixed-axis XYZ).
pub fn quat_from_rpy(roll: f64, pitch: f64, yaw: f64) -> DQuat {
    let (sr, cr) = fm::sin_cos(roll * 0.5);
    let (sp, cp) = fm::sin_cos(pitch * 0.5);
    let (sy, cy) = fm::sin_cos(yaw * 0.5);
    DQuat::from_xyzw(
        sr * cp * cy - cr * sp * sy,
        cr * sp * cy + sr * cp * sy,
        cr * cp * sy - sr * sp * cy,
        cr * cp * cy + sr * sp * sy,
    )
}

/// Roll, pitch and yaw of a rotation, with pitch in `[-pi/2, pi/2]`.
///
/// At pitch = ±90° roll and yaw are not separable; roll is then reported as zero.
pub fn rpy_from_quat(rotation: DQuat) -> [f64; 3] {
    let m = DMat3::from_quat(rotation.normalize());
    // `m.x_axis` is the first column, so `m.x_axis.z` is row 2, column 0.
    let sin_pitch = -m.x_axis.z;
    if sin_pitch.abs() < 1.0 - 1e-12 {
        [fm::atan2(m.y_axis.z, m.z_axis.z), fm::asin(sin_pitch), fm::atan2(m.x_axis.y, m.x_axis.x)]
    } else {
        [0.0, fm::asin_clamped(sin_pitch), fm::atan2(-m.y_axis.x, m.y_axis.y)]
    }
}

/// Rotation from a rotation vector (axis times angle), the orientation format of UR controllers.
pub fn quat_from_rotvec(rotvec: DVec3) -> DQuat {
    let angle = rotvec.length();
    if angle < 1e-12 {
        // First-order expansion: sin(a/2) * axis = rotvec / 2.
        return DQuat::from_xyzw(rotvec.x * 0.5, rotvec.y * 0.5, rotvec.z * 0.5, 1.0).normalize();
    }
    let (s, c) = fm::sin_cos(angle * 0.5);
    let axis = rotvec / angle;
    DQuat::from_xyzw(axis.x * s, axis.y * s, axis.z * s, c)
}

/// Rotation vector (axis times angle) of a rotation, with the angle in `[0, pi]`.
pub fn rotvec_from_quat(rotation: DQuat) -> DVec3 {
    let q = rotation.normalize();
    // q and -q are the same rotation; pick the one with the shorter angle.
    let (v, w) =
        if q.w < 0.0 { (-DVec3::new(q.x, q.y, q.z), -q.w) } else { (DVec3::new(q.x, q.y, q.z), q.w) };
    let sin_half = v.length();
    if sin_half < 1e-12 {
        return v * 2.0;
    }
    let angle = 2.0 * fm::atan2(sin_half, w);
    v * (angle / sin_half)
}

/// A pose from a position and URDF roll-pitch-yaw angles (`<origin xyz="..." rpy="..."/>`).
pub fn pose_from_xyz_rpy(xyz: [f64; 3], rpy: [f64; 3]) -> DPose3 {
    DPose3::from_parts(DVec3::from_array(xyz), quat_from_rpy(rpy[0], rpy[1], rpy[2]))
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, PI};

    use super::*;

    fn close(a: DVec3, b: DVec3) -> bool {
        (a - b).length() < 1e-12
    }

    fn same_rotation(a: DQuat, b: DQuat) -> bool {
        a.dot(b).abs() > 1.0 - 1e-12
    }

    #[test]
    fn rpy_is_yaw_times_pitch_times_roll() {
        let (roll, pitch, yaw) = (0.3, -0.7, 1.9);
        let composed =
            DQuat::from_rotation_z(yaw) * DQuat::from_rotation_y(pitch) * DQuat::from_rotation_x(roll);
        assert!(same_rotation(quat_from_rpy(roll, pitch, yaw), composed));
    }

    #[test]
    fn rpy_matches_urdf_examples() {
        // rpy = (pi/2, 0, 0) turns +Y into +Z.
        assert!(close(quat_from_rpy(FRAC_PI_2, 0.0, 0.0) * DVec3::Y, DVec3::Z));
        // rpy = (0, 0, pi) flips X and Y (the UR base_link -> base joint).
        assert!(close(quat_from_rpy(0.0, 0.0, PI) * DVec3::X, -DVec3::X));
        // rpy = (pi/2, pi, pi): the UR wrist_3 joint origin.
        let q = quat_from_rpy(FRAC_PI_2, PI, PI);
        assert!(close(q * DVec3::X, DVec3::X));
        assert!(close(q * DVec3::Y, -DVec3::Z));
    }

    #[test]
    fn rpy_round_trips() {
        for (roll, pitch, yaw) in [(0.0, 0.0, 0.0), (0.3, -0.7, 1.9), (-3.0, 1.5, -2.5), (1.0, -1.2, 3.1)] {
            let q = quat_from_rpy(roll, pitch, yaw);
            let [r, p, y] = rpy_from_quat(q);
            assert!(same_rotation(quat_from_rpy(r, p, y), q), "{roll} {pitch} {yaw}");
            assert!((r - roll).abs() < 1e-12 && (p - pitch).abs() < 1e-12 && (y - yaw).abs() < 1e-12);
        }
    }

    #[test]
    fn rpy_at_gimbal_lock_still_reproduces_the_rotation() {
        for pitch in [FRAC_PI_2, -FRAC_PI_2] {
            let q = quat_from_rpy(0.4, pitch, -1.1);
            let [r, p, y] = rpy_from_quat(q);
            assert_eq!(r, 0.0);
            assert!((p - pitch).abs() < 1e-6);
            assert!(quat_from_rpy(r, p, y).dot(q).abs() > 1.0 - 1e-9);
        }
    }

    #[test]
    fn rotvec_round_trips_including_tiny_and_half_turn_angles() {
        let axis = DVec3::new(1.0, -2.0, 0.5).normalize();
        for angle in [0.0, 1e-14, 1e-6, 0.5, 2.0, PI - 1e-9] {
            let rotvec = axis * angle;
            let back = rotvec_from_quat(quat_from_rotvec(rotvec));
            assert!((back - rotvec).length() < 1e-9, "angle {angle}: {back:?}");
        }
        // At exactly half a turn both axis directions describe the same rotation.
        let half_turn = quat_from_rotvec(axis * PI);
        let back = rotvec_from_quat(half_turn);
        assert!((back.length() - PI).abs() < 1e-12);
        assert!(same_rotation(quat_from_rotvec(back), half_turn));
    }

    #[test]
    fn rotvec_angle_never_exceeds_half_a_turn() {
        let q = DQuat::from_axis_angle(DVec3::Z, 1.5 * PI);
        let rotvec = rotvec_from_quat(q);
        assert!((rotvec.length() - 0.5 * PI).abs() < 1e-12);
        assert!(rotvec.z < 0.0, "270 degrees about +Z is 90 degrees about -Z");
        assert!(same_rotation(quat_from_rotvec(rotvec), q));
    }

    #[test]
    fn pose_from_urdf_origin() {
        let pose = pose_from_xyz_rpy([1.0, 2.0, 3.0], [0.0, 0.0, FRAC_PI_2]);
        assert!(close(pose.transform_point(DVec3::X), DVec3::new(1.0, 3.0, 3.0)));
    }
}
