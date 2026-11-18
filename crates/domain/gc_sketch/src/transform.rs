//! Copy, Move, Rotate, Scale and Stretch of 2D sketch geometry (Inventor 2026 help, To Copy, Move, or
//! Rotate 2D Sketch Geometry, GUID-F8114270-18F7-4C7A-947E-AC489EDACFA9; To Stretch, Scale, or Offset 2D
//! Sketch Geometry, GUID-962CEE71-8BE4-4DA6-B51E-341319C64FA4).
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

/// How a move treats what holds the selection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoveRules {
    /// Break constraints tying the selection to other geometry (Move, Rotate, Scale relaxed; never Stretch,
    /// whose connected geometry stretches).
    pub break_ties: bool,
    /// Break horizontal and vertical constraints within the selection (a relaxed Rotate).
    pub break_level: bool,
    /// Dimensions between the selection and other geometry take their new values.
    pub recompute_crossing: bool,
    /// Dimensions within the selection take their new values (a relaxed Scale).
    pub recompute_inside: bool,
    /// Circle radii are multiplied by this (Scale; 1 otherwise).
    pub radius: f64,
}

impl MoveRules {
    /// Move or Rotate (`turned`), relaxed or not.
    pub fn moving(turned: bool, relax: bool) -> Self {
        MoveRules {
            break_ties: relax,
            break_level: relax && turned,
            recompute_crossing: relax,
            recompute_inside: false,
            radius: 1.0,
        }
    }
}

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
        radius: f64,
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
                CurveGeom::Circle { c, r } => CurveGeom::Circle { c: to(c)?, r: r * radius },
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

    /// Moves `curves` and `points` (all their points by `place`), as `rules` say.
    pub fn move_with(
        &mut self,
        alloc: &mut IdAlloc,
        curves: &[Id<Curve>],
        extra: &[Id<SkPoint>],
        place: &dyn Fn([f64; 3]) -> [f64; 3],
        rules: MoveRules,
    ) -> Result<(), SketchError> {
        if (curves.is_empty() && extra.is_empty()) || self.is_spatial() {
            return Err(SketchError::InvalidValue);
        }
        let mut points = self.selected_points(curves)?;
        points.extend(extra.iter().copied().filter(|p| self.points.contains_key(p)));
        let selected: BTreeSet<Id<Curve>> = curves.iter().copied().collect();
        let touches = |pts: &[Id<SkPoint>], cvs: &[Id<Curve>]| {
            let inside = pts.iter().any(|p| points.contains(p)) || cvs.iter().any(|c| selected.contains(c));
            let outside =
                pts.iter().any(|p| !points.contains(p)) || cvs.iter().any(|c| !selected.contains(c));
            (inside, outside)
        };
        // The points that move with the selection: it, and what is coincident with it (a stretched line's
        // end is its own point, held on the moved one).
        let mut moving = points.clone();
        if !rules.break_ties {
            loop {
                let before = moving.len();
                for c in self.constraints.values() {
                    if let GeoConstraint::Coincident(p, q) = *c {
                        if moving.contains(&p) {
                            moving.insert(q);
                        }
                        if moving.contains(&q) {
                            moving.insert(p);
                        }
                    }
                }
                if moving.len() == before {
                    break;
                }
            }
        }
        let dimension_touches = |pts: &[Id<SkPoint>], cvs: &[Id<Curve>]| {
            let inside = pts.iter().any(|p| moving.contains(p)) || cvs.iter().any(|c| selected.contains(c));
            let outside =
                pts.iter().any(|p| !moving.contains(p)) || cvs.iter().any(|c| !selected.contains(c));
            (inside, outside)
        };
        let mut recompute = Vec::new();
        if rules.break_ties || rules.break_level {
            self.constraints.retain(|_, c| {
                let (inside, outside) = touches(&Self::constraint_points(c), &Self::constraint_curves(c));
                let level = matches!(c, GeoConstraint::Horizontal(..) | GeoConstraint::Vertical(..));
                !(inside && ((rules.break_ties && outside) || (rules.break_level && level)))
            });
        }
        for (id, d) in &self.dimensions {
            let (pts, cvs) = Self::dimension_refs(&d.kind);
            let (inside, outside) = dimension_touches(&pts, &cvs);
            if (inside && outside && rules.recompute_crossing)
                || (inside && !outside && rules.recompute_inside)
            {
                recompute.push(*id);
            }
        }
        // Circles of the selection grow with a scale.
        if (rules.radius - 1.0).abs() > 1e-15 {
            for id in curves {
                if let Some(Curve { geom: CurveGeom::Circle { r, .. }, .. }) = self.curves.get_mut(id) {
                    *r *= rules.radius;
                }
            }
        }
        // What moves goes where it is going (points held on the selection with it).
        for p in &moving {
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

impl Sketch {
    /// Moves or turns `curves` (see [`MoveRules::moving`]).
    pub fn move_placed(
        &mut self,
        alloc: &mut IdAlloc,
        curves: &[Id<Curve>],
        place: &dyn Fn([f64; 3]) -> [f64; 3],
        turned: bool,
        relax: bool,
    ) -> Result<(), SketchError> {
        self.move_with(alloc, curves, &[], place, MoveRules::moving(turned, relax))
    }
}

/// The items a copy made, for a command's report.
pub fn copied_items(result: &CopyResult) -> Vec<Item> {
    let mut items: Vec<Item> = result.points.iter().map(|p| Item::Point(*p)).collect();
    items.extend(result.curves.iter().map(|c| Item::Curve(*c)));
    items.extend(result.constraints.iter().map(|c| Item::Constraint(*c)));
    items
}
