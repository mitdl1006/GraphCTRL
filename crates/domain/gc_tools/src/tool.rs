//! The tools.

#[path = "shapes.rs"]
mod shapes;

use gc_core::Id;
use gc_doc::{DimSlot, EndSnap, SketchOp, tangent_arc};
use gc_math::fm;
use gc_sketch::{Curve, CurveGeom, DimKind, GeoConstraint, Item, Piece, SkPoint, Sketch, SketchGrab};

use crate::pick::{Shape, Snap, crossings, curve_shape, nearest_curve, pick, snap};

/// Keys tools react to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// Step back: cancel the current input, or leave the tool.
    Escape,
    /// Delete the selection.
    Delete,
    /// Line tool: take back the last segment of the chain.
    Backspace,
    /// Finish the current shape with the values typed into the input boxes.
    Enter,
}

/// Commands from the right-click menu while a tool runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolCommand {
    /// Line tool: draw the closing segment back to where the chain started; the tool stays on.
    Close,
    /// End the current input (like Esc) without leaving the tool.
    Done,
    /// Shift went down (`true`) or up: Trim works as Extend and Extend as Trim while it is held.
    Alternate(bool),
}

/// An input box of the heads-up display next to the cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum HudField {
    /// Length of a line or of a rectangle's first side.
    Length,
    /// Direction of a line, degrees from the sketch's X axis.
    Angle,
    /// Radius of an arc or a fillet.
    Radius,
    /// Diameter of a circle.
    Diameter,
    /// Width of a rectangle (first side for the three-point rectangle: the second side).
    Width,
    /// Height of a two-point rectangle.
    Height,
    /// Offset distance.
    Distance,
    /// A point's X coordinate in the sketch (the Point tool).
    X,
    /// A point's Y coordinate in the sketch (the Point tool).
    Y,
}

/// One input box as the display should show it now.
#[derive(Clone, Debug, PartialEq)]
pub struct HudSlot {
    /// Which box.
    pub field: HudField,
    /// The value the cursor gives right now, in SI units (metres, radians).
    pub measured: f64,
    /// The value typed and locked, if any (shown with a lock).
    pub locked: Option<String>,
}

/// Input to a tool. Positions are sketch coordinates; `px` is the size of one screen pixel in sketch units.
#[derive(Clone, Debug, PartialEq)]
pub enum ToolEvent {
    /// The cursor moved without a button held.
    Move {
        /// Cursor position.
        pos: [f64; 2],
        /// Size of a pixel.
        px: f64,
        /// Ctrl is held: do not snap.
        suppress_snap: bool,
    },
    /// A click (press and release without moving).
    Click {
        /// Cursor position.
        pos: [f64; 2],
        /// Size of a pixel.
        px: f64,
        /// Ctrl is held: do not snap; in the select tool, add to the selection.
        modifier: bool,
    },
    /// A click whose target was found by the caller (3D sketches pick in screen space).
    ClickItem {
        /// What was clicked, if anything.
        item: Option<Item>,
        /// Where a dimension text placed by this click goes.
        pos: [f64; 2],
        /// True while Ctrl is held.
        modifier: bool,
    },
    /// The primary button went down and the cursor started moving.
    DragStart {
        /// Where the button went down.
        pos: [f64; 2],
        /// Size of a pixel.
        px: f64,
    },
    /// The cursor moved with the button held.
    Drag {
        /// Cursor position.
        pos: [f64; 2],
    },
    /// The button was released after a drag.
    DragEnd,
    /// A key was pressed.
    Key(Key),
    /// The user confirmed the value box that a [`ToolOutput::RequestValue`] opened.
    Value(String),
    /// The user dismissed the value box.
    CancelValue,
    /// A double click: the line tool ends its chain there.
    DoubleClick {
        /// Cursor position.
        pos: [f64; 2],
        /// Size of a pixel.
        px: f64,
        /// Ctrl is held: do not snap.
        modifier: bool,
    },
    /// A command from the right-click menu.
    Command(ToolCommand),
    /// A value typed into an input box (`Some`: locked to it) or cleared (`None`).
    Hud {
        /// Which box.
        field: HudField,
        /// The value as typed (a number, with or without a unit, or an expression).
        value: Option<String>,
    },
}

/// The geometric constraints the constraint tool can add.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[allow(missing_docs, reason = "the names are the documentation")]
pub enum ConstraintKind {
    Coincident,
    Horizontal,
    Vertical,
    Parallel,
    Perpendicular,
    Collinear,
    Tangent,
    Equal,
    Concentric,
    Midpoint,
    Fix,
    Symmetric,
    /// 3D: the line or the two points lie along X, Y or Z.
    AlongX,
    AlongY,
    AlongZ,
    /// 3D: the point lies on the XY, YZ or ZX plane through the sketch's origin.
    OnXY,
    OnYZ,
    OnZX,
}

/// Which tool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolKind {
    /// Select, drag, delete.
    Select,
    /// Draw chained lines.
    Line,
    /// Draw a circle by centre and a point on it.
    Circle,
    /// Draw an arc by centre, start point and end point. The arc runs the way the cursor swept.
    Arc,
    /// Draw a rectangle by two opposite corners.
    Rectangle,
    /// Draw an arc through three points: its two ends, then a point that sets the side and the radius.
    Arc3,
    /// Draw an arc that continues an existing line or arc from its end: click the end, then where the
    /// arc goes.
    ArcTangent,
    /// Draw a rectangle at any angle: two corners along one side, then the width.
    Rectangle3,
    /// Cut a curve back to the nearest crossing curves: hover to see the piece, click to remove it, or
    /// drag across curves to trim each one where the drag crosses it.
    Trim,
    /// Lengthen a line or arc to the next curve: click near the end (or drag across ends).
    Extend,
    /// Round the corner between two lines: click both lines (or the corner point) with the radius typed
    /// into the [`HudField::Radius`] box; the radius stays for the next fillet.
    Fillet,
    /// Place a dimension.
    Dimension,
    /// Add a geometric constraint.
    Constraint(ConstraintKind),
    /// Place points (centre points unless the Center Point toggle is off).
    Point,
    /// Draw a regular polygon: centre, then a corner or an edge midpoint.
    Polygon,
    /// Draw a straight slot of this kind: two points, then the width.
    Slot(gc_doc::SlotKind),
    /// Draw an arc slot of this kind: three points for the centre arc, then the width.
    ArcSlot(gc_doc::ArcSlotKind),
    /// Offset curves (whole loops by default): pick one, then where the offset goes.
    Offset,
}

/// What a tool wants done.
#[derive(Clone, Debug, PartialEq)]
pub enum ToolOutput {
    /// Apply this edit to the sketch.
    Edit(SketchOp),
    /// Select `item` (or clear the selection); with `extend`, toggle it in the current selection.
    Select {
        /// What was clicked.
        item: Option<Item>,
        /// Toggle instead of replace.
        extend: bool,
    },
    /// Start dragging.
    BeginDrag {
        /// What was grabbed.
        grab: SketchGrab,
        /// Where.
        cursor: [f64; 2],
    },
    /// Continue dragging.
    DragTo([f64; 2]),
    /// Finish the drag and keep the result.
    EndDrag,
    /// Abandon the drag.
    CancelDrag,
    /// Delete the selected items.
    DeleteSelection,
    /// Ask the user for the value of a dimension about to be placed. Answer with [`ToolEvent::Value`].
    RequestValue {
        /// What will be measured.
        kind: DimKind,
        /// Its current value in SI units, as the default.
        measured: f64,
        /// Where the text goes.
        text_pos: [f64; 2],
    },
    /// The tool is done; return to the select tool.
    Finish,
}

/// What the status bar should tell the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(missing_docs, reason = "the names are the documentation")]
pub enum Prompt {
    Select,
    LineStart,
    LineEnd,
    CircleCentre,
    CircleRim,
    /// Arc: click the centre.
    ArcCentre,
    /// Arc: click where it starts.
    ArcStart,
    /// Arc: click where it ends.
    ArcEnd,
    RectangleFirst,
    RectangleSecond,
    DimensionPick,
    DimensionPlace,
    DimensionValue,
    ConstraintPick,
    /// Point: click where it goes.
    PointPlace,
    /// Polygon: click the centre.
    PolygonCentre,
    /// Polygon: click a corner (inscribed) or an edge midpoint (circumscribed).
    PolygonCorner,
    /// Slot: the first point (an arc centre, the start of the centre line or the slot centre).
    SlotFirst,
    /// Slot: the second point.
    SlotSecond,
    /// Slot: the width.
    SlotWidth,
    /// Offset: pick a curve.
    OffsetPick,
    /// Offset: click where the offset goes, or type the distance.
    OffsetPlace,
    ArcSlotCentre,
    ArcSlotThrough,
    ArcSlotEnd,
    /// A sketch dialog's Geometry selector is active.
    PickGeometry,
    /// A sketch dialog's line selector is active.
    PickLine,
    /// A sketch dialog's point (axis) selector is active.
    PickPoint,
}

/// What a tool would draw right now, on top of the sketch.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Preview {
    /// Rubber-band segments.
    pub lines: Vec<([f64; 2], [f64; 2])>,
    /// Rubber-band circles: centre and radius.
    pub circles: Vec<([f64; 2], f64)>,
    /// Rubber-band arcs: centre, radius, start angle and signed sweep (positive is counter-clockwise).
    pub arcs: Vec<([f64; 2], f64, f64, f64)>,
    /// Where the next point would go, and why.
    pub snap: Option<Snap>,
    /// Items picked so far (highlighted).
    pub picks: Vec<Item>,
    /// Dotted alignment lines: from an existing point to where the cursor snapped level or plumb with it.
    pub alignments: Vec<([f64; 2], [f64; 2])>,
    /// What a click would remove (Trim): to be drawn highlighted over the sketch.
    pub cut: Vec<Piece>,
}

/// A tool with its state.
#[derive(Clone, Debug, PartialEq)]
pub struct Tool {
    kind: ToolKind,
    /// First point of the line, circle or rectangle in progress.
    anchor: Option<[f64; 2]>,
    /// Where the cursor snaps to right now.
    cursor: Option<Snap>,
    picks: Vec<Item>,
    /// A dimension waiting for its value: what and where.
    pending: Option<(DimKind, [f64; 2])>,
    dragging: bool,
    /// Arc: the start point, once clicked.
    arc_start: Option<[f64; 2]>,
    /// Arc: the angle swept from the start so far, signed, following the cursor round the centre.
    arc_sweep: f64,
    /// What the first point snapped to.
    anchor_snap: Option<Snap>,
    /// Three-point arc and rectangle: the second point and what it snapped to.
    second: Option<Snap>,
    /// Line tool: the points of the chain drawn so far, start first.
    chain: Vec<[f64; 2]>,
    /// Line tool: dragging an arc out of the chain's end.
    arc_drag: bool,
    /// Tangent arc: the end it starts from and the curve it continues.
    tangent_from: Option<(Id<SkPoint>, Id<Curve>)>,
    /// Draw construction geometry (the Format panel's Construction toggle).
    construction: bool,
    /// Values typed into the input boxes, locked.
    hud: Vec<(HudField, String)>,
    /// Trim, Extend, Fillet: the curve under the cursor and the closest position on it.
    hover: Option<(Id<Curve>, [f64; 2])>,
    /// Shift is held: Trim and Extend swap.
    alternate: bool,
    /// Trim and Extend: dragging across curves, from here.
    sweep_from: Option<[f64; 2]>,
    /// Fillet: the first line and where it was clicked.
    fillet_first: Option<(Id<Curve>, [f64; 2])>,
    /// Fillet: the radius, kept from one fillet to the next.
    fillet_radius: String,
    /// Point: the Center Point toggle.
    center_point: bool,
    /// Polygon: number of sides and circumscribed.
    polygon_sides: u32,
    polygon_circumscribed: bool,
    /// Offset: Loop Select, Constrain Offset, the picked curve and the distance the cursor gives.
    offset_loop: bool,
    offset_constrain: bool,
    offset_pick: Option<Id<Curve>>,
    offset_measured: f64,
    /// Arc slot: the third click.
    slot_third: Option<[f64; 2]>,
}

fn angle_about(centre: [f64; 2], point: [f64; 2]) -> f64 {
    fm::atan2(point[1] - centre[1], point[0] - centre[0])
}

fn xy(sketch: &Sketch, point: Id<SkPoint>) -> Option<[f64; 2]> {
    sketch.pos(point).map(|p| [p[0], p[1]])
}

fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// The dimension the picked items call for, with signs taken from the current geometry.
fn dimension_for(sketch: &Sketch, picks: &[Item]) -> Option<DimKind> {
    let side_of = |p: Id<SkPoint>, line: Id<Curve>| -> Option<f64> {
        let (a, b) = sketch.line(line)?;
        let (a, b, p) = (xy(sketch, a)?, xy(sketch, b)?, xy(sketch, p)?);
        let cross = (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
        Some(if cross >= 0.0 { 1.0 } else { -1.0 })
    };
    match *picks {
        [Item::Curve(curve)] => match sketch.curves.get(&curve)?.geom {
            CurveGeom::Line { a, b } => Some(DimKind::Distance { p: a, q: b }),
            CurveGeom::Circle { .. } => Some(DimKind::Diameter(curve)),
            CurveGeom::Arc { .. } => Some(DimKind::Radius(curve)),
        },
        [Item::Point(p)] if sketch.is_spatial() => {
            // The distance of a point from the XY plane, on the side it is on now.
            let side = if sketch.pos(p)?[2] >= 0.0 { 1.0 } else { -1.0 };
            Some(DimKind::PlaneDistance { p, axis: 2, side })
        }
        [Item::Point(p), Item::Point(q)] if p != q => Some(DimKind::Distance { p, q }),
        [Item::Point(p), Item::Curve(line)] | [Item::Curve(line), Item::Point(p)] => {
            Some(DimKind::PointLine { p, line, side: side_of(p, line)? })
        }
        [Item::Curve(l1), Item::Curve(l2)] if l1 != l2 => {
            let ((a1, b1), (a2, b2)) = (sketch.line(l1)?, sketch.line(l2)?);
            let dir =
                |a, b| Some([xy(sketch, b)?[0] - xy(sketch, a)?[0], xy(sketch, b)?[1] - xy(sketch, a)?[1]]);
            let (u, w) = (dir(a1, b1)?, dir(a2, b2)?);
            let angle = fm::atan2(u[0] * w[1] - u[1] * w[0], u[0] * w[0] + u[1] * w[1]);
            if angle.abs() < 1e-6 || (angle.abs() - std::f64::consts::PI).abs() < 1e-6 {
                // Parallel lines: the distance between them.
                Some(DimKind::PointLine { p: a2, line: l1, side: side_of(a2, l1)? })
            } else {
                // Reverse the second line when needed so that the measured angle is in (0, pi).
                Some(DimKind::Angle { l1, rev1: false, l2, rev2: angle < 0.0 })
            }
        }
        _ => None,
    }
}

/// The constraint of `kind` between the picked items, once enough are picked.
fn constraint_for(sketch: &Sketch, kind: ConstraintKind, picks: &[Item]) -> Option<GeoConstraint> {
    use ConstraintKind as K;
    let line = |c: Id<Curve>| sketch.line(c);
    match (kind, picks) {
        (K::Coincident, [Item::Point(p), Item::Point(q)]) if p != q => {
            Some(GeoConstraint::Coincident(*p, *q))
        }
        (K::Coincident, [Item::Point(p), Item::Curve(c)] | [Item::Curve(c), Item::Point(p)]) => {
            Some(GeoConstraint::PointOnCurve(*p, *c))
        }
        (K::Horizontal, [Item::Curve(c)]) => line(*c).map(|(a, b)| GeoConstraint::Horizontal(a, b)),
        (K::Vertical, [Item::Curve(c)]) => line(*c).map(|(a, b)| GeoConstraint::Vertical(a, b)),
        (K::Horizontal, [Item::Point(p), Item::Point(q)]) if p != q => {
            Some(GeoConstraint::Horizontal(*p, *q))
        }
        (K::Vertical, [Item::Point(p), Item::Point(q)]) if p != q => Some(GeoConstraint::Vertical(*p, *q)),
        (K::Parallel, [Item::Curve(a), Item::Curve(b)]) if a != b => Some(GeoConstraint::Parallel(*a, *b)),
        (K::Perpendicular, [Item::Curve(a), Item::Curve(b)]) if a != b => {
            Some(GeoConstraint::Perpendicular(*a, *b))
        }
        (K::Collinear, [Item::Curve(a), Item::Curve(b)]) if a != b => Some(GeoConstraint::Collinear(*a, *b)),
        (K::Equal, [Item::Curve(a), Item::Curve(b)]) if a != b => Some(GeoConstraint::Equal(*a, *b)),
        (K::Concentric, [Item::Curve(a), Item::Curve(b)]) if a != b => {
            Some(GeoConstraint::Concentric(*a, *b))
        }
        (K::Tangent, [Item::Curve(a), Item::Curve(b)]) if a != b => {
            // Put the line first, and capture on which side the circle lies or how the circles touch.
            let (first, second) = if line(*b).is_some() { (*b, *a) } else { (*a, *b) };
            let (centre, radius) = sketch.round(second)?;
            let centre = xy(sketch, centre)?;
            let (side, external) = match line(first) {
                Some((p, q)) => {
                    let (p, q) = (xy(sketch, p)?, xy(sketch, q)?);
                    let cross = (q[0] - p[0]) * (centre[1] - p[1]) - (q[1] - p[1]) * (centre[0] - p[0]);
                    (if cross >= 0.0 { 1.0 } else { -1.0 }, true)
                }
                None => {
                    let (other_centre, other_radius) = sketch.round(first)?;
                    (1.0, distance(centre, xy(sketch, other_centre)?) >= radius.max(other_radius))
                }
            };
            Some(GeoConstraint::Tangent { a: first, b: second, side, external })
        }
        (K::Midpoint, [Item::Point(p), Item::Curve(c)] | [Item::Curve(c), Item::Point(p)]) => {
            line(*c).map(|_| GeoConstraint::Midpoint(*p, *c))
        }
        (K::Fix, [Item::Point(p)]) => Some(GeoConstraint::Fix(*p)),
        (K::AlongX | K::AlongY | K::AlongZ, [Item::Point(p), Item::Point(q)]) if p != q => {
            Some(GeoConstraint::AlongAxis { p: *p, q: *q, axis: axis_of(kind) })
        }
        (K::AlongX | K::AlongY | K::AlongZ, [Item::Curve(c)]) => {
            line(*c).map(|(p, q)| GeoConstraint::AlongAxis { p, q, axis: axis_of(kind) })
        }
        (K::OnXY | K::OnYZ | K::OnZX, [Item::Point(p)]) => {
            Some(GeoConstraint::OnPlane { p: *p, axis: axis_of(kind) })
        }
        (K::Symmetric, [Item::Point(p), Item::Point(q), Item::Curve(axis)]) if p != q => {
            line(*axis).map(|_| GeoConstraint::Symmetric { p: *p, q: *q, axis: *axis })
        }
        _ => None,
    }
}

/// The axis a 3D constraint kind is about: the direction of a line, or the normal of a plane.
fn axis_of(kind: ConstraintKind) -> usize {
    match kind {
        ConstraintKind::AlongX | ConstraintKind::OnYZ => 0,
        ConstraintKind::AlongY | ConstraintKind::OnZX => 1,
        _ => 2,
    }
}

/// The most items a constraint of this kind needs.
fn picks_needed(kind: ConstraintKind) -> usize {
    match kind {
        ConstraintKind::Fix | ConstraintKind::OnXY | ConstraintKind::OnYZ | ConstraintKind::OnZX => 1,
        ConstraintKind::Symmetric => 3,
        _ => 2,
    }
}

/// A typed value as a plain number in SI units, for shaping the preview: millimetres (or degrees for an
/// angle) unless a unit is given. `None` for an expression the tool cannot read on its own (it still
/// becomes a dimension when the shape is finished).
fn plain_value(text: &str, angle: bool) -> Option<f64> {
    let text = text.trim();
    let (number, unit) = match text.find(|c: char| c.is_alphabetic() || c == '°') {
        Some(at) => (text[..at].trim(), text[at..].trim()),
        None => (text, ""),
    };
    let value: f64 = number.parse().ok()?;
    let scale = match (unit, angle) {
        ("", false) | ("mm", false) => 0.001,
        ("cm", false) => 0.01,
        ("m", false) => 1.0,
        ("in", false) => 0.0254,
        ("", true) | ("deg", true) | ("°", true) => std::f64::consts::PI / 180.0,
        ("rad", true) => 1.0,
        _ => return None,
    };
    let value = value * scale;
    value.is_finite().then_some(value)
}

/// The curve (line or arc) whose two ends are at `p` and `q`, in either order.
fn curve_between(sketch: &Sketch, p: [f64; 2], q: [f64; 2]) -> Option<(Id<Curve>, Id<SkPoint>)> {
    let near = |id: Id<SkPoint>, at: [f64; 2]| xy(sketch, id).is_some_and(|pos| distance(pos, at) < 1e-9);
    sketch.curves.iter().rev().find_map(|(id, curve)| {
        let (a, b) = match curve.geom {
            CurveGeom::Line { a, b } => (a, b),
            CurveGeom::Arc { s, e, .. } => (s, e),
            CurveGeom::Circle { .. } => return None,
        };
        if near(a, p) && near(b, q) {
            Some((*id, b))
        } else if near(b, p) && near(a, q) {
            Some((*id, a))
        } else {
            None
        }
    })
}

fn end_snap(snap: &Snap) -> EndSnap {
    EndSnap {
        point: snap.point,
        curve: snap.curve,
        also_on: snap.also_on,
        midpoint_of: snap.midpoint_of,
        aligned: snap.aligned.map(|(id, level, _)| (id, level)),
        tangent_to: snap.tangent_to,
    }
}

impl Tool {
    /// A tool in its initial state.
    pub fn new(kind: ToolKind) -> Self {
        Self {
            kind,
            anchor: None,
            cursor: None,
            picks: Vec::new(),
            pending: None,
            dragging: false,
            arc_start: None,
            arc_sweep: 0.0,
            anchor_snap: None,
            second: None,
            chain: Vec::new(),
            arc_drag: false,
            tangent_from: None,
            construction: false,
            hud: Vec::new(),
            hover: None,
            alternate: false,
            sweep_from: None,
            fillet_first: None,
            fillet_radius: DEFAULT_FILLET_RADIUS.to_owned(),
            center_point: true,
            polygon_sides: 6,
            polygon_circumscribed: false,
            offset_loop: true,
            offset_constrain: true,
            offset_pick: None,
            offset_measured: 0.0,
            slot_third: None,
        }
    }

    /// Which tool this is.
    pub fn kind(&self) -> ToolKind {
        self.kind
    }

    /// True when new geometry is drawn as construction geometry.
    pub fn construction(&self) -> bool {
        self.construction
    }

    /// Draws new geometry as construction geometry (or not) from now on.
    pub fn set_construction(&mut self, construction: bool) {
        self.construction = construction;
    }

    /// What the status bar should say.
    pub fn prompt(&self) -> Prompt {
        if shapes::is_shape_tool(self.kind) {
            return self.shape_prompt();
        }
        match (self.kind, self.anchor.is_some()) {
            (ToolKind::Select, _) => Prompt::Select,
            (ToolKind::Line, false) => Prompt::LineStart,
            (ToolKind::Line, true) => Prompt::LineEnd,
            (ToolKind::Circle, false) => Prompt::CircleCentre,
            (ToolKind::Circle, true) => Prompt::CircleRim,
            (ToolKind::Arc, false) => Prompt::ArcCentre,
            (ToolKind::Arc, true) if self.arc_start.is_none() => Prompt::ArcStart,
            (ToolKind::Arc, true) => Prompt::ArcEnd,
            (ToolKind::Arc3, false) => Prompt::ArcStart,
            (ToolKind::Arc3, true) if self.second.is_none() => Prompt::ArcEnd,
            (ToolKind::Arc3, true) => Prompt::ArcCentre,
            (ToolKind::ArcTangent, _) if self.tangent_from.is_none() => Prompt::ArcStart,
            (ToolKind::ArcTangent, _) => Prompt::ArcEnd,
            (ToolKind::Rectangle | ToolKind::Rectangle3, false) => Prompt::RectangleFirst,
            (ToolKind::Rectangle | ToolKind::Rectangle3, true) => Prompt::RectangleSecond,
            (ToolKind::Dimension, _) if self.pending.is_some() => Prompt::DimensionValue,
            (ToolKind::Dimension, _) if self.picks.is_empty() => Prompt::DimensionPick,
            (ToolKind::Dimension, _) => Prompt::DimensionPlace,
            (ToolKind::Constraint(_) | ToolKind::Trim | ToolKind::Extend | ToolKind::Fillet, _) => {
                Prompt::ConstraintPick
            }
            // Answered by `shape_prompt` above.
            (
                ToolKind::Point
                | ToolKind::Polygon
                | ToolKind::Slot(_)
                | ToolKind::ArcSlot(_)
                | ToolKind::Offset,
                _,
            ) => self.shape_prompt(),
        }
    }

    /// The input boxes the display should show now (none when the tool takes no typed values at this
    /// step).
    pub fn hud(&self) -> Vec<HudSlot> {
        if shapes::is_shape_tool(self.kind) {
            return self.shape_hud();
        }
        if self.kind == ToolKind::Fillet {
            let locked = self.locked(HudField::Radius).map(str::to_owned);
            let text = locked.as_deref().unwrap_or(&self.fillet_radius);
            let measured = plain_value(text, false).unwrap_or(0.0);
            return vec![HudSlot { field: HudField::Radius, measured, locked }];
        }
        let Some(anchor) = self.anchor.or(self.tangent_from.and(self.cursor.map(|c| c.pos))) else {
            return Vec::new();
        };
        let cursor = self.cursor.map_or(anchor, |c| c.pos);
        let slot =
            |field, measured| HudSlot { field, measured, locked: self.locked(field).map(str::to_owned) };
        match self.kind {
            ToolKind::Line if !self.arc_drag => {
                vec![
                    slot(HudField::Length, distance(anchor, cursor)),
                    slot(HudField::Angle, angle_about(anchor, cursor)),
                ]
            }
            ToolKind::Circle => vec![slot(HudField::Diameter, 2.0 * distance(anchor, cursor))],
            ToolKind::Arc if self.arc_start.is_none() => {
                vec![slot(HudField::Radius, distance(anchor, cursor))]
            }
            ToolKind::Rectangle => vec![
                slot(HudField::Width, (cursor[0] - anchor[0]).abs()),
                slot(HudField::Height, (cursor[1] - anchor[1]).abs()),
            ],
            ToolKind::Rectangle3 => match self.second {
                None => vec![
                    slot(HudField::Length, distance(anchor, cursor)),
                    slot(HudField::Angle, angle_about(anchor, cursor)),
                ],
                Some(second) => {
                    let side = [second.pos[0] - anchor[0], second.pos[1] - anchor[1]];
                    let length = fm::hypot(side[0], side[1]).max(f64::MIN_POSITIVE);
                    let width = ((cursor[0] - second.pos[0]) * -side[1]
                        + (cursor[1] - second.pos[1]) * side[0])
                        / length;
                    vec![slot(HudField::Width, width.abs())]
                }
            },
            _ => Vec::new(),
        }
    }

    fn locked(&self, field: HudField) -> Option<&str> {
        self.hud.iter().find(|(f, _)| *f == field).map(|(_, v)| v.as_str())
    }

    /// The locked value of a box as a number, when the tool can read it.
    fn locked_value(&self, field: HudField) -> Option<f64> {
        self.locked(field).and_then(|text| plain_value(text, field == HudField::Angle))
    }

    /// The cursor moved to where the locked values put it.
    fn apply_locks(&self, at: Snap) -> Snap {
        let Some(anchor) = self.anchor else { return at };
        let mut pos = at.pos;
        match self.kind {
            ToolKind::Line | ToolKind::Rectangle3 if self.second.is_none() => {
                let length = self.locked_value(HudField::Length).unwrap_or_else(|| distance(anchor, pos));
                let angle = self.locked_value(HudField::Angle).unwrap_or_else(|| angle_about(anchor, pos));
                if self.locked(HudField::Length).is_some() || self.locked(HudField::Angle).is_some() {
                    pos = [anchor[0] + length * fm::cos(angle), anchor[1] + length * fm::sin(angle)];
                }
            }
            ToolKind::Circle | ToolKind::Arc if self.arc_start.is_none() => {
                let radius = match self.kind {
                    ToolKind::Circle => self.locked_value(HudField::Diameter).map(|d| d / 2.0),
                    _ => self.locked_value(HudField::Radius),
                };
                if let Some(radius) = radius {
                    let angle = angle_about(anchor, pos);
                    pos = [anchor[0] + radius * fm::cos(angle), anchor[1] + radius * fm::sin(angle)];
                }
            }
            ToolKind::Rectangle => {
                let sign = |v: f64| if v < 0.0 { -1.0 } else { 1.0 };
                if let Some(w) = self.locked_value(HudField::Width) {
                    pos[0] = anchor[0] + sign(pos[0] - anchor[0]) * w;
                }
                if let Some(h) = self.locked_value(HudField::Height) {
                    pos[1] = anchor[1] + sign(pos[1] - anchor[1]) * h;
                }
            }
            ToolKind::Rectangle3 => {
                if let (Some(second), Some(width)) = (self.second, self.locked_value(HudField::Width)) {
                    let side = [second.pos[0] - anchor[0], second.pos[1] - anchor[1]];
                    let length = fm::hypot(side[0], side[1]).max(f64::MIN_POSITIVE);
                    let n = [-side[1] / length, side[0] / length];
                    let along = (pos[0] - second.pos[0]) * n[0] + (pos[1] - second.pos[1]) * n[1];
                    let sign = if along < 0.0 { -1.0 } else { 1.0 };
                    pos = [second.pos[0] + n[0] * width * sign, second.pos[1] + n[1] * width * sign];
                }
            }
            _ => return at,
        }
        if pos.iter().zip(&at.pos).all(|(a, b)| a.to_bits() == b.to_bits()) {
            at
        } else {
            Snap { pos, ..snap_free(pos) }
        }
    }

    /// What to draw on top of the sketch.
    pub fn preview(&self, sketch: &Sketch) -> Preview {
        let mut preview = Preview { snap: self.cursor, picks: self.picks.clone(), ..Preview::default() };
        if shapes::is_shape_tool(self.kind) {
            if let Some(curve) = self.offset_pick {
                preview.picks.push(Item::Curve(curve));
            }
            self.shape_preview(sketch, &mut preview);
            return preview;
        }
        if let Some((_, _, from)) = self.cursor.and_then(|c| c.aligned) {
            preview.alignments.push((from, self.cursor.map_or(from, |c| c.pos)));
        }
        let draw = |preview: &mut Preview, piece: Piece| match piece {
            Piece::Segment(a, b) => preview.lines.push((a, b)),
            Piece::Arc { centre, radius, start, sweep } => preview.arcs.push((centre, radius, start, sweep)),
            Piece::Circle { centre, radius } => preview.circles.push((centre, radius)),
        };
        if let Some((curve, at)) = self.hover {
            match (self.cutting(), self.fillet_first) {
                (Some(ToolKind::Trim), _) => preview.cut.extend(sketch.trim_preview(curve, at)),
                (Some(_), _) => {
                    if let Some(piece) = sketch.extend_preview(curve, at) {
                        draw(&mut preview, piece);
                    }
                }
                (None, Some(first)) if self.kind == ToolKind::Fillet => {
                    let radius = plain_value(&self.radius_text(), false);
                    if let Some(piece) = radius.and_then(|r| sketch.fillet_preview([first, (curve, at)], r)) {
                        draw(&mut preview, piece);
                    }
                }
                _ => {}
            }
            return preview;
        }
        let cursor = self.cursor.map(|s| s.pos);
        if let (Some((from, curve)), Some(cursor)) =
            (self.tangent_from.or(self.chain_end(sketch).filter(|_| self.arc_drag)), cursor)
            && (self.kind == ToolKind::ArcTangent || self.arc_drag)
        {
            if let Some(arc) = tangent_arc(sketch, from, curve, cursor) {
                preview.arcs.push((arc.centre, arc.radius, arc.start_angle, arc.sweep));
            }
            return preview;
        }
        if let (Some(anchor), Some(cursor)) = (self.anchor, cursor) {
            match self.kind {
                ToolKind::Line => preview.lines.push((anchor, cursor)),
                ToolKind::Circle => preview.circles.push((anchor, distance(anchor, cursor))),
                ToolKind::Arc => match self.arc_start {
                    // Before the start is chosen: the radius, and the circle the arc will lie on.
                    None => {
                        preview.lines.push((anchor, cursor));
                        preview.circles.push((anchor, distance(anchor, cursor)));
                    }
                    Some(start) => {
                        preview.lines.push((anchor, start));
                        preview.lines.push((anchor, cursor));
                        preview.arcs.push((
                            anchor,
                            distance(anchor, start),
                            angle_about(anchor, start),
                            self.arc_sweep,
                        ));
                    }
                },
                ToolKind::Arc3 => match self.second {
                    None => preview.lines.push((anchor, cursor)),
                    Some(second) => {
                        if let Some((centre, radius, start, sweep)) = arc_through(anchor, second.pos, cursor)
                        {
                            preview.arcs.push((centre, radius, start, sweep));
                        } else {
                            preview.lines.push((anchor, second.pos));
                        }
                    }
                },
                ToolKind::Rectangle => {
                    let corners = [anchor, [cursor[0], anchor[1]], cursor, [anchor[0], cursor[1]]];
                    preview.lines.extend((0..4).map(|i| (corners[i], corners[(i + 1) % 4])));
                }
                ToolKind::Rectangle3 => match self.second {
                    None => preview.lines.push((anchor, cursor)),
                    Some(second) => {
                        let side = [second.pos[0] - anchor[0], second.pos[1] - anchor[1]];
                        let length = fm::hypot(side[0], side[1]).max(f64::MIN_POSITIVE);
                        let n = [-side[1] / length, side[0] / length];
                        let width = (cursor[0] - second.pos[0]) * n[0] + (cursor[1] - second.pos[1]) * n[1];
                        let corners = [
                            anchor,
                            second.pos,
                            [second.pos[0] + n[0] * width, second.pos[1] + n[1] * width],
                            [anchor[0] + n[0] * width, anchor[1] + n[1] * width],
                        ];
                        preview.lines.extend((0..4).map(|i| (corners[i], corners[(i + 1) % 4])));
                    }
                },
                _ => {}
            }
        }
        preview
    }

    /// Line tool: the last curve of the chain and its point at the chain's end.
    fn chain_end(&self, sketch: &Sketch) -> Option<(Id<SkPoint>, Id<Curve>)> {
        let [.., before, end] = self.chain.as_slice() else { return None };
        curve_between(sketch, *before, *end).map(|(curve, point)| (point, curve))
    }

    fn reset(&mut self) {
        self.anchor = None;
        self.anchor_snap = None;
        self.second = None;
        self.arc_start = None;
        self.arc_sweep = 0.0;
        self.picks.clear();
        self.pending = None;
        self.chain.clear();
        self.arc_drag = false;
        self.tangent_from = None;
        self.fillet_first = None;
        self.sweep_from = None;
        self.offset_pick = None;
        self.slot_third = None;
        if self.kind != ToolKind::Fillet {
            self.hud.clear(); // the fillet radius stays
        }
    }

    /// Trim or Extend, after Shift.
    fn cutting(&self) -> Option<ToolKind> {
        match (self.kind, self.alternate) {
            (ToolKind::Trim, false) | (ToolKind::Extend, true) => Some(ToolKind::Trim),
            (ToolKind::Extend, false) | (ToolKind::Trim, true) => Some(ToolKind::Extend),
            _ => None,
        }
    }

    /// The fillet radius to use: the typed one, else the last one.
    fn radius_text(&self) -> String {
        self.locked(HudField::Radius).map_or_else(|| self.fillet_radius.clone(), str::to_owned)
    }

    /// The edit that trims or extends `curve` at `at`.
    fn cut_op(&self, curve: Id<Curve>, at: [f64; 2]) -> Option<SketchOp> {
        match self.cutting()? {
            ToolKind::Trim => Some(SketchOp::Trim { curve, at }),
            _ => Some(SketchOp::Extend { curve, at }),
        }
    }

    /// Trim or extend every curve the drag crosses from `from` to `to`.
    fn sweep(&self, sketch: &Sketch, from: [f64; 2], to: [f64; 2]) -> Vec<ToolOutput> {
        let path = Shape::Segment(from, to);
        sketch
            .curves
            .iter()
            .filter_map(|(id, curve)| {
                let at = *crossings(&path, &curve_shape(sketch, curve)?).first()?;
                // Extend works on the end nearer the crossing, so the crossing must be on the curve.
                self.cut_op(*id, at).map(ToolOutput::Edit)
            })
            .collect()
    }

    /// Fillet: the two lines that meet at `point` (end points there), each with its middle.
    fn corner_lines(sketch: &Sketch, point: Id<SkPoint>) -> Option<[(Id<Curve>, [f64; 2]); 2]> {
        let at = xy(sketch, point)?;
        let lines: Vec<(Id<Curve>, [f64; 2])> = sketch
            .curves
            .keys()
            .filter_map(|id| {
                let (a, b) = sketch.line(*id)?;
                let (pa, pb) = (xy(sketch, a)?, xy(sketch, b)?);
                (distance(pa, at) < 1e-9 || distance(pb, at) < 1e-9)
                    .then(|| (*id, [(pa[0] + pb[0]) / 2.0, (pa[1] + pb[1]) / 2.0]))
            })
            .collect();
        match lines.as_slice() {
            [first, second] => Some([*first, *second]),
            _ => None,
        }
    }

    /// An edit as drawn: wrapped as construction geometry when the toggle is on, and with the locked
    /// input values attached as dimensions.
    fn emit(&mut self, op: SketchOp, slots: &[(HudField, DimSlot)]) -> ToolOutput {
        let dims: Vec<(DimSlot, String)> = slots
            .iter()
            .filter_map(|(field, slot)| self.locked(*field).map(|text| (*slot, text.to_owned())))
            .collect();
        self.hud.clear();
        let op = if dims.is_empty() { op } else { SketchOp::WithDimensions { op: Box::new(op), dims } };
        ToolOutput::Edit(if self.construction { SketchOp::AsConstruction(Box::new(op)) } else { op })
    }

    /// Feeds one event to the tool. `sketch` is the sketch as it is now (after earlier outputs were
    /// applied).
    pub fn handle(&mut self, sketch: &Sketch, event: ToolEvent) -> Vec<ToolOutput> {
        if shapes::is_shape_tool(self.kind)
            && let Some(out) = self.shape_event(sketch, &event)
        {
            return out;
        }
        let moved = matches!(event, ToolEvent::Move { .. });
        let out = self.handle_event(sketch, event);
        if moved && shapes::is_shape_tool(self.kind) {
            self.shape_moved(sketch);
        }
        out
    }

    fn handle_event(&mut self, sketch: &Sketch, event: ToolEvent) -> Vec<ToolOutput> {
        match event {
            ToolEvent::Move { pos, px, suppress_snap } => {
                let drawing = matches!(
                    self.kind,
                    ToolKind::Line
                        | ToolKind::Circle
                        | ToolKind::Arc
                        | ToolKind::Arc3
                        | ToolKind::ArcTangent
                        | ToolKind::Rectangle
                        | ToolKind::Rectangle3
                        | ToolKind::Point
                        | ToolKind::Polygon
                        | ToolKind::Slot(_)
                        | ToolKind::ArcSlot(_)
                        | ToolKind::Offset
                );
                self.hover = matches!(self.kind, ToolKind::Trim | ToolKind::Extend | ToolKind::Fillet)
                    .then(|| nearest_curve(sketch, pos, px, &[]))
                    .flatten();
                let previous = if self.kind == ToolKind::Line { self.anchor } else { None };
                let before = self.cursor.map(|s| s.pos);
                self.cursor =
                    drawing.then(|| self.apply_locks(snap(sketch, pos, px, previous, suppress_snap)));
                // The arc follows the cursor round its centre, so that it can be drawn either way and
                // past half a turn.
                if let (ToolKind::Arc, Some(centre), Some(start), Some(now)) =
                    (self.kind, self.anchor, self.arc_start, self.cursor)
                {
                    let from = angle_about(centre, before.unwrap_or(start));
                    let mut step = angle_about(centre, now.pos) - from;
                    let tau = std::f64::consts::TAU;
                    step -= tau * (step / tau).round();
                    self.arc_sweep = (self.arc_sweep + step).clamp(-tau, tau);
                }
                Vec::new()
            }
            ToolEvent::Click { pos, px, modifier } => self.click(sketch, pos, px, modifier, None),
            ToolEvent::ClickItem { item, pos, modifier } => {
                self.click(sketch, pos, 0.0, modifier, Some(item))
            }
            ToolEvent::DoubleClick { pos, px, modifier } => {
                if self.kind != ToolKind::Line {
                    return self.click(sketch, pos, px, modifier, None);
                }
                // The second click of a double click may already have drawn the last segment.
                let mut out = Vec::new();
                if self.anchor.is_some_and(|a| distance(a, snap(sketch, pos, px, None, modifier).pos) > px) {
                    out = self.click(sketch, pos, px, modifier, None);
                }
                self.reset();
                out
            }
            ToolEvent::DragStart { pos, px } if self.kind == ToolKind::Select => {
                let grab = match pick(sketch, pos, px) {
                    Some(Item::Point(p)) => SketchGrab::Point(p),
                    Some(Item::Curve(c)) => SketchGrab::Curve(c),
                    _ => return Vec::new(),
                };
                self.dragging = true;
                vec![ToolOutput::BeginDrag { grab, cursor: pos }]
            }
            // Line tool: dragging out of the chain's end draws a tangent arc.
            ToolEvent::DragStart { pos, px } if self.kind == ToolKind::Line => {
                if self.anchor.is_some_and(|a| distance(a, pos) <= crate::tolerance::POINT_PX * px)
                    && self.chain_end(sketch).is_some()
                {
                    self.arc_drag = true;
                    self.cursor = Some(snap(sketch, pos, px, None, true));
                }
                Vec::new()
            }
            ToolEvent::Drag { pos } if self.arc_drag => {
                self.cursor = Some(snap_free(pos));
                Vec::new()
            }
            ToolEvent::DragEnd if self.arc_drag => {
                self.arc_drag = false;
                let (Some((from, curve)), Some(end)) = (self.chain_end(sketch), self.cursor) else {
                    return Vec::new();
                };
                if tangent_arc(sketch, from, curve, end.pos).is_none() {
                    return Vec::new();
                }
                self.chain.push(end.pos);
                self.anchor = Some(end.pos);
                vec![self.emit(
                    SketchOp::AddTangentArc {
                        from,
                        curve,
                        end: [end.pos[0], end.pos[1], 0.0],
                        snap_end: end_snap(&end),
                    },
                    &[],
                )]
            }
            ToolEvent::DragStart { pos, .. } if self.cutting().is_some() => {
                self.sweep_from = Some(pos);
                Vec::new()
            }
            ToolEvent::Drag { pos } if self.sweep_from.is_some() => {
                let from = self.sweep_from.replace(pos).unwrap_or(pos);
                self.hover = None;
                self.sweep(sketch, from, pos)
            }
            ToolEvent::DragEnd if self.sweep_from.is_some() => {
                self.sweep_from = None;
                Vec::new()
            }
            ToolEvent::Drag { pos } if self.dragging => vec![ToolOutput::DragTo(pos)],
            ToolEvent::DragEnd if self.dragging => {
                self.dragging = false;
                vec![ToolOutput::EndDrag]
            }
            ToolEvent::DragStart { .. } | ToolEvent::Drag { .. } | ToolEvent::DragEnd => Vec::new(),
            ToolEvent::Key(Key::Delete) if self.kind == ToolKind::Select => vec![ToolOutput::DeleteSelection],
            ToolEvent::Key(Key::Delete) => Vec::new(),
            ToolEvent::Key(Key::Backspace) if self.kind == ToolKind::Line => {
                // Take back the chain's last segment (line or arc); with none left, the start point.
                let last = self.chain_end(sketch).map(|(_, curve)| curve);
                self.chain.pop();
                self.anchor = self.chain.last().copied();
                if self.anchor.is_none() {
                    self.reset();
                }
                last.map(|curve| ToolOutput::Edit(SketchOp::Delete(vec![Item::Curve(curve)])))
                    .into_iter()
                    .collect()
            }
            ToolEvent::Key(Key::Backspace) => Vec::new(),
            ToolEvent::Key(Key::Enter) => {
                // Finish the shape at the locked values, as a click there would.
                let Some(cursor) = self.cursor else { return Vec::new() };
                let pos = cursor.pos;
                self.click(sketch, pos, 0.0, true, None)
            }
            ToolEvent::Command(ToolCommand::Close) if self.kind == ToolKind::Line => {
                let (Some(&start), Some(end)) = (self.chain.first(), self.anchor) else { return Vec::new() };
                if self.chain.len() < 3 {
                    return Vec::new(); // closing needs at least two segments
                }
                let snap_a = curve_between(sketch, self.chain[self.chain.len() - 2], end)
                    .map(|(_, point)| EndSnap { point: Some(point), ..EndSnap::default() })
                    .unwrap_or_default();
                let snap_b = curve_between(sketch, start, self.chain[1])
                    .and_then(|(curve, other)| {
                        let (a, b) = match sketch.curves.get(&curve)?.geom {
                            CurveGeom::Line { a, b } => (a, b),
                            CurveGeom::Arc { s, e, .. } => (s, e),
                            CurveGeom::Circle { .. } => return None,
                        };
                        Some(if other == a { b } else { a })
                    })
                    .map(|point| EndSnap { point: Some(point), ..EndSnap::default() })
                    .unwrap_or_default();
                self.reset();
                vec![self.emit(
                    SketchOp::AddLine {
                        a: [end[0], end[1], 0.0],
                        b: [start[0], start[1], 0.0],
                        snap_a,
                        snap_b,
                        horizontal: None,
                    },
                    &[],
                )]
            }
            ToolEvent::Command(ToolCommand::Close) => Vec::new(),
            ToolEvent::Command(ToolCommand::Alternate(on)) => {
                self.alternate = on;
                Vec::new()
            }
            ToolEvent::Command(ToolCommand::Done) => {
                if self.dragging {
                    self.dragging = false;
                    return vec![ToolOutput::CancelDrag];
                }
                self.reset();
                Vec::new()
            }
            ToolEvent::Hud { field, value } => {
                self.hud.retain(|(f, _)| *f != field);
                if let Some(value) = value.filter(|v| !v.trim().is_empty()) {
                    self.hud.push((field, value));
                }
                if let Some(cursor) = self.cursor {
                    self.cursor = Some(self.apply_locks(cursor));
                }
                Vec::new()
            }
            ToolEvent::Key(Key::Escape) => {
                if self.dragging {
                    self.dragging = false;
                    vec![ToolOutput::CancelDrag]
                } else if self.anchor.is_some()
                    || !self.picks.is_empty()
                    || self.pending.is_some()
                    || self.tangent_from.is_some()
                    || self.fillet_first.is_some()
                    || self.offset_pick.is_some()
                {
                    self.reset();
                    Vec::new()
                } else if self.kind == ToolKind::Select {
                    vec![ToolOutput::Select { item: None, extend: false }]
                } else {
                    vec![ToolOutput::Finish]
                }
            }
            ToolEvent::Value(source) => match self.pending.take() {
                Some((kind, text_pos)) => {
                    self.reset();
                    vec![ToolOutput::Edit(SketchOp::AddDimension { kind, source, text_pos })]
                }
                None => Vec::new(),
            },
            ToolEvent::CancelValue => {
                self.reset();
                Vec::new()
            }
        }
    }

    /// Where the click puts the next point: snapped, then moved to the locked values.
    fn place(
        &self,
        sketch: &Sketch,
        pos: [f64; 2],
        px: f64,
        previous: Option<[f64; 2]>,
        modifier: bool,
    ) -> Snap {
        let typed = !self.hud.is_empty() && px == 0.0;
        let snapped = if typed {
            self.cursor.unwrap_or_else(|| snap_free(pos))
        } else {
            snap(sketch, pos, px, previous, modifier)
        };
        self.apply_locks(snapped)
    }

    /// `picked` is what the caller found under the cursor; without it the tool picks in the sketch plane.
    fn click(
        &mut self,
        sketch: &Sketch,
        pos: [f64; 2],
        px: f64,
        modifier: bool,
        picked: Option<Option<Item>>,
    ) -> Vec<ToolOutput> {
        let pick = |sketch: &Sketch, pos: [f64; 2], px: f64| picked.unwrap_or_else(|| pick(sketch, pos, px));
        match self.kind {
            // Their clicks are handled by `shape_event` before this.
            ToolKind::Point
            | ToolKind::Polygon
            | ToolKind::Slot(_)
            | ToolKind::ArcSlot(_)
            | ToolKind::Offset => Vec::new(),
            ToolKind::Select => vec![ToolOutput::Select { item: pick(sketch, pos, px), extend: modifier }],
            ToolKind::Line => {
                let here = self.place(sketch, pos, px, self.anchor, modifier);
                let Some(start) = self.anchor else {
                    self.anchor = Some(here.pos);
                    self.anchor_snap = Some(here);
                    self.chain = vec![here.pos];
                    return Vec::new();
                };
                if distance(start, here.pos) <= px {
                    return Vec::new(); // a click on the start point is not a line
                }
                // The start is snapped again now: the previous segment's end point exists by now, so a
                // chain becomes connected lines.
                let at_start = match self.anchor_snap.filter(|_| self.chain.len() == 1) {
                    Some(first) => first,
                    None => snap(sketch, start, px.max(1e-9), None, modifier),
                };
                self.anchor = Some(here.pos);
                self.anchor_snap = None;
                self.chain.push(here.pos);
                vec![self.emit(
                    SketchOp::AddLine {
                        a: [start[0], start[1], 0.0],
                        b: [here.pos[0], here.pos[1], 0.0],
                        snap_a: end_snap(&at_start),
                        snap_b: end_snap(&here),
                        horizontal: here.horizontal,
                    },
                    &[(HudField::Length, DimSlot::Length)],
                )]
            }
            ToolKind::Circle => {
                let here = self.place(sketch, pos, px, None, modifier);
                let Some(centre) = self.anchor.take() else {
                    self.anchor = Some(here.pos);
                    self.anchor_snap = Some(here);
                    return Vec::new();
                };
                let radius = distance(centre, here.pos);
                if radius <= px {
                    self.anchor = Some(centre);
                    return Vec::new();
                }
                let centre_snap = self.anchor_snap.take().map(|s| end_snap(&s)).unwrap_or_default();
                let out = self.emit(
                    SketchOp::AddCircle {
                        centre: [centre[0], centre[1], 0.0],
                        radius,
                        centre_snap,
                        rim_snap: end_snap(&here),
                    },
                    &[(HudField::Diameter, DimSlot::Diameter)],
                );
                self.reset();
                vec![out]
            }
            ToolKind::Arc => {
                let here = self.place(sketch, pos, px, None, modifier).pos;
                let Some(centre) = self.anchor else {
                    self.anchor = Some(here);
                    return Vec::new();
                };
                let Some(start) = self.arc_start else {
                    if distance(centre, here) > px {
                        self.arc_start = Some(here);
                        self.arc_sweep = 0.0;
                        // The sweep is measured from the start point, not from wherever the cursor was.
                        self.cursor = Some(snap_free(here));
                    }
                    return Vec::new();
                };
                let radius = distance(centre, start);
                if (self.arc_sweep * radius).abs() <= px {
                    return Vec::new(); // no arc yet
                }
                // The end lies on the circle through the start, in the direction of the click.
                let angle = angle_about(centre, start) + self.arc_sweep;
                let end = [centre[0] + radius * fm::cos(angle), centre[1] + radius * fm::sin(angle)];
                // Stored arcs run counter-clockwise from start to end.
                let (first, last) = if self.arc_sweep > 0.0 { (start, end) } else { (end, start) };
                let out = self.emit(
                    SketchOp::AddArc {
                        centre: [centre[0], centre[1], 0.0],
                        start: [first[0], first[1], 0.0],
                        end: [last[0], last[1], 0.0],
                    },
                    &[(HudField::Radius, DimSlot::Radius)],
                );
                self.reset();
                vec![out]
            }
            ToolKind::Arc3 => {
                let here = self.place(sketch, pos, px, None, modifier);
                let Some(first) = self.anchor else {
                    self.anchor = Some(here.pos);
                    self.anchor_snap = Some(here);
                    return Vec::new();
                };
                let Some(second) = self.second else {
                    if distance(first, here.pos) > px {
                        self.second = Some(here);
                    }
                    return Vec::new();
                };
                if arc_through(first, second.pos, here.pos).is_none() {
                    return Vec::new(); // on the chord's line: no arc
                }
                let snap_a = self.anchor_snap.map(|s| end_snap(&s)).unwrap_or_default();
                let out = self.emit(
                    SketchOp::AddArcThrough {
                        a: [first[0], first[1], 0.0],
                        b: [second.pos[0], second.pos[1], 0.0],
                        through: [here.pos[0], here.pos[1], 0.0],
                        snap_a,
                        snap_b: end_snap(&second),
                    },
                    &[],
                );
                self.reset();
                vec![out]
            }
            ToolKind::ArcTangent => {
                let Some((from, curve)) = self.tangent_from else {
                    // The first click picks the end of a line or arc to continue.
                    let Some(Item::Point(point)) = pick(sketch, pos, px) else { return Vec::new() };
                    let owner = sketch.curves.iter().find_map(|(id, c)| match c.geom {
                        CurveGeom::Line { a, b } | CurveGeom::Arc { s: a, e: b, .. }
                            if a == point || b == point =>
                        {
                            Some(*id)
                        }
                        _ => None,
                    });
                    if let Some(curve) = owner {
                        self.tangent_from = Some((point, curve));
                    }
                    return Vec::new();
                };
                let here = self.place(sketch, pos, px, None, modifier);
                if tangent_arc(sketch, from, curve, here.pos).is_none() {
                    return Vec::new();
                }
                let out = self.emit(
                    SketchOp::AddTangentArc {
                        from,
                        curve,
                        end: [here.pos[0], here.pos[1], 0.0],
                        snap_end: end_snap(&here),
                    },
                    &[],
                );
                self.reset();
                vec![out]
            }
            ToolKind::Rectangle => {
                let here = self.place(sketch, pos, px, None, modifier);
                let Some(corner) = self.anchor else {
                    self.anchor = Some(here.pos);
                    self.anchor_snap = Some(here);
                    return Vec::new();
                };
                if (here.pos[0] - corner[0]).abs() <= px || (here.pos[1] - corner[1]).abs() <= px {
                    return Vec::new();
                }
                let snap_a = self.anchor_snap.map(|s| end_snap(&s)).unwrap_or_default();
                let out = self.emit(
                    SketchOp::AddRectangle { a: corner, b: here.pos, snap_a },
                    &[(HudField::Width, DimSlot::Width), (HudField::Height, DimSlot::Height)],
                );
                self.reset();
                vec![out]
            }
            ToolKind::Rectangle3 => {
                let here = self.place(sketch, pos, px, None, modifier);
                let Some(first) = self.anchor else {
                    self.anchor = Some(here.pos);
                    self.anchor_snap = Some(here);
                    return Vec::new();
                };
                let Some(second) = self.second else {
                    if distance(first, here.pos) > px {
                        self.second = Some(here);
                    }
                    return Vec::new();
                };
                let side = [second.pos[0] - first[0], second.pos[1] - first[1]];
                let length = fm::hypot(side[0], side[1]);
                let width = ((here.pos[0] - second.pos[0]) * -side[1]
                    + (here.pos[1] - second.pos[1]) * side[0])
                    / length;
                if width.abs() <= px {
                    return Vec::new();
                }
                let snap_a = self.anchor_snap.map(|s| end_snap(&s)).unwrap_or_default();
                let out = self.emit(
                    SketchOp::AddRectangle3 {
                        a: [first[0], first[1], 0.0],
                        b: [second.pos[0], second.pos[1], 0.0],
                        c: [here.pos[0], here.pos[1], 0.0],
                        snap_a,
                    },
                    &[(HudField::Length, DimSlot::Width), (HudField::Width, DimSlot::Height)],
                );
                self.reset();
                vec![out]
            }
            ToolKind::Dimension => {
                if self.pending.is_some() {
                    return Vec::new(); // the value box is open
                }
                // A click on geometry extends the pick while that still gives a dimension; any other
                // click places the dimension that the picks so far describe.
                if let Some(item @ (Item::Point(_) | Item::Curve(_))) = pick(sketch, pos, px) {
                    let mut extended = self.picks.clone();
                    extended.push(item);
                    let single_point = matches!(extended.as_slice(), [Item::Point(_)]);
                    if single_point || dimension_for(sketch, &extended).is_some() {
                        self.picks = extended;
                        return Vec::new();
                    }
                }
                let Some(kind) = dimension_for(sketch, &self.picks) else { return Vec::new() };
                let measured = sketch.measure(&kind).unwrap_or(0.0);
                self.pending = Some((kind, pos));
                vec![ToolOutput::RequestValue { kind, measured, text_pos: pos }]
            }
            ToolKind::Trim | ToolKind::Extend => {
                let hit = match picked {
                    Some(Some(Item::Curve(curve))) => Some((curve, pos)),
                    Some(_) => None,
                    None => nearest_curve(sketch, pos, px, &[]),
                };
                let Some((curve, at)) = hit else { return Vec::new() };
                self.hover = None;
                self.cut_op(curve, at).map(ToolOutput::Edit).into_iter().collect()
            }
            ToolKind::Fillet => {
                let radius = self.radius_text();
                if self.fillet_first.is_none()
                    && let Some(Item::Point(point)) = pick(sketch, pos, px)
                    && let Some(lines) = Self::corner_lines(sketch, point)
                {
                    self.fillet_radius.clone_from(&radius);
                    return vec![ToolOutput::Edit(SketchOp::Fillet { lines, radius })];
                }
                let hit = match picked {
                    Some(Some(Item::Curve(curve))) => Some((curve, pos)),
                    Some(_) => None,
                    None => nearest_curve(sketch, pos, px, &[]),
                };
                let Some((curve, at)) = hit.filter(|(c, _)| sketch.line(*c).is_some()) else {
                    return Vec::new();
                };
                match self.fillet_first.take() {
                    Some(first) if first.0 != curve => {
                        self.picks.clear();
                        self.hover = None;
                        self.fillet_radius.clone_from(&radius);
                        vec![ToolOutput::Edit(SketchOp::Fillet { lines: [first, (curve, at)], radius })]
                    }
                    _ => {
                        self.fillet_first = Some((curve, at));
                        self.picks = vec![Item::Curve(curve)];
                        Vec::new()
                    }
                }
            }
            ToolKind::Constraint(kind) => {
                let Some(item @ (Item::Point(_) | Item::Curve(_))) = pick(sketch, pos, px) else {
                    return Vec::new();
                };
                self.picks.push(item);
                if let Some(constraint) = constraint_for(sketch, kind, &self.picks) {
                    self.picks.clear();
                    return vec![ToolOutput::Edit(SketchOp::AddConstraint(constraint))];
                }
                if self.picks.len() >= picks_needed(kind) {
                    self.picks.clear(); // these items do not fit this constraint: start over
                }
                Vec::new()
            }
        }
    }
}

/// The fillet radius until one is typed.
const DEFAULT_FILLET_RADIUS: &str = "2 mm";

/// The cursor position as it is, with nothing snapped.
fn snap_free(pos: [f64; 2]) -> Snap {
    Snap {
        pos,
        kind: crate::pick::SnapKind::Free,
        point: None,
        curve: None,
        horizontal: None,
        inference: None,
        also_on: None,
        midpoint_of: None,
        aligned: None,
        tangent_to: None,
    }
}

/// The arc from `a` to `b` through `through`: centre, radius, start angle (at `a`) and signed sweep.
pub(crate) fn arc_through(a: [f64; 2], b: [f64; 2], through: [f64; 2]) -> Option<([f64; 2], f64, f64, f64)> {
    let d = 2.0 * (a[0] * (b[1] - through[1]) + b[0] * (through[1] - a[1]) + through[0] * (a[1] - b[1]));
    let scale = distance(a, b).max(distance(a, through)).max(1e-12);
    if d.abs() < 1e-9 * scale * scale {
        return None;
    }
    let sq = |p: [f64; 2]| p[0] * p[0] + p[1] * p[1];
    let centre = [
        (sq(a) * (b[1] - through[1]) + sq(b) * (through[1] - a[1]) + sq(through) * (a[1] - b[1])) / d,
        (sq(a) * (through[0] - b[0]) + sq(b) * (a[0] - through[0]) + sq(through) * (b[0] - a[0])) / d,
    ];
    let tau = std::f64::consts::TAU;
    let (ta, tb, tt) = (angle_about(centre, a), angle_about(centre, b), angle_about(centre, through));
    let ccw = (tb - ta).rem_euclid(tau);
    let sweep = if (tt - ta).rem_euclid(tau) < ccw { ccw } else { ccw - tau };
    Some((centre, distance(centre, a), ta, sweep))
}
