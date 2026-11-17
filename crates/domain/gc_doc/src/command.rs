//! Edits as data, and the function that applies them.

use std::collections::{BTreeMap, BTreeSet};

use gc_core::{Entity, Id, IdAlloc};
use gc_expr::{Quantity, Scope, Unit};
use gc_sketch::{
    AddResult, Curve, DimKind, Dimension, ExternalRef, GeoConstraint, Item, SkPoint, Sketch, SketchError,
    Space, Status,
};
use serde::{Deserialize, Serialize};

use crate::document::{ChangeSet, DocDelta, Document, Frame, Param};
use crate::txn::{DocError, Txn};

/// What the end of a new line was snapped to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EndSnap {
    /// An existing point the end coincides with.
    pub point: Option<Id<SkPoint>>,
    /// An existing curve the end lies on.
    pub curve: Option<Id<Curve>>,
}

/// An edit of one sketch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SketchOp {
    /// Adds a free point.
    AddPoint {
        /// Position.
        pos: [f64; 3],
    },
    /// Adds a line. Snaps and the inferred direction become constraints where they are consistent and
    /// are silently left out where they are not.
    AddLine {
        /// Start.
        a: [f64; 3],
        /// End.
        b: [f64; 3],
        /// What the start was snapped to.
        snap_a: EndSnap,
        /// What the end was snapped to.
        snap_b: EndSnap,
        /// `Some(true)` when the line was drawn horizontal, `Some(false)` when vertical.
        horizontal: Option<bool>,
    },
    /// Adds a circle.
    AddCircle {
        /// Centre.
        centre: [f64; 3],
        /// Radius.
        radius: f64,
    },
    /// Adds an arc, counter-clockwise from `start` to `end`.
    AddArc {
        /// Centre.
        centre: [f64; 3],
        /// Start point.
        start: [f64; 3],
        /// End point.
        end: [f64; 3],
    },
    /// Adds an axis-aligned rectangle between two opposite corners: four lines, coincident corners,
    /// horizontal and vertical sides.
    AddRectangle {
        /// First corner.
        a: [f64; 2],
        /// Opposite corner.
        b: [f64; 2],
        /// What the first corner was snapped to.
        snap_a: EndSnap,
    },
    /// Adds a geometric constraint. Refused when redundant or conflicting.
    AddConstraint(GeoConstraint),
    /// Adds a driving dimension with the value of `source`. Refused when redundant or conflicting.
    AddDimension {
        /// What to measure.
        kind: DimKind,
        /// The expression typed by the user.
        source: String,
        /// Where the text is placed.
        text_pos: [f64; 2],
    },
    /// Adds a reference dimension, which only displays a measurement.
    AddReferenceDimension {
        /// What to measure.
        kind: DimKind,
        /// Where the text is placed.
        text_pos: [f64; 2],
    },
    /// Changes the expression of a driving dimension.
    SetDimension {
        /// The dimension.
        id: Id<Dimension>,
        /// The new expression.
        source: String,
    },
    /// Moves the text of a dimension.
    MoveDimensionText {
        /// The dimension.
        id: Id<Dimension>,
        /// The new position.
        text_pos: [f64; 2],
    },
    /// Writes coordinates that an interactive drag produced. They must satisfy the constraints.
    SetCoords {
        /// New point positions.
        points: Vec<(Id<SkPoint>, [f64; 3])>,
        /// New circle radii.
        radii: Vec<(Id<Curve>, f64)>,
    },
    /// Marks a curve as construction geometry or as regular geometry.
    SetConstruction {
        /// The curve.
        curve: Id<Curve>,
        /// The new state.
        construction: bool,
    },
    /// Deletes items and everything that depends on them.
    Delete(Vec<Item>),
}

/// An edit of the document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    /// Adds a named parameter. `angle` selects degrees instead of millimetres for bare numbers.
    AddParam {
        /// The name.
        name: String,
        /// The expression.
        source: String,
        /// True for an angle.
        angle: bool,
    },
    /// Changes a parameter's expression. Every dimension that uses it follows.
    SetParam {
        /// The parameter.
        id: Id<Param>,
        /// The new expression.
        source: String,
    },
    /// Deletes a parameter that nothing uses.
    DeleteParam {
        /// The parameter.
        id: Id<Param>,
    },
    /// Creates an empty sketch on a frame, with the frame's origin as a fixed reference point.
    CreateSketch {
        /// Name shown in the browser.
        name: String,
        /// The frame whose XY plane (2D) or space (3D) the sketch uses.
        frame: Id<Frame>,
        /// True for a 3D sketch.
        spatial: bool,
    },
    /// Deletes a sketch.
    DeleteSketch {
        /// The sketch.
        id: Id<Sketch>,
    },
    /// Edits a sketch.
    Sketch {
        /// The sketch.
        sketch: Id<Sketch>,
        /// The edit.
        op: SketchOp,
    },
    /// Edits the robot cell.
    Cell(crate::cell::CellOp),
}

/// Something a command created.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Created {
    /// A parameter.
    Param(Id<Param>),
    /// A sketch.
    Sketch(Id<Sketch>),
    /// A sketch point.
    Point(Id<SkPoint>),
    /// A sketch curve.
    Curve(Id<Curve>),
    /// A geometric constraint.
    Constraint(Id<GeoConstraint>),
    /// A dimension.
    Dimension(Id<Dimension>),
    /// A body.
    Body(Id<crate::cell::Body>),
    /// A robot.
    Robot(Id<crate::cell::Robot>),
    /// A waypoint.
    Waypoint(Id<crate::cell::Waypoint>),
    /// A path.
    Path(Id<crate::cell::MotionPath>),
    /// A work frame.
    Frame(Id<crate::cell::WorkFrame>),
    /// A work plane, axis or point.
    WorkFeature(Id<crate::design::WorkFeature>),
    /// A modelling feature.
    Feature(Id<crate::design::Feature>),
    /// An imported assembly.
    Assembly(Id<crate::design::Assembly>),
}

/// The result of a command that was applied.
#[derive(Debug)]
pub struct Applied {
    /// What to record in the history to make the command undoable.
    pub delta: DocDelta,
    /// The entities that changed.
    pub changed: ChangeSet,
    /// What the command created, in creation order.
    pub created: Vec<Created>,
    /// A short description for the undo menu.
    pub label: &'static str,
}

/// Why a command was refused. The document is unchanged.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum EditError {
    /// The document rejected the change.
    #[error(transparent)]
    Doc(#[from] DocError),
    /// The sketch rejected the change.
    #[error(transparent)]
    Sketch(#[from] SketchError),
    /// An expression could not be read or evaluated; the range is in bytes of the source.
    #[error("{message}")]
    Expr {
        /// What is wrong.
        message: String,
        /// Start of the offending part.
        start: usize,
        /// End of the offending part.
        end: usize,
    },
    /// The constraint or dimension follows from these items. A dimension may be added as a reference
    /// dimension instead.
    #[error("this is already implied by existing constraints")]
    Redundant(Vec<Item>),
    /// The constraint or dimension contradicts these items.
    #[error("this conflicts with existing constraints")]
    Conflicting(Vec<Item>),
    /// The name is not a valid parameter name, or is taken.
    #[error("`{0}` cannot be used as a parameter name")]
    BadName(String),
    /// Parameters refer to each other in a circle.
    #[error("parameters refer to each other in a circle")]
    Cycle,
    /// Something else still uses this and it cannot be deleted.
    #[error("this is still in use")]
    InUse,
    /// A work frame's origin must be fully constrained, so that the frame is where the dimensions say.
    #[error("the origin point is not fully constrained")]
    Underconstrained,
    /// The points that set a work frame's axes coincide with its origin or lie on one line with it.
    #[error("the axis points do not give a frame")]
    DegenerateFrame,
    /// A value is not a finite number, not positive where it must be, or of the wrong length.
    #[error("a value is out of range")]
    BadValue,
}

/// Unit of a bare number in a dimension field.
fn field_unit(kind: &DimKind) -> Unit {
    if matches!(kind, DimKind::Angle { .. }) { Unit::Deg } else { Unit::Mm }
}

fn evaluate(source: &str, scope: &dyn Scope, unit: Unit) -> Result<f64, EditError> {
    let expr = gc_expr::parse(source).map_err(|e| EditError::Expr {
        message: e.kind.to_string(),
        start: e.span.start,
        end: e.span.end,
    })?;
    expr.eval_as(scope, unit).map_err(|e| EditError::Expr {
        message: e.kind.to_string(),
        start: e.span.start,
        end: e.span.end,
    })
}

/// Parameter values by name.
fn scope_of(doc: &Document) -> BTreeMap<String, Quantity> {
    doc.params()
        .iter()
        .map(|(_, p)| {
            let quantity = if p.unit.dim() == Unit::Deg.dim() {
                Quantity::angle(p.value)
            } else {
                Quantity::length(p.value)
            };
            (p.name.clone(), quantity)
        })
        .collect()
}

/// The names an expression refers to (empty when it does not parse).
fn names_in(source: &str) -> BTreeSet<String> {
    gc_expr::parse(source).map(|e| e.names().into_iter().map(str::to_owned).collect()).unwrap_or_default()
}

/// Re-evaluates every parameter in dependency order, then every driving dimension, and lets the sketches
/// follow. Any failure fails the whole command.
fn propagate(txn: &mut Txn<'_>) -> Result<(), EditError> {
    let ids_by_name: BTreeMap<String, Id<Param>> =
        txn.doc().params().iter().map(|(id, p)| (p.name.clone(), id)).collect();
    let deps: BTreeMap<Id<Param>, BTreeSet<Id<Param>>> = txn
        .doc()
        .params()
        .iter()
        .map(|(id, p)| (id, names_in(&p.source).iter().filter_map(|n| ids_by_name.get(n).copied()).collect()))
        .collect();
    let (order, blocked) = gc_expr::eval_order(&deps);
    if !blocked.is_empty() {
        return Err(EditError::Cycle);
    }
    for id in order {
        let Some(param) = txn.doc().get(id) else { continue };
        let value = evaluate(&param.source, &scope_of(txn.doc()), param.unit)?;
        if value.to_bits() != param.value.to_bits() {
            txn.modify(id, |p: &mut Param, _| p.value = value)?;
        }
    }
    let scope = scope_of(txn.doc());
    let sketches: Vec<Id<Sketch>> = txn.doc().sketches().iter().map(|(id, _)| id).collect();
    for sketch_id in sketches {
        let Some(sketch) = txn.doc().get(sketch_id) else { continue };
        let mut updates = Vec::new();
        for (id, dimension) in sketch.dimensions.iter().filter(|(_, d)| d.driving) {
            let value = evaluate(&dimension.source, &scope, field_unit(&dimension.kind))?;
            if value.to_bits() != dimension.value.to_bits() {
                updates.push((*id, dimension.source.clone(), value));
            }
        }
        for (id, source, value) in updates {
            txn.modify(sketch_id, |s: &mut Sketch, _| s.set_dimension(id, &source, value))??;
        }
    }
    Ok(())
}

fn added<T: Entity>(result: AddResult<T>) -> Result<Id<T>, EditError> {
    match result {
        AddResult::Added { id, .. } => Ok(id),
        AddResult::Redundant(with) => Err(EditError::Redundant(with)),
        AddResult::Conflicting(with) => Err(EditError::Conflicting(with)),
    }
}

/// Adds a constraint that was inferred from how something was drawn; dropped unless it is accepted.
fn infer(sketch: &mut Sketch, ids: &mut IdAlloc, constraint: GeoConstraint, created: &mut Vec<Created>) {
    if let Ok(AddResult::Added { id, .. }) = sketch.try_add_constraint(ids, constraint) {
        created.push(Created::Constraint(id));
    }
}

fn sketch_op(
    sketch: &mut Sketch,
    ids: &mut IdAlloc,
    op: &SketchOp,
    scope: &dyn Scope,
) -> Result<(Vec<Created>, &'static str), EditError> {
    let mut created = Vec::new();
    let label = match op {
        SketchOp::AddPoint { pos } => {
            created.push(Created::Point(sketch.add_point(ids, *pos)));
            "Point"
        }
        SketchOp::AddLine { a, b, snap_a, snap_b, horizontal } => {
            let line = sketch.add_line(ids, *a, *b);
            created.push(Created::Curve(line));
            if let Some((start, end)) = sketch.line(line) {
                for (end_point, snap) in [(start, snap_a), (end, snap_b)] {
                    if let Some(point) = snap.point {
                        infer(sketch, ids, GeoConstraint::Coincident(point, end_point), &mut created);
                    } else if let Some(curve) = snap.curve {
                        infer(sketch, ids, GeoConstraint::PointOnCurve(end_point, curve), &mut created);
                    }
                }
                match horizontal {
                    Some(true) => infer(sketch, ids, GeoConstraint::Horizontal(start, end), &mut created),
                    Some(false) => infer(sketch, ids, GeoConstraint::Vertical(start, end), &mut created),
                    None => {}
                }
            }
            "Line"
        }
        SketchOp::AddCircle { centre, radius } => {
            created.push(Created::Curve(sketch.add_circle(ids, *centre, *radius)?));
            "Circle"
        }
        SketchOp::AddArc { centre, start, end } => {
            created.push(Created::Curve(sketch.add_arc(ids, *centre, *start, *end)?));
            "Arc"
        }
        SketchOp::AddRectangle { a, b, snap_a } => {
            let corners = [[a[0], a[1], 0.0], [b[0], a[1], 0.0], [b[0], b[1], 0.0], [a[0], b[1], 0.0]];
            let lines: Vec<Id<Curve>> =
                (0..4).map(|i| sketch.add_line(ids, corners[i], corners[(i + 1) % 4])).collect();
            let ends: Vec<(Id<SkPoint>, Id<SkPoint>)> =
                lines.iter().filter_map(|l| sketch.line(*l)).collect();
            created.extend(lines.iter().map(|l| Created::Curve(*l)));
            for i in 0..ends.len() {
                let next = ends[(i + 1) % ends.len()];
                let id =
                    added(sketch.try_add_constraint(ids, GeoConstraint::Coincident(ends[i].1, next.0))?)?;
                created.push(Created::Constraint(id));
            }
            for (i, (start, end)) in ends.iter().enumerate() {
                let constraint = if i % 2 == 0 {
                    GeoConstraint::Horizontal(*start, *end)
                } else {
                    GeoConstraint::Vertical(*start, *end)
                };
                created.push(Created::Constraint(added(sketch.try_add_constraint(ids, constraint)?)?));
            }
            if let (Some(point), Some((corner, _))) = (snap_a.point, ends.first()) {
                infer(sketch, ids, GeoConstraint::Coincident(point, *corner), &mut created);
            } else if let (Some(curve), Some((corner, _))) = (snap_a.curve, ends.first()) {
                infer(sketch, ids, GeoConstraint::PointOnCurve(*corner, curve), &mut created);
            }
            "Rectangle"
        }
        SketchOp::AddConstraint(constraint) => {
            created.push(Created::Constraint(added(sketch.try_add_constraint(ids, *constraint)?)?));
            "Constraint"
        }
        SketchOp::AddDimension { kind, source, text_pos } => {
            let value = evaluate(source, scope, field_unit(kind))?;
            let result = sketch.try_add_dimension(ids, *kind, source, value, *text_pos)?;
            created.push(Created::Dimension(added(result)?));
            "Dimension"
        }
        SketchOp::AddReferenceDimension { kind, text_pos } => {
            created.push(Created::Dimension(sketch.add_reference_dimension(ids, *kind, *text_pos)));
            "Reference dimension"
        }
        SketchOp::SetDimension { id, source } => {
            let kind = sketch.dimensions.get(id).ok_or(SketchError::MissingItem)?.kind;
            let value = evaluate(source, scope, field_unit(&kind))?;
            sketch.set_dimension(*id, source, value)?;
            "Edit dimension"
        }
        SketchOp::MoveDimensionText { id, text_pos } => {
            sketch.dimensions.get_mut(id).ok_or(SketchError::MissingItem)?.text_pos = *text_pos;
            "Move dimension"
        }
        SketchOp::SetCoords { points, radii } => {
            for (id, pos) in points {
                sketch.points.get_mut(id).ok_or(SketchError::MissingPoint)?.pos = *pos;
            }
            for (id, radius) in radii {
                match sketch.curves.get_mut(id).map(|c| &mut c.geom) {
                    Some(gc_sketch::CurveGeom::Circle { r, .. }) => *r = *radius,
                    Some(_) => {}
                    None => return Err(SketchError::MissingCurve.into()),
                }
            }
            "Drag"
        }
        SketchOp::SetConstruction { curve, construction } => {
            sketch.curves.get_mut(curve).ok_or(SketchError::MissingCurve)?.construction = *construction;
            "Construction"
        }
        SketchOp::Delete(items) => {
            sketch.delete(items);
            "Delete"
        }
    };
    // Whatever the edit was, the sketch must end up consistent.
    let report = sketch.solve()?;
    if report.status != Status::Solved {
        return Err(SketchError::Unsolved(report.status).into());
    }
    Ok((created, label))
}

fn run(txn: &mut Txn<'_>, command: &Command) -> Result<(Vec<Created>, &'static str), EditError> {
    match command {
        Command::AddParam { name, source, angle } => {
            let taken = txn.doc().params().iter().any(|(_, p)| &p.name == name);
            if taken || !gc_expr::is_valid_param_name(name) {
                return Err(EditError::BadName(name.clone()));
            }
            let unit = if *angle { Unit::Deg } else { Unit::Mm };
            let value = evaluate(source, &scope_of(txn.doc()), unit)?;
            let id = txn.insert(Param { name: name.clone(), source: source.clone(), unit, value });
            propagate(txn)?;
            Ok((vec![Created::Param(id)], "Add parameter"))
        }
        Command::SetParam { id, source } => {
            // Parse first so that a typo is reported against this field.
            gc_expr::parse(source).map_err(|e| EditError::Expr {
                message: e.kind.to_string(),
                start: e.span.start,
                end: e.span.end,
            })?;
            txn.modify(*id, |p: &mut Param, _| p.source.clone_from(source))?;
            propagate(txn)?;
            Ok((Vec::new(), "Edit parameter"))
        }
        Command::DeleteParam { id } => {
            let name = txn.doc().get(*id).ok_or(DocError::Missing(id.erase()))?.name.clone();
            let used_by_param = txn
                .doc()
                .params()
                .iter()
                .any(|(other, p)| other != *id && names_in(&p.source).contains(&name));
            let used_by_dimension = txn.doc().sketches().iter().any(|(_, s)| {
                s.dimensions.values().any(|d| d.driving && names_in(&d.source).contains(&name))
            });
            if used_by_param || used_by_dimension {
                return Err(EditError::InUse);
            }
            txn.remove(*id)?;
            Ok((Vec::new(), "Delete parameter"))
        }
        Command::CreateSketch { name, frame, spatial } => {
            let frame = frame.erase();
            let mut sketch = Sketch::new(if *spatial { Space::Spatial(frame) } else { Space::Planar(frame) });
            sketch.name.clone_from(name);
            let id = txn.insert(sketch);
            txn.modify(id, |s: &mut Sketch, ids| {
                s.add_external(ids, ExternalRef::FrameOrigin(frame), [0.0; 3]);
            })?;
            Ok((vec![Created::Sketch(id)], "New sketch"))
        }
        Command::DeleteSketch { id } => {
            txn.remove(*id)?;
            Ok((Vec::new(), "Delete sketch"))
        }
        Command::Sketch { sketch, op } => {
            let scope = scope_of(txn.doc());
            txn.modify(*sketch, |s: &mut Sketch, ids| sketch_op(s, ids, op, &scope))?
        }
        Command::Cell(op) => crate::cell::run(txn, op),
    }
}

/// Applies a command. On success the document is changed and the result says how to undo it. On failure
/// the document is exactly as before.
pub fn apply(doc: &mut Document, command: &Command) -> Result<Applied, EditError> {
    let mut txn = Txn::new(doc);
    let (created, label) = run(&mut txn, command)?; // an early return drops `txn`, which rolls back
    let (delta, changed) = txn.commit()?;
    Ok(Applied { delta, changed, created, label })
}
