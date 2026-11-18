//! Trim, extend and fillet: where curves cross, and the edits that cut or lengthen curves there.
//!
//! The edits keep the geometry where it is. A cut end loses what tied it to the part that went away
//! (coincidences, dimensions to it) and is tied to the curve that cut it instead: coincident with that
//! curve's point when it ends there, otherwise on the curve. A line keeps its horizontal or vertical
//! constraint; a line split in two keeps its pieces collinear, an arc split in two its pieces on one
//! circle.

use std::f64::consts::TAU;

use gc_core::{Id, IdAlloc};
use gc_math::fm;

use crate::edit::{AddResult, Item, SketchError};
use crate::model::{Curve, CurveGeom, DimKind, GeoConstraint, SkPoint, Sketch};

/// Parameters closer than this (relative: of a line's length, or radians) count as the same place.
const EPS: f64 = 1e-9;

/// A piece of a curve, for drawing what a trim removes or an extend adds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Piece {
    /// A segment between two points.
    Segment([f64; 2], [f64; 2]),
    /// An arc: centre, radius, start angle and counter-clockwise sweep.
    Arc {
        /// Centre.
        centre: [f64; 2],
        /// Radius.
        radius: f64,
        /// Angle of the start.
        start: f64,
        /// Counter-clockwise sweep (positive).
        sweep: f64,
    },
    /// A whole circle.
    Circle {
        /// Centre.
        centre: [f64; 2],
        /// Radius.
        radius: f64,
    },
}

/// What a trim, extend or fillet created.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CutResult {
    /// New curves.
    pub curves: Vec<Id<Curve>>,
    /// New constraints.
    pub constraints: Vec<Id<GeoConstraint>>,
}

/// A curve as plane geometry.
#[derive(Clone, Copy, Debug)]
enum Shape {
    /// From `a` to `b`.
    Segment([f64; 2], [f64; 2]),
    /// Centre, radius, and for an arc its start angle and counter-clockwise sweep.
    Round([f64; 2], f64, Option<(f64, f64)>),
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    fm::hypot(a[0] - b[0], a[1] - b[1])
}

fn angle(centre: [f64; 2], p: [f64; 2]) -> f64 {
    fm::atan2(p[1] - centre[1], p[0] - centre[0])
}

fn on_circle(centre: [f64; 2], radius: f64, angle: f64) -> [f64; 2] {
    [centre[0] + radius * fm::cos(angle), centre[1] + radius * fm::sin(angle)]
}

impl Shape {
    /// Where a point on the full line or circle lies along this curve: a line's `t` (0 at `a`, 1 at `b`),
    /// an arc's counter-clockwise angle from its start, a circle's angle in `[0, 2π)`.
    fn param(&self, p: [f64; 2]) -> f64 {
        match *self {
            Shape::Segment(a, b) => {
                let d = [b[0] - a[0], b[1] - a[1]];
                let l2 = d[0] * d[0] + d[1] * d[1];
                if l2 > 0.0 { ((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / l2 } else { 0.0 }
            }
            Shape::Round(c, _, None) => angle(c, p).rem_euclid(TAU),
            Shape::Round(c, _, Some((start, _))) => (angle(c, p) - start).rem_euclid(TAU),
        }
    }

    /// The parameter range of the curve itself.
    fn span(&self) -> f64 {
        match *self {
            Shape::Segment(..) => 1.0,
            Shape::Round(_, _, None) => TAU,
            Shape::Round(_, _, Some((_, sweep))) => sweep,
        }
    }

    /// True when a point known to lie on the full line or circle lies on this curve.
    fn holds(&self, p: [f64; 2]) -> bool {
        match self {
            Shape::Round(_, _, None) => true,
            Shape::Segment(..) => (-EPS..=1.0 + EPS).contains(&self.param(p)),
            Shape::Round(_, _, Some(_)) => {
                let t = self.param(p);
                t <= self.span() + EPS || t >= TAU - EPS
            }
        }
    }

    /// The point at a parameter.
    fn at(&self, t: f64) -> [f64; 2] {
        match *self {
            Shape::Segment(a, b) => [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])],
            Shape::Round(c, r, None) => on_circle(c, r, t),
            Shape::Round(c, r, Some((start, _))) => on_circle(c, r, start + t),
        }
    }

    /// The part between two parameters (`from < to`).
    fn piece(&self, from: f64, to: f64) -> Piece {
        match *self {
            Shape::Segment(..) => Piece::Segment(self.at(from), self.at(to)),
            Shape::Round(centre, radius, arc) => {
                let base = arc.map_or(0.0, |(start, _)| start);
                Piece::Arc { centre, radius, start: base + from, sweep: to - from }
            }
        }
    }

    /// The whole curve.
    fn whole(&self) -> Piece {
        match *self {
            Shape::Segment(a, b) => Piece::Segment(a, b),
            Shape::Round(centre, radius, None) => Piece::Circle { centre, radius },
            Shape::Round(centre, radius, Some((start, sweep))) => Piece::Arc { centre, radius, start, sweep },
        }
    }
}

/// Where the full lines or circles of two shapes meet.
fn meet(first: &Shape, second: &Shape) -> Vec<[f64; 2]> {
    match (*first, *second) {
        (Shape::Segment(a, b), Shape::Segment(c, d)) => {
            let (r, s) = ([b[0] - a[0], b[1] - a[1]], [d[0] - c[0], d[1] - c[1]]);
            let denom = r[0] * s[1] - r[1] * s[0];
            if denom.abs() <= 1e-12 * fm::hypot(r[0], r[1]) * fm::hypot(s[0], s[1]) {
                Vec::new()
            } else {
                let t = ((c[0] - a[0]) * s[1] - (c[1] - a[1]) * s[0]) / denom;
                vec![[a[0] + t * r[0], a[1] + t * r[1]]]
            }
        }
        (Shape::Segment(a, b), Shape::Round(c, r, _)) | (Shape::Round(c, r, _), Shape::Segment(a, b)) => {
            let d = [b[0] - a[0], b[1] - a[1]];
            let f = [a[0] - c[0], a[1] - c[1]];
            let qa = d[0] * d[0] + d[1] * d[1];
            let qb = 2.0 * (f[0] * d[0] + f[1] * d[1]);
            let qc = f[0] * f[0] + f[1] * f[1] - r * r;
            let disc = qb * qb - 4.0 * qa * qc;
            if qa <= 0.0 || disc < -1e-12 * qa * r * r {
                Vec::new()
            } else {
                let root = disc.max(0.0).sqrt();
                let ts = if root <= 0.0 {
                    vec![-qb / (2.0 * qa)]
                } else {
                    vec![(-qb - root) / (2.0 * qa), (-qb + root) / (2.0 * qa)]
                };
                ts.into_iter().map(|t| [a[0] + t * d[0], a[1] + t * d[1]]).collect()
            }
        }
        (Shape::Round(c1, r1, _), Shape::Round(c2, r2, _)) => {
            let d = dist(c1, c2);
            if d <= 0.0 || d > r1 + r2 + EPS * r1.max(r2) || d < (r1 - r2).abs() - EPS * r1.max(r2) {
                Vec::new()
            } else {
                let along = (r1 * r1 - r2 * r2 + d * d) / (2.0 * d);
                let h = (r1 * r1 - along * along).max(0.0).sqrt();
                let u = [(c2[0] - c1[0]) / d, (c2[1] - c1[1]) / d];
                let m = [c1[0] + u[0] * along, c1[1] + u[1] * along];
                if h <= 0.0 {
                    vec![m]
                } else {
                    vec![[m[0] - u[1] * h, m[1] + u[0] * h], [m[0] + u[1] * h, m[1] - u[0] * h]]
                }
            }
        }
    }
}

/// Where a curve is cut by another one: the parameter along the curve, the position, and by what.
#[derive(Clone, Copy, Debug)]
struct Cut {
    t: f64,
    pos: [f64; 2],
    by: Id<Curve>,
}

impl Sketch {
    fn xy(&self, p: Id<SkPoint>) -> Option<[f64; 2]> {
        self.pos(p).map(|q| [q[0], q[1]])
    }

    fn shape(&self, curve: Id<Curve>) -> Option<Shape> {
        Some(match self.curves.get(&curve)?.geom {
            CurveGeom::Line { a, b } => Shape::Segment(self.xy(a)?, self.xy(b)?),
            CurveGeom::Circle { c, r } => Shape::Round(self.xy(c)?, r, None),
            CurveGeom::Arc { c, s, e } => {
                let (c, s, e) = (self.xy(c)?, self.xy(s)?, self.xy(e)?);
                let start = angle(c, s);
                Shape::Round(c, dist(c, s), Some((start, (angle(c, e) - start).rem_euclid(TAU))))
            }
        })
    }

    /// Where the other curves cross `curve` (on both curves' extents, or with `extended` on the full line
    /// or circle of `curve`), sorted along it, without repeats.
    fn cuts(&self, curve: Id<Curve>, extended: bool) -> Vec<Cut> {
        let Some(shape) = self.shape(curve) else { return Vec::new() };
        let mut cuts: Vec<Cut> = Vec::new();
        for (id, _) in self.curves.iter().filter(|(id, _)| **id != curve) {
            let Some(other) = self.shape(*id) else { continue };
            for pos in meet(&shape, &other) {
                if other.holds(pos) && (extended || shape.holds(pos)) {
                    cuts.push(Cut { t: shape.param(pos), pos, by: *id });
                }
            }
        }
        cuts.sort_by(|a, b| a.t.total_cmp(&b.t));
        cuts.dedup_by(|later, earlier| (later.t - earlier.t).abs() <= EPS);
        cuts
    }

    /// The piece of `curve` that a trim at `at` would remove, between the cuts on either side of `at`.
    pub fn trim_preview(&self, curve: Id<Curve>, at: [f64; 2]) -> Option<Piece> {
        let shape = self.shape(curve)?;
        Some(match self.trim_span(curve, at)? {
            Span::Whole => shape.whole(),
            Span::Part { from, to, .. } => shape.piece(from.t, to.t),
            Span::Circle { from, to } => {
                let sweep = (to.t - from.t).rem_euclid(TAU);
                shape.piece(from.t, from.t + sweep)
            }
        })
    }

    fn trim_span(&self, curve: Id<Curve>, at: [f64; 2]) -> Option<Span> {
        let shape = self.shape(curve)?;
        let t = shape.param(at);
        let end = shape.span();
        let inner: Vec<Cut> = self.cuts(curve, false);
        if let Shape::Round(_, _, None) = shape {
            if inner.len() < 2 {
                return Some(Span::Whole);
            }
            let after = inner.iter().find(|c| c.t > t).or(inner.first()).copied()?;
            let before = inner.iter().rev().find(|c| c.t <= t).or(inner.last()).copied()?;
            return Some(Span::Circle { from: before, to: after });
        }
        let inner: Vec<Cut> = inner.into_iter().filter(|c| c.t > EPS && c.t < end - EPS).collect();
        let before = inner.iter().rev().find(|c| c.t <= t).copied();
        let after = inner.iter().find(|c| c.t > t).copied();
        let start = Cut { t: 0.0, pos: shape.at(0.0), by: curve };
        let stop = Cut { t: end, pos: shape.at(end), by: curve };
        Some(match (before, after) {
            (None, None) => Span::Whole,
            (from, to) => Span::Part {
                from: from.unwrap_or(start),
                to: to.unwrap_or(stop),
                at_start: from.is_none(),
                at_end: to.is_none(),
            },
        })
    }

    /// Removes the piece of `curve` around `at` up to the nearest crossing curves on either side (the
    /// whole curve when nothing crosses it; a circle needs two crossings).
    pub fn trim(
        &mut self,
        alloc: &mut IdAlloc,
        curve: Id<Curve>,
        at: [f64; 2],
    ) -> Result<CutResult, SketchError> {
        let span = self.trim_span(curve, at).ok_or(SketchError::MissingCurve)?;
        let geom = self.curves.get(&curve).ok_or(SketchError::MissingCurve)?.geom;
        let construction = self.curves[&curve].construction;
        let mut result = CutResult::default();
        match (span, geom) {
            (Span::Whole, _) => self.delete(&[Item::Curve(curve)]),
            (Span::Circle { from, to }, CurveGeom::Circle { c, .. }) => {
                // What is left runs counter-clockwise from where the removed piece ends to where it
                // starts: the circle becomes that arc and keeps its constraints and dimensions.
                let s = self.add_point(alloc, [to.pos[0], to.pos[1], 0.0]);
                let e = self.add_point(alloc, [from.pos[0], from.pos[1], 0.0]);
                if let Some(k) = self.curves.get_mut(&curve) {
                    k.geom = CurveGeom::Arc { c, s, e };
                }
                self.attach(alloc, s, to.pos, to.by, &mut result);
                self.attach(alloc, e, from.pos, from.by, &mut result);
            }
            (Span::Part { from, to, at_start, at_end }, _) => {
                let (first, last) = Self::ends(&geom).ok_or(SketchError::InvalidValue)?;
                if at_start {
                    self.move_end(curve, first, Some(last), to.pos);
                    self.attach(alloc, first, to.pos, to.by, &mut result);
                } else if at_end {
                    self.move_end(curve, last, Some(first), from.pos);
                    self.attach(alloc, last, from.pos, from.by, &mut result);
                } else {
                    self.split(alloc, curve, geom, construction, (from, to), &mut result)?;
                }
            }
            (Span::Circle { .. }, _) => return Err(SketchError::InvalidValue),
        }
        Ok(result)
    }

    /// Splits `curve` around a removed middle piece: the curve keeps its start and ends at `from`; a new
    /// curve runs from `to` to where the curve ended and takes over what was tied to that end.
    fn split(
        &mut self,
        alloc: &mut IdAlloc,
        curve: Id<Curve>,
        geom: CurveGeom,
        construction: bool,
        (from, to): (Cut, Cut),
        result: &mut CutResult,
    ) -> Result<(), SketchError> {
        let (first, last) = Self::ends(&geom).ok_or(SketchError::InvalidValue)?;
        let last_pos = self.pos(last).ok_or(SketchError::MissingPoint)?;
        let piece = match geom {
            CurveGeom::Line { .. } => self.add_line(alloc, [to.pos[0], to.pos[1], 0.0], last_pos),
            CurveGeom::Arc { c, .. } => {
                let centre = self.pos(c).ok_or(SketchError::MissingPoint)?;
                self.add_arc(alloc, centre, [to.pos[0], to.pos[1], 0.0], last_pos)?
            }
            CurveGeom::Circle { .. } => return Err(SketchError::InvalidValue),
        };
        if let Some(k) = self.curves.get_mut(&piece) {
            k.construction = construction;
        }
        result.curves.push(piece);
        let (piece_first, piece_last) =
            Self::ends(&self.curves[&piece].geom).ok_or(SketchError::InvalidValue)?;
        // What held the old end now holds the new piece's end; what tied both ends together stays.
        let moved = |p: Id<SkPoint>| if p == last { piece_last } else { p };
        let mut constraints = std::mem::take(&mut self.constraints);
        for c in constraints.values_mut() {
            let points = Self::constraint_points(c);
            if points.contains(&last) && !points.contains(&first) {
                *c = map_constraint(c, moved);
            }
        }
        self.constraints = constraints;
        for d in self.dimensions.values_mut() {
            let (points, _) = Self::dimension_refs(&d.kind);
            if points.contains(&last) && !points.contains(&first) {
                d.kind = map_dimension(&d.kind, moved);
            }
        }
        self.move_end(curve, last, Some(first), from.pos);
        let together = match geom {
            CurveGeom::Arc { c, .. } => {
                let CurveGeom::Arc { c: piece_c, .. } = self.curves[&piece].geom else {
                    return Err(SketchError::InvalidValue);
                };
                vec![GeoConstraint::Coincident(c, piece_c), GeoConstraint::Equal(curve, piece)]
            }
            _ => vec![GeoConstraint::Collinear(curve, piece)],
        };
        for constraint in together {
            self.add_quietly(alloc, constraint, result);
        }
        self.attach(alloc, last, from.pos, from.by, result);
        self.attach(alloc, piece_first, to.pos, to.by, result);
        Ok(())
    }

    /// A line's or arc's two ends (an arc's in counter-clockwise order).
    fn ends(geom: &CurveGeom) -> Option<(Id<SkPoint>, Id<SkPoint>)> {
        match *geom {
            CurveGeom::Line { a, b } => Some((a, b)),
            CurveGeom::Arc { s, e, .. } => Some((s, e)),
            CurveGeom::Circle { .. } => None,
        }
    }

    /// Moves an end of `curve` along it to `to` and lets go of what tied it where it was: constraints and
    /// dimensions on the end (except a line's horizontal, vertical or along-axis constraint with its
    /// `other` end), and constraints that depend on the curve's length.
    fn move_end(&mut self, curve: Id<Curve>, end: Id<SkPoint>, other: Option<Id<SkPoint>>, to: [f64; 2]) {
        let is_line = self.line(curve).is_some();
        let own = |p: Id<SkPoint>, q: Id<SkPoint>| {
            is_line && ((p == end && Some(q) == other) || (q == end && Some(p) == other))
        };
        self.constraints.retain(|_, c| match *c {
            GeoConstraint::Horizontal(p, q)
            | GeoConstraint::Vertical(p, q)
            | GeoConstraint::AlongAxis { p, q, .. }
                if own(p, q) =>
            {
                true
            }
            GeoConstraint::Equal(a, b) if is_line && (a == curve || b == curve) => false,
            GeoConstraint::Midpoint(_, line) if line == curve => false,
            _ => !Self::constraint_points(c).contains(&end),
        });
        self.dimensions.retain(|_, d| !Self::dimension_refs(&d.kind).0.contains(&end));
        if let Some(p) = self.points.get_mut(&end) {
            p.pos = [to[0], to[1], p.pos[2]];
        }
    }

    /// Ties a cut end to the curve that cut it: coincident with that curve's point if there is one at
    /// `pos`, otherwise on the curve.
    fn attach(
        &mut self,
        alloc: &mut IdAlloc,
        end: Id<SkPoint>,
        pos: [f64; 2],
        by: Id<Curve>,
        result: &mut CutResult,
    ) {
        let Some(curve) = self.curves.get(&by) else { return };
        let scale = self.shape(by).map_or(1.0, |s| match s {
            Shape::Segment(a, b) => dist(a, b),
            Shape::Round(_, r, _) => r,
        });
        let at_point = curve
            .points()
            .into_iter()
            .filter(|p| *p != end)
            .find(|p| self.xy(*p).is_some_and(|q| dist(q, pos) <= 1e-7 * scale.max(1e-3)));
        let constraint = match at_point {
            Some(point) => GeoConstraint::Coincident(point, end),
            None => GeoConstraint::PointOnCurve(end, by),
        };
        self.add_quietly(alloc, constraint, result);
    }

    /// Adds a constraint if it removes freedom; a redundant one is left out.
    fn add_quietly(&mut self, alloc: &mut IdAlloc, constraint: GeoConstraint, result: &mut CutResult) {
        if let Ok(AddResult::Added { id, .. }) = self.try_add_constraint(alloc, constraint) {
            result.constraints.push(id);
        }
    }

    /// Which end of a line or arc an extend at `at` lengthens, where to, and the curve it reaches: the
    /// end nearer `at`, to the first curve that the line or circle meets beyond it.
    fn extend_target(&self, curve: Id<Curve>, at: [f64; 2]) -> Option<(Id<SkPoint>, Id<SkPoint>, Cut, bool)> {
        let shape = self.shape(curve)?;
        let (first, last) = Self::ends(&self.curves.get(&curve)?.geom)?;
        let span = shape.span();
        let t = shape.param(at);
        let at_last = match shape {
            Shape::Segment(..) => t > 0.5,
            // On the arc's own sweep: the nearer end; beyond it: the end on that side of the gap.
            _ => {
                if t <= span {
                    t > span / 2.0
                } else {
                    t - span < (TAU - span) / 2.0
                }
            }
        };
        let cuts = self.cuts(curve, true);
        let target = match shape {
            Shape::Segment(..) if at_last => cuts.into_iter().find(|c| c.t > 1.0 + EPS),
            Shape::Segment(..) => cuts.into_iter().rev().find(|c| c.t < -EPS),
            _ if at_last => cuts.into_iter().find(|c| c.t > span + EPS && c.t < TAU - EPS),
            _ => cuts.into_iter().rev().find(|c| c.t > span + EPS && c.t < TAU - EPS),
        }?;
        if at_last { Some((last, first, target, true)) } else { Some((first, last, target, false)) }
    }

    /// The piece an extend at `at` would add to `curve`.
    pub fn extend_preview(&self, curve: Id<Curve>, at: [f64; 2]) -> Option<Piece> {
        let shape = self.shape(curve)?;
        let (_, _, target, at_last) = self.extend_target(curve, at)?;
        Some(match shape {
            Shape::Segment(..) if at_last => shape.piece(1.0, target.t),
            Shape::Segment(..) => shape.piece(target.t, 0.0),
            _ if at_last => shape.piece(shape.span(), target.t),
            _ => shape.piece(target.t, TAU),
        })
    }

    /// Lengthens the line or arc at its end nearer `at` up to the first curve it meets. The end must be
    /// open: an end joined to other geometry is refused.
    pub fn extend(
        &mut self,
        alloc: &mut IdAlloc,
        curve: Id<Curve>,
        at: [f64; 2],
    ) -> Result<CutResult, SketchError> {
        let (end, other, target, _) = self.extend_target(curve, at).ok_or(SketchError::InvalidValue)?;
        let joined = self
            .constraints
            .values()
            .any(|c| matches!(*c, GeoConstraint::Coincident(p, q) if p == end || q == end));
        if joined {
            return Err(SketchError::InvalidValue);
        }
        let mut result = CutResult::default();
        self.move_end(curve, end, Some(other), target.pos);
        self.attach(alloc, end, target.pos, target.by, &mut result);
        Ok(result)
    }

    /// The fillet arc between two lines: centre, the two tangent points, and for each line the end that
    /// moves to its tangent point. Each line keeps the side of the lines' crossing where it was picked.
    fn fillet_plan(&self, lines: [(Id<Curve>, [f64; 2]); 2], radius: f64) -> Option<Fillet> {
        if !(radius.is_finite() && radius > 0.0) {
            return None;
        }
        let [(l1, at1), (l2, at2)] = lines;
        if l1 == l2 {
            return None;
        }
        let (s1, s2) = (self.shape(l1)?, self.shape(l2)?);
        let (Shape::Segment(a1, b1), Shape::Segment(a2, b2)) = (s1, s2) else { return None };
        let x = *meet(&s1, &s2).first()?;
        let unit = |v: [f64; 2]| {
            let n = fm::hypot(v[0], v[1]);
            (n > 0.0).then(|| [v[0] / n, v[1] / n])
        };
        let dot = |u: [f64; 2], v: [f64; 2]| u[0] * v[0] + u[1] * v[1];
        let side = |a: [f64; 2], b: [f64; 2], at: [f64; 2]| -> Option<[f64; 2]> {
            let d = unit([b[0] - a[0], b[1] - a[1]])?;
            let s = dot([at[0] - x[0], at[1] - x[1]], d);
            // Picked right at the crossing: the side where most of the line is.
            let s = if s.abs() > 1e-12 {
                s
            } else {
                dot([a[0] + b[0] - 2.0 * x[0], a[1] + b[1] - 2.0 * x[1]], d)
            };
            Some(if s >= 0.0 { d } else { [-d[0], -d[1]] })
        };
        let (u1, u2) = (side(a1, b1, at1)?, side(a2, b2, at2)?);
        let cos = dot(u1, u2).clamp(-1.0, 1.0);
        let theta = fm::acos(cos);
        if !(1e-6..=std::f64::consts::PI - 1e-6).contains(&theta) {
            return None;
        }
        let back = radius / fm::tan(theta / 2.0);
        let bisector = unit([u1[0] + u2[0], u1[1] + u2[1]])?;
        let out = radius / fm::sin(theta / 2.0);
        let centre = [x[0] + bisector[0] * out, x[1] + bisector[1] * out];
        let touch = |u: [f64; 2]| [x[0] + u[0] * back, x[1] + u[1] * back];
        let (t1, t2) = (touch(u1), touch(u2));
        // The end on the crossing's side moves; the other must lie beyond the tangent point.
        let mover = |line: Id<Curve>, u: [f64; 2]| -> Option<(Id<SkPoint>, Id<SkPoint>)> {
            let (a, b) = self.line(line)?;
            let along = |p: Id<SkPoint>| self.xy(p).map(|q| dot([q[0] - x[0], q[1] - x[1]], u));
            let (da, db) = (along(a)?, along(b)?);
            let (near, far, far_along) = if da <= db { (a, b, db) } else { (b, a, da) };
            (far_along > back * (1.0 + 1e-9)).then_some((near, far))
        };
        Some(Fillet { centre, touch: [t1, t2], ends: [mover(l1, u1)?, mover(l2, u2)?], lines: [l1, l2] })
    }

    /// The arc a fillet of `radius` between two lines would add.
    pub fn fillet_preview(&self, lines: [(Id<Curve>, [f64; 2]); 2], radius: f64) -> Option<Piece> {
        let plan = self.fillet_plan(lines, radius)?;
        let (a1, a2) = (angle(plan.centre, plan.touch[0]), angle(plan.centre, plan.touch[1]));
        let ccw = (a2 - a1).rem_euclid(TAU);
        let (start, sweep) = if ccw <= std::f64::consts::PI { (a1, ccw) } else { (a2, TAU - ccw) };
        Some(Piece::Arc { centre: plan.centre, radius, start, sweep })
    }

    /// Rounds the corner between two lines with an arc of `radius` tangent to both, trimming or
    /// extending the lines to the arc. Each line keeps the side of the corner where it was picked. The
    /// new arc is the first curve of the result.
    pub fn fillet(
        &mut self,
        alloc: &mut IdAlloc,
        lines: [(Id<Curve>, [f64; 2]); 2],
        radius: f64,
    ) -> Result<CutResult, SketchError> {
        if self.is_spatial() {
            return Err(SketchError::WrongSpace);
        }
        let plan = self.fillet_plan(lines, radius).ok_or(SketchError::InvalidValue)?;
        let mut result = CutResult::default();
        for i in 0..2 {
            let (end, far) = plan.ends[i];
            self.move_end(plan.lines[i], end, Some(far), plan.touch[i]);
        }
        let (a1, a2) = (angle(plan.centre, plan.touch[0]), angle(plan.centre, plan.touch[1]));
        let first_starts = (a2 - a1).rem_euclid(TAU) <= std::f64::consts::PI;
        let (s, e) =
            if first_starts { (plan.touch[0], plan.touch[1]) } else { (plan.touch[1], plan.touch[0]) };
        let lift = |p: [f64; 2]| [p[0], p[1], 0.0];
        let arc = self.add_arc(alloc, lift(plan.centre), lift(s), lift(e))?;
        result.curves.push(arc);
        let CurveGeom::Arc { s: arc_s, e: arc_e, .. } = self.curves[&arc].geom else {
            return Err(SketchError::InvalidValue);
        };
        let joints = if first_starts { [arc_s, arc_e] } else { [arc_e, arc_s] };
        for i in 0..2 {
            let (end, _) = plan.ends[i];
            self.add_required(alloc, GeoConstraint::Coincident(end, joints[i]), &mut result)?;
            let (a, b) = self.line(plan.lines[i]).ok_or(SketchError::MissingCurve)?;
            let (a, b) =
                (self.xy(a).ok_or(SketchError::MissingPoint)?, self.xy(b).ok_or(SketchError::MissingPoint)?);
            let cross = (b[0] - a[0]) * (plan.centre[1] - a[1]) - (b[1] - a[1]) * (plan.centre[0] - a[0]);
            let side = if cross >= 0.0 { 1.0 } else { -1.0 };
            let tangent = GeoConstraint::Tangent { a: plan.lines[i], b: arc, side, external: true };
            self.add_quietly(alloc, tangent, &mut result);
        }
        Ok(result)
    }

    /// Where a split at `at` would cut `curve`: the crossing with another curve nearest `at` (Inventor 2026
    /// help, To Split, Trim, or Extend Curves, GUID-DD00037F-6BBA-4B95-AA90-4F99DCEB667D). `None` for a
    /// circle, or when nothing crosses the curve between its ends.
    pub fn split_preview(&self, curve: Id<Curve>, at: [f64; 2]) -> Option<[f64; 2]> {
        self.split_cut(curve, at).map(|cut| cut.pos)
    }

    fn split_cut(&self, curve: Id<Curve>, at: [f64; 2]) -> Option<Cut> {
        let shape = self.shape(curve)?;
        if let Shape::Round(_, _, None) = shape {
            return None;
        }
        let t = shape.param(at);
        let end = shape.span();
        self.cuts(curve, false)
            .into_iter()
            .filter(|c| c.t > EPS && c.t < end - EPS)
            .min_by(|a, b| (a.t - t).abs().total_cmp(&(b.t - t).abs()))
    }

    /// Splits `curve` in two at the crossing nearest `at`: the curve ends there and a new curve carries on
    /// from there, joined to it and held in line (a line) or on the same circle (an arc). The new curve is
    /// the first of the result.
    pub fn split_at(
        &mut self,
        alloc: &mut IdAlloc,
        curve: Id<Curve>,
        at: [f64; 2],
    ) -> Result<CutResult, SketchError> {
        if self.is_spatial() {
            return Err(SketchError::WrongSpace);
        }
        let cut = self.split_cut(curve, at).ok_or(SketchError::InvalidValue)?;
        let geom = self.curves.get(&curve).ok_or(SketchError::MissingCurve)?.geom;
        let construction = self.curves[&curve].construction;
        let mut result = CutResult::default();
        self.split(alloc, curve, geom, construction, (cut, cut), &mut result)?;
        // Nothing was taken out: the two pieces meet.
        let (_, end) = Self::ends(&self.curves[&curve].geom).ok_or(SketchError::InvalidValue)?;
        let piece = *result.curves.first().ok_or(SketchError::InvalidValue)?;
        let (start, _) = Self::ends(&self.curves[&piece].geom).ok_or(SketchError::InvalidValue)?;
        self.add_quietly(alloc, GeoConstraint::Coincident(end, start), &mut result);
        Ok(result)
    }

    /// Where a chamfer cutting `distances` back from the corner of two lines would join them: the corner,
    /// and the two cut points. Each line keeps the side of the corner where it was picked.
    pub fn chamfer_plan(
        &self,
        lines: [(Id<Curve>, [f64; 2]); 2],
        distances: [f64; 2],
    ) -> Option<([f64; 2], [[f64; 2]; 2])> {
        self.chamfer_ends(lines, distances).map(|(corner, cuts, _)| (corner, cuts))
    }

    #[allow(clippy::type_complexity, reason = "a private plan: corner, cut points, moving ends")]
    fn chamfer_ends(
        &self,
        lines: [(Id<Curve>, [f64; 2]); 2],
        distances: [f64; 2],
    ) -> Option<([f64; 2], [[f64; 2]; 2], [(Id<SkPoint>, Id<SkPoint>); 2])> {
        if self.is_spatial() || !distances.iter().all(|d| d.is_finite() && *d > 0.0) {
            return None;
        }
        let [(l1, at1), (l2, at2)] = lines;
        if l1 == l2 {
            return None;
        }
        let (s1, s2) = (self.shape(l1)?, self.shape(l2)?);
        let (Shape::Segment(a1, b1), Shape::Segment(a2, b2)) = (s1, s2) else { return None };
        let x = *meet(&s1, &s2).first()?;
        let unit = |v: [f64; 2]| {
            let n = fm::hypot(v[0], v[1]);
            (n > 0.0).then(|| [v[0] / n, v[1] / n])
        };
        let dot = |u: [f64; 2], v: [f64; 2]| u[0] * v[0] + u[1] * v[1];
        let side = |a: [f64; 2], b: [f64; 2], at: [f64; 2]| -> Option<[f64; 2]> {
            let d = unit([b[0] - a[0], b[1] - a[1]])?;
            let s = dot([at[0] - x[0], at[1] - x[1]], d);
            let s = if s.abs() > 1e-12 {
                s
            } else {
                dot([a[0] + b[0] - 2.0 * x[0], a[1] + b[1] - 2.0 * x[1]], d)
            };
            Some(if s >= 0.0 { d } else { [-d[0], -d[1]] })
        };
        let u = [side(a1, b1, at1)?, side(a2, b2, at2)?];
        let cuts = [0, 1].map(|i| [x[0] + u[i][0] * distances[i], x[1] + u[i][1] * distances[i]]);
        let mover = |line: Id<Curve>, u: [f64; 2], back: f64| -> Option<(Id<SkPoint>, Id<SkPoint>)> {
            let (a, b) = self.line(line)?;
            let along = |p: Id<SkPoint>| self.xy(p).map(|q| dot([q[0] - x[0], q[1] - x[1]], u));
            let (da, db) = (along(a)?, along(b)?);
            let (near, far, far_along) = if da <= db { (a, b, db) } else { (b, a, da) };
            (far_along > back * (1.0 + 1e-9)).then_some((near, far))
        };
        let ends = [mover(l1, u[0], distances[0])?, mover(l2, u[1], distances[1])?];
        Some((x, cuts, ends))
    }

    /// Bevels the corner of two lines: each is cut back to `distances` from where they meet (or extended
    /// to there), and a new line joins the cut points (Inventor 2026 help, To Create 2D Shape Geometry,
    /// GUID-D489CE6D-7299-4211-A43A-F3580A4BA357, Create Chamfers). The new line is the first curve of the
    /// result.
    pub fn chamfer(
        &mut self,
        alloc: &mut IdAlloc,
        lines: [(Id<Curve>, [f64; 2]); 2],
        distances: [f64; 2],
    ) -> Result<CutResult, SketchError> {
        if self.is_spatial() {
            return Err(SketchError::WrongSpace);
        }
        let (_, cuts, ends) = self.chamfer_ends(lines, distances).ok_or(SketchError::InvalidValue)?;
        let mut result = CutResult::default();
        for i in 0..2 {
            let (end, far) = ends[i];
            self.move_end(lines[i].0, end, Some(far), cuts[i]);
        }
        let lift = |p: [f64; 2]| [p[0], p[1], 0.0];
        let bevel = self.add_line(alloc, lift(cuts[0]), lift(cuts[1]));
        result.curves.push(bevel);
        let (start, stop) = self.line(bevel).ok_or(SketchError::MissingCurve)?;
        self.add_required(alloc, GeoConstraint::Coincident(ends[0].0, start), &mut result)?;
        self.add_required(alloc, GeoConstraint::Coincident(ends[1].0, stop), &mut result)?;
        Ok(result)
    }

    fn add_required(
        &mut self,
        alloc: &mut IdAlloc,
        constraint: GeoConstraint,
        result: &mut CutResult,
    ) -> Result<(), SketchError> {
        match self.try_add_constraint(alloc, constraint)? {
            AddResult::Added { id, .. } => {
                result.constraints.push(id);
                Ok(())
            }
            AddResult::Redundant(_) | AddResult::Conflicting(_) => Err(SketchError::InvalidValue),
        }
    }
}

/// The part a trim removes.
#[derive(Clone, Copy, Debug)]
enum Span {
    /// Nothing crosses the curve: all of it.
    Whole,
    /// From one cut (or the curve's start) to the next (or the curve's end).
    Part { from: Cut, to: Cut, at_start: bool, at_end: bool },
    /// A circle, counter-clockwise from one cut to the next.
    Circle { from: Cut, to: Cut },
}

/// A planned fillet.
#[derive(Clone, Copy, Debug)]
struct Fillet {
    centre: [f64; 2],
    touch: [[f64; 2]; 2],
    /// Per line: the end that moves to the tangent point, and the other end.
    ends: [(Id<SkPoint>, Id<SkPoint>); 2],
    lines: [Id<Curve>; 2],
}

/// A constraint with its points and curves renamed (a copy's constraints).
pub(crate) fn rename(
    c: &GeoConstraint,
    point: impl Fn(Id<SkPoint>) -> Id<SkPoint>,
    curve: impl Fn(Id<Curve>) -> Id<Curve>,
) -> GeoConstraint {
    match map_constraint(c, &point) {
        GeoConstraint::PointOnCurve(p, k) => GeoConstraint::PointOnCurve(p, curve(k)),
        GeoConstraint::Midpoint(p, k) => GeoConstraint::Midpoint(p, curve(k)),
        GeoConstraint::Symmetric { p, q, axis } => GeoConstraint::Symmetric { p, q, axis: curve(axis) },
        GeoConstraint::Parallel(a, b) => GeoConstraint::Parallel(curve(a), curve(b)),
        GeoConstraint::Perpendicular(a, b) => GeoConstraint::Perpendicular(curve(a), curve(b)),
        GeoConstraint::Collinear(a, b) => GeoConstraint::Collinear(curve(a), curve(b)),
        GeoConstraint::Equal(a, b) => GeoConstraint::Equal(curve(a), curve(b)),
        GeoConstraint::Concentric(a, b) => GeoConstraint::Concentric(curve(a), curve(b)),
        GeoConstraint::Tangent { a, b, side, external } => {
            GeoConstraint::Tangent { a: curve(a), b: curve(b), side, external }
        }
        other => other,
    }
}

/// A constraint with its points renamed.
fn map_constraint(c: &GeoConstraint, f: impl Fn(Id<SkPoint>) -> Id<SkPoint>) -> GeoConstraint {
    match *c {
        GeoConstraint::Coincident(p, q) => GeoConstraint::Coincident(f(p), f(q)),
        GeoConstraint::PointOnCurve(p, k) => GeoConstraint::PointOnCurve(f(p), k),
        GeoConstraint::Midpoint(p, k) => GeoConstraint::Midpoint(f(p), k),
        GeoConstraint::Horizontal(p, q) => GeoConstraint::Horizontal(f(p), f(q)),
        GeoConstraint::Vertical(p, q) => GeoConstraint::Vertical(f(p), f(q)),
        GeoConstraint::Symmetric { p, q, axis } => GeoConstraint::Symmetric { p: f(p), q: f(q), axis },
        GeoConstraint::Fix(p) => GeoConstraint::Fix(f(p)),
        GeoConstraint::AlongAxis { p, q, axis } => GeoConstraint::AlongAxis { p: f(p), q: f(q), axis },
        GeoConstraint::OnPlane { p, axis } => GeoConstraint::OnPlane { p: f(p), axis },
        other => other,
    }
}

/// A dimension with its points renamed.
fn map_dimension(d: &DimKind, f: impl Fn(Id<SkPoint>) -> Id<SkPoint>) -> DimKind {
    match *d {
        DimKind::Distance { p, q } => DimKind::Distance { p: f(p), q: f(q) },
        DimKind::HDistance { p, q, sign } => DimKind::HDistance { p: f(p), q: f(q), sign },
        DimKind::VDistance { p, q, sign } => DimKind::VDistance { p: f(p), q: f(q), sign },
        DimKind::PointLine { p, line, side } => DimKind::PointLine { p: f(p), line, side },
        DimKind::PlaneDistance { p, axis, side } => DimKind::PlaneDistance { p: f(p), axis, side },
        other => other,
    }
}
