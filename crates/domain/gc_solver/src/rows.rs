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
            Self::Cross { .. }
            | Self::Dot { .. }
            | Self::EqualLen
            | Self::Angle { .. }
            | Self::MidOnLine { .. } => 8,
            Self::PointLine3 { .. } => 9,
            Self::PointSphere => 7,
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
        let eight = [0.1, 0.2, 2.3, 1.1, -0.4, 0.9, 1.7, 2.6];
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
