//! The design side of the model tree (plan 9.1): work features (planes, axes, points), modelling
//! features (extrusions of sketch profiles, used for field volumes in M2-2), assemblies imported from CAD
//! files (fixed references), and the tree's order with suppression and the rollback marker.

use gc_core::{AnyId, Entity, Id, Kind};
use gc_sketch::{Curve, SkPoint, Sketch};
use serde::{Deserialize, Serialize};

use crate::cell::{Look, Placement};

/// One of the three origin planes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum OriginPlane {
    /// The XY plane (normal +Z).
    Xy,
    /// The XZ plane (normal −Y).
    Xz,
    /// The YZ plane (normal +X).
    Yz,
}

/// A plane a work feature is built on.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum PlaneRef {
    /// An origin plane.
    Origin(OriginPlane),
    /// A work plane.
    Work(Id<WorkFeature>),
    /// An origin plane of a component (it follows the component).
    Component(Occurrence, OriginPlane),
}

/// A point a work feature is built through.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum PointRef {
    /// The origin's centre point.
    Origin,
    /// A fixed position in the world, metres.
    Fixed([f64; 3]),
    /// A sketch point (follows the sketch).
    SketchPoint {
        /// The sketch.
        sketch: Id<Sketch>,
        /// The point in it.
        point: Id<SkPoint>,
    },
    /// A work point.
    Work(Id<WorkFeature>),
    /// The centre point of a component's origin (it follows the component).
    Component(Occurrence),
}

/// How a work feature is defined.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum WorkGeometry {
    /// A plane parallel to another at a distance along its normal (metres).
    PlaneOffset {
        /// The plane it is parallel to.
        base: PlaneRef,
        /// Distance along the base's normal.
        distance: f64,
    },
    /// A plane through three points.
    PlaneThroughPoints {
        /// The points (not on one line).
        points: [PointRef; 3],
    },
    /// An axis through two points.
    AxisThroughPoints {
        /// The points (distinct).
        points: [PointRef; 2],
    },
    /// An axis along a plane's normal through a point.
    AxisNormalToPlane {
        /// The plane.
        plane: PlaneRef,
        /// The point it passes through.
        through: PointRef,
    },
    /// A point.
    Point {
        /// Where.
        at: PointRef,
    },
}

impl WorkGeometry {
    /// True for a plane.
    pub fn is_plane(&self) -> bool {
        matches!(self, Self::PlaneOffset { .. } | Self::PlaneThroughPoints { .. })
    }

    /// True for a point.
    pub fn is_point(&self) -> bool {
        matches!(self, Self::Point { .. })
    }

    fn planes(&self) -> Vec<PlaneRef> {
        match self {
            Self::PlaneOffset { base, .. } => vec![*base],
            Self::AxisNormalToPlane { plane, .. } => vec![*plane],
            _ => Vec::new(),
        }
    }

    fn points(&self) -> Vec<PointRef> {
        match self {
            Self::PlaneThroughPoints { points } => points.to_vec(),
            Self::AxisThroughPoints { points } => points.to_vec(),
            Self::AxisNormalToPlane { through, .. } => vec![*through],
            Self::Point { at } => vec![*at],
            Self::PlaneOffset { .. } => Vec::new(),
        }
    }

    /// True when every number is finite.
    pub fn is_finite(&self) -> bool {
        let fixed_ok = self
            .points()
            .iter()
            .all(|p| !matches!(p, PointRef::Fixed(v) if v.iter().any(|x| !x.is_finite())));
        let distance_ok = !matches!(self, Self::PlaneOffset { distance, .. } if !distance.is_finite());
        fixed_ok && distance_ok
    }
}

/// A work plane, axis or point.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkFeature {
    /// Name shown in the model tree.
    pub name: String,
    /// How it is defined.
    pub geometry: WorkGeometry,
    /// Shown in the viewport.
    #[serde(default = "shown")]
    pub visible: bool,
}

fn shown() -> bool {
    true
}

impl Entity for WorkFeature {
    const KIND: Kind = Kind::WorkFeature;
    fn refs(&self, out: &mut Vec<AnyId>) {
        for plane in self.geometry.planes() {
            match plane {
                PlaneRef::Work(id) => out.push(id.erase()),
                PlaneRef::Component(occurrence, _) => out.push(occurrence.owner()),
                PlaneRef::Origin(_) => {}
            }
        }
        for point in self.geometry.points() {
            match point {
                PointRef::SketchPoint { sketch, .. } => out.push(sketch.erase()),
                PointRef::Work(id) => out.push(id.erase()),
                PointRef::Component(occurrence) => out.push(occurrence.owner()),
                PointRef::Origin | PointRef::Fixed(_) => {}
            }
        }
    }
}

/// How far an extrusion goes from its sketch plane, metres.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Extent {
    /// To one side, along the sketch normal (or against it when reversed).
    Distance(f64),
    /// Half to each side.
    Symmetric(f64),
    /// Different distances to the two sides.
    TwoSided {
        /// Along the normal.
        forward: f64,
        /// Against it.
        backward: f64,
    },
}

impl Extent {
    /// The range along the sketch normal, `(from, to)` with `from < to`; `None` when empty or not finite.
    pub fn range(&self, reversed: bool) -> Option<(f64, f64)> {
        let (from, to) = match *self {
            Self::Distance(d) => (0.0, d),
            Self::Symmetric(d) => (-d / 2.0, d / 2.0),
            Self::TwoSided { forward, backward } => (-backward, forward),
        };
        let (from, to) = if reversed { (-to, -from) } else { (from, to) };
        (from.is_finite() && to.is_finite() && to > from).then_some((from, to))
    }
}

/// A closed sketch profile pushed along the sketch normal into a volume.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Extrude {
    /// The sketch (a 2D sketch on a plane).
    pub sketch: Id<Sketch>,
    /// The curves of the profile (closed loops); empty for every closed loop of the sketch.
    pub profile: Vec<Id<Curve>>,
    /// How far it goes.
    pub extent: Extent,
    /// Against the sketch normal instead of along it.
    #[serde(default)]
    pub reversed: bool,
}

/// What a feature does.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum FeatureKind {
    /// An extrusion.
    Extrude(Extrude),
}

/// A modelling feature. In M2 an extrusion makes the volume of a field; there is no solid modelling.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Feature {
    /// Name shown in the model tree.
    pub name: String,
    /// What it does.
    pub kind: FeatureKind,
}

impl Feature {
    /// The sketch the feature consumes (shown folded under it in the tree).
    pub fn sketch(&self) -> Id<Sketch> {
        match &self.kind {
            FeatureKind::Extrude(extrude) => extrude.sketch,
        }
    }
}

impl Entity for Feature {
    const KIND: Kind = Kind::Feature;
    fn refs(&self, out: &mut Vec<AnyId>) {
        out.push(self.sketch().erase());
    }
}

/// One part of an imported assembly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AssemblyPart {
    /// Name from the file.
    pub name: String,
    /// The parent part (index into [`Assembly::parts`], always lower); `None` for a root.
    pub parent: Option<u32>,
    /// The part's frame relative to its parent's (a root's: relative to the assembly's placement),
    /// column-major 4 × 4, may contain scale.
    pub matrix: [[f64; 4]; 4],
    /// The meshes it shows (indices into [`Assembly::meshes`]).
    pub meshes: Vec<u32>,
}

/// A mesh of an imported assembly. The triangles stay in the file; the document keeps what the model
/// tree shows and what tells a changed file apart.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AssemblyMesh {
    /// Name from the file.
    pub name: String,
    /// Base colour, RGBA in 0..=1.
    pub color: [f32; 4],
    /// Number of triangles.
    pub triangles: u32,
}

/// An assembly imported from a CAD file (GLB, later STEP). A fixed reference: its parts are not edited,
/// only the whole is placed and shown or hidden.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Assembly {
    /// Name shown in the model tree.
    pub name: String,
    /// Path of the file.
    pub file: String,
    /// Where the assembly's origin stands in the world.
    pub placement: Placement,
    /// The part tree, parents before children.
    pub parts: Vec<AssemblyPart>,
    /// The meshes the parts show.
    pub meshes: Vec<AssemblyMesh>,
    /// How it is shown.
    #[serde(default)]
    pub look: Look,
}

impl Assembly {
    /// True when the tree is well formed: parents come before children and mesh indices exist.
    pub fn is_valid(&self) -> bool {
        self.parts.iter().enumerate().all(|(index, part)| {
            part.parent.is_none_or(|p| (p as usize) < index)
                && part.meshes.iter().all(|m| (*m as usize) < self.meshes.len())
                && part.matrix.iter().flatten().all(|v| v.is_finite())
        })
    }

    /// The world matrix of a part relative to the assembly's origin (the product down from its root).
    pub fn part_matrix(&self, part: usize) -> Option<[[f64; 4]; 4]> {
        let mut chain = Vec::new();
        let mut at = Some(part);
        while let Some(index) = at {
            let node = self.parts.get(index)?;
            chain.push(index);
            at = node.parent.map(|p| p as usize).filter(|p| *p < index);
        }
        Some(chain.iter().rev().fold(IDENTITY, |acc, i| multiply(&acc, &self.parts[*i].matrix)))
    }
}

const IDENTITY: [[f64; 4]; 4] =
    [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];

fn multiply(a: &[[f64; 4]; 4], b: &[[f64; 4]; 4]) -> [[f64; 4]; 4] {
    let mut out = [[0.0; 4]; 4];
    for (column, out_column) in out.iter_mut().enumerate() {
        for (row, value) in out_column.iter_mut().enumerate() {
            *value = (0..4).map(|k| a[k][row] * b[column][k]).sum();
        }
    }
    out
}

impl Entity for Assembly {
    const KIND: Kind = Kind::Assembly;
    fn refs(&self, _: &mut Vec<AnyId>) {}
}

/// The model tree's state: the order of its design items (sketches, work features, features,
/// assemblies), which items are suppressed or hidden, and where the rollback marker stands. At most one
/// per document.
///
/// It is read through [`crate::Document::model_order`], which drops ids that no longer exist and appends
/// items it does not list, so commands that add or delete items need not keep it up to date.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Timeline {
    /// Items in tree order.
    pub items: Vec<AnyId>,
    /// Items that are kept but not computed (not drawn, not used for planning).
    #[serde(default)]
    pub suppressed: Vec<AnyId>,
    /// The last item computed; the items after it are rolled back. `None`: the marker is at the end.
    #[serde(default)]
    pub rollback_after: Option<AnyId>,
    /// Items hidden in the viewport that keep no look of their own (sketches, waypoints, paths, work
    /// frames, work features, features).
    #[serde(default)]
    pub hidden: Vec<AnyId>,
    /// Component states that are not kept elsewhere (assembly P1-1).
    #[serde(default, skip_serializing_if = "Components::is_empty")]
    pub components: Components,
    /// Component patterns: copies of components arranged in rows and columns or round an axis.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub patterns: Vec<crate::pattern::ComponentPattern>,
    /// View and positional representations.
    #[serde(default, skip_serializing_if = "crate::representation::Representations::is_empty")]
    pub representations: crate::representation::Representations,
}

/// An occurrence in the assembly tree (docs/ux/inventor-assembly-parity.md, P1-1): a component placed
/// in the cell, or a part inside an imported assembly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Occurrence {
    /// A body, robot or imported assembly.
    Item(AnyId),
    /// A part of an imported assembly.
    Part {
        /// The assembly.
        assembly: Id<Assembly>,
        /// Index into [`Assembly::parts`].
        index: u32,
    },
    /// A link of a robot: its frame follows the robot's joint values.
    Link {
        /// The robot.
        robot: Id<crate::cell::Robot>,
        /// Index into [`crate::cell::Robot::links`].
        index: u32,
    },
}

impl Occurrence {
    /// The document item the occurrence belongs to (an assembly for its parts).
    pub fn owner(&self) -> AnyId {
        match *self {
            Occurrence::Item(item) => item,
            Occurrence::Part { assembly, .. } => assembly.erase(),
            Occurrence::Link { robot, .. } => robot.erase(),
        }
    }
}

/// A state of a component that can be switched on and off.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComponentFlag {
    /// Fixed in space: all its degrees of freedom are removed (assembly constraints move only the others).
    Grounded,
    /// Drawn.
    Visible,
    /// Selectable; a disabled component is drawn in the background style.
    Enabled,
    /// Kept but not computed: not drawn, not an obstacle.
    Suppressed,
}

/// Component states kept in the model tree's state. Visibility of bodies, robots and assemblies stays in
/// their looks and suppression of assemblies in [`Timeline::suppressed`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Components {
    /// Grounded components.
    #[serde(default)]
    pub grounded: Vec<Occurrence>,
    /// Disabled components.
    #[serde(default)]
    pub disabled: Vec<Occurrence>,
    /// Hidden assembly parts.
    #[serde(default)]
    pub hidden: Vec<Occurrence>,
    /// Suppressed assembly parts.
    #[serde(default)]
    pub suppressed: Vec<Occurrence>,
    /// Origin elements shown in the viewport (all are hidden by default).
    #[serde(default)]
    pub origins_shown: Vec<(Occurrence, OriginElement)>,
    /// Components in the contact set (Inventor 2026 help, About the Contact Solver,
    /// GUID-0BBD4ACE-7BD1-40BF-A011-74416B229035): they push one another, or stop, while dragged or driven.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contact_set: Vec<Occurrence>,
    /// Components whose degrees-of-freedom symbol is shown (iProperties, Occurrence tab).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dof_shown: Vec<Occurrence>,
    /// The degrees-of-freedom symbol is shown for every component (View tab, Visibility panel).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dof_all: bool,
    /// Interactive Contact: which components the contact solver analyses.
    #[serde(default, skip_serializing_if = "ContactSolver::is_off")]
    pub contact_solver: ContactSolver,
}

/// Interactive Contact (Document Settings, Modeling tab): the contact solver off, on for the contact set
/// only, or on for all components.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContactSolver {
    /// Contact Solver Off.
    #[default]
    Off,
    /// Contact Set Only.
    ContactSet,
    /// All Components.
    All,
}

impl ContactSolver {
    /// True for [`ContactSolver::Off`].
    pub fn is_off(&self) -> bool {
        *self == Self::Off
    }
}

/// An element of a component's origin folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum OriginElement {
    /// An origin plane.
    Plane(OriginPlane),
    /// An axis: 0 = X, 1 = Y, 2 = Z.
    Axis(u8),
    /// The centre point.
    Point,
}

impl Components {
    /// True when nothing is stored.
    pub fn is_empty(&self) -> bool {
        self.grounded.is_empty()
            && self.disabled.is_empty()
            && self.hidden.is_empty()
            && self.suppressed.is_empty()
            && self.origins_shown.is_empty()
            && self.contact_set.is_empty()
            && self.contact_solver.is_off()
            && self.dof_shown.is_empty()
            && !self.dof_all
    }
}

impl Entity for Timeline {
    const KIND: Kind = Kind::Timeline;
    // Not references: a deleted item simply drops out of the order.
    fn refs(&self, _: &mut Vec<AnyId>) {}
}

/// What a sketch plane is attached to, so that the sketch follows it (plan 9.1: sketches on work planes
/// and on faces of imported parts).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum PlaneSource {
    /// A work plane: the sketch's XY plane is the work plane.
    Work(Id<WorkFeature>),
    /// A flat face of an imported part, given as the plane's pose in the part's frame (worked out from
    /// the picked triangle when the sketch was made). Follows the assembly when it is moved.
    Face {
        /// The assembly.
        assembly: Id<Assembly>,
        /// The part (index into [`Assembly::parts`]).
        part: u32,
        /// The picked triangle (mesh, triangle index), kept to show what the sketch is on.
        triangle: (u32, u32),
        /// Origin of the plane in the part's frame.
        xyz: [f64; 3],
        /// Rotation of the plane in the part's frame (the plane is its XY plane), scalar last.
        quat_xyzw: [f64; 4],
    },
    /// An origin plane of a component: the sketch moves with the component.
    Component {
        /// The component.
        occurrence: Occurrence,
        /// Which of its origin planes.
        plane: OriginPlane,
    },
}

impl PlaneSource {
    pub(crate) fn reference(&self) -> AnyId {
        match self {
            Self::Work(id) => id.erase(),
            Self::Face { assembly, .. } => assembly.erase(),
            Self::Component { occurrence, .. } => occurrence.owner(),
        }
    }
}
