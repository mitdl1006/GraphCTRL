//! Residual rows: the scalar equations a constraint lowers to.
//!
//! Every residual is a length (metres), so one tolerance scaled by the sketch size applies to all of them.
//! No residual produces NaN: where a formula has no derivative (a zero-length line, a point at a circle's
//! centre) a fallback derivative is used and the row is flagged degenerate.

use std::f64::consts::{PI, TAU};

use crate::scalar::Scalar;

/// What one row computes. The comment on each variant gives the order of its inputs.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum RowKind {
    /// `[a]`: `a - target`.
    Value { target: f64 },
    /// `[a, b]`: `b - a - target`.
    Diff { target: f64 },
    /// `[p.., q..]` (`dim` coordinates each): `|q - p| - target`.
    Distance { dim: usize, target: f64 },
    /// `[px, py, ax, ay, bx, by]`: signed distance of `p` from the line `a -> b` (left positive) minus
    /// `target`. With `frozen` the cross product is divided by that constant instead of the line's length.
    PointLine { frozen: Option<f64>, target: f64 },
    /// `[px, py, cx, cy, r]`: `|p - c| - r`.
    PointCircle,
    /// `[px, py, cx, cy, mx, my, b]`: the point on the ellipse with centre `c`, major axis end `m` and
    /// minor radius `b`: `(|(x / a, y / b)| - 1) * b`, with `(x, y)` the point in the ellipse's axes and
    /// `a = |m - c|` (a length, zero on the ellipse).
    PointEllipse,
    /// `[p, a, b]` (one coordinate each): `p - (a + b) / 2`.
    Mid,
    /// `[ax, ay, bx, by, cx, cy, dx, dy]`: `cross(b - a, d - c) / scale`.
    Cross { scale: f64 },
    /// Same inputs: `dot(b - a, d - c) / scale`.
    Dot { scale: f64 },
    /// `[ax, ay, bx, by, cx, cy, mx, my, b]`: the line touches the ellipse with centre `c`, major axis end
    /// `m` and minor radius `b`: the centre's signed distance from the line minus `side` times the
    /// ellipse's reach towards the line, `sqrt((n·w)² + b²(d·w / |w|)²)` (`w = m - c`, `d` and `n` the
    /// line's unit direction and left normal).
    LineEllipseTangent { side: f64 },
    /// `[ax, ay, bx, by, px, py, cx, cy, mx, my, b]`: the line runs along the ellipse at its point `p`: the
    /// cosine between the line and the ellipse's normal there (`(x / a², y / b²)` in its axes), times `a`.
    EndTangentEllipse,
    /// `[px, py, cx, cy, mx, my, b, qx, qy]`: the ellipse's normal at its point `p` passes through `q`: the
    /// cross product of the unit normal there with `q - p` (a length).
    EllipseNormalThrough,
    /// `[px, py, c1.., m1.., b1, c2.., m2.., b2]`: the two ellipses' normals at `p` are parallel: the cross
    /// product of the unit normals, times `scale`.
    EllipseNormals { scale: f64 },
    /// `[ax, ay, bx, by, cx, cy, r]`: signed distance of `c` from the line minus `side * (r + gap)` (zero gap:
    /// tangent; else the line keeps `gap` from the circle itself).
    LineCircleTangent { side: f64, gap: f64 },
    /// `[c1x, c1y, r1, c2x, c2y, r2]`: `|c2 - c1| - (r1 + r2) - gap` (external) or
    /// `|c2 - c1| - sign * (r1 - r2) + gap` (one inside the other, `gap` between the circles themselves).
    CircleCircle { external: bool, sign: f64, gap: f64 },
    /// `[ax, ay, bx, by, cx, cy, dx, dy]`: `|b - a| - |d - c|`.
    EqualLen,
    /// Same inputs: `scale * wrap(angle from (b - a) to (d - c) - target)`, each direction optionally reversed.
    Angle { target: f64, scale: f64, rev1: bool, rev2: bool },
    /// `[px, py, qx, qy, ax, ay, bx, by]`: signed distance of the midpoint of `p q` from the line, with the
    /// cross product divided by `frozen`.
    MidOnLine { frozen: f64 },
    /// `[a.., b.., c.., d..]` (3D points): `scale` times component `k` of `cross(b - a, d - c)` over both
    /// lengths (a sine). Divided by the live lengths, so a line shrinking to a point does not satisfy it.
    Cross3 { scale: f64, k: usize },
    /// Same inputs: `scale` times `dot(b - a, d - c)` over both lengths (a cosine).
    Dot3 { scale: f64 },
    /// Same inputs: `|b - a| - |d - c|`.
    EqualLen3,
    /// Same inputs: `scale * (angle between (b - a) and (d - c) - target)`, the angle in `[0, pi]`, each
    /// direction optionally reversed.
    Angle3 { target: f64, scale: f64, rev1: bool, rev2: bool },
    /// `[p.., a.., b..]` (3D points): component `k` of `cross(b - a, p - a) / |b - a|` (the point's offset
    /// from the line across it).
    PointLine3 { k: usize },
    /// `[px, py, pz, cx, cy, cz, r]`: `|p - c| - r` (a point of a 3D arc).
    PointSphere,
    /// `[p.., c.., s.., e..]` (3D points): the signed distance of `p` from the plane through `c`, `s` and
    /// `e` (a point in a 3D arc's plane).
    Coplanar,
}

/// Lengths below this (relative to the sketch size) have no usable direction.
pub(crate) const DEGENERATE: f64 = 1e-9;

/// `sqrt(dx² + dy² (+ dz²))` with a fallback derivative when the length vanishes.
fn length<S: Scalar>(d: &[S], size: f64, degenerate: &mut bool) -> S {
    let mut n2 = S::constant(0.0);
    for c in d {
        n2 = n2 + *c * *c;
    }
    let value = n2.value().sqrt();
    if value <= DEGENERATE * size {
        *degenerate = true;
        // Pretend the vector points along its first axis: finite derivative, right value.
        d[0].with_value(value)
    } else {
        n2.sqrt()
    }
}

/// The unit normal of the ellipse (centre `c`, major axis end `m`, minor radius `b`) at its point `p`, from
/// `[px, py, cx, cy, mx, my, b]`; `None` (degenerate) for no ellipse to speak of.
fn ellipse_normal<S: Scalar>(v: &[S; 7], size: f64, degenerate: &mut bool) -> Option<(S, S)> {
    let (wx, wy) = (v[4] - v[2], v[5] - v[3]);
    let a = length(&[wx, wy], size, degenerate);
    let b = v[6];
    if a.value() == 0.0 || b.value().abs() <= DEGENERATE * size {
        *degenerate = true;
        return None;
    }
    let (ux, uy) = (wx / a, wy / a);
    let (px, py) = (v[0] - v[2], v[1] - v[3]);
    let (x, y) = (px * ux + py * uy, py * ux - px * uy);
    let (nl_x, nl_y) = (x / (a * a), y / (b * b));
    let (nx, ny) = (ux * nl_x - uy * nl_y, uy * nl_x + ux * nl_y);
    let n = length(&[nx, ny], size / (size * size).max(1e-300), degenerate);
    if n.value() == 0.0 {
        return None;
    }
    Some((nx / n, ny / n))
}

/// `v[to..to + 3] - v[from..from + 3]`.
fn sub3<S: Scalar>(v: &[S], to: usize, from: usize) -> [S; 3] {
    [v[to] - v[from], v[to + 1] - v[from + 1], v[to + 2] - v[from + 2]]
}

fn cross3<S: Scalar>(u: [S; 3], w: [S; 3]) -> [S; 3] {
    [u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]]
}

/// Wraps an angle difference into `(-pi, pi]`; a value within 1e-12 of `-pi` counts as `+pi`.
pub(crate) fn wrap_angle(a: f64) -> f64 {
    let w = a - TAU * ((a + PI) / TAU).floor();
    if w <= -PI + 1e-12 { w + TAU } else { w }
}

impl RowKind {
    /// Number of inputs.
    #[cfg(test)]
    pub fn arity(&self) -> usize {
        match self {
            Self::Value { .. } => 1,
            Self::Diff { .. } => 2,
            Self::Distance { dim, .. } => 2 * dim,
            Self::Mid => 3,
            Self::PointCircle => 5,
            Self::PointEllipse => 7,
            Self::PointLine { .. } | Self::CircleCircle { .. } => 6,
            Self::LineCircleTangent { .. } => 7,
            Self::LineEllipseTangent { .. } => 9,
            Self::EllipseNormalThrough => 9,
            Self::EllipseNormals { .. } => 12,
            Self::EndTangentEllipse => 11,
            Self::Cross { .. }
            | Self::Dot { .. }
            | Self::EqualLen
            | Self::Angle { .. }
            | Self::MidOnLine { .. } => 8,
            Self::PointLine3 { .. } => 9,
            Self::PointSphere => 7,
            Self::Coplanar => 12,
            Self::Cross3 { .. } | Self::Dot3 { .. } | Self::EqualLen3 | Self::Angle3 { .. } => 12,
        }
    }

    /// Evaluates the residual. `size` is the sketch scale used by the degeneracy guards.
    pub fn eval<S: Scalar>(&self, v: &[S], size: f64, degenerate: &mut bool) -> S {
        let c = S::constant;
        match *self {
            Self::Value { target } => v[0] - c(target),
            Self::Diff { target } => v[1] - v[0] - c(target),
            Self::Distance { dim, target } => {
                let mut d = [c(0.0); 3];
                for k in 0..dim {
                    d[k] = v[dim + k] - v[k];
                }
                length(&d[..dim], size, degenerate) - c(target)
            }
            Self::PointLine { frozen, target } => {
                let (dx, dy) = (v[4] - v[2], v[5] - v[3]);
                let cross = dx * (v[1] - v[3]) - dy * (v[0] - v[2]);
                let divisor = match frozen {
                    Some(scale) => c(scale),
                    None => length(&[dx, dy], size, degenerate),
                };
                if divisor.value() == 0.0 { cross - c(target) } else { cross / divisor - c(target) }
            }
            Self::PointCircle => length(&[v[0] - v[2], v[1] - v[3]], size, degenerate) - v[4],
            Self::PointEllipse => {
                let (ux, uy) = (v[0] - v[2], v[1] - v[3]);
                let (dx, dy) = (v[4] - v[2], v[5] - v[3]);
                let a = length(&[dx, dy], size, degenerate);
                let b = v[6];
                if a.value() <= DEGENERATE * size || b.value().abs() <= DEGENERATE * size {
                    // No ellipse to speak of: keep the point at the major radius from the centre.
                    *degenerate = true;
                    return length(&[ux, uy], size, degenerate) - a;
                }
                let x = (ux * dx + uy * dy) / a;
                let y = (dx * uy - dy * ux) / a;
                (length(&[x / a, y / b], size, degenerate) - c(1.0)) * b
            }
            Self::Mid => v[0] - (v[1] + v[2]) / c(2.0),
            Self::Cross { scale } => {
                ((v[2] - v[0]) * (v[7] - v[5]) - (v[3] - v[1]) * (v[6] - v[4])) / c(scale)
            }
            Self::Dot { scale } => ((v[2] - v[0]) * (v[6] - v[4]) + (v[3] - v[1]) * (v[7] - v[5])) / c(scale),
            Self::LineCircleTangent { side, gap } => {
                let (dx, dy) = (v[2] - v[0], v[3] - v[1]);
                let cross = dx * (v[5] - v[1]) - dy * (v[4] - v[0]);
                let len = length(&[dx, dy], size, degenerate);
                let distance = if len.value() == 0.0 { cross } else { cross / len };
                distance - c(side) * (v[6] + c(gap))
            }
            Self::LineEllipseTangent { side } => {
                let (dx, dy) = (v[2] - v[0], v[3] - v[1]);
                let l = length(&[dx, dy], size, degenerate);
                let (wx, wy) = (v[6] - v[4], v[7] - v[5]);
                let a = length(&[wx, wy], size, degenerate);
                if l.value() == 0.0 || a.value() == 0.0 {
                    return c(0.0);
                }
                let distance = (dx * (v[5] - v[1]) - dy * (v[4] - v[0])) / l;
                let across = (dx * wy - dy * wx) / l;
                let along = (dx * wx + dy * wy) / (l * a);
                distance - c(side) * length(&[across, v[8] * along], size, degenerate)
            }
            Self::EllipseNormalThrough => {
                let Some((nx, ny)) =
                    ellipse_normal(&[v[0], v[1], v[2], v[3], v[4], v[5], v[6]], size, degenerate)
                else {
                    return c(0.0);
                };
                nx * (v[8] - v[1]) - ny * (v[7] - v[0])
            }
            Self::EllipseNormals { scale } => {
                let first = ellipse_normal(&[v[0], v[1], v[2], v[3], v[4], v[5], v[6]], size, degenerate);
                let second = ellipse_normal(&[v[0], v[1], v[7], v[8], v[9], v[10], v[11]], size, degenerate);
                let (Some((ax, ay)), Some((bx, by))) = (first, second) else { return c(0.0) };
                (ax * by - ay * bx) * c(scale)
            }
            Self::EndTangentEllipse => {
                let (dx, dy) = (v[2] - v[0], v[3] - v[1]);
                let l = length(&[dx, dy], size, degenerate);
                let (wx, wy) = (v[8] - v[6], v[9] - v[7]);
                let a = length(&[wx, wy], size, degenerate);
                let b = v[10];
                if l.value() == 0.0 || a.value() == 0.0 || b.value().abs() <= DEGENERATE * size {
                    *degenerate = true;
                    return c(0.0);
                }
                let (ux, uy) = (wx / a, wy / a);
                let (px, py) = (v[4] - v[6], v[5] - v[7]);
                let (x, y) = (px * ux + py * uy, py * ux - px * uy);
                let (nx_local, ny_local) = (x / (a * a), y / (b * b));
                // The normal back in the sketch: along u and the left of u.
                let (nx, ny) = (ux * nx_local - uy * ny_local, uy * nx_local + ux * ny_local);
                let n = length(&[nx, ny], size, degenerate);
                if n.value() == 0.0 {
                    return c(0.0);
                }
                (dx * nx + dy * ny) / (l * n) * a
            }
            Self::CircleCircle { external, sign, gap } => {
                let apart = length(&[v[3] - v[0], v[4] - v[1]], size, degenerate);
                if external {
                    apart - (v[2] + v[5]) - c(gap)
                } else {
                    apart - c(sign) * (v[2] - v[5]) + c(gap)
                }
            }
            Self::EqualLen => {
                length(&[v[2] - v[0], v[3] - v[1]], size, degenerate)
                    - length(&[v[6] - v[4], v[7] - v[5]], size, degenerate)
            }
            Self::Angle { target, scale, rev1, rev2 } => {
                let flip = |on: bool| c(if on { -1.0 } else { 1.0 });
                let (ux, uy) = ((v[2] - v[0]) * flip(rev1), (v[3] - v[1]) * flip(rev1));
                let (wx, wy) = ((v[6] - v[4]) * flip(rev2), (v[7] - v[5]) * flip(rev2));
                let short =
                    |x: S, y: S| x.value() * x.value() + y.value() * y.value() <= (DEGENERATE * size).powi(2);
                if short(ux, uy) || short(wx, wy) {
                    *degenerate = true;
                    return c(0.0);
                }
                let angle = S::atan2(ux * wy - uy * wx, ux * wx + uy * wy);
                let error = angle - c(target);
                (error - c(error.value() - wrap_angle(error.value()))) * c(scale)
            }
            Self::MidOnLine { frozen } => {
                let (mx, my) = ((v[0] + v[2]) / c(2.0), (v[1] + v[3]) / c(2.0));
                ((v[6] - v[4]) * (my - v[5]) - (v[7] - v[5]) * (mx - v[4])) / c(frozen)
            }
            Self::Cross3 { scale, k } => {
                let (u, w) = (sub3(v, 3, 0), sub3(v, 9, 6));
                let lengths = length(&u, size, degenerate) * length(&w, size, degenerate);
                if lengths.value() == 0.0 {
                    return c(0.0);
                }
                cross3(u, w)[k] / lengths * c(scale)
            }
            Self::Dot3 { scale } => {
                let (u, w) = (sub3(v, 3, 0), sub3(v, 9, 6));
                let lengths = length(&u, size, degenerate) * length(&w, size, degenerate);
                if lengths.value() == 0.0 {
                    return c(0.0);
                }
                (u[0] * w[0] + u[1] * w[1] + u[2] * w[2]) / lengths * c(scale)
            }
            Self::EqualLen3 => {
                length(&sub3(v, 3, 0), size, degenerate) - length(&sub3(v, 9, 6), size, degenerate)
            }
            Self::Angle3 { target, scale, rev1, rev2 } => {
                let flip = |on: bool| c(if on { -1.0 } else { 1.0 });
                let (u, w) = (sub3(v, 3, 0), sub3(v, 9, 6));
                let (u, w) = (u.map(|x| x * flip(rev1)), w.map(|x| x * flip(rev2)));
                let short = |d: [S; 3]| {
                    d.iter().map(|x| x.value() * x.value()).sum::<f64>() <= (DEGENERATE * size).powi(2)
                };
                if short(u) || short(w) {
                    *degenerate = true;
                    return c(0.0);
                }
                let across = length(&cross3(u, w), size, degenerate);
                let along = u[0] * w[0] + u[1] * w[1] + u[2] * w[2];
                (S::atan2(across, along) - c(target)) * c(scale)
            }
            Self::PointSphere => length(&sub3(v, 0, 3), size, degenerate) - v[6],
            Self::Coplanar => {
                let normal = cross3(sub3(v, 6, 3), sub3(v, 9, 3));
                let n = length(&normal, size * size, degenerate);
                if n.value() == 0.0 {
                    return c(0.0);
                }
                let p = sub3(v, 0, 3);
                (p[0] * normal[0] + p[1] * normal[1] + p[2] * normal[2]) / n
            }
            Self::PointLine3 { k } => {
                let (d, p) = (sub3(v, 6, 3), sub3(v, 0, 3));
                let len = length(&d, size, degenerate);
                if len.value() == 0.0 {
                    return c(0.0);
                }
                cross3(d, p)[k] / len
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scalar::{Dual, MAX_VARS};

    fn check_gradient(kind: &RowKind, at: &[f64]) {
        let mut flag = false;
        let duals: Vec<Dual> = at.iter().enumerate().map(|(i, v)| Dual::variable(*v, i)).collect();
        let dual = kind.eval(&duals, 1.0, &mut flag);
        assert!((dual.v - kind.eval(at, 1.0, &mut flag)).abs() < 1e-14);
        for slot in 0..at.len() {
            let h = 1e-6;
            let (mut plus, mut minus) = (at.to_vec(), at.to_vec());
            plus[slot] += h;
            minus[slot] -= h;
            let numeric = (kind.eval(&plus, 1.0, &mut flag) - kind.eval(&minus, 1.0, &mut flag)) / (2.0 * h);
            assert!(
                (dual.d[slot] - numeric).abs() < 1e-7,
                "{kind:?} slot {slot}: {} vs {numeric}",
                dual.d[slot]
            );
        }
        assert!(at.len() <= MAX_VARS);
    }

    #[test]
    fn analytic_gradients_match_finite_differences() {
        let eight = [0.1, 0.2, 2.3, 1.1, -0.4, 0.9, 1.7, 2.6, 0.6, 0.15, 0.4];
        let kinds = [
            RowKind::Value { target: 0.3 },
            RowKind::Diff { target: 0.3 },
            RowKind::Distance { dim: 2, target: 1.0 },
            RowKind::Distance { dim: 3, target: 1.0 },
            RowKind::PointLine { frozen: None, target: 0.4 },
            RowKind::PointLine { frozen: Some(2.5), target: 0.0 },
            RowKind::PointCircle,
            RowKind::PointEllipse,
            RowKind::Mid,
            RowKind::Cross { scale: 2.0 },
            RowKind::Dot { scale: 2.0 },
            RowKind::LineCircleTangent { side: -1.0, gap: 0.0 },
            RowKind::LineCircleTangent { side: 1.0, gap: 0.3 },
            RowKind::LineEllipseTangent { side: 1.0 },
            RowKind::LineEllipseTangent { side: -1.0 },
            RowKind::EndTangentEllipse,
            RowKind::EllipseNormalThrough,
            RowKind::CircleCircle { external: true, sign: 1.0, gap: 0.0 },
            RowKind::CircleCircle { external: false, sign: -1.0, gap: 0.0 },
            RowKind::CircleCircle { external: true, sign: 1.0, gap: 0.2 },
            RowKind::CircleCircle { external: false, sign: 1.0, gap: 0.1 },
            RowKind::EqualLen,
            RowKind::Angle { target: 0.7, scale: 1.5, rev1: false, rev2: true },
            RowKind::MidOnLine { frozen: 3.0 },
        ];
        for kind in &kinds {
            check_gradient(kind, &eight[..kind.arity()]);
        }
        let twelve = [0.1, 0.2, 0.3, 2.3, 1.1, -0.7, -0.4, 0.9, 0.5, 1.7, 2.6, -1.2];
        let spatial = [
            RowKind::Cross3 { scale: 2.0, k: 0 },
            RowKind::Cross3 { scale: 2.0, k: 2 },
            RowKind::Dot3 { scale: 1.5 },
            RowKind::EqualLen3,
            RowKind::Angle3 { target: 0.7, scale: 1.5, rev1: false, rev2: true },
            RowKind::PointLine3 { k: 1 },
            RowKind::PointSphere,
            RowKind::Coplanar,
            RowKind::EllipseNormals { scale: 1.5 },
        ];
        for kind in &spatial {
            check_gradient(kind, &twelve[..kind.arity()]);
        }
    }

    #[test]
    fn known_values() {
        let mut flag = false;
        let on_left = RowKind::PointLine { frozen: None, target: 0.0 };
        assert_eq!(on_left.eval(&[1.0, 2.0, 0.0, 0.0, 4.0, 0.0], 1.0, &mut flag), 2.0);
        let quarter = RowKind::Angle { target: 0.0, scale: 1.0, rev1: false, rev2: false };
        let angle = quarter.eval(&[0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0], 1.0, &mut flag);
        assert!((angle - PI / 2.0).abs() < 1e-15);
        assert!(!flag);
    }

    #[test]
    fn degenerate_inputs_are_flagged_and_stay_finite() {
        let cases: [(RowKind, Vec<f64>); 5] = [
            (RowKind::Distance { dim: 2, target: 1.0 }, vec![1.0, 1.0, 1.0, 1.0]),
            (RowKind::PointLine { frozen: None, target: 0.5 }, vec![3.0, 4.0, 1.0, 1.0, 1.0, 1.0]),
            (RowKind::PointCircle, vec![0.0, 0.0, 0.0, 0.0, 5.0]),
            (RowKind::EqualLen, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 2.0]),
            (
                RowKind::Angle { target: 1.0, scale: 1.0, rev1: false, rev2: false },
                vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 2.0],
            ),
        ];
        for (kind, at) in cases {
            let mut flag = false;
            let duals: Vec<Dual> = at.iter().enumerate().map(|(i, v)| Dual::variable(*v, i)).collect();
            let dual = kind.eval(&duals, 1.0, &mut flag);
            assert!(flag, "{kind:?} should be flagged");
            assert!(dual.v.is_finite() && dual.d.iter().all(|d| d.is_finite()), "{kind:?}");
        }
    }

    #[test]
    fn angle_wrapping() {
        assert_eq!(wrap_angle(0.0), 0.0);
        assert_eq!(wrap_angle(PI), PI);
        assert_eq!(wrap_angle(-PI), PI);
        assert!((wrap_angle(1.5 * PI) + 0.5 * PI).abs() < 1e-12);
        assert!((wrap_angle(-PI + 1e-13) - PI).abs() < 1e-12, "just above -pi is taken as +pi");
    }
}
