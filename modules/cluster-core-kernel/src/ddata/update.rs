//! Distributed-data update command.

#[cfg(test)]
#[path = "update_test.rs"]
mod tests;

use alloc::string::String;

use crate::ddata::{Key, ReplicatedData, ReplicatorEntry, UpdateResponse, UpdateWriteOutcome, WriteConsistency};

/// Command metadata for modifying a CRDT value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Update<D: ReplicatedData, C = ()> {
  key:         Key<D>,
  consistency: WriteConsistency,
  request:     Option<C>,
}

impl<D: ReplicatedData, C> Update<D, C> {
  /// Creates an update command without request context.
  #[must_use]
  pub const fn new(key: Key<D>, consistency: WriteConsistency) -> Self {
    Self { key, consistency, request: None }
  }

  /// Returns an update command with request context.
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

impl<D: ReplicatedData, C: Clone> Update<D, C> {
  /// Evaluates the update command against a local entry snapshot.
  ///
  /// The modify function receives `None` when the entry is missing and `Some(data)` when the
  /// entry is present. Deleted entries reject the modify function without calling it.
  #[must_use]
  pub fn evaluate<F>(
    &self,
    entry: &ReplicatorEntry<D>,
    modify: F,
    outcome: UpdateWriteOutcome,
  ) -> (ReplicatorEntry<D>, UpdateResponse<D, C>)
  where
    F: FnOnce(Option<&D>) -> Result<D, String>, {
    match entry {
      | ReplicatorEntry::Deleted => (ReplicatorEntry::Deleted, UpdateResponse::DataDeleted {
        key:     self.key.clone(),
        request: self.request.clone(),
      }),
      | ReplicatorEntry::Missing | ReplicatorEntry::Present(_) => match modify(entry.data()) {
        | Ok(data) => {
          let response = match outcome {
            | UpdateWriteOutcome::Success => {
              UpdateResponse::Success { key: self.key.clone(), request: self.request.clone() }
            },
            | UpdateWriteOutcome::Timeout => {
              UpdateResponse::Timeout { key: self.key.clone(), request: self.request.clone() }
            },
            | UpdateWriteOutcome::StoreFailure => {
              UpdateResponse::StoreFailure { key: self.key.clone(), request: self.request.clone() }
            },
          };
          (ReplicatorEntry::Present(data), response)
        },
        | Err(message) => (entry.clone(), UpdateResponse::ModifyFailure {
          key: self.key.clone(),
          message,
          request: self.request.clone(),
        }),
      },
    }
  }
}
