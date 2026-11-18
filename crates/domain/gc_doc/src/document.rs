//! Entities, stores and the document.

use std::collections::{BTreeMap, BTreeSet};

use gc_core::{AnyId, Entity, Id, IdAlloc, Kind, Revision};
use gc_expr::Unit;
use gc_sketch::Sketch;

use crate::cell::{Body, MotionPath, Robot, Waypoint, WorkFrame};

/// The ids of the entities a commit, undo or redo touched.
pub type ChangeSet = BTreeSet<AnyId>;

/// A named value usable in dimension expressions, e.g. `width = 120 mm`.
#[derive(Clone, Debug, PartialEq)]
pub struct Param {
    /// The name expressions refer to.
    pub name: String,
    /// The expression as typed.
    pub source: String,
    /// The unit a bare number in `source` is read in.
    pub unit: Unit,
    /// The value of `source` in SI units.
    pub value: f64,
}

impl Entity for Param {
    const KIND: Kind = Kind::Param;
    fn refs(&self, _: &mut Vec<AnyId>) {}
}

/// What a frame is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameKind {
    /// The world frame: the root of everything.
    World,
    /// One of the three origin planes; sketches are placed on these.
    OriginPlane,
    /// A frame the user created (offset work plane, mount frame).
    User,
}

/// A coordinate frame: a pose relative to a parent frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// Name shown in the browser.
    pub name: String,
    /// What it is for.
    pub kind: FrameKind,
    /// The frame this pose is relative to; `None` for the world frame.
    pub parent: Option<Id<Frame>>,
    /// Translation in the parent frame, metres.
    pub xyz: [f64; 3],
    /// Rotation in the parent frame, unit quaternion with the scalar last.
    pub quat_xyzw: [f64; 4],
    /// For a sketch plane attached to a work plane or a part face: what it follows. Its pose is then
    /// worked out from that, and `xyz`/`quat_xyzw` keep the pose it had when last resolved.
    pub source: Option<crate::design::PlaneSource>,
}

impl Entity for Frame {
    const KIND: Kind = Kind::Frame;
    fn refs(&self, out: &mut Vec<AnyId>) {
        out.extend(self.parent.map(Id::erase));
        out.extend(self.source.map(|s| s.reference()));
    }
}

/// All entities of one type, ordered by id so that iteration and files are deterministic.
#[derive(Clone, Debug, PartialEq)]
pub struct Store<T: Entity> {
    pub(crate) items: BTreeMap<Id<T>, T>,
}

impl<T: Entity> Default for Store<T> {
    fn default() -> Self {
        Self { items: BTreeMap::new() }
    }
}

impl<T: Entity> Store<T> {
    /// The entity with this id.
    pub fn get(&self, id: Id<T>) -> Option<&T> {
        self.items.get(&id)
    }

    /// All entities, ascending by id.
    pub fn iter(&self) -> impl Iterator<Item = (Id<T>, &T)> {
        self.items.iter().map(|(id, item)| (*id, item))
    }

    /// Number of entities.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// True when the store holds nothing.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// The first "before" image of each touched entity. `None` means it did not exist.
#[derive(Clone, Debug, PartialEq)]
pub struct Delta<T: Entity> {
    pub(crate) images: BTreeMap<Id<T>, Option<T>>,
}

impl<T: Entity> Default for Delta<T> {
    fn default() -> Self {
        Self { images: BTreeMap::new() }
    }
}

impl<T: Entity> Delta<T> {
    /// Exchanges the images with the live store. The same call undoes and redoes.
    fn swap(&mut self, store: &mut Store<T>, changed: &mut ChangeSet) {
        for (id, image) in &mut self.images {
            *image = match image.take() {
                Some(old) => store.items.insert(*id, old),
                None => store.items.remove(id),
            };
            changed.insert(id.erase());
        }
    }
}

/// Maps an entity type to its store in the document and its delta.
pub trait Table<T: Entity> {
    /// The store of `T`.
    fn store(doc: &Document) -> &Store<T>;
    /// The store of `T`, mutably. Only [`crate::Txn`] calls this.
    #[doc(hidden)]
    fn store_mut(doc: &mut Document) -> &mut Store<T>;
    /// The delta of `T`.
    #[doc(hidden)]
    fn delta_mut(delta: &mut DocDelta) -> &mut Delta<T>;
}

macro_rules! tables {
    ($( $field:ident : $ty:ty ),* $(,)?) => {
        /// Everything the user creates and saves.
        #[derive(Clone, Debug, PartialEq)]
        pub struct Document {
            $( $field: Store<$ty>, )*
            pub(crate) ids: IdAlloc,
            pub(crate) revision: Revision,
        }

        /// What one transaction changed: enough to undo it, and after undoing, to redo it.
        #[derive(Clone, Debug, Default, PartialEq)]
        pub struct DocDelta {
            $( $field: Delta<$ty>, )*
        }

        impl DocDelta {
            /// Exchanges all images with the document and returns the ids touched.
            pub(crate) fn swap_all(&mut self, doc: &mut Document) -> ChangeSet {
                let mut changed = ChangeSet::new();
                $( self.$field.swap(&mut doc.$field, &mut changed); )*
                changed
            }

            /// The ids of everything this delta touches.
            pub fn touched(&self) -> ChangeSet {
                let mut changed = ChangeSet::new();
                $( changed.extend(self.$field.images.keys().map(|id| id.erase())); )*
                changed
            }

            /// True when nothing was changed.
            pub fn is_empty(&self) -> bool {
                true $( && self.$field.images.is_empty() )*
            }
        }

        $(
            impl Table<$ty> for Document {
                fn store(doc: &Document) -> &Store<$ty> { &doc.$field }
                fn store_mut(doc: &mut Document) -> &mut Store<$ty> { &mut doc.$field }
                fn delta_mut(delta: &mut DocDelta) -> &mut Delta<$ty> { &mut delta.$field }
            }
        )*

        impl Document {
            /// True when an entity with this id exists.
            pub fn contains(&self, id: AnyId) -> bool {
                $( if id.kind == <$ty as Entity>::KIND {
                    return self.$field.items.contains_key(&Id::from_raw(id.raw));
                } )*
                false
            }

            /// Every reference between entities, as `(from, to)` pairs.
            pub(crate) fn references(&self) -> Vec<(AnyId, AnyId)> {
                let mut out = Vec::new();
                let mut refs = Vec::new();
                $( for (id, item) in self.$field.iter() {
                    refs.clear();
                    item.refs(&mut refs);
                    out.extend(refs.iter().map(|to| (id.erase(), *to)));
                } )*
                out
            }

            /// True when both documents hold the same entities, whatever their revision counters say.
            pub fn same_content(&self, other: &Self) -> bool {
                true $( && self.$field == other.$field )*
            }
        }
    };
}

tables! {
    params: Param,
    frames: Frame,
    sketches: Sketch,
    bodies: Body,
    robots: Robot,
    waypoints: Waypoint,
    paths: MotionPath,
    work_frames: WorkFrame,
    work_features: crate::design::WorkFeature,
    features: crate::design::Feature,
    assemblies: crate::design::Assembly,
    timelines: crate::design::Timeline,
    relationships: crate::relate::Relationship,
}

/// Everything a stored document consists of, as [`Document::restore`] takes it.
#[derive(Clone, Debug)]
pub struct Contents {
    /// The next id the document would hand out.
    pub next_id: std::num::NonZeroU64,
    /// Parameters.
    pub params: BTreeMap<Id<Param>, Param>,
    /// Frames.
    pub frames: BTreeMap<Id<Frame>, Frame>,
    /// Sketches.
    pub sketches: BTreeMap<Id<Sketch>, Sketch>,
    /// Bodies.
    pub bodies: BTreeMap<Id<Body>, Body>,
    /// Robots.
    pub robots: BTreeMap<Id<Robot>, Robot>,
    /// Waypoints.
    pub waypoints: BTreeMap<Id<Waypoint>, Waypoint>,
    /// Paths.
    pub paths: BTreeMap<Id<MotionPath>, MotionPath>,
    /// Work frames.
    pub work_frames: BTreeMap<Id<WorkFrame>, WorkFrame>,
    /// Work planes, axes and points.
    pub work_features: BTreeMap<Id<crate::design::WorkFeature>, crate::design::WorkFeature>,
    /// Modelling features.
    pub features: BTreeMap<Id<crate::design::Feature>, crate::design::Feature>,
    /// Imported assemblies.
    pub assemblies: BTreeMap<Id<crate::design::Assembly>, crate::design::Assembly>,
    /// The model tree's order (at most one).
    pub timelines: BTreeMap<Id<crate::design::Timeline>, crate::design::Timeline>,
    /// Assembly constraints and joints.
    pub relationships: BTreeMap<Id<crate::relate::Relationship>, crate::relate::Relationship>,
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

impl Document {
    /// A new document with the world frame and the three origin planes.
    ///
    /// Each origin plane is a frame whose local XY plane is the sketch plane:
    /// XY (U +X, V +Y, normal +Z), XZ (U +X, V +Z, normal -Y), YZ (U +Y, V +Z, normal +X).
    pub fn new() -> Self {
        let mut doc = Self {
            params: Store::default(),
            frames: Store::default(),
            sketches: Store::default(),
            bodies: Store::default(),
            robots: Store::default(),
            waypoints: Store::default(),
            paths: Store::default(),
            work_frames: Store::default(),
            work_features: Store::default(),
            features: Store::default(),
            assemblies: Store::default(),
            timelines: Store::default(),
            relationships: Store::default(),
            ids: IdAlloc::new(),
            revision: Revision::default(),
        };
        let world: Id<Frame> = doc.ids.alloc();
        let half = std::f64::consts::FRAC_1_SQRT_2;
        let frame = |name: &str, kind, parent, quat_xyzw| Frame {
            name: name.to_owned(),
            kind,
            parent,
            xyz: [0.0; 3],
            quat_xyzw,
            source: None,
        };
        doc.frames.items.insert(world, frame("World", FrameKind::World, None, [0.0, 0.0, 0.0, 1.0]));
        for (name, quat) in
            [("XY", [0.0, 0.0, 0.0, 1.0]), ("XZ", [half, 0.0, 0.0, half]), ("YZ", [0.5, 0.5, 0.5, 0.5])]
        {
            let id: Id<Frame> = doc.ids.alloc();
            doc.frames.items.insert(id, frame(name, FrameKind::OriginPlane, Some(world), quat));
        }
        doc
    }

    /// The raw value the next new id will get. Saved with the document so that ids are never reused.
    pub fn next_id(&self) -> std::num::NonZeroU64 {
        self.ids.peek()
    }

    /// Rebuilds a document from stored contents. Fails when an entity refers to one that is missing.
    pub fn restore(contents: Contents) -> Result<Self, crate::DocError> {
        let mut doc = Self {
            params: Store { items: contents.params },
            frames: Store { items: contents.frames },
            sketches: Store { items: contents.sketches },
            bodies: Store { items: contents.bodies },
            robots: Store { items: contents.robots },
            waypoints: Store { items: contents.waypoints },
            paths: Store { items: contents.paths },
            work_frames: Store { items: contents.work_frames },
            work_features: Store { items: contents.work_features },
            features: Store { items: contents.features },
            assemblies: Store { items: contents.assemblies },
            timelines: Store { items: contents.timelines },
            relationships: Store { items: contents.relationships },
            ids: IdAlloc::new(),
            revision: Revision::default(),
        };
        // Whatever the file claims, new ids must be above every id in use.
        let mut highest = contents.next_id;
        let mut raise = |raw: std::num::NonZeroU64| highest = highest.max(raw.saturating_add(1));
        doc.params.items.keys().for_each(|id| raise(id.raw()));
        doc.frames.items.keys().for_each(|id| raise(id.raw()));
        doc.bodies.items.keys().for_each(|id| raise(id.raw()));
        doc.robots.items.keys().for_each(|id| raise(id.raw()));
        doc.waypoints.items.keys().for_each(|id| raise(id.raw()));
        doc.paths.items.keys().for_each(|id| raise(id.raw()));
        doc.work_frames.items.keys().for_each(|id| raise(id.raw()));
        doc.work_features.items.keys().for_each(|id| raise(id.raw()));
        doc.features.items.keys().for_each(|id| raise(id.raw()));
        doc.assemblies.items.keys().for_each(|id| raise(id.raw()));
        doc.timelines.items.keys().for_each(|id| raise(id.raw()));
        doc.relationships.items.keys().for_each(|id| raise(id.raw()));
        for (id, sketch) in &doc.sketches.items {
            raise(id.raw());
            sketch.points.keys().for_each(|p| raise(p.raw()));
            sketch.curves.keys().for_each(|c| raise(c.raw()));
            sketch.constraints.keys().for_each(|c| raise(c.raw()));
            sketch.dimensions.keys().for_each(|d| raise(d.raw()));
        }
        if let Some(before) = std::num::NonZeroU64::new(highest.get() - 1) {
            doc.ids.reserve_through(before);
        }
        match doc.references().into_iter().find(|(_, to)| !doc.contains(*to)) {
            Some((from, to)) => Err(crate::DocError::Dangling(from, to)),
            None => Ok(doc),
        }
    }

    /// Counts committed changes (including undo and redo). Caches compare it to know they are stale.
    pub fn revision(&self) -> Revision {
        self.revision
    }

    /// The entity with this id.
    pub fn get<T: Entity>(&self, id: Id<T>) -> Option<&T>
    where
        Self: Table<T>,
    {
        <Self as Table<T>>::store(self).get(id)
    }

    /// All named parameters.
    pub fn params(&self) -> &Store<Param> {
        &self.params
    }

    /// All frames.
    pub fn frames(&self) -> &Store<Frame> {
        &self.frames
    }

    /// All sketches.
    pub fn sketches(&self) -> &Store<Sketch> {
        &self.sketches
    }

    /// All bodies.
    pub fn bodies(&self) -> &Store<Body> {
        &self.bodies
    }

    /// All robots.
    pub fn robots(&self) -> &Store<Robot> {
        &self.robots
    }

    /// All waypoints.
    pub fn waypoints(&self) -> &Store<Waypoint> {
        &self.waypoints
    }

    /// All paths.
    pub fn paths(&self) -> &Store<MotionPath> {
        &self.paths
    }

    /// All work frames.
    pub fn work_frames(&self) -> &Store<WorkFrame> {
        &self.work_frames
    }

    /// Work planes, axes and points.
    pub fn work_features(&self) -> &Store<crate::design::WorkFeature> {
        &self.work_features
    }

    /// Modelling features.
    pub fn features(&self) -> &Store<crate::design::Feature> {
        &self.features
    }

    /// Imported assemblies.
    pub fn assemblies(&self) -> &Store<crate::design::Assembly> {
        &self.assemblies
    }

    /// Assembly constraints and joints.
    pub fn relationships(&self) -> &Store<crate::relate::Relationship> {
        &self.relationships
    }

    /// The stored model-tree order, if any was set (see [`Document::model_order`]).
    pub fn timeline(&self) -> Option<(Id<crate::design::Timeline>, &crate::design::Timeline)> {
        self.timelines.iter().next()
    }

    /// The Document Settings (the defaults for a document that has none saved).
    pub fn settings(&self) -> crate::settings::DocSettings {
        self.timeline().map(|(_, t)| t.settings).unwrap_or_default()
    }

    /// The design items of the model tree (sketches, work features, features, assemblies) in order: the
    /// stored order without ids that no longer exist, then the items it does not list, oldest first.
    pub fn model_order(&self) -> Vec<AnyId> {
        let mut order: Vec<AnyId> = self
            .timeline()
            .map(|(_, t)| t.items.iter().copied().filter(|id| self.contains(*id)).collect())
            .unwrap_or_default();
        let mut rest: Vec<AnyId> = self
            .sketches
            .items
            .keys()
            .map(|id| id.erase())
            .chain(self.work_features.items.keys().map(|id| id.erase()))
            .chain(self.features.items.keys().map(|id| id.erase()))
            .chain(self.assemblies.items.keys().map(|id| id.erase()))
            .filter(|id| !order.contains(id))
            .collect();
        rest.sort_by_key(|id| id.raw);
        order.extend(rest);
        order
    }

    /// True when the item is suppressed, or stands after the rollback marker: kept, but not computed.
    pub fn is_inactive(&self, item: AnyId) -> bool {
        let Some((_, timeline)) = self.timeline() else { return false };
        if timeline.suppressed.contains(&item) {
            return true;
        }
        match timeline.rollback_after.filter(|marker| self.contains(*marker)) {
            None => false,
            Some(marker) => {
                let order = self.model_order();
                let at = |id: AnyId| order.iter().position(|o| *o == id);
                matches!((at(marker), at(item)), (Some(m), Some(i)) if i > m)
            }
        }
    }

    /// The pose of a frame in the world: its own pose composed with those of its ancestors.
    pub fn frame_world_pose(&self, frame: Id<Frame>) -> Option<gc_math::DPose3> {
        let mut pose = gc_math::DPose3::IDENTITY;
        let mut at = Some(frame);
        // A parent chain longer than the number of frames would be a cycle.
        for _ in 0..=self.frames.len() {
            let Some(id) = at else { return Some(pose) };
            let f = self.frames.get(id)?;
            // A frame attached to a work plane or a part face is where that is now.
            if let Some(attached) = f.source.and_then(|source| self.plane_source_pose(&source)) {
                return Some(attached * pose);
            }
            let [x, y, z, w] = f.quat_xyzw;
            let local = gc_math::DPose3::from_parts(
                gc_math::DVec3::from_array(f.xyz),
                gc_math::DQuat::from_xyzw(x, y, z, w),
            );
            pose = local * pose;
            at = f.parent;
        }
        None
    }

    /// The world frame.
    pub fn world(&self) -> Option<Id<Frame>> {
        self.frames.iter().find(|(_, f)| f.kind == FrameKind::World).map(|(id, _)| id)
    }

    /// The origin plane with the given name (`"XY"`, `"XZ"` or `"YZ"`).
    pub fn origin_plane(&self, name: &str) -> Option<Id<Frame>> {
        self.frames.iter().find(|(_, f)| f.kind == FrameKind::OriginPlane && f.name == name).map(|(id, _)| id)
    }
}
