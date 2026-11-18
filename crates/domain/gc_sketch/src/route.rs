//! Open curves as runs a robot's tool can travel: their two ends, points along them, and chains of
//! curves joined end to end. (GraphCTRL's own: a sketch is drawn to say where robots go.)

use gc_core::Id;

use crate::{Arc3, Curve, CurveGeom, SkPoint, Sketch, spline_closed};

/// Ends of two curves closer than this (m) are joined.
const JOINED: f64 = 1e-7;

fn near(a: [f64; 3], b: [f64; 3]) -> bool {
    (0..3).all(|k| (a[k] - b[k]).abs() <= JOINED)
}

impl Sketch {
    /// The two ends of an open curve, in the direction it is stored: a line's first and second point, an
    /// arc's or elliptical arc's start and end (counter-clockwise in a 2D sketch), an open spline's first
    /// and last fit point. `None` for a circle, a whole ellipse, a closed spline or a missing curve.
    pub fn curve_ends(&self, curve: Id<Curve>) -> Option<(Id<SkPoint>, Id<SkPoint>)> {
        match &self.curves.get(&curve)?.geom {
            CurveGeom::Line { a, b } => Some((*a, *b)),
            CurveGeom::Arc { s, e, .. } => Some((*s, *e)),
            CurveGeom::Ellipse { arc, .. } => *arc,
            CurveGeom::Spline { points } if !spline_closed(points) => {
                Some((*points.first()?, *points.last()?))
            }
            CurveGeom::Spline { .. } | CurveGeom::Circle { .. } => None,
        }
    }

    /// Points along an open curve from its first end to its second ([`Sketch::curve_ends`]), in sketch
    /// coordinates, close enough together to stand for the curve (an arc in steps of at most 3 degrees,
    /// a spline in 16 steps per span). A line gives its two ends. `None` for a closed or degenerate curve.
    pub fn curve_run(&self, curve: Id<Curve>) -> Option<Vec<[f64; 3]>> {
        let geom = &self.curves.get(&curve)?.geom;
        let flat = |points: Vec<[f64; 2]>| points.into_iter().map(|p| [p[0], p[1], 0.0]).collect();
        match geom {
            CurveGeom::Line { a, b } => Some(vec![self.pos(*a)?, self.pos(*b)?]),
            CurveGeom::Arc { c, s, e } => {
                let (c, s, e) = (self.pos(*c)?, self.pos(*s)?, self.pos(*e)?);
                let steps = |sweep: f64| {
                    #[allow(
                        clippy::cast_possible_truncation,
                        clippy::cast_sign_loss,
                        reason = "at most a full turn in 3 degree steps"
                    )]
                    let n = (sweep / 3.0_f64.to_radians()).ceil().clamp(1.0, 720.0) as usize;
                    n
                };
                #[allow(clippy::cast_precision_loss, reason = "a few hundred steps")]
                if self.is_spatial() {
                    let arc = Arc3::new(c, s, e)?;
                    let n = steps(arc.sweep);
                    Some((0..=n).map(|k| arc.at(k as f64 / n as f64)).collect())
                } else {
                    let radius = gc_math::fm::hypot(s[0] - c[0], s[1] - c[1]);
                    if radius < 1e-12 {
                        return None;
                    }
                    let start = gc_math::fm::atan2(s[1] - c[1], s[0] - c[0]);
                    let end = gc_math::fm::atan2(e[1] - c[1], e[0] - c[0]);
                    let mut sweep = (end - start).rem_euclid(std::f64::consts::TAU);
                    if sweep < 1e-12 {
                        sweep = std::f64::consts::TAU;
                    }
                    let n = steps(sweep);
                    let mut run: Vec<[f64; 3]> = (0..=n)
                        .map(|k| {
                            let (sin, cos) = gc_math::fm::sin_cos(start + sweep * k as f64 / n as f64);
                            [c[0] + radius * cos, c[1] + radius * sin, 0.0]
                        })
                        .collect();
                    // The ends are the sketch points themselves, whatever the rounding says.
                    if let Some(first) = run.first_mut() {
                        *first = [s[0], s[1], 0.0];
                    }
                    if let Some(last) = run.last_mut() {
                        *last = [e[0], e[1], 0.0];
                    }
                    Some(run)
                }
            }
            CurveGeom::Ellipse { arc: Some(_), .. } => {
                let (points, closed) = self.ellipse_geom_outline(geom, 96)?;
                (!closed).then(|| flat(points))
            }
            CurveGeom::Spline { points } if !spline_closed(points) => self.spline_outline3(geom, 16),
            CurveGeom::Ellipse { arc: None, .. } | CurveGeom::Spline { .. } | CurveGeom::Circle { .. } => {
                None
            }
        }
    }

    /// Points round a closed curve (a circle, a whole ellipse, a closed spline), in sketch coordinates,
    /// counter-clockwise for a circle or ellipse and as drawn for a spline, ending on the first point
    /// again. It starts at the point of the curve nearest to `from` (for a spline: the nearest of its
    /// points here); without `from`, at the circle's +X point, the ellipse's major axis end, the spline's
    /// first fit point. `None` for an open, missing or degenerate curve.
    pub fn curve_loop(&self, curve: Id<Curve>, from: Option<[f64; 3]>) -> Option<Vec<[f64; 3]>> {
        const STEPS: usize = 120;
        let geom = &self.curves.get(&curve)?.geom;
        // A point at parameter `t` of a full turn, from the start parameter on.
        #[allow(clippy::cast_precision_loss, reason = "a hundred steps")]
        let turn = |start: f64, at: &dyn Fn(f64) -> [f64; 3]| -> Vec<[f64; 3]> {
            let mut points: Vec<[f64; 3]> =
                (0..STEPS).map(|k| at(start + std::f64::consts::TAU * k as f64 / STEPS as f64)).collect();
            points.extend(points.first().copied());
            points
        };
        match geom {
            CurveGeom::Circle { c, r } => {
                let c = self.pos(*c)?;
                if !(r.is_finite() && *r > 1e-12) {
                    return None;
                }
                let start = from.map_or(0.0, |p| gc_math::fm::atan2(p[1] - c[1], p[0] - c[0]));
                Some(turn(start, &|t| {
                    let (sin, cos) = gc_math::fm::sin_cos(t);
                    [c[0] + r * cos, c[1] + r * sin, c[2]]
                }))
            }
            CurveGeom::Ellipse { c, major, minor, arc: None } => {
                let (c, m) = (self.pos(*c)?, self.pos(*major)?);
                let (dx, dy) = (m[0] - c[0], m[1] - c[1]);
                let a = gc_math::fm::hypot(dx, dy);
                if !(a > 1e-12 && minor.is_finite() && minor.abs() > 1e-12) {
                    return None;
                }
                let (ux, uy) = (dx / a, dy / a);
                let start = from
                    .map_or(0.0, |p| crate::ellipse_param([c[0], c[1]], [m[0], m[1]], *minor, [p[0], p[1]]));
                Some(turn(start, &|t| {
                    let (sin, cos) = gc_math::fm::sin_cos(t);
                    [c[0] + a * cos * ux - minor * sin * uy, c[1] + a * cos * uy + minor * sin * ux, c[2]]
                }))
            }
            CurveGeom::Spline { points } if spline_closed(points) => {
                let mut round = self.spline_outline3(geom, 16)?;
                if round.len() < 4 {
                    return None;
                }
                if let Some(p) = from {
                    // The outline ends on its first point: turn the loop so that it starts nearest `from`.
                    round.pop();
                    let far = |q: &[f64; 3]| (0..3).map(|k| (q[k] - p[k]) * (q[k] - p[k])).sum::<f64>();
                    let nearest = round
                        .iter()
                        .enumerate()
                        .min_by(|(_, a), (_, b)| far(a).total_cmp(&far(b)))
                        .map_or(0, |(index, _)| index);
                    round.rotate_left(nearest);
                    round.extend(round.first().copied());
                }
                Some(round)
            }
            CurveGeom::Line { .. }
            | CurveGeom::Arc { .. }
            | CurveGeom::Ellipse { arc: Some(_), .. }
            | CurveGeom::Spline { .. } => None,
        }
    }

    /// True for a circle, a whole ellipse or a closed spline that can be travelled ([`Sketch::curve_loop`]).
    pub fn is_loop(&self, curve: Id<Curve>) -> bool {
        self.curve_loop(curve, None).is_some()
    }

    /// Arranges open curves into one chain joined end to end (ends within a tenth of a micrometre): each
    /// curve with whether it is travelled against its stored direction. The chain starts at the free end
    /// of the curve that comes first in `curves` (for a closed loop: at the first curve's first end, going
    /// along it). `None` when a curve is closed, missing or listed twice, when three curves meet at one
    /// place, or when the curves do not all connect. One closed curve on its own ([`Sketch::is_loop`]) is a
    /// chain too.
    pub fn chain(&self, curves: &[Id<Curve>]) -> Option<Vec<(Id<Curve>, bool)>> {
        if let [only] = curves
            && self.is_loop(*only)
        {
            return Some(vec![(*only, false)]);
        }
        struct Link {
            id: Id<Curve>,
            ends: [[f64; 3]; 2],
        }
        let mut links = Vec::with_capacity(curves.len());
        for (index, id) in curves.iter().enumerate() {
            if curves[..index].contains(id) {
                return None;
            }
            let (a, b) = self.curve_ends(*id)?;
            links.push(Link { id: *id, ends: [self.pos(a)?, self.pos(b)?] });
        }
        // The other curves touching an end of a curve.
        let touching = |of: usize, end: usize| -> Vec<(usize, usize)> {
            let at = links[of].ends[end];
            let mut found = Vec::new();
            for (other, link) in links.iter().enumerate() {
                for (side, place) in link.ends.iter().enumerate() {
                    if (other, side) != (of, end) && other != of && near(*place, at) {
                        found.push((other, side));
                    }
                }
            }
            found
        };
        for of in 0..links.len() {
            if (0..2).any(|end| touching(of, end).len() > 1) {
                return None;
            }
        }
        // Where to start: the first listed curve with a free end, entered at that end.
        let free = (0..links.len()).find_map(|of| {
            let open: Vec<usize> = (0..2).filter(|end| touching(of, *end).is_empty()).collect();
            // A curve free at both ends is travelled as stored.
            open.first().map(|end| (of, if open.len() == 2 { 0 } else { *end }))
        });
        let (mut at, mut entered) = free.unwrap_or((0, 0));
        let mut order = Vec::with_capacity(links.len());
        let mut seen = vec![false; links.len()];
        loop {
            if std::mem::replace(seen.get_mut(at)?, true) {
                break;
            }
            order.push((links[at].id, entered == 1));
            match touching(at, 1 - entered).first() {
                Some((next, side)) => (at, entered) = (*next, *side),
                None => break,
            }
        }
        (order.len() == links.len()).then_some(order)
    }

    /// The chain a curve belongs to: the curve and every open curve joined to it end to end, as far as
    /// the ends are joined to exactly one other curve (a branch or a free end stops it). Curves in `skip`
    /// are left out and stop the chain. In chain order ([`Sketch::chain`]). A closed curve (a circle, a
    /// whole ellipse, a closed spline) is a chain on its own; empty for a missing curve.
    pub fn chain_at(&self, curve: Id<Curve>, skip: &[Id<Curve>]) -> Vec<Id<Curve>> {
        let ends = |id: Id<Curve>| -> Option<[[f64; 3]; 2]> {
            let (a, b) = self.curve_ends(id)?;
            Some([self.pos(a)?, self.pos(b)?])
        };
        if skip.contains(&curve) {
            return Vec::new();
        }
        if ends(curve).is_none() {
            return if self.is_loop(curve) { vec![curve] } else { Vec::new() };
        }
        // The open curves with an end at a place, whatever their role.
        let meeting = |place: [f64; 3]| -> Vec<Id<Curve>> {
            self.curves
                .keys()
                .filter(|id| ends(**id).is_some_and(|e| e.iter().any(|p| near(*p, place))))
                .copied()
                .collect()
        };
        let mut chain = vec![curve];
        for end in 0..2 {
            let mut at = curve;
            let mut place = match ends(at) {
                Some(e) => e[end],
                None => continue,
            };
            loop {
                let here = meeting(place);
                let next = match here.as_slice() {
                    [a, b] => *if *a == at { b } else { a },
                    _ => break,
                };
                if chain.contains(&next) || skip.contains(&next) {
                    break;
                }
                let Some(next_ends) = ends(next) else { break };
                // Both ends at the same place: a loop of one curve cannot be continued.
                place = if near(next_ends[0], place) { next_ends[1] } else { next_ends[0] };
                if end == 0 {
                    chain.insert(0, next);
                } else {
                    chain.push(next);
                }
                at = next;
            }
        }
        match self.chain(&chain) {
            Some(ordered) => {
                // Keep the direction of the curve that was asked for where possible.
                let mut ids: Vec<Id<Curve>> = ordered.iter().map(|(id, _)| *id).collect();
                if ordered.iter().any(|(id, flipped)| *id == curve && *flipped) {
                    ids.reverse();
                }
                ids
            }
            None => vec![curve],
        }
    }
}
