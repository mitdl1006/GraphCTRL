//! The constraints the solver understands, in terms of parameters.

use crate::ParamId;
use crate::rows::RowKind;

/// A 2D point: its x and y parameters.
pub type P2 = [ParamId; 2];
/// A 3D point: its x, y and z parameters.
pub type P3 = [ParamId; 3];

/// A line segment through two 2D points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Line2 {
    /// Start point.
    pub a: P2,
    /// End point.
    pub b: P2,
}

/// A line segment through two 3D points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Line3 {
    /// Start point.
    pub a: P3,
    /// End point.
    pub b: P3,
}

/// A circle, or the circle an arc lies on: centre and radius parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Round {
    /// Centre.
    pub c: P2,
    /// Radius (a length parameter).
    pub r: ParamId,
}

/// A geometric constraint or a driving dimension.
///
/// Sides and signs are fixed when the constraint is created and stored with it, so the solver stays on the
/// branch the user drew: a tangent line does not jump to the other side of its circle.
#[derive(Clone, Debug, PartialEq)]
pub enum Constraint {
    /// The parameters keep their current values.
    Fix(Vec<ParamId>),
    /// Two 2D points coincide.
    Coincident(P2, P2),
    /// Two 3D points coincide.
    Coincident3(P3, P3),
    /// The point lies on the (infinite) line.
    PointOnLine(P2, Line2),
    /// The point lies on the circle.
    PointOnCircle(P2, Round),
    /// The point lies on the ellipse with centre `c`, major axis end `major` and minor radius `minor`.
    PointOnEllipse {
        /// The point.
        p: P2,
        /// Centre.
        c: P2,
        /// End of the major axis.
        major: P2,
        /// Minor radius (a length parameter).
        minor: ParamId,
    },
    /// The point is the midpoint of the segment.
    Midpoint(P2, Line2),
    /// The two points have the same y.
    Horizontal(P2, P2),
    /// The two points have the same x.
    Vertical(P2, P2),
    /// The lines are parallel.
    Parallel(Line2, Line2),
    /// The lines are perpendicular.
    Perpendicular(Line2, Line2),
    /// The second line lies on the first.
    Collinear(Line2, Line2),
    /// The line touches the circle; the centre is on the left of `a -> b` when `side` is `+1`.
    TangentLineCircle {
        /// The line.
        line: Line2,
        /// The circle or arc.
        round: Round,
        /// `+1.0` or `-1.0`.
        side: f64,
    },
    /// The circles touch from outside (`external`) or one inside the other.
    TangentCircleCircle {
        /// First circle.
        a: Round,
        /// Second circle.
        b: Round,
        /// True for touching from outside.
        external: bool,
    },
    /// A line and an arc that share the point `at` are tangent there.
    EndTangent {
        /// The line.
        line: Line2,
        /// The arc's circle.
        round: Round,
        /// The shared point, as a point of the arc.
        at: P2,
        /// True when the line continues the arc smoothly; false for a cusp. See [`Constraint::Arc`].
        smooth: bool,
        /// `+1.0` when travelling the arc counter-clockwise leaves `at`, `-1.0` when it arrives there.
        leaving: f64,
    },
    /// The segments have the same length.
    EqualLength(Line2, Line2),
    /// The circles have the same radius.
    EqualRadius(Round, Round),
    /// `p` and `q` mirror each other across the axis.
    Symmetric {
        /// First point.
        p: P2,
        /// Second point.
        q: P2,
        /// Mirror axis.
        axis: Line2,
    },
    /// Ties an arc's end points to its circle and guards its sweep (counter-clockwise from `start` to
    /// `end`) against jumping. Added once per arc by the sketch layer.
    Arc {
        /// The arc's circle.
        round: Round,
        /// Start point.
        start: P2,
        /// End point.
        end: P2,
    },
    /// Distance between two points.
    Distance {
        /// First point.
        p: P2,
        /// Second point.
        q: P2,
        /// The distance, positive.
        value: f64,
    },
    /// `q.x - p.x = sign * value`.
    HDistance {
        /// First point.
        p: P2,
        /// Second point.
        q: P2,
        /// `+1.0` or `-1.0`.
        sign: f64,
        /// The distance, positive.
        value: f64,
    },
    /// `q.y - p.y = sign * value`.
    VDistance {
        /// First point.
        p: P2,
        /// Second point.
        q: P2,
        /// `+1.0` or `-1.0`.
        sign: f64,
        /// The distance, positive.
        value: f64,
    },
    /// Distance of a point from a line, on the given side (`+1` = left of `a -> b`).
    PointLineDistance {
        /// The point.
        p: P2,
        /// The line.
        line: Line2,
        /// `+1.0` or `-1.0`.
        side: f64,
        /// The distance, positive.
        value: f64,
    },
    /// Angle from the first line to the second, counter-clockwise, each optionally reversed.
    Angle {
        /// First line.
        l1: Line2,
        /// Measure from `b -> a` instead of `a -> b`.
        rev1: bool,
        /// Second line.
        l2: Line2,
        /// Measure to `b -> a` instead of `a -> b`.
        rev2: bool,
        /// The angle in radians, in `(0, 2*pi)`.
        value: f64,
    },
    /// Radius of a circle or arc.
    Radius {
        /// The circle.
        round: Round,
        /// The radius, positive.
        value: f64,
    },
    /// Diameter of a circle or arc.
    Diameter {
        /// The circle.
        round: Round,
        /// The diameter, positive.
        value: f64,
    },
    /// Distance between two 3D points.
    Distance3 {
        /// First point.
        p: P3,
        /// Second point.
        q: P3,
        /// The distance, positive.
        value: f64,
    },
    /// One coordinate has a given value (distance of a 3D point from a principal plane, with its side
    /// folded into the sign).
    CoordValue {
        /// The coordinate parameter.
        param: ParamId,
        /// Its value.
        value: f64,
    },
    /// A 3D arc: its start and end are as far from its centre as its radius says.
    Arc3 {
        /// Centre.
        c: P3,
        /// Radius parameter.
        r: ParamId,
        /// Start.
        start: P3,
        /// End.
        end: P3,
    },
    /// Two 3D lines are parallel.
    Parallel3(Line3, Line3),
    /// Two 3D lines are at right angles (they need not meet).
    Perpendicular3(Line3, Line3),
    /// Two 3D lines lie on one line.
    Collinear3(Line3, Line3),
    /// Two 3D lines are as long as each other.
    EqualLength3(Line3, Line3),
    /// A 3D point lies on (the extension of) a 3D line.
    PointOnLine3(P3, Line3),
    /// A 3D point is the midpoint of a 3D line.
    Midpoint3(P3, Line3),
    /// Angle between two 3D lines, in `(0, pi)`, each direction optionally reversed.
    Angle3 {
        /// First line.
        l1: Line3,
        /// Its direction reversed.
        rev1: bool,
        /// Second line.
        l2: Line3,
        /// Its direction reversed.
        rev2: bool,
        /// The angle, radians.
        value: f64,
    },
    /// `q - p` is parallel to one axis of the sketch frame: the two other coordinates are equal.
    AlongAxis {
        /// First point.
        p: P3,
        /// Second point.
        q: P3,
        /// 0, 1 or 2 for X, Y or Z.
        axis: usize,
    },
}

/// What a constraint contributes to the system.
#[derive(Debug, Default)]
pub(crate) struct Lowered {
    /// `value(b) = value(a) + offset`: handled by substitution, not by a residual row.
    pub merges: Vec<(ParamId, ParamId, f64)>,
    /// `value(param) = value`: handled by substitution.
    pub fixes: Vec<(ParamId, f64)>,
    /// Residual rows with their inputs.
    pub rows: Vec<(RowKind, Vec<ParamId>)>,
}

fn line_vars(l: &Line2) -> [ParamId; 4] {
    [l.a[0], l.a[1], l.b[0], l.b[1]]
}

fn line3_vars(l: &Line3) -> [ParamId; 6] {
    [l.a[0], l.a[1], l.a[2], l.b[0], l.b[1], l.b[2]]
}

/// The two axes across a 3D direction: the components of a cross product with it that are independent
/// (the one along its largest component follows from the others).
fn across(d: [f64; 3]) -> [usize; 2] {
    let big = (0..3).fold(0, |best, k| if d[k].abs() > d[best].abs() { k } else { best });
    [(big + 1) % 3, (big + 2) % 3]
}

impl Constraint {
    /// The driving value of a dimension, or `None` for a geometric constraint.
    pub fn value(&self) -> Option<f64> {
        match self {
            Self::Distance { value, .. }
            | Self::HDistance { value, .. }
            | Self::VDistance { value, .. }
            | Self::PointLineDistance { value, .. }
            | Self::Angle { value, .. }
            | Self::Radius { value, .. }
            | Self::Diameter { value, .. }
            | Self::Distance3 { value, .. }
            | Self::Angle3 { value, .. }
            | Self::CoordValue { value, .. } => Some(*value),
            _ => None,
        }
    }

    /// Replaces the driving value of a dimension. Returns `false` for a geometric constraint.
    pub fn set_value(&mut self, new: f64) -> bool {
        match self {
            Self::Distance { value, .. }
            | Self::HDistance { value, .. }
            | Self::VDistance { value, .. }
            | Self::PointLineDistance { value, .. }
            | Self::Angle { value, .. }
            | Self::Radius { value, .. }
            | Self::Diameter { value, .. }
            | Self::Distance3 { value, .. }
            | Self::Angle3 { value, .. }
            | Self::CoordValue { value, .. } => {
                *value = new;
                true
            }
            _ => false,
        }
    }

    /// True when the dimension value is a length (as opposed to an angle); used for the sketch scale.
    pub(crate) fn length_value(&self) -> Option<f64> {
        match self {
            Self::Angle { .. } | Self::Angle3 { .. } => None,
            other => other.value(),
        }
    }

    /// Lowers the constraint. `get` reads current parameter values (for frozen scales and `Fix`).
    pub(crate) fn lower(&self, get: &dyn Fn(ParamId) -> f64) -> Lowered {
        let mut out = Lowered::default();
        let len = |l: &Line2| {
            let (dx, dy) = (get(l.b[0]) - get(l.a[0]), get(l.b[1]) - get(l.a[1]));
            (dx * dx + dy * dy).sqrt()
        };
        // A frozen scale must never be zero; a degenerate line is reported separately.
        let nonzero = |s: f64| if s > 0.0 && s.is_finite() { s } else { 1.0 };
        let two_lines = |a: &Line2, b: &Line2| [line_vars(a), line_vars(b)].concat();
        let dir3 = |l: &Line3| [0, 1, 2].map(|k| get(l.b[k]) - get(l.a[k]));
        let len3 = |l: &Line3| {
            let d = dir3(l);
            (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
        };
        let two3 = |a: &Line3, b: &Line3| [line3_vars(a), line3_vars(b)].concat();
        match self {
            Self::Fix(params) => out.fixes.extend(params.iter().map(|p| (*p, get(*p)))),
            Self::Coincident(p, q) => out.merges.extend([(p[0], q[0], 0.0), (p[1], q[1], 0.0)]),
            Self::Coincident3(p, q) => out.merges.extend((0..3).map(|k| (p[k], q[k], 0.0))),
            Self::PointOnLine(p, line) => out.rows.push((
                RowKind::PointLine { frozen: Some(nonzero(len(line))), target: 0.0 },
                [p.as_slice(), &line_vars(line)].concat(),
            )),
            Self::PointOnCircle(p, round) => {
                out.rows.push((RowKind::PointCircle, vec![p[0], p[1], round.c[0], round.c[1], round.r]));
            }
            Self::PointOnEllipse { p, c, major, minor } => {
                out.rows
                    .push((RowKind::PointEllipse, vec![p[0], p[1], c[0], c[1], major[0], major[1], *minor]));
            }
            Self::Midpoint(p, line) => {
                for k in 0..2 {
                    out.rows.push((RowKind::Mid, vec![p[k], line.a[k], line.b[k]]));
                }
            }
            Self::Horizontal(p, q) => out.merges.push((p[1], q[1], 0.0)),
            Self::Vertical(p, q) => out.merges.push((p[0], q[0], 0.0)),
            Self::Parallel(a, b) => {
                out.rows.push((RowKind::Cross { scale: nonzero(len(a).max(len(b))) }, two_lines(a, b)));
            }
            Self::Perpendicular(a, b) => {
                out.rows.push((RowKind::Dot { scale: nonzero(len(a).max(len(b))) }, two_lines(a, b)));
            }
            Self::Collinear(a, b) => {
                for point in [b.a, b.b] {
                    out.rows.push((
                        RowKind::PointLine { frozen: Some(nonzero(len(a))), target: 0.0 },
                        [point.as_slice(), &line_vars(a)].concat(),
                    ));
                }
            }
            Self::TangentLineCircle { line, round, side } => out.rows.push((
                RowKind::LineCircleTangent { side: *side },
                [line_vars(line).as_slice(), &[round.c[0], round.c[1], round.r]].concat(),
            )),
            Self::TangentCircleCircle { a, b, external } => {
                let sign = if get(a.r) >= get(b.r) { 1.0 } else { -1.0 };
                out.rows.push((
                    RowKind::CircleCircle { external: *external, sign },
                    vec![a.c[0], a.c[1], a.r, b.c[0], b.c[1], b.r],
                ));
            }
            Self::EndTangent { line, round, at, .. } => {
                // The line is perpendicular to the radius at the shared point.
                let radius = Line2 { a: round.c, b: *at };
                out.rows.push((
                    RowKind::Dot { scale: nonzero(len(line).max(len(&radius))) },
                    two_lines(line, &radius),
                ));
            }
            Self::EqualLength(a, b) => out.rows.push((RowKind::EqualLen, two_lines(a, b))),
            Self::EqualRadius(a, b) => out.merges.push((a.r, b.r, 0.0)),
            Self::Symmetric { p, q, axis } => {
                out.rows.push((
                    RowKind::MidOnLine { frozen: nonzero(len(axis)) },
                    [p.as_slice(), q.as_slice(), &line_vars(axis)].concat(),
                ));
                let chord = Line2 { a: *p, b: *q };
                out.rows.push((
                    RowKind::Dot { scale: nonzero(len(&chord).max(len(axis))) },
                    two_lines(&chord, axis),
                ));
            }
            Self::Arc { round, start, end } => {
                for point in [start, end] {
                    out.rows.push((
                        RowKind::PointCircle,
                        vec![point[0], point[1], round.c[0], round.c[1], round.r],
                    ));
                }
            }
            Self::Distance { p, q, value } => {
                out.rows.push((
                    RowKind::Distance { dim: 2, target: *value },
                    [p.as_slice(), q.as_slice()].concat(),
                ));
            }
            Self::HDistance { p, q, sign, value } => out.merges.push((p[0], q[0], sign * value)),
            Self::VDistance { p, q, sign, value } => out.merges.push((p[1], q[1], sign * value)),
            Self::PointLineDistance { p, line, side, value } => out.rows.push((
                RowKind::PointLine { frozen: None, target: side * value },
                [p.as_slice(), &line_vars(line)].concat(),
            )),
            Self::Angle { l1, rev1, l2, rev2, value } => out.rows.push((
                RowKind::Angle {
                    target: *value,
                    scale: nonzero(len(l1).min(len(l2))),
                    rev1: *rev1,
                    rev2: *rev2,
                },
                two_lines(l1, l2),
            )),
            Self::Radius { round, value } => out.fixes.push((round.r, *value)),
            Self::Diameter { round, value } => out.fixes.push((round.r, value / 2.0)),
            Self::Distance3 { p, q, value } => {
                out.rows.push((
                    RowKind::Distance { dim: 3, target: *value },
                    [p.as_slice(), q.as_slice()].concat(),
                ));
            }
            Self::CoordValue { param, value } => out.fixes.push((*param, *value)),
            Self::AlongAxis { p, q, axis } => {
                out.merges.extend((0..3).filter(|k| k != axis).map(|k| (p[k], q[k], 0.0)));
            }
            Self::Parallel3(a, b) => {
                let scale = nonzero(len3(a).max(len3(b)));
                for k in across(dir3(a)) {
                    out.rows.push((RowKind::Cross3 { scale, k }, two3(a, b)));
                }
            }
            Self::Perpendicular3(a, b) => {
                out.rows.push((RowKind::Dot3 { scale: nonzero(len3(a).max(len3(b))) }, two3(a, b)));
            }
            Self::Collinear3(a, b) => {
                for point in [b.a, b.b] {
                    for k in across(dir3(a)) {
                        out.rows
                            .push((RowKind::PointLine3 { k }, [point.as_slice(), &line3_vars(a)].concat()));
                    }
                }
            }
            Self::EqualLength3(a, b) => out.rows.push((RowKind::EqualLen3, two3(a, b))),
            Self::Arc3 { c, r, start, end } => {
                for p in [start, end] {
                    out.rows.push((RowKind::PointSphere, [p.as_slice(), c.as_slice(), &[*r]].concat()));
                }
            }
            Self::PointOnLine3(p, line) => {
                for k in across(dir3(line)) {
                    out.rows.push((RowKind::PointLine3 { k }, [p.as_slice(), &line3_vars(line)].concat()));
                }
            }
            Self::Midpoint3(p, line) => {
                for k in 0..3 {
                    out.rows.push((RowKind::Mid, vec![p[k], line.a[k], line.b[k]]));
                }
            }
            Self::Angle3 { l1, rev1, l2, rev2, value } => out.rows.push((
                RowKind::Angle3 {
                    target: *value,
                    scale: nonzero(len3(l1).min(len3(l2))),
                    rev1: *rev1,
                    rev2: *rev2,
                },
                two3(l1, l2),
            )),
        }
        out
    }

    /// The same contribution with substitutions written as explicit rows. Used for a candidate constraint,
    /// whose effect on the degrees of freedom must be measured row by row.
    pub(crate) fn lower_explicit(&self, get: &dyn Fn(ParamId) -> f64) -> Lowered {
        let mut out = self.lower(get);
        for (a, b, offset) in out.merges.drain(..) {
            out.rows.push((RowKind::Diff { target: offset }, vec![a, b]));
        }
        for (param, value) in out.fixes.drain(..) {
            out.rows.push((RowKind::Value { target: value }, vec![param]));
        }
        out
    }
}
