//! Typed ids, id allocation, revisions, tolerances and unit newtypes.
//!
//! Everything here is plain data shared by every other crate. Nothing in this crate knows about geometry,
//! documents or the user interface.

#![forbid(unsafe_code)]

mod hash;
mod id;
mod units;

pub use hash::{StableHash, StableHasher};
pub use id::{AnyId, Entity, Id, IdAlloc, Kind};
pub use units::{Meters, Radians, Tolerance};

use serde::{Deserialize, Serialize};

/// Counts committed changes of a document. Caches remember the revision they were built from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Revision(pub u64);

impl Revision {
    /// The revision after this one.
    #[must_use]
    pub fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }

    /// Advances to the next revision in place.
    pub fn bump(&mut self) {
        *self = self.next();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revision_only_moves_forward() {
        let mut r = Revision::default();
        r.bump();
        assert_eq!(r, Revision(1));
        assert!(r.next() > r);
        assert_eq!(Revision(u64::MAX).next(), Revision(u64::MAX));
    }
}
