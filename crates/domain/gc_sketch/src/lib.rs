//! Sketches: points, lines, circles and arcs with geometric constraints and dimensions.
//!
//! A [`Sketch`] is plain data. Solving, adding constraints with redundancy checks and dragging are done by
//! translating it to a [`gc_solver::System`] and writing the result back. A sketch that was changed
//! through this API always satisfies its driving constraints: an edit that cannot be solved is refused
//! and leaves the sketch as it was.
//!
//! Conventions: a planar sketch lies in the XY plane of its frame (U = +X, V = +Y, normal = +Z); positions
//! are metres, angles radians. Every curve owns its end points; shared corners are `Coincident`
//! constraints, which the solver turns into bit-identical coordinates.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::float_cmp, reason = "tests compare exactly representable values"))]

mod arc3;
mod bind;
mod cut;
mod drag;
mod edit;
mod model;
mod pattern;
mod transform;

pub use arc3::Arc3;
pub use cut::{CutResult, Piece};
pub use drag::{SketchDrag, SketchGrab};
pub use edit::{AddResult, Item, SketchError, SketchReport};
pub use gc_solver::{DepKind, DragRefused, FrameOutcome, Status};
pub use model::{
    Curve, CurveGeom, DimKind, Dimension, ExternalRef, GeoConstraint, PointKind, SkPoint, Sketch, Space,
    closed_spline_points, closed_spline_points3, ellipse_param, ellipse_points, spline_closed, spline_points,
    spline_points3,
};
pub use pattern::{PatternCopy, PatternDefinition, PatternDirection, PatternStep, SketchPattern};
pub use transform::{CopyResult, MoveRules, copied_items};
