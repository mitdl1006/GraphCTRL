//! The tools.

use gc_core::Id;
use gc_doc::{EndSnap, SketchOp};
use gc_math::fm;
use gc_sketch::{Curve, CurveGeom, DimKind, GeoConstraint, Item, SkPoint, Sketch, SketchGrab};

use crate::pick::{Snap, pick, snap};

/// Keys tools react to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// Step back: cancel the current input, or leave the tool.
    Escape,
    /// Delete the selection.
    Delete,
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
    /// Place a dimension.
    Dimension,
    /// Add a geometric constraint.
    Constraint(ConstraintKind),
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
        (K::Symmetric, [Item::Point(p), Item::Point(q), Item::Curve(axis)]) if p != q => {
            line(*axis).map(|_| GeoConstraint::Symmetric { p: *p, q: *q, axis: *axis })
        }
        _ => None,
    }
}

/// The most items a constraint of this kind needs.
fn picks_needed(kind: ConstraintKind) -> usize {
    match kind {
        ConstraintKind::Fix => 1,
        ConstraintKind::Symmetric => 3,
        _ => 2,
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
        }
    }

    /// Which tool this is.
    pub fn kind(&self) -> ToolKind {
        self.kind
    }

    /// What the status bar should say.
    pub fn prompt(&self) -> Prompt {
        match (self.kind, self.anchor.is_some()) {
            (ToolKind::Select, _) => Prompt::Select,
            (ToolKind::Line, false) => Prompt::LineStart,
            (ToolKind::Line, true) => Prompt::LineEnd,
            (ToolKind::Circle, false) => Prompt::CircleCentre,
            (ToolKind::Circle, true) => Prompt::CircleRim,
            (ToolKind::Arc, false) => Prompt::ArcCentre,
            (ToolKind::Arc, true) if self.arc_start.is_none() => Prompt::ArcStart,
            (ToolKind::Arc, true) => Prompt::ArcEnd,
            (ToolKind::Rectangle, false) => Prompt::RectangleFirst,
            (ToolKind::Rectangle, true) => Prompt::RectangleSecond,
            (ToolKind::Dimension, _) if self.pending.is_some() => Prompt::DimensionValue,
            (ToolKind::Dimension, _) if self.picks.is_empty() => Prompt::DimensionPick,
            (ToolKind::Dimension, _) => Prompt::DimensionPlace,
            (ToolKind::Constraint(_), _) => Prompt::ConstraintPick,
        }
    }

    /// What to draw on top of the sketch.
    pub fn preview(&self) -> Preview {
        let mut preview = Preview { snap: self.cursor, picks: self.picks.clone(), ..Preview::default() };
        if let (Some(anchor), Some(cursor)) = (self.anchor, self.cursor.map(|s| s.pos)) {
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
                ToolKind::Rectangle => {
                    let corners = [anchor, [cursor[0], anchor[1]], cursor, [anchor[0], cursor[1]]];
                    preview.lines.extend((0..4).map(|i| (corners[i], corners[(i + 1) % 4])));
                }
                _ => {}
            }
        }
        preview
    }

    fn reset(&mut self) {
        self.anchor = None;
        self.arc_start = None;
        self.arc_sweep = 0.0;
        self.picks.clear();
        self.pending = None;
    }

    /// Feeds one event to the tool. `sketch` is the sketch as it is now (after earlier outputs were
    /// applied).
    pub fn handle(&mut self, sketch: &Sketch, event: ToolEvent) -> Vec<ToolOutput> {
        match event {
            ToolEvent::Move { pos, px, suppress_snap } => {
                let drawing = matches!(
                    self.kind,
                    ToolKind::Line | ToolKind::Circle | ToolKind::Arc | ToolKind::Rectangle
                );
                let previous = if self.kind == ToolKind::Line { self.anchor } else { None };
                let before = self.cursor.map(|s| s.pos);
                self.cursor = drawing.then(|| snap(sketch, pos, px, previous, suppress_snap));
                // The arc follows the cursor round its centre, so that it can be drawn either way and
                // past half a turn.
                if let (Some(centre), Some(start), Some(now)) = (self.anchor, self.arc_start, self.cursor) {
                    let from = angle_about(centre, before.unwrap_or(start));
                    let mut step = angle_about(centre, now.pos) - from;
                    let tau = std::f64::consts::TAU;
                    step -= tau * (step / tau).round();
                    self.arc_sweep = (self.arc_sweep + step).clamp(-tau, tau);
                }
                Vec::new()
            }
            ToolEvent::Click { pos, px, modifier } => self.click(sketch, pos, px, modifier),
            ToolEvent::DragStart { pos, px } if self.kind == ToolKind::Select => {
                let grab = match pick(sketch, pos, px) {
                    Some(Item::Point(p)) => SketchGrab::Point(p),
                    Some(Item::Curve(c)) => SketchGrab::Curve(c),
                    _ => return Vec::new(),
                };
                self.dragging = true;
                vec![ToolOutput::BeginDrag { grab, cursor: pos }]
            }
            ToolEvent::Drag { pos } if self.dragging => vec![ToolOutput::DragTo(pos)],
            ToolEvent::DragEnd if self.dragging => {
                self.dragging = false;
                vec![ToolOutput::EndDrag]
            }
            ToolEvent::DragStart { .. } | ToolEvent::Drag { .. } | ToolEvent::DragEnd => Vec::new(),
            ToolEvent::Key(Key::Delete) if self.kind == ToolKind::Select => vec![ToolOutput::DeleteSelection],
            ToolEvent::Key(Key::Delete) => Vec::new(),
            ToolEvent::Key(Key::Escape) => {
                if self.dragging {
                    self.dragging = false;
                    vec![ToolOutput::CancelDrag]
                } else if self.anchor.is_some() || !self.picks.is_empty() || self.pending.is_some() {
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

    fn click(&mut self, sketch: &Sketch, pos: [f64; 2], px: f64, modifier: bool) -> Vec<ToolOutput> {
        let end_snap = |s: &Snap| EndSnap { point: s.point, curve: s.curve };
        match self.kind {
            ToolKind::Select => vec![ToolOutput::Select { item: pick(sketch, pos, px), extend: modifier }],
            ToolKind::Line => {
                let here = snap(sketch, pos, px, self.anchor, modifier);
                let Some(start) = self.anchor.replace(here.pos) else { return Vec::new() };
                if distance(start, here.pos) <= px {
                    self.anchor = Some(start);
                    return Vec::new(); // a click on the start point is not a line
                }
                // The start is snapped again now: the previous segment's end point exists by now, so a
                // chain becomes connected lines.
                let at_start = snap(sketch, start, px, None, modifier);
                vec![ToolOutput::Edit(SketchOp::AddLine {
                    a: [start[0], start[1], 0.0],
                    b: [here.pos[0], here.pos[1], 0.0],
                    snap_a: end_snap(&at_start),
                    snap_b: end_snap(&here),
                    horizontal: here.horizontal,
                })]
            }
            ToolKind::Circle => {
                let here = snap(sketch, pos, px, None, modifier).pos;
                let Some(centre) = self.anchor.take() else {
                    self.anchor = Some(here);
                    return Vec::new();
                };
                let radius = distance(centre, here);
                if radius <= px {
                    self.anchor = Some(centre);
                    return Vec::new();
                }
                vec![ToolOutput::Edit(SketchOp::AddCircle { centre: [centre[0], centre[1], 0.0], radius })]
            }
            ToolKind::Arc => {
                let here = snap(sketch, pos, px, None, modifier).pos;
                let Some(centre) = self.anchor else {
                    self.anchor = Some(here);
                    return Vec::new();
                };
                let Some(start) = self.arc_start else {
                    if distance(centre, here) > px {
                        self.arc_start = Some(here);
                        self.arc_sweep = 0.0;
                        // The sweep is measured from the start point, not from wherever the cursor was.
                        self.cursor = Some(Snap { pos: here, ..snap(sketch, here, px, None, true) });
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
                self.reset();
                vec![ToolOutput::Edit(SketchOp::AddArc {
                    centre: [centre[0], centre[1], 0.0],
                    start: [first[0], first[1], 0.0],
                    end: [last[0], last[1], 0.0],
                })]
            }
            ToolKind::Rectangle => {
                let here = snap(sketch, pos, px, None, modifier).pos;
                let Some(corner) = self.anchor.take() else {
                    self.anchor = Some(here);
                    return Vec::new();
                };
                if (here[0] - corner[0]).abs() <= px || (here[1] - corner[1]).abs() <= px {
                    self.anchor = Some(corner);
                    return Vec::new();
                }
                let at_corner = snap(sketch, corner, px, None, modifier);
                vec![ToolOutput::Edit(SketchOp::AddRectangle {
                    a: corner,
                    b: here,
                    snap_a: end_snap(&at_corner),
                })]
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
