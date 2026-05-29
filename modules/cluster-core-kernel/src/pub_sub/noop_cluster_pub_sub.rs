//! No-op implementation of the ClusterPubSub trait.

use super::{PubSubError, PubSubSubscriber, PubSubTopic, PublishAck, PublishRequest, cluster_pub_sub::ClusterPubSub};
use crate::TopologyUpdate;

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

  fn subscribe(&mut self, _topic: &PubSubTopic, _subscriber: PubSubSubscriber) -> Result<(), PubSubError> {
    Ok(())
  }

  fn unsubscribe(&mut self, _topic: &PubSubTopic, _subscriber: PubSubSubscriber) -> Result<(), PubSubError> {
    Ok(())
  }

  fn publish(&mut self, _request: PublishRequest) -> Result<PublishAck, PubSubError> {
    Ok(PublishAck::accepted())
  }

  fn on_topology(&mut self, _update: &TopologyUpdate) {}
}
