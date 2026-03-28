//! Public pub/sub API facade.

use alloc::string::String;

use fraktor_actor_rs::core::kernel::{serialization::SerializationExtension, system::ActorSystem};
use fraktor_utils_rs::core::sync::SharedAccess;

use super::{
  BatchingProducer, BatchingProducerConfig, ClusterPubSubShared, PubSubError, PubSubPublisher, PubSubSubscriber,
  PubSubTopic, PublishAck, PublishRequest,
};
use crate::core::ClusterExtension;

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
