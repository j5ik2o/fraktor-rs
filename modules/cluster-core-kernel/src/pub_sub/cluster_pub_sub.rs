//! Abstraction over cluster-wide pub/sub control.

use alloc::vec::Vec;

use fraktor_remote_core_rs::address::UniqueAddress;

use super::{
  DistributedPubSubConfig, MediatorCommand, MediatorCommandOutcome, PubSubError, PubSubSubscriber, PubSubTopic,
  PublishAck, PublishRequest, TopicRegistryApplyOutcome, TopicRegistryDelta, TopicRegistryStatus,
};
use crate::TopologyUpdate;

mod cluster_pub_sub_impl;

pub use cluster_pub_sub_impl::ClusterPubSubImpl;

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

  /// Subscribes to a topic.
  ///
  /// # Errors
  ///
  /// Returns an error if the subscription fails.
  fn subscribe(&mut self, topic: &PubSubTopic, subscriber: PubSubSubscriber) -> Result<(), PubSubError>;

  /// Unsubscribes from a topic.
  ///
  /// # Errors
  ///
  /// Returns an error if the unsubscription fails.
  fn unsubscribe(&mut self, topic: &PubSubTopic, subscriber: PubSubSubscriber) -> Result<(), PubSubError>;

  /// Publishes to a topic and returns acknowledgement.
  ///
  /// # Errors
  ///
  /// Returns an error only for system-level failures.
  fn publish(&mut self, request: PublishRequest) -> Result<PublishAck, PubSubError>;

  /// Returns distributed mediator configuration used by this pub/sub implementation.
  fn mediator_config(&self) -> DistributedPubSubConfig {
    DistributedPubSubConfig::default()
  }

  /// Returns this node's mediator registry status for gossip.
  fn mediator_status(&self) -> TopicRegistryStatus {
    TopicRegistryStatus::default()
  }

  /// Records a peer mediator registry status observed through gossip.
  fn record_mediator_peer_status(&mut self, _owner: UniqueAddress, _status: TopicRegistryStatus) {}

  /// Collects mediator registry delta entries newer than the peer status.
  fn collect_mediator_delta(&self, _peer_status: &TopicRegistryStatus) -> TopicRegistryDelta {
    TopicRegistryDelta::default()
  }

  /// Applies mediator registry delta entries observed through gossip.
  fn apply_mediator_delta(
    &mut self,
    _delta: &TopicRegistryDelta,
    _active_owners: &[UniqueAddress],
  ) -> Vec<TopicRegistryApplyOutcome> {
    Vec::new()
  }

  /// Applies a distributed mediator command.
  ///
  /// # Errors
  ///
  /// Returns [`PubSubError::NotStarted`] when the implementation does not expose a mediator state.
  fn apply_mediator_command(
    &mut self,
    _command: MediatorCommand,
    _now_millis: u64,
    _active_owners: &[UniqueAddress],
  ) -> Result<MediatorCommandOutcome, PubSubError> {
    Err(PubSubError::NotStarted)
  }

  /// Applies a topology update to refresh routing decisions.
  fn on_topology(&mut self, update: &TopologyUpdate);
}
