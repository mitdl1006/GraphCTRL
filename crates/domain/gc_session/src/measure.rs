//! Measure in the cell, as Inventor's Measure command does in an assembly (Tools tab > Measure panel >
//! Measure; https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-1805AB8F-F782-4B24-A332-B4F2E10A3D00.htm):
//! with component priority, two components give their minimum distance; a point gives its position,
//! two points the distance and the delta; two directed items the angle between them.
//!
//! Distances use the collision shapes, so a measured gap and the collision check agree.

use gc_collide::{Closest, Geometry};
use gc_math::{DPose3, DVec3};
use gc_motion::RobotScene;

use crate::Session;
use crate::cell::{CellItem, PADDING, pose_of, shape_of, tool_shapes};

/// What one selected item measures on its own.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemMeasure {
    /// The item's name.
    pub name: String,
    /// Its origin in the world (metres): a robot's base, a body's placement, a waypoint's tool centre
    /// point, a frame's origin.
    pub position: [f64; 3],
}

/// What two selected items measure together.
#[derive(Clone, Debug, PartialEq)]
pub struct PairMeasure {
    /// The minimum distance between their shapes and where it is; `None` when one has no shapes (a
    /// path) or they could not be measured.
    pub minimum: Option<Closest>,
    /// The maximum distance between their shapes and where it is (Inventor's Maximum Distance).
    pub maximum: Option<Closest>,
    /// The distance between their origins (Inventor's Center to Center).
    pub centre_to_centre: f64,
    /// The second origin minus the first, per world axis.
    pub delta: [f64; 3],
    /// The angle between their Z axes (radians), when both have a direction: frames, waypoints (the
    /// tool axis), robots and bodies (their placement).
    pub angle: Option<f64>,
}

impl Session {
    /// The pose of an item as it is shown; `None` for a path.
    fn item_pose(&self, item: CellItem) -> Option<DPose3> {
        match item {
            CellItem::Body(id) => self.shown_body(id).map(|b| pose_of(&b.placement)),
            CellItem::Robot(id) => self.shown_base(id).map(|p| pose_of(&p)),
            CellItem::Waypoint(id) => self.waypoint_pose(id),
            CellItem::Frame(id) => self.work_frame_pose(id),
            CellItem::Path(_) => None,
        }
    }

    /// The shapes of an item as it is shown, in the world: a body's solid, a robot's links and tool at
    /// its shown joint values, a waypoint or frame as its origin point. `None` for a path, or when a mesh
    /// is missing.
    pub fn item_geometry(&self, item: CellItem) -> Option<Geometry> {
        match item {
            CellItem::Body(id) => {
                let body = self.shown_body(id)?;
                let shape = shape_of(&body.solid, &self.cell.meshes)?;
                Geometry::from_shapes(&[(pose_of(&body.placement), shape)]).ok()
            }
            CellItem::Robot(id) => {
                // A suppressed robot takes no part in measuring or interference.
                if self.doc().is_component_suppressed(id.erase()) {
                    return None;
                }
                let loaded = self.robot_model(id)?;
                let stored = self.doc().get(id)?;
                let base = self.shown_base(id)?;
                let tool = self.shown_tool(id)?;
                let (scene, _) = RobotScene::new(
                    std::sync::Arc::clone(&loaded.model),
                    loaded.chain.clone(),
                    &stored.tip_link,
                    pose_of(&base),
                    &[],
                    PADDING,
                    &|uri| loaded.file(uri),
                    &tool_shapes(tool.as_ref(), &self.cell.meshes).unwrap_or_default(),
                )
                .ok()?;
                Some(scene.geometry(&self.robot_joints(id)?))
            }
            CellItem::Waypoint(_) | CellItem::Frame(_) => {
                self.item_pose(item).map(|pose| Geometry::point(pose.translation.to_array()))
            }
            CellItem::Path(_) => None,
        }
    }

    /// What a selected item measures on its own; `None` for a path.
    pub fn measure_item(&self, item: CellItem) -> Option<ItemMeasure> {
        let pose = self.item_pose(item)?;
        Some(ItemMeasure {
            name: self.item_name(item).unwrap_or_default(),
            position: pose.translation.to_array(),
        })
    }

    /// What two selected items measure together; `None` when either is a path or gone.
    pub fn measure_pair(&self, first: CellItem, second: CellItem) -> Option<PairMeasure> {
        let (a, b) = (self.item_pose(first)?, self.item_pose(second)?);
        let (minimum, maximum) = match (self.item_geometry(first), self.item_geometry(second)) {
            (Some(ga), Some(gb)) => (ga.closest(&gb).ok().flatten(), ga.farthest(&gb)),
            _ => (None, None),
        };
        let delta = b.translation - a.translation;
        let (za, zb) = (a.rotation * DVec3::Z, b.rotation * DVec3::Z);
        let angle = gc_math::fm::atan2(za.cross(zb).length(), za.dot(zb));
        Some(PairMeasure {
            minimum,
            maximum,
            centre_to_centre: delta.length(),
            delta: delta.to_array(),
            angle: Some(angle),
        })
    }
}

/// A piece of geometry picked with the Edges and Faces priority (world frame, metres).
#[derive(Clone, Debug, PartialEq)]
pub enum Feature {
    /// A point: a corner, or a point on a surface.
    Point([f64; 3]),
    /// A straight edge between two corners.
    Edge([f64; 3], [f64; 3]),
    /// A cylindrical face: the middle of its axis, the unit axis, the radius and the length.
    Cylinder {
        /// The middle of the axis.
        centre: [f64; 3],
        /// The unit axis.
        axis: [f64; 3],
        /// The radius.
        radius: f64,
        /// The length along the axis.
        length: f64,
    },
    /// A flat face: a point of its plane, its unit normal and its area.
    Face {
        /// A point of the plane.
        point: [f64; 3],
        /// The unit normal.
        normal: [f64; 3],
        /// The area.
        area: f64,
    },
}

/// What two features measure together.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FeaturePair {
    /// The smallest distance, when it is defined (a face pair only when the faces are parallel).
    pub distance: Option<f64>,
    /// The nearest points (on the first, on the second), when the distance is defined.
    pub closest: Option<([f64; 3], [f64; 3])>,
    /// The second point minus the first, for two points.
    pub delta: Option<[f64; 3]>,
    /// With a cylinder: the distance measured to its axis (Inventor's Center to Center).
    pub centre_distance: Option<f64>,
    /// The angle between two directed features (radians): lines 0–90°, a line and a plane 0–90°, two
    /// planes between their normals 0–180°.
    pub angle: Option<f64>,
}

/// Directions closer than this (sine of the angle) count as parallel.
const PARALLEL: f64 = 1e-9;

fn v(p: [f64; 3]) -> DVec3 {
    DVec3::from_array(p)
}

fn angle_between(a: DVec3, b: DVec3) -> f64 {
    gc_math::fm::atan2(a.cross(b).length(), a.dot(b))
}

/// The nearest point to `p` on the segment `a`–`b`.
fn on_segment(p: DVec3, a: DVec3, b: DVec3) -> DVec3 {
    let d = b - a;
    let len2 = d.length_squared();
    if len2 <= 0.0 {
        return a;
    }
    a + d * ((p - a).dot(d) / len2).clamp(0.0, 1.0)
}

/// The nearest points of two segments (Ericson, Real-Time Collision Detection, 5.1.9).
fn segments(p1: DVec3, q1: DVec3, p2: DVec3, q2: DVec3) -> (DVec3, DVec3) {
    let (d1, d2, r) = (q1 - p1, q2 - p2, p1 - p2);
    let (a, e, f) = (d1.length_squared(), d2.length_squared(), d2.dot(r));
    if a <= f64::EPSILON && e <= f64::EPSILON {
        return (p1, p2);
    }
    let (s, t) = if a <= f64::EPSILON {
        (0.0, (f / e).clamp(0.0, 1.0))
    } else {
        let c = d1.dot(r);
        if e <= f64::EPSILON {
            ((-c / a).clamp(0.0, 1.0), 0.0)
        } else {
            let b = d1.dot(d2);
            let denom = a * e - b * b;
            let mut s = if denom > 0.0 { ((b * f - c * e) / denom).clamp(0.0, 1.0) } else { 0.0 };
            let mut t = (b * s + f) / e;
            if t < 0.0 {
                t = 0.0;
                s = (-c / a).clamp(0.0, 1.0);
            } else if t > 1.0 {
                t = 1.0;
                s = ((b - c) / a).clamp(0.0, 1.0);
            }
            (s, t)
        }
    };
    (p1 + d1 * s, p2 + d2 * t)
}

/// A cylinder's axis as a segment, its radius, and the feature it is measured against.
fn with_cylinder(centre: [f64; 3], axis: [f64; 3], radius: f64, length: f64, other: &Feature) -> FeaturePair {
    let (c, a) = (v(centre), v(axis));
    let half = a * (length / 2.0).max(1e-9);
    let (lo, hi) = (c - half, c + half);
    // The distance to the axis, and the gap to the surface.
    let gap = |centre_distance: f64, other_radius: f64| (centre_distance - radius - other_radius).max(0.0);
    // Nearest points on the surfaces: each axis point moved toward the other by its radius (not past it,
    // so overlapping surfaces meet at one point and agree with a gap of 0).
    let surfaces = |on_axis: DVec3, other: DVec3, other_radius: f64| {
        let d = on_axis.distance(other);
        let toward = (other - on_axis).try_normalize().unwrap_or(DVec3::ZERO);
        if d <= radius + other_radius {
            let meet = on_axis + toward * (d * radius / (radius + other_radius).max(f64::MIN_POSITIVE));
            (meet, meet)
        } else {
            (on_axis + toward * radius, other - toward * other_radius)
        }
    };
    match other {
        Feature::Point(p) => {
            let foot = on_segment(v(*p), lo, hi);
            let d = v(*p).distance(foot);
            let on_surface = foot + (v(*p) - foot).try_normalize().unwrap_or(DVec3::ZERO) * radius;
            FeaturePair {
                distance: Some((d - radius).abs()),
                closest: Some((on_surface.to_array(), *p)),
                centre_distance: Some(d),
                ..FeaturePair::default()
            }
        }
        Feature::Edge(e0, e1) => {
            let (on_axis, on_edge) = segments(lo, hi, v(*e0), v(*e1));
            let d = on_axis.distance(on_edge);
            let angle = angle_between(a, v(*e1) - v(*e0));
            let (p, q) = surfaces(on_axis, on_edge, 0.0);
            FeaturePair {
                distance: Some(gap(d, 0.0)),
                closest: Some((p.to_array(), q.to_array())),
                centre_distance: Some(d),
                angle: Some(angle.min(std::f64::consts::PI - angle)),
                ..FeaturePair::default()
            }
        }
        Feature::Face { point, normal, .. } => {
            let n = v(*normal);
            let angle = (std::f64::consts::FRAC_PI_2 - angle_between(a, n)).abs();
            let mut pair = FeaturePair { angle: Some(angle), ..FeaturePair::default() };
            if a.dot(n).abs() <= PARALLEL.max(1e-6) {
                let d = (c - v(*point)).dot(n).abs();
                pair.centre_distance = Some(d);
                pair.distance = Some(gap(d, 0.0));
                let (p, q) = surfaces(c, c - n * (c - v(*point)).dot(n), 0.0);
                pair.closest = Some((p.to_array(), q.to_array()));
            }
            pair
        }
        Feature::Cylinder { centre: c2, axis: a2, radius: r2, length: l2 } => {
            let half2 = v(*a2) * (l2 / 2.0).max(1e-9);
            let (on1, on2) = segments(lo, hi, v(*c2) - half2, v(*c2) + half2);
            let d = on1.distance(on2);
            let angle = angle_between(a, v(*a2));
            let (p, q) = surfaces(on1, on2, *r2);
            FeaturePair {
                distance: Some(gap(d, *r2)),
                closest: Some((p.to_array(), q.to_array())),
                centre_distance: Some(d),
                angle: Some(angle.min(std::f64::consts::PI - angle)),
                ..FeaturePair::default()
            }
        }
    }
}

/// The angle at `vertex` between the directions to `a` and to `c` (Inventor: two points, then a third
/// with Shift); `None` when a point coincides with the vertex or a value is not finite.
pub fn angle_at(a: [f64; 3], vertex: [f64; 3], c: [f64; 3]) -> Option<f64> {
    let (u, w) = (v(a) - v(vertex), v(c) - v(vertex));
    let usable = |d: DVec3| d.is_finite() && d.length() > 1e-12;
    (usable(u) && usable(w)).then(|| angle_between(u, w))
}

/// What two picked features measure together (Inventor's Measure with Edges and Faces priority).
pub fn measure_features(first: &Feature, second: &Feature) -> FeaturePair {
    if let Feature::Cylinder { centre, axis, radius, length } = first {
        return with_cylinder(*centre, *axis, *radius, *length, second);
    }
    if let Feature::Cylinder { centre, axis, radius, length } = second {
        let mut pair = with_cylinder(*centre, *axis, *radius, *length, first);
        pair.closest = pair.closest.map(|(a, b)| (b, a));
        return pair;
    }
    let with_points = |a: DVec3, b: DVec3| FeaturePair {
        distance: Some(a.distance(b)),
        closest: Some((a.to_array(), b.to_array())),
        ..FeaturePair::default()
    };
    let plane_point = |p: DVec3, point: DVec3, n: DVec3| p - n * (p - point).dot(n);
    match (first, second) {
        (Feature::Point(a), Feature::Point(b)) => {
            FeaturePair { delta: Some((v(*b) - v(*a)).to_array()), ..with_points(v(*a), v(*b)) }
        }
        (Feature::Point(p), Feature::Edge(a, b)) => with_points(v(*p), on_segment(v(*p), v(*a), v(*b))),
        (Feature::Edge(a, b), Feature::Point(p)) => with_points(on_segment(v(*p), v(*a), v(*b)), v(*p)),
        (Feature::Point(p), Feature::Face { point, normal, .. }) => {
            with_points(v(*p), plane_point(v(*p), v(*point), v(*normal)))
        }
        (Feature::Face { point, normal, .. }, Feature::Point(p)) => {
            with_points(plane_point(v(*p), v(*point), v(*normal)), v(*p))
        }
        (Feature::Edge(a1, b1), Feature::Edge(a2, b2)) => {
            let (on1, on2) = segments(v(*a1), v(*b1), v(*a2), v(*b2));
            let angle = angle_between(v(*b1) - v(*a1), v(*b2) - v(*a2));
            FeaturePair { angle: Some(angle.min(std::f64::consts::PI - angle)), ..with_points(on1, on2) }
        }
        (Feature::Edge(a, b), Feature::Face { point, normal, .. })
        | (Feature::Face { point, normal, .. }, Feature::Edge(a, b)) => {
            let (d, n) = (v(*b) - v(*a), v(*normal));
            let to_normal = angle_between(d, n);
            let angle = (std::f64::consts::FRAC_PI_2 - to_normal).abs();
            // A line parallel to the plane has a distance to it.
            let parallel = d.try_normalize().is_some_and(|d| d.dot(n).abs() <= PARALLEL);
            let mut pair = FeaturePair { angle: Some(angle), ..FeaturePair::default() };
            if parallel {
                let on_line = v(*a);
                let on_plane = plane_point(on_line, v(*point), n);
                pair.distance = Some(on_line.distance(on_plane));
                pair.closest = Some(if matches!(first, Feature::Edge(..)) {
                    (on_line.to_array(), on_plane.to_array())
                } else {
                    (on_plane.to_array(), on_line.to_array())
                });
            }
            pair
        }
        // Cylinders were measured above.
        (Feature::Cylinder { .. }, _) | (_, Feature::Cylinder { .. }) => FeaturePair::default(),
        (Feature::Face { point: p1, normal: n1, .. }, Feature::Face { point: p2, normal: n2, .. }) => {
            let (n1, n2) = (v(*n1), v(*n2));
            let mut pair = FeaturePair { angle: Some(angle_between(n1, n2)), ..FeaturePair::default() };
            if n1.cross(n2).length() <= PARALLEL {
                let foot = plane_point(v(*p1), v(*p2), n2);
                pair.distance = Some(v(*p1).distance(foot));
                pair.closest = Some((p1.to_owned(), foot.to_array()));
            }
            pair
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UP: [f64; 3] = [0.0, 0.0, 1.0];

    #[test]
    fn features_measure_like_inventor() {
        let p = Feature::Point([0.0, 0.0, 0.0]);
        let q = Feature::Point([3.0, 4.0, 0.0]);
        let both = measure_features(&p, &q);
        assert!((both.distance.unwrap() - 5.0).abs() < 1e-12);
        assert_eq!(both.delta, Some([3.0, 4.0, 0.0]));

        let edge = Feature::Edge([1.0, -1.0, 2.0], [1.0, 1.0, 2.0]);
        assert!((measure_features(&p, &edge).distance.unwrap() - 5.0_f64.sqrt()).abs() < 1e-12);

        let floor = Feature::Face { point: [0.0, 0.0, 0.0], normal: UP, area: 1.0 };
        let top = Feature::Face { point: [5.0, 5.0, 0.3], normal: [0.0, 0.0, -1.0], area: 1.0 };
        let faces = measure_features(&floor, &top);
        assert!((faces.distance.unwrap() - 0.3).abs() < 1e-12);
        assert!((faces.angle.unwrap() - std::f64::consts::PI).abs() < 1e-12);
        let wall = Feature::Face { point: [0.0; 3], normal: [1.0, 0.0, 0.0], area: 1.0 };
        let tilted = measure_features(&floor, &wall);
        assert!(tilted.distance.is_none());
        assert!((tilted.angle.unwrap() - std::f64::consts::FRAC_PI_2).abs() < 1e-12);

        // An edge parallel to the floor: angle 0 and its height; a vertical edge: 90°, no distance.
        let level = measure_features(&edge, &floor);
        assert!(level.angle.unwrap().abs() < 1e-12);
        assert!((level.distance.unwrap() - 2.0).abs() < 1e-12);
        let post = Feature::Edge([0.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
        let upright = measure_features(&post, &floor);
        assert!((upright.angle.unwrap() - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert!(upright.distance.is_none());

        // Two skew edges: nearest points and the acute angle.
        let across = Feature::Edge([-1.0, 0.0, 1.0], [1.0, 0.0, 1.0]);
        let skew = measure_features(&post, &Feature::Edge([-1.0, 0.0, 3.0], [1.0, 0.0, 3.0]));
        assert!((skew.distance.unwrap() - 2.0).abs() < 1e-12);
        let crossing = measure_features(&post, &across);
        assert!(crossing.distance.unwrap().abs() < 1e-12);
        assert!((crossing.angle.unwrap() - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        // A cylinder of radius 0.05 along Z at the origin: a point 0.3 away on X is 0.25 from its surface;
        // a parallel cylinder of radius 0.02 at 0.3 is 0.23 from it, 0.3 centre to centre.
        let shaft = Feature::Cylinder { centre: [0.0; 3], axis: UP, radius: 0.05, length: 1.0 };
        let near = measure_features(&shaft, &Feature::Point([0.3, 0.0, 0.0]));
        assert!(
            (near.distance.unwrap() - 0.25).abs() < 1e-12
                && (near.centre_distance.unwrap() - 0.3).abs() < 1e-12
        );
        let pin =
            Feature::Cylinder { centre: [0.3, 0.0, 0.2], axis: [0.0, 0.0, -1.0], radius: 0.02, length: 0.5 };
        let pair = measure_features(&pin, &shaft);
        assert!((pair.distance.unwrap() - 0.23).abs() < 1e-12, "{pair:?}");
        assert!(pair.angle.unwrap().abs() < 1e-12);
        let side = Feature::Face { point: [0.5, 0.0, 0.0], normal: [1.0, 0.0, 0.0], area: 1.0 };
        let to_wall = measure_features(&shaft, &side);
        assert!((to_wall.distance.unwrap() - 0.45).abs() < 1e-12);
        let reversed = measure_features(
            &Feature::Edge([0.0; 3], [1.0, 0.0, 0.0]),
            &Feature::Edge([0.0; 3], [-1.0, 0.1, 0.0]),
        );
        assert!(reversed.angle.unwrap() < 0.11);
    }
}

/// One interference found by [`Session::interference`].
#[derive(Clone, Debug, PartialEq)]
pub struct InterferenceRow {
    /// The first component.
    pub first: CellItem,
    /// The second component.
    pub second: CellItem,
    /// The overlap: volume, centroid and sample points.
    pub overlap: gc_collide::Interference,
}

/// Grid samples per axis when estimating an overlap's volume.
const INTERFERENCE_CELLS: usize = 24;

impl Session {
    /// Analyse interference as Inventor does (Inspect tab > Interference panel > Analyze Interference;
    /// https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-80DED0A6-AC65-4C7C-AD85-0144EE6DAB5B.htm):
    /// every component of `first` against every component of `second`, or, when `second` is empty, the
    /// components of `first` among themselves. A robot counts as one component. Volumes are estimated
    /// on a grid over each overlap.
    pub fn interference(&self, first: &[CellItem], second: &[CellItem]) -> Vec<InterferenceRow> {
        let geometry: std::collections::BTreeMap<CellItem, Geometry> =
            first.iter().chain(second).filter_map(|item| Some((*item, self.item_geometry(*item)?))).collect();
        // A component listed twice counts once; it never interferes with itself.
        let unique = |set: &[CellItem]| {
            let mut seen = std::collections::BTreeSet::new();
            set.iter().copied().filter(|item| seen.insert(*item)).collect::<Vec<_>>()
        };
        let (first, second) = (unique(first), unique(second));
        let (first, second) = (first.as_slice(), second.as_slice());
        let mut pairs = Vec::new();
        if second.is_empty() {
            for (i, a) in first.iter().enumerate() {
                pairs.extend(first.iter().skip(i + 1).map(|b| (*a, *b)));
            }
        } else {
            for a in first {
                pairs.extend(second.iter().filter(|b| *b != a).map(|b| (*a, *b)));
            }
        }
        pairs
            .into_iter()
            .filter_map(|(a, b)| {
                let overlap =
                    geometry.get(&a)?.interference(geometry.get(&b)?, INTERFERENCE_CELLS).ok()??;
                Some(InterferenceRow { first: a, second: b, overlap })
            })
            .collect()
    }
}
