//! Arcs in a 3D sketch (Inventor 2026 help, To Create Lines, Arcs, and Bends,
//! GUID-4F861674-F74C-474E-869A-4C190ED6EC4B: Three Point Arc and Center Point Arc on the 3D Sketch tab's
//! Draw panel).
//!
//! A 3D arc is stored like a 2D one, centre, start and end, and lies in the plane of those three points,
//! the short way round from start to end. So a 3D arc turns less than half a circle: three points
//! cannot tell more apart.

/// An arc in space, ready to sample.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arc3 {
    /// Centre.
    pub centre: [f64; 3],
    /// Radius.
    pub radius: f64,
    /// Unit vector from the centre to the start.
    pub u: [f64; 3],
    /// Unit vector in the arc's plane, a quarter turn on from `u` towards the end.
    pub v: [f64; 3],
    /// Angle from start to end, in `(0, pi)`.
    pub sweep: f64,
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

impl Arc3 {
    /// The arc round `centre` from `start` towards `end` (whose distance does not matter). `None` when the
    /// three points are in line (no plane) or a point sits on the centre.
    pub fn new(centre: [f64; 3], start: [f64; 3], end: [f64; 3]) -> Option<Arc3> {
        let (s, e) = (sub(start, centre), sub(end, centre));
        let (radius, reach) = (norm(s), norm(e));
        if radius < 1e-12 || reach < 1e-12 {
            return None;
        }
        let u = s.map(|x| x / radius);
        // The part of the end direction square to `u`.
        let along = dot(e, u);
        let across = sub(e, u.map(|x| x * along));
        let side = norm(across);
        if side < 1e-9 * reach {
            return None;
        }
        let v = across.map(|x| x / side);
        let sweep = gc_math::fm::atan2(side, along);
        Some(Arc3 { centre, radius, u, v, sweep })
    }

    /// The arc through `a`, then `through`, to `b`: its centre, start and end. `None` when the points are
    /// in line or the arc would turn half a circle or more.
    pub fn through(a: [f64; 3], through: [f64; 3], b: [f64; 3]) -> Option<([f64; 3], [f64; 3], [f64; 3])> {
        // The circumcentre of the triangle a, b, through.
        let (ab, at) = (sub(b, a), sub(through, a));
        let n = [ab[1] * at[2] - ab[2] * at[1], ab[2] * at[0] - ab[0] * at[2], ab[0] * at[1] - ab[1] * at[0]];
        let n2 = dot(n, n);
        if n2 < 1e-24 * dot(ab, ab).max(1e-300) * dot(at, at).max(1e-300) {
            return None;
        }
        let cross = |x: [f64; 3], y: [f64; 3]| {
            [x[1] * y[2] - x[2] * y[1], x[2] * y[0] - x[0] * y[2], x[0] * y[1] - x[1] * y[0]]
        };
        let term = |k: f64, v: [f64; 3]| v.map(|x| x * k);
        let first = term(dot(ab, ab), cross(at, n));
        let second = term(dot(at, at), cross(n, ab));
        let offset = [0, 1, 2].map(|i| (first[i] + second[i]) / (2.0 * n2));
        let centre = [a[0] + offset[0], a[1] + offset[1], a[2] + offset[2]];
        let arc = Arc3::new(centre, a, b)?;
        // `through` must lie on the short way round.
        let t = sub(through, centre);
        let angle = gc_math::fm::atan2(dot(t, arc.v), dot(t, arc.u));
        (angle > 0.0 && angle < arc.sweep).then_some((centre, a, b))
    }

    /// The point at `t` of the way from start to end.
    pub fn at(&self, t: f64) -> [f64; 3] {
        let angle = self.sweep * t;
        let (c, s) = (gc_math::fm::cos(angle), gc_math::fm::sin(angle));
        [0, 1, 2].map(|k| self.centre[k] + self.radius * (c * self.u[k] + s * self.v[k]))
    }
}
