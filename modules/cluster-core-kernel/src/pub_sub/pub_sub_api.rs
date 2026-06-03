//! Public pub/sub API facade.

use alloc::{string::String, vec::Vec};

use fraktor_actor_core_kernel_rs::{serialization::SerializationExtension, system::ActorSystem};
use fraktor_remote_core_rs::address::UniqueAddress;
use fraktor_utils_core_rs::sync::SharedAccess;

use super::{
  BatchingProducer, BatchingProducerConfig, ClusterPubSubShared, MediatorCommand, MediatorCommandOutcome, PubSubError,
  PubSubPublisher, PubSubSubscriber, PubSubTopic, PublishAck, PublishRequest, TopicRegistryApplyOutcome,
  TopicRegistryDelta, TopicRegistryStatus,
};
use crate::ClusterExtension;

/// Pub/sub API facade bound to an actor system.
pub struct PubSubApi {
  system:    ActorSystem,
  pub_sub:   ClusterPubSubShared,
  publisher: PubSubPublisher,
}

impl PubSubApi {
  /// Retrieves the pub/sub API from an actor system.
  ///
  /// # Errors
  ///
  /// Returns an error if the cluster extension is not installed or serialization is unavailable.
  pub fn try_from_system(system: &ActorSystem) -> Result<Self, PubSubError> {
    let extension = system.extended().extension_by_type::<ClusterExtension>().ok_or(PubSubError::NotStarted)?;

    let pub_sub = extension.pub_sub_shared();
    let serialization = system
      .extended()
      .extension_by_type::<SerializationExtension>()
      .ok_or(PubSubError::SerializationFailed { reason: String::from("serialization extension not installed") })?;
    let publisher = PubSubPublisher::new(pub_sub.clone(), serialization.registry());
    Ok(Self { system: system.clone(), pub_sub, publisher })
  }

  /// Subscribes to a topic.
  ///
  /// # Errors
  ///
  /// Returns an error if the subscription fails.
  pub fn subscribe(&self, topic: &PubSubTopic, subscriber: PubSubSubscriber) -> Result<(), PubSubError> {
    self.pub_sub.with_write(|pub_sub| pub_sub.subscribe(topic, subscriber))
  }

  /// Unsubscribes from a topic.
  ///
  /// # Errors
  ///
  /// Returns an error if the unsubscription fails.
  pub fn unsubscribe(&self, topic: &PubSubTopic, subscriber: PubSubSubscriber) -> Result<(), PubSubError> {
    self.pub_sub.with_write(|pub_sub| pub_sub.unsubscribe(topic, subscriber))
  }

  /// Publishes a message.
  ///
  /// # Errors
  ///
  /// Returns an error for system-level failures.
  pub fn publish(&self, request: &PublishRequest) -> Result<PublishAck, PubSubError> {
    self.publisher.publish(request)
  }

  /// Applies a distributed mediator command through the installed cluster pub/sub implementation.
  ///
  /// # Errors
  ///
  /// Returns an error if the implementation is not started or cannot apply the command.
  pub fn apply_mediator_command(
    &self,
    command: MediatorCommand,
    now_millis: u64,
    active_owners: &[UniqueAddress],
  ) -> Result<MediatorCommandOutcome, PubSubError> {
    self.pub_sub.with_write(|pub_sub| pub_sub.apply_mediator_command(command, now_millis, active_owners))
  }

  /// Returns this node's mediator registry status for gossip.
  #[must_use]
  pub fn mediator_status(&self) -> TopicRegistryStatus {
    self.pub_sub.with_read(|pub_sub| pub_sub.mediator_status())
  }

  /// Records a peer mediator registry status observed through gossip.
  pub fn record_mediator_peer_status(&self, owner: UniqueAddress, status: TopicRegistryStatus) {
    self.pub_sub.with_write(|pub_sub| pub_sub.record_mediator_peer_status(owner, status));
  }

  /// Collects mediator registry delta entries newer than the peer status.
  #[must_use]
  pub fn collect_mediator_delta(&self, peer_status: &TopicRegistryStatus) -> TopicRegistryDelta {
    self.pub_sub.with_read(|pub_sub| pub_sub.collect_mediator_delta(peer_status))
  }

  /// Applies mediator registry delta entries observed through gossip.
  #[must_use]
  pub fn apply_mediator_delta(
    &self,
    delta: &TopicRegistryDelta,
    active_owners: &[UniqueAddress],
  ) -> Vec<TopicRegistryApplyOutcome> {
    self.pub_sub.with_write(|pub_sub| pub_sub.apply_mediator_delta(delta, active_owners))
  }

  /// Returns a publisher handle.
  #[must_use]
  pub fn publisher(&self) -> PubSubPublisher {
    self.publisher.clone()
  }

  /// Creates a batching producer bound to the specified topic.
  #[must_use]
  pub fn batching_producer(&self, topic: PubSubTopic, config: BatchingProducerConfig) -> BatchingProducer {
    let scheduler = self.system.state().scheduler();
    BatchingProducer::new(topic, self.publisher.clone(), scheduler, config)
  }
}
