//! Responses produced by distributed-data delete commands.

use crate::ddata::{Key, ReplicatedData};

/// Response family for a distributed-data delete command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteResponse<D: ReplicatedData, C = ()> {
  /// The delete was accepted and the requested write policy completed.
  Success {
    /// Key that was deleted.
    key:     Key<D>,
    /// Caller-provided request context.
    request: Option<C>,
  },
  /// The delete was accepted locally, but requested replication did not complete in time.
  Timeout {
    /// Key that was deleted.
    key:     Key<D>,
    /// Caller-provided request context.
    request: Option<C>,
  },
  /// The key was already deleted.
  DataDeleted {
    /// Key that was deleted.
    key:     Key<D>,
    /// Caller-provided request context.
    request: Option<C>,
  },
  /// The delete was accepted locally, but durable storage failed.
  StoreFailure {
    /// Key that was deleted.
    key:     Key<D>,
    /// Caller-provided request context.
    request: Option<C>,
  },
}

impl<D: ReplicatedData, C> DeleteResponse<D, C> {
  /// Returns the key associated with this response.
  #[must_use]
  pub const fn key(&self) -> &Key<D> {
    match self {
      | Self::Success { key, .. }
      | Self::Timeout { key, .. }
      | Self::DataDeleted { key, .. }
      | Self::StoreFailure { key, .. } => key,
    }
  }

  /// Returns the request context associated with this response.
  #[must_use]
  pub const fn request(&self) -> Option<&C> {
    match self {
      | Self::Success { request, .. }
      | Self::Timeout { request, .. }
      | Self::DataDeleted { request, .. }
      | Self::StoreFailure { request, .. } => request.as_ref(),
    }
  }

  /// Returns true when this response means that a local tombstone was accepted.
  #[must_use]
  pub const fn is_locally_deleted(&self) -> bool {
    matches!(self, Self::Success { .. } | Self::Timeout { .. } | Self::StoreFailure { .. })
  }
}
