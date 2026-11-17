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
    /// A second curve the end lies on (the end is at an intersection).
    #[serde(default)]
    pub also_on: Option<Id<Curve>>,
    /// The line whose midpoint the end is.
    #[serde(default)]
    pub midpoint_of: Option<Id<Curve>>,
    /// An existing point the end is level with (`true`) or plumb with (`false`), from an alignment line.
    #[serde(default)]
    pub aligned: Option<(Id<SkPoint>, bool)>,
    /// A circle or arc the new curve touches there (a line end on a circle where the line is tangent to
    /// it, or a circle's rim on another curve).
    #[serde(default)]
    pub tangent_to: Option<Id<Curve>>,
}

/// Which measurement of newly drawn geometry a typed value becomes (see [`SketchOp::WithDimensions`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DimSlot {
    /// The length of the first line created.
    Length,
    /// The diameter of the first circle created.
    Diameter,
    /// The radius of the first arc created.
    Radius,
    /// The length of the first side of a rectangle.
    Width,
    /// The length of the second side of a rectangle.
    Height,
}

/// An edit of one sketch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SketchOp {
    /// Adds a free point.
    AddPoint {
        /// Position.
        pos: [f64; 3],
    },
    /// The Point command: places a standalone point without constraints, a centre point (Inventor's
    /// default, a cross) or a sketch point (a dot) (Inventor 2026 help, To Work with Points,
    /// GUID-D4A4288F-32F1-4E23-A382-930EBA152E87).
    PlacePoint {
        /// Position.
        pos: [f64; 3],
        /// A centre point rather than a sketch point.
        center: bool,
    },
    /// The Center Point toggle on selected standalone points (curve ends and centres are refused).
    SetCenterPoint {
        /// The points.
        points: Vec<Id<SkPoint>>,
        /// Centre points, or sketch points.
        center: bool,
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
        /// What the centre was snapped to.
        #[serde(default)]
        centre_snap: EndSnap,
        /// What the rim point was snapped to: on another curve, the circle is made tangent to it.
        #[serde(default)]
        rim_snap: EndSnap,
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
    /// Marks several curves as construction geometry or as regular geometry, as one undo step.
    SetConstructionOf {
        /// The curves.
        curves: Vec<Id<Curve>>,
        /// The new state.
        construction: bool,
    },
    /// Deletes items and everything that depends on them.
    Delete(Vec<Item>),
    /// Renames a dimension's model parameter (`d0` → `HGHT`) and the references to it in parameters and
    /// dimensions. Refused when the name is malformed or a parameter or dimension has it.
    RenameDimension {
        /// The dimension.
        id: Id<Dimension>,
        /// The new name.
        name: String,
    },
    /// Projects geometry of a component or work feature onto the sketch as reference geometry that follows
    /// it (Project Geometry): a point, or a reference line between two.
    Project {
        /// What to project.
        source: crate::project::ProjectSource,
        /// Project as construction geometry.
        construction: bool,
    },
    /// Removes the piece of a curve around `at` up to the nearest crossing curves on either side; the
    /// cut ends are tied to the crossing curves. A curve that nothing crosses is deleted.
    Trim {
        /// The curve.
        curve: Id<Curve>,
        /// Where it was clicked (on the piece to remove).
        at: [f64; 2],
    },
    /// Lengthens a line or arc at its end nearer `at` up to the first curve it meets, and ties the end to
    /// that curve. The end must be open.
    Extend {
        /// The curve.
        curve: Id<Curve>,
        /// Where it was clicked (near the end to lengthen).
        at: [f64; 2],
    },
    /// Rounds the corner between two lines with a tangent arc of the given radius (with a radius
    /// dimension), trimming or extending the lines to it. Each line keeps the side of the corner where it
    /// was clicked.
    Fillet {
        /// The two lines and where each was clicked.
        lines: [(Id<Curve>, [f64; 2]); 2],
        /// The radius as typed, e.g. `"5 mm"`.
        radius: String,
    },
    /// Adds fixed lines (sketch coordinates), as projected reference geometry: Project Cut Edges puts the
    /// edges where the sketch plane cuts the model into the sketch this way. One undo step.
    ProjectedLines {
        /// Start and end of each line.
        lines: Vec<([f64; 3], [f64; 3])>,
    },
    /// Applies an edit and marks the curves it creates as construction geometry (the Format panel's
    /// Construction toggle held while drawing). One undo step.
    AsConstruction(Box<SketchOp>),
    /// Adds an arc that starts at an existing curve end and continues it smoothly (tangent), ending at
    /// `end`: the line tool's end-point drag and the tangent arc tool.
    AddTangentArc {
        /// The end point the arc starts from.
        from: Id<SkPoint>,
        /// The curve it continues (a line or an arc ending at `from`).
        curve: Id<Curve>,
        /// Where the arc ends.
        end: [f64; 3],
        /// What the end was snapped to.
        snap_end: EndSnap,
    },
    /// Adds an arc from `a` to `b` through `through` (the three-point arc).
    AddArcThrough {
        /// First end.
        a: [f64; 3],
        /// Second end.
        b: [f64; 3],
        /// A point the arc passes through between them (sets the side and the radius).
        through: [f64; 3],
        /// What the first end was snapped to.
        snap_a: EndSnap,
        /// What the second end was snapped to.
        snap_b: EndSnap,
    },
    /// Adds a rectangle at any angle: `a` to `b` is one side, `c` sets the other side's length (the
    /// three-point rectangle). Four lines, coincident corners, one right angle and two pairs of parallel
    /// sides.
    AddRectangle3 {
        /// First corner.
        a: [f64; 3],
        /// Second corner (along the first side).
        b: [f64; 3],
        /// A point that sets the width.
        c: [f64; 3],
        /// What the first corner was snapped to.
        snap_a: EndSnap,
    },
    /// Applies a drawing edit and attaches values typed into the input boxes as driving dimensions of
    /// what it created (the HUD). One undo step; a value the sketch cannot take refuses the whole edit.
    WithDimensions {
        /// The drawing edit.
        op: Box<SketchOp>,
        /// What each value measures, and the value as typed (an expression).
        dims: Vec<(DimSlot, String)>,
    },
    /// Makes a dimension driving (it constrains the sketch) or driven (it only shows a measurement).
    /// Making one driving is refused when the sketch does not allow it (already constrained there).
    SetDriving {
        /// The dimension.
        id: Id<Dimension>,
        /// True for driving.
        driving: bool,
    },
}

/// An edit of the document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    /// Renames a model-tree item (sketch, body, robot, waypoint, path, work frame, work feature,
    /// feature, assembly). Refused when the name is empty or another item of the same kind has it.
    Rename {
        /// The item.
        item: gc_core::AnyId,
        /// The new name (leading and trailing spaces are dropped).
        name: String,
    },
    /// Shows or hides a model-tree item. Bodies, robots and assemblies keep it in their look; other
    /// items in the model tree's state.
    SetVisible {
        /// The item.
        item: gc_core::AnyId,
        /// True to show it.
        visible: bool,
    },
    /// Switches a state of a component of the assembly tree: grounded, visible, enabled, suppressed
    /// (suppressing bodies and robots is not supported yet).
    SetComponent {
        /// The component.
        occurrence: crate::design::Occurrence,
        /// Which state.
        flag: crate::design::ComponentFlag,
        /// The new value.
        on: bool,
    },
    /// Shows or hides elements of a component's origin folder (planes, axes, centre point).
    SetOriginVisible {
        /// The component.
        occurrence: crate::design::Occurrence,
        /// The elements.
        elements: Vec<crate::design::OriginElement>,
        /// True to show them.
        visible: bool,
    },
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
    /// Renames a parameter and the references to it in other parameters and in dimensions, as one step.
    /// Refused when the name is not a valid parameter name or another parameter has it.
    RenameParam {
        /// The parameter.
        id: Id<Param>,
        /// The new name.
        name: String,
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
    /// Creates an empty 2D sketch on a work plane or a face of an imported part. The sketch follows
    /// that plane when it changes; the plane cannot be deleted while the sketch uses it.
    CreateSketchOn {
        /// Name shown in the browser.
        name: String,
        /// What the sketch plane is attached to.
        source: crate::design::PlaneSource,
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
    /// An assembly constraint.
    Relationship(Id<crate::relate::Relationship>),
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
    /// Another item of the same kind already has this name.
    #[error("the name is already taken")]
    NameTaken,
    /// A value is not a finite number, not positive where it must be, or of the wrong length.
    #[error("a value is out of range")]
    BadValue,
    /// Assembly constraints cannot all be satisfied; these are not (ids and names, in the same order).
    #[error("these assembly constraints cannot be satisfied together: {}", names.join(", "))]
    Unsolvable {
        /// The constraints that are not satisfied.
        ids: Vec<Id<crate::relate::Relationship>>,
        /// Their names as shown in the Relationships folder.
        names: Vec<String>,
    },
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
    let mut scope: BTreeMap<String, Quantity> = doc
        .params()
        .iter()
        .map(|(_, p)| {
            let quantity = if p.unit.dim() == Unit::Deg.dim() {
                Quantity::angle(p.value)
            } else {
                Quantity::length(p.value)
            };
            (p.name.clone(), quantity)
        })
        .collect();
    // Model parameters: every named dimension, driving ones by their value, reference ones by what they
    // measure now.
    for (_, sketch) in doc.sketches().iter() {
        for dimension in sketch.dimensions.values().filter(|d| !d.name.is_empty()) {
            let value =
                if dimension.driving { Some(dimension.value) } else { sketch.measure(&dimension.kind) };
            let Some(value) = value else { continue };
            let quantity = if field_unit(&dimension.kind) == Unit::Deg {
                Quantity::angle(value)
            } else {
                Quantity::length(value)
            };
            scope.entry(dimension.name.clone()).or_insert(quantity);
        }
    }
    scope
}

/// Splits `NAME = expression` (Inventor: typing `HGHT = 5mm` in a dimension names its parameter HGHT):
/// the name and the expression; `None` when the text is a plain expression.
fn named_source(source: &str) -> Option<(String, String)> {
    let (name, rest) = source.split_once('=')?;
    let name = name.trim();
    gc_expr::is_valid_param_name(name).then(|| (name.to_owned(), rest.trim().to_owned()))
}

/// True when `name` is free for a parameter or dimension (other than `except`).
fn name_free(doc: &Document, name: &str, except: Option<(Id<Sketch>, Id<Dimension>)>) -> bool {
    let param = doc.params().iter().any(|(_, p)| p.name == name);
    let dimension = doc
        .sketches()
        .iter()
        .any(|(sid, s)| s.dimensions.iter().any(|(did, d)| d.name == name && Some((sid, *did)) != except));
    !param && !dimension
}

/// Gives every unnamed dimension the next free model parameter name `dN` (above the highest in use).
fn name_dimensions(txn: &mut Txn<'_>) -> Result<(), EditError> {
    let doc = txn.doc();
    let unnamed: Vec<(Id<Sketch>, Id<Dimension>)> = doc
        .sketches()
        .iter()
        .flat_map(|(sid, s)| {
            s.dimensions.iter().filter(|(_, d)| d.name.is_empty()).map(move |(did, _)| (sid, *did))
        })
        .collect();
    if unnamed.is_empty() {
        return Ok(());
    }
    let highest = doc
        .sketches()
        .iter()
        .flat_map(|(_, s)| s.dimensions.values().map(|d| d.name.clone()).collect::<Vec<_>>())
        .chain(doc.params().iter().map(|(_, p)| p.name.clone()))
        .filter_map(|n| n.strip_prefix('d').and_then(|k| k.parse::<u64>().ok()))
        .max();
    for (next, (sketch, dimension)) in (highest.map_or(0, |h| h + 1)..).zip(unnamed) {
        let name = format!("d{next}");
        txn.modify(sketch, |s: &mut Sketch, _| {
            if let Some(d) = s.dimensions.get_mut(&dimension) {
                d.name = name;
            }
        })?;
    }
    Ok(())
}

/// The names an expression refers to (empty when it does not parse).
/// `source` with every reference to the parameter `old` renamed to `new`; `None` when it does not refer
/// to `old` (or does not parse).
fn rename_in(source: &str, old: &str, new: &str) -> Option<String> {
    fn spans(expr: &gc_expr::Expr, old: &str, out: &mut Vec<(usize, usize)>) {
        match expr {
            gc_expr::Expr::Name { name, span } if name == old => out.push((span.start, span.end)),
            gc_expr::Expr::Neg(inner, _) => spans(inner, old, out),
            gc_expr::Expr::Binary { lhs, rhs, .. } => {
                spans(lhs, old, out);
                spans(rhs, old, out);
            }
            gc_expr::Expr::Call { args, .. } => args.iter().for_each(|a| spans(a, old, out)),
            _ => {}
        }
    }
    let expr = gc_expr::parse(source).ok()?;
    let mut found = Vec::new();
    spans(&expr, old, &mut found);
    if found.is_empty() {
        return None;
    }
    found.sort_unstable();
    let mut out = source.to_owned();
    for (start, end) in found.into_iter().rev() {
        out.replace_range(start..end, new);
    }
    Some(out)
}

fn names_in(source: &str) -> BTreeSet<String> {
    gc_expr::parse(source).map(|e| e.names().into_iter().map(str::to_owned).collect()).unwrap_or_default()
}

/// Re-evaluates every parameter in dependency order, then every driving dimension, and lets the sketches
/// follow. Any failure fails the whole command.
fn propagate(txn: &mut Txn<'_>) -> Result<(), EditError> {
    /// Something whose value comes from an expression.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
    enum Node {
        Param(Id<Param>),
        Dimension(Id<Sketch>, Id<Dimension>),
    }
    let doc = txn.doc();
    let mut by_name: BTreeMap<String, Node> = BTreeMap::new();
    let mut sources: Vec<(Node, String)> = Vec::new();
    for (id, p) in doc.params().iter() {
        by_name.insert(p.name.clone(), Node::Param(id));
        sources.push((Node::Param(id), p.source.clone()));
    }
    for (sid, s) in doc.sketches().iter() {
        for (did, d) in s.dimensions.iter().filter(|(_, d)| d.driving) {
            if !d.name.is_empty() {
                by_name.entry(d.name.clone()).or_insert(Node::Dimension(sid, *did));
            }
            sources.push((Node::Dimension(sid, *did), d.source.clone()));
        }
    }
    let deps: BTreeMap<Node, BTreeSet<Node>> = sources
        .iter()
        .map(|(node, source)| {
            (*node, names_in(source).iter().filter_map(|n| by_name.get(n).copied()).collect())
        })
        .collect();
    let (order, blocked) = gc_expr::eval_order(&deps);
    if !blocked.is_empty() {
        return Err(EditError::Cycle);
    }
    for node in order {
        match node {
            Node::Param(id) => {
                let Some(param) = txn.doc().get(id) else { continue };
                let value = evaluate(&param.source, &scope_of(txn.doc()), param.unit)?;
                if value.to_bits() != param.value.to_bits() {
                    txn.modify(id, |p: &mut Param, _| p.value = value)?;
                }
            }
            Node::Dimension(sketch_id, id) => {
                let Some(dimension) = txn.doc().get(sketch_id).and_then(|s| s.dimensions.get(&id)) else {
                    continue;
                };
                let source = dimension.source.clone();
                let value = evaluate(&source, &scope_of(txn.doc()), field_unit(&dimension.kind))?;
                if value.to_bits() != dimension.value.to_bits() {
                    txn.modify(sketch_id, |s: &mut Sketch, _| s.set_dimension(id, &source, value))??;
                }
            }
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

/// A value rounded to 6 significant decimals for writing into a dimension's source text.
fn round_for_display(value: f64) -> f64 {
    (value * 1e6).round() / 1e6
}

/// Constraints for an end point from what it was snapped to.
fn snap_constraint(
    sketch: &mut Sketch,
    ids: &mut IdAlloc,
    end: Id<SkPoint>,
    snap: &EndSnap,
    created: &mut Vec<Created>,
) {
    if let Some(point) = snap.point {
        infer(sketch, ids, GeoConstraint::Coincident(point, end), created);
        return;
    }
    if let Some(line) = snap.midpoint_of {
        infer(sketch, ids, GeoConstraint::Midpoint(end, line), created);
        return;
    }
    for curve in [snap.curve, snap.also_on].into_iter().flatten() {
        infer(sketch, ids, GeoConstraint::PointOnCurve(end, curve), created);
    }
    if let Some((point, level)) = snap.aligned {
        let aligned =
            if level { GeoConstraint::Horizontal(point, end) } else { GeoConstraint::Vertical(point, end) };
        infer(sketch, ids, aligned, created);
    }
}

/// A tangency between two curves, with the side captured from how they lie now.
fn tangency(sketch: &Sketch, a: Id<Curve>, b: Id<Curve>) -> Option<GeoConstraint> {
    let xy = |p: Id<SkPoint>| sketch.pos(p).map(|q| [q[0], q[1]]);
    let (first, second) = if sketch.line(b).is_some() { (b, a) } else { (a, b) };
    let (centre, radius) = sketch.round(second)?;
    let centre = xy(centre)?;
    let (side, external) = match sketch.line(first) {
        Some((p, q)) => {
            let (p, q) = (xy(p)?, xy(q)?);
            let cross = (q[0] - p[0]) * (centre[1] - p[1]) - (q[1] - p[1]) * (centre[0] - p[0]);
            (if cross >= 0.0 { 1.0 } else { -1.0 }, true)
        }
        None => {
            let (other, other_radius) = sketch.round(first)?;
            let other = xy(other)?;
            (1.0, gc_math::fm::hypot(centre[0] - other[0], centre[1] - other[1]) >= radius.max(other_radius))
        }
    };
    Some(GeoConstraint::Tangent { a: first, b: second, side, external })
}

/// The unit direction in which `curve` arrives at its end `at` (for an arc, the tangent there, pointing
/// away from the arc).
pub(crate) fn leaving_direction(sketch: &Sketch, curve: Id<Curve>, at: Id<SkPoint>) -> Option<[f64; 2]> {
    let p = |id| sketch.pos(id).map(|q| [q[0], q[1]]);
    let unit = |v: [f64; 2]| {
        let n = gc_math::fm::hypot(v[0], v[1]);
        (n > 1e-12).then(|| [v[0] / n, v[1] / n])
    };
    match sketch.curves.get(&curve)?.geom {
        gc_sketch::CurveGeom::Line { a, b } => {
            let (from, to) = if b == at {
                (a, b)
            } else if a == at {
                (b, a)
            } else {
                return None;
            };
            let (from, to) = (p(from)?, p(to)?);
            unit([to[0] - from[0], to[1] - from[1]])
        }
        gc_sketch::CurveGeom::Arc { c, s, e } => {
            let (centre, here) = (p(c)?, p(at)?);
            let radial = [here[0] - centre[0], here[1] - centre[1]];
            // Counter-clockwise from s to e: leaving at e goes counter-clockwise, at s clockwise.
            if at == e {
                unit([-radial[1], radial[0]])
            } else if at == s {
                unit([radial[1], -radial[0]])
            } else {
                None
            }
        }
        gc_sketch::CurveGeom::Circle { .. } => None,
    }
}

/// A tangent arc as the tools preview it and [`SketchOp::AddTangentArc`] makes it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TangentArc {
    /// Centre.
    pub centre: [f64; 2],
    /// Radius.
    pub radius: f64,
    /// Angle of the start (the joint) about the centre.
    pub start_angle: f64,
    /// Signed sweep from the joint to the end (positive is counter-clockwise).
    pub sweep: f64,
}

/// The arc that leaves `curve` at its end `from` smoothly and ends at `end`; `None` when `end` lies
/// straight ahead (that is a line) or `from` is not an end of `curve`.
pub fn tangent_arc(
    sketch: &Sketch,
    from: Id<SkPoint>,
    curve: Id<Curve>,
    end: [f64; 2],
) -> Option<TangentArc> {
    let start = sketch.pos(from)?;
    let t = leaving_direction(sketch, curve, from)?;
    let n = [-t[1], t[0]];
    let d = [end[0] - start[0], end[1] - start[1]];
    let along = d[0] * n[0] + d[1] * n[1];
    if along.abs() < 1e-9 * gc_math::fm::hypot(d[0], d[1]).max(1e-12) {
        return None;
    }
    // The centre lies on the normal at the start: c = start + n·r with r = |d|² / (2 d·n).
    let r = (d[0] * d[0] + d[1] * d[1]) / (2.0 * along);
    let centre = [start[0] + n[0] * r, start[1] + n[1] * r];
    let angle = |p: [f64; 2]| gc_math::fm::atan2(p[1] - centre[1], p[0] - centre[0]);
    let tau = std::f64::consts::TAU;
    let (a0, a1) = (angle([start[0], start[1]]), angle(end));
    // Turning left (r > 0) the arc runs counter-clockwise from the joint.
    let sweep = if r > 0.0 { (a1 - a0).rem_euclid(tau) } else { -(a0 - a1).rem_euclid(tau) };
    Some(TangentArc { centre, radius: r.abs(), start_angle: a0, sweep })
}

/// The circle through three points: centre and radius; `None` when they lie on a line.
fn circle_through(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> Option<([f64; 2], f64)> {
    let d = 2.0 * (a[0] * (b[1] - c[1]) + b[0] * (c[1] - a[1]) + c[0] * (a[1] - b[1]));
    let scale = (a[0] - c[0]).abs().max((a[1] - c[1]).abs()).max((b[0] - c[0]).abs()).max(1e-12);
    if d.abs() < 1e-9 * scale * scale {
        return None;
    }
    let sq = |p: [f64; 2]| p[0] * p[0] + p[1] * p[1];
    let ux = (sq(a) * (b[1] - c[1]) + sq(b) * (c[1] - a[1]) + sq(c) * (a[1] - b[1])) / d;
    let uy = (sq(a) * (c[0] - b[0]) + sq(b) * (a[0] - c[0]) + sq(c) * (b[0] - a[0])) / d;
    Some(([ux, uy], gc_math::fm::hypot(a[0] - ux, a[1] - uy)))
}

/// Records what a trim, extend or fillet created.
fn push_cut(created: &mut Vec<Created>, cut: &gc_sketch::CutResult) {
    created.extend(cut.curves.iter().map(|c| Created::Curve(*c)));
    created.extend(cut.constraints.iter().map(|c| Created::Constraint(*c)));
}

/// Adds a constraint that the edit needs; refused or conflicting means the edit fails.
fn require(
    sketch: &mut Sketch,
    ids: &mut IdAlloc,
    constraint: GeoConstraint,
    created: &mut Vec<Created>,
) -> Result<(), EditError> {
    created.push(Created::Constraint(added(sketch.try_add_constraint(ids, constraint)?)?));
    Ok(())
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
        SketchOp::PlacePoint { pos, center } => {
            if !pos.iter().all(|v| v.is_finite()) {
                return Err(SketchError::InvalidValue.into());
            }
            let point = sketch.add_point(ids, *pos);
            if let Some(p) = sketch.points.get_mut(&point) {
                p.center = *center;
            }
            created.push(Created::Point(point));
            if *center { "Center point" } else { "Point" }
        }
        SketchOp::SetCenterPoint { points, center } => {
            let owned: std::collections::BTreeSet<Id<SkPoint>> =
                sketch.curves.values().flat_map(gc_sketch::Curve::points).collect();
            let standalone = |p: &Id<SkPoint>| {
                sketch.points.get(p).is_some_and(|q| matches!(q.kind, gc_sketch::PointKind::Free))
                    && !owned.contains(p)
            };
            if points.is_empty() || !points.iter().all(standalone) {
                return Err(SketchError::InvalidValue.into());
            }
            for p in points {
                if let Some(q) = sketch.points.get_mut(p) {
                    q.center = *center;
                }
            }
            "Center point"
        }
        SketchOp::AddLine { a, b, snap_a, snap_b, horizontal } => {
            let line = sketch.add_line(ids, *a, *b);
            created.push(Created::Curve(line));
            if let Some((start, end)) = sketch.line(line) {
                for (end_point, snap) in [(start, snap_a), (end, snap_b)] {
                    snap_constraint(sketch, ids, end_point, snap, &mut created);
                    if let Some(constraint) = snap.tangent_to.and_then(|round| tangency(sketch, line, round))
                    {
                        infer(sketch, ids, constraint, &mut created);
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
        SketchOp::AddCircle { centre, radius, centre_snap, rim_snap } => {
            let circle = sketch.add_circle(ids, *centre, *radius)?;
            created.push(Created::Curve(circle));
            if let Some((c, _)) = sketch.round(circle) {
                snap_constraint(sketch, ids, c, centre_snap, &mut created);
            }
            // The rim on another curve: the circle touches it there.
            if let Some(constraint) =
                rim_snap.tangent_to.or(rim_snap.curve).and_then(|other| tangency(sketch, other, circle))
            {
                infer(sketch, ids, constraint, &mut created);
            }
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
        SketchOp::ProjectedLines { lines } => {
            if lines.is_empty() || lines.iter().any(|(a, b)| !a.iter().chain(b).all(|v| v.is_finite())) {
                return Err(SketchError::InvalidValue.into());
            }
            for (a, b) in lines {
                let line = sketch.add_line(ids, *a, *b);
                created.push(Created::Curve(line));
                if let Some((start, end)) = sketch.line(line) {
                    infer(sketch, ids, GeoConstraint::Fix(start), &mut created);
                    infer(sketch, ids, GeoConstraint::Fix(end), &mut created);
                }
            }
            "Project Cut Edges"
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
        SketchOp::AsConstruction(inner) => {
            if matches!(**inner, SketchOp::AsConstruction(_)) {
                return Err(SketchError::InvalidValue.into());
            }
            let (inner_created, label) = sketch_op(sketch, ids, inner, scope)?;
            for item in &inner_created {
                if let Created::Curve(curve) = item
                    && let Some(c) = sketch.curves.get_mut(curve)
                {
                    c.construction = true;
                }
            }
            created.extend(inner_created);
            label
        }
        SketchOp::AddTangentArc { from, curve, end, snap_end } => {
            let start = sketch.pos(*from).ok_or(SketchError::MissingPoint)?;
            let t = leaving_direction(sketch, *curve, *from).ok_or(SketchError::InvalidValue)?;
            // The centre lies on the normal at the start: c = start + n·r with r = |d|² / (2 d·n).
            let n = [-t[1], t[0]];
            let d = [end[0] - start[0], end[1] - start[1]];
            let along = d[0] * n[0] + d[1] * n[1];
            if along.abs() < 1e-9 * gc_math::fm::hypot(d[0], d[1]).max(1e-12) {
                return Err(SketchError::InvalidValue.into()); // straight ahead: a line, not an arc
            }
            let r = (d[0] * d[0] + d[1] * d[1]) / (2.0 * along);
            let centre = [start[0] + n[0] * r, start[1] + n[1] * r, 0.0];
            // Turning left (r > 0) the arc runs counter-clockwise from the start.
            let (s_pos, e_pos) =
                if r > 0.0 { ([start[0], start[1], 0.0], *end) } else { (*end, [start[0], start[1], 0.0]) };
            let arc = sketch.add_arc(ids, centre, s_pos, e_pos)?;
            created.push(Created::Curve(arc));
            let gc_sketch::CurveGeom::Arc { s, e, .. } = sketch.curves[&arc].geom else {
                return Err(SketchError::InvalidValue.into());
            };
            let (joined, free_end) = if r > 0.0 { (s, e) } else { (e, s) };
            require(sketch, ids, GeoConstraint::Coincident(*from, joined), &mut created)?;
            let tangent = match sketch.line(*curve) {
                Some(_) => {
                    let side = if r > 0.0 { 1.0 } else { -1.0 };
                    GeoConstraint::Tangent { a: *curve, b: arc, side, external: true }
                }
                None => GeoConstraint::Tangent { a: *curve, b: arc, side: 1.0, external: true },
            };
            infer(sketch, ids, tangent, &mut created);
            snap_constraint(sketch, ids, free_end, snap_end, &mut created);
            "Arc"
        }
        SketchOp::AddArcThrough { a, b, through, snap_a, snap_b } => {
            let (centre, _) = circle_through([a[0], a[1]], [b[0], b[1]], [through[0], through[1]])
                .ok_or(SketchError::InvalidValue)?;
            // Counter-clockwise from a to b passes through `through` when it lies left of a→b... seen
            // from the centre: compare the angles.
            let angle = |p: &[f64; 3]| gc_math::fm::atan2(p[1] - centre[1], p[0] - centre[0]);
            let tau = std::f64::consts::TAU;
            let ccw = |from: f64, to: f64| (to - from).rem_euclid(tau);
            let a_first = ccw(angle(a), angle(through)) < ccw(angle(a), angle(b));
            let (first, last) = if a_first { (a, b) } else { (b, a) };
            let arc = sketch.add_arc(ids, [centre[0], centre[1], 0.0], *first, *last)?;
            created.push(Created::Curve(arc));
            let gc_sketch::CurveGeom::Arc { s, e, .. } = sketch.curves[&arc].geom else {
                return Err(SketchError::InvalidValue.into());
            };
            let (end_a, end_b) = if a_first { (s, e) } else { (e, s) };
            snap_constraint(sketch, ids, end_a, snap_a, &mut created);
            snap_constraint(sketch, ids, end_b, snap_b, &mut created);
            "Arc"
        }
        SketchOp::AddRectangle3 { a, b, c, snap_a } => {
            let side = [b[0] - a[0], b[1] - a[1]];
            let length = gc_math::fm::hypot(side[0], side[1]);
            if length < 1e-12 {
                return Err(SketchError::InvalidValue.into());
            }
            let n = [-side[1] / length, side[0] / length];
            let width = (c[0] - b[0]) * n[0] + (c[1] - b[1]) * n[1];
            if width.abs() < 1e-12 {
                return Err(SketchError::InvalidValue.into());
            }
            let corners = [
                [a[0], a[1], 0.0],
                [b[0], b[1], 0.0],
                [b[0] + n[0] * width, b[1] + n[1] * width, 0.0],
                [a[0] + n[0] * width, a[1] + n[1] * width, 0.0],
            ];
            let lines: Vec<Id<Curve>> =
                (0..4).map(|i| sketch.add_line(ids, corners[i], corners[(i + 1) % 4])).collect();
            let ends: Vec<(Id<SkPoint>, Id<SkPoint>)> =
                lines.iter().filter_map(|l| sketch.line(*l)).collect();
            created.extend(lines.iter().map(|l| Created::Curve(*l)));
            for i in 0..ends.len() {
                require(
                    sketch,
                    ids,
                    GeoConstraint::Coincident(ends[i].1, ends[(i + 1) % 4].0),
                    &mut created,
                )?;
            }
            require(sketch, ids, GeoConstraint::Perpendicular(lines[0], lines[1]), &mut created)?;
            require(sketch, ids, GeoConstraint::Parallel(lines[0], lines[2]), &mut created)?;
            require(sketch, ids, GeoConstraint::Parallel(lines[1], lines[3]), &mut created)?;
            if let Some((corner, _)) = ends.first() {
                snap_constraint(sketch, ids, *corner, snap_a, &mut created);
            }
            "Rectangle"
        }
        SketchOp::WithDimensions { op, dims } => {
            if matches!(**op, SketchOp::WithDimensions { .. }) {
                return Err(SketchError::InvalidValue.into());
            }
            let (inner, label) = sketch_op(sketch, ids, op, scope)?;
            let curves: Vec<Id<Curve>> = inner
                .iter()
                .filter_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None })
                .collect();
            created.extend(inner);
            let lines: Vec<Id<Curve>> =
                curves.iter().copied().filter(|c| sketch.line(*c).is_some()).collect();
            let first_of = |want: fn(&gc_sketch::CurveGeom) -> bool| {
                curves.iter().copied().find(|c| sketch.curves.get(c).is_some_and(|k| want(&k.geom)))
            };
            let circle = first_of(|g| matches!(g, gc_sketch::CurveGeom::Circle { .. }));
            let arc = first_of(|g| matches!(g, gc_sketch::CurveGeom::Arc { .. }));
            for (slot, source) in dims {
                let kind = match slot {
                    DimSlot::Length | DimSlot::Width => {
                        lines.first().and_then(|l| sketch.line(*l)).map(|(p, q)| DimKind::Distance { p, q })
                    }
                    DimSlot::Height => {
                        lines.get(1).and_then(|l| sketch.line(*l)).map(|(p, q)| DimKind::Distance { p, q })
                    }
                    DimSlot::Diameter => circle.map(DimKind::Diameter),
                    DimSlot::Radius => arc.map(DimKind::Radius),
                };
                let kind = kind.ok_or(SketchError::InvalidValue)?;
                // The text beside what it measures.
                let text_pos = match kind {
                    DimKind::Distance { p, q } => {
                        let (a, b) = (sketch.pos(p).unwrap_or_default(), sketch.pos(q).unwrap_or_default());
                        [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]
                    }
                    DimKind::Diameter(c) | DimKind::Radius(c) => sketch
                        .round(c)
                        .and_then(|(centre, r)| sketch.pos(centre).map(|at| [at[0] + r, at[1]]))
                        .unwrap_or_default(),
                    _ => [0.0; 2],
                };
                let value = evaluate(source, scope, field_unit(&kind))?;
                let result = sketch.try_add_dimension(ids, kind, source, value, text_pos)?;
                created.push(Created::Dimension(added(result)?));
            }
            label
        }
        SketchOp::SetDriving { id, driving } => {
            if *driving {
                let kind = sketch.dimensions.get(id).ok_or(SketchError::MissingItem)?.kind;
                let measured = sketch.measure(&kind).ok_or(SketchError::InvalidValue)?;
                // The current measurement, written the way a user would type it.
                let source = match field_unit(&kind) {
                    Unit::Deg => format!("{} deg", round_for_display(measured.to_degrees())),
                    _ => format!("{} mm", round_for_display(measured * 1000.0)),
                };
                added(sketch.set_driving(*id, Some((&source, measured)))?)?;
            } else {
                sketch.set_driving(*id, None)?;
            }
            if *driving { "Driving dimension" } else { "Driven dimension" }
        }
        SketchOp::SetConstructionOf { curves, construction } => {
            for curve in curves {
                sketch.curves.get_mut(curve).ok_or(SketchError::MissingCurve)?.construction = *construction;
            }
            "Construction"
        }
        SketchOp::Delete(items) => {
            sketch.delete(items);
            "Delete"
        }
        // Resolved against the document in `run`; inside a wrapper it is not supported.
        SketchOp::RenameDimension { .. } => return Err(SketchError::InvalidValue.into()),
        SketchOp::Project { .. } => return Err(SketchError::InvalidValue.into()),
        SketchOp::Trim { curve, at } => {
            push_cut(&mut created, &sketch.trim(ids, *curve, *at)?);
            "Trim"
        }
        SketchOp::Extend { curve, at } => {
            push_cut(&mut created, &sketch.extend(ids, *curve, *at)?);
            "Extend"
        }
        SketchOp::Fillet { lines, radius } => {
            let value = evaluate(radius, scope, Unit::Mm)?;
            let cut = sketch.fillet(ids, *lines, value)?;
            push_cut(&mut created, &cut);
            let arc = *cut.curves.first().ok_or(SketchError::InvalidValue)?;
            let kind = DimKind::Radius(arc);
            let text_pos = sketch
                .round(arc)
                .and_then(|(centre, _)| sketch.pos(centre))
                .map_or([0.0; 2], |c| [c[0], c[1]]);
            let result = sketch.try_add_dimension(ids, kind, radius, value, text_pos)?;
            created.push(Created::Dimension(added(result)?));
            "Fillet"
        }
    };
    // Whatever the edit was, the sketch must end up consistent.
    let report = sketch.solve()?;
    if report.status != Status::Solved {
        return Err(SketchError::Unsolved(report.status).into());
    }
    Ok((created, label))
}

pub(crate) fn run(txn: &mut Txn<'_>, command: &Command) -> Result<(Vec<Created>, &'static str), EditError> {
    match command {
        Command::AddParam { name, source, angle } => {
            let taken = !name_free(txn.doc(), name, None);
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
        Command::RenameParam { id, name } => {
            let name = name.trim().to_owned();
            let old = txn.doc().get(*id).ok_or(DocError::Missing(id.erase()))?.name.clone();
            if name == old {
                return Ok((Vec::new(), "Rename parameter"));
            }
            let dimension_has_it =
                txn.doc().sketches().iter().any(|(_, s)| s.dimensions.values().any(|d| d.name == name));
            let taken = dimension_has_it
                || txn.doc().params().iter().any(|(other, p)| other != *id && p.name == name);
            if taken || !gc_expr::is_valid_param_name(&name) {
                return Err(EditError::BadName(name));
            }
            let params: Vec<(Id<Param>, String)> = txn
                .doc()
                .params()
                .iter()
                .filter_map(|(other, p)| rename_in(&p.source, &old, &name).map(|source| (other, source)))
                .collect();
            for (other, source) in params {
                txn.modify(other, |p: &mut Param, _| p.source = source)?;
            }
            let sketches: Vec<Id<Sketch>> = txn
                .doc()
                .sketches()
                .iter()
                .filter(|(_, s)| s.dimensions.values().any(|d| rename_in(&d.source, &old, &name).is_some()))
                .map(|(id, _)| id)
                .collect();
            for sketch in sketches {
                txn.modify(sketch, |s: &mut Sketch, _| {
                    for d in s.dimensions.values_mut() {
                        if let Some(source) = rename_in(&d.source, &old, &name) {
                            d.source = source;
                        }
                    }
                })?;
            }
            txn.modify(*id, |p: &mut Param, _| p.name.clone_from(&name))?;
            propagate(txn)?;
            Ok((Vec::new(), "Rename parameter"))
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
        Command::CreateSketchOn { name, source } => {
            let doc = txn.doc();
            let pose = doc.plane_source_pose(source).ok_or(EditError::BadValue)?;
            let world = doc.world().ok_or(EditError::BadValue)?;
            let frame = txn.insert(Frame {
                name: format!("{name} plane"),
                kind: crate::document::FrameKind::User,
                parent: Some(world),
                xyz: pose.translation.to_array(),
                quat_xyzw: pose.rotation.to_array(),
                source: Some(*source),
            });
            let mut sketch = Sketch::new(Space::Planar(frame.erase()));
            sketch.name.clone_from(name);
            let id = txn.insert(sketch);
            txn.modify(id, |s: &mut Sketch, ids| {
                s.add_external(ids, ExternalRef::FrameOrigin(frame.erase()), [0.0; 3]);
            })?;
            Ok((vec![Created::Sketch(id)], "New sketch"))
        }
        Command::DeleteSketch { id } => {
            let removed = txn.remove(*id)?;
            // A plane made for this sketch alone (attached to a work plane or a face) goes with it.
            let (Space::Planar(frame) | Space::Spatial(frame)) = removed.space;
            let frame: Id<Frame> = Id::from_raw(frame.raw);
            let attached = txn.doc().get(frame).is_some_and(|f| f.source.is_some());
            let used = txn.doc().references().iter().any(|(_, to)| *to == frame.erase());
            if attached && !used {
                txn.remove(frame)?;
            }
            Ok((Vec::new(), "Delete sketch"))
        }
        Command::Sketch { sketch, op: SketchOp::Project { source, construction } } => {
            crate::project::project(txn, *sketch, source, *construction)
        }
        Command::Sketch { sketch, op: SketchOp::AddDimension { kind, source, text_pos } }
            if named_source(source).is_some() =>
        {
            let (name, expression) = named_source(source).ok_or(EditError::BadValue)?;
            if !name_free(txn.doc(), &name, None) {
                return Err(EditError::BadName(name));
            }
            let op = SketchOp::AddDimension { kind: *kind, source: expression, text_pos: *text_pos };
            let scope = scope_of(txn.doc());
            let (created, label) =
                txn.modify(*sketch, |s: &mut Sketch, ids| sketch_op(s, ids, &op, &scope))??;
            if let Some(Created::Dimension(id)) = created.first() {
                let id = *id;
                txn.modify(*sketch, |s: &mut Sketch, _| {
                    if let Some(d) = s.dimensions.get_mut(&id) {
                        d.name = name;
                    }
                })?;
            }
            propagate(txn)?;
            Ok((created, label))
        }
        Command::Sketch { sketch, op: SketchOp::SetDimension { id, source } }
            if named_source(source).is_some() =>
        {
            let (name, expression) = named_source(source).ok_or(EditError::BadValue)?;
            rename_dimension(txn, *sketch, *id, &name)?;
            let op = SketchOp::SetDimension { id: *id, source: expression };
            let scope = scope_of(txn.doc());
            let result = txn.modify(*sketch, |s: &mut Sketch, ids| sketch_op(s, ids, &op, &scope))??;
            propagate(txn)?;
            Ok(result)
        }
        Command::Sketch { sketch, op: SketchOp::RenameDimension { id, name } } => {
            rename_dimension(txn, *sketch, *id, name.trim())?;
            propagate(txn)?;
            Ok((Vec::new(), "Rename parameter"))
        }
        Command::Sketch { sketch, op } => {
            let scope = scope_of(txn.doc());
            let result = txn.modify(*sketch, |s: &mut Sketch, ids| sketch_op(s, ids, op, &scope))??;
            // Dimensions and parameters that use this sketch's dimensions follow (and loops are refused).
            if matches!(
                op,
                SketchOp::SetDimension { .. } | SketchOp::SetDriving { .. } | SketchOp::AddDimension { .. }
            ) {
                propagate(txn)?;
            }
            Ok(result)
        }
        Command::Cell(op) => crate::cell::run(txn, op),
        Command::Rename { item, name } => crate::tree::rename(txn, *item, name),
        Command::SetVisible { item, visible } => crate::tree::set_visible(txn, *item, *visible),
        Command::SetOriginVisible { occurrence, elements, visible } => {
            crate::tree::set_origin_visible(txn, *occurrence, elements, *visible)
        }
        Command::SetComponent { occurrence, flag, on } => {
            crate::tree::set_component(txn, *occurrence, *flag, *on)
        }
    }
}

/// Applies a command. On success the document is changed and the result says how to undo it. On failure
/// the document is exactly as before.
pub fn apply(doc: &mut Document, command: &Command) -> Result<Applied, EditError> {
    let mut txn = Txn::new(doc);
    let (created, label) = run(&mut txn, command)?; // an early return drops `txn`, which rolls back
    crate::pattern::refresh(&mut txn)?;
    crate::project::refresh(&mut txn)?;
    crate::tree::prune_states(&mut txn)?;
    name_dimensions(&mut txn)?;
    let (delta, changed) = txn.commit()?;
    Ok(Applied { delta, changed, created, label })
}

/// Renames a dimension's model parameter and every reference to it.
fn rename_dimension(
    txn: &mut Txn<'_>,
    sketch: Id<Sketch>,
    id: Id<Dimension>,
    name: &str,
) -> Result<(), EditError> {
    let old = txn
        .doc()
        .get(sketch)
        .and_then(|s| s.dimensions.get(&id))
        .ok_or(EditError::Sketch(SketchError::MissingItem))?
        .name
        .clone();
    if name == old {
        return Ok(());
    }
    if !gc_expr::is_valid_param_name(name) || !name_free(txn.doc(), name, Some((sketch, id))) {
        return Err(EditError::BadName(name.to_owned()));
    }
    if !old.is_empty() {
        rewrite_references(txn, &old, name)?;
    }
    txn.modify(sketch, |s: &mut Sketch, _| {
        if let Some(d) = s.dimensions.get_mut(&id) {
            name.clone_into(&mut d.name);
        }
    })?;
    Ok(())
}

/// Renames references to `old` in every parameter's and dimension's expression.
fn rewrite_references(txn: &mut Txn<'_>, old: &str, new: &str) -> Result<(), EditError> {
    let params: Vec<(Id<Param>, String)> = txn
        .doc()
        .params()
        .iter()
        .filter_map(|(other, p)| rename_in(&p.source, old, new).map(|source| (other, source)))
        .collect();
    for (other, source) in params {
        txn.modify(other, |p: &mut Param, _| p.source = source)?;
    }
    let sketches: Vec<Id<Sketch>> = txn
        .doc()
        .sketches()
        .iter()
        .filter(|(_, s)| s.dimensions.values().any(|d| rename_in(&d.source, old, new).is_some()))
        .map(|(id, _)| id)
        .collect();
    for sketch in sketches {
        txn.modify(sketch, |s: &mut Sketch, _| {
            for d in s.dimensions.values_mut() {
                if let Some(source) = rename_in(&d.source, old, new) {
                    d.source = source;
                }
            }
        })?;
    }
    Ok(())
}
