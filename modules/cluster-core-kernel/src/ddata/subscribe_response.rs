//! Responses produced for distributed-data subscriptions.

use crate::ddata::{Key, ReplicatedData};

/// Event family delivered to subscribers of a distributed-data key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubscribeResponse<D: ReplicatedData> {
  /// The subscribed value changed.
  Changed {
    /// Key whose value changed.
    key:  Key<D>,
    /// New CRDT value.
    data: D,
  },
  /// The subscribed key was deleted.
  Deleted {
    /// Key that was deleted.
    key: Key<D>,
  },
}

impl<D: ReplicatedData> SubscribeResponse<D> {
  /// Returns the key associated with this event.
  #[must_use]
  pub const fn key(&self) -> &Key<D> {
    match self {
      | Self::Changed { key, .. } | Self::Deleted { key } => key,
    }
  }

  /// Returns the changed data when this event carries a value.
  #[must_use]
  pub const fn data(&self) -> Option<&D> {
    match self {
      | Self::Changed { data, .. } => Some(data),
      | Self::Deleted { .. } => None,
    }
  }
}
