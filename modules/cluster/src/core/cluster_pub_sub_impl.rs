//! EventStream-based ClusterPubSub implementation backed by PubSubBroker.

#[cfg(test)]
mod tests;

use alloc::{
  format,
  string::{String, ToString},
  vec::Vec,
};

use fraktor_actor_rs::core::{event_stream::EventStreamGeneric, messaging::AnyMessageGeneric};
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use crate::core::{
  ClusterEvent, ClusterPubSub, KindRegistry, PubSubBroker, PubSubError, PubSubEvent, StartupMode,
  kind_registry::TOPIC_ACTOR_KIND,
};

/// PubSubBroker-backed ClusterPubSub implementation with EventStream integration.
///
/// This implementation requires TopicActorKind to be registered in the KindRegistry
/// before starting. On start, it creates the topic for TopicActorKind and publishes
/// events to EventStream.
pub struct ClusterPubSubImpl<TB: RuntimeToolbox + 'static> {
  event_stream:         ArcShared<EventStreamGeneric<TB>>,
  broker:               PubSubBroker,
  has_topic_actor_kind: bool,
  started:              bool,
  advertised_address:   String,
}

impl<TB: RuntimeToolbox + 'static> ClusterPubSubImpl<TB> {
  /// Creates a new PubSubImpl with EventStream and KindRegistry reference.
  ///
  /// The KindRegistry is checked for TopicActorKind presence at construction time.
  #[must_use]
  pub fn new(event_stream: ArcShared<EventStreamGeneric<TB>>, registry: &KindRegistry) -> Self {
    let has_topic_actor_kind = registry.contains(TOPIC_ACTOR_KIND);
    Self {
      event_stream,
      broker: PubSubBroker::new(),
      has_topic_actor_kind,
      started: false,
      advertised_address: String::from("pubsub"),
    }
  }

  /// Creates a new PubSubImpl with a custom advertised address.
  #[must_use]
  pub fn with_advertised_address(mut self, address: impl Into<String>) -> Self {
    self.advertised_address = address.into();
    self
  }

  /// Subscribes to a topic.
  ///
  /// # Errors
  ///
  /// Returns an error if the topic does not exist or if the subscription fails.
  pub fn subscribe(&mut self, topic: &str, subscriber: impl Into<String>) -> Result<(), PubSubError> {
    if !self.started {
      return Err(PubSubError::TopicNotFound { topic: topic.to_string() });
    }
    let result = self.broker.subscribe(topic, subscriber.into());
    self.flush_broker_events_to_stream();
    result
  }

  /// Publishes to a topic and returns subscribers.
  ///
  /// # Errors
  ///
  /// Returns an error if the topic does not exist or publish fails.
  pub fn publish(&mut self, topic: &str) -> Result<Vec<String>, PubSubError> {
    let result = self.broker.publish(topic);
    self.flush_broker_events_to_stream();
    result
  }

  /// Drains broker events (for testing).
  #[must_use]
  pub fn drain_events(&mut self) -> Vec<PubSubEvent> {
    self.broker.drain_events()
  }

  fn flush_broker_events_to_stream(&mut self) {
    let events = self.broker.drain_events();
    for event in events {
      self.publish_pubsub_event(event);
    }
  }

  fn publish_pubsub_event(&self, event: PubSubEvent) {
    let payload = AnyMessageGeneric::new(event);
    let stream_event = fraktor_actor_rs::core::event_stream::EventStreamEvent::Extension {
      name: String::from("cluster-pubsub"),
      payload,
    };
    self.event_stream.publish(&stream_event);
  }

  fn publish_cluster_event(&self, event: ClusterEvent) {
    let payload = AnyMessageGeneric::new(event);
    let stream_event =
      fraktor_actor_rs::core::event_stream::EventStreamEvent::Extension { name: String::from("cluster"), payload };
    self.event_stream.publish(&stream_event);
  }
}

impl<TB: RuntimeToolbox + 'static> ClusterPubSub for ClusterPubSubImpl<TB> {
  fn start(&mut self) -> Result<(), PubSubError> {
    // TopicActorKind がなければ起動失敗
    if !self.has_topic_actor_kind {
      let reason = format!("TopicActorKind '{}' is not registered in KindRegistry", TOPIC_ACTOR_KIND);
      self.publish_cluster_event(ClusterEvent::StartupFailed {
        address: self.advertised_address.clone(),
        mode:    StartupMode::Member,
        reason:  reason.clone(),
      });
      return Err(PubSubError::TopicNotFound { topic: reason });
    }

    // prototopic トピックを作成
    let result = self.broker.create_topic(TOPIC_ACTOR_KIND.to_string());
    self.flush_broker_events_to_stream();

    // 重複時はエラーだが起動は成功とみなす
    match result {
      | Ok(()) | Err(PubSubError::TopicAlreadyExists { .. }) => {
        self.started = true;
        Ok(())
      },
      | Err(e) => {
        self.publish_cluster_event(ClusterEvent::StartupFailed {
          address: self.advertised_address.clone(),
          mode:    StartupMode::Member,
          reason:  format!("{e:?}"),
        });
        Err(e)
      },
    }
  }

  fn stop(&mut self) -> Result<(), PubSubError> {
    self.started = false;
    Ok(())
  }
}
