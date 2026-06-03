//! No-op implementation of the ClusterPubSub trait.

#[cfg(test)]
#[path = "noop_cluster_pub_sub_test.rs"]
mod tests;

use fraktor_remote_core_rs::address::UniqueAddress;

use super::{
  MediatorCommand, MediatorCommandOutcome, PubSubError, PubSubSubscriber, PubSubTopic, PublishAck, PublishRequest,
  cluster_pub_sub::ClusterPubSub,
};
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

  fn apply_mediator_command(
    &mut self,
    _command: MediatorCommand,
    _now_millis: u64,
    _active_owners: &[UniqueAddress],
  ) -> Result<MediatorCommandOutcome, PubSubError> {
    Ok(MediatorCommandOutcome::Noop)
  }

  fn on_topology(&mut self, _update: &TopologyUpdate) {}
}
