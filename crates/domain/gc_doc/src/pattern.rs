//! Component patterns (docs/ux/inventor-assembly-parity.md, Pattern): copies of components arranged in
//! columns and rows or round an axis, that follow the parent components.
//!
//! Inventor 2026 help: Create rectangular patterns of components (GUID-CF627358-120F-4142-A9D0-13217BD3432D),
//! Create circular patterns of components (GUID-FF7B08E3-624B-4382-B4E5-304C0354C048): direction from an
//! origin axis, edge, work axis or face (a planar face's normal, a revolved face's axis), Flip, count and
//! spacing; circular: Rotational or Fixed orientation, Incremental or Fitted positioning, Midplane.

use gc_core::{AnyId, Id, Kind};
use gc_math::{DPose3, DQuat, DVec3};
use serde::{Deserialize, Serialize};

use crate::cell::{Body, Placement, Robot};
use crate::command::{Created, EditError};
use crate::design::{Assembly, Timeline};
use crate::relate::Selection;
use crate::txn::Txn;

/// One direction of a rectangular pattern.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PatternDirection {
    /// What gives the direction (an origin axis, an edge, a work axis, a face).
    pub along: Selection,
    /// Reverse the direction.
    pub flip: bool,
    /// How many occurrences, the parent included.
    pub count: u32,
    /// Distance between occurrences, metres.
    pub spacing: f64,
}

/// How the copies are arranged.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum PatternLayout {
    /// Columns, and optionally rows.
    Rectangular {
        /// The column direction.
        column: PatternDirection,
        /// The row direction.
        row: Option<PatternDirection>,
    },
    /// Round an axis.
    Circular {
        /// The rotation axis (an edge, work axis, cylindrical face or origin axis).
        axis: Selection,
        /// Reverse the axis.
        flip: bool,
        /// How many occurrences, the parent included.
        count: u32,
        /// Incremental: the angle between occurrences; fitted: the angle they cover together (radians).
        angle: f64,
        /// Fitted positioning instead of incremental.
        fitted: bool,
        /// The copies turn as they go round (Rotational); otherwise they keep the parent's orientation
        /// (Fixed).
        rotational: bool,
        /// Spread both ways from the parent.
        midplane: bool,
    },
}

/// A component pattern: its parent components and, per occurrence after the parent, the copies.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComponentPattern {
    /// Name shown in the browser.
    pub name: String,
    /// The components patterned (bodies, robots, assemblies).
    pub parents: Vec<AnyId>,
    /// The arrangement.
    pub layout: PatternLayout,
    /// Per occurrence (1, 2, … in layout order; the parent is 0): the copies, one per parent.
    pub elements: Vec<PatternElement>,
    /// Occurrences made independent: their copies left the pattern and the place is not filled again.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub independent: Vec<u32>,
}

/// An occurrence of a pattern.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PatternElement {
    /// Its place in the layout (column + columns × row for rectangular patterns).
    pub index: u32,
    /// The copies of the parents.
    pub items: Vec<AnyId>,
}

impl PatternLayout {
    fn is_valid(&self) -> bool {
        let direction = |d: &PatternDirection| d.count >= 1 && d.spacing.is_finite();
        match self {
            PatternLayout::Rectangular { column, row } => {
                direction(column)
                    && row.as_ref().is_none_or(direction)
                    && self.count() >= 2
                    && self.count() <= 1000
            }
            PatternLayout::Circular { count, angle, .. } => (2..=1000).contains(count) && angle.is_finite(),
        }
    }

    /// Occurrences, the parent included.
    pub fn count(&self) -> u32 {
        match self {
            PatternLayout::Rectangular { column, row } => {
                column.count.saturating_mul(row.map_or(1, |r| r.count))
            }
            PatternLayout::Circular { count, .. } => *count,
        }
    }
}

impl crate::document::Document {
    /// The world transform from the parent to occurrence `index` of a layout; `None` when a direction or
    /// axis no longer resolves.
    fn pattern_transform(&self, layout: &PatternLayout, index: u32) -> Option<DPose3> {
        let direction = |d: &PatternDirection| -> Option<DVec3> {
            let (_, v) = self.selection_axis(&d.along)?;
            Some(if d.flip { -v } else { v })
        };
        match layout {
            PatternLayout::Rectangular { column, row } => {
                let (c, r) = (index % column.count.max(1), index / column.count.max(1));
                let mut offset = direction(column)? * (f64::from(c) * column.spacing);
                if let Some(row) = row {
                    offset += direction(row)? * (f64::from(r) * row.spacing);
                }
                Some(DPose3::from_parts(offset, DQuat::IDENTITY))
            }
            PatternLayout::Circular { axis, flip, count, angle, fitted, midplane, .. } => {
                let (point, mut z) = self.selection_axis(axis)?;
                if *flip {
                    z = -z;
                }
                let full = (angle.abs() - std::f64::consts::TAU).abs() < 1e-9;
                let step = match (fitted, full) {
                    (false, _) => *angle,
                    (true, true) => angle / f64::from(*count),
                    (true, false) => angle / f64::from(count.saturating_sub(1).max(1)),
                };
                let k = if *midplane {
                    // Alternating sides: 1 → +1, 2 → −1, 3 → +2, …
                    let half = f64::from(index.div_ceil(2));
                    if index % 2 == 1 { half } else { -half }
                } else {
                    f64::from(index)
                };
                let turn = DQuat::from_axis_angle(z, step * k);
                // Rotation about the axis line through `point`.
                Some(DPose3::from_parts(point - turn * point, turn))
            }
        }
    }

    /// Where a pattern copy stands: the parent moved by the occurrence's transform (Fixed circular patterns
    /// move it round the axis without turning it).
    fn copy_pose(&self, layout: &PatternLayout, index: u32, parent: AnyId) -> Option<DPose3> {
        let parent_pose = self.owner_pose(parent)?;
        let transform = self.pattern_transform(layout, index)?;
        let moved = transform * parent_pose;
        Some(match layout {
            PatternLayout::Circular { rotational: false, .. } => {
                DPose3::from_parts(moved.translation, parent_pose.rotation)
            }
            _ => moved,
        })
    }

    /// The component patterns.
    pub fn patterns(&self) -> Vec<ComponentPattern> {
        self.timeline().map(|(_, t)| t.patterns.clone()).unwrap_or_default()
    }
}

fn placement_of(pose: &DPose3) -> Placement {
    Placement { xyz: pose.translation.to_array(), rpy: gc_math::rpy_from_quat(pose.rotation) }
}

/// A copy of a component, named after it, at a pose.
fn copy_component(txn: &mut Txn<'_>, item: AnyId, pose: &DPose3, suffix: u32) -> Result<AnyId, EditError> {
    let placement = placement_of(pose);
    Ok(match item.kind {
        Kind::Body => {
            let mut body = txn.doc().get(Id::<Body>::from_raw(item.raw)).ok_or(EditError::BadValue)?.clone();
            body.name = format!("{}:{}", body.name, suffix + 1);
            body.placement = placement;
            txn.insert(body).erase()
        }
        Kind::RobotInstance => {
            let mut robot =
                txn.doc().get(Id::<Robot>::from_raw(item.raw)).ok_or(EditError::BadValue)?.clone();
            robot.name = format!("{}:{}", robot.name, suffix + 1);
            robot.placement = placement;
            txn.insert(robot).erase()
        }
        Kind::Assembly => {
            let mut assembly =
                txn.doc().get(Id::<Assembly>::from_raw(item.raw)).ok_or(EditError::BadValue)?.clone();
            assembly.name = format!("{}:{}", assembly.name, suffix + 1);
            assembly.placement = placement;
            txn.insert(assembly).erase()
        }
        _ => return Err(EditError::BadValue),
    })
}

fn remove_component(txn: &mut Txn<'_>, item: AnyId) -> Result<(), EditError> {
    match item.kind {
        Kind::Body => drop(txn.remove(Id::<Body>::from_raw(item.raw))?),
        Kind::RobotInstance => drop(txn.remove(Id::<Robot>::from_raw(item.raw))?),
        Kind::Assembly => drop(txn.remove(Id::<Assembly>::from_raw(item.raw))?),
        _ => {}
    }
    Ok(())
}

/// Brings a pattern's copies to its layout: adds missing occurrences, removes extra ones.
fn fill(
    txn: &mut Txn<'_>,
    pattern: &mut ComponentPattern,
    created: &mut Vec<Created>,
) -> Result<(), EditError> {
    let count = pattern.layout.count();
    let (keep, extra): (Vec<PatternElement>, Vec<PatternElement>) =
        pattern.elements.drain(..).partition(|e| e.index >= 1 && e.index < count);
    for element in extra {
        for item in element.items {
            remove_component(txn, item)?;
        }
    }
    pattern.elements = keep;
    for index in 1..count {
        if pattern.elements.iter().any(|e| e.index == index) || pattern.independent.contains(&index) {
            continue;
        }
        let mut items = Vec::new();
        for parent in pattern.parents.clone() {
            let pose = txn.doc().copy_pose(&pattern.layout, index, parent).ok_or(EditError::BadValue)?;
            let copy = copy_component(txn, parent, &pose, index)?;
            created.push(match copy.kind {
                Kind::Body => Created::Body(Id::from_raw(copy.raw)),
                Kind::RobotInstance => Created::Robot(Id::from_raw(copy.raw)),
                _ => Created::Assembly(Id::from_raw(copy.raw)),
            });
            items.push(copy);
        }
        pattern.elements.push(PatternElement { index, items });
    }
    pattern.elements.sort_by_key(|e| e.index);
    Ok(())
}

fn write_patterns(txn: &mut Txn<'_>, patterns: Vec<ComponentPattern>) -> Result<(), EditError> {
    crate::cell::set_timeline(txn, |t: &mut Timeline| t.patterns = patterns)
}

/// Adds a pattern of components: copies placed by the layout.
pub(crate) fn add(
    txn: &mut Txn<'_>,
    name: &str,
    parents: &[AnyId],
    layout: &PatternLayout,
) -> Result<(Vec<Created>, &'static str), EditError> {
    if parents.is_empty() || !layout.is_valid() || !parents.iter().all(|p| txn.doc().owner_pose(*p).is_some())
    {
        return Err(EditError::BadValue);
    }
    let mut pattern = ComponentPattern {
        name: name.to_owned(),
        parents: parents.to_vec(),
        layout: *layout,
        elements: Vec::new(),
        independent: Vec::new(),
    };
    let mut created = Vec::new();
    fill(txn, &mut pattern, &mut created)?;
    let mut patterns = txn.doc().patterns();
    patterns.push(pattern);
    write_patterns(txn, patterns)?;
    Ok((created, "Pattern components"))
}

/// Changes a pattern's layout (count, spacing, direction): occurrences are added or removed.
pub(crate) fn set(
    txn: &mut Txn<'_>,
    index: usize,
    layout: &PatternLayout,
) -> Result<(Vec<Created>, &'static str), EditError> {
    if !layout.is_valid() {
        return Err(EditError::BadValue);
    }
    let mut patterns = txn.doc().patterns();
    let mut pattern = patterns.get(index).cloned().ok_or(EditError::BadValue)?;
    pattern.layout = *layout;
    let mut created = Vec::new();
    fill(txn, &mut pattern, &mut created)?;
    patterns[index] = pattern;
    write_patterns(txn, patterns)?;
    Ok((created, "Edit pattern"))
}

/// Deletes a pattern: its copies go too, unless `keep_copies` (they become ordinary components).
pub(crate) fn delete(
    txn: &mut Txn<'_>,
    index: usize,
    keep_copies: bool,
) -> Result<(Vec<Created>, &'static str), EditError> {
    let mut patterns = txn.doc().patterns();
    if index >= patterns.len() {
        return Err(EditError::BadValue);
    }
    let pattern = patterns.remove(index);
    if !keep_copies {
        for element in pattern.elements {
            for item in element.items {
                remove_component(txn, item)?;
            }
        }
    }
    write_patterns(txn, patterns)?;
    Ok((Vec::new(), "Delete pattern"))
}

/// Makes an occurrence independent: its copies leave the pattern and stay where they are.
pub(crate) fn make_independent(
    txn: &mut Txn<'_>,
    index: usize,
    element: u32,
) -> Result<(Vec<Created>, &'static str), EditError> {
    let mut patterns = txn.doc().patterns();
    let pattern = patterns.get_mut(index).ok_or(EditError::BadValue)?;
    let before = pattern.elements.len();
    pattern.elements.retain(|e| e.index != element);
    if pattern.elements.len() == before {
        return Err(EditError::BadValue);
    }
    // The layout keeps its count: the occurrence is simply not filled again.
    pattern.independent.push(element);
    write_patterns(txn, patterns)?;
    Ok((Vec::new(), "Make independent"))
}

/// Moves every pattern copy to where its parent and the layout put it; patterns whose parents are gone
/// lose their association (the copies stay as they are). Run after every command.
pub(crate) fn refresh(txn: &mut Txn<'_>) -> Result<(), EditError> {
    let patterns = txn.doc().patterns();
    if patterns.is_empty() {
        return Ok(());
    }
    let mut kept = Vec::with_capacity(patterns.len());
    let mut changed = false;
    for mut pattern in patterns {
        if !pattern.parents.iter().all(|p| txn.doc().owner_pose(*p).is_some()) {
            changed = true;
            continue;
        }
        let elements_before = pattern.elements.len();
        pattern.elements.retain(|e| e.items.iter().all(|i| txn.doc().owner_pose(*i).is_some()));
        changed |= pattern.elements.len() != elements_before;
        for element in &pattern.elements {
            for (parent, item) in pattern.parents.iter().zip(&element.items) {
                let Some(pose) = txn.doc().copy_pose(&pattern.layout, element.index, *parent) else {
                    continue;
                };
                let now = txn.doc().owner_pose(*item);
                let moved = now.is_none_or(|n| {
                    (n.translation - pose.translation).length() > 1e-12
                        || n.rotation.angle_between(pose.rotation) > 1e-12
                });
                if moved {
                    crate::cell::set_owner_pose(txn, *item, pose)?;
                }
            }
        }
        kept.push(pattern);
    }
    if changed {
        write_patterns(txn, kept)?;
    }
    Ok(())
}
