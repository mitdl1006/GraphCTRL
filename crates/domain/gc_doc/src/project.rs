//! Project Geometry (docs/ux/inventor-assembly-parity.md, P1-5): points and edges of components and work
//! features projected onto a sketch as reference geometry that follows its source.
//!
//! Inventor 2026 help: To Project Geometry into a 2D Sketch (GUID-B8D8B07B-86F2-4CB7-AB21-3131D94AD1CC),
//! About Projecting Sketch Geometry (GUID-B61431DA-FABC-41EE-945C-6E54B4806582): projected geometry is linked
//! to the original and updates when it changes; when the source is deleted it becomes fixed sketch curves
//! with constraints and dimensions kept; Shift+click projects as construction when the toggle is off (and
//! the other way round).

use gc_core::{AnyId, Id, IdAlloc, Kind};
use gc_math::{DPose3, DVec3};
use gc_sketch::{Curve, CurveGeom, ExternalRef, GeoConstraint, PointKind, SkPoint, Sketch, Space};
use serde::{Deserialize, Serialize};

use crate::command::{Created, EditError};
use crate::design::{Occurrence, WorkFeature};
use crate::document::Document;
use crate::placement::WorkShape;
use crate::txn::Txn;

/// Half the length of a projected axis, metres (an axis has no ends; its projection is a reference line
/// through the point, long enough to constrain to).
const AXIS_HALF_LENGTH: f64 = 0.1;

/// What to project.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ProjectSource {
    /// A point fixed in a component's frame: a vertex, or `[0; 3]` for its origin's centre point.
    Point {
        /// The component (a part or robot link of one included).
        occurrence: Occurrence,
        /// Where, in its frame (metres).
        local: [f64; 3],
    },
    /// A straight edge of a component, by its two ends in the component's frame.
    Edge {
        /// The component.
        occurrence: Occurrence,
        /// One end.
        a: [f64; 3],
        /// The other end.
        b: [f64; 3],
    },
    /// An axis of a component's origin folder (0 = X, 1 = Y, 2 = Z), as a reference line.
    OriginAxis {
        /// The component.
        occurrence: Occurrence,
        /// Which axis.
        axis: u8,
    },
    /// A work point.
    WorkPoint(Id<WorkFeature>),
    /// A work axis, as a reference line.
    WorkAxis(Id<WorkFeature>),
}

fn nm(v: f64) -> i64 {
    #[allow(clippy::cast_possible_truncation, reason = "nanometres of positions within a cell fit in i64")]
    let n = (v * 1e9).round() as i64;
    n
}

#[allow(clippy::cast_precision_loss, reason = "nanometres within a cell are exact enough in f64")]
fn metres(n: i64) -> f64 {
    n as f64 * 1e-9
}

/// The reference a projected point keeps: its owner, part and position in that frame.
fn reference(occurrence: Occurrence, local: [f64; 3]) -> ExternalRef {
    let (owner, part) = match occurrence {
        Occurrence::Item(item) => (item, 0),
        Occurrence::Part { assembly, index } => (assembly.erase(), index + 1),
        Occurrence::Link { robot, index } => (robot.erase(), index + 1),
    };
    ExternalRef::Projected { owner, part, local_nm: local.map(nm) }
}

impl Document {
    /// Where a projected point's source is in the world now; `None` when the source is gone.
    fn projected_world(&self, owner: AnyId, part: u32, local_nm: [i64; 3]) -> Option<DVec3> {
        let local = DVec3::from_array(local_nm.map(metres));
        if owner.kind == Kind::WorkFeature {
            return match self.work_shape(Id::from_raw(owner.raw))? {
                WorkShape::Point(p) => Some(p + local),
                WorkShape::Axis { point, direction } => Some(point + direction.normalize_or_zero() * local.z),
                WorkShape::Plane(pose) => Some(pose.translation + pose.rotation * local),
            };
        }
        let occurrence = match (owner.kind, part) {
            (_, 0) => Occurrence::Item(owner),
            (Kind::Assembly, n) => Occurrence::Part { assembly: Id::from_raw(owner.raw), index: n - 1 },
            (Kind::RobotInstance, n) => Occurrence::Link { robot: Id::from_raw(owner.raw), index: n - 1 },
            _ => return None,
        };
        if !self.has_occurrence(occurrence) {
            return None;
        }
        let pose = self.component_origin(occurrence)?.pose;
        Some(pose.translation + pose.rotation * local)
    }

    /// The world pose of a sketch's frame.
    fn sketch_frame(&self, sketch: &Sketch) -> Option<DPose3> {
        let (Space::Planar(frame) | Space::Spatial(frame)) = sketch.space;
        self.frame_world_pose(Id::from_raw(frame.raw))
    }
}

/// A world point in a sketch's coordinates, projected onto the plane for a planar sketch.
fn on_sketch(frame: &DPose3, planar: bool, world: DVec3) -> [f64; 3] {
    let local = frame.rotation.inverse() * (world - frame.translation);
    if planar { [local.x, local.y, 0.0] } else { local.to_array() }
}

/// The projected points a source gives (one, or two for a reference line), as references.
fn references(doc: &Document, source: &ProjectSource) -> Result<(Vec<ExternalRef>, bool), EditError> {
    let exists = |o: Occurrence| if doc.has_occurrence(o) { Ok(()) } else { Err(EditError::BadValue) };
    Ok(match *source {
        ProjectSource::Point { occurrence, local } => {
            exists(occurrence)?;
            (vec![reference(occurrence, local)], false)
        }
        ProjectSource::Edge { occurrence, a, b } => {
            exists(occurrence)?;
            (vec![reference(occurrence, a), reference(occurrence, b)], true)
        }
        ProjectSource::OriginAxis { occurrence, axis } => {
            exists(occurrence)?;
            let mut end = [0.0; 3];
            end[usize::from(axis.min(2))] = AXIS_HALF_LENGTH;
            let start = end.map(|v| -v);
            (vec![reference(occurrence, start), reference(occurrence, end)], true)
        }
        ProjectSource::WorkPoint(id) => match doc.work_shape(id) {
            Some(WorkShape::Point(_)) => {
                (vec![ExternalRef::Projected { owner: id.erase(), part: 0, local_nm: [0; 3] }], false)
            }
            _ => return Err(EditError::BadValue),
        },
        ProjectSource::WorkAxis(id) => match doc.work_shape(id) {
            Some(WorkShape::Axis { .. }) => {
                let end =
                    |z: f64| ExternalRef::Projected { owner: id.erase(), part: 0, local_nm: [0, 0, nm(z)] };
                (vec![end(-AXIS_HALF_LENGTH), end(AXIS_HALF_LENGTH)], true)
            }
            _ => return Err(EditError::BadValue),
        },
    })
}

/// Projects a source onto a sketch: reference points (and a reference line between two), which the
/// solver keeps where the source is.
pub(crate) fn project(
    txn: &mut Txn<'_>,
    sketch: Id<Sketch>,
    source: &ProjectSource,
    construction: bool,
) -> Result<(Vec<Created>, &'static str), EditError> {
    let doc = txn.doc();
    let (refs, line) = references(doc, source)?;
    let current = doc.get(sketch).ok_or(EditError::BadValue)?;
    let frame = doc.sketch_frame(current).ok_or(EditError::BadValue)?;
    let planar = !current.is_spatial();
    let mut placed = Vec::new();
    for r in refs {
        let ExternalRef::Projected { owner, part, local_nm } = r else { continue };
        let world = doc.projected_world(owner, part, local_nm).ok_or(EditError::BadValue)?;
        placed.push((r, on_sketch(&frame, planar, world)));
    }
    if line
        && placed.len() == 2
        && (DVec3::from_array(placed[0].1) - DVec3::from_array(placed[1].1)).length() < 1e-9
    {
        // An edge or axis seen end-on projects to a point.
        placed.truncate(1);
    }
    txn.modify(sketch, |s: &mut Sketch, ids| {
        let mut created = Vec::new();
        let points: Vec<Id<SkPoint>> = placed.iter().map(|(r, pos)| s.add_external(ids, *r, *pos)).collect();
        created.extend(points.iter().map(|p| Created::Point(*p)));
        if let [a, b] = points[..] {
            let id: Id<Curve> = ids.alloc();
            s.curves.insert(id, Curve { geom: CurveGeom::Line { a, b }, construction });
            created.push(Created::Curve(id));
        }
        Ok((created, "Project geometry"))
    })?
}

/// A projected point's new position in its sketch, or `None` when its source is gone.
type PointUpdate = (Id<SkPoint>, Option<[f64; 3]>);

/// Moves projected points to where their sources are now; a point whose source is gone becomes an
/// ordinary point held by a Fix constraint (the projection turns into fixed sketch curves). Run after
/// every command.
pub(crate) fn refresh(txn: &mut Txn<'_>) -> Result<(), EditError> {
    let doc = txn.doc();
    let mut updates: Vec<(Id<Sketch>, Vec<PointUpdate>)> = Vec::new();
    for (id, sketch) in doc.sketches().iter() {
        let mut changes = Vec::new();
        let Some(frame) = doc.sketch_frame(sketch) else { continue };
        let planar = !sketch.is_spatial();
        for (point_id, point) in &sketch.points {
            let PointKind::External(ExternalRef::Projected { owner, part, local_nm }) = point.kind else {
                continue;
            };
            match doc.projected_world(owner, part, local_nm) {
                Some(world) => {
                    let pos = on_sketch(&frame, planar, world);
                    let moved = pos.iter().zip(&point.pos).any(|(a, b)| (a - b).abs() > 1e-12);
                    if moved {
                        changes.push((*point_id, Some(pos)));
                    }
                }
                None => changes.push((*point_id, None)),
            }
        }
        if !changes.is_empty() {
            updates.push((id, changes));
        }
    }
    for (sketch, changes) in updates {
        txn.modify(sketch, |s: &mut Sketch, ids: &mut IdAlloc| {
            for (point, pos) in &changes {
                let Some(p) = s.points.get_mut(point) else { continue };
                match pos {
                    Some(pos) => p.pos = *pos,
                    None => {
                        p.kind = PointKind::Free;
                        let fix: Id<GeoConstraint> = ids.alloc();
                        s.constraints.insert(fix, GeoConstraint::Fix(*point));
                    }
                }
            }
            // The geometry tied to the projection follows; a sketch that cannot follow keeps its other
            // coordinates and shows as needing attention.
            let _report = s.solve();
        })?;
    }
    Ok(())
}
