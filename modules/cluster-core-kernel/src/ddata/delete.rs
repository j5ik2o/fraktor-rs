//! Distributed-data delete command.

#[cfg(test)]
#[path = "delete_test.rs"]
mod tests;

use crate::ddata::{DeleteResponse, DeleteWriteOutcome, Key, ReplicatedData, ReplicatorEntry, WriteConsistency};

/// Command requesting deletion of a CRDT value for a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delete<D: ReplicatedData, C = ()> {
  key:         Key<D>,
  consistency: WriteConsistency,
  request:     Option<C>,
}

impl<D: ReplicatedData, C> Delete<D, C> {
  /// Creates a delete command without request context.
  #[must_use]
  pub const fn new(key: Key<D>, consistency: WriteConsistency) -> Self {
    Self { key, consistency, request: None }
  }

  /// Returns a delete command with request context.
  #[must_use]
  pub fn with_request(mut self, request: C) -> Self {
    self.request = Some(request);
    self
  }

  /// Returns the requested key.
  #[must_use]
  pub const fn key(&self) -> &Key<D> {
    &self.key
  }

  /// Returns the write consistency level.
  #[must_use]
  pub const fn consistency(&self) -> WriteConsistency {
    self.consistency
  }

  /// Returns the request context.
  #[must_use]
  pub const fn request(&self) -> Option<&C> {
    self.request.as_ref()
  }
}

impl<D: ReplicatedData, C: Clone> Delete<D, C> {
  /// Evaluates the delete command against a local entry snapshot.
  ///
  /// A missing key still becomes a deleted entry so later operations observe the tombstone.
  #[must_use]
  pub fn evaluate(
    &self,
    entry: &ReplicatorEntry<D>,
    outcome: DeleteWriteOutcome,
  ) -> (ReplicatorEntry<D>, DeleteResponse<D, C>) {
    if entry.is_deleted() {
      return (ReplicatorEntry::Deleted, DeleteResponse::DataDeleted {
        key:     self.key.clone(),
        request: self.request.clone(),
      });
    }

    let response = match outcome {
      | DeleteWriteOutcome::Success => {
        DeleteResponse::Success { key: self.key.clone(), request: self.request.clone() }
      },
      | DeleteWriteOutcome::Timeout => {
        DeleteResponse::ReplicationFailure { key: self.key.clone(), request: self.request.clone() }
      },
      | DeleteWriteOutcome::StoreFailure => {
        DeleteResponse::StoreFailure { key: self.key.clone(), request: self.request.clone() }
      },
    };
    (ReplicatorEntry::Deleted, response)
  }
}
