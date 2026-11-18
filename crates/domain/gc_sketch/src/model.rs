//! The sketch data model.

use std::collections::BTreeMap;

use gc_core::{AnyId, Entity, Id, Kind};
use serde::{Deserialize, Serialize};

/// Where a sketch lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Space {
    /// In the XY plane of a frame; points use two coordinates.
    Planar(AnyId),
    /// In the 3D space of a frame; points use three coordinates.
    Spatial(AnyId),
}

/// Something outside the sketch that a point follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExternalRef {
    /// The origin of a frame (the sketch's own frame for the projected origin).
    FrameOrigin(AnyId),
    /// A point of another sketch.
    SketchPoint {
        /// The other sketch.
        sketch: AnyId,
        /// The point in it.
        point: Id<SkPoint>,
    },
    /// The source was deleted; the point stays where it last was.
    Broken,
    /// Projected geometry (Project Geometry): a point fixed in the frame of something outside the sketch —
    /// a component (`part` 0), a part or robot link of it (`part` = index + 1), or a work feature — at
    /// `local_nm` (nanometres, in that frame), projected onto the sketch plane. The document repositions it
    /// after every edit; when the source is deleted the point becomes an ordinary fixed point.
    Projected {
        /// The document item it follows.
        owner: AnyId,
        /// 0 for the item itself, otherwise the index of its part or link plus one.
        part: u32,
        /// Where in that frame, nanometres.
        local_nm: [i64; 3],
    },
}

/// Whether the solver may move a point.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PointKind {
    /// An ordinary sketch point.
    Free,
    /// A reference to outside geometry: fixed, and repositioned from its source before each solve.
    External(ExternalRef),
}

/// A sketch point.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkPoint {
    /// Position in the sketch frame (z is 0 and unused in a planar sketch).
    pub pos: [f64; 3],
    /// Free or reference.
    pub kind: PointKind,
    /// A centre point (drawn as a cross; holes use them) rather than a plain sketch point (a dot). Only
    /// standalone points carry the style; curve ends and centres do not (Inventor 2026 help, To Work with
    /// Points, GUID-D4A4288F-32F1-4E23-A382-930EBA152E87).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub center: bool,
}

impl Entity for SkPoint {
    const KIND: Kind = Kind::SketchPoint;
    fn refs(&self, _: &mut Vec<AnyId>) {}
}

/// The shape of a curve.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum CurveGeom {
    /// A segment from `a` to `b`.
    Line {
        /// Start point.
        a: Id<SkPoint>,
        /// End point.
        b: Id<SkPoint>,
    },
    /// A full circle.
    Circle {
        /// Centre point.
        c: Id<SkPoint>,
        /// Radius.
        r: f64,
    },
    /// An arc, counter-clockwise from `s` to `e` about `c`.
    Arc {
        /// Centre point.
        c: Id<SkPoint>,
        /// Start point.
        s: Id<SkPoint>,
        /// End point.
        e: Id<SkPoint>,
    },
    /// A full ellipse (Inventor 2026 help, To Create 2D Shape Geometry, Ellipse): its centre, the end of its
    /// major axis (the major radius is their distance, the axis their direction) and its minor radius.
    Ellipse {
        /// Centre point.
        c: Id<SkPoint>,
        /// End of the major axis.
        major: Id<SkPoint>,
        /// Minor radius.
        minor: f64,
        /// An elliptical arc (what a trim leaves): its start and end, counter-clockwise; `None` for the whole
        /// ellipse.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        arc: Option<(Id<SkPoint>, Id<SkPoint>)>,
    },
}

/// A curve of a sketch.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Curve {
    /// The shape.
    pub geom: CurveGeom,
    /// Construction geometry helps to constrain but is not part of the result.
    pub construction: bool,
}

impl Entity for Curve {
    const KIND: Kind = Kind::SketchCurve;
    fn refs(&self, _: &mut Vec<AnyId>) {}
}

impl Curve {
    /// The points the curve owns.
    pub fn points(&self) -> Vec<Id<SkPoint>> {
        match self.geom {
            CurveGeom::Line { a, b } => vec![a, b],
            CurveGeom::Circle { c, .. } => vec![c],
            CurveGeom::Arc { c, s, e } => vec![c, s, e],
            CurveGeom::Ellipse { c, major, arc: None, .. } => vec![c, major],
            CurveGeom::Ellipse { c, major, arc: Some((s, e)), .. } => vec![c, major, s, e],
        }
    }
}

/// A geometric constraint.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum GeoConstraint {
    /// Two points coincide.
    Coincident(Id<SkPoint>, Id<SkPoint>),
    /// A point lies on a line (extended) or circle.
    PointOnCurve(Id<SkPoint>, Id<Curve>),
    /// A point is the midpoint of a line.
    Midpoint(Id<SkPoint>, Id<Curve>),
    /// Two points are level (same V). For a horizontal line, its end points.
    Horizontal(Id<SkPoint>, Id<SkPoint>),
    /// Two points are plumb (same U). For a vertical line, its end points.
    Vertical(Id<SkPoint>, Id<SkPoint>),
    /// Two lines are parallel.
    Parallel(Id<Curve>, Id<Curve>),
    /// Two lines are perpendicular.
    Perpendicular(Id<Curve>, Id<Curve>),
    /// The second line lies on the first.
    Collinear(Id<Curve>, Id<Curve>),
    /// A line touches a circle or arc, or two circles touch.
    Tangent {
        /// First curve (the line, if one is a line).
        a: Id<Curve>,
        /// Second curve.
        b: Id<Curve>,
        /// Line and circle: `+1` when the centre is left of the line's direction. Captured at creation.
        side: f64,
        /// Two circles: true when they touch from outside. Captured at creation.
        external: bool,
    },
    /// Two lines have the same length, or two circles the same radius.
    Equal(Id<Curve>, Id<Curve>),
    /// Two circles or arcs share their centre.
    Concentric(Id<Curve>, Id<Curve>),
    /// Two points mirror each other across a line.
    Symmetric {
        /// First point.
        p: Id<SkPoint>,
        /// Second point.
        q: Id<SkPoint>,
        /// The mirror line.
        axis: Id<Curve>,
    },
    /// A point stays where it is.
    Fix(Id<SkPoint>),
    /// 3D: the segment between two points is parallel to an axis of the frame (0 = X, 1 = Y, 2 = Z).
    AlongAxis {
        /// First point.
        p: Id<SkPoint>,
        /// Second point.
        q: Id<SkPoint>,
        /// The axis.
        axis: usize,
    },
    /// 3D: a point lies on the principal plane whose normal is the given axis.
    OnPlane {
        /// The point.
        p: Id<SkPoint>,
        /// The plane's normal axis (0 = YZ plane, 1 = ZX plane, 2 = XY plane).
        axis: usize,
    },
}

impl Entity for GeoConstraint {
    const KIND: Kind = Kind::Constraint;
    fn refs(&self, _: &mut Vec<AnyId>) {}
}

/// What a dimension measures. Signs and sides are captured at creation and keep the solver on the branch
/// that was drawn.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum DimKind {
    /// Distance between two points (also the length of a line).
    Distance {
        /// First point.
        p: Id<SkPoint>,
        /// Second point.
        q: Id<SkPoint>,
    },
    /// Horizontal distance: `sign * (q.u - p.u)`.
    HDistance {
        /// First point.
        p: Id<SkPoint>,
        /// Second point.
        q: Id<SkPoint>,
        /// `+1.0` or `-1.0`.
        sign: f64,
    },
    /// Vertical distance: `sign * (q.v - p.v)`.
    VDistance {
        /// First point.
        p: Id<SkPoint>,
        /// Second point.
        q: Id<SkPoint>,
        /// `+1.0` or `-1.0`.
        sign: f64,
    },
    /// Distance of a point from a line; `side = +1` when the point is left of the line's direction.
    /// Also used between two parallel lines (with a point of the second line).
    PointLine {
        /// The point.
        p: Id<SkPoint>,
        /// The line.
        line: Id<Curve>,
        /// `+1.0` or `-1.0`.
        side: f64,
    },
    /// Angle from the first line to the second, counter-clockwise, each optionally reversed.
    Angle {
        /// First line.
        l1: Id<Curve>,
        /// Measure the first line from `b` to `a`.
        rev1: bool,
        /// Second line.
        l2: Id<Curve>,
        /// Measure the second line from `b` to `a`.
        rev2: bool,
    },
    /// Radius of a circle or arc.
    Radius(Id<Curve>),
    /// Diameter of a circle or arc.
    Diameter(Id<Curve>),
    /// 3D: distance of a point from a principal plane: `side * coordinate`.
    PlaneDistance {
        /// The point.
        p: Id<SkPoint>,
        /// The plane's normal axis.
        axis: usize,
        /// `+1.0` or `-1.0`.
        side: f64,
    },
}

/// A dimension.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Dimension {
    /// What it measures.
    pub kind: DimKind,
    /// The expression as typed, e.g. `"d0 * 2"`.
    pub source: String,
    /// The value of `source` in SI units (metres or radians). Ignored for a reference dimension.
    pub value: f64,
    /// A driving dimension constrains the sketch; a reference dimension only displays a measurement.
    pub driving: bool,
    /// Where the dimension text is placed, in sketch coordinates.
    pub text_pos: [f64; 2],
    /// Its model parameter name (`d0`, `d1`, … or one the user gave), unique in the document, by which
    /// expressions refer to it; empty until the document names it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
}

impl Entity for Dimension {
    const KIND: Kind = Kind::Dimension;
    fn refs(&self, _: &mut Vec<AnyId>) {}
}

/// A sketch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sketch {
    /// Name shown in the browser.
    pub name: String,
    /// Where the sketch lives.
    pub space: Space,
    /// Points by id.
    pub points: BTreeMap<Id<SkPoint>, SkPoint>,
    /// Curves by id.
    pub curves: BTreeMap<Id<Curve>, Curve>,
    /// Geometric constraints by id.
    pub constraints: BTreeMap<Id<GeoConstraint>, GeoConstraint>,
    /// Dimensions by id.
    pub dimensions: BTreeMap<Id<Dimension>, Dimension>,
    /// Rectangular and circular patterns, kept so they can be edited as a whole.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub patterns: Vec<crate::SketchPattern>,
}

impl Entity for Sketch {
    const KIND: Kind = Kind::Sketch;

    fn refs(&self, out: &mut Vec<AnyId>) {
        let (Space::Planar(frame) | Space::Spatial(frame)) = self.space;
        out.push(frame);
        for point in self.points.values() {
            match point.kind {
                PointKind::External(ExternalRef::FrameOrigin(frame)) => out.push(frame),
                PointKind::External(ExternalRef::SketchPoint { sketch, .. }) => out.push(sketch),
                // Projections do not hold their source: deleting it turns them into fixed points.
                PointKind::External(ExternalRef::Broken | ExternalRef::Projected { .. })
                | PointKind::Free => {}
            }
        }
    }
}

impl Sketch {
    /// An empty sketch.
    pub fn new(space: Space) -> Self {
        Self {
            name: String::new(),
            space,
            points: BTreeMap::new(),
            curves: BTreeMap::new(),
            constraints: BTreeMap::new(),
            dimensions: BTreeMap::new(),
            patterns: Vec::new(),
        }
    }

    /// True for a 3D sketch.
    pub fn is_spatial(&self) -> bool {
        matches!(self.space, Space::Spatial(_))
    }

    /// Position of a point, if it exists.
    pub fn pos(&self, point: Id<SkPoint>) -> Option<[f64; 3]> {
        self.points.get(&point).map(|p| p.pos)
    }

    /// End points of a line, if `curve` is one.
    pub fn line(&self, curve: Id<Curve>) -> Option<(Id<SkPoint>, Id<SkPoint>)> {
        match self.curves.get(&curve)?.geom {
            CurveGeom::Line { a, b } => Some((a, b)),
            _ => None,
        }
    }

    /// Points along an ellipse or elliptical arc in the sketch plane, with whether they close (see
    /// [`ellipse_points`]). `None` for another curve or a degenerate ellipse.
    pub fn ellipse_outline(&self, curve: Id<Curve>, segments: usize) -> Option<(Vec<[f64; 2]>, bool)> {
        self.ellipse_geom_outline(&self.curves.get(&curve)?.geom, segments)
    }

    /// [`Sketch::ellipse_outline`] for a curve's shape (`None` when it is not an ellipse).
    pub fn ellipse_geom_outline(&self, geom: &CurveGeom, segments: usize) -> Option<(Vec<[f64; 2]>, bool)> {
        let CurveGeom::Ellipse { c, major, minor, arc } = *geom else { return None };
        let xy = |p: Id<SkPoint>| self.pos(p).map(|q| [q[0], q[1]]);
        let ends = match arc {
            Some((s, e)) => Some((xy(s)?, xy(e)?)),
            None => None,
        };
        ellipse_points(xy(c)?, xy(major)?, minor, ends, segments)
    }

    /// An elliptical arc's start and end (counter-clockwise); `None` for a whole ellipse or another curve.
    pub fn ellipse_arc(&self, curve: Id<Curve>) -> Option<(Id<SkPoint>, Id<SkPoint>)> {
        match self.curves.get(&curve)?.geom {
            CurveGeom::Ellipse { arc, .. } => arc,
            CurveGeom::Line { .. } | CurveGeom::Circle { .. } | CurveGeom::Arc { .. } => None,
        }
    }

    /// An ellipse's centre, major axis end and minor radius; `None` for another kind of curve.
    pub fn ellipse(&self, curve: Id<Curve>) -> Option<(Id<SkPoint>, Id<SkPoint>, f64)> {
        match self.curves.get(&curve)?.geom {
            CurveGeom::Ellipse { c, major, minor, .. } => Some((c, major, minor)),
            CurveGeom::Line { .. } | CurveGeom::Circle { .. } | CurveGeom::Arc { .. } => None,
        }
    }

    /// Centre and current radius of a circle or arc, if `curve` is one.
    pub fn round(&self, curve: Id<Curve>) -> Option<(Id<SkPoint>, f64)> {
        match self.curves.get(&curve)?.geom {
            CurveGeom::Circle { c, r } => Some((c, r)),
            CurveGeom::Arc { c, s, .. } => {
                let (centre, start) = (self.pos(c)?, self.pos(s)?);
                let squares = (0..3).map(|k| (start[k] - centre[k]).powi(2)).sum::<f64>();
                Some((c, squares.sqrt()))
            }
            CurveGeom::Line { .. } | CurveGeom::Ellipse { .. } => None,
        }
    }

    /// The points a constraint refers to directly.
    pub(crate) fn constraint_points(c: &GeoConstraint) -> Vec<Id<SkPoint>> {
        match *c {
            GeoConstraint::Coincident(p, q)
            | GeoConstraint::Horizontal(p, q)
            | GeoConstraint::Vertical(p, q)
            | GeoConstraint::AlongAxis { p, q, .. }
            | GeoConstraint::Symmetric { p, q, .. } => vec![p, q],
            GeoConstraint::PointOnCurve(p, _)
            | GeoConstraint::Midpoint(p, _)
            | GeoConstraint::Fix(p)
            | GeoConstraint::OnPlane { p, .. } => vec![p],
            _ => Vec::new(),
        }
    }

    /// The curves a constraint refers to.
    pub(crate) fn constraint_curves(c: &GeoConstraint) -> Vec<Id<Curve>> {
        match *c {
            GeoConstraint::PointOnCurve(_, curve) | GeoConstraint::Midpoint(_, curve) => vec![curve],
            GeoConstraint::Parallel(a, b)
            | GeoConstraint::Perpendicular(a, b)
            | GeoConstraint::Collinear(a, b)
            | GeoConstraint::Tangent { a, b, .. }
            | GeoConstraint::Equal(a, b)
            | GeoConstraint::Concentric(a, b) => vec![a, b],
            GeoConstraint::Symmetric { axis, .. } => vec![axis],
            _ => Vec::new(),
        }
    }

    /// The points and curves a dimension refers to.
    pub(crate) fn dimension_refs(d: &DimKind) -> (Vec<Id<SkPoint>>, Vec<Id<Curve>>) {
        match *d {
            DimKind::Distance { p, q }
            | DimKind::HDistance { p, q, .. }
            | DimKind::VDistance { p, q, .. } => (vec![p, q], Vec::new()),
            DimKind::PointLine { p, line, .. } => (vec![p], vec![line]),
            DimKind::Angle { l1, l2, .. } => (Vec::new(), vec![l1, l2]),
            DimKind::Radius(curve) | DimKind::Diameter(curve) => (Vec::new(), vec![curve]),
            DimKind::PlaneDistance { p, .. } => (vec![p], Vec::new()),
        }
    }
}

/// The eccentric angle of a point seen in the ellipse's own axes (centre `c`, major axis end `m`, minor
/// radius `b`): the parameter of `c + a·cos t·u + b·sin t·v`, in `[0, 2π)`.
pub fn ellipse_param(c: [f64; 2], m: [f64; 2], b: f64, p: [f64; 2]) -> f64 {
    let (dx, dy) = (m[0] - c[0], m[1] - c[1]);
    let a = gc_math::fm::hypot(dx, dy).max(f64::MIN_POSITIVE);
    let (ux, uy) = (dx / a, dy / a);
    let (x, y) = (p[0] - c[0], p[1] - c[1]);
    let along = (x * ux + y * uy) / a;
    let across = (y * ux - x * uy) / b.abs().max(f64::MIN_POSITIVE);
    gc_math::fm::atan2(across, along).rem_euclid(std::f64::consts::TAU)
}

/// Points along the ellipse with centre `c`, major axis end `m` and minor radius `b`, for drawing and
/// picking: the whole ellipse (`segments` points, a closed loop, the first at `m`), or with `ends` the
/// arc from the first end counter-clockwise to the second (`segments + 1` points, open). The flag is
/// true for a closed loop. `None` for a degenerate ellipse or fewer than three segments.
pub fn ellipse_points(
    c: [f64; 2],
    m: [f64; 2],
    b: f64,
    ends: Option<([f64; 2], [f64; 2])>,
    segments: usize,
) -> Option<(Vec<[f64; 2]>, bool)> {
    let (dx, dy) = (m[0] - c[0], m[1] - c[1]);
    let a = gc_math::fm::hypot(dx, dy);
    if !(a > 1e-12 && a.is_finite() && b.is_finite() && segments >= 3) {
        return None;
    }
    let (ux, uy) = (dx / a, dy / a);
    let at = |t: f64| {
        let (sin, cos) = gc_math::fm::sin_cos(t);
        [c[0] + a * cos * ux - b * sin * uy, c[1] + a * cos * uy + b * sin * ux]
    };
    #[allow(clippy::cast_precision_loss, reason = "a few hundred segments")]
    let n = segments as f64;
    Some(match ends {
        None => {
            let step = std::f64::consts::TAU / n;
            #[allow(clippy::cast_precision_loss, reason = "a few hundred segments")]
            ((0..segments).map(|k| at(step * k as f64)).collect(), true)
        }
        Some((s, e)) => {
            let start = ellipse_param(c, m, b, s);
            let sweep = (ellipse_param(c, m, b, e) - start).rem_euclid(std::f64::consts::TAU);
            #[allow(clippy::cast_precision_loss, reason = "a few hundred segments")]
            ((0..=segments).map(|k| at(start + sweep * k as f64 / n)).collect(), false)
        }
    })
}
