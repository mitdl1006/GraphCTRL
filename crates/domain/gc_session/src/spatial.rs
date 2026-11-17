//! Drawing in a 3D sketch: chained lines whose points the user interface places with
//! [`gc_tools::spatial::place`].

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
}

impl Default for SpatialState {
    fn default() -> Self {
        Self { anchor: None, hover: None, plane: 2 }
    }
}

impl SpatialState {
    pub(crate) fn reset(&mut self) {
        self.anchor = None;
        self.hover = None;
    }

    /// Drops the line in progress. Returns false when there was none.
    pub(crate) fn drop_line(&mut self) -> bool {
        self.hover = None;
        self.anchor.take().is_some()
    }
}

impl Session {
    /// True when the sketch being edited is a 3D sketch.
    pub fn sketch_is_spatial(&self) -> bool {
        self.active_sketch().is_some_and(|(_, sketch)| sketch.is_spatial())
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

    /// Where the 3D line in progress starts.
    pub fn spatial_anchor(&self) -> Option<[f64; 3]> {
        self.spatial.anchor.map(|(pos, _)| pos)
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
        if self.tool() != ToolKind::Line || !self.sketch_is_spatial() {
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
            snap_a: EndSnap { point: start_point, curve: None },
            snap_b: EndSnap { point, curve: None },
            horizontal: None,
        };
        let Ok(created) = self.sketch_op(line) else { return };
        let ends = created.iter().find_map(|c| match c {
            Created::Curve(curve) => self.active_sketch()?.1.line(*curve),
            _ => None,
        });
        let Some((from, to)) = ends else { return };
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
}
