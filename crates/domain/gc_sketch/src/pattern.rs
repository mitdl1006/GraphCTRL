//! Sketch patterns kept as objects (Inventor 2026 help, To Create and Edit Sketch Patterns,
//! GUID-AAD3A699-FED8-4614-A31D-0A2316328CEE, Edit Rectangular and Circular Patterns): a pattern remembers
//! what it was made from and what it added, so it can be edited, deleted as a whole or have elements
//! suppressed. The document builds and rebuilds the geometry; the sketch keeps the record and the copies'
//! positions.
//!
//! Only one construction step per direction (a rectangular pattern: from the first original point, parallel
//! to the direction, carrying the spacing dimension) or one spoke (a circular pattern: as long as the first
//! original's, carrying the angle dimension) takes part in solving. The copies are held still in the solver
//! and placed after each solve by moving the originals along the steps or turning them about the axis, so
//! a pattern of thousands of elements costs no more to solve than one element (a pattern of 100 × 100 was
//! minutes when every copy was constrained to its neighbour).

use gc_core::Id;
use serde::{Deserialize, Serialize};

use crate::{Curve, CurveGeom, Item, SkPoint, Sketch};

/// One direction of a sketch rectangular pattern.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PatternDirection {
    /// A line giving the direction (from its start to its end).
    pub line: Id<Curve>,
    /// Number of elements along it, the originals included, 2 to 500.
    pub count: u32,
    /// Spacing between neighbours (or overall, fitted), an expression in millimetres by default.
    pub spacing: String,
    /// The other way along the line.
    pub flip: bool,
}

/// The values a pattern was made from (the dialog's).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PatternDefinition {
    /// A rectangular pattern along one or two directions.
    Rectangular {
        /// The curves patterned.
        curves: Vec<Id<Curve>>,
        /// Direction 1.
        first: PatternDirection,
        /// Direction 2.
        second: Option<PatternDirection>,
        /// Spacing is the overall extent.
        fitted: bool,
    },
    /// A circular pattern round a point.
    Circular {
        /// The curves patterned.
        curves: Vec<Id<Curve>>,
        /// The axis point.
        axis: Id<SkPoint>,
        /// Number of elements, the originals included.
        count: u32,
        /// Angle between neighbours (or overall, fitted), an expression in degrees by default.
        angle: String,
        /// Clockwise.
        flip: bool,
        /// The angle is the overall extent.
        fitted: bool,
    },
}

impl PatternDefinition {
    /// The curves patterned.
    pub fn curves(&self) -> &[Id<Curve>] {
        match self {
            PatternDefinition::Rectangular { curves, .. } | PatternDefinition::Circular { curves, .. } => {
                curves
            }
        }
    }
}

/// How a pattern's copies follow its originals.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum PatternStep {
    /// Rectangular: element (i, j) is the originals moved by i × (first − from) + j × (second − from).
    Translate {
        /// The first original point, where the steps start.
        from: Id<SkPoint>,
        /// End of the step along direction 1.
        first: Id<SkPoint>,
        /// End of the step along direction 2.
        second: Option<Id<SkPoint>>,
    },
    /// Circular: element (k, 0) is the originals turned about `axis` k times the angle from `from` to `to`.
    Rotate {
        /// The axis point.
        axis: Id<SkPoint>,
        /// An original point off the axis.
        from: Id<SkPoint>,
        /// Where the first turn takes it.
        to: Id<SkPoint>,
    },
}

/// One copied point.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PatternCopy {
    /// Its element (index into `elements`).
    pub element: usize,
    /// The original point.
    pub original: Id<SkPoint>,
    /// The copy.
    pub copy: Id<SkPoint>,
}

/// A sketch pattern.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SketchPattern {
    /// What it was made from.
    pub definition: PatternDefinition,
    /// The copies, element by element (the originals are not an element).
    pub elements: Vec<Vec<Id<Curve>>>,
    /// Everything it added: the copies, their points, construction, constraints and dimensions.
    pub items: Vec<Item>,
    /// Suppressed elements (indices into `elements`): their curves are construction.
    pub suppressed: Vec<usize>,
    /// How the copies follow.
    pub step: PatternStep,
    /// Where each element sits: (i, j) of a rectangular pattern, (k, 0) of a circular one.
    pub places: Vec<[u32; 2]>,
    /// The copied points.
    pub copies: Vec<PatternCopy>,
    /// Copied circles with their originals (`(original, copy)`): the copy keeps the radius.
    pub circles: Vec<(Id<Curve>, Id<Curve>)>,
}

impl SketchPattern {
    /// Where the original point at `p` goes in element `element`, given the step points' positions.
    fn place(&self, sketch: &Sketch, element: usize, p: [f64; 3]) -> Option<[f64; 3]> {
        let [i, j] = *self.places.get(element)?;
        let pos = |id: Id<SkPoint>| sketch.pos(id);
        match self.step {
            PatternStep::Translate { from, first, second } => {
                let o = pos(from)?;
                let a = pos(first)?;
                let b = match second {
                    Some(second) => pos(second)?,
                    None => o,
                };
                let (i, j) = (f64::from(i), f64::from(j));
                Some([
                    p[0] + i * (a[0] - o[0]) + j * (b[0] - o[0]),
                    p[1] + i * (a[1] - o[1]) + j * (b[1] - o[1]),
                    p[2] + i * (a[2] - o[2]) + j * (b[2] - o[2]),
                ])
            }
            PatternStep::Rotate { axis, from, to } => {
                let c = pos(axis)?;
                let (f, t) = (pos(from)?, pos(to)?);
                let turn = gc_math::fm::atan2(t[1] - c[1], t[0] - c[0])
                    - gc_math::fm::atan2(f[1] - c[1], f[0] - c[0]);
                let angle = turn * f64::from(i);
                let (sin, cos) = (gc_math::fm::sin(angle), gc_math::fm::cos(angle));
                let (x, y) = (p[0] - c[0], p[1] - c[1]);
                Some([c[0] + cos * x - sin * y, c[1] + sin * x + cos * y, p[2]])
            }
        }
    }
}

impl Sketch {
    /// The pattern `curve` belongs to, as an original or a copy.
    pub fn pattern_of(&self, curve: Id<Curve>) -> Option<usize> {
        self.patterns.iter().position(|p| {
            p.definition.curves().contains(&curve) || p.elements.iter().any(|e| e.contains(&curve))
        })
    }

    /// The pattern and element `curve` is a copy in.
    pub fn pattern_element_of(&self, curve: Id<Curve>) -> Option<(usize, usize)> {
        self.patterns
            .iter()
            .enumerate()
            .find_map(|(i, p)| p.elements.iter().position(|e| e.contains(&curve)).map(|e| (i, e)))
    }

    /// Suppresses (or brings back) elements of a pattern: suppressed curves turn construction (dashed, out
    /// of profiles), the relationships stay.
    pub fn suppress_pattern_elements(&mut self, pattern: usize, elements: &[usize], suppress: bool) {
        let Some(record) = self.patterns.get(pattern) else { return };
        // Brought back, a copy is construction again only if its original is.
        let originals: Vec<bool> = record
            .definition
            .curves()
            .iter()
            .map(|c| self.curves.get(c).is_some_and(|c| c.construction))
            .collect();
        let mut changes: Vec<(Id<Curve>, bool)> = Vec::new();
        let mut suppressed = record.suppressed.clone();
        for e in elements {
            let Some(element) = record.elements.get(*e) else { continue };
            if suppress == suppressed.contains(e) {
                continue;
            }
            if suppress {
                suppressed.push(*e);
            } else {
                suppressed.retain(|s| s != e);
            }
            let matched = element.len() == originals.len();
            for (k, id) in element.iter().enumerate() {
                let construction = suppress || (matched && originals[k]);
                changes.push((*id, construction));
            }
        }
        suppressed.sort_unstable();
        if let Some(record) = self.patterns.get_mut(pattern) {
            record.suppressed = suppressed;
        }
        for (id, construction) in changes {
            if let Some(curve) = self.curves.get_mut(&id) {
                curve.construction = construction;
            }
        }
    }

    /// Removes exactly `items` — unlike [`Sketch::delete`], the points of a removed curve stay unless they
    /// are listed (a pattern's construction shares the originals' points) — and what refers to them.
    pub fn remove_exactly(&mut self, items: &[Item]) {
        let mut points = std::collections::BTreeSet::new();
        let mut curves = std::collections::BTreeSet::new();
        for item in items {
            match *item {
                Item::Point(p) => {
                    points.insert(p);
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
        self.curves.retain(|id, _| !curves.contains(id));
        // A listed point still used by a curve that stays is kept.
        let used: std::collections::BTreeSet<Id<SkPoint>> =
            self.curves.values().flat_map(Curve::points).collect();
        points.retain(|p| !used.contains(p));
        self.points.retain(|id, _| !points.contains(id));
        self.constraints.retain(|_, c| {
            Self::constraint_points(c).iter().all(|p| !points.contains(p))
                && Self::constraint_curves(c).iter().all(|c| !curves.contains(c))
        });
        self.dimensions.retain(|_, d| {
            let (dim_points, dim_curves) = Self::dimension_refs(&d.kind);
            dim_points.iter().all(|p| !points.contains(p)) && dim_curves.iter().all(|c| !curves.contains(c))
        });
    }

    /// The copies the solver holds still: their points and copied circles.
    pub(crate) fn pattern_held(&self) -> (Vec<Id<SkPoint>>, Vec<Id<Curve>>) {
        let points = self.patterns.iter().flat_map(|p| p.copies.iter().map(|c| c.copy)).collect();
        let circles = self.patterns.iter().flat_map(|p| p.circles.iter().map(|(_, c)| *c)).collect();
        (points, circles)
    }

    /// Puts every copy where its pattern places it now. True when something moved.
    pub fn refresh_patterns(&mut self) -> bool {
        let mut moves: Vec<(Id<SkPoint>, [f64; 3])> = Vec::new();
        let mut radii: Vec<(Id<Curve>, f64)> = Vec::new();
        for pattern in &self.patterns {
            for copy in &pattern.copies {
                let (Some(original), Some(now)) = (self.pos(copy.original), self.pos(copy.copy)) else {
                    continue;
                };
                if let Some(at) = pattern.place(self, copy.element, original)
                    && at.iter().zip(now).any(|(a, b)| (a - b).abs() > 1e-13)
                {
                    moves.push((copy.copy, at));
                }
            }
            for (original, copy) in &pattern.circles {
                if let (Some(CurveGeom::Circle { r, .. }), Some(CurveGeom::Circle { r: now, .. })) = (
                    self.curves.get(original).map(|c| c.geom.clone()),
                    self.curves.get(copy).map(|c| c.geom.clone()),
                ) && (r - now).abs() > 1e-13
                {
                    radii.push((*copy, r));
                }
            }
        }
        let moved = !moves.is_empty() || !radii.is_empty();
        for (id, at) in moves {
            if let Some(point) = self.points.get_mut(&id) {
                point.pos = at;
            }
        }
        for (id, value) in radii {
            if let Some(Curve { geom: CurveGeom::Circle { r, .. }, .. }) = self.curves.get_mut(&id) {
                *r = value;
            }
        }
        moved
    }

    /// Drops what no longer exists from the patterns; a pattern whose originals are gone is forgotten (its
    /// copies stay as plain geometry).
    pub fn prune_patterns(&mut self) {
        let curves = &self.curves;
        let points = &self.points;
        let constraints = &self.constraints;
        let dimensions = &self.dimensions;
        let exists = |item: &Item| match item {
            Item::Point(p) => points.contains_key(p),
            Item::Curve(c) => curves.contains_key(c),
            Item::Constraint(c) => constraints.contains_key(c),
            Item::Dimension(d) => dimensions.contains_key(d),
        };
        self.patterns.retain_mut(|p| {
            p.items.retain(exists);
            for element in &mut p.elements {
                element.retain(|c| curves.contains_key(c));
            }
            p.copies.retain(|c| points.contains_key(&c.copy) && points.contains_key(&c.original));
            p.circles.retain(|(a, b)| curves.contains_key(a) && curves.contains_key(b));
            p.definition.curves().iter().all(|c| curves.contains_key(c))
        });
        // A pattern whose direction line or axis is gone can no longer be placed (Inventor: without the
        // associative relationship the geometry "is no longer a pattern, but geometric elements that you can
        // edit individually"): the record goes with its step construction and dimensions; the copies stay
        // as ordinary curves.
        let mut released = Vec::new();
        let curves = &self.curves;
        let points = &self.points;
        self.patterns.retain(|p| {
            let guided = match &p.definition {
                PatternDefinition::Rectangular { first, second, .. } => {
                    std::iter::once(first).chain(second).all(|d| curves.contains_key(&d.line))
                }
                PatternDefinition::Circular { axis, .. } => points.contains_key(axis),
            };
            if !guided {
                released.push(p.clone());
            }
            guided
        });
        for pattern in released {
            let kept_curves: std::collections::BTreeSet<Id<Curve>> =
                pattern.elements.iter().flatten().copied().collect();
            let kept_points: std::collections::BTreeSet<Id<SkPoint>> = pattern
                .copies
                .iter()
                .map(|c| c.copy)
                .chain(kept_curves.iter().filter_map(|c| self.curves.get(c)).flat_map(Curve::points))
                .collect();
            let scaffold: Vec<Item> = pattern
                .items
                .iter()
                .copied()
                .filter(|item| match item {
                    Item::Curve(c) => !kept_curves.contains(c),
                    Item::Point(p) => !kept_points.contains(p),
                    Item::Constraint(_) | Item::Dimension(_) => true,
                })
                .collect();
            self.remove_exactly(&scaffold);
        }
    }
}
