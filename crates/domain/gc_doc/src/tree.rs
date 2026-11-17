//! Model-tree commands and questions that apply to items of any kind: renaming, showing and hiding,
//! what depends on an item, where the rollback marker stands.

use gc_core::{AnyId, Id, Kind};

use crate::cell::{Body, MotionPath, Robot, Waypoint, WorkFrame};
use crate::command::{Created, EditError};
use crate::design::{Assembly, Feature, WorkFeature};
use crate::document::Document;
use crate::txn::Txn;
use gc_sketch::Sketch;

type Outcome = Result<(Vec<Created>, &'static str), EditError>;

/// Renames an item, refusing an empty name or one another item of the same kind has.
pub(crate) fn rename(txn: &mut Txn<'_>, item: AnyId, name: &str) -> Outcome {
    let name = name.trim().to_owned();
    if name.is_empty() {
        return Err(EditError::BadValue);
    }
    let doc = txn.doc();
    macro_rules! rename_in {
        ($store:ident, $ty:ty) => {{
            if doc.$store().iter().any(|(id, e)| id.erase() != item && e.name == name) {
                return Err(EditError::NameTaken);
            }
            txn.modify(Id::<$ty>::from_raw(item.raw), |e: &mut $ty, _| e.name = name)?;
        }};
    }
    match item.kind {
        Kind::Sketch => rename_in!(sketches, Sketch),
        Kind::Body => rename_in!(bodies, Body),
        Kind::RobotInstance => rename_in!(robots, Robot),
        Kind::Waypoint => rename_in!(waypoints, Waypoint),
        Kind::Path => rename_in!(paths, MotionPath),
        Kind::WorkFrame => rename_in!(work_frames, WorkFrame),
        Kind::WorkFeature => rename_in!(work_features, WorkFeature),
        Kind::Feature => rename_in!(features, Feature),
        Kind::Assembly => rename_in!(assemblies, Assembly),
        // Parameters are referred to by name in expressions; origin frames are fixed.
        _ => return Err(EditError::BadValue),
    }
    Ok((Vec::new(), "Rename"))
}

/// Shows or hides an item: in its look for bodies, robots and assemblies, in the tree state otherwise.
pub(crate) fn set_visible(txn: &mut Txn<'_>, item: AnyId, visible: bool) -> Outcome {
    if !txn.doc().contains(item) {
        return Err(EditError::BadValue);
    }
    match item.kind {
        Kind::Body => {
            txn.modify(Id::<Body>::from_raw(item.raw), |b: &mut Body, _| b.look.hidden = !visible)?
        }
        Kind::RobotInstance => {
            txn.modify(Id::<Robot>::from_raw(item.raw), |r: &mut Robot, _| r.look.hidden = !visible)?
        }
        Kind::Assembly => {
            txn.modify(Id::<Assembly>::from_raw(item.raw), |a: &mut Assembly, _| a.look.hidden = !visible)?;
        }
        Kind::Sketch | Kind::Waypoint | Kind::Path | Kind::WorkFrame | Kind::WorkFeature | Kind::Feature => {
            crate::cell::set_timeline(txn, |t| {
                t.hidden.retain(|h| *h != item);
                if !visible {
                    t.hidden.push(item);
                }
            })?;
        }
        _ => return Err(EditError::BadValue),
    }
    Ok((Vec::new(), if visible { "Show" } else { "Hide" }))
}

impl Document {
    /// True when the item is shown (whatever stores its visibility).
    pub fn is_visible(&self, item: AnyId) -> bool {
        match item.kind {
            Kind::Body => self.get(Id::<Body>::from_raw(item.raw)).is_some_and(|b| !b.look.hidden),
            Kind::RobotInstance => self.get(Id::<Robot>::from_raw(item.raw)).is_some_and(|r| !r.look.hidden),
            Kind::Assembly => self.get(Id::<Assembly>::from_raw(item.raw)).is_some_and(|a| !a.look.hidden),
            _ => self.contains(item) && !self.timeline().is_some_and(|(_, t)| t.hidden.contains(&item)),
        }
    }

    /// Everything that depends on the item, directly or through other items, in a stable order (direct
    /// users first): what deleting it would need deleting first.
    pub fn dependents_of(&self, item: AnyId) -> Vec<AnyId> {
        let references = self.references();
        let mut found: Vec<AnyId> = Vec::new();
        let mut frontier = vec![item];
        while let Some(used) = frontier.first().copied() {
            frontier.remove(0);
            let mut users: Vec<AnyId> = references
                .iter()
                .filter(|(user, to)| *to == used && *user != item && !found.contains(user))
                .map(|(user, _)| *user)
                .collect();
            users.sort();
            users.dedup();
            found.extend(&users);
            frontier.extend(users);
        }
        found
    }

    /// Where the rollback marker stands in [`Document::model_order`]: `Some(i)` when the items after
    /// position `i` are rolled back, `None` when it is at the end.
    pub fn rollback_index(&self) -> Option<usize> {
        let marker = self.timeline()?.1.rollback_after?;
        self.model_order().iter().position(|id| *id == marker)
    }
}
