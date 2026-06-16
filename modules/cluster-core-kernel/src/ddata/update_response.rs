//! Responses produced by distributed-data update commands.

use alloc::string::String;

use crate::ddata::{Key, ReplicatedData};

/// Response family for a distributed-data update command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateResponse<D: ReplicatedData, C = ()> {
  /// The update was accepted and the requested write policy completed.
  Success {
    /// Key that was updated.
    key:     Key<D>,
    /// Caller-provided request context.
    request: Option<C>,
  },
  /// The update was accepted locally, but requested replication did not complete in time.
  Timeout {
    /// Key that was updated.
    key:     Key<D>,
    /// Caller-provided request context.
    request: Option<C>,
  },
  /// The key has been deleted and the update was rejected.
  DataDeleted {
    /// Key that was updated.
    key:     Key<D>,
    /// Caller-provided request context.
    request: Option<C>,
  },
  /// The modify function failed before the update could be accepted.
  ModifyFailure {
    /// Key that was updated.
    key:     Key<D>,
    /// Failure message produced by the modify function.
    message: String,
    /// Caller-provided request context.
    request: Option<C>,
  },
  /// The update was accepted locally, but durable storage failed.
  StoreFailure {
    /// Key that was updated.
    key:     Key<D>,
    /// Caller-provided request context.
    request: Option<C>,
  },
}

impl<D: ReplicatedData, C> UpdateResponse<D, C> {
  /// Returns the key associated with this response.
  #[must_use]
  pub const fn key(&self) -> &Key<D> {
    match self {
      | Self::Success { key, .. }
      | Self::Timeout { key, .. }
      | Self::DataDeleted { key, .. }
      | Self::ModifyFailure { key, .. }
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
      | Self::ModifyFailure { request, .. }
      | Self::StoreFailure { request, .. } => request.as_ref(),
    }
  }

  /// Returns the modify failure message, when available.
  #[must_use]
  pub fn message(&self) -> Option<&str> {
    match self {
      | Self::ModifyFailure { message, .. } => Some(message),
      | Self::Success { .. } | Self::Timeout { .. } | Self::DataDeleted { .. } | Self::StoreFailure { .. } => None,
    }
  }

  /// Returns true when this response means that a new local value was accepted.
  #[must_use]
  pub const fn is_locally_applied(&self) -> bool {
    matches!(self, Self::Success { .. } | Self::Timeout { .. } | Self::StoreFailure { .. })
  }
}
