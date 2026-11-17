//! Sketch tools as state machines that know nothing about windows or widgets.
//!
//! A tool receives [`ToolEvent`]s in sketch coordinates and answers with [`ToolOutput`]s: commands to
//! apply, drags to start, values to ask the user for. What it would draw right now is its
//! [`Preview`]. Because tools are pure state machines, whole editing sessions run in tests without a
//! user interface.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::float_cmp, reason = "tests compare exactly representable values"))]

mod pick;
pub mod spatial;
mod tool;

pub use pick::{Inference, Snap, SnapKind, pick, snap};
pub use tool::{
    ConstraintKind, HudField, HudSlot, Key, Preview, Prompt, Tool, ToolCommand, ToolEvent, ToolKind,
    ToolOutput,
};

/// Pixel tolerances of picking and snapping.
pub mod tolerance {
    /// A point is hit within this many pixels.
    pub const POINT_PX: f64 = 8.0;
    /// A curve is hit within this many pixels.
    pub const CURVE_PX: f64 = 6.0;
    /// A line being drawn snaps to horizontal or vertical within this many pixels.
    pub const DIRECTION_PX: f64 = 4.0;
    /// A dimension text is hit within this many pixels.
    pub const TEXT_PX: f64 = 14.0;
}
