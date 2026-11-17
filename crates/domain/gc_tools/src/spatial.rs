//! Placing points of a 3D sketch with a 2D cursor.
//!
//! A cursor is a ray, not a point. Where on the ray the new point goes is decided in this order:
//! 1. an existing point under the cursor: the new point coincides with it;
//! 2. one of the three axes through the previous point, when the cursor is on it: the new point lies on
//!    that axis (the line being drawn runs along X, Y or Z);
//! 3. the working plane through the previous point (through the origin for the first point).
//!
//! A working plane seen nearly edge-on gives meaningless positions; then nothing is placed and the user
//! switches to another plane.

use gc_core::Id;
use gc_math::{DVec3, Ray};
use gc_sketch::SkPoint;

use crate::tolerance;

/// Where a new point of a 3D sketch goes, and what that implies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement3 {
    /// The position, in the coordinates the ray was given in.
    pub pos: DVec3,
    /// The existing point it coincides with.
    pub point: Option<Id<SkPoint>>,
    /// The axis (0 = X, 1 = Y, 2 = Z) through the previous point it lies on.
    pub axis: Option<usize>,
}

/// The unit vector of an axis.
pub fn axis_dir(axis: usize) -> DVec3 {
    [DVec3::X, DVec3::Y, DVec3::Z][axis % 3]
}

/// A direction closer than this to another counts as along it (the cosine of ten degrees).
const NEARLY_PARALLEL: f64 = 0.984_807_753;

/// Decides where the cursor's ray places a point.
///
/// - `previous`: the point the line being drawn starts at.
/// - `plane_axis`: the normal of the working plane (2 = the XY plane).
/// - `points`: the existing points of the sketch.
/// - `pixels`: how far from the cursor a position appears on screen, or `None` when it is not visible.
pub fn place(
    ray: &Ray,
    previous: Option<DVec3>,
    plane_axis: usize,
    points: &[(Id<SkPoint>, DVec3)],
    pixels: &dyn Fn(DVec3) -> Option<f64>,
) -> Option<Placement3> {
    let nearest = points
        .iter()
        .filter_map(|(id, pos)| Some((pixels(*pos)?, *id, *pos)))
        .filter(|(distance, ..)| *distance <= tolerance::POINT_PX)
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    if let Some((_, id, pos)) = nearest {
        return Some(Placement3 { pos, point: Some(id), axis: None });
    }
    if let Some(start) = previous {
        let mut locked: Option<(f64, usize, DVec3)> = None;
        for axis in 0..3 {
            let along = axis_dir(axis);
            let cosine = along.dot(ray.dir);
            if cosine.abs() > NEARLY_PARALLEL {
                continue; // seen end-on, the axis is a dot: any position on it is under the cursor
            }
            // The point of the axis closest to the ray.
            let offset = start - ray.origin;
            let t = (offset.dot(ray.dir) * cosine - offset.dot(along)) / (1.0 - cosine * cosine);
            let candidate = start + along * t;
            if let Some(distance) = pixels(candidate)
                && distance <= tolerance::POINT_PX
                && t.abs() > 1e-9
                && locked.is_none_or(|(best, ..)| distance < best)
            {
                locked = Some((distance, axis, candidate));
            }
        }
        if let Some((_, axis, pos)) = locked {
            return Some(Placement3 { pos, point: None, axis: Some(axis) });
        }
    }
    let normal = axis_dir(plane_axis);
    let facing = normal.dot(ray.dir);
    // Closer than ten degrees to edge-on: refuse.
    if facing.abs() < 0.173_648_178 {
        return None;
    }
    let t = (previous.unwrap_or(DVec3::ZERO) - ray.origin).dot(normal) / facing;
    Some(Placement3 { pos: ray.origin + ray.dir * t, point: None, axis: None })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A camera looking along `dir`: positions map to pixels at 1000 px per unit, perpendicular to it.
    fn pixels_for(ray: Ray) -> impl Fn(DVec3) -> Option<f64> {
        move |p: DVec3| {
            let offset = p - ray.origin;
            Some((offset - ray.dir * offset.dot(ray.dir)).length() * 1000.0)
        }
    }

    fn ray(through: DVec3, dir: DVec3) -> Ray {
        let dir = dir.normalize();
        Ray { origin: through - dir * 5.0, dir }
    }

    #[test]
    fn an_existing_point_wins_then_an_axis_then_the_plane() {
        let view = DVec3::new(-1.0, 1.0, -1.0); // the home view
        let previous = DVec3::new(0.1, 0.0, 0.0);
        let id = |raw: u64| Id::<SkPoint>::from_raw(std::num::NonZeroU64::new(raw).unwrap());
        let points = [(id(7), DVec3::new(0.3, 0.2, 0.1))];

        // Five pixels beside an existing point: it coincides with the point.
        let cursor = ray(DVec3::new(0.3, 0.2, 0.105), view);
        let placed = place(&cursor, Some(previous), 2, &points, &pixels_for(cursor)).unwrap();
        assert_eq!((placed.pos, placed.point, placed.axis), (points[0].1, Some(id(7)), None));

        // On the vertical through the previous point: locked to Z, exactly above it.
        let cursor = ray(DVec3::new(0.1, 0.0, 0.25), view);
        let placed = place(&cursor, Some(previous), 2, &points, &pixels_for(cursor)).unwrap();
        assert_eq!((placed.point, placed.axis), (None, Some(2)));
        assert!((placed.pos - DVec3::new(0.1, 0.0, 0.25)).length() < 1e-12);

        // Three pixels off that vertical still locks; thirty pixels off falls onto the working plane.
        let cursor = ray(DVec3::new(0.103, 0.0, 0.25), view);
        let placed = place(&cursor, Some(previous), 2, &points, &pixels_for(cursor)).unwrap();
        assert_eq!(placed.axis, Some(2));
        assert!(placed.pos.x == 0.1 && placed.pos.y == 0.0);
        let cursor = ray(DVec3::new(0.2, 0.15, 0.0), view);
        let placed = place(&cursor, Some(previous), 2, &points, &pixels_for(cursor)).unwrap();
        assert_eq!((placed.point, placed.axis), (None, None));
        assert!(
            (placed.pos - DVec3::new(0.2, 0.15, 0.0)).length() < 1e-12,
            "on the XY plane through the start"
        );

        // The first point goes onto the working plane through the origin; with the YZ plane, x is 0.
        let cursor = ray(DVec3::new(0.0, 0.4, 0.3), view);
        let placed = place(&cursor, None, 0, &[], &pixels_for(cursor)).unwrap();
        assert!((placed.pos - DVec3::new(0.0, 0.4, 0.3)).length() < 1e-12);
    }

    #[test]
    fn an_edge_on_plane_places_nothing_and_an_end_on_axis_does_not_lock() {
        // Looking along Y, the XY plane is edge-on.
        let cursor = ray(DVec3::new(0.2, 0.0, 0.3), DVec3::Y);
        assert_eq!(place(&cursor, None, 2, &[], &pixels_for(cursor)), None);
        // The XZ plane faces the view: fine.
        assert!(place(&cursor, None, 1, &[], &pixels_for(cursor)).is_some());
        // The Y axis through the previous point is seen end-on: a cursor on it does not lock to Y.
        let previous = DVec3::new(0.2, 0.0, 0.3);
        let placed = place(&cursor, Some(previous), 1, &[], &pixels_for(cursor)).unwrap();
        assert_eq!(placed.axis, None);
    }
}
