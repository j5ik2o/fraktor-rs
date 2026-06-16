//! Distributed-data get command.

#[cfg(test)]
#[path = "get_test.rs"]
mod tests;

use crate::ddata::{GetResponse, Key, ReadConsistency, ReplicatedData, ReplicatorEntry};

/// Command requesting a CRDT value for a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Get<D: ReplicatedData, C = ()> {
  key:         Key<D>,
  consistency: ReadConsistency,
  request:     Option<C>,
}

impl<D: ReplicatedData, C> Get<D, C> {
  /// Creates a get command without request context.
  #[must_use]
  pub const fn new(key: Key<D>, consistency: ReadConsistency) -> Self {
    Self { key, consistency, request: None }
  }

  /// Returns a get command with request context.
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

  /// Returns the read consistency level.
  #[must_use]
  pub const fn consistency(&self) -> ReadConsistency {
    self.consistency
  }

  /// Returns the request context.
  #[must_use]
  pub const fn request(&self) -> Option<&C> {
    self.request.as_ref()
  }
}

impl<D: ReplicatedData, C: Clone> Get<D, C> {
  /// Evaluates the get command against a local entry snapshot.
  #[must_use]
  pub fn respond_from(&self, entry: &ReplicatorEntry<D>) -> GetResponse<D, C> {
    match entry {
      | ReplicatorEntry::Missing => GetResponse::NotFound { key: self.key.clone(), request: self.request.clone() },
      | ReplicatorEntry::Present(data) => {
        GetResponse::Success { key: self.key.clone(), data: data.clone(), request: self.request.clone() }
      },
      | ReplicatorEntry::Deleted => {
        GetResponse::DataDeleted { key: self.key.clone(), request: self.request.clone() }
      },
    }
  }

  /// Builds a consistency failure response for this command.
  #[must_use]
  pub fn failure(&self) -> GetResponse<D, C> {
    GetResponse::Failure { key: self.key.clone(), request: self.request.clone() }
  }
}
