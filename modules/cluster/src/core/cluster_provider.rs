//! ClusterProvider abstraction for membership/discovery backends (no_std).
//!
//! Mirrors protoactor-go's `cluster.ClusterProvider`. Defined in core so that
//! no_std logic can depend on it; std adapters provide concrete transport.

use crate::core::cluster_provider_error::ClusterProviderError;

/// Integrates the cluster runtime with an external membership system.
pub trait ClusterProvider: Send + Sync {
  /// Starts the node as a full cluster member.
  ///
  /// # Errors
  ///
  /// Returns [`ClusterProviderError`] when the provider fails to initialise
  /// membership, discovery, or transport wiring.
  fn start_member(&self) -> Result<(), ClusterProviderError>;

  /// Starts the node as a lightweight cluster client.
  ///
  /// # Errors
  ///
  /// Returns [`ClusterProviderError`] when client initialisation fails.
  fn start_client(&self) -> Result<(), ClusterProviderError>;

  /// Shuts down the provider and releases resources.
  ///
  /// `graceful` indicates whether in-flight operations should be drained before teardown.
  ///
  /// # Errors
  ///
  /// Returns [`ClusterProviderError`] when teardown steps fail.
  fn shutdown(&self, graceful: bool) -> Result<(), ClusterProviderError>;
}
