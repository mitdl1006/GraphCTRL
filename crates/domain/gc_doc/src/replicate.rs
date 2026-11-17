//! Mirror, copy and replace components (docs/ux/inventor-assembly-parity.md).
//!
//! Inventor 2026 help:
//! - About Mirrored Assembly Components (GUID-66AA4A06-F63F-49C3-8F39-42FE4999F1FC), To Mirror Assembly
//!   Components (GUID-C5B3D3DF-27B3-4B4C-9058-A949138D8B8A), Mirror Components: Reference
//!   (GUID-7B19CA69-8D14-40DA-9543-115A8AFE6336): components are mirrored across a work plane, planar face
//!   or origin plane; a reused component is "transformed by a rotation and translation": a plane of symmetry
//!   is calculated and intersected with the reflection plane to give the axis it turns about. "The Mirror
//!   command re-creates any relationship that has both references in the set of components selected for
//!   mirror or reuse. If the orientation is ambiguous, the new relationship is suppressed. Suppressed
//!   relationships are listed in a message". Options: Mirror Relationships, Ground New Components.
//! - About Copied Assembly Components (GUID-0E21F4A0-DE77-469D-9381-9F55CFA6BF49), Copy assembly
//!   components (GUID-5AAA6CB1-7BB2-4AF0-9A3D-AB729E5160C5): copies keep placement, visibility and
//!   appearance; Copy Relationships applies the relationships between the selected components to the
//!   copies; Ground New Components; copies are not associative.
//! - Replace assembly components (GUID-E5FAA224-7332-4AC6-A78E-793FDACFEEDD): Replace or Replace All
//!   occurrences; the replacement stands where the original stood, origins coincident; relationships are
//!   kept where possible, the rest must be applied again.
//!
//! The cell keeps no part files, so Mirror and Reuse give the same thing here: a new instance turned by a
//! rotation and translation (exact for boxes, cylinders and spheres, which are symmetric about their own
//! planes; a mesh, assembly or robot is not reflected, see docs/handoff/backend.md).

use gc_core::{AnyId, Id, Kind};
use gc_math::{DMat3, DPose3, DQuat, DVec3};
use serde::{Deserialize, Serialize};

use crate::cell::{Body, Placement, Robot, Solid};
use crate::command::{Created, EditError};
use crate::design::{Assembly, ComponentFlag, Occurrence};
use crate::document::Document;
use crate::relate::{AngleSolution, Geometry, RelationKind, Relationship, Selection};
use crate::txn::Txn;

/// What a component is replaced with.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Replacement {
    /// A body takes this shape (a mesh file or a primitive); it keeps its name, placement and look.
    Solid(Solid),
    /// An imported assembly takes this file's part tree; its placement and look are kept, the name and
    /// file are the new ones.
    Assembly(Assembly),
}

/// What happened to the relationships of a mirror, copy or replace.
#[derive(Clone, Debug, Default, PartialEq)]
struct ReplicateReport {
    /// Relationships made for the copies.
    relationships: Vec<Id<Relationship>>,
    /// Relationships suppressed because they could not be satisfied.
    suppressed: Vec<Id<Relationship>>,
}

/// A reflection across a world plane.
#[derive(Clone, Copy, Debug)]
struct Mirror {
    point: DVec3,
    normal: DVec3,
}

impl Mirror {
    fn point(&self, p: DVec3) -> DVec3 {
        p - 2.0 * (p - self.point).dot(self.normal) * self.normal
    }

    fn matrix(&self) -> DMat3 {
        let n = self.normal;
        DMat3::IDENTITY - 2.0 * DMat3::from_cols(n * n.x, n * n.y, n * n.z)
    }
}

/// How a copy relates to its source: the local axis flipped (a reflection in the component's own frame)
/// when mirrored.
#[derive(Clone, Copy, Debug)]
struct Mapping {
    flipped: Option<usize>,
}

fn flip(v: DVec3, axis: Option<usize>) -> DVec3 {
    match axis {
        Some(0) => DVec3::new(-v.x, v.y, v.z),
        Some(1) => DVec3::new(v.x, -v.y, v.z),
        Some(2) => DVec3::new(v.x, v.y, -v.z),
        _ => v,
    }
}

/// The mirrored pose of a component at `pose`: its origin reflected, turned so that its own plane of
/// symmetry (the local coordinate plane whose normal lies closest to the mirror's) is reflected onto
/// itself. The local axis flipped is returned with it.
fn mirrored(pose: &DPose3, mirror: &Mirror) -> (DPose3, usize) {
    let r = DMat3::from_quat(pose.rotation);
    let axis = (0..3)
        .max_by(|a, b| r.col(*a).dot(mirror.normal).abs().total_cmp(&r.col(*b).dot(mirror.normal).abs()))
        .unwrap_or(2);
    let f = DMat3::from_diagonal(flip(DVec3::ONE, Some(axis)));
    let rotation = DQuat::from_mat3(&(mirror.matrix() * r * f)).normalize();
    (DPose3::from_parts(mirror.point(pose.translation), rotation), axis)
}

fn placement_of(pose: &DPose3) -> Placement {
    Placement { xyz: pose.translation.to_array(), rpy: gc_math::rpy_from_quat(pose.rotation) }
}

/// A new instance of a component at a pose, named after it.
fn instance(
    txn: &mut Txn<'_>,
    item: AnyId,
    pose: &DPose3,
    created: &mut Vec<Created>,
) -> Result<AnyId, EditError> {
    let placement = placement_of(pose);
    let doc = txn.doc();
    let copy = match item.kind {
        Kind::Body => {
            let mut body = doc.get(Id::<Body>::from_raw(item.raw)).ok_or(EditError::BadValue)?.clone();
            body.name = next_name(doc, &body.name);
            body.placement = placement;
            let id = txn.insert(body);
            created.push(Created::Body(id));
            id.erase()
        }
        Kind::RobotInstance => {
            let mut robot = doc.get(Id::<Robot>::from_raw(item.raw)).ok_or(EditError::BadValue)?.clone();
            robot.name = next_name(doc, &robot.name);
            robot.placement = placement;
            let id = txn.insert(robot);
            created.push(Created::Robot(id));
            id.erase()
        }
        Kind::Assembly => {
            let mut assembly =
                doc.get(Id::<Assembly>::from_raw(item.raw)).ok_or(EditError::BadValue)?.clone();
            assembly.name = next_name(doc, &assembly.name);
            assembly.placement = placement;
            let id = txn.insert(assembly);
            created.push(Created::Assembly(id));
            id.erase()
        }
        _ => return Err(EditError::BadValue),
    };
    Ok(copy)
}

/// `base:N` with the lowest N no component or relationship uses (the stem before an existing `:N` kept).
fn next_name(doc: &Document, name: &str) -> String {
    let stem = match name.rsplit_once(':') {
        Some((stem, n)) if !stem.is_empty() && n.parse::<u32>().is_ok() => stem,
        _ => name,
    };
    let taken = |candidate: &str| {
        doc.bodies().iter().any(|(_, b)| b.name == candidate)
            || doc.robots().iter().any(|(_, r)| r.name == candidate)
            || doc.assemblies().iter().any(|(_, a)| a.name == candidate)
            || doc.relationships().iter().any(|(_, r)| r.name == candidate)
    };
    (1..).map(|n| format!("{stem}:{n}")).find(|c| !taken(c)).unwrap_or_else(|| format!("{stem}:copy"))
}

impl Document {
    /// The pose of an occurrence in its owner's frame (identity for the owner itself).
    fn local_pose(&self, occurrence: Occurrence) -> Option<DPose3> {
        let owner = self.owner_pose(occurrence.owner())?;
        let at = self.component_origin(occurrence)?.pose;
        Some(owner.inverse() * at)
    }

    /// The components with the same source as `item`: bodies showing the same mesh file (or the same
    /// primitive), assemblies from the same file, robots from the same description. What Replace All
    /// replaces.
    pub fn same_source(&self, item: AnyId) -> Vec<AnyId> {
        match item.kind {
            Kind::Body => {
                let Some(solid) = self.get(Id::<Body>::from_raw(item.raw)).map(|b| b.solid.clone()) else {
                    return Vec::new();
                };
                self.bodies().iter().filter(|(_, b)| b.solid == solid).map(|(id, _)| id.erase()).collect()
            }
            Kind::Assembly => {
                let Some(file) = self.get(Id::<Assembly>::from_raw(item.raw)).map(|a| a.file.clone()) else {
                    return Vec::new();
                };
                self.assemblies().iter().filter(|(_, a)| a.file == file).map(|(id, _)| id.erase()).collect()
            }
            Kind::RobotInstance => {
                let Some(urdf) = self.get(Id::<Robot>::from_raw(item.raw)).map(|r| r.urdf.clone()) else {
                    return Vec::new();
                };
                self.robots().iter().filter(|(_, r)| r.urdf == urdf).map(|(id, _)| id.erase()).collect()
            }
            _ => Vec::new(),
        }
    }
}

/// A selection moved to a copy: the same occurrence on the copy's owner, the geometry reflected in the
/// owner's frame when mirrored.
fn map_selection(
    doc: &Document,
    selection: &Selection,
    copies: &[(AnyId, AnyId, Mapping)],
) -> Option<Selection> {
    let owner = selection.occurrence.owner();
    let &(_, copy, mapping) = copies.iter().find(|(source, _, _)| *source == owner)?;
    let occurrence = match selection.occurrence {
        Occurrence::Item(_) => Occurrence::Item(copy),
        Occurrence::Part { index, .. } => Occurrence::Part { assembly: Id::from_raw(copy.raw), index },
        Occurrence::Link { index, .. } => Occurrence::Link { robot: Id::from_raw(copy.raw), index },
        Occurrence::Tool { .. } => Occurrence::Tool { robot: Id::from_raw(copy.raw) },
    };
    let Some(axis) = mapping.flipped else {
        return Some(Selection { occurrence, geometry: selection.geometry });
    };
    // In the occurrence's frame: to the owner's, reflect, back.
    let local = doc.local_pose(selection.occurrence)?;
    let inverse = local.inverse();
    let apply = |pose: &DPose3, v: DVec3| pose.translation + pose.rotation * v;
    let p = |v: [f64; 3]| apply(&inverse, flip(apply(&local, DVec3::from_array(v)), Some(axis))).to_array();
    let d =
        |v: [f64; 3]| (inverse.rotation * flip(local.rotation * DVec3::from_array(v), Some(axis))).to_array();
    let geometry = match selection.geometry {
        Geometry::Plane { point, normal } => Geometry::Plane { point: p(point), normal: d(normal) },
        Geometry::Axis { point, direction } => Geometry::Axis { point: p(point), direction: d(direction) },
        Geometry::Point(point) => Geometry::Point(p(point)),
        Geometry::Circle { centre, normal } => Geometry::Circle { centre: p(centre), normal: d(normal) },
        Geometry::Frame { origin, z, x } => Geometry::Frame { origin: p(origin), z: d(z), x: d(x) },
        Geometry::Cylinder { point, direction, radius } => {
            Geometry::Cylinder { point: p(point), direction: d(direction), radius }
        }
        Geometry::Sphere { centre, radius } => Geometry::Sphere { centre: p(centre), radius },
    };
    Some(Selection { occurrence, geometry })
}

/// The relationships between the sources, re-made for the copies. A third reference (symmetry plane,
/// angle reference) outside the set stays as it is for a copy; for a mirror it makes the orientation
/// ambiguous and the relationship is made suppressed.
fn copied_relationships(
    doc: &Document,
    copies: &[(AnyId, AnyId, Mapping)],
    mirrored: bool,
) -> Vec<(Relationship, bool)> {
    let in_set = |s: &Selection| copies.iter().any(|(source, _, _)| *source == s.occurrence.owner());
    let mut out = Vec::new();
    for (id, relationship) in doc.relationships().iter() {
        if doc.is_inactive(id.erase()) || !in_set(&relationship.first) || !in_set(&relationship.second) {
            continue;
        }
        let (Some(first), Some(second)) = (
            map_selection(doc, &relationship.first, copies),
            map_selection(doc, &relationship.second, copies),
        ) else {
            continue;
        };
        let mut ambiguous = false;
        let mut third = |s: &Selection| -> Selection {
            if in_set(s) {
                map_selection(doc, s, copies).unwrap_or(*s)
            } else {
                ambiguous |= mirrored;
                *s
            }
        };
        let kind = match relationship.kind {
            RelationKind::Symmetry { plane } => RelationKind::Symmetry { plane: third(&plane) },
            RelationKind::Angle { angle, solution: AngleSolution::Explicit(reference) } => {
                RelationKind::Angle { angle, solution: AngleSolution::Explicit(third(&reference)) }
            }
            // A reflection turns the right-hand rule round.
            RelationKind::Angle { angle, solution: AngleSolution::Directed } if mirrored => {
                RelationKind::Angle { angle: -angle, solution: AngleSolution::Directed }
            }
            other => other,
        };
        let copy = Relationship {
            name: relationship.name.clone(),
            kind,
            first,
            second,
            limits: relationship.limits,
        };
        out.push((copy, ambiguous));
    }
    out
}

/// Adds relationships, then solves; those that cannot be satisfied are suppressed (one round per
/// conflict found) and returned. Fails only when relationships that were there before stop solving.
fn add_and_solve(
    txn: &mut Txn<'_>,
    relationships: Vec<(Relationship, bool)>,
    report: &mut ReplicateReport,
    created: &mut Vec<Created>,
) -> Result<(), EditError> {
    for (mut relationship, ambiguous) in relationships {
        let stem =
            relationship.name.rsplit_once(':').map_or(relationship.name.as_str(), |(s, _)| s).to_owned();
        relationship.name = next_name(txn.doc(), &stem);
        let id = txn.insert(relationship);
        created.push(Created::Relationship(id));
        report.relationships.push(id);
        if ambiguous {
            suppress(txn, id)?;
            report.suppressed.push(id);
        }
    }
    solve_suppressing(txn, &report.relationships.clone(), &mut report.suppressed)
}

fn suppress(txn: &mut Txn<'_>, id: Id<Relationship>) -> Result<(), EditError> {
    crate::cell::set_timeline(txn, |t| {
        if !t.suppressed.contains(&id.erase()) {
            t.suppressed.push(id.erase());
        }
    })
}

/// Solves; relationships among `candidates` that stand in the way are suppressed until the rest solves.
fn solve_suppressing(
    txn: &mut Txn<'_>,
    candidates: &[Id<Relationship>],
    suppressed: &mut Vec<Id<Relationship>>,
) -> Result<(), EditError> {
    for _ in 0..=candidates.len() {
        let sick = match txn.doc().solve_relationships() {
            Ok(_) => return crate::cell::settle(txn),
            Err(unsolved) => unsolved.sick,
        };
        let ours: Vec<Id<Relationship>> =
            sick.iter().copied().filter(|id| candidates.contains(id) && !suppressed.contains(id)).collect();
        if ours.is_empty() {
            // Not of this edit's making: refuse with the usual message.
            return crate::cell::settle(txn);
        }
        for id in ours {
            suppress(txn, id)?;
            suppressed.push(id);
        }
    }
    crate::cell::settle(txn)
}

/// Copies the enabled state of a source to its copy, and grounds the copy when asked.
fn copy_states(txn: &mut Txn<'_>, source: AnyId, copy: AnyId, ground: bool) -> Result<(), EditError> {
    if !txn.doc().component_flag(Occurrence::Item(source), ComponentFlag::Enabled) {
        crate::tree::set_component(txn, Occurrence::Item(copy), ComponentFlag::Enabled, false)?;
    }
    if ground {
        crate::tree::set_component(txn, Occurrence::Item(copy), ComponentFlag::Grounded, true)?;
    }
    Ok(())
}

fn check_items(doc: &Document, items: &[AnyId]) -> Result<(), EditError> {
    let mut seen = Vec::with_capacity(items.len());
    for item in items {
        if seen.contains(item)
            || !matches!(item.kind, Kind::Body | Kind::RobotInstance | Kind::Assembly)
            || doc.owner_pose(*item).is_none()
        {
            return Err(EditError::BadValue);
        }
        seen.push(*item);
    }
    if items.is_empty() { Err(EditError::BadValue) } else { Ok(()) }
}

/// Mirrors components across a plane (a planar face, work plane or origin plane on any component).
pub(crate) fn mirror(
    txn: &mut Txn<'_>,
    items: &[AnyId],
    plane: &Selection,
    relationships: bool,
    ground: bool,
) -> Result<(Vec<Created>, &'static str), EditError> {
    check_items(txn.doc(), items)?;
    if !matches!(plane.geometry, Geometry::Plane { .. }) || !plane.geometry.is_finite() {
        return Err(EditError::BadValue);
    }
    let (point, normal) = txn.doc().selection_axis(plane).ok_or(EditError::BadValue)?;
    if !point.is_finite() || !normal.is_finite() {
        return Err(EditError::BadValue);
    }
    let mirror = Mirror { point, normal };
    let mut created = Vec::new();
    let mut copies = Vec::with_capacity(items.len());
    for item in items {
        let pose = txn.doc().owner_pose(*item).ok_or(EditError::BadValue)?;
        let (at, axis) = mirrored(&pose, &mirror);
        let copy = instance(txn, *item, &at, &mut created)?;
        copy_states(txn, *item, copy, ground)?;
        copies.push((*item, copy, Mapping { flipped: Some(axis) }));
    }
    finish(txn, copies, relationships, true, created).map(|c| (c, "Mirror components"))
}

/// Copies components, each moved by `offset` (a world transform; the identity leaves them where the
/// sources are).
pub(crate) fn copy(
    txn: &mut Txn<'_>,
    items: &[AnyId],
    offset: &Placement,
    relationships: bool,
    ground: bool,
) -> Result<(Vec<Created>, &'static str), EditError> {
    check_items(txn.doc(), items)?;
    if !offset.xyz.iter().chain(&offset.rpy).all(|v| v.is_finite()) {
        return Err(EditError::BadValue);
    }
    let shift = gc_math::pose_from_xyz_rpy(offset.xyz, offset.rpy);
    let mut created = Vec::new();
    let mut copies = Vec::with_capacity(items.len());
    for item in items {
        let pose = shift * txn.doc().owner_pose(*item).ok_or(EditError::BadValue)?;
        let copy = instance(txn, *item, &pose, &mut created)?;
        copy_states(txn, *item, copy, ground)?;
        copies.push((*item, copy, Mapping { flipped: None }));
    }
    finish(txn, copies, relationships, false, created).map(|c| (c, "Copy components"))
}

fn finish(
    txn: &mut Txn<'_>,
    copies: Vec<(AnyId, AnyId, Mapping)>,
    relationships: bool,
    mirrored: bool,
    mut created: Vec<Created>,
) -> Result<Vec<Created>, EditError> {
    let mut report = ReplicateReport::default();
    if relationships {
        let made = copied_relationships(txn.doc(), &copies, mirrored);
        add_and_solve(txn, made, &mut report, &mut created)?;
    }
    Ok(created)
}

/// True when a selection's geometry still lies on a primitive (a box's faces, edges and corners, a
/// cylinder's faces and axis, a sphere's centre); origin geometry always does. Mesh shapes are not known
/// to the document and are taken as fitting.
fn fits(solid: &Solid, geometry: &Geometry) -> bool {
    const TOL: f64 = 1e-6;
    let on_origin_line = |p: [f64; 3], d: [f64; 3]| {
        let d = DVec3::from_array(d).normalize_or_zero();
        let p = DVec3::from_array(p);
        (p - d * p.dot(d)).length() < TOL
    };
    match (solid, *geometry) {
        (Solid::Mesh { .. }, _) => true,
        // Origin planes, axes, the origin point and the origin frame.
        (_, Geometry::Plane { point, normal })
            if DVec3::from_array(point).dot(DVec3::from_array(normal)).abs() < TOL =>
        {
            true
        }
        (_, Geometry::Axis { point, direction }) if on_origin_line(point, direction) => true,
        (_, Geometry::Point(p) | Geometry::Frame { origin: p, .. })
            if DVec3::from_array(p).length() < TOL =>
        {
            true
        }
        (Solid::Box { size }, Geometry::Plane { point, normal }) => {
            let n = DVec3::from_array(normal).normalize_or_zero();
            (0..3).any(|k| {
                let mut e = DVec3::ZERO;
                e[k] = 1.0;
                n.dot(e).abs() > 1.0 - TOL
                    && (DVec3::from_array(point).dot(e).abs() - size[k] / 2.0).abs() < TOL
            })
        }
        (Solid::Box { size }, Geometry::Point(p)) => (0..3).all(|k| (p[k].abs() - size[k] / 2.0).abs() < TOL),
        (Solid::Cylinder { length, .. }, Geometry::Plane { point, normal }) => {
            DVec3::from_array(normal).normalize_or_zero().z.abs() > 1.0 - TOL
                && (point[2].abs() - length / 2.0).abs() < TOL
        }
        (Solid::Cylinder { radius: r, .. }, Geometry::Cylinder { point, direction, radius }) => {
            on_origin_line(point, direction) && (radius - r).abs() < TOL
        }
        (Solid::Cylinder { radius: r, length }, Geometry::Circle { centre, normal }) => {
            on_origin_line(centre, [0.0, 0.0, 1.0])
                && DVec3::from_array(normal).normalize_or_zero().z.abs() > 1.0 - TOL
                && (centre[2].abs() - length / 2.0).abs() < TOL
                && *r > 0.0
        }
        (Solid::Sphere { radius: r }, Geometry::Sphere { centre, radius }) => {
            DVec3::from_array(centre).length() < TOL && (radius - r).abs() < TOL
        }
        _ => false,
    }
}

/// Replaces components: each keeps its place (origins coincident), name (bodies) and look. The
/// relationships on them are kept where their geometry still exists on the new shape (assembly parts are
/// found again by name); the others, and those that no longer solve, are suppressed for the user to apply
/// again.
pub(crate) fn replace(
    txn: &mut Txn<'_>,
    items: &[AnyId],
    with: &Replacement,
) -> Result<(Vec<Created>, &'static str), EditError> {
    check_items(txn.doc(), items)?;
    let mut report = ReplicateReport::default();
    let mut lost: Vec<Id<Relationship>> = Vec::new();
    let relationships: Vec<(Id<Relationship>, Relationship)> =
        txn.doc().relationships().iter().map(|(id, r)| (id, r.clone())).collect();
    for item in items {
        match (item.kind, with) {
            (Kind::Body, Replacement::Solid(solid)) => {
                if !solid.is_valid() {
                    return Err(EditError::BadValue);
                }
                let id = Id::<Body>::from_raw(item.raw);
                txn.modify(id, |b: &mut Body, _| b.solid = solid.clone())?;
                for (rid, relationship) in &relationships {
                    let touches =
                        |s: &Selection| s.occurrence == Occurrence::Item(*item) && !fits(solid, &s.geometry);
                    if touches(&relationship.first)
                        || touches(&relationship.second)
                        || relationship.third().is_some_and(touches)
                    {
                        lost.push(*rid);
                    }
                }
            }
            (Kind::Assembly, Replacement::Assembly(assembly)) => {
                if !assembly.is_valid() {
                    return Err(EditError::BadValue);
                }
                let id = Id::<Assembly>::from_raw(item.raw);
                let old_parts: Vec<String> = txn
                    .doc()
                    .get(id)
                    .ok_or(EditError::BadValue)?
                    .parts
                    .iter()
                    .map(|p| p.name.clone())
                    .collect();
                // Old part index → new index of the part of the same name (the first one), if any.
                let remap = |index: u32| -> Option<u32> {
                    let name = old_parts.get(index as usize)?;
                    let at = assembly.parts.iter().position(|p| &p.name == name)?;
                    u32::try_from(at).ok()
                };
                txn.modify(id, |a: &mut Assembly, _| {
                    a.name = assembly.name.clone();
                    a.file = assembly.file.clone();
                    a.parts = assembly.parts.clone();
                    a.meshes = assembly.meshes.clone();
                })?;
                for (rid, relationship) in &relationships {
                    let mut moved = relationship.clone();
                    let mut gone = false;
                    let mut fix = |s: &mut Selection| {
                        if let Occurrence::Part { assembly: a, index } = s.occurrence
                            && a == id
                        {
                            match remap(index) {
                                Some(new) => s.occurrence = Occurrence::Part { assembly: a, index: new },
                                None => gone = true,
                            }
                        }
                    };
                    fix(&mut moved.first);
                    fix(&mut moved.second);
                    match &mut moved.kind {
                        RelationKind::Symmetry { plane: s }
                        | RelationKind::Angle { solution: AngleSolution::Explicit(s), .. } => fix(s),
                        _ => {}
                    }
                    if gone {
                        lost.push(*rid);
                    } else if moved != *relationship {
                        txn.modify(*rid, |r: &mut Relationship, _| *r = moved.clone())?;
                    }
                }
                // Part states (hidden, suppressed, …) of parts that are gone are dropped when the
                // timeline is next written; positions of the rest are kept by index.
            }
            _ => return Err(EditError::BadValue),
        }
    }
    lost.sort();
    lost.dedup();
    for id in &lost {
        if !txn.doc().is_inactive(id.erase()) {
            suppress(txn, *id)?;
            report.suppressed.push(*id);
        }
    }
    let candidates: Vec<Id<Relationship>> = relationships
        .iter()
        .map(|(id, _)| *id)
        .filter(|id| {
            let r = relationships.iter().find(|(i, _)| i == id).map(|(_, r)| r);
            r.is_some_and(|r| [&r.first, &r.second].iter().any(|s| items.contains(&s.occurrence.owner())))
        })
        .collect();
    solve_suppressing(txn, &candidates, &mut report.suppressed)?;
    // Touch the timeline so that states of parts that no longer exist are dropped.
    crate::cell::set_timeline(txn, |_| {})?;
    Ok((Vec::new(), if items.len() > 1 { "Replace all" } else { "Replace component" }))
}
