//! Entities, stores and the document.

use std::collections::{BTreeMap, BTreeSet};

use gc_core::{AnyId, Entity, Id, IdAlloc, Kind, Revision};
use gc_expr::Unit;
use gc_sketch::Sketch;

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
}

impl Entity for Frame {
    const KIND: Kind = Kind::Frame;
    fn refs(&self, out: &mut Vec<AnyId>) {
        out.extend(self.parent.map(Id::erase));
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

    /// The world frame.
    pub fn world(&self) -> Option<Id<Frame>> {
        self.frames.iter().find(|(_, f)| f.kind == FrameKind::World).map(|(id, _)| id)
    }

    /// The origin plane with the given name (`"XY"`, `"XZ"` or `"YZ"`).
    pub fn origin_plane(&self, name: &str) -> Option<Id<Frame>> {
        self.frames.iter().find(|(_, f)| f.kind == FrameKind::OriginPlane && f.name == name).map(|(id, _)| id)
    }
}
