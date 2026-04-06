//! Downing strategy abstractions for explicit member down commands.

mod noop_downing_provider;

pub use noop_downing_provider::NoopDowningProvider;

use crate::core::ClusterProviderError;

/// Strategy hook invoked before a member is explicitly downed.
pub trait DowningProvider: Send + Sync {
  /// Handles explicit downing for the given authority.
  ///
  /// # Errors
  ///
  /// Returns [`ClusterProviderError`] when the strategy rejects the down command.
  fn down(&mut self, authority: &str) -> Result<(), ClusterProviderError>;
}
