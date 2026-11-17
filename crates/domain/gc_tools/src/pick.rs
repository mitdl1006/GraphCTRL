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
fn nearest_curve(
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

/// Where a new point should go for the cursor at `at`.
///
/// Priority: an existing point, then a curve, then level or plumb with `previous` (the start of the
/// segment being drawn). With `suppress` (Ctrl held) the cursor position is used as it is.
pub fn snap(sketch: &Sketch, at: [f64; 2], px: f64, previous: Option<[f64; 2]>, suppress: bool) -> Snap {
    let free = Snap { pos: at, kind: SnapKind::Free, point: None, curve: None, horizontal: None };
    if suppress {
        return free;
    }
    if let Some((point, pos)) = nearest_point(sketch, at, px, &[]) {
        return Snap { pos, kind: SnapKind::Point, point: Some(point), ..free };
    }
    if let Some((curve, pos)) = nearest_curve(sketch, at, px, &[]) {
        return Snap { pos, kind: SnapKind::OnCurve, curve: Some(curve), ..free };
    }
    if let Some(previous) = previous {
        let reach = tolerance::DIRECTION_PX * px;
        let (dx, dy) = ((at[0] - previous[0]).abs(), (at[1] - previous[1]).abs());
        if dy <= reach && dx > reach {
            return Snap {
                pos: [at[0], previous[1]],
                kind: SnapKind::Horizontal,
                horizontal: Some(true),
                ..free
            };
        }
        if dx <= reach && dy > reach {
            return Snap {
                pos: [previous[0], at[1]],
                kind: SnapKind::Vertical,
                horizontal: Some(false),
                ..free
            };
        }
    }
    free
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
