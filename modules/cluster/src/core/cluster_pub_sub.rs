//! Abstraction over cluster-wide pub/sub control.

use crate::core::pub_sub_error::PubSubError;

/// Starts and stops the cluster pub/sub subsystem.
pub trait ClusterPubSub: Send + Sync {
  /// Starts pub/sub services.
  ///
  /// # Errors
  ///
  /// Returns an error if the pub/sub subsystem fails to start.
  fn start(&mut self) -> Result<(), PubSubError>;

  /// Stops pub/sub services.
  ///
  /// # Errors
  ///
  /// Returns an error if the pub/sub subsystem fails to stop.
  fn stop(&mut self) -> Result<(), PubSubError>;
}
