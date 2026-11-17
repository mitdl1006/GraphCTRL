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
mod pattern;
mod placement;
mod project;
mod relate;
mod replicate;
mod representation;
mod tree;
mod txn;

mod design;
pub use cell::{
    AcmOverride, Body, CellOp, ContactAllowance, LinkMotion, Look, MotionPath, Placement, Robot, RobotLink,
    Solid, Tool, Waypoint, WaypointTarget, WorkFrame,
};
pub use command::{
    Applied, Command, Created, DimSlot, EditError, EndSnap, SketchOp, SlotKind, TangentArc, apply,
    tangent_arc,
};
pub use design::{
    Assembly, AssemblyMesh, AssemblyPart, ComponentFlag, Components, ContactSolver, Extent, Extrude, Feature,
    FeatureKind, Occurrence, OriginElement, OriginPlane, PlaneRef, PlaneSource, PointRef, Timeline,
    WorkFeature, WorkGeometry,
};
pub use document::{ChangeSet, Contents, DocDelta, Document, Frame, FrameKind, Param, Store, Table};
pub use gc_sketch::Sketch;
pub use history::History;
pub use pattern::{ComponentPattern, PatternDirection, PatternElement, PatternLayout};
pub use placement::{ComponentOrigin, WorkShape};
pub use project::ProjectSource;
pub use relate::{
    AngleSolution, AssembleSolution, AxisSense, ComponentStatus, Dof, DofAxis, DofSymbol, Geometry, Limits,
    RelationKind, RelationStatus, Relationship, Selection, Unsolved, possible_relations,
};
pub use replicate::Replacement;
pub use representation::{PositionalRepresentation, Representations, ViewRepresentation};
pub use tree::ComponentNode;
pub use txn::{DocError, Txn};
