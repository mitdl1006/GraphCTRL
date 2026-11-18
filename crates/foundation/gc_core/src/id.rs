//! Typed entity ids.

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::num::NonZeroU64;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// What an id refers to. Every id of a document, whatever its kind, comes from the same [`IdAlloc`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Kind {
    /// A named parameter (user variable).
    Param,
    /// A coordinate frame.
    Frame,
    /// A sketch (2D or 3D).
    Sketch,
    /// A point inside a sketch.
    SketchPoint,
    /// A curve inside a sketch.
    SketchCurve,
    /// A geometric constraint inside a sketch.
    Constraint,
    /// A dimension inside a sketch.
    Dimension,
    /// An equipment body or obstacle.
    Body,
    /// A robot placed in the cell.
    RobotInstance,
    /// A tool mounted on a robot.
    Tool,
    /// A waypoint.
    Waypoint,
    /// A path through waypoints.
    Path,
    /// A work coordinate frame whose origin is a sketch point.
    WorkFrame,
    /// A work plane, axis or point (design feature without volume).
    WorkFeature,
    /// A modelling feature (extrusion of a sketch profile).
    Feature,
    /// An assembly imported from a CAD file (fixed reference).
    Assembly,
    /// The order of the model tree, with suppression and the rollback marker.
    Timeline,
    /// An assembly constraint or joint between two components.
    Relationship,
    /// What a sketch element is for a robot: a waypoint (a point) or a path (a chain of curves).
    SketchRole,
}

/// A value stored in a document under an [`Id`].
pub trait Entity: Clone + PartialEq + 'static {
    /// The kind tag carried by erased ids of this entity.
    const KIND: Kind;

    /// Appends the ids this entity points at (used for referential integrity and delete cascades).
    fn refs(&self, out: &mut Vec<AnyId>);
}

/// Id of a `T`. Mixing ids of different entity types is a compile error.
///
/// Ids are allocated from one counter per document and never reused, so undoing a delete restores the same
/// id. An id serialises as a bare integer.
pub struct Id<T> {
    raw: NonZeroU64,
    kind: PhantomData<fn() -> T>,
}

impl<T> Id<T> {
    /// Wraps a raw value, e.g. one read from a file.
    pub const fn from_raw(raw: NonZeroU64) -> Self {
        Self { raw, kind: PhantomData }
    }

    /// The raw value.
    pub const fn raw(self) -> NonZeroU64 {
        self.raw
    }
}

impl<T: Entity> Id<T> {
    /// Forgets the static type and keeps the kind as a runtime tag.
    pub const fn erase(self) -> AnyId {
        AnyId { kind: T::KIND, raw: self.raw }
    }
}

// Written by hand so that no bound on `T` is required.
impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Id<T> {}
impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}
impl<T> Eq for Id<T> {}
impl<T> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<T> Ord for Id<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.raw.cmp(&other.raw)
    }
}
impl<T> Hash for Id<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}
impl<T> fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.raw)
    }
}
impl<T> Serialize for Id<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.raw.serialize(serializer)
    }
}
impl<'de, T> Deserialize<'de> for Id<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        NonZeroU64::deserialize(deserializer).map(Self::from_raw)
    }
}

/// An id whose entity type is only known at run time: change sets, selection, references.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AnyId {
    /// What the id refers to.
    pub kind: Kind,
    /// The raw value, unique within the document across all kinds.
    pub raw: NonZeroU64,
}

impl AnyId {
    /// Recovers the typed id when the kind matches.
    pub fn downcast<T: Entity>(self) -> Option<Id<T>> {
        (self.kind == T::KIND).then(|| Id::from_raw(self.raw))
    }
}

/// Hands out ids for one document: strictly increasing, never reused.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct IdAlloc {
    next: NonZeroU64,
}

impl Default for IdAlloc {
    fn default() -> Self {
        Self { next: NonZeroU64::MIN }
    }
}

impl IdAlloc {
    /// An allocator whose first id is 1.
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocates the next id.
    pub fn alloc<T>(&mut self) -> Id<T> {
        let id = Id::from_raw(self.next);
        // 2^64 allocations cannot happen in practice; saturating keeps this function total.
        self.next = self.next.saturating_add(1);
        id
    }

    /// The raw value the next allocation will return.
    pub fn peek(&self) -> NonZeroU64 {
        self.next
    }

    /// Makes sure future ids are greater than `used` (called for every id found in a loaded file).
    pub fn reserve_through(&mut self, used: NonZeroU64) {
        self.next = self.next.max(used.saturating_add(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, PartialEq)]
    struct Apple;
    impl Entity for Apple {
        const KIND: Kind = Kind::Body;
        fn refs(&self, _: &mut Vec<AnyId>) {}
    }
    #[derive(Clone, PartialEq)]
    struct Pear;
    impl Entity for Pear {
        const KIND: Kind = Kind::Tool;
        fn refs(&self, _: &mut Vec<AnyId>) {}
    }

    #[test]
    fn ids_increase_and_are_never_reused() {
        let mut alloc = IdAlloc::new();
        let a: Id<Apple> = alloc.alloc();
        let b: Id<Pear> = alloc.alloc();
        assert_eq!(a.raw().get(), 1);
        assert_eq!(b.raw().get(), 2);
        assert_eq!(alloc.peek().get(), 3);
    }

    #[test]
    fn reserve_through_skips_ids_found_in_a_file() {
        let mut alloc = IdAlloc::new();
        alloc.reserve_through(NonZeroU64::new(41).unwrap());
        assert_eq!(alloc.alloc::<Apple>().raw().get(), 42);
        alloc.reserve_through(NonZeroU64::new(7).unwrap());
        assert_eq!(alloc.alloc::<Apple>().raw().get(), 43);
    }

    #[test]
    fn erase_and_downcast_round_trip_only_for_the_same_kind() {
        let mut alloc = IdAlloc::new();
        let apple: Id<Apple> = alloc.alloc();
        let any = apple.erase();
        assert_eq!(any.kind, Kind::Body);
        assert_eq!(any.downcast::<Apple>(), Some(apple));
        assert_eq!(any.downcast::<Pear>(), None);
    }

    #[test]
    fn ids_serialise_as_bare_integers_and_reject_zero() {
        let id: Id<Apple> = Id::from_raw(NonZeroU64::new(5).unwrap());
        assert_eq!(serde_json::to_string(&id).unwrap(), "5");
        assert_eq!(serde_json::from_str::<Id<Apple>>("5").unwrap(), id);
        assert!(serde_json::from_str::<Id<Apple>>("0").is_err());
    }
}
