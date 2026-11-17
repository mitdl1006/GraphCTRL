//! Finding what is under the cursor, and where a new point should go.

use gc_core::Id;
use gc_math::fm;
use gc_sketch::{Curve, CurveGeom, Item, SkPoint, Sketch};

use crate::tolerance;

/// Why a position was adjusted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapKind {
    /// Nothing nearby: the cursor position itself.
    Free,
    /// An existing point.
    Point,
    /// A position on an existing curve.
    OnCurve,
    /// Level with the previous point.
    Horizontal,
    /// Plumb with the previous point.
    Vertical,
}

/// Which inference rule placed a snapped point (for the marker drawn at the cursor). The constraint it
/// implies is in the [`Snap`]'s other fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Inference {
    /// The centre of a circle or arc.
    Centre,
    /// The midpoint of a line.
    Midpoint,
    /// A quadrant point of a circle (0°, 90°, 180°, 270°).
    Quadrant,
    /// Where two curves cross.
    Intersection,
    /// Where the segment from the previous point touches a circle or arc.
    Tangent,
    /// Level or plumb with an existing point (a dotted alignment line is shown).
    Aligned,
}

/// Where a new point goes and what that implies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snap {
    /// The adjusted position.
    pub pos: [f64; 2],
    /// What kind of snap this is (for the marker drawn at the cursor).
    pub kind: SnapKind,
    /// The existing point the new point coincides with.
    pub point: Option<Id<SkPoint>>,
    /// The existing curve the new point lies on.
    pub curve: Option<Id<Curve>>,
    /// `Some(true)` when the segment from the previous point is horizontal, `Some(false)` when vertical.
    pub horizontal: Option<bool>,
    /// The finer rule behind the snap, when there is one.
    pub inference: Option<Inference>,
    /// A second curve the point lies on (an intersection).
    pub also_on: Option<Id<Curve>>,
    /// The line whose midpoint the point is.
    pub midpoint_of: Option<Id<Curve>>,
    /// An existing point the point is level (`true`) or plumb (`false`) with, and where that point is
    /// (the alignment line runs from there to the snapped position).
    pub aligned: Option<(Id<SkPoint>, bool, [f64; 2])>,
    /// A circle or arc the segment from the previous point touches at this point.
    pub tangent_to: Option<Id<Curve>>,
}

fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

fn xy(sketch: &Sketch, point: Id<SkPoint>) -> Option<[f64; 2]> {
    sketch.pos(point).map(|p| [p[0], p[1]])
}

/// The point of a curve closest to `at`, where the curve is a segment, a circle or an arc.
fn closest_on_curve(sketch: &Sketch, curve: &Curve, at: [f64; 2]) -> Option<[f64; 2]> {
    match curve.geom {
        CurveGeom::Line { a, b } => {
            let (a, b) = (xy(sketch, a)?, xy(sketch, b)?);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let length2 = dx * dx + dy * dy;
            let t = if length2 > 0.0 {
                (((at[0] - a[0]) * dx + (at[1] - a[1]) * dy) / length2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            Some([a[0] + t * dx, a[1] + t * dy])
        }
        CurveGeom::Circle { c, r } => {
            let c = xy(sketch, c)?;
            let d = distance(at, c);
            (d > 0.0).then(|| [c[0] + (at[0] - c[0]) * r / d, c[1] + (at[1] - c[1]) * r / d])
        }
        CurveGeom::Arc { c, s, e } => {
            let (c, s, e) = (xy(sketch, c)?, xy(sketch, s)?, xy(sketch, e)?);
            let (r, d) = (distance(s, c), distance(at, c));
            if d == 0.0 {
                return None;
            }
            let angle = |p: [f64; 2]| fm::atan2(p[1] - c[1], p[0] - c[0]);
            let tau = std::f64::consts::TAU;
            let sweep = (angle(e) - angle(s)).rem_euclid(tau);
            let along = (angle(at) - angle(s)).rem_euclid(tau);
            if along <= sweep {
                Some([c[0] + (at[0] - c[0]) * r / d, c[1] + (at[1] - c[1]) * r / d])
            } else if distance(at, s) <= distance(at, e) {
                Some(s)
            } else {
                Some(e)
            }
        }
    }
}

/// The nearest point within the point tolerance. Among coincident points the lowest id wins.
fn nearest_point(
    sketch: &Sketch,
    at: [f64; 2],
    px: f64,
    skip: &[Id<SkPoint>],
) -> Option<(Id<SkPoint>, [f64; 2])> {
    let mut best: Option<(f64, Id<SkPoint>, [f64; 2])> = None;
    for (id, point) in &sketch.points {
        if skip.contains(id) {
            continue;
        }
        let pos = [point.pos[0], point.pos[1]];
        let d = distance(pos, at);
        if d <= tolerance::POINT_PX * px && best.is_none_or(|(least, ..)| d < least) {
            best = Some((d, *id, pos));
        }
    }
    best.map(|(_, id, pos)| (id, pos))
}

/// The nearest curve within the curve tolerance, with the closest position on it.
pub(crate) fn nearest_curve(
    sketch: &Sketch,
    at: [f64; 2],
    px: f64,
    skip: &[Id<Curve>],
) -> Option<(Id<Curve>, [f64; 2])> {
    let mut best: Option<(f64, Id<Curve>, [f64; 2])> = None;
    for (id, curve) in &sketch.curves {
        if skip.contains(id) {
            continue;
        }
        let Some(pos) = closest_on_curve(sketch, curve, at) else { continue };
        let d = distance(pos, at);
        if d <= tolerance::CURVE_PX * px && best.is_none_or(|(least, ..)| d < least) {
            best = Some((d, *id, pos));
        }
    }
    best.map(|(_, id, pos)| (id, pos))
}

/// What a click at `at` selects: a point before a curve before a dimension text. `px` is the size of one
/// screen pixel in sketch units.
pub fn pick(sketch: &Sketch, at: [f64; 2], px: f64) -> Option<Item> {
    if let Some((point, _)) = nearest_point(sketch, at, px, &[]) {
        return Some(Item::Point(point));
    }
    if let Some((curve, _)) = nearest_curve(sketch, at, px, &[]) {
        return Some(Item::Curve(curve));
    }
    sketch
        .dimensions
        .iter()
        .find(|(_, d)| distance(d.text_pos, at) <= tolerance::TEXT_PX * px)
        .map(|(id, _)| Item::Dimension(*id))
}

/// Where two curves (lines, circles, arcs) cross, within the curves' extents.
fn intersections(sketch: &Sketch, a: &Curve, b: &Curve) -> Vec<[f64; 2]> {
    let (Some(first), Some(second)) = (curve_shape(sketch, a), curve_shape(sketch, b)) else {
        return Vec::new();
    };
    crossings(&first, &second)
}

/// A curve as a shape to intersect.
pub(crate) fn curve_shape(sketch: &Sketch, c: &Curve) -> Option<Shape> {
    Some(match c.geom {
        CurveGeom::Line { a, b } => Shape::Segment(xy(sketch, a)?, xy(sketch, b)?),
        CurveGeom::Circle { c, r } => Shape::Round(xy(sketch, c)?, r, None),
        CurveGeom::Arc { c, s, e } => {
            let (c, s, e) = (xy(sketch, c)?, xy(sketch, s)?, xy(sketch, e)?);
            Shape::Round(c, distance(c, s), Some((s, e)))
        }
    })
}

/// A curve for intersecting: a segment, or a circle (with the arc's ends when it is an arc).
#[derive(Clone, Copy, Debug)]
pub(crate) enum Shape {
    Segment([f64; 2], [f64; 2]),
    Round([f64; 2], f64, Option<([f64; 2], [f64; 2])>),
}

impl Shape {
    /// True when a point known to lie on the full line or circle lies on this part of it.
    fn holds(&self, p: [f64; 2]) -> bool {
        match *self {
            Shape::Segment(a, b) => {
                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                let l2 = dx * dx + dy * dy;
                l2 > 0.0 && {
                    let t = ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2;
                    (-1e-9..=1.0 + 1e-9).contains(&t)
                }
            }
            Shape::Round(_, _, None) => true,
            Shape::Round(c, _, Some((s, e))) => {
                let angle = |q: [f64; 2]| fm::atan2(q[1] - c[1], q[0] - c[0]);
                let tau = std::f64::consts::TAU;
                (angle(p) - angle(s)).rem_euclid(tau) <= (angle(e) - angle(s)).rem_euclid(tau) + 1e-9
            }
        }
    }
}

/// Where two shapes cross.
pub(crate) fn crossings(first: &Shape, second: &Shape) -> Vec<[f64; 2]> {
    let candidates = match (*first, *second) {
        (Shape::Segment(a, b), Shape::Segment(c, d)) => {
            let (r, s) = ([b[0] - a[0], b[1] - a[1]], [d[0] - c[0], d[1] - c[1]]);
            let denom = r[0] * s[1] - r[1] * s[0];
            if denom.abs() < 1e-15 {
                Vec::new()
            } else {
                let t = ((c[0] - a[0]) * s[1] - (c[1] - a[1]) * s[0]) / denom;
                vec![[a[0] + t * r[0], a[1] + t * r[1]]]
            }
        }
        (Shape::Segment(a, b), Shape::Round(c, r, _)) | (Shape::Round(c, r, _), Shape::Segment(a, b)) => {
            let d = [b[0] - a[0], b[1] - a[1]];
            let f = [a[0] - c[0], a[1] - c[1]];
            let (qa, qb, qc) = (
                d[0] * d[0] + d[1] * d[1],
                2.0 * (f[0] * d[0] + f[1] * d[1]),
                f[0] * f[0] + f[1] * f[1] - r * r,
            );
            let disc = qb * qb - 4.0 * qa * qc;
            if qa <= 0.0 || disc < 0.0 {
                Vec::new()
            } else {
                let root = disc.sqrt();
                [(-qb - root) / (2.0 * qa), (-qb + root) / (2.0 * qa)]
                    .iter()
                    .map(|t| [a[0] + t * d[0], a[1] + t * d[1]])
                    .collect()
            }
        }
        (Shape::Round(c1, r1, _), Shape::Round(c2, r2, _)) => {
            let d = distance(c1, c2);
            if d <= 0.0 || d > r1 + r2 || d < (r1 - r2).abs() {
                Vec::new()
            } else {
                let along = (r1 * r1 - r2 * r2 + d * d) / (2.0 * d);
                let h = (r1 * r1 - along * along).max(0.0).sqrt();
                let u = [(c2[0] - c1[0]) / d, (c2[1] - c1[1]) / d];
                let m = [c1[0] + u[0] * along, c1[1] + u[1] * along];
                vec![[m[0] - u[1] * h, m[1] + u[0] * h], [m[0] + u[1] * h, m[1] - u[0] * h]]
            }
        }
    };
    candidates.into_iter().filter(|p| first.holds(*p) && second.holds(*p)).collect()
}

/// Where a new point should go for the cursor at `at`.
///
/// Priority: an existing point, an intersection, a line's midpoint, a circle's quadrant point, the point
/// where the segment from `previous` touches a circle, any position on a curve; otherwise level or plumb
/// with `previous` (the start of the segment being drawn) and with other points, on either axis. With
/// `suppress` (Ctrl held) the cursor position is used as it is.
pub fn snap(sketch: &Sketch, at: [f64; 2], px: f64, previous: Option<[f64; 2]>, suppress: bool) -> Snap {
    let free = Snap {
        pos: at,
        kind: SnapKind::Free,
        point: None,
        curve: None,
        horizontal: None,
        inference: None,
        also_on: None,
        midpoint_of: None,
        aligned: None,
        tangent_to: None,
    };
    if suppress {
        return free;
    }
    let near = tolerance::POINT_PX * px;
    if let Some((point, pos)) = nearest_point(sketch, at, px, &[]) {
        let centre = sketch
            .curves
            .values()
            .any(|c| matches!(c.geom, CurveGeom::Circle { c, .. } | CurveGeom::Arc { c, .. } if c == point));
        return Snap {
            pos,
            kind: SnapKind::Point,
            point: Some(point),
            inference: centre.then_some(Inference::Centre),
            ..free
        };
    }
    // Intersections of the curves close to the cursor.
    let close: Vec<(Id<Curve>, &Curve)> = sketch
        .curves
        .iter()
        .filter(|(_, c)| closest_on_curve(sketch, c, at).is_some_and(|p| distance(p, at) <= 2.0 * near))
        .map(|(id, c)| (*id, c))
        .collect();
    let mut best: Option<(f64, Snap)> = None;
    let mut offer = |d: f64, snap: Snap| {
        if d <= near && best.is_none_or(|(least, _)| d < least) {
            best = Some((d, snap));
        }
    };
    for (i, (ida, a)) in close.iter().enumerate() {
        for (idb, b) in &close[i + 1..] {
            for pos in intersections(sketch, a, b) {
                let snap = Snap {
                    pos,
                    kind: SnapKind::OnCurve,
                    curve: Some(*ida),
                    also_on: Some(*idb),
                    inference: Some(Inference::Intersection),
                    ..free
                };
                offer(distance(pos, at), snap);
            }
        }
    }
    for (id, curve) in &sketch.curves {
        match curve.geom {
            CurveGeom::Line { a, b } => {
                let (Some(a), Some(b)) = (xy(sketch, a), xy(sketch, b)) else { continue };
                let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
                let snap = Snap {
                    pos: mid,
                    kind: SnapKind::OnCurve,
                    curve: Some(*id),
                    midpoint_of: Some(*id),
                    inference: Some(Inference::Midpoint),
                    ..free
                };
                offer(distance(mid, at), snap);
            }
            CurveGeom::Circle { c, r } => {
                let (Some(centre_id), Some(c)) = (Some(c), xy(sketch, c)) else { continue };
                for (k, offset) in [[r, 0.0], [0.0, r], [-r, 0.0], [0.0, -r]].into_iter().enumerate() {
                    let pos = [c[0] + offset[0], c[1] + offset[1]];
                    // A quadrant point is on the circle and level (0°, 180°) or plumb (90°, 270°) with the centre.
                    let snap = Snap {
                        pos,
                        kind: SnapKind::OnCurve,
                        curve: Some(*id),
                        aligned: Some((centre_id, k % 2 == 0, c)),
                        inference: Some(Inference::Quadrant),
                        ..free
                    };
                    offer(distance(pos, at), snap);
                }
            }
            CurveGeom::Arc { .. } => {}
        }
    }
    // Tangent from the previous point to a circle or arc.
    if let Some(from) = previous {
        for (id, curve) in &sketch.curves {
            let (centre, radius, shape) = match curve.geom {
                CurveGeom::Circle { c, r } => match xy(sketch, c) {
                    Some(c) => (c, r, Shape::Round(c, r, None)),
                    None => continue,
                },
                CurveGeom::Arc { c, s, e } => match (xy(sketch, c), xy(sketch, s), xy(sketch, e)) {
                    (Some(c), Some(s), Some(e)) => {
                        (c, distance(c, s), Shape::Round(c, distance(c, s), Some((s, e))))
                    }
                    _ => continue,
                },
                CurveGeom::Line { .. } => continue,
            };
            let d = distance(from, centre);
            if d <= radius * (1.0 + 1e-9) {
                continue;
            }
            // The touching points: seen from the centre, at ±acos(r/d) from the direction to `from`.
            let base = fm::atan2(from[1] - centre[1], from[0] - centre[0]);
            let spread = fm::acos((radius / d).clamp(-1.0, 1.0));
            for angle in [base + spread, base - spread] {
                let pos = [centre[0] + radius * fm::cos(angle), centre[1] + radius * fm::sin(angle)];
                if shape.holds(pos) {
                    let snap = Snap {
                        pos,
                        kind: SnapKind::OnCurve,
                        tangent_to: Some(*id),
                        inference: Some(Inference::Tangent),
                        ..free
                    };
                    offer(distance(pos, at), snap);
                }
            }
        }
    }
    if let Some((_, snap)) = best {
        return snap;
    }
    if let Some((curve, pos)) = nearest_curve(sketch, at, px, &[]) {
        return Snap { pos, kind: SnapKind::OnCurve, curve: Some(curve), ..free };
    }
    let reach = tolerance::DIRECTION_PX * px;
    let mut result = free;
    // Level or plumb with the previous point: the segment is horizontal or vertical.
    if let Some(previous) = previous {
        let (dx, dy) = ((at[0] - previous[0]).abs(), (at[1] - previous[1]).abs());
        if dy <= reach && dx > reach {
            result = Snap {
                pos: [at[0], previous[1]],
                kind: SnapKind::Horizontal,
                horizontal: Some(true),
                ..result
            };
        } else if dx <= reach && dy > reach {
            result = Snap {
                pos: [previous[0], at[1]],
                kind: SnapKind::Vertical,
                horizontal: Some(false),
                ..result
            };
        }
    }
    // Level or plumb with another point, on the axis the previous point does not already fix.
    let fixed_y = result.horizontal == Some(true);
    let fixed_x = result.horizontal == Some(false);
    let mut alignment: Option<(f64, Id<SkPoint>, bool, [f64; 2])> = None;
    for (id, point) in &sketch.points {
        let pos = [point.pos[0], point.pos[1]];
        if previous.is_some_and(|p| distance(p, pos) < 1e-12) {
            continue;
        }
        let (dx, dy) = ((at[0] - pos[0]).abs(), (at[1] - pos[1]).abs());
        if !fixed_x && dx <= reach && dy > near && alignment.is_none_or(|(least, ..)| dx < least) {
            alignment = Some((dx, *id, false, pos));
        }
        if !fixed_y && dy <= reach && dx > near && alignment.is_none_or(|(least, ..)| dy < least) {
            alignment = Some((dy, *id, true, pos));
        }
    }
    if let Some((_, id, level, pos)) = alignment {
        let snapped = if level { [result.pos[0], pos[1]] } else { [pos[0], result.pos[1]] };
        result = Snap {
            pos: snapped,
            aligned: Some((id, level, pos)),
            inference: Some(Inference::Aligned),
            ..result
        };
    }
    result
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use gc_core::{AnyId, IdAlloc, Kind};
    use gc_sketch::Space;

    use super::*;

    fn sketch() -> (Sketch, IdAlloc) {
        let frame = AnyId { kind: Kind::Frame, raw: NonZeroU64::new(1).unwrap() };
        let mut alloc = IdAlloc::new();
        alloc.reserve_through(frame.raw);
        (Sketch::new(Space::Planar(frame)), alloc)
    }

    #[test]
    fn points_win_over_curves_and_curves_over_nothing() {
        let (mut s, mut ids) = sketch();
        let line = s.add_line(&mut ids, [0.0, 0.0, 0.0], [10.0, 0.0, 0.0]);
        let (a, _) = s.line(line).unwrap();
        let px = 0.1; // one pixel = 0.1 units, so 8 px = 0.8
        assert_eq!(pick(&s, [0.5, 0.3], px), Some(Item::Point(a)));
        assert_eq!(pick(&s, [5.0, 0.5], px), Some(Item::Curve(line)));
        assert_eq!(pick(&s, [5.0, 0.7], px), None);
        assert_eq!(pick(&s, [11.5, 0.0], px), None, "beyond the end of the segment");
    }

    #[test]
    fn snapping_reports_what_it_implies() {
        let (mut s, mut ids) = sketch();
        let line = s.add_line(&mut ids, [0.0, 0.0, 0.0], [10.0, 0.0, 0.0]);
        let (_, b) = s.line(line).unwrap();
        let px = 0.1;
        let to_point = snap(&s, [9.8, 0.3], px, None, false);
        assert_eq!((to_point.kind, to_point.point, to_point.pos), (SnapKind::Point, Some(b), [10.0, 0.0]));
        let to_curve = snap(&s, [4.0, 0.4], px, None, false);
        assert_eq!(
            (to_curve.kind, to_curve.curve, to_curve.pos),
            (SnapKind::OnCurve, Some(line), [4.0, 0.0])
        );
        let level = snap(&s, [7.0, 5.3], px, Some([2.0, 5.0]), false);
        assert_eq!((level.kind, level.horizontal, level.pos), (SnapKind::Horizontal, Some(true), [7.0, 5.0]));
        let plumb = snap(&s, [2.2, 9.0], px, Some([2.0, 5.0]), false);
        assert_eq!((plumb.kind, plumb.horizontal, plumb.pos), (SnapKind::Vertical, Some(false), [2.0, 9.0]));
        assert_eq!(snap(&s, [7.0, 7.0], px, Some([2.0, 5.0]), false).kind, SnapKind::Free);
        assert_eq!(snap(&s, [9.8, 0.3], px, None, true).pos, [9.8, 0.3], "Ctrl suppresses snapping");
    }

    #[test]
    fn arcs_are_only_hit_inside_their_sweep() {
        let (mut s, mut ids) = sketch();
        // Quarter arc from (5, 0) to (0, 5) about the origin.
        let arc = s.add_arc(&mut ids, [0.0; 3], [5.0, 0.0, 0.0], [0.0, 5.0, 0.0]).unwrap();
        let px = 0.05;
        let on = 5.0 * std::f64::consts::FRAC_1_SQRT_2;
        assert_eq!(pick(&s, [on + 0.1, on + 0.1], px), Some(Item::Curve(arc)));
        assert_eq!(pick(&s, [-on, -on], px), None, "the far side of the circle is not part of the arc");
    }
}
