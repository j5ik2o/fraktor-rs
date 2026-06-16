//! Distributed-data unsubscribe command.

use crate::ddata::{Key, ReplicatedData};

/// Command unregistering a subscriber for change notifications of a CRDT key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsubscribe<D: ReplicatedData, S> {
  key:        Key<D>,
  subscriber: S,
}

impl<D: ReplicatedData, S> Unsubscribe<D, S> {
  /// Creates an unsubscribe command.
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
}
