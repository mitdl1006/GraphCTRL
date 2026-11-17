//! Dragging sketch geometry.

use std::collections::BTreeMap;

use gc_core::Id;
use gc_solver::{DragRefused, Dragging, FrameOutcome, Grab, ParamId};

use crate::model::{Curve, CurveGeom, SkPoint, Sketch};

/// What the user took hold of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SketchGrab {
    /// A point: it follows the cursor.
    Point(Id<SkPoint>),
    /// A curve: a line moves as a whole, a circle or arc changes its radius.
    Curve(Id<Curve>),
}

/// A drag in progress. It does not borrow the sketch: the sketch stays untouched until
/// [`SketchDrag::apply`] writes the result, so cancelling is simply dropping this value.
pub struct SketchDrag {
    drag: Dragging,
    points: BTreeMap<Id<SkPoint>, Vec<ParamId>>,
    radii: BTreeMap<Id<Curve>, ParamId>,
}

impl Sketch {
    /// Starts a drag with the cursor at `cursor` (sketch coordinates). Only planar sketches can be
    /// dragged this way.
    pub fn begin_drag(&self, grab: SketchGrab, cursor: [f64; 2]) -> Result<SketchDrag, DragRefused> {
        if self.is_spatial() {
            return Err(DragRefused::NothingMobile);
        }
        let binding = self.bind(None).map_err(|_unbindable| DragRefused::Unsolved)?;
        let p2 = |p: Id<SkPoint>| match binding.points.get(&p).map(Vec::as_slice) {
            Some([x, y]) => Some([*x, *y]),
            _ => None,
        };
        let solver_grab = match grab {
            SketchGrab::Point(p) => p2(p).map(Grab::Point),
            SketchGrab::Curve(curve) => match self.curves.get(&curve).map(|c| c.geom) {
                Some(CurveGeom::Line { a, b }) => p2(a).zip(p2(b)).map(|(a, b)| Grab::Body(vec![a, b])),
                Some(CurveGeom::Circle { c, .. } | CurveGeom::Arc { c, .. }) => p2(c)
                    .zip(binding.radii.get(&curve))
                    .map(|(c, r)| Grab::Rim(gc_solver::Round { c, r: *r })),
                None => None,
            },
        };
        let Some(solver_grab) = solver_grab else { return Err(DragRefused::NothingMobile) };
        match binding.sys.begin_drag(&solver_grab, cursor) {
            Ok(drag) => Ok(SketchDrag { drag, points: binding.points, radii: binding.radii }),
            Err((_, refused)) => Err(refused),
        }
    }
}

impl Sketch {
    /// Starts dragging a point of a 3D sketch with the cursor at `cursor` (sketch-space coordinates: the
    /// caller turns the mouse ray into a position, on a screen-parallel plane or along a locked axis).
    /// Only points can be dragged this way.
    pub fn begin_drag_3d(&self, point: Id<SkPoint>, cursor: [f64; 3]) -> Result<SketchDrag, DragRefused> {
        if !self.is_spatial() {
            return Err(DragRefused::NothingMobile);
        }
        let binding = self.bind(None).map_err(|_unbindable| DragRefused::Unsolved)?;
        let grab = match binding.points.get(&point).map(Vec::as_slice) {
            Some([x, y, z]) => Grab::Point3([*x, *y, *z]),
            _ => return Err(DragRefused::NothingMobile),
        };
        match binding.sys.begin_drag_3d(&grab, cursor) {
            Ok(drag) => Ok(SketchDrag { drag, points: binding.points, radii: binding.radii }),
            Err((_, refused)) => Err(refused),
        }
    }
}

impl SketchDrag {
    /// Moves a cursor in space (a drag started with [`Sketch::begin_drag_3d`]).
    pub fn update_3d(&mut self, cursor: [f64; 3]) -> FrameOutcome {
        self.drag.update_3d(cursor)
    }

    /// Moves the cursor.
    pub fn update(&mut self, cursor: [f64; 2]) -> FrameOutcome {
        self.drag.update(cursor)
    }

    /// The previewed position of a point at the current frame.
    pub fn pos(&self, point: Id<SkPoint>) -> Option<[f64; 3]> {
        let params = self.points.get(&point)?;
        let mut pos = [0.0; 3];
        for (axis, param) in params.iter().enumerate() {
            pos[axis] = self.drag.system().value(*param);
        }
        Some(pos)
    }

    /// The previewed radius of a circle or arc at the current frame.
    pub fn radius(&self, curve: Id<Curve>) -> Option<f64> {
        self.radii.get(&curve).map(|param| self.drag.system().value(*param))
    }

    /// Ends the drag and writes the final geometry into the sketch. Returns whether anything changed.
    pub fn apply(self, sketch: &mut Sketch) -> bool {
        let system = self.drag.commit();
        sketch.write_back(&system, &self.points, &self.radii)
    }
}
