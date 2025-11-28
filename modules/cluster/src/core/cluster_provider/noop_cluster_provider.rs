//! No-op cluster provider for single-node or testing scenarios.

use crate::core::{ClusterProvider, ClusterProviderError};

/// Provider that performs no network operations.
///
/// Useful for tests or single-process runs where membership is predetermined.
#[derive(Default)]
pub struct NoopClusterProvider;

impl NoopClusterProvider {
  /// Creates a new no-op provider.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl ClusterProvider for NoopClusterProvider {
  fn start_member(&mut self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn start_client(&mut self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn shutdown(&mut self, _graceful: bool) -> Result<(), ClusterProviderError> {
    Ok(())
  }
}
