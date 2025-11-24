//! Abstraction over cluster-wide pub/sub control.

use crate::core::pub_sub_error::PubSubError;

/// Starts and stops the cluster pub/sub subsystem.
pub trait ClusterPubSub: Send + Sync {
  /// Starts pub/sub services.
  fn start(&self) -> Result<(), PubSubError>;

  /// Stops pub/sub services.
  fn stop(&self) -> Result<(), PubSubError>;
}
