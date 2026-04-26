//! Durable state operation errors.

#[cfg(test)]
mod tests;

use alloc::string::String;
use core::fmt::{Display, Formatter, Result as FmtResult};

/// Errors returned by durable state store operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DurableStateError {
  /// Failed to load a durable state object.
  GetObjectFailed(String),
  /// Failed to persist a durable state object.
  UpsertObjectFailed(String),
  /// Failed to delete a durable state object.
  DeleteObjectFailed(String),
  /// Failed to read durable state updates.
  ChangesFailed(String),
  /// Durable state provider identifier already exists.
  ProviderAlreadyRegistered(String),
  /// Durable state provider identifier was not found.
  ProviderNotFound(String),
}

impl DurableStateError {
  /// Creates a provider duplicate error.
  #[must_use]
  pub fn provider_already_registered(id: impl Into<String>) -> Self {
    Self::ProviderAlreadyRegistered(id.into())
  }

  /// Creates a provider not-found error.
  #[must_use]
  pub fn provider_not_found(id: impl Into<String>) -> Self {
    Self::ProviderNotFound(id.into())
  }
}

impl Display for DurableStateError {
  fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
    match self {
      | Self::GetObjectFailed(reason) => write!(formatter, "get durable state object failed: {}", reason),
      | Self::UpsertObjectFailed(reason) => write!(formatter, "upsert durable state object failed: {}", reason),
      | Self::DeleteObjectFailed(reason) => write!(formatter, "delete durable state object failed: {}", reason),
      | Self::ChangesFailed(reason) => write!(formatter, "durable state changes failed: {}", reason),
      | Self::ProviderAlreadyRegistered(id) => write!(formatter, "durable state provider '{}' already exists", id),
      | Self::ProviderNotFound(id) => write!(formatter, "durable state provider '{}' not found", id),
    }
  }
}
