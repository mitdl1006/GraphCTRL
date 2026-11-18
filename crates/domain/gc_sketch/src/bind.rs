//! Translation between a sketch and the solver's parameters and constraints.

use std::collections::BTreeMap;

use gc_core::Id;
use gc_solver::{Constraint, ConstraintId, Line2, Line3, P2, P3, ParamId, Round, System};

use crate::edit::SketchError;
use crate::model::{Curve, CurveGeom, DimKind, Dimension, GeoConstraint, PointKind, SkPoint, Sketch};

/// A solver system built from a sketch, with the maps to get back.
pub(crate) struct Binding {
    pub sys: System,
    pub points: BTreeMap<Id<SkPoint>, Vec<ParamId>>,
    pub radii: BTreeMap<Id<Curve>, ParamId>,
    /// For each line with a Fix constraint: the line where it lies now, as two held points it keeps to.
    fixed_lines: BTreeMap<Id<Curve>, Line2>,
}

/// The solver id of a sketch item: its document-wide raw id. Blame from the solver maps straight back.
pub(crate) fn solver_id<T>(id: Id<T>) -> ConstraintId {
    ConstraintId(id.raw().get())
}

impl Binding {
    fn params(&self, point: Id<SkPoint>) -> Result<&[ParamId], SketchError> {
        self.points.get(&point).map(Vec::as_slice).ok_or(SketchError::MissingPoint)
    }

    fn p2(&self, point: Id<SkPoint>) -> Result<P2, SketchError> {
        match self.params(point)? {
            [x, y] => Ok([*x, *y]),
            _ => Err(SketchError::WrongSpace),
        }
    }

    fn p3(&self, point: Id<SkPoint>) -> Result<P3, SketchError> {
        match self.params(point)? {
            [x, y, z] => Ok([*x, *y, *z]),
            _ => Err(SketchError::WrongSpace),
        }
    }

    fn line(&self, sketch: &Sketch, curve: Id<Curve>) -> Result<Line2, SketchError> {
        let (a, b) = sketch.line(curve).ok_or(SketchError::WrongCurveKind)?;
        Ok(Line2 { a: self.p2(a)?, b: self.p2(b)? })
    }

    /// A line, or an ellipse's major axis (from its centre to the major axis end): what Parallel,
    /// Perpendicular and Collinear take ("lines or ellipse axes", GUID-A85A7A30-7D81-4C75-8769-CAD034EEA930).
    fn axis_line(&self, sketch: &Sketch, curve: Id<Curve>) -> Result<Line2, SketchError> {
        if let Some(CurveGeom::Ellipse { c, major, .. }) = sketch.curves.get(&curve).map(|k| &k.geom) {
            return Ok(Line2 { a: self.p2(*c)?, b: self.p2(*major)? });
        }
        self.line(sketch, curve)
    }

    fn line3(&self, sketch: &Sketch, curve: Id<Curve>) -> Result<Line3, SketchError> {
        let (a, b) = sketch.line(curve).ok_or(SketchError::WrongCurveKind)?;
        Ok(Line3 { a: self.p3(a)?, b: self.p3(b)? })
    }

    /// A 3D arc's radius as a [`Round`] for the constraints that read only the radius (equal radius, a
    /// radius dimension); the centre given is two of its three coordinates and must not be used.
    fn radius_only(&self, sketch: &Sketch, curve: Id<Curve>) -> Result<Round, SketchError> {
        let (centre, _) = sketch.round(curve).ok_or(SketchError::WrongCurveKind)?;
        let r = *self.radii.get(&curve).ok_or(SketchError::WrongCurveKind)?;
        let c = self.p3(centre)?;
        Ok(Round { c: [c[0], c[1]], r })
    }

    /// An ellipse's centre, major axis end and minor radius parameter.
    fn ellipse(
        &self,
        sketch: &Sketch,
        curve: Id<Curve>,
    ) -> Result<(P2, P2, gc_solver::ParamId), SketchError> {
        let (c, major, _) = sketch.ellipse(curve).ok_or(SketchError::WrongCurveKind)?;
        let minor = *self.radii.get(&curve).ok_or(SketchError::WrongCurveKind)?;
        Ok((self.p2(c)?, self.p2(major)?, minor))
    }

    fn round(&self, sketch: &Sketch, curve: Id<Curve>) -> Result<Round, SketchError> {
        let (centre, _) = sketch.round(curve).ok_or(SketchError::WrongCurveKind)?;
        let r = *self.radii.get(&curve).ok_or(SketchError::WrongCurveKind)?;
        Ok(Round { c: self.p2(centre)?, r })
    }

    /// Tangency of two curves that meet at an end (a shared point or two coincident end points): the
    /// joint form, or `None` when they do not meet at an end.
    /// A line tangent to an ellipse or elliptical arc (To Apply Geometric Constraints,
    /// GUID-A85A7A30-7D81-4C75-8769-CAD034EEA930: Tangent takes two curves): where a line end lies on the
    /// ellipse (an arc end, or a point held on it), the line runs along the ellipse there; otherwise it
    /// touches the ellipse on the `side` it is on. `None` when the pair is not a line and an ellipse.
    fn ellipse_tangent(
        &self,
        sketch: &Sketch,
        a: Id<Curve>,
        b: Id<Curve>,
        side: f64,
    ) -> Result<Option<Constraint>, SketchError> {
        let (line_id, e) = if sketch.line(a).is_some() && sketch.ellipse(b).is_some() {
            (a, b)
        } else if sketch.line(b).is_some() && sketch.ellipse(a).is_some() {
            (b, a)
        } else {
            return Ok(None);
        };
        let (c, major, _) = sketch.ellipse(e).ok_or(SketchError::WrongCurveKind)?;
        let minor = *self.radii.get(&e).ok_or(SketchError::WrongCurveKind)?;
        let line = self.line(sketch, line_id)?;
        let (la, lb) = sketch.line(line_id).ok_or(SketchError::WrongCurveKind)?;
        let arc_ends: Vec<Id<SkPoint>> = match sketch.curves.get(&e).map(|k| &k.geom) {
            Some(CurveGeom::Ellipse { arc: Some((s, t)), .. }) => vec![*s, *t],
            Some(_) | None => Vec::new(),
        };
        let on_ellipse = |p: Id<SkPoint>| {
            arc_ends.contains(&p)
                || sketch.constraints.values().any(|k| match *k {
                    GeoConstraint::PointOnCurve(q, curve) => q == p && curve == e,
                    GeoConstraint::Coincident(x, y) => {
                        (x == p && arc_ends.contains(&y)) || (y == p && arc_ends.contains(&x))
                    }
                    _ => false,
                })
        };
        let (c, major) = (self.p2(c)?, self.p2(major)?);
        Ok(Some(match [la, lb].into_iter().find(|p| on_ellipse(*p)) {
            Some(at) => Constraint::EndTangentEllipse { line, at: self.p2(at)?, c, major, minor },
            None => Constraint::TangentLineEllipse { line, c, major, minor, side },
        }))
    }

    fn end_tangent(
        &self,
        sketch: &Sketch,
        a: Id<Curve>,
        b: Id<Curve>,
    ) -> Result<Option<Constraint>, SketchError> {
        let ends = |curve: Id<Curve>| -> Vec<Id<SkPoint>> {
            match sketch.curves.get(&curve).map(|c| c.geom.clone()) {
                Some(CurveGeom::Line { a, b }) => vec![a, b],
                Some(CurveGeom::Arc { s, e, .. }) => vec![s, e],
                _ => Vec::new(),
            }
        };
        let joined = |p: Id<SkPoint>, q: Id<SkPoint>| {
            p == q
                || sketch.constraints.values().any(|c| {
                    matches!(*c, GeoConstraint::Coincident(x, y) if (x == p && y == q) || (x == q && y == p))
                })
        };
        let Some((pa, pb)) = ends(a)
            .into_iter()
            .flat_map(|p| ends(b).into_iter().map(move |q| (p, q)))
            .find(|(p, q)| joined(*p, *q))
        else {
            return Ok(None);
        };
        let arc_end =
            |curve: Id<Curve>, at: Id<SkPoint>| match sketch.curves.get(&curve).map(|c| c.geom.clone()) {
                Some(CurveGeom::Arc { c, s, .. }) => Some((c, if at == s { 1.0 } else { -1.0 })),
                _ => None,
            };
        Ok(match (self.line(sketch, a), self.line(sketch, b)) {
            (Ok(line), Err(_)) => {
                let Some((_, leaving)) = arc_end(b, pb) else { return Ok(None) };
                Some(Constraint::EndTangent {
                    line,
                    round: self.round(sketch, b)?,
                    at: self.p2(pb)?,
                    smooth: true,
                    leaving,
                })
            }
            (Err(_), Ok(line)) => {
                let Some((_, leaving)) = arc_end(a, pa) else { return Ok(None) };
                Some(Constraint::EndTangent {
                    line,
                    round: self.round(sketch, a)?,
                    at: self.p2(pa)?,
                    smooth: true,
                    leaving,
                })
            }
            (Err(_), Err(_)) => match (arc_end(a, pa), arc_end(b, pb)) {
                (Some((ca, _)), Some((cb, _))) => Some(Constraint::Parallel(
                    Line2 { a: self.p2(ca)?, b: self.p2(pa)? },
                    Line2 { a: self.p2(cb)?, b: self.p2(pb)? },
                )),
                _ => None,
            },
            (Ok(_), Ok(_)) => None,
        })
    }

    /// What a constraint needs in the system before it is lowered: a fixed line's held copy (two points
    /// held where the line's ends are now).
    pub fn prepare(&mut self, sketch: &Sketch, c: &GeoConstraint) -> Result<(), SketchError> {
        if let GeoConstraint::FixCurve(curve) = *c
            && !sketch.is_spatial()
            && !self.fixed_lines.contains_key(&curve)
            && let Some((a, b)) = sketch.line(curve)
        {
            let (pa, pb) = (
                sketch.pos(a).ok_or(SketchError::MissingPoint)?,
                sketch.pos(b).ok_or(SketchError::MissingPoint)?,
            );
            let held = Line2 { a: self.sys.add_point2(pa[0], pa[1]), b: self.sys.add_point2(pb[0], pb[1]) };
            let params = vec![held.a[0], held.a[1], held.b[0], held.b[1]];
            self.sys.add(solver_id(curve), Constraint::Fix(params));
            self.fixed_lines.insert(curve, held);
        }
        Ok(())
    }

    /// A Fix on a curve: a 2D line keeps to its held copy (its ends slide along it); anything else holds
    /// its points and radius.
    fn fix_curve(&self, sketch: &Sketch, curve: Id<Curve>) -> Result<Constraint, SketchError> {
        if let Some(held) = self.fixed_lines.get(&curve) {
            return Ok(Constraint::Collinear(self.line(sketch, curve)?, *held));
        }
        let geom = &sketch.curves.get(&curve).ok_or(SketchError::MissingCurve)?.geom;
        let points: Vec<Id<SkPoint>> = match *geom {
            // Arc ends slide along the circle; the centre and radius hold.
            CurveGeom::Arc { c, .. } if !sketch.is_spatial() => vec![c],
            CurveGeom::Ellipse { c, major, .. } => vec![c, major],
            CurveGeom::Line { .. }
            | CurveGeom::Circle { .. }
            | CurveGeom::Arc { .. }
            | CurveGeom::Spline { .. } => sketch.curves[&curve].points(),
        };
        let mut params = Vec::new();
        for p in points {
            params.extend_from_slice(self.params(p)?);
        }
        if let Some(r) = self.radii.get(&curve) {
            params.push(*r);
        }
        Ok(Constraint::Fix(params))
    }

    pub fn lower_constraint(&self, sketch: &Sketch, c: &GeoConstraint) -> Result<Constraint, SketchError> {
        let spatial = sketch.is_spatial();
        Ok(match *c {
            GeoConstraint::FixCurve(curve) => self.fix_curve(sketch, curve)?,
            GeoConstraint::Coincident(p, q) if spatial => Constraint::Coincident3(self.p3(p)?, self.p3(q)?),
            GeoConstraint::Coincident(p, q) => Constraint::Coincident(self.p2(p)?, self.p2(q)?),
            // A 3D sketch has lines only; their relations are written in space (Inventor 2026 help, To Apply
            // Geometric Constraints, GUID-A85A7A30-7D81-4C75-8769-CAD034EEA930).
            GeoConstraint::PointOnCurve(p, curve) if spatial => {
                Constraint::PointOnLine3(self.p3(p)?, self.line3(sketch, curve)?)
            }
            GeoConstraint::Midpoint(p, line) if spatial => {
                Constraint::Midpoint3(self.p3(p)?, self.line3(sketch, line)?)
            }
            GeoConstraint::Parallel(a, b) if spatial => {
                Constraint::Parallel3(self.line3(sketch, a)?, self.line3(sketch, b)?)
            }
            GeoConstraint::Perpendicular(a, b) if spatial => {
                Constraint::Perpendicular3(self.line3(sketch, a)?, self.line3(sketch, b)?)
            }
            GeoConstraint::Collinear(a, b) if spatial => {
                Constraint::Collinear3(self.line3(sketch, a)?, self.line3(sketch, b)?)
            }
            GeoConstraint::Equal(a, b) if spatial => match (self.line3(sketch, a), self.line3(sketch, b)) {
                (Ok(l1), Ok(l2)) => Constraint::EqualLength3(l1, l2),
                (Err(_), Err(_)) => {
                    Constraint::EqualRadius(self.radius_only(sketch, a)?, self.radius_only(sketch, b)?)
                }
                _ => return Err(SketchError::WrongCurveKind),
            },
            GeoConstraint::Horizontal(..)
            | GeoConstraint::Vertical(..)
            | GeoConstraint::Tangent { .. }
            | GeoConstraint::Concentric(..)
            | GeoConstraint::Symmetric { .. }
                if spatial =>
            {
                return Err(SketchError::WrongSpace);
            }
            GeoConstraint::PointOnCurve(p, curve) if sketch.ellipse(curve).is_some() => {
                let (c, major, minor) = self.ellipse(sketch, curve)?;
                Constraint::PointOnEllipse { p: self.p2(p)?, c, major, minor }
            }
            GeoConstraint::PointOnCurve(p, curve) => match self.line(sketch, curve) {
                Ok(line) => Constraint::PointOnLine(self.p2(p)?, line),
                Err(_) => Constraint::PointOnCircle(self.p2(p)?, self.round(sketch, curve)?),
            },
            GeoConstraint::Midpoint(p, line) => Constraint::Midpoint(self.p2(p)?, self.line(sketch, line)?),
            GeoConstraint::Horizontal(p, q) => Constraint::Horizontal(self.p2(p)?, self.p2(q)?),
            GeoConstraint::Vertical(p, q) => Constraint::Vertical(self.p2(p)?, self.p2(q)?),
            GeoConstraint::Parallel(a, b) => {
                Constraint::Parallel(self.axis_line(sketch, a)?, self.axis_line(sketch, b)?)
            }
            GeoConstraint::Perpendicular(a, b) => {
                Constraint::Perpendicular(self.axis_line(sketch, a)?, self.axis_line(sketch, b)?)
            }
            GeoConstraint::Collinear(a, b) => {
                // The second's points are put on the first's line: an ellipse goes second, so that its
                // centre and axis end come onto the line rather than its axis shrinking to nothing.
                let (a, b) = if sketch.ellipse(a).is_some() { (b, a) } else { (a, b) };
                Constraint::Collinear(self.axis_line(sketch, a)?, self.axis_line(sketch, b)?)
            }
            GeoConstraint::Tangent { a, b, side, external } => {
                // Curves that meet at an end are tangent *there*: written as "the radius at the joint is
                // square to the line" (or "the radii are in line" for two arcs). The general form, the
                // centre's distance from the line, has a zero derivative at a shared point, so it would
                // remove no freedom there.
                if let Some(ellipse) = self.ellipse_tangent(sketch, a, b, side)? {
                    return Ok(ellipse);
                }
                if let Some(joined) = self.end_tangent(sketch, a, b)? {
                    return Ok(joined);
                }
                match (self.line(sketch, a), self.line(sketch, b)) {
                    (Ok(line), Err(_)) => {
                        Constraint::TangentLineCircle { line, round: self.round(sketch, b)?, side }
                    }
                    (Err(_), Ok(line)) => {
                        Constraint::TangentLineCircle { line, round: self.round(sketch, a)?, side }
                    }
                    (Err(_), Err(_)) => Constraint::TangentCircleCircle {
                        a: self.round(sketch, a)?,
                        b: self.round(sketch, b)?,
                        external,
                    },
                    (Ok(_), Ok(_)) => return Err(SketchError::WrongCurveKind),
                }
            }
            // Two ellipses (pieces of one cut ellipse): the same minor radius.
            GeoConstraint::Equal(a, b) if sketch.ellipse(a).is_some() && sketch.ellipse(b).is_some() => {
                let ((ca, _, ra), (cb, _, rb)) = (self.ellipse(sketch, a)?, self.ellipse(sketch, b)?);
                Constraint::EqualRadius(Round { c: ca, r: ra }, Round { c: cb, r: rb })
            }
            GeoConstraint::Equal(a, b) => match (self.line(sketch, a), self.line(sketch, b)) {
                (Ok(l1), Ok(l2)) => Constraint::EqualLength(l1, l2),
                (Err(_), Err(_)) => Constraint::EqualRadius(self.round(sketch, a)?, self.round(sketch, b)?),
                _ => return Err(SketchError::WrongCurveKind),
            },
            GeoConstraint::Concentric(a, b) => {
                Constraint::Coincident(self.round(sketch, a)?.c, self.round(sketch, b)?.c)
            }
            GeoConstraint::Symmetric { p, q, axis } => {
                Constraint::Symmetric { p: self.p2(p)?, q: self.p2(q)?, axis: self.line(sketch, axis)? }
            }
            GeoConstraint::Fix(p) => Constraint::Fix(self.params(p)?.to_vec()),
            GeoConstraint::AlongAxis { p, q, axis } if axis < 3 => {
                Constraint::AlongAxis { p: self.p3(p)?, q: self.p3(q)?, axis }
            }
            GeoConstraint::OnPlane { p, axis } if axis < 3 => {
                Constraint::CoordValue { param: self.p3(p)?[axis], value: 0.0 }
            }
            GeoConstraint::AlongAxis { .. } | GeoConstraint::OnPlane { .. } => {
                return Err(SketchError::InvalidValue);
            }
        })
    }

    pub fn lower_dimension(
        &self,
        sketch: &Sketch,
        kind: &DimKind,
        value: f64,
    ) -> Result<Constraint, SketchError> {
        let is_angle = matches!(kind, DimKind::Angle { .. });
        let in_domain = if is_angle { value > 0.0 && value < std::f64::consts::TAU } else { value > 0.0 };
        if !value.is_finite() || !in_domain {
            return Err(SketchError::InvalidValue);
        }
        Ok(match *kind {
            DimKind::Distance { p, q } if sketch.is_spatial() => {
                Constraint::Distance3 { p: self.p3(p)?, q: self.p3(q)?, value }
            }
            DimKind::Distance { p, q } => Constraint::Distance { p: self.p2(p)?, q: self.p2(q)?, value },
            DimKind::HDistance { p, q, sign } => {
                Constraint::HDistance { p: self.p2(p)?, q: self.p2(q)?, sign, value }
            }
            DimKind::VDistance { p, q, sign } => {
                Constraint::VDistance { p: self.p2(p)?, q: self.p2(q)?, sign, value }
            }
            DimKind::PointLine { p, line, side } => {
                Constraint::PointLineDistance { p: self.p2(p)?, line: self.line(sketch, line)?, side, value }
            }
            DimKind::LineArc { .. } | DimKind::ArcArc { .. } if sketch.is_spatial() => {
                return Err(SketchError::WrongSpace);
            }
            DimKind::LineArc { line, arc, side } => Constraint::LineCircleDistance {
                line: self.line(sketch, line)?,
                round: self.round(sketch, arc)?,
                side,
                value,
            },
            DimKind::ArcArc { a, b, external } => Constraint::CircleCircleDistance {
                a: self.round(sketch, a)?,
                b: self.round(sketch, b)?,
                external,
                value,
            },
            // In space the angle has no sense of turning: it is between 0 and 180°.
            DimKind::Angle { l1, rev1, l2, rev2 } if sketch.is_spatial() => {
                if value >= std::f64::consts::PI {
                    return Err(SketchError::InvalidValue);
                }
                Constraint::Angle3 {
                    l1: self.line3(sketch, l1)?,
                    rev1,
                    l2: self.line3(sketch, l2)?,
                    rev2,
                    value,
                }
            }
            DimKind::Angle { l1, rev1, l2, rev2 } => Constraint::Angle {
                l1: self.line(sketch, l1)?,
                rev1,
                l2: self.line(sketch, l2)?,
                rev2,
                value,
            },
            DimKind::Radius(curve) if sketch.is_spatial() => {
                Constraint::Radius { round: self.radius_only(sketch, curve)?, value }
            }
            DimKind::Diameter(curve) if sketch.is_spatial() => {
                Constraint::Diameter { round: self.radius_only(sketch, curve)?, value }
            }
            // An ellipse's radius dimension is its minor radius (the major one is the distance from the
            // centre to the major axis end).
            DimKind::Radius(curve) if sketch.ellipse(curve).is_some() => {
                let (c, _, minor) = self.ellipse(sketch, curve)?;
                Constraint::Radius { round: Round { c, r: minor }, value }
            }
            DimKind::Radius(curve) => Constraint::Radius { round: self.round(sketch, curve)?, value },
            DimKind::Diameter(curve) => Constraint::Diameter { round: self.round(sketch, curve)?, value },
            DimKind::PlaneDistance { p, axis, side } if axis < 3 => {
                Constraint::CoordValue { param: self.p3(p)?[axis], value: side * value }
            }
            DimKind::PlaneDistance { .. } => return Err(SketchError::InvalidValue),
        })
    }
}

impl Sketch {
    /// Builds the solver system. `skip` leaves out one constraint or dimension (the one being edited).
    pub(crate) fn bind(&self, skip: Option<ConstraintId>) -> Result<Binding, SketchError> {
        let mut binding = Binding {
            sys: System::new(),
            points: BTreeMap::new(),
            radii: BTreeMap::new(),
            fixed_lines: BTreeMap::new(),
        };
        let spatial = self.is_spatial();
        for (id, point) in &self.points {
            let params = if spatial {
                binding.sys.add_point3(point.pos[0], point.pos[1], point.pos[2]).to_vec()
            } else {
                binding.sys.add_point2(point.pos[0], point.pos[1]).to_vec()
            };
            binding.points.insert(*id, params);
        }
        for (id, curve) in &self.curves {
            match curve.geom {
                CurveGeom::Line { .. } => {}
                CurveGeom::Circle { .. } if spatial => return Err(SketchError::WrongSpace),
                // A 3D arc lies in the plane of its three points (`Arc3`).
                CurveGeom::Arc { c, s, e } if spatial => {
                    let (_, radius) = self.round(*id).ok_or(SketchError::MissingPoint)?;
                    let r = binding.sys.add_radius(radius);
                    binding.radii.insert(*id, r);
                    let arc =
                        Constraint::Arc3 { c: binding.p3(c)?, r, start: binding.p3(s)?, end: binding.p3(e)? };
                    binding.sys.add(solver_id(*id), arc);
                }
                CurveGeom::Circle { r, .. } => {
                    binding.radii.insert(*id, binding.sys.add_radius(r));
                }
                CurveGeom::Ellipse { .. } | CurveGeom::Spline { .. } if spatial => {
                    return Err(SketchError::WrongSpace);
                }
                // A spline follows its fit points; it adds nothing to solve.
                CurveGeom::Spline { .. } => {}
                CurveGeom::Ellipse { c, major, minor, arc } => {
                    let r = binding.sys.add_radius(minor);
                    binding.radii.insert(*id, r);
                    // An elliptical arc's ends stay on its ellipse.
                    if let Some((s, e)) = arc {
                        for end in [s, e] {
                            let on = Constraint::PointOnEllipse {
                                p: binding.p2(end)?,
                                c: binding.p2(c)?,
                                major: binding.p2(major)?,
                                minor: r,
                            };
                            // Keyed by the end point (the curve's own key holds nothing here).
                            binding.sys.add(solver_id(end), on);
                        }
                    }
                }
                CurveGeom::Arc { s, e, .. } => {
                    let (_, radius) = self.round(*id).ok_or(SketchError::MissingPoint)?;
                    binding.radii.insert(*id, binding.sys.add_radius(radius));
                    let arc = Constraint::Arc {
                        round: binding.round(self, *id)?,
                        start: binding.p2(s)?,
                        end: binding.p2(e)?,
                    };
                    binding.sys.add(solver_id(*id), arc);
                }
            }
        }
        for (id, point) in &self.points {
            if matches!(point.kind, PointKind::External(_)) {
                binding.sys.add(solver_id(*id), Constraint::Fix(binding.params(*id)?.to_vec()));
            }
        }
        // Pattern copies follow their originals after the solve (`refresh_patterns`), not in it.
        let (held_points, held_circles) = self.pattern_held();
        for id in held_points {
            if let Ok(params) = binding.params(id) {
                let params = params.to_vec();
                binding.sys.add(solver_id(id), Constraint::Fix(params));
            }
        }
        for id in held_circles {
            if let Some(param) = binding.radii.get(&id).copied() {
                binding.sys.add(solver_id(id), Constraint::Fix(vec![param]));
            }
        }
        for (id, constraint) in &self.constraints {
            if Some(solver_id(*id)) != skip {
                binding.prepare(self, constraint)?;
                let lowered = binding.lower_constraint(self, constraint)?;
                binding.sys.add(solver_id(*id), lowered);
            }
        }
        for (id, dimension) in self.dimensions.iter().filter(|(_, d)| d.driving) {
            if Some(solver_id(*id)) != skip {
                let lowered = binding.lower_dimension(self, &dimension.kind, dimension.value)?;
                binding.sys.add(solver_id(*id), lowered);
            }
        }
        Ok(binding)
    }

    /// Copies positions and radii from a solver system back into the sketch. Returns whether any changed.
    pub(crate) fn write_back(
        &mut self,
        sys: &System,
        points: &BTreeMap<Id<SkPoint>, Vec<ParamId>>,
        radii: &BTreeMap<Id<Curve>, ParamId>,
    ) -> bool {
        let mut changed = false;
        for (id, params) in points {
            if let Some(point) = self.points.get_mut(id) {
                for (axis, param) in params.iter().enumerate() {
                    let value = sys.value(*param);
                    changed |= point.pos[axis].to_bits() != value.to_bits();
                    point.pos[axis] = value;
                }
            }
        }
        for (id, param) in radii {
            if let Some(Curve {
                geom: CurveGeom::Circle { r, .. } | CurveGeom::Ellipse { minor: r, .. },
                ..
            }) = self.curves.get_mut(id)
            {
                let value = sys.value(*param);
                changed |= r.to_bits() != value.to_bits();
                *r = value;
            }
        }
        changed
    }

    /// The value a dimension of this kind currently has, measured with the same signs as the driving
    /// constraint, so switching a dimension between driving and reference keeps its value.
    pub fn measure(&self, kind: &DimKind) -> Option<f64> {
        let pos = |p: Id<SkPoint>| self.pos(p);
        let dir = |line: Id<Curve>, reversed: bool| -> Option<[f64; 2]> {
            let (a, b) = self.line(line)?;
            let (a, b) = (pos(a)?, pos(b)?);
            let sign = if reversed { -1.0 } else { 1.0 };
            Some([sign * (b[0] - a[0]), sign * (b[1] - a[1])])
        };
        Some(match *kind {
            DimKind::Distance { p, q } => {
                let (p, q) = (pos(p)?, pos(q)?);
                (0..3).map(|k| (q[k] - p[k]).powi(2)).sum::<f64>().sqrt()
            }
            DimKind::HDistance { p, q, sign } => sign * (pos(q)?[0] - pos(p)?[0]),
            DimKind::VDistance { p, q, sign } => sign * (pos(q)?[1] - pos(p)?[1]),
            DimKind::PointLine { p, line, side } => {
                let (a, b) = self.line(line)?;
                let (a, b, p) = (pos(a)?, pos(b)?, pos(p)?);
                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                let length = (dx * dx + dy * dy).sqrt();
                if length == 0.0 {
                    return None;
                }
                side * (dx * (p[1] - a[1]) - dy * (p[0] - a[0])) / length
            }
            DimKind::LineArc { line, arc, side } => {
                let (a, b) = self.line(line)?;
                let (centre, r) = self.round(arc)?;
                let (a, b, c) = (pos(a)?, pos(b)?, pos(centre)?);
                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                let length = (dx * dx + dy * dy).sqrt();
                if length == 0.0 {
                    return None;
                }
                side * (dx * (c[1] - a[1]) - dy * (c[0] - a[0])) / length - r
            }
            DimKind::ArcArc { a, b, external } => {
                let ((ca, ra), (cb, rb)) = (self.round(a)?, self.round(b)?);
                let (ca, cb) = (pos(ca)?, pos(cb)?);
                let apart = ((cb[0] - ca[0]).powi(2) + (cb[1] - ca[1]).powi(2)).sqrt();
                if external { apart - ra - rb } else { (ra - rb).abs() - apart }
            }
            DimKind::Angle { l1, rev1, l2, rev2 } if self.is_spatial() => {
                let dir3 = |line: Id<Curve>, reversed: bool| -> Option<[f64; 3]> {
                    let (a, b) = self.line(line)?;
                    let (a, b) = (pos(a)?, pos(b)?);
                    let sign = if reversed { -1.0 } else { 1.0 };
                    Some([0, 1, 2].map(|k| sign * (b[k] - a[k])))
                };
                let (u, w) = (dir3(l1, rev1)?, dir3(l2, rev2)?);
                let cross = [u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]];
                let across = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
                gc_math::fm::atan2(across, u[0] * w[0] + u[1] * w[1] + u[2] * w[2])
            }
            DimKind::Angle { l1, rev1, l2, rev2 } => {
                let (u, w) = (dir(l1, rev1)?, dir(l2, rev2)?);
                gc_math::fm::atan2(u[0] * w[1] - u[1] * w[0], u[0] * w[0] + u[1] * w[1])
                    .rem_euclid(std::f64::consts::TAU)
            }
            DimKind::Radius(curve) => match self.ellipse(curve) {
                Some((_, _, minor)) => minor,
                None => self.round(curve)?.1,
            },
            DimKind::Diameter(curve) => 2.0 * self.round(curve)?.1,
            DimKind::PlaneDistance { p, axis, side } => side * *pos(p)?.get(axis)?,
        })
    }
}

impl Dimension {
    /// The value to display: the driving value, or the measurement for a reference dimension.
    pub fn display_value(&self, sketch: &Sketch) -> Option<f64> {
        if self.driving { Some(self.value) } else { sketch.measure(&self.kind) }
    }
}
