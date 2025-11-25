//! No-op implementation of the ClusterPubSub trait.

use crate::core::{cluster_pub_sub::ClusterPubSub, pub_sub_error::PubSubError};

/// A no-op pub/sub that does nothing.
///
/// This implementation is useful for testing, single-node clusters,
/// or scenarios where pub/sub is not required.
#[derive(Clone, Debug, Default)]
pub struct NoopClusterPubSub;

impl NoopClusterPubSub {
  /// Creates a new no-op pub/sub.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl ClusterPubSub for NoopClusterPubSub {
  fn start(&mut self) -> Result<(), PubSubError> {
    Ok(())
  }

  fn stop(&mut self) -> Result<(), PubSubError> {
    Ok(())
  }
}
