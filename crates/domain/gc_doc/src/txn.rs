//! Transactions: the only way to change a document.

use gc_core::{AnyId, Entity, Id, IdAlloc};

use crate::document::{ChangeSet, DocDelta, Document, Table};

/// Why a transaction step or commit failed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DocError {
    /// No entity with this id exists.
    #[error("no such entity: {0:?}")]
    Missing(AnyId),
    /// After the change, `.0` would refer to `.1`, which does not exist.
    #[error("{0:?} refers to {1:?}, which does not exist")]
    Dangling(AnyId, AnyId),
}

/// An open change to a document. Commit it, or drop it to undo everything it did.
pub struct Txn<'d> {
    doc: &'d mut Document,
    delta: DocDelta,
    finished: bool,
}

impl<'d> Txn<'d> {
    /// Opens a transaction.
    pub fn new(doc: &'d mut Document) -> Self {
        Self { doc, delta: DocDelta::default(), finished: false }
    }

    /// The document as it is now, including this transaction's changes.
    pub fn doc(&self) -> &Document {
        self.doc
    }

    /// Adds an entity under a new id.
    pub fn insert<T: Entity>(&mut self, value: T) -> Id<T>
    where
        Document: Table<T>,
    {
        let id = self.doc.ids.alloc();
        <Document as Table<T>>::delta_mut(&mut self.delta).images.entry(id).or_insert(None);
        <Document as Table<T>>::store_mut(self.doc).items.insert(id, value);
        id
    }

    /// Changes an entity. The closure also gets the id allocator, for entities that contain ids of their
    /// own (a sketch allocates ids for its points and curves).
    pub fn modify<T: Entity, R>(
        &mut self,
        id: Id<T>,
        edit: impl FnOnce(&mut T, &mut IdAlloc) -> R,
    ) -> Result<R, DocError>
    where
        Document: Table<T>,
    {
        // The closure works on a copy of the allocator while the store is borrowed. Should it panic, the
        // document keeps its own allocator and the transaction rolls the entities back.
        let mut ids = self.doc.ids.clone();
        let result = match <Document as Table<T>>::store_mut(self.doc).items.get_mut(&id) {
            Some(item) => {
                <Document as Table<T>>::delta_mut(&mut self.delta)
                    .images
                    .entry(id)
                    .or_insert_with(|| Some(item.clone()));
                Ok(edit(item, &mut ids))
            }
            None => Err(DocError::Missing(id.erase())),
        };
        self.doc.ids = ids;
        result
    }

    /// Removes an entity and returns it.
    pub fn remove<T: Entity>(&mut self, id: Id<T>) -> Result<T, DocError>
    where
        Document: Table<T>,
    {
        let removed = <Document as Table<T>>::store_mut(self.doc)
            .items
            .remove(&id)
            .ok_or(DocError::Missing(id.erase()))?;
        <Document as Table<T>>::delta_mut(&mut self.delta)
            .images
            .entry(id)
            .or_insert_with(|| Some(removed.clone()));
        Ok(removed)
    }

    /// Checks that no entity refers to a missing one, then makes the changes permanent.
    /// On failure everything is rolled back.
    pub fn commit(mut self) -> Result<(DocDelta, ChangeSet), DocError> {
        if let Some((from, to)) = self.doc.references().into_iter().find(|(_, to)| !self.doc.contains(*to)) {
            return Err(DocError::Dangling(from, to)); // dropping `self` rolls back
        }
        self.finished = true;
        let delta = std::mem::take(&mut self.delta);
        let changed = delta.touched();
        if !delta.is_empty() {
            self.doc.revision.bump();
        }
        Ok((delta, changed))
    }
}

impl Drop for Txn<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.delta.swap_all(self.doc);
        }
    }
}
