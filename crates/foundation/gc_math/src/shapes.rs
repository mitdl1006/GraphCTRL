//! Planes, rays and axis-aligned boxes.

use crate::{DPose3, DVec3};

/// A ray: `origin + t * dir` for `t >= 0`, with `dir` of unit length.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    /// Start point.
    pub origin: DVec3,
    /// Unit direction.
    pub dir: DVec3,
}

impl Ray {
    /// A ray through `origin` along `dir`. Returns `None` when `dir` has no usable length.
    pub fn new(origin: DVec3, dir: DVec3) -> Option<Self> {
        dir.try_normalize().map(|dir| Self { origin, dir })
    }

    /// The point at parameter `t` (a distance, since `dir` is a unit vector).
    pub fn at(&self, t: f64) -> DVec3 {
        self.origin + self.dir * t
    }

    /// Parameter of the point of the ray's line that is closest to `point` (may be negative).
    pub fn closest_parameter(&self, point: DVec3) -> f64 {
        (point - self.origin).dot(self.dir)
    }

    /// Distance from `point` to the ray's (infinite) line.
    pub fn line_distance(&self, point: DVec3) -> f64 {
        (point - self.at(self.closest_parameter(point))).length()
    }
}

/// A plane: all points `p` with `normal · p = offset`, `normal` of unit length.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    /// Unit normal.
    pub normal: DVec3,
    /// Signed distance of the plane from the origin along the normal.
    pub offset: f64,
}

impl Plane {
    /// The plane through `point` with the given normal. Returns `None` when the normal has no usable length.
    pub fn from_point_normal(point: DVec3, normal: DVec3) -> Option<Self> {
        normal.try_normalize().map(|normal| Self { normal, offset: normal.dot(point) })
    }

    /// Distance of `point` from the plane, positive on the side the normal points to.
    pub fn signed_distance(&self, point: DVec3) -> f64 {
        self.normal.dot(point) - self.offset
    }

    /// The closest point of the plane.
    pub fn project(&self, point: DVec3) -> DVec3 {
        point - self.normal * self.signed_distance(point)
    }

    /// Ray parameter where the ray meets the plane.
    ///
    /// `None` when the ray points away from the plane or makes an angle with it whose sine is below
    /// `min_sin_angle` (a grazing ray gives a numerically meaningless hit far away).
    pub fn intersect_ray(&self, ray: &Ray, min_sin_angle: f64) -> Option<f64> {
        let approach = self.normal.dot(ray.dir);
        if approach.abs() <= min_sin_angle {
            return None;
        }
        let t = -self.signed_distance(ray.origin) / approach;
        (t >= 0.0).then_some(t)
    }
}

/// An axis-aligned box. The empty box has `min > max` and absorbs nothing until a point is added.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    /// Corner with the smallest coordinates.
    pub min: DVec3,
    /// Corner with the largest coordinates.
    pub max: DVec3,
}

impl Aabb {
    /// The box that contains nothing.
    pub const EMPTY: Self = Self { min: DVec3::INFINITY, max: DVec3::NEG_INFINITY };

    /// The smallest box containing all points.
    pub fn from_points(points: impl IntoIterator<Item = DVec3>) -> Self {
        points.into_iter().fold(Self::EMPTY, |acc, point| acc.including(point))
    }

    /// True when the box contains nothing.
    pub fn is_empty(&self) -> bool {
        self.min.cmpgt(self.max).any()
    }

    /// This box grown to contain `point`.
    #[must_use]
    pub fn including(&self, point: DVec3) -> Self {
        Self { min: self.min.min(point), max: self.max.max(point) }
    }

    /// The smallest box containing both boxes.
    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        Self { min: self.min.min(other.min), max: self.max.max(other.max) }
    }

    /// This box grown by `margin` on every side.
    #[must_use]
    pub fn inflated(&self, margin: f64) -> Self {
        if self.is_empty() { *self } else { Self { min: self.min - margin, max: self.max + margin } }
    }

    /// The centre. Meaningless for the empty box.
    pub fn center(&self) -> DVec3 {
        (self.min + self.max) * 0.5
    }

    /// Edge lengths (zero for the empty box).
    pub fn size(&self) -> DVec3 {
        if self.is_empty() { DVec3::ZERO } else { self.max - self.min }
    }

    /// Length of the space diagonal (zero for the empty box).
    pub fn diagonal(&self) -> f64 {
        self.size().length()
    }

    /// True when `point` lies inside or on the boundary.
    pub fn contains(&self, point: DVec3) -> bool {
        point.cmpge(self.min).all() && point.cmple(self.max).all()
    }

    /// True when the boxes share at least one point.
    pub fn intersects(&self, other: &Self) -> bool {
        self.min.cmple(other.max).all() && other.min.cmple(self.max).all()
    }

    /// The eight corners.
    pub fn corners(&self) -> [DVec3; 8] {
        let (a, b) = (self.min, self.max);
        [
            DVec3::new(a.x, a.y, a.z),
            DVec3::new(b.x, a.y, a.z),
            DVec3::new(a.x, b.y, a.z),
            DVec3::new(b.x, b.y, a.z),
            DVec3::new(a.x, a.y, b.z),
            DVec3::new(b.x, a.y, b.z),
            DVec3::new(a.x, b.y, b.z),
            DVec3::new(b.x, b.y, b.z),
        ]
    }

    /// The axis-aligned box around this box after moving it by `pose`.
    #[must_use]
    pub fn transformed(&self, pose: &DPose3) -> Self {
        if self.is_empty() {
            return *self;
        }
        Self::from_points(self.corners().map(|corner| pose.transform_point(corner)))
    }

    /// Entry and exit parameters of a ray through the box (slab test), clipped to `t >= 0`.
    pub fn intersect_ray(&self, ray: &Ray) -> Option<(f64, f64)> {
        let (mut enter, mut exit) = (0.0_f64, f64::INFINITY);
        for axis in 0..3 {
            let (origin, dir) = (ray.origin[axis], ray.dir[axis]);
            let (low, high) = (self.min[axis], self.max[axis]);
            if dir == 0.0 {
                if origin < low || origin > high {
                    return None;
                }
                continue;
            }
            let (t0, t1) = ((low - origin) / dir, (high - origin) / dir);
            enter = enter.max(t0.min(t1));
            exit = exit.min(t0.max(t1));
        }
        (enter <= exit).then_some((enter, exit))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quat_from_rpy;

    #[test]
    fn ray_needs_a_direction_and_measures_distances() {
        assert!(Ray::new(DVec3::ZERO, DVec3::ZERO).is_none());
        let ray = Ray::new(DVec3::ZERO, DVec3::new(0.0, 0.0, 5.0)).unwrap();
        assert_eq!(ray.at(2.0), DVec3::new(0.0, 0.0, 2.0));
        assert_eq!(ray.closest_parameter(DVec3::new(3.0, 4.0, 7.0)), 7.0);
        assert!((ray.line_distance(DVec3::new(3.0, 4.0, 7.0)) - 5.0).abs() < 1e-15);
    }

    #[test]
    fn plane_distance_projection_and_ray_hit() {
        let plane = Plane::from_point_normal(DVec3::new(0.0, 0.0, 2.0), DVec3::new(0.0, 0.0, 3.0)).unwrap();
        assert_eq!(plane.signed_distance(DVec3::new(1.0, 1.0, 5.0)), 3.0);
        assert_eq!(plane.project(DVec3::new(1.0, 1.0, 5.0)), DVec3::new(1.0, 1.0, 2.0));
        let down = Ray::new(DVec3::new(1.0, 1.0, 5.0), -DVec3::Z).unwrap();
        assert_eq!(plane.intersect_ray(&down, 1e-9), Some(3.0));
        let up = Ray::new(DVec3::new(1.0, 1.0, 5.0), DVec3::Z).unwrap();
        assert_eq!(plane.intersect_ray(&up, 1e-9), None);
        let grazing = Ray::new(DVec3::new(0.0, 0.0, 5.0), DVec3::new(1.0, 0.0, -0.01)).unwrap();
        assert!(plane.intersect_ray(&grazing, 1e-9).is_some());
        assert_eq!(plane.intersect_ray(&grazing, 0.17), None, "below 10 degrees is rejected");
    }

    #[test]
    fn empty_box_behaves_as_the_identity_of_union() {
        let empty = Aabb::EMPTY;
        assert!(empty.is_empty());
        assert_eq!(empty.diagonal(), 0.0);
        let unit = Aabb::from_points([DVec3::ZERO, DVec3::ONE]);
        assert_eq!(empty.union(&unit), unit);
        assert!(!empty.intersects(&unit));
        assert!(empty.inflated(1.0).is_empty());
    }

    #[test]
    fn box_queries() {
        let a = Aabb::from_points([DVec3::new(1.0, -1.0, 0.0), DVec3::new(-1.0, 1.0, 2.0)]);
        assert_eq!(a.min, DVec3::new(-1.0, -1.0, 0.0));
        assert_eq!(a.center(), DVec3::new(0.0, 0.0, 1.0));
        assert!(a.contains(DVec3::new(1.0, 1.0, 2.0)));
        assert!(!a.contains(DVec3::new(1.0, 1.0, 2.1)));
        let touching = Aabb::from_points([DVec3::new(1.0, 0.0, 0.0), DVec3::new(2.0, 1.0, 1.0)]);
        assert!(a.intersects(&touching));
        assert!(!a.intersects(&touching.inflated(-0.0).transformed(&DPose3::translation(0.5, 0.0, 0.0))));
    }

    #[test]
    fn transformed_box_contains_the_rotated_corners() {
        let unit = Aabb::from_points([DVec3::ZERO, DVec3::ONE]);
        let pose = DPose3::from_parts(
            DVec3::new(10.0, 0.0, 0.0),
            quat_from_rpy(0.0, 0.0, std::f64::consts::FRAC_PI_4),
        );
        let moved = unit.transformed(&pose);
        assert!((moved.size().x - std::f64::consts::SQRT_2).abs() < 1e-12);
        assert!((moved.size().z - 1.0).abs() < 1e-12);
        assert!(moved.inflated(1e-12).contains(pose.transform_point(DVec3::ONE)));
    }

    #[test]
    fn ray_box_slab_test() {
        let unit = Aabb::from_points([DVec3::ZERO, DVec3::ONE]);
        let hit = Ray::new(DVec3::new(0.5, 0.5, 3.0), -DVec3::Z).unwrap();
        assert_eq!(unit.intersect_ray(&hit), Some((2.0, 3.0)));
        let inside = Ray::new(DVec3::splat(0.5), DVec3::X).unwrap();
        assert_eq!(unit.intersect_ray(&inside), Some((0.0, 0.5)));
        let miss = Ray::new(DVec3::new(2.0, 0.5, 3.0), -DVec3::Z).unwrap();
        assert_eq!(unit.intersect_ray(&miss), None);
        let behind = Ray::new(DVec3::new(0.5, 0.5, 3.0), DVec3::Z).unwrap();
        assert_eq!(unit.intersect_ray(&behind), None);
    }
}
