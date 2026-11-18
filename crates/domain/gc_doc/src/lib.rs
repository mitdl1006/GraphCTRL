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
mod folder;
mod grip;
mod history;
mod iproperties;
mod pattern;
mod placement;
mod project;
mod relate;
mod replicate;
mod representation;
mod role;
mod settings;
mod sketch_offset;
mod subassembly;
mod toolkit;
mod tree;
mod txn;
mod work;

mod design;
pub use cell::{
    AcmOverride, Body, CellOp, ContactAllowance, LinkMotion, Look, MotionPath, Placement, Robot, RobotLink,
    Solid, Tool, Waypoint, WaypointTarget, WorkFrame,
};
pub use command::{
    Applied, ArcSlotKind, ChamferKind, Command, Created, DimSlot, EditError, EndSnap, HelixShape, Motion,
    SketchOp, SketchPatternDirection, SlotKind, TangentArc, apply, evaluate_value, flipped,
    preview_sketch_op, tangent_arc,
};
pub use design::{
    Assembly, AssemblyMesh, AssemblyPart, ComponentFlag, Components, ContactSolver, CurveRef, Extent,
    Extrude, Feature, FeatureKind, LineRef, Occurrence, OriginAxis, OriginElement, OriginPlane, PlaneRef,
    PlaneSource, PointRef, Timeline, WorkFeature, WorkGeometry,
};
pub use document::{ChangeSet, Contents, DocDelta, Document, Frame, FrameKind, Param, Store, Table};
pub use folder::{BrowserFolder, FolderOp};
pub use gc_expr::Unit;
pub use gc_sketch::Sketch;
pub use grip::{GripLine, GripMotion, GripReference, grip_pose, grip_pose_on};
pub use history::History;
pub use iproperties::{BomStructure, DEFAULT_DENSITY, IProperties};
pub use pattern::{ComponentPattern, PatternDirection, PatternElement, PatternLayout};
pub use placement::{ComponentOrigin, WorkShape};
pub use project::ProjectSource;
pub use relate::{
    AngleSolution, AssembleSolution, AxisSense, ComponentStatus, Dof, DofAxis, DofSymbol, Geometry,
    JointCondition, JointRange, Limits, LockedPose, RelationKind, RelationStatus, Relationship, Selection,
    Unsolved, is_joint_kind, possible_relations, takes_limits,
};
pub use replicate::Replacement;
pub use representation::{PositionalRepresentation, Representations, ViewRepresentation};
pub use role::{AimReference, PathMotion, PathTravel, RoleKind, SketchRole, ToolAim};
pub use settings::{AngleUnit, DimensionDisplay, DocSettings, LengthUnit};
pub use subassembly::{Subassembly, SubassemblyId, SubassemblyOp, TreeKey, TreeRow};
pub use toolkit::{ActionScope, FittedTool, IoDirection, IoSignal, ToolAction, ToolKit};
pub use tree::ComponentNode;
pub use txn::{DocError, Txn};
pub use work::{
    Effect, EffectTarget, Location, MoveTarget, ObjectType, RobotChoice, StateValue, StateVar, Step, Task,
    WorkObject, WorkOp, effect_number,
};
