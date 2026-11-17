//! Sketch patterns kept as objects (Inventor 2026 help, To Create and Edit Sketch Patterns,
//! GUID-AAD3A699-FED8-4614-A31D-0A2316328CEE, Edit Rectangular and Circular Patterns): a pattern remembers
//! what it was made from and what it added, so it can be edited, deleted as a whole or have elements
//! suppressed. The document builds and rebuilds the geometry; the sketch only keeps the record.

use gc_core::Id;
use serde::{Deserialize, Serialize};

use crate::{Curve, Item, SkPoint, Sketch};

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
            p.definition.curves().iter().all(|c| curves.contains_key(c))
        });
    }
}
