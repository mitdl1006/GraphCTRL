//! Drawing in a 3D sketch: chained lines and arcs whose points the user interface places with
//! [`gc_tools::spatial::place`]. Arcs: Three Point Arc (two ends, then a point on it) and Center Point Arc
//! (centre, start, end), as on Inventor's 3D Sketch tab (Inventor 2026 help, To Create Lines, Arcs, and
//! Bends, GUID-4F861674-F74C-474E-869A-4C190ED6EC4B).

use gc_core::Id;
use gc_doc::{Created, EndSnap, SketchOp};
use gc_sketch::{GeoConstraint, SkPoint};
use gc_tools::ToolKind;

use crate::Session;

/// State of the 3D line being drawn.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SpatialState {
    /// Where the line in progress starts, and the point there once it exists.
    anchor: Option<([f64; 3], Option<Id<SkPoint>>)>,
    /// Where the cursor would place the next point, and the axis it is locked to.
    hover: Option<([f64; 3], Option<usize>)>,
    /// The normal of the working plane: 2 = XY, 0 = YZ, 1 = ZX.
    plane: usize,
    /// The points clicked so far for an arc, with the existing points they were placed on.
    arc: Vec<([f64; 3], Option<Id<SkPoint>>)>,
    /// The fit points clicked so far for a spline, with the existing points they were placed on.
    spline: Vec<([f64; 3], Option<Id<SkPoint>>)>,
    /// The line the line in progress continues from (for Auto-Bend).
    previous: Option<Id<gc_sketch::Curve>>,
    /// Auto-Bend's radius as typed, when it is on.
    auto_bend: Option<String>,
}

impl Default for SpatialState {
    fn default() -> Self {
        Self {
            anchor: None,
            hover: None,
            plane: 2,
            arc: Vec::new(),
            spline: Vec::new(),
            previous: None,
            auto_bend: None,
        }
    }
}

impl SpatialState {
    pub(crate) fn reset(&mut self) {
        self.anchor = None;
        self.hover = None;
        self.arc.clear();
        self.spline.clear();
        self.previous = None;
    }

    /// Drops the line or arc in progress. Returns false when there was none.
    pub(crate) fn drop_line(&mut self) -> bool {
        self.hover = None;
        let arc = !self.arc.is_empty() || !self.spline.is_empty();
        self.arc.clear();
        self.spline.clear();
        self.previous = None;
        self.anchor.take().is_some() || arc
    }
}

impl Session {
    /// True when the sketch being edited is a 3D sketch.
    pub fn sketch_is_spatial(&self) -> bool {
        self.active_sketch().is_some_and(|(_, sketch)| sketch.is_spatial())
    }

    /// Auto-Bend for 3D lines (Sketch tab of Application Options, GUID-1898C432-CFD2-46F1-8BAE-631392DFBF1D:
    /// "Automatically places tangent corner bends on 3D lines as you sketch them. Default setting is off"):
    /// `Some(radius)` turns it on with that radius as typed, `None` off. The document's 3D Sketch Auto-Bend
    /// Radius is `doc_settings().auto_bend_text()`, the radius to pass.
    pub fn set_auto_bend(&mut self, radius: Option<String>) {
        self.spatial.auto_bend = radius;
    }

    /// Auto-Bend's radius when it is on.
    pub fn auto_bend(&self) -> Option<&str> {
        self.spatial.auto_bend.as_deref()
    }

    /// The Document Settings of the open document.
    pub fn doc_settings(&self) -> gc_doc::DocSettings {
        self.doc().settings()
    }

    /// Replaces the Document Settings (one undo step; saved with the document). Refused when not valid.
    pub fn set_doc_settings(&mut self, settings: gc_doc::DocSettings) -> Result<(), gc_doc::EditError> {
        self.execute(&gc_doc::Command::Cell(gc_doc::CellOp::SetDocSettings(settings))).map(|_| ())
    }

    /// The normal of the working plane of 3D sketching: 2 = XY, 0 = YZ, 1 = ZX.
    pub fn spatial_plane(&self) -> usize {
        self.spatial.plane
    }

    /// Switches the working plane: XY, YZ, ZX and round again.
    pub fn cycle_spatial_plane(&mut self) {
        self.spatial.plane = match self.spatial.plane {
            2 => 0,
            0 => 1,
            _ => 2,
        };
    }

    /// Where the 3D line in progress starts, or the last point of the arc in progress (axis locks run
    /// through it).
    pub fn spatial_anchor(&self) -> Option<[f64; 3]> {
        match self.spatial.arc.last().or(self.spatial.spline.last()) {
            Some((pos, _)) => Some(*pos),
            None => self.spatial.anchor.map(|(pos, _)| pos),
        }
    }

    /// The centre, start and end of the arc the clicks so far and `last` would make.
    fn spatial_arc(&self, last: [f64; 3]) -> Option<([f64; 3], [f64; 3], [f64; 3])> {
        let [(a, _), (b, _)] = <[_; 2]>::try_from(self.spatial.arc.as_slice()).ok()?;
        match self.tool() {
            ToolKind::Arc3 => gc_sketch::Arc3::through(a, last, b),
            _ => Some((a, b, last)),
        }
    }

    /// The arc in progress as points along it (the rubber band), once two points are placed; with one, the
    /// segment to the cursor.
    pub fn spatial_arc_preview(&self) -> Option<Vec<[f64; 3]>> {
        let (cursor, _) = self.spatial.hover?;
        match self.spatial.arc.len() {
            1 => Some(vec![self.spatial.arc[0].0, cursor]),
            2 => {
                let (centre, start, end) = self.spatial_arc(cursor)?;
                let arc = gc_sketch::Arc3::new(centre, start, end)?;
                Some((0..=48).map(|i| arc.at(f64::from(i) / 48.0)).collect())
            }
            _ => None,
        }
    }

    /// Tells the session where the cursor would place the next point (for the rubber band); `None` when
    /// it would place nothing.
    pub fn spatial_hover(&mut self, hover: Option<([f64; 3], Option<usize>)>) {
        self.spatial.hover = hover;
    }

    /// The rubber band of the 3D line in progress: from, to, and the axis the end is locked to.
    pub fn spatial_preview(&self) -> Option<([f64; 3], [f64; 3], Option<usize>)> {
        let (from, _) = self.spatial.anchor?;
        let (to, axis) = self.spatial.hover?;
        Some((from, to, axis))
    }

    /// A click of the line tool in a 3D sketch at `pos` (sketch coordinates). `point` is the existing
    /// point there, `axis` the axis through the previous point the position lies on. The first click
    /// starts a line; every further click ends one and starts the next from there. What the placement
    /// implies becomes constraints where that is consistent, and is left out where it is not.
    pub fn spatial_click(&mut self, pos: [f64; 3], point: Option<Id<SkPoint>>, axis: Option<usize>) {
        if !self.sketch_is_spatial() {
            return;
        }
        if matches!(self.tool(), ToolKind::Arc | ToolKind::Arc3) {
            self.spatial_arc_click(pos, point);
            return;
        }
        if self.tool() == ToolKind::Spline {
            self.spatial_spline_click(pos, point);
            return;
        }
        if self.tool() != ToolKind::Line {
            return;
        }
        let Some((start, start_point)) = self.spatial.anchor else {
            self.spatial.anchor = Some((pos, point));
            return;
        };
        let length = (0..3).map(|k| (pos[k] - start[k]).powi(2)).sum::<f64>().sqrt();
        if length < 1e-9 || (point.is_some() && point == start_point) {
            return; // a click on the start is not a line
        }
        let line = SketchOp::AddLine {
            a: start,
            b: pos,
            snap_a: EndSnap { point: start_point, ..EndSnap::default() },
            snap_b: EndSnap { point, ..EndSnap::default() },
            horizontal: None,
        };
        // With Auto-Bend, the corner with the line before gets a bend.
        let op = match (&self.spatial.auto_bend, self.spatial.previous) {
            (Some(radius), Some(previous)) => {
                SketchOp::AddLineBent { line: Box::new(line), previous, radius: radius.clone() }
            }
            (Some(_), None) | (None, _) => line,
        };
        let Ok(created) = self.sketch_op(op) else { return };
        let new = created.iter().find_map(|c| match c {
            Created::Curve(curve) => self.active_sketch()?.1.line(*curve).map(|_| *curve),
            _ => None,
        });
        let Some(new) = new else { return };
        let Some((from, to)) = self.active_sketch().and_then(|(_, sk)| sk.line(new)) else { return };
        self.spatial.previous = Some(new);
        if let Some(axis) = axis {
            // Implied by how the point was placed: dropped without a word if it does not fit.
            let along = SketchOp::AddConstraint(GeoConstraint::AlongAxis { p: from, q: to, axis });
            if self.sketch_op(along).is_err() {
                self.last_error = None;
            }
        }
        // The next line starts at the end of this one, wherever solving left it.
        let end = self.point_pos(to).unwrap_or(pos);
        self.spatial.anchor = Some((end, Some(to)));
    }

    /// A click of the spline tool in a 3D sketch: a fit point; a second click on the last one finishes
    /// the spline (as in a 2D sketch), and so does [`Session::spatial_finish`] (Enter).
    fn spatial_spline_click(&mut self, pos: [f64; 3], point: Option<Id<SkPoint>>) {
        let again = self.spatial.spline.last().is_some_and(|(at, p)| {
            (point.is_some() && *p == point) || (0..3).map(|k| (at[k] - pos[k]).powi(2)).sum::<f64>() < 1e-18
        });
        if again {
            self.spatial_finish();
        } else {
            self.spatial.spline.push((pos, point));
        }
    }

    /// Finishes the 3D spline in progress (Enter): a spline through the fit points clicked, two or more;
    /// with fewer, nothing.
    pub fn spatial_finish(&mut self) {
        if self.spatial.spline.len() < 2 {
            return;
        }
        let points: Vec<[f64; 3]> = self.spatial.spline.iter().map(|(p, _)| *p).collect();
        let snaps: Vec<EndSnap> = self
            .spatial
            .spline
            .iter()
            .map(|(_, point)| EndSnap { point: *point, ..EndSnap::default() })
            .collect();
        if self.sketch_op(SketchOp::AddSpline { points, snaps }).is_ok() {
            self.spatial.spline.clear();
        }
    }

    /// The 3D spline in progress through its fit points and the cursor, as points along it.
    pub fn spatial_spline_preview(&self) -> Option<Vec<[f64; 3]>> {
        if self.spatial.spline.is_empty() {
            return None;
        }
        let mut fit: Vec<[f64; 3]> = self.spatial.spline.iter().map(|(p, _)| *p).collect();
        if let Some((cursor, _)) = self.spatial.hover {
            fit.push(cursor);
        }
        Some(gc_sketch::spline_points3(&fit, 16))
    }

    /// A click of an arc tool in a 3D sketch: three points, then the arc.
    fn spatial_arc_click(&mut self, pos: [f64; 3], point: Option<Id<SkPoint>>) {
        let repeated = self.spatial.arc.iter().any(|(at, p)| {
            (point.is_some() && *p == point) || (0..3).map(|k| (at[k] - pos[k]).powi(2)).sum::<f64>() < 1e-18
        });
        if repeated {
            return;
        }
        if self.spatial.arc.len() < 2 {
            self.spatial.arc.push((pos, point));
            return;
        }
        let Some((centre, start, end)) = self.spatial_arc(pos) else {
            self.last_error = Some(gc_sketch::SketchError::InvalidValue.into());
            return;
        };
        // Which existing points the centre, start and end were placed on.
        let (first, second) = (self.spatial.arc[0].1, self.spatial.arc[1].1);
        let snaps = match self.tool() {
            ToolKind::Arc3 => [None, first, second],
            _ => [first, second, point],
        };
        let op = SketchOp::AddSpatialArc { centre, start, end, snaps };
        if self.sketch_op(op).is_ok() {
            self.spatial.arc.clear();
        }
    }
}
