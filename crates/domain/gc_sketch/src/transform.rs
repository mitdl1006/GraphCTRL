//! Copy, Move and Rotate of 2D sketch geometry (Inventor 2026 help, To Copy, Move, or Rotate 2D Sketch
//! Geometry, GUID-F8114270-18F7-4C7A-947E-AC489EDACFA9).
//!
//! - A copy takes the curves, their points and the geometric constraints among them (nothing ties a copy
//!   to the rest of the sketch).
//! - A move puts the selected curves' points where the motion says, holding them there while the sketch
//!   solves; what is tied to them follows. If the sketch cannot take it, the move is refused — unless
//!   constraints are relaxed: then constraints tying the selection to other geometry are broken (and a
//!   turn breaks horizontal and vertical constraints within it), and dimensions tying it to other geometry
//!   take the values they measure afterwards (Inventor's More options set to Always).

use std::collections::{BTreeMap, BTreeSet};

use gc_core::{Id, IdAlloc};

use crate::edit::{Item, SketchError};
use crate::{Curve, CurveGeom, DimKind, GeoConstraint, SkPoint, Sketch, Status};

/// What a copy made.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CopyResult {
    /// The copied curves, in the order of the originals.
    pub curves: Vec<Id<Curve>>,
    /// The copied points.
    pub points: Vec<Id<SkPoint>>,
    /// The copied constraints.
    pub constraints: Vec<Id<GeoConstraint>>,
}

impl Sketch {
    fn selected_points(&self, curves: &[Id<Curve>]) -> Result<BTreeSet<Id<SkPoint>>, SketchError> {
        let mut points = BTreeSet::new();
        for id in curves {
            points.extend(self.curves.get(id).ok_or(SketchError::MissingCurve)?.points());
        }
        Ok(points)
    }

    /// Copies `curves` with their points moved by `place`, and the geometric constraints among them.
    pub fn copy_placed(
        &mut self,
        alloc: &mut IdAlloc,
        curves: &[Id<Curve>],
        place: &dyn Fn([f64; 3]) -> [f64; 3],
    ) -> Result<CopyResult, SketchError> {
        if curves.is_empty() || self.is_spatial() {
            return Err(SketchError::InvalidValue);
        }
        let points = self.selected_points(curves)?;
        let mut result = CopyResult::default();
        let mut point_map = BTreeMap::new();
        for p in &points {
            let at = place(self.pos(*p).ok_or(SketchError::MissingPoint)?);
            let copy = self.add_point(alloc, at);
            if let (Some(original), Some(new)) = (self.points.get(p).copied(), self.points.get_mut(&copy)) {
                new.center = original.center;
            }
            point_map.insert(*p, copy);
            result.points.push(copy);
        }
        let to = |p: Id<SkPoint>| point_map.get(&p).copied().ok_or(SketchError::MissingPoint);
        let mut curve_map = BTreeMap::new();
        for id in curves {
            let curve = self.curves[id];
            let geom = match curve.geom {
                CurveGeom::Line { a, b } => CurveGeom::Line { a: to(a)?, b: to(b)? },
                CurveGeom::Circle { c, r } => CurveGeom::Circle { c: to(c)?, r },
                CurveGeom::Arc { c, s, e } => CurveGeom::Arc { c: to(c)?, s: to(s)?, e: to(e)? },
            };
            let copy: Id<Curve> = alloc.alloc();
            self.curves.insert(copy, Curve { geom, construction: curve.construction });
            curve_map.insert(*id, copy);
            result.curves.push(copy);
        }
        // Constraints wholly within the selection come along.
        let selected: BTreeSet<Id<Curve>> = curves.iter().copied().collect();
        let inside: Vec<GeoConstraint> = self
            .constraints
            .values()
            .filter(|c| {
                Self::constraint_points(c).iter().all(|p| points.contains(p))
                    && Self::constraint_curves(c).iter().all(|c| selected.contains(c))
                    && !matches!(c, GeoConstraint::Fix(_))
            })
            .copied()
            .collect();
        for constraint in inside {
            let mapped = crate::cut::rename(
                &constraint,
                |p| point_map.get(&p).copied().unwrap_or(p),
                |c| curve_map.get(&c).copied().unwrap_or(c),
            );
            // A turned copy may not keep a horizontal or vertical; it is left out then.
            if let Ok(crate::AddResult::Added { id, .. }) = self.try_add_constraint(alloc, mapped) {
                result.constraints.push(id);
            }
        }
        Ok(result)
    }

    /// Moves `curves` (their points by `place`). `turned` says the motion turns them (horizontal and
    /// vertical constraints within the selection then break when relaxing).
    pub fn move_placed(
        &mut self,
        alloc: &mut IdAlloc,
        curves: &[Id<Curve>],
        place: &dyn Fn([f64; 3]) -> [f64; 3],
        turned: bool,
        relax: bool,
    ) -> Result<(), SketchError> {
        if curves.is_empty() || self.is_spatial() {
            return Err(SketchError::InvalidValue);
        }
        let points = self.selected_points(curves)?;
        let selected: BTreeSet<Id<Curve>> = curves.iter().copied().collect();
        let touches = |pts: &[Id<SkPoint>], cvs: &[Id<Curve>]| {
            let inside = pts.iter().any(|p| points.contains(p)) || cvs.iter().any(|c| selected.contains(c));
            let outside =
                pts.iter().any(|p| !points.contains(p)) || cvs.iter().any(|c| !selected.contains(c));
            (inside, outside)
        };
        let mut recompute = Vec::new();
        if relax {
            self.constraints.retain(|_, c| {
                let (inside, outside) = touches(&Self::constraint_points(c), &Self::constraint_curves(c));
                let level = matches!(c, GeoConstraint::Horizontal(..) | GeoConstraint::Vertical(..));
                !(inside && (outside || (turned && level)))
            });
            for (id, d) in &self.dimensions {
                let (pts, cvs) = Self::dimension_refs(&d.kind);
                if let (true, true) = touches(&pts, &cvs) {
                    recompute.push(*id);
                }
            }
        }
        for p in &points {
            if let Some(point) = self.points.get(p).copied() {
                let at = place(point.pos);
                if let Some(point) = self.points.get_mut(p) {
                    point.pos = at;
                }
            }
        }
        // Relaxed dimensions take what they measure now.
        for id in recompute {
            let Some(kind) = self.dimensions.get(&id).map(|d| d.kind) else { continue };
            let Some(value) = self.measure(&kind) else { continue };
            if let Some(d) = self.dimensions.get_mut(&id) {
                let angle = matches!(kind, DimKind::Angle { .. });
                d.source = if angle {
                    format!("{:.6} deg", value.to_degrees())
                } else {
                    format!("{:.6} mm", value * 1e3)
                };
                d.value = value;
            }
        }
        // Hold the selection where it was put while the rest settles.
        let mut held = Vec::new();
        for p in &points {
            let id: Id<GeoConstraint> = alloc.alloc();
            self.constraints.insert(id, GeoConstraint::Fix(*p));
            held.push(id);
        }
        let report = self.solve();
        for id in &held {
            self.constraints.remove(id);
        }
        match report {
            Ok(report) if report.status == Status::Solved => Ok(()),
            Ok(report) => Err(SketchError::Unsolved(report.status)),
            Err(error) => Err(error),
        }
    }
}

/// The items a copy made, for a command's report.
pub fn copied_items(result: &CopyResult) -> Vec<Item> {
    let mut items: Vec<Item> = result.points.iter().map(|p| Item::Point(*p)).collect();
    items.extend(result.curves.iter().map(|c| Item::Curve(*c)));
    items.extend(result.constraints.iter().map(|c| Item::Constraint(*c)));
    items
}
