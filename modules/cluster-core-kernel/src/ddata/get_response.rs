//! Responses produced by distributed-data get commands.

use crate::ddata::{Key, ReplicatedData};

/// Response family for a distributed-data get command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GetResponse<D: ReplicatedData, C = ()> {
  /// The key was found and the value is included.
  Success {
    /// Key that was read.
    key:     Key<D>,
    /// Retrieved CRDT value.
    data:    D,
    /// Caller-provided request context.
    request: Option<C>,
  },
  /// No value exists for the key.
  NotFound {
    /// Key that was read.
    key:     Key<D>,
    /// Caller-provided request context.
    request: Option<C>,
  },
  /// The read could not satisfy the requested consistency level.
  Failure {
    /// Key that was read.
    key:     Key<D>,
    /// Caller-provided request context.
    request: Option<C>,
  },
  /// The key has been deleted.
  DataDeleted {
    /// Key that was read.
    key:     Key<D>,
    /// Caller-provided request context.
    request: Option<C>,
  },
}

impl<D: ReplicatedData, C> GetResponse<D, C> {
  /// Returns the key associated with this response.
  #[must_use]
  pub const fn key(&self) -> &Key<D> {
    match self {
      | Self::Success { key, .. }
      | Self::NotFound { key, .. }
      | Self::Failure { key, .. }
      | Self::DataDeleted { key, .. } => key,
    }
  }

  /// Returns the request context associated with this response.
  #[must_use]
  pub const fn request(&self) -> Option<&C> {
    match self {
      | Self::Success { request, .. }
      | Self::NotFound { request, .. }
      | Self::Failure { request, .. }
      | Self::DataDeleted { request, .. } => request.as_ref(),
    }
  }

  /// Returns the retrieved data when this response is successful.
  #[must_use]
  pub const fn data(&self) -> Option<&D> {
    match self {
      | Self::Success { data, .. } => Some(data),
      | Self::NotFound { .. } | Self::Failure { .. } | Self::DataDeleted { .. } => None,
    }
  }

  /// Returns true when this response carries data.
  #[must_use]
  pub const fn is_success(&self) -> bool {
    matches!(self, Self::Success { .. })
  }
}
