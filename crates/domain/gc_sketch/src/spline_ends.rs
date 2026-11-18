//! How a spline's ends are shaped by the curves they join: Tangent ("click the first curve … Click the
//! second curve") and Smooth (G2) ("a continuous curvature (G2) condition between a spline and another
//! curve, such as a line, arc, or spline … click the spline and the curve attached to the spline endpoint",
//! To Apply Geometric Constraints, GUID-A85A7A30-7D81-4C75-8769-CAD034EEA930).
//!
//! A spline follows its fit points and has no part in solving. A tangent or smooth end is a condition on the
//! curve through them: the spline leaves its end along the joined curve's direction there (a clamped end),
//! and for Smooth with the joined curve's curvature too (an extra knot near the end, placed so that the
//! curvature matches). The joined curve is not changed.

use gc_core::Id;

use crate::model::{Curve, CurveGeom, GeoConstraint, SkPoint, Sketch, spline_closed};

/// What an end of a spline must match.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SplineEnd {
    /// The unit direction the spline runs in at the end (along its own direction of travel).
    pub tangent: Option<[f64; 3]>,
    /// The curvature vector there (towards the centre of curvature, of length 1/radius), for Smooth (G2).
    pub curvature: Option<[f64; 3]>,
}

type P = [f64; 3];

fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn norm(a: P) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

fn unit(a: P) -> Option<P> {
    let n = norm(a);
    (n > 1e-12 && n.is_finite()).then(|| [a[0] / n, a[1] / n, a[2] / n])
}

fn dot(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The condition at one end of a cubic spline.
#[derive(Clone, Copy, Debug)]
enum Cond {
    /// The second derivative (zero: natural).
    Second(f64),
    /// The first derivative.
    First(f64),
}

/// Second derivatives at the knots of the cubic spline through `y` at parameters `t`.
fn second_derivatives(t: &[f64], y: &[f64], start: Cond, end: Cond) -> Vec<f64> {
    let n = y.len();
    if n < 2 {
        return vec![0.0; n];
    }
    let h: Vec<f64> = (0..n - 1).map(|i| (t[i + 1] - t[i]).max(1e-12)).collect();
    // Tridiagonal rows: lower[i] m[i-1] + diag[i] m[i] + upper[i] m[i+1] = rhs[i].
    let (mut lower, mut diag, mut upper, mut rhs) = (vec![0.0; n], vec![0.0; n], vec![0.0; n], vec![0.0; n]);
    for i in 1..n - 1 {
        lower[i] = h[i - 1];
        diag[i] = 2.0 * (h[i - 1] + h[i]);
        upper[i] = h[i];
        rhs[i] = 6.0 * ((y[i + 1] - y[i]) / h[i] - (y[i] - y[i - 1]) / h[i - 1]);
    }
    match start {
        Cond::Second(m) => {
            diag[0] = 1.0;
            rhs[0] = m;
        }
        Cond::First(d) => {
            diag[0] = 2.0 * h[0];
            upper[0] = h[0];
            rhs[0] = 6.0 * ((y[1] - y[0]) / h[0] - d);
        }
    }
    let last = n - 1;
    match end {
        Cond::Second(m) => {
            diag[last] = 1.0;
            lower[last] = 0.0;
            rhs[last] = m;
        }
        Cond::First(d) => {
            lower[last] = h[last - 1];
            diag[last] = 2.0 * h[last - 1];
            rhs[last] = 6.0 * (d - (y[last] - y[last - 1]) / h[last - 1]);
        }
    }
    // Thomas algorithm.
    for i in 1..n {
        if diag[i - 1].abs() < 1e-300 {
            return vec![0.0; n];
        }
        let w = lower[i] / diag[i - 1];
        diag[i] -= w * upper[i - 1];
        rhs[i] -= w * rhs[i - 1];
    }
    let mut m = vec![0.0; n];
    if diag[last].abs() < 1e-300 {
        return m;
    }
    m[last] = rhs[last] / diag[last];
    for i in (0..last).rev() {
        m[i] = (rhs[i] - upper[i] * m[i + 1]) / diag[i];
    }
    m
}

/// The value of the cubic spline (knots `t`, values `y`, second derivatives `m`) at `s`.
fn value_at(t: &[f64], y: &[f64], m: &[f64], s: f64) -> f64 {
    let n = t.len();
    let mut i = 0;
    while i + 2 < n && s > t[i + 1] {
        i += 1;
    }
    let h = (t[i + 1] - t[i]).max(1e-12);
    let (a, b) = ((t[i + 1] - s) / h, (s - t[i]) / h);
    a * y[i] + b * y[i + 1] + ((a * a * a - a) * m[i] + (b * b * b - b) * m[i + 1]) * h * h / 6.0
}

/// Points along the open spline through `fit` (two or more, in space), `per_span` steps between
/// neighbouring fit points (fit point `k` at index `k * per_span`), with its ends shaped by `start` and
/// `end`: free ends are natural, tangent ends clamped, smooth ends clamped with the curvature matched by an
/// extra knot a third of the way into the end span.
pub fn shaped_spline_points3(fit: &[P], per_span: usize, start: SplineEnd, end: SplineEnd) -> Vec<P> {
    let n = fit.len();
    if n < 2 || per_span == 0 {
        return fit.to_vec();
    }
    // Knots: the fit points, and an extra one near each smooth end (placed along the tangent at first).
    let mut knots: Vec<P> = fit.to_vec();
    let smooth_start = start.tangent.is_some() && start.curvature.is_some();
    let smooth_end = end.tangent.is_some() && end.curvature.is_some();
    let span = |a: P, b: P| norm(sub(b, a)).max(1e-12);
    if smooth_start && let Some(d) = start.tangent {
        let h = span(fit[0], fit[1]) / 3.0;
        knots.insert(1, [fit[0][0] + d[0] * h, fit[0][1] + d[1] * h, fit[0][2] + d[2] * h]);
    }
    if smooth_end && let Some(d) = end.tangent {
        let k = knots.len();
        let h = span(knots[k - 2], knots[k - 1]) / 3.0;
        let last = knots[k - 1];
        knots.insert(k - 1, [last[0] - d[0] * h, last[1] - d[1] * h, last[2] - d[2] * h]);
    }
    let mut t = vec![0.0; knots.len()];
    for i in 1..knots.len() {
        t[i] = t[i - 1] + span(knots[i - 1], knots[i]);
    }
    let (qs, qe) = (usize::from(smooth_start), knots.len() - 1 - usize::from(smooth_end));
    // Where each fit point sits in the knots.
    let fit_at: Vec<usize> = (0..n).map(|k| if k == 0 { 0 } else { k + usize::from(smooth_start) }).collect();
    let mut coords: Vec<(Vec<f64>, Vec<f64>)> = Vec::with_capacity(3);
    for c in 0..3 {
        let start_cond = start.tangent.map_or(Cond::Second(0.0), |d| Cond::First(d[c]));
        let end_cond = end.tangent.map_or(Cond::Second(0.0), |d| Cond::First(d[c]));
        let mut y: Vec<f64> = knots.iter().map(|p| p[c]).collect();
        // Smooth ends: the extra knots' values are set so that the curvature at the ends matches. The
        // second derivatives are linear in them: find the line from a few trial solves.
        let targets = (start.curvature.map(|k| k[c]), end.curvature.map(|k| k[c]));
        let solve = |y: &[f64]| second_derivatives(&t, y, start_cond, end_cond);
        match (smooth_start, smooth_end, targets) {
            (true, false, (Some(ks), _)) => {
                let base = y[qs];
                let m0 = solve(&y)[0];
                y[qs] = base + 1.0;
                let m1 = solve(&y)[0];
                y[qs] = if (m1 - m0).abs() > 1e-15 { base + (ks - m0) / (m1 - m0) } else { base };
            }
            (false, true, (_, Some(ke))) => {
                let last = y.len() - 1;
                let base = y[qe];
                let m0 = solve(&y)[last];
                y[qe] = base + 1.0;
                let m1 = solve(&y)[last];
                y[qe] = if (m1 - m0).abs() > 1e-15 { base + (ke - m0) / (m1 - m0) } else { base };
            }
            (true, true, (Some(ks), Some(ke))) => {
                let last = y.len() - 1;
                let (bs, be) = (y[qs], y[qe]);
                let m00 = solve(&y);
                y[qs] = bs + 1.0;
                let m10 = solve(&y);
                y[qs] = bs;
                y[qe] = be + 1.0;
                let m01 = solve(&y);
                // [a b; c d] [dqs dqe] = [ks - s0, ke - e0]
                let (a, b) = (m10[0] - m00[0], m01[0] - m00[0]);
                let (cc, d) = (m10[last] - m00[last], m01[last] - m00[last]);
                let det = a * d - b * cc;
                let (rs, re) = (ks - m00[0], ke - m00[last]);
                if det.abs() > 1e-15 {
                    y[qs] = bs + (rs * d - b * re) / det;
                    y[qe] = be + (a * re - cc * rs) / det;
                } else {
                    y[qe] = be;
                }
            }
            _ => {}
        }
        let m = solve(&y);
        coords.push((y, m));
    }
    let mut out = Vec::with_capacity((n - 1) * per_span + 1);
    for k in 0..n - 1 {
        let (from, to) = (t[fit_at[k]], t[fit_at[k + 1]]);
        for step in 0..per_span {
            #[allow(clippy::cast_precision_loss, reason = "a few dozen steps")]
            let s = from + (to - from) * step as f64 / per_span as f64;
            out.push(std::array::from_fn(|c| value_at(&t, &coords[c].0, &coords[c].1, s)));
        }
    }
    out.push(fit[n - 1]);
    out
}

impl Sketch {
    /// True for a constraint that shapes a spline rather than being solved: a Tangent or Smooth with a
    /// spline in it.
    pub fn shapes_spline(&self, c: &GeoConstraint) -> bool {
        let spline = |curve: Id<Curve>| {
            matches!(self.curves.get(&curve).map(|k| &k.geom), Some(CurveGeom::Spline { .. }))
        };
        match *c {
            GeoConstraint::Tangent { a, b, .. } => spline(a) || spline(b),
            GeoConstraint::Smooth(_, _) => true,
            _ => false,
        }
    }

    /// The end of open spline `spline` joined to curve `other` (the same point or coincident points) and
    /// whether it is the spline's last; `None` when no end of it joins an end of the other.
    pub(crate) fn spline_joint(
        &self,
        spline: Id<Curve>,
        other: Id<Curve>,
    ) -> Option<(Id<SkPoint>, Id<SkPoint>, bool)> {
        let CurveGeom::Spline { points } = &self.curves.get(&spline)?.geom else { return None };
        if spline_closed(points) || spline == other {
            return None;
        }
        let ends: Vec<Id<SkPoint>> = match &self.curves.get(&other)?.geom {
            CurveGeom::Line { a, b } => vec![*a, *b],
            CurveGeom::Arc { s, e, .. } => vec![*s, *e],
            CurveGeom::Spline { points } if !spline_closed(points) => {
                [points.first(), points.last()].into_iter().flatten().copied().collect()
            }
            CurveGeom::Spline { .. } | CurveGeom::Circle { .. } | CurveGeom::Ellipse { .. } => Vec::new(),
        };
        let joined = |p: Id<SkPoint>, q: Id<SkPoint>| {
            p == q
                || self.constraints.values().any(|c| {
                    matches!(*c, GeoConstraint::Coincident(x, y) if (x == p && y == q) || (x == q && y == p))
                })
        };
        let (first, last) = (*points.first()?, *points.last()?);
        for (end, is_last) in [(first, false), (last, true)] {
            if let Some(q) = ends.iter().find(|q| joined(end, **q)) {
                return Some((end, *q, is_last));
            }
        }
        None
    }

    /// The direction leaving curve `other` at its end `at` (away from the rest of it), and its curvature
    /// vector there; positions from `pos`.
    fn leaving(
        &self,
        other: Id<Curve>,
        at: Id<SkPoint>,
        pos: &dyn Fn(Id<SkPoint>) -> Option<P>,
    ) -> Option<(P, P)> {
        let here = pos(at)?;
        match &self.curves.get(&other)?.geom {
            CurveGeom::Line { a, b } => {
                let far = if *a == at { *b } else { *a };
                Some((unit(sub(here, pos(far)?))?, [0.0; 3]))
            }
            CurveGeom::Arc { c, s, e } => {
                let (centre, ps, pe) = (pos(*c)?, pos(*s)?, pos(*e)?);
                let radius = sub(here, centre);
                let r = norm(radius);
                let normal = {
                    let (u, w) = (sub(ps, centre), sub(pe, centre));
                    unit([u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]])
                        .unwrap_or([0.0, 0.0, 1.0])
                };
                // Square to the radius in the arc's plane, pointing away from the other end.
                let along = unit([
                    normal[1] * radius[2] - normal[2] * radius[1],
                    normal[2] * radius[0] - normal[0] * radius[2],
                    normal[0] * radius[1] - normal[1] * radius[0],
                ])?;
                let other_end = if *s == at { pe } else { ps };
                let away = if dot(along, sub(other_end, here)) > 0.0 {
                    [-along[0], -along[1], -along[2]]
                } else {
                    along
                };
                let to_centre = [-radius[0] / (r * r), -radius[1] / (r * r), -radius[2] / (r * r)];
                Some((away, to_centre))
            }
            CurveGeom::Spline { points } => {
                let fit: Option<Vec<P>> = points.iter().map(|p| pos(*p)).collect();
                let mut fit = fit?;
                let at_start = points.first() == Some(&at);
                if at_start {
                    fit.reverse();
                }
                // The other spline's own natural shape, ending at the joint.
                let line = crate::model::spline_points3(&fit, 16);
                let k = line.len();
                if k < 3 {
                    return None;
                }
                let (p0, p1, p2) = (line[k - 3], line[k - 2], line[k - 1]);
                let d = unit(sub(p2, p1))?;
                let h = norm(sub(p2, p1)).max(1e-12);
                let second = [
                    (p2[0] - 2.0 * p1[0] + p0[0]) / (h * h),
                    (p2[1] - 2.0 * p1[1] + p0[1]) / (h * h),
                    (p2[2] - 2.0 * p1[2] + p0[2]) / (h * h),
                ];
                Some((d, second))
            }
            CurveGeom::Circle { .. } | CurveGeom::Ellipse { .. } => None,
        }
    }

    /// How the ends of open spline `curve` are shaped by its Tangent and Smooth constraints, with positions
    /// from `pos` (a drag preview's, or the sketch's own).
    pub fn spline_ends_with(
        &self,
        curve: Id<Curve>,
        pos: &dyn Fn(Id<SkPoint>) -> Option<P>,
    ) -> (SplineEnd, SplineEnd) {
        let (mut start, mut end) = (SplineEnd::default(), SplineEnd::default());
        for c in self.constraints.values() {
            let (other, smooth) = match *c {
                GeoConstraint::Tangent { a, b, .. } if a == curve => (b, false),
                GeoConstraint::Tangent { a, b, .. } if b == curve => (a, false),
                GeoConstraint::Smooth(a, b) if a == curve => (b, true),
                GeoConstraint::Smooth(a, b) if b == curve => (a, true),
                _ => continue,
            };
            let Some((_, joint, is_last)) = self.spline_joint(curve, other) else { continue };
            let Some((away, curvature)) = self.leaving(other, joint, pos) else { continue };
            // Leaving the other curve is entering the spline at its start, or arriving at its end along
            // the other way.
            let (tangent, end_ref) =
                if is_last { ([-away[0], -away[1], -away[2]], &mut end) } else { (away, &mut start) };
            end_ref.tangent = Some(tangent);
            if smooth {
                end_ref.curvature = Some(curvature);
            }
        }
        (start, end)
    }

    /// [`Sketch::spline_ends_with`] with the sketch's own positions.
    pub fn spline_ends(&self, curve: Id<Curve>) -> (SplineEnd, SplineEnd) {
        self.spline_ends_with(curve, &|p| self.pos(p))
    }
}
