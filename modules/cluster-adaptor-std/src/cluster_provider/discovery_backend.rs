//! Generic discovery backend execution contract.

use std::{string::String, vec::Vec};

use super::DiscoveryBackendError;

/// Backend that produces provider-neutral authority snapshots.
pub trait DiscoveryBackend {
  /// Returns the backend identity used for observability.
  fn source_identity(&self) -> &str;

  /// Discovers the next authority snapshot from polling or subscription input.
  ///
  /// # Errors
  ///
  /// Returns [`DiscoveryBackendError`] when the backend cannot produce an authority snapshot.
  fn discover(&mut self) -> Result<Vec<String>, DiscoveryBackendError>;
}
