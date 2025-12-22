//! Public pub/sub API facade.

use alloc::string::String;

use fraktor_actor_rs::core::{serialization::SerializationExtensionGeneric, system::ActorSystemGeneric};
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::SharedAccess};

use crate::core::{
  BatchingProducerConfig, BatchingProducerGeneric, ClusterExtensionGeneric, ClusterPubSubShared, PubSubError,
  PubSubPublisherGeneric, PubSubSubscriber, PubSubTopic, PublishAck, PublishRequest,
};

/// Pub/sub API facade bound to an actor system.
pub struct PubSubApiGeneric<TB: RuntimeToolbox + 'static> {
  system:    ActorSystemGeneric<TB>,
  pub_sub:   ClusterPubSubShared<TB>,
  publisher: PubSubPublisherGeneric<TB>,
}

impl<TB: RuntimeToolbox + 'static> PubSubApiGeneric<TB> {
  /// Retrieves the pub/sub API from an actor system.
  ///
  /// # Errors
  ///
  /// Returns an error if the cluster extension is not installed or serialization is unavailable.
  pub fn try_from_system(system: &ActorSystemGeneric<TB>) -> Result<Self, PubSubError> {
    let extension =
      system.extended().extension_by_type::<ClusterExtensionGeneric<TB>>().ok_or(PubSubError::NotStarted)?;

    let pub_sub = extension.pub_sub_shared();
    let serialization = system
      .extended()
      .extension_by_type::<SerializationExtensionGeneric<TB>>()
      .ok_or(PubSubError::SerializationFailed { reason: String::from("serialization extension not installed") })?;
    let publisher = PubSubPublisherGeneric::new(pub_sub.clone(), serialization.registry());
    Ok(Self { system: system.clone(), pub_sub, publisher })
  }

  /// Subscribes to a topic.
  ///
  /// # Errors
  ///
  /// Returns an error if the subscription fails.
  pub fn subscribe(&self, topic: &PubSubTopic, subscriber: PubSubSubscriber<TB>) -> Result<(), PubSubError> {
    self.pub_sub.with_write(|pub_sub| pub_sub.subscribe(topic, subscriber))
  }

  /// Unsubscribes from a topic.
  ///
  /// # Errors
  ///
  /// Returns an error if the unsubscription fails.
  pub fn unsubscribe(&self, topic: &PubSubTopic, subscriber: PubSubSubscriber<TB>) -> Result<(), PubSubError> {
    self.pub_sub.with_write(|pub_sub| pub_sub.unsubscribe(topic, subscriber))
  }

  /// Publishes a message.
  ///
  /// # Errors
  ///
  /// Returns an error for system-level failures.
  pub fn publish(&self, request: &PublishRequest<TB>) -> Result<PublishAck, PubSubError> {
    self.publisher.publish(request)
  }

  /// Returns a publisher handle.
  #[must_use]
  pub fn publisher(&self) -> PubSubPublisherGeneric<TB> {
    self.publisher.clone()
  }

  /// Creates a batching producer bound to the specified topic.
  ///
  /// # Errors
  ///
  /// Returns an error if the scheduler context is unavailable.
  pub fn batching_producer(
    &self,
    topic: PubSubTopic,
    config: BatchingProducerConfig,
  ) -> Result<BatchingProducerGeneric<TB>, PubSubError> {
    let scheduler = self.system.scheduler_context().ok_or(PubSubError::NotStarted)?.scheduler();
    Ok(BatchingProducerGeneric::new(topic, self.publisher.clone(), scheduler, config))
  }
}
