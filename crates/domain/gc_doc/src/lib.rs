//! The document: everything the user creates and saves, and the only ways to change it.
//!
//! - [`Document`] owns the data. Its fields are private; reading is free, writing goes through a [`Txn`].
//! - A [`Txn`] records the first "before" image of everything it touches. Committing yields a
//!   [`DocDelta`]; dropping the transaction without committing puts every image back.
//! - [`History`] keeps deltas. Undo and redo are the same operation: swap the images with the document.
//! - [`Command`] is an edit as plain data. [`apply`] runs one in a transaction, re-solves the sketches it
//!   affects and commits, or leaves the document exactly as it was and says why.
//!
//! Invariant: every sketch in a committed document satisfies its driving constraints.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::float_cmp, reason = "tests compare exactly representable values"))]

mod cell;
mod command;
mod document;
mod history;
mod txn;

pub use cell::{
    Body, CellOp, Look, MotionPath, Placement, Robot, Solid, Waypoint, WaypointTarget, WorkFrame,
};
pub use command::{Applied, Command, Created, EditError, EndSnap, SketchOp, apply};
pub use document::{ChangeSet, Contents, DocDelta, Document, Frame, FrameKind, Param, Store, Table};
pub use gc_sketch::Sketch;
pub use history::History;
pub use txn::{DocError, Txn};
