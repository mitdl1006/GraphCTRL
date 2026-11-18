//! Model-tree commands and questions that apply to items of any kind: renaming, showing and hiding,
//! what depends on an item, where the rollback marker stands.

use gc_core::{AnyId, Id, Kind};

use crate::cell::{Body, MotionPath, Robot, Waypoint, WorkFrame};
use crate::command::{Created, EditError};
use crate::design::{Assembly, ComponentFlag, Feature, Occurrence, OriginElement, WorkFeature};
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

/// A component of the assembly tree with its states (docs/ux/inventor-assembly-parity.md, P1-1).
#[derive(Clone, Debug, PartialEq)]
pub struct ComponentNode {
    /// The occurrence.
    pub occurrence: Occurrence,
    /// Its parent (`None` for a component placed in the cell).
    pub parent: Option<Occurrence>,
    /// Name shown in the tree.
    pub name: String,
    /// Fixed in space.
    pub grounded: bool,
    /// Drawn (its own flag; a part of a hidden parent is not drawn either).
    pub visible: bool,
    /// Selectable.
    pub enabled: bool,
    /// Kept but not computed (its own flag).
    pub suppressed: bool,
}

fn listed(list: &[Occurrence], occurrence: Occurrence) -> bool {
    list.contains(&occurrence)
}

impl Document {
    fn components_state(&self) -> crate::design::Components {
        self.timeline().map(|(_, t)| t.components.clone()).unwrap_or_default()
    }

    /// True when the occurrence exists.
    pub fn has_occurrence(&self, occurrence: Occurrence) -> bool {
        match occurrence {
            Occurrence::Item(item) => {
                matches!(item.kind, Kind::Body | Kind::RobotInstance | Kind::Assembly) && self.contains(item)
            }
            Occurrence::Part { assembly, index } => {
                self.get(assembly).is_some_and(|a| (index as usize) < a.parts.len())
            }
            Occurrence::Link { robot, index } => {
                self.get(robot).is_some_and(|r| (index as usize) < r.links.len())
            }
            Occurrence::Tool { robot } => self.get(robot).is_some_and(|r| r.tool.is_some()),
        }
    }

    /// A state of a component as it is stored (not inherited from its parents).
    pub fn component_flag(&self, occurrence: Occurrence, flag: ComponentFlag) -> bool {
        let state = self.components_state();
        match (flag, occurrence) {
            (ComponentFlag::Grounded, _) => listed(&state.grounded, occurrence),
            (ComponentFlag::Enabled, _) => !listed(&state.disabled, occurrence),
            (ComponentFlag::Visible, Occurrence::Item(item)) => self.is_visible(item),
            (
                ComponentFlag::Visible,
                Occurrence::Part { .. } | Occurrence::Link { .. } | Occurrence::Tool { .. },
            ) => !listed(&state.hidden, occurrence),
            (ComponentFlag::Suppressed, Occurrence::Item(item)) if item.kind == Kind::Assembly => {
                self.timeline().is_some_and(|(_, t)| t.suppressed.contains(&item))
            }
            (ComponentFlag::Suppressed, Occurrence::Item(_)) => listed(&state.suppressed, occurrence),
            (ComponentFlag::Suppressed, Occurrence::Part { .. }) => listed(&state.suppressed, occurrence),
            (ComponentFlag::Suppressed, Occurrence::Link { .. } | Occurrence::Tool { .. }) => false,
        }
    }

    /// The parts of an assembly that are drawn (`for_obstacles`: that count as obstacles, where hidden
    /// parts still do), as flags per part index. A part of a hidden or suppressed parent is left out too.
    pub fn parts_shown(&self, assembly: Id<Assembly>, for_obstacles: bool) -> Vec<bool> {
        let Some(a) = self.get(assembly) else { return Vec::new() };
        let state = self.components_state();
        let mut shown: Vec<bool> = Vec::with_capacity(a.parts.len());
        for (index, part) in a.parts.iter().enumerate() {
            #[allow(clippy::cast_possible_truncation, reason = "part counts fit in u32 (checked on import)")]
            let occurrence = Occurrence::Part { assembly, index: index as u32 };
            let own = !listed(&state.suppressed, occurrence)
                && (for_obstacles || !listed(&state.hidden, occurrence));
            let parent = part.parent.is_none_or(|p| shown.get(p as usize).copied().unwrap_or(false));
            shown.push(own && parent);
        }
        shown
    }

    /// The assembly tree: bodies, robots and imported assemblies in model order, each followed by its
    /// parts (parents before children).
    pub fn components(&self) -> Vec<ComponentNode> {
        let mut items: Vec<AnyId> = self.bodies().iter().map(|(id, _)| id.erase()).collect();
        items.extend(self.robots().iter().map(|(id, _)| id.erase()));
        let assemblies: Vec<AnyId> =
            self.model_order().into_iter().filter(|i| i.kind == Kind::Assembly).collect();
        items.extend(assemblies);
        let mut out = Vec::new();
        for item in items {
            let name = match item.kind {
                Kind::Body => self.get(Id::<Body>::from_raw(item.raw)).map(|b| b.name.clone()),
                Kind::RobotInstance => self.get(Id::<Robot>::from_raw(item.raw)).map(|r| r.name.clone()),
                _ => self.get(Id::<Assembly>::from_raw(item.raw)).map(|a| a.name.clone()),
            };
            let Some(name) = name else { continue };
            out.push(self.node(Occurrence::Item(item), None, name));
            if item.kind == Kind::Assembly {
                let id = Id::<Assembly>::from_raw(item.raw);
                let parts = self.get(id).map(|a| a.parts.clone()).unwrap_or_default();
                for (index, part) in parts.iter().enumerate() {
                    #[allow(clippy::cast_possible_truncation, reason = "part counts fit in u32")]
                    let occurrence = Occurrence::Part { assembly: id, index: index as u32 };
                    let parent = match part.parent {
                        Some(p) => Occurrence::Part { assembly: id, index: p },
                        None => Occurrence::Item(item),
                    };
                    out.push(self.node(occurrence, Some(parent), part.name.clone()));
                }
            }
        }
        out
    }

    /// A robot's links as sub-components, root first, each under its parent link (the root under the
    /// robot), then its tool (if it carries one) under the tip link. Empty when the robot's link tree is
    /// not known. Not part of [`Document::components`], which
    /// lists what was placed in the cell.
    pub fn robot_links(&self, robot: Id<Robot>) -> Vec<ComponentNode> {
        let links = self.get(robot).map(|r| r.links.clone()).unwrap_or_default();
        links
            .iter()
            .enumerate()
            .map(|(index, link)| {
                #[allow(clippy::cast_possible_truncation, reason = "link counts fit in u32")]
                let occurrence = Occurrence::Link { robot, index: index as u32 };
                let parent = match link.parent {
                    Some(p) => Occurrence::Link { robot, index: p },
                    None => Occurrence::Item(robot.erase()),
                };
                self.node(occurrence, Some(parent), link.name.clone())
            })
            .chain(self.tool_node(robot))
            .collect()
    }

    /// The robot's tool as a sub-component under the link it is mounted on (the tip link).
    fn tool_node(&self, robot: Id<Robot>) -> Option<ComponentNode> {
        let r = self.get(robot)?;
        r.tool.as_ref()?;
        let parent = r
            .links
            .iter()
            .position(|l| l.name == r.tip_link)
            .and_then(|i| u32::try_from(i).ok())
            .map_or(Occurrence::Item(robot.erase()), |index| Occurrence::Link { robot, index });
        Some(self.node(Occurrence::Tool { robot }, Some(parent), "Tool".to_owned()))
    }

    fn node(&self, occurrence: Occurrence, parent: Option<Occurrence>, name: String) -> ComponentNode {
        ComponentNode {
            occurrence,
            parent,
            name,
            grounded: self.component_flag(occurrence, ComponentFlag::Grounded),
            visible: self.component_flag(occurrence, ComponentFlag::Visible),
            enabled: self.component_flag(occurrence, ComponentFlag::Enabled),
            suppressed: self.component_flag(occurrence, ComponentFlag::Suppressed),
        }
    }

    /// True when an element of a component's origin folder is shown (all are hidden by default).
    pub fn origin_visible(&self, occurrence: Occurrence, element: OriginElement) -> bool {
        self.has_occurrence(occurrence)
            && self.components_state().origins_shown.contains(&(occurrence, element))
    }

    /// The contact set.
    pub fn contact_set(&self) -> Vec<Occurrence> {
        self.components_state().contact_set
    }

    /// The Interactive Contact setting.
    pub fn contact_solver(&self) -> crate::design::ContactSolver {
        self.components_state().contact_solver
    }

    /// True when a component (body, robot or assembly) is in the contact set.
    pub fn in_contact_set(&self, item: AnyId) -> bool {
        self.components_state().contact_set.contains(&Occurrence::Item(item))
    }

    /// The components the contact solver analyses: none when it is off; the contact set, or all
    /// components, otherwise. Hidden, disabled and suppressed components take no part (Inventor 2026 help,
    /// Detect contact between assembly components, GUID-FA1A8282-3A7E-45BE-8DB3-F637FB1C3292).
    pub fn contact_participants(&self) -> Vec<AnyId> {
        use crate::design::ContactSolver;
        let state = self.components_state();
        let all: Vec<AnyId> = self
            .bodies()
            .iter()
            .map(|(id, _)| id.erase())
            .chain(self.robots().iter().map(|(id, _)| id.erase()))
            .chain(self.assemblies().iter().map(|(id, _)| id.erase()))
            .collect();
        let candidates: Vec<AnyId> = match state.contact_solver {
            ContactSolver::Off => return Vec::new(),
            ContactSolver::ContactSet => {
                all.into_iter().filter(|i| state.contact_set.contains(&Occurrence::Item(*i))).collect()
            }
            ContactSolver::All => all,
        };
        candidates
            .into_iter()
            .filter(|i| {
                let o = Occurrence::Item(*i);
                self.component_flag(o, ComponentFlag::Visible)
                    && self.component_flag(o, ComponentFlag::Enabled)
                    && !self.is_component_suppressed(*i)
            })
            .collect()
    }

    /// True when a body, robot or assembly is suppressed (kept but not computed).
    pub fn is_component_suppressed(&self, item: AnyId) -> bool {
        self.component_flag(Occurrence::Item(item), ComponentFlag::Suppressed)
    }

    /// True when the document has no component yet (the first one placed is grounded).
    pub(crate) fn has_no_components(&self) -> bool {
        self.bodies().is_empty() && self.robots().is_empty() && self.assemblies().is_empty()
    }
}

/// Switches a state of a component.
pub(crate) fn set_component(
    txn: &mut Txn<'_>,
    occurrence: Occurrence,
    flag: ComponentFlag,
    on: bool,
) -> Outcome {
    // A robot's link or tool moves with the robot: it is not grounded on its own.
    if !txn.doc().has_occurrence(occurrence)
        || (flag == ComponentFlag::Grounded
            && matches!(occurrence, Occurrence::Link { .. } | Occurrence::Tool { .. }))
    {
        return Err(EditError::BadValue);
    }
    let toggle = |list: &mut Vec<Occurrence>, present: bool| {
        list.retain(|o| *o != occurrence);
        if present {
            list.push(occurrence);
        }
    };
    let label = match (flag, occurrence) {
        (ComponentFlag::Visible, Occurrence::Item(item)) => return set_visible(txn, item, on),
        (ComponentFlag::Suppressed, Occurrence::Item(item)) if item.kind != Kind::Assembly => {
            // Bodies and robots: kept but neither drawn nor an obstacle nor planned for.
            crate::cell::set_timeline(txn, |t| toggle(&mut t.components.suppressed, on))?;
            if on { "Suppress" } else { "Unsuppress" }
        }
        (ComponentFlag::Suppressed, Occurrence::Item(item)) => {
            crate::cell::set_timeline(txn, |t| {
                t.suppressed.retain(|s| *s != item);
                if on {
                    t.suppressed.push(item);
                }
            })?;
            if on { "Suppress" } else { "Unsuppress" }
        }
        (ComponentFlag::Grounded, _) => {
            crate::cell::set_timeline(txn, |t| toggle(&mut t.components.grounded, on))?;
            if on { "Ground" } else { "Unground" }
        }
        (ComponentFlag::Enabled, _) => {
            crate::cell::set_timeline(txn, |t| toggle(&mut t.components.disabled, !on))?;
            if on { "Enable" } else { "Disable" }
        }
        (ComponentFlag::Suppressed, Occurrence::Link { .. } | Occurrence::Tool { .. }) => {
            return Err(EditError::BadValue);
        }
        (
            ComponentFlag::Visible,
            Occurrence::Part { .. } | Occurrence::Link { .. } | Occurrence::Tool { .. },
        ) => {
            crate::cell::set_timeline(txn, |t| toggle(&mut t.components.hidden, !on))?;
            if on { "Show" } else { "Hide" }
        }
        (ComponentFlag::Suppressed, Occurrence::Part { .. }) => {
            crate::cell::set_timeline(txn, |t| toggle(&mut t.components.suppressed, on))?;
            if on { "Suppress" } else { "Unsuppress" }
        }
    };
    Ok((Vec::new(), label))
}

/// Drops the stored states (grounded, hidden, contact set, …) of components that no longer exist, so that
/// a deleted component leaves no stale entry behind (undo brings both back). Run after every command.
pub(crate) fn prune_states(txn: &mut Txn<'_>) -> Result<(), EditError> {
    let Some((_, timeline)) = txn.doc().timeline() else { return Ok(()) };
    let c = &timeline.components;
    let stale = c
        .grounded
        .iter()
        .chain(&c.disabled)
        .chain(&c.hidden)
        .chain(&c.suppressed)
        .chain(&c.contact_set)
        .chain(&c.dof_shown)
        .copied()
        .chain(c.origins_shown.iter().map(|(o, _)| *o))
        .any(|o| !txn.doc().has_occurrence(o))
        || c.joint_conditions.keys().any(|id| txn.doc().get(*id).is_none());
    if stale {
        // Writing the timeline drops them.
        crate::cell::set_timeline(txn, |_| {})?;
    }
    Ok(())
}

/// Grounds a component just placed when it is the first one (Inventor: the first component placed is
/// grounded, aligned with the assembly's origin).
pub(crate) fn ground_if_first(txn: &mut Txn<'_>, first: bool, item: AnyId) -> Result<(), EditError> {
    if first {
        crate::cell::set_timeline(txn, |t| t.components.grounded.push(Occurrence::Item(item)))?;
    }
    Ok(())
}

/// Shows or hides elements of a component's origin folder (several at once: one undo step).
pub(crate) fn set_origin_visible(
    txn: &mut Txn<'_>,
    occurrence: Occurrence,
    elements: &[OriginElement],
    visible: bool,
) -> Outcome {
    if !txn.doc().has_occurrence(occurrence)
        || elements.iter().any(|e| matches!(e, OriginElement::Axis(a) if *a > 2))
    {
        return Err(EditError::BadValue);
    }
    crate::cell::set_timeline(txn, |t| {
        let shown = &mut t.components.origins_shown;
        if visible {
            // Each element once, in the order first shown: showing it again changes nothing.
            for element in elements {
                if !shown.contains(&(occurrence, *element)) {
                    shown.push((occurrence, *element));
                }
            }
        } else {
            shown.retain(|(o, e)| !(*o == occurrence && elements.contains(e)));
        }
    })?;
    Ok((Vec::new(), if visible { "Show origin" } else { "Hide origin" }))
}
