//! Distributed-data subscribe command.

#[cfg(test)]
#[path = "subscribe_test.rs"]
mod tests;

use crate::ddata::{Key, ReplicatedData, SubscribeResponse};

/// Command registering a subscriber for change notifications of a CRDT key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscribe<D: ReplicatedData, S> {
  key:        Key<D>,
  subscriber: S,
}

impl<D: ReplicatedData, S> Subscribe<D, S> {
  /// Creates a subscribe command.
  #[must_use]
  pub const fn new(key: Key<D>, subscriber: S) -> Self {
    Self { key, subscriber }
  }

  /// Returns the subscribed key.
  #[must_use]
  pub const fn key(&self) -> &Key<D> {
    &self.key
  }

  /// Returns the subscriber identifier.
  #[must_use]
  pub const fn subscriber(&self) -> &S {
    &self.subscriber
  }

  /// Builds a changed event for this subscription.
  #[must_use]
  pub fn changed(&self, data: D) -> SubscribeResponse<D> {
    SubscribeResponse::Changed { key: self.key.clone(), data }
  }

  /// Builds a deleted event for this subscription.
  #[must_use]
  pub fn deleted(&self) -> SubscribeResponse<D> {
    SubscribeResponse::Deleted { key: self.key.clone() }
  }
}
