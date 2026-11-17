//! Camera, view navigation and the mapping between screen and world. No GPU and no user-interface types:
//! everything here is plain numbers and is tested numerically.
//!
//! Conventions: right-handed, Z up. Screen coordinates are pixels with the origin at the top-left corner
//! of the viewport and Y pointing down.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::float_cmp, reason = "tests compare exactly representable values"))]

mod camera;
mod nav;
pub mod viewcube;

pub use camera::{Camera, StandardView};
pub use nav::{NavAction, NavInput, NavPreset, nav_action};
