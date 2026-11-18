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

/// How the first two clicks of a straight slot are read (Inventor 2026 help, To Create 2D Shape Geometry,
/// GUID-D489CE6D-7299-4211-A43A-F3580A4BA357, Slots).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlotKind {
    /// Center to Center Slot: the two arc centres.
    CenterToCenter,
    /// Overall Slot: the start and end of the slot's centre line, ends included.
    Overall,
    /// Center Point Slot: the slot's centre, then one arc centre.
    CenterPoint,
}

/// How the first three clicks of an arc slot are read (Inventor 2026 help, To Create 2D Shape Geometry,
/// GUID-D489CE6D-7299-4211-A43A-F3580A4BA357, Slots).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArcSlotKind {
    /// Three Point Arc Slot: the centre arc's start and end, then a point it passes through.
    ThreePoint,
    /// Center Point Arc Slot: the centre arc's centre, start and end.
    CenterPoint,
}

/// How Move, Copy and Rotate take geometry (Inventor 2026 help, To Copy, Move, or Rotate 2D Sketch
/// Geometry, GUID-F8114270-18F7-4C7A-947E-AC489EDACFA9).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Motion {
    /// From the base point to the end point.
    Translate {
        /// Displacement, metres.
        by: [f64; 2],
    },
    /// About the centre point.
    Rotate {
        /// Centre.
        centre: [f64; 2],
        /// Angle, radians, counter-clockwise.
        angle: f64,
    },
    /// Scale about the base point (To Stretch, Scale, or Offset 2D Sketch Geometry,
    /// GUID-962CEE71-8BE4-4DA6-B51E-341319C64FA4); circles scale with it.
    Scale {
        /// Base point.
        centre: [f64; 2],
        /// Scale factor, positive.
        factor: f64,
    },
}

impl Motion {
    /// Where the motion takes a point.
    pub fn place(&self, p: [f64; 3]) -> [f64; 3] {
        match *self {
            Motion::Translate { by } => [p[0] + by[0], p[1] + by[1], p[2]],
            Motion::Rotate { centre, angle } => {
                let (sin, cos) = (gc_math::fm::sin(angle), gc_math::fm::cos(angle));
                let (x, y) = (p[0] - centre[0], p[1] - centre[1]);
                [centre[0] + cos * x - sin * y, centre[1] + sin * x + cos * y, p[2]]
            }
            Motion::Scale { centre, factor } => {
                [centre[0] + factor * (p[0] - centre[0]), centre[1] + factor * (p[1] - centre[1]), p[2]]
            }
        }
    }

    /// What it does to a circle's radius.
    pub fn radius_factor(&self) -> f64 {
        match *self {
            Motion::Scale { factor, .. } => factor,
            _ => 1.0,
        }
    }
}

/// The 2D Chamfer dialog's types; sizes are expressions (millimetres, degrees by default).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ChamferKind {
    /// Equal Distance: the same distance back from the corner along both lines.
    Equal(String),
    /// Unequal Distance: a distance along the first line and one along the second.
    Unequal(String, String),
    /// Distance and Angle: the angle from the first line and the distance along the second.
    DistanceAngle {
        /// Along the second line.
        distance: String,
        /// From the first line.
        angle: String,
    },
}

/// One direction of a sketch rectangular pattern (kept in the sketch's pattern record).
pub use gc_sketch::PatternDirection as SketchPatternDirection;

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
    /// The Polygon command (Inventor 2026 help, To Create 2D Shape Geometry,
    /// GUID-D489CE6D-7299-4211-A43A-F3580A4BA357: up to 120 sides, Inscribed — "the vertex ... determines
    /// the size and orientation" — or Circumscribed — "the midpoint of an edge"): a regular polygon round
    /// `centre` with a corner (inscribed) or an edge midpoint (circumscribed) at `at`. A construction
    /// circle through the corners, or touching the edges, carries its size; the polygon keeps four degrees
    /// of freedom (centre, size, turn).
    AddPolygon {
        /// Centre.
        centre: [f64; 3],
        /// A corner (inscribed) or the midpoint of an edge (circumscribed).
        at: [f64; 3],
        /// Number of sides, 3 to 120.
        sides: u32,
        /// Circumscribed (edges touch the circle) rather than inscribed (corners on it).
        circumscribed: bool,
    },
    /// A straight slot (Center to Center, Overall or Center Point): a construction centre line between
    /// the arc centres, two sides and two half-round ends tangent to them, ends of equal radius. It keeps
    /// five degrees of freedom (the two arc centres and the width).
    AddSlot {
        /// First click.
        a: [f64; 3],
        /// Second click.
        b: [f64; 3],
        /// Width (the third click), metres.
        width: f64,
        /// How the clicks are read.
        kind: SlotKind,
    },
    /// An arc slot (Three Point Arc Slot or Center Point Arc Slot, Inventor 2026 help, To Create 2D Shape
    /// Geometry, GUID-D489CE6D-7299-4211-A43A-F3580A4BA357): a construction centre arc counter-clockwise
    /// from `start` to `end` round `centre` (`end` only gives the direction), an outer and an inner arc
    /// concentric with it and half-round ends centred on its ends. The ends' tangency is built from
    /// first-order pieces (a construction radius through each end on which the end's centre and both
    /// arc ends lie), so the slot keeps six degrees of freedom (centre, radius, two angles, width).
    AddArcSlot {
        /// The centre arc's centre.
        centre: [f64; 3],
        /// The centre arc's start (it sets the radius).
        start: [f64; 3],
        /// The direction of the centre arc's end.
        end: [f64; 3],
        /// Width, metres (less than twice the radius).
        width: f64,
    },
    /// The sketch Mirror command (Inventor 2026 help, To Create and Edit Sketch Patterns,
    /// GUID-AAD3A699-FED8-4614-A31D-0A2316328CEE, Create Mirror Patterns): copies of the curves reflected
    /// across a line, every copied point symmetric to its original about it (points on the line are
    /// shared), circles of equal radius. The copies add no degrees of freedom.
    Mirror {
        /// The curves to mirror.
        curves: Vec<Id<Curve>>,
        /// The mirror line.
        axis: Id<Curve>,
    },
    /// The sketch Rectangular Pattern (Inventor 2026 help, To Create and Edit Sketch Patterns,
    /// GUID-AAD3A699-FED8-4614-A31D-0A2316328CEE): copies of the curves along one or two directions
    /// (lines), `count` along each (the originals included) at `spacing` (an expression; fitted: the whole
    /// extent). Fully constrained: every copied point is one construction step from the previous copy,
    /// parallel to the direction and as long as the first step, which carries the spacing dimension —
    /// editing that dimension moves the whole pattern.
    RectangularPattern {
        /// The curves to pattern.
        curves: Vec<Id<Curve>>,
        /// Direction 1.
        first: SketchPatternDirection,
        /// Direction 2, if any.
        second: Option<SketchPatternDirection>,
        /// Fitted: `spacing` is the overall distance, shared out between the elements.
        fitted: bool,
    },
    /// The sketch Circular Pattern: copies round an axis point, `count` in all (the originals included),
    /// `angle` (an expression) between neighbours (fitted: the overall angle, a full turn shared out by
    /// `count`). Fully constrained: each copied point keeps its original's distance from the axis and an
    /// angle dimension from the previous copy, all from the same expression (name a model parameter in it
    /// to edit them together).
    CircularPattern {
        /// The curves to pattern.
        curves: Vec<Id<Curve>>,
        /// The axis: a point (a centre, a vertex, a sketch point).
        axis: Id<SkPoint>,
        /// Number of elements, the originals included, 2 to 500.
        count: u32,
        /// The angle between neighbours (or overall, fitted), an expression.
        angle: String,
        /// Clockwise instead of counter-clockwise.
        flip: bool,
        /// Fitted: `angle` is the overall angle.
        fitted: bool,
    },
    /// Edit Pattern: rebuilds the rectangular or circular pattern `element` belongs to (an original or a
    /// copy) from new values, `suppressed` naming its suppressed elements. The copies are made anew, so
    /// relationships added to them by hand go with the old ones.
    EditSketchPattern {
        /// Any curve of the pattern.
        element: Id<Curve>,
        /// The new values.
        definition: gc_sketch::PatternDefinition,
        /// Elements to suppress (indices, in the order they were made).
        suppressed: Vec<usize>,
    },
    /// Delete Pattern: removes the copies and everything the pattern added; the originals stay.
    DeleteSketchPattern {
        /// Any curve of the pattern.
        element: Id<Curve>,
    },
    /// Suppress Element(s): the elements the copies belong to turn construction (dashed, out of profiles)
    /// while their relationships stay; `suppress: false` brings them back.
    SuppressPatternElements {
        /// Copies in the elements.
        curves: Vec<Id<Curve>>,
        /// Suppress or bring back.
        suppress: bool,
    },
    /// The sketch Offset command: copies of curves at a distance — whole loops by default (Loop Select),
    /// held equidistant (Constrain Offset). Positive distances go outside a closed loop (circles grow),
    /// or to the left of an open chain.
    Offset {
        /// The curves picked.
        curves: Vec<Id<Curve>>,
        /// Distance, metres (signed).
        distance: f64,
        /// Take the whole loops the picked curves belong to.
        loop_select: bool,
        /// Keep the offset at one distance from the original everywhere.
        constrain: bool,
        /// Where the curve was picked. An ellipse picked between its quadrants offsets to a spline, near a
        /// quadrant (or with no pick) to an ellipse (To Create 2D Shape Geometry,
        /// GUID-D489CE6D-7299-4211-A43A-F3580A4BA357).
        #[serde(default)]
        picked: Option<[f64; 2]>,
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
    /// Adds an ellipse (Inventor 2026 help, To Create 2D Shape Geometry, Ellipse: the centre, a point that
    /// sets the first axis, then a point that sets the minor radius).
    AddEllipse {
        /// Centre.
        centre: [f64; 3],
        /// End of the major (first) axis.
        major: [f64; 3],
        /// Minor radius.
        minor: f64,
        /// What the centre was snapped to.
        #[serde(default)]
        centre_snap: EndSnap,
    },
    /// Adds an interpolation spline through fit points (Inventor 2026 help, Spline (Interpolation): click
    /// the fit points, then finish). Each fit point may be snapped (coincident with a point, on a curve).
    AddSpline {
        /// The fit points, first to last.
        points: Vec<[f64; 3]>,
        /// What each fit point was snapped to (missing entries: nothing).
        #[serde(default)]
        snaps: Vec<EndSnap>,
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
    /// A 3D line drawn with Auto-Bend on (Sketch tab of Application Options, GUID-1898C432-CFD2-46F1-8BAE-
    /// 631392DFBF1D: "Automatically places tangent corner bends on 3D lines as you sketch them"): the line
    /// (`line`, an `AddLine`), then a Bend of `radius` where it turns from `previous`. Where the bend does
    /// not fit (the lines too short, in line), the line is kept without it. One undo step.
    AddLineBent {
        /// The `AddLine` for the new line.
        line: Box<SketchOp>,
        /// The line it continues from.
        previous: Id<Curve>,
        /// The bend radius as typed (the document's 3D Sketch Auto-Bend Radius).
        radius: String,
    },
    /// Bend (To Create Lines, Arcs, and Bends, GUID-4F861674-F74C-474E-869A-4C190ED6EC4B): a tangent arc of
    /// `radius` at the corner where two lines of a 3D sketch meet, with a radius dimension.
    Bend {
        /// The two lines.
        lines: [Id<Curve>; 2],
        /// The radius as typed, e.g. `"20 mm"`.
        radius: String,
    },
    /// Move, Copy or Rotate the curves (Inventor 2026 help, To Copy, Move, or Rotate 2D Sketch Geometry,
    /// GUID-F8114270-18F7-4C7A-947E-AC489EDACFA9). `copy` makes moved copies (with the constraints among
    /// them) and leaves the originals; otherwise the curves themselves move, and with `relax` constraints
    /// tying them elsewhere break and such dimensions take their new values (the More options at Always;
    /// without it, at Never, a move the sketch cannot take is refused).
    Transform {
        /// The curves.
        curves: Vec<Id<Curve>>,
        /// How.
        motion: Motion,
        /// Copy instead of move.
        copy: bool,
        /// Break and recompute what holds the curves back.
        relax: bool,
    },
    /// Flip Direction (Inventor 2026 help, To Work with Sketch Dimensions,
    /// GUID-026FE8D3-AFBB-4834-BC18-73D9CFF78185): the geometry goes to the other side of the dimensions —
    /// a point-line or parallel-line distance, a horizontal or vertical distance between points, an angle
    /// (the second line turns the other way). Refused when one of them is of another kind.
    FlipDimensions(Vec<Id<gc_sketch::Dimension>>),
    /// Automatic Dimensions and Constraints (Inventor 2026 help, To Apply 2D Sketch Dimensions
    /// Automatically, GUID-1DC71C6F-9F6D-4846-8E73-9CB7D3452882): adds the constraints the curves nearly
    /// have (horizontal, vertical, ends that meet) and dimensions (lengths, radii, then positions from the
    /// sketch origin) that still remove freedom, with the values they have now. No curves: the whole sketch.
    AutoDimension {
        /// The curves; empty for all.
        curves: Vec<Id<Curve>>,
        /// Add dimensions.
        dimensions: bool,
        /// Add constraints.
        constraints: bool,
    },
    /// Stretch (Inventor 2026 help, To Stretch, Scale, or Offset 2D Sketch Geometry,
    /// GUID-962CEE71-8BE4-4DA6-B51E-341319C64FA4): the curves and points move by `by`, and the geometry
    /// joined to them stretches to follow; with `relax`, dimensions across the selection's edge take
    /// their new values, otherwise a stretch they hold back is refused.
    Stretch {
        /// Curves that move whole.
        curves: Vec<Id<Curve>>,
        /// Points that move (vertices picked on their own).
        points: Vec<Id<SkPoint>>,
        /// Displacement, metres.
        by: [f64; 2],
        /// Recompute what holds the stretch back.
        relax: bool,
    },
    /// Split (Inventor 2026 help, To Split, Trim, or Extend Curves, GUID-DD00037F-6BBA-4B95-AA90-4F99DCEB667D):
    /// cuts a line or arc in two at its crossing with another curve nearest `at`.
    Split {
        /// The curve.
        curve: Id<Curve>,
        /// Where it was clicked.
        at: [f64; 2],
    },
    /// Split Spline (To Create, Lock Spline Length, and Edit Splines, GUID-DB15C479-66F2-456A-9E54-40A60F83DC92:
    /// "To split a spline, right-click a point and choose Split Spline"): the spline in two at one of its
    /// inner fit points, which both pieces share.
    SplitSpline {
        /// The spline.
        curve: Id<Curve>,
        /// The inner fit point to split at.
        point: Id<SkPoint>,
    },
    /// Symmetric on two lines, circles or arcs about a line (GUID-A85A7A30-7D81-4C75-8769-CAD034EEA930): "click
    /// the first line or curve. Click the second line or curve. Click the line of symmetry." Their points are
    /// made symmetric in pairs (`Sketch::symmetric_pairs`); pairs already symmetric are left as they are.
    SymmetricCurves {
        /// The first line or curve.
        first: Id<Curve>,
        /// The second.
        second: Id<Curve>,
        /// The line of symmetry.
        axis: Id<Curve>,
    },
    /// Close Curve (GUID-DB15C479-66F2-456A-9E54-40A60F83DC92: "To close a curve, right-click the starting
    /// point and choose Close Curve"): an open spline of three or more fit points closes smoothly.
    CloseSpline {
        /// The spline.
        curve: Id<Curve>,
    },
    /// Insert Point ("right-click the spline and choose Insert Point. Then click the curve"): a new fit
    /// point on the spline nearest `at`.
    InsertSplinePoint {
        /// The spline.
        curve: Id<Curve>,
        /// Where it was clicked.
        at: [f64; 2],
    },
    /// 2D Chamfer (To Create 2D Shape Geometry, GUID-D489CE6D-7299-4211-A43A-F3580A4BA357): bevels the
    /// corner of two lines. With `dimensions`, a construction point at the corner (held on both lines)
    /// carries the chamfer's sizes as dimensions (Inventor's Create Dimensions).
    Chamfer {
        /// The two lines and where each was clicked (each keeps that side of the corner).
        lines: [(Id<Curve>, [f64; 2]); 2],
        /// The kind and sizes.
        kind: ChamferKind,
        /// Add dimensions for the sizes.
        dimensions: bool,
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
    /// An arc in a 3D sketch (Three Point Arc or Center Point Arc on the 3D Sketch tab; Inventor 2026 help,
    /// To Create Lines, Arcs, and Bends, GUID-4F861674-F74C-474E-869A-4C190ED6EC4B): round `centre` from
    /// `start` towards `end` (only its direction counts) in the plane of the three, the short way round
    /// (a 3D arc turns less than half a circle). `snaps` are existing points the centre, start and end are
    /// coincident with.
    AddSpatialArc {
        /// Centre.
        centre: [f64; 3],
        /// Start (sets the radius).
        start: [f64; 3],
        /// Direction of the end.
        end: [f64; 3],
        /// Points the centre, start and end were placed on.
        snaps: [Option<Id<SkPoint>>; 3],
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
    /// Midpoint Line (Inventor 2026 help, To Create Lines, Arcs, and Bends,
    /// GUID-4F861674-F74C-474E-869A-4C190ED6EC4B): a line through `end` whose midpoint is at `mid`; the
    /// midpoint is a point of the sketch held at the line's middle, and takes what it was snapped to.
    AddMidpointLine {
        /// The midpoint.
        mid: [f64; 3],
        /// One end.
        end: [f64; 3],
        /// What the midpoint was snapped to.
        snap_mid: EndSnap,
        /// What the end was snapped to.
        snap_end: EndSnap,
    },
    /// Two Point Center Rectangle (To Create 2D Shape Geometry, GUID-D489CE6D-7299-4211-A43A-F3580A4BA357):
    /// an axis-aligned rectangle round `centre` with a corner at `corner`, and a construction diagonal
    /// whose midpoint is the centre point.
    AddCenterRectangle {
        /// Centre.
        centre: [f64; 3],
        /// A corner.
        corner: [f64; 3],
        /// What the centre was snapped to.
        snap_centre: EndSnap,
    },
    /// Three Point Center Rectangle (same help): round `centre`, `side` the middle of one side (its
    /// direction and distance), `width` setting how far the adjacent sides reach.
    AddCenterRectangle3 {
        /// Centre.
        centre: [f64; 3],
        /// The middle of one side.
        side: [f64; 3],
        /// A point setting the other half-size.
        width: [f64; 3],
        /// What the centre was snapped to.
        snap_centre: EndSnap,
    },
    /// Tangent Circle: the circle touching three lines — of the (up to four) such circles, the one whose
    /// touching points are nearest to where each line was clicked (`near`).
    AddTangentCircle {
        /// The three lines.
        lines: [Id<Curve>; 3],
        /// Where each was clicked.
        near: [[f64; 2]; 3],
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
    /// Runs commands one after another as one edit (one undo step); refused whole when any is refused.
    /// The label is the last command's.
    Sequence(Vec<Command>),
    /// Runs a command that creates sketches and gives them an owner (Inventor 2026 help, Sketches in
    /// assemblies, GUID-2C180C3A-2064-4671-B1C2-F74B383B5D39): `None` is the top-level assembly, a
    /// component is a sketch made while editing that component in place. One undo step.
    Owned {
        /// The owner.
        owner: Option<crate::subassembly::TreeKey>,
        /// The command (CreateSketch, CreateSketchOn).
        command: Box<Command>,
    },
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
    /// A subassembly made in the cell.
    Subassembly(crate::subassembly::SubassemblyId),
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
    /// A relationship would take freedom away from this protected joint (Protect).
    #[error("the protected joint {0} would lose its motion")]
    Protected(String),
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

/// Evaluates what was typed in an input box (a number, a unit, an expression, parameter names) for a field
/// of unit `unit`, in SI units; `None` when it does not evaluate to a finite value.
#[must_use]
pub fn evaluate_value(doc: &Document, source: &str, unit: Unit) -> Option<f64> {
    let source = named_source(source).map_or_else(|| source.to_owned(), |(_, expr)| expr);
    evaluate(&source, &scope_of(doc), unit).ok().filter(|v| v.is_finite())
}

/// The name a new sketch takes: as given (without leading and trailing spaces), or when that is empty the
/// first free "SketchN" (as the browser numbers them).
fn sketch_name(doc: &Document, name: &str) -> String {
    let name = name.trim();
    if !name.is_empty() {
        return name.to_owned();
    }
    (1..)
        .map(|n| format!("Sketch{n}"))
        .find(|n| doc.sketches().iter().all(|(_, s)| s.name != *n))
        .unwrap_or_default()
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

/// What a sketch pattern copies: the curves and their points, each point once.
struct PatternSources {
    curves: Vec<(Id<Curve>, Curve)>,
    points: Vec<Id<SkPoint>>,
}

fn pattern_sources(sketch: &Sketch, curves: &[Id<Curve>]) -> Result<PatternSources, EditError> {
    if curves.is_empty() {
        return Err(SketchError::InvalidValue.into());
    }
    let mut points = Vec::new();
    let mut out = Vec::new();
    for id in curves {
        let curve = sketch.curves.get(id).ok_or(SketchError::MissingCurve)?.clone();
        for p in curve.points() {
            if !points.contains(&p) {
                points.push(p);
            }
        }
        out.push((*id, curve));
    }
    Ok(PatternSources { curves: out, points })
}

/// A construction line between two existing points (it shares them).
fn construction_between(
    sketch: &mut Sketch,
    ids: &mut IdAlloc,
    a: Id<SkPoint>,
    b: Id<SkPoint>,
    created: &mut Vec<Created>,
) -> Result<Id<Curve>, EditError> {
    let id: Id<Curve> = ids.alloc();
    sketch.curves.insert(id, Curve { geom: gc_sketch::CurveGeom::Line { a, b }, construction: true });
    created.push(Created::Curve(id));
    Ok(id)
}

/// A pattern's copies as they are made: elements, places, copied points and circles.
#[derive(Default)]
struct PatternBuild {
    elements: Vec<Vec<Id<Curve>>>,
    places: Vec<[u32; 2]>,
    copies: Vec<gc_sketch::PatternCopy>,
    circles: Vec<(Id<Curve>, Id<Curve>)>,
}

impl PatternBuild {
    /// One element: every original point moved by `place` (points that stay put are shared), and copies of
    /// the curves on them. No constraints: the copies are held and placed by the sketch.
    fn element(
        &mut self,
        sketch: &mut Sketch,
        ids: &mut IdAlloc,
        sources: &PatternSources,
        at: [u32; 2],
        place: impl Fn([f64; 3]) -> [f64; 3],
        created: &mut Vec<Created>,
    ) -> Result<(), EditError> {
        let element = self.elements.len();
        let mut map = std::collections::BTreeMap::new();
        for point in &sources.points {
            let p = sketch.pos(*point).ok_or(SketchError::MissingItem)?;
            let q = place(p);
            if q.iter().zip(p).all(|(a, b)| (a - b).abs() < 1e-12) {
                // On a circular pattern's axis: every copy shares it.
                map.insert(*point, *point);
                continue;
            }
            let copy = sketch.add_point(ids, q);
            created.push(Created::Point(copy));
            self.copies.push(gc_sketch::PatternCopy { element, original: *point, copy });
            map.insert(*point, copy);
        }
        let to = |p: Id<SkPoint>| map.get(&p).copied().ok_or(EditError::Sketch(SketchError::MissingItem));
        let mut copies = Vec::new();
        for (original, curve) in &sources.curves {
            let geom = match curve.geom {
                gc_sketch::CurveGeom::Line { a, b } => gc_sketch::CurveGeom::Line { a: to(a)?, b: to(b)? },
                gc_sketch::CurveGeom::Circle { c, r } => gc_sketch::CurveGeom::Circle { c: to(c)?, r },
                gc_sketch::CurveGeom::Arc { c, s, e } => {
                    gc_sketch::CurveGeom::Arc { c: to(c)?, s: to(s)?, e: to(e)? }
                }
                gc_sketch::CurveGeom::Spline { ref points } => gc_sketch::CurveGeom::Spline {
                    points: points.iter().map(|p| to(*p)).collect::<Result<_, _>>()?,
                },
                gc_sketch::CurveGeom::Ellipse { c, major, minor, arc } => gc_sketch::CurveGeom::Ellipse {
                    c: to(c)?,
                    major: to(major)?,
                    minor,
                    arc: match arc {
                        Some((s, e)) => Some((to(s)?, to(e)?)),
                        None => None,
                    },
                },
            };
            let id: Id<Curve> = ids.alloc();
            sketch.curves.insert(id, Curve { geom, construction: curve.construction });
            created.push(Created::Curve(id));
            copies.push(id);
            if matches!(curve.geom, gc_sketch::CurveGeom::Circle { .. }) {
                self.circles.push((*original, id));
            }
        }
        self.elements.push(copies);
        self.places.push(at);
        Ok(())
    }

    /// Keeps the pattern in the sketch.
    fn finish(
        self,
        sketch: &mut Sketch,
        definition: gc_sketch::PatternDefinition,
        step: gc_sketch::PatternStep,
        created: &[Created],
    ) {
        sketch.patterns.push(gc_sketch::SketchPattern {
            definition,
            elements: self.elements,
            items: pattern_items(created),
            suppressed: Vec::new(),
            step,
            places: self.places,
            copies: self.copies,
            circles: self.circles,
        });
    }
}

/// The centre point of a rectangle just made (`inner`, its four lines first): a construction diagonal
/// between opposite corners, and a point at its middle that takes what the centre was snapped to.
fn centre_point(
    sketch: &mut Sketch,
    ids: &mut IdAlloc,
    inner: &[Created],
    centre: [f64; 3],
    snap: &EndSnap,
    created: &mut Vec<Created>,
) -> Result<(), EditError> {
    let lines: Vec<Id<Curve>> =
        inner.iter().filter_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None }).collect();
    let corner = |k: usize| lines.get(k).and_then(|l| sketch.line(*l)).map(|(a, _)| a);
    let (Some(first), Some(third)) = (corner(0), corner(2)) else {
        return Err(SketchError::InvalidValue.into());
    };
    let diagonal = construction_between(sketch, ids, first, third, created)?;
    let middle = sketch.add_point(ids, [centre[0], centre[1], 0.0]);
    created.push(Created::Point(middle));
    require(sketch, ids, GeoConstraint::Midpoint(middle, diagonal), created)?;
    snap_constraint(sketch, ids, middle, snap, created);
    Ok(())
}

/// The circle touching the three lines (as infinite lines) whose touching points are nearest to `near`.
fn tangent_circle(sketch: &Sketch, lines: &[Id<Curve>; 3], near: &[[f64; 2]; 3]) -> Option<([f64; 2], f64)> {
    // Each line as a unit normal and offset: n · p = d.
    let mut forms = Vec::new();
    for line in lines {
        let (a, b) = sketch.line(*line)?;
        let (a, b) = (sketch.pos(a)?, sketch.pos(b)?);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let length = gc_math::fm::hypot(dx, dy);
        if length < 1e-12 {
            return None;
        }
        let n = [-dy / length, dx / length];
        forms.push((n, n[0] * a[0] + n[1] * a[1]));
    }
    let mut best: Option<(f64, [f64; 2], f64)> = None;
    // A centre at signed distance r from each line, on the chosen sides: s_i (n_i · c - d_i) = r.
    for signs in [[1.0, 1.0, 1.0], [1.0, 1.0, -1.0], [1.0, -1.0, 1.0], [-1.0, 1.0, 1.0]] {
        for flip in [1.0, -1.0] {
            let s: Vec<f64> = signs.iter().map(|v| v * flip).collect();
            // Unknowns c.x, c.y, r: s_i n_i · c - r = s_i d_i.
            let row = |i: usize| -> [f64; 4] {
                let (n, d) = forms[i];
                [s[i] * n[0], s[i] * n[1], -1.0, s[i] * d]
            };
            let (r0, r1, r2) = (row(0), row(1), row(2));
            let det = |m: [[f64; 3]; 3]| {
                m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
                    - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
                    + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
            };
            let a = [[r0[0], r0[1], r0[2]], [r1[0], r1[1], r1[2]], [r2[0], r2[1], r2[2]]];
            let d = det(a);
            if d.abs() < 1e-12 {
                continue;
            }
            let replace = |k: usize| {
                let mut m = a;
                for (i, r) in [r0, r1, r2].iter().enumerate() {
                    m[i][k] = r[3];
                }
                det(m) / d
            };
            let (cx, cy, r) = (replace(0), replace(1), replace(2));
            if !(r.is_finite() && r > 1e-9) {
                continue;
            }
            // How far the touching points are from the clicks.
            let miss: f64 = (0..3)
                .map(|i| {
                    let (n, dd) = forms[i];
                    let off = n[0] * cx + n[1] * cy - dd;
                    let touch = [cx - n[0] * off, cy - n[1] * off];
                    gc_math::fm::hypot(touch[0] - near[i][0], touch[1] - near[i][1])
                })
                .sum();
            if best.is_none_or(|(m, ..)| miss < m) {
                best = Some((miss, [cx, cy], r));
            }
        }
    }
    best.map(|(_, c, r)| (c, r))
}

/// A dimension of the other direction (Flip Direction), or `None` for a kind that has none.
pub fn flipped(kind: &DimKind) -> Option<DimKind> {
    Some(match *kind {
        DimKind::PointLine { p, line, side } => DimKind::PointLine { p, line, side: -side },
        // The circle on the other side of the line; the circles apart or one inside the other.
        DimKind::LineArc { line, arc, side } => DimKind::LineArc { line, arc, side: -side },
        DimKind::ArcArc { a, b, external } => DimKind::ArcArc { a, b, external: !external },
        DimKind::HDistance { p, q, sign } => DimKind::HDistance { p, q, sign: -sign },
        DimKind::VDistance { p, q, sign } => DimKind::VDistance { p, q, sign: -sign },
        // The same angle measured the other way round: the second line turns to the other side.
        DimKind::Angle { l1, rev1, l2, rev2 } => DimKind::Angle { l1: l2, rev1: rev2, l2: l1, rev2: rev1 },
        _ => return None,
    })
}

/// Automatic Dimensions and Constraints over `curves` (see [`SketchOp::AutoDimension`]): every candidate
/// is tried in turn and kept only when it removes freedom.
fn auto_dimension(
    sketch: &mut Sketch,
    ids: &mut IdAlloc,
    curves: &[Id<Curve>],
    dimensions: bool,
    constraints: bool,
    created: &mut Vec<Created>,
) -> Result<(), EditError> {
    let xy = |sketch: &Sketch, p: Id<SkPoint>| sketch.pos(p).map(|q| [q[0], q[1]]);
    let mut points: Vec<Id<SkPoint>> = Vec::new();
    for curve in curves {
        for p in sketch.curves.get(curve).ok_or(SketchError::MissingCurve)?.points() {
            if !points.contains(&p) {
                points.push(p);
            }
        }
    }
    let size = points
        .iter()
        .filter_map(|p| xy(sketch, *p))
        .fold(0.0_f64, |m, p| m.max(p[0].abs()).max(p[1].abs()))
        .max(1e-3);
    let close = 1e-6 * size;
    if constraints {
        // Lines that are level or plumb.
        for curve in curves {
            let Some((a, b)) = sketch.line(*curve) else { continue };
            let (Some(pa), Some(pb)) = (xy(sketch, a), xy(sketch, b)) else { continue };
            let level = if (pb[1] - pa[1]).abs() <= close {
                Some(GeoConstraint::Horizontal(a, b))
            } else if (pb[0] - pa[0]).abs() <= close {
                Some(GeoConstraint::Vertical(a, b))
            } else {
                None
            };
            if let Some(constraint) = level {
                try_constraint(sketch, ids, constraint, created);
            }
        }
        // Ends that meet.
        for (i, p) in points.iter().enumerate() {
            for q in &points[i + 1..] {
                if let (Some(a), Some(b)) = (xy(sketch, *p), xy(sketch, *q))
                    && gc_math::fm::hypot(b[0] - a[0], b[1] - a[1]) <= close
                {
                    try_constraint(sketch, ids, GeoConstraint::Coincident(*p, *q), created);
                }
            }
        }
    }
    if !dimensions {
        return Ok(());
    }
    // Sizes first, then where things are: from the sketch's origin point.
    let mut candidates: Vec<DimKind> = Vec::new();
    for curve in curves {
        match sketch.curves.get(curve).map(|c| c.geom.clone()) {
            Some(gc_sketch::CurveGeom::Line { a, b }) => candidates.push(DimKind::Distance { p: a, q: b }),
            Some(gc_sketch::CurveGeom::Circle { .. }) => candidates.push(DimKind::Diameter(*curve)),
            Some(gc_sketch::CurveGeom::Arc { .. }) => candidates.push(DimKind::Radius(*curve)),
            // The major radius (centre to major axis end) and the minor radius.
            Some(gc_sketch::CurveGeom::Ellipse { c, major, .. }) => {
                candidates.push(DimKind::Distance { p: c, q: major });
                candidates.push(DimKind::Radius(*curve));
            }
            // A spline's fit points are placed by their own dimensions.
            Some(gc_sketch::CurveGeom::Spline { .. }) | None => {}
        }
    }
    let origin = sketch.points.iter().find_map(|(id, p)| {
        matches!(p.kind, gc_sketch::PointKind::External(gc_sketch::ExternalRef::FrameOrigin(_)))
            .then_some(*id)
    });
    if let Some(origin) = origin {
        for p in &points {
            if *p == origin {
                continue;
            }
            let (Some(o), Some(at)) = (xy(sketch, origin), xy(sketch, *p)) else { continue };
            if (at[0] - o[0]).abs() > close {
                candidates.push(DimKind::HDistance { p: origin, q: *p, sign: (at[0] - o[0]).signum() });
            }
            if (at[1] - o[1]).abs() > close {
                candidates.push(DimKind::VDistance { p: origin, q: *p, sign: (at[1] - o[1]).signum() });
            }
        }
    }
    for kind in candidates {
        let Some(value) = sketch.measure(&kind) else { continue };
        if !(value.is_finite() && value > close) {
            continue;
        }
        let source = format!("{:.6} mm", value * 1e3);
        let text_pos = auto_text_pos(sketch, &kind);
        if let Ok(gc_sketch::AddResult::Added { id, .. }) =
            sketch.try_add_dimension(ids, kind, &source, value, text_pos)
        {
            created.push(Created::Dimension(id));
        }
    }
    Ok(())
}

/// Where an automatic dimension's text goes: by what it measures.
fn auto_text_pos(sketch: &Sketch, kind: &DimKind) -> [f64; 2] {
    let at = |p: Id<SkPoint>| sketch.pos(p).map_or([0.0; 2], |q| [q[0], q[1]]);
    match *kind {
        DimKind::Distance { p, q } | DimKind::HDistance { p, q, .. } | DimKind::VDistance { p, q, .. } => {
            let (a, b) = (at(p), at(q));
            [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]
        }
        DimKind::Radius(c) | DimKind::Diameter(c) => {
            sketch.round(c).map_or([0.0; 2], |(centre, _)| at(centre))
        }
        _ => [0.0; 2],
    }
}

/// What a pattern added, as sketch items.
fn pattern_items(created: &[Created]) -> Vec<gc_sketch::Item> {
    created
        .iter()
        .filter_map(|c| match c {
            Created::Point(p) => Some(gc_sketch::Item::Point(*p)),
            Created::Curve(c) => Some(gc_sketch::Item::Curve(*c)),
            Created::Constraint(c) => Some(gc_sketch::Item::Constraint(*c)),
            Created::Dimension(d) => Some(gc_sketch::Item::Dimension(*d)),
            _ => None,
        })
        .collect()
}

/// The edit that makes a pattern from its values.
fn pattern_op(definition: &gc_sketch::PatternDefinition) -> SketchOp {
    match definition.clone() {
        gc_sketch::PatternDefinition::Rectangular { curves, first, second, fitted } => {
            SketchOp::RectangularPattern { curves, first, second, fitted }
        }
        gc_sketch::PatternDefinition::Circular { curves, axis, count, angle, flip, fitted } => {
            SketchOp::CircularPattern { curves, axis, count, angle, flip, fitted }
        }
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
        gc_sketch::CurveGeom::Circle { .. }
        | gc_sketch::CurveGeom::Ellipse { .. }
        | gc_sketch::CurveGeom::Spline { .. } => None,
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

/// What a sketch edit would make, worked out on a copy of the sketch (for a tool's preview): the copy
/// and what was created. `None` when the edit is refused. Expressions see no model parameters.
pub fn preview_sketch_op(sketch: &Sketch, op: &SketchOp) -> Option<(Sketch, Vec<Created>)> {
    let mut copy = sketch.clone();
    let highest = copy
        .points
        .keys()
        .map(|p| p.raw())
        .chain(copy.curves.keys().map(|c| c.raw()))
        .chain(copy.constraints.keys().map(|c| c.raw()))
        .chain(copy.dimensions.keys().map(|d| d.raw()))
        .max();
    let mut ids = IdAlloc::new();
    if let Some(highest) = highest {
        ids.reserve_through(highest);
    }
    let (created, _) = sketch_op(&mut copy, &mut ids, op, &gc_expr::NoParams).ok()?;
    Some((copy, created))
}

/// Adds a constraint unless it is redundant or conflicting (then silently left out).
pub(crate) fn try_constraint(
    sketch: &mut Sketch,
    ids: &mut IdAlloc,
    constraint: GeoConstraint,
    created: &mut Vec<Created>,
) {
    infer(sketch, ids, constraint, created);
}

/// A tangency between two curves as they lie now.
pub(crate) fn tangency_of(sketch: &Sketch, a: Id<Curve>, b: Id<Curve>) -> Option<GeoConstraint> {
    tangency(sketch, a, b)
}

/// Adds a constraint that the edit needs; refused or conflicting means the edit fails.
pub(crate) fn require_constraint(
    sketch: &mut Sketch,
    ids: &mut IdAlloc,
    constraint: GeoConstraint,
    created: &mut Vec<Created>,
) -> Result<(), EditError> {
    require(sketch, ids, constraint, created)
}

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
        SketchOp::AddPolygon { centre, at, sides, circumscribed } => {
            if !(3..=120).contains(sides) || !centre.iter().chain(at).all(|v| v.is_finite()) {
                return Err(SketchError::InvalidValue.into());
            }
            let (dx, dy) = (at[0] - centre[0], at[1] - centre[1]);
            let reach = gc_math::fm::hypot(dx, dy);
            if reach < 1e-9 {
                return Err(SketchError::InvalidValue.into());
            }
            let n = *sides as usize;
            let step = std::f64::consts::TAU / f64::from(*sides);
            let start = gc_math::fm::atan2(dy, dx);
            // Corners: on the circle (inscribed), or half a step either side of the clicked edge midpoint,
            // farther out (circumscribed).
            let (radius, first) = if *circumscribed {
                (reach / gc_math::fm::cos(step / 2.0), start - step / 2.0)
            } else {
                (reach, start)
            };
            #[allow(clippy::cast_precision_loss, reason = "at most 120 sides")]
            let corners: Vec<[f64; 3]> = (0..n)
                .map(|i| {
                    let angle = first + step * i as f64;
                    [
                        centre[0] + radius * gc_math::fm::cos(angle),
                        centre[1] + radius * gc_math::fm::sin(angle),
                        0.0,
                    ]
                })
                .collect();
            let lines: Vec<Id<Curve>> =
                (0..n).map(|i| sketch.add_line(ids, corners[i], corners[(i + 1) % n])).collect();
            let ends: Vec<(Id<SkPoint>, Id<SkPoint>)> =
                lines.iter().filter_map(|l| sketch.line(*l)).collect();
            let circle = sketch.add_circle(ids, *centre, reach)?;
            if let Some(c) = sketch.curves.get_mut(&circle) {
                c.construction = true;
            }
            created.extend(lines.iter().map(|l| Created::Curve(*l)));
            created.push(Created::Curve(circle));
            for i in 0..n {
                require(
                    sketch,
                    ids,
                    GeoConstraint::Coincident(ends[i].1, ends[(i + 1) % n].0),
                    &mut created,
                )?;
            }
            if *circumscribed {
                for line in &lines {
                    let tangent = tangency(sketch, *line, circle).ok_or(SketchError::InvalidValue)?;
                    require(sketch, ids, tangent, &mut created)?;
                }
                // Equal sides: with every edge touching the circle, an odd polygon is then regular; an
                // even one only up to alternating tangent lengths (a rhombus), which one edge touching
                // at its midpoint settles (and the last equality then follows).
                let equal = if n.is_multiple_of(2) { n - 2 } else { n - 1 };
                for i in 0..equal {
                    require(sketch, ids, GeoConstraint::Equal(lines[i], lines[i + 1]), &mut created)?;
                }
                if n.is_multiple_of(2) {
                    // A construction radius to the first edge's midpoint, square to it: the edge touches
                    // at its midpoint (a point merely on the circle would hold only to second order).
                    let radius_line = sketch.add_line(ids, *centre, *at);
                    if let Some(c) = sketch.curves.get_mut(&radius_line) {
                        c.construction = true;
                    }
                    created.push(Created::Curve(radius_line));
                    let (from, to) = sketch.line(radius_line).ok_or(SketchError::InvalidValue)?;
                    let middle = sketch.round(circle).map(|(c, _)| c).ok_or(SketchError::InvalidValue)?;
                    require(sketch, ids, GeoConstraint::Coincident(from, middle), &mut created)?;
                    require(sketch, ids, GeoConstraint::Midpoint(to, lines[0]), &mut created)?;
                    require(sketch, ids, GeoConstraint::Perpendicular(radius_line, lines[0]), &mut created)?;
                }
            } else {
                for (corner, _) in &ends {
                    require(sketch, ids, GeoConstraint::PointOnCurve(*corner, circle), &mut created)?;
                }
                for i in 0..n - 1 {
                    require(sketch, ids, GeoConstraint::Equal(lines[i], lines[i + 1]), &mut created)?;
                }
            }
            "Polygon"
        }
        SketchOp::AddSlot { a, b, width, kind } => {
            if !a.iter().chain(b).all(|v| v.is_finite()) || !(width.is_finite() && *width > 0.0) {
                return Err(SketchError::InvalidValue.into());
            }
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let span = gc_math::fm::hypot(dx, dy);
            let h = width / 2.0;
            let (d, n) = if span > 1e-12 {
                ([dx / span, dy / span], [-dy / span, dx / span])
            } else {
                ([1.0, 0.0], [0.0, 1.0])
            };
            let (c1, c2) = match kind {
                SlotKind::CenterToCenter => ([a[0], a[1]], [b[0], b[1]]),
                SlotKind::Overall => {
                    if span <= 2.0 * h {
                        return Err(SketchError::InvalidValue.into());
                    }
                    ([a[0] + d[0] * h, a[1] + d[1] * h], [b[0] - d[0] * h, b[1] - d[1] * h])
                }
                SlotKind::CenterPoint => ([2.0 * a[0] - b[0], 2.0 * a[1] - b[1]], [b[0], b[1]]),
            };
            if gc_math::fm::hypot(c2[0] - c1[0], c2[1] - c1[1]) < 1e-9 {
                return Err(SketchError::InvalidValue.into());
            }
            let p = |c: [f64; 2], s: f64| [c[0] + n[0] * h * s, c[1] + n[1] * h * s, 0.0];
            let centre_line = sketch.add_line(ids, [c1[0], c1[1], 0.0], [c2[0], c2[1], 0.0]);
            if let Some(c) = sketch.curves.get_mut(&centre_line) {
                c.construction = true;
            }
            let bottom = sketch.add_line(ids, p(c1, -1.0), p(c2, -1.0));
            let top = sketch.add_line(ids, p(c2, 1.0), p(c1, 1.0));
            // Counter-clockwise: round the far end from the bottom side to the top, and back at the start.
            let far = sketch.add_arc(ids, [c2[0], c2[1], 0.0], p(c2, -1.0), p(c2, 1.0))?;
            let near = sketch.add_arc(ids, [c1[0], c1[1], 0.0], p(c1, 1.0), p(c1, -1.0))?;
            created.extend([bottom, far, top, near, centre_line].map(Created::Curve));
            let arc_points =
                |sketch: &Sketch, arc: Id<Curve>| match sketch.curves.get(&arc).map(|c| c.geom.clone()) {
                    Some(gc_sketch::CurveGeom::Arc { c, s, e }) => Some((c, s, e)),
                    _ => None,
                };
            let (Some((fc, fs, fe)), Some((nc, ns, ne)), Some((la, lb)), Some((ba, bb)), Some((ta, tb))) = (
                arc_points(sketch, far),
                arc_points(sketch, near),
                sketch.line(centre_line),
                sketch.line(bottom),
                sketch.line(top),
            ) else {
                return Err(SketchError::InvalidValue.into());
            };
            for (p, q) in [(la, nc), (lb, fc), (bb, fs), (fe, ta), (tb, ns), (ne, ba)] {
                require(sketch, ids, GeoConstraint::Coincident(p, q), &mut created)?;
            }
            for (line, arc) in [(bottom, far), (top, far), (top, near), (bottom, near)] {
                let tangent = tangency(sketch, line, arc).ok_or(SketchError::InvalidValue)?;
                require(sketch, ids, tangent, &mut created)?;
            }
            require(sketch, ids, GeoConstraint::Equal(far, near), &mut created)?;
            "Slot"
        }
        SketchOp::AddArcSlot { centre, start, end, width } => {
            if !centre.iter().chain(start).chain(end).all(|v| v.is_finite())
                || !(width.is_finite() && *width > 0.0)
            {
                return Err(SketchError::InvalidValue.into());
            }
            let o = [centre[0], centre[1]];
            let angle = |p: &[f64; 3]| gc_math::fm::atan2(p[1] - o[1], p[0] - o[0]);
            let r = gc_math::fm::hypot(start[0] - o[0], start[1] - o[1]);
            let h = width / 2.0;
            let reach = gc_math::fm::hypot(end[0] - o[0], end[1] - o[1]);
            let (ts, te) = (angle(start), angle(end));
            let sweep = (te - ts).rem_euclid(std::f64::consts::TAU);
            if r < 1e-9 || reach < 1e-9 || h >= r || !(1e-6..std::f64::consts::TAU - 1e-6).contains(&sweep) {
                return Err(SketchError::InvalidValue.into());
            }
            let at = |radius: f64, t: f64| {
                [o[0] + radius * gc_math::fm::cos(t), o[1] + radius * gc_math::fm::sin(t), 0.0]
            };
            let o3 = [o[0], o[1], 0.0];
            let middle = sketch.add_arc(ids, o3, at(r, ts), at(r, te))?;
            let outer = sketch.add_arc(ids, o3, at(r + h, ts), at(r + h, te))?;
            let inner = sketch.add_arc(ids, o3, at(r - h, ts), at(r - h, te))?;
            // The ends bulge outwards: round the end from the outer arc to the inner, at the start back.
            let end_cap = sketch.add_arc(ids, at(r, te), at(r + h, te), at(r - h, te))?;
            let start_cap = sketch.add_arc(ids, at(r, ts), at(r - h, ts), at(r + h, ts))?;
            let radius_s = sketch.add_line(ids, o3, at(r + h, ts));
            let radius_e = sketch.add_line(ids, o3, at(r + h, te));
            for construction in [middle, radius_s, radius_e] {
                if let Some(c) = sketch.curves.get_mut(&construction) {
                    c.construction = true;
                }
            }
            created
                .extend([outer, end_cap, inner, start_cap, middle, radius_s, radius_e].map(Created::Curve));
            let arc = |sketch: &Sketch, id: Id<Curve>| match sketch.curves.get(&id).map(|c| c.geom.clone()) {
                Some(gc_sketch::CurveGeom::Arc { c, s, e }) => Some((c, s, e)),
                _ => None,
            };
            let (
                Some((mc, ms, me)),
                Some((oc, os, oe)),
                Some((ic, is, ie)),
                Some((ec, es, ee)),
                Some((sc, ss, se)),
                Some((ra, rb)),
                Some((qa, qb)),
            ) = (
                arc(sketch, middle),
                arc(sketch, outer),
                arc(sketch, inner),
                arc(sketch, end_cap),
                arc(sketch, start_cap),
                sketch.line(radius_s),
                sketch.line(radius_e),
            )
            else {
                return Err(SketchError::InvalidValue.into());
            };
            for (p, q) in [
                (oc, mc),
                (ic, mc),
                (ec, me),
                (sc, ms),
                (os, se),
                (oe, es),
                (is, ss),
                (ie, ee),
                (ra, mc),
                (rb, os),
                (qa, mc),
                (qb, oe),
            ] {
                require(sketch, ids, GeoConstraint::Coincident(p, q), &mut created)?;
            }
            for (p, line) in [(ms, radius_s), (is, radius_s), (me, radius_e), (ie, radius_e)] {
                try_constraint(sketch, ids, GeoConstraint::PointOnCurve(p, line), &mut created);
            }
            "Slot"
        }
        SketchOp::Mirror { curves, axis } => {
            let (Some((la, lb)), false) = (sketch.line(*axis), curves.contains(axis)) else {
                return Err(SketchError::InvalidValue.into());
            };
            let (Some(a), Some(b)) = (sketch.pos(la), sketch.pos(lb)) else {
                return Err(SketchError::MissingItem.into());
            };
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let length = gc_math::fm::hypot(dx, dy);
            if curves.is_empty() || length < 1e-12 {
                return Err(SketchError::InvalidValue.into());
            }
            let (ux, uy) = (dx / length, dy / length);
            let reflect = |p: [f64; 3]| {
                let (px, py) = (p[0] - a[0], p[1] - a[1]);
                let along = px * ux + py * uy;
                let (fx, fy) = (a[0] + along * ux, a[1] + along * uy);
                [2.0 * fx - p[0], 2.0 * fy - p[1], p[2]]
            };
            let off_line = |p: [f64; 3]| ((p[0] - a[0]) * uy - (p[1] - a[1]) * ux).abs() > 1e-9;
            let originals: Vec<Curve> = curves
                .iter()
                .map(|c| sketch.curves.get(c).cloned().ok_or(SketchError::MissingCurve))
                .collect::<Result<_, _>>()?;
            // Each point once: its copy (or itself on the mirror line).
            let mut copies: std::collections::BTreeMap<Id<SkPoint>, Id<SkPoint>> =
                std::collections::BTreeMap::new();
            for curve in &originals {
                for point in curve.points() {
                    if copies.contains_key(&point) {
                        continue;
                    }
                    let pos = sketch.pos(point).ok_or(SketchError::MissingItem)?;
                    let copy = if off_line(pos) { sketch.add_point(ids, reflect(pos)) } else { point };
                    copies.insert(point, copy);
                }
            }
            let map = |p: Id<SkPoint>| copies.get(&p).copied().ok_or(SketchError::MissingItem);
            let mut made = Vec::new();
            for (original, curve) in curves.iter().zip(&originals) {
                let geom = match curve.geom {
                    gc_sketch::CurveGeom::Line { a, b } => {
                        gc_sketch::CurveGeom::Line { a: map(a)?, b: map(b)? }
                    }
                    gc_sketch::CurveGeom::Circle { c, r } => gc_sketch::CurveGeom::Circle { c: map(c)?, r },
                    // A reflection turns the arc's direction round: it runs back from the copied end.
                    gc_sketch::CurveGeom::Arc { c, s, e } => {
                        gc_sketch::CurveGeom::Arc { c: map(c)?, s: map(e)?, e: map(s)? }
                    }
                    gc_sketch::CurveGeom::Spline { ref points } => gc_sketch::CurveGeom::Spline {
                        points: points.iter().map(|p| map(*p)).collect::<Result<_, _>>()?,
                    },
                    // Like an arc, an elliptical arc runs back from the copied end.
                    gc_sketch::CurveGeom::Ellipse { c, major, minor, arc } => gc_sketch::CurveGeom::Ellipse {
                        c: map(c)?,
                        major: map(major)?,
                        minor,
                        arc: match arc {
                            Some((s, e)) => Some((map(e)?, map(s)?)),
                            None => None,
                        },
                    },
                };
                let id: Id<Curve> = ids.alloc();
                sketch.curves.insert(id, Curve { geom, construction: curve.construction });
                created.push(Created::Curve(id));
                made.push((*original, id, matches!(curve.geom, gc_sketch::CurveGeom::Circle { .. })));
            }
            for (point, copy) in copies.clone() {
                if point != copy {
                    created.push(Created::Point(copy));
                    require(
                        sketch,
                        ids,
                        GeoConstraint::Symmetric { p: point, q: copy, axis: *axis },
                        &mut created,
                    )?;
                }
            }
            for (original, copy, circle) in made {
                if circle {
                    require(sketch, ids, GeoConstraint::Equal(original, copy), &mut created)?;
                }
            }
            "Mirror"
        }
        SketchOp::RectangularPattern { curves, first, second, fitted } => {
            let directions: Vec<&SketchPatternDirection> =
                std::iter::once(first).chain(second.as_ref()).collect();
            let mut steps = Vec::new();
            for d in &directions {
                if !(2..=500).contains(&d.count) {
                    return Err(SketchError::InvalidValue.into());
                }
                let (a, b) = sketch.line(d.line).ok_or(SketchError::InvalidValue)?;
                let (pa, pb) = (
                    sketch.pos(a).ok_or(SketchError::MissingItem)?,
                    sketch.pos(b).ok_or(SketchError::MissingItem)?,
                );
                let length = gc_math::fm::hypot(pb[0] - pa[0], pb[1] - pa[1]);
                if length < 1e-12 || curves.contains(&d.line) {
                    return Err(SketchError::InvalidValue.into());
                }
                let sign = if d.flip { -1.0 } else { 1.0 };
                // The spacing between neighbours (fitted: the whole extent shared out).
                let source =
                    if *fitted { format!("({})/{}", d.spacing, d.count - 1) } else { d.spacing.clone() };
                let spacing = evaluate(&source, scope, Unit::Mm)?;
                if !(spacing.is_finite() && spacing > 0.0) {
                    return Err(SketchError::InvalidValue.into());
                }
                steps.push((
                    [sign * (pb[0] - pa[0]) / length, sign * (pb[1] - pa[1]) / length],
                    spacing,
                    source,
                    d.count,
                ));
            }
            let originals = pattern_sources(sketch, curves)?;
            let from = *originals.points.first().ok_or(SketchError::InvalidValue)?;
            let start = sketch.pos(from).ok_or(SketchError::MissingItem)?;
            // One construction step per direction from the first original point: parallel to the direction,
            // carrying the spacing dimension. The copies follow the steps (gc_sketch `refresh_patterns`).
            let mut ends = Vec::new();
            for (axis, (dir, spacing, source, _)) in steps.iter().enumerate() {
                let end = sketch
                    .add_point(ids, [start[0] + dir[0] * spacing, start[1] + dir[1] * spacing, start[2]]);
                created.push(Created::Point(end));
                let step = construction_between(sketch, ids, from, end, &mut created)?;
                require(sketch, ids, GeoConstraint::Parallel(step, directions[axis].line), &mut created)?;
                let kind = DimKind::Distance { p: from, q: end };
                let result = sketch.try_add_dimension(ids, kind, source, *spacing, [0.0; 2])?;
                created.push(Created::Dimension(added(result)?));
                ends.push(end);
            }
            let step =
                gc_sketch::PatternStep::Translate { from, first: ends[0], second: ends.get(1).copied() };
            let counts = (steps[0].3, steps.get(1).map_or(1, |s| s.3));
            let offset = |i: u32, j: u32| {
                let mut v = [0.0; 2];
                for (k, n) in [i, j].into_iter().enumerate() {
                    if let Some((dir, spacing, _, _)) = steps.get(k) {
                        v[0] += f64::from(n) * dir[0] * spacing;
                        v[1] += f64::from(n) * dir[1] * spacing;
                    }
                }
                v
            };
            let mut record = PatternBuild::default();
            for j in 0..counts.1 {
                for i in 0..counts.0 {
                    if (i, j) == (0, 0) {
                        continue;
                    }
                    let v = offset(i, j);
                    record.element(
                        sketch,
                        ids,
                        &originals,
                        [i, j],
                        |p| [p[0] + v[0], p[1] + v[1], p[2]],
                        &mut created,
                    )?;
                }
            }
            record.finish(
                sketch,
                gc_sketch::PatternDefinition::Rectangular {
                    curves: curves.clone(),
                    first: first.clone(),
                    second: second.clone(),
                    fitted: *fitted,
                },
                step,
                &created,
            );
            "Rectangular pattern"
        }
        SketchOp::CircularPattern { curves, axis, count, angle, flip, fitted } => {
            if !(2..=500).contains(count) {
                return Err(SketchError::InvalidValue.into());
            }
            let centre = sketch.pos(*axis).ok_or(SketchError::MissingPoint)?;
            let source = if *fitted {
                // Fitted: the angle is shared out; a full turn leaves no element on the first one.
                let total = evaluate(angle, scope, Unit::Deg)?;
                let full = (total.abs() - std::f64::consts::TAU).abs() < 1e-9;
                format!("({angle})/{}", if full { *count } else { count - 1 })
            } else {
                angle.clone()
            };
            let step = evaluate(&source, scope, Unit::Deg)? * if *flip { -1.0 } else { 1.0 };
            if !step.is_finite() || step.abs() < 1e-12 {
                return Err(SketchError::InvalidValue.into());
            }
            let originals = pattern_sources(sketch, curves)?;
            let off_axis = |p: &[f64; 3]| gc_math::fm::hypot(p[0] - centre[0], p[1] - centre[1]) >= 1e-12;
            // One spoke from the axis to an original point off it, and one as long turned by the step angle
            // (it carries the angle dimension). The copies follow the turn (gc_sketch `refresh_patterns`).
            let from = originals
                .points
                .iter()
                .copied()
                .find(|p| *p != *axis && sketch.pos(*p).is_some_and(|q| off_axis(&q)))
                .ok_or(SketchError::InvalidValue)?;
            let turn = |p: [f64; 3], k: u32| {
                let (sin, cos) =
                    (gc_math::fm::sin(step * f64::from(k)), gc_math::fm::cos(step * f64::from(k)));
                let (x, y) = (p[0] - centre[0], p[1] - centre[1]);
                [centre[0] + cos * x - sin * y, centre[1] + sin * x + cos * y, p[2]]
            };
            let start = sketch.pos(from).ok_or(SketchError::MissingItem)?;
            let spoke = construction_between(sketch, ids, *axis, from, &mut created)?;
            let to = sketch.add_point(ids, turn(start, 1));
            created.push(Created::Point(to));
            let turned = construction_between(sketch, ids, *axis, to, &mut created)?;
            require(sketch, ids, GeoConstraint::Equal(turned, spoke), &mut created)?;
            // Counter-clockwise from the earlier spoke (from the later one when turning the other way).
            let (l1, l2) = if step > 0.0 { (spoke, turned) } else { (turned, spoke) };
            let kind = DimKind::Angle { l1, rev1: false, l2, rev2: false };
            let result = sketch.try_add_dimension(ids, kind, &source, step.abs(), [0.0; 2])?;
            created.push(Created::Dimension(added(result)?));
            let mut record = PatternBuild::default();
            for k in 1..*count {
                record.element(sketch, ids, &originals, [k, 0], |p| turn(p, k), &mut created)?;
            }
            record.finish(
                sketch,
                gc_sketch::PatternDefinition::Circular {
                    curves: curves.clone(),
                    axis: *axis,
                    count: *count,
                    angle: angle.clone(),
                    flip: *flip,
                    fitted: *fitted,
                },
                gc_sketch::PatternStep::Rotate { axis: *axis, from, to },
                &created,
            );
            "Circular pattern"
        }
        SketchOp::EditSketchPattern { element, definition, suppressed } => {
            let index = sketch.pattern_of(*element).ok_or(SketchError::InvalidValue)?;
            let old = sketch.patterns.remove(index);
            sketch.remove_exactly(&old.items);
            let (rebuilt, _) = sketch_op(sketch, ids, &pattern_op(definition), scope)?;
            created.extend(rebuilt);
            let last = sketch.patterns.len().saturating_sub(1);
            sketch.suppress_pattern_elements(last, suppressed, true);
            "Edit pattern"
        }
        SketchOp::DeleteSketchPattern { element } => {
            let index = sketch.pattern_of(*element).ok_or(SketchError::InvalidValue)?;
            let old = sketch.patterns.remove(index);
            sketch.remove_exactly(&old.items);
            "Delete pattern"
        }
        SketchOp::SuppressPatternElements { curves, suppress } => {
            let mut by_pattern: std::collections::BTreeMap<usize, Vec<usize>> =
                std::collections::BTreeMap::new();
            for curve in curves {
                let (pattern, element) =
                    sketch.pattern_element_of(*curve).ok_or(SketchError::InvalidValue)?;
                by_pattern.entry(pattern).or_default().push(element);
            }
            if by_pattern.is_empty() {
                return Err(SketchError::InvalidValue.into());
            }
            for (pattern, elements) in by_pattern {
                sketch.suppress_pattern_elements(pattern, &elements, *suppress);
            }
            if *suppress { "Suppress elements" } else { "Unsuppress elements" }
        }
        SketchOp::Offset { curves, distance, loop_select, constrain, picked } => {
            crate::sketch_offset::offset(
                sketch,
                ids,
                curves,
                *distance,
                *loop_select,
                *constrain,
                *picked,
                &mut created,
            )?
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
        SketchOp::AddSpline { points, snaps } => {
            let spline = sketch.add_spline(ids, points)?;
            created.push(Created::Curve(spline));
            let fit = if let Some(gc_sketch::CurveGeom::Spline { points }) =
                sketch.curves.get(&spline).map(|c| &c.geom)
            {
                points.clone()
            } else {
                Vec::new()
            };
            for (point, snap) in fit.into_iter().zip(snaps) {
                snap_constraint(sketch, ids, point, snap, &mut created);
            }
            "Spline"
        }
        SketchOp::AddEllipse { centre, major, minor, centre_snap } => {
            let ellipse = sketch.add_ellipse(ids, *centre, *major, *minor)?;
            created.push(Created::Curve(ellipse));
            if let Some((c, _, _)) = sketch.ellipse(ellipse) {
                snap_constraint(sketch, ids, c, centre_snap, &mut created);
            }
            "Ellipse"
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
        SketchOp::AddSpatialArc { centre, start, end, snaps } => {
            if !sketch.is_spatial() || !centre.iter().chain(start).chain(end).all(|v| v.is_finite()) {
                return Err(SketchError::InvalidValue.into());
            }
            let arc = gc_sketch::Arc3::new(*centre, *start, *end).ok_or(SketchError::InvalidValue)?;
            if arc.sweep > std::f64::consts::PI - 1e-6 {
                return Err(SketchError::InvalidValue.into());
            }
            let ends = [*centre, *start, arc.at(1.0)];
            let points = ends.map(|p| sketch.add_point(ids, p));
            let id: Id<Curve> = ids.alloc();
            sketch.curves.insert(
                id,
                Curve {
                    geom: gc_sketch::CurveGeom::Arc { c: points[0], s: points[1], e: points[2] },
                    construction: false,
                },
            );
            created.extend(points.iter().map(|p| Created::Point(*p)));
            created.push(Created::Curve(id));
            for (point, snap) in points.iter().zip(snaps) {
                if let Some(snap) = snap {
                    require(sketch, ids, GeoConstraint::Coincident(*snap, *point), &mut created)?;
                }
            }
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
        SketchOp::AddMidpointLine { mid, end, snap_mid, snap_end } => {
            if !mid.iter().chain(end).all(|v| v.is_finite()) {
                return Err(SketchError::InvalidValue.into());
            }
            let start = [2.0 * mid[0] - end[0], 2.0 * mid[1] - end[1], 0.0];
            if gc_math::fm::hypot(end[0] - mid[0], end[1] - mid[1]) < 1e-12 {
                return Err(SketchError::InvalidValue.into());
            }
            let line = sketch.add_line(ids, start, [end[0], end[1], 0.0]);
            created.push(Created::Curve(line));
            let middle = sketch.add_point(ids, [mid[0], mid[1], 0.0]);
            created.push(Created::Point(middle));
            require(sketch, ids, GeoConstraint::Midpoint(middle, line), &mut created)?;
            snap_constraint(sketch, ids, middle, snap_mid, &mut created);
            if let Some((_, b)) = sketch.line(line) {
                snap_constraint(sketch, ids, b, snap_end, &mut created);
            }
            "Midpoint line"
        }
        SketchOp::AddCenterRectangle { centre, corner, snap_centre } => {
            let (c, k) = ([centre[0], centre[1]], [corner[0], corner[1]]);
            if !c.iter().chain(&k).all(|v| v.is_finite())
                || (k[0] - c[0]).abs() < 1e-12
                || (k[1] - c[1]).abs() < 1e-12
            {
                return Err(SketchError::InvalidValue.into());
            }
            let rectangle = SketchOp::AddRectangle {
                a: [2.0 * c[0] - k[0], 2.0 * c[1] - k[1]],
                b: k,
                snap_a: EndSnap::default(),
            };
            let (inner, _) = sketch_op(sketch, ids, &rectangle, scope)?;
            centre_point(sketch, ids, &inner, *centre, snap_centre, &mut created)?;
            created.splice(0..0, inner);
            "Rectangle"
        }
        SketchOp::AddCenterRectangle3 { centre, side, width, snap_centre } => {
            let u = [side[0] - centre[0], side[1] - centre[1]];
            let half = gc_math::fm::hypot(u[0], u[1]);
            if !(half.is_finite() && half > 1e-12) || !width.iter().all(|v| v.is_finite()) {
                return Err(SketchError::InvalidValue.into());
            }
            // Across: the width point's distance from the line through the centre and the side's middle.
            let n = [-u[1] / half, u[0] / half];
            let h = (width[0] - centre[0]) * n[0] + (width[1] - centre[1]) * n[1];
            if h.abs() < 1e-12 {
                return Err(SketchError::InvalidValue.into());
            }
            let at = |su: f64, sn: f64| {
                [centre[0] + su * u[0] + sn * h * n[0], centre[1] + su * u[1] + sn * h * n[1], 0.0]
            };
            let rectangle = SketchOp::AddRectangle3 {
                a: at(1.0, -1.0),
                b: at(1.0, 1.0),
                c: at(-1.0, 1.0),
                snap_a: EndSnap::default(),
            };
            let (inner, _) = sketch_op(sketch, ids, &rectangle, scope)?;
            centre_point(sketch, ids, &inner, *centre, snap_centre, &mut created)?;
            created.splice(0..0, inner);
            "Rectangle"
        }
        SketchOp::AddTangentCircle { lines, near } => {
            let (centre, radius) = tangent_circle(sketch, lines, near).ok_or(SketchError::InvalidValue)?;
            let circle = sketch.add_circle(ids, [centre[0], centre[1], 0.0], radius)?;
            created.push(Created::Curve(circle));
            for line in lines {
                let tangent = tangency(sketch, *line, circle).ok_or(SketchError::InvalidValue)?;
                require(sketch, ids, tangent, &mut created)?;
            }
            "Tangent circle"
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
            // Delete removes nonpattern geometry (Inventor 2026 help, To Create and Edit Sketch Patterns,
            // GUID-AAD3A699-FED8-4614-A31D-0A2316328CEE): what a pattern made is left to Delete Pattern and
            // Suppress; an original takes its pattern with it.
            let made: std::collections::BTreeSet<gc_sketch::Item> =
                sketch.patterns.iter().flat_map(|p| p.items.iter().copied()).collect();
            let items: Vec<gc_sketch::Item> = items.iter().copied().filter(|i| !made.contains(i)).collect();
            let doomed: Vec<usize> = sketch
                .patterns
                .iter()
                .enumerate()
                .filter(|(_, p)| {
                    p.definition.curves().iter().any(|c| items.contains(&gc_sketch::Item::Curve(*c)))
                })
                .map(|(i, _)| i)
                .collect();
            for index in doomed.into_iter().rev() {
                let old = sketch.patterns.remove(index);
                sketch.remove_exactly(&old.items);
            }
            sketch.delete(&items);
            "Delete"
        }
        // Resolved against the document in `run`; inside a wrapper it is not supported.
        SketchOp::RenameDimension { .. } => return Err(SketchError::InvalidValue.into()),
        SketchOp::Project { .. } => return Err(SketchError::InvalidValue.into()),
        SketchOp::Trim { curve, at } => {
            push_cut(&mut created, &sketch.trim(ids, *curve, *at)?);
            "Trim"
        }
        SketchOp::Transform { curves, motion, copy, relax } => {
            let place = |p: [f64; 3]| motion.place(p);
            let fine = match *motion {
                Motion::Translate { by } => by.iter().all(|v| v.is_finite()),
                Motion::Rotate { centre, angle } => centre.iter().all(|v| v.is_finite()) && angle.is_finite(),
                Motion::Scale { centre, factor } => {
                    centre.iter().all(|v| v.is_finite()) && factor.is_finite() && factor > 1e-9
                }
            };
            if !fine {
                return Err(SketchError::InvalidValue.into());
            }
            if *copy {
                let result = sketch.copy_placed(ids, curves, &place, motion.radius_factor())?;
                created.extend(result.points.iter().map(|p| Created::Point(*p)));
                created.extend(result.curves.iter().map(|c| Created::Curve(*c)));
                created.extend(result.constraints.iter().map(|c| Created::Constraint(*c)));
                "Copy"
            } else {
                match motion {
                    Motion::Scale { factor, .. } => {
                        // Relaxed, a scale also gives the selection's own dimensions their new sizes.
                        let rules = gc_sketch::MoveRules {
                            break_ties: *relax,
                            break_level: false,
                            recompute_crossing: *relax,
                            recompute_inside: *relax,
                            radius: *factor,
                        };
                        sketch.move_with(ids, curves, &[], &place, rules)?;
                        "Scale"
                    }
                    Motion::Rotate { .. } => {
                        sketch.move_placed(ids, curves, &place, true, *relax)?;
                        "Rotate"
                    }
                    Motion::Translate { .. } => {
                        sketch.move_placed(ids, curves, &place, false, *relax)?;
                        "Move"
                    }
                }
            }
        }
        SketchOp::Stretch { curves, points, by, relax } => {
            if !by.iter().all(|v| v.is_finite()) {
                return Err(SketchError::InvalidValue.into());
            }
            let motion = Motion::Translate { by: *by };
            // Connected geometry stretches: nothing is broken; relaxed, the dimensions across the
            // selection's edge take their new values.
            let rules = gc_sketch::MoveRules {
                break_ties: false,
                break_level: false,
                recompute_crossing: *relax,
                recompute_inside: false,
                radius: 1.0,
            };
            sketch.move_with(ids, curves, points, &|p| motion.place(p), rules)?;
            "Stretch"
        }
        SketchOp::FlipDimensions(dims) => {
            if dims.is_empty() {
                return Err(SketchError::InvalidValue.into());
            }
            for id in dims {
                let dimension = sketch.dimensions.get_mut(id).ok_or(SketchError::MissingItem)?;
                dimension.kind = flipped(&dimension.kind).ok_or(SketchError::InvalidValue)?;
            }
            "Flip direction"
        }
        SketchOp::AutoDimension { curves, dimensions, constraints } => {
            if sketch.is_spatial() {
                return Err(SketchError::WrongSpace.into());
            }
            let chosen: Vec<Id<Curve>> =
                if curves.is_empty() { sketch.curves.keys().copied().collect() } else { curves.clone() };
            auto_dimension(sketch, ids, &chosen, *dimensions, *constraints, &mut created)?;
            "Automatic dimensions"
        }
        SketchOp::Split { curve, at } => {
            push_cut(&mut created, &sketch.split_at(ids, *curve, *at)?);
            "Split"
        }
        SketchOp::SymmetricCurves { first, second, axis } => {
            let pairs = sketch.symmetric_pairs(*first, *second, *axis).ok_or(SketchError::InvalidValue)?;
            for constraint in pairs {
                match sketch.try_add_constraint(ids, constraint)? {
                    AddResult::Added { id, .. } => created.push(Created::Constraint(id)),
                    AddResult::Redundant(_) => {}
                    AddResult::Conflicting(with) => return Err(EditError::Conflicting(with)),
                }
            }
            if created.is_empty() {
                return Err(SketchError::InvalidValue.into());
            }
            "Symmetric"
        }
        SketchOp::CloseSpline { curve } => {
            sketch.close_spline(*curve)?;
            "Close Curve"
        }
        SketchOp::InsertSplinePoint { curve, at } => {
            created.push(Created::Point(sketch.insert_spline_point(ids, *curve, *at)?));
            "Insert Point"
        }
        SketchOp::SplitSpline { curve, point } => {
            created.push(Created::Curve(sketch.split_spline(ids, *curve, *point)?));
            "Split Spline"
        }
        SketchOp::Chamfer { lines, kind, dimensions } => {
            let length = |text: &str| -> Result<f64, EditError> {
                let v = evaluate(text, scope, Unit::Mm)?;
                if v.is_finite() && v > 0.0 { Ok(v) } else { Err(SketchError::InvalidValue.into()) }
            };
            let distances = match kind {
                ChamferKind::Equal(d) => [length(d)?; 2],
                ChamferKind::Unequal(d1, d2) => [length(d1)?, length(d2)?],
                ChamferKind::DistanceAngle { distance, angle } => {
                    let d2 = length(distance)?;
                    let alpha = evaluate(angle, scope, Unit::Deg)?;
                    // The corner's angle, from where the lines were picked.
                    // (A tiny cut, only to find the corner.)
                    let (corner, _) =
                        sketch.chamfer_plan(*lines, [1e-9, 1e-9]).ok_or(SketchError::InvalidValue)?;
                    let towards = |(line, at): (Id<Curve>, [f64; 2])| -> Option<[f64; 2]> {
                        let (a, b) = sketch.line(line)?;
                        let (a, b) = (sketch.pos(a)?, sketch.pos(b)?);
                        let d = [b[0] - a[0], b[1] - a[1]];
                        let n = gc_math::fm::hypot(d[0], d[1]);
                        let s = (at[0] - corner[0]) * d[0] + (at[1] - corner[1]) * d[1];
                        Some(if s >= 0.0 { [d[0] / n, d[1] / n] } else { [-d[0] / n, -d[1] / n] })
                    };
                    let (u1, u2) = (
                        towards(lines[0]).ok_or(SketchError::InvalidValue)?,
                        towards(lines[1]).ok_or(SketchError::InvalidValue)?,
                    );
                    let theta = gc_math::fm::acos((u1[0] * u2[0] + u1[1] * u2[1]).clamp(-1.0, 1.0));
                    // Triangle corner, cut 1, cut 2: angle alpha at cut 1, theta at the corner.
                    let third = std::f64::consts::PI - alpha - theta;
                    if !(alpha > 0.0 && third > 1e-9) {
                        return Err(SketchError::InvalidValue.into());
                    }
                    [d2 * gc_math::fm::sin(third) / gc_math::fm::sin(alpha), d2]
                }
            };
            let (corner, _) = sketch.chamfer_plan(*lines, distances).ok_or(SketchError::InvalidValue)?;
            let cut = sketch.chamfer(ids, *lines, distances)?;
            push_cut(&mut created, &cut);
            if *dimensions {
                let bevel = *cut.curves.first().ok_or(SketchError::InvalidValue)?;
                let (c1, c2) = sketch.line(bevel).ok_or(SketchError::MissingCurve)?;
                // The corner as a point held on both lines (extended).
                let x = sketch.add_point(ids, [corner[0], corner[1], 0.0]);
                created.push(Created::Point(x));
                for (line, _) in lines {
                    require(sketch, ids, GeoConstraint::PointOnCurve(x, *line), &mut created)?;
                }
                let mut dimension =
                    |sketch: &mut Sketch, kind: DimKind, source: &str, value: f64| -> Result<(), EditError> {
                        let result = sketch.try_add_dimension(ids, kind, source, value, corner)?;
                        created.push(Created::Dimension(added(result)?));
                        Ok(())
                    };
                match kind {
                    ChamferKind::Equal(d) => {
                        dimension(sketch, DimKind::Distance { p: x, q: c1 }, d, distances[0])?;
                        dimension(sketch, DimKind::Distance { p: x, q: c2 }, d, distances[1])?;
                    }
                    ChamferKind::Unequal(d1, d2) => {
                        dimension(sketch, DimKind::Distance { p: x, q: c1 }, d1, distances[0])?;
                        dimension(sketch, DimKind::Distance { p: x, q: c2 }, d2, distances[1])?;
                    }
                    ChamferKind::DistanceAngle { distance, angle } => {
                        dimension(sketch, DimKind::Distance { p: x, q: c2 }, distance, distances[1])?;
                        let alpha = evaluate(angle, scope, Unit::Deg)?;
                        // The reading of the angle between the first line and the bevel that gives alpha.
                        let (first, _) = lines[0];
                        let reading = [(false, false), (false, true), (true, false), (true, true)]
                            .into_iter()
                            .flat_map(|(rev1, rev2)| {
                                [
                                    DimKind::Angle { l1: first, rev1, l2: bevel, rev2 },
                                    DimKind::Angle { l1: bevel, rev1, l2: first, rev2 },
                                ]
                            })
                            .find(|k| sketch.measure(k).is_some_and(|m| (m - alpha).abs() < 1e-6))
                            .ok_or(SketchError::InvalidValue)?;
                        dimension(sketch, reading, angle, alpha)?;
                    }
                }
            }
            "Chamfer"
        }
        SketchOp::Extend { curve, at } => {
            push_cut(&mut created, &sketch.extend(ids, *curve, *at)?);
            "Extend"
        }
        SketchOp::AddLineBent { line, previous, radius } => {
            let SketchOp::AddLine { .. } = **line else { return Err(SketchError::InvalidValue.into()) };
            let (made, _) = sketch_op(sketch, ids, line, scope)?;
            let new = made.iter().find_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None });
            created.extend(made);
            if let Some(new) = new {
                let before = sketch.clone();
                let bend = SketchOp::Bend { lines: [*previous, new], radius: radius.clone() };
                match sketch_op(sketch, ids, &bend, scope) {
                    Ok((bent, _)) => created.extend(bent),
                    Err(_) => *sketch = before,
                }
            }
            "Line"
        }
        SketchOp::Bend { lines, radius } => {
            let value = evaluate(radius, scope, Unit::Mm)?;
            let cut = sketch.bend(ids, *lines, value)?;
            push_cut(&mut created, &cut);
            let arc = *cut.curves.first().ok_or(SketchError::InvalidValue)?;
            let text_pos =
                sketch.round(arc).and_then(|(c, _)| sketch.pos(c)).map_or([0.0; 2], |c| [c[0], c[1]]);
            let result = sketch.try_add_dimension(ids, DimKind::Radius(arc), radius, value, text_pos)?;
            created.push(Created::Dimension(added(result)?));
            "Bend"
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
    sketch.prune_patterns();
    // Whatever the edit was, the sketch must end up consistent.
    let report = sketch.solve()?;
    if report.status != Status::Solved {
        return Err(SketchError::Unsolved(report.status).into());
    }
    Ok((created, label))
}

pub(crate) fn run(txn: &mut Txn<'_>, command: &Command) -> Result<(Vec<Created>, &'static str), EditError> {
    match command {
        Command::Sequence(commands) => {
            let mut created = Vec::new();
            let mut label = "Edit";
            for command in commands {
                let (more, last) = run(txn, command)?;
                created.extend(more);
                label = last;
            }
            if commands.is_empty() {
                return Err(EditError::BadValue);
            }
            Ok((created, label))
        }
        Command::Owned { owner, command } => {
            if owner.is_some_and(|o| !txn.doc().has_tree_row(o)) {
                return Err(EditError::BadValue);
            }
            let (created, label) = run(txn, command)?;
            let sketches: Vec<Id<Sketch>> = created
                .iter()
                .filter_map(|c| if let Created::Sketch(id) = c { Some(*id) } else { None })
                .collect();
            if let Some(owner) = *owner
                && !sketches.is_empty()
            {
                crate::cell::set_timeline(txn, |t| {
                    for sketch in &sketches {
                        t.components.sketch_owners.retain(|(s, _)| s != sketch);
                        t.components.sketch_owners.push((*sketch, owner));
                    }
                })?;
            }
            Ok((created, label))
        }
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
            let name = sketch_name(txn.doc(), name);
            let frame = frame.erase();
            let mut sketch = Sketch::new(if *spatial { Space::Spatial(frame) } else { Space::Planar(frame) });
            sketch.name = name;
            let id = txn.insert(sketch);
            txn.modify(id, |s: &mut Sketch, ids| {
                s.add_external(ids, ExternalRef::FrameOrigin(frame), [0.0; 3]);
            })?;
            Ok((vec![Created::Sketch(id)], "New sketch"))
        }
        Command::CreateSketchOn { name, source } => {
            let name = &sketch_name(txn.doc(), name);
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
            let present = |doc: &Document| -> Vec<bool> {
                let Some(s) = doc.get(*sketch) else { return Vec::new() };
                doc.work_features()
                    .iter()
                    .flat_map(|(_, w)| {
                        let curves = w.geometry.curves().into_iter().filter(|r| r.sketch == *sketch);
                        let points = w.geometry.sketch_points().into_iter().filter(|(k, _)| k == sketch);
                        curves
                            .map(|r| s.curves.contains_key(&r.curve))
                            .chain(points.map(|(_, p)| s.points.contains_key(&p)))
                            .collect::<Vec<bool>>()
                    })
                    .collect()
            };
            let before = present(txn.doc());
            let result = txn.modify(*sketch, |s: &mut Sketch, ids| sketch_op(s, ids, op, &scope))??;
            // A curve or point a work feature is built on cannot go (like a sketch a feature uses).
            if before.iter().zip(present(txn.doc())).any(|(was, is)| *was && !is) {
                return Err(EditError::InUse);
            }
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
