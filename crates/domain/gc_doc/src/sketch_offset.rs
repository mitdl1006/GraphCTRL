//! The sketch Offset command (Inventor 2026 help, To Stretch, Scale, or Offset 2D Sketch Geometry,
//! GUID-962CEE71-8BE4-4DA6-B51E-341319C64FA4): "By default, Inventor selects loops (curves that are joined
//! at the endpoints) and constrains the offset geometry to be equidistant from the original curve"; Loop
//! Select and Constrain Offset can be turned off.
//!
//! Lines are offset parallel, arcs and circles concentric; neighbouring offset curves meet where they
//! cross (or where they touch, for tangent joints). With Constrain Offset, every offset curve has a
//! construction gauge to its original (square to a line, along a radius of an arc or circle), all gauges
//! equal: the offset keeps one degree of freedom, its distance, for a closed loop.

use gc_core::{Id, IdAlloc};
use gc_math::fm;
use gc_sketch::{Curve, CurveGeom, GeoConstraint, SkPoint, Sketch, SketchError};

use crate::command::{Created, EditError};

const TOL: f64 = 1e-9;

type P = [f64; 2];

fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1]]
}

fn add(a: P, b: P) -> P {
    [a[0] + b[0], a[1] + b[1]]
}

fn scale(a: P, k: f64) -> P {
    [a[0] * k, a[1] * k]
}

fn norm(a: P) -> f64 {
    fm::hypot(a[0], a[1])
}

fn unit(a: P) -> Option<P> {
    let n = norm(a);
    (n > TOL).then(|| scale(a, 1.0 / n))
}

fn left(a: P) -> P {
    [-a[1], a[0]]
}

fn cross(a: P, b: P) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

/// A chain of steps and whether it closes.
type Chain = (Vec<Step>, bool);

/// A curve as travelled in a chain: from `start` to `end` (sketch points), with its shape.
#[derive(Clone, Copy, Debug)]
struct Step {
    curve: Id<Curve>,
    start: Id<SkPoint>,
    end: Id<SkPoint>,
    /// For an arc: its centre and whether it is travelled clockwise (against its own direction).
    arc: Option<(Id<SkPoint>, bool)>,
}

fn xy(sketch: &Sketch, p: Id<SkPoint>) -> Result<P, EditError> {
    let q = sketch.pos(p).ok_or(SketchError::MissingPoint)?;
    Ok([q[0], q[1]])
}

/// The ends of a line or arc, in its own direction.
fn ends(curve: &Curve) -> Option<(Id<SkPoint>, Id<SkPoint>)> {
    match curve.geom {
        CurveGeom::Line { a, b } => Some((a, b)),
        CurveGeom::Arc { s, e, .. } => Some((s, e)),
        CurveGeom::Circle { .. } => None,
    }
}

/// The selection grown to whole loops: curves whose ends meet (same place) the selected ones.
fn grow(sketch: &Sketch, chosen: &[Id<Curve>]) -> Result<Vec<Id<Curve>>, EditError> {
    let mut out: Vec<Id<Curve>> = chosen.to_vec();
    let ends_at = |c: Id<Curve>| -> Vec<P> {
        sketch
            .curves
            .get(&c)
            .and_then(ends)
            .map(|(a, b)| [a, b].iter().filter_map(|p| xy(sketch, *p).ok()).collect())
            .unwrap_or_default()
    };
    loop {
        let reached: Vec<P> = out.iter().flat_map(|c| ends_at(*c)).collect();
        let more: Vec<Id<Curve>> = sketch
            .curves
            .iter()
            .filter(|(id, c)| !out.contains(id) && !c.construction)
            .filter(|(id, _)| ends_at(**id).iter().any(|p| reached.iter().any(|q| norm(sub(*p, *q)) < TOL)))
            .map(|(id, _)| *id)
            .collect();
        if more.is_empty() {
            return Ok(out);
        }
        out.extend(more);
    }
}

/// Chains of the lines and arcs (ordered so that each starts where the last ended), each with whether it
/// closes; circles apart.
fn chains(sketch: &Sketch, curves: &[Id<Curve>]) -> Result<(Vec<Chain>, Vec<Id<Curve>>), EditError> {
    let mut circles = Vec::new();
    let mut open: Vec<Id<Curve>> = Vec::new();
    for id in curves {
        let curve = sketch.curves.get(id).ok_or(SketchError::MissingCurve)?;
        match curve.geom {
            CurveGeom::Circle { .. } => circles.push(*id),
            _ => open.push(*id),
        }
    }
    let step_of = |id: Id<Curve>, reversed: bool| -> Result<Step, EditError> {
        let curve = sketch.curves.get(&id).ok_or(SketchError::MissingCurve)?;
        let (a, b) = ends(curve).ok_or(SketchError::InvalidValue)?;
        let arc = match curve.geom {
            CurveGeom::Arc { c, .. } => Some((c, reversed)),
            _ => None,
        };
        Ok(if reversed {
            Step { curve: id, start: b, end: a, arc }
        } else {
            Step { curve: id, start: a, end: b, arc }
        })
    };
    let same = |p: Id<SkPoint>, q: Id<SkPoint>| -> bool {
        p == q || matches!((xy(sketch, p), xy(sketch, q)), (Ok(a), Ok(b)) if norm(sub(a, b)) < TOL)
    };
    let mut out = Vec::new();
    while let Some(first) = open.first().copied() {
        open.remove(0);
        let mut chain = vec![step_of(first, false)?];
        // Forward from the end, then backward from the start.
        loop {
            let tail = chain.last().map(|s| s.end).ok_or(SketchError::InvalidValue)?;
            let next = open.iter().enumerate().find_map(|(i, id)| {
                let (a, b) = sketch.curves.get(id).and_then(ends)?;
                if same(a, tail) {
                    Some((i, false))
                } else if same(b, tail) {
                    Some((i, true))
                } else {
                    None
                }
            });
            let Some((i, reversed)) = next else { break };
            let id = open.remove(i);
            chain.push(step_of(id, reversed)?);
        }
        loop {
            let head = chain.first().map(|s| s.start).ok_or(SketchError::InvalidValue)?;
            let next = open.iter().enumerate().find_map(|(i, id)| {
                let (a, b) = sketch.curves.get(id).and_then(ends)?;
                if same(b, head) {
                    Some((i, false))
                } else if same(a, head) {
                    Some((i, true))
                } else {
                    None
                }
            });
            let Some((i, reversed)) = next else { break };
            let id = open.remove(i);
            chain.insert(0, step_of(id, reversed)?);
        }
        let closed = chain.len() > 1
            && matches!((chain.first(), chain.last()), (Some(f), Some(l)) if same(f.start, l.end));
        out.push((chain, closed));
    }
    Ok((out, circles))
}

/// The direction of travel at a point of a step (`at_end`: at its end, else at its start).
fn tangent(sketch: &Sketch, step: &Step, at_end: bool) -> Result<P, EditError> {
    let (s, e) = (xy(sketch, step.start)?, xy(sketch, step.end)?);
    match step.arc {
        None => unit(sub(e, s)).ok_or_else(|| SketchError::InvalidValue.into()),
        Some((c, clockwise)) => {
            let p = if at_end { e } else { s };
            let radial = sub(p, xy(sketch, c)?);
            let t = unit(left(radial)).ok_or(SketchError::InvalidValue)?;
            Ok(if clockwise { scale(t, -1.0) } else { t })
        }
    }
}

/// The offset curve of a step: a line (point, unit direction) or a circle (centre, radius).
#[derive(Clone, Copy, Debug)]
enum Shape {
    Line(P, P),
    Circle(P, f64),
}

fn offset_shape(sketch: &Sketch, step: &Step, d: f64) -> Result<Shape, EditError> {
    let s = xy(sketch, step.start)?;
    match step.arc {
        None => {
            let t = tangent(sketch, step, false)?;
            Ok(Shape::Line(add(s, scale(left(t), d)), t))
        }
        Some((c, clockwise)) => {
            let centre = xy(sketch, c)?;
            let r = norm(sub(s, centre));
            // Left of a counter-clockwise arc is towards its centre.
            let radius = if clockwise { r + d } else { r - d };
            if radius <= TOL {
                return Err(SketchError::InvalidValue.into());
            }
            Ok(Shape::Circle(centre, radius))
        }
    }
}

/// Where two offset curves meet, nearest to `near`.
fn meet(a: Shape, b: Shape, near: P) -> Option<P> {
    let nearest = |points: Vec<P>| {
        points.into_iter().min_by(|p, q| norm(sub(*p, near)).total_cmp(&norm(sub(*q, near))))
    };
    match (a, b) {
        (Shape::Line(p, u), Shape::Line(q, v)) => {
            let denominator = cross(u, v);
            if denominator.abs() < 1e-12 {
                return None;
            }
            let t = cross(sub(q, p), v) / denominator;
            Some(add(p, scale(u, t)))
        }
        (Shape::Line(p, u), Shape::Circle(c, r)) | (Shape::Circle(c, r), Shape::Line(p, u)) => {
            let f = sub(p, c);
            let b = f[0] * u[0] + f[1] * u[1];
            let disc = b * b - (norm(f) * norm(f) - r * r);
            if disc < 0.0 {
                return None;
            }
            let root = disc.sqrt();
            nearest(vec![add(p, scale(u, -b + root)), add(p, scale(u, -b - root))])
        }
        (Shape::Circle(c1, r1), Shape::Circle(c2, r2)) => {
            let d = norm(sub(c2, c1));
            if d < TOL {
                return None;
            }
            let a = (r1 * r1 - r2 * r2 + d * d) / (2.0 * d);
            let h2 = r1 * r1 - a * a;
            if h2 < 0.0 {
                return None;
            }
            let u = scale(sub(c2, c1), 1.0 / d);
            let m = add(c1, scale(u, a));
            let h = h2.sqrt();
            nearest(vec![add(m, scale(left(u), h)), add(m, scale(left(u), -h))])
        }
    }
}

fn require(
    sketch: &mut Sketch,
    ids: &mut IdAlloc,
    c: GeoConstraint,
    created: &mut Vec<Created>,
) -> Result<(), EditError> {
    crate::command::require_constraint(sketch, ids, c, created)
}

fn construction(
    sketch: &mut Sketch,
    ids: &mut IdAlloc,
    a: Id<SkPoint>,
    b: Id<SkPoint>,
    created: &mut Vec<Created>,
) -> Id<Curve> {
    let id: Id<Curve> = ids.alloc();
    sketch.curves.insert(id, Curve { geom: CurveGeom::Line { a, b }, construction: true });
    created.push(Created::Curve(id));
    id
}

fn point(sketch: &mut Sketch, ids: &mut IdAlloc, at: P, created: &mut Vec<Created>) -> Id<SkPoint> {
    let p = sketch.add_point(ids, [at[0], at[1], 0.0]);
    created.push(Created::Point(p));
    p
}

/// Offsets curves by `distance` (positive: outside a closed loop, or left of an open chain's direction;
/// circles grow). Returns the label.
pub(crate) fn offset(
    sketch: &mut Sketch,
    ids: &mut IdAlloc,
    curves: &[Id<Curve>],
    distance: f64,
    loop_select: bool,
    constrain: bool,
    created: &mut Vec<Created>,
) -> Result<&'static str, EditError> {
    if curves.is_empty() || !distance.is_finite() || distance.abs() < TOL || sketch.is_spatial() {
        return Err(SketchError::InvalidValue.into());
    }
    let chosen = if loop_select { grow(sketch, curves)? } else { curves.to_vec() };
    let (chains, circles) = chains(sketch, &chosen)?;
    // Gauges: construction lines whose lengths are the offset distance, to be made equal.
    let mut gauges: Vec<Id<Curve>> = Vec::new();
    for (chain, closed) in chains {
        // A closed loop is travelled clockwise so that its left is outside.
        let d = if closed {
            let mut area = 0.0;
            for step in &chain {
                let (s, e) = (xy(sketch, step.start)?, xy(sketch, step.end)?);
                area += cross(s, e);
            }
            if area > 0.0 { -distance } else { distance }
        } else {
            distance
        };
        let shapes: Vec<Shape> =
            chain.iter().map(|s| offset_shape(sketch, s, d)).collect::<Result<_, _>>()?;
        let n = chain.len();
        // The joints: start of each step (and the open chain's end).
        let joint_count = if closed { n } else { n + 1 };
        let mut joints: Vec<Id<SkPoint>> = Vec::with_capacity(joint_count);
        // Steps meeting tangentially (before, after): their offsets are made tangent too.
        let mut smooth: Vec<(usize, usize)> = Vec::new();
        for k in 0..joint_count {
            let (before, after) = match (closed, k) {
                (true, k) => (Some((k + n - 1) % n), Some(k)),
                (false, 0) => (None, Some(0)),
                (false, k) if k == n => (Some(n - 1), None),
                (false, k) => (Some(k - 1), Some(k)),
            };
            let shared = match after {
                Some(a) => xy(sketch, chain[a].start)?,
                None => xy(sketch, chain[n - 1].end)?,
            };
            let normal_of = |i: usize, at_end: bool| -> Result<P, EditError> {
                Ok(left(tangent(sketch, &chain[i], at_end)?))
            };
            let naive = match (before, after) {
                (Some(b), Some(a)) => {
                    let avg = add(normal_of(b, true)?, normal_of(a, false)?);
                    add(shared, scale(unit(avg).unwrap_or(normal_of(a, false)?), d))
                }
                (Some(b), None) => add(shared, scale(normal_of(b, true)?, d)),
                (None, Some(a)) => add(shared, scale(normal_of(a, false)?, d)),
                (None, None) => shared,
            };
            let at = match (before, after) {
                (Some(b), Some(a)) => {
                    let tangent_joint =
                        cross(tangent(sketch, &chain[b], true)?, tangent(sketch, &chain[a], false)?).abs()
                            < 1e-9;
                    if tangent_joint {
                        smooth.push((b, a));
                        add(shared, scale(normal_of(a, false)?, d))
                    } else {
                        meet(shapes[b], shapes[a], naive).ok_or(SketchError::InvalidValue)?
                    }
                }
                _ => naive,
            };
            joints.push(point(sketch, ids, at, created));
        }
        let mut made_curves: Vec<Id<Curve>> = Vec::with_capacity(n);
        for (k, step) in chain.iter().enumerate() {
            let (from, to) = (joints[k], joints[(k + 1) % joints.len()]);
            let original = step.curve;
            let made: Id<Curve> = ids.alloc();
            made_curves.push(made);
            match step.arc {
                None => {
                    sketch.curves.insert(
                        made,
                        Curve { geom: CurveGeom::Line { a: from, b: to }, construction: false },
                    );
                    created.push(Created::Curve(made));
                    require(sketch, ids, GeoConstraint::Parallel(made, original), created)?;
                    if constrain {
                        // From the foot on the original line, square to it, to the offset start.
                        let a = xy(sketch, from)?;
                        let (s, t) = (xy(sketch, step.start)?, tangent(sketch, step, false)?);
                        let along = (a[0] - s[0]) * t[0] + (a[1] - s[1]) * t[1];
                        let foot = point(sketch, ids, add(s, scale(t, along)), created);
                        let gauge = construction(sketch, ids, foot, from, created);
                        require(sketch, ids, GeoConstraint::PointOnCurve(foot, original), created)?;
                        require(sketch, ids, GeoConstraint::Perpendicular(gauge, original), created)?;
                        gauges.push(gauge);
                    }
                }
                Some((c, clockwise)) => {
                    let centre = point(sketch, ids, xy(sketch, c)?, created);
                    let (s, e) = if clockwise { (to, from) } else { (from, to) };
                    sketch.curves.insert(
                        made,
                        Curve { geom: CurveGeom::Arc { c: centre, s, e }, construction: false },
                    );
                    created.push(Created::Curve(made));
                    require(sketch, ids, GeoConstraint::Coincident(centre, c), created)?;
                    if constrain {
                        // Along a radius: from the original's start out to a point on the offset arc.
                        let o = xy(sketch, c)?;
                        let orig_start = xy(sketch, step.start)?;
                        let radial = unit(sub(orig_start, o)).ok_or(SketchError::InvalidValue)?;
                        let Shape::Circle(_, r) = shapes[k] else {
                            return Err(SketchError::InvalidValue.into());
                        };
                        let on = point(sketch, ids, add(o, scale(radial, r)), created);
                        let gauge = construction(sketch, ids, step.start, on, created);
                        require(sketch, ids, GeoConstraint::PointOnCurve(on, made), created)?;
                        require(sketch, ids, GeoConstraint::PointOnCurve(c, gauge), created)?;
                        gauges.push(gauge);
                    }
                }
            }
        }
        // A line and an arc that met tangentially still do (the joint alone would hold only to second
        // order).
        for (b, a) in smooth {
            if chain[b].arc.is_some() || chain[a].arc.is_some() {
                let tangent = crate::command::tangency_of(sketch, made_curves[b], made_curves[a])
                    .ok_or(SketchError::InvalidValue)?;
                require(sketch, ids, tangent, created)?;
            }
        }
    }
    for circle in circles {
        let Some((c, r)) = sketch.round(circle) else { continue };
        let radius = r + distance;
        if radius <= TOL {
            return Err(SketchError::InvalidValue.into());
        }
        let o = xy(sketch, c)?;
        let centre = point(sketch, ids, o, created);
        let made: Id<Curve> = ids.alloc();
        sketch
            .curves
            .insert(made, Curve { geom: CurveGeom::Circle { c: centre, r: radius }, construction: false });
        created.push(Created::Curve(made));
        require(sketch, ids, GeoConstraint::Coincident(centre, c), created)?;
        if constrain {
            // A radius level with the centre, from the original circle out to the offset one.
            let inner = point(sketch, ids, add(o, [r, 0.0]), created);
            let outer = point(sketch, ids, add(o, [radius, 0.0]), created);
            let gauge = construction(sketch, ids, inner, outer, created);
            require(sketch, ids, GeoConstraint::PointOnCurve(inner, circle), created)?;
            require(sketch, ids, GeoConstraint::PointOnCurve(outer, made), created)?;
            require(sketch, ids, GeoConstraint::Horizontal(c, inner), created)?;
            require(sketch, ids, GeoConstraint::Horizontal(c, outer), created)?;
            gauges.push(gauge);
        }
    }
    if constrain {
        // Left out where the shape already forces one distance (a smooth loop): there they would be
        // redundant.
        for gauge in gauges.iter().skip(1) {
            crate::command::try_constraint(sketch, ids, GeoConstraint::Equal(gauges[0], *gauge), created);
        }
    }
    Ok("Offset")
}
