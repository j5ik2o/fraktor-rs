//! No-op implementation of the ClusterPubSub trait.

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::{
  ClusterTopology, PubSubError, PubSubSubscriber, PubSubTopic, PublishAck, PublishRequest,
  cluster_pub_sub::ClusterPubSub,
};

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

impl<TB: RuntimeToolbox> ClusterPubSub<TB> for NoopClusterPubSub {
  fn start(&mut self) -> Result<(), PubSubError> {
    Ok(())
  }

  fn stop(&mut self) -> Result<(), PubSubError> {
    Ok(())
  }

  fn subscribe(&mut self, _topic: &PubSubTopic, _subscriber: PubSubSubscriber<TB>) -> Result<(), PubSubError> {
    Ok(())
  }

  fn unsubscribe(&mut self, _topic: &PubSubTopic, _subscriber: PubSubSubscriber<TB>) -> Result<(), PubSubError> {
    Ok(())
  }

  fn publish(&mut self, _request: PublishRequest<TB>) -> Result<PublishAck, PubSubError> {
    Ok(PublishAck::accepted())
  }

  fn on_topology(&mut self, _topology: &ClusterTopology) {}
}
