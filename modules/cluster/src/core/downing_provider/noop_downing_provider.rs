//! No-op downing strategy implementation.

#[cfg(test)]
mod tests;

use super::DowningProvider;
use crate::core::ClusterProviderError;

/// Downing strategy that accepts all down commands without side effects.
#[derive(Default)]
pub struct NoopDowningProvider;

impl NoopDowningProvider {
  /// Creates a new no-op downing strategy.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl DowningProvider for NoopDowningProvider {
  fn down(&mut self, _authority: &str) -> Result<(), ClusterProviderError> {
    Ok(())
  }
}
