//! Representations (docs/ux/inventor-assembly-parity.md, P2-20): saved views (what is shown) and saved
//! positions (relationship values, suppression and component placement overriding the Primary one).
//!
//! Inventor 2026 help, To Create, Edit, or Copy Positional Representations
//! (GUID-34C28C35-45CA-4476-9336-30146F7C9471): an assembly has a positional representation named
//! [Primary], the default state, which new ones copy; overrides to relationships and components; one is
//! active at a time. View representations save visibility and display state (spec A62, A64).

use gc_core::{AnyId, Id};
use serde::{Deserialize, Serialize};

use crate::cell::Placement;
use crate::command::{Created, EditError};
use crate::design::{ComponentFlag, Occurrence};
use crate::relate::Relationship;
use crate::txn::Txn;

/// A saved view: which components are hidden or not enabled, their transparency, and display state the
/// viewport keeps (camera, section view) as text it reads back itself (Inventor 2026 help, About View
/// Representations, GUID-9A49CDC1-4DC7-4462-81D7-E18B8339DE0F: visibility, enabled state, transparency,
/// zoom and viewing angle).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ViewRepresentation {
    /// Name shown in the Representations folder.
    pub name: String,
    /// Components (parts and robot links included) hidden in this view; all others are shown.
    pub hidden: Vec<Occurrence>,
    /// Components not enabled (shown in the background style, not selectable) in this view.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub disabled: Vec<Occurrence>,
    /// Transparency of bodies, robots and assemblies that are not opaque in this view.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transparency: Vec<(AnyId, f64)>,
    /// The camera as the viewport writes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<String>,
    /// The section view definition as the viewport writes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    /// Lock (Inventor 2026 help, To Create, Edit, and Restore View Representations,
    /// GUID-397F9263-12C3-47A1-B6A6-16FC05EDEF47: "Prevents changes to the View representation display
    /// characteristics. Components added after a view is locked are not visible while the view is
    /// active").
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub locked: bool,
    /// The components there were when the view was locked (what came later is hidden in it).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub known: Vec<Occurrence>,
}

/// A saved position: overrides on top of the Primary state.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PositionalRepresentation {
    /// Name shown in the Representations folder.
    pub name: String,
    /// Relationship values (offset in metres or angle in radians) in this position.
    pub values: Vec<(Id<Relationship>, f64)>,
    /// Relationships suppressed (`true`) or brought back (`false`) in this position.
    pub suppression: Vec<(Id<Relationship>, bool)>,
    /// Where components stand in this position (for ones no relationship holds).
    pub placements: Vec<(AnyId, Placement)>,
}

/// All representations and which are active.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Representations {
    /// Saved views.
    #[serde(default)]
    pub views: Vec<ViewRepresentation>,
    /// Saved positions ([Primary] is not stored: it is the document itself).
    #[serde(default)]
    pub positions: Vec<PositionalRepresentation>,
    /// The active position (`None`: [Primary]).
    #[serde(default)]
    pub active_position: Option<usize>,
    /// The view representation activated last (shown checked in the browser), if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_view: Option<usize>,
    /// Component placements of [Primary] while another position is active, to go back to.
    #[serde(default)]
    pub primary_placements: Vec<(AnyId, Placement)>,
}

impl Representations {
    /// True when nothing is stored.
    pub fn is_empty(&self) -> bool {
        self.views.is_empty()
            && self.positions.is_empty()
            && self.active_position.is_none()
            && self.active_view.is_none()
    }
}

impl crate::document::Document {
    /// The representations.
    pub fn representations(&self) -> Representations {
        self.timeline().map(|(_, t)| t.representations.clone()).unwrap_or_default()
    }

    /// The active positional representation's overrides, if one is active.
    pub(crate) fn active_position(&self) -> Option<PositionalRepresentation> {
        let reps = self.timeline().map(|(_, t)| &t.representations)?;
        reps.active_position.and_then(|i| reps.positions.get(i)).cloned()
    }

    /// Every component that can be shown or hidden: bodies, robots, assemblies, their parts and links.
    fn all_occurrences(&self) -> Vec<Occurrence> {
        let mut out: Vec<Occurrence> = self.components().iter().map(|n| n.occurrence).collect();
        for (id, _) in self.robots().iter() {
            out.extend(self.robot_links(id).iter().map(|n| n.occurrence));
        }
        out
    }
}

fn write(txn: &mut Txn<'_>, reps: Representations) -> Result<(), EditError> {
    crate::cell::set_timeline(txn, |t| t.representations = reps)
}

/// What a view representation saves of the display as it is now: hidden, not enabled, transparency.
fn display_now(doc: &crate::document::Document) -> (Vec<Occurrence>, Vec<Occurrence>, Vec<(AnyId, f64)>) {
    let all = doc.all_occurrences();
    let hidden = all.iter().copied().filter(|o| !doc.component_flag(*o, ComponentFlag::Visible)).collect();
    let disabled = all.iter().copied().filter(|o| !doc.component_flag(*o, ComponentFlag::Enabled)).collect();
    let transparency = doc
        .bodies()
        .iter()
        .map(|(id, b)| (id.erase(), b.look.transparency))
        .chain(doc.robots().iter().map(|(id, r)| (id.erase(), r.look.transparency)))
        .chain(doc.assemblies().iter().map(|(id, a)| (id.erase(), a.look.transparency)))
        .filter(|(_, t)| *t > 0.0)
        .collect();
    (hidden, disabled, transparency)
}

/// True when the active view representation is locked (components added then are hidden in it).
pub(crate) fn active_view_locked(doc: &crate::document::Document) -> bool {
    doc.timeline().is_some_and(|(_, t)| {
        let reps = &t.representations;
        reps.active_view.and_then(|i| reps.views.get(i)).is_some_and(|v| v.locked)
    })
}

/// Keeps the active view representation in step after a command (Inventor 2026 help, To Create, Edit,
/// and Restore View Representations, GUID-397F9263-12C3-47A1-B6A6-16FC05EDEF47: "Edit a View
/// representation … Activate. Edit the display of components"; "select Lock so the representation does
/// not update changes to the display"; "Components added after a view is locked are not visible while
/// the view is active"). An unlocked active view takes the display as it is now; a locked one stays as
/// it is and hides the components that came after it was locked.
pub(crate) fn follow(txn: &mut Txn<'_>) -> Result<(), EditError> {
    let mut reps = txn.doc().representations();
    let Some(index) = reps.active_view else { return Ok(()) };
    let Some(view) = reps.views.get(index).cloned() else { return Ok(()) };
    if !view.locked {
        let (hidden, disabled, transparency) = display_now(txn.doc());
        if hidden == view.hidden && disabled == view.disabled && transparency == view.transparency {
            return Ok(());
        }
        reps.views[index] = ViewRepresentation { hidden, disabled, transparency, ..view };
        return write(txn, reps);
    }
    let new: Vec<Occurrence> =
        txn.doc().all_occurrences().into_iter().filter(|o| !view.known.contains(o)).collect();
    if new.is_empty() {
        return Ok(());
    }
    for occurrence in &new {
        if txn.doc().component_flag(*occurrence, ComponentFlag::Visible) {
            crate::tree::set_component(txn, *occurrence, ComponentFlag::Visible, false)?;
        }
    }
    let mut reps = txn.doc().representations();
    if let Some(view) = reps.views.get_mut(index) {
        view.known.extend(new.iter().copied());
        for occurrence in &new {
            if !view.hidden.contains(occurrence) {
                view.hidden.push(*occurrence);
            }
        }
    }
    write(txn, reps)
}

/// Lock or unlock a view representation.
pub(crate) fn lock_view(
    txn: &mut Txn<'_>,
    index: usize,
    locked: bool,
) -> Result<(Vec<Created>, &'static str), EditError> {
    let mut reps = txn.doc().representations();
    let known = if locked { txn.doc().all_occurrences() } else { Vec::new() };
    let view = reps.views.get_mut(index).ok_or(EditError::BadValue)?;
    view.locked = locked;
    view.known = known;
    write(txn, reps)?;
    Ok((Vec::new(), if locked { "Lock view representation" } else { "Unlock view representation" }))
}

/// A free name made from `name` with a number appended ("View1" gives "View2"; "Open" gives "Open1").
fn numbered(name: &str, taken: &dyn Fn(&str) -> bool) -> String {
    let stem = name.trim_end_matches(|c: char| c.is_ascii_digit());
    let stem = if stem.is_empty() { name } else { stem };
    (1u32..).map(|n| format!("{stem}{n}")).find(|n| !taken(n)).unwrap_or_else(|| name.to_owned())
}

/// Copy (same help page: "Duplicates the current View representation to make it active. The
/// incremental number appends the name"): a copy of a view representation, unlocked, at the end of the
/// list; it becomes the active one and the display is set as it says.
pub(crate) fn copy_view(txn: &mut Txn<'_>, index: usize) -> Result<(Vec<Created>, &'static str), EditError> {
    let mut reps = txn.doc().representations();
    let source = reps.views.get(index).cloned().ok_or(EditError::BadValue)?;
    let name = numbered(&source.name, &|n| reps.views.iter().any(|v| v.name == n));
    reps.views.push(ViewRepresentation { name, locked: false, known: Vec::new(), ..source });
    let copy = reps.views.len() - 1;
    write(txn, reps)?;
    activate_view(txn, copy)?;
    Ok((Vec::new(), "Copy view representation"))
}

/// Copy of a positional representation (Inventor 2026 help, To Create, Edit, or Copy Positional
/// Representations, GUID-34C28C35-45CA-4476-9336-30146F7C9471: "The copied representation is placed in
/// the browser at the same level as the [Primary] representation"): the same overrides under a
/// numbered name, at the end of the list; which one is active does not change.
pub(crate) fn copy_position(
    txn: &mut Txn<'_>,
    index: usize,
) -> Result<(Vec<Created>, &'static str), EditError> {
    let mut reps = txn.doc().representations();
    let source = reps.positions.get(index).cloned().ok_or(EditError::BadValue)?;
    let name = numbered(&source.name, &|n| n == "Primary" || reps.positions.iter().any(|p| p.name == n));
    reps.positions.push(PositionalRepresentation { name, ..source });
    write(txn, reps)?;
    Ok((Vec::new(), "Copy positional representation"))
}

/// Saves the current visibility as a new view representation.
pub(crate) fn add_view(
    txn: &mut Txn<'_>,
    name: &str,
    camera: Option<&str>,
    section: Option<&str>,
) -> Result<(Vec<Created>, &'static str), EditError> {
    let doc = txn.doc();
    let (hidden, disabled, transparency) = display_now(doc);
    let mut reps = doc.representations();
    if name.trim().is_empty() || reps.views.iter().any(|v| v.name == name) {
        return Err(EditError::NameTaken);
    }
    reps.views.push(ViewRepresentation {
        name: name.trim().to_owned(),
        hidden,
        disabled,
        transparency,
        camera: camera.map(str::to_owned),
        section: section.map(str::to_owned),
        locked: false,
        known: Vec::new(),
    });
    // "It becomes the active View representation, as designated by a check mark" (To Create, Edit, and
    // Restore View Representations, GUID-397F9263-12C3-47A1-B6A6-16FC05EDEF47).
    reps.active_view = Some(reps.views.len() - 1);
    write(txn, reps)?;
    Ok((Vec::new(), "New view representation"))
}

/// Replaces a view representation.
pub(crate) fn set_view(
    txn: &mut Txn<'_>,
    index: usize,
    view: &ViewRepresentation,
) -> Result<(Vec<Created>, &'static str), EditError> {
    let mut reps = txn.doc().representations();
    let slot = reps.views.get_mut(index).ok_or(EditError::BadValue)?;
    // Lock keeps the display characteristics; the name may still change.
    if slot.locked {
        let name = view.name.clone();
        *slot = ViewRepresentation { name, ..slot.clone() };
    } else {
        *slot = ViewRepresentation { locked: false, known: Vec::new(), ..view.clone() };
    }
    write(txn, reps)?;
    Ok((Vec::new(), "Edit view representation"))
}

/// Shows and hides components as a view representation says (one undo step).
pub(crate) fn activate_view(
    txn: &mut Txn<'_>,
    index: usize,
) -> Result<(Vec<Created>, &'static str), EditError> {
    let view = txn.doc().representations().views.get(index).cloned().ok_or(EditError::BadValue)?;
    for occurrence in txn.doc().all_occurrences() {
        let visible = !view.hidden.contains(&occurrence);
        if txn.doc().component_flag(occurrence, ComponentFlag::Visible) != visible {
            crate::tree::set_component(txn, occurrence, ComponentFlag::Visible, visible)?;
        }
        let enabled = !view.disabled.contains(&occurrence);
        if txn.doc().component_flag(occurrence, ComponentFlag::Enabled) != enabled {
            crate::tree::set_component(txn, occurrence, ComponentFlag::Enabled, enabled)?;
        }
    }
    // Transparency: as saved, opaque otherwise.
    let doc = txn.doc();
    let items: Vec<AnyId> = doc
        .bodies()
        .iter()
        .map(|(id, _)| id.erase())
        .chain(doc.robots().iter().map(|(id, _)| id.erase()))
        .chain(doc.assemblies().iter().map(|(id, _)| id.erase()))
        .collect();
    for item in items {
        let t = view.transparency.iter().find(|(i, _)| *i == item).map_or(0.0, |(_, t)| t.clamp(0.0, 0.95));
        set_transparency(txn, item, t)?;
    }
    let mut reps = txn.doc().representations();
    reps.active_view = Some(index);
    write(txn, reps)?;
    Ok((Vec::new(), "Activate view representation"))
}

/// Adds a positional representation (a copy of [Primary]: no overrides).
pub(crate) fn add_position(txn: &mut Txn<'_>, name: &str) -> Result<(Vec<Created>, &'static str), EditError> {
    let mut reps = txn.doc().representations();
    let name = name.trim();
    if name.is_empty() || name == "Primary" || reps.positions.iter().any(|p| p.name == name) {
        return Err(EditError::NameTaken);
    }
    reps.positions.push(PositionalRepresentation { name: name.to_owned(), ..Default::default() });
    write(txn, reps)?;
    Ok((Vec::new(), "New positional representation"))
}

/// Replaces a positional representation's overrides (re-solved when it is the active one).
pub(crate) fn set_position(
    txn: &mut Txn<'_>,
    index: usize,
    position: &PositionalRepresentation,
) -> Result<(Vec<Created>, &'static str), EditError> {
    if !position.values.iter().all(|(_, v)| v.is_finite()) {
        return Err(EditError::BadValue);
    }
    let mut reps = txn.doc().representations();
    *reps.positions.get_mut(index).ok_or(EditError::BadValue)? = position.clone();
    let active = reps.active_position == Some(index);
    write(txn, reps)?;
    if active {
        apply_placements(txn, &position.placements)?;
        crate::cell::settle(txn)?;
    }
    Ok((Vec::new(), "Edit positional representation"))
}

fn apply_placements(txn: &mut Txn<'_>, placements: &[(AnyId, Placement)]) -> Result<(), EditError> {
    for (item, placement) in placements {
        if txn.doc().owner_pose(*item).is_some() {
            let pose = gc_math::pose_from_xyz_rpy(placement.xyz, placement.rpy);
            crate::cell::set_owner_pose(txn, *item, pose)?;
        }
    }
    Ok(())
}

/// Activates a positional representation (`None`: [Primary]): its overrides apply and the relationships
/// are solved again; going back to [Primary] puts the components where [Primary] had them.
pub(crate) fn activate_position(
    txn: &mut Txn<'_>,
    index: Option<usize>,
) -> Result<(Vec<Created>, &'static str), EditError> {
    let mut reps = txn.doc().representations();
    if index.is_some_and(|i| i >= reps.positions.len()) {
        return Err(EditError::BadValue);
    }
    if reps.active_position == index {
        return Ok((Vec::new(), "Activate positional representation"));
    }
    // Leaving [Primary]: remember where its components stand.
    if reps.active_position.is_none() {
        let doc = txn.doc();
        reps.primary_placements = doc
            .bodies()
            .iter()
            .map(|(id, b)| (id.erase(), b.placement))
            .chain(doc.robots().iter().map(|(id, r)| (id.erase(), r.placement)))
            .chain(doc.assemblies().iter().map(|(id, a)| (id.erase(), a.placement)))
            .collect();
    }
    let primary = reps.primary_placements.clone();
    reps.active_position = index;
    if index.is_none() {
        reps.primary_placements.clear();
    }
    let overrides =
        index.and_then(|i| reps.positions.get(i)).map(|p| p.placements.clone()).unwrap_or_default();
    write(txn, reps)?;
    apply_placements(txn, &primary)?;
    apply_placements(txn, &overrides)?;
    crate::cell::settle(txn)?;
    Ok((Vec::new(), "Activate positional representation"))
}

/// Deletes a view or positional representation (the active position goes back to [Primary] first).
pub(crate) fn delete(
    txn: &mut Txn<'_>,
    view: Option<usize>,
    position: Option<usize>,
) -> Result<(Vec<Created>, &'static str), EditError> {
    if let Some(i) = position
        && txn.doc().representations().active_position == Some(i)
    {
        activate_position(txn, None)?;
    }
    let mut reps = txn.doc().representations();
    if let Some(i) = view {
        if i >= reps.views.len() {
            return Err(EditError::BadValue);
        }
        reps.views.remove(i);
        reps.active_view = match reps.active_view {
            Some(a) if a == i => None,
            Some(a) if a > i => Some(a - 1),
            other => other,
        };
    }
    if let Some(i) = position {
        if i >= reps.positions.len() {
            return Err(EditError::BadValue);
        }
        reps.positions.remove(i);
        reps.active_position = reps.active_position.map(|a| if a > i { a - 1 } else { a });
    }
    write(txn, reps)?;
    Ok((Vec::new(), "Delete representation"))
}

fn set_transparency(txn: &mut Txn<'_>, item: AnyId, transparency: f64) -> Result<(), EditError> {
    use gc_core::Kind;
    let differs = |now: f64| (now - transparency).abs() > 1e-12;
    match item.kind {
        Kind::Body => {
            let id = Id::<crate::cell::Body>::from_raw(item.raw);
            if txn.doc().get(id).is_some_and(|b| differs(b.look.transparency)) {
                txn.modify(id, |b: &mut crate::cell::Body, _| b.look.transparency = transparency)?;
            }
        }
        Kind::RobotInstance => {
            let id = Id::<crate::cell::Robot>::from_raw(item.raw);
            if txn.doc().get(id).is_some_and(|r| differs(r.look.transparency)) {
                txn.modify(id, |r: &mut crate::cell::Robot, _| r.look.transparency = transparency)?;
            }
        }
        Kind::Assembly => {
            let id = Id::<crate::design::Assembly>::from_raw(item.raw);
            if txn.doc().get(id).is_some_and(|a| differs(a.look.transparency)) {
                txn.modify(id, |a: &mut crate::design::Assembly, _| a.look.transparency = transparency)?;
            }
        }
        _ => {}
    }
    Ok(())
}
