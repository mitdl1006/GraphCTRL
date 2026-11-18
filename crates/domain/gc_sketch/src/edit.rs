//! Editing a sketch: adding geometry, constraints and dimensions, solving and deleting.

use std::collections::BTreeSet;

use gc_core::{Id, IdAlloc};
use gc_solver::{AddOutcome, Blame, DepKind, SolveReport, Status};

use crate::bind::{Binding, solver_id};
use crate::model::{
    Curve, CurveGeom, DimKind, Dimension, ExternalRef, GeoConstraint, PointKind, SkPoint, Sketch,
};

/// Why a sketch edit was refused.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum SketchError {
    /// A point id that the sketch does not have.
    #[error("the sketch has no such point")]
    MissingPoint,
    /// A curve id that the sketch does not have.
    #[error("the sketch has no such curve")]
    MissingCurve,
    /// A constraint or dimension id that the sketch does not have.
    #[error("the sketch has no such constraint or dimension")]
    MissingItem,
    /// The constraint does not apply to this kind of curve (e.g. the radius of a line).
    #[error("this does not apply to that kind of curve")]
    WrongCurveKind,
    /// The item belongs to 2D sketches but the sketch is 3D, or the reverse.
    #[error("this is not available in this kind of sketch")]
    WrongSpace,
    /// A value outside its domain: a length that is not positive, an angle outside (0, 360°).
    #[error("the value is out of range")]
    InvalidValue,
    /// The sketch cannot be solved with the change; it was left as it was.
    #[error("the sketch cannot be solved with this change ({0:?})")]
    Unsolved(Status),
}

/// A sketch item, as named in reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum Item {
    /// A point (reported when it is a fixed reference).
    Point(Id<SkPoint>),
    /// A curve (reported for the implicit constraints of an arc).
    Curve(Id<Curve>),
    /// A geometric constraint.
    Constraint(Id<GeoConstraint>),
    /// A dimension.
    Dimension(Id<Dimension>),
}

/// The state of a sketch after a solve.
#[derive(Clone, Debug, PartialEq)]
pub struct SketchReport {
    /// How the solve ended.
    pub status: Status,
    /// Remaining degrees of freedom: how many more dimensions the sketch needs.
    pub dof: usize,
    /// Redundant and conflicting items: `(item, what it depends on, how)`.
    pub dependencies: Vec<(Item, Vec<Item>, DepKind)>,
    /// Items that refer to collapsed geometry.
    pub degenerate: Vec<Item>,
    free_points: BTreeSet<Id<SkPoint>>,
    free_radii: BTreeSet<Id<Curve>>,
}

impl SketchReport {
    /// True when the point can still move.
    pub fn point_is_free(&self, point: Id<SkPoint>) -> bool {
        self.free_points.contains(&point)
    }

    /// True when any part of the curve can still move (shown in the "under-constrained" colour).
    pub fn curve_is_free(&self, sketch: &Sketch, curve: Id<Curve>) -> bool {
        self.free_radii.contains(&curve)
            || sketch.curves.get(&curve).is_some_and(|c| c.points().iter().any(|p| self.point_is_free(*p)))
    }
}

/// The result of trying to add a constraint or driving dimension.
#[derive(Clone, Debug, PartialEq)]
pub enum AddResult<T> {
    /// It was added and the sketch re-solved.
    Added {
        /// The id of the new item.
        id: Id<T>,
        /// The state of the sketch.
        report: SketchReport,
    },
    /// It follows from existing items; the sketch is unchanged.
    Redundant(Vec<Item>),
    /// It contradicts existing items; the sketch is unchanged.
    Conflicting(Vec<Item>),
}

impl Sketch {
    fn item_of(&self, raw: u64) -> Option<Item> {
        let raw = std::num::NonZeroU64::new(raw)?;
        if self.constraints.contains_key(&Id::from_raw(raw)) {
            Some(Item::Constraint(Id::from_raw(raw)))
        } else if self.dimensions.contains_key(&Id::from_raw(raw)) {
            Some(Item::Dimension(Id::from_raw(raw)))
        } else if self.curves.contains_key(&Id::from_raw(raw)) {
            Some(Item::Curve(Id::from_raw(raw)))
        } else if self.points.contains_key(&Id::from_raw(raw)) {
            Some(Item::Point(Id::from_raw(raw)))
        } else {
            None
        }
    }

    fn items(&self, blame: &[Blame]) -> Vec<Item> {
        let set: BTreeSet<Item> = blame
            .iter()
            .filter_map(|b| match b {
                Blame::Row(c, _) | Blame::Merged(c) => self.item_of(c.0),
            })
            .collect();
        set.into_iter().collect()
    }

    fn report(&self, binding: &Binding, solved: &SolveReport) -> SketchReport {
        let own = |c: gc_solver::ConstraintId| self.item_of(c.0);
        SketchReport {
            status: solved.status,
            dof: solved.dof,
            dependencies: solved
                .dependencies
                .iter()
                .filter_map(|d| Some((own(d.row.0)?, self.items(&d.with), d.kind)))
                .collect(),
            degenerate: solved.degenerate.iter().filter_map(|c| own(*c)).collect(),
            free_points: binding
                .points
                .iter()
                .filter(|(_, params)| params.iter().any(|p| solved.is_mobile(*p)))
                .map(|(id, _)| *id)
                .collect(),
            free_radii: binding
                .radii
                .iter()
                .filter(|(_, p)| solved.is_mobile(**p))
                .map(|(id, _)| *id)
                .collect(),
        }
    }

    /// Adds a free point.
    pub fn add_point(&mut self, alloc: &mut IdAlloc, pos: [f64; 3]) -> Id<SkPoint> {
        let id = alloc.alloc();
        self.points.insert(id, SkPoint { pos, kind: PointKind::Free, center: false });
        id
    }

    /// Adds a reference point that follows outside geometry. The caller keeps its position up to date.
    pub fn add_external(&mut self, alloc: &mut IdAlloc, source: ExternalRef, pos: [f64; 3]) -> Id<SkPoint> {
        let id = alloc.alloc();
        self.points.insert(id, SkPoint { pos, kind: PointKind::External(source), center: false });
        id
    }

    /// Adds a line with its own two end points.
    pub fn add_line(&mut self, alloc: &mut IdAlloc, a: [f64; 3], b: [f64; 3]) -> Id<Curve> {
        let (a, b) = (self.add_point(alloc, a), self.add_point(alloc, b));
        let id = alloc.alloc();
        self.curves.insert(id, Curve { geom: CurveGeom::Line { a, b }, construction: false });
        id
    }

    /// Adds a circle with its own centre point.
    pub fn add_circle(
        &mut self,
        alloc: &mut IdAlloc,
        centre: [f64; 3],
        radius: f64,
    ) -> Result<Id<Curve>, SketchError> {
        if self.is_spatial() {
            return Err(SketchError::WrongSpace);
        }
        if !(radius.is_finite() && radius > 0.0) {
            return Err(SketchError::InvalidValue);
        }
        let c = self.add_point(alloc, centre);
        let id = alloc.alloc();
        self.curves.insert(id, Curve { geom: CurveGeom::Circle { c, r: radius }, construction: false });
        Ok(id)
    }

    /// Adds an interpolation spline through new fit points at `points` (two or more, finite, no two in a
    /// row at the same place).
    pub fn add_spline(&mut self, alloc: &mut IdAlloc, points: &[[f64; 3]]) -> Result<Id<Curve>, SketchError> {
        // In a 2D sketch the fit points lie in its plane; in a 3D sketch anywhere ("Interpolation splines
        // can be created in both 2D and 3D sketches", GUID-DB15C479-66F2-456A-9E54-40A60F83DC92).
        let spatial = self.is_spatial();
        let finite = points.iter().flatten().all(|v| v.is_finite());
        let apart = points.windows(2).all(|w| {
            let dz = if spatial { w[1][2] - w[0][2] } else { 0.0 };
            ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2) + dz * dz).sqrt() > 1e-9
        });
        if points.len() < 2 || !finite || !apart {
            return Err(SketchError::InvalidValue);
        }
        let fit: Vec<Id<SkPoint>> = points.iter().map(|p| self.add_point(alloc, *p)).collect();
        let id = alloc.alloc();
        self.curves.insert(id, Curve { geom: CurveGeom::Spline { points: fit }, construction: false });
        Ok(id)
    }

    /// Adds an ellipse with its own centre and major axis end points and the given minor radius.
    pub fn add_ellipse(
        &mut self,
        alloc: &mut IdAlloc,
        centre: [f64; 3],
        major: [f64; 3],
        minor: f64,
    ) -> Result<Id<Curve>, SketchError> {
        if self.is_spatial() {
            return Err(SketchError::WrongSpace);
        }
        let a = ((major[0] - centre[0]).powi(2) + (major[1] - centre[1]).powi(2)).sqrt();
        let finite = centre.iter().chain(&major).all(|v| v.is_finite());
        if !(finite && a > 1e-9 && minor.is_finite() && minor > 1e-9) {
            return Err(SketchError::InvalidValue);
        }
        let c = self.add_point(alloc, centre);
        let major = self.add_point(alloc, major);
        let id = alloc.alloc();
        self.curves.insert(
            id,
            Curve { geom: CurveGeom::Ellipse { c, major, minor, arc: None }, construction: false },
        );
        Ok(id)
    }

    /// Adds an arc (counter-clockwise from `start` to `end`) with its own three points. The end point is
    /// moved onto the circle through `start`.
    pub fn add_arc(
        &mut self,
        alloc: &mut IdAlloc,
        centre: [f64; 3],
        start: [f64; 3],
        end: [f64; 3],
    ) -> Result<Id<Curve>, SketchError> {
        if self.is_spatial() {
            return Err(SketchError::WrongSpace);
        }
        let radius = ((start[0] - centre[0]).powi(2) + (start[1] - centre[1]).powi(2)).sqrt();
        let reach = ((end[0] - centre[0]).powi(2) + (end[1] - centre[1]).powi(2)).sqrt();
        if !(radius.is_finite() && radius > 0.0 && reach.is_finite() && reach > 0.0) {
            return Err(SketchError::InvalidValue);
        }
        let scale = radius / reach;
        let end = [centre[0] + (end[0] - centre[0]) * scale, centre[1] + (end[1] - centre[1]) * scale, 0.0];
        let (c, s, e) =
            (self.add_point(alloc, centre), self.add_point(alloc, start), self.add_point(alloc, end));
        let id = alloc.alloc();
        self.curves.insert(id, Curve { geom: CurveGeom::Arc { c, s, e }, construction: false });
        Ok(id)
    }

    /// Solves the sketch. Coordinates change only when the result is [`Status::Solved`].
    pub fn solve(&mut self) -> Result<SketchReport, SketchError> {
        let mut binding = self.bind(None)?;
        let solved = binding.sys.solve();
        let report = self.report(&binding, &solved);
        if solved.status == Status::Solved {
            self.write_back(&binding.sys, &binding.points, &binding.radii);
            // Pattern copies follow the solved originals; what hangs on the copies is solved again.
            if self.refresh_patterns() {
                let mut binding = self.bind(None)?;
                let solved = binding.sys.solve();
                let report = self.report(&binding, &solved);
                if solved.status == Status::Solved {
                    self.write_back(&binding.sys, &binding.points, &binding.radii);
                }
                return Ok(report);
            }
        }
        Ok(report)
    }

    /// How far the stored coordinates are from satisfying the constraints, relative to the tolerance
    /// (at most 1 when the sketch is consistent). Used to check loaded files without changing them.
    pub fn violation(&self) -> Result<f64, SketchError> {
        Ok(self.bind(None)?.sys.violation())
    }

    fn add_outcome<T>(
        &mut self,
        binding: &Binding,
        id: Id<T>,
        outcome: AddOutcome,
        insert: impl FnOnce(&mut Self),
    ) -> Result<AddResult<T>, SketchError> {
        match outcome {
            AddOutcome::Added(solved) | AddOutcome::AddedSingular(solved) => {
                insert(self);
                self.write_back(&binding.sys, &binding.points, &binding.radii);
                Ok(AddResult::Added { id, report: self.report(binding, &solved) })
            }
            AddOutcome::Redundant { with } => Ok(AddResult::Redundant(self.items(&with))),
            AddOutcome::Conflicting { with } => Ok(AddResult::Conflicting(self.items(&with))),
            AddOutcome::Failed(status) => Err(SketchError::Unsolved(status)),
        }
    }

    /// Adds a spline's Tangent or Smooth: the spline's end must be joined to an end of the other curve (a
    /// line, arc or open spline), and that end must not be shaped already (a second condition there is
    /// redundant).
    fn add_spline_shaping(
        &mut self,
        alloc: &mut IdAlloc,
        constraint: GeoConstraint,
    ) -> Result<AddResult<GeoConstraint>, SketchError> {
        let (a, b) = match constraint {
            GeoConstraint::Tangent { a, b, .. } | GeoConstraint::Smooth(a, b) => (a, b),
            _ => return Err(SketchError::InvalidValue),
        };
        let is_spline =
            |c: Id<Curve>| matches!(self.curves.get(&c).map(|k| &k.geom), Some(CurveGeom::Spline { .. }));
        // The spline whose end is shaped: the first that is a spline (Smooth names the spline first).
        let (spline, other) = if is_spline(a) { (a, b) } else { (b, a) };
        let (end, _, _) = self.spline_joint(spline, other).ok_or(SketchError::InvalidValue)?;
        let taken: Vec<Id<GeoConstraint>> = self
            .constraints
            .iter()
            .filter(|(_, c)| self.shapes_spline(c))
            .filter(|(_, c)| {
                let (x, y) = match **c {
                    GeoConstraint::Tangent { a, b, .. } | GeoConstraint::Smooth(a, b) => (a, b),
                    _ => return false,
                };
                let partner = if x == spline {
                    y
                } else if y == spline {
                    x
                } else {
                    return false;
                };
                self.spline_joint(spline, partner).is_some_and(|(e, _, _)| e == end)
            })
            .map(|(id, _)| *id)
            .collect();
        if !taken.is_empty() {
            return Ok(AddResult::Redundant(taken.into_iter().map(Item::Constraint).collect()));
        }
        let id: Id<GeoConstraint> = alloc.alloc();
        self.constraints.insert(id, constraint);
        let report = self.solve()?;
        Ok(AddResult::Added { id, report })
    }

    /// Adds a geometric constraint if it is consistent and removes freedom.
    pub fn try_add_constraint(
        &mut self,
        alloc: &mut IdAlloc,
        constraint: GeoConstraint,
    ) -> Result<AddResult<GeoConstraint>, SketchError> {
        // Tangent and Smooth take two curves; a curve with itself is not a pair.
        if let GeoConstraint::Tangent { a, b, .. } | GeoConstraint::Smooth(a, b) = constraint
            && a == b
        {
            return Err(SketchError::InvalidValue);
        }
        if self.shapes_spline(&constraint) {
            return self.add_spline_shaping(alloc, constraint);
        }
        let mut binding = self.bind(None)?;
        binding.prepare(self, &constraint)?;
        let lowered = binding.lower_constraint(self, &constraint)?;
        let id: Id<GeoConstraint> = alloc.alloc();
        let outcome = binding.sys.try_add(solver_id(id), lowered);
        self.add_outcome(&binding, id, outcome, |sketch| {
            sketch.constraints.insert(id, constraint);
        })
    }

    /// Adds a driving dimension if it is consistent and removes freedom. When it is refused as redundant
    /// or conflicting, the caller may offer [`Sketch::add_reference_dimension`] instead.
    pub fn try_add_dimension(
        &mut self,
        alloc: &mut IdAlloc,
        kind: DimKind,
        source: &str,
        value: f64,
        text_pos: [f64; 2],
    ) -> Result<AddResult<Dimension>, SketchError> {
        let mut binding = self.bind(None)?;
        let lowered = binding.lower_dimension(self, &kind, value)?;
        let id: Id<Dimension> = alloc.alloc();
        let outcome = binding.sys.try_add(solver_id(id), lowered);
        self.add_outcome(&binding, id, outcome, |sketch| {
            let dimension = Dimension {
                kind,
                source: source.to_owned(),
                value,
                driving: true,
                text_pos,
                name: String::new(),
            };
            sketch.dimensions.insert(id, dimension);
        })
    }

    /// Adds a reference dimension: it shows a measurement and constrains nothing.
    pub fn add_reference_dimension(
        &mut self,
        alloc: &mut IdAlloc,
        kind: DimKind,
        text_pos: [f64; 2],
    ) -> Id<Dimension> {
        let id = alloc.alloc();
        let dimension = Dimension {
            kind,
            source: String::new(),
            value: 0.0,
            driving: false,
            text_pos,
            name: String::new(),
        };
        self.dimensions.insert(id, dimension);
        id
    }

    /// Makes a dimension driven (`None`: it only shows the measurement) or driving with the given source
    /// text and value. Making it driving is checked like adding it: refused (with the items in the way)
    /// when it would be redundant or conflicting.
    pub fn set_driving(
        &mut self,
        id: Id<Dimension>,
        driving: Option<(&str, f64)>,
    ) -> Result<AddResult<Dimension>, SketchError> {
        let dimension = self.dimensions.get(&id).ok_or(SketchError::MissingItem)?.clone();
        let Some((source, value)) = driving else {
            if let Some(d) = self.dimensions.get_mut(&id) {
                d.driving = false;
            }
            let binding = self.bind(None)?;
            let solved = binding.sys.clone().solve();
            return Ok(AddResult::Added { id, report: self.report(&binding, &solved) });
        };
        if dimension.driving {
            return Err(SketchError::InvalidValue);
        }
        let mut binding = self.bind(None)?;
        let lowered = binding.lower_dimension(self, &dimension.kind, value)?;
        let outcome = binding.sys.try_add(solver_id(id), lowered);
        self.add_outcome(&binding, id, outcome, |sketch| {
            if let Some(d) = sketch.dimensions.get_mut(&id) {
                d.driving = true;
                d.value = value;
                source.clone_into(&mut d.source);
            }
        })
    }

    /// Changes the value of a driving dimension. The geometry follows continuously; if the new value
    /// cannot be reached the sketch is left unchanged and the error says why.
    pub fn set_dimension(
        &mut self,
        id: Id<Dimension>,
        source: &str,
        value: f64,
    ) -> Result<SketchReport, SketchError> {
        let dimension = self.dimensions.get(&id).ok_or(SketchError::MissingItem)?;
        if !dimension.driving {
            return Err(SketchError::InvalidValue);
        }
        let mut binding = self.bind(None)?;
        binding.lower_dimension(self, &dimension.kind, value)?; // validates the value's domain
        let solved = binding.sys.set_dimension(solver_id(id), value);
        if solved.status != Status::Solved {
            return Err(SketchError::Unsolved(solved.status));
        }
        if let Some(dimension) = self.dimensions.get_mut(&id) {
            dimension.value = value;
            source.clone_into(&mut dimension.source);
        }
        self.write_back(&binding.sys, &binding.points, &binding.radii);
        Ok(self.report(&binding, &solved))
    }

    /// Deletes items together with everything that depends on them: a curve takes its points with it,
    /// and constraints and dimensions that refer to deleted geometry go too. Reference points that follow
    /// outside geometry are never deleted this way.
    pub fn delete(&mut self, items: &[Item]) {
        let mut points: BTreeSet<Id<SkPoint>> = BTreeSet::new();
        let mut curves: BTreeSet<Id<Curve>> = BTreeSet::new();
        for item in items {
            match *item {
                Item::Point(p) => {
                    let owned = self.curves.values().any(|c| c.points().contains(&p));
                    let external =
                        self.points.get(&p).is_some_and(|p| matches!(p.kind, PointKind::External(_)));
                    if !owned && !external {
                        points.insert(p);
                    }
                }
                Item::Curve(c) => {
                    curves.insert(c);
                }
                Item::Constraint(c) => {
                    self.constraints.remove(&c);
                }
                Item::Dimension(d) => {
                    self.dimensions.remove(&d);
                }
            }
        }
        for curve in &curves {
            if let Some(removed) = self.curves.remove(curve) {
                points.extend(removed.points());
            }
        }
        for point in &points {
            self.points.remove(point);
        }
        self.constraints.retain(|_, c| {
            Self::constraint_points(c).iter().all(|p| !points.contains(p))
                && Self::constraint_curves(c).iter().all(|c| !curves.contains(c))
        });
        self.dimensions.retain(|_, d| {
            let (dim_points, dim_curves) = Self::dimension_refs(&d.kind);
            dim_points.iter().all(|p| !points.contains(p)) && dim_curves.iter().all(|c| !curves.contains(c))
        });
    }
}
